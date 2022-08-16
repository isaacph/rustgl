use itertools::Itertools;
use nalgebra::{Vector2, Vector3};
use serde::{Deserialize, Serialize};

use crate::model::{world::{character::CharacterID, component::{Component, ComponentUpdateData, GetComponentID, ComponentID, ComponentStorageContainer, ComponentUpdate}, WorldError, WorldSystem, WorldInfo, ComponentSystem, commands::{CharacterCommand, WorldCommand, Priority}, World, CharacterCommandState, Update, WorldErrorI}, commands::GetCommandID};
use super::{ability::{Ability, AbilityInfo, AbilityCommand, AbilityUpdate}, status::StatusID, base::make_move_update};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FlashExecution {
    pub target_pos: Vector2<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Flash {
    pub ability: Ability<FlashExecution>,
}

#[derive(Clone)]
pub struct FlashInfo {
    pub ability: AbilityInfo,
    pub range: f32,
    pub duration: f32,
    pub cooldown: f32,
}

impl Default for Flash {
    fn default() -> Self {
        Self { ability: Ability::new(StatusID::Flash) }
    }
}

impl Component for Flash {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        match update.clone() {
            ComponentUpdateData::Flash(FlashUpdate(x)) => x,
            _ => self.clone()
        }
    }
}

impl GetComponentID for Flash {
    const ID: ComponentID = ComponentID::Flash;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FlashUpdate(pub Flash);

impl Default for FlashUpdate {
    fn default() -> Self {
        Self::new()
    }
}

impl FlashUpdate {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl FlashInfo {
    pub fn init(
        duration: f32,
        cooldown: f32,
        wind_up_time: f32,
        casting_time: f32,
        wind_down_time: f32,
        fire_time: f32,
        range: f32) -> Result<Self, WorldError> {
        Ok(Self {
            ability: AbilityInfo::new(wind_up_time, casting_time, wind_down_time, fire_time)?,
            range,
            duration,
            cooldown
        })
    }
}

pub struct FlashAbilitySystem;

impl WorldSystem for FlashAbilitySystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for FlashAbilitySystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Flash
    }

    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<CharacterCommandState, WorldError> {
        match cmd {
            CharacterCommand::Flash(cmd) => {
                let flash = world.flash.get_component(cid)?;
                flash.ability.validate_command(world, cid, &cmd.to_ability_command(world, cid)?)
            },
            _ => Err(WorldErrorI::InvalidCommandMapping.err())
        }
    }

    fn update_character(&self, world: &World, commands: &[WorldCommand], cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        let ctype = world.base.get_component(cid)?.ctype;
        let info = world.info.flash.get(&ctype).ok_or_else(|| WorldErrorI::MissingCharacterInfoComponent(ctype, ComponentID::Flash).err())?;
        let flash = world.flash.get_component(cid)?;
        let commands = commands.iter().filter_map(|cmd| match cmd {
            WorldCommand::CharacterComponent(
                ccid,
                ComponentID::Flash,
                CharacterCommand::Flash(cmd)) => {
                    if *ccid == *cid {
                        cmd.to_ability_command(world, cid).ok()
                    } else { None }
                },
            _ => None
        }).collect_vec();
        let (au, updates) = flash.ability.update(world, &info.ability, &commands, cid, delta_time, fire_flash)?;
        Ok(updates
           .into_iter()
           .chain(
               au.into_iter()
               .map(|update| Update::Comp(
                    ComponentUpdate {
                        cid: *cid,
                        data: ComponentUpdateData::Flash(FlashUpdate(Flash {
                            ability: update.0
                        }))
                    }
               )))
           .collect_vec())
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &[ComponentUpdateData]) -> Result<Vec<ComponentUpdateData>, WorldError> {
        if world.characters.get(cid).is_none() || world.flash.get_component(cid).is_err() {
            return Ok(changes.iter()
                .filter_map(|change| match change.clone() {
                    ComponentUpdateData::Flash(up) => Some(AbilityUpdate(up.0.ability)),
                    _ => None
                })
                .at_most_one()
                .map_err(|_| WorldErrorI::MultipleUpdateOverrides(*cid, ComponentID::Flash).err())?
                .into_iter()
            .map(|up| ComponentUpdateData::Flash(FlashUpdate(Flash {
                ability: up.0
            })))
            .into_iter()
            .collect_vec())
        }
        let flash = world.flash.get_component(cid)?;
        Ok(flash.ability.reduce(cid, world, &changes.iter()
            .filter_map(|change| match change.clone() {
                ComponentUpdateData::Flash(up) => Some(AbilityUpdate(up.0.ability)),
                _ => None
            })
            .at_most_one()
            .map_err(|_| WorldErrorI::MultipleUpdateOverrides(*cid, ComponentID::Flash).err())?
            .into_iter()
            .collect_vec())?
        .map(|up| ComponentUpdateData::Flash(FlashUpdate(Flash {
            ability: up.0
        })))
        .into_iter()
        .collect_vec())
    }
}

pub fn fire_flash(world: &World, cid: &CharacterID) -> Result<Vec<Update>, WorldError> {
    let base = world.base.get_component(cid)?;
    let info = world.info.flash.get(&base.ctype)
        .ok_or_else(|| WorldErrorI::MissingCharacterInfoComponent(base.ctype, ComponentID::Flash).err())?;
    let execution = world.flash.get_component(cid)?.ability.execution.as_ref()
        .ok_or_else(|| WorldErrorI::UnexpectedComponentState(
                *cid,
                ComponentID::Flash,
                "Component called fire while not executing".to_string()).err())?;
    let target_pos = execution.data.target_pos;
    let pos = Vector2::new(base.position.x, base.position.y);
    let mut dir = target_pos - pos;
    if dir.x == 0.0 && dir.y == 0.0 || dir.x.is_nan() || dir.y.is_nan() {
        println!("Warning: player flashed with bad direction");
        return Ok(vec![])
    }
    if dir.magnitude() > info.range {
        dir.normalize_mut();
        dir *= info.range;
    }
    let move_by = Vector3::new(dir.x, dir.y, 0.0);
    Ok(vec![make_move_update(*cid, Priority::Cast, move_by)])
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FlashCommand {
    pub target_pos: Vector2<f32>,
}

impl FlashCommand {
    pub fn to_ability_command(&self, world: &World, cid: &CharacterID) -> Result<AbilityCommand<FlashExecution>, WorldError> {
        let ctype = world.base.get_component(cid)?.ctype;
        let info = world.info.flash.get(&ctype).ok_or_else(|| WorldErrorI::MissingCharacterInfoComponent(ctype, ComponentID::Flash).err())?;
        Ok(AbilityCommand {
            duration: info.duration,
            cooldown: info.cooldown,
            exec_data: FlashExecution { target_pos: self.target_pos }
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FlashRequest {
    pub user: CharacterID,
    pub target_pos: Vector2<f32>,
}

impl GetCommandID for FlashRequest {
    fn command_id(&self) -> crate::model::commands::CommandID {
        crate::model::commands::CommandID::FlashRequest
    }
}

#[cfg(feature = "server")]
pub mod server {
    use std::net::SocketAddr;
    use crate::{model::{player::{server::PlayerCommand, model::{PlayerID, PlayerDataView}, commands::ChatMessage}, PrintError, world::{component::ComponentID, commands::{WorldCommand, CharacterCommand}}}, server::{commands::{ProtocolSpec, SendCommands}, main::Server}, networking::Protocol};
    use super::{FlashRequest, FlashCommand};

    impl<'a> PlayerCommand<'a> for FlashRequest {
        const PROTOCOL: ProtocolSpec = ProtocolSpec::One(Protocol::UDP);

        fn run(self, addr: &SocketAddr, player_id: &PlayerID, server: &mut Server) {
            // check if the player can use the requested character
            if server.player_manager.can_use_character(player_id, &self.user) {
                server.run_world_command(
                    Some(addr),
                    WorldCommand::CharacterComponent(
                        self.user,
                        ComponentID::Flash,
                        CharacterCommand::Flash(FlashCommand {
                            target_pos: self.target_pos,
                        })
                    )
                );
            } else {
                server.connection.send(
                    Protocol::TCP,
                    addr,
                    &ChatMessage("Error: no permission".to_string())
                ).print()
            }
        }
    }
}

