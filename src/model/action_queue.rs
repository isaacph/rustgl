use super::world::commands::WorldCommand;


#[derive(Default)]
pub struct ActionQueue {
    pub next_action: Option<WorldCommand>,
}

impl ActionQueue {
    pub fn enqueue(self: &mut ActionQueue, action: WorldCommand) {
        self.next_action = Some(action);
    }
}
