use crate::{client::{commands::ClientCommand, game::Game}, networking::Protocol};
use super::commands::{PlayerDataPayload, ChatMessage, IndicateClientPlayer};

impl <'a> ClientCommand<'a> for PlayerDataPayload {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.character_name.clear();
        for player in self.0.players.values() {
            match player.selected_char {
                Some(cid) => {
                    game.character_name.insert(cid, player.name.clone())
                },
                None => None
            };
        }
        game.players = self.0;
        game.chatbox.println("Updated players");
    }
}

impl <'a> ClientCommand<'a> for ChatMessage {
    fn run(self, (_, game):(Protocol, &mut Game)) {
        game.chatbox.println(self.0.as_str());
    }
}

impl<'a> ClientCommand<'a> for IndicateClientPlayer {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.selected_player = self.0;
        game.chatbox.println(format!("New player selection: {:?}", self.0).as_str());
    }
}
