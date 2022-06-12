use std::{net::{TcpStream, UdpSocket, SocketAddr}, collections::VecDeque, sync::mpsc::TryRecvError, io::{ErrorKind, Read, Write}, time::Duration};

use crate::{model::{SerializedServerCommand, GetAddress, SetUDPAddress, EchoMessage, SerializedClientCommand, Protocol}, networking::{tcp_buffering::{TcpSendState, TcpRecvState}}, console_stream, udp_recv_all};

use super::{tcp_buffering, config::MAX_UDP_MESSAGE_SIZE};

pub struct Client {
    pub tcp: TcpStream,
    pub udp: UdpSocket,
    pub addr_tcp: SocketAddr,
    pub addr_udp: SocketAddr,
    pub udp_message_queue: VecDeque<SerializedServerCommand>,
    pub tcp_send: tcp_buffering::TcpSendState,
    pub tcp_recv: tcp_buffering::TcpRecvState,
}

impl Client {
    pub fn send_udp(&mut self, packet: SerializedServerCommand) {
        if packet.data.len() > MAX_UDP_MESSAGE_SIZE {
            println!("Attempted to send UDP message that was too big: {} > {}", packet.data.len(), MAX_UDP_MESSAGE_SIZE);
            return;
        }
        self.udp_message_queue.push_back(packet);
    }
    pub fn send_tcp(&mut self, packet: SerializedServerCommand) {
        self.tcp_send.enqueue(packet.data).unwrap();
    }
}

pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> std::io::Result<()> {
    let mut client = Client {
        tcp: TcpStream::connect(addresses.1)?,
        udp: UdpSocket::bind("0.0.0.0:0")?,
        addr_tcp: addresses.1,
        addr_udp: addresses.0,
        udp_message_queue: VecDeque::new(),
        tcp_send: TcpSendState::init(),
        tcp_recv: TcpRecvState::init()
    };
    println!("Connected on {}", client.tcp.local_addr()?);
    client.tcp.set_nonblocking(true)?;
    client.udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    loop {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                let split: Vec<&str> = msg.split(" ").collect();
                match &split[..] {
                    ["getaddr"] => {
                        client.send_udp((&GetAddress).into());
                    },
                    ["setaddr", _, ..] => {
                        client.send_tcp(
                            (&SetUDPAddress(msg["setaddr ".len()..msg.len()].into())).into()
                        );
                        // let mut buffer = Vec::from(msg["setaddr ".len()..msg.len()].as_bytes());
                        // buffer.push(0u8);
                        // match client.tcp_send.enqueue(buffer) {
                        //     Ok(()) => (),
                        //     Err(err) => println!("Error sending command: {}", err)
                        // }
                    }
                    ["udp", "echo", _, ..] => {
                        client.send_udp((&EchoMessage(msg["udp echo ".len()..msg.len()].into())).into());
                    },
                    ["tcp", "echo", _, ..] => {
                        client.send_tcp(
                            (&EchoMessage(msg["tcp echo ".len()..msg.len()].into())).into()
                        );
                    },
                    ["tcp", "big", len] => {
                        client.send_tcp(
                            (&EchoMessage({
                                let mut s = String::new();
                                for _ in 0..len.parse().unwrap() {
                                    s.push('t');
                                }
                                s
                            })).into()
                        );
                    }
                    _ => println!("Invalid command: {}", msg)
                };
                // tcp_write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }

        let (recv, err) = udp_recv_all(&client.udp, buffer.as_mut(), None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received UDP from {:?}: {}", addr, String::from_utf8_lossy(packet.as_slice()));
                let command = SerializedClientCommand {
                    data: packet
                };
                match command.execute((Protocol::UDP, &mut client)) {
                    Ok(()) => (),
                    Err(err) => println!("Error deserializing UDP command: {}", err)
                }
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error receiving UDP: {}", err)
            },
            None => ()
        }
        while let Some(message) = client.udp_message_queue.pop_front() {
            match client.udp.send_to(message.data.as_slice(), &client.addr_udp) {
                Ok(sent) => println!("Sent UDP {} bytes", sent),
                Err(err) => {
                    match err.kind() {
                        ErrorKind::WouldBlock => break,
                        _ => println!("Error sending UDP: {}", err)
                    }
                    client.udp_message_queue.push_front(message);
                }
            }
        }

        // tcp stuff
        match client.tcp.read(buffer.as_mut()) {
            Ok(size) => match size {
                0 => break,
                _ => {
                    for data in client.tcp_recv.receive(&buffer[0..size]) {
                        let str = String::from_utf8_lossy(&data);
                        println!("Received TCP from {}: {}", client.addr_tcp, str);
                        match SerializedClientCommand::from(data).execute((Protocol::TCP, &mut client)) {
                            Ok(()) => (),
                            Err(err) => {
                                println!("{}", err);
                            }
                        }
                    }
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                std::io::ErrorKind::ConnectionReset => {
                    println!("Error receiving TCP: {}", err);
                    break
                },
                _ => {
                    println!("Error receiving TCP from {}: {}", client.addr_tcp, err);
                }
            }
        }
        if let Some(error) = client.tcp_recv.failed() {
            println!("Error in TCP stream: {}", error);
            break;
        }
        let mut quit = false;
        while let Some(buffer) = client.tcp_send.next_send() {
            match client.tcp.write(buffer) {
                Ok(sent) => {
                    client.tcp_send.update_buffer(sent);
                    println!("Sent {} TCP bytes", sent);
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => {
                        println!("Error writing TCP to {}: {}", client.addr_tcp, err);
                        quit = true;
                    }
                }
            }
        }
        if quit {
            break;
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}