use serde::{Serialize, Deserialize};
use crate::model::{world::{World, character::{CharacterID, CharacterType}, commands::WorldCommand, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer}, WorldInfo}, commands::GetCommandID};
use self::fsm::Fsm;

use super::movement::walk_to;

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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttack {
    pub attack: Option<AutoAttackInstance>
}

impl GetComponentID for AutoAttack {
    const ID: ComponentID = ComponentID::AutoAttack;
}

#[derive(Clone)]
pub struct AutoAttackInfo {
    pub fsm: Fsm<AutoAttackPhase, AutoAttackFireEvent>
}

impl AutoAttackInfo {
    // first 3 times are durations, last time is fixed point after start
    // if last time is greater than the sum of the first 3, then it will be reduced to the sum of
    // the first 3
    // all params are normalized to the sum of the first 3
    pub fn init(ctype: CharacterType, wind_up_time: f32, casting_time: f32, wind_down_time: f32, fire_time: f32) -> Result<Self, WorldError> {
        Ok(AutoAttackInfo {
            fsm: Fsm::new(vec![
                (wind_up_time, AutoAttackPhase::WindUp),
                (casting_time, AutoAttackPhase::Casting),
                (wind_down_time, AutoAttackPhase::WindDown)
            ],
            AutoAttackPhase::Complete,
            &[(fire_time, AutoAttackFireEvent)])
                .ok_or(WorldError::InvalidComponentInfo(ctype, ComponentID::AutoAttack))?
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
    pub target: CharacterID
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
        // check if not already attacking the same target
        let base = world.base.get_component(&self.attacker)?;
        let auto_attack = world.auto_attack.get_component(&self.attacker)?;
        if auto_attack.is_casting(base.ctype, &world.info) {
            return Err(WorldError::IllegalInterrupt(self.attacker))
        }
        if let Some(attack) = &auto_attack.attack {
            if attack.target == self.target {
                return Err(WorldError::NoopCommand)
            }
        }
        // todo later: check if not stunned or already casting a separate spell
        // right now it's fine that this interrupts movements
        Ok(())
    }
    fn run(&mut self, world: &mut World) -> Result<(), WorldError> {
        world.movement.get_component_mut(&self.attacker)?.destination = None;
        let auto_attack = world.auto_attack.get_component_mut(&self.attacker)?;
        auto_attack.attack = Some(AutoAttackInstance {
            execution: None,
            target: self.target,
        });
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
        Self { attack: None }
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

pub fn auto_attack_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
    let cids: Vec<CharacterID> = world.auto_attack.components.keys().copied().collect();
    for cid in cids {
        let (ctype, attack_speed) = {
            let base = world.base.get_component(&cid)?;
            (base.ctype, base.attack_speed)
        };
        let attack_info = world.info.auto_attack.get(&ctype)
        .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
        if let Some(attack) = &mut world.auto_attack.get_component_mut(&cid)?.attack {
            let target = attack.target;
            let attacker_range = world.base.get_component_mut(&cid)?.range;
            let target_pos = world.base.get_component_mut(&target)?.position;

            // preliminary movement for attack
            let mut remaining_time = Some(delta_time);
            if attack.execution.is_none() {// attack hasn't started yet, move to attack
                if let Some(new_remaining_time) = walk_to(world, &cid, &target_pos, attacker_range, remaining_time.unwrap_or(0.0))? {
                    // we are in range, start the auto
                    let attack = (&mut world.auto_attack.get_component_mut(&cid)?.attack).as_mut()
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
                    for change in attack_info.fsm.get_state_changes(
                            attack_speed,
                            execution.timer,
                            execution.timer + remaining_time) {
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
                    world.auto_attack.get_component_mut(&cid)?
                        .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                        .execution.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                        .timer += remaining_time;
                }
            }
        }
    }
    Ok(())
}

pub fn auto_attack_fire(world: &mut World, cid: &CharacterID, time_since_fire: f32) -> Result<(), WorldError> {
    world.base.get_component_mut(cid)?.position.y -= 1.0;
    println!("Fire action this long ago: {}, for now we just jump lol", time_since_fire);
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
                server.run_world_command(Some(addr), AutoAttackCommand {
                    attacker: self.attacker,
                    target: self.target
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
            if let Some(attack) = &self.attack {
                if let Some(execution) = &attack.execution {
                    return attack_info.fsm.get_current_state(execution.timer) == AutoAttackPhase::Casting
                }
            }
        }
        false
    }
}
