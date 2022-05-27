use std::{net::SocketAddr, collections::VecDeque};

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
    let address: SocketAddr = format!("127.0.0.1:{}",
        match port {Some(port) => port, _ => 0}
    ).parse().unwrap();
    let address = address.into();
    socket.bind(&address)?;
    socket.listen(128)?;
    match addr {
        Some(addr) => socket.connect(&addr)?,
        None => ()
    };

    Ok(socket)
}

struct TcpRecvState {
    buffer: [u8; MAX_TCP_MESSAGE_SIZE],
    length: usize,
    remaining: usize,
}

struct TcpSendState {
    queue: VecDeque<Vec<u8>>,
    queue_size: usize,
    buffer: [u8; MAX_TCP_MESSAGE_SIZE],
    length: usize,
    position: usize
}

pub mod server {
    use std::cmp;
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
        tcp_socket: Socket,
        connection_data: HashMap<ConnectionID, ConnectionInfo>,
        addr_id_map: HashMap<SocketAddr, ConnectionID>,
        all_connection_ids: Vec<ConnectionID>,
        generator: ConnectionIDGenerator,
        udp_message_queue: Vec<(Vec<ConnectionID>, Vec<u8>)>,
        read_buffer: [MaybeUninit<u8>; 1<<20]
    }

    impl ServerConnection {
        pub fn new(port: u16) -> Result<ServerConnection> {
            Ok(ServerConnection {
                udp_socket: make_udp_socket(Some(port))?,
                tcp_socket: make_tcp_socket(Some(port), None)?,
                connection_data: HashMap::new(),
                addr_id_map: HashMap::new(),
                all_connection_ids: Vec::new(),
                generator: ConnectionIDGenerator::new(),
                udp_message_queue: Vec::new(),
                read_buffer: [MaybeUninit::<u8>::uninit(); 1<<20]
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
                    buffer: [0; MAX_TCP_MESSAGE_SIZE],
                    length: 0,
                    remaining: 0
                },
                tcp_send: TcpSendState {
                    buffer: [0; MAX_TCP_MESSAGE_SIZE],
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
                match self.udp_socket.recv_from(&mut self.read_buffer) {
                    Ok((size, addr)) => {
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

            // tcp polling
            // accept new clients
            loop {
                match self.tcp_socket.accept() {
                    Ok((tcp_socket, addr)) => {
                        let addr: SocketAddr = match addr.as_socket() {
                            Some(addr) => addr,
                            None => {
                                println!("Error understanding connection address");
                                continue
                            }
                        };
                        self.new_connection(&addr, tcp_socket);
                    },
                    Err(v) => match v.kind() {
                        std::io::ErrorKind::WouldBlock => break,
                        _ => {
                            println!("Error reading TCP message: {}", v);
                        }
                    }
                }
            }

            // listen to existing clients on tcp sockets
            let mut to_kick = vec![];
            for (id, info) in &mut self.connection_data {
                let mut should_kick = false;
                // try to get all of the data
                while !should_kick {
                    match info.tcp_socket.recv(&mut self.read_buffer) {
                        Ok(size) => if size == 0 {
                            // no data left
                            break;
                        } else {
                            // process the packets out of the socket

                            // convert current chunk to readable array
                            // read_start is where we are currently at in processing
                            let mut extension_read_start = 0;
                            let extension: &[u8] = unsafe {
                                std::mem::transmute(&self.read_buffer[0..size])
                            };

                            // work until reaching the end of the current extension of data
                            let msg = &mut info.tcp_recv;
                            while extension_read_start < extension.len() {
                                // we need to find another remaining number of bytes to process
                                if msg.remaining == 0 {
                                    if msg.length + extension.len() - extension_read_start < 4 {
                                        // copy the rest of extension in
                                        let copy_len = extension.len() - extension_read_start;
                                        msg.buffer[msg.length..msg.length + copy_len]
                                            .copy_from_slice(&extension[extension_read_start..]);
                                        msg.length += copy_len;
                                        //extension_read_start += copy_len; // will not be read again
                                        break;

                                    } else if msg.length >= 4 {
                                        // this shouldn't happen
                                        panic!("Logic error decoding TCP message!");

                                    } else {
                                        // we have enough bytes to find message length
                                        // remove remaining message length bytes from extension
                                        let copy_len = 4 - msg.length;
                                        msg.buffer[msg.length..msg.length + copy_len].copy_from_slice(
                                            &extension[extension_read_start..extension_read_start + copy_len]);
                                        //msg.length += extend_by; // will be cleared anyway
                                        extension_read_start += copy_len;

                                        // find full message length
                                        let remaining_in_bytes: [u8; 4] = msg.buffer[0..4].try_into().unwrap();
                                        msg.remaining = u32::from_be_bytes(remaining_in_bytes) as usize;

                                        // clear message buffer (should have only 4 bytes)
                                        msg.length = 0;
                                    }
                                }
                                if msg.remaining > MAX_TCP_MESSAGE_SIZE {
                                    should_kick = true;
                                    println!("Client attempted to send message that was too big: {}", msg.remaining);
                                    break;
                                }
                                if msg.remaining > 0 {
                                    // copy either remaining message bytes, or all new bytes, whichever is smaller
                                    let copy_len = cmp::min(msg.remaining, extension.len() - extension_read_start);
                                    msg.buffer[msg.length..msg.length + copy_len]
                                        .copy_from_slice(&extension[extension_read_start..]);
                                    msg.length += copy_len;
                                    msg.remaining -= copy_len;
                                    extension_read_start += copy_len;

                                    // if we copied all remaining message bytes
                                    if msg.remaining == 0 {
                                        // we finished a packet!
                                        // store the finished message
                                        let finished_message = msg.buffer[0..msg.length].to_vec();
                                        match messages.get_mut(id) {
                                            Some(list) => list.push(finished_message),
                                            None => {
                                                messages.insert(*id, vec![finished_message]);
                                            }
                                        };
                                        // clear the message
                                        msg.length = 0;
                                    }
                                }
                            }
                            // reached end of current extension of data
                        },
                        Err(v) => match v.kind() {
                            std::io::ErrorKind::WouldBlock => break,
                            _ => {
                                println!("Error reading TCP message: {}", v);
                                // an error will desync the data stream, so we need the connection to be reset
                                should_kick = true;
                                break;
                            }
                        }
                    }
                }
                if should_kick {
                    to_kick.push(*id);
                }
            }
            for id in to_kick {
                self.kick(&id);
            }
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

            // tcp sending
            for (id, info) in &mut self.connection_data {
                let data = &mut info.tcp_send;
                loop {
                    // send as many bytes of previous unsent message
                    while data.position < data.length {
                        match info.tcp_socket.send(&data.buffer[data.position..data.length]) {
                            Ok(sent) => {
                                data.position += sent;
                            },
                            Err(v) => match v.kind() {
                                std::io::ErrorKind::WouldBlock => break,
                                _ => {
                                    println!("Error writing TCP {} bytes to {} ({}): {}",
                                        data.length - data.position,
                                        info.address,
                                        id,
                                        v);
                                    break;
                                }
                            }
                        }
                    }
                    // choose new message if there are none currently being sent
                    if data.position >= data.length {
                        if let Some(next) = data.queue.pop_front() {
                            data.buffer[0..next.len()].copy_from_slice(next.as_slice());
                            data.position = 0;
                            data.length = next.len();
                            data.queue_size -= next.len();
                        } else {
                            break;
                        }
                    } else { // can happen if error on prev loop
                        break;
                    }
                }
            }
        }

        pub fn send_udp(&mut self, receivers: Vec<ConnectionID>, data: Vec<u8>) {
            assert!(data.len() <= MAX_UDP_MESSAGE_SIZE);
            self.udp_message_queue.push((receivers, data));
        }

        pub fn send_tcp(&mut self, receivers: Vec<ConnectionID>, data: &Vec<u8>) {
            assert!(data.len() <= MAX_TCP_MESSAGE_SIZE);
            // eventually: reuse the same data to send to each receiver
            for id in receivers {
                let info = match self.connection_data.get_mut(&id) {
                    Some(info) => info,
                    None => {
                        println!("Attempt to send TCP data to non-existent client {}", id);
                        break
                    }
                };
                info.tcp_send.queue.push_back(data.clone());
                info.tcp_send.queue_size += data.len();
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

    pub struct Connection {
        udp_socket: Option<Socket>,
        tcp_socket: Option<Socket>,
        tcp_send: TcpSendState,
        tcp_recv: TcpRecvState,
        server_address: SockAddr,
        server_address_socket: SocketAddr,
        message_queue: Vec<Vec<u8>>,
        read_buffer: [MaybeUninit<u8>; 1<<20]
    }

    impl Connection {
        pub fn new(server_address: &SocketAddr) -> (Connection, Option<Error>) {
            let server_address_obj: SockAddr = (*server_address).into();
            let mut error = None;
            let (udp_socket, tcp_socket) = match (make_udp_socket(None), make_tcp_socket(None, Some(server_address_obj))) {
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
            let mut connection = Connection {
                server_address: server_address_obj,
                server_address_socket: *server_address,
                message_queue: Vec::new(),
                udp_socket: None,
                tcp_socket: None,
                tcp_send: TcpSendState {
                    queue: VecDeque::new(),
                    queue_size: 0,
                    buffer: [0; MAX_TCP_MESSAGE_SIZE],
                    length: 0,
                    position: 0
                },
                tcp_recv: TcpRecvState {
                    buffer: [0; MAX_TCP_MESSAGE_SIZE],
                    length: 0,
                    remaining: 0
                },
                read_buffer: [MaybeUninit::zeroed(); 1<<20]
            };
            let error = match connection.connect() {
                Ok(()) => None,
                Err(e) => Some(e)
            };
            (connection, error)
        }

        pub fn connect(&mut self) -> Result<()> {
            let mut error = None;
            (self.udp_socket, self.tcp_socket) = match (make_udp_socket(None), make_tcp_socket(None, Some(self.server_address))) {
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
                None => Ok(()),
                Some(e1) => Err(e1)
            }
        }

        pub fn disconnect(&mut self) {
            (self.udp_socket, self.tcp_socket) = (None, None);
        }

        pub fn get_server_address(&self) -> SocketAddr {
            self.server_address_socket
        }

        pub fn set_server_address(&mut self, addr: &SocketAddr) {
            self.server_address_socket = addr.clone();
            self.server_address = (*addr).into();
        }

        pub fn poll(&mut self) -> Vec<Vec<u8>> {
            let mut messages: Vec<Vec<u8>> = Vec::new();
            if let (Some(udp_socket), Some(tcp_socket)) = (self.udp_socket, self.tcp_socket) {
                loop {
                    match udp_socket.recv_from(&mut self.read_buffer) {
                        Ok((size, addr)) => {
                            let addr: SocketAddr = match addr.as_socket() {
                                Some(addr) => addr,
                                None => {
                                    println!("Error understanding connection address");
                                    continue
                                }
                            };
                            if addr != self.server_address_socket {
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
                loop {
                    match tcp_socket.recv(&mut self.read_buffer) {
                        Ok(size) => {

                        },
                        Err(v) => {
                            match v.kind() {
                                ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error reading TCP message: {}", v);
                                }
                            }
                        }
                    }
                }
            }
            messages
        }
    
        pub fn flush(&mut self) {
            let mut failed: Vec<Vec<u8>> = Vec::new();
            if let Some(udp_socket) = self.udp_socket {
                for data in self.message_queue.drain(0..self.message_queue.len()) {
                    let addr = &self.server_address;
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
            }
            self.message_queue.append(&mut failed);
            // match error {
            //     None => {
            //         self.message_queue.clear();
            //         Ok(())
            //     },
            //     Some(err) => {
            //         self.message_queue = self.message_queue.split_off(stopped);
            //         Err(err)
            //     }
            // }
        }

        pub fn send_udp(&mut self, data: Vec<u8>) {
            if data.len() > MAX_UDP_MESSAGE_SIZE {
                panic!("Client attempted to send packet larger than UDP packet size: {} > {}", data.len(), MAX_UDP_MESSAGE_SIZE);
            }
            self.message_queue.push(data);
        }

        pub fn send_tcp(&mut self, data: Vec<u8>) {
            if data.len() > MAX_TCP_MESSAGE_SIZE {
                panic!("Client attempted to send packet larger than TCP packet size: {} > {}", data.len(), MAX_TCP_MESSAGE_SIZE);
            }
            self.tcp_send.queue_size += data.len();
            self.tcp_send.queue.push_back(data);
        }
    }
}
