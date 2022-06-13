use serde::{Serialize, Deserialize};
use crate::{game::Game, networking::server::ConnectionID, server::Server};

// command execution macro
macro_rules! commands {
    ($command_trait_name:ident, @step2 $_idx:expr, $context:ident, $cmdid:ident, $cmd:ident, ) => {
        panic!("Invalid server command id: {}", $cmdid)
    };
    ($command_trait_name:ident, @step2 $idx:expr, $context:ident, $cmdid:ident, $cmd:ident, $head:path, $($tail:path,)*) => {
        if $cmdid == $idx {
            let deserialized: $head = bincode::deserialize(&$cmd).unwrap(); // TODO: error handling
            $command_trait_name::run(deserialized, $context);
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
        fn $execute_fn_name<'a>(context: $context_type, cmdid: u16, cmd: &'a [u8]) {
            commands!($command_trait_name, @step2 0u16, context, cmdid, cmd, $($x,)*);
        }
        pub trait $id_trait_name {
            fn id(&self) -> u16;
        }
        commands!($id_trait_name, @step 0u16, $($x,)*);
    };
    // compatibility with trailing comma for client command list:
    ($execute_fn_name:ident, $command_trait_name:ident, $id_trait_name:ident, $context_type:ty, [$( $x: path, )*] ) => {
        commands!($execute_fn_name, $command_trait_name, $id_trait_name, $context_type, [$($x),*]);
    };
}

// generate handling for client and server commands

pub trait ClientCommand<'a>: Serialize + Deserialize<'a> {
    fn run(self, client: &mut Game);
}

commands!(
    execute_client_command,
    ClientCommand,
    ClientCommandID,
    &mut Game,
    // list client commands here:
    [
        crate::game::EchoMessage,
        crate::game::ChatMessage,
        crate::world::UpdateCharacter,
        crate::server::EmptyCommand,
        crate::world::player::PlayerDataPayload,
    ]
);

pub trait ServerCommand<'a>: Serialize + Deserialize<'a> {
    fn run(self, context: (&ConnectionID, &mut Server));
}

commands!(
    execute_server_command,
    ServerCommand,
    ServerCommandID,
    (&ConnectionID, &mut Server),
    // list server commands here:
    [
        crate::game::EchoMessage,
        crate::server::StopServer,
        crate::game::ChatMessage,
        crate::world::UpdateCharacter,
        crate::world::GenerateCharacter,
        crate::server::EmptyCommand,
        crate::world::player::PlayerLogIn,
        crate::world::player::PlayerLogOut,
    ]
);

pub struct SerializedServerCommand {
    pub data: Vec<u8>
}

pub struct SerializedClientCommand {
    pub data: Vec<u8>
}

impl SerializedServerCommand {
    pub fn new(data: Vec<u8>) -> Self {
        SerializedServerCommand { data }
    }
    pub fn from<'a, T>(command: &T) -> Self
    where T: ServerCommand<'a> + ServerCommandID {
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        SerializedServerCommand {
            data
        }
    }
    pub fn execute(&self, client_id: &ConnectionID, server: &mut Server) {
        let data = &self.data;
        let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
        execute_server_command((client_id, server), id, &data.as_slice()[..data.len() - 2]);
    }
}

impl SerializedClientCommand {
    pub fn new(data: Vec<u8>) -> Self {
        SerializedClientCommand { data }
    }
    pub fn from<'a, T>(command: &T) -> Self
    where T: ClientCommand<'a> + ClientCommandID {
        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
        let mut id = Vec::from(command.id().to_be_bytes());
        data.append(&mut id);
        SerializedClientCommand {
            data
        }
    }
    pub fn execute(&self, client: &mut Game) {
        let data = &self.data;
        let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
        execute_client_command(client, id, &data.as_slice()[..data.len() - 2]);
    }
}

pub trait SerializedWrapperDecay {
    fn decay(self) -> Vec<u8>;
}

impl SerializedWrapperDecay for SerializedServerCommand {
    fn decay(self) -> Vec<u8> {
        self.data
    }
}

impl SerializedWrapperDecay for SerializedClientCommand {
    fn decay(self) -> Vec<u8> {
        self.data
    }
}

pub trait VecSerializedWrapperDecay {
    fn decay(self) -> Vec<Vec<u8>>;
}

impl<T> VecSerializedWrapperDecay for Vec<T> where T: SerializedWrapperDecay {
    fn decay(self) -> Vec<Vec<u8>> {
        self.into_iter().map(|ser| ser.decay()).collect()
    }
}
//// ----------------------- send/execute function implementations below -----------------------
//
//pub trait SendClientCommands {
//    fn send<'a, T>(&mut self, client: Vec<ConnectionID>, command: &T) where T: ClientCommand<'a> + ClientCommandID;
//}
//
//impl SendClientCommands for ServerConnection {
//    fn send<'a, T>(self: &mut ServerConnection, clients: Vec<ConnectionID>, command: &T)
//    where T: ClientCommand<'a> + ClientCommandID {
//        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
//        let mut id = Vec::from(command.id().to_be_bytes());
//        data.append(&mut id);
//        self.send_raw(clients, data);
//    }
//}
//
//pub trait ExecuteClientCommands {
//    fn execute(&mut self, commands: Vec<Vec<u8>>);
//}
//
//impl ExecuteClientCommands for Game<'_> {
//    fn execute(&mut self, commands: Vec<Vec<u8>>) {
//        for mut data in commands {
//            let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
//            data.truncate(data.len() - 2);
//            execute_client_command(self, id, data);
//        }
//    }
//}
//
//pub trait SendServerCommands {
//    fn send<'a, T>(&mut self, command: &T) where T: ServerCommand<'a> + ServerCommandID;
//}
//
//impl SendServerCommands for Connection {
//    fn send<'a, T>(&mut self, command: &T)
//    where T: ServerCommand<'a> + ServerCommandID {
//        let mut data: Vec<u8> = bincode::serialize(command).unwrap(); // TODO: error handling
//        let mut id = Vec::from(command.id().to_be_bytes());
//        data.append(&mut id);
//        self.send_raw(data);
//    }
//}
//
//pub trait ExecuteServerCommands {
//    fn execute(&mut self, commands: HashMap<ConnectionID, Vec<Vec<u8>>>);
//}
//
//impl ExecuteServerCommands for Server {
//    fn execute(&mut self, commands: HashMap<ConnectionID, Vec<Vec<u8>>>) {
//        for (client_id, queue) in commands {
//            for mut data in queue {
//                let id = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
//                data.truncate(data.len() - 2);
//                execute_server_command((&client_id, self), id, data);
//            }
//        }
//    }
//}
