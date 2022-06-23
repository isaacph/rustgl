use std::{net::{SocketAddr, UdpSocket, TcpStream, TcpListener, Shutdown}, io::{ErrorKind, self, Read, Write}, sync::mpsc::{TryRecvError, Receiver, self}, time::Duration, thread, collections::HashMap};
use std::io::Result;

use crate::{networking::{client::{Client, ClientResult}, Protocol}, model::commands::{GetAddress, SetUDPAddress, EchoMessage, SerializedClientCommand, SerializedServerCommand}};

use super::{common::udp_recv_all, server::{Server, ServerUpdate}};

pub fn console_client_udp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    // let send_to: SocketAddr = format!("127.0.0.1:{}", {
    //     let mut buffer = String::new();
    //     io::stdin().read_line(&mut buffer).unwrap();
    //     buffer.truncate(buffer.len() - 2);
    //     buffer
    // }).parse().unwrap();
    let udp = UdpSocket::bind("0.0.0.0:0")?;
    udp.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    loop {
        let (recv, err) = udp_recv_all(&udp, buffer.as_mut(), None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received from {:?}: {}", addr, std::str::from_utf8(packet.as_ref()).unwrap());
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error receiving: {}", err)
            },
            None => ()
        }
        let message = match stdin_channel.try_recv() {
            Ok(v) => v,
            Err(TryRecvError::Empty) => {
                std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
                continue
            },
            Err(TryRecvError::Disconnected) => break,
        };
        match udp.send_to(message.as_bytes(), &addresses.0) {
            Ok(sent) => println!("Sent {} bytes", sent),
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error sending: {}", err)
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    Ok(())
}

pub fn console_stream() -> Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer.trim_end().into()).unwrap();
    });
    rx
}

pub fn echo_server_udp(ports: (u16, u16)) -> Result<()> {
    let udp = UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?;
    udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    let mut run = true;
    while run {
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        let (recv, err) = udp_recv_all(&udp, &mut buffer, None);
        for (addr, packets) in recv {
            for packet in packets {
                let str = String::from_utf8_lossy(packet.as_ref());
                println!("Recv from {:?}: {}", addr, str);
                match udp.send_to(packet.as_ref(), addr) {
                    Ok(size) => println!("Sent {} bytes", size),
                    Err(err) => println!("Error sending: {}", err)
                }
                if str.as_ref() == "false" {
                    run = false;
                }
            }
        }
        if let Some(err) = err {
            match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error receiving: {}", err)
            }
        }
    }
    Ok(())
}
// struct ConnectionInfo {
//     stream: TcpStream,
//     write_buffer: Vec<u8>
// }
// loop {
//     match tcp.accept() {
//         Ok((stream, addr)) => {
//             println!("New connection from {}", addr);
//             connections.insert(addr, ConnectionInfo {
//                 stream,
//                 write_buffer: vec![]
//             });
//         },
//         Err(err) => match err.kind() {
//             std::io::ErrorKind::WouldBlock => break,
//             _ => {
//                 println!("Error with TCP accept: {}", err);
//                 break;
//             }
//         }
//     }
// }

pub fn console_client_tcp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    let addr = addresses.1;
    let mut stream = TcpStream::connect(addr)?;
    println!("Connected on {}", stream.local_addr()?);
    stream.set_nonblocking(true)?;
    let stdin_channel = console_stream();
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    let mut write_buffer = vec![];
    loop {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }
        match stream.read(buffer.as_mut()) {
            Ok(size) => match size {
                0 => break,
                _ => {
                    let data = &buffer[0..size];
                    let str = String::from_utf8_lossy(data);
                    println!("Received from {}: {}", addr, str);
                }
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => {
                    println!("Error receiving from {}: {}", addr, err);
                }
            }
        }
        match stream.write(write_buffer.as_mut_slice()) {
            Ok(sent) => {
                write_buffer.drain(0..sent);
            },
            Err(err) => match err.kind() {
                std::io::ErrorKind::WouldBlock => (),
                _ => println!("Error writing to {}: {}", addr, err)
            }
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("Disconnected from server.");
    Ok(())
}

pub fn echo_server_tcp(ports: (u16, u16)) -> Result<()> {
    let tcp = TcpListener::bind(format!("0.0.0.0:{}", ports.1))?;
    tcp.set_nonblocking(true)?;
    struct ConnectionInfo {
        stream: TcpStream,
        write_buffer: Vec<u8>,
    }
    let mut connections: HashMap<SocketAddr, ConnectionInfo> = HashMap::new();
    let mut buffer = Box::new([0u8; 1024]);
    loop {
        loop {
            match tcp.accept() {
                Ok((stream, addr)) => {
                    println!("New connection from {}", addr);
                    connections.insert(addr, ConnectionInfo {
                        stream,
                        write_buffer: vec![]
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
        for (addr, info) in &mut connections {
            match info.stream.read(buffer.as_mut()) {
                Ok(size) => match size {
                    0 => disconnects.push(*addr),
                    _ => {
                        let data = &buffer[0..size];
                        let str = String::from_utf8_lossy(data);
                        println!("Received from {}: {}", addr, str);
                        info.write_buffer.extend(data);
                    }
                },
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => (),
                    _ => {
                        println!("Error receiving from {}: {}", addr, err);
                        disconnects.push(*addr);
                        continue;
                    }
                }
            }
            if info.write_buffer.len() > 0 {
                match info.stream.write(info.write_buffer.as_mut_slice()) {
                    Ok(sent) => match sent {
                        0 => disconnects.push(*addr),
                        _ => {
                            info.write_buffer.drain(0..sent);
                        }
                    },
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        _ => {
                            println!("Error writing to {}: {}", addr, err);
                            disconnects.push(*addr);
                            continue;
                        }
                    }
                }
            }
        }
        for addr in disconnects {
            if let Some(info) = connections.remove(&addr) {
                if let Err(err) = info.stream.shutdown(Shutdown::Both) {
                    println!("Error disconnecting from {}: {}", addr, err);
                } else {
                    println!("Disconnected from {}", addr);
                }
            }
        }
    }
}

pub fn console_client_both(addresses: (SocketAddr, SocketAddr)) -> std::io::Result<()> {
    let mut client = Client::init_disconnected();
    client.connect(addresses.0, addresses.1);
    let stdin_channel = console_stream();
    let mut run = true;
    while run {
        match stdin_channel.try_recv() {
            Ok(msg) => {
                match (|| -> ClientResult<()> {
                    let split: Vec<&str> = msg.split(' ').collect();
                    match &split[..] {
                        ["stop", ..] => {
                            run = false;
                        }
                        ["disconnect", ..] => {
                            client.disconnect();
                        }
                        ["getaddr"] => {
                            client.send_udp(GetAddress)?;
                        },
                        ["setaddr", _, ..] => {
                            client.send_tcp(SetUDPAddress(msg["setaddr ".len()..msg.len()].into()))?;
                        }
                        ["udp", "echo", _, ..] => {
                            client.send_udp(EchoMessage(msg["udp echo ".len()..msg.len()].into()))?;
                        },
                        ["tcp", "echo", _, ..] => {
                            client.send_tcp(EchoMessage(msg["tcp echo ".len()..msg.len()].into()))?;
                        },
                        ["tcp", "big", len] => {
                            client.send_tcp(EchoMessage({
                                let mut s = String::new();
                                for _ in 0..len.parse().unwrap() {
                                    s.push('t');
                                }
                                s
                            }))?;
                        }
                        ["connect", _, _] => {
                            match (split[1].parse(), split[2].parse()) {
                                (Ok(udp), Ok(tcp)) => {
                                    client.connect(udp, tcp);
                                },
                                (Err(err), _) => {
                                    println!("Error parsing udp addr: {}", err);
                                },
                                (Ok(_), Err(err)) => {
                                    println!("Error parsing tcp addr: {}", err);
                                }
                            }
                        },
                        _ => println!("Invalid command: {}", msg)
                    };
                    Ok(())
                })() {
                    Ok(()) => (),
                    Err(err) => {
                        println!("Error running command: {}", err);
                    }
                }
                // tcp_write_buffer.extend(msg.as_bytes());
            },
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }

        for message in client.update() {
            match SerializedClientCommand::from(message).execute((Protocol::TCP, &mut client)) {
                Ok(()) => (),
                Err(err) => {
                    println!("{}", err);
                }
            }
        }

        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    println!("User requested stop");
    Ok(())
}

pub fn echo_server_both(ports: (u16, u16)) -> std::io::Result<()> {
    let mut server = Server::init(ports)?;
    loop {
        // execute all packets
        let ServerUpdate {
            messages,
            connects,
            disconnects
        } = server.update();
        for (protocol, addr, message) in messages {
            match SerializedServerCommand::from(message).execute(((protocol, &addr), &mut server)) {
                Ok(()) => println!("Server ran client command from {}", addr),
                Err(err) => println!("Error running client {} command: {}", addr, err)
            }
        }
        for addr in connects {
            println!("New connection from {}", addr);
        }
        for addr in disconnects {
            println!("Disconnected from {}", addr);
        }
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
}
