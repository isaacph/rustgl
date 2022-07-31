use std::{collections::HashMap, fmt::Display};

use serde::{Serialize, Deserialize, de::DeserializeOwned};

use super::{character::CharacterID, WorldError, system::base::CharacterBaseUpdate, World};
use strum_macros::EnumIter;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy, EnumIter)]
pub enum ComponentID {
    Base,
    Health,
    Movement,
    IceWiz,
    CasterMinion,
    AutoAttack,
    Projectile,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ComponentUpdateData {
    Base(CharacterBaseUpdate),
}

impl ComponentUpdateData {
    pub fn component_id(&self) -> ComponentID {
        match *self {
            ComponentUpdateData::Base(_) => ComponentID::Base,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComponentUpdate {
    pub cid: CharacterID,
    pub data: ComponentUpdateData,
}

impl Display for ComponentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait GetComponentID {
    const ID: ComponentID;
}

pub trait Component: GetComponentID {
    fn update(&self, update: &ComponentUpdateData) -> Self;
}


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterHealth {
    pub health: f32,
    pub max_health: f32,
}

impl Component for CharacterHealth {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        self.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterHealthUpdate {
    Change(f32) // todo: add cause
}

impl GetComponentID for CharacterHealth {
    const ID: ComponentID = ComponentID::Health;
}

pub trait ComponentStorageCommon {
    fn get_characters(&self) -> Vec<CharacterID>;
    fn has_character(&self, cid: &CharacterID) -> bool;
    fn serialize(&self, cid: &CharacterID) -> Option<Vec<u8>>;
    fn deserialize_insert(&mut self, cid: &CharacterID, data: Vec<u8>);
    fn erase(&mut self, cid: &CharacterID);
    fn update(&mut self, updates: &Vec<ComponentUpdate>) -> Result<(), Vec<WorldError>>;
}

pub trait ComponentStorageContainer<T: Sized + Serialize>: ComponentStorageCommon {
    fn get_storage(&self) -> &HashMap<CharacterID, T>;
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T>;
    fn get_component_mut(&mut self, cid: &CharacterID) -> Result<&mut T, WorldError>;
    fn get_component(&self, cid: &CharacterID) -> Result<&T, WorldError>;
}

#[derive(Debug, Clone)]
pub struct ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component {
    pub components: HashMap<CharacterID, T>
}

impl<T> GetComponentID for ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component {
    const ID: ComponentID = T::ID;
}

impl<T> ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component {
    pub fn new() -> ComponentStorage<T> {
        ComponentStorage { components: HashMap::new() }
    }
    pub fn ser(&self, id: &CharacterID) -> Option<Vec<u8>> {
        self.components.get(id).map(|c| bincode::serialize(c).unwrap())
    }
}

impl<T> Default for ComponentStorage<T>
where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Serialize + GetComponentID + DeserializeOwned + Component> ComponentStorageCommon for ComponentStorage<T> {
    fn get_characters(&self) -> Vec<CharacterID> {
        self.components.keys().copied().collect()
    }
    fn has_character(&self, cid: &CharacterID) -> bool {
        self.components.contains_key(cid)
    }
    fn serialize(&self, cid: &CharacterID) -> Option<Vec<u8>> {
        self.components.get(cid).map(|component| bincode::serialize(component).ok()).unwrap_or(None)
    }
    fn deserialize_insert(&mut self, cid: &CharacterID, data: Vec<u8>) {
        let des: T = match bincode::deserialize(data.as_slice()) {
            Err(e) => {
                println!("Failed to deserialize component of id {} for {:?}: {}", Self::ID, *cid, e);
                return
            },
            Ok(x) => x
        };
        self.components.insert(*cid, des);
    }
    fn erase(&mut self, cid: &CharacterID) {
        self.components.remove(cid);
    }
    fn update(&mut self, updates: &Vec<ComponentUpdate>) -> Result<(), Vec<WorldError>> {
        let err: Vec<WorldError> = updates.iter()
        .filter_map(|comp| self.get_component(&comp.cid).err())
        .collect();
        self.components.extend(
            updates.iter()
            .filter_map(|update| self.components
                .get(&update.cid)
                .map(|comp| (update.cid, comp.update(&update.data)))));
        if err.len() > 0 {
            Err(err)
        } else {
            Ok(())
        }
    }
}

impl<T> ComponentStorageContainer<T> for ComponentStorage<T>
        where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component {
    fn get_storage(&self) -> &HashMap<CharacterID, T> {
        &self.components
    }
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T> {
        &mut self.components
    }
    fn get_component_mut(&mut self, cid: &CharacterID) -> Result<&mut T, WorldError> {
        match self.components.get_mut(cid) {
            Some(comp) => Ok(comp),
            None => Err(WorldError::MissingCharacterComponent(*cid, T::ID))
        }
    }
    fn get_component(&self, cid: &CharacterID) -> Result<&T, WorldError> {
        match self.components.get(cid) {
            Some(comp) => Ok(comp),
            None => Err(WorldError::MissingCharacterComponent(*cid, T::ID))
        }
    }
}


