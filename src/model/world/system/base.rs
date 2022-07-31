use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};

use crate::model::world::{character::{CharacterType, CharacterID}, component::{GetComponentID, ComponentID, ComponentUpdateData, Component, ComponentUpdate}, WorldSystem, WorldInfo, WorldError, ComponentSystem, World, commands::{CharacterCommand, WorldCommand}};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CharacterFlip {
    Left, Right
}

impl CharacterFlip {
    pub fn from_dir(dir: &Vector2<f32>) -> Option<CharacterFlip> {
        if dir.x > 0.0 {
            Some(CharacterFlip::Right)
        } else if dir.x < 0.0 {
            Some(CharacterFlip::Left)
        } else {
            None
        }
    }

    pub fn dir(&self) -> f32 {
        match *self {
            Self::Left => -1.0,
            Self::Right => 1.0
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterBase {
    pub ctype: CharacterType,
    pub position: Vector3<f32>,
    pub center_offset: Vector3<f32>,
    pub speed: f32,
    pub attack_damage: f32,
    pub range: f32,
    pub attack_speed: f32,
    pub flip: CharacterFlip,
    pub targetable: bool,
}

impl Component for CharacterBase {
    fn update(&self, change: &ComponentUpdateData) -> Self {
        let mut next = self.clone();
        match *change {
            ComponentUpdateData::Base(change) => match change {
                CharacterBaseUpdate::FlipUpdate(flip) => next.flip = flip,
                CharacterBaseUpdate::PositionUpdate(_, change) => match change {
                    CharacterBasePositionUpdate::Move(add) => next.position += add,
                    CharacterBasePositionUpdate::Override(pos) =>  next.position = pos,
                }
            }
        }
        next
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterBasePositionUpdate {
    Move(Vector3<f32>),
    Override(Vector3<f32>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Walk,
    Cast,
    Stun
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterBaseUpdate {
    FlipUpdate(CharacterFlip),
    PositionUpdate(Priority, CharacterBasePositionUpdate),
}

impl GetComponentID for CharacterBase {
    const ID: ComponentID = ComponentID::Base;
}

pub struct BaseSystem;

impl WorldSystem for BaseSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for BaseSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Base
    }

    // fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError> {
    //     Err(WorldError::InvalidCommandMapping)
    // }

    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<ComponentUpdate>, WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }

    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError> {
        Ok(())
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Vec<ComponentUpdateData> {
        vec![]
    }
}