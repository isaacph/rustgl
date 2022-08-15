use std::collections::HashSet;

use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::world::{component::{GetComponentID, ComponentID, Component, ComponentUpdateData}, WorldSystem, WorldInfo, WorldError, ComponentSystem, World, character::CharacterID, commands::{CharacterCommand, WorldCommand}, CharacterCommandState, Update, WorldErrorI};

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone)]
pub enum Layer {
    Terrain,
    Minions,
    Players,
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum Collider {
    Point,
    Circle(f32),
}
impl Eq for Collider {}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Collision {
    pub collider: Collider,
    pub against: HashSet<Layer>
}

impl GetComponentID for Collision {
    const ID: ComponentID = ComponentID::Collision;
}

impl Default for Collision {
    fn default() -> Self {
        Self { collider: Collider::Point, against: HashSet::new() }
    }
}

impl Component for Collision {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match *update {
            ComponentUpdateData::Collision(CollisionUpdate(col)) => col,
            _ => self.clone(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CollisionUpdate(Collision);

#[derive(Clone)]
pub struct CollisionSystem {
    pub terrain: HashSet<Vector2<i32>>,
}

impl CollisionSystem {
    pub fn new() -> Self {
        Self {
            terrain: Default::default()
        }
    }
}

impl Default for CollisionSystem {
    fn default() -> Self {
        // todo: grab some default terrain
        Self::new()
    }
}

impl WorldSystem for CollisionSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for CollisionSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Collision
    }

    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        Err(WorldErrorI::InvalidCommandMapping.err())
    }

    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        Ok(vec![])
    }
}