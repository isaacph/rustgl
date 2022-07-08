use std::cmp;
use crate::{model::commands::{ServerCommandID, core::{SendAddress, EchoMessage, SetUDPAddress}}, networking::{client::{Client, ClientError}, Protocol}};
use crate::{commands_execute, _commands_execute_static_def};
use super::game::Game;

//pub mod core;
//pub mod player;
//pub mod world;

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    (Protocol, &mut Game),
    // list all commands the client can execute here:
    [
        crate::model::commands::core::SendAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::player::commands::ChatMessage,
        crate::model::player::commands::PlayerDataPayload,
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

// list how the client will respond to each command below
impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        //println!("Server sent their view of client's address: {}", self.0);
        game.finding_addr = false;
        match game.connection.send(Protocol::TCP, &SetUDPAddress(self.0)) {
            Ok(()) => (),
            Err(err) => println!("Failed to send address to server: {}", err)
        }
    }
}

impl <'a> ClientCommand<'a> for EchoMessage {
    fn run(self, _context: (Protocol, &mut Game)) {
        println!("Echoed message: {}", &self.0.as_str()[0..cmp::min(self.0.len(), 4096)]);
    }
}

