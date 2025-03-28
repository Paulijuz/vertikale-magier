use serde::{Deserialize, Serialize};
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    fmt,
    iter::zip,
    time::SystemTime,
};

use crate::{
    elevator::{
        controller::ElevatorState,
        requests::{Direction, RequestType},
    },
    hall_request_assigner::RequestAssignments,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevatorView {
    pub state: ElevatorState,
    pub active: bool,
    pub timestamp_last_event: SystemTime,
}
impl fmt::Display for ElevatorView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // let cab_requests_string = self
        //     .cab_requests
        //     .iter()
        //     .map(|&v| if v { "*" } else { "-" })
        //     .collect::<Vec<_>>()
        //     .join(" ");

        let age = match SystemTime::now().duration_since(self.timestamp_last_event) {
            Ok(age) => age.as_secs().to_string(),
            Err(_) => String::from("From the future"),
        };

        writeln!(
            f,
            "Age: {} s\nActive: {}\nState: {:?}\nDirection: {:?}\nFloor: {}", //\nInternal orders:\n  1 2 3 4\n  {}",
            age,
            self.active,
            self.state.behaviour,
            self.state.direction,
            self.state.floor + 1,
            // cab_requests_string,
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestStatus {
    #[default]
    Inactive,
    Pending,
    Active,
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inactive => f.pad("-"),
            Self::Pending => f.pad("~"),
            Self::Active => f.pad("*"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RequestState {
    pub status: RequestStatus,
    pub acks: HashSet<String>,
    pub iteration: u32,
}

impl RequestState {
    fn set_inactive(&mut self, iteration_check: u32) -> bool {
        if self.status != RequestStatus::Active {
            return false;
        }

        if self.iteration != iteration_check {
            return false;
        } 
        
        self.acks.clear();
        self.status = RequestStatus::Inactive;

        return true;
    }

    fn set_pending(&mut self) -> bool {
        if self.status != RequestStatus::Inactive {
            return false;
        }

        self.acks.clear();
        self.status = RequestStatus::Pending;

        return true;
    }

    fn set_active(&mut self, required_acks: &HashSet<String>) -> bool {
        if self.status != RequestStatus::Pending {
            return false;
        }

        if !required_acks.is_subset(&self.acks) {
            return false;
        }

        self.iteration += 1;
        self.status = RequestStatus::Active;
        self.acks.clear();

        return true;
    }

    fn add_ack(&mut self, name: String, iteration_check: u32) -> bool {
        if self.iteration != iteration_check {
            return false;
        }

        if self.acks.contains(&name) {
            return false;
        }

        self.acks.insert(name);

        return true;
    }

    fn merge(&mut self, other: &Self) -> bool {
        let new_state = Self {
            status: max(self.status, other.status),
            acks: HashSet::new(),
            iteration: max(self.iteration, other.iteration),
        };

        let changed = self != &new_state;
        *self = new_state;

        changed
    }

    fn as_bool(&self) -> bool {
        self.status == RequestStatus::Active
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestStates {
    cab: HashMap<String, Vec<RequestState>>, // List of all active elevators
    hall_up: Vec<RequestState>,
    hall_down: Vec<RequestState>,
    iteration: i32,
    num_floors: usize,
}

pub fn system_state_to_string(
    request_states: &RequestStates,
    request_assignments: &RequestAssignments,
    elevator_views: &HashMap<String, ElevatorView>,
) -> String {
    let mut output = String::new();

    output += &format!("Elevators:\n");

    let mut sorted_elevators: Vec<(&String, &ElevatorView)> =
        elevator_views.iter().collect::<Vec<_>>();
    sorted_elevators.sort_by_key(|(name, _)| *name);

    for (name, elevator_state) in sorted_elevators {
        output += &format!("{name}:\n");

        for line in elevator_state.to_string().lines() {
            output += &format!("  {line}\n");
        }
    }

    output += &format!("Assignmeents: \n");
    for (up, down) in zip(&request_assignments.hall_up, &request_assignments.hall_down) {
        output += &format!("{:?} | {:?}\n", up, down);
    }

    output += &format!("Requests:\n");
    output += &format!("{:>5} | {:<10} | {:<10}\n", "Floor", "Down", "Up");
    for (floor, (up, down)) in zip(&request_states.hall_up, &request_states.hall_down).enumerate() {
        output += &format!("{:>5} | {:<3} ({:<4}) | {:<3} ({:<4})\n", floor + 1, down.status, down.acks.len(), up.status, up.acks.len());
    }

    output
}

impl RequestStates {
    pub fn new(num_floors: usize) -> Self {
        Self {
            cab: HashMap::new(),
            hall_up: vec![RequestState::default(); num_floors],
            hall_down: vec![RequestState::default(); num_floors],
            num_floors,
            iteration: 0,
        }
    }

    pub fn num_floors(&self) -> usize {
        self.num_floors
    }

    fn request_state_mut(
        &mut self,
        floor: usize,
        name: String,
        request_type: RequestType,
    ) -> &mut RequestState {
        match request_type {
            RequestType::Hall(Direction::Up) => &mut self.hall_up[floor],
            RequestType::Hall(Direction::Down) => &mut self.hall_down[floor],
            RequestType::Cab => &mut self
                .cab
                .entry(name)
                .or_insert(vec![RequestState::default(); self.num_floors])[floor],
        }
    }

    fn request_state(
        &self,
        floor: usize,
        name: &String,
        request_type: RequestType,
    ) -> Option<&RequestState> {
        match request_type {
            RequestType::Hall(Direction::Up) => Some(&self.hall_up[floor]),
            RequestType::Hall(Direction::Down) => Some(&self.hall_down[floor]),
            RequestType::Cab => self
                .cab
                .get(name).map(|requests| &requests[floor])
        }
    }


    pub fn set_inactive(
        &mut self,
        floor: usize,
        name: String,
        request_type: RequestType,
        iteration_check: u32,
    ) -> bool {
        self.request_state_mut(floor, name, request_type)
            .set_inactive(iteration_check)
    }

    pub fn set_pending(&mut self, floor: usize, name: String, request_type: RequestType) -> bool {
        self.request_state_mut(floor, name, request_type)
            .set_pending()
    }

    pub fn set_all_acked_active(&mut self, required_acks: &HashSet<String>) -> bool {
        let mut changed = false;

        for cab_requests in self.cab.values_mut() {
            for cab_request in cab_requests {
                changed |= cab_request.set_active(required_acks);
            }
        }

        for hall_up_request in &mut self.hall_up {
            changed |= hall_up_request.set_active(required_acks);
        }

        for hall_down_request in &mut self.hall_down {
            changed |= hall_down_request.set_active(required_acks);
        }

        changed
    }

    pub fn add_ack(
        &mut self,
        floor: usize,
        request_name: String,
        request_type: RequestType,
        ack_name: String,
        iteration_check: u32,
    ) -> bool {
        self.request_state_mut(floor, request_name, request_type)
            .add_ack(ack_name, iteration_check)
    }

    pub fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        for name in self.cab.clone().keys().chain(other.cab.keys()) {
            let Some(self_cab_requests) = self.cab.get_mut(name) else {
                continue;
            };

            let Some(other_cab_requests) = other.cab.get(name) else {
                continue;
            };

            for (self_cab_request, other_cab_request) in zip(self_cab_requests, other_cab_requests)
            {
                changed |= self_cab_request.merge(other_cab_request);
            }
        }

        for (self_hall_up_state, other_hall_up_state) in zip(&mut self.hall_up, &other.hall_up) {
            changed |= self_hall_up_state.merge(other_hall_up_state);
        }

        for (self_hall_down_state, other_hall_down_state) in
            zip(&mut self.hall_down, &other.hall_down)
        {
            changed |= self_hall_down_state.merge(other_hall_down_state);
        }

        changed
    }

    pub fn not_acked(&self, name: &String) -> Vec<(String, usize, RequestType, u32)> {
        let hall_up = self
            .hall_up
            .iter()
            .enumerate()
            .filter(|(_, request)| !request.acks.contains(name))
            .map(|(floor, request)| (name.clone(), floor, RequestType::Hall(Direction::Up), request.iteration));

        let hall_down = self
            .hall_down
            .iter()
            .enumerate()
            .filter(|(_, request)| !request.acks.contains(name))
            .map(|(floor, request)| (name.clone(), floor, RequestType::Hall(Direction::Down), request.iteration));

        let mut requests: Vec<_> = hall_up.into_iter().chain(hall_down).collect();

        for (name, cab) in &self.cab {
            requests
                .extend(
                    cab.iter()
                        .enumerate()
                        .filter(|(_, request)| !request.acks.contains(name))
                        .map(|(floor, request)| (name.clone(), floor, RequestType::Cab, request.iteration)),
                );
        }
                
        requests
    }

    pub fn iteration(
        &mut self,
        floor: usize,
        name: &String,
        request_type: RequestType,
    ) -> u32 {
        self.request_state(floor, name, request_type).map_or(0, |request| request.iteration)
    }

    pub fn hall_requests_as_bools(&self) -> Vec<(bool, bool)> {
        zip(&self.hall_up, &self.hall_down)
            .map(|(up, down)| (up.as_bool(), down.as_bool()))
            .collect()
    }

    pub fn cab_requests_as_bools(&self, name: &String) -> Vec<bool> {
        self.cab
            .get(name)
            .map_or(vec![false; self.num_floors], |requests| {
                requests.iter().map(|request| request.as_bool()).collect()
            })
    }

    pub fn all_cab_requests_as_bools(&self) -> HashMap<String, Vec<bool>> {
        self.cab
            .keys()
            .map(|name| (name.clone(), self.cab_requests_as_bools(name)))
            .collect()
    }
}
