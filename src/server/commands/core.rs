use std::net::SocketAddr;
use crate::{model::commands::core::{GetAddress, SendAddress, SetUDPAddress, EchoMessage}, networking::Protocol, server::main::Server};
use super::{ServerCommand, SendCommands};

// these commands are special
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