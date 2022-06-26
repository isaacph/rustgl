use std::net::SocketAddr;
use serde::{Serialize, Deserialize};

use crate::{model::{commands::{player::{ChatMessage, PlayerLogIn, PlayerLogOut, PlayerDataPayload}}, world::player::{PlayerDataView, PlayerID}}, networking::Protocol, server::main::Server};
use super::{ProtocolServerCommand, ProtocolSpec, SendCommands};

pub trait PlayerCommand<'a>: Deserialize<'a> + Serialize {
    const PROTOCOL: ProtocolSpec;
    fn run(self, player_id: PlayerID, server: &mut Server);
}

impl<'a, T: PlayerCommand<'a>> ProtocolServerCommand<'a> for T {
    const PROTOCOL: ProtocolSpec = T::PROTOCOL;
    fn run(self, tcp_addr: &SocketAddr, server: &mut Server) {
        match server.player_manager.get_connected_player_mut(tcp_addr) {
            Some(player) => self.run(player.id, server),
            None =>
            match server.connection.send(Protocol::TCP, tcp_addr, &ChatMessage(format!("Could not run player command, you are not logged in"))) {
                Ok(()) => (), Err(err) => println!("Error sending error message to player that is not logged in: {}, err: {}", tcp_addr, err)
            }
        }
    }
}

impl<'a> ProtocolServerCommand<'a> for PlayerLogIn {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, addr: &SocketAddr, server: &mut Server) {
        match {
            if self.existing {
                if let Some(name) = &self.name {
                    match if let Some(player) = server.player_manager.get_player_with_name(name) {
                        Ok(player.id)
                    } else {
                        Err(format!("Cannot sign in: player with name {} is not found", name))
                    } {
                        Ok(pid) => {
                            Ok(server.player_manager.map_existing_player(Some(addr), Some(&pid)).unwrap().name.clone())
                        },
                        Err(x) => Err(x)
                    }
                } else {
                    Err(format!("Cannot sign into unnamed character"))
                }
            } else {
                let player = server.player_manager.create_player(Some(*addr), self.name);
                Ok(player.name.clone())
            }
        } {
            Ok(player_name) => {
                server.broadcast(Protocol::TCP, &ChatMessage(format!("{} logged in.", player_name)));
                server.broadcast(Protocol::TCP, &PlayerDataPayload(server.player_manager.get_view()));
            },
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
    fn run(self, addr: &SocketAddr, server: &mut Server) {
        if let Some(player) = server.player_manager.map_existing_player(Some(addr), None) {
            let msg = ChatMessage(format!("{} logged out.", player.name));
            server.broadcast(Protocol::TCP, &msg);
            match server.connection.send(Protocol::TCP, addr, &msg) {_ => ()}
        } else {
            match server.connection.send(Protocol::TCP, addr, &ChatMessage(format!("Failed to log out, was not logged in"))) {
                Ok(()) => (), Err(err) => println!("Error sending message to {}: {}", addr, err)
            }
        }
    }
}

impl<'a> PlayerCommand<'a> for ChatMessage {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);
    fn run(self, id: PlayerID, server: &mut Server) {
        server.broadcast(Protocol::TCP, &ChatMessage(format!("<{}> {}", server.player_manager.get_player(&id).unwrap().name, self.0)))
    }
}