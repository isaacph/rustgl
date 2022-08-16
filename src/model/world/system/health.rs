use serde::{Serialize, Deserialize};

use crate::model::world::{component::{Component, ComponentUpdateData, GetComponentID, ComponentID, ComponentUpdate}, WorldSystem, WorldError, WorldInfo, ComponentSystem, commands::{CharacterCommand, WorldCommand}, character::CharacterID, World, Update, CharacterCommandState, WorldErrorI};


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterHealth {
    pub health: f32,
    pub max_health: f32,
}

impl Component for CharacterHealth {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match *update {
            ComponentUpdateData::Health(CharacterHealthUpdate::New(x)) => {
                println!("Health reinit");
                Self { health: x, max_health: x }},
            ComponentUpdateData::Health(CharacterHealthUpdate::Change(x)) => {
                Self { health: self.health + x, max_health: self.max_health }
            },
            _ => *self
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

pub fn make_health_update(cid: &CharacterID, health_change: f32) -> Update {
    Update::Comp(
        ComponentUpdate {
            cid: *cid,
            data: ComponentUpdateData::Health(
                CharacterHealthUpdate::Change(health_change)
            )
        }
    )
}

impl ComponentSystem for HealthSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Health
    }
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        Err(WorldErrorI::InvalidCommandMapping.err())
    }
    fn update_character(&self, _: &World, _: &[WorldCommand], _: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }
    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &[ComponentUpdateData]) -> Result<Vec<ComponentUpdateData>, WorldError> {
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.iter()
                .filter(|new| matches!(*new, ComponentUpdateData::Health(CharacterHealthUpdate::New(_))))
                .cloned().collect();
            if new_changes.is_empty() {
                return Err(WorldErrorI::InvalidReduceMapping(*cid, ComponentID::Status).err())
            } else if new_changes.len() > 1 {
                return Err(WorldErrorI::MultipleUpdateOverrides(*cid, ComponentID::Status).err())
            } else {
                return Ok(new_changes)
            }
        }
        let mut current = None;
        for change in changes {
            if let ComponentUpdateData::Health(change) = change.clone() {
                match change {
                    CharacterHealthUpdate::New(_health) => (), // ignore, only applies if cid is new
                    CharacterHealthUpdate::Change(delta) => current = Some(current.unwrap_or(0.0) + delta)
                }
            }
        }
        Ok(current.into_iter()
           .map(|change| ComponentUpdateData::Health(CharacterHealthUpdate::Change(change)))
           .collect())
    }
}
