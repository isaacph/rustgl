use std::time::{Duration, SystemTime};

use serde::{Serialize, Deserialize};

use crate::game::{Game, ChatMessage};
use crate::networking::server::{ServerConnection, ConnectionID};
use crate::networking_wrapping::{ServerCommand, SerializedServerCommand, ClientCommand, VecSerializedWrapperDecay, SerializedClientCommand};

use crate::world::World;
use crate::world::character::CharacterIDGenerator;
use crate::world::player::PlayerManager;

use self::update_loop::UpdateLoop;

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
        pub fn send_next_update(&mut self, world: &World) -> Vec<SerializedClientCommand> {
            let now = SystemTime::now();
            let should_update = match self.last_update {
                None => true,
                Some(time) => now.duration_since(time).unwrap() > self.update_interval
            };
            match should_update {
                false => vec![],
                true =>
                    world.characters.iter()
                        .filter_map(|cid| world.make_cmd_update_character(*cid))
                        .map(|cmd| SerializedClientCommand::from(&cmd))
                        .collect()
            }
        }
    }
}



#[derive(Serialize, Deserialize, Debug)]
pub struct EmptyCommand;
impl<'a> ServerCommand<'a> for EmptyCommand {
    fn run(self, _: (&ConnectionID, &mut Server)) {}
}
impl<'a> ClientCommand<'a> for EmptyCommand {
    fn run(self, _: &mut Game) {}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StopServer();

impl<'a> ServerCommand<'a> for StopServer {
    fn run(self, (_, server): (&ConnectionID, &mut Server)) {
        server.stop = true;
    }
}

pub struct Server {
    pub connection: ServerConnection,
    pub stop: bool,
    pub world: World,
    pub character_id_gen: CharacterIDGenerator,
    pub player_manager: PlayerManager
}

impl Server {
    pub fn run(port: u16) {
        let world = World::new();
        let mut server = Server {
            connection: ServerConnection::new(port).unwrap(),
            stop: false,
            world,
            character_id_gen: CharacterIDGenerator::new(),
            player_manager: PlayerManager::new()
        };
        let mut update_loop = UpdateLoop::init(&server.world);

        while !server.stop {
            let messages = server.connection.poll_raw();
            for (cid, data) in messages {
                for data in data {
                    let ser = SerializedServerCommand::new(data);
                    ser.execute(&cid, &mut server);
                }
            }
            server.connection.send_all(
                server.connection.all_connection_ids(),
                update_loop.send_next_update(&server.world).decay()
            );
            server.connection.flush();
            for con_id in server.connection.prune_dead_connections(SystemTime::now()) {
                if let Some(player) = server.player_manager.map_existing_player(Some(&con_id), None) {
                    server.connection.send_raw(
                        server.connection.all_connection_ids(),
                        SerializedClientCommand::from(
                            &ChatMessage::new(format!("{} was disconnected.", player.name))
                        ).data
                    );
                }
            }
            std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
        }
    }
}
