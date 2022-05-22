use std::time::Duration;

use serde::{Serialize, Deserialize};

use crate::networking::server::{ServerConnection, ConnectionID};
use crate::networking_wrapping::{ServerCommand, SerializedServerCommand};

use crate::world::World;
use crate::world::character::CharacterIDGenerator;

pub mod update_loop {
    use std::time::{SystemTime, Duration};
    use crate::networking_wrapping::SerializedClientCommand;

    use super::World;
    pub struct UpdateLoop {
        last_update: Option<SystemTime>,
        update_interval: Duration
    }

    impl UpdateLoop {
        pub fn init(_world: &World) -> UpdateLoop {
            UpdateLoop {
                last_update: Some(SystemTime::now()),
                update_interval: Duration::new(1, 0)
            }
        }
        pub fn send_next_update(&mut self, _world: &World) -> Vec<SerializedClientCommand> {
            let now = SystemTime::now();
            let should_update = match self.last_update {
                None => true,
                Some(time) => now.duration_since(time).unwrap() > self.update_interval
            };
            match should_update {
                false => vec![],
                true => {
                    vec![]
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StopServer();

impl<'a> ServerCommand<'a> for StopServer {
    fn run(&mut self, (_, server): (&ConnectionID, &mut Server)) {
        server.stop = true;
    }
}

pub struct Server {
    pub connection: ServerConnection,
    pub stop: bool,
    pub world: World,
    pub character_id_gen: CharacterIDGenerator,
}

impl Server {
    pub fn run(port: u16) {
        let mut server = Server {
            connection: ServerConnection::new(port).unwrap(),
            stop: false,
            world: World::new(),
            character_id_gen: CharacterIDGenerator::new()
        };

        while !server.stop {
            let messages = server.connection.poll_raw();
            for (cid, data) in messages {
                for data in data {
                    let ser = SerializedServerCommand::new(data);
                    ser.execute(&cid, &mut server);
                }
            }
            server.connection.flush();
            //server.connection.prune_dead_connections(SystemTime::now());
            std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
        }
    }
}
