use std::{net::{TcpStream, SocketAddr, UdpSocket, TcpListener, Shutdown}, collections::{VecDeque, HashMap}, io::{Read, Write}};

use crate::{model::{SerializedClientCommand, Protocol, SerializedServerCommand}, udp_recv_all};

use super::tcp_buffering::{TcpRecvState, TcpSendState};

pub struct ConnectionInfo {
    pub stream: TcpStream,
    pub _tcp_address: SocketAddr,
    pub udp_address: Option<SocketAddr>,
    pub udp_send_queue: VecDeque<SerializedClientCommand>,
    pub tcp_recv: TcpRecvState,
    pub tcp_send: TcpSendState
}

pub struct Server {
    pub udp: UdpSocket,
    pub tcp: TcpListener,
    pub connections: HashMap<SocketAddr, ConnectionInfo>,
    pub corresponding_tcp_to_udp: HashMap<SocketAddr, SocketAddr>
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

pub fn echo_server_both(ports: (u16, u16)) -> std::io::Result<()> {
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

        // execute all packets
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
        // std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
}