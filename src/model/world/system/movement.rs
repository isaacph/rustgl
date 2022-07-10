use std::net::SocketAddr;

use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::{model::{world::{World, character::CharacterID, WorldError, commands::WorldCommand, component::ComponentID}, commands::GetCommandID, player::{server::PlayerCommand, model::PlayerID, commands::ChatMessage}, Subscription, PrintError}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};

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
                movement.destination = Some(self.destination);
                Ok(())
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

pub fn movement_system_update(world: &mut World, delta_time: f32) {
    for (cid, movement) in &mut world.movement.components {
        match movement.destination {
            Some(dest) => {
                match world.base.components.get_mut(cid) {
                    Some(base) => {
                        let speed = base.speed;
                        let travel = speed * delta_time;

                        let v = dest - base.position;
                        if v.magnitude() <= travel {
                            base.position = dest;
                            movement.destination = None;
                        } else {
                            base.position += v.normalize() * travel;
                        }
                    },
                    None => world.errors.push(
                        WorldError::MissingCharacterComponent(*cid, ComponentID::Base)
                    )
                }
            },
            None => ()
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
                format!("Invalid numbers")
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
