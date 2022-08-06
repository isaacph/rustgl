use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy, PartialOrd, Ord)]
pub struct CharacterID(u64);

impl CharacterID {
    pub fn get_num(&self) -> u64 {
        self.0
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CharacterIDGenerator(u64);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CharacterIDRange(u64, u64);

impl CharacterIDRange {
    pub fn split_id(&self) -> (Option<CharacterID>, Self) {
        let mut split = self.clone();
        (self.next_id(), split)
    }
    pub fn next_id(&mut self) -> Option<CharacterID> {
        if self.0 < self.1 {
            let id = self.0;
            self.0 += 1;
            Some(CharacterID(id))
        } else {
            None
        }
    }
    pub fn take_range(&mut self, max: u64) -> CharacterIDRange {
        let mid = std::cmp::min(self.1, self.0 + max);
        let old_0 = self.0;
        *self = CharacterIDRange(mid, self.1);
        CharacterIDRange(old_0, mid)
    }
    pub fn is_empty(&self) -> bool {
        self.0 >= self.1
    }
}

impl CharacterIDGenerator {
    pub fn new() -> Self {
        CharacterIDGenerator(0)
    }
    pub fn generate(&mut self) -> CharacterID {
        self.0 += 1;
        CharacterID(self.0 - 1)
    }
    pub fn generate_range(&mut self, count: u64) -> CharacterIDRange {
        let start = self.0;
        self.0 += count;
        CharacterIDRange(start, self.0)
    }
}

impl Default for CharacterIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum CharacterType {
    IceWiz,
    CasterMinion,
    Projectile,
}

