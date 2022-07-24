use std::net::SocketAddr;
use std::time::Duration;
use crate::model::world::commands::WorldCommand;
use crate::model::{Subscription, PrintError, TICK_RATE};
use crate::model::commands::{GetCommandID, MakeBytes};
use crate::model::player::commands::{ChatMessage, PlayerDataPayload, IndicateClientPlayer};
use crate::model::player::model::{PlayerManager, PlayerManagerUpdate, PlayerDataView};
use crate::model::world::{World, WorldError};
use crate::model::world::character::CharacterIDGenerator;
use crate::networking::Protocol;
use crate::networking::server::{Server as Connection, ServerUpdate};
use crate::server::commands::{SendCommands, execute_server_command};
use self::update_loop::UpdateLoop;

pub mod update_loop {
    use std::time::{Duration, Instant};
    use crate::model::{world::World, commands::MakeBytes};

    pub struct UpdateLoop {
        last_update: Option<Instant>,
        update_interval: Duration,
        pub errors: Vec<String>
    }

    impl UpdateLoop {
        pub fn init(_world: &World) -> UpdateLoop {
            UpdateLoop {
                last_update: Some(Instant::now()),
                update_interval: Duration::new(1, 0),
                errors: vec![],
            }
        }

        pub fn send_next_update(&mut self, world: &World, now: Instant) -> Vec<Box<[u8]>> {
            let should_update = match self.last_update {
                None => true,
                Some(time) => now - time > self.update_interval
            };
            match should_update {
                false => vec![],
                true => {
                    self.last_update = Some(now);
                    world.characters.iter()
                        .filter_map(|cid| world.make_cmd_update_character(*cid))
                        .filter_map(|cmd| match (&cmd).make_bytes() {
                            Ok(bytes) => Some(bytes),
                            Err(err) => {
                                self.errors.push(format!("Error serializing update command: {}", err));
                                None
                            }
                        })
                        .collect()
                }
            }
        }
    }
}


pub struct Server {
    pub stop: bool,
    pub world: World,
    pub character_id_gen: CharacterIDGenerator,
    pub player_manager: PlayerManager,
    pub connection: Connection,
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
                connection: Connection::init(ports)?,
            }
        };
        let mut update_loop = UpdateLoop::init(&server.world);

        let mut tick_timer = 0.0;

        let mut last_time = std::time::Instant::now();
        while !server.stop {
            let current_time = std::time::Instant::now();
            let delta_duration = current_time - last_time;
            last_time = current_time;
            let delta_time = delta_duration.as_secs_f32();

            tick_timer += delta_time;
            while tick_timer >= 1.0 / TICK_RATE {
                let delta_time = 1.0 / TICK_RATE;
                tick_timer -= delta_time;
                server.world.update(delta_time);
                for error in server.world.errors.drain(0..server.world.errors.len()) {
                    println!("Server world error: {:?}", error);
                }
            }

            let ServerUpdate {
                messages,
                connects,
                disconnects
            } = server.connection.update();

            let updates: Vec<PlayerManagerUpdate> = server.player_manager.updates.drain(0..).collect();
            let changed = !updates.is_empty();
            for update in updates {
                match update {
                    PlayerManagerUpdate::PlayerLogIn(player_id, addr) => {
                        if let Some(player) = server.player_manager.get_player(&player_id) {
                            let name = String::from(&player.name);
                            server.broadcast(Subscription::Chat, Protocol::TCP, &ChatMessage(format!("{} logged in.", name)));
                            server.connection.send(Protocol::TCP, &addr, &IndicateClientPlayer(Some(player_id))).print()
                        }
                    },
                    PlayerManagerUpdate::PlayerLogOut(player_id, addr) => {
                        if let Some(player) = server.player_manager.get_player(&player_id) {
                            let chat_msg = ChatMessage(format!("{} logged out.", player.name));
                            server.broadcast(Subscription::Chat, Protocol::TCP, &chat_msg);
                            // only send update to player if they are no longer logged into any
                            // accounts
                            if server.player_manager.get_connected_player(&addr).is_none() {
                                server.connection.send(Protocol::TCP, &addr, &chat_msg).ok();
                                server.connection.send(Protocol::TCP, &addr, &IndicateClientPlayer(None)).print()
                            }
                        }
                    },
                    PlayerManagerUpdate::PlayerInfoUpdate(_) => ()
                }
            }
            if changed {
                server.broadcast(Subscription::Chat, Protocol::TCP, &PlayerDataPayload(server.player_manager.get_view()));
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
                // println!("Message from {} over {}", addr, match protocol {
                //     Protocol::TCP => "TCP",
                //     Protocol::UDP => "UDP"
                // });
                match execute_server_command(&message, ((protocol, &addr), &mut server)) {
                    Ok(()) => (),// println!("Ran command"),
                    Err(err) => println!("Error running command: {}", err)
                }
            }

            let update_data = update_loop.send_next_update(&server.world, current_time);
            server.broadcast_data(Subscription::World, Protocol::UDP, &update_data);

            for error in update_loop.errors.drain(0..update_loop.errors.len()) {
                println!("Update loop error: {}", error);
            }

            std::thread::sleep(Duration::new(0, 1000000 * 16)); // wait 16 ms
        }
        Ok(())
    }

    pub fn broadcast<T>(&mut self, sub: Subscription, protocol: Protocol, message: &T) where T: GetCommandID {
        match message.make_bytes() {
            Ok(bytes) => self.broadcast_data(sub, protocol, &vec![bytes]),
            Err(err) => println!("Error serializing broadcast command {:?}: {}", message.command_id(), err),
        }
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

    pub fn run_world_command<'a, T: WorldCommand<'a>>(&mut self, addr: Option<&SocketAddr>, mut command: T) {
        match command.validate(&self.world) {
            Ok(()) => (),
            Err(WorldError::NoopCommand) => (), // the command did nothing
            // Err(WorldError::IllegalInterrupt(_)) => (), // we are ignoring interrupts
            Err(err) => match addr {
                Some(addr) => return self.connection.send(Protocol::TCP, addr, &ChatMessage(format!("Error running world command: {:?}", err))).print(),
                None => println!("Error running anonymous world command: {:?}", err),
            }
        };
        match command.run(&mut self.world) {
            Ok(()) => self.broadcast(Subscription::World, Protocol::UDP, &command),
            Err(err) => match addr {
                Some(addr) => self.connection.send(Protocol::TCP, addr, &ChatMessage(format!("Error running world command: {:?}", err))).print(),
                None => println!("Error running anonymous world command: {:?}", err),
            }
        }
    }
}
