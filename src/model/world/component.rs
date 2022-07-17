use std::{collections::HashMap, fmt::Display};

use nalgebra::Vector2;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use super::character::{CharacterID, CharacterType};
use strum_macros::EnumIter;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy, EnumIter)]
pub enum ComponentID {
    Base,
    Health,
    Movement,
    IceWiz,
    AutoAttack,
    ActionQueue,
}

impl Display for ComponentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterBase {
    pub ctype: CharacterType,
    pub position: Vector2<f32>,
    pub speed: f32,
    pub attack_damage: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CharacterHealth {
    pub health: f32
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
        self.components.get(id).map(|c| bincode::serialize(c).unwrap())
    }
}

impl<T> Default for ComponentStorage<T>
where T: Sized + Serialize + DeserializeOwned
{
    fn default() -> Self {
        Self::new()
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


