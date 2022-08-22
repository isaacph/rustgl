#[cfg(feature = "server")]
use rustgl::server::main::Server;

fn main() {
    #[cfg(feature = "server")]
    Server::run((1234, 1235)).unwrap();
}
