use crate::{model::commands::ServerCommandID, networking::{client::{Client, ClientError}, Protocol}};
use crate::{commands_execute, _commands_execute_static_def};

use super::game::Game;

pub mod core;
pub mod player;
pub mod world;

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    (Protocol, &mut Game),
    // list all commands the client can execute here:
    [
        crate::model::commands::core::SendAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::commands::player::ChatMessage,
        crate::model::commands::player::PlayerDataPayload,
        crate::model::world::commands::UpdateCharacter,
    ]
);

pub trait SendCommands {
    fn send<T: ServerCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError>;
}

impl SendCommands for Client {
    fn send<T: ServerCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError> {
        self.send_data(protocol, command.make_bytes())
    }
}

