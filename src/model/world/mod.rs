use itertools::Itertools;
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
        ComponentStorage,
        ComponentID,
        ComponentStorageContainer, ComponentUpdateData, ComponentUpdate, ComponentStorageCommon
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
        },
        base::{
            CharacterBase,
            BaseSystem
        },
        health::{
            CharacterHealth,

        }, status::{Status, StatusSystem},
    }
};

use super::{commands::CommandID, Tick};

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
    Info(String),
    BadLogic,
    SimultaneousAddRemoveCharacterID(CharacterID),
    InvalidReduceMapping(CharacterID, ComponentID),
    MultipleNewCommands(CharacterID, ComponentID),
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
    pub status: ComponentStorage<Status>,

    pub icewiz: ComponentStorage<IceWiz>,
    pub caster_minion: ComponentStorage<CasterMinion>,

    // the tick local to the world, should be 100% in sync between client and server
    pub tick: Tick,
}

pub trait WorldSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError>;
}

pub trait ComponentSystem: WorldSystem {
    fn get_component_id(&self) -> ComponentID;
    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError>;
    // fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError>;
    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError>;
    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError>;
}

#[derive(Clone, Debug)]
pub enum Update {
    Comp(ComponentUpdate),
    World(WorldUpdate),
}

#[derive(Clone, Debug)]
pub enum WorldUpdate {
    NewCharacterID(CharacterID),
    RemoveCharacterID(CharacterID),
}

impl WorldUpdate {
    pub fn apply_update(&self, world: &mut World) -> Result<(), WorldError> {
        use WorldUpdate::*;
        match *self {
            NewCharacterID(id) => world.characters.insert(id),
            RemoveCharacterID(id) => world.characters.remove(&id),
        };
        Ok(())
    }
}

// immutable info about the world
// ex: base stats for wizards
pub struct WorldInfo {
    pub base: HashMap<CharacterType, CharacterBase>,
    pub health: HashMap<CharacterType, CharacterHealth>,
    pub auto_attack: HashMap<CharacterType, AutoAttackInfo>,

    pub component_systems: HashMap<ComponentID, Box<dyn ComponentSystem>>,
}

impl WorldInfo {
    pub fn new() -> WorldInfo {
        WorldInfo {
            base: HashMap::new(),
            health: HashMap::new(),
            auto_attack: HashMap::new(),
            component_systems: HashMap::new(),
        }
    }
    pub fn combine(infos: Vec<WorldInfo>, systems: HashMap<ComponentID, Box<dyn ComponentSystem>>) -> WorldInfo {
        let mut combo = WorldInfo::new();
        for info in infos {
            combo.base.extend(info.base.into_iter());
            combo.health.extend(info.health.into_iter());
            combo.auto_attack.extend(info.auto_attack.into_iter());
        }
        combo.component_systems = systems;
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
            Box::new(MovementSystem) as Box<dyn ComponentSystem>,
            Box::new(AutoAttackSystem) as Box<dyn ComponentSystem>,
            Box::new(ProjectileSystem) as Box<dyn ComponentSystem>,
            Box::new(IceWizSystem) as Box<dyn ComponentSystem>,
            Box::new(CasterMinionSystem) as Box<dyn ComponentSystem>,
            Box::new(BaseSystem) as Box<dyn ComponentSystem>,
            Box::new(StatusSystem) as Box<dyn ComponentSystem>,
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
            tick: 0,
            info: Rc::new(WorldInfo::combine(info, systems)),
            characters: HashSet::new(),
            base: ComponentStorage::new(),
            health: ComponentStorage::new(),
            movement: ComponentStorage::new(),
            auto_attack: ComponentStorage::new(),
            icewiz: ComponentStorage::new(),
            caster_minion: ComponentStorage::new(),
            projectile: ComponentStorage::new(),
            status: ComponentStorage::new(),
        }
    }

    pub fn update(&mut self, commands: &Vec<WorldCommand>, delta_time: f32) {
        let info = self.info.clone();
        let mut sorted: Vec<CharacterID> = self.characters.clone().into_iter().collect();
        sorted.sort_by_key(|id| id.get_num());
        let update_res: Vec<Result<Update, WorldError>> = sorted.iter().flat_map(
            |cid| self.get_components(cid).iter()
            .map(|comp_id| match info.component_systems.get(comp_id) {
                Some(system) => system.update_character(
                    self,
                    &commands.clone().into_iter().filter(|cmd| match cmd {
                        WorldCommand::CharacterComponent(ccid, ccomp_id, cmd) =>
                            *cid == *ccid && *comp_id == *ccomp_id,
                        _ => false
                    }).collect(),
                    cid,
                    delta_time
                ),
                None => Ok(vec![])
            }))
            .flat_map(|vec| vec
                .map_or_else(
                    |err| vec![Err(err)],
                    |res| res.into_iter()
                        .map(|res| Ok(res))
                        .collect())
                .into_iter())
            .collect();
        let component_updates: Vec<ComponentUpdate> = update_res.iter()
            .flat_map(|res| res.ok().into_iter()
                .filter_map(|res| match res {
                    Update::Comp(c) => Some(c),
                    _ => None
                })).collect();
        let world_updates: Vec<WorldUpdate> = update_res.iter()
            .flat_map(|res| res.ok().into_iter()
                .filter_map(|res| match res {
                    Update::World(c) => Some(c),
                    _ => None
                })).collect();
        let update_errors = update_res.iter().filter_map(|res| res.err());
        let mut reduce_errors = vec![];
        let reduced: Vec<Update> = self.info.component_systems.iter()
            .flat_map(|(comp_id, system)| self.get_characters(comp_id).iter()
                .flat_map(|cid| match system.reduce_changes(cid, self, &component_updates.into_iter()
                    .filter(|update| update.data.component_id() == *comp_id)
                    .map(|update| update.data).collect()) {
                        Ok(v) => v,
                        Err(err) => {
                            reduce_errors.push(err);
                            vec![]
                        }
                    }.iter()
                    .map(|data| Update::Comp(ComponentUpdate {
                        cid: *cid, data: *data
                    }))))
            .chain(match self.reduce_world_updates(world_updates) {
                Ok(v) => v.into_iter().filter_map(|update| match update {
                    Ok(update) => Some(Update::World(update)),
                    Err(err) => {
                        reduce_errors.push(err);
                        None
                    }
                }).collect(),
                Err(err) => {
                    reduce_errors.push(err);
                    vec![]
                }
            }.into_iter())
            .collect();
        let mut world = self.clone();
        world.errors.extend(update_errors);
        world.errors.extend(reduce_errors);
        world.errors.extend(reduced.into_iter()
            .filter_map(|x| match x { Update::World(x) => Some(x), _ => None })
            .filter_map(|update| update.apply_update(self).err()));
        world.errors.extend(ComponentID::iter()
            .flat_map(|comp_id| world
                .get_storage(&comp_id)
                .update(&reduced.into_iter()
                    .filter_map(|x| match x { Update::Comp(x) => Some(x), _ => None })
                    .filter(|update| update.data.component_id() == comp_id)
                    .collect())
                .err()
                .map(|errs| errs.iter())
                .unwrap_or(vec![].iter()))
            .cloned());
        self.tick += 1;
    }

    pub fn reduce_world_updates(&self, update: Vec<WorldUpdate>) -> Result<Vec<Result<WorldUpdate, WorldError>>, WorldError> {
        Ok(update.into_iter()
            .filter_map(|u| match u {
                WorldUpdate::NewCharacterID(id) | WorldUpdate::RemoveCharacterID(id) => Some((id, u)),
                _ => None,
            })
            .group_by(|(id, _)| id).into_iter()
            .map(|(id, group)| {
                let add = group.any(|(_, u)| match u {
                    WorldUpdate::NewCharacterID(_) => true, _ => false,
                });
                let remove = group.any(|(_, u)| match u {
                    WorldUpdate::RemoveCharacterID(_) => true, _ => false,
                });
                if add && remove {
                    Err(WorldError::SimultaneousAddRemoveCharacterID(*id))
                } else if add {
                    Ok(WorldUpdate::NewCharacterID(*id))
                } else {
                    Ok(WorldUpdate::RemoveCharacterID(*id))
                }
            })
            .collect())
    }

    // pub fn run_command(&mut self, _tick: i32, command: WorldCommand) -> Result<CommandRunResult, WorldError> {
    //     let x = format!("{:?}", command);
    //     self.errors.push(WorldError::Info(format!("Run cmd: {:?}", x.as_str()[0..std::cmp::min(70, x.len())].to_string())));
    //     match command {
    //         WorldCommand::CharacterComponent(cid, compid, cmd) => {
    //             match self.characters.get(&cid) {
    //                 Some(_) => {
    //                     match self.info.clone().component_systems.get(&compid) {
    //                         Some(system) => {
    //                             if let Err(err) = system.validate_character_command(self, &cid, &cmd) {
    //                                 return Ok(CommandRunResult::Invalid(err));
    //                             }
    //                             if let Err(err) = system.run_character_command(self, &cid, cmd) {
    //                                 return Ok(CommandRunResult::ValidError(err));
    //                             }
    //                             Ok(CommandRunResult::Valid)
    //                         },
    //                         None => Err(WorldError::MissingCharacterComponentSystem(cid, compid)),
    //                     }
    //                 },
    //                 None => Err(WorldError::MissingCharacter(cid, "Failed to run command for character".to_string())),
    //             }
    //         },
    //         WorldCommand::World(_) => {
    //             panic!("Not implemented");
    //         },
    //         WorldCommand::Update(update_cmd) => {
    //             // println!("Update character");
    //             update_cmd.update_character(self).map(|_| CommandRunResult::Valid)
    //         }
    //     }
    // }

    pub fn get_storage(&self, comp_id: &ComponentID) -> &dyn ComponentStorageCommon {
        match comp_id {
            ComponentID::Base => &self.base as &dyn ComponentStorageCommon,
            ComponentID::Health => &self.health as &dyn ComponentStorageCommon,
            ComponentID::Movement => &self.movement as &dyn ComponentStorageCommon,
            ComponentID::IceWiz => &self.icewiz as &dyn ComponentStorageCommon,
            ComponentID::AutoAttack => &self.auto_attack as &dyn ComponentStorageCommon,
            ComponentID::Projectile => &self.projectile as &dyn ComponentStorageCommon,
            ComponentID::CasterMinion => &self.caster_minion as &dyn ComponentStorageCommon,
            ComponentID::Status => &self.status as &dyn ComponentStorageCommon,
        }
    }

    pub fn get_storage_mut(&mut self, comp_id: &ComponentID) -> &mut dyn ComponentStorageCommon {
        match comp_id {
            ComponentID::Base => &mut self.base as &mut dyn ComponentStorageCommon,
            ComponentID::Health => &mut self.health as &mut dyn ComponentStorageCommon,
            ComponentID::Movement => &mut self.movement as &mut dyn ComponentStorageCommon,
            ComponentID::IceWiz => &mut self.icewiz as &mut dyn ComponentStorageCommon,
            ComponentID::AutoAttack => &mut self.auto_attack as &mut dyn ComponentStorageCommon,
            ComponentID::Projectile => &mut self.projectile as &mut dyn ComponentStorageCommon,
            ComponentID::CasterMinion => &mut self.caster_minion as &mut dyn ComponentStorageCommon,
            ComponentID::Status => &mut self.status as &mut dyn ComponentStorageCommon,
        }
    }

    pub fn has_component(&self, id: &CharacterID, cid: &ComponentID) -> bool {
        self.get_storage(cid).has_character(id)
    }

    pub fn serialize_component(&self, id: &CharacterID, cid: &ComponentID) -> Option<Vec<u8>> {
        self.get_storage(cid).serialize(id)
    }

    pub fn update_component(&mut self, id: &CharacterID, cid: &ComponentID, data: Vec<u8>) {
        // self.get_storage_mut(cid).deserialize_insert(id, data);
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
                    } else if let Some(my_exec) = &current.execution {
                        if let Some(their_exec) = &des.execution {
                            if my_exec.timer != their_exec.timer {
                                self.errors.push(WorldError::DesyncError(*id, *cid, format!(
                                            "Current executing timer: {}, remote executing timer: {}",
                                            my_exec.timer,
                                            their_exec.timer)));
                            }
                        }
                    }
                }
                self.auto_attack.get_storage_mut().insert(*id, des);
//                insert(&mut self.auto_attack, id, cid, data)
            },
            ComponentID::Projectile => insert(&mut self.projectile, id, cid, data),
            ComponentID::CasterMinion => insert(&mut self.caster_minion, id, cid, data),
            ComponentID::Status => insert(&mut self.status, id, cid, data),
            // _ => panic!("Deserialization not implemented for component id: {}", cid)
        }
    }

    pub fn erase_component(&mut self, cid: &CharacterID, id: &ComponentID) {
        self.get_storage_mut(id).erase(cid);
    }

    pub fn get_characters(&mut self, cid: &ComponentID) -> Vec<CharacterID> {
        self.get_storage(cid).get_characters()
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
