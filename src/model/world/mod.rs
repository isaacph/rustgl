use strum::IntoEnumIterator;
use std::collections::{HashMap, HashSet};
use serde::{Serialize, de::DeserializeOwned};

use self::{
//    player::{
//        TeamID,
//        Team, PlayerData
//    },
    character::{CharacterID, CharacterType, CharacterIDGenerator},
    component::{
        CharacterBase,
        ComponentStorage,
        ComponentID,
        CharacterHealth,
        ComponentStorageContainer
    },
    commands::{
        UpdateCharacter,
        WorldCommand
    },
    system::{
        movement::{
            Movement,
            movement_system_update,
            movement_system_init
        },
        icewiz::{
            IceWiz,
            IceWizInfo,
            icewiz_system_init,
            icewiz_system_update,
        }, auto_attack::{AutoAttack, auto_attack_system_init, auto_attack_system_update}, action_queue::{ActionQueue, action_queue_system_init, action_queue_system_update, ActionQueueInfo, ActionStatus, Action},
    }
};

use super::player::model::{TeamID, Team, PlayerData};

//pub mod player;
pub mod character;
pub mod component;
pub mod system;
pub mod commands;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;


#[derive(Debug, Clone)]
pub enum WorldError {
    MissingCharacter(CharacterID),
    MissingCharacterComponent(CharacterID, ComponentID),
    MissingCharacterCreator(CharacterType),
    MissingActionStatus(CharacterID, Action),
    UnexpectedActionStatus(CharacterID, Action, ActionStatus),
}

pub trait CharacterCreator {
    fn create(&mut self, world: &mut World, character_id: &CharacterID);
}

// This is necessary because of the borrow checker
// We can't have a borrowed character creator operator on the world directly
// So we have to have a borrowed character creator creator that produces an owned character creator
pub trait CharacterCreatorCreator {
    fn create(&self) -> Box<dyn CharacterCreator>;
}

pub struct World {
    pub errors: Vec<WorldError>,

    pub character_creator: HashMap<CharacterType, Box<dyn CharacterCreatorCreator>>,
    pub info: WorldInfo,
    pub teams: HashMap<TeamID, Team>,
    pub characters: HashSet<CharacterID>,
    pub players: PlayerData,

    // components that should be serialized across the internet
    pub base: ComponentStorage<CharacterBase>,
    pub health: ComponentStorage<CharacterHealth>,
    pub movement: ComponentStorage<Movement>,
    pub icewiz: ComponentStorage<IceWiz>,
    pub auto_attack: ComponentStorage<AutoAttack>,
    pub action_queue: ComponentStorage<ActionQueue>,
}

// immutable info about the world
// ex: base stats for wizards
pub struct WorldInfo {
    pub base: HashMap<CharacterType, CharacterBase>,
    pub health: HashMap<CharacterType, CharacterHealth>,

    pub icewiz: IceWizInfo,
    pub action_queue: ActionQueueInfo
}

impl WorldInfo {
    pub fn new() -> WorldInfo {
        WorldInfo {
            base: HashMap::new(),
            health: HashMap::new(),

            icewiz: IceWizInfo::init(),
            action_queue: ActionQueueInfo::init()
        }
    }
}

impl World {
    pub fn new() -> World {
        let mut world = World {
            errors: vec![],
            info: WorldInfo::new(),
            teams: HashMap::new(),
            characters: HashSet::new(),
            character_creator: HashMap::new(),
            players: PlayerData { players: HashMap::new() },
            base: ComponentStorage::new(),
            health: ComponentStorage::new(),
            movement: ComponentStorage::new(),
            auto_attack: ComponentStorage::new(),
            icewiz: ComponentStorage::new(),
            action_queue: ComponentStorage::new(),
        };
        // init each system
        movement_system_init(&mut world);
        icewiz_system_init(&mut world);
        auto_attack_system_init(&mut world);
        action_queue_system_init(&mut world);

        world
    }

    pub fn update(&mut self, delta_time: f32) {
        movement_system_update(self, delta_time);
        icewiz_system_update(self, delta_time);
        auto_attack_system_update(self, delta_time);
        action_queue_system_update(self, delta_time);
    }

    pub fn has_component(&self, id: &CharacterID, cid: &ComponentID) -> bool {
        fn test<T>(storage: &dyn ComponentStorageContainer<T>, id: &CharacterID) -> bool
                where T: Sized + Serialize {
            storage.get_storage().keys().any(|oid| *id == *oid)
        }
        match cid {
            ComponentID::Base => test(&self.base, id),
            ComponentID::Health => test(&self.health, id),
            ComponentID::Movement => test(&self.movement, id),
            ComponentID::IceWiz => test(&self.icewiz, id),
            ComponentID::AutoAttack => test(&self.auto_attack, id),
            ComponentID::ActionQueue => test(&self.action_queue, id),
            // _ => panic!("Component id not linked: {}", cid)
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
            ComponentID::Movement => ser(&self.movement, id),
            ComponentID::IceWiz => ser(&self.icewiz, id),
            ComponentID::AutoAttack => ser(&self.auto_attack, id),
            ComponentID::ActionQueue => ser(&self.action_queue, id),
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
            ComponentID::Movement => insert(&mut self.movement, id, cid, data),
            ComponentID::IceWiz => insert(&mut self.icewiz, id, cid, data),
            ComponentID::AutoAttack => insert(&mut self.auto_attack, id, cid, data),
            ComponentID::ActionQueue => insert(&mut self.action_queue, id, cid, data),
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

    pub fn run_command<'a, T: WorldCommand<'a>>(&mut self, command: T) {
        match T::run(command, self) {
            Ok(()) => (),
            Err(err) => self.errors.push(err)
        }
    }

    // Only returns Err if world info is corrupt and missing the character creator
    pub fn create_character(&mut self, idgen: &mut CharacterIDGenerator, typ: CharacterType) -> Result<CharacterID, String> {
        let mut creator = match self.character_creator.get(&typ) {
            Some(creator) => creator.create(),
            None => {
                let err = WorldError::MissingCharacterCreator(typ);
                self.errors.push(err.clone());
                return Err(format!("{:?}", err));
            }
        };
        let id = idgen.generate();
        self.characters.insert(id);
        creator.create(self, &id);
        Ok(id)
    }

    pub fn get_components(&self, id: &CharacterID) -> Vec<ComponentID> {
        ComponentID::iter().filter(|cid| self.has_component(id, cid)).collect()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for WorldInfo {
    fn default() -> Self {
        Self::new()
    }
}

