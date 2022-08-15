use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use crate::model::action_queue::ActionQueue;
use crate::model::world::commands::{WorldCommand, RunWorldCommand};
use crate::model::world::logging::Logger;
use crate::model::{Subscription, PrintError, TICK_RATE};
use crate::model::commands::{GetCommandID, MakeBytes};
use crate::model::player::commands::{ChatMessage, PlayerDataPayload, IndicateClientPlayer};
use crate::model::player::model::{PlayerManager, PlayerManagerUpdate, PlayerDataView};
use crate::model::world::{World, WorldError, CharacterCommandState, WorldErrorI};
use crate::model::world::character::{CharacterIDGenerator, CharacterID};
use crate::networking::Protocol;
use crate::networking::server::{Server as Connection, ServerUpdate};
use self::update_loop::UpdateLoop;

use super::commands::{SendCommands, execute_server_command};

pub mod update_loop {
    use std::time::{Duration, Instant};
    use crate::model::{world::{World, commands::FixWorld}, commands::MakeBytes, WorldTick};

    pub struct UpdateLoop {
        last_update: Option<Instant>,
        update_interval: Duration,
        pub errors: Vec<String>
    }

    impl UpdateLoop {
        pub fn init(_world: &World) -> UpdateLoop {
            UpdateLoop {
                last_update: Some(Instant::now()),
                update_interval: Duration::new(0, 1000 * 1000 * 1000),
                errors: vec![],
            }
        }

        pub fn send_next_update(&mut self, world: &World, now: Instant, tick: WorldTick, tick_ordering: &mut u32) -> Vec<Box<[u8]>> {
            let should_update = match self.last_update {
                None => true,
                Some(time) => now - time > self.update_interval
            };
            match should_update {
                false => vec![],
                true => {
                    self.last_update = Some(now);
                    let x: Vec<Box<[u8]>> = world.characters.iter()
                        .filter_map(|cid| world.make_cmd_update_character(*cid))
                        .filter_map(|cmd| match
                                    (&FixWorld {
                                        update: cmd,
                                        tick,
                                        ordering: {
                                            let x = *tick_ordering;
                                            *tick_ordering += 1;
                                            x
                                        },
                                    })
                                    // (&RunWorldCommand {
                                    //     command: WorldCommand::Update(cmd),
                                    //     tick,
                                    //     ordering: {
                                    //         let x = *tick_ordering;
                                    //         *tick_ordering += 1;
                                    //         x
                                    //     }
                                    // })
                        .make_bytes() {
                            Ok(bytes) => Some(bytes),
                            Err(err) => {
                                self.errors.push(format!("Error serializing update command: {}", err));
                                None
                            }
                        })
                        .collect();
                    x
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
    pub tick_ordering: u32,
    pub world_commands: Vec<WorldCommand>,
    pub action_queues: HashMap<CharacterID, ActionQueue>,
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
                tick_ordering: 0,
                world_commands: vec![],
                action_queues: Default::default(),
            }
        };
        let mut update_loop = UpdateLoop::init(&server.world);

        // match test::make_attack_circle(10, 10.0, server.character_id_gen.generate_range(100000), &mut server.world) {
        //     Ok(()) => (),
        //     Err(err) => println!("??????? {:?}", err)
        // }
        // match test::make_mover(server.character_id_gen.generate(), &mut server.world) {
        //     Ok(()) => (),
        //     Err(err) => println!("??????? {:?}", err)
        // }

        let mut logger = Logger::init("server.log").unwrap();

        let mut tick_timer = 0.0;
        let mut last_time = std::time::Instant::now();
        while !server.stop {
            let current_time = std::time::Instant::now();
            let delta_duration = current_time - last_time;
            last_time = current_time;
            let delta_time = delta_duration.as_secs_f32();


            let ServerUpdate {
                mut messages,
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

            for (protocol, addr, message) in messages.drain(0..messages.len()) {
                match execute_server_command(&message, ((protocol, &addr), &mut server)) {
                    Ok(()) => (),// println!("Ran command"),
                    Err(err) => println!("Error running command: {}", err)
                }
            }

            tick_timer += delta_time;
            while tick_timer >= 1.0 / TICK_RATE {
                let delta_time = 1.0 / TICK_RATE;
                tick_timer -= delta_time;

                let update_data = update_loop.send_next_update(&server.world, current_time, server.world.tick, &mut server.tick_ordering);
                server.broadcast_data(Subscription::World, Protocol::UDP, &update_data);

                let mut t_o = server.tick_ordering;
                let mut commands = server.world_commands.clone();
                server.world_commands.clear();

                // add in player queued commands
                let mut forget_queues = vec![];
                for (cid, queue) in &mut server.action_queues {
                    if server.world.characters.get(cid).is_none() {
                        forget_queues.push(*cid);
                    } else if let Some(action) = &queue.next_action {
                        match server.world.validate_command(action) {
                            Ok(Some(CharacterCommandState::Ready)) => {
                                println!("Command in queue ready: {:?}", action);
                                commands.push(action.clone());
                                queue.next_action = None;
                            },
                            Ok(Some(CharacterCommandState::Queued)) => println!("Command in queue: {:?}", action),
                            Ok(_) => queue.next_action = None,
                            Err(err) => server.world.errors.push(err),
                        }
                    }
                }
                for cid in forget_queues {
                    server.action_queues.remove(&cid);
                }

                for command in &commands {
                    server.broadcast(Subscription::World, Protocol::UDP, &RunWorldCommand {
                        command: command.clone(),
                        tick: server.world.tick,
                        ordering: {
                            let ordering = t_o;
                            t_o += 1;
                            ordering
                        },
                    });
                }
                server.tick_ordering = t_o;
                server.world = server.world.update(&commands, delta_time);
                logger.log(&server.world);
                for error in server.world.errors.drain(0..server.world.errors.len()) {
                    match error {
                        WorldError(WorldErrorI::Info(st)) => println!("Tick {}, {}", server.world.tick, st),
                        _ => println!("Server world error: {:?}", error),
                    }
                }

                server.tick_ordering = 0;
            }

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

    pub fn run_world_command(&mut self, addr: Option<&SocketAddr>, command: WorldCommand) {
        let res = self.world.validate_command(&command);
        if let Ok(res) = res {
            match (&command, res) {
                (WorldCommand::CharacterComponent(cid, _, _), Some(_)) => {
                    let cid = *cid;
                    self.action_queues
                        .entry(cid)
                        .or_insert(ActionQueue { next_action: None })
                        .next_action = Some(command);
                },
                _ => self.world_commands.push(command)
            }
        } else {
            match addr {
                Some(addr) =>
                    self.connection.send(Protocol::TCP, addr, &ChatMessage(format!("Error validating wcmd on server: {:?}", res))).print(),
                None => println!("Error running wcmd locally: {:?}", res)
            }
        }
        // match match self.world.run_command(self.tick, command.clone()) {
        //     Ok(status) => match status {
        //         CommandRunResult::Valid | CommandRunResult::ValidError(_) => {
        //             if let CommandRunResult::ValidError(err) = status {
        //                 self.world.errors.push(err);
        //             }
        //             self.broadcast(Subscription::World, Protocol::UDP, &RunWorldCommand {
        //                 tick: self.tick,
        //                 command,
        //                 ordering: order
        //             });
        //             Ok(())
        //         },
        //         CommandRunResult::Invalid(err) => Err(err),
        //     },
        //     Err(err) => Err(err)
        // } {
        //     Ok(()) => (),
        //     Err(WorldError::NoopCommand) => (), // the command did nothing
        //     // Err(WorldError::IllegalInterrupt(_)) => (), // we are ignoring interrupts
        //     Err(err) => match addr {
        //         Some(addr) => return self.connection.send(Protocol::TCP, addr, &ChatMessage(format!("Error running world command: {:?}", err))).print(),
        //         None => println!("Error running anonymous world command: {:?}", err),
        //     }
        // }
    }
}
