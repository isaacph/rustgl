use std::{net::{TcpStream, UdpSocket, SocketAddr}, collections::VecDeque, sync::mpsc::TryRecvError, io::{ErrorKind, Read, Write}, time::Duration, cmp};

use crate::{model::{GetAddress, SetUDPAddress, EchoMessage, SerializedClientCommand, SerializedServerCommand}, networking::{tcp_buffering::{TcpSendState, TcpRecvState}, Protocol, config::RECV_BUFFER_SIZE}, console_stream, udp_recv_all};

use super::{tcp_buffering, config::MAX_UDP_MESSAGE_SIZE};

// maximum number of network commands to process for each type of processing in one cycle
// note the types are TCP send, TCP recv, UDP send, UDP recv
const MAX_PACKETS_PROCESS: usize = 256;

pub struct Client {
    pub tcp: Option<TcpStream>,
    pub udp: UdpSocket,
    pub addr_tcp: SocketAddr,
    pub addr_udp: SocketAddr,
    pub udp_message_queue: VecDeque<Vec<u8>>,
    pub tcp_send: tcp_buffering::TcpSendState,
    pub tcp_recv: tcp_buffering::TcpRecvState,
    pub recv_buffer: Box<[u8]>
}

impl Client {
    pub fn send_udp<T>(&mut self, packet: T)
    where
        T: Into<SerializedServerCommand>
    {
        let SerializedServerCommand(data) = packet.into();
        if data.len() > MAX_UDP_MESSAGE_SIZE {
            println!("Attempted to send UDP message that was too big: {} > {}", data.len(), MAX_UDP_MESSAGE_SIZE);
            return;
        }
        self.udp_message_queue.push_back(data);
    }
    pub fn send_tcp<T>(&mut self, message: T)
    where
        T: Into<SerializedServerCommand>
    {
        let SerializedServerCommand(data) = message.into();
        self.tcp_send.enqueue(data).unwrap();
    }
    pub fn disconnect(&mut self) {
        println!("Disconnected from server");
    }

    fn update_udp_recv(&mut self) -> Vec<Vec<u8>> {
        let (recv, err) = udp_recv_all(&self.udp, self.recv_buffer.as_mut(), Some(MAX_PACKETS_PROCESS));
        let mut messages = vec![];
        for (addr, recvd) in recv {
            for message in recvd {
                println!("Received UDP from {:?}: {}", addr, String::from_utf8_lossy(message.as_slice()));
                messages.push(message);
                // let command = SerializedClientCommand {
                //     data: message
                // };
                // match command.execute((Protocol::UDP, self)) {
                //     Ok(()) => (),
                //     Err(err) => println!("Error deserializing UDP command: {}", err)
                // }
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error receiving UDP: {}", err)
            },
            None => ()
        }
        messages
    }

    fn update_udp_send(&mut self) {
        let mut processed = 0;
        while let Some(message) = self.udp_message_queue.pop_front() {
            match self.udp.send_to(message.as_slice(), &self.addr_udp) {
                Ok(sent) => println!("Sent UDP {} bytes", sent),
                Err(err) => {
                    match err.kind() {
                        ErrorKind::WouldBlock => break,
                        _ => println!("Error sending UDP: {}", err)
                    }
                    self.udp_message_queue.push_front(message);
                }
            }
            processed += 1;
            if processed >= MAX_PACKETS_PROCESS {
                break;
            }
        }
    }

    fn update_tcp_recv(&mut self) -> Vec<Vec<u8>> {
        let mut messages = vec![];
        if let Some(tcp) = &mut self.tcp {
            let mut quit = false;
            let mut approx_packets = 0;
            while approx_packets < MAX_PACKETS_PROCESS { // limit how much time we spend receiving
                match tcp.read(self.recv_buffer.as_mut()) {
                    Ok(size) => match size {
                        0 => {
                            quit = true;
                            break;
                        },
                        _ => {
                            println!("Received TCP bytes length: {}", size);
                            for data in self.tcp_recv.receive(&self.recv_buffer[0..size]) {
                                let str = String::from_utf8_lossy(&data[0..cmp::min(data.len(), 1024)]);
                                println!("Received full message TCP length {} from {}: {}", data.len(), self.addr_tcp, str);
                                messages.push(data);
                            }
                            approx_packets += 1 + (size - 1) / 1024;
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => break,
                        std::io::ErrorKind::ConnectionReset => {
                            println!("Error receiving TCP: {}", err);
                            quit = true;
                            break;
                        },
                        _ => {
                            println!("Error receiving TCP from {}: {}", self.addr_tcp, err);
                            quit = true;
                            break;
                        }
                    }
                }
            }
            if let Some(error) = self.tcp_recv.failed() {
                println!("Error in TCP recv stream: {}", error);
                self.disconnect();
            }
            if quit {
                self.disconnect();
            }
        }
        messages
    }

    fn update_tcp_send(&mut self) {
        // tcp stuff
        if let Some(tcp) = &mut self.tcp {
            let mut quit = false;
            let mut processed = 0;
            while let Some(buffer) = self.tcp_send.next_send() {
                match tcp.write(buffer) {
                    Ok(sent) => match sent {
                        0 => break,
                        _ => {
                            self.tcp_send.update_buffer(sent);
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
                            println!("Error writing TCP to {}: {}", self.addr_tcp, err);
                            quit = true;
                        }
                    }
                }
            }
            if quit {
                self.disconnect();
            }
        }
    }
}

pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> std::io::Result<()> {
    let tcp = TcpStream::connect(addresses.1)?;
    println!("Connected on {}", tcp.local_addr()?);
    tcp.set_nonblocking(true)?;
    let mut client = Client {
        tcp: Some(tcp),
        udp: UdpSocket::bind("0.0.0.0:0")?,
        addr_tcp: addresses.1,
        addr_udp: addresses.0,
        udp_message_queue: VecDeque::new(),
        tcp_send: TcpSendState::init(),
        tcp_recv: TcpRecvState::init(),
        recv_buffer: vec![0u8; RECV_BUFFER_SIZE].into_boxed_slice()
    };
    
    client.udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    loop {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                let split: Vec<&str> = msg.split(" ").collect();
                match &split[..] {
                    ["getaddr"] => {
                        client.send_udp(GetAddress);
                    },
                    ["setaddr", _, ..] => {
                        client.send_tcp(SetUDPAddress(msg["setaddr ".len()..msg.len()].into()));
                    }
                    ["udp", "echo", _, ..] => {
                        client.send_udp(EchoMessage(msg["udp echo ".len()..msg.len()].into()));
                    },
                    ["tcp", "echo", _, ..] => {
                        client.send_tcp(EchoMessage(msg["tcp echo ".len()..msg.len()].into()));
                    },
                    ["tcp", "big", len] => {
                        client.send_tcp(EchoMessage({
                            let mut s = String::new();
                            for _ in 0..len.parse().unwrap() {
                                s.push('t');
                            }
                            s
                        }));
                    }
                    _ => println!("Invalid command: {}", msg)
                };
                // tcp_write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }

        let mut messages = vec![];
        messages.append(&mut client.update_udp_recv());
        client.update_udp_send();
        messages.append(&mut client.update_tcp_recv());
        client.update_tcp_send();
        for message in messages {
            match SerializedClientCommand::from(message).execute((Protocol::TCP, &mut client)) {
                Ok(()) => (),
                Err(err) => {
                    println!("{}", err);
                }
            }
        }

        //std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}
