
use std::collections::{HashMap};

use serde::{Serialize, de::DeserializeOwned, Deserialize};

// use crate::{networking_wrapping::{ServerCommand, ClientCommand}, game::Game, networking::server::ConnectionID, server::Server};

use self::{player::{TeamID, PlayerID, Team, Player}, character::{CharacterID, Character}, component::{CharacterBase, ComponentStorage, ComponentID, CharacterHealth, ComponentStorageContainer}};

pub mod player;
pub mod character;
pub mod component;

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCharacter {
    id: CharacterID,
    components: HashMap<ComponentID, Vec<u8>>
}

impl UpdateCharacter {
    pub fn update_character(&mut self, world: &mut World) {
        for (cid, data) in self.components.drain() {
            world.update_component(&self.id, &cid, data);
        }
    }
}

pub struct World {
    pub teams: HashMap<TeamID, Team>,
    pub players: HashMap<PlayerID, Player>,
    pub characters: HashMap<CharacterID, Character>,

    pub base: ComponentStorage<CharacterBase>,
    pub health: ComponentStorage<CharacterHealth>,
}

impl World {
    pub fn new() -> World {
        World {
            teams: HashMap::new(),
            players: HashMap::new(),
            characters: HashMap::new(),
            base: ComponentStorage::<CharacterBase>::new(),
            health: ComponentStorage::<CharacterHealth>::new(),
        }
    }

    pub fn serialize_component(&self, id: &CharacterID, cid: &ComponentID) -> Option<Vec<u8>> {
        fn ser<T>(storage: &dyn ComponentStorageContainer<T>, id: &CharacterID) -> Option<Vec<u8>>
                where T: Sized + Serialize {
            match storage.get_storage().get(id) {
                None => None,
                Some(c) => Some(bincode::serialize(c).unwrap())
            }
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
            _ => panic!("Deserialization not implemented for component id: {}", cid)
        }
    }

    pub fn make_cmd_update_character(&self, id: CharacterID) -> Option<UpdateCharacter> {
        match self.characters.get(&id) {
            None => None,
            Some(character) => {
                let components = character.components.clone();
                let components: HashMap<ComponentID, Vec<u8>> = components.iter().map(
                    |cid| (*cid, self.serialize_component(&id, cid).unwrap())
                ).collect();
                Some(UpdateCharacter {
                    id,
                    components
                })
            }
        }
    }
}