use serde::{Serialize, Deserialize};

use crate::model::world::{component::{Component, ComponentUpdateData, GetComponentID, ComponentID}, WorldSystem, WorldError, WorldInfo, ComponentSystem, commands::{CharacterCommand, WorldCommand}, character::CharacterID, World, Update, CharacterCommandState};


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterHealth {
    pub health: f32,
    pub max_health: f32,
}

impl Component for CharacterHealth {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match *update {
            ComponentUpdateData::Health(CharacterHealthUpdate::New(x)) =>
                Self { health: x, max_health: x },
            ComponentUpdateData::Health(CharacterHealthUpdate::Change(x)) =>
                Self { health: self.health + x, max_health: self.max_health },
            _ => self.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterHealthUpdate {
    Change(f32), // todo: add cause
    New(f32),
}

impl GetComponentID for CharacterHealth {
    const ID: ComponentID = ComponentID::Health;
}

impl Default for CharacterHealth {
    fn default() -> Self {
        Self { health: 1.0, max_health: 1.0 }
    }
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
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn update_character(&self, _: &World, _: &Vec<WorldCommand>, _: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }
    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.into_iter().filter(|new| match *new {
                ComponentUpdateData::Health(CharacterHealthUpdate::New(_)) => true,
                _ => false,
            }).cloned().collect();
            if new_changes.len() == 0 {
                return Err(WorldError::InvalidReduceMapping(*cid, ComponentID::Status))
            } else if new_changes.len() > 1 {
                return Err(WorldError::MultipleUpdateOverrides(*cid, ComponentID::Status))
            } else {
                return Ok(new_changes)
            }
        }
        let mut current = None;
        for change in changes {
            match change.clone() {
                ComponentUpdateData::Health(change) => match change {
                    CharacterHealthUpdate::New(_health) => (), // ignore, only applies if cid is new
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
