use serde::{Serialize, Deserialize};

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

#[derive(Serialize, Deserialize, Debug)]
pub struct Player {
    id: PlayerID,
    name: String,

}