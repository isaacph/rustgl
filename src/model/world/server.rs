use std::net::SocketAddr;

use crate::{model::{player::{server::PlayerCommand, model::{PlayerID, PlayerDataView}, commands::ChatMessage}, Subscription, PrintError}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
use super::{commands::{UpdateCharacter, GenerateCharacter, ListChar, EnsureCharacter}, character::CharacterType};

impl<'a> PlayerCommand<'a> for UpdateCharacter {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::Both;

    fn run(self, _: &std::net::SocketAddr, _: &PlayerID, server: &mut Server) {
        // TODO: validate update command
        server.broadcast(Subscription::World, Protocol::UDP, &self);
        self.update_character(&mut server.world);
    }
}

impl<'a> PlayerCommand<'a> for GenerateCharacter {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::Both;

    fn run(self, tcp_addr: &std::net::SocketAddr, player_id: &PlayerID, server: &mut Server) {
        let id = match server.world.create_character(&mut server.character_id_gen, self.0) {
            Ok(id) => {
                server.connection.send(
                    Protocol::TCP,
                    tcp_addr,
                    &ChatMessage(format!("Character generated, ID: {:?}", id))
                ).print();
                id
            },
            Err(err) => return server.connection.send(
                Protocol::TCP,
                tcp_addr,
                &ChatMessage(format!("Failed to generate character: {:?}", err))
            ).print()
        };
        if let Some(cmd) = server.world.make_cmd_update_character(id) {
            server.broadcast(Subscription::World, Protocol::UDP, &cmd);
            match server.player_manager.get_player_mut(player_id) {
                Some(player) => player.selected_char = Some(id),
                None => ()
            }
        } else {
            server.connection.send(Protocol::TCP, tcp_addr, &ChatMessage("Failed to generate character".to_string())).print();
        };
    }
}

impl<'a> PlayerCommand<'a> for ListChar {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::TCP);

    fn run(self, addr: &SocketAddr, _: &PlayerID, server: &mut Server) {
        server.connection.send(Protocol::TCP, addr, &ChatMessage(
            format!("Character list:\n{}", {
                let x: Vec<String> = server.world.characters.iter().map(
                    |cid| format!("{:?}", cid)
                ).collect();
                x.join(", ")
            })
        )).print();
    }
}

impl<'a> PlayerCommand<'a> for EnsureCharacter {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::Both;

    fn run(self, addr: &SocketAddr, pid: &PlayerID, server: &mut Server) {
        match server.player_manager.get_player(pid) {
            Some(player) => {
                if player.selected_char.is_none() {
                    match server.player_manager.get_player_mut(pid) {
                        Some(_) => GenerateCharacter(CharacterType::IceWiz).run(addr, pid, server),
                        None => server.connection.send(Protocol::TCP, addr, &ChatMessage("Player not found".to_string())).print()
                    }
                }
            },
            None => server.connection.send(Protocol::TCP, addr, &ChatMessage("Player not found".to_string())).print()
        }
    }
}
