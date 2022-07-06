use crate::{model::commands::player::{PlayerDataPayload, ChatMessage}, networking::Protocol, client::game::Game};

use super::ClientCommand;

impl <'a> ClientCommand<'a> for PlayerDataPayload {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.world.players = self.0;
        game.chatbox.println("Updated players");
    }
}

impl <'a> ClientCommand<'a> for ChatMessage {
    fn run(self, (_, game):(Protocol, &mut Game)) {
        game.chatbox.println(self.0.as_str());
    }
}

