use serde::{Serialize, Deserialize};

use crate::model::world::{component::{Component, ComponentUpdateData, GetComponentID, ComponentID}, WorldSystem, WorldError, WorldInfo, ComponentSystem, commands::{CharacterCommand, WorldCommand}, character::CharacterID, World, Update};


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterHealth {
    pub health: f32,
    pub max_health: f32,
}

impl Component for CharacterHealth {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        self.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterHealthUpdate {
    Change(f32) // todo: add cause
}

impl GetComponentID for CharacterHealth {
    const ID: ComponentID = ComponentID::Health;
}

pub struct HealthSystem;

impl WorldSystem for HealthSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for HealthSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Health
    }
    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }
    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        let mut current = None;
        for change in changes {
            match *change {
                ComponentUpdateData::Health(change) => match change {
                    CharacterHealthUpdate::Change(delta) => current = Some(current.unwrap_or(0.0) + delta)
                },
                _ => (),
            }
        }
        Ok(current.into_iter()
           .map(|change| ComponentUpdateData::Health(CharacterHealthUpdate::Change(change)))
           .collect())
    }
}
