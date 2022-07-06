use std::fmt::Display;

use serde::{Serialize, Deserialize};
use strum_macros::EnumString;

pub mod world;
pub mod commands;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash, EnumString)]
pub enum Subscription {
    Chat,
    World,
}

impl Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", *self)
    }
}

pub trait PrintError {
    fn print(&self);
}

impl PrintError for std::result::Result<(), String> {
    fn print(&self) {
        match self {
            Ok(_) => (),
            Err(e) => println!("Error: {}", e),
        }
    }
}
