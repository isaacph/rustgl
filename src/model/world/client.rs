use std::collections::HashMap;

use crate::{networking::Protocol, client::{game::{Game, TickCommand}, commands::ClientCommand}, model::world::commands::WorldCommand};

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
        // println!("Add new command {:?} to tick {}", self.command, self.tick);

        // sum up relative ticks to find which is most popular
        let offset = match self.command {
            _ => 0,
        };
        let server_tick_slot = game.tick_count.entry(game.tick_base + offset).or_insert_with(HashMap::new);
        *server_tick_slot.entry(self.tick - game.tick_base + offset).or_insert(0) += 1;

        // add command to its correct tick
        let command = TickCommand::WorldCommand(self.command);
        match game.tick_commands.get_mut(&self.tick) {
            Some(commands) => {
                commands.insert(
                    commands.iter()
                        .position(|(other_ord, _)| self.ordering < *other_ord)
                        .unwrap_or(commands.len()),
                    (self.ordering, command)
                );
            },
            None => {
                game.tick_commands.insert(self.tick, vec![(self.ordering, command)]);
            },
        }
    }
}
