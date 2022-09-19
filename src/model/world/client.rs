use std::collections::HashMap;

use crate::{networking::Protocol, client::{game::{Game, TickCommand}, commands::ClientCommand}};

use super::{commands::{ClearWorld, RunWorldCommand, FixWorld}, World, system::collision::CollisionInfo};

// impl<'a> ClientCommand<'a> for UpdateCharacter {
//     fn run(self, (_, game): (Protocol, &mut Game)) {
//         self.update_character(&mut game.world);
//     }
// }

impl<'a> ClientCommand<'a> for ClearWorld {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        game.world = World::new(CollisionInfo::test_collision());
        for cid in game.world.characters.clone() {
            game.world.erase_character(&cid).ok();
        }
    }
}

impl<'a> ClientCommand<'a> for FixWorld {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        let command = TickCommand::FixWorld(self);
        add_tick_command(command, game);
    }
}

impl<'a> ClientCommand<'a> for RunWorldCommand {
    fn run(self, (_, game): (Protocol, &mut Game)) {
        // println!("Add new command {:?} to tick {}", self.command, self.tick);
        let command = TickCommand::WorldCommand(self.tick, self.ordering, self.command);
        add_tick_command(command, game);
    }
}

pub fn add_tick_command(command: TickCommand, game: &mut Game) {
    // todo: discard old ticks
    let offset = 0;
    let (tick, ordering) = match &command {
        TickCommand::WorldCommand(tick, ordering, _wc) => (tick, ordering),
        TickCommand::FixWorld(FixWorld { update: _, ordering, tick }) => (tick, ordering),
    };
    let server_tick_slot = game.tick_count.entry(game.tick_base + offset).or_insert_with(HashMap::new);
    *server_tick_slot.entry(tick - game.tick_base + offset).or_insert(0) += 1;

    // add command to its correct tick in the sorted position according to "ordering"
    match game.tick_commands.get_mut(tick) {
        Some(commands) => {
            commands.insert(
                commands.iter()
                    .position(|(other_ord, _)| ordering < other_ord)
                    .unwrap_or(commands.len()),
                (*ordering, command)
            );
        },
        None => {
            game.tick_commands.insert(*tick, vec![(*ordering, command)]);
        },
    }
}
