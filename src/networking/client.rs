use std::{net::{TcpStream, UdpSocket, SocketAddr}, collections::VecDeque, sync::mpsc::{TryRecvError, channel, Receiver}, io::{ErrorKind, Read, Write}, cmp, thread, fmt::Display};

// where we're at right now is we need to finish changing from messages to ClientUpdate
use crate::networking::{AddressPair, tcp_buffering::{TcpSendState, TcpRecvState}, config::{RECV_BUFFER_SIZE, CONNECT_TIMEOUT}};

use super::{tcp_buffering, config::{MAX_UDP_MESSAGE_SIZE, MAX_TCP_MESSAGE_SIZE}, common::udp_recv_all, Protocol};

// maximum number of network commands to process for each type of processing in one cycle
// note the types are TCP send, TCP recv, UDP send, UDP recv
const MAX_PACKETS_PROCESS: usize = 256;

#[derive(Debug)]
pub enum ClientError {
    NoConnection,
    DiscardedConnection,
    FailedConnection(String),
    BadCommand(String),
    Other(String)
}

impl From<ClientError> for String {
    fn from(e: ClientError) -> Self {
        format!("{}", e)
    }
}

#[derive(Debug)]
pub enum ClientUpdate {
    Connected,
    Disconnected(Option<ClientError>),
    PreventedReconnection(Option<ClientError>),
    Log(String),
    Error(ClientError),
    Message(Protocol, Box<[u8]>)
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Display for ClientUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

type InternalResult = Result<(), Option<ClientError>>;
pub type ClientMultiResult = Result<Vec<ClientUpdate>, Vec<ClientUpdate>>;

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
    pub fn send_data<T>(&mut self, protocol: Protocol, packet: T) -> Result<(), ClientError>
    where
        T: Into<Box<[u8]>>
    {
        if let Some(con) = &mut self.connection {
            let data: Box<[u8]> = packet.into();
            match protocol {
                Protocol::UDP => {
                    if data.len() > MAX_UDP_MESSAGE_SIZE {
                        return Err(ClientError::BadCommand(
                            format!("Attempted to send UDP message that was too big: {} > {}",
                                    data.len(),
                                    MAX_UDP_MESSAGE_SIZE)));
                    }
                    con.udp_message_queue.push_back(data);
                    Ok(())
                }, Protocol::TCP => {
                    if data.len() > MAX_TCP_MESSAGE_SIZE {
                        return Err(ClientError::BadCommand(
                            format!("Attempted to send TCP message that was too big: {} > {}",
                                    data.len(),
                                    MAX_TCP_MESSAGE_SIZE)));
                    }
                    match con.tcp_send.enqueue(data) {
                        Ok(()) => Ok(()),
                        Err(msg) => Err(ClientError::Other(msg))
                    }
                }
            }
        } else {
            Err(ClientError::NoConnection)
        }
    }

    pub fn disconnect(&mut self, error: Option<ClientError>) -> Option<ClientUpdate> {
        let mut update = None;
        if self.connection.is_some() {
            update = Some(ClientUpdate::Disconnected(error));
        }
        else if self.next_attempt.is_some() {
            update = Some(ClientUpdate::PreventedReconnection(error));
        }
        self.connection = None;
        self.connecting = None;
        self.next_attempt = None;
        update
    }

    fn update_udp_recv(&mut self, updates: &mut Vec<ClientUpdate>) -> InternalResult {
        if let Some(con) = &mut self.connection {
            let (recv, err) = udp_recv_all(&con.udp, con.recv_buffer.as_mut(), Some(MAX_PACKETS_PROCESS));
            for (addr, recvd) in recv {
                for message in recvd {
                    updates.push(ClientUpdate::Log(format!("Received UDP from {:?}: {}", addr, String::from_utf8_lossy(message.as_ref()))));
                    updates.push(ClientUpdate::Message(Protocol::UDP, message));
                }
            }
            match err {
                Some(err) => match err.kind() {
                    ErrorKind::WouldBlock => Ok(()),
                    _ => Err(Some(ClientError::Other(format!("Error receiving UDP: {}", err))))
                },
                None => Ok(())
            }
        } else {
            Err(Some(ClientError::NoConnection))
        }
    }

    fn update_udp_send(&mut self, updates: &mut Vec<ClientUpdate>) -> InternalResult {
        if let Some(con) = &mut self.connection {
            let mut processed = 0;
            while let Some(message) = con.udp_message_queue.pop_front() {
                match con.udp.send_to(message.as_ref(), &con.remote_addr_udp) {
                    Ok(sent) => updates.push(ClientUpdate::Log(format!("Sent UDP {} bytes", sent))),
                    Err(err) => {
                        con.udp_message_queue.push_front(message);
                        return match err.kind() {
                            ErrorKind::WouldBlock => Ok(()),
                            _ => Err(Some(ClientError::Other(format!("Error sending UDP: {}", err))))
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
            Err(Some(ClientError::NoConnection))
        }
    }

    fn update_tcp_recv(&mut self, updates: &mut Vec<ClientUpdate>) -> InternalResult {
        if let Some(con) = &mut self.connection {
            let tcp = &mut con.tcp;
            let mut approx_packets = 0;
            while approx_packets < MAX_PACKETS_PROCESS { // limit how much time we spend receiving
                match tcp.read(con.recv_buffer.as_mut()) {
                    Ok(size) => match size {
                        0 => {
                            if let Some(update) = self.disconnect(Some(ClientError::Other("Remote closed TCP connection".to_string()))) {
                                updates.push(update);
                            }
                            return Err(None)
                        },
                        _ => {
                            updates.push(ClientUpdate::Log(format!("Received TCP bytes length: {}", size)));
                            for data in con.tcp_recv.receive(&con.recv_buffer[0..size]) {
                                let str = String::from_utf8_lossy(&data[0..cmp::min(data.len(), 1024)]);
                                updates.push(ClientUpdate::Log(format!("Received full message TCP length {} from {}: {}", data.len(), con.remote_addr_tcp, str)));
                                updates.push(ClientUpdate::Message(Protocol::TCP, data));
                            }
                            approx_packets += 1 + (size - 1) / 1024;
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => break,
                        _ => {
                            if let Some(update) = self.disconnect(Some(ClientError::Other(format!("Error receiving TCP: {}", err)))) {
                                updates.push(update);
                            }
                            return Err(None)
                        }
                    }
                }
            }
            if let Some(error) = con.tcp_recv.failed() {
                if let Some(update) = self.disconnect(Some(ClientError::Other(format!("Error in TCP recv stream: {}", error)))) {
                    updates.push(update);
                }
                Err(None)
            } else {
                Ok(())
            }
        } else {
            Err(Some(ClientError::NoConnection))
        }
    }

    fn update_tcp_send(&mut self, updates: &mut Vec<ClientUpdate>) -> InternalResult {
        if let Some(con) = &mut self.connection {
            // tcp stuff
            let mut processed = 0;
            while let Some(buffer) = con.tcp_send.next_send() {
                match con.tcp.write(buffer) {
                    Ok(sent) => match sent {
                        0 => break,
                        _ => {
                            con.tcp_send.update_buffer(sent);
                            updates.push(ClientUpdate::Log(format!("Sent {} TCP bytes", sent)));
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
                            if let Some(update) = self.disconnect(Some(ClientError::Other(format!("Error writing TCP to {}: {}", addr, err)))) {
                                updates.push(update);
                            }
                            return Err(None);
                        }
                    }
                }
            }
            Ok(())
        } else {
            Err(Some(ClientError::NoConnection))
        }
    }

    pub fn update(&mut self) -> Vec<ClientUpdate> {
        let mut updates = vec![];
        if let Some(update) = self.check_connection() {
            updates.push(update);
        }
        if self.is_connected() {
            match (|| -> InternalResult {
                self.update_udp_recv(&mut updates)?;
                self.update_udp_send(&mut updates)?;
                self.update_tcp_recv(&mut updates)?;
                self.update_tcp_send(&mut updates)?;
                Ok(())
            })() {
                Ok(()) => (),
                Err(err) => if let Some(err) = err {
                    updates.push(ClientUpdate::Error(err))
                }
            };
        }
        updates
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

    fn check_connection(&mut self) -> Option<ClientUpdate> {
        if !self.is_connected() {
            if let Some((addr, rx)) = &self.connecting {
                match rx.try_recv() {
                    Ok(Ok(tcp)) => {
                        match UdpSocket::bind("0.0.0.0:0") {
                            Ok(udp) => {
                                // finish the connection
                                if let Some(new_addr) = &self.next_attempt {
                                    self.connecting = Some(Self::start_connecting(new_addr));
                                    Some(ClientUpdate::Error(ClientError::DiscardedConnection))
                                } else {
                                    let addr = *addr;
                                    self.connecting = None;
                                    match (udp.set_nonblocking(true), tcp.set_nonblocking(true)) {
                                        (Ok(_), Ok(_)) => {
                                            self.finish_connecting(&addr, tcp, udp);
                                            Some(ClientUpdate::Connected)
                                        },
                                        (Ok(_), Err(err)) => {
                                            self.connecting = None;
                                            Some(ClientUpdate::Error(
                                                    ClientError::FailedConnection(
                                                        format!("Error setting TCP stream to nonblocking: {}", err))))
                                        }
                                        (Err(err), _) => {
                                            self.connecting = None;
                                            Some(ClientUpdate::Error(
                                                    ClientError::FailedConnection(
                                                        format!("Error setting TCP stream to nonblocking: {}", err))))
                                        }
                                    }
                                }
                            },
                            Err(err) => {
                                self.connecting = None;
                                Some(ClientUpdate::Error(
                                        ClientError::FailedConnection(
                                            format!("Error opening UDP socket at 0.0.0.0:0: {}", err))))
                            }
                        }
                    },
                    Ok(Err(err)) => {
                        let addr = addr.tcp.clone();
                        self.connecting = None;
                        Some(ClientUpdate::Error(
                                ClientError::FailedConnection(
                                    format!("Error connecting TCP to {}: {}", addr, err))))
                    },
                    Err(TryRecvError::Disconnected) => {
                        self.connecting = None;
                        Some(ClientUpdate::Error(
                                ClientError::FailedConnection(
                                    format!("TCP connect thread disconnected unexpectedly"))))
                    },
                    Err(TryRecvError::Empty) => None
                }
            } else if let Some(addr) = &self.next_attempt {
                self.connecting = Some(Self::start_connecting(addr));
                None
            } else {
                None
            }
        } else {
            (self.connecting, self.next_attempt) = (None, None);
            None
        }
    }
}

