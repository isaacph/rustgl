use std::time::Duration;
use crate::model::Subscription;
use crate::model::commands::ClientCommandID;
use crate::model::player::commands::{ChatMessage, PlayerDataPayload};
use crate::model::player::model::{PlayerManager, PlayerManagerUpdate, PlayerDataView};
use crate::model::world::World;
use crate::model::world::character::CharacterIDGenerator;
use crate::networking::Protocol;
use crate::networking::server::{Server as Connection, ServerUpdate};
use crate::server::commands::{SendCommands, execute_server_command};
use self::update_loop::UpdateLoop;

pub mod update_loop {
    use std::time::{SystemTime, Duration};
    use crate::model::{commands::ClientCommandID, world::World};

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

        pub fn send_next_update(&mut self, world: &World) -> Vec<Box<[u8]>> {
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
                        .map(|cmd| (&cmd).make_bytes())
                        .collect()
            }
        }
    }
}


pub struct Server {
    pub stop: bool,
    pub world: World,
    pub character_id_gen: CharacterIDGenerator,
    pub player_manager: PlayerManager,
    pub connection: Connection
}


impl Server {
    pub fn run(ports: (u16, u16)) -> Result<(), std::io::Error> {
        let mut server = {
            let world = World::new();
            Server {
                stop: false,
                world,
                character_id_gen: CharacterIDGenerator::new(),
                player_manager: PlayerManager::new(),
                connection: Connection::init(ports)?
            }
        };
        let mut update_loop = UpdateLoop::init(&server.world);

        while !server.stop {
            let ServerUpdate {
                messages,
                connects,
                disconnects
            } = server.connection.update();

            let updates: Vec<PlayerManagerUpdate> = server.player_manager.updates.drain(0..).collect();
            for update in updates {
                match update {
                    PlayerManagerUpdate::PlayerLogIn(player_id, _) => {
                        if let Some(player) = server.player_manager.get_player(&player_id) {
                            let name = String::from(&player.name);
                            server.broadcast(Subscription::Chat, Protocol::TCP, &ChatMessage(format!("{} logged in.", name)));
                            server.broadcast(Subscription::Chat, Protocol::TCP, &PlayerDataPayload(server.player_manager.get_view()));
                        }
                    },
                    PlayerManagerUpdate::PlayerLogOut(player_id, addr) => {
                        if let Some(player) = server.player_manager.get_player(&player_id) {
                            let chat_msg = ChatMessage(format!("{} logged out.", player.name));
                            server.broadcast(Subscription::Chat, Protocol::TCP, &chat_msg);
                            server.broadcast(Subscription::Chat, Protocol::TCP, &PlayerDataPayload(server.player_manager.get_view()));
                            // only send update to player if they are no longer logged into any
                            // accounts
                            if server.player_manager.get_connected_player(&addr).is_none() {
                                server.connection.send(Protocol::TCP, &addr, &chat_msg).ok();
                            }
                        }
                    }
                }
            }

            for addr in connects {
                println!("Connection from {}", addr);
            }
            for addr in disconnects {
                println!("Disconnect from {}", addr);
                if let Some(id) = server.player_manager.get_connected_player(&addr) {
                    server.player_manager.map_existing_player(None, Some(&id));
                }
            }
            for (protocol, addr, message) in messages {
                println!("Message from {} over {}: {}", addr, match protocol {
                    Protocol::TCP => "TCP",
                    Protocol::UDP => "UDP"
                }, String::from_utf8_lossy(&message));
                match execute_server_command(&message, ((protocol, &addr), &mut server)) {
                    Ok(()) => println!("Ran command"),
                    Err(err) => println!("Error running command: {}", err)
                }
            }

            let update_data = update_loop.send_next_update(&server.world);
            server.broadcast_data(Subscription::World, Protocol::UDP, &update_data);

            std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        }
        Ok(())
    }

    pub fn broadcast<T>(&mut self, sub: Subscription, protocol: Protocol, message: &T) where T: ClientCommandID {
        self.broadcast_data(sub, protocol, &vec![message.make_bytes()]);
    }

    pub fn broadcast_data(&mut self, sub: Subscription, protocol: Protocol, message: &Vec<Box<[u8]>>) {
        for id in self.player_manager.all_player_ids().iter() {
            if let (Some(addr), Some(subs)) = (self.player_manager.get_player_connection(id), self.player_manager.get_player_subscriptions(id)) {
                if subs.iter().any(|player_sub| *player_sub == sub) {
                    for message in message {
                        match protocol {
                            Protocol::TCP => match self.connection.send_data(protocol, &addr, message.clone()) {
                                Ok(()) => (), Err(err) => println!("Error sending TCP message to {}: {}", addr, err)
                            },
                            Protocol::UDP => match self.connection.send_data(protocol, &addr, message.clone()) {
                                Ok(()) => (), Err(err) => println!("Error sending UDP message to {}: {}", addr, err)
                            }
                        }
                    }
                }
            }
        }
    }
}
