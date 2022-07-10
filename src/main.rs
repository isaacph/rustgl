extern crate glfw;

pub mod graphics;
// pub mod chatbox;
// pub mod networking_wrapping;
pub mod networking;
pub mod model;
pub mod client;
pub mod server;
// pub mod game;
// pub mod server;
// pub mod world;

use std::net::ToSocketAddrs;
use std::{io::Result, net::SocketAddr};
use std::env;
use std::io::{self, Write};

use client::game;
use networking::example::both::{echo_server_both, console_client_both};
use networking::example::{echo_server_udp, console_client_udp, console_client_tcp, echo_server_tcp};

pub fn grab_console_line(prompt: &str) -> String {
    let mut buffer = String::new();
    io::stdout().write_all(prompt.as_bytes()).unwrap();
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut buffer).unwrap();
    String::from(buffer.trim())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let auto_addr = Some(("neotrias.link:1234".to_socket_addrs().unwrap().next().unwrap(), "neotrias.link:1235".to_socket_addrs().unwrap().next().unwrap()));
    match args[1].as_str() {
        "client" => {
            game::Game::run(auto_addr);
        },
        _ => {
            let ports: (u16, u16) =
                        (grab_console_line("UDP port: ").parse().expect("Invalid port"),
                         grab_console_line("TCP port: ").parse().expect("Invalid port"));
            let addresses: (SocketAddr, SocketAddr) = (
                format!("127.0.0.1:{}", ports.0).parse().unwrap(),
                format!("127.0.0.1:{}", ports.1).parse().unwrap()
            );
            match args[1].as_str() {
                "server" => {
                    server::main::Server::run(ports)?
                },
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
        }
    }
    Ok(())
}

