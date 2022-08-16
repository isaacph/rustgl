use nalgebra::{Vector3, Vector2};
use serde::{Serialize, Deserialize};

use crate::model::world::{character::{CharacterID, CharacterType}, component::{GetComponentID, ComponentID, ComponentStorageContainer, ComponentUpdateData, Component, ComponentUpdate}, World, WorldError, WorldInfo, WorldSystem, commands::{CharacterCommand, WorldCommand, Priority}, ComponentSystem, Update, WorldUpdate, CharacterCommandState, WorldErrorI};

use super::{base::{CharacterBase, CharacterFlip, make_move_update, make_flip_update, CharacterBaseUpdate}, health::make_health_update};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Projectile {
    pub origin: CharacterID,
    pub target: CharacterID,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectileUpdate(Projectile);

impl Component for Projectile {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match update.clone() {
            ComponentUpdateData::Projectile(ProjectileUpdate(p)) => p,
            _ => self.clone(),
        }
    }
}

impl GetComponentID for Projectile {
    const ID: ComponentID = ComponentID::Projectile;
}

impl Default for Projectile {
    fn default() -> Self {
        Self { origin: CharacterID::error(), target: CharacterID::error() }
    }
}

pub struct ProjectileCreationInfo {
    pub proj_id: CharacterID,
    pub origin: CharacterID,
    pub target: CharacterID,
    pub starting_offset: Vector3<f32>,
    pub speed: f32,
    pub damage: f32,
}

pub fn create(
    world: &World,
    info: &ProjectileCreationInfo
) -> Result<Vec<Update>, WorldError> {
    let typ = CharacterType::Projectile;
    let position = {
        let base = world.base.get_component(&info.origin)?;
        base.position + Vector3::new(info.starting_offset.x * base.flip.dir(), info.starting_offset.y, info.starting_offset.z)
    };
    Ok(vec![
        Update::World(WorldUpdate::NewCharacterID(info.proj_id)),
        Update::Comp(ComponentUpdate {
            cid: info.proj_id,
            data: ComponentUpdateData::Base(
                CharacterBaseUpdate::New(
                    CharacterBase {
                        ctype: typ,
                        position,
                        center_offset: Vector3::new(0.0, 0.0, 0.0),
                        speed: info.speed,
                        attack_damage: info.damage,
                        range: 0.0,
                        attack_speed: 0.0,
                        flip: CharacterFlip::Right,
                        targetable: false,
                    }
                )
            )
        }),
        Update::Comp(ComponentUpdate {
            cid: info.proj_id,
            data: ComponentUpdateData::Projectile(
                ProjectileUpdate(
                    Projectile {
                        origin: info.origin,
                        target: info.target
                    }
                )
            )
        })
    ])
}

pub struct ProjectileSystem;

impl WorldSystem for ProjectileSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        let mut info = WorldInfo::new();
        info.base.insert(CharacterType::Projectile,
            CharacterBase {
                ctype: CharacterType::Projectile,
                position: Vector3::new(0.0, 0.0, 0.0),
                center_offset: Vector3::new(0.0, 0.0, 0.0),
                attack_damage: 0.0,
                range: 0.0,
                attack_speed: 0.0,
                flip: CharacterFlip::Right,
                targetable: false,
                speed: 0.0,
            }
        );
        Ok(info)
    }
}

// try to start an action on a target at a range. if in range, return Some(0.0), indicating to start the action.
// if out of range, move towards the target, and if the target ends up in range during the frame,
// return Some(x) where x is the remaining time to spend on the attack, after consuming necessary
// time for walking
pub fn fly_to(world: &World, cid: &CharacterID, dest: &Vector3<f32>, range: f32, delta_time: f32) -> Result<(bool, Vec<Update>), WorldError> {
    let base = world.base.get_component(cid)?;
    let speed = base.speed;
    let max_travel = speed * delta_time;
    let dir = dest - base.position;
    if dir.x == 0.0 && dir.y == 0.0 && dir.z == 0.0 {
        return Ok((true, vec![]))
    }
    let dist = dir.magnitude();
    if speed <= 0.0 {
        return Ok((dist <= range, vec![]))
    }
    let flip = CharacterFlip::from_dir(&Vector2::new(dir.x, dir.y)).unwrap_or(base.flip);
    if f32::max(dist - max_travel, 0.0) <= range {
        let travel = f32::max(dist - range, 0.0);
        let _remaining_time = delta_time - travel / speed;
        let offset = dir / dist * travel;
        // base.position += offset;
        Ok((true, vec![
            make_move_update(*cid, Priority::Walk, offset),
            make_flip_update(*cid, Priority::Walk, flip)
        ]))
    } else {
        let offset = dir / dist * max_travel;
        // base.position += offset;
        Ok((false, vec![
            make_move_update(*cid, Priority::Walk, offset),
            make_flip_update(*cid, Priority::Walk, flip)
        ]))
    }
}

impl ComponentSystem for ProjectileSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Projectile
    }

    fn update_character(&self, world: &World, _commands: &[WorldCommand], cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        let target = world.projectile.get_component(cid)?.target;
        if world.characters.get(&target).is_none() {
            return Ok(vec![Update::World(WorldUpdate::RemoveCharacterID(*cid))]);
            // return Err(WorldError::MissingCharacter(target, "Projectile target doesn't exist".to_string()))
        }
        let base = world.base.get_component(&target)?;
        let dest = base.position + base.center_offset;
        let range = 0.0;
        let damage = world.base.get_component(cid)?.attack_damage;
        let (arrived, fly_updates) = fly_to(world, cid, &dest, range, delta_time)?;
        if arrived {
            // world.erase_character(&cid)?;
            // do damage
            let health = world.health.get_component(&target)?;
            // health.health -= damage;
            // if health.health - damage <= 0.0 {
            //     world.erase_character(&target)?;
            // }
            Ok([
               Some(Update::World(WorldUpdate::RemoveCharacterID(*cid))),
               Some(make_health_update(&target, -damage)),
               if health.health - damage <= 0.0 {
                   Some(Update::World(WorldUpdate::RemoveCharacterID(target)))
               } else {
                   None
               }
            ].into_iter().flatten().collect())
        } else {
            Ok(fly_updates)
        }
    }

    // fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
    //     Err(WorldError::InvalidCommandMapping)
    // }

    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        Err(WorldErrorI::InvalidCommandMapping.err())
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &[ComponentUpdateData]) -> Result<Vec<ComponentUpdateData>, WorldError> {
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.iter()
                .filter(|new| matches!(*new, ComponentUpdateData::Projectile(ProjectileUpdate(_))))
                .cloned().collect();
            if new_changes.is_empty() {
                return Err(WorldErrorI::InvalidReduceMapping(*cid, ComponentID::Status).err())
            } else if new_changes.len() > 1 {
                return Err(WorldErrorI::MultipleUpdateOverrides(*cid, ComponentID::Status).err())
            } else {
                return Ok(new_changes)
            }
        }
        if changes.len() > 1 {
            Err(WorldErrorI::InvalidReduceMapping(*cid, ComponentID::Projectile).err())
        } else {
            Ok(changes.to_vec())
        }
    }
}

// pub fn projectile_system_init() -> Result<WorldInfo, WorldError> {
//     let mut info = WorldInfo::new();
//     info.base.insert(CharacterType::Projectile,
//         CharacterBase {
//             ctype: CharacterType::Projectile,
//             position: Vector3::new(0.0, 0.0, 0.0),
//             center_offset: Vector3::new(0.0, 0.0, 0.0),
//             attack_damage: 0.0,
//             range: 0.0,
//             attack_speed: 0.0,
//             flip: CharacterFlip::Right,
//             targetable: false,
//             speed: 0.0,
//         }
//     );
//     Ok(info)
// }

// fn projectile_update(world: &mut World, delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
//     if let Some(frame_id) = world.projectile_system.create_frame.get(&cid) {
//         if *frame_id == world.frame_id {
//             world.projectile_system.create_frame.remove(&cid); // only needed at most once
//             return Ok(())
//         }
//     }
//     let target = world.projectile.get_component(&cid)?.target;
//     if world.characters.get(&target).is_none() {
//         world.erase_character(&cid)?;
//         return Err(WorldError::MissingCharacter(target, "Projectile target doesn't exist".to_string()))
//     }
//     let base = world.base.get_component(&target)?;
//     let dest = base.position + base.center_offset;
//     let range = 0.0;
//     let damage = world.base.get_component(&cid)?.attack_damage;
//     match fly_to(world, &cid, &dest, range, delta_time)? {
//         Some(_) => {
//             world.erase_character(&cid)?;
//             // do damage
//             let health = world.health.get_component_mut(&target)?;
//             health.health -= damage;
//             if health.health <= 0.0 {
//                 world.erase_character(&target)?;
//             }
//         },
//         None => (),
//     }
//     Ok(())
// }
// 
// pub fn projectile_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
//     let cids: Vec<CharacterID> = world.projectile.components.keys().copied().collect();
//     for cid in cids {
//         projectile_update(world, delta_time, cid).err_log(world);
//     }
//     Ok(())
// }
