use serde::{Serialize, Deserialize};


pub mod commands;
pub mod client;
pub mod server;
pub mod tcp_buffering;
pub mod config;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub enum Protocol {
    TCP, UDP
}