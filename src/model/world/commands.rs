use std::collections::HashMap;
use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::commands::GetCommandID;

use super::{World, character::{CharacterID, CharacterIDGenerator, self}, component::{ComponentID, CharacterHealth, CharacterBase}, WorldError, system::movement::Movement};

pub trait WorldCommand<'a>: Deserialize<'a> {
    fn run(self, world: &mut World) -> Result<(), WorldError>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCharacter {
    pub id: CharacterID,
    pub components: HashMap<ComponentID, Vec<u8>>
}

impl GetCommandID for UpdateCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::UpdateCharacter
    }
}

impl UpdateCharacter {
    pub fn update_character(mut self, world: &mut World) {
        println!("Updating character");
        world.characters.insert(self.id);
        for (cid, data) in self.components.drain() {
            world.update_component(&self.id, &cid, data);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GenerateCharacter;

impl GenerateCharacter {
    pub fn new() -> Self {
        GenerateCharacter
    }
    pub fn generate_character(world: &mut World, idgen: &mut CharacterIDGenerator) -> CharacterID {
        let id = idgen.generate();
        world.characters.insert(id);
        world.base.components.insert(id, CharacterBase {
            ctype: character::CharacterType::HERO,
            position: Vector2::new(0.0, 0.0),
            speed: 1.0
        });
        world.health.components.insert(id, CharacterHealth {
            health: 100.0
        });
        world.movement.components.insert(id, Movement {
            destination: None
        });
        id
    }
}

impl GetCommandID for GenerateCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::GenerateCharacter
    }
}

impl Default for GenerateCharacter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListChar;

impl GetCommandID for ListChar {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::ListChar
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EnsureCharacter;

impl GetCommandID for EnsureCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::EnsureCharacter
    }
}

