extern crate glfw;

// pub mod graphics;
// pub mod chatbox;
// pub mod networking;
// pub mod networking_wrapping;
pub mod networking_commands;
// pub mod game;
// pub mod server;
// pub mod world;
pub mod networking2;

use std::collections::HashMap;
use std::net::{UdpSocket, TcpListener, TcpStream, Shutdown};
use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc::{self, Receiver};
use std::sync::mpsc::TryRecvError;
use std::io::{self, ErrorKind, Read, Write};

use serde::{Serialize, Deserialize};

// fn echo_server(port_udp: u16, port_tcp: u16) -> Result<()> {
//     let mut server = networking::server::ServerConnection::new(port_udp, port_tcp)?;
//     let mut stop = false;
//     while !stop {
//         for (id, data) in server.poll() {
//             for packet in data {
//                 if String::from_utf8_lossy(packet.as_slice()).eq("stop") {
//                     stop = true;
//                 }
//                 server.send_udp(vec![id], packet);
//             }
//         }
//         server.flush();
//         std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
//     }
//     Ok(())
// }

fn console_client_udp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
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
                println!("Received from {:?}: {}", addr, std::str::from_utf8(packet.as_slice()).unwrap());
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

fn grab_console_line(prompt: &str) -> String {
    let mut buffer = String::new();
    io::stdout().write(prompt.as_bytes()).unwrap();
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut buffer).unwrap();
    String::from(buffer.trim())
}

fn console_stream() -> Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer.trim_end().into()).unwrap();
    });
    rx
}

fn udp_recv_all(socket: &UdpSocket, buffer: &mut [u8], limit: Option<usize>)
    -> (HashMap<SocketAddr, Vec<Vec<u8>>>, Option<std::io::Error>) {
    let mut error = None;
    let mut map: HashMap<SocketAddr, Vec<Vec<u8>>> = HashMap::new();
    let limit = match limit {
        Some(limit) => limit,
        None => usize::MAX
    };
    for _ in 0..limit {
        match socket.recv_from(buffer) {
            Ok((sent, addr)) => {
                let packet = Vec::from(&buffer[0..sent]);
                let spot = map.get_mut(&addr);
                match spot {
                    Some(spot) => {
                        spot.push(packet);
                    },
                    None => {
                        map.insert(addr, vec![packet]);
                    }
                }
            },
            Err(err) => {
                error = Some(err);
                break
            }
        }
    }
    (map, error)
}

fn echo_server_udp(ports: (u16, u16)) -> Result<()> {
    let udp = UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?;
    udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    let mut run = true;
    while run {
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        let (recv, err) = udp_recv_all(&udp, &mut buffer, None);
        for (addr, packets) in recv {
            for packet in packets {
                let str = String::from_utf8_lossy(packet.as_slice());
                println!("Recv from {:?}: {}", addr, str);
                match udp.send_to(packet.as_slice(), addr) {
                    Ok(size) => println!("Sent {} bytes", size),
                    Err(err) => println!("Error sending: {}", err)
                }
                match str.as_ref() {
                    "stop" => run = false,
                    _ => ()
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

fn echo_server_tcp(ports: (u16, u16)) -> Result<()> {
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

#[derive(Serialize, Deserialize)]
struct GetAddress;

#[derive(Serialize, Deserialize)]
struct SendAddress(String);

impl<'a> ServerCommand<'a> for GetAddress {
    fn run(self, (addr, server): (&SocketAddr, &mut ServerData)) {
        let packet: SerializedClientCommand = SendAddress(addr.to_string()).into();
        match server.udp.send_to(packet.data.as_slice(), addr) {
            Ok(size) => println!("Sent UDP {} bytes", size),
            Err(err) => println!("Error UDP sending: {}", err)
        };
    }
}

impl<'a> ClientCommand<'a> for SendAddress {
    fn run(self, _client: &mut ClientData) {
        println!("Server sent their view of client's address: {}", self.0);
    }
}

commands_execute!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    SerializedClientCommand,
    &mut ClientData,
    // list commands here:
    [
        SendAddress
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
    (&SocketAddr, &mut ServerData),
    // list commands here:
    [
        GetAddress
    ]
);

pub struct ClientData {
    tcp: TcpStream,
    udp: UdpSocket,
    addr_tcp: SocketAddr,
    addr_udp: SocketAddr
}

pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    let mut client = ClientData {
        tcp: TcpStream::connect(addresses.1)?,
        udp: UdpSocket::bind("0.0.0.0:0")?,
        addr_tcp: addresses.1,
        addr_udp: addresses.0
    };
    println!("Connected on {}", client.tcp.local_addr()?);
    client.tcp.set_nonblocking(true)?;
    client.udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    let mut tcp_write_buffer: Vec<u8> = vec![];
    loop {
        let mut udp_message = None;
        match stdin_channel.try_recv() {
            Ok(msg) => {
                let split: Vec<&str> = msg.split(" ").collect();
                match &split[..] {
                    ["getaddr"] => {
                        udp_message = Some(SerializedServerCommand::from(GetAddress).data);
                    },
                    ["setaddr", _, ..] => {
                        tcp_write_buffer.extend(msg["setaddr ".len()..msg.len()].as_bytes());
                        tcp_write_buffer.push(0u8);
                    }
                    ["udp", _, ..] => {
                        udp_message = Some(Vec::from(msg["udp ".len()..msg.len()].as_bytes()));
                    },
                    ["tcp", _, ..] => {
                        tcp_write_buffer.extend(msg["tcp ".len()..msg.len()].as_bytes())
                    },
                    _ => println!("Invalid command: {}", msg)
                };
                // tcp_write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }

        let (recv, err) = udp_recv_all(&client.udp, buffer.as_mut(), None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received UDP from {:?}: {}", addr, std::str::from_utf8(packet.as_slice()).unwrap());
                let command = SerializedClientCommand {
                    data: packet
                };
                match command.execute(&mut client) {
                    Ok(()) => (),
                    Err(err) => println!("Error deserializing UDP command: {}", err)
                }
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error receiving UDP: {}", err)
            },
            None => ()
        }
        if let Some(message) = udp_message {
            match client.udp.send_to(message.as_slice(), &client.addr_udp) {
                Ok(sent) => println!("Sent UDP {} bytes", sent),
                Err(err) => match err.kind() {
                    ErrorKind::WouldBlock => (),
                    _ => println!("Error sending UDP: {}", err)
                }
            }
        }

        // tcp stuff
        match client.tcp.read(buffer.as_mut()) {
            Ok(size) => match size {
                0 => break,
                _ => {
                    let data = &buffer[0..size];
                    let str = String::from_utf8_lossy(data);
                    println!("Received TCP from {}: {}", client.addr_tcp, str);
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => {
                    println!("Error receiving TCP from {}: {}", client.addr_tcp, err);
                }
            }
        }
        match client.tcp.write(tcp_write_buffer.as_mut_slice()) {
            Ok(sent) => {
                tcp_write_buffer.drain(0..sent);
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error writing TCP to {}: {}", client.addr_tcp, err)
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}

struct ConnectionInfo {
    stream: TcpStream,
    write_buffer: Vec<u8>,
    _tcp_address: SocketAddr,
    udp_address: Option<SocketAddr>,
    udp_address_buffer_temp: Vec<u8>
}

pub struct ServerData {
    udp: UdpSocket,
    tcp: TcpListener,
    connections: HashMap<SocketAddr, ConnectionInfo>
}

const MAX_UDP_ADDR_SIZE: usize = 50;

fn echo_server_both(ports: (u16, u16)) -> Result<()> {
    let mut data = ServerData {
        udp: UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?,
        tcp: TcpListener::bind(format!("0.0.0.0:{}", ports.1))?,
        connections: HashMap::new()
    };
    data.udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    data.tcp.set_nonblocking(true)?;
    loop {
        // do UDP
        let (recv, err) = udp_recv_all(&data.udp, &mut buffer, None);
        for (addr, packets) in recv {
            for packet in packets {
                let str = String::from_utf8_lossy(packet.as_slice()).to_string();
                // match udp.send_to(packet.as_slice(), addr) {
                //     Ok(size) => println!("Sent UDP {} bytes", size),
                //     Err(err) => println!("Error UDP sending: {}", err)
                // }
                let command = SerializedServerCommand {
                    data: packet
                };
                match command.execute((&addr, &mut data)) {
                    Ok(()) => println!("Ran UDP command from {:?}: {}", addr, str),
                    Err(err) => println!("Error deserializing UDP packet from {}: {}", addr, err),
                }
            }
        }
        // errors for udp
        if let Some(err) = err {
            match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error UDP receiving: {}", err)
            }
        }

        // listen on TCP
        loop {
            match data.tcp.accept() {
                Ok((stream, addr)) => {
                    println!("New connection from {}", addr);
                    data.connections.insert(addr, ConnectionInfo {
                        stream,
                        write_buffer: vec![],
                        _tcp_address: addr,
                        udp_address: None,
                        udp_address_buffer_temp: vec![]
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
        for (addr, info) in &mut data.connections {
            match info.stream.read(buffer.as_mut()) {
                Ok(size) => match size {
                    0 => disconnects.push(*addr),
                    _ => {
                        // process first CString of stream into UDP address
                        // kick if bad string (too long or failed to parse)
                        let mut kick = None;
                        let data = if let None = info.udp_address {
                            let mut finished = false;
                            let udp_addr_grab = {
                                let mut len = buffer.len();
                                for i in 0..buffer.len() {
                                    if buffer[i] == 0u8 {
                                        len = i;
                                        finished = true;
                                        break;
                                    }
                                }
                                len
                            };
                            if udp_addr_grab + info.udp_address_buffer_temp.len() > MAX_UDP_ADDR_SIZE {
                                kick = Some(format!("UDP address length is too long"));
                            } else {
                                // try to construct client's UDP address
                                info.udp_address_buffer_temp.extend(&buffer[0..udp_addr_grab]);
                                if finished {
                                    let mut address_buffer = vec![];
                                    std::mem::swap( // clear info UDP address buffer while grabbing ownership of it
                                        &mut address_buffer,
                                        &mut info.udp_address_buffer_temp
                                    );
                                    match String::from_utf8(address_buffer) {
                                        Ok(str) => match str.parse() {
                                            Ok(udp_addr) => {
                                                info.udp_address = Some(udp_addr);
                                                println!("Got UDP address from {}: {}", addr, udp_addr);
                                            },
                                            Err(err) => kick = Some(format!(
                                                "Failed to parse UDP address. Address: {}, Error: {}",
                                                str,
                                                err.to_string()
                                            )),
                                        },
                                        Err(err) => kick = Some(format!(
                                            "Failed to parse UTF8 for UDP address: {}",
                                            err.to_string()
                                        )),
                                    }
                                }
                            }
                            &buffer[udp_addr_grab..size]
                        } else {
                            &buffer[0..size]
                        };
                        if let Some(err_str) = kick {
                            disconnects.push(*addr);
                            println!("Error receiving UDP address from {}: {}", addr, err_str);
                        } else {
                            let str = String::from_utf8_lossy(data);
                            println!("Received TCP from {}: {}", addr, str);
                            info.write_buffer.extend(data);
                        }
                    }
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => {
                        println!("Error TCP receiving from {}: {}", addr, err);
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
                            println!("Error TCP writing to {}: {}", addr, err);
                            disconnects.push(*addr);
                            continue;
                        }
                    }
                }
            }
        }
        for addr in disconnects {
            if let Some(info) = data.connections.remove(&addr) {
                if let Err(err) = info.stream.shutdown(Shutdown::Both) {
                    println!("Error disconnecting from {}: {}", addr, err);
                } else {
                    println!("Disconnected from {}", addr);
                }
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let ports: (u16, u16) =
                (grab_console_line("UDP port: ").parse().expect("Invalid port"),
                 grab_console_line("TCP port: ").parse().expect("Invalid port"));
    let addresses: (SocketAddr, SocketAddr) = (
        format!("127.0.0.1:{}", ports.0).parse().unwrap(),
        format!("127.0.0.1:{}", ports.1).parse().unwrap()
    );
    match args[1].as_str() {
        // "echo_server" => {
        //     echo_server(1234, 1235)?
        // },
        // "server" => {
        //     // server::Server::run(1234);
        // },
        // "gclient" => {
        //     // game::Game::run(&server_address);
        // }
        // "client" => { // client
        //     console_client(server_address_udp, server_address_tcp)?
        // },
        "server" => {
            echo_server_udp(ports)?
        },
        "client" => {
            console_client_udp(addresses)?
        },
        "tcpclient" => {
            console_client_tcp(addresses)?
        },
        "tcpserver" => {
            echo_server_tcp(ports)?
        },
        "bothserver" => {
            echo_server_both(ports)?
        },
        "bothclient" => {
            console_client_both(addresses)?
        },
        _ => {
            println!("Unknown mode");
        }
    }
    Ok(())
}

