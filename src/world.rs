
use nalgebra::Vector2;
use std::collections::{HashMap, HashSet};

use serde::{Serialize, de::DeserializeOwned, Deserialize};

use crate::{networking_wrapping::{ServerCommand, ClientCommand, SerializedClientCommand}, game::Game, networking::server::ConnectionID, server::Server};

use self::{
    player::{
        TeamID,
        Team, PlayerData
    },
    character::{CharacterID, CharacterIDGenerator},
    component::{
        CharacterBase,
        ComponentStorage,
        ComponentID,
        CharacterHealth,
        ComponentStorageContainer
    }
};
use strum::IntoEnumIterator;

pub mod player;
pub mod character;
pub mod component;

pub trait WorldCommand<'a>: Serialize + Deserialize<'a> {
    fn run(self, world: &mut World);
}

impl<'a, T> ClientCommand<'a> for T where T: WorldCommand<'a> {
    fn run(self, client: &mut Game) {
        self.run(&mut client.world)
    }
}

impl<'a, T> ServerCommand<'a> for T where T: WorldCommand<'a> {
    fn run(self, (_, server): (&ConnectionID, &mut Server)) {
        self.run(&mut server.world)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateCharacter {
    id: CharacterID,
    components: HashMap<ComponentID, Vec<u8>>
}

impl UpdateCharacter {
    pub fn update_character(mut self, world: &mut World) {
        world.characters.insert(self.id);
        for (cid, data) in self.components.drain() {
            world.update_component(&self.id, &cid, data);
        }
    }
}

impl<'a> ClientCommand<'a> for UpdateCharacter {
    fn run(self, client: &mut Game) {
        self.update_character(&mut client.world);
    } 
}

impl<'a> ServerCommand<'a> for UpdateCharacter {
    fn run(self, (_, server): (&ConnectionID, &mut Server)) {
        let ser = SerializedClientCommand::from(&self);
        self.update_character(&mut server.world);
        server.connection.send_udp(server.connection.all_connection_ids(), ser.data);
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
        world.characters.insert(id.clone());
        world.base.components.insert(id, CharacterBase {
            ctype: character::CharacterType::HERO,
            position: Vector2::new(200.0, 200.0)
        });
        world.health.components.insert(id, CharacterHealth {
            health: 100.0
        });
        id
    }
}

impl<'a> ServerCommand<'a> for GenerateCharacter {
    fn run(self, (_, server): (&ConnectionID, &mut Server)) {
        let id = Self::generate_character(&mut server.world, &mut server.character_id_gen);
        let cmd = server.world.make_cmd_update_character(id).unwrap();
        let ser = SerializedClientCommand::from(&cmd);
        server.connection.send_udp(server.connection.all_connection_ids(), ser.data);
    }
}

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
                    |cid| match self.serialize_component(&id, &cid) {
                        Some(ser) => Some((cid, ser)),
                        None => None
                    }
                ).collect();
                Some(UpdateCharacter {
                    id,
                    components
                })
            }
        }
    }
}
