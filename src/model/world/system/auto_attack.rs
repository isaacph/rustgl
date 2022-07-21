use nalgebra::Vector2;
use serde::{Serialize, Deserialize};
use crate::model::{world::{World, character::{CharacterID, CharacterType}, commands::WorldCommand, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer, CharacterBase, CharacterFlip}, WorldInfo, ErrLog}, commands::GetCommandID};
use self::fsm::Fsm;

use super::{movement::move_to, projectile::{self, ProjectileCreationInfo}};

pub mod fsm;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackExecution {
    timer: f32,
    starting_attack_speed: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackInstance {
    pub execution: Option<AutoAttackExecution>,
    pub target: CharacterID,
    pub projectile_gen_id: CharacterID,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttack {
    pub attack: Option<AutoAttackInstance>,
    pub timer: f32,
    pub target: Option<CharacterID>,
}

impl GetComponentID for AutoAttack {
    const ID: ComponentID = ComponentID::AutoAttack;
}

#[derive(Clone)]
pub struct AutoAttackInfo {
    pub fsm: Fsm<AutoAttackPhase, AutoAttackFireEvent>,
    pub projectile_offset: Vector2<f32>,
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
        projectile_offset: Vector2<f32>
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AutoAttackFireEvent;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackRequest {
    pub attacker: CharacterID,
    pub target: CharacterID
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackCommand {
    pub attacker: CharacterID,
    pub target: CharacterID,
    pub projectile_gen_id: CharacterID,
}

impl<'a> WorldCommand<'a> for AutoAttackCommand {
    fn validate(&self, world: &World) -> Result<(), WorldError> {
        if self.attacker == self.target {
            return Err(WorldError::InvalidCommand)
        }
        if !world.characters.contains(&self.attacker) {
            return Err(WorldError::MissingCharacter(self.attacker))
        }
        if !world.characters.contains(&self.target) {
            return Err(WorldError::MissingCharacter(self.target))
        }
        Ok(())
    }
    fn run(&mut self, world: &mut World) -> Result<(), WorldError> {
        // stop moving
        world.movement.get_component_mut(&self.attacker)?.destination = None;

        // set target (who to attack next even if already doing something else)
        let auto_attack = world.auto_attack.get_component_mut(&self.attacker)?;
        auto_attack.target = Some(self.target);

        // check if busy
        let base = world.base.get_component(&self.attacker)?;
        if auto_attack.is_casting(base.ctype, base, &world.info) {
            return Ok(())
        }
        if auto_attack.timer > 0.0 {
            return Ok(())
        }

        // if not busy, start the attack
        auto_attack_start(world, &self.attacker, &self.target, &self.projectile_gen_id)?;
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
        Self { attack: None, timer: 0.0, target: None }
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

fn auto_attack_start(world: &mut World, attacker: &CharacterID, target: &CharacterID, projectile_gen_id: &CharacterID) -> Result<(), WorldError> {
    let auto_attack = world.auto_attack.get_component_mut(attacker)?;
    auto_attack.attack = Some(AutoAttackInstance {
        execution: None,
        target: *target,
        projectile_gen_id: *projectile_gen_id,
    });
    Ok(())
}

fn auto_attack_update(world: &mut World, delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
    let (ctype, attack_speed) = {
        let base = world.base.get_component(&cid)?;
        (base.ctype, base.attack_speed)
    };
    let attack_info = world.info.auto_attack.get(&ctype)
    .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
    let auto_attack = world.auto_attack.get_component_mut(&cid)?;
    auto_attack.timer -= delta_time;
    if let Some(attack) = &auto_attack.attack {
        let target = attack.target;
        let attacker_range = world.base.get_component_mut(&cid)?.range;

        // preliminary movement for attack
        let mut remaining_time = Some(delta_time);
        if attack.execution.is_none() {// attack hasn't started yet, move to attack
            let target_pos = world.base.get_component_mut(&target)?.position;
            if let Some(new_remaining_time) = move_to(world, &cid, &target_pos, attacker_range, remaining_time.unwrap_or(0.0))? {
                // we are in range, start the auto
                // set the cooldown
                let auto_attack = &mut world.auto_attack.get_component_mut(&cid)?;
                auto_attack.timer = attack_speed - new_remaining_time;

                // start the execution fsm
                let attack = (auto_attack.attack).as_mut()
                    .ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
                attack.execution = Some(AutoAttackExecution {
                    timer: 0.0,
                    starting_attack_speed: attack_speed,
                });
                remaining_time = Some(new_remaining_time);
            } else {
                remaining_time = None;
            }
        }

        // attack fsm
        if let Some(remaining_time) = remaining_time {
            if let Some(execution) = world.auto_attack.get_component_mut(&cid)?
                .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                .execution.as_mut() {
                let timer = execution.timer;
                let (changes, _changed) = attack_info.fsm.get_state_changes(
                        attack_speed,
                        execution.timer,
                        execution.timer + remaining_time);
                for change in changes {
                    match change {
                        fsm::Changes::Event(time_since, _) => auto_attack_fire(world, &cid, time_since)?,
                        fsm::Changes::StateChange(time_since, phase) => {
                            println!("New AA phase: {:?}, timer: {}, increment: {}, time_since: {}",
                                phase,
                                timer,
                                remaining_time,
                                time_since);
                            match phase {
                                AutoAttackPhase::WindUp |
                                AutoAttackPhase::Casting |
                                AutoAttackPhase::WindDown => (),
                                AutoAttackPhase::Complete => {
                                    // finish the auto attack
                                    world.auto_attack.get_component_mut(&cid)?.attack = None;
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
                            world.auto_attack.get_component_mut(&cid)?.attack = None;
                        }
                    },
                    _ => (),
                }
                world.auto_attack.get_component_mut(&cid)?
                    .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                    .execution.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                    .timer += remaining_time;
            }
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

pub fn auto_attack_fire(world: &mut World, cid: &CharacterID, time_since_fire: f32) -> Result<(), WorldError> {
    // world.base.get_component_mut(cid)?.position.y -= 1.0;
    // println!("Fire action this long ago: {}, for now we just jump lol", time_since_fire);
    let (ctype, damage) = {
        let base = world.base.get_component(cid)?;
        (base.ctype, base.attack_damage)
    };
    let attack = world.auto_attack.get_component(cid)?.attack.as_ref()
        .ok_or(WorldError::UnexpectedComponentState(*cid, ComponentID::AutoAttack))?;
    let gen_id = attack.projectile_gen_id;
    let origin = *cid;
    let target = attack.target;

    // flip towards target
    let target_pos = world.base.get_component(cid)?.position;
    let base = world.base.get_component_mut(cid)?;
    base.flip = CharacterFlip::from_dir(&(target_pos - base.position)).unwrap_or(base.flip);

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
                let gen_id = server.character_id_gen.generate();
                server.run_world_command(Some(addr), AutoAttackCommand {
                    attacker: self.attacker,
                    target: self.target,
                    projectile_gen_id: gen_id,
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
    pub fn is_casting(&self, ctype: CharacterType, base: &CharacterBase, info: &WorldInfo) -> bool {
        if let Some(attack_info) = info.auto_attack.get(&ctype) {
            if let Some(attack) = &self.attack {
                if let Some(execution) = &attack.execution {
                    return attack_info.fsm.get_current_state(base.attack_speed, execution.timer) == AutoAttackPhase::Casting
                }
            }
        }
        false
    }
}
