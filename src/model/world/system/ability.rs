use itertools::Itertools;
use serde::{Serialize, Deserialize};

use crate::model::{WorldTick, world::{character::CharacterID, CharacterCommandState, WorldError, World, component::{ComponentStorageContainer, ComponentUpdate, ComponentUpdateData}, Update, WorldErrorI}, TICK_RATE};

use super::{auto_attack::fsm::{Fsm, self}, status::{StatusID, StatusPrio, Status, StatusUpdate}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Execution<E> {
    pub time_start: WorldTick,
    pub duration: f32,
    pub cooldown: f32,
    pub data: E,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ability<E> {
    pub status_id: StatusID,
    pub cooldown_timeout: WorldTick, // cooldown
    pub execution: Option<Execution<E>>, // currently executing attack
}

impl<'a, E> Ability<E> where E: Serialize + Deserialize<'a> + Clone {
    pub fn new(status_id: StatusID) -> Self {
        Self { status_id, cooldown_timeout: WorldTick::MIN, execution: None }
    }

    pub fn get_override_status(&self) -> Status {
        Status {
            prio: StatusPrio::AbilityOverride,
            id: self.status_id
        }
    }

    pub fn combine(
        self: &Ability<E>,
        cooldown_timeout: Option<WorldTick>,
        execution: Option<Option<Execution<E>>>) -> Option<AbilityUpdate<E>> {
        if cooldown_timeout.is_none() && execution.is_none() {
            return None;
        }
        let mut a = self.clone();
        if let Some(t) = cooldown_timeout {
            a.cooldown_timeout = t;
        }
        if let Some(e) = execution {
            a.execution = e;
        }
        Some(AbilityUpdate(a))
    }

    pub fn validate_command(&self, world: &World, cid: &CharacterID, cmd: &AbilityCommand<E>) -> Result<CharacterCommandState, WorldError> {
        if cmd.duration < 0.0 || cmd.cooldown < 0.0 {
            return Err(WorldErrorI::InvalidCommandData(*cid, "Ability cannot have negative cooldown or duration".to_string()).err());
        }
        if self.get_override_status().can_override(&world.status.get_component(cid)?.current) {
            return Ok(CharacterCommandState::Ready);
        }
        Ok(CharacterCommandState::Queued)
    }

    pub fn update<T>(&self, world: &World, info: &AbilityInfo, commands: &Vec<AbilityCommand<E>>, cid: &CharacterID, delta_time: f32, fire: T) -> Result<(Option<AbilityUpdate<E>>, Vec<Update>), WorldError>
            where T: Fn(&World, &CharacterID) -> Result<Vec<Update>, WorldError> {
        let status = &world.status.get_component(cid)?.current;
        let is_status = status.id == self.status_id;
        let (executing, casting, fire_attack_updates, cooldown_updates) = {
            let executing;
            let (mut casting, mut fire_attack_updates) = (false, vec![]);
            let mut cooldown_updates = None;
            if let Some(execution) = &self.execution {
                let timer = (world.tick - execution.time_start) as f32 * 1.0 / TICK_RATE;
                let (changes, _changed) = info.fsm.get_state_changes(
                        execution.duration,
                        timer,
                        timer + delta_time);
                changes.iter().fold(Ok(()), |status, change| {
                    match (status?, change) {
                    ((), fsm::Changes::Event(_time_since, FireEvent)) => {
                        fire_attack_updates = fire(world, cid)?;
                        cooldown_updates = Some(
                            world.tick +
                            f32::ceil(
                                (execution.cooldown - (world.tick - execution.time_start) as f32 / TICK_RATE)
                                * TICK_RATE)
                            as WorldTick);
                        Ok(())
                    },
                    _ => Ok(()),
                }})?;
                let state = info.fsm.get_current_state(execution.duration, timer + delta_time);
                executing = state != Phase::Complete;
                casting = state == Phase::Casting;
            } else {
                executing = false;
            }

            (executing, casting, fire_attack_updates, cooldown_updates)
        };
        let on_cooldown = self.cooldown_timeout > world.tick;
        let command = commands.iter().reduce(|a, b|
            if a.cooldown < b.cooldown || a.cooldown == b.cooldown && a.duration <= b.duration {
                a
            } else { b }
        );

        // execution
        let execution_update = if executing && !is_status {
            Some(None)
        } else if !casting && !on_cooldown && command.is_some() {
            let command = command.ok_or_else(|| WorldErrorI::BadLogic.err())?;
            // start execution
            Some(Some(Execution {
                duration: command.duration,
                cooldown: command.cooldown,
                time_start: world.tick + 1,
                data: command.exec_data.clone(),
            }))
        } else if !executing && self.execution.is_some() {
            Some(None)
        } else {
            None
        };

        // status
        let status_update = if executing && is_status && casting {
            // maintain prioritized status priority
            if status.prio != StatusPrio::AbilityPrioritized {
                Some(StatusUpdate::ChangePrio(self.status_id, StatusPrio::AbilityPrioritized))
            } else {
                None
            }
        } else if command.is_some() {
            // new override status priority
            Some(StatusUpdate::Try(self.get_override_status().prio, Status {
                id: self.status_id,
                prio: StatusPrio::Ability,
            }))
        } else if executing && is_status && !casting {
            // maintain regular status priority
            if status.prio != StatusPrio::Ability {
                Some(StatusUpdate::ChangePrio(self.status_id, StatusPrio::Ability))
            } else {
                None
            }
        } else if is_status && !executing {
            Some(StatusUpdate::Cancel(self.status_id))
        } else {
            None
        };

        // map updates to Update type
        let ability_update = self.combine(cooldown_updates, execution_update);
        let update_status_updates = status_update.into_iter()
            .map(|status| Update::Comp(ComponentUpdate {
                cid: *cid,
                data: ComponentUpdateData::Status(status)
            }));

        // return collected updates
        let collected_updates = update_status_updates.into_iter()
           .chain(fire_attack_updates.into_iter())
           .collect_vec();
        if !collected_updates.is_empty() {
            // println!("AA Updates: {:?}", collected_updates);
        }
        Ok((ability_update, collected_updates))
    }

    pub fn reduce(&self, _cid: &CharacterID, _world: &World, changes: &[AbilityUpdate<E>]) -> Result<Option<AbilityUpdate<E>>, WorldError> {
        Ok(changes.iter()
           .reduce(|a, b|
            if a.0.execution.is_some() && (b.0.execution.is_none() ||
               a.0.cooldown_timeout > b.0.cooldown_timeout ||
               a.0.cooldown_timeout == b.0.cooldown_timeout && a.0.status_id >= b.0.status_id) {
               a } else { b }
        ).cloned())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    WindUp,
    Casting,
    WindDown,
    Complete
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct FireEvent;

#[derive(Clone)]
pub struct AbilityInfo {
    pub fsm: Fsm<Phase, FireEvent>,
}

impl AbilityInfo {
    pub fn new(
        wind_up_time: f32,
        casting_time: f32,
        wind_down_time: f32,
        fire_time: f32) -> Result<AbilityInfo, WorldError> {
        Ok(Self {
            fsm: Fsm::new(vec![
                (wind_up_time, Phase::WindUp),
                (casting_time, Phase::Casting),
                (wind_down_time, Phase::WindDown)
            ],
            Phase::Complete,
            &[(fire_time, FireEvent)]).ok_or_else(|| WorldErrorI::BadLogic.err())?
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AbilityCommand<E> {
    pub duration: f32,
    pub cooldown: f32,
    pub exec_data: E,
}

#[derive(Debug, Clone)]
pub struct AbilityUpdate<E>(pub Ability<E>);
