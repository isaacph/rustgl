use serde::{Serialize, Deserialize};
use crate::model::{Subscription, commands::{GetCommandID, CommandID}};
use super::model::PlayerData;

#[derive(Serialize, Deserialize)]
pub struct ChatMessage(pub String);

#[derive(Serialize, Deserialize)]
pub struct PlayerDataPayload(pub PlayerData);

#[derive(Serialize, Deserialize)]
pub struct GetPlayerData;

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerLogIn {
    pub existing: bool,
    pub name: Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct PlayerLogOut;

#[derive(Serialize, Deserialize)]
pub enum PlayerSubCommand {
    ListSubs,
    AddSubs(Vec<Subscription>),
    DelSubs(Vec<Subscription>),
    SetSubs(Vec<Subscription>),
}

#[derive(Serialize, Deserialize)]
pub struct PlayerSubs(pub PlayerSubCommand);

impl GetCommandID for ChatMessage {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::ChatMessage
    }
}

impl GetCommandID for PlayerDataPayload {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::PlayerDataPayload
    }
}

impl GetCommandID for GetPlayerData {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::GetPlayerData
    }
}

impl GetCommandID for PlayerLogIn {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::PlayerLogIn
    }
}

impl GetCommandID for PlayerLogOut {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::PlayerLogOut
    }
}

impl GetCommandID for PlayerSubs {
    fn command_id(&self) -> crate::model::commands::CommandID {
        CommandID::PlayerSubs
    }
}