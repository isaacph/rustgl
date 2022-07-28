use nalgebra::Vector3;
use serde::{Serialize, Deserialize};
use crate::model::world::{World, character::{CharacterID, CharacterType}, component::{CharacterBase, CharacterHealth, GetComponentID, ComponentID, CharacterFlip}, WorldError, WorldInfo, WorldSystem, commands::CharacterCommand};
use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IceWiz {
}

impl GetComponentID for IceWiz {
    const ID: ComponentID = ComponentID::IceWiz;
}

pub fn create(world: &mut World, id: &CharacterID, position: Vector3<f32>) -> Result<(), WorldError> {
    let typ = CharacterType::IceWiz;
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
    world.icewiz.components.insert(id, IceWiz {});
    world.auto_attack.components.insert(id, AutoAttack::new());
    Ok(())
}

pub struct IceWizSystem;

impl WorldSystem for IceWizSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::IceWiz
    }

    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        let mut info = WorldInfo::new();
        info.base.insert(CharacterType::IceWiz, CharacterBase {
            ctype: CharacterType::IceWiz,
            position: Vector3::new(0.0, 0.0, 0.0),
            center_offset: Vector3::new(0.0, 0.0, -0.4),
            speed: 1.0,
            attack_damage: 1.0,
            range: 1.0,
            attack_speed: 1.0,
            flip: CharacterFlip::Right,
            targetable: true,
        });
        info.health.insert(CharacterType::IceWiz, CharacterHealth {
            health: 1000.0,
            max_health: 1000.0,
        });
        info.auto_attack.insert(CharacterType::IceWiz, AutoAttackInfo::init(
            CharacterType::IceWiz,
            1.0, // wind up duration
            3.0, // casting duration
            1.0, // wind down duration
            2.5, // fire time (after animation start)
            1.2, // projectile speed
            Vector3::new(0.2, 0.0, -0.4) // projectile offset
        )?);
        Ok(info)
    }

    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn update_character(&self, _: &mut World, _: &CharacterID, _: f32) -> Result<(), WorldError> {
        Ok(())
    }
}

