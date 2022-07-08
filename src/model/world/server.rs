use crate::{model::{player::{server::PlayerCommand, model::PlayerID, commands::ChatMessage}, Subscription, PrintError}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
use super::commands::{UpdateCharacter, GenerateCharacter, MoveCharacter};

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

    fn run(self, tcp_addr: &std::net::SocketAddr, _: &PlayerID, server: &mut Server) {
        let id = Self::generate_character(&mut server.world, &mut server.character_id_gen);
        if let Some(cmd) = server.world.make_cmd_update_character(id) {
            server.broadcast(Subscription::World, Protocol::UDP, &cmd);
        } else {
            server.connection.send(Protocol::TCP, tcp_addr, &ChatMessage("Failed to generate character".to_string())).print();
        };
    }
}

impl<'a> PlayerCommand<'a> for MoveCharacter {
    const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);

    fn run(self, _: &std::net::SocketAddr, player_id: &PlayerID, server: &mut Server) {
        // get the character at self.id
        // check if character can move the character
        // start the movement of the character
        // tell everyone the character has started moving
    }
}
