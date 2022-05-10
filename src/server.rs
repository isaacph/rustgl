
use std::time::Duration;

use serde::{Serialize, Deserialize};

use crate::networking::server::{ServerConnection, ClientID};
use crate::networking_wrapping::{ExecuteServerCommands, ServerCommand};


#[derive(Serialize, Deserialize, Debug)]
pub struct StopServer();

impl<'a> ServerCommand<'a> for StopServer {
    // fn id(&self) -> ServerCommandID {
    //     ServerCommandID::StopServer
    // }
    fn run(&self, _: &ClientID, server: &mut Server) {
        server.stop = true;
    }
}

pub struct Server {
    pub connection: ServerConnection,
    pub stop: bool
}

impl Server {
    pub fn run(port: u16) {
        let mut server = Server {
            connection: ServerConnection::new(port).unwrap(),
            stop: false
        };

        while !server.stop {
            let messages = server.connection.poll_raw();
            server.execute(messages);
            server.connection.flush();
            std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
        }
    }
}