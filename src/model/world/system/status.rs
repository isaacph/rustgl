use std::{collections::HashMap, fmt::Debug};
use serde::{Serialize, Deserialize};
use crate::model::{world::{World, WorldInfo, component::{ComponentID, GetComponentID}, character::CharacterID, WorldError}, commands::CommandID};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StatusType {
    Passive,
    Active
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusEffect {
    pub typ: StatusType,
    pub duration: f32,
    pub uid: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Status {
//    queue: Vec<ActionQueueEntry>,
    pub effects: Vec<StatusEffect>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusInfo {
    pub originator: HashMap<StatusID, ComponentID>
}

impl GetComponentID for Status {
    const ID: ComponentID = ComponentID::Status;
}

pub fn status_system_init(_: &mut World) -> Result<(), WorldError> {
    Ok(())
}

pub fn status_system_update(world: &mut World, delta_time: f32) -> Result<(), WorldError> {
    let components_all: HashMap<CharacterID, Vec<ComponentID>> = world.action_queue.components.keys()
        .map(|cid| (*cid, world.get_components(cid))).collect();
    for (cid, status) in &mut world.status.components {
        let remove = vec![];
        for index in 0..status.effects.len() {
            let effect = &mut status.effects[index];
            effect.duration -= delta_time;
            if effect.duration <= 0.0 {
                remove.push(index);
            }
        }
        for index in remove.into_iter().rev() {
            status.effects.remove(index);
        }
    }
    Ok(())
}

impl Status {
    pub fn new() -> Self {
        Self {
            effects: vec![]
        }
    }

    pub fn get_status(&self, uid: StatusID) -> Option<&StatusEffect> {
        self.effects.iter().find(|effect| effect.uid == uid)
    }

    pub fn get_status_mut(&mut self, uid: StatusID) -> Option<&mut StatusEffect> {
        self.effects.iter_mut().find(|effect| effect.uid == uid)
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash, Ord)]
pub struct StatusID(u64);

pub struct StatusIDGenerator {
    counter: u64
}

impl StatusIDGenerator {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
    
    pub fn generate(&mut self) -> StatusID {
        let uid = self.counter;
        self.counter += 1;
        StatusID(uid)
    }
}

// #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
// pub enum Action {
//     Idle,
//     Move,
//     AutoAttackMove,
//     AutoAttack(AutoAttackPhase), // the phase where the player cannot interrupt
// }

//pub struct ActionQueueInfo {
//     // the priority of each action
//     pub priority: HashMap<Action, i32>,
// 
//     // associated component with each action
//     // if the component with the given component id is missing for a character
//     // then the action will be removed
//     pub component: HashMap<Action, ComponentID>,
//}

//impl ActionQueueInfo {
//    pub fn init() -> Self {
////         let mut priority = HashMap::new();
////         let mut component = HashMap::new();
////         priority.insert(Action::Move, 1);
////         component.insert(Action::Move, ComponentID::Movement);
////         priority.insert(Action::AutoAttackMove, 0);
////         component.insert(Action::AutoAttack, ComponentID::AutoAttack);
////         priority.insert(Action::AutoAttackCasting, 2);
////         component.insert(Action::AutoAttackCasting, ComponentID::AutoAttack);
////         Self { priority, component }
//    }
//}

// #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
// pub struct ActionQueueEntry {
//     action: Action,
//     ttl: f32,
// }
// // parameter is the amount of time before this action gets autoremoved from queue
// // however, the current action cannot be autoremoved, it will keep decreasing while staying in the
// // queue
// #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
// pub enum ActionStatus {
//     None,
//     Current(f32),
//     Waiting(f32)
// }
// 
// impl ActionStatus {
//     pub fn is_present(&self) -> bool {
//         match self {
//             ActionStatus::None => false,
//             ActionStatus::Current(_) => true,
//             ActionStatus::Waiting(_) => true,
//         }
//     }
//     pub fn is_none(&self) -> bool {
//         !self.is_present()
//     }
// }

//impl ActionQueue {
    // pub fn new() -> Self {
    //     Self { queue: vec![], carry_time: 0.0 }
    // }

    // // adds an action to the queue. actions are sorted first by priority, then by order of adding
    // // if timeout <= 0, then returns the status of the action without changing anything
    // // if timeout > 0, action is already in queue, updates the action's timeout to max of current and given timeout
    // // if timeout > 0, action is not in queue, then adds the action with the given timeout
    // pub fn add_action(&mut self, info: &WorldInfo, action: Action, ttl: f32) -> ActionStatus {
    //     // find last action with lesser priority
    //     if action == Action::Idle {
    //         return self.get_status(action);
    //     }
    //     if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
    //         // already there
    //         let ttl = f32::max(self.queue[position].ttl, ttl);
    //         self.queue[position].ttl = ttl;
    //         if position == self.queue.len() - 1 {
    //             ActionStatus::Current(ttl)
    //         } else {
    //             ActionStatus::Waiting(ttl)
    //         }
    //     } else {
    //         if ttl <= 0.0 {
    //             return ActionStatus::None
    //         }
    //         // need to add this action
    //         if let Some(add_priority) = info.action_queue.priority.get(&action) {
    //             // find the first action with less priority
    //             let mut position = 0;
    //             for i in (0..self.queue.len()).rev() {
    //                 let priority = if let Some(priority) = info.action_queue.priority.get(&self.queue[i].action) {
    //                     *priority
    //                 } else {
    //                     0
    //                 };
    //                 if priority < *add_priority {
    //                     position = i + 1;
    //                     break;
    //                 }
    //             };
    //             // insert this action at position after that lesser priority action
    //             self.queue.insert(position, ActionQueueEntry { action, ttl });
    //             if position == self.queue.len() - 1 {
    //                 ActionStatus::Current(ttl)
    //             } else {
    //                 ActionStatus::Waiting(ttl)
    //             }
    //         } else {
    //             println!("Error: action does not have priority assigned: {:?}", action);
    //             ActionStatus::None
    //         }
    //     }
    // }

    // // can be used to promote or demote an action by switching the action with a new one with more
    // // or less priority
    // // so for auto attacking, the actual casting phase of the attack has higher priority than
    // // moving, but the wind up and down phases have lesser priority
    // //
    // // if you swap with an action of the same priority, then you take that action's place in queue
    // // if you demote, you take the front of that level of the queue
    // pub fn swap_action(&mut self, info: &WorldInfo, old_action: Action, new_action: Action, ttl: f32, cid: &CharacterID) -> Result<ActionStatus, WorldError> {
    //     let old_pos = self.queue.iter().position(|other| old_action == other.action)
    //         .ok_or(WorldError::MissingActionStatus(*cid, old_action))?;
    //     let ActionQueueEntry { action: _, ttl: old_ttl } = self.queue.remove(old_pos);
    //     let new_prio = info.action_queue.priority.get(&new_action)
    //         .ok_or(WorldError::MissingActionDescription(new_action))?;
    //     let mut insert_pos = None;
    //     for i in (old_pos..self.queue.len()).rev() {
    //         let i_prio = info.action_queue.priority.get(&self.queue[i].action)
    //             .ok_or(WorldError::MissingActionDescription(self.queue[i].action))?;
    //         if i_prio < new_prio {
    //             insert_pos = Some(i + 1);
    //             break;
    //         }
    //     }
    //     if insert_pos.is_none() {
    //         for i in (0..old_pos).rev() {
    //             let i_prio = info.action_queue.priority.get(&self.queue[i].action)
    //                 .ok_or(WorldError::MissingActionDescription(self.queue[i].action))?;
    //             if i_prio <= new_prio {
    //                 insert_pos = Some(i + 1);
    //                 break;
    //             }
    //         }
    //     }
    //     self.queue.insert(insert_pos.unwrap_or(0), ActionQueueEntry {
    //         action: new_action,
    //         ttl: old_ttl + ttl
    //     });
    //     Ok(self.get_status(new_action))
    // }

    // pub fn add_carry_time(&mut self, carry_time: f32) {
    //     self.carry_time += carry_time;
    // }

    // // resets and returns the current carry time for the current action
    // pub fn reset_carry_time(&mut self) -> f32 {
    //     let ct = self.carry_time;
    //     self.carry_time = 0.0;
    //     ct
    // }

    // pub fn update(&mut self, info: &WorldInfo, components: Option<&Vec<ComponentID>>, delta_time: f32) {
    //     let mut to_remove = vec![];
    //     for i in 0..self.queue.len() {
    //         self.queue[i].ttl -= delta_time;
    //         let has_req_component = match components {
    //             Some(components) => match info.action_queue.component.get(&self.queue[i].action) {
    //                 Some(component) => components.iter().any(|component2| *component == *component2),
    //                 None => false,
    //             },
    //             None => true
    //         };
    //         if has_req_component && self.queue[i].ttl <= 0.0 {
    //             to_remove.push(i);
    //         }
    //     }
    //     for &i in to_remove.iter().rev() {
    //         self.queue.remove(i);
    //     }
    // }

    // pub fn get_current(&self) -> Option<ActionQueueEntry> {
    //     self.queue.get(self.queue.len() - 1).copied()
    // }

    // pub fn get_current_action(&self) -> Action {
    //     match self.get_current() {
    //         Some(ActionQueueEntry { action, ttl: _ }) => action,
    //         None => Action::Idle
    //     }
    // }

    // // finds the action in the queue and returns the status
    // pub fn get_status(&self, action: Action) -> ActionStatus {
    //     if action == Action::Idle {
    //         if self.queue.is_empty() {
    //             return ActionStatus::Current(0.0);
    //         } else {
    //             return ActionStatus::None;
    //         }
    //     }
    //     if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
    //         if position == self.queue.len() - 1 {
    //             ActionStatus::Current(self.queue[position].ttl)
    //         } else {
    //             ActionStatus::Waiting(self.queue[position].ttl)
    //         }
    //     } else {
    //         ActionStatus::None
    //     }
    // }

    // // removes the action from the queue. if the action was present, returns its timeout
    // // at the time of removal
    // pub fn remove_action(&mut self, action: Action) -> Option<f32> {
    //     if let Some(position) = self.queue.iter().position(|entry| entry.action == action) {
    //         let entry = self.queue.remove(position);
    //         Some(entry.ttl)
    //     } else { None }
    // }
    // // pub fn fix_action<T>(&mut self, cid: &CharacterID, action: Action, corresponding: Option<T>) -> Result<Option<T>, WorldError>
    // //         where T: Debug {
    // //     self.fix_actions(cid, &[(action, corresponding)]).map(|(cor, _)| cor)
    // // }

    // // pub fn fix_actions<T>(&mut self, cid: &CharacterID, actions: &[(Action, Option<T>)]) -> Result<Option<(T, Action)>, WorldError>
    // //         where T: Debug {
    // //     let status = self.get_status(action);
    // //     match (status, corresponding) {
    // //         // valid states
    // //         (ActionStatus::Current(_), Some(x)) => Ok(Some((x, status))),
    // //         (ActionStatus::Waiting(_), Some(_)) |
    // //         (ActionStatus::None, None) => Ok(None),
    // //         // invalid states
    // //         (ActionStatus::Current(_), None) |
    // //         (ActionStatus::Waiting(_), None) => {
    // //             self.reset_carry_time();
    // //             self.remove_action(action);
    // //             Err(WorldError::UnexpectedActionStatus(*cid, action, status))
    // //         },
    // //         (ActionStatus::None, Some(_)) => {
    // //             Err(WorldError::MissingActionStatus(*cid, action))
    // //         }
    // //     }
    // // }

    // pub fn reset_ttl(&mut self, cid: &CharacterID, action: Action) -> Result<ActionStatus, WorldError> {
    //     let entry = self.queue.iter_mut()
    //         .find(|entry| entry.action == action)
    //         .ok_or(WorldError::MissingActionStatus(*cid, action))?;
    //     entry.ttl = 0.0;
    //     Ok(self.get_status(action))
    // }
//}
