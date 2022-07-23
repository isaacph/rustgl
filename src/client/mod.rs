use crate::{model::PrintError, networking::client::ClientError};

pub mod commands;
pub mod game;
pub mod chatbox;
pub mod camera;
pub mod render;

impl PrintError for std::result::Result<(), ClientError> {
    fn print(&self) {
        match self {
            Ok(()) => (),
            Err(err) => println!("Error: {:?}", err)
        }
    }
}
