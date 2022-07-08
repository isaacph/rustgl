use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use crate::{commands_execute, _commands_execute_static_def};
use crate::model::commands::ClientCommandID;
use crate::{model::commands::core::{GetAddress, SendAddress, SetUDPAddress, EchoMessage}, networking::Protocol, server::main::Server};

//pub mod core;
//pub mod player;
//pub mod world;

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
        crate::model::player::commands::ChatMessage,
        crate::model::player::commands::PlayerLogIn,
        crate::model::player::commands::PlayerLogOut,
        crate::model::player::commands::GetPlayerData,
        crate::model::player::commands::PlayerSubs,
        crate::model::world::commands::GenerateCharacter,
        crate::model::world::commands::UpdateCharacter,
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

pub enum ProtocolSpec {
    One(Protocol),
    Both
}
pub trait ProtocolServerCommand<'a>: Deserialize<'a> + Serialize {
    const PROTOCOL: ProtocolSpec;
    fn run(self, protocol: Protocol, tcp_addr: &SocketAddr, server: &mut Server);
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
            self.run(protocol, &addr, server);
        }
    }
}

// list how the server will respond to each command below
impl<'a> ServerCommand<'a> for GetAddress {
    fn run(self, ((_, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        match server.connection.send_udp_to_unidentified(addr, &SendAddress(addr.to_string())) {
            Ok(size) => println!("Sent UDP {} bytes", size),
            Err(err) => println!("Error UDP sending: {}", err)
        };
    }
}

impl<'a> ServerCommand<'a> for SetUDPAddress {
    fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        match (protocol, self.0.parse()) {
            (Protocol::TCP, Ok(udp_addr)) => {
                match server.connection.set_client_udp_addr(addr, &udp_addr) {
                    Ok(()) => println!("Set UDP address for client at TCP address {}: {}", addr, udp_addr),
                    Err(err) => println!("Invalid SetUDPAddress command from {}: {}", addr, err)
                }
            },
            _ => println!("Invalid SetUDPAddress command from {}", addr)
        }
    }
}

impl<'a> ServerCommand<'a> for EchoMessage {
    fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        println!("Running echo");
        match protocol {
            Protocol::TCP => 
            match server.connection.send(protocol, addr, &self) {
                Ok(()) => (),
                Err(err) => println!("Error echoing TCP to {}: {}", addr, err)
            },
            Protocol::UDP => {
                let udp_addr = *addr;
                match server.connection.get_tcp_address(&udp_addr) {
                    Some(tcp_addr) => {
                        match server.connection.send(protocol, &tcp_addr, &self) {
                            Ok(()) => (),
                            Err(err) => println!("Error echoing UDP to client with TCP address {}: {}", udp_addr, err)
                        }
                    },
                    None => println!("No client has UDP address {}", addr)
                }
            }
        }
    }
}
