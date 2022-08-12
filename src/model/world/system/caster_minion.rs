use nalgebra::{Vector3, Vector2};
use serde::{Serialize, Deserialize};

use crate::model::world::{component::{GetComponentID, ComponentID, ComponentUpdateData, Component, ComponentUpdate}, World, character::{CharacterID, CharacterType}, WorldError, WorldInfo, WorldSystem, commands::{CharacterCommand, WorldCommand}, ComponentSystem, Update, WorldUpdate};

use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo, AutoAttackUpdate}, base::{CharacterBase, CharacterFlip, CharacterBaseUpdate}, health::{CharacterHealth, CharacterHealthUpdate}, status::{StatusUpdate, idle_status}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CasterMinion {

}

impl Component for CasterMinion {
    fn update(&self, _update: &ComponentUpdateData) -> Self {
        self.clone()
    }
}

impl GetComponentID for CasterMinion {
    const ID: ComponentID = ComponentID::CasterMinion;
}

impl Default for CasterMinion {
    fn default() -> Self {
        CasterMinion {  }
    }
}

pub struct CasterMinionSystem;

impl WorldSystem for CasterMinionSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        let mut info = WorldInfo::new();
        info.base.insert(CharacterType::CasterMinion, CharacterBase {
            ctype: CharacterType::CasterMinion,
            position: Vector3::new(0.0, 0.0, 0.0),
            center_offset: Vector3::new(0.0, 0.0, -0.2),
            speed: 1.0,
            attack_damage: 1.0,
            range: 1.0,
            attack_speed: 2.0,
            flip: CharacterFlip::Right,
            targetable: true,
        });
        info.health.insert(CharacterType::CasterMinion, CharacterHealth {
            health: 1000.0,
            max_health: 1000.0,
        });
        info.auto_attack.insert(CharacterType::CasterMinion, AutoAttackInfo::init(
            CharacterType::CasterMinion,
            1.0, // wind up duration
            3.0, // casting duration
            1.0, // wind down duration
            2.5, // fire time (after animation start)
            0.5, // projectile speed
            Vector3::new(0.12, 0.0, -0.12) // projectile offset
        )?);
        Ok(info)
    }
}

impl ComponentSystem for CasterMinionSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::CasterMinion
    }
    // fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
    //     Err(WorldError::InvalidCommandMapping)
    // }
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }
    fn update_character(&self, _: &World, _: &Vec<WorldCommand>, _: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }
    fn reduce_changes(&self, _: &CharacterID, _: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        Ok(changes.clone())
    }
}

pub fn create(world: &World, id: &CharacterID, position: Vector2<f32>) -> Result<Vec<Update>, WorldError> {
    let typ = CharacterType::CasterMinion;
    let id = *id;
    // start these two at base stats
    let mut base = *world.info.base.get(&typ)
    .ok_or(WorldError::MissingCharacterInfoComponent(typ, ComponentID::Base))?;
    base.position = Vector3::new(position.x, position.y, 0.0);
    Ok([
        ComponentUpdateData::Base(CharacterBaseUpdate::New(base)),
        ComponentUpdateData::Health(CharacterHealthUpdate::New(
            world.info.health.get(&typ)
                .ok_or(WorldError::MissingCharacterInfoComponent(typ, ComponentID::Health))?
                .health
        )),
        ComponentUpdateData::Movement(Movement {
            destination: None,
        }),
        ComponentUpdateData::AutoAttack(AutoAttackUpdate(AutoAttack::new())),
        ComponentUpdateData::CasterMinion,
        ComponentUpdateData::Status(StatusUpdate::New(idle_status()))
    ].into_iter()
    .map(|cud| Update::Comp(ComponentUpdate {
        cid: id,
        data: cud,
    }))
    .chain(Some(Update::World(WorldUpdate::NewCharacterID(id))).into_iter())
    .collect())
}

