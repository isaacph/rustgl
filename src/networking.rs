use std::net::SocketAddr;

use socket2::{Socket, Domain, Type, Protocol};
use std::io::Result;

fn make_socket(port: Option<u16>) -> Result<Socket> {
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

pub mod server {
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::io::Result;
    use std::mem::MaybeUninit;
    use std::time::{SystemTime, Duration};
    use socket2::Socket;
    use std::net::SocketAddr;

    use super::make_socket;

    // the time after the last message after which to declare the connection dead
    const LAST_MESSAGE_DEAD_TIME: Duration = Duration::new(10, 0);

    struct ConnectionInfo {
        address: SocketAddr,
        last_message: SystemTime
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
        socket: Socket,
        connection_data: HashMap<ConnectionID, ConnectionInfo>,
        addr_id_map: HashMap<SocketAddr, ConnectionID>,
        all_connection_ids: Vec<ConnectionID>,
        generator: ConnectionIDGenerator,
        message_queue: Vec<(Vec<ConnectionID>, Vec<u8>)>,
    }

    impl ServerConnection {
        pub fn new(port: u16) -> Result<ServerConnection> {
            let socket = make_socket(Some(port))?;
            Ok(ServerConnection {
                socket,
                connection_data: HashMap::new(),
                addr_id_map: HashMap::new(),
                all_connection_ids: Vec::new(),
                generator: ConnectionIDGenerator::new(),
                message_queue: Vec::new()
            })
        }

        fn new_connection(&mut self, addr: &SocketAddr) -> ConnectionID {
            let id = self.generator.generate();
            self.addr_id_map.insert(*addr, id.clone());
            self.connection_data.insert(id.clone(), ConnectionInfo {
                address: *addr,
                last_message: SystemTime::now()
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
            match self.connection_data.get(id) {
                Some(data) => Some(data.address),
                None => None
            }
        }
        pub fn all_connection_ids(&self) -> Vec<ConnectionID> {
            self.all_connection_ids.clone()
        }

        pub fn poll_raw(&mut self) -> HashMap<ConnectionID, Vec<Vec<u8>>> {
            let mut buffer = [MaybeUninit::<u8>::uninit(); 16384];
            let mut messages: HashMap<ConnectionID, Vec<Vec<u8>>> = HashMap::new();
            loop {
                match self.socket.recv_from(buffer.as_mut_slice()) {
                    Ok((size, addr)) => {
                        let addr: SocketAddr = match addr.as_socket() {
                            Some(addr) => addr,
                            None => {
                                println!("Error understanding connection address");
                                continue
                            }
                        };
                        let result: Vec<u8> = unsafe {
                            let temp: &[u8; 16384] = std::mem::transmute(&buffer);
                            &temp[0..size]
                        }.to_vec();
                        println!("Received {} bytes from {:?}: {}", size, addr, String::from_utf8_lossy(&result));
                        let id = match self.addr_id_map.get(&addr) {
                            Some(id) => *id,
                            None => self.new_connection(&addr)
                        };
                        match self.connection_data.get_mut(&id) {
                            Some(data) => data.last_message = SystemTime::now(),
                            None => panic!("Error: connection found in address map but not in data map: {}", id)
                        }
                        match messages.get_mut(&id) {
                            Some(list) => list.push(result),
                            None => {
                                messages.insert(id, vec![result]);
                            }
                        };
                    }
                    Err(v) => {
                        match v.kind() {
                            std::io::ErrorKind::WouldBlock => (),
                            _ => {
                                println!("Error reading message: {}", v);
                            }
                        }
                        break;
                    }
                };
            }
            messages
        }
        pub fn flush(&mut self) {
            let mut retry: Vec<(Vec<ConnectionID>, Vec<u8>)> = Vec::new();
            let send_to = |id: &ConnectionID, data: &Vec<u8>| -> bool { // returns whether to retry
                let con_data = match self.connection_data.get(&id) {
                    Some(data) => data,
                    None => {
                        println!("Error sending data to unknown client (id={})", id);
                        return false;
                    }
                };
                let addr = con_data.address.into();
                let written = self.socket.send_to(data.as_slice(), &addr);
                match written {
                    Ok(written) => {
                        if written != data.len() {
                            println!("Error writing {} bytes to {}: {} < {}", data.len(), con_data.address, written, data.len());
                        }
                        false
                    },
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::WouldBlock => true,
                            _ => {
                                println!("Error writing {} bytes to {}: {}", data.len(), con_data.address, e);
                                return true;
                            }
                        }
                    }
                }
            };
            for (ids, data) in self.message_queue.drain(0..self.message_queue.len()) {
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
            self.message_queue.append(&mut retry);
        }
        pub fn send_raw(&mut self, receivers: Vec<ConnectionID>, data: Vec<u8>) {
            self.message_queue.push((receivers, data));
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
    use std::io::Result;
    use std::mem::MaybeUninit;
    use socket2::{Socket, SockAddr};
    use std::net::SocketAddr;
    use std::io::ErrorKind;

    use super::make_socket;

    pub struct Connection {
        socket: Socket,
        server_address: SockAddr,
        server_address_socket: SocketAddr,
        message_queue: Vec<Vec<u8>>
    }

    impl Connection {
        pub fn new(server_address: &SocketAddr) -> Result<Connection> {
            let socket = make_socket(None)?;
            Ok(Connection {
                socket,
                server_address: (*server_address).into(),
                server_address_socket: *server_address,
                message_queue: Vec::new()
            })
        }

        pub fn get_server_address(&self) -> SocketAddr {
            self.server_address_socket
        }

        pub fn set_server_address(&mut self, addr: &SocketAddr) {
            self.server_address_socket = addr.clone();
            self.server_address = (*addr).into();
        }

        pub fn poll_raw(&mut self) -> Vec<Vec<u8>> {
            let mut buffer = [MaybeUninit::<u8>::uninit(); 16384];
            let mut messages: Vec<Vec<u8>> = Vec::new();
            loop {
                match self.socket.recv_from(buffer.as_mut_slice()) {
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
                            let temp: &[u8; 16384] = std::mem::transmute(&buffer);
                            &temp[0..size]
                        }.to_vec();
                        println!("Received {} bytes from {:?}: {}", size, addr, String::from_utf8_lossy(&result));
                        messages.push(result);
                    }
                    Err(v) => {
                        match v.kind() {
                            ErrorKind::WouldBlock => (),
                            _ => {
                                println!("Error reading message: {}", v);
                            }
                        }
                        break;
                    }
                };
            }
            messages
        }
        pub fn flush(&mut self) {
            let mut failed: Vec<Vec<u8>> = Vec::new();
            for data in self.message_queue.drain(0..self.message_queue.len()) {
                let addr = &self.server_address;
                let written = self.socket.send_to(data.as_slice(), addr);
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
        pub fn send_raw(&mut self, data: Vec<u8>) {
            self.message_queue.push(data);
        }
    }
}