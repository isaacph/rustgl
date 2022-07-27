use crate::{model::world::commands::UpdateCharacter, networking::Protocol, client::{game::Game, commands::ClientCommand}};

use super::{commands::ClearWorld, World};

impl<'a> ClientCommand<'a> for UpdateCharacter {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        self.update_character(&mut game.world);
    }
}

impl<'a> ClientCommand<'a> for ClearWorld {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.world = World::new();
        for cid in game.world.characters.clone() {
            game.world.erase_character(&cid).ok();
        }
    }
}
