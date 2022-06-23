use std::{net::{TcpStream, UdpSocket, SocketAddr}, collections::VecDeque, sync::mpsc::{TryRecvError, channel, Receiver}, io::{ErrorKind, Read, Write}, cmp, thread, fmt::Display};

use crate::{networking::{AddressPair, tcp_buffering::{TcpSendState, TcpRecvState}, config::{RECV_BUFFER_SIZE, CONNECT_TIMEOUT}}, model::commands::SerializedServerCommand};

use super::{tcp_buffering, config::MAX_UDP_MESSAGE_SIZE, common::udp_recv_all};

// maximum number of network commands to process for each type of processing in one cycle
// note the types are TCP send, TCP recv, UDP send, UDP recv
const MAX_PACKETS_PROCESS: usize = 256;

#[derive(Debug)]
pub enum ClientError {
    NoConnection,
    Disconnected,
    Other(String)
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

struct Connection {
    pub tcp: TcpStream,
    pub udp: UdpSocket,
    pub remote_addr_tcp: SocketAddr,
    pub remote_addr_udp: SocketAddr,
    pub udp_message_queue: VecDeque<Box<[u8]>>,
    pub tcp_send: tcp_buffering::TcpSendState,
    pub tcp_recv: tcp_buffering::TcpRecvState,
    pub recv_buffer: Box<[u8]>,
}

type Connecting = (AddressPair, Receiver<std::io::Result<TcpStream>>);
pub struct Client {
    connection: Option<Connection>,
    connecting: Option<Connecting>,
    next_attempt: Option<AddressPair>
}

impl Client {
    pub fn send_udp<T>(&mut self, packet: T) -> ClientResult<()>
    where
        T: Into<SerializedServerCommand>
    {
        if let Some(con) = &mut self.connection {
            let SerializedServerCommand(data) = packet.into();
            if data.len() > MAX_UDP_MESSAGE_SIZE {
                println!("Attempted to send UDP message that was too big: {} > {}", data.len(), MAX_UDP_MESSAGE_SIZE);
                return Ok(());
            }
            con.udp_message_queue.push_back(data);
            Ok(())
        } else {
            Err(ClientError::NoConnection)
        }
    }

    pub fn send_tcp<T>(&mut self, message: T) -> ClientResult<()>
    where
        T: Into<SerializedServerCommand>
    {
        if let Some(con) = &mut self.connection {
            let SerializedServerCommand(data) = message.into();
            match con.tcp_send.enqueue(data) {
                Ok(()) => Ok(()),
                Err(msg) => Err(ClientError::Other(msg))
            }
        } else {
            Err(ClientError::NoConnection)
        }
    }

    pub fn disconnect(&mut self) {
        if self.connection.is_some() {
            println!("Disconnected from server");
        }
        if self.connecting.is_some() || self.next_attempt.is_some() {
            println!("Preventing reconnection");
        }
        self.connection = None;
        self.connecting = None;
        self.next_attempt = None;
    }

    fn update_udp_recv(&mut self, messages: &mut Vec<Box<[u8]>>) -> ClientResult<()> {
        if let Some(con) = &mut self.connection {
            let (recv, err) = udp_recv_all(&con.udp, con.recv_buffer.as_mut(), Some(MAX_PACKETS_PROCESS));
            for (addr, recvd) in recv {
                for message in recvd {
                    println!("Received UDP from {:?}: {}", addr, String::from_utf8_lossy(message.as_ref()));
                    messages.push(message);
                }
            }
            match err {
                Some(err) => match err.kind() {
                    ErrorKind::WouldBlock => Ok(()),
                    _ => Err(ClientError::Other(format!("Error receiving UDP: {}", err)))
                },
                None => Ok(())
            }
        } else {
            Err(ClientError::NoConnection)
        }
    }

    fn update_udp_send(&mut self) -> ClientResult<()> {
        if let Some(con) = &mut self.connection {
            let mut processed = 0;
            while let Some(message) = con.udp_message_queue.pop_front() {
                match con.udp.send_to(message.as_ref(), &con.remote_addr_udp) {
                    Ok(sent) => println!("Sent UDP {} bytes", sent),
                    Err(err) => {
                        con.udp_message_queue.push_front(message);
                        return match err.kind() {
                            ErrorKind::WouldBlock => Ok(()),
                            _ => Err(ClientError::Other(format!("Error sending UDP: {}", err)))
                        }
                    }
                }
                processed += 1;
                if processed >= MAX_PACKETS_PROCESS {
                    break;
                }
            }
            Ok(())
        } else {
            Err(ClientError::NoConnection)
        }
    }

    fn update_tcp_recv(&mut self, messages: &mut Vec<Box<[u8]>>) -> ClientResult<()> {
        if let Some(con) = &mut self.connection {
            let tcp = &mut con.tcp;
            let mut approx_packets = 0;
            while approx_packets < MAX_PACKETS_PROCESS { // limit how much time we spend receiving
                match tcp.read(con.recv_buffer.as_mut()) {
                    Ok(size) => match size {
                        0 => return Err(ClientError::Disconnected),
                        _ => {
                            println!("Received TCP bytes length: {}", size);
                            for data in con.tcp_recv.receive(&con.recv_buffer[0..size]) {
                                let str = String::from_utf8_lossy(&data[0..cmp::min(data.len(), 1024)]);
                                println!("Received full message TCP length {} from {}: {}", data.len(), con.remote_addr_tcp, str);
                                messages.push(data);
                            }
                            approx_packets += 1 + (size - 1) / 1024;
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => break,
                        _ => {
                            self.disconnect();
                            return Err(ClientError::Other(format!("Error receiving TCP: {}", err)))
                        }
                    }
                }
            }
            if let Some(error) = con.tcp_recv.failed() {
                self.disconnect();
                Err(ClientError::Other(format!("Error in TCP recv stream: {}", error)))
            } else {
                Ok(())
            }
        } else {
            Err(ClientError::NoConnection)
        }
    }

    fn update_tcp_send(&mut self) -> ClientResult<()> {
        if let Some(con) = &mut self.connection {
            // tcp stuff
            let mut processed = 0;
            while let Some(buffer) = con.tcp_send.next_send() {
                match con.tcp.write(buffer) {
                    Ok(sent) => match sent {
                        0 => break,
                        _ => {
                            con.tcp_send.update_buffer(sent);
                            println!("Sent {} TCP bytes", sent);
                            processed += 1 + (sent - 1) / 1024;
                            if processed >= MAX_PACKETS_PROCESS {
                                break;
                            }
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => break,
                        _ => {
                            let addr = con.remote_addr_tcp;
                            self.disconnect();
                            return Err(ClientError::Other(format!("Error writing TCP to {}: {}", addr, err)));
                        }
                    }
                }
            }
            Ok(())
        } else {
            Err(ClientError::NoConnection)
        }
    }

    pub fn update(&mut self) -> Vec<Box<[u8]>> {
        self.check_connection();
        let mut messages = vec![];
        match (|| -> ClientResult<()> {
            self.update_udp_recv(&mut messages)?;
            self.update_udp_send()?;
            self.update_tcp_recv(&mut messages)?;
            self.update_tcp_send()?;
            Ok(())
        })() {
            Ok(()) => (),
            Err(err) => match err {
                ClientError::NoConnection => (),
                _ => println!("Client error: {}", err)
            }
        }
        messages
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    pub fn init_disconnected() -> Client {
        Client {
            connection: None,
            connecting: None,
            next_attempt: None
        }
    }

    fn start_connecting(addr: &AddressPair) -> Connecting {
        let connect_addr = addr.tcp;
        let (tx, rx) = channel();
        let _ = thread::spawn(move || {
            tx.send(TcpStream::connect_timeout(&connect_addr, CONNECT_TIMEOUT))
        });
        (*addr, rx)
    }

    pub fn connect(&mut self, remote_addr_udp: SocketAddr, remote_addr_tcp: SocketAddr) {
        let addr = AddressPair {
            udp: remote_addr_udp,
            tcp: remote_addr_tcp
        };
        if self.connecting.is_some() {
            self.next_attempt = Some(addr);
        } else {
            self.connecting = Some(Self::start_connecting(&addr));
        }
    }

    fn finish_connecting(&mut self, addr: &AddressPair, tcp: TcpStream, udp: UdpSocket) {
        self.connection = Some(Connection {
            tcp,
            udp,
            remote_addr_udp: addr.udp,
            remote_addr_tcp: addr.tcp,
            udp_message_queue: VecDeque::new(),
            tcp_send: TcpSendState::init(),
            tcp_recv: TcpRecvState::init(),
            recv_buffer: vec![0u8; RECV_BUFFER_SIZE].into_boxed_slice()
        });
    }

    fn check_connection(&mut self) {
        if !self.is_connected() {
            if let Some((addr, rx)) = &self.connecting {
                match rx.try_recv() {
                    Ok(Ok(tcp)) => {
                        match UdpSocket::bind("0.0.0.0:0") {
                            Ok(udp) => {
                                // finish the connection
                                if let Some(new_addr) = &self.next_attempt {
                                    println!("Connection discarded: remote {}, new connection attempt is: remote {}", addr, new_addr);
                                    self.connecting = Some(Self::start_connecting(new_addr));
                                } else {
                                    println!("Connected on TCP to {}", addr);
                                    let addr = *addr;
                                    self.connecting = None;
                                    match (udp.set_nonblocking(true), tcp.set_nonblocking(true)) {
                                        (Ok(_), Ok(_)) => {
                                            self.finish_connecting(&addr, tcp, udp);
                                        },
                                        (Ok(_), Err(err)) => {
                                            println!("Error setting TCP stream to nonblocking: {}", err);
                                        }
                                        (Err(err), _) => {
                                            println!("Error setting UDP stream to nonblocking: {}", err);
                                        }
                                    }
                                }
                            },
                            Err(err) => {
                                println!("Error opening UDP socket at 0.0.0.0:0: {}", err);
                                self.connecting = None;
                            }
                        }
                    },
                    Ok(Err(err)) => {
                        println!("Error connecting TCP to {}: {}", addr.tcp, err);
                        self.connecting = None;
                    },
                    Err(TryRecvError::Disconnected) => {
                        println!("TCP connect thread disconnected unexpectedly");
                        self.connecting = None;
                    },
                    Err(TryRecvError::Empty) => ()
                }
            } else if let Some(addr) = &self.next_attempt {
                self.connecting = Some(Self::start_connecting(addr));
            }
        } else {
            (self.connecting, self.next_attempt) = (None, None);
        }
    }
}

