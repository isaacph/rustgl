use std::{any::Any, fmt::Display};

use nalgebra::Vector3;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct CharacterID(i32);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct CharacterTypeID(u32);

impl Display for CharacterTypeID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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

pub trait Character: ContainableInWorld {
    fn id(&self) -> CharacterID;
    fn position(&self) -> Vector3<f32>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hero {
    pub id: CharacterID,
    pub position: Vector3<f32>,
}

impl Hero {
    pub fn new(generator: &mut CharacterIDGenerator) -> Self {
        Hero {
            id: generator.generate(),
            position: Vector3::new(0.0, 0.0, 0.0)
        }
    }
}

impl Character for Hero {
    fn id(&self) -> CharacterID {
        self.id
    }
    fn position(&self) -> Vector3<f32> {
        self.position
    }
}

pub trait ContainableInWorld {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_id(&self) -> CharacterTypeID;
}

impl ContainableInWorld for Hero {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn type_id(&self) -> CharacterTypeID {
        CharacterTypeID(0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializedCharacter {
    id: CharacterTypeID,
    data: Vec<u8>
}

impl SerializedCharacter {
    pub fn serialize(character: &dyn Character) -> SerializedCharacter {
        if character.type_id() == CharacterTypeID(0) {
            let hero: &Hero = match character.as_any().downcast_ref() {
                Some(h) => h,
                None => panic!("Character type ID was incorrect: {}", character.type_id())
            };
            return SerializedCharacter {
                id: character.type_id(),
                data: match bincode::serialize(hero) {
                    Ok(data) => data,
                    Err(e) => panic!("Failed to serialize character of type {}: {}", character.type_id(), e)
                }
            }
        }
        panic!("Attempted to serialize an unknown character type: {}", character.type_id());
    }
    pub fn deserialize(&self) -> Box<dyn Character> {
        if self.id == CharacterTypeID(0) {
            let hero: Box<Hero> = match bincode::deserialize(&self.data) {
                Ok(r) => Box::new(r),
                Err(e) => panic!("Failed to deserialize character of type {}: {}", self.id, e)
            };
            let cast: Box<dyn Character> = hero;
            return cast
        }
        panic!("Attempted to deserialize an unknown character type: {}", self.id);
    }
}

// #[derive(Serialize, Deserialize, Debug)]
// pub struct Minion {
//     pub id: CharacterID,
//     pub position: Vector3<f32>,
// }