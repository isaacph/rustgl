use std::cmp;
use serde::Deserialize;

use crate::{model::{commands::{core::{SendAddress, EchoMessage, SetUDPAddress}, GetCommandID, MakeBytes, CommandID}, world::commands::WorldCommand}, networking::{client::{Client, ClientError}, Protocol}};
use super::game::Game;

//pub mod core;
//pub mod player;
//pub mod world;

// commands_execute!(
//     execute_client_command,
//     ClientCommand,
//     ClientCommandID,
//     (Protocol, &mut Game),
//     // list all commands the client can execute here:
//     [
//         crate::model::commands::core::SendAddress,
//         crate::model::commands::core::EchoMessage,
//         crate::model::player::commands::ChatMessage,
//         crate::model::player::commands::PlayerDataPayload,
//         crate::model::world::commands::UpdateCharacter,
//     ]
// );

pub trait ClientCommand<'a>: Deserialize<'a> {
    fn run(self, context: (Protocol, &mut Game));
}

// tell how to deserialize and run each type of command
// stands for "deserialize and run"
fn drun<'a, T: ClientCommand<'a>>(data: &'a [u8], context: (Protocol, &mut Game)) -> Result<(), bincode::Error> {
    let deserialized: T = bincode::deserialize::<'a>(data)?; // TODO: error handling
    T::run(deserialized, context);
    Ok(())
}
// stands for "deserialize and run in world"
fn drun_w<'a, T: WorldCommand<'a>>(data: &'a [u8], (_, game): (Protocol, &mut Game)) -> Result<(), bincode::Error> {
    let mut deserialized: T = bincode::deserialize::<'a>(data)?; // TODO: error handling
    match T::run(&mut deserialized, &mut game.world) {
        Ok(()) => (),
        Err(err) => game.world.errors.push(err),
    }
    Ok(())
}

pub fn execute_client_command(command: &[u8], context: (Protocol, &mut Game)) -> Result<(), String> {
    let id_num = u16::from_be_bytes([command[command.len() - 2], command[command.len() - 1]]);
    let data = &command[..command.len() - 2];

    use crate::model::commands::CommandID::*;
    match CommandID::try_from(id_num) {
        Ok(id) => match match id {
            // place all command deserializations here
            SendAddress => drun::<crate::model::commands::core::SendAddress>(data, context),
            EchoMessage => drun::<crate::model::commands::core::EchoMessage>(data, context),
            ChatMessage => drun::<crate::model::player::commands::ChatMessage>(data, context),
            SetUDPAddress => drun::<crate::model::player::commands::PlayerDataPayload>(data, context),
            UpdateCharacter => drun::<crate::model::world::commands::UpdateCharacter>(data, context),
            MoveCharacter => drun_w::<crate::model::world::system::movement::MoveCharacter>(data, context),
            AutoAttackCommand => drun_w::<crate::model::world::system::auto_attack::AutoAttackCommand>(data, context),
            IndicateClientPlayer => drun::<crate::model::player::commands::IndicateClientPlayer>(data, context),
            PlayerDataPayload => drun::<crate::model::player::commands::PlayerDataPayload>(data, context),
            _ => {
                println!("Command ID not implemented on client: {:?}", id);
                Ok(())
            }
        } {
            Ok(()) => Ok(()),
            // handle bincode error
            Err(err) => Err(format!("Bincode deserialize fail: {}", err))
        },
        // handle id not found error
        Err(err) => Err(format!("Failure to find client command ID: {}", err))
    }
}

pub trait SendCommands {
    fn send<T: GetCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError>;
}

impl SendCommands for Client {
    fn send<T: GetCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError> {
        match command.make_bytes() {
            Ok(bytes) => self.send_data(protocol, bytes),
            Err(err) => Err(ClientError::Other(format!(
                "Command {:?} serialize fail: {}",
                command.command_id(), err
            )))
        }
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

