use nalgebra::Vector2;
use serde::{Serialize, Deserialize};
use crate::model::world::{World, CharacterCreatorCreator, CharacterCreator, character::{CharacterID, CharacterType}, component::{CharacterBase, CharacterHealth}};
use super::{movement::Movement, auto_attack::AutoAttack, action_queue::ActionQueue};

#[derive(Serialize, Deserialize, Debug)]
pub struct IceWiz {
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
    fn create(&mut self, world: &mut World, id: &CharacterID) {
        let id = *id;
        world.characters.insert(id);
        world.info.base.insert(CharacterType::IceWiz, CharacterBase {
            ctype: CharacterType::IceWiz,
            position: Vector2::new(0.0, 0.0),
            speed: 1.0,
            attack_damage: 1.0,
        });
        world.info.health.insert(CharacterType::IceWiz, CharacterHealth {
            health: 100.0
        });
        world.base.components.insert(id, CharacterBase {
            ctype: CharacterType::IceWiz,
            position: Vector2::new(0.0, 0.0),
            speed: 1.0,
            attack_damage: 1.0,
        });
        world.health.components.insert(id, CharacterHealth {
            health: 100.0
        });
        world.movement.components.insert(id, Movement {
            destination: None
        });
        world.icewiz.components.insert(id, IceWiz {});
        world.auto_attack.components.insert(id, AutoAttack {});
        world.action_queue.components.insert(id, ActionQueue::new());
    }
}

pub fn icewiz_system_init(world: &mut World) {
    world.character_creator.insert(CharacterType::IceWiz, Box::new(IceWizCreatorCreator));
}

pub fn icewiz_system_update(_: &mut World, _: f32) {
}
