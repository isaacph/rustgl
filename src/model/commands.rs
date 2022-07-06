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

pub mod player {
    use serde::{Serialize, Deserialize};
    use crate::model::{world::player::PlayerData, Subscription};

    #[derive(Serialize, Deserialize)]
    pub struct ChatMessage(pub String);

    #[derive(Serialize, Deserialize)]
    pub struct PlayerDataPayload(pub PlayerData);

    #[derive(Serialize, Deserialize)]
    pub struct GetPlayerData;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct PlayerLogIn {
        pub existing: bool,
        pub name: Option<String>
    }

    #[derive(Serialize, Deserialize)]
    pub struct PlayerLogOut;

    #[derive(Serialize, Deserialize)]
    pub enum PlayerSubCommand {
        ListSubs,
        AddSubs(Vec<Subscription>),
        DelSubs(Vec<Subscription>),
        SetSubs(Vec<Subscription>),
    }

    #[derive(Serialize, Deserialize)]
    pub struct PlayerSubs(pub PlayerSubCommand);
}

commands_id!(
    ClientCommandID,
    [
        crate::model::commands::core::SendAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::commands::player::ChatMessage,
        crate::model::commands::player::PlayerDataPayload,
        crate::model::world::commands::UpdateCharacter,
    ]
);

commands_id!(
    ServerCommandID,
    [
        crate::model::commands::core::GetAddress,
        crate::model::commands::core::SetUDPAddress,
        crate::model::commands::core::EchoMessage,
        crate::model::commands::player::ChatMessage,
        crate::model::commands::player::PlayerLogIn,
        crate::model::commands::player::PlayerLogOut,
        crate::model::commands::player::GetPlayerData,
        crate::model::world::commands::GenerateCharacter,
        crate::model::world::commands::UpdateCharacter,
        crate::model::commands::player::PlayerSubs,
    ]
);

// impl Into<SerializedServerCommand> for GetAddress {
//     fn into(self) -> SerializedServerCommand {
//         (&self).into()
//     }
// }
