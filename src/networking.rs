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
    use std::time::SystemTime;
    use socket2::Socket;
    use std::net::SocketAddr;

    use super::make_socket;

    // the time after the last message after which to declare the client dead
    // const LAST_MESSAGE_DEAD_TIME: Duration = Duration::new(10, 0);

    struct ClientInfo {
        address: SocketAddr,
        last_message: SystemTime
    }

    #[derive(PartialEq, Eq, Hash, Clone, Copy)]
    pub struct ClientID(i32);

    impl Display for ClientID {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    struct ClientIDGenerator {
        counter: i32
    }

    impl ClientIDGenerator {
        fn new() -> Self {
            Self {
                counter: 0
            }
        }
        fn generate(&mut self) -> ClientID {
            self.counter += 1;
            ClientID(self.counter - 1)
        }
    }

    pub struct ServerConnection {
        socket: Socket,
        client_data: HashMap<ClientID, ClientInfo>,
        addr_id_map: HashMap<SocketAddr, ClientID>,
        generator: ClientIDGenerator,
        message_queue: Vec<(ClientID, Vec<u8>)>,
    }

    impl ServerConnection {
        pub fn new(port: u16) -> Result<ServerConnection> {
            let socket = make_socket(Some(port))?;
            Ok(ServerConnection {
                socket,
                client_data: HashMap::new(),
                addr_id_map: HashMap::new(),
                generator: ClientIDGenerator::new(),
                message_queue: Vec::new()
            })
        }

        fn new_client(&mut self, addr: &SocketAddr) -> ClientID {
            let id = self.generator.generate();
            self.addr_id_map.insert(*addr, id.clone());
            self.client_data.insert(id.clone(), ClientInfo {
                address: *addr,
                last_message: SystemTime::now()
            });
            id
        }

        pub fn get_address(&self, id: &ClientID) -> Option<SocketAddr> {
            match self.client_data.get(id) {
                Some(data) => Some(data.address),
                None => None
            }
        }

        pub fn poll(&mut self) -> HashMap<ClientID, Vec<Vec<u8>>> {
            let mut buffer = [MaybeUninit::<u8>::uninit(); 16384];
            let mut messages: HashMap<ClientID, Vec<Vec<u8>>> = HashMap::new();
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
                            let temp: &[u8; 1024] = std::mem::transmute(&buffer);
                            &temp[0..size]
                        }.to_vec();
                        println!("Received {} bytes from {:?}: {}", size, addr, std::str::from_utf8(&result).unwrap());
                        let id = match self.addr_id_map.get(&addr) {
                            Some(id) => *id,
                            None => self.new_client(&addr)
                        };
                        match self.client_data.get_mut(&id) {
                            Some(data) => data.last_message = SystemTime::now(),
                            None => panic!("Error: client found in address map but not in data map: {}", id)
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
            let mut failed: Vec<(ClientID, Vec<u8>)> = Vec::new();
            for (id, data) in self.message_queue.drain(0..self.message_queue.len()) {
                let client_data = match self.client_data.get(&id) {
                    Some(data) => data,
                    None => {
                        println!("Error sending data to unknown client (id={})", id);
                        continue;
                    }
                };
                let addr = client_data.address.into();
                let written = self.socket.send_to(data.as_slice(), &addr);
                match written {
                    Ok(written) => {
                        if written != data.len() {
                            println!("Error writing {} bytes to {}: {} < {}", data.len(), client_data.address, written, data.len());
                        }
                    },
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::WouldBlock => (),
                            _ => {
                                println!("Error writing {} bytes to {}: {}", data.len(), client_data.address, e);
                                failed.push((id, data));
                                continue;
                            }
                        }
                    }
                }
            }
            self.message_queue.append(&mut failed);
        }
        pub fn send(&mut self, client: &ClientID, data: Vec<u8>) {
            self.message_queue.push((*client, data));
        }
        pub fn last_message_time(&self, client: &ClientID) -> Option<SystemTime> {
            match self.client_data.get(&client) {
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

        pub fn poll(&mut self) -> Vec<Vec<u8>> {
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
                            let temp: &[u8; 1024] = std::mem::transmute(&buffer);
                            &temp[0..size]
                        }.to_vec();
                        println!("Received {} bytes from {:?}: {}", size, addr, std::str::from_utf8(&result).unwrap());
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
                            ErrorKind::WouldBlock => (),
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
        pub fn send(&mut self, data: Vec<u8>) {
            self.message_queue.push(data);
        }
    }
}