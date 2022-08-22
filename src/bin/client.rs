
#[cfg(feature = "client")]
extern crate glfw;

use std::net::ToSocketAddrs;
use std::net::SocketAddr;

#[cfg(feature = "client")]
use rustgl::client::game;

pub fn auto_addr() -> (SocketAddr, SocketAddr) {
    let auto_addr_str = ("127.0.0.1:1234", "127.0.0.1:1235");
    // let auto_addr_str = ("test.neotrias.link:1234", "test.neotrias.link:1235");
    (
        auto_addr_str.0.to_socket_addrs().unwrap().next().unwrap(),
        auto_addr_str.1.to_socket_addrs().unwrap().next().unwrap()
    )
}

fn main() {
    #[cfg(feature = "client")]
    game::Game::run(Some(auto_addr()));
}
