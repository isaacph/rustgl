use std::collections::HashMap;

use crate::model::{world::{ComponentID, ComponentSystem, character::CharacterID, World, WorldError, commands::{CharacterCommand, WorldCommand}, Update, component::{ComponentUpdateData, Component, GetComponentID, ComponentStorageContainer, ComponentUpdate}, WorldSystem, WorldInfo}, WorldTick};
use itertools::Itertools;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusID {
    Idle,
    Walk,
    AutoAttack,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum StatusPrio {
    Impossible, // never gets executed
    Lowest,
    //AbilityQueued,
    Ability,
    AbilityOverride,
    AbilityPrioritized,
    Stunned,
}

impl StatusPrio {
    pub fn get_prio(self: &StatusPrio) -> i32 {
        use StatusPrio::*;
        match *self {
            Impossible => -1,
            Idle => 0,
            //AbilityQueued => 1,
            Ability => 2,
            AbilityOverride => 3,
            AbilityPrioritized => 4,
            Stunned => 5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub prio: StatusPrio,
    pub id: StatusID,
    pub timeout: WorldTick,
    pub start: WorldTick, // time when this status was requested
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StatusComponent {
    pub status: Status,
    pub runner_up: Status,
}

pub fn idle_status(start: WorldTick) -> Status {
    Status {
        id: StatusID::Idle,
        prio: StatusPrio::Lowest,
        timeout: WorldTick::MAX,
        start,
    }
}

impl GetComponentID for StatusComponent {
    const ID: ComponentID = ComponentID::Status;
}

impl Component for StatusComponent {
    fn update(&self, update: &ComponentUpdateData) -> Self {
        use ComponentUpdateData::Status;
        use StatusUpdate::*;
        match *update {
            Status(RunnerUp(runner_up)) => Self {
                status: self.status,
                runner_up,
            },
            // only one of these two should arrive
            Status(New(status)) => Self { status, runner_up: self.runner_up },
            Status(Try(status)) => Self { status, runner_up: self.runner_up },
            _ => self.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StatusUpdate {
    New(Status), // should only be used for initializing statuses of new characters
                 // note that if two New statuses are passed for the same character,
                 // status will fail to change and an error will be thrown
    Try(Status),
    Cancel(StatusID),
    RunnerUp(Status), // reserved for reduce stage output, will be discarded if it comes as input
    Reevaluate,
    ChangePrio(StatusID, StatusPrio),
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
        let StatusComponent { status, runner_up } = world.status.get_component(cid)?;
        if status.timeout <= world.tick || runner_up.timeout <= world.tick {
            use ComponentUpdateData::Status as CStatus;
            use StatusUpdate::*;
            Ok(vec![Update::Comp(ComponentUpdate {
                cid: *cid,
                data: CStatus(Reevaluate)
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
        })
        .filter(|id| *id != StatusID::Idle);

        // get prio changes
        let prio_changes: HashMap<StatusID, StatusPrio> = changes.into_iter().filter_map(|x| match *x {
            CStatus(ChangePrio(id, new)) => Some((id, new)),
            _ => None
        })
        // deduplicate prio changes
        .group_by(|change| change.0)
        .into_iter()
        .flat_map(|(id, group)| group
            .max_by_key(|(_, prio)| prio.get_prio())
            .into_iter())
        .collect();

        // get status resets (called New)
        let new_changes = changes.into_iter().filter_map(|new| match *new {
            CStatus(New(new)) => Some(new),
            _ => None,
        })
        // remove canceled statuses
        .filter(|status| cancel.any(|id| status.id == id));

        match new_changes.count() {
            1 => // prioritize New over other status types
                Ok(new_changes.map(|new| CStatus(New(new))).collect()),
            0 =>  {// reduce most prioritized state
                // get status change requests (called Try)
                let mut iter = changes.into_iter()
                    .filter_map(|change| match *change {
                        CStatus(Try(change)) => Some(change),
                        _ => None
                    })
                    // add current state
                    .chain([
                        // add runner_up state
                        world.status.get_component(cid)?.clone().status,
                        // add minimum (idle) state
                        world.status.get_component(cid)?.clone().runner_up]
                        // carry out prio changes
                        .into_iter()
                        .map(|status| match prio_changes.get(&status.id) {
                            Some(prio) => Status {
                                id: status.id,
                                prio: *prio,
                                timeout: status.timeout,
                                start: status.start
                            },
                            None => status
                        }))
                    // remove canceled statuses
                    .filter(|status| !cancel.any(|id| status.id == id))
                    // deduplicate by id
                    .group_by(|status| status.id)
                    .into_iter()
                    .flat_map(|(_, group)| group
                        .max_by_key(|status| (status.start, status.timeout))
                        .into_iter())
                    // sort by prio then id
                    .sorted_unstable_by_key(|status| std::cmp::Reverse(
                        (status.prio.get_prio(), status.id)
                    ))
                    .take(2);
                let (status, runner_up) = (iter.next(), iter.next());
                Ok(status
                   .into_iter()
                   .map(|status| CStatus(Try(status)))
                   .chain(runner_up
                       .into_iter()
                       .map(|status| CStatus(RunnerUp(status))))
                   .collect())
            },
            _ => // there are multiple New commands
                Err(WorldError::MultipleNewCommands(*cid, ComponentID::Status))
        }
    }
}
