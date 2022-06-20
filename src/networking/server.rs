use std::{net::{TcpStream, SocketAddr, UdpSocket, TcpListener, Shutdown}, collections::{VecDeque, HashMap}, io::{Read, Write}, cmp, fmt::Display};
use crate::model::SerializedClientCommand;
use super::{tcp_buffering::{TcpRecvState, TcpSendState}, Protocol, config::RECV_BUFFER_SIZE, common::udp_recv_all};

pub struct ConnectionInfo {
    stream: TcpStream,
    tcp_address: SocketAddr,
    udp_address: Option<SocketAddr>,
    udp_send_queue: VecDeque<SerializedClientCommand>,
    tcp_recv: TcpRecvState,
    tcp_send: TcpSendState
}

pub struct Server {
    udp: UdpSocket,
    tcp: TcpListener,
    connections: HashMap<SocketAddr, ConnectionInfo>,
    corresponding_tcp_to_udp: HashMap<SocketAddr, SocketAddr>,
    recv_buffer: Box<[u8]>
}

#[derive(Debug)]
pub enum ServerError {
    NoConnection,
    Disconnected,
    Other(String)
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type ServerResult<T> = Result<T, ServerError>;

pub struct ServerUpdate {
    pub messages: Vec<(Protocol, SocketAddr, Box<[u8]>)>,
    pub connects: Vec<SocketAddr>,
    pub disconnects: Vec<SocketAddr>
}

impl Server {
    pub fn send_tcp(&mut self, tcp_addr: &SocketAddr, data: SerializedClientCommand) -> std::result::Result<(), String> {
        println!("Sending message to {}, length {}", tcp_addr, data.0.len());
        match self.connections.get_mut(tcp_addr) {
            Some(info) => info.tcp_send.enqueue(data.0),
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

    pub fn get_tcp_address(&self, udp_addr: &SocketAddr) -> Option<SocketAddr> {
        self.corresponding_tcp_to_udp.get(udp_addr).copied()
    }

    pub fn send_udp_to_unidentified(&mut self, udp_addr: &SocketAddr, data: SerializedClientCommand) -> std::io::Result<usize>{
        self.udp.send_to(&data.0, udp_addr)
    }

    pub fn set_client_udp_addr(&mut self, tcp_addr: &SocketAddr, udp_addr: &SocketAddr) -> std::result::Result<(), String> {
        match self.connections.get_mut(tcp_addr) {
            Some(info) => {
                info.udp_address = Some(*udp_addr);
                self.corresponding_tcp_to_udp.insert(*udp_addr, *tcp_addr);
                Ok(())
            },
            None => {
                Err(format!("Client with TCP address {} not found", tcp_addr))
            }
        }
    }

    pub fn update_udp_recv(&mut self, messages: &mut Vec<(Protocol, SocketAddr, Box<[u8]>)>) -> ServerResult<()> {
        // recv UDP
        let (recv, err) = udp_recv_all(&self.udp, &mut self.recv_buffer, None);
        for (addr, data) in recv {
            for packet in data {
                let s = String::from_utf8_lossy(packet.as_ref()).to_string();
                println!("Received UPD from {:?} of len {}: {}", addr, packet.len(), s);
                messages.push((Protocol::UDP, addr, packet));
                // match command.execute(((Protocol::UDP, &addr), &mut server)) {
                //     Ok(()) => println!("Ran UDP command from {:?}: {}", addr, str),
                //     Err(err) => println!("Error deserializing UDP packet from {}: {}", addr, err),
                // }
            }
        }
        // errors for udp
        if let Some(err) = err {
            match err.kind() {
                std::io::ErrorKind::WouldBlock => Ok(()),
                _ => {
                    Err(ServerError::Other(format!("Error UDP receiving: {}", err)))
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn update_tcp_listen(&mut self, connections: &mut Vec<SocketAddr>) -> ServerResult<()> {
        // listen on TCP
        loop {
            match self.tcp.accept() {
                Ok((stream, addr)) => {
                    // println!("New connection from {}", addr);
                    match stream.set_nonblocking(true) {
                        Ok(()) => {
                            self.connections.insert(addr, ConnectionInfo {
                                stream,
                                tcp_address: addr,
                                udp_address: None,
                                udp_send_queue: VecDeque::new(),
                                tcp_recv: TcpRecvState::init(),
                                tcp_send: TcpSendState::init()
                            });
                            connections.push(addr);
                        },
                        Err(err) => {
                            println!("Failed to accept connection from {} since could not set nonblocking: {}", addr, err);
                        }
                    }
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => return Ok(()),
                    _ => {
                        return Err(ServerError::Other(format!("Error with TCP accept: {}", err)));
                    }
                }
            }
        }
    }

    pub fn update(&mut self) -> ServerUpdate {
        let mut messages: Vec<(Protocol, SocketAddr, Box<[u8]>)> = Vec::new();
        let mut connects = vec![];
        let mut disconnects = vec![];
        match (|| -> ServerResult<()> {
            self.update_udp_recv(&mut messages)?;
            self.update_tcp_listen(&mut connects)?;
            for (addr, info) in &mut self.connections {
                match (|| -> ServerResult<()> {
                    info.update_udp_send(&self.udp)?;
                    info.update_tcp_recv(&mut messages, &mut self.recv_buffer)?;
                    info.update_tcp_send()?;
                    Ok(())
                })() {
                    Ok(()) => (),
                    Err(err) => {
                        println!("Client error: {}", err);
                        disconnects.push(*addr);
                    }
                }
            }
            Ok(())
        })() {
            Ok(()) => (),
            Err(err) => {
                println!("Server error: {}", err);
            }
        }
        
        // disconnect clients
        for addr in &disconnects {
            if let Some(info) = self.connections.remove(&addr) {
                if let Err(err) = info.stream.shutdown(Shutdown::Both) {
                    println!("Error disconnecting from {}: {}", addr, err);
                }
            }
        }
        ServerUpdate {
            messages,
            connects,
            disconnects
        }
    }

    pub fn init(ports: (u16, u16)) -> std::io::Result<Self> {
        let server = Server {
            udp: UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?,
            tcp: TcpListener::bind(format!("0.0.0.0:{}", ports.1))?,
            connections: HashMap::new(),
            corresponding_tcp_to_udp: HashMap::new(),
            recv_buffer: vec![0u8; RECV_BUFFER_SIZE].into_boxed_slice(),
        };
        server.udp.set_nonblocking(true)?;
        server.tcp.set_nonblocking(true)?;
        Ok(server)
    }
}

impl ConnectionInfo {
    pub fn update_udp_send(&mut self, udp: &UdpSocket) -> ServerResult<()> {
        // send udp
        if let Some(udp_address) = self.udp_address {
            while let Some(packet) = self.udp_send_queue.pop_front() {
                match udp.send_to(packet.0.as_ref(), udp_address) {
                    Ok(sent) => {
                        if sent != packet.0.len() {
                            println!("Somehow didn't send entire UDP packet");
                        }
                    },
                    Err(err) => {
                        self.udp_send_queue.push_front(packet);
                        return match err.kind() {
                            std::io::ErrorKind::WouldBlock => Ok(()),
                            _ => Err(ServerError::Other(
                                format!("Error sending UDP packet to client (TCP address {}): {}",
                                    self.tcp_address,
                                    err
                                )))
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    pub fn update_tcp_recv(&mut self, messages: &mut Vec<(Protocol, SocketAddr, Box<[u8]>)>, buffer: &mut [u8]) -> ServerResult<()> {
        // read tcp
        let addr = self.tcp_address;
        match self.stream.read(buffer) {
            Ok(size) => match size {
                0 => return Err(ServerError::Disconnected),
                _ => {
                    println!("Received TCP bytes: {}", size);
                    let data = self.tcp_recv.receive(&buffer[0..size]);
                    for message in &data {
                        let str = String::from_utf8_lossy(&message[0..cmp::min(1024, message.len())]);
                        println!("Received full message TCP length {} from {}: {}", message.len(), addr, str);
                    }
                    messages.extend(data.into_iter().map(|data| (Protocol::TCP, addr, data.into())));
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(ServerError::Other(format!("Error TCP receiving from {}: {}", addr, err)))
            }
        }
        if let Some(error) = self.tcp_recv.failed() {
            Err(ServerError::Other(format!("Error in TCP recv stream: {}", error)))
        } else {
            Ok(())
        }
    }

    pub fn update_tcp_send(&mut self) -> ServerResult<()> {
        // send tcp
        let addr = self.tcp_address;
        while let Some(buffer) = self.tcp_send.next_send() {
            match self.stream.write(buffer) {
                Ok(sent) => match sent {
                    0 => return Err(ServerError::Disconnected),
                    _ => {
                        println!("Sent {} TCP bytes", sent);
                        self.tcp_send.update_buffer(sent)
                    }
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => return Err(ServerError::Other(format!("Error TCP writing to {}: {}", addr, err)))
                }
            }
        }
        Ok(())
    }
}
