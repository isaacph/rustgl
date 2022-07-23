use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::Serialize;

// no longer applies --
// define all client and server command data structures
// they must all be listed in the macro below to auto generate an ID for them to be serialized
// this enables commands to be serialized on both the client and the server
// to execute a command, the client or server side must add the command name to
// the commands_execute macro list, and the method of execution must be specified by
// implementing the ClientCommand or ServerCommand traits

#[derive(IntoPrimitive, TryFromPrimitive, Debug, Clone, Copy)]
#[repr(u16)]
pub enum CommandID {
    // place all internal command IDs here
    
    // commands to run on server
    GetAddress,
    SetUDPAddress,
    PlayerLogIn,
    PlayerLogOut,
    GetPlayerData,
    PlayerSubs,
    GenerateCharacter,
    MoveCharacterRequest,
    ListChar,
    EnsureCharacter,
    IndicateClientPlayer,

    // commands to run on client
    SendAddress,
    PlayerDataPayload,

    // commands to run on both
    EchoMessage,
    ChatMessage,
    UpdateCharacter,

    // world commands (also on both, but focus run on world)
    MoveCharacter,
    AutoAttackCommand,
    AutoAttackRequest,
    ClearWorld,
}

pub mod core {
    use serde::{Serialize, Deserialize};
    use super::GetCommandID;

    #[derive(Serialize, Deserialize)]
    pub struct GetAddress;
    
    #[derive(Serialize, Deserialize)]
    pub struct SendAddress(pub String);
    
    #[derive(Serialize, Deserialize)]
    pub struct SetUDPAddress(pub String);
    
    #[derive(Serialize, Deserialize)]
    pub struct EchoMessage(pub String);

    impl GetCommandID for GetAddress {
        fn command_id(&self) -> super::CommandID {
            super::CommandID::GetAddress
        }
    }

    impl GetCommandID for SendAddress {
        fn command_id(&self) -> super::CommandID {
            super::CommandID::SendAddress
        }
    }
    
    impl GetCommandID for SetUDPAddress {
        fn command_id(&self) -> super::CommandID {
            super::CommandID::SetUDPAddress
        }
    }
    
    impl GetCommandID for EchoMessage {
        fn command_id(&self) -> super::CommandID {
            super::CommandID::EchoMessage
        }
    }
}

// commands_id!(
//     ClientCommandID,
//     [
//         crate::model::commands::core::SendAddress,
//         crate::model::commands::core::EchoMessage,
//         crate::model::player::commands::ChatMessage,
//         crate::model::player::commands::PlayerDataPayload,
//         crate::model::world::commands::UpdateCharacter,
//     ]
// );

// commands_id!(
//     ServerCommandID,
//     [
//         crate::model::commands::core::GetAddress,
//         crate::model::commands::core::SetUDPAddress,
//         crate::model::commands::core::EchoMessage,
//         crate::model::player::commands::ChatMessage,
//         crate::model::player::commands::PlayerLogIn,
//         crate::model::player::commands::PlayerLogOut,
//         crate::model::player::commands::GetPlayerData,
//         crate::model::player::commands::PlayerSubs,
//         crate::model::world::commands::GenerateCharacter,
//         crate::model::world::commands::UpdateCharacter,
//     ]
// );

pub trait MakeBytes {
    fn make_bytes(&self) -> Result<Box<[u8]>, bincode::Error>;
}

impl<T: GetCommandID> MakeBytes for T {
    fn make_bytes(&self) -> Result<Box<[u8]>, bincode::Error> {
        let mut data: Vec<u8> = bincode::serialize(self)?; // TODO: error handling
        let id_u16: u16 = self.command_id().into();
        let mut id = Vec::from(id_u16.to_be_bytes());
        data.append(&mut id);
        Ok(data.into_boxed_slice())
    }
}

// let mut data: Vec<u8> = bincode::serialize(command)?; // TODO: error handling
//         let id_u16: u16 = command.server_command_id().into();
//         let mut id = Vec::from(id_u16.to_be_bytes());
//         data.append(&mut id);
//         Ok(data.into_boxed_slice())

pub trait GetCommandID: Serialize {
    fn command_id(&self) -> CommandID;
}

// impl Into<SerializedServerCommand> for GetAddress {
//     fn into(self) -> SerializedServerCommand {
//         (&self).into()
//     }
// }
