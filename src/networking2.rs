use socket2::{Protocol, Socket, Domain, Type};
use std::{io::Result, net::SocketAddr};

pub fn make_socket(port: Option<u16>, protocol: Protocol) -> Result<Socket> {
    let socket: Socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(protocol))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;
    let address: SocketAddr = format!("0.0.0.0:{}",
        match port {Some(port) => port, _ => 0}
    ).parse().unwrap();
    let address = address.into();
    socket.bind(&address)?;

    Ok(socket)
}

const UDP_READ_BUFFER_SIZE: usize = 1024;


pub mod wrapper {
    use std::{mem::MaybeUninit, net::SocketAddr, collections::HashMap, io::Error};
    use socket2::{Socket, Protocol};
    use super::{UDP_READ_BUFFER_SIZE, make_socket};
    use std::io::Result;

    pub struct UdpConnection {
        socket: Socket,
        recv_buffer: Box<[MaybeUninit<u8>]>
    }

    impl UdpConnection {
        pub fn new(port: Option<u16>) -> Result<UdpConnection> {
            let socket = make_socket(port, Protocol::UDP)?;
            Ok(UdpConnection {
                socket,
                recv_buffer: vec![MaybeUninit::zeroed(); UDP_READ_BUFFER_SIZE].into_boxed_slice()
            })
        }

        pub fn send(&mut self, target_addr: &SocketAddr, data: &[u8]) -> Result<usize> {
            self.socket.send_to(data, &(*target_addr).into())
        }

        pub fn recv(&mut self) -> Result<(SocketAddr, Vec<u8>)> {
            match self.socket.recv_from(self.recv_buffer.as_mut()) {
                Ok((usize, addr)) => match addr.as_socket() {
                    Some(addr) => Ok((addr, unsafe {
                        let data: &[u8] = std::mem::transmute(&self.recv_buffer[0..usize]);
                        data.to_vec()
                    })),
                    None => Err(std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "Invalid address"))
                },
                Err(err) => Err(err),
            }
        }

        pub fn recv_all(&mut self, limit: Option<usize>) -> (HashMap<SocketAddr, Vec<Vec<u8>>>, Option<Error>) {
            let mut error = None;
            let mut map: HashMap<SocketAddr, Vec<Vec<u8>>> = HashMap::new();
            let limit = match limit {
                Some(limit) => limit,
                None => usize::MAX
            };
            for _ in 0..limit {
                match self.recv() {
                    Ok((addr, packet)) => {
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
    }

    pub mod TcpConnection {
        pub fn new(port: Option<u16>) {
            
        }
    }
}