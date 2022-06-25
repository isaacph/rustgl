use std::cmp;

use crate::{model::commands::{SendAddress, SetUDPAddress, EchoMessage, ServerCommandID}, networking::{client::{Client, ClientError}, Protocol}};
use crate::{commands_execute, _commands_execute_static_def};
use serde::{Deserialize, Serialize};

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    (Protocol, &mut Client),
    // list all commands the client can execute here:
    [
        SendAddress,
        EchoMessage
    ]
);

pub trait SendCommands {
    fn send_tcp<T: ServerCommandID>(&mut self, command: &T) -> std::result::Result<(), ClientError>;
    fn send_udp<T: ServerCommandID>(&mut self, command: &T) -> std::result::Result<(), ClientError>;
}

impl SendCommands for Client {
    fn send_tcp<T: ServerCommandID>(&mut self, command: &T) -> std::result::Result<(), ClientError> {
        self.send_tcp_data(command.make_bytes())
    }
    fn send_udp<T: ServerCommandID>(&mut self, command: &T) -> std::result::Result<(), ClientError> {
        self.send_udp_data(command.make_bytes())
    }
}

// list how the client will respond to each command below

impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, (_, client): (Protocol, &mut Client)) {
        //println!("Server sent their view of client's address: {}", self.0);
        match client.send_tcp(&SetUDPAddress(self.0)) {
            Ok(()) => (),
            Err(err) => println!("Failed to send address to server: {}", err)
        }
    }
}

impl <'a> ClientCommand<'a> for EchoMessage {
    fn run(self, _context: (Protocol, &mut Client)) {
        println!("Echoed message: {}", &self.0.as_str()[0..cmp::min(self.0.len(), 4096)]);
    }
}
