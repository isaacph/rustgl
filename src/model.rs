use serde::{Serialize, Deserialize};
use crate::{commands_id, _commands_id_static_def};

// define all client and server command data structures
// they must all be listed in the macro below to auto generate an ID for them to be serialized
// this enables commands to be serialized on both the client and the server
// to execute a command, the client or server side must add the command name to
// the commands_execute macro list, and the method of execution must be specified by
// implementing the ClientCommand or ServerCommand traits

#[derive(Serialize, Deserialize)]
pub struct GetAddress;

#[derive(Serialize, Deserialize)]
pub struct SendAddress(pub String);

#[derive(Serialize, Deserialize)]
pub struct SetUDPAddress(pub String);

#[derive(Serialize, Deserialize)]
pub struct EchoMessage(pub String);

commands_id!(
    ClientCommandID,
    SerializedClientCommand,
    [
        SendAddress,
        EchoMessage
    ]
);

commands_id!(
    ServerCommandID,
    SerializedServerCommand,
    [
        GetAddress,
        SetUDPAddress,
        EchoMessage
    ]
);

// impl Into<SerializedServerCommand> for GetAddress {
//     fn into(self) -> SerializedServerCommand {
//         (&self).into()
//     }
// }