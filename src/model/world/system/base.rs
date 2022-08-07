use nalgebra::{Vector2, Vector3};
use serde::{Serialize, Deserialize};

use crate::model::world::{character::{CharacterType, CharacterID}, component::{GetComponentID, ComponentID, ComponentUpdateData, Component, ComponentUpdate}, WorldSystem, WorldInfo, WorldError, ComponentSystem, World, commands::{CharacterCommand, WorldCommand, Priority}, Update};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CharacterFlip {
    Left, Right
}

impl CharacterFlip {
    pub fn from_dir(dir: &Vector2<f32>) -> Option<CharacterFlip> {
        if dir.x > 0.0 {
            Some(CharacterFlip::Right)
        } else if dir.x < 0.0 {
            Some(CharacterFlip::Left)
        } else {
            None
        }
    }

    pub fn dir(&self) -> f32 {
        match *self {
            Self::Left => -1.0,
            Self::Right => 1.0
        }
    }

    pub fn combine(&self, other: &CharacterFlip) -> CharacterFlip {
        use CharacterFlip::*;
        match (*self, *other) {
            (Left, Left) => Left,
            (Right, Right) => Right,
            (Right, Left) => Right,
            (Left, Right) => Right
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CharacterBase {
    pub ctype: CharacterType,
    pub position: Vector3<f32>,
    pub center_offset: Vector3<f32>,
    pub speed: f32,
    pub attack_damage: f32,
    pub range: f32,
    pub attack_speed: f32,
    pub flip: CharacterFlip,
    pub targetable: bool,
}

impl Component for CharacterBase {
    fn update(&self, change: &ComponentUpdateData) -> Self {
        let mut next = self.clone();
        match change.clone() {
            ComponentUpdateData::Base(change) => match change {
                CharacterBaseUpdate::New(component) => next = component,
                CharacterBaseUpdate::Update(_, update) => match update {
                    CharacterBaseUpdateSwitch::FlipUpdate(flip) => next.flip = flip,
                    CharacterBaseUpdateSwitch::PositionUpdate(change) => match change {
                        CharacterBasePositionUpdate::Move(add) => next.position += add,
                        CharacterBasePositionUpdate::Override(pos) =>  next.position = pos,
                    }
                }
            },
            _ => (),
        }
        next
    }
}

impl Default for CharacterBase {
    fn default() -> Self {
        Self {
            ctype: CharacterType::Unknown,
            position: Vector3::default(),
            center_offset: Vector3::default(),
            speed: 0.0,
            attack_damage: 0.0,
            range: 0.0,
            attack_speed: 0.0,
            flip: CharacterFlip::Right,
            targetable: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum CharacterBasePositionUpdate {
    Move(Vector3<f32>),
    Override(Vector3<f32>),
}

impl CharacterBasePositionUpdate {
    pub fn combine(self, other: CharacterBasePositionUpdate) -> CharacterBasePositionUpdate {
        // when combining, overrides take priority, else add moves together
        match self {
            CharacterBasePositionUpdate::Move(d) => match other {
                CharacterBasePositionUpdate::Move(d2) => CharacterBasePositionUpdate::Move(d + d2),
                CharacterBasePositionUpdate::Override(_) => other,
            },
            CharacterBasePositionUpdate::Override(_) => self
        }
    }
}

pub fn empty_move() -> CharacterBaseUpdate {
    CharacterBaseUpdate::Update(
        Priority::Walk,
        CharacterBaseUpdateSwitch::PositionUpdate(
            CharacterBasePositionUpdate::Move(
                Vector3::new(0.0, 0.0, 0.0)
            )
        )
    )
}

pub fn empty_flip() -> CharacterBaseUpdate {
    CharacterBaseUpdate::Update(
        Priority::Walk,
        CharacterBaseUpdateSwitch::FlipUpdate(
            CharacterFlip::Right
        )
    )
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterBaseUpdate {
    New(CharacterBase),
    Update(Priority, CharacterBaseUpdateSwitch),
}

pub fn make_move_update(cid: CharacterID, priority: Priority, mov: Vector3<f32>) -> Update {
    Update::Comp(
        ComponentUpdate {
            cid,
            data: ComponentUpdateData::Base(
                CharacterBaseUpdate::Update(
                    priority,
                    CharacterBaseUpdateSwitch::PositionUpdate(
                        CharacterBasePositionUpdate::Move(mov))))
        })
}

pub fn make_flip_update(cid: CharacterID, priority: Priority, flip: CharacterFlip) -> Update {
    Update::Comp(
        ComponentUpdate {
            cid,
            data: ComponentUpdateData::Base(
                CharacterBaseUpdate::Update(
                    priority,
                    CharacterBaseUpdateSwitch::FlipUpdate(flip)))
        })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CharacterBaseUpdateSwitch {
    FlipUpdate(CharacterFlip),
    PositionUpdate(CharacterBasePositionUpdate),
}

impl CharacterBaseUpdateSwitch {
    pub fn pos(self) -> Option<CharacterBasePositionUpdate> {
        match self {
            CharacterBaseUpdateSwitch::PositionUpdate(pos) => Some(pos),
            _ => None,
        }
    }
    pub fn flip(self) -> Option<CharacterFlip> {
        match self {
            CharacterBaseUpdateSwitch::FlipUpdate(f) => Some(f),
            _ => None,
        }
    }
}

impl GetComponentID for CharacterBase {
    const ID: ComponentID = ComponentID::Base;
}

pub struct BaseSystem;

impl WorldSystem for BaseSystem {
    fn init_world_info(&self) -> Result<WorldInfo, WorldError> {
        Ok(WorldInfo::new())
    }
}

impl ComponentSystem for BaseSystem {
    fn get_component_id(&self) -> ComponentID {
        ComponentID::Base
    }

    // fn run_character_command(&self, world: &mut World, cid: &CharacterID, cmd: CharacterCommand) -> Result<(), WorldError> {
    //     Err(WorldError::InvalidCommandMapping)
    // }

    fn update_character(&self, _: &World, _: &Vec<WorldCommand>, _: &CharacterID, _: f32) -> Result<Vec<Update>, WorldError> {
        Ok(vec![])
    }

    fn validate_character_command(&self, _: &World, _: &CharacterID, _: &CharacterCommand) -> Result<(), WorldError> {
        Err(WorldError::InvalidCommandMapping)
    }

    fn reduce_changes(&self, cid: &CharacterID, world: &World, changes: &Vec<ComponentUpdateData>) -> Result<Vec<ComponentUpdateData>, WorldError> {
        if !world.characters.contains(cid) {
            // get status resets (called New)
            let new_changes: Vec<ComponentUpdateData> = changes.into_iter().filter(|new| match *new {
                ComponentUpdateData::Base(CharacterBaseUpdate::New(_)) => true,
                _ => false,
            }).cloned().collect();
            if new_changes.len() == 0 {
                return Err(WorldError::InvalidReduceMapping(*cid, ComponentID::AutoAttack))
            } else if new_changes.len() > 1 {
                return Err(WorldError::MultipleUpdateOverrides(*cid, ComponentID::AutoAttack))
            } else {
                return Ok(new_changes)
            }
        }
        let mut changes = changes.into_iter()
        .filter_map(|update| match update.clone() { // only allow base type updates
            ComponentUpdateData::Base(update) => Some(update),
            _ => None,
        });
        Ok(changes
            .find(|change| match *change { // check if we create a new component
                 CharacterBaseUpdate::New(_) => true,
                 _ => false,
            })
            .map_or(changes // if we don't create a new component
                .filter_map(|change| match change { // only look at updates
                    CharacterBaseUpdate::Update(prio, sw) => Some((prio, sw)),
                    _ => None,
                })
                .fold([None, None], |[flip, mv]: [Option<(Priority, CharacterBaseUpdateSwitch)>; 2], (prio, sw)| match sw.clone() {
                // this actually reduces updates (besides the type that overrides the component)
                    CharacterBaseUpdateSwitch::FlipUpdate(update) => flip.clone().map_or([Some((prio, sw.clone())), mv.clone()],
                    // flip updates take highest or equal priority, and override each other
                    |(flip_prio, flip_sw)| if prio > flip_prio {
                        [Some((prio, sw)), mv]
                    } else if prio == flip_prio {
                        [Some((
                            prio,
                            CharacterBaseUpdateSwitch::FlipUpdate(
                                flip_sw.flip()
                                .unwrap_or(CharacterFlip::Right)
                                .combine(&update)))),
                        mv]
                    } else {
                        [flip, mv]
                    }),
                    CharacterBaseUpdateSwitch::PositionUpdate(_update) => mv.clone().map_or([flip.clone(), Some((prio, sw.clone()))],
                    // move updates take highest priority, or if equal priority then combine
                    |(mv_prio, mv_sw)| if prio > mv_prio {
                        [flip, Some((prio, sw))]
                    } else if prio == mv_prio {
                        [flip, Some((
                            prio,
                            // combine call
                            sw.pos().zip(mv_sw.clone().pos())
                            .map_or(mv_sw.clone(), |(pos, mv_pos)| CharacterBaseUpdateSwitch::PositionUpdate(pos.combine(mv_pos)))))]
                    } else {
                        [flip, mv]
                    }),
                })
               .into_iter()
               .flat_map(|change| change.into_iter() // combine reduced updates
                   .map(|change| ComponentUpdateData::Base(CharacterBaseUpdate::Update(change.0, change.1)))).collect(),
           |new| vec![ComponentUpdateData::Base(new)]) // if we do have a "New" update then
                                                       // only reduce to just that update
        )
    }
}
