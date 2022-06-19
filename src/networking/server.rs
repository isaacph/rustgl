use std::{net::{TcpStream, SocketAddr, UdpSocket, TcpListener, Shutdown}, collections::{VecDeque, HashMap}, io::{Read, Write}, cmp, time::Duration};

use crate::{model::{SerializedClientCommand, SerializedServerCommand}, udp_recv_all};

use super::{tcp_buffering::{TcpRecvState, TcpSendState}, Protocol, config::RECV_BUFFER_SIZE};

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
    pub corresponding_tcp_to_udp: HashMap<SocketAddr, SocketAddr>,
    pub recv_buffer: Box<[u8]>
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
}

pub fn echo_server_both(ports: (u16, u16)) -> std::io::Result<()> {
    let mut server = Server {
        udp: UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?,
        tcp: TcpListener::bind(format!("0.0.0.0:{}", ports.1))?,
        connections: HashMap::new(),
        corresponding_tcp_to_udp: HashMap::new(),
        recv_buffer: vec![0u8; RECV_BUFFER_SIZE].into_boxed_slice(),
    };
    server.udp.set_nonblocking(true)?;
    server.tcp.set_nonblocking(true)?;
    loop {
        // recv UDP
        let mut messages: Vec<(Protocol, SocketAddr, SerializedServerCommand)> = Vec::new();
        let (recv, err) = udp_recv_all(&server.udp, &mut server.recv_buffer, None);
        for (addr, data) in recv {
            for packet in data {
                let s = String::from_utf8_lossy(packet.as_ref()).to_string();
                println!("Received UPD from {:?} of len {}: {}", addr, packet.len(), s);
                let command = SerializedServerCommand(packet);
                messages.push((Protocol::UDP, addr, command));
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
                    match stream.set_nonblocking(true) {
                        Ok(()) => {
                            server.connections.insert(addr, ConnectionInfo {
                                stream,
                                _tcp_address: addr,
                                udp_address: None,
                                udp_send_queue: VecDeque::new(),
                                tcp_recv: TcpRecvState::init(),
                                tcp_send: TcpSendState::init()
                            });
                        },
                        Err(err) => {
                            println!("Failed to accept connection from {} since could not set nonblocking: {}", addr, err);
                        }
                    }
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
                    match server.udp.send_to(packet.0.as_ref(), udp_address) {
                        Ok(sent) => {
                            if sent != packet.0.len() {
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
            match info.stream.read(server.recv_buffer.as_mut()) {
                Ok(size) => match size {
                    0 => disconnects.push(*addr),
                    _ => {
                        println!("Received TCP bytes: {}", size);
                        let data = info.tcp_recv.receive(&server.recv_buffer[0..size]);
                        for message in &data {
                            let str = String::from_utf8_lossy(&message[0..cmp::min(1024, message.len())]);
                            println!("Received full message TCP length {} from {}: {}", message.len(), addr, str);
                        }
                        messages.extend(data.into_iter().map(|data| (Protocol::TCP, *addr, data.into())));
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
        for (protocol, addr, message) in messages {
            match message.execute(((protocol, &addr), &mut server)) {
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
