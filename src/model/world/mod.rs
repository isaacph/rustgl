use nalgebra::Vector3;
use strum::IntoEnumIterator;
use std::{collections::{HashMap, HashSet}, rc::Rc};
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
    commands::{UpdateCharacter, WorldCommand, CharacterCommand},
    system::{
        movement::{
            Movement,
            MovementSystem
        },
        icewiz::{
            IceWiz,
            self, IceWizSystem,
        },
        auto_attack::{
            AutoAttack,
            AutoAttackInfo,
            AutoAttackPhase, AutoAttackSystem
        },
        projectile::{
            Projectile,
            ProjectileSystem
        },
        caster_minion::{
            CasterMinion,
            self, CasterMinionSystem
        }
    }
};

use super::commands::CommandID;

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
    MissingCharacter(CharacterID, String),
    MissingCharacterComponent(CharacterID, ComponentID),
    MissingCharacterComponentSystem(CharacterID, ComponentID),
    MissingCharacterCreator(CharacterType),
    InvalidCommandReplacement(CharacterID, CommandID),
    InvalidCommand,
    InvalidCommandMapping,
    OutOfRange(CharacterID), // character whose range we are out of
    UnexpectedComponentState(CharacterID, ComponentID, String),
    MissingCharacterInfoComponent(CharacterType, ComponentID),
    InvalidComponentInfo(CharacterType, ComponentID),
    InvalidAttackPhase(CharacterID, AutoAttackPhase),
    NoopCommand,
    IllegalInterrupt(CharacterID),
    OnCooldown(CharacterID, ComponentID),
    DesyncError(CharacterID, ComponentID, String),
}

pub trait CharacterCreator {
    fn create(&mut self, world: &mut World, character_id: &CharacterID) -> Result<(), WorldError>;
}

// This is necessary because of the borrow checker
// We can't have a borrowed character creator operator on the world directly
// So we have to have a borrowed character creator creator that produces an owned character creator
pub trait CharacterCreatorCreator {
    fn create(&self) -> Box<dyn CharacterCreator>;
}

#[derive(Clone)]
pub struct World {
    pub errors: Vec<WorldError>,

    pub info: Rc<WorldInfo>,
    // pub teams: HashMap<TeamID, Team>,
    pub characters: HashSet<CharacterID>,
    // pub players: PlayerData,

    // components that should be serialized across the internet
    pub base: ComponentStorage<CharacterBase>,
    pub health: ComponentStorage<CharacterHealth>,
    pub movement: ComponentStorage<Movement>,
    pub auto_attack: ComponentStorage<AutoAttack>,
    pub projectile: ComponentStorage<Projectile>,

    pub icewiz: ComponentStorage<IceWiz>,
    pub caster_minion: ComponentStorage<CasterMinion>,

    pub frame_id: u64,
}

pub trait WorldSystem {
    fn get_component_id(&self) -> ComponentID;
    fn init_world_info(&self) -> Result<WorldInfo, WorldError>;
    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError>;
    fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError>;
    fn update_character(&self, world: &mut World, cid: &CharacterID, delta_time: f32) -> Result<(), WorldError>;
}

// immutable info about the world
// ex: base stats for wizards
pub struct WorldInfo {
    pub base: HashMap<CharacterType, CharacterBase>,
    pub health: HashMap<CharacterType, CharacterHealth>,
    pub auto_attack: HashMap<CharacterType, AutoAttackInfo>,

    pub systems: HashMap<ComponentID, Box<dyn WorldSystem>>,
}

impl WorldInfo {
    pub fn new() -> WorldInfo {
        WorldInfo {
            base: HashMap::new(),
            health: HashMap::new(),
            auto_attack: HashMap::new(),
            systems: HashMap::new(),
        }
    }
    pub fn combine(infos: Vec<WorldInfo>, systems: HashMap<ComponentID, Box<dyn WorldSystem>>) -> WorldInfo {
        let mut combo = WorldInfo::new();
        for info in infos {
            combo.base.extend(info.base.into_iter());
            combo.health.extend(info.health.into_iter());
            combo.auto_attack.extend(info.auto_attack.into_iter());
        }
        combo.systems = systems;
        combo
    }
}

pub enum CommandRunResult {
    Invalid(WorldError),
    ValidError(WorldError),
    Valid,
}

impl World {
    pub fn new() -> World {
        // init each system
        let mut systems = HashMap::new();
        for system in [
            Box::new(MovementSystem) as Box<dyn WorldSystem>,
            Box::new(AutoAttackSystem) as Box<dyn WorldSystem>,
            Box::new(ProjectileSystem) as Box<dyn WorldSystem>,
            Box::new(IceWizSystem) as Box<dyn WorldSystem>,
            Box::new(CasterMinionSystem) as Box<dyn WorldSystem>,
        ] {
            systems.insert(system.get_component_id(), system);
        }
        let (info, errors): (Vec<WorldInfo>, Vec<WorldError>) = systems.values()
            .map(|system| system.init_world_info())
            .fold((vec![], vec![]), |(mut info, mut errors), res| {
                match res {
                    Ok(ninfo) => info.push(ninfo),
                    Err(err) => errors.push(err),
                };
                (info, errors)
            }
        );
        World {
            errors,
            frame_id: 0,
            info: Rc::new(WorldInfo::combine(info, systems)),
            characters: HashSet::new(),
            base: ComponentStorage::new(),
            health: ComponentStorage::new(),
            movement: ComponentStorage::new(),
            auto_attack: ComponentStorage::new(),
            icewiz: ComponentStorage::new(),
            caster_minion: ComponentStorage::new(),
            projectile: ComponentStorage::new(),
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        let info = self.info.clone();
        for cid in self.characters.clone().iter() {
            for comp_id in self.get_components(cid) {
                match match info.systems.get(&comp_id) {
                    Some(system) => system.update_character(self, cid, delta_time),
                    // None => Err(WorldError::MissingCharacterComponentSystem(*cid, comp_id)),
                    None => Ok(()) // components don't need to have systems
                } {
                    Ok(()) => (),
                    Err(err) => self.errors.push(err)
                }
            }
        }
        // let errors = self.characters.clone().iter()
        //     .flat_map(|cid| info.systems.values()
        //     .map(|system| system.update_character(self, cid, delta_time)))
        //     .filter_map(|res| match res {
        //     Ok(()) => None,
        //     Err(err) => Some(err),
        // });
        // self.errors.extend(errors);
        self.frame_id += 1;
    }

    pub fn run_command(&mut self, command: WorldCommand) -> Result<CommandRunResult, WorldError> {
        match command {
            WorldCommand::CharacterComponent(cid, compid, cmd) => {
                match self.characters.get(&cid) {
                    Some(_) => {
                        match self.info.clone().systems.get(&compid) {
                            Some(system) => {
                                if let Err(err) = system.validate_character_command(self, &cid, &cmd) {
                                    return Ok(CommandRunResult::Invalid(err));
                                }
                                if let Err(err) = system.run_character_command(self, &cid, cmd) {
                                    return Ok(CommandRunResult::ValidError(err));
                                }
                                Ok(CommandRunResult::Valid)
                            },
                            None => Err(WorldError::MissingCharacterComponentSystem(cid, compid)),
                        }
                    },
                    None => Err(WorldError::MissingCharacter(cid, "Failed to run command for character".to_string())),
                }
            },
            WorldCommand::World(_) => {
                panic!("Not implemented");
            },
            WorldCommand::Update(update_cmd) => {
                println!("Update character");
                update_cmd.update_character(self).map(|_| CommandRunResult::Valid)
            }
        }
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
            ComponentID::Projectile => test(&self.projectile, id),
            ComponentID::CasterMinion => test(&self.caster_minion, id),
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
            ComponentID::Projectile => ser(&self.projectile, id),
            ComponentID::CasterMinion => ser(&self.caster_minion, id),
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
            // ComponentID::Base => insert(&mut self.base, id, cid, data),
            ComponentID::Base => {
                let des: CharacterBase = match bincode::deserialize(data.as_slice()) {
                    Err(e) => {
                        println!("Failed to deserialize component of id {}: {}", cid, e);
                        return
                    },
                    Ok(x) => x
                };
                if let Some(current) = self.base.components.get(id) {
                    let diff = (current.position - des.position).magnitude();
                    if diff > 0.0 {
                        self.errors.push(WorldError::DesyncError(*id, *cid, format!("pos diff: {}", diff)));
                    }
                }
                self.base.get_storage_mut().insert(*id, des);
//                insert(&mut self.auto_attack, id, cid, data)
            },
            ComponentID::Health => insert(&mut self.health, id, cid, data),
            ComponentID::Movement => insert(&mut self.movement, id, cid, data),
            ComponentID::IceWiz => insert(&mut self.icewiz, id, cid, data),
            ComponentID::AutoAttack => {
                let des: AutoAttack = match bincode::deserialize(data.as_slice()) {
                    Err(e) => {
                        println!("Failed to deserialize component of id {}: {}", cid, e);
                        return
                    },
                    Ok(x) => x
                };
                if let Some(current) = self.auto_attack.components.get(id) {
                    if current.execution.is_some() != des.execution.is_some() {
                        self.errors.push(WorldError::DesyncError(*id, *cid, format!(
                                    "Current executing: {}, remote executing: {}",
                                    current.execution.is_some(),
                                    des.execution.is_some())));
                    }
                }
                self.auto_attack.get_storage_mut().insert(*id, des);
//                insert(&mut self.auto_attack, id, cid, data)
            },
            ComponentID::Projectile => insert(&mut self.projectile, id, cid, data),
            ComponentID::CasterMinion => insert(&mut self.caster_minion, id, cid, data),
            // _ => panic!("Deserialization not implemented for component id: {}", cid)
        }
    }

    pub fn erase_component(&mut self, cid: &CharacterID, id: &ComponentID) {
        fn erase<T>(storage: &mut dyn ComponentStorageContainer<T>, id: &CharacterID)
                where T: Sized + Serialize + DeserializeOwned {
            storage.get_storage_mut().remove(id);
        }
        match id {
            ComponentID::Base => erase(&mut self.base, cid),
            ComponentID::Health => erase(&mut self.health, cid),
            ComponentID::Movement => erase(&mut self.movement, cid),
            ComponentID::IceWiz => erase(&mut self.icewiz, cid),
            ComponentID::AutoAttack => erase(&mut self.auto_attack, cid),
            ComponentID::Projectile => erase(&mut self.projectile, cid),
            ComponentID::CasterMinion => erase(&mut self.caster_minion, cid),
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

    // pub fn run_command<'a, T: WorldCommand<'a>>(&mut self, command: T) {
    //     match T::run(command, self) {
    //         Ok(()) => (),
    //         Err(err) => self.errors.push(err)
    //     }
    // }

    // Only returns Err if world info is corrupt and missing the character creator
    pub fn create_character(&mut self, idgen: &mut CharacterIDGenerator, typ: CharacterType) -> Result<CharacterID, WorldError> {
        let id = idgen.generate();
        let position = Vector3::new(0.0, 0.0, 0.0);
        match typ {
            CharacterType::CasterMinion => caster_minion::create(self, &id, position)?,
            CharacterType::IceWiz => icewiz::create(self, &id, position)?,
            _ => return Err(WorldError::MissingCharacterCreator(typ)),
        };
        Ok(id)
    }

    pub fn erase_character(&mut self, cid: &CharacterID) -> Result<(), WorldError> {
        self.characters.remove(cid);
        for id in self.get_components(cid) {
            self.erase_component(cid, &id);
        }
        Ok(())
    }

    pub fn get_components(&self, id: &CharacterID) -> HashSet<ComponentID> {
        ComponentID::iter().filter(|cid| self.has_component(id, cid)).collect()
    }

    pub fn diff(&self, other: &Self) -> Vec<String> {
        let mut diff = vec![];
        diff.extend(
            other.characters.difference(&self.characters)
            .map(|cid| format!("Self missing cid {:?}", *cid))
        );
        diff.extend(
            self.characters.difference(&other.characters)
            .map(|cid| format!("Other missing cid {:?}", *cid))
        );
        for cid in self.characters.intersection(&other.characters) {
            let self_comp = self.get_components(cid);
            let other_comp = other.get_components(cid);
            diff.extend(
                other_comp.difference(&self_comp)
                .map(|comp_id| format!("Self {:?} missing component {:?}", *cid, *comp_id))
            );
            diff.extend(
                self_comp.difference(&other_comp)
                .map(|comp_id| format!("Other {:?} missing component {:?}", *cid, *comp_id))
            );
            diff.extend(self_comp.intersection(&other_comp).filter_map(|comp_id| {
                match comp_id {
                    ComponentID::Base => {
                        let mut diff = vec![];
                        let s = self.base.get_component(cid).ok()?;
                        let o = other.base.get_component(cid).ok()?;
                        if s.ctype != o.ctype {
                            diff.push(format!("{:?}.{:?}: Self ctype: {:?}, Other ctype: {:?}", comp_id, cid, s.ctype, o.ctype));
                        }
                        let pos_diff = (s.position - o.position).magnitude();
                        if pos_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self pos - other pos = {}", comp_id, cid, pos_diff));
                        }
                        let center_offset_diff = (s.center_offset - o.center_offset).magnitude();
                        if center_offset_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self center_offset - other center_offset = {}", comp_id, cid, center_offset_diff));
                        }
                        let speed_diff = s.speed - o.speed;
                        if speed_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self speed - other speed = {}", comp_id, cid, speed_diff));
                        }
                        let ad_diff = s.attack_damage - o.attack_damage;
                        if ad_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self ad - other ad = {}", comp_id, cid, ad_diff));
                        }
                        let range_diff = s.range - o.range;
                        if range_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self range - other range = {}", comp_id, cid, range_diff));
                        }
                        let as_diff = s.attack_speed - o.attack_speed;
                        if as_diff > 0.0 {
                            diff.push(format!("{:?}.{:?}: Self as - other as = {}", comp_id, cid, as_diff));
                        }
                        if s.flip != o.flip {
                            diff.push(format!("{:?}.{:?}: Self flip {:?}, other flip {:?}", comp_id, cid, s.flip, o.flip));
                        }
                        if s.targetable != o.targetable {
                            diff.push(format!("{:?}.{:?}: Self targetable {:?}, other targetable {:?}", comp_id, cid, s.targetable, o.targetable));
                        }

                        Some(diff.into_iter())
                    },
                    _ => None,
                }
            }).flatten());
        }
        diff
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

pub trait ErrLog {
    fn err_log(self, log: &mut World);
}

impl ErrLog for Result<(), WorldError> {
    fn err_log(self, log: &mut World) {
        if let Err(e) = self {
            log.errors.push(e);
        }
    }
}
