
use std::collections::HashMap;

use self::{player::{TeamID, PlayerID, Team, Player}, character::{CharacterID, ContainableInWorld, Character}};

pub mod player;
pub mod character;

pub struct World {
    pub teams: HashMap<TeamID, Team>,
    pub players: HashMap<PlayerID, Player>,
    characters: HashMap<CharacterID, Box<dyn Character>>,
}

impl World {
    pub fn new() -> World {
        World {
            teams: HashMap::new(),
            players: HashMap::new(),
            characters: HashMap::new()
        }
    }

    fn get_character<'a>(&mut self, id: CharacterID) -> Option<&mut Box<dyn Character>> {
        match self.characters.get_mut(&id) {
            None => None,
            Some(x) => {
                let inner = x.as_any_mut();
                let cast: Option<&mut Box<dyn Character>> = inner.downcast_mut();
                cast
            }
        }
    }

    fn put_character<'a>(&mut self, id: CharacterID, ch: Box<dyn Character>)
            -> Option<Box<dyn Character>> {
        self.characters.insert(id, ch)
    }
}