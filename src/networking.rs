use serde::{Serialize, Deserialize};
use crate::SocketAddr;
use std::fmt::Display;

pub mod commands;
pub mod client;
pub mod server;
pub mod tcp_buffering;
pub mod config;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub enum Protocol {
    TCP, UDP
}

#[derive(Debug, Copy, Clone)]
pub struct AddressPair {
    pub tcp: SocketAddr,
    pub udp: SocketAddr
}

impl Display for AddressPair {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(out, "{{udp: {}, tcp: {}}}", self.udp, self.tcp)
    }
}
