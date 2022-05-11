use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::{game::Game, networking::{client::Connection, server::{ConnectionID, ServerConnection}}, server::Server};

// command handling macro
macro_rules! commands {
    ($command_trait_name:ident, @step2 $_idx:expr, $context:ident, $cmdid:ident, $cmd:ident, ) => {
        panic!("Invalid server command id: {}", $cmdid)
    };
    ($command_trait_name:ident, @step2 $idx:expr, $context:ident, $cmdid:ident, $cmd:ident, $head:path, $($tail:path,)*) => {
        if $cmdid == $idx {
            let deserialized: $head = bincode::deserialize(&$cmd).unwrap(); // TODO: error handling
            $command_trait_name::run(&deserialized, $context);
            return;
        }
        commands!($command_trait_name, @step2 $idx + 1u16, $context, $cmdid, $cmd, $($tail,)*);
    };
    ($id_trait_name:ident, @step $idx:expr, ) => {};
    ($id_trait_name:ident, @step $idx:expr, $head:path, $($tail:path,)*) => {
        impl $id_trait_name for $head {
            fn id(&self) -> u16 {
                $idx
            }
        }
        commands!($id_trait_name, @step $idx + 1u16, $($tail,)*);
    };
    ($execute_fn_name:ident, $command_trait_name:ident, $id_trait_name:ident, $context_type:ty, [$( $x: path ),*] ) => {
        fn $execute_fn_name(context: $context_type, cmdid: u16, cmd: Vec<u8>) {
            commands!($command_trait_name, @step2 0u16, context, cmdid, cmd, $($x,)*);
        }
        pub trait $id_trait_name {
            fn id(&self) -> u16;
        }
        commands!($id_trait_name, @step 0u16, $($x,)*);
    };
}

// generate handling for client and server commands


pub trait ClientCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, client: &mut Game);
}

commands!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    &mut Game,
    // list client commands here:
    [crate::game::EchoMessage, crate::game::ChatMessage]
);

pub trait ServerCommand<'a>: Serialize + Deserialize<'a> {
    fn run(&self, context: (&ConnectionID, &mut Server));
}

commands!(
    execute_server_command,
    ServerCommand,
    ServerCommandID,
    (&ConnectionID, &mut Server),
    // list server commands here:
    [crate::game::EchoMessage, crate::server::StopServer, crate::game::ChatMessage]
);


// ----------------------- send/execute function implementations below -----------------------

pub trait SendClientCommands {
    fn send<'a, T>(&mut self, client: Vec<ConnectionID>, command: &T) where T: ClientCommand<'a> + ClientCommandID;
}

impl SendClientCommands for ServerConnection {
    fn send<'a, T>(self: &mut ServerConnection, clients: Vec<ConnectionID>, command: &T)
    where T: ClientCommand<'a> + ClientCommandID {
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        self.send_raw(clients, data);
    }
}

pub trait ExecuteClientCommands {
    fn execute(&mut self, commands: Vec<Vec<u8>>);
}

impl ExecuteClientCommands for Game<'_> {
    fn execute(&mut self, commands: Vec<Vec<u8>>) {
        for mut data in commands {
            let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
            data.truncate(data.len() - 2);
            execute_client_command(self, id, data);
        }
    }
}

pub trait SendServerCommands {
    fn send<'a, T>(&mut self, command: &T) where T: ServerCommand<'a> + ServerCommandID;
}

impl SendServerCommands for Connection {
    fn send<'a, T>(&mut self, command: &T)
    where T: ServerCommand<'a> + ServerCommandID {
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        self.send_raw(data);
    }
}

pub trait ExecuteServerCommands {
    fn execute(&mut self, commands: HashMap<ConnectionID, Vec<Vec<u8>>>);
}

impl ExecuteServerCommands for Server {
    fn execute(&mut self, commands: HashMap<ConnectionID, Vec<Vec<u8>>>) {
        for (client_id, queue) in commands {
            for mut data in queue {
                let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
                data.truncate(data.len() - 2);
                execute_server_command((&client_id, self), id, data);
            }
        }
    }
}