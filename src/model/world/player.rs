use std::{collections::{HashMap, HashSet}, fmt::Display, net::SocketAddr};
use serde::{Serialize, Deserialize};
use crate::model::Subscription;

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

impl Default for TeamIDGenerator {
    fn default() -> Self {
        Self::new()
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

impl Default for PlayerIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Player {
   pub id: PlayerID,
   pub name: String,
}


pub trait PlayerDataView {
    fn get_player(&self, id: &PlayerID) -> Option<&Player>;
    fn get_player_with_name(&self, name: &str) -> Option<&Player>;
    fn all_player_ids(&self) -> Box<[PlayerID]>;
}

struct PlayerMetadata {
    connection: Option<SocketAddr>,
    subscriptions: HashSet<Subscription>
}

pub struct PlayerManager {
    players: HashMap<PlayerID, Player>,
    id_gen: PlayerIDGenerator,
    player_metadata: HashMap<PlayerID, PlayerMetadata>,
    pub updates: Vec<PlayerManagerUpdate>
}

pub enum PlayerManagerUpdate {
    PlayerLogIn(PlayerID, SocketAddr),
    PlayerLogOut(PlayerID, SocketAddr)
}

impl PlayerManager {
    pub fn new() -> PlayerManager {
        PlayerManager {
            players: HashMap::new(),
            player_metadata: HashMap::new(),
            id_gen: PlayerIDGenerator::new(),
            updates: vec![]
        }
    }

    pub fn create_player(&mut self, con: Option<SocketAddr>, name: Option<String>) -> &mut Player {
        let id = self.id_gen.generate();
        let name = {
            let base_name = match name {
                None => format!("Player{}", id),
                Some(name) => name
            };
            if self.players.values().any(|player| player.name == base_name) {
                // search for a name
                let mut i = 1;
                let mut search = true;
                let mut name = String::from("");
                while search {
                    name = format!("{} ({})", base_name, i);
                    search = self.players.values().any(|player| player.name == name);
                    i += 1;
                    println!("searching name: {}", name);
                }
                name
            } else {
                base_name
            }
        };

        self.player_metadata.insert(id, PlayerMetadata {
            connection: None,
            subscriptions: HashSet::new()
        });

        self.players.insert(id, Player {
            id,
            name
        });
        self.map_existing_player(con.as_ref(), Some(&id));
        self.players.get_mut(&id).unwrap()
    }

    pub fn map_existing_player(&mut self, con_id: Option<&SocketAddr>, player_id: Option<&PlayerID>) -> Option<&mut Player> {
        match (con_id, player_id) {
            (Some(con_id), Some(player_id)) => {
                if let Some(prev_player_id) = self.get_connected_player(con_id) { // check if connection had previous player
                    if prev_player_id == *player_id {
                        return self.players.get_mut(player_id); // if same player, then do nothing
                    } else {
                        self.map_existing_player(None, Some(&prev_player_id)); // log out this previous player
                    }
                }
                match (self.players.get_mut(player_id), self.player_metadata.get_mut(player_id)) {
                    (Some(player), Some(metadata)) => {
                        if let Some(prev_connection) = metadata.connection { // check if player had previous connection
                            self.updates.push(PlayerManagerUpdate::PlayerLogOut(*player_id, prev_connection));
                        }
                        self.updates.push(PlayerManagerUpdate::PlayerLogIn(*player_id, *con_id));
                        metadata.connection = Some(*con_id);
                        Some(player)
                    },
                    _ => None
                }
            },
            (Some(con_id), None) => {
                self.player_metadata.iter_mut().for_each(|(player_id, metadata)| {
                    match metadata.connection {
                        Some(other_con) => if *con_id == other_con {
                            self.updates.push(PlayerManagerUpdate::PlayerLogOut(*player_id, *con_id));
                            metadata.connection = None;
                        },
                        None => ()
                    }
                });
                None
            },
            (None, Some(player_id)) => {
                if let Some(metadata) = self.player_metadata.get_mut(player_id) {
                    if let Some(con_id) = metadata.connection {
                        self.updates.push(PlayerManagerUpdate::PlayerLogOut(*player_id, con_id));
                        metadata.connection = None;
                        return self.players.get_mut(player_id)
                    }
                }
                None
            },
            (None, None) => None
        }
    }

    pub fn is_connected(&self, player_id: &PlayerID) -> Option<SocketAddr> {
        match self.player_metadata.get(player_id) {
            Some(metadata) => metadata.connection,
            None => None
        }
    }

    pub fn get_view(&self) -> PlayerData {
        PlayerData {
            players: self.players.clone()
        }
    }

    pub fn get_player_connection(&self, player_id: &PlayerID) -> Option<SocketAddr> {
        match self.player_metadata.get(player_id) {
            Some(metadata) => metadata.connection,
            None => None
        }
    }

    pub fn get_connected_player(&mut self, con_id: &SocketAddr) -> Option<PlayerID> {
        self.player_metadata.iter_mut()
            .fold(None, |found, (id, meta)| match (found, meta.connection) {
                (Some(con), _) => Some(con),
                (None, Some(con)) => if con == *con_id {
                    Some(*id)
                } else {
                    None
                },
                (None, None) => None,
            }
        )
    }

    pub fn get_player_mut(&mut self, id: &PlayerID) -> Option<&mut Player> {
        self.players.get_mut(id)
    }

    pub fn get_player_with_name_mut(&mut self, name: &str) -> Option<&mut Player> {
        self.players.values_mut().find(|player| player.name == name)
    }

    pub fn get_player_subscriptions_mut(&mut self, id: &PlayerID) -> Option<&mut HashSet<Subscription>> {
        if let Some(meta) = self.player_metadata.get_mut(id) {
            Some(&mut meta.subscriptions)
        } else {
            None
        }
    }

    pub fn get_player_subscriptions(&self, id: &PlayerID) -> Option<HashSet<Subscription>> {
        self.player_metadata.get(id).map(|meta| meta.subscriptions.clone())
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerDataView for PlayerManager {
    fn get_player(&self, id: &PlayerID) -> Option<&Player> {
        self.players.get(id)
    }
    fn get_player_with_name(&self, name: &str) -> Option<&Player> {
        self.players.values().find(|player| player.name == name)
    }
    fn all_player_ids(&self) -> Box<[PlayerID]> {
        self.players.keys().copied().collect()
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
        for player in self.players.values() {
            if player.name == name {
                return Some(player);
            }
        }
        None
    }

    fn all_player_ids(&self) -> Box<[PlayerID]> {
        self.players.keys().copied().collect()
    }
}
