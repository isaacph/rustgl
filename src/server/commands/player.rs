use std::net::SocketAddr;
use serde::{Serialize, Deserialize};

use crate::{model::{commands::player::{ChatMessage, PlayerLogIn, PlayerLogOut, GetPlayerData, PlayerDataPayload, PlayerSubs, PlayerSubCommand}, world::player::{PlayerDataView, PlayerID}, Subscription, PrintError}, networking::Protocol, server::main::Server};
use super::{ProtocolServerCommand, ProtocolSpec, SendCommands};

pub trait PlayerCommand<'a>: Deserialize<'a> + Serialize {
    const PROTOCOL: ProtocolSpec;
    fn run(self, addr: &SocketAddr, player_id: &PlayerID, server: &mut Server);
}

impl<'a, T: PlayerCommand<'a>> ProtocolServerCommand<'a> for T {
    const PROTOCOL: ProtocolSpec = T::PROTOCOL;
    fn run(self, _: Protocol, tcp_addr: &SocketAddr, server: &mut Server) {
        match server.player_manager.get_connected_player(tcp_addr) {
            Some(player_id) => self.run(tcp_addr, &player_id, server),
            None =>
            match server.connection.send(Protocol::TCP, tcp_addr, &ChatMessage("Could not run player command, you are not logged in".to_string())) {
                Ok(()) => (), Err(err) => println!("Error sending error message to player that is not logged in: {}, err: {}", tcp_addr, err)
            }
        }
    }
}

impl<'a> ProtocolServerCommand<'a> for PlayerLogIn {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, _: Protocol, addr: &SocketAddr, server: &mut Server) {
        match {
            if self.existing {
                if let Some(name) = &self.name {
                    match if let Some(player) = server.player_manager.get_player_with_name(name) {
                        Ok(player.id)
                    } else {
                        Err(format!("Cannot sign in: player with name {} is not found", name))
                    } {
                        Ok(pid) => {
                            if server.player_manager.is_connected(&pid).is_none() {
                                let name = server.player_manager.map_existing_player(Some(addr), Some(&pid)).unwrap().name.clone();
                                if let Some(subs) = server.player_manager.get_player_subscriptions_mut(&pid) {
                                    subs.insert(Subscription::Chat);
                                    Ok(name)
                                } else {
                                    Err("Error: player missing metadata!".to_string())
                                }
                            } else {
                                Err(format!("Cannot sign in: player already signed into {}", name))
                            }
                        },
                        Err(x) => Err(x)
                    }
                } else {
                    Err("Cannot sign into unnamed character".to_string())
                }
            } else {
                let player = server.player_manager.create_player(Some(*addr), self.name);
                let (name, id) = (player.name.clone(), player.id);
                if let Some(subs) = server.player_manager.get_player_subscriptions_mut(&id) {
                    subs.insert(Subscription::Chat);
                    Ok(name)
                } else {
                    Err("Error: player missing metadata!".to_string())
                }
            }
        } {
            Ok(_) => (),
            Err(e) => {
                match server.connection.send(
                    Protocol::TCP,
                    addr,
                    &ChatMessage(e)
                ) {
                    Ok(()) => (),
                    Err(err) => println!("Error sending UDP to client {}: {}", addr, err)
                }
            }
        }
    }
}

impl<'a> ProtocolServerCommand<'a> for PlayerLogOut {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, _: Protocol, addr: &SocketAddr, server: &mut Server) {
        if server.player_manager.get_connected_player(addr).is_none() {
            match server.connection.send(Protocol::TCP, addr, &ChatMessage("Failed to log out, was not logged in".to_string())) {
                Ok(()) => (), Err(err) => println!("Error sending message to {}: {}", addr, err)
            }
        } else {
            server.player_manager.map_existing_player(Some(addr), None);
        }
    }
}

impl<'a> PlayerCommand<'a> for ChatMessage {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, _: &SocketAddr, id: &PlayerID, server: &mut Server) {
        server.broadcast(Subscription::Chat, Protocol::TCP, &ChatMessage(format!("<{}> {}", server.player_manager.get_player(id).unwrap().name, self.0)))
    }
}

impl <'a> PlayerCommand<'a> for GetPlayerData {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, addr: &SocketAddr, _: &PlayerID, server: &mut Server) {
        let cmd = PlayerDataPayload(server.player_manager.get_view());
        server.connection.send(Protocol::TCP, addr, &cmd).print();
    }
}

impl<'a> PlayerCommand<'a> for PlayerSubs {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, addr: &SocketAddr, player_id: &PlayerID, server: &mut Server) {
        match self.0 {
            PlayerSubCommand::ListSubs => match server.player_manager.get_player_subscriptions(player_id) {
                Some(subs) => {
                    let mut str = String::from("Player subscriptions: ");
                    for sub in subs {
                        str += format!("{}, ", sub).as_str();
                    }
                    server.connection.send(Protocol::TCP, addr, &ChatMessage(str)).print();
                },
                None => ()
            }
            PlayerSubCommand::AddSubs(to_add) => {
                let mut added = vec![];
                if let Some(subs) = server.player_manager.get_player_subscriptions_mut(player_id) {
                    for sub in to_add {
                        if subs.insert(sub.clone()) {
                            added.push(sub);
                        }
                    }
                }
                let msg: String = String::from("Added subcriptions: ") +
                    &added.iter().map(|add| {
                    format!("{:?}", add)
                }).collect::<Vec<String>>().join(" ");
                server.connection.send(Protocol::TCP, addr, &ChatMessage(msg)).print();
            },
            PlayerSubCommand::DelSubs(to_del) => {
                let mut deleted = vec![];
                if let Some(subs) = server.player_manager.get_player_subscriptions_mut(player_id) {
                    for sub in to_del {
                        if subs.remove(&sub) {
                            deleted.push(sub);
                        }
                    }
                }
                let msg: String = String::from("Deleted subcriptions: ") +
                    &deleted.iter().map(|add| {
                    format!("{:?}", add)
                }).collect::<Vec<String>>().join(" ");
                server.connection.send(Protocol::TCP, addr, &ChatMessage(msg)).print();
            },
            PlayerSubCommand::SetSubs(new_subs) => {
                if let Some(subs) = server.player_manager.get_player_subscriptions_mut(player_id) {
                    subs.drain();
                    subs.extend(new_subs);
                    let msg = String::from("Replaced subscriptions: ") +
                        &subs.iter().map(|s| format!("{:?}", s))
                        .collect::<Vec<String>>().join(" ");
                    server.connection.send(Protocol::TCP, addr, &ChatMessage(msg)).print();
                } else {
                    server.connection.send(Protocol::TCP, addr, &ChatMessage("Failed to replace subscriptions, player metadata is missing".to_string())).print();
                }
            },
        }
    }
}
