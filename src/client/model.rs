use std::cmp;

use crate::{model::{SendAddress, SetUDPAddress, EchoMessage, SerializedClientCommand, SerializedServerCommand}, networking::{client::Client, Protocol}};
use crate::{commands_execute, _commands_execute_static_def};
use serde::{Deserialize, Serialize};

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    SerializedClientCommand,
    (Protocol, &mut Client),
    // list all commands the client can execute here:
    [
        SendAddress,
        EchoMessage
    ]
);

// list how the client will respond to each command below

impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, (_, client): (Protocol, &mut Client)) {
        //println!("Server sent their view of client's address: {}", self.0);
        let packet: SerializedServerCommand = (&SetUDPAddress(self.0)).into();
        client.send_tcp(packet);
    }
}

impl <'a> ClientCommand<'a> for EchoMessage {
    fn run(self, _context: (Protocol, &mut Client)) {
        println!("Echoed message: {}", &self.0.as_str()[0..cmp::min(self.0.len(), 4096)]);
    }
}