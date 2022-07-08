use crate::{model::world::commands::UpdateCharacter, networking::Protocol, client::{game::Game, commands::ClientCommand}};

impl<'a> ClientCommand<'a> for UpdateCharacter {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        self.update_character(&mut game.world);
    }
}
