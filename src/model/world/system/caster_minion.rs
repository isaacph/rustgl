use nalgebra::Vector3;
use serde::{Serialize, Deserialize};

use crate::model::world::{component::{GetComponentID, ComponentID, CharacterBase, CharacterFlip, CharacterHealth}, World, character::{CharacterID, CharacterType}, WorldError, WorldInfo, WorldSystem, commands::CharacterCommand};

use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CasterMinion {

}

impl GetComponentID for CasterMinion {
    const ID: ComponentID = ComponentID::CasterMinion;
}

pub struct CasterMinionSystem;

impl WorldSystem for CasterMinionSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::CasterMinion
    }

    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        let mut info = WorldInfo::new();
        info.base.insert(CharacterType::CasterMinion, CharacterBase {
            ctype: CharacterType::CasterMinion,
            position: Vector3::new(0.0, 0.0, 0.0),
            center_offset: Vector3::new(0.0, 0.0, -0.2),
            speed: 1.0,
            attack_damage: 1.0,
            range: 1.0,
            attack_speed: 2.0,
            flip: CharacterFlip::Right,
            targetable: true,
        });
        info.health.insert(CharacterType::CasterMinion, CharacterHealth {
            health: 1000.0,
            max_health: 1000.0,
        });
        info.auto_attack.insert(CharacterType::CasterMinion, AutoAttackInfo::init(
            CharacterType::CasterMinion,
            1.0, // wind up duration
            3.0, // casting duration
            1.0, // wind down duration
            2.5, // fire time (after animation start)
            0.5, // projectile speed
            Vector3::new(0.12, 0.0, -0.12) // projectile offset
        )?);
        Ok(info)
    }

    fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn update_character(&self, _: &mut World, _: &CharacterID, _: f32) -> Result<(), WorldError> {
        Ok(())
    }
}

pub fn create(world: &mut World, id: &CharacterID, position: Vector3<f32>) -> Result<(), WorldError> {
    let typ = CharacterType::CasterMinion;
    let id = *id;
    world.characters.insert(id);
    // start these two at base stats
    let mut base = *world.info.base.get(&typ)
    .ok_or(WorldError::MissingCharacterInfoComponent(typ, ComponentID::Base))?;
    base.position = position;
    world.base.components.insert(id, base);
    world.health.components.insert(id,
        *world.info.health.get(&typ)
            .ok_or(WorldError::MissingCharacterInfoComponent(typ, ComponentID::Health))?
    );
    // start the rest at empty states
    world.movement.components.insert(id, Movement {
        destination: None
    });
    world.caster_minion.components.insert(id, CasterMinion {});
    world.auto_attack.components.insert(id, AutoAttack::new());
    Ok(())
}

