use serde::{Serialize, Deserialize};
use crate::model::Subscription;
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
