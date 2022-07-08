use crate::{commands_id, _commands_id_static_def};

// define all client and server command data structures
// they must all be listed in the macro below to auto generate an ID for them to be serialized
// this enables commands to be serialized on both the client and the server
// to execute a command, the client or server side must add the command name to
// the commands_execute macro list, and the method of execution must be specified by
// implementing the ClientCommand or ServerCommand traits

pub mod core {
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    pub struct GetAddress;
    
    #[derive(Serialize, Deserialize)]
    pub struct SendAddress(pub String);
    
    #[derive(Serialize, Deserialize)]
    pub struct SetUDPAddress(pub String);
    
    #[derive(Serialize, Deserialize)]
    pub struct EchoMessage(pub String);
}

commands_id!(
    ClientCommandID,
    [
        crate::model::commands::core::SendAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::player::commands::ChatMessage,
        crate::model::player::commands::PlayerDataPayload,
        crate::model::world::commands::UpdateCharacter,
    ]
);

commands_id!(
    ServerCommandID,
    [
        crate::model::commands::core::GetAddress,
        crate::model::commands::core::SetUDPAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::player::commands::ChatMessage,
        crate::model::player::commands::PlayerLogIn,
        crate::model::player::commands::PlayerLogOut,
        crate::model::player::commands::GetPlayerData,
        crate::model::player::commands::PlayerSubs,
        crate::model::world::commands::GenerateCharacter,
        crate::model::world::commands::UpdateCharacter,
    ]
);

// impl Into<SerializedServerCommand> for GetAddress {
//     fn into(self) -> SerializedServerCommand {
//         (&self).into()
//     }
// }
