use nalgebra::{Vector3, Vector2};
use serde::{Serialize, Deserialize};
use crate::model::world::{World, character::{CharacterID, CharacterType}, component::{GetComponentID, ComponentID, ComponentUpdateData, Component, ComponentUpdate}, WorldError, WorldInfo, WorldSystem, commands::{CharacterCommand, WorldCommand}, ComponentSystem, Update, WorldUpdate, system::{status::{StatusUpdate, idle_status}, flash::FlashUpdate}, CharacterCommandState, WorldErrorI};
use super::{movement::Movement, auto_attack::{AutoAttack, AutoAttackInfo, AutoAttackUpdate}, base::{CharacterBase, CharacterFlip, CharacterBaseUpdate}, health::{CharacterHealth, CharacterHealthUpdate}, flash::FlashInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IceWiz {
}

impl Component for IceWiz {
    fn update(&self, _update: &ComponentUpdateData) -> Self {
        self.clone()
    }
}

impl GetComponentID for IceWiz {
    const ID: ComponentID = ComponentID::IceWiz;
}

pub fn create(world: &World, id: &CharacterID, position: Vector2<f32>) -> Result<Vec<Update>, WorldError> {
    let typ = CharacterType::IceWiz;
    let id = *id;
    // start these two at base stats
    let mut base = *world.info.base.get(&typ)
    .ok_or_else(|| WorldErrorI::MissingCharacterInfoComponent(typ, ComponentID::Base).err())?;
    base.position = Vector3::new(position.x, position.y, 0.0);
    println!("Running create for cid {:?}", id);
    Ok([
        ComponentUpdateData::Base(CharacterBaseUpdate::New(base)),
        ComponentUpdateData::Health(CharacterHealthUpdate::New(
            world.info.health.get(&typ)
                .ok_or_else(|| WorldErrorI::MissingCharacterInfoComponent(typ, ComponentID::Health).err())?
                .health
        )),
        ComponentUpdateData::Movement(Movement {
            destination: None,
        }),
        ComponentUpdateData::AutoAttack(AutoAttackUpdate(AutoAttack::new())),
        ComponentUpdateData::IceWiz,
        ComponentUpdateData::Status(StatusUpdate::New(idle_status())),
        ComponentUpdateData::Flash(FlashUpdate::new())
    ].into_iter()
    .map(|cud| Update::Comp(ComponentUpdate {
        cid: id,
        data: cud,
    }))
    .chain(Some(Update::World(WorldUpdate::NewCharacterID(id))).into_iter())
    .collect())
}

pub struct IceWizSystem;

impl WorldSystem for IceWizSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        let mut info = WorldInfo::new();
        info.base.insert(CharacterType::IceWiz, CharacterBase {
            ctype: CharacterType::IceWiz,
            position: Vector3::new(0.0, 0.0, 0.0),
            center_offset: Vector3::new(0.0, 0.0, -0.4),
            speed: 1.0,
            attack_damage: 10.0,
            range: 1.0,
            attack_speed: 2.0,
            flip: CharacterFlip::Right,
            targetable: true,
        });
        info.health.insert(CharacterType::IceWiz, CharacterHealth {
            health: 100.0,
            max_health: 100.0,
        });
        info.auto_attack.insert(CharacterType::IceWiz, AutoAttackInfo::init(
            CharacterType::IceWiz,
            1.0, // wind up duration
            2.0, // casting duration
            1.0, // wind down duration
            3.0, // fire time (after animation start)
            1.2, // projectile speed
            Vector3::new(0.2, 0.0, -0.35) // projectile offset
        )?);
        info.flash.insert(CharacterType::IceWiz, FlashInfo::init(
            0.5, // duration
            0.0, // cooldown
            1.0, // wind up duration
            2.0, // casting duration
            1.0, // wind down duration
            3.0, // fire time (after animation start)
            2.0 // range
        )?);
        Ok(info)
    }
}

impl ComponentSystem for IceWizSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::IceWiz
    }
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        Err(WorldErrorI::InvalidCommandMapping.err())
    }
    // fn run_character_command(&self, _: &mut World, _: &CharacterID, _: CharacterCommand) -> Result<(), WorldError> {
    //     Err(WorldError::InvalidCommandMapping)
    // }
    fn update_character(&self, _: &World, _: &[WorldCommand], _: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }
    fn reduce_changes(&self, _: &CharacterID, _: &World, changes: &[ComponentUpdateData]) -> Result<Vec<ComponentUpdateData>, WorldError> {
        Ok(changes.to_vec())
    }
}

