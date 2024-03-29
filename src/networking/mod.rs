use serde::{Serialize, Deserialize};
use std::{fmt::Display, net::SocketAddr};

pub mod commands;
pub mod client;
pub mod server;
pub mod tcp_buffering;
pub mod config;
pub mod common;
pub mod example;

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Copy, Clone)]
pub enum Protocol {
    TCP, UDP
}

impl Display for Protocol {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(out, "{:?}", self)
    }
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
