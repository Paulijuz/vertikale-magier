use std::iter::zip;

use super::state::{merge_request_vectors, requests_states_as_bools, RequestState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HallRequestDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the requests for all elevators of the system.
pub struct HallRequests {
    up: Vec<RequestState>,
    down: Vec<RequestState>,
}

impl HallRequests {
    pub fn new(num_floors: usize) -> Self {
        Self {
            up: vec![RequestState::default(); num_floors],
            down: vec![RequestState::default(); num_floors],
            // cab: HashMap::new(),
        }
    }

    fn request_state_mut(
        &mut self,
        floor: usize,
        direction: HallRequestDirection,
    ) -> &mut RequestState {
        match direction {
            HallRequestDirection::Up => &mut self.up[floor],
            HallRequestDirection::Down => &mut self.down[floor],
        }
    }

    pub fn set_inactive(&mut self, floor: usize, direction: HallRequestDirection) {
        self.request_state_mut(floor, direction).set_inactive();
    }

    pub fn set_pending(&mut self, floor: usize, direction: HallRequestDirection) {
        self.request_state_mut(floor, direction).set_pending();
    }

    pub fn set_active(&mut self, floor: usize, direction: HallRequestDirection) {
        self.request_state_mut(floor, direction).set_active();
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self {
            up: merge_request_vectors(&self.up, &other.up),
            down: merge_request_vectors(&self.down, &other.down),
        }
    }

    pub fn as_bools(&self) -> Vec<(bool, bool)> {
        zip(
            requests_states_as_bools(&self.up),
            requests_states_as_bools(&self.down),
        )
        .collect()
    }
}
