extern crate glfw;

pub mod graphics;
pub mod chatbox;
pub mod networking;
pub mod networking_wrapping;
pub mod game;
pub mod server;
pub mod world;

use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::io;

fn echo_server(port: u16) -> Result<()> {
    let mut server = networking::server::ServerConnection::new(port)?;
    let mut stop = false;
    while !stop {
        for (id, data) in server.poll() {
            for packet in data {
                if String::from_utf8_lossy(packet.as_slice()).eq("stop") {
                    stop = true;
                }
                server.send_udp(vec![id], packet);
            }
        }
        server.flush();
        std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
    }
    Ok(())
}

fn console_client(address: SocketAddr) -> Result<()> {
    let mut client = networking::client::Connection::new(&address);
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || loop {
            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer).unwrap();
            tx.send(buffer).unwrap();
        });
        rx
    };
    loop {
        for packet in client.poll() {
            println!("Received from server: {}", std::str::from_utf8(packet.as_slice()).unwrap());
        }
        client.flush();
        let message = match stdin_channel.try_recv() {
            Ok(v) => v,
            Err(TryRecvError::Empty) => continue,
            Err(TryRecvError::Disconnected) => break,
        };
        client.send_udp(Vec::from(message.as_bytes()));
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let server_address: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    match args[1].as_str() {
        "echo_server" => {
            echo_server(1234)
        },
        "server" => {
            server::Server::run(1234);
            Ok(())
        },
        "gclient" => {
            game::Game::run(&server_address);
            Ok(())
        }
        "client" => { // client
            console_client(server_address)
        },
        _ => {
            println!("Unknown mode");
            Ok(())
        }
    }
}

