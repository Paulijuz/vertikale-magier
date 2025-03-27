use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::state::{merge_request_maps, requests_states_as_bools, RequestState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CabRequests {
    map: HashMap<String, Vec<RequestState>>,
    num_floors: usize,
}

impl CabRequests {
    pub fn new(num_floors: usize) -> Self {
        Self {
            map: HashMap::new(),
            num_floors,
        }
    }

    fn request_state_mut(&mut self, floor: usize, name: String) -> &mut RequestState {
        let request_states_entry = self
            .map
            .entry(name)
            .or_insert(vec![RequestState::default(); self.num_floors]);

        &mut request_states_entry[floor]
    }

    pub fn set_inactive(&mut self, floor: usize, name: String) {
        self.request_state_mut(floor, name).set_inactive();
    }

    pub fn set_pending(&mut self, floor: usize, name: String) {
        self.request_state_mut(floor, name).set_pending();
    }

    pub fn set_active(&mut self, floor: usize, name: String) {
        self.request_state_mut(floor, name).set_active();
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self {
            map: merge_request_maps(&self.map, &other.map),
            num_floors: self.num_floors,
        }
    }

    pub fn as_bools(&self, name: &String) -> Vec<bool> {
        if let Some(request_states) = self.map.get(name) {
            requests_states_as_bools(request_states)
        } else {
            vec![false; self.num_floors]
        }
    }
}
