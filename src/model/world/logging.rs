use std::{fs::{File, OpenOptions}, io::{self, Write}};

use super::{World, WorldError, WorldErrorI};


pub struct Logger {
    file: File
}

impl Logger {
    pub fn init(file_name: &str) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).write(true).open(file_name).unwrap();
        Ok(Self {
            file
        })
    }

    pub fn log(&mut self, world: &World) {
        for error in &world.errors {
            let data = match error {
                WorldError(WorldErrorI::Info(s)) => format!("Tick {}, {:?}", world.tick, s),
                _ => format!("Tick {}, Error: {:?}", world.tick, error),
            };
            writeln!(self.file, "{}", data).map_err(|_| println!("Error writing to log")).ok();
        }
        self.file.flush().map_err(|_| println!("Error flushing log file")).ok();
    }
}
