
use nalgebra::Vector2;
use serde::{Serialize, Deserialize};
use crate::model::{world::{character::CharacterID, commands::WorldCommand, World, WorldError, component::ComponentID}, commands::GetCommandID};
use super::action_queue::Action;

#[derive(Serialize, Deserialize, Debug)]
pub struct Movement {
    pub destination: Option<Vector2<f32>>
}

#[derive(Serialize, Deserialize)]
pub struct MoveCharacter {
    pub to_move: CharacterID,
    pub destination: Vector2<f32>
}

impl<'a> WorldCommand<'a> for MoveCharacter {
    fn run(self, world: &mut World) -> Result<(), WorldError> {
        match world.movement.components.get_mut(&self.to_move) {
            Some(movement) => {
                match world.action_queue.components.get_mut(&&self.to_move) {
                    Some(action_queue) => {
                        action_queue.add_action(&world.info, Action::Move, f32::MAX);
                        movement.destination = Some(self.destination);
                        Ok(())
                    },
                    None => Err(WorldError::MissingCharacterComponent(self.to_move, ComponentID::ActionQueue))
                }
            },
            None => Err(WorldError::MissingCharacterComponent(self.to_move, ComponentID::Movement))
        }
    }
}

impl GetCommandID for MoveCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::MoveCharacter
    }
}

pub fn movement_system_init(_: &mut World) {
    // noop
}

pub fn movement_system_update(world: &mut World, delta_time: f32) {
    for (cid, movement) in &mut world.movement.components {
        match world.action_queue.components.get_mut(cid) {
            Some(action_queue) => {
                // we have: movement + action_queue components
                // ensure that the action_queue's action status matches the movement component's data
                match action_queue.fix_action(cid, Action::Move, movement.destination.as_ref()) {
                    Ok(Some(dest)) => // currently executing the action
                    match world.base.components.get_mut(cid) {
                        Some(base) => {
                            let speed = base.speed;
                            let travel = speed * delta_time;

                            let v = dest - base.position;
                            if v.magnitude() <= travel {
                                base.position = *dest;
                                movement.destination = None;
                                action_queue.remove_action(Action::Move);
                            } else {
                                base.position += v.normalize() * travel;
                            }
                        },
                        None => world.errors.push(
                            WorldError::MissingCharacterComponent(*cid, ComponentID::Base)
                        )
                    },
                    Ok(None) => (), // waiting to execute the action
                    Err(WorldError::MissingActionStatus(_, _)) => {
                        // this error occurs usually when the action runs out of time
                        // so like the player buffered a second action but the buffer expired
                        // we should fully cancel the movement action in this case
                        movement.destination = None;
                    },
                    Err(err) => world.errors.push(err)
                }
            },
            None => world.errors.push(
                WorldError::MissingCharacterComponent(*cid, ComponentID::ActionQueue)
            )
        }
    }
}

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

    use crate::{model::{player::{server::PlayerCommand, model::PlayerID, commands::ChatMessage}, PrintError, Subscription}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};

    use super::{MoveCharacterRequest, MoveCharacter};

    impl<'a> PlayerCommand<'a> for MoveCharacterRequest {
        const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);
        fn run(self, addr: &SocketAddr, _: &PlayerID, server: &mut Server) {
            // check if character can move the character at self.id
            // currently only validation is that the character exists and destination is not NaN
            match (server.world.characters.get(&self.id), self.dest.x.is_nan(), self.dest.y.is_nan()) {
                (None, _, _) => server.connection.send(Protocol::TCP, addr, &ChatMessage(
                    format!("Invalid character ID: {:?}", self.id)
                )).print(),
                (_, true, _) |
                (_, _, true) => server.connection.send(Protocol::TCP, addr, &ChatMessage(
                    "Invalid numbers".to_string()
                )).print(),
                (Some(_), false, false) => {
                    let cmd = MoveCharacter {
                        to_move: self.id,
                        destination: self.dest,
                    };
                    // tell everyone the character has started moving
                    server.broadcast(Subscription::World, Protocol::UDP, &cmd);
                    // start the movement of the character locally
                    server.world.run_command(cmd);
                },
            }
        }
    }
}
