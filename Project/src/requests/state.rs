use std::{cmp::max, collections::{HashMap, HashSet}, iter::zip};
use serde::{Deserialize, Serialize};

// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub enum RequestType {
//     Hall(RequestDirection),
//     Cab(String),
// }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestStatus {
    #[default]
    Inactive,
    Pending,
    Active,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// While a local request can be a simple boolean, a global request
/// requires extra information for synchronization.
///
/// TODO: Improve comment
pub struct RequestState {
    status: RequestStatus,
    acks: HashSet<String>,
    iteration: u32,
}

impl RequestState {
    /// Gets the current status.
    pub fn status(&self) -> RequestStatus {
        self.status
    }

    pub fn as_bool(&self) -> bool {
        self.status == RequestStatus::Active
    }

    pub fn set_inactive(&mut self) {
        self.status = RequestStatus::Inactive;
    }

    pub fn set_pending(&mut self) {
        self.status = RequestStatus::Pending;
    }

    pub fn set_active(&mut self) {
        self.status = RequestStatus::Active;
    }

    // Adds a new acknowledgment to the request.
    pub fn add_ack(&mut self, ack: String) {
        self.acks.insert(ack);
    }

    /// Clears all acknowledgments stored in this request.
    pub fn clear_acks(&mut self) {
        self.acks.clear();
    }

    /// Returnes a new `RequestState` struct created by merging two
    /// `RequestState` structs by assuming the worst of both.
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            status: max(self.status, other.status),
            iteration: max(self.iteration, other.iteration),
            acks: HashSet::new(),
        }
    }
}

/// Utility function for converting a vector of `GlobalRequestState` structs to booleans.
pub fn requests_states_as_bools(request_states: &Vec<RequestState>) -> Vec<bool> {
    request_states
        .iter()
        .map(|request| request.as_bool())
        .collect()
}

/// Takes two vectors of `RequestState` structs and create a new third array with the merged states.
pub fn merge_request_vectors(requests_a: &Vec<RequestState>, requests_b: &Vec<RequestState>) -> Vec<RequestState> {
    zip(requests_a, requests_b)
        .map(|(self_reqeust, other_requesst)| self_reqeust.merge(other_requesst))
        .collect()
}

pub fn merge_request_maps(
    request_map_a: &HashMap<String, Vec<RequestState>>,
    request_map_b: &HashMap<String, Vec<RequestState>>,
) -> HashMap<String, Vec<RequestState>> {
    let mut merged = HashMap::new();

    // First loop: merge values from requests_a and requests_b
    for (key, requests_a) in request_map_a {
        if let Some(requests_b) = request_map_b.get(key) {
            merged.insert(key.clone(), merge_request_vectors(requests_a, requests_b));
        } else {
            merged.insert(key.clone(), requests_a.clone());
        }
    }

    // Second loop: add remaining keys from requests_b
    for (key, requests_b) in request_map_b {
        if !request_map_a.contains_key(key) {
            merged.insert(key.clone(), requests_b.clone());
        }
    }

    merged
}