use std::net::SocketAddr;

use serde::{Serialize, Deserialize};

use crate::{commands_execute, commands_id, _commands_execute_static_def, _commands_id_static_def, networking::{client::Client, server::Server}};

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub enum Protocol {
    TCP, UDP
}

#[derive(Serialize, Deserialize)]
pub struct GetAddress;

#[derive(Serialize, Deserialize)]
pub struct SendAddress(pub String);

#[derive(Serialize, Deserialize)]
pub struct SetUDPAddress(pub String);

impl<'a> ServerCommand<'a> for GetAddress {
    fn run(self, ((_, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        let packet: SerializedClientCommand = (&SendAddress(addr.to_string())).into();
        match server.udp.send_to(packet.data.as_slice(), addr) {
            Ok(size) => println!("Sent UDP {} bytes", size),
            Err(err) => println!("Error UDP sending: {}", err)
        };
    }
}

impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, (_, client): (Protocol, &mut Client)) {
        //println!("Server sent their view of client's address: {}", self.0);
        let packet: SerializedServerCommand = (&SetUDPAddress(self.0)).into();
        client.send_tcp(packet);
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

#[derive(Serialize, Deserialize)]
pub struct EchoMessage(pub String);

impl<'a> ServerCommand<'a> for EchoMessage {
    fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
        match protocol {
            Protocol::TCP => 
            match server.send_tcp(addr, SerializedClientCommand::from(&self)) {
                Ok(()) => (),
                Err(err) => println!("Error echoing TCP to {}: {}", addr, err)
            },
            Protocol::UDP => {
                let udp_addr = addr.clone();
                match server.corresponding_tcp_to_udp.get(&udp_addr) {
                    Some(tcp_addr) => {
                        let tcp_addr = tcp_addr.clone();
                        match server.send_udp(&tcp_addr, SerializedClientCommand::from(&self)) {
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

impl <'a> ClientCommand<'a> for EchoMessage {
    fn run(self, _context: (Protocol, &mut Client)) {
        println!("Echoed message: {}", self.0);
    }
}

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    SerializedClientCommand,
    (Protocol, &mut Client),
    // list commands here:
    [
        SendAddress,
        EchoMessage
    ]
);

// for real server-only that doesn't need to execute client commands
// commands_id!(
//     ClientCommandID,
//     SerializedClientCommand,
//     [
//         SendAddress
//     ]
// );

commands_execute!(
    execute_server_command,
    ServerCommand,
    ServerCommandID,
    SerializedServerCommand,
    ((Protocol, &SocketAddr), &mut Server),
    // list commands here:
    [
        GetAddress,
        SetUDPAddress,
        EchoMessage
    ]
);