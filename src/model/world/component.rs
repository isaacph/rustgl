use std::{collections::HashMap, fmt::{Display, Debug}};

use itertools::Itertools;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use super::{character::CharacterID, WorldError, system::{base::CharacterBaseUpdate, projectile::ProjectileUpdate, status::StatusUpdate, movement::Movement, auto_attack::AutoAttackUpdate, flash::FlashUpdate}, system::{health::CharacterHealthUpdate, collision::CollisionUpdate}, WorldErrorI};
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
    Status,
    Flash,
    Collision,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ComponentUpdateData {
    Base(CharacterBaseUpdate),
    Health(CharacterHealthUpdate),
    Projectile(ProjectileUpdate),
    Status(StatusUpdate),
    Movement(Movement),
    AutoAttack(AutoAttackUpdate),
    Flash(FlashUpdate),
    Collision(CollisionUpdate),
    CasterMinion,
    IceWiz,
}

impl ComponentUpdateData {
    pub fn component_id(&self) -> ComponentID {
        match *self {
            ComponentUpdateData::Base(_) => ComponentID::Base,
            ComponentUpdateData::Health(_) => ComponentID::Health,
            ComponentUpdateData::Projectile(_) => ComponentID::Projectile,
            ComponentUpdateData::Status(_) => ComponentID::Status,
            ComponentUpdateData::Movement(_) => ComponentID::Movement,
            ComponentUpdateData::AutoAttack(_) => ComponentID::AutoAttack,
            ComponentUpdateData::Flash(_) => ComponentID::Flash,
            ComponentUpdateData::CasterMinion => ComponentID::CasterMinion,
            ComponentUpdateData::IceWiz => ComponentID::IceWiz,
            ComponentUpdateData::Collision(_) => ComponentID::Collision,
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

pub trait Component: GetComponentID + Default {
    fn update(&self, update: &ComponentUpdateData) -> Self;
}


pub trait ComponentStorageCommon {
    fn get_characters(&self) -> Vec<CharacterID>;
    fn has_character(&self, cid: &CharacterID) -> bool;
    fn serialize(&self, cid: &CharacterID) -> Option<Vec<u8>>;
    fn deserialize_insert(&mut self, cid: &CharacterID, data: Vec<u8>);
    fn erase(&mut self, cid: &CharacterID);
    fn update(&mut self, updates: &[ComponentUpdate]) -> Result<(), Vec<WorldError>>;
}

pub trait ComponentStorageContainer<T: Sized + Serialize>: ComponentStorageCommon {
    fn get_storage(&self) -> &HashMap<CharacterID, T>;
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T>;
    fn get_component_mut(&mut self, cid: &CharacterID) -> Result<&mut T, WorldError>;
    fn get_component(&self, cid: &CharacterID) -> Result<&T, WorldError>;
}

#[derive(Debug, Clone)]
pub struct ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component + Debug {
    pub components: HashMap<CharacterID, T>
}

impl<T> GetComponentID for ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component + Debug {
    const ID: ComponentID = T::ID;
}

impl<T> ComponentStorage<T> where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component + Debug {
    pub fn new() -> ComponentStorage<T> {
        ComponentStorage { components: HashMap::new() }
    }
    pub fn ser(&self, id: &CharacterID) -> Option<Vec<u8>> {
        self.components.get(id).map(|c| bincode::serialize(c).unwrap())
    }
}

impl<T> Default for ComponentStorage<T>
where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component + Debug
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Serialize + GetComponentID + DeserializeOwned + Component + Debug> ComponentStorageCommon for ComponentStorage<T> {
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
    fn update(&mut self, updates: &[ComponentUpdate]) -> Result<(), Vec<WorldError>> {
        // let err: Vec<WorldError> = updates.iter()
        // .filter_map(|comp| self.get_component(&comp.cid).err())
        // .collect();
        if !updates.is_empty() {
            // println!("Requested updates to component: {:?}", updates);
        }
        // chain updates together for updates with same character ID
        // let comp_id = Self::ID;
        let updates = updates.iter()
            .map(|update| (update.cid, update))
            .into_group_map()
            .into_iter()
            .filter_map(|(cid, update)| {
                let def = Default::default();
                let start = self.components.get(&cid);
                if start.is_none() {
                    // println!("Warning: component missing: {:?}", comp_id);
                }
                let start = start.unwrap_or(&def);
                update
                .into_iter()
                .fold(None, |current, update| match current {
                    None => Some(start.update(&update.data)),
                    Some(current) => Some(current.update(&update.data)),
                })
                .map(|update| (cid, update))
            })
            .collect_vec();
        if !updates.is_empty() {
            // println!("Updating component: {:?}", updates);
        }
        self.components.extend(updates.into_iter());
        // if !err.is_empty() {
        //     Err(err)
        // } else {
        //     Ok(())
        // }
        Ok(())
    }
}

impl<T> ComponentStorageContainer<T> for ComponentStorage<T>
        where T: Sized + Serialize + DeserializeOwned + GetComponentID + Component + Debug {
    fn get_storage(&self) -> &HashMap<CharacterID, T> {
        &self.components
    }
    fn get_storage_mut(&mut self) -> &mut HashMap<CharacterID, T> {
        &mut self.components
    }
    fn get_component_mut(&mut self, cid: &CharacterID) -> Result<&mut T, WorldError> {
        match self.components.get_mut(cid) {
            Some(comp) => Ok(comp),
            None => Err(WorldErrorI::MissingCharacterComponent(*cid, T::ID).err())
        }
    }
    fn get_component(&self, cid: &CharacterID) -> Result<&T, WorldError> {
        match self.components.get(cid) {
            Some(comp) => Ok(comp),
            None => Err(WorldErrorI::MissingCharacterComponent(*cid, T::ID).err())
        }
    }
}

