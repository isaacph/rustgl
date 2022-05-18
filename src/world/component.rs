use std::{collections::HashMap, fmt::Display};

use nalgebra::Vector3;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use super::{character::CharacterID};


pub trait ComponentIdentified {
    fn component_id() -> ComponentID;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ComponentID {
    Base,
    Health
}

impl Display for ComponentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterBase {
    pub position: Vector3<f32>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterHealth {
    pub health: f32
}

impl ComponentIdentified for CharacterBase {
    fn component_id() -> ComponentID {
        ComponentID::Base
    }
}

pub trait ComponentStorageContainer<T: Sized + Serialize> {
    fn get_storage(&self) -> &HashMap<CharacterID, T>;
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T>;
}

#[derive(Debug)]
pub struct ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned {
    pub components: HashMap<CharacterID, T>
}

impl<T> ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned {
    pub fn new() -> ComponentStorage<T> {
        ComponentStorage { components: HashMap::new() }
    }
    pub fn ser(&self, id: &CharacterID) -> Option<Vec<u8>> {
        match self.components.get(id) {
            None => None,
            Some(c) => Some(bincode::serialize(c).unwrap())
        }
    }
}

impl<T> ComponentIdentified for ComponentStorage<T> where T: ComponentIdentified + Sized + Serialize + DeserializeOwned {
    fn component_id() -> ComponentID {
        T::component_id()
    }
}

impl<T> ComponentStorageContainer<T> for ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned {
    fn get_storage(&self) -> &HashMap<CharacterID, T> {
        &self.components
    }
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T> {
        &mut self.components
    }
}