
use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};
use crate::model::{world::{character::CharacterID, commands::{CharacterCommand, Priority, WorldCommand}, World, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer, ComponentUpdateData, Component, ComponentUpdate}, WorldSystem, WorldInfo, ComponentSystem, Update, system::status::{StatusUpdate, StatusPrio, StatusID, Status}}, commands::GetCommandID, util::{ItClosest, GroundPos, ItClosestRef}};

use super::base::{CharacterFlip, make_flip_update, make_move_update};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Movement {
    pub destination: Option<Vector2<f32>>
}

impl Component for Movement {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match update.clone() {
            ComponentUpdateData::Movement(m) => m,
            _ => self.clone()
        }
    }
}

impl GetComponentID for Movement {
    const ID: ComponentID = ComponentID::Movement;
}

pub fn make_movement_component_update(cid: CharacterID, dest: Option<Vector2<f32>>) -> Update {
    Update::Comp(ComponentUpdate {
        cid,
        data: ComponentUpdateData::Movement(
            Movement {
                destination: dest
            }
        )
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoveCharacter {
    pub destination: Vector2<f32>,
}

// impl WorldCommand for MoveCharacter {
//     fn validate(&self, world: &World) -> Result<(), WorldError> {
//         if self.destination.x.is_nan() || self.destination.y.is_nan() {
//             return Err(WorldError::InvalidCommand);
//         }
//         if !world.characters.contains(&self.to_move) {
//             return Err(WorldError::MissingCharacter(
//                 self.to_move,
//                 "Cannot move nonexistent character".to_string()))
//         }
//         let base = world.base.get_component(&self.to_move)?;
//         if self.reset {
//             if let Some(auto_attack) = world.auto_attack.components.get(&self.to_move) {
//                 if auto_attack.is_casting(base.ctype, &world.info) {
//                     return Err(WorldError::IllegalInterrupt(self.to_move));
//                 }
//             }
//         }
//         Ok(())
//     }
//     fn run(&mut self, world: &mut World) -> Result<(), WorldError> {
//         let movement = world.movement.get_component_mut(&self.to_move)?;
//         movement.destination = Some(self.destination);
//         if let Some(auto_attack) = world.auto_attack.components.get_mut(&self.to_move) {
//             auto_attack.targeting = None;
//         }
//         if self.reset {
//             world.auto_attack.get_component_mut(&self.to_move)?.execution = None;
//         }
//         Ok(())
//     }
// }
// 
// impl GetCommandID for MoveCharacter {
//     fn command_id(&self) -> crate::model::commands::CommandID {
//         crate::model::commands::CommandID::MoveCharacter
//     }
// }


// try to start an action on a target at a range. if in range, return Some(0.0), indicating to start the action.
// if out of range, move towards the target, and if the target ends up in range during the frame,
// return Some(x) where x is the remaining time to spend on the attack, after consuming necessary
// time for walking
pub fn walk_to(world: &World, cid: &CharacterID, dest: &Vector2<f32>, range: f32, delta_time: f32) -> Result<(bool, Vec<Update>), WorldError> {
    let base = world.base.get_component(cid)?;
    let speed = base.speed;
    let max_travel = speed * delta_time;
    let pos = Vector2::new(base.position.x, base.position.y);
    let dir = dest - pos;
    if dir.x == 0.0 && dir.y == 0.0 {
        return Ok((true, vec![]))
    }
    let dist = dir.magnitude();
    if speed <= 0.0 {
        return Ok((dist <= range, vec![]))
    }
    let flip = CharacterFlip::from_dir(&Vector2::new(dir.x, dir.y)).unwrap_or(base.flip);
    if f32::max(dist - max_travel, 0.0) <= range {
        let travel = f32::max(dist - range, 0.0);
        let _remaining_time = delta_time - travel / speed;
        let offset = dir / dist * travel;
        // base.position += offset;
        Ok((true, vec![
            make_move_update(*cid, Priority::Walk, Vector3::new(offset.x, offset.y, 0.0)),
            make_flip_update(*cid, Priority::Walk, flip)
        ]))
    } else {
        let offset = dir / dist * max_travel;
        // base.position += offset;
        Ok((false, vec![
            make_move_update(*cid, Priority::Walk, Vector3::new(offset.x, offset.y, 0.0)),
            make_flip_update(*cid, Priority::Walk, flip)
        ]))
    }
}

pub struct MovementSystem;

impl WorldSystem for MovementSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for MovementSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Movement
    }

    // fn validate_character_state(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<(), WorldError> {
    //     Ok(())
    // }

    // make status update
    //   if movement command is sent, queue it as update
    //   else if has destination and walk is the status, then
    //     if reached destination, queue cancel walk status
    //     else if walk status prio is not Ability
    //       queue update status prio to ability
    //     else
    //       do nothing
    //   else if has destination and walk is in queue
    //     if walk status prio is not Ability
    //       queue update status prio to ability
    //     else if there is no walk status
    //       queue cancel walk status
    //   else if has destination and walk is not the status
    //     queue cancel walk status
    //   else if has no destination and walk is the status
    //     cancel walk
    //   else do nothing
    // make position update
    //   if walk is the status and has destination, then
    //     if reached destination, set position to destination
    //     else queue move position towards destination
    //   else
    //     do nothing
    // make movement update
    //   if movement command is sent
    //     queue set destination
    //   else if has a destination and (arrived or walk is not the status)
    //     queue remove destination
    //   else
    //     do nothing
    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        use ComponentUpdateData::Status as CStatus;
        use StatusUpdate::{Cancel, ChangePrio, Try};
        let pos = world.base.get_component(cid)?.position;
        let dest = world.movement.get_component(&cid)?.destination;
        let statuses = world.status.get_component(&cid)?;
        let status = &statuses.status;
        let walk_status = statuses.get_status(StatusID::Walk);
        let (arrived, position_updates) = match (status.id, dest) {
            (StatusID::Walk, Some(dest)) => {
                walk_to(world, cid, &dest, 0.0, delta_time)?
            },
            _ => (false, vec![])
        };
        let movement_command = commands.into_iter()
            .filter_map(|cmd| match *cmd {
                WorldCommand::CharacterComponent(cmd_cid, cmd_comp_id, CharacterCommand::Movement(MoveCharacter { destination })) =>
                    if *cid == cmd_cid && cmd_comp_id == ComponentID::Movement {
                        Some(destination)
                    } else { None },
                _ => None
            })
            .closest_to(&pos.ground_pos());
        let movement_updates = if let Some(dest) = movement_command {
            vec![make_movement_component_update(*cid, Some(dest))]
        } else if dest.is_some() && (arrived || !statuses.status_queued(StatusID::Walk)) {
            vec![make_movement_component_update(*cid, None)]
        } else {
            vec![]
        };
        let status_updates = if movement_command.is_some() {
            vec![Update::Comp(ComponentUpdate {
                cid: *cid,
                data: CStatus(Try(Status {
                    prio: StatusPrio::AbilityOverride,
                    id: StatusID::Walk,
                    timeout: world.tick + 6000,
                    start: world.tick
                }))
            })]
        } else if dest.is_some() && status.id == StatusID::Walk {
            if arrived {
                vec![Update::Comp(ComponentUpdate {
                    cid: *cid,
                    data: CStatus(Cancel(StatusID::Walk))
                })]
            } else if status.prio != StatusPrio::Ability {
                vec![Update::Comp(ComponentUpdate {
                    cid: *cid,
                    data: CStatus(ChangePrio(StatusID::Walk, StatusPrio::Ability))
                })]
            } else { vec![] }
        } else if dest.is_some() {
            if let Some(ws) = walk_status {
                if ws.prio != StatusPrio::Ability  {
                    vec![Update::Comp(ComponentUpdate {
                        cid: *cid,
                        data: CStatus(ChangePrio(StatusID::Walk, StatusPrio::Ability))
                    })]
                } else { vec![] }
            } else { 
                vec![Update::Comp(ComponentUpdate {
                    cid: *cid,
                    data: CStatus(Cancel(StatusID::Walk))
                })]
            }
        } else if dest.is_none() && statuses.status_queued(StatusID::Walk) {
            vec![Update::Comp(ComponentUpdate {
                cid: *cid,
                data: CStatus(Cancel(StatusID::Walk))
            })]
        } else {
            vec![]
        };
        let x: Vec<Update> = position_updates.into_iter().chain(
           movement_updates.into_iter().chain(
           status_updates.into_iter())).collect();
        if !x.is_empty() {
            println!("Movement update: {:?}", x);
        }
        Ok(x)
    }

    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError> {
        match cmd {
            CharacterCommand::Movement(cmd) => {
                if cmd.destination.x.is_nan() || cmd.destination.y.is_nan() {
                    return Err(WorldError::InvalidCommand);
                }
                if !world.characters.contains(cid) {
                    return Err(WorldError::MissingCharacter(
                        *cid,
                        "Cannot move nonexistent character".to_string()))
                }
                // let base = world.base.get_component(cid)?;
                // if cmd.reset {
                //     if let Some(auto_attack) = world.auto_attack.components.get(cid) {
                //         if auto_attack.is_casting(base.ctype, &world.info) {
                //             return Err(WorldError::IllegalInterrupt(*cid));
                //         }
                //     }
                // }
                Ok(())
            },
            _ => Err(WorldError::InvalidCommandMapping)
        }
    }

    // fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError> {
    //     match cmd {
    //         CharacterCommand::Movement(cmd) => {
    //             let movement = world.movement.get_component_mut(cid)?;
    //             movement.destination = Some(cmd.destination);
    //             if let Some(auto_attack) = world.auto_attack.components.get_mut(cid) {
    //                 auto_attack.targeting = None;
    //             }
    //             if cmd.reset {
    //                 world.auto_attack.get_component_mut(cid)?.execution = None;
    //             }
    //             Ok(())
    //         },
    //         _ => Err(WorldError::InvalidCommandMapping)
    //     }
    // }
    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        use ComponentUpdateData::Movement as CUD_M;
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.into_iter().filter(|new| match *new {
                CUD_M(_) => true,
                _ => false,
            }).cloned().collect();
            if new_changes.len() == 0 {
                return Err(WorldError::InvalidReduceMapping(*cid, ComponentID::Movement))
            } else if new_changes.len() > 1 {
                return Err(WorldError::MultipleUpdateOverrides(*cid, ComponentID::Movement))
            } else {
                return Ok(new_changes)
            }
        }
        let pos = {
            let pos3 = world.base.get_component(cid)?.position;
            Vector2::new(pos3.x, pos3.y)
        };
        // if a None is passed in, return None
        // else if a dest is passed in, return the closest
        // else return nothing
        if changes.into_iter().any(|change| match *change {
            CUD_M(Movement { destination: None }) => true,
            _ => false,
        }) {
            return Ok(vec![CUD_M(Movement { destination: None })]);
        }
        Ok(changes
            .into_iter()
            .filter_map(|change| match change {
                CUD_M(Movement { destination: Some(dest) }) => Some(dest),
                _ => None,
            })
            // choose a destination deterministically
            .closest_to(&pos)
            .into_iter()
            .map(|dest| CUD_M(Movement { destination: Some(*dest) }))
            .collect())
    }
}

// pub fn movement_system_init() -> Result<WorldInfo, WorldError> {
//     Ok(WorldInfo::new())
// }
// 
// fn movement_update(world: &mut World, delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
//     match world.movement.get_component(&cid)?.destination.as_ref() {
//         Some(dest) => {// currently executing the action
//             // cancel auto attack
//             let base = world.base.get_component(&cid)?;
//             if let Some(auto_attack) = world.auto_attack.components.get_mut(&cid) {
//                 auto_attack.targeting = None;
//                 if !auto_attack.is_casting(base.ctype, &world.info) {
//                     auto_attack.execution = None;
//                 } else {
//                     return Ok(());
//                 }
//             }
// 
//             // move
//             let dest = *dest;
//             match walk_to(world, &cid, &dest, 0.0, delta_time)? {
//                 Some(_) => {
//                     world.movement.get_component_mut(&cid)?.destination = None;
//                 },
//                 None => ()
//             }
//         }
//         None => (), // waiting to execute the action
//     }
//     Ok(())
// }
// 
// pub fn movement_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
//     let cids: Vec<CharacterID> = world.movement.components.keys().copied().collect();
//     for cid in cids {
//         movement_update(world, delta_time, cid).err_log(world);
//     }
//     Ok(())
// }

#[derive(Serialize, Deserialize, Debug)]
pub struct MoveCharacterRequest {
    pub id: CharacterID,
    pub dest: Vector2<f32>
}

impl GetCommandID for MoveCharacterRequest {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::MoveCharacterRequest
    }
}

#[cfg(feature = "server")]
pub mod server {
    use std::net::SocketAddr;
    use crate::{model::{player::{server::PlayerCommand, model::{PlayerID, PlayerDataView}, commands::ChatMessage}, PrintError, world::{commands::{WorldCommand, CharacterCommand}, component::ComponentID}}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
    use super::{MoveCharacterRequest, MoveCharacter};

    impl<'a> PlayerCommand<'a> for MoveCharacterRequest {
        const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);
        fn run(self, addr: &SocketAddr, pid: &PlayerID, server: &mut Server) {
            // check if character can move the character at self.id
            println!("I received your command!");
            if server.player_manager.can_use_character(pid, &self.id) {
                // // determine if attack should reset character state
                // let reset = if let Some(auto_attack) = server.world.auto_attack.components.get(&self.id) {
                //     if let Some(base) = server.world.base.components.get(&self.id) {
                //         !auto_attack.is_casting(base.ctype, &server.world.info)
                //     } else { false }
                // } else { false };
                let command = WorldCommand::CharacterComponent(self.id, ComponentID::Movement, CharacterCommand::Movement(MoveCharacter {
                    destination: self.dest,
                }));
                println!("Run world command: {:?}", command);
                server.run_world_command(Some(addr), command);
            } else {
                server.connection.send(Protocol::TCP, addr, &ChatMessage("Error: missing permissions".to_string())).print()
            }
        }
    }
}

