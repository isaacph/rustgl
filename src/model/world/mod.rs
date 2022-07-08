use strum::IntoEnumIterator;
use std::collections::{HashMap, HashSet};
use serde::{Serialize, de::DeserializeOwned};

use self::{
//    player::{
//        TeamID,
//        Team, PlayerData
//    },
    character::CharacterID,
    component::{
        CharacterBase,
        ComponentStorage,
        ComponentID,
        CharacterHealth,
        ComponentStorageContainer
    }, commands::UpdateCharacter
};

use super::player::model::{TeamID, Team, PlayerData};

//pub mod player;
pub mod character;
pub mod component;
pub mod commands;
pub mod server;
pub mod client;

pub struct World {
    pub teams: HashMap<TeamID, Team>,
    pub characters: HashSet<CharacterID>,
    pub players: PlayerData,

    pub base: ComponentStorage<CharacterBase>,
    pub health: ComponentStorage<CharacterHealth>,
}

impl World {
    pub fn new() -> World {
        World {
            teams: HashMap::new(),
            characters: HashSet::new(),
            players: PlayerData { players: HashMap::new() },
            base: ComponentStorage::<CharacterBase>::new(),
            health: ComponentStorage::<CharacterHealth>::new(),
        }
    }

    pub fn serialize_component(&self, id: &CharacterID, cid: &ComponentID) -> Option<Vec<u8>> {
        fn ser<T>(storage: &dyn ComponentStorageContainer<T>, id: &CharacterID) -> Option<Vec<u8>>
                where T: Sized + Serialize {
            storage.get_storage().get(id).map(|c| bincode::serialize(c).unwrap())
        }
        match cid {
            ComponentID::Base => ser(&self.base, id),
            ComponentID::Health => ser(&self.health, id),
            // _ => panic!("Serialization not implemented for component id: {}", cid)
        }
    }

    pub fn update_component(&mut self, id: &CharacterID, cid: &ComponentID, data: Vec<u8>) {
        fn insert<T>(storage: &mut dyn ComponentStorageContainer<T>, id: &CharacterID, cid: &ComponentID, data: Vec<u8>)
                where T: Sized + Serialize + DeserializeOwned {
            let des: T = match bincode::deserialize(data.as_slice()) {
                Err(e) => {
                    println!("Failed to deserialize component of id {}: {}", cid, e);
                    return
                },
                Ok(x) => x
            };
            storage.get_storage_mut().insert(*id, des);
        }
        match cid {
            ComponentID::Base => insert(&mut self.base, id, cid, data),
            ComponentID::Health => insert(&mut self.health, id, cid, data),
            // _ => panic!("Deserialization not implemented for component id: {}", cid)
        }
    }

    pub fn make_cmd_update_character(&self, id: CharacterID) -> Option<UpdateCharacter> {
        match self.characters.get(&id) {
            None => None,
            Some(&id) => {
                let components = ComponentID::iter();
                let components: HashMap<ComponentID, Vec<u8>> = components.filter_map(
                    |cid| self.serialize_component(&id, &cid).map(|ser| (cid, ser))
                ).collect();
                Some(UpdateCharacter {
                    id,
                    components
                })
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
