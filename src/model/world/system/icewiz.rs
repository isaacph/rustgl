use nalgebra::Vector2;
use serde::{Serialize, Deserialize};
use crate::model::world::{World, CharacterCreatorCreator, CharacterCreator, character::{CharacterID, CharacterType}, component::{CharacterBase, CharacterHealth, GetComponentID, ComponentID}, WorldError};
use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo}};

#[derive(Serialize, Deserialize, Debug)]
pub struct IceWiz {
}

impl GetComponentID for IceWiz {
    const ID: ComponentID = ComponentID::IceWiz;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IceWizInfo {
}

impl IceWizInfo {
    pub fn init() -> Self {
        Self {}
    }
}

pub struct IceWizCreatorCreator;
pub struct IceWizCreator;
impl CharacterCreatorCreator for IceWizCreatorCreator {
    fn create(&self) -> Box<dyn CharacterCreator> {
        Box::new(IceWizCreator)
    }
}
impl CharacterCreator for IceWizCreator {
    fn create(&mut self, world: &mut World, id: &CharacterID) -> Result<(), WorldError> {
        let typ = CharacterType::IceWiz;
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
        world.icewiz.components.insert(id, IceWiz {});
        world.auto_attack.components.insert(id, AutoAttack::new());
        Ok(())
    }
}

pub fn icewiz_system_init(world: &mut World) -> Result<(), WorldError> {
    world.info.base.insert(CharacterType::IceWiz, CharacterBase {
        ctype: CharacterType::IceWiz,
        position: Vector2::new(0.0, 0.0),
        speed: 1.0,
        attack_damage: 1.0,
        range: 1.0,
        attack_speed: 1.0,
    });
    world.info.health.insert(CharacterType::IceWiz, CharacterHealth {
        health: 100.0
    });
    world.info.auto_attack.insert(CharacterType::IceWiz, AutoAttackInfo::init(
        CharacterType::IceWiz,
        1.0, // wind up duration
        1.0, // casting duration
        1.0, // wind down duration
        1.0  // fire time (after animation start)
    )?);
//    world.info.auto_attack.insert(CharacterType::IceWiz, AutoAttackInfo {
//        wind_up_time: 1.0 / 3.0,
//        casting_time: 1.0 / 3.0,
//        wind_down_time: 1.0 / 3.0,
//        fire_time: AutoAttackTiming::AfterWindUp(0.0),
//    });
    world.character_creator.insert(CharacterType::IceWiz, Box::new(IceWizCreatorCreator));
    Ok(())
}

pub fn icewiz_system_update(_: &mut World, _: f32) -> Result<(), WorldError> {
    Ok(())
}
