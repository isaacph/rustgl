use std::collections::HashMap;

use crate::model::{world::{ComponentID, ComponentSystem, character::CharacterID, World, WorldError, commands::{CharacterCommand, WorldCommand}, Update, component::{ComponentUpdateData, Component, GetComponentID, ComponentStorageContainer, ComponentUpdate}, WorldSystem, WorldInfo}, WorldTick};
use itertools::Itertools;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub enum StatusID {
    Idle,
    Walk,
    AutoAttack,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
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
            Lowest => 0,
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

impl Default for StatusComponent {
    fn default() -> Self {
        Self { status: idle_status(WorldTick::MIN), runner_up: vec![] }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StatusComponent {
    pub status: Status,
    pub runner_up: Vec<Status>,
}

impl StatusComponent {
    pub fn status_queued(&self, id: StatusID) -> bool {
        self.status.id == id || self.runner_up.iter().any(|r| r.id == id)
    }
    pub fn statuses_ahead<'a>(&'a self, id: StatusID) -> Vec<&'a Status> {
        if id == self.status.id {
            vec![]
        } else if let Some(p) = self.runner_up.iter().rev().position(|s| s.id == id) {
            (&self.runner_up).iter().skip(p).collect()
        } else {
            vec![]
        }
    }
    pub fn get_status<'a>(&'a self, id: StatusID) -> Option<&'a Status> {
        if id == self.status.id {
            Some(&self.status)
        } else {
            self.runner_up.iter().find(|s| s.id == id)
        }
    }
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
        match update.clone() {
            Status(RunnerUp(runner_up)) => Self {
                status: self.status.clone(),
                runner_up,
            },
            // only one of these two should arrive
            Status(New(status)) => Self { status, runner_up: self.runner_up.clone() },
            Status(Try(status)) => Self { status, runner_up: self.runner_up.clone() },
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
    RunnerUp(Vec<Status>), // reserved for reduce stage output, will be discarded if it comes as input
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
    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }

    fn update_character(&self, world: &World, _: &Vec<WorldCommand>, cid: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        // check if we need an update for timeout
        let StatusComponent { status, runner_up } = world.status.get_component(cid)?;
        if status.timeout <= world.tick || runner_up.iter().any(|r| r.timeout <= world.tick) {
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
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.into_iter().filter(|new| match *new {
                CStatus(New(_)) => true,
                _ => false,
            }).cloned().collect();
            if new_changes.len() == 0 {
                return Err(WorldError::InvalidReduceMapping(*cid, ComponentID::Status))
            } else if new_changes.len() > 1 {
                return Err(WorldError::MultipleUpdateOverrides(*cid, ComponentID::Status))
            } else {
                return Ok(new_changes)
            }
        }
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
        .flat_map(|(_, group)| group
            .max_by_key(|(_, prio)| prio.get_prio())
            .into_iter())
        .collect();

        // get status change requests (called Try)
        let mut iter = changes.into_iter()
            .filter_map(|change| match change.clone() {
                CStatus(Try(change)) => Some(change),
                _ => None
            })
            // add current state
            .chain(
                // add previous state
                [world.status.get_component(cid)?.clone().status].into_iter()
                // add runner_up state
                .chain(world.status.get_component(cid)?.clone().runner_up.into_iter())
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
            // add minimum (idle) state
            .chain([idle_status(world.tick)].into_iter())
            // remove canceled statuses
            .filter(|status| !cancel.clone().any(|id| status.id == id))
            // deduplicate by id
            .group_by(|status| status.id)
            .into_iter()
            .flat_map(|(_, group)| group
                .max_by_key(|status| (status.start, status.timeout))
                .into_iter())
            // sort by prio then id
            .sorted_unstable_by_key(|status| std::cmp::Reverse(
                (status.prio.get_prio(), status.id)
            ));
        let (status, runner_up): (Option<Status>, Vec<Status>) = (iter.next(), iter.collect());
        Ok(status
           .into_iter()
           .map(|status| CStatus(Try(status)))
           .chain(std::iter::once(CStatus(RunnerUp(runner_up))))
           .collect())
    }
}
