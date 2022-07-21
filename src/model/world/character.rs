use serde::{Serialize, Deserialize};

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

impl Default for CharacterIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum CharacterType {
    IceWiz,
    Projectile,
}

