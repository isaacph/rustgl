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
}

impl Default for StatusComponent {
    fn default() -> Self {
        Self { current: idle_status() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StatusComponent {
    pub current: Status,
}

pub fn idle_status() -> Status {
    Status {
        id: StatusID::Idle,
        prio: StatusPrio::Lowest,
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
            Status(New(status)) => Self { current: status },
            _ => self.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StatusUpdate {
    New(Status), // should only be used for initializing statuses of new characters
                 // note that if two New statuses are passed for the same character,
                 // status will fail to change and an error will be thrown
    Try(StatusPrio, Status), // the first param is the queueing/overriding priority
    Cancel(StatusID),
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
        Ok(vec![])
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
                CStatus(Try(prio, change)) => Some((prio, change)),
                _ => None
            })
            // add current state
            .chain(
                // add previous state
                [world.status.get_component(cid)?.clone().current].into_iter()
                // carry out prio changes
                .into_iter()
                .map(|status| match prio_changes.get(&status.id) {
                    Some(prio) => (*prio, Status {
                        id: status.id,
                        prio: *prio,
                    }),
                    None => (status.prio, status)
                }))
            // add minimum (idle) state
            .chain([idle_status()].into_iter().map(|s| (s.prio, s)))
            // remove canceled statuses
            .filter(|(_prio, status)| !cancel.clone().any(|id| status.id == id))
            // deduplicate by id
            .map(|(prio, status)| (status.id, (prio, status)))
            .into_group_map()
            .into_iter()
            .flat_map(|(_, group)| group.into_iter()
                .max_by_key(|(prio, status)| prio.get_prio())
                .into_iter())
            // sort by prio then id
            .sorted_unstable_by(|(a_prio, a), (b_prio, b)| a_prio.get_prio().cmp(&b_prio.get_prio()).reverse()
                                .then(a.id.cmp(&b.id)));
        let status: Option<Status> = iter.next().map(|(_, status)| status);
        Ok(status
           .into_iter()
           .map(|status| CStatus(New(status))) // New indicates this is supposed to be the only one
           .collect())
    }
}
