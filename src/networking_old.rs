use std::{net::SocketAddr, collections::VecDeque, mem::MaybeUninit, cmp};

use socket2::{Socket, Domain, Type, Protocol, SockAddr};
use std::io::Result;

// what we're assuming for MTU size
pub const MAX_UDP_MESSAGE_SIZE: usize = 512;
pub const MAX_TCP_MESSAGE_SIZE: usize = 1<<20; // why would you send more than 1MB? even that's probably too much
pub const MAX_TCP_MESSAGE_QUEUE_SIZE: usize = 1<<26; // max they can ddos me for 640 mb

fn make_udp_socket(port: Option<u16>) -> Result<Socket> {
    let socket: Socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;
    let address: SocketAddr = format!("127.0.0.1:{}",
        match port {Some(port) => port, _ => 0}
    ).parse().unwrap();
    let address = address.into();
    socket.bind(&address)?;

    Ok(socket)
}

fn make_tcp_socket(port: Option<u16>, addr: Option<SockAddr>) -> Result<Socket> {
    let socket: Socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;
    let address: SocketAddr = format!("0.0.0.0:{}",
        match port {Some(port) => port, _ => 0}
    ).parse().unwrap();
    let address = address.into();
    socket.bind(&address)?;
    match addr {
        Some(addr) => match socket.connect(&addr) {
            Ok(()) => Ok(socket),
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => Ok(socket), // this is what will happen
                _ => Err(err),
            }
        },
        None => {
            socket.listen(128)?;
            Ok(socket)
        }
    }
}

struct TcpRecvState {
    buffer: Box<[u8]>,
    length: usize,
    remaining: usize,
    failure: Option<String>
}

impl TcpRecvState {
    pub fn receive(&mut self, data: &[MaybeUninit<u8>]) -> Vec<Vec<u8>> {
        let mut messages: Vec<Vec<u8>> = vec![];

        let mut extension_read_start = 0;
        let extension: &[u8] = unsafe {
            std::mem::transmute(data)
        };

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
                self.failure = Some(format!("Client attempted to send message that was too big: {}", self.remaining));
                break;
            }
            if self.remaining > 0 {
                // copy either remaining message bytes, or all new bytes, whichever is smaller
                let copy_len = cmp::min(self.remaining, extension.len() - extension_read_start);
                self.buffer[self.length..self.length + copy_len]
                    .copy_from_slice(&extension[extension_read_start..]);
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

struct TcpSendState {
    queue: VecDeque<Vec<u8>>,
    queue_size: usize,
    buffer: Box<[u8]>,
    length: usize,
    position: usize
}

impl TcpSendState {
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
                self.length = next.len();
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

pub mod server {
    use std::collections::{HashMap, VecDeque};
    use std::fmt::Display;
    use std::io::Result;
    use std::mem::MaybeUninit;
    use std::time::{SystemTime, Duration};
    use socket2::{Socket};
    use std::net::SocketAddr;

    use super::{make_udp_socket, MAX_UDP_MESSAGE_SIZE, make_tcp_socket, MAX_TCP_MESSAGE_SIZE, TcpRecvState, TcpSendState};

    // the time after the last message after which to declare the connection dead
    const LAST_MESSAGE_DEAD_TIME: Duration = Duration::new(10, 0);

    struct ConnectionInfo {
        address: SocketAddr,
        last_message: SystemTime,
        tcp_socket: Socket,
        tcp_recv: TcpRecvState,
        tcp_send: TcpSendState
    }

    #[derive(PartialEq, Eq, Hash, Clone, Copy)]
    pub struct ConnectionID(i32);

    impl Display for ConnectionID {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    struct ConnectionIDGenerator {
        counter: i32
    }

    impl ConnectionIDGenerator {
        fn new() -> Self {
            Self {
                counter: 0
            }
        }
        fn generate(&mut self) -> ConnectionID {
            self.counter += 1;
            ConnectionID(self.counter - 1)
        }
    }

    pub struct ServerConnection {
        udp_socket: Socket,
        // tcp_socket: Socket,
        connection_data: HashMap<ConnectionID, ConnectionInfo>,
        addr_id_map: HashMap<SocketAddr, ConnectionID>,
        all_connection_ids: Vec<ConnectionID>,
        generator: ConnectionIDGenerator,
        udp_message_queue: Vec<(Vec<ConnectionID>, Vec<u8>)>,
        read_buffer: Box<[MaybeUninit<u8>]>
    }

    impl ServerConnection {
        pub fn new(port_udp: u16, port_tcp: u16) -> Result<ServerConnection> {
            Ok(ServerConnection {
                udp_socket: make_udp_socket(Some(port_udp))?,
                // tcp_socket: make_tcp_socket(Some(port_tcp), None)?,
                connection_data: HashMap::new(),
                addr_id_map: HashMap::new(),
                all_connection_ids: Vec::new(),
                generator: ConnectionIDGenerator::new(),
                udp_message_queue: Vec::new(),
                read_buffer: vec![MaybeUninit::<u8>::uninit()].into_boxed_slice()
            })
        }

        fn new_connection(&mut self, addr: &SocketAddr, tcp_socket: Socket) -> ConnectionID {
            let id = self.generator.generate();
            self.addr_id_map.insert(*addr, id.clone());
            self.connection_data.insert(id.clone(), ConnectionInfo {
                address: *addr,
                last_message: SystemTime::now(),
                tcp_socket,
                tcp_recv: TcpRecvState {
                    buffer: vec![0u8; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
                    length: 0,
                    remaining: 0,
                    failure: None
                },
                tcp_send: TcpSendState {
                    buffer: vec![0; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
                    length: 0,
                    position: 0,
                    queue: VecDeque::new(),
                    queue_size: 0,
                }
            });
            self.all_connection_ids.push(id);
            id
        }

        pub fn kick(&mut self, id: &ConnectionID) -> bool {
            println!("Connection was kicked: {}", id);
            match self.connection_data.remove(id) {
                Some(data) => {
                    self.addr_id_map.remove(&data.address);
                    let mut idx = usize::MAX;
                    for i in 0..self.all_connection_ids.len() {
                        if self.all_connection_ids[i] == *id {
                            idx = i;
                            break;
                        }
                    }
                    if idx != usize::MAX {
                        self.all_connection_ids.remove(idx);
                    }
                    println!("Connection was kicked: {}", id);
                    true
                },
                None => false
            }
        }

        pub fn get_address(&self, id: &ConnectionID) -> Option<SocketAddr> {
            self.connection_data.get(id).map(|data| data.address)
        }
        pub fn all_connection_ids(&self) -> Vec<ConnectionID> {
            self.all_connection_ids.clone()
        }

        pub fn poll(&mut self) -> HashMap<ConnectionID, Vec<Vec<u8>>> {
            // udp polling
            let mut messages: HashMap<ConnectionID, Vec<Vec<u8>>> = HashMap::new();
            loop {
                match self.udp_socket.recv_from(self.read_buffer.as_mut()) {
                    Ok((size, addr)) => {
                        println!("Tentatively received {}", size);
                        let addr: SocketAddr = match addr.as_socket() {
                            Some(addr) => addr,
                            None => {
                                println!("Error understanding connection address");
                                continue
                            }
                        };
                        let result: &[u8] = unsafe {
                            std::mem::transmute(&self.read_buffer[0..size])
                        };
                        println!("Received {} bytes from {:?}: {}", size, addr, String::from_utf8_lossy(&result));
                        let id = match self.addr_id_map.get(&addr) {
                            Some(id) => *id,
                            None => {
                                println!("Rejected UDP packet from unconnected client: {}", addr);
                                continue
                            }
                        };
                        
                        match self.connection_data.get_mut(&id) {
                            Some(data) => data.last_message = SystemTime::now(),
                            None => panic!("Error: connection found in address map but not in data map: {}", id)
                        }
                        match messages.get_mut(&id) {
                            Some(list) => list.push(result.to_vec()),
                            None => {
                                messages.insert(id, vec![result.to_vec()]);
                            }
                        };
                    }
                    Err(v) => {
                        match v.kind() {
                            std::io::ErrorKind::WouldBlock => (),
                            _ => {
                                println!("Error reading UDP message: {}", v);
                            }
                        }
                        break;
                    }
                };
            }

            // // tcp polling
            // // accept new clients
            // loop {
            //     match self.tcp_socket.accept() {
            //         Ok((tcp_socket, addr)) => {
            //             let addr: SocketAddr = match addr.as_socket() {
            //                 Some(addr) => addr,
            //                 None => {
            //                     println!("Error understanding connection address");
            //                     continue
            //                 }
            //             };
            //             self.new_connection(&addr, tcp_socket);
            //         },
            //         Err(v) => match v.kind() {
            //             std::io::ErrorKind::WouldBlock => break,
            //             _ => {
            //                 println!("Error reading TCP message: {}", v);
            //             }
            //         }
            //     }
            // }

            // // listen to existing clients on tcp sockets
            // let mut to_kick = vec![];
            // for (id, info) in &mut self.connection_data {
            //     let mut should_kick = false;
            //     // try to get all of the data
            //     while !should_kick {
            //         match info.tcp_socket.recv(self.read_buffer.as_mut()) {
            //             Ok(size) => if size == 0 {
            //                 // no data left
            //                 break;
            //             } else {
            //                 let msg = &mut info.tcp_recv;
            //                 // process the packets out of the socket
            //                 let mut packets = msg.receive(&self.read_buffer[0..size]);
            //                 if !packets.is_empty() {
            //                     match messages.get_mut(id) {
            //                         Some(list) => list.append(&mut packets),
            //                         None => {
            //                             messages.insert(*id, packets);
            //                         }
            //                     };
            //                 }
            //                 if let Some(err) = &msg.failure {
            //                     should_kick = true;
            //                     println!("Kicking client: {}", err);
            //                 }
            //             },
            //             Err(v) => match v.kind() {
            //                 std::io::ErrorKind::WouldBlock => break,
            //                 _ => {
            //                     println!("Error reading TCP message: {}", v);
            //                     // an error will desync the data stream, so we need the connection to be reset
            //                     should_kick = true;
            //                     break;
            //                 }
            //             }
            //         }
            //     }
            //     if should_kick {
            //         to_kick.push(*id);
            //     }
            // }
            // for id in to_kick {
            //     self.kick(&id);
            // }
            messages
        }

        pub fn flush(&mut self) {
            // udp sending
            let mut retry: Vec<(Vec<ConnectionID>, Vec<u8>)> = Vec::new();
            let mut send_to = |id: &ConnectionID, data: &Vec<u8>| -> bool { // returns whether to retry
                let info = match self.connection_data.get_mut(&id) {
                    Some(data) => data,
                    None => {
                        println!("Error sending data to unknown client (id={})", id);
                        return false;
                    }
                };
                let addr = info.address.into();
                let written = self.udp_socket.send_to(data.as_slice(), &addr);
                match written {
                    Ok(written) => {
                        if written != data.len() {
                            // for now we don't handle the big data case, because UDP packets are small. it's probably fine
                            println!("Error writing {} bytes to {}: {} < {}", data.len(), info.address, written, data.len());
                        }
                        false
                    },
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::WouldBlock => true,
                            _ => {
                                println!("Error writing UDP {} bytes to {} ({}): {}", data.len(), info.address, id, e);
                                return true;
                            }
                        }
                    }
                }
            };
            for (ids, data) in self.udp_message_queue.drain(0..self.udp_message_queue.len()) {
                let mut retry_list = vec![];
                for id in ids {
                    let retry = send_to(&id, &data);
                    if retry {
                        retry_list.push(id);
                    }
                }
                if retry_list.len() > 0 {
                    retry.push((retry_list, data)); // retry all clients it failed to send to
                }
            }
            self.udp_message_queue.append(&mut retry);

            // // tcp sending
            // for (id, info) in &mut self.connection_data {
            //     let data = &mut info.tcp_send;
            //     loop {
            //         // send as many bytes of previous unsent message
            //         if let Some(buffer) = data.next_send() {
            //             match info.tcp_socket.send(buffer) {
            //                 Ok(sent) => {
            //                     data.update_buffer(sent);
            //                 },
            //                 Err(v) => match v.kind() {
            //                     std::io::ErrorKind::WouldBlock => break,
            //                     _ => {
            //                         println!("Error writing TCP {} bytes to {} ({}): {}",
            //                             data.length - data.position,
            //                             info.address,
            //                             id,
            //                             v);
            //                         break;
            //                     }
            //                 }
            //             }
            //         }
            //     }
            // }
        }

        pub fn send_udp(&mut self, receivers: Vec<ConnectionID>, data: Vec<u8>) {
            assert!(data.len() <= MAX_UDP_MESSAGE_SIZE);
            self.udp_message_queue.push((receivers, data));
        }

        pub fn send_tcp(&mut self, receivers: Vec<ConnectionID>, data: &Vec<u8>) {
            assert!(data.len() <= MAX_TCP_MESSAGE_SIZE);
            // eventually: reuse the same data to send to each receiver
            let mut to_kick = vec![];
            for id in receivers {
                let info = match self.connection_data.get_mut(&id) {
                    Some(info) => info,
                    None => {
                        println!("Attempt to send TCP data to non-existent client {}", id);
                        break
                    }
                };
                match info.tcp_send.enqueue(data.clone()) {
                    Ok(()) => (),
                    Err(err) => {
                        println!("Kicking client: {}", err);
                        to_kick.push(id);
                    }
                }
            }
            for id in to_kick {
                self.kick(&id);
            }
        }

        pub fn send_udp_all(&mut self, receivers: Vec<ConnectionID>, data: Vec<Vec<u8>>) {
            self.udp_message_queue.extend(data.into_iter().map(|data| {
                assert!(data.len() <= MAX_UDP_MESSAGE_SIZE);
                (receivers.clone(), data)
            }));
        }

        pub fn prune_dead_connections(&mut self, check_against: SystemTime) -> Vec<ConnectionID> {
            let mut kick = vec![];
            for con_id in &self.all_connection_ids {
                let con_id = con_id.clone();
                match self.last_message_time(&con_id) {
                    Some(time) => {
                        match check_against.duration_since(time) {
                            Ok(difference) => {
                                if difference >= LAST_MESSAGE_DEAD_TIME {
                                    kick.push(con_id);
                                }
                            },
                            _ => ()
                        }
                    },
                    None => ()
                }
            };
            let mut kicked = vec![];
            for con_id in kick {
                match self.kick(&con_id) {
                    true => kicked.push(con_id.clone()),
                    _ => ()
                }
            }
            kicked
        }
        pub fn last_message_time(&self, connection: &ConnectionID) -> Option<SystemTime> {
            match self.connection_data.get(&connection) {
                Some(data) => Some(data.last_message),
                None => None
            }
        }
    }
}

pub mod client {
    use std::{io::Result, collections::VecDeque};
    use std::mem::MaybeUninit;
    use socket2::{Socket, SockAddr};
    use std::net::SocketAddr;
    use std::io::{ErrorKind, Error};

    use super::{make_udp_socket, MAX_UDP_MESSAGE_SIZE, TcpSendState, MAX_TCP_MESSAGE_SIZE, TcpRecvState, make_tcp_socket};

    #[derive(Clone, Eq, PartialEq)]
    pub enum TcpConnectionStatus {
        NotConnected,
        Connecting,
        Connected
    }

    pub struct Connection {
        udp_socket: Option<Socket>,
        tcp_socket: Option<Socket>,
        tcp_connection_status: TcpConnectionStatus,
        tcp_send: TcpSendState,
        tcp_recv: TcpRecvState,
        server_address_udp: SockAddr,
        server_address_socket_udp: SocketAddr,
        server_address_tcp: SockAddr,
        server_address_socket_tcp: SocketAddr,
        message_queue: Vec<Vec<u8>>,
        read_buffer: Box<[MaybeUninit<u8>]>
    }

    impl Connection {
        pub fn new(server_address_udp: &SocketAddr, server_address_tcp: &SocketAddr) -> (Connection, Option<Error>) {
            let mut connection = Connection {
                server_address_udp: (*server_address_udp).into(),
                server_address_socket_udp: *server_address_udp,
                server_address_tcp: (*server_address_tcp).into(),
                server_address_socket_tcp: *server_address_tcp,
                message_queue: Vec::new(),
                udp_socket: None,
                tcp_socket: None,
                tcp_connection_status: TcpConnectionStatus::NotConnected,
                tcp_send: TcpSendState {
                    queue: VecDeque::new(),
                    queue_size: 0,
                    buffer: vec![0; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
                    length: 0,
                    position: 0
                },
                tcp_recv: TcpRecvState {
                    buffer: vec![0; MAX_TCP_MESSAGE_SIZE].into_boxed_slice(),
                    length: 0,
                    remaining: 0,
                    failure: None
                },
                read_buffer: vec![MaybeUninit::zeroed(); 1<<20].into_boxed_slice()
            };
            let error = match connection.connect() {
                Ok(()) => None,
                Err(e) => Some(e)
            };
            (connection, error)
        }

        pub fn connect(&mut self) -> Result<()> {
            match (&self.udp_socket, &self.tcp_socket) {
                (Some(_), _) |
                (_, Some(_)) => {
                    self.disconnect();
                },
                _ => ()
            }
            let mut error = None;
            (self.udp_socket, self.tcp_socket) = match (make_udp_socket(None), make_tcp_socket(None, Some(self.server_address_socket_tcp.into()))) {
                (Ok(udp_socket), Ok(tcp_socket)) => (Some(udp_socket), Some(tcp_socket)),
                (Err(e1), _) => {
                    error = Some(e1);
                    (None, None)
                },
                (_, Err(e2)) => {
                    error = Some(e2);
                    (None, None)
                }
            };
            match error {
                None => {
                    self.tcp_connection_status = TcpConnectionStatus::Connecting;
                    Ok(())
                },
                Some(e1) => Err(e1)
            }
        }

        pub fn disconnect(&mut self) {
            println!("Disconnecting from server");
            (self.udp_socket, self.tcp_socket) = (None, None);
        }

        pub fn get_server_address_udp(&self) -> (SocketAddr, SocketAddr) {
            (self.server_address_socket_udp, self.server_address_socket_tcp)
        }

        pub fn set_server_address(&mut self, addr_udp: &SocketAddr, addr_tcp: &SocketAddr) {
            self.server_address_socket_udp = addr_udp.clone();
            self.server_address_udp = (*addr_udp).into();
            self.server_address_socket_tcp = addr_tcp.clone();
            self.server_address_tcp = (*addr_tcp).into();
        }

        pub fn poll(&mut self) -> Vec<Vec<u8>> {
            match self.tcp_connection_status {
                TcpConnectionStatus::NotConnected => return vec![],
                TcpConnectionStatus::Connecting => {
                    // spam connect until it gives an error IsConn
                    if let Some(tcp_socket) = &mut self.tcp_socket {
                        match tcp_socket.connect(&self.server_address_tcp) {
                            Ok(()) => {
                                println!("Error, confusing connection OK status");
                                self.disconnect();
                                return vec![];
                            },
                            Err(err) => {
                                match (err.kind(), err.raw_os_error()) {
                                    (_, Some(10056)) => {
                                        // connection works
                                        println!("Connection works error");
                                        self.tcp_connection_status = TcpConnectionStatus::Connected;
                                    },
                                    (std::io::ErrorKind::WouldBlock, _) => {
                                        // connection in progress
                                        println!("{}", err);
                                        return vec![];
                                    },
                                    (_, Some(10037)) => {
                                        // connection in progress
                                        println!("{}", err);
                                        return vec![];
                                    },
                                    _ => {
                                        panic!("What is this error: {}", err);
                                    },
                                }
                            },
                        }
                    } else {
                        self.tcp_connection_status = TcpConnectionStatus::NotConnected;
                        return vec![];
                    }
                },
                TcpConnectionStatus::Connected => (),
            }
            let mut messages: Vec<Vec<u8>> = Vec::new();
            if let (Some(udp_socket), Some(tcp_socket)) = (&self.udp_socket, &self.tcp_socket) {
                // get udp packets
                loop {
                    match udp_socket.recv_from(self.read_buffer.as_mut()) {
                        Ok((size, addr)) => {
                            let addr: SocketAddr = match addr.as_socket() {
                                Some(addr) => addr,
                                None => {
                                    println!("Error understanding connection address");
                                    continue
                                }
                            };
                            if addr != self.server_address_socket_tcp {
                                println!("Error: received packet from non-server address");
                                continue;
                            }
                            let result: Vec<u8> = unsafe {
                                let temp: &[u8; MAX_UDP_MESSAGE_SIZE] = std::mem::transmute(&self.read_buffer);
                                &temp[0..size]
                            }.to_vec();
                            println!("Received {} bytes from {:?}: {}", size, addr, String::from_utf8_lossy(&result));
                            messages.push(result);
                        }
                        Err(v) => {
                            match v.kind() {
                                ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error reading UDP message: {}", v);
                                }
                            }
                            break;
                        }
                    };
                }
                // get tcp packets
                let mut quit = None;
                loop {
                    match tcp_socket.recv(self.read_buffer.as_mut()) {
                        Ok(size) => if size == 0 {
                            break;
                        } else {
                            let mut packets = self.tcp_recv.receive(&self.read_buffer[0..size]);
                            messages.append(&mut packets);
                            if let Some(err) = &self.tcp_recv.failure {
                                quit = Some(err.clone());
                                break;
                            }
                        },
                        Err(v) => {
                            match v.kind() {
                                ErrorKind::WouldBlock => break,
                                _ => {
                                    quit = Some(v.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                if let Some(err) = quit {
                    println!("Error on TCP stream: {}", err);
                    self.disconnect();
                }
            }
            messages
        }

        pub fn status(&self) -> TcpConnectionStatus {
            self.tcp_connection_status.clone()
        }

        pub fn flush(&mut self) {
            if self.tcp_connection_status != TcpConnectionStatus::Connected {
                return
            }
            let mut failed: Vec<Vec<u8>> = Vec::new();
            if let (Some(udp_socket), Some(tcp_socket)) = (&mut self.udp_socket, &mut self.tcp_socket) {
                for data in self.message_queue.drain(0..self.message_queue.len()) {
                    let addr = &self.server_address_tcp;
                    let written = udp_socket.send_to(data.as_slice(), addr);
                    match written {
                        Ok(written) => {
                            if written != data.len() {
                                println!("Error writing {} bytes to server: {} < {}", data.len(), written, data.len());
                            }
                        },
                        Err(e) => {
                            match e.kind() {
                                ErrorKind::WouldBlock => {
                                    failed.push(data);
                                },
                                _ => {
                                    println!("Error writing {} bytes to server: {}", data.len(), e);
                                    failed.push(data);
                                    continue;
                                }
                            }
                        }
                    }
                }
                self.message_queue.append(&mut failed);

                // send tcp messages
                let mut quit = None;
                loop {
                    if let Some(buffer) = self.tcp_send.next_send() {
                        match tcp_socket.send(buffer) {
                            Ok(sent) => {
                                self.tcp_send.update_buffer(sent);
                            },
                            Err(err) => {
                                quit = Some(err);
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                if let Some(err) = quit {
                    println!("Error on TCP send: {}", err);
                    self.disconnect();
                }
            }
        }

        pub fn send_udp(&mut self, data: Vec<u8>) {
            if data.len() > MAX_UDP_MESSAGE_SIZE {
                panic!("Client attempted to send packet larger than UDP packet size: {} > {}", data.len(), MAX_UDP_MESSAGE_SIZE);
            }
            self.message_queue.push(data);
        }

        pub fn send_tcp(&mut self, data: Vec<u8>) {
            self.tcp_send.enqueue(data).unwrap();
        }
    }
}
