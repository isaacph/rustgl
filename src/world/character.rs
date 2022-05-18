use serde::{Serialize, Deserialize};

use super::component::ComponentID;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct CharacterID(i32);

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterIDGenerator(i32);

impl CharacterIDGenerator {
    pub fn new() -> Self {
        CharacterIDGenerator(0)
    }
    pub fn generate(&mut self) -> CharacterID {
        self.0 += 1;
        CharacterID(self.0 - 1)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Character {
    pub id: CharacterID,
    pub components: Vec<ComponentID>
}

impl Character {
    pub fn new(id_gen: &mut CharacterIDGenerator, components: Vec<ComponentID>) -> Self {
        Character {
            id: id_gen.generate(),
            components
        }
    }
}