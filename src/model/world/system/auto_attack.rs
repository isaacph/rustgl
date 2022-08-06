use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};
use crate::model::{world::{World, character::{CharacterID, CharacterType, CharacterIDRange}, commands::{CharacterCommand, WorldCommand}, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer, ComponentUpdateData, Component, ComponentUpdate}, WorldInfo, WorldSystem, ComponentSystem, Update}, commands::GetCommandID, WorldTick, TICK_RATE};
use self::fsm::Fsm;

use super::{movement::walk_to, projectile::{self, ProjectileCreationInfo}, base::CharacterFlip, status::{StatusID, StatusPrio, StatusUpdate, Status}};

pub mod fsm;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoAttackExecution {
    pub time_start: WorldTick,
    pub starting_attack_speed: f32,
    pub target: CharacterID,
    pub projectile_gen_id: CharacterID,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoAttackTargeting {
    pub target: CharacterID,
    pub ids: CharacterIDRange,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoAttack {
    pub cooldown_timeout: WorldTick, // cooldown
    pub execution: Option<AutoAttackExecution>, // currently executing attack
    pub targeting: Option<AutoAttackTargeting>, // currently scheduled string of attacks
}

impl Component for AutoAttack {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        self.clone()
    }
}

impl GetComponentID for AutoAttack {
    const ID: ComponentID = ComponentID::AutoAttack;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoAttackUpdate(AutoAttack);

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutoAttackCommand {
    pub target: CharacterID,
    pub projectile_gen_ids: CharacterIDRange,
}

pub struct AutoAttackSystem;

impl WorldSystem for AutoAttackSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for AutoAttackSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::AutoAttack
    }

    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError> {
        match cmd {
            CharacterCommand::AutoAttack(cmd) => {
                if *cid == cmd.target {
                    return Err(WorldError::InvalidCommand)
                }
                if !world.characters.contains(cid) {
                    return Err(WorldError::MissingCharacter(*cid, "Nonexistent character cannot attack".to_string()))
                }
                if !world.characters.contains(&cmd.target) {
                    return Err(WorldError::MissingCharacter(cmd.target, "Cannot attack nonexistent character".to_string()))
                }
                if !world.base.get_component(&cmd.target)?.targetable {
                    return Err(WorldError::InvalidCommand);
                }
                Ok(())
            },
            _ => Err(WorldError::InvalidCommandMapping)
        }
    }

    // fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError> {
    //     match cmd {
    //         CharacterCommand::AutoAttack(cmd) => {
    //             // stop moving
    //             world.movement.get_component_mut(cid)?.destination = None;

    //             // set target (who to attack next even if already doing something else)
    //             let auto_attack = world.auto_attack.get_component_mut(cid)?;
    //             auto_attack.targeting = Some(AutoAttackTargeting {
    //                 target: cmd.target,
    //                 ids: cmd.projectile_gen_ids,
    //             });

    //             // check if busy
    //             let base = world.base.get_component(cid)?;
    //             if auto_attack.is_casting(base.ctype, &world.info) {
    //                 return Ok(())
    //             }
    //             if auto_attack.timer > 0.0 {
    //                 return Ok(())
    //             }

    //             // if not busy, start the attack
    //             // auto_attack_start(world, &cmd.attacker, &cmd.target, &cmd.projectile_gen_id)?;
    //             Ok(())
    //         },
    //         _ => Err(WorldError::InvalidCommandMapping)
    //     }
    // }

    // overall auto attack logic is:
    // on request, set targeting
    // get fsm updates
    //
    // note is casting is whether it should be casting the next frame
    // note executing is whether it should be executing next frame, excluding start of attack,
    // which is handled by arrived
    // note targeting is whether it should be targeting next frame
    //
    // movement update
    //   if not executing and targeting and status is AA
    //     queue walk to target, record arrived
    // execution update
    //   if executing and status is not AA
    //     queue cancel execution
    //   else if not executing and owning status and arrived and not cooldown and targeting
    //     if proj_ids left
    //       queue start execution
    //     else record stop_targeting for targeting update
    //   else if not executing
    //     queue cancel execution
    // targeting update
    //   if command and target is new
    //     queue new target
    //   else if AA is not in status queue and targeting or stop_targeting
    //     queue no target
    //   else if generated new proj id
    //     queue updated target
    //   else if proj ids are empty
    //     queue no target
    // status update
    //   if (arrived or executing) and AA is status and is casting
    //     if prio != AbilityPrioritized
    //       queue AbilityPrioritized status prio
    //   else if command and target is new
    //     queue AA status with ability override
    //   else if (executing or arrived) and AA is status and not casting
    //     if prio != Ability
    //       queue Ability status prio
    //   else if targeting and AA is in status queue
    //     if prio != Ability
    //       queue Ability status prio
    //   else if executing and AA is not status
    //     queue cancel status
    //   else if AA is in status queue and not executing or targeting
    //     queue cancel status
    // projectile update
    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        let statuses = world.status.get_component(&cid)?;
        let status_in_queue = statuses.get_status(StatusID::AutoAttack);
        let is_status = statuses.status.id == StatusID::AutoAttack;
        let (ctype, attack_speed, range) = {
            let base = world.base.get_component(cid)?;
            (base.ctype, base.attack_speed, base.range)
        };
        let attack_info = world.info.auto_attack.get(&ctype)
            .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
        let auto_attack = world.auto_attack.get_component(cid)?;
        let (executing, casting, fire_attack) = {
            let (mut executing, mut casting, mut fire_attack) = (false, false, false);
            if let Some(execution) = auto_attack.execution {
                let timer = (world.tick - execution.time_start) as f32 * 1.0 / TICK_RATE;
                let (changes, _changed) = attack_info.fsm.get_state_changes(
                        attack_speed,
                        timer,
                        timer + delta_time);
                changes.into_iter().for_each(|change| match change {
                    fsm::Changes::Event(_time_since, AutoAttackFireEvent) => fire_attack = true,
                    _ => (),
                });
                let state = attack_info.fsm.get_current_state(attack_speed, timer);
                executing = state != AutoAttackPhase::Complete;
                casting = state == AutoAttackPhase::Casting;
            } else {
                executing = false;
            }

            (executing, casting, fire_attack)
        };
        let targeting = auto_attack.targeting.is_some();
        let on_cooldown = auto_attack.cooldown_timeout <= world.tick;
        let command = commands.into_iter()
            .filter_map(|cmd| match *cmd {
                WorldCommand::CharacterComponent(
                    cmdcid,
                    comp_id,
                    CharacterCommand::AutoAttack(command)
                ) => if cmdcid == *cid && comp_id == ComponentID::AutoAttack // check if new target
                    && !auto_attack.targeting.into_iter().any(|t| t.target == command.target) {
                    Some(command)
                } else { None },
                _ => None
            })
            .deterministic_filter_cmd();

        // movement
        let (arrived, movement_updates) = if !executing && is_status {
            if let Some(AutoAttackTargeting { target, ids: _ids }) = auto_attack.targeting {
                let target_pos = world.base.get_component_mut(&target)?.position;
                let target_pos = Vector2::new(target_pos.x, target_pos.y);
                walk_to(world, cid, &target_pos, range, delta_time)?
            } else { (false, vec![]) }
        } else { (false, vec![]) };

        // execution
        let (stop_targeting, execution_update, new_proj_ids) = if executing && !is_status {
            (false, Some(None), None)
        } else if !executing && is_status && arrived && !on_cooldown {
            if let Some(targeting) = auto_attack.targeting {
                if let (Some(proj_id), new_range) = targeting.ids.split_id() {
                    (false, Some(Some(AutoAttackExecution {
                        projectile_gen_id: proj_id,
                        starting_attack_speed: attack_speed,
                        time_start: world.tick,
                        target: targeting.target
                    })), Some(new_range))
                } else {
                    (true, None, None)
                }
            } else {
                (false, None, None)
            }
        } else {
            (false, None, None)
        };

        // targeting
        let targeting_update = if let Some(cmd) = command {
            // already been filtered for new target
            Some(Some(AutoAttackTargeting {
                target: cmd.target,
                ids: cmd.projectile_gen_ids
            }))
        } else if !status_in_queue.is_none() && targeting || stop_targeting {
            Some(None)
        } else if let Some(new_ids) = new_proj_ids {
            let mut update_targeting = auto_attack.targeting.ok_or(WorldError::BadLogic)?;
            update_targeting.ids = new_ids;
            Some(Some(update_targeting))
        } else if let Some(t) = auto_attack.targeting {
            if t.ids.is_empty() {
                Some(None)
            } else { None }
        } else { None };

        // status
        let status_update = if (arrived || executing) && is_status && casting {
            let status = status_in_queue.ok_or(WorldError::BadLogic)?;
            if status.prio != StatusPrio::AbilityPrioritized {
                Some(StatusUpdate::ChangePrio(StatusID::AutoAttack, StatusPrio::AbilityPrioritized))
            } else {
                None
            }
        } else if command.is_some() {
            Some(StatusUpdate::Try(Status {
                id: StatusID::AutoAttack,
                prio: StatusPrio::AbilityOverride,
                timeout: world.tick + 6000,
                start: world.tick,
            }))
        } else if (arrived || executing) && is_status && !casting || targeting && status_in_queue.is_some() {
            let status = status_in_queue.ok_or(WorldError::BadLogic)?;
            if status.prio != StatusPrio::AbilityPrioritized {
                Some(StatusUpdate::ChangePrio(StatusID::AutoAttack, StatusPrio::Ability))
            } else {
                None
            }
        } else if executing && !is_status || (!executing && !targeting && status_in_queue.is_some()) {
            Some(StatusUpdate::Cancel(StatusID::AutoAttack))
        } else {
            None
        };

        let combined_movement_targeting_updates

        Ok(movement_updates.into_iter()
           .chain())
    }

    // fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<ComponentUpdate>, WorldError> {
    //     let (ctype, attack_speed) = {
    //         let base = world.base.get_component(cid)?;
    //         (base.ctype, base.attack_speed)
    //     };
    //     let attack_info = world.info.auto_attack.get(&ctype)
    //     .ok_or(WorldError::MissingCharacterInfoComponent(ctype, ComponentID::AutoAttack))?.clone();
    //     let auto_attack = world.auto_attack.get_component_mut(cid)?;
    //     auto_attack.timer -= delta_time;
    //     
    //     if let Some(targeting) = &mut auto_attack.targeting {
    //         let target = targeting.target;
    //         if !world.characters.contains(&target) || targeting.ids.is_empty() {
    //             auto_attack.targeting = None;
    //         } else if auto_attack.execution.is_none() {
    //             let attacker_range = world.base.get_component_mut(cid)?.range;

    //             // preliminary movement for attack
    //             let target_pos = world.base.get_component_mut(&target)?.position;
    //             let target_pos = Vector2::new(target_pos.x, target_pos.y);
    //             if let Some(new_remaining_time) = walk_to(world, cid, &target_pos, attacker_range, delta_time)? {
    //                 // we are in range, start the auto
    //                 // set the cooldown
    //                 let auto_attack = &mut world.auto_attack.get_component_mut(cid)?;
    //                 let targeting = auto_attack.targeting.as_mut()
    //                     .ok_or_else(|| WorldError::UnexpectedComponentState(
    //                         *cid,
    //                         ComponentID::AutoAttack,
    //                         "Targeting was removed?".to_string()))?;
    //                 auto_attack.timer = attack_speed - new_remaining_time;

    //                 // start the execution fsm
    //                 world.errors.push(WorldError::Info(format!("Start AA execution for {:?} on {:?}", *cid, target)));
    //                 auto_attack.execution = Some(AutoAttackExecution {
    //                     timer: 0.0,
    //                     starting_attack_speed: attack_speed,
    //                     target,
    //                     projectile_gen_id: targeting.ids.next_id()
    //                         .ok_or_else(|| WorldError::UnexpectedComponentState(
    //                             *cid,
    //                             ComponentID::AutoAttack,
    //                             "Ran out of auto attack IDs".to_string()))?
    //                 });
    //                 return Ok(());
    //             }
    //         }
    //     }
    //     if let Some(execution) = &mut world.auto_attack.get_component_mut(cid)?.execution {
    //         // attack fsm
    //         let target = execution.target;
    //         let timer = execution.timer;
    //         let (changes, _changed) = attack_info.fsm.get_state_changes(
    //                 attack_speed,
    //                 execution.timer,
    //                 execution.timer + delta_time);
    //         for change in changes {
    //             match change {
    //                 fsm::Changes::Event(time_since, _) => auto_attack_fire(world, cid, time_since)?,
    //                 fsm::Changes::StateChange(_time_since, phase) => {
    //                     //println!("New AA phase: {:?}, timer: {}, increment: {}, time_since: {}",
    //                     //    phase,
    //                     //    timer,
    //                     //    delta_time,
    //                     //    time_since);
    //                     match phase {
    //                         AutoAttackPhase::WindUp |
    //                         AutoAttackPhase::Casting |
    //                         AutoAttackPhase::WindDown => (),
    //                         AutoAttackPhase::Complete => {
    //                             // finish the auto attack
    //                             world.auto_attack.get_component_mut(cid)?.execution = None;
    //                         },
    //                     }
    //                 },
    //             }
    //         }
    //         match attack_info.fsm.get_current_state(attack_speed, timer) {
    //             AutoAttackPhase::WindUp |
    //             AutoAttackPhase::Casting => {
    //                 // cancel if target dies
    //                 if !world.characters.contains(&target) {
    //                     world.auto_attack.get_component_mut(cid)?.execution = None;
    //                 }
    //             },
    //             _ => (),
    //         }
    //         if let Some(execution) = &mut world.auto_attack.get_component_mut(cid)?.execution {
    //             execution.timer += delta_time;
    //         }
    //     }
    //     Ok(())
    // }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Vec<ComponentUpdateData> {
        vec![]
    }
}

impl AutoAttack {
    pub fn new() -> Self {
        Self { execution: None, cooldown_timeout: WorldTick::MIN, targeting: None }
    }
}

impl GetCommandID for AutoAttackRequest {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::AutoAttackRequest
    }
}

pub fn auto_attack_system_init() -> Result<WorldInfo, WorldError> {
    // noop
    Ok(WorldInfo::new())
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

// fn auto_attack_update(world: &mut World, mut delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
// }
// 
// pub fn auto_attack_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
//     let cids: Vec<CharacterID> = world.auto_attack.components.keys().copied().collect();
//     for cid in cids {
//         auto_attack_update(world, delta_time, cid).err_log(world);
//     }
//     Ok(())
// }

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
    projectile::create(world, &info)?;
    Ok(())
}

#[cfg(feature = "server")]
pub mod server {
    use std::net::SocketAddr;
    use crate::{model::{player::{server::PlayerCommand, model::{PlayerID, PlayerDataView}, commands::ChatMessage}, PrintError, world::{component::ComponentID, commands::{WorldCommand, CharacterCommand}}}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
    use super::{AutoAttackRequest, AutoAttackCommand};

    impl<'a> PlayerCommand<'a> for AutoAttackRequest {
        const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);

        fn run(self, addr: &SocketAddr, player_id: &PlayerID, server: &mut Server) {
            // check if the player can use the requested character
            if server.player_manager.can_use_character(player_id, &self.attacker) {
                let gen_ids = server.character_id_gen.generate_range(1000);
                server.run_world_command(
                    Some(addr),
                    WorldCommand::CharacterComponent(
                        self.attacker,
                        ComponentID::AutoAttack,
                        CharacterCommand::AutoAttack(AutoAttackCommand {
                            target: self.target,
                            projectile_gen_ids: gen_ids,
                        })
                    )
                );
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
    // pub fn is_casting(&self, ctype: CharacterType, info: &WorldInfo) -> bool {
    //     if let Some(attack_info) = info.auto_attack.get(&ctype) {
    //         if let Some(execution) = &self.execution {
    //             let state = attack_info.fsm.get_current_state(execution.starting_attack_speed, execution.timer);
    //             return state == AutoAttackPhase::Casting
    //         }
    //     }
    //     false
    // }
}

trait FilterAACmd {
    fn deterministic_filter_cmd(self) -> Option<AutoAttackCommand>;
}

impl<T: Iterator<Item = AutoAttackCommand>> FilterAACmd for T {
    fn deterministic_filter_cmd(self) -> Option<AutoAttackCommand> {
        self.max_by_key(|cmd| (cmd.projectile_gen_ids, cmd.target))
    }
}
