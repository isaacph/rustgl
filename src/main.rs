extern crate glfw;

// pub mod graphics;
// pub mod chatbox;
// pub mod networking;
// pub mod networking_wrapping;
// pub mod game;
// pub mod server;
// pub mod world;
pub mod networking2;

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
    let mut udp = networking2::wrapper::UdpConnection::new(None)?;
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
    loop {
        let (recv, err) = udp.recv_all(None);
        for (addr, packets) in recv {
            for packet in packets {
                println!("Received from {:?}: {}", addr, std::str::from_utf8(packet.as_slice()).unwrap());
            }
        }
        match err {
            Some(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => println!("Error sending: {}", err)
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
        match udp.send(&addresses.0, message.as_bytes()) {
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

fn echo_server(ports: (u16, u16)) -> Result<()> {
    let mut udp = networking2::wrapper::UdpConnection::new(Some(ports.0))?;
    loop {
        match udp.recv() {
            Ok((addr, data)) => {
                let data = String::from_utf8_lossy(data.as_slice());
                println!("Recv from {:?}: {}", addr, data);
                match udp.send(&addr, str::as_bytes(&data)) {
                    Ok(size) => println!("Sent {} bytes", size),
                    Err(err) => println!("Error sending: {}", err)
                }
                match data.as_ref() {
                    "stop" => break,
                    _ => ()
                }
            }
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => (),
                _ => {
                    println!("Echo server recv error: {}", err);
                }
            },
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

