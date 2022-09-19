use std::collections::HashSet;

use nalgebra::Vector2;
use serde::{Serialize, Deserialize};

use crate::model::world::{component::{GetComponentID, ComponentID, Component, ComponentUpdateData}, WorldSystem, WorldInfo, WorldError, ComponentSystem, World, character::CharacterID, commands::{CharacterCommand, WorldCommand}, CharacterCommandState, Update, WorldErrorI};
use image::io::Reader as ImageReader;

const COLLISION_TEST_TEXTURE_PATH: &str = "map/collision.png";

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
        match update {
            ComponentUpdateData::Collision(CollisionUpdate(col)) => col.clone(),
            _ => self.clone(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CollisionUpdate(Collision);

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CollisionInfo {
    pub terrain: HashSet<Vector2<i32>>
}

impl CollisionInfo {
    pub fn test_collision() -> Self {
        let mut terrain = HashSet::new();
        let img = ImageReader::open(COLLISION_TEST_TEXTURE_PATH).unwrap().decode().unwrap().to_rgba8();
        for j in 0..img.height() {
            for i in 0..img.width() {
                if img[(i, j)].0[3] > 0 {
                    terrain.insert(Vector2::new(i as i32, j as i32));
                }
            }
        }

        Self { terrain }
    }
}

#[derive(Clone)]
pub struct CollisionSystem {
    info: CollisionInfo,
}

impl CollisionSystem {
    pub fn new(info: CollisionInfo) -> Self {
        Self {
            info,
        }
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

    fn update_character(&self, world: &World, commands: &[WorldCommand], cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &[ComponentUpdateData]) -> Result<Vec<ComponentUpdateData>, WorldError> {
        Ok(vec![])
    }
}
