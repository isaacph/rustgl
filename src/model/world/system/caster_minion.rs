use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::world::{component::{GetComponentID, ComponentID, CharacterBase, CharacterFlip, CharacterHealth}, CharacterCreatorCreator, CharacterCreator, World, character::{CharacterID, CharacterType}, WorldError};

use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo}};



#[derive(Serialize, Deserialize, Debug)]
pub struct CasterMinion {

}

impl GetComponentID for CasterMinion {
    const ID: ComponentID = ComponentID::CasterMinion;
}

pub struct CasterMinionCreatorCreator;
pub struct CasterMinionCreator;
impl CharacterCreatorCreator for CasterMinionCreatorCreator {
    fn create(&self) -> Box<dyn CharacterCreator> {
        Box::new(CasterMinionCreator)
    }
}
impl CharacterCreator for CasterMinionCreator {
    fn create(&mut self, world: &mut World, id: &CharacterID) -> Result<(), WorldError> {
        let typ = CharacterType::CasterMinion;
        let id = *id;
        world.characters.insert(id);
        // start these two at base stats
        world.base.components.insert(id,
            *world.info.base.get(&typ)
                .ok_or(WorldError::MissingCharacterInfoComponent(typ, ComponentID::Base))?
        );
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
}

pub fn caster_minion_system_init(world: &mut World) -> Result<(), WorldError> {
    world.info.base.insert(CharacterType::CasterMinion, CharacterBase {
        ctype: CharacterType::CasterMinion,
        position: Vector2::new(0.0, 0.0),
        speed: 1.0,
        attack_damage: 1.0,
        range: 1.0,
        attack_speed: 2.0,
        flip: CharacterFlip::Right,
        targetable: true,
    });
    world.info.health.insert(CharacterType::CasterMinion, CharacterHealth {
        health: 100.0
    });
    world.info.auto_attack.insert(CharacterType::CasterMinion, AutoAttackInfo::init(
        CharacterType::CasterMinion,
        1.0, // wind up duration
        3.0, // casting duration
        1.0, // wind down duration
        2.5, // fire time (after animation start)
        0.5, // projectile speed
        Vector2::new(0.3, 0.0) // projectile offset
    )?);
    world.character_creator.insert(CharacterType::CasterMinion, Box::new(CasterMinionCreatorCreator));
    Ok(())
}

pub fn caster_minion_system_update(_: &mut World, _: f32) -> Result<(), WorldError> {
    Ok(())
}