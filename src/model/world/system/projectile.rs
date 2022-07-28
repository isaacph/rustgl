use nalgebra::Vector3;
use serde::{Serialize, Deserialize};

use crate::model::world::{character::{CharacterID, CharacterType}, component::{GetComponentID, ComponentID, CharacterBase, ComponentStorageContainer, CharacterFlip}, World, WorldError, WorldInfo, WorldSystem, commands::CharacterCommand};

use super::movement::fly_to;

#[derive(Serialize, Deserialize, Clone)]
pub struct Projectile {
    pub origin: CharacterID,
    pub target: CharacterID,
}

impl GetComponentID for Projectile {
    const ID: ComponentID = ComponentID::Projectile;
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
    world: &mut World,
    info: &ProjectileCreationInfo
) -> Result<(), WorldError> {
    let typ = CharacterType::Projectile;
    world.characters.insert(info.proj_id);
    let position = {
        let base = world.base.get_component(&info.origin)?;
        base.position + Vector3::new(info.starting_offset.x * base.flip.dir(), info.starting_offset.y, info.starting_offset.z)
    };
    world.base.components.insert(info.proj_id,
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
    );
    world.projectile.components.insert(info.proj_id,
        Projectile {
            origin: info.origin,
            target: info.target
        }
    );
    Ok(())
}

pub struct ProjectileSystem;

impl WorldSystem for ProjectileSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Projectile
    }

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

    fn update_character(&self, world: &mut World, cid: &CharacterID, delta_time: f32) -> Result<(), WorldError> {
        let target = world.projectile.get_component(&cid)?.target;
        if world.characters.get(&target).is_none() {
            world.erase_character(&cid)?;
            return Err(WorldError::MissingCharacter(target, "Projectile target doesn't exist".to_string()))
        }
        let base = world.base.get_component(&target)?;
        let dest = base.position + base.center_offset;
        let range = 0.0;
        let damage = world.base.get_component(&cid)?.attack_damage;
        match fly_to(world, &cid, &dest, range, delta_time)? {
            Some(_) => {
                world.erase_character(&cid)?;
                // do damage
                let health = world.health.get_component_mut(&target)?;
                health.health -= damage;
                if health.health <= 0.0 {
                    world.erase_character(&target)?;
                }
            },
            None => (),
        }
        Ok(())
    }

    fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }

    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
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
