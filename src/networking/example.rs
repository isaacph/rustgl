use std::{net::{SocketAddr, UdpSocket, TcpStream, TcpListener, Shutdown}, io::{ErrorKind, self, Read, Write}, sync::mpsc::{TryRecvError, Receiver, self}, time::Duration, thread, collections::HashMap};
use std::io::Result;

use super::common::udp_recv_all;

pub fn console_client_udp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    // let send_to: SocketAddr = format!("127.0.0.1:{}", {
    //     let mut buffer = String::new();
    //     io::stdin().read_line(&mut buffer).unwrap();
    //     buffer.truncate(buffer.len() - 2);
    //     buffer
    // }).parse().unwrap();
    let udp = UdpSocket::bind("0.0.0.0:0")?;
    udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    loop {
        let (recv, err) = udp_recv_all(&udp, buffer.as_mut(), None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received from {:?}: {}", addr, std::str::from_utf8(packet.as_ref()).unwrap());
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error receiving: {}", err)
            },
            None => ()
        }
        let message = match stdin_channel.try_recv() {
            Ok(v) => v,
            Err(TryRecvError::Empty) => {
                std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
                continue
            },
            Err(TryRecvError::Disconnected) => break,
        };
        match udp.send_to(message.as_bytes(), &addresses.0) {
            Ok(sent) => println!("Sent {} bytes", sent),
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error sending: {}", err)
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    Ok(())
}

pub fn console_stream() -> Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer.trim_end().into()).unwrap();
    });
    rx
}

pub fn echo_server_udp(ports: (u16, u16)) -> Result<()> {
    let udp = UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?;
    udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    let mut run = true;
    while run {
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        let (recv, err) = udp_recv_all(&udp, &mut buffer, None);
        for (addr, packets) in recv {
            for packet in packets {
                let str = String::from_utf8_lossy(packet.as_ref());
                println!("Recv from {:?}: {}", addr, str);
                match udp.send_to(packet.as_ref(), addr) {
                    Ok(size) => println!("Sent {} bytes", size),
                    Err(err) => println!("Error sending: {}", err)
                }
                if str.as_ref() == "false" {
                    run = false;
                }
            }
        }
        if let Some(err) = err {
            match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error receiving: {}", err)
            }
        }
    }
    Ok(())
}
// struct ConnectionInfo {
//     stream: TcpStream,
//     write_buffer: Vec<u8>
// }
// loop {
//     match tcp.accept() {
//         Ok((stream, addr)) => {
//             println!("New connection from {}", addr);
//             connections.insert(addr, ConnectionInfo {
//                 stream,
//                 write_buffer: vec![]
//             });
//         },
//         Err(err) => match err.kind() {
//             std::io::ErrorKind::WouldBlock => break,
//             _ => {
//                 println!("Error with TCP accept: {}", err);
//                 break;
//             }
//         }
//     }
// }

pub fn console_client_tcp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    let addr = addresses.1;
    let mut stream = TcpStream::connect(addr)?;
    println!("Connected on {}", stream.local_addr()?);
    stream.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    let mut write_buffer = vec![];
    loop {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }
        match stream.read(buffer.as_mut()) {
            Ok(size) => match size {
                0 => break,
                _ => {
                    let data = &buffer[0..size];
                    let str = String::from_utf8_lossy(data);
                    println!("Received from {}: {}", addr, str);
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => {
                    println!("Error receiving from {}: {}", addr, err);
                }
            }
        }
        match stream.write(write_buffer.as_mut_slice()) {
            Ok(sent) => {
                write_buffer.drain(0..sent);
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error writing to {}: {}", addr, err)
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}

pub fn echo_server_tcp(ports: (u16, u16)) -> Result<()> {
    let tcp = TcpListener::bind(format!("0.0.0.0:{}", ports.1))?;
    tcp.set_nonblocking(true)?;
    struct ConnectionInfo {
        stream: TcpStream,
        write_buffer: Vec<u8>,
    }
    let mut connections: HashMap<SocketAddr, ConnectionInfo> = HashMap::new();
    let mut buffer = Box::new([0u8; 1024]);
    loop {
        loop {
            match tcp.accept() {
                Ok((stream, addr)) => {
                    println!("New connection from {}", addr);
                    connections.insert(addr, ConnectionInfo {
                        stream,
                        write_buffer: vec![]
                    });
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => break,
                    _ => {
                        println!("Error with TCP accept: {}", err);
                        break;
                    }
                }
            }
        }
        let mut disconnects = vec![];
        for (addr, info) in &mut connections {
            match info.stream.read(buffer.as_mut()) {
                Ok(size) => match size {
                    0 => disconnects.push(*addr),
                    _ => {
                        let data = &buffer[0..size];
                        let str = String::from_utf8_lossy(data);
                        println!("Received from {}: {}", addr, str);
                        info.write_buffer.extend(data);
                    }
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => {
                        println!("Error receiving from {}: {}", addr, err);
                        disconnects.push(*addr);
                        continue;
                    }
                }
            }
            if info.write_buffer.len() > 0 {
                match info.stream.write(info.write_buffer.as_mut_slice()) {
                    Ok(sent) => match sent {
                        0 => disconnects.push(*addr),
                        _ => {
                            info.write_buffer.drain(0..sent);
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        _ => {
                            println!("Error writing to {}: {}", addr, err);
                            disconnects.push(*addr);
                            continue;
                        }
                    }
                }
            }
        }
        for addr in disconnects {
            if let Some(info) = connections.remove(&addr) {
                if let Err(err) = info.stream.shutdown(Shutdown::Both) {
                    println!("Error disconnecting from {}: {}", addr, err);
                } else {
                    println!("Disconnected from {}", addr);
                }
            }
        }
    }
}

pub mod both {
    use std::{net::SocketAddr, sync::mpsc::TryRecvError, time::Duration};

    use serde::{Serialize, Deserialize};
    use crate::{commands_id, _commands_id_static_def, networking::{client::{Client, ClientError, ClientUpdate}, example::{console_stream, both::{server::execute_server_command, client::{execute_client_command, SendCommands}}}, server::{Server, ServerUpdate}, Protocol}};
    

    // define all client and server command data structures
    // they must all be listed in the macro below to auto generate an ID for them to be serialized
    // this enables commands to be serialized on both the client and the server
    // to execute a command, the client or server side must add the command name to
    // the commands_execute macro list, and the method of execution must be specified by
    // implementing the ClientCommand or ServerCommand traits

    #[derive(Serialize, Deserialize)]
    struct GetAddress;

    #[derive(Serialize, Deserialize)]
    struct SendAddress(pub String);

    #[derive(Serialize, Deserialize)]
    struct SetUDPAddress(pub String);

    #[derive(Serialize, Deserialize)]
    struct EchoMessage(pub String);

    commands_id!(
        ClientCommandID,
        [
            SendAddress,
            EchoMessage
        ]
    );

    commands_id!(
        ServerCommandID,
        [
            GetAddress,
            SetUDPAddress,
            EchoMessage
        ]
    );

    mod server {
        use serde::{Deserialize, Serialize};

        use crate::networking::Protocol;
        use crate::networking::server::Server;
        use crate::{commands_execute, _commands_execute_static_def};
        use std::net::SocketAddr;

        use super::ClientCommandID;

        commands_execute!(
            execute_server_command,
            ServerCommand,
            ServerCommandID,
            ((Protocol, &SocketAddr), &mut Server),
            // list all commands that the server can execute here here:
            [
                super::GetAddress,
                super::SetUDPAddress,
                super::EchoMessage
            ]
        );

        // list how the server will respond to each command below
        pub trait SendCommands {
            fn send<T: ClientCommandID>(&mut self, protocol: Protocol, tcp_addr: &SocketAddr, command: &T) -> std::result::Result<(), String>;
            fn send_udp_to_unidentified<T: ClientCommandID>(&mut self, udp_addr: &SocketAddr, command: &T) -> std::io::Result<usize>;
        }

        impl SendCommands for Server {
            fn send<T: ClientCommandID>(&mut self, protocol: Protocol, tcp_addr: &SocketAddr, command: &T) -> std::result::Result<(), String> {
                self.send_data(protocol, tcp_addr, command.make_bytes())
            }
            fn send_udp_to_unidentified<T: ClientCommandID>(&mut self, udp_addr: &SocketAddr, command: &T) -> std::io::Result<usize> {
                self.send_udp_data_to_unidentified(udp_addr, &command.make_bytes())
            }
        }

        pub trait ProtocolServerCommand<'a>: Deserialize<'a> + Serialize {
            const PROTOCOL: Box<[Protocol]>;
            fn run(self, addr: &SocketAddr, server: &mut Server);
        }

        impl<'a, T> ServerCommand<'a> for T where T: ProtocolServerCommand<'a> {
            fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
                if T::PROTOCOL.contains(&protocol) {
                    let addr = match protocol {
                        Protocol::TCP => *addr,
                        Protocol::UDP => {
                            match server.get_tcp_address(addr) {
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

        // these commands are special
        impl<'a> ServerCommand<'a> for super::GetAddress {
            fn run(self, ((_, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
                match server.send_udp_to_unidentified(addr, &super::SendAddress(addr.to_string())) {
                    Ok(size) => println!("Sent UDP {} bytes", size),
                    Err(err) => println!("Error UDP sending: {}", err)
                };
            }
        }

        impl<'a> ServerCommand<'a> for super::SetUDPAddress {
            fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
                match (protocol, self.0.parse()) {
                    (Protocol::TCP, Ok(udp_addr)) => {
                        match server.set_client_udp_addr(addr, &udp_addr) {
                            Ok(()) => println!("Set UDP address for client at TCP address {}: {}", addr, udp_addr),
                            Err(err) => println!("Invalid SetUDPAddress command from {}: {}", addr, err)
                        }
                    },
                    _ => println!("Invalid SetUDPAddress command from {}", addr)
                }
            }
        }

        impl<'a> ServerCommand<'a> for super::EchoMessage {
            fn run(self, ((protocol, addr), server): ((Protocol, &SocketAddr), &mut Server)) {
                println!("Running echo");
                match protocol {
                    Protocol::TCP => 
                    match server.send(protocol, addr, &self) {
                        Ok(()) => (),
                        Err(err) => println!("Error echoing TCP to {}: {}", addr, err)
                    },
                    Protocol::UDP => {
                        let udp_addr = *addr;
                        match server.get_tcp_address(&udp_addr) {
                            Some(tcp_addr) => {
                                let tcp_addr = tcp_addr;
                                match server.send(protocol, &tcp_addr, &self) {
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
    }

    pub fn echo_server_both(ports: (u16, u16)) -> std::io::Result<()> {
        let mut server = Server::init(ports)?;
        loop {
            // execute all packets
            let ServerUpdate {
                messages,
                connects,
                disconnects
            } = server.update();
            for (protocol, addr, message) in messages {
                match execute_server_command(&message, ((protocol, &addr), &mut server)) {
                    Ok(()) => println!("Server ran client command from {}", addr),
                    Err(err) => println!("Error running client {} command: {}", addr, err)
                }
            }
            for addr in connects {
                println!("New connection from {}", addr);
            }
            for addr in disconnects {
                println!("Disconnected from {}", addr);
            }
            std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        }
    }

    mod client {
        use std::cmp;

        use crate::{commands_execute, _commands_execute_static_def, networking::{Protocol, client::{Client, ClientError}}};

        use super::ServerCommandID;

        commands_execute!(
            execute_client_command,
            ClientCommand,
            ClientCommandID,
            (Protocol, &mut Client),
            // list all commands the client can execute here:
            [
                super::SendAddress,
                super::EchoMessage
            ]
        );

        pub trait SendCommands {
            fn send<T: ServerCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError>;
        }

        impl SendCommands for Client {
            fn send<T: ServerCommandID>(&mut self, protocol: Protocol, command: &T) -> std::result::Result<(), ClientError> {
                self.send_data(protocol, command.make_bytes())
            }
        }

        // list how the client will respond to each command below

        impl<'a> ClientCommand<'a> for super::SendAddress {
            fn run(self, (_, client): (Protocol, &mut Client)) {
                //println!("Server sent their view of client's address: {}", self.0);
                match client.send(Protocol::TCP, &super::SetUDPAddress(self.0)) {
                    Ok(()) => (),
                    Err(err) => println!("Failed to send address to server: {}", err)
                }
            }
        }

        impl <'a> ClientCommand<'a> for super::EchoMessage {
            fn run(self, _context: (Protocol, &mut Client)) {
                println!("Echoed message: {}", &self.0.as_str()[0..cmp::min(self.0.len(), 4096)]);
            }
        }
    }

    pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> std::io::Result<()> {
        let mut client = Client::init_disconnected();
        client.connect(addresses.0, addresses.1);
        let stdin_channel = console_stream();
        let mut run = true;
        while run {
            let mut updates = vec![];
            match stdin_channel.try_recv() {
                Ok(msg) => {
                    match (|| -> std::result::Result<(), ClientError> {
                        let split: Vec<&str> = msg.split(' ').collect();
                        match &split[..] {
                            ["stop", ..] => {
                                run = false;
                            }
                            ["disconnect", ..] => {
                                if let Some(update) = client.disconnect(None) {
                                    updates.push(update);
                                }
                            }
                            ["getaddr"] => {
                                client.send(Protocol::UDP, &GetAddress)?;
                            },
                            ["setaddr", _, ..] => {
                                client.send(Protocol::TCP, &SetUDPAddress(msg["setaddr ".len()..msg.len()].into()))?;
                            }
                            ["udp", "echo", _, ..] => {
                                client.send(Protocol::UDP, &EchoMessage(msg["udp echo ".len()..msg.len()].into()))?;
                            },
                            ["tcp", "echo", _, ..] => {
                                client.send(Protocol::TCP, &EchoMessage(msg["tcp echo ".len()..msg.len()].into()))?;
                            },
                            ["tcp", "big", len] => {
                                client.send(Protocol::TCP, &EchoMessage({
                                    let mut s = String::new();
                                    for _ in 0..len.parse().unwrap() {
                                        s.push('t');
                                    }
                                    s
                                }))?;
                            }
                            ["connect", _, _] => {
                                match (split[1].parse(), split[2].parse()) {
                                    (Ok(udp), Ok(tcp)) => {
                                        client.connect(udp, tcp);
                                    },
                                    (Err(err), _) => {
                                        println!("Error parsing udp addr: {}", err);
                                    },
                                    (Ok(_), Err(err)) => {
                                        println!("Error parsing tcp addr: {}", err);
                                    }
                                }
                            },
                            _ => println!("Invalid command: {}", msg)
                        };
                        Ok(())
                    })() {
                        Ok(()) => (),
                        Err(err) => {
                            println!("Error running command: {}", err);
                        }
                    }
                    // tcp_write_buffer.extend(msg.as_bytes());
                },
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => break,
            }
    
            updates.extend(client.update());
            for update in updates {
                match update {
                    ClientUpdate::Message(protocol, message) => match execute_client_command(&message, (protocol, &mut client)) {
                        Ok(()) => (),
                        Err(err) => {
                            println!("{}", err);
                        }
                    },
                    _ => println!("{}", update)
                }
            }
    
            std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        }
        println!("User requested stop");
        Ok(())
    }
    
}

