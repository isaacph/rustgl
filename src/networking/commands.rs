// use serde::{Serialize, Deserialize};

#[macro_export]
#[doc(hidden)]
macro_rules! _commands_execute_static_def {
    ($command_trait_name:ident,
        $context_type:ty) => {
        pub trait $command_trait_name<'a>: serde::Serialize + serde::Deserialize<'a> {
            fn run(self, context: $context_type);
        }
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! _commands_id_static_def {
    ($id_trait_name:ident) => {
        pub trait $id_trait_name: serde::Serialize {
            fn id(&self) -> u16;
            fn make_bytes(&self) -> Box<[u8]>;
        }
    }
}

#[macro_export]
macro_rules! commands_id {
    ($id_trait_name:ident, @step $idx:expr, ) => {};
    ($id_trait_name:ident, @step $idx:expr, $head:path, $($tail:path,)*) => {
        impl $id_trait_name for $head {
            fn id(&self) -> u16 {
                $idx
            }
            fn make_bytes(&self) -> Box<[u8]> {
                let mut data: Vec<u8> = bincode::serialize(self).unwrap(); // TODO: error handling
                let mut id = Vec::from(($idx as u16).to_be_bytes());
                data.append(&mut id);
                data.into_boxed_slice()
            }
        }
        commands_id!($id_trait_name, @step $idx + 1u16, $($tail,)*);
    };
    // requires trailing comma
    ($id_trait_name:ident,
        [$head:path, $($tail:path,)*]) => {
        _commands_id_static_def!(
            $id_trait_name);
        commands_id!($id_trait_name, @step 0u16, $head, $($tail,)*);
    };
    // support for no trailing comma
    ($id_trait_name:ident,
        [$head:path, $($tail:path),*]) => {
        commands_id!($id_trait_name, [$head, $($tail),* ,]);
    };
    // support for single no comma
    ($id_trait_name:ident, [$head:path]) => {
        commands_id!($id_trait_name, [$head,]);
    };
    // support for empty
    ($id_trait_name:ident, []) => {}
}

#[macro_export]
macro_rules! commands_execute {
    // generates ability to execute commands + command ids
    // requires trailing comma (compatibility is below)
    ($execute_fn_name:ident,
        $command_trait_name:ident,
        $id_trait_name:ident,
        $context_type:ty,
        [$head:path, $($tail:path,)*] ) => {
        // commands_id!($id_trait_name, $serialized_command_name, [$head, $($tail,)*]);
        _commands_execute_static_def!(
            $command_trait_name,
            $context_type);
        pub fn $execute_fn_name(data: &[u8], context: $context_type) -> bincode::Result<()> {
            let cmdid = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
            let cmd = &data[..data.len() - 2];
            commands_execute!($command_trait_name, @step2 0u16, context, cmdid, cmd, $head, $($tail,)*);
        }
    };

    ($command_trait_name:ident, @step2 $_idx:expr, $context:ident, $cmdid:ident, $cmd:ident, ) => {
        return Err(Box::new(bincode::ErrorKind::Custom(format!("Invalid command id: {}", $cmdid))));
    };
    ($command_trait_name:ident, @step2 $idx:expr, $context:ident, $cmdid:ident, $cmd:ident, $head:path, $($tail:path,)*) => {
        if $cmdid == $idx {
            let deserialized: $head = bincode::deserialize(&$cmd)?; // TODO: error handling
            $command_trait_name::run(deserialized, $context);
            return Ok(())
        }
        commands_execute!($command_trait_name, @step2 $idx + 1u16, $context, $cmdid, $cmd, $($tail,)*);
    };

    // allow non-borrows to also be serialized (without extra steps)

    // compatibility with trailing comma for client command list:
    ($execute_fn_name:ident,
        $command_trait_name:ident,
        $id_trait_name:ident,
        $context_type:ty,
        [$head:path, $($tail:path),*] ) => {
        commands_execute!(
            $execute_fn_name,
            $command_trait_name,
            $id_trait_name,
            $context_type,
            [$head, $($tail,)*]
        );
    };

    // compatibility with single and no comma
    ($execute_fn_name:ident,
        $command_trait_name:ident,
        $id_trait_name:ident,
        $execute_command:ident,
        $context_type:ty,
        [$head:path] ) => {
        commands_execute!($execute_fn_name,
            $command_trait_name,
            $id_trait_name,
            $context_type,
            [$head,]
        );
    };

    // compatibility with empty
    ($execute_fn_name:ident,
        $command_trait_name:ident,
        $id_trait_name:ident,
        $context_type:ty,
        [] ) => {};
}

