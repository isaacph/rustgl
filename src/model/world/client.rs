use crate::{networking::Protocol, client::{game::Game, commands::ClientCommand}};

use super::{commands::{ClearWorld, RunWorldCommand}, World};

// impl<'a> ClientCommand<'a> for UpdateCharacter {
//     fn run(self, (_, game): (Protocol, &mut Game)) {
//         self.update_character(&mut game.world);
//     }
// }

impl<'a> ClientCommand<'a> for ClearWorld {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.world = World::new();
        for cid in game.world.characters.clone() {
            game.world.erase_character(&cid).ok();
        }
    }
}

impl<'a> ClientCommand<'a> for RunWorldCommand {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        // todo: discard old ticks
        println!("Add new command {:?} to tick {}", self.command, self.tick);

        // sum up relative ticks to find which is most popular
        if let Some(op) = game.server_tick_options.get_mut(&self.tick.overflowing_sub(game.tick_base).0) {
            *op += 1;
        } else {
            game.server_tick_options.insert(self.tick.overflowing_sub(game.tick_base).0, 1);
        }

        // add command to its correct tick
        match game.tick_commands.get_mut(&self.tick) {
            Some(commands) => commands.push(self.command),
            None => {
                game.tick_commands.insert(self.tick, vec![self.command]);
            },
        }
    }
}
