use std::cmp;
use crate::{model::commands::core::{SendAddress, SetUDPAddress, EchoMessage}, networking::Protocol, client::game::Game};
use super::{ClientCommand, SendCommands};

// list how the client will respond to each command below
impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        //println!("Server sent their view of client's address: {}", self.0);
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
