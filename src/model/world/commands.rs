use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use crate::model::{commands::GetCommandID, WorldTick};

use super::{World, character::{CharacterID, CharacterType}, component::ComponentID, system::{movement::MoveCharacter, auto_attack::AutoAttackCommand}, WorldError};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WorldCommand {
    CharacterComponent(CharacterID, ComponentID, CharacterCommand), // these are for user input
    World(GlobalCommand),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GlobalCommand {
    Clear,
    CreateCharacter(CharacterID, CharacterType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterCommand {
    Movement(MoveCharacter),
    AutoAttack(AutoAttackCommand),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateCharacter {
    pub id: CharacterID,
    pub components: HashMap<ComponentID, Vec<u8>>
}

impl UpdateCharacter {
    pub fn update_character(mut self, world: &mut World) -> Result<(), WorldError> {
        // let x: Vec<ComponentID> = self.components.keys().copied().collect();
        // println!("Updating character, {:?}", x);
        world.characters.insert(self.id);
        for (cid, data) in self.components.drain() {
            world.update_component(&self.id, &cid, data);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FixWorld {
    pub update: UpdateCharacter,
    pub ordering: u32,
    pub tick: WorldTick,
}

impl GetCommandID for FixWorld {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::FixWorld
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub enum Priority {
    Walk,
    Cast,
    Stun,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GenerateCharacter(pub CharacterType);

impl GenerateCharacter {
    pub fn new(typ: CharacterType) -> Self {
        GenerateCharacter(typ)
    }
    // pub fn generate_character(world: &mut World, idgen: &mut CharacterIDGenerator, typ: CharacterType) -> Result<CharacterID {
    //     match world.create_character(idgen, typ) {
    //         Ok(id) => id,
    //         Err(err) => 
    //     }
    //     let id = idgen.generate();
    //     world.characters.insert(id);
    //     world.base.components.insert(id, CharacterBase {
    //         ctype: character::CharacterType::IceWiz,
    //         position: Vector2::new(0.0, 0.0),
    //         speed: 1.0
    //     });
    //     world.health.components.insert(id, CharacterHealth {
    //         health: 100.0
    //     });
    //     world.movement.components.insert(id, Movement {
    //         destination: None
    //     });
    //     id
    // }
}

impl GetCommandID for GenerateCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::GenerateCharacter
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListChar;

impl GetCommandID for ListChar {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::ListChar
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EnsureCharacter;

impl GetCommandID for EnsureCharacter {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::EnsureCharacter
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClearWorld;

impl GetCommandID for ClearWorld {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::ClearWorld
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunWorldCommand {
    pub command: WorldCommand,
    pub tick: i32,
    pub ordering: u32,
}

impl GetCommandID for RunWorldCommand {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::RunWorldCommand
    }
}
