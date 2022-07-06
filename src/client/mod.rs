use crate::{model::PrintError, networking::client::ClientError};

pub mod game;
pub mod chatbox;
pub mod commands;

impl PrintError for std::result::Result<(), ClientError> {
    fn print(&self) {
        match self {
            Ok(()) => (),
            Err(err) => println!("Error: {:?}", err)
        }
    }
}
