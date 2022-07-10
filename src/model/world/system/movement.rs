use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::world::{World, character::CharacterID, WorldError};

#[derive(Serialize, Deserialize, Debug)]
pub struct Movement {
    pub destination: Option<Vector2<f32>>
}

#[derive(Serialize, Deserialize)]
pub struct MovementCommand {
    pub to_move: CharacterID,
    pub destination: Vector2<f32>
}

// impl WorldCommand for MovementCommand {
//     fn run(self, world: &mut World) {
//         match world.movement.components.get_mut(&self.to_move) {
//             Some(movement) => {
//                 movement.destination = Some(self.destination);
//                 Ok(())
//             },
//             None => Err(WorldError::MissingCharacter(self.to_move))
//         }
//     }
// }

pub fn movement_system_update(world: &mut World) {
    
}