extern crate glfw;

// pub mod graphics;
// pub mod chatbox;
// pub mod networking;
// pub mod networking_wrapping;
// pub mod game;
// pub mod server;
// pub mod world;
pub mod networking2;

use std::collections::HashMap;
use std::net::UdpSocket;
use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::io::{self, ErrorKind};

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

fn console_client(addresses: (SocketAddr, SocketAddr)) -> Result<()> {
    // let send_to: SocketAddr = format!("127.0.0.1:{}", {
    //     let mut buffer = String::new();
    //     io::stdin().read_line(&mut buffer).unwrap();
    //     buffer.truncate(buffer.len() - 2);
    //     buffer
    // }).parse().unwrap();
    let udp = UdpSocket::bind("0.0.0.0:0")?;
    udp.set_nonblocking(true)?;
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || loop {
            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer).unwrap();
            buffer.truncate(buffer.len() - 2);
            tx.send(buffer).unwrap();
        });
        rx
    };
    let mut buffer = vec![0u8; 1024].into_boxed_slice();
    loop {
        let (recv, err) = udp_recv_all(&udp, buffer.as_mut(), None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received from {:?}: {}", addr, std::str::from_utf8(packet.as_slice()).unwrap());
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

fn udp_recv_all(socket: &UdpSocket, buffer: &mut [u8], limit: Option<usize>)
    -> (HashMap<SocketAddr, Vec<Vec<u8>>>, Option<std::io::Error>) {
    let mut error = None;
    let mut map: HashMap<SocketAddr, Vec<Vec<u8>>> = HashMap::new();
    let limit = match limit {
        Some(limit) => limit,
        None => usize::MAX
    };
    for _ in 0..limit {
        match socket.recv_from(buffer) {
            Ok((sent, addr)) => {
                let packet = Vec::from(&buffer[0..sent]);
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

fn echo_server(ports: (u16, u16)) -> Result<()> {
    let udp = UdpSocket::bind(format!("0.0.0.0:{}", ports.0))?;
    udp.set_nonblocking(true)?;
    let mut buffer: Box<[u8]> = vec![0u8; 1024].into_boxed_slice();
    let mut run = true;
    while run {
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
        let (recv, err) = udp_recv_all(&udp, &mut buffer, None);
        for (addr, packets) in recv {
            for packet in packets {
                let str = String::from_utf8_lossy(packet.as_slice());
                println!("Recv from {:?}: {}", addr, str);
                match udp.send_to(packet.as_slice(), addr) {
                    Ok(size) => println!("Sent {} bytes", size),
                    Err(err) => println!("Error sending: {}", err)
                }
                match str.as_ref() {
                    "stop" => run = false,
                    _ => ()
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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let ports = (1234, 1235);
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
        "server" => {
            echo_server(ports)?
        },
        "client" => {
            console_client(addresses)?
        },
        _ => {
            println!("Unknown mode");
        }
    }
    Ok(())
}

