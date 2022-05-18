
use std::time::{Duration, SystemTime};

use serde::{Serialize, Deserialize};

use crate::networking::server::{ServerConnection, ConnectionID};
use crate::networking_wrapping::{ExecuteServerCommands, ServerCommand};


#[derive(Serialize, Deserialize, Debug)]
pub struct StopServer();

impl<'a> ServerCommand<'a> for StopServer {
    fn run(&mut self, (_, server): (&ConnectionID, &mut Server)) {
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
            server.connection.prune_dead_connections(SystemTime::now());
            std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
        }
    }
}