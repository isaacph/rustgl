use serde::{Serialize, Deserialize};

use crate::model::{world::{World, character::CharacterID, commands::WorldCommand, WorldError, component::{ComponentID, GetComponentID, ComponentStorageContainer}}, commands::GetCommandID};

use super::action_queue;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackInstance {
    pub timer: f32,
    pub target: CharacterID,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttack {
    pub attack: Option<AutoAttackInstance>
}

impl GetComponentID for AutoAttack {
    const ID: ComponentID = ComponentID::AutoAttack;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackInfo {
    pub wind_up_time: f32,
    pub fire_time: f32,
    pub wind_down_time: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackRequest {
    pub attacker: CharacterID,
    pub target: CharacterID
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttackCommand {
    pub attacker: CharacterID,
    pub target: CharacterID
}

impl<'a> WorldCommand<'a> for AutoAttackCommand {
    fn run(self, world: &mut World) -> Result<(), crate::model::world::WorldError> {
        if !world.characters.contains(&self.attacker) {
            return Err(WorldError::MissingCharacter(self.attacker))
        }
        if !world.characters.contains(&self.target) {
            return Err(WorldError::MissingCharacter(self.target))
        }
        let auto_attack = world.auto_attack.get_component_mut(&self.attacker)?;
        let action_queue = world.action_queue.get_component_mut(&self.attacker)?;
        Ok(())
    }
}

impl GetCommandID for AutoAttackCommand {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::AutoAttackCommand
    }
}

impl AutoAttack {
    pub fn new() -> Self {
        Self { attack: None }
    }
}

pub fn auto_attack_system_init(_: &mut World) {
    // noop
}

pub fn auto_attack_system_update(_: &mut World, _: f32) {
    // make current attacks continue
}

pub mod server {

}