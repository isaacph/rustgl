extern crate glfw;

// pub mod graphics;
// pub mod chatbox;
// pub mod networking_wrapping;
pub mod networking;
pub mod model;
pub mod client;
pub mod server;
// pub mod game;
// pub mod server;
// pub mod world;

use std::collections::HashMap;
use std::net::{UdpSocket, TcpListener, TcpStream, Shutdown};
use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc::{self, Receiver};
use std::sync::mpsc::TryRecvError;
use std::io::{self, ErrorKind, Read, Write};

use model::SerializedServerCommand;
use networking::server::{Server, ServerUpdate};

use crate::model::{GetAddress, SetUDPAddress, EchoMessage, SerializedClientCommand};
use crate::networking::Protocol;
use crate::networking::client::{Client, ClientResult};

// fn echo_server(port_udp: u16, port_tcp: u16) -> Result<()> {
//     let mut server = networking::server::ServerConnection::new(port_udp, port_tcp)?;
//     let mut stop = false;
//     while !stop {
//         for (id, data) in server.poll() {
//             for packet in data {
//                 if String::from_utf8_lossy(packet.as_slice()).eq("stop") {
//                     stop = true;
//                 }
//                 server.send_udp(vec![id], packet);
//             }
//         }
//         server.flush();
//         std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
//     }
//     Ok(())
// }

fn console_client_udp(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
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

pub fn grab_console_line(prompt: &str) -> String {
    let mut buffer = String::new();
    io::stdout().write_all(prompt.as_bytes()).unwrap();
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut buffer).unwrap();
    String::from(buffer.trim())
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

pub fn udp_recv_all(socket: &UdpSocket, buffer: &mut [u8], limit: Option<usize>)
    -> (HashMap<SocketAddr, Vec<Box<[u8]>>>, Option<std::io::Error>) {
    let mut error = None;
    let mut map: HashMap<SocketAddr, Vec<Box<[u8]>>> = HashMap::new();
    let limit = match limit {
        Some(limit) => limit,
        None => usize::MAX
    };
    for _ in 0..limit {
        match socket.recv_from(buffer) {
            Ok((sent, addr)) => {
                let packet = Vec::from(&buffer[0..sent]).into_boxed_slice();
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

fn echo_server_udp(ports: (u16, u16)) -> Result<()> {
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

fn echo_server_tcp(ports: (u16, u16)) -> Result<()> {
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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let ports: (u16, u16) =
                (grab_console_line("UDP port: ").parse().expect("Invalid port"),
                 grab_console_line("TCP port: ").parse().expect("Invalid port"));
    let addresses: (SocketAddr, SocketAddr) = (
        format!("127.0.0.1:{}", ports.0).parse().unwrap(),
        format!("127.0.0.1:{}", ports.1).parse().unwrap()
    );
    match args[1].as_str() {
        // "echo_server" => {
        //     echo_server(1234, 1235)?
        // },
        // "server" => {
        //     // server::Server::run(1234);
        // },
        // "gclient" => {
        //     // game::Game::run(&server_address);
        // }
        // "client" => { // client
        //     console_client(server_address_udp, server_address_tcp)?
        // },
        "udpserver" => {
            echo_server_udp(ports)?
        },
        "udpclient" => {
            console_client_udp(addresses)?
        },
        "tcpclient" => {
            console_client_tcp(addresses)?
        },
        "tcpserver" => {
            echo_server_tcp(ports)?
        },
        "bothserver" => {
            echo_server_both(ports)?
        },
        "bothclient" => {
            console_client_both(addresses)?
        },
        _ => {
            println!("Unknown mode");
        }
    }
    Ok(())
}

