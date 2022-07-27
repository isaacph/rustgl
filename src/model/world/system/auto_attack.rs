use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};
use crate::model::{world::{World, character::{CharacterID, CharacterType, CharacterIDRange}, commands::WorldCommand, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer, CharacterFlip}, WorldInfo, ErrLog}, commands::GetCommandID};
use self::fsm::Fsm;

use super::{movement::walk_to, projectile::{self, ProjectileCreationInfo}};

pub mod fsm;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackExecution {
    pub timer: f32,
    pub starting_attack_speed: f32,
    pub target: CharacterID,
    pub projectile_gen_id: CharacterID,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackTargeting {
    pub target: CharacterID,
    pub ids: CharacterIDRange,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttack {
    pub timer: f32, // cooldown
    pub execution: Option<AutoAttackExecution>, // currently executing attack
    pub targeting: Option<AutoAttackTargeting>, // currently scheduled string of attacks
}

impl GetComponentID for AutoAttack {
    const ID: ComponentID = ComponentID::AutoAttack;
}

#[derive(Clone)]
pub struct AutoAttackInfo {
    pub fsm: Fsm<AutoAttackPhase, AutoAttackFireEvent>,
    pub projectile_offset: Vector3<f32>,
    pub projectile_speed: f32,
}

impl AutoAttackInfo {
    // first 3 times are durations, last time is fixed point after start
    // if last time is greater than the sum of the first 3, then it will be reduced to the sum of
    // the first 3
    // all params are normalized to the sum of the first 3
    pub fn init(
        ctype: CharacterType,
        wind_up_time: f32,
        casting_time: f32,
        wind_down_time: f32,
        fire_time: f32,
        projectile_speed: f32,
        projectile_offset: Vector3<f32>
    ) -> Result<Self, WorldError> {
        Ok(AutoAttackInfo {
            fsm: Fsm::new(vec![
                (wind_up_time, AutoAttackPhase::WindUp),
                (casting_time, AutoAttackPhase::Casting),
                (wind_down_time, AutoAttackPhase::WindDown)
            ],
            AutoAttackPhase::Complete,
            &[(fire_time, AutoAttackFireEvent)])
                .ok_or(WorldError::InvalidComponentInfo(ctype, ComponentID::AutoAttack))?,
            projectile_offset,
            projectile_speed
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoAttackPhase {
    WindUp,
    Casting,
    WindDown,
    Complete
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct AutoAttackFireEvent;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackRequest {
    pub attacker: CharacterID,
    pub target: CharacterID
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackCommand {
    pub tick: u32,
    pub attacker: CharacterID,
    pub target: CharacterID,
    pub projectile_gen_ids: CharacterIDRange,
}

impl<'a> WorldCommand<'a> for AutoAttackCommand {
    fn get_tick(&self) -> u32 {
        self.tick
    }
    fn validate(&self, world: &World) -> Result<(), WorldError> {
        if self.attacker == self.target {
            return Err(WorldError::InvalidCommand)
        }
        if !world.characters.contains(&self.attacker) {
            return Err(WorldError::MissingCharacter(self.attacker, "Nonexistent character cannot attack".to_string()))
        }
        if !world.characters.contains(&self.target) {
            return Err(WorldError::MissingCharacter(self.target, "Cannot attack nonexistent character".to_string()))
        }
        if !world.base.get_component(&self.target)?.targetable {
            return Err(WorldError::InvalidCommand);
        }
        Ok(())
    }
    fn run(&mut self, world: &mut World) -> Result<(), WorldError> {
        // stop moving
        world.movement.get_component_mut(&self.attacker)?.destination = None;

        // set target (who to attack next even if already doing something else)
        let auto_attack = world.auto_attack.get_component_mut(&self.attacker)?;
        auto_attack.targeting = Some(AutoAttackTargeting {
            target: self.target,
            ids: self.projectile_gen_ids.clone(),
        });

        // check if busy
        let base = world.base.get_component(&self.attacker)?;
        if auto_attack.is_casting(base.ctype, &world.info) {
            return Ok(())
        }
        if auto_attack.timer > 0.0 {
            return Ok(())
        }

        // if not busy, start the attack
        // auto_attack_start(world, &self.attacker, &self.target, &self.projectile_gen_id)?;
        Ok(())
    }
}


impl GetCommandID for AutoAttackCommand {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::AutoAttackCommand
    }
}

impl AutoAttack {
    pub fn new() -> Self {
        Self { execution: None, timer: 0.0, targeting: None }
    }
}

impl GetCommandID for AutoAttackRequest {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::AutoAttackRequest
    }
}

pub fn auto_attack_system_init(_: &mut World) -> Result<(), WorldError> {
    // noop
    Ok(())
}

// fn auto_attack_start(world: &mut World, attacker: &CharacterID, target: &CharacterID, projectile_gen_id: &CharacterID) -> Result<(), WorldError> {
//     let auto_attack = world.auto_attack.get_component_mut(attacker)?;
//     auto_attack.attack = Some(AutoAttackInstance {
//         execution: None,
//         target: *target,
//         projectile_gen_id: *projectile_gen_id,
//     });
//     Ok(())
// }

fn auto_attack_update(world: &mut World, mut delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
    let (ctype, attack_speed) = {
        let base = world.base.get_component(&cid)?;
        (base.ctype, base.attack_speed)
    };
    let attack_info = world.info.auto_attack.get(&ctype)
    .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
    let auto_attack = world.auto_attack.get_component_mut(&cid)?;
    auto_attack.timer -= delta_time;
    
    if let Some(targeting) = &mut auto_attack.targeting {
        let target = targeting.target;
        if !world.characters.contains(&target) || targeting.ids.is_empty() {
            auto_attack.targeting = None;
        } else if auto_attack.execution.is_none() {
            let attacker_range = world.base.get_component_mut(&cid)?.range;

            // preliminary movement for attack
            let target_pos = world.base.get_component_mut(&target)?.position;
            let target_pos = Vector2::new(target_pos.x, target_pos.y);
            if let Some(new_remaining_time) = walk_to(world, &cid, &target_pos, attacker_range, delta_time)? {
                // we are in range, start the auto
                // set the cooldown
                let auto_attack = &mut world.auto_attack.get_component_mut(&cid)?;
                let targeting = auto_attack.targeting.as_mut()
                    .ok_or_else(|| WorldError::UnexpectedComponentState(
                        cid,
                        ComponentID::AutoAttack,
                        "Targeting was removed?".to_string()))?;
                auto_attack.timer = attack_speed - new_remaining_time;

                // start the execution fsm
                auto_attack.execution = Some(AutoAttackExecution {
                    timer: 0.0,
                    starting_attack_speed: attack_speed,
                    target,
                    projectile_gen_id: targeting.ids.next_id()
                        .ok_or_else(|| WorldError::UnexpectedComponentState(
                            cid,
                            ComponentID::AutoAttack,
                            "Ran out of auto attack IDs".to_string()))?
                });
                delta_time = new_remaining_time;
            } else {
                delta_time = 0.0;
            }
        }
    }

    if let Some(execution) = &mut world.auto_attack.get_component_mut(&cid)?.execution {
        // attack fsm
        let target = execution.target;
        let timer = execution.timer;
        let (changes, _changed) = attack_info.fsm.get_state_changes(
                attack_speed,
                execution.timer,
                execution.timer + delta_time);
        for change in changes {
            match change {
                fsm::Changes::Event(time_since, _) => auto_attack_fire(world, &cid, time_since)?,
                fsm::Changes::StateChange(_time_since, phase) => {
                    //println!("New AA phase: {:?}, timer: {}, increment: {}, time_since: {}",
                    //    phase,
                    //    timer,
                    //    delta_time,
                    //    time_since);
                    match phase {
                        AutoAttackPhase::WindUp |
                        AutoAttackPhase::Casting |
                        AutoAttackPhase::WindDown => (),
                        AutoAttackPhase::Complete => {
                            // finish the auto attack
                            world.auto_attack.get_component_mut(&cid)?.execution = None;
                        },
                    }
                },
            }
        }
        match attack_info.fsm.get_current_state(attack_speed, timer) {
            AutoAttackPhase::WindUp |
            AutoAttackPhase::Casting => {
                // cancel if target dies
                if !world.characters.contains(&target) {
                    world.auto_attack.get_component_mut(&cid)?.execution = None;
                }
            },
            _ => (),
        }
        if let Some(execution) = &mut world.auto_attack.get_component_mut(&cid)?.execution {
            execution.timer += delta_time;
        }
    }
    Ok(())
}

pub fn auto_attack_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
    let cids: Vec<CharacterID> = world.auto_attack.components.keys().copied().collect();
    for cid in cids {
        auto_attack_update(world, delta_time, cid).err_log(world);
    }
    Ok(())
}

fn auto_attack_fire(world: &mut World, cid: &CharacterID, time_since_fire: f32) -> Result<(), WorldError> {
    // world.base.get_component_mut(cid)?.position.y -= 1.0;
    // println!("Fire action this long ago: {}, for now we just jump lol", time_since_fire);
    let (ctype, damage) = {
        let base = world.base.get_component(cid)?;
        (base.ctype, base.attack_damage)
    };
    let execution = world.auto_attack.get_component(cid)?.execution.as_ref()
        .ok_or_else(|| WorldError::UnexpectedComponentState(
            *cid,
            ComponentID::AutoAttack,
            "Tried to fire without currently executing auto attack".to_string()))?;
    let gen_id = execution.projectile_gen_id;
    let origin = *cid;
    let target = execution.target;

    // flip towards target
    let target_pos = world.base.get_component(cid)?.position;
    let base = world.base.get_component_mut(cid)?;
    base.flip = CharacterFlip::from_dir(&(Vector2::new(target_pos.x, target_pos.y) - Vector2::new(base.position.x, base.position.y))).unwrap_or(base.flip);

    // make auto attack info
    let aa_info = world.info.auto_attack.get(&ctype)
        .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?;
    let info = ProjectileCreationInfo {
        starting_offset: aa_info.projectile_offset,
        speed: aa_info.projectile_speed,
        damage,
        proj_id: gen_id,
        origin,
        target,
    };
    projectile::create(world, &info, time_since_fire)?;
    Ok(())
}

#[cfg(feature = "server")]
pub mod server {
    use std::net::SocketAddr;
    use crate::{model::{player::{server::PlayerCommand, model::{PlayerID, PlayerDataView}, commands::ChatMessage}, PrintError}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
    use super::{AutoAttackRequest, AutoAttackCommand};

    impl<'a> PlayerCommand<'a> for AutoAttackRequest {
        const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);

        fn run(self, addr: &SocketAddr, player_id: &PlayerID, server: &mut Server) {
            // check if the player can use the requested character
            if server.player_manager.can_use_character(player_id, &self.attacker) {
                let gen_ids = server.character_id_gen.generate_range(1000);
                server.run_world_command(Some(addr), AutoAttackCommand {
                    tick: server.tick,
                    attacker: self.attacker,
                    target: self.target,
                    projectile_gen_ids: gen_ids,
                });
            } else {
                server.connection.send(
                    Protocol::TCP,
                    addr,
                    &ChatMessage("Error: no permission".to_string())
                ).print()
            }
        }
    }
}

impl Default for AutoAttack {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoAttack {
    pub fn is_casting(&self, ctype: CharacterType, info: &WorldInfo) -> bool {
        if let Some(attack_info) = info.auto_attack.get(&ctype) {
            if let Some(execution) = &self.execution {
                let state = attack_info.fsm.get_current_state(execution.starting_attack_speed, execution.timer);
                return state == AutoAttackPhase::Casting
            }
        }
        false
    }
}
