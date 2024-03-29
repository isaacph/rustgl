
#[derive(Clone)]
pub struct State<E> {
    typ: E,
    start_time: f32,
    end_time: f32,
}

#[derive(Clone)]
pub struct Fsm<E: Clone + Copy + Eq, V: Clone + Copy + Eq> {
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

impl<E: Clone + Copy + Eq, V: Clone + Copy + Eq> Fsm<E, V> {
    pub fn new(mut states: Vec<(f32, E)>, ending: E, events: &[(f32, V)]) -> Option<Fsm<E, V>> {
        let sum = {
            let mut sum = states.iter().map(|(t, _)| f32::abs(*t)).sum();
            if sum <= 0.0 {
                sum = 1.0;
            }
            sum
        };
        let (events, end_events): (Vec<_>, Vec<_>) = events.iter().map(|event| (event.0 / sum, event.1)).partition(|event| event.0 < 1.0);
        let mut s = Self {
            states: states.iter_mut().fold((vec![], 0.0),
            |(mut states, prev_duration): (Vec<State<E>>, f32), (next, typ)| {
                let prev_start: f32 = states.last().map(|state| state.start_time).unwrap_or(0.0);
                states.push(State {
                    typ: *typ,
                    start_time: prev_start + prev_duration / sum,
                    end_time: prev_start + prev_duration / sum + *next / sum,
                });
                (states, *next)
            }).0,
            ending,
            events,
            end_events: end_events.into_iter().map(|event| event.1).collect()
        };
        s.states.push(State {
            typ: ending,
            start_time: 1.0,
            end_time: f32::INFINITY,
        });
        Some(s)
    }

    pub fn get_state_changes(&self, total_duration: f32, start: f32, end: f32) -> (Vec<Changes<E, V>>, bool) {
        let (start, end) = (start / total_duration, end / total_duration);
        // add events
        let mut changes: Vec<Changes<E, V>> = self.events.iter().filter_map(
            |event| if start <= event.0 && event.0 < end {
                Some(Changes::Event((end - event.0) * total_duration, event.1))
            } else {
                None
            }
        ).collect();
        // add ending events
        if start <= 1.0 && 1.0 <= end {
            changes.extend(
                self.end_events.clone().into_iter()
                .map(|event| Changes::Event((end - 1.0) * total_duration, event)));
        }
        // add state changes
        changes.extend(self.states.clone().into_iter().filter(|state| if state.typ != self.ending {
            start <= state.start_time && state.start_time < end
        } else {
            start <= state.start_time && state.start_time <= end
        }).map(|event|
            Changes::StateChange(
                // ensure state changes don't last longer than until the next state
                f32::min(event.end_time, end - event.start_time) * total_duration,
                event.typ
            )
        ));
        changes.sort_by(|a, b| a.get_time().partial_cmp(&b.get_time()).unwrap_or(std::cmp::Ordering::Equal));
        let is_empty = changes.is_empty();
        (changes, !is_empty)
    }

    pub fn get_current_state(&self, total_duration: f32, time: f32) -> E {
        match self.states.iter().find(|state|
                state.start_time * total_duration <= time &&
                time < state.end_time * total_duration
        ) {
            Some(state) => state.typ,
            None => self.ending
        }
    }

    // gets the time that passed, and the new state, along with a list of events that occurred
    pub fn get_until_first_state_change(&self, total_duration: f32, start: f32, end: f32) -> (Vec<Changes<E, V>>, bool) {
        let (changes, _) = self.get_state_changes(total_duration, start, end);
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

    pub fn get_event_time(&self, total_duration: f32, event: V) -> Option<f32> {
        for e in &self.events {
            if e.1 == event {
                return Some(e.0 * total_duration)
            }
        }
        for e in &self.end_events {
            if *e == event {
                return Some(total_duration)
            }
        }
        None
    }
}

