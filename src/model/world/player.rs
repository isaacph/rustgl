use crate::networking_wrapping::VecSerializedWrapperDecay;
use std::{collections::HashMap, fmt::Display};

use serde::{Serialize, Deserialize};

use crate::{networking::server::ConnectionID, networking_wrapping::{ServerCommand, SerializedClientCommand}, game::ChatMessage};

use super::{WorldCommand, World};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TeamID(i32);

#[derive(Serialize, Deserialize, Debug)]
pub struct TeamIDGenerator(i32);

impl TeamIDGenerator {
    pub fn new() -> Self {
        TeamIDGenerator(0)
    }
    pub fn generate(&mut self) -> TeamID {
        self.0 += 1;
        TeamID(self.0 - 1)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Team {
    id: TeamID,
    name: String
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct PlayerID(i32);

impl Display for PlayerID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerIDGenerator(i32);

impl PlayerIDGenerator {
    pub fn new() -> Self {
        PlayerIDGenerator(0)
    }
    pub fn generate(&mut self) -> PlayerID {
        self.0 += 1;
        PlayerID(self.0 - 1)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Player {
    pub id: PlayerID,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerDataPayload {
    data: PlayerData
}

impl PlayerDataPayload {
    pub fn from(data: PlayerData) -> PlayerDataPayload {
        PlayerDataPayload { data }
    }
}

impl<'a> WorldCommand<'a> for PlayerDataPayload {
    fn run(self, world: &mut World) {
        world.players = self.data
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerLogIn {
    pub existing: bool,
    pub name: Option<String>
}

impl<'a> ServerCommand<'a> for PlayerLogIn {
    fn run(self, (con_id, server): (&ConnectionID, &mut crate::server::Server)) {
        match {
            if self.existing {
                if let Some(name) = &self.name {
                    match if let Some(player) = server.player_manager.get_player_with_name(name) {
                        Ok(player.id)
                    } else {
                        Err(format!("Cannot sign in: player with name {} is not found", name))
                    } {
                        Ok(pid) => Ok(&server.player_manager.map_existing_player(Some(con_id), Some(&pid)).unwrap().name),
                        Err(x) => Err(x)
                    }
                } else {
                    Err(format!("Cannot sign into unnamed character"))
                }
            } else {
                let player = server.player_manager.create_player(Some(*con_id), self.name);
                Ok(&player.name)
            }
        } {
            Ok(player_name) => {
                server.connection.send_udp_all(
                    server.connection.all_connection_ids(),
                    vec![
                        SerializedClientCommand::from(
                            &ChatMessage::new(format!("{} logged in.", player_name))
                        ),
                        SerializedClientCommand::from(
                            &PlayerDataPayload::from(server.player_manager.get_view())
                        )
                    ].decay()
                );
                server.world.players = server.player_manager.get_view()
            },
            Err(e) => {
                server.connection.send_udp(
                    vec![*con_id],
                    SerializedClientCommand::from(
                        &ChatMessage::new(e)
                    ).data
                )
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerLogOut;

impl<'a> ServerCommand<'a> for PlayerLogOut {
    fn run(self, (con_id, server): (&ConnectionID, &mut crate::server::Server)) {
        if let Some(player) = server.player_manager.map_existing_player(Some(con_id), None) {
            server.connection.send_udp(
                server.connection.all_connection_ids(),
                SerializedClientCommand::from(
                    &ChatMessage::new(format!("{} logged out.", player.name))
                ).data
            );
        } else {
            server.connection.send_udp(
                vec![*con_id],
                SerializedClientCommand::from(
                    &ChatMessage::new(format!("Failed to log out, was not logged in"))
                ).data
            );
        }
    }
}

pub trait PlayerDataView {
    fn get_player(&self, id: &PlayerID) -> Option<&Player>;
    fn get_player_with_name(&self, name: &str) -> Option<&Player>;
}

pub struct PlayerManager {
    players: HashMap<PlayerID, Player>,
    id_gen: PlayerIDGenerator,
    connection_player: HashMap<ConnectionID, PlayerID>,
    player_connection: HashMap<PlayerID, ConnectionID>,
    name_player: HashMap<String, PlayerID>,
}

impl PlayerManager {
    pub fn new() -> PlayerManager {
        PlayerManager {
            players: HashMap::new(),
            id_gen: PlayerIDGenerator::new(),
            connection_player: HashMap::new(),
            player_connection: HashMap::new(),
            name_player: HashMap::new()
        }
    }

    pub fn create_player(&mut self, con_id: Option<ConnectionID>, name: Option<String>) -> &mut Player {
        let id = self.id_gen.generate();
        let name = {
            let base_name = match name {
                None => format!("Player{}", id),
                Some(name) => name
            };
            match self.name_player.contains_key(&base_name) {
                false => base_name,
                true => {
                    // search for a name
                    let mut i = 1;
                    let mut search = true;
                    let mut name = String::from("");
                    while search {
                        name = format!("{} ({})", base_name, i);
                        search = self.name_player.contains_key(&name);
                        i += 1;
                    }
                    name
                }
            }
        };
        if let Some(con_id) = con_id {
            self.connection_player.insert(con_id, id);
            self.player_connection.insert(id, con_id);
        };
        self.name_player.insert(name.clone(), id);
        self.players.insert(id, Player {
            id,
            name
        });
        self.players.get_mut(&id).unwrap()
    }

    pub fn map_existing_player(&mut self, con_id: Option<&ConnectionID>, player_id: Option<&PlayerID>) -> Option<&mut Player> {
        match (con_id, player_id) {
            (Some(con_id), Some(player_id)) => match self.players.get_mut(player_id) {
                Some(player) => {
                    self.connection_player.insert(*con_id, *player_id);
                    self.player_connection.insert(*player_id, *con_id);
                    Some(player)
                },
                None => None
            },
            (Some(con_id), None) => {
                if let Some(player_id) = self.connection_player.remove(con_id) {
                    self.player_connection.remove(&player_id);
                    self.players.get_mut(&player_id)
                } else { None }
            },
            (None, Some(player_id)) => {
                if let Some(con_id) = self.player_connection.remove(player_id) {
                    self.connection_player.remove(&con_id);
                    self.players.get_mut(&player_id)
                } else { None }
            },
            (None, None) => None
        }
    }

    pub fn ensure_player(&mut self, con_id: &ConnectionID) -> &mut Player {
        let id = match self.connection_player.get(con_id) {
            Some(id) => if self.players.contains_key(id) {
                Some(id.clone())
            } else { None },
            None => None
        };
        if let None = id {
            self.create_player(Some(*con_id), None);
        }
        self.players.get_mut(&id.unwrap()).unwrap()
    }

    pub fn get_view(&self) -> PlayerData {
        PlayerData {
            players: self.players.clone()
        }
    }

    pub fn get_player_connection(&self, player_id: &PlayerID) -> Option<ConnectionID> {
        match self.player_connection.get(player_id) {
            Some(id) => Some(*id),
            None => None
        }
    }

    pub fn get_connected_player_mut(&mut self, con_id: &ConnectionID) -> Option<&mut Player> {
        match self.connection_player.get(con_id) {
            Some(id) => self.players.get_mut(id),
            None => None
        }
    }

    pub fn get_connected_player(&self, con_id: &ConnectionID) -> Option<&Player> {
        match self.connection_player.get(con_id) {
            Some(id) => self.players.get(id),
            None => None
        }
    }

    pub fn get_player_mut(&mut self, id: &PlayerID) -> Option<&mut Player> {
        self.players.get_mut(id)
    }
}

impl PlayerDataView for PlayerManager {
    fn get_player(&self, id: &PlayerID) -> Option<&Player> {
        self.players.get(id)
    }
    fn get_player_with_name(&self, name: &str) -> Option<&Player> {
        match self.name_player.get(name) {
            Some(id) => self.players.get(id),
            None => None
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerData {
    pub players: HashMap<PlayerID, Player>
}

impl PlayerDataView for PlayerData {
    fn get_player(&self, id: &PlayerID) -> Option<&Player> {
        self.players.get(id)
    }
    fn get_player_with_name(&self, name: &str) -> Option<&Player> {
        for (_, player) in &self.players {
            if player.name == name {
                return Some(player);
            }
        }
        None
    }
}