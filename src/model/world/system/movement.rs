
use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};
use crate::model::{world::{character::CharacterID, commands::{CharacterCommand, Priority, WorldCommand}, World, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer, ComponentUpdateData, Component, ComponentUpdate}, WorldSystem, WorldInfo, ComponentSystem, Update, system::status::{StatusUpdate, Status, StatusPrio, StatusID, StatusComponent}}, commands::GetCommandID, WorldTick};

use super::base::{CharacterFlip, make_flip_update, make_move_update};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Movement {
    pub destination: Option<Vector2<f32>>
}

impl Component for Movement {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        self.clone()
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
    pub reset: bool,
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
    let base = world.base.get_component_mut(cid)?;
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
        let remaining_time = delta_time - travel / speed;
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

    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        match world.movement.get_component(cid)?.destination.as_ref() {
            Some(dest) => {// currently executing the action
                // // cancel auto attack
                // let base = world.base.get_component(cid)?;
                // if let Some(auto_attack) = world.auto_attack.components.get_mut(cid) {
                //     auto_attack.targeting = None;
                //     if !auto_attack.is_casting(base.ctype, &world.info) {
                //         auto_attack.execution = None;
                //     } else {
                //         return Ok(());
                //     }
                // }
                use ComponentUpdateData::Status as CStatus;
                use StatusUpdate::{Cancel, ChangePrio};
                let StatusComponent {status, runner_up} = world.status.get_component(&cid)?;
                match (status.id, runner_up.id) {
                    (StatusID::Walk, _) => {
                        let (arrived, updates) = walk_to(world, cid, dest, 0.0, delta_time)?;
                        Ok(updates.into_iter()
                            .chain([
                                match status.prio {
                                    StatusPrio::Ability => None,
                                    _ => Some(Update::Comp(ComponentUpdate {
                                        cid: *cid,
                                        data: CStatus(ChangePrio(StatusID::Walk, StatusPrio::Ability))
                                    })),
                                },
                                if arrived {
                                    Some(Update::Comp(ComponentUpdate {
                                        cid: *cid,
                                        data: CStatus(Cancel(StatusID::Walk))
                                    }))
                                } else {
                                    None
                                }
                            ]
                            .into_iter()
                            .flatten())
                        .collect())
                    },
                    (_, StatusID::Walk) => Ok(vec![]), // we are waiting, no op
                    _ => { // action is no longer queued, forget destination
                        Ok(vec![make_movement_component_update(*cid, None)])
                    }
                }
            }
            None => Ok(vec![]),
        }
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
        let cur_dest = world.movement.get_component(cid)?.destination;
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
            .fold((None, f32::MAX), |(cur, cur_dist), next_dest| {
                let dir = next_dest - pos;
                let dist = if dir.x == 0.0 && dir.y == 0.0 {
                    0.0
                } else {
                    dir.magnitude()
                };
                if dist < cur_dist {
                    (Some(next_dest), dist)
                } else if dist == cur_dist {
                    match cur {
                        Some(curd) => {
                            if next_dest.x < curd.x || next_dest.x == curd.x && next_dest.y < curd.y {
                                (Some(next_dest), dist)
                            } else {
                                (cur, cur_dist)
                            }
                        },
                        None => (Some(next_dest), dist),
                    }
                } else {
                    (cur, cur_dist)
                }
            }).0
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
            if server.player_manager.can_use_character(pid, &self.id) {
                // determine if attack should reset character state
                let reset = if let Some(auto_attack) = server.world.auto_attack.components.get(&self.id) {
                    if let Some(base) = server.world.base.components.get(&self.id) {
                        !auto_attack.is_casting(base.ctype, &server.world.info)
                    } else { false }
                } else { false };
                let command = WorldCommand::CharacterComponent(self.id, ComponentID::Movement, CharacterCommand::Movement(MoveCharacter {
                    destination: self.dest,
                    reset
                }));
                server.run_world_command(Some(addr), command);
            } else {
                server.connection.send(Protocol::TCP, addr, &ChatMessage("Error: missing permissions".to_string())).print()
            }
        }
    }
}

