use serde::{Serialize, Deserialize};

use crate::model::world::World;

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoAttack {

}

pub fn auto_attack_system_init(_: &mut World) {

}

pub fn auto_attack_system_update(_: &mut World, _: f32) {
    
}