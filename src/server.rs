pub mod model;
use std::time::Duration;

use crate::{model::world::{character::CharacterIDGenerator, World}, networking::{self, server::ServerUpdate, Protocol}};

use crate::networking::server::Server as Connection;

//pub mod update_loop {
//    use std::time::{SystemTime, Duration};
//    use crate::networking_wrapping::SerializedClientCommand;
//
//    use super::World;
//    pub struct UpdateLoop {
//        last_update: Option<SystemTime>,
//        update_interval: Duration
//    }
//
//    impl UpdateLoop {
//        pub fn init(_world: &World) -> UpdateLoop {
//            UpdateLoop {
//                last_update: Some(SystemTime::now()),
//                update_interval: Duration::new(1, 0)
//            }
//        }
//        pub fn send_next_update(&mut self, world: &World) -> Vec<SerializedClientCommand> {
//            let now = SystemTime::now();
//            let should_update = match self.last_update {
//                None => true,
//                Some(time) => now.duration_since(time).unwrap() > self.update_interval
//            };
//            match should_update {
//                false => vec![],
//                true =>
//                    world.characters.iter()
//                        .filter_map(|cid| world.make_cmd_update_character(*cid))
//                        .map(|cmd| SerializedClientCommand::from(&cmd))
//                        .collect()
//            }
//        }
//    }
//}
//


//#[derive(Serialize, Deserialize, Debug)]
//pub struct EmptyCommand;
//impl<'a> ServerCommand<'a> for EmptyCommand {
//    fn run(self, _: (&ConnectionID, &mut Server)) {}
//}
//impl<'a> ClientCommand<'a> for EmptyCommand {
//    fn run(self, _: &mut Game) {}
//}
//
//#[derive(Serialize, Deserialize, Debug)]
//pub struct StopServer();
//
//impl<'a> ServerCommand<'a> for StopServer {
//    fn run(self, (_, server): (&ConnectionID, &mut Server)) {
//        server.stop = true;
//    }
//}

pub struct Server {
    pub stop: bool,
    pub world: World,
    pub character_id_gen: CharacterIDGenerator,
    //pub player_manager: PlayerManager
    pub connection: Connection
}

impl Server {
    pub fn run(ports: (u16, u16)) -> Result<(), std::io::Error> {
        let world = World::new();
        let mut server = Server {
            stop: false,
            world,
            character_id_gen: CharacterIDGenerator::new(),
            //player_manager: PlayerManager::new()
            connection: Connection::init(ports)?
        };
        //let mut update_loop = UpdateLoop::init(&server.world);

        //while !server.stop {
        loop {
            //for (cid, data) in messages {
            //    for data in data {
            //        let ser = SerializedServerCommand::new(data);
            //        ser.execute(&cid, &mut server);
            //    }
            //}
            //server.connection.send_udp_all(
            //    server.connection.all_connection_ids(),
            //    update_loop.send_next_update(&server.world).decay()
            //);
            //server.connection.flush();
            //for con_id in server.connection.prune_dead_connections(SystemTime::now()) {
            //    if let Some(player) = server.player_manager.map_existing_player(Some(&con_id), None) {
            //        server.connection.send_udp(
            //            server.connection.all_connection_ids(),
            //            SerializedClientCommand::from(
            //                &ChatMessage::new(format!("{} was disconnected.", player.name))
            //            ).data
            //        );
            //    }
            //}

            let ServerUpdate {
                messages,
                connects,
                disconnects
            } = server.connection.update();

            for addr in connects {
                println!("Connection from {}", addr);
            }
            for addr in disconnects {
                println!("Disconnect from {}", addr);
            }
            for (protocol, addr, message) in messages {
                println!("Message from {} over {}: {}", addr, match protocol {
                    Protocol::TCP => "TCP",
                    Protocol::UDP => "UDP"
                }, String::from_utf8_lossy(&message));
            }

            std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
        }
    }
}
