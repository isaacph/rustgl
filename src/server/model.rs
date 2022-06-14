use std::net::SocketAddr;

use serde::{Serialize, Deserialize};

use crate::networking::Protocol;
use crate::{commands_execute, _commands_execute_static_def};
use crate::model::{GetAddress, SetUDPAddress, EchoMessage, SerializedServerCommand, SerializedClientCommand, SendAddress};
use crate::networking::server::Server;

commands_execute!(
    execute_server_command,
    ServerCommand,
    ServerCommandID,
    SerializedServerCommand,
    ((Protocol, &SocketAddr), &mut Server),
    // list all commands that the server can execute here here:
    [
        GetAddress,
        SetUDPAddress,
        EchoMessage
    ]
);

// list how the server will respond to each command below

impl<'a> ServerCommand<'a> for GetAddress {
    fn run(self, ((_, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        let packet: SerializedClientCommand = (&SendAddress(addr.to_string())).into();
        match server.udp.send_to(packet.0.as_slice(), addr) {
            Ok(size) => println!("Sent UDP {} bytes", size),
            Err(err) => println!("Error UDP sending: {}", err)
        };
    }
}

impl<'a> ServerCommand<'a> for SetUDPAddress {
    fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        match (protocol, self.0.parse(), server.connections.get_mut(addr)) {
            (Protocol::TCP, Ok(udp_addr), Some(info)) => {
                info.udp_address = Some(udp_addr);
                server.corresponding_tcp_to_udp.insert(udp_addr, *addr);
                println!("Set UDP address for client at TCP address {}: {}", addr, udp_addr);
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
            match server.send_tcp(addr, self.into()) {
                Ok(()) => (),
                Err(err) => println!("Error echoing TCP to {}: {}", addr, err)
            },
            Protocol::UDP => {
                let udp_addr = *addr;
                match server.corresponding_tcp_to_udp.get(&udp_addr) {
                    Some(tcp_addr) => {
                        let tcp_addr = *tcp_addr;
                        match server.send_udp(&tcp_addr, (&self).into()) {
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
