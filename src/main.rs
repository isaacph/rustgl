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

use std::collections::{HashMap, VecDeque};
use std::net::{UdpSocket, TcpListener, TcpStream, Shutdown};
use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc::{self, Receiver};
use std::sync::mpsc::TryRecvError;
use std::io::{self, ErrorKind, Read, Write};

use serde::{Serialize, Deserialize};

use crate::tcp_buffering::{TcpSendState, TcpRecvState};

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

pub const MAX_UDP_MESSAGE_SIZE: usize = 512;
pub const MAX_TCP_MESSAGE_SIZE: usize = 1<<20; // why would you send more than 1MB? even that's probably too much
pub const MAX_TCP_MESSAGE_QUEUE_SIZE: usize = 1<<26; // max they can ddos me for 640 mb
pub const TCP_BUFFER_SIZE: usize = 1<<10;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub enum Protocol {
    TCP, UDP
}

#[derive(Serialize, Deserialize)]
struct GetAddress;

#[derive(Serialize, Deserialize)]
struct SendAddress(String);

#[derive(Serialize, Deserialize)]
struct SetUDPAddress(String);

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
    fn run(self, _: (Protocol, &mut Client)) {
        println!("Server sent their view of client's address: {}", self.0);
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
struct EchoMessage(String);

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

mod tcp_buffering {
    use std::{collections::VecDeque, cmp};

    use crate::{MAX_TCP_MESSAGE_SIZE, MAX_TCP_MESSAGE_QUEUE_SIZE, TCP_BUFFER_SIZE};

    pub struct TcpRecvState {
        buffer: Box<[u8]>,
        length: usize,
        remaining: usize,
        failure: Option<String>
    }
    
    impl TcpRecvState {
        pub fn init() -> TcpRecvState {
            TcpRecvState {
                buffer: vec![0u8; TCP_BUFFER_SIZE].into_boxed_slice(),
                length: 0,
                remaining: 0,
                failure: None
            }
        }

        pub fn failed(&self) -> Option<String> {
            self.failure.clone()
        }

        pub fn receive(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
            let mut messages: Vec<Vec<u8>> = vec![];
    
            let mut extension_read_start = 0;
            let extension: &[u8] = data;
    
            // work until reaching the end of the current extension of data
            while extension_read_start < extension.len() {
                // we need to find another remaining number of bytes to process
                if self.remaining == 0 {
                    if self.length + extension.len() - extension_read_start < 4 {
                        // copy the rest of extension in
                        let copy_len = extension.len() - extension_read_start;
                        self.buffer[self.length..self.length + copy_len]
                            .copy_from_slice(&extension[extension_read_start..]);
                            self.length += copy_len;
                        //extension_read_start += copy_len; // will not be read again
                        break;
    
                    } else if self.length >= 4 {
                        // this shouldn't happen
                        panic!("Logic error decoding TCP message!");
    
                    } else {
                        // we have enough bytes to find message length
                        // remove remaining message length bytes from extension
                        let copy_len = 4 - self.length;
                        self.buffer[self.length..self.length + copy_len].copy_from_slice(
                            &extension[extension_read_start..extension_read_start + copy_len]);
                        //self.length += extend_by; // will be cleared anyway
                        extension_read_start += copy_len;
    
                        // find full message length
                        let remaining_in_bytes: [u8; 4] = self.buffer[0..4].try_into().unwrap();
                        self.remaining = u32::from_be_bytes(remaining_in_bytes) as usize;
    
                        // clear message buffer (should have only 4 bytes)
                        self.length = 0;
                    }
                }
                if self.remaining > MAX_TCP_MESSAGE_SIZE {
                    self.failure = Some(format!("Attempted to decode message that was too big: {}", self.remaining));
                    break;
                }
                if self.remaining > 0 {
                    // copy either remaining message bytes, or all new bytes, whichever is smaller
                    let copy_len = cmp::min(self.remaining, extension.len() - extension_read_start);
                    self.buffer[self.length..self.length + copy_len]
                        .copy_from_slice(&extension[extension_read_start..extension_read_start + copy_len]);
                    self.length += copy_len;
                    self.remaining -= copy_len;
                    extension_read_start += copy_len;
    
                    // if we copied all remaining message bytes
                    if self.remaining == 0 {
                        // we finished a packet!
                        // store the finished message
                        let finished_message = self.buffer[0..self.length].to_vec();
                        messages.push(finished_message);
                        // clear the message
                        self.length = 0;
                    }
                }
            }
            messages
        }
    }
    
    pub struct TcpSendState {
        queue: VecDeque<Vec<u8>>,
        queue_size: usize,
        buffer: Box<[u8]>,
        length: usize,
        position: usize
    }
    
    impl TcpSendState {
        pub fn init() -> Self {
            TcpSendState {
                queue: VecDeque::new(),
                queue_size: 0,
                buffer: vec![0u8; TCP_BUFFER_SIZE].into_boxed_slice(),
                length: 0,
                position: 0
            }
        }

        pub fn next_send(&self) -> Option<&[u8]> {
            if self.position < self.length {
                Some(&self.buffer[self.position..self.length])
            } else {
                None
            }
        }

        pub fn update_buffer(&mut self, sent: usize) {
            self.position += sent;
            if self.position >= self.length {
                if let Some(next) = self.queue.pop_front() {
                    let length: [u8; 4] = u32::to_be_bytes(next.len() as u32);
                    self.buffer[0..4].copy_from_slice(&length);
                    self.buffer[4..4 + next.len()].copy_from_slice(next.as_slice());
                    self.position = 0;
                    self.length = next.len() + 4;
                    self.queue_size -= next.len();
                }
            }
        }

        pub fn enqueue(&mut self, packet: Vec<u8>) -> std::result::Result<(), String> {
            if packet.len() + 4 > MAX_TCP_MESSAGE_SIZE {
                Err(format!("Tried to send message that was too big: {} > {}", packet.len() + 4, MAX_TCP_MESSAGE_SIZE))
            } else if self.queue_size + packet.len() > MAX_TCP_MESSAGE_QUEUE_SIZE {
                Err(format!("Exceeded maximum message queue size for client"))
            } else {
                self.queue_size += packet.len();
                self.queue.push_back(packet);
                self.update_buffer(0);
                Ok(())
            }
        }
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

pub struct Client {
    tcp: TcpStream,
    udp: UdpSocket,
    addr_tcp: SocketAddr,
    addr_udp: SocketAddr,
    udp_message_queue: VecDeque<SerializedServerCommand>,
    tcp_send: tcp_buffering::TcpSendState,
    tcp_recv: tcp_buffering::TcpRecvState,
}

impl Client {
    pub fn send_udp(&mut self, packet: SerializedServerCommand) {
        if packet.data.len() > MAX_UDP_MESSAGE_SIZE {
            println!("Attempted to send UDP message that was too big: {} > {}", packet.data.len(), MAX_UDP_MESSAGE_SIZE);
            return;
        }
        self.udp_message_queue.push_back(packet);
    }
    pub fn send_tcp(&mut self, packet: SerializedServerCommand) {
        self.tcp_send.enqueue(packet.data).unwrap();
    }
}

pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    let mut client = Client {
        tcp: TcpStream::connect(addresses.1)?,
        udp: UdpSocket::bind("0.0.0.0:0")?,
        addr_tcp: addresses.1,
        addr_udp: addresses.0,
        udp_message_queue: VecDeque::new(),
        tcp_send: TcpSendState::init(),
        tcp_recv: TcpRecvState::init()
    };
    println!("Connected on {}", client.tcp.local_addr()?);
    client.tcp.set_nonblocking(true)?;
    client.udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    loop {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                let split: Vec<&str> = msg.split(" ").collect();
                match &split[..] {
                    ["getaddr"] => {
                        client.send_udp((&GetAddress).into());
                    },
                    ["setaddr", _, ..] => {
                        client.send_tcp(
                            (&SetUDPAddress(msg["setaddr ".len()..msg.len()].into())).into()
                        );
                        // let mut buffer = Vec::from(msg["setaddr ".len()..msg.len()].as_bytes());
                        // buffer.push(0u8);
                        // match client.tcp_send.enqueue(buffer) {
                        //     Ok(()) => (),
                        //     Err(err) => println!("Error sending command: {}", err)
                        // }
                    }
                    ["udp", "echo", _, ..] => {
                        client.send_udp((&EchoMessage(msg["udp echo ".len()..msg.len()].into())).into());
                    },
                    ["tcp", "echo", _, ..] => {
                        client.send_tcp(
                            (&EchoMessage(msg["tcp echo ".len()..msg.len()].into())).into()
                        );
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
                println!("Received UDP from {:?}: {}", addr, String::from_utf8_lossy(packet.as_slice()));
                let command = SerializedClientCommand {
                    data: packet
                };
                match command.execute((Protocol::UDP, &mut client)) {
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
        while let Some(message) = client.udp_message_queue.pop_front() {
            match client.udp.send_to(message.data.as_slice(), &client.addr_udp) {
                Ok(sent) => println!("Sent UDP {} bytes", sent),
                Err(err) => {
                    match err.kind() {
                        ErrorKind::WouldBlock => break,
                        _ => println!("Error sending UDP: {}", err)
                    }
                    client.udp_message_queue.push_front(message);
                }
            }
        }

        // tcp stuff
        match client.tcp.read(buffer.as_mut()) {
            Ok(size) => match size {
                0 => break,
                _ => {
                    for data in client.tcp_recv.receive(&buffer[0..size]) {
                        let str = String::from_utf8_lossy(&data);
                        println!("Received TCP from {}: {}", client.addr_tcp, str);
                        match SerializedClientCommand::from(data).execute((Protocol::TCP, &mut client)) {
                            Ok(()) => (),
                            Err(err) => {
                                println!("{}", err);
                            }
                        }
                    }
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => {
                    println!("Error receiving TCP from {}: {}", client.addr_tcp, err);
                }
            }
        }
        if let Some(error) = client.tcp_recv.failed() {
            println!("Error in TCP stream: {}", error);
            break;
        }
        let mut quit = false;
        while let Some(buffer) = client.tcp_send.next_send() {
            match client.tcp.write(buffer) {
                Ok(sent) => {
                    client.tcp_send.update_buffer(sent);
                    println!("Sent {} TCP bytes", sent);
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => {
                        println!("Error writing TCP to {}: {}", client.addr_tcp, err);
                        quit = true;
                    }
                }
            }
        }
        if quit {
            break;
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}

struct ConnectionInfo {
    stream: TcpStream,
    _tcp_address: SocketAddr,
    udp_address: Option<SocketAddr>,
    udp_send_queue: VecDeque<SerializedClientCommand>,
    tcp_recv: TcpRecvState,
    tcp_send: TcpSendState
}

pub struct Server {
    udp: UdpSocket,
    tcp: TcpListener,
    connections: HashMap<SocketAddr, ConnectionInfo>,
    corresponding_tcp_to_udp: HashMap<SocketAddr, SocketAddr>
}

impl Server {
    pub fn send_tcp(&mut self, tcp_addr: &SocketAddr, data: SerializedClientCommand) -> std::result::Result<(), String> {
        match self.connections.get_mut(tcp_addr) {
            Some(info) => info.tcp_send.enqueue(data.data),
            None => Err(format!("Client with TCP address {} not found", tcp_addr))
        }
    }
    pub fn send_udp(&mut self, tcp_addr: &SocketAddr, data: SerializedClientCommand) -> std::result::Result<(), String> {
        match self.connections.get_mut(tcp_addr) {
            Some(info) => {
                info.udp_send_queue.push_back(data);
                Ok(())
            },
            None => Err(format!("Client with TCP address {} not found", tcp_addr))
        }
    }
}

fn echo_server_both(ports: (u16, u16)) -> Result<()> {
    let mut server = Server {
        udp: UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?,
        tcp: TcpListener::bind(format!("0.0.0.0:{}", ports.1))?,
        connections: HashMap::new(),
        corresponding_tcp_to_udp: HashMap::new()
    };
    server.udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    server.tcp.set_nonblocking(true)?;
    loop {
        // recv UDP
        let mut packets: Vec<(Protocol, SocketAddr, SerializedServerCommand)> = Vec::new();
        let (recv, err) = udp_recv_all(&server.udp, &mut buffer, None);
        for (addr, data) in recv {
            for packet in data {
                let _ = String::from_utf8_lossy(packet.as_slice()).to_string();
                let command = SerializedServerCommand {
                    data: packet
                };
                packets.push((Protocol::UDP, addr, command));
                // match command.execute(((Protocol::UDP, &addr), &mut server)) {
                //     Ok(()) => println!("Ran UDP command from {:?}: {}", addr, str),
                //     Err(err) => println!("Error deserializing UDP packet from {}: {}", addr, err),
                // }
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
            match server.tcp.accept() {
                Ok((stream, addr)) => {
                    println!("New connection from {}", addr);
                    server.connections.insert(addr, ConnectionInfo {
                        stream,
                        _tcp_address: addr,
                        udp_address: None,
                        udp_send_queue: VecDeque::new(),
                        tcp_recv: TcpRecvState::init(),
                        tcp_send: TcpSendState::init()
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
        for (addr, info) in &mut server.connections {
            // send udp
            if let Some(udp_address) = info.udp_address {
                while let Some(packet) = info.udp_send_queue.pop_front() {
                    match server.udp.send_to(packet.data.as_slice(), udp_address) {
                        Ok(sent) => {
                            if sent != packet.data.len() {
                                println!("Somehow didn't send entire UDP packet");
                            }
                        },
                        Err(err) => {
                            match err.kind() {
                                std::io::ErrorKind::WouldBlock => (),
                                _ => println!("Error sending UDP packet to client (TCP address {}): {}", addr, err)
                            }
                            info.udp_send_queue.push_front(packet);
                            break;
                        }
                    }
                }
            }

            // read tcp
            match info.stream.read(buffer.as_mut()) {
                Ok(size) => match size {
                    0 => disconnects.push(*addr),
                    _ => {
                        let data = info.tcp_recv.receive(&buffer[0..size]);
                        for packet in &data {
                            let str = String::from_utf8_lossy(&packet);
                            println!("Received TCP from {}: {}", addr, str);
                        }
                        packets.extend(data.into_iter().map(|data| (Protocol::TCP, *addr, data.into())));
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
            if let Some(error) = info.tcp_recv.failed() {
                println!("Error in TCP recv stream: {}", error);
                disconnects.push(*addr);
                continue;
            }

            // send tcp
            while let Some(buffer) = info.tcp_send.next_send() {
                match info.stream.write(buffer) {
                    Ok(sent) => match sent {
                        0 => {
                            disconnects.push(*addr);
                            break;
                        },
                        _ => {
                            println!("Sent {} TCP bytes", sent);
                            info.tcp_send.update_buffer(sent)
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        _ => {
                            println!("Error TCP writing to {}: {}", addr, err);
                            disconnects.push(*addr);
                            break;
                        }
                    }
                }
            }
        }
        for (protocol, addr, packet) in packets {
            match packet.execute(((protocol, &addr), &mut server)) {
                Ok(()) => println!("Server ran client command from {}", addr),
                Err(err) => println!("Error running client {} command: {}", addr, err)
            }
        }
        for addr in disconnects {
            if let Some(info) = server.connections.remove(&addr) {
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

