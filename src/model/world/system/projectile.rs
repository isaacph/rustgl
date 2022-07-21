use std::collections::HashMap;

use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::world::{character::{CharacterID, CharacterType}, component::{GetComponentID, ComponentID, CharacterBase, ComponentStorageContainer, CharacterFlip}, World, WorldError, ErrLog};

use super::movement::move_to;

#[derive(Serialize, Deserialize)]
pub struct Projectile {
    pub origin: CharacterID,
    pub target: CharacterID,
}

impl GetComponentID for Projectile {
    const ID: ComponentID = ComponentID::Projectile;
}

#[derive(Serialize, Deserialize)]
pub struct ProjectileInfo {
    create_frame: HashMap<CharacterID, u64>
}

impl ProjectileInfo {
    pub fn init() -> Self {
        ProjectileInfo { create_frame: HashMap::new() }
    }
}

pub struct ProjectileCreationInfo {
    pub proj_id: CharacterID,
    pub origin: CharacterID,
    pub target: CharacterID,
    pub starting_offset: Vector2<f32>,
    pub speed: f32,
    pub damage: f32,
}

pub fn create(
    world: &mut World,
    info: &ProjectileCreationInfo,
    init_travel_time: f32
) -> Result<(), WorldError> {
    let typ = CharacterType::Projectile;
    world.characters.insert(info.proj_id);
    world.base.components.insert(info.proj_id,
        CharacterBase {
            ctype: typ,
            position: world.base.get_component(&info.origin)?.position + info.starting_offset,
            speed: info.speed,
            attack_damage: info.damage,
            range: 0.0,
            attack_speed: 0.0,
            flip: CharacterFlip::Right,
        }
    );
    world.projectile.components.insert(info.proj_id,
        Projectile {
            origin: info.origin,
            target: info.target
        }
    );
    projectile_update(world, init_travel_time, info.proj_id)
}

pub fn projectile_system_init(_: &mut World) -> Result<(), WorldError> {
    Ok(())
}

fn projectile_update(world: &mut World, delta_time: f32, cid: CharacterID) -> Result<(), WorldError> {
    if let Some(frame_id) = world.info.projectile.create_frame.get(&cid) {
        if *frame_id == world.frame_id {
            world.info.projectile.create_frame.remove(&cid); // only needed at most once
            return Ok(())
        }
    }
    let target = world.projectile.get_component(&cid)?.target;
    let dest = world.base.get_component(&target)?.position;
    let range = 0.0;
    let damage = world.base.get_component(&cid)?.attack_damage;
    match move_to(world, &cid, &dest, range, delta_time)? {
        Some(_) => {
            world.erase_character(&cid)?;
            // do damage
            println!("Would do {} damage to {:?}", damage, target);
        },
        None => (),
    }
    Ok(())
}

pub fn projectile_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
    let cids: Vec<CharacterID> = world.projectile.components.keys().copied().collect();
    for cid in cids {
        projectile_update(world, delta_time, cid).err_log(world);
    }
    Ok(())
}
