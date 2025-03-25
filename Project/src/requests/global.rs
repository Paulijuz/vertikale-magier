use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum GlobalRequestStatus<T: Eq> {
    Inactive,
    Pending,
    Dispatched(T),
}

impl<T: Eq> Default for GlobalRequestStatus<T> {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// While a local request can be a simple boolean, a global request
/// requires extra information for synchronization.
pub struct GlobalRequestState<T: Eq> {
    status: GlobalRequestStatus<T>,
    iteration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the requests for all elevators of the system.
pub struct GlobalRequests {
    hall_up: Vec<GlobalRequestState<String>>,
    hall_down: Vec<GlobalRequestState<String>>,
    // For cab requests we use empty type for identification since
    // each cab request is inherently tied to one elevator.
    cab: HashMap<String, Vec<GlobalRequestState<()>>>,
}

/// Utility function for converting a vector of `GlobalRequestState` structs to booleans.
fn convert_to_local<T: Eq + Clone>(
    name: T,
    hall_requests: Vec<GlobalRequestState<T>>,
) -> Vec<bool> {
    let dispatched = GlobalRequestStatus::Dispatched(name);

    hall_requests
        .iter()
        .map(|request| request.status == dispatched)
        .collect()
}

impl GlobalRequests {
    pub fn new(num_floors: usize) -> Self {
        Self {
            hall_up: vec![GlobalRequestState::default(); num_floors],
            hall_down: vec![GlobalRequestState::default(); num_floors],
            cab: HashMap::new(),
        }
    }

    fn set_inactive(&mut self) {}

    fn set_pending(&mut self) {}

    fn set_dispatched(&mut self) {}
}
