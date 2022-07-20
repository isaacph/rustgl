
//pub struct State {
//    pub id: i32,
//    pub start_time: f32,
//}

#[derive(Clone)]
pub struct State<E> {
    typ: E,
    id: usize,
    start_time: f32,
}

#[derive(Clone)]
pub struct Fsm<E: Clone, V: Clone> {
    states: Vec<State<E>>,
    ending: E,
    events: Vec<(f32, V)>,
    end_events: Vec<V>,
}

#[derive(Clone)]
pub enum Changes<E: Clone, V: Clone> {
    StateChange(f32, E), // f32 is the time elapsed since this event or state change started
    Event(f32, V)
}

impl<E: Clone, V: Clone> Changes<E, V> {
    pub fn get_time(&self) -> f32 {
        match *self {
            Changes::StateChange(time, _) => time,
            Changes::Event(time, _) => time
        }
    }
}

impl<E: Clone + Copy + Eq, V: Clone + Copy> Fsm<E, V> {
    pub fn new(mut states: Vec<(f32, E)>, ending: E, events: &[(f32, V)]) -> Option<Fsm<E, V>> {
        let sum = {
            let mut sum = states.iter().map(|(t, _)| f32::abs(*t)).sum();
            if sum <= 0.0 {
                sum = 1.0;
            }
            sum
        };
        let (events, end_events): (Vec<_>, Vec<_>) = events.iter().partition(|event| event.0 >= sum);
        let mut s = Self {
            states: states.iter_mut().fold((vec![], 0.0), |(mut states, prev_duration), (next, typ)| {
                let placeholder = State {
                    typ: ending,
                    id: usize::MAX,
                    start_time: 0.0
                };
                let prev_start = states.last().unwrap_or(&placeholder).start_time;
                states.push(State {
                    typ: *typ,
                    id: states.len(),
                    start_time: prev_start + prev_duration / sum
                });
                (states, *next)
            }).0,
            ending,
            events,
            end_events: end_events.into_iter().map(|event| event.1).collect()
        };
        s.states.push(State {
            typ: ending,
            id: s.states.len(),
            start_time: 1.0
        });
        Some(s)
    }

    pub fn get_state_changes(&self, total_duration: f32, start: f32, end: f32) -> Vec<Changes<E, V>> {
        let (start, end) = (start / total_duration, end / total_duration);
        let mut changes: Vec<Changes<E, V>> = self.events.iter().filter_map(
            |event| if start <= event.0 && event.0 < end {
                Some(Changes::Event((end - event.0) * total_duration, event.1))
            } else {
                None
            }
        ).collect();
        if start <= 1.0 && 1.0 <= end {
            changes.extend(self.end_events.clone().into_iter().map(|event| Changes::Event((end - 1.0) * total_duration, event)));
        }
        changes.extend(self.states.clone().into_iter().filter(|state| if state.typ != self.ending {
            start <= state.start_time && state.start_time < end
        } else {
            start <= state.start_time && state.start_time <= end
        }).map(|event| Changes::StateChange((end - event.start_time) * total_duration, event.typ)));
        changes.sort_by(|a, b| a.get_time().partial_cmp(&b.get_time()).unwrap_or(std::cmp::Ordering::Equal));
        changes
    }

    pub fn get_current_state(&self, time: f32) -> E {
        match self.states.iter().find(|state| state.start_time <= time) {
            Some(state) => state.typ,
            None => self.ending
        }
    }

    // gets the time that passed, and the new state, along with a list of events that occurred
    pub fn get_until_first_state_change(&self, total_duration: f32, start: f32, end: f32) -> (Vec<Changes<E, V>>, bool) {
        let changes = self.get_state_changes(total_duration, start, end);
        if let Some(pos) = changes.iter()
        .position(|change| matches!(change, Changes::StateChange(_, _))) {
            (changes.split_at(
                pos + 1
            ).0.to_vec(), true)
        } else {
            (changes, false) // if no state change then return all found events
        }
    }

    pub fn get_state(&self, id: usize) -> Option<E> {
        self.states.get(id).map(|s| s.typ)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

