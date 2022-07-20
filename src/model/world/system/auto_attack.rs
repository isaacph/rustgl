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
//    // put these times as a proportion of the total attack time
//    pub wind_up_time: f32,
//    pub casting_time: f32,
//    pub wind_down_time: f32,
//
//    // time at which to wind_up_time ends at which to fire the projectile
//    // also a proportion of total attack time
//    //   if fire_time <= wind_up_time, then casts during (or at the very end of) wind up phase
//    // elif fire_time <= wind_up_time + casting_time, then casts during (or at the very end of)
//    //       casting phase
//    // elif fire_time <= sum of all 3, then casts during (or at the very end of) the entire
//    //       animation
//    pub fire_time: AutoAttackTiming,
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
        //let (attacker_pos, attacker_range) = {
        //    let base = world.base.get_component_mut(&self.attacker)?;
        //    (base.position, base.range)
        //};
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
        if let Some(attack) = &mut world.auto_attack.get_component_mut(&cid)?.attack {
            let target = attack.target;
            let attack_info = world.info.auto_attack.get(&ctype)
                .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
            let mut remaining_time_next = Some(delta_time);
            let mut count = 0;
            while let Some(remaining_time) = remaining_time_next {
                count += 1;
                if remaining_time <= 0.0 {
                    break;
                }
                if let Some(execution) = world.auto_attack.get_component_mut(&cid)?
                    .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                    .execution.as_mut() {
                    // we reach phase 1
                    let timer = execution.timer;
                    let (changes, changes_state) = attack_info.fsm.get_until_first_state_change(attack_speed, timer, timer + remaining_time);
                    if changes.len() > 0 {
                        println!("{} changes", changes.len());
                    }
                    for change in changes {
                        match change {
                            fsm::Changes::Event(time_since, _) => auto_attack_fire(world, &cid, time_since)?,
                            fsm::Changes::StateChange(time_since, phase) => {
                            println!("New AA phase: {:?}, timer: {}, time_since: {}, count: {}", phase, timer, time_since, count);
                            match phase {
                                AutoAttackPhase::WindUp |
                                AutoAttackPhase::Casting |
                                AutoAttackPhase::WindDown => {
                                    let execution = world.auto_attack.get_component_mut(&cid)?
                                        .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                                        .execution.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
                                    execution.timer += time_since;
                                    remaining_time_next = Some(remaining_time - time_since);
                                },
                                AutoAttackPhase::Complete => {
                                    // finish the auto attack
                                    world.auto_attack.get_component_mut(&cid)?.attack = None;
                                    remaining_time_next = None;
                                },
                            }},
                        }
                    }
                    if !changes_state {
                        let execution = world.auto_attack.get_component_mut(&cid)?
                            .attack.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?
                            .execution.as_mut().ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
                        execution.timer += remaining_time;
                        remaining_time_next = None;
                    }
                    continue;
                } else {
                    // attack hasn't started yet, move to attack
                    let attacker_range = world.base.get_component_mut(&cid)?.range;
                    let target_pos = world.base.get_component_mut(&target)?.position;
                    if let Some(new_remaining_time) = walk_to(world, &cid, &target_pos, attacker_range, remaining_time)? {
                        // we are in range, start the auto
                        let attack = (&mut world.auto_attack.get_component_mut(&cid)?.attack).as_mut()
                            .ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
                        attack.execution = Some(AutoAttackExecution {
                            timer: 0.0,
                            starting_attack_speed: attack_speed,
                        });
                        remaining_time_next = Some(new_remaining_time);
                    } else {
                        break
                    }
                    continue;
                }
            }
        }
        // while let Some(remaining_time) = remaining_time_next {

        // }
    }
    Ok(())
    // make current attacks continue
    // let cids: Vec<CharacterID> = world.auto_attack.components.keys().copied().collect();
    // for cid in cids {
    //     let (ctype, attack_speed) = {
    //         let base = world.base.get_component(&cid)?;
    //         (base.ctype, base.attack_speed)
    //     };
    //     let attack_info = world.info.auto_attack.get(&ctype)
    //         .ok_or(WorldError::MissingCharacterInfoComponent(ctype, Action::AutoAttack))?.clone();
    //     let mut remaining_time = Some(delta_time);
    //     while let Some(remaining_time_val) = remaining_time {
    //         let auto_attack = world.auto_attack.get_component_mut(&cid)?;
    //         let action_queue = world.action_queue.get_component_mut(&cid)?;
    //         let action = Action::AutoAttack;
    //         match action_queue.fix_action(&cid, action, auto_attack.attack.as_mut()) {
    //             Ok(Some((attack, status))) => {
    //                 action_queue.reset_ttl(&cid, action)?;
    //                 if attack.execution.is_none() {
    //                     // attack hasn't started yet, move to attack
    //                     let attacker_range = world.base.get_component_mut(&cid)?.range;
    //                     let target_pos = world.base.get_component_mut(&attack.target)?.position;
    //                     if let Some(new_remaining_time) = walk_to(world, &cid, &target_pos, attacker_range, remaining_time_val)? {
    //                         // we are in range, start the auto
    //                         let attack = (&mut world.auto_attack.get_component_mut(&cid)?.attack).as_mut()
    //                             .ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
    //                         attack.execution = Some(AutoAttackExecution {
    //                             timer: 0.0,
    //                             starting_attack_speed: attack_speed,
    //                         });
    //                         remaining_time = Some(new_remaining_time);
    //                     }
    //                     continue;
    //                 }
    //                 let AutoAttackExecution { timer, starting_attack_speed } = 
    //                         (&mut world.auto_attack.get_component_mut(&cid)?.attack).as_mut()
    //                         .ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?.execution
    //                         .ok_or(WorldError::UnexpectedComponentState(cid, ComponentID::AutoAttack))?;
    //                 let fire_time = attack_info.calc(starting_attack_speed).ok_or(WorldError::InvalidComponentInfo(ctype, action))?.fire;
    //                 let fire_phase = fire_time.get_phase().ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                 let prev_phase = attack_info.get_phase(starting_attack_speed, timer).ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                 let next_phase = attack_info.get_next_phase(prev_phase);
    //                 let next_phase_time = attack_info.get_phase_start(starting_attack_speed, attack_info.get_next_phase(prev_phase));
    //                 let max_next_phase = attack_info.get_phase(starting_attack_speed, timer + remaining_time_val).ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                 // what do we want to happen
    //                 // we need to check whether the action was fired since last frame
    //                 let mut should_fire = false;
    //                 if timer <= fire_time && fire_time < timer + remaining_time_val {
    //                     // fire the attack
    //                     should_fire = true;
    //                     println!("Attack fired");
    //                 }
    //                 // we need to check if the current phase ended
    //                 //   if it ended, we need to subtract the time the phase consumed from
    //                 //   remaining_time
    //                 //   and if it ended, we need to swap out the action priorities
    //                 if next_phase_time < timer + remaining_time_val {
    //                     // current phase ended
    //                     remaining_time = Some(timer - next_phase_time);
    //                     match next_phase {
    //                         AutoAttackPhase::Casting => {
    //                             // swap to the next phase
    //                             // this changes priority and makes the action uninterruptable
    //                             action_queue.swap_action(&world.info, Action::AutoAttack, Action::AutoAttackCasting, 0.0, &cid)?;
    //                         },
    //                         AutoAttackPhase::Complete => {
    //                             // // ensure the ending attack fires - probably not necessary
    //                             // if fire_phase == next_phase {
    //                             //     should_fire = true;
    //                             // }
    //                             // end the attack
    //                             remaining_time = None;
    //                             world.auto_attack.get_component_mut(&cid)?.attack = None;
    //                             break;
    //                         },
    //                         AutoAttackPhase::WindUp |
    //                         AutoAttackPhase::WindDown => {
    //                             world.auto_attack.get_component_mut(&cid)?.attack = None;
    //                             remaining_time = None;
    //                             return Err(WorldError::InvalidAttackPhase(cid, next_phase));
    //                         },
    //                     }
    //                 }

    //                 if should_fire {
    //                     world.base.get_component_mut(&cid)?.position.y -= 1.0;
    //                     println!("Fire action, for now we just jump lol");
    //                 }
    //                 continue;
    //             },
    //             Ok(None) => (), // the action is not current, could be AutoAttackCasting, or could
    //                             // be in queue, if execution is None (else ttl = 0)
    //             Err(WorldError::MissingActionStatus(_, _)) => {
    //                 // action was stopped, stop it here too
    //                 world.auto_attack.get_component_mut(&cid)?.attack = None;
    //                 remaining_time = None;
    //             },
    //             Err(err) => world.errors.push(err)
    //         };
    //         let auto_attack = world.auto_attack.get_component_mut(&cid)?;
    //         match world.action_queue.get_component_mut(&cid)?.fix_action(&cid, Action::AutoAttackCasting, auto_attack.attack.as_mut()) {
    //             Ok(Some((attack, status))) => {
    //                 // action_queue.reset_ttl(&cid, action)?; // shouldn't be necessary since first
    //                 // phase already set ttl to zero
    //                 let timer = attack.timer
    //                     .ok_or(WorldError::UnexpectedActionStatus(cid, Action::AutoAttackCasting, status))?;
    //                 // check if we reach the fire time. if so we fire
    //                     let fire_time = attack_info.calc(starting_attack_speed).ok_or(WorldError::InvalidComponentInfo(ctype, action))?.fire;
    //                     let fire_phase = fire_time.get_phase().ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                     let prev_phase = attack_info.get_phase(starting_attack_speed, timer).ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                     let next_phase = attack_info.get_next_phase(prev_phase);
    //                     let next_phase_time = attack_info.get_phase_start(starting_attack_speed, attack_info.get_next_phase(prev_phase));
    //                     let max_next_phase = attack_info.get_phase(starting_attack_speed, timer + remaining_time_val).ok_or(WorldError::InvalidComponentInfo(ctype, action))?;
    //                     // what do we want to happen
    //                     // we need to check whether the action was fired since last frame
    //                     let mut should_fire = false;
    //                     if timer <= fire_time && fire_time < timer + remaining_time_val {
    //                         // fire the attack
    //                         should_fire = true;
    //                         println!("Attack fired");
    //                     }
    //                     // we need to check if the current phase ended
    //                     //   if it ended, we need to subtract the time the phase consumed from
    //                     //   remaining_time
    //                     //   and if it ended, we need to swap out the action priorities
    //                     if next_phase_time < timer + remaining_time_val {
    //                         // current phase ended
    //                         remaining_time = Some(timer - next_phase_time);
    //                         match next_phase {
    //                             AutoAttackPhase::Casting => {
    //                                 // swap to the next phase
    //                                 // this changes priority and makes the action uninterruptable
    //                                 action_queue.swap_action(&world.info, Action::AutoAttack, Action::AutoAttackCasting, 0.0, &cid)?;
    //                             },
    //                             AutoAttackPhase::Complete => {
    //                                 // // ensure the ending attack fires - probably not necessary
    //                                 // if fire_phase == next_phase {
    //                                 //     should_fire = true;
    //                                 // }
    //                                 // end the attack
    //                                 remaining_time = None;
    //                                 world.auto_attack.get_component_mut(&cid)?.attack = None;
    //                                 break;
    //                             },
    //                             AutoAttackPhase::WindUp |
    //                             AutoAttackPhase::WindDown => {
    //                                 world.auto_attack.get_component_mut(&cid)?.attack = None;
    //                                 remaining_time = None;
    //                                 return Err(WorldError::InvalidAttackPhase(cid, next_phase));
    //                             },
    //                         }
    //                     }

    //                     if should_fire {
    //                         world.base.get_component_mut(&cid)?.position.y -= 1.0;
    //                         println!("Fire action, for now we just jump lol");
    //                     }
    //                     continue;
    //                 // also check if we reach the end of the phase. if so we swap action
    //                 continue;
    //             },
    //             Ok(None) => (), // the action is not current, could be the other action
    //                             // could not be in queue: ttl is set to 0
    //             Err(WorldError::MissingActionStatus(_, _)) => {
    //                 // action was stopped, stop it here too
    //                 world.auto_attack.get_component_mut(&cid)?.attack = None;
    //             },
    //             Err(err) => world.errors.push(err)
    //         };
    //     }
    // }
    // Ok(())
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
