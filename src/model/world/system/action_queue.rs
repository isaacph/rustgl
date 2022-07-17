use std::{collections::HashMap, fmt::Debug};
use serde::{Serialize, Deserialize};
use crate::model::world::{World, WorldInfo, component::{ComponentID, GetComponentID}, character::CharacterID, WorldError};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum Action {
    Idle,
    Move,
    AutoAttack
}

pub struct ActionQueueInfo {
    // the priority of each action
    pub priority: HashMap<Action, i32>,

    // associated component with each action
    // if the component with the given component id is missing for a character
    // then the action will be removed
    pub component: HashMap<Action, ComponentID>,
}

impl ActionQueueInfo {
    pub fn init() -> Self {
        let mut priority = HashMap::new();
        let mut component = HashMap::new();
        priority.insert(Action::Move, 0);
        component.insert(Action::Move, ComponentID::Movement);
        priority.insert(Action::AutoAttack, 1);
        component.insert(Action::AutoAttack, ComponentID::AutoAttack);
        Self { priority, component }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ActionQueueEntry {
    pub action: Action,
    pub ttl: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionQueue {
    queue: Vec<ActionQueueEntry>
}

impl GetComponentID for ActionQueue {
    const ID: ComponentID = ComponentID::ActionQueue;
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ActionStatus {
    None,
    Current,
    Waiting(f32)
}

impl ActionQueue {
    pub fn new() -> Self {
        Self { queue: vec![] }
    }

    // adds an action to the queue. actions are sorted first by priority, then by order of adding
    // if timeout <= 0, then returns the status of the action without changing anything
    // if timeout > 0, action is already in queue, updates the action's timeout to max of current and given timeout
    // if timeout > 0, action is not in queue, then adds the action with the given timeout
    pub fn add_action(&mut self, info: &WorldInfo, action: Action, ttl: f32) -> ActionStatus {
        // find last action with lesser priority
        if action == Action::Idle {
            return self.get_status(action);
        }
        if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
            // already there
            let ttl = f32::max(self.queue[position].ttl, ttl);
            self.queue[position].ttl = ttl;
            if position == self.queue.len() - 1 {
                ActionStatus::Current
            } else {
                ActionStatus::Waiting(ttl)
            }
        } else {
            if ttl <= 0.0 {
                return ActionStatus::None
            }
            // need to add this action
            if let Some(add_priority) = info.action_queue.priority.get(&action) {
                // find the first action with less priority
                let mut position = 0;
                for i in (0..self.queue.len()).rev() {
                    let priority = if let Some(priority) = info.action_queue.priority.get(&self.queue[i].action) {
                        *priority
                    } else {
                        0
                    };
                    if priority < *add_priority {
                        position = i + 1;
                        break;
                    }
                };
                // insert this action at position after that lesser priority action
                self.queue.insert(position, ActionQueueEntry { action, ttl });
                if position == self.queue.len() - 1 {
                    ActionStatus::Current
                } else {
                    ActionStatus::Waiting(ttl)
                }
            } else {
                println!("Error: action does not have priority assigned: {:?}", action);
                ActionStatus::None
            }
        }
    }

    pub fn update(&mut self, info: &WorldInfo, components: Option<&Vec<ComponentID>>, delta_time: f32) {
        let mut to_remove = vec![];
        for i in 0..self.queue.len() {
            self.queue[i].ttl -= delta_time;
            let has_req_component = match components {
                Some(components) => match info.action_queue.component.get(&self.queue[i].action) {
                    Some(component) => components.iter().any(|component2| *component == *component2),
                    None => false,
                },
                None => true
            };
            if has_req_component && self.queue[i].ttl <= 0.0 {
                to_remove.push(i);
            }
        }
        for &i in to_remove.iter().rev() {
            self.queue.remove(i);
        }
    }

    pub fn get_current(&self) -> Option<ActionQueueEntry> {
        self.queue.get(self.queue.len() - 1).copied()
    }

    pub fn get_current_action(&self) -> Action {
        match self.get_current() {
            Some(ActionQueueEntry { action, ttl: _ }) => action,
            None => Action::Idle
        }
    }

    // finds the action in the queue and returns the status
    pub fn get_status(&self, action: Action) -> ActionStatus {
        if action == Action::Idle {
            if self.queue.is_empty() {
                return ActionStatus::Current;
            } else {
                return ActionStatus::None;
            }
        }
        if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
            if position == self.queue.len() - 1 {
                ActionStatus::Current
            } else {
                ActionStatus::Waiting(self.queue[position].ttl)
            }
        } else {
            ActionStatus::None
        }
    }

    // removes the action from the queue. if the action was present, returns its timeout
    // at the time of removal
    pub fn remove_action(&mut self, action: Action) -> Option<f32> {
        if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
            let entry = self.queue.remove(position);
            Some(entry.ttl)
        } else { None }
    }

    pub fn fix_action<'a, T>(&mut self, cid: &CharacterID, action: Action, corresponding: Option<&'a T>) -> Result<Option<&'a T>, WorldError>
            where T: Debug {
        let status = self.get_status(action);
        match (status, corresponding) {
            // valid states
            (ActionStatus::Current, Some(x)) => Ok(Some(x)),
            (ActionStatus::Waiting(_), Some(_)) |
            (ActionStatus::None, None) => Ok(None),
            // invalid states
            (ActionStatus::Current, None) |
            (ActionStatus::Waiting(_), None) => {
                self.remove_action(action);
                Err(WorldError::UnexpectedActionStatus(*cid, action, status))
            },
            (ActionStatus::None, Some(_)) => {
                Err(WorldError::MissingActionStatus(*cid, action))
            }
        }
    }
}

pub fn action_queue_system_init(_: &mut World) {
}

pub fn action_queue_system_update(world: &mut World, delta_time: f32) {
    let components_all: HashMap<CharacterID, Vec<ComponentID>> = world.action_queue.components.keys()
        .map(|cid| (*cid, world.get_components(cid))).collect();
    for (cid, action_queue) in &mut world.action_queue.components {
        let components = components_all.get(cid);
        action_queue.update(&world.info, components, delta_time);
    }
}