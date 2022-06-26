use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use crate::networking::Protocol;
use crate::{commands_execute, _commands_execute_static_def};
use crate::model::commands::ClientCommandID;
use super::main::Server;

pub mod core;
pub mod player;

commands_execute!(
    execute_server_command,
    ServerCommand,
    ServerCommandID,
    ((Protocol, &SocketAddr), &mut Server),
    // list all commands that the server can execute here here:
    [
        crate::model::commands::core::GetAddress,
        crate::model::commands::core::SetUDPAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::commands::player::ChatMessage,
        crate::model::commands::player::PlayerLogIn,
        crate::model::commands::player::PlayerLogOut
    ]
);

pub trait SendCommands {
    fn send<T: ClientCommandID>(&mut self, protocol: Protocol, tcp_addr: &SocketAddr, command: &T) -> std::result::Result<(), String>;
    fn send_udp_to_unidentified<T: ClientCommandID>(&mut self, udp_addr: &SocketAddr, command: &T) -> std::io::Result<usize>;
}

impl SendCommands for crate::networking::server::Server {
    fn send<T: ClientCommandID>(&mut self, protocol: Protocol, tcp_addr: &SocketAddr, command: &T) -> std::result::Result<(), String> {
        self.send_data(protocol, tcp_addr, command.make_bytes())
    }
    fn send_udp_to_unidentified<T: ClientCommandID>(&mut self, udp_addr: &SocketAddr, command: &T) -> std::io::Result<usize> {
        self.send_udp_data_to_unidentified(udp_addr, &command.make_bytes())
    }
}

// list how the server will respond to each command below
pub enum ProtocolSpec {
    One(Protocol),
    Both
}
pub trait ProtocolServerCommand<'a>: Deserialize<'a> + Serialize {
    const PROTOCOL: ProtocolSpec;
    fn run(self, tcp_addr: &SocketAddr, server: &mut Server);
}

impl<'a, T> ServerCommand<'a> for T where T: ProtocolServerCommand<'a> {
    fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        if match T::PROTOCOL {
            ProtocolSpec::One(p2) => p2 == protocol,
            ProtocolSpec::Both => true
        } {
            let addr = match protocol {
                Protocol::TCP => *addr,
                Protocol::UDP => {
                    match server.connection.get_tcp_address(addr) {
                        Some(addr) => addr,
                        None => {
                            println!("Error: UDP server command called by client without UDP address set");
                            return
                        }
                    }
                }
            };
            self.run(&addr, server);
        }
    }
}