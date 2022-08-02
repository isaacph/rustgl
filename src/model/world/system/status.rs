
use crate::model::{world::{ComponentID, ComponentSystem, character::CharacterID, World, WorldError, commands::{CharacterCommand, WorldCommand}, Update, component::{ComponentUpdateData, Component, GetComponentID, ComponentStorageContainer, ComponentUpdate}, WorldSystem, WorldInfo}, WorldTick};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Actively {
    Idle,
    Casting {
        overridable: bool,
        caster: ComponentID,
    },
    Stunned,
}

impl Actively {
    pub fn priority(&self) -> i32 {
        use Actively::*;
        match *self {
            Idle => 0,
            Casting { overridable: true, caster } => match caster {
                ComponentID::Movement => 10,
                ComponentID::AutoAttack => 11,
                _ => 10,
            },
            Casting { overridable: false, caster: _ } => 20,
            Stunned => 30,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub actively: Actively,
    pub timeout: WorldTick,
}

pub fn idle_status() -> Status {
    use Actively::*;
    Status { actively: Idle, timeout: i32::MAX }
}

impl GetComponentID for Status {
    const ID: ComponentID = ComponentID::Status;
}

impl Component for Status {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        use ComponentUpdateData::Status;
        use StatusUpdate::*;
        match *update {
            Status(New(rewrite)) => rewrite,
            Status(Try(switch)) => switch,
            _ => self.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StatusUpdate {
    New(Status),
    Try(Status),
    Cancel(ComponentID),
}

pub struct StatusSystem;

impl WorldSystem for StatusSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for StatusSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Status
    }
    fn validate_character_command(&self, world: &World, cid: &CharacterID, cmd: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }

    fn update_character(&self, world: &World, commands: &Vec<WorldCommand>, cid: &CharacterID, delta_time: f32) -> Result<Vec<Update>, WorldError> {
        // check if we need an update for timeout
        let status = world.status.get_component(cid)?;
        if status.timeout <= world.tick {
            use ComponentUpdateData::Status as CStatus;
            use StatusUpdate::*;
            Ok(vec![Update::Comp(ComponentUpdate {
                cid: *cid,
                data: CStatus(Try(idle_status()))
            })])
        } else {
            Ok(vec![])
        }
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        use ComponentUpdateData::Status as CStatus;
        use StatusUpdate::*;
        let cancel = changes.into_iter().filter_map(|data| match *data {
            CStatus(Cancel(cancel)) => Some(cancel),
            _ => None,
        });
        match changes.into_iter()
            .map(|c| match *c {
                CStatus(New(_)) => 1,
                _ => 0,
            }).sum() {
            1 => // replace with the singular New command
                Ok(changes.into_iter()
                .find(|x| match *x {
                    CStatus(New(_)) => true,
                    _ => false
                }).cloned().into_iter().collect()),
            0 =>  // reduce most prioritized state
                Ok(changes.into_iter()
                   // add current state
                   .chain(std::iter::once(&CStatus(Try(world.status.get_component(cid)?.clone()))))
                   .map(|update| match *update {
                       CStatus(Try(Status { actively, timeout })) =>
                        // check timeout
                       if timeout <= world.tick {
                           (update, -1) // makes this result impossible
                       } else {
                           // check canceled
                           match actively {
                               Actively::Casting { overridable, caster } => {
                                   if cancel.any(|c| c == caster) {
                                       (update, -1) // makes this result impossible
                                   } else {
                                       (update, actively.priority())
                                   }
                               },
                               _ => (update, actively.priority())
                           }
                       },
                       _ => (update, -2) // please be more impossible
                   })
                   // choose highest priority
                   .max_by_key(|(update, prio)| prio)
                   // collect result
                   .map(|(update, _)| update.clone())
                   .into_iter()
                   .collect()),
            _ => // there are multiple New commands
                Err(WorldError::MultipleNewCommands(*cid, ComponentID::Status))
        }
    }
}
