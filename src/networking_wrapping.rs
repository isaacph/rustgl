use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::{game::Game, networking::{client::Connection, server::{ClientID, ServerConnection}}, server::Server};

// kinda convulated right now, and the implementation is super repetitive
// (TODO: try to generify the code into oblivion)
// current strategy for new commands: just add it to the macro list below (ignore the macro definitions)

macro_rules! client_commands {
    (@step2 $idx:expr, $client:ident, $w:ident, ) => {
        panic!("Invalid client command id: {}", $w.0);
    };
    (@step2 $idx:expr, $client:ident, $w:ident, $head:tt, $($tail:tt,)*) => {
        if $w.0 == $idx { // pattern matching won't let me use generated expressions like $idx D:
            $client.run::<$head>(&$w);
            return;
        }
        client_commands!(@step2 $idx + 1u16, $client, $w, $($tail,)*);
    };
    (@step $idx:expr, ) => {};
    (@step $idx:expr, $head:path, $($tail:path,)*) => {
        impl ClientCommandID for $head {
            fn id(&self) -> u16 {
                $idx
            }
        }
        client_commands!(@step $idx + 1u16, $($tail,)*);
    };
    ( $( $x: path ),* ) => {
        fn execute_client_command(client: &mut Game, w: ClientCommandWrapper) {
            client_commands!(@step2 0u16, client, w, $($x,)*);
        }
        client_commands!(@step 0u16, $($x,)*);
    };
}

macro_rules! server_commands {
    (@step2 $_idx:expr, $c:ident, $w:ident, ) => {
        panic!("Invalid server command id: {}", $w.0)
    };
    (@step2 $idx:expr, $c:ident, $w:ident, $head:tt, $($tail:tt,)*) => {
        if $w.0 == $idx {
            $c.run::<$head>(&$w);
            return;
        }
        server_commands!(@step2 $idx + 1u16, $c, $w, $($tail,)*);
    };
    (@step $idx:expr, ) => {};
    (@step $idx:expr, $head:path, $($tail:path,)*) => {
        impl ServerCommandID for $head {
            fn id(&self) -> u16 {
                $idx
            }
        }
        server_commands!(@step $idx + 1u16, $($tail,)*);
    };
    ( $( $x: path ),* ) => {
        fn execute_server_command(c: (&mut Server, &ClientID), w: ServerCommandWrapper) {
            server_commands!(@step2 0u16, c, w, $($x,)*);
        }
        server_commands!(@step 0u16, $($x,)*);
    };
}


client_commands!(crate::game::EchoMessage);
server_commands!(crate::game::EchoMessage, crate::server::StopServer);

// fn execute_client_command(client: &mut Game, w: ClientCommandWrapper) {
//     match w.0 {
//         0 => client.run::<crate::game::EchoMessage>(&w),
//         _ => panic!("Invalid client command id: {}", w.0)
//     }
// }

// impl ClientCommandIDT for crate::game::EchoMessage {
//     fn id(&self) -> u16 {
//         0
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// pub enum ServerCommandID {
//     EchoMessage,
//     StopServer
// }

// fn execute_server_command(c: (&mut Server, &ClientID), w: ServerCommandWrapper) {
//     match w.0 {
//         0 => c.run::<crate::game::EchoMessage>(&w),
//         1 => c.run::<crate::server::StopServer>(&w),
//         _ => panic!("Invalid server command id: {}", w.0)
//     };
// }





// implementation below:
pub trait ClientCommandID {
    fn id(&self) -> u16;
}
pub trait ClientCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, client: &mut Game);
}

trait RunCommandShortenerClient {
    fn run<'a, T: ClientCommand<'a>>(self, wrapper: &'a ClientCommandWrapper);
}
impl RunCommandShortenerClient for &mut Game<'_> {
    fn run<'a, T: ClientCommand<'a>>(self, wrapper: &'a ClientCommandWrapper) {
        let x: T = bincode::deserialize(&wrapper.1).unwrap();
        ClientCommand::run(&x, self);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientCommandWrapper(pub u16, pub Vec<u8>);

pub trait ExecuteClientCommands {
    fn execute(&mut self, commands: Vec<Vec<u8>>);
}

impl ExecuteClientCommands for Game<'_> {
    fn execute(&mut self, commands: Vec<Vec<u8>>) {
        for mut data in commands {
            let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
            data.truncate(data.len() - 2);
            let wrapper = ClientCommandWrapper(id, data);
            execute_client_command(self, wrapper);
        }
    }
}

pub trait SendServerCommands {
    fn send<'a, T>(&mut self, command: &T) where T: ServerCommand<'a> + ServerCommandID;
}

impl SendServerCommands for Connection {
    fn send<'a, T>(&mut self, command: &T)
    where T: ServerCommand<'a> + ServerCommandID {
        // TODO: remove duplicated serializing
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        // let wrapper = ServerCommandWrapper(command.id(), data);
        // let wrapped: Vec<u8> = bincode::serialize(&wrapper).unwrap(); // TODO: error handling
        self.send_raw(data);
    }
}

trait RunCommandShortenerServer {
    fn run<'a, T: ServerCommand<'a>>(self, wrapper: &'a ServerCommandWrapper);
}
impl RunCommandShortenerServer for (&mut Server, &ClientID) {
    fn run<'a, T: ServerCommand<'a>>(self, wrapper: &'a ServerCommandWrapper) {
        let (server, id) = self;
        let x: T = bincode::deserialize(&wrapper.1).unwrap();
        ServerCommand::run(&x, id, server);
    }
}

pub trait ServerCommandID {
    fn id(&self) -> u16;
}

pub trait ServerCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, source: &ClientID, server: &mut Server);
}

pub trait SendClientCommands {
    fn send<'a, T>(&mut self, client: &ClientID, command: &T) where T: ClientCommand<'a> + ClientCommandID;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerCommandWrapper(pub u16, pub Vec<u8>);

impl SendClientCommands for ServerConnection {
    fn send<'a, T>(self: &mut ServerConnection, client: &ClientID, command: &T)
    where T: ClientCommand<'a> + ClientCommandID {
        // TODO: remove duplicated serializing
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        // let wrapper = ServerCommandWrapper(command.id(), data);
        // let wrapped: Vec<u8> = bincode::serialize(&wrapper).unwrap(); // TODO: error handling
        self.send_raw(client, data);
    }
}

pub trait ExecuteServerCommands {
    fn execute(&mut self, commands: HashMap<ClientID, Vec<Vec<u8>>>);
}

impl ExecuteServerCommands for Server {
    fn execute(&mut self, commands: HashMap<ClientID, Vec<Vec<u8>>>) {
        for (client_id, queue) in commands {
            for mut data in queue {
                let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
                data.truncate(data.len() - 2);
                let wrapper = ServerCommandWrapper(id, data);
                execute_server_command((self, &client_id), wrapper);
            }
        }
    }
}