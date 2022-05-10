use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::{game::Game, networking::{client::Connection, server::{ClientID, ServerConnection}}, server::Server};

// kinda convulated right now, and implementing code is repetitive
// (TODO: try to generify the code into oblivion)
// current strategy for new commands:
//   add a client/server command id enum and then map the enum to
//   the struct name in corresponding execute_*_command function

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ClientCommandID {
    EchoMessage
}

fn execute_client_command(client: &mut Game, w: ClientCommandWrapper) {
    use ClientCommandID::*;
    match w.0 {
        EchoMessage => client.run::<crate::game::EchoMessage>(&w)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerCommandID {
    EchoMessage,
    StopServer
}

fn execute_server_command(c: (&mut Server, &ClientID), w: ServerCommandWrapper) {
    use ServerCommandID::*;
    match w.0 {
        EchoMessage => c.run::<crate::game::EchoMessage>(&w),
        StopServer => c.run::<crate::server::StopServer>(&w),
    };
}





// implementation below:
pub trait ClientCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, client: &mut Game);
    fn id(&self) -> ClientCommandID;
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
pub struct ClientCommandWrapper(pub ClientCommandID, pub Vec<u8>);

pub trait ExecuteClientCommands {
    fn execute(&mut self, commands: Vec<Vec<u8>>);
}

impl ExecuteClientCommands for Game<'_> {
    fn execute(&mut self, commands: Vec<Vec<u8>>) {
        for data in commands {
            let wrapper: ClientCommandWrapper = bincode::deserialize(data.as_slice()).unwrap();
            execute_client_command(self, wrapper);
        }
    }
}

pub trait SendServerCommands {
    fn send<'a, T>(&mut self, command: &T) where T: ServerCommand<'a>;
}

impl SendServerCommands for Connection {
    fn send<'a, T>(&mut self, command: &T)
    where T: ServerCommand<'a> {
        // TODO: remove duplicated serializing
        let data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let wrapper = ServerCommandWrapper(command.id(), data);
        let wrapped: Vec<u8> = bincode::serialize(&wrapper).unwrap(); // TODO: error handling
        self.send_raw(wrapped);
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

pub trait ServerCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, source: &ClientID, server: &mut Server);
    fn id(&self) -> ServerCommandID;
}

pub trait SendClientCommands {
    fn send<'a, T>(&mut self, client: &ClientID, command: &T) where T: ClientCommand<'a>;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerCommandWrapper(pub ServerCommandID, pub Vec<u8>);

impl SendClientCommands for ServerConnection {
    fn send<'a, T>(self: &mut ServerConnection, client: &ClientID, command: &T)
    where T: ClientCommand<'a> {
        // TODO: remove duplicated serializing
        let data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let wrapper = ClientCommandWrapper(command.id(), data);
        let wrapped: Vec<u8> = bincode::serialize(&wrapper).unwrap(); // TODO: error handling
        self.send_raw(client, wrapped);
    }
}

pub trait ExecuteServerCommands {
    fn execute(&mut self, commands: HashMap<ClientID, Vec<Vec<u8>>>);
}

impl ExecuteServerCommands for Server {
    fn execute(&mut self, commands: HashMap<ClientID, Vec<Vec<u8>>>) {
        for (client_id, queue) in commands {
            for data in queue {
                let wrapper: ServerCommandWrapper = bincode::deserialize(data.as_slice()).unwrap(); // TODO: error handling
                execute_server_command((self, &client_id), wrapper);
            }
        }
    }
}