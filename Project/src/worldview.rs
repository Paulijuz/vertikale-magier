use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, time::SystemTime};

use crate::{
    elevator::{
        controller::{Behaviour, ElevatorState},
        requests::{Direction, RequestType},
    },
    hall_request_assigner::{HraBehaviour, HraDirection, HraState},
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
            "Age: {} s\nActive: {}\nState: {:?}\nDirection: {:?}\nFloor: {}",//\nInternal orders:\n  1 2 3 4\n  {}",
            age,
            self.active,
            self.state.behaviour,
            self.state.direction,
            self.state.floor + 1,
            // cab_requests_string,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestStatus {
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

impl Default for RequestStatus {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HallRequest {
    pub up: RequestStatus,
    pub down: RequestStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestStates {
    pub cab_requests: HashMap<String, Vec<RequestStatus>>, // List of all active elevators
    pub hall_requests: Vec<HallRequest>,
    pub iteration: i32,
    num_floors: usize,
}

impl fmt::Display for RequestStates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Iteration: {}", self.iteration)?;
        writeln!(f, "Elevators:")?;

        // let mut sorted_elevators: Vec<(&String, &ElevatorView)> =
            // self.elevators.iter().collect::<Vec<_>>();
        // sorted_elevators.sort_by_key(|(name, _)| *name);

        // for (name, elevator_state) in sorted_elevators {
        //     writeln!(f, "  {name}:")?;

        //     for line in elevator_state.to_string().lines() {
        //         writeln!(f, "    {line}")?;
        //     }
        // }

        writeln!(f, "Orders:")?;
        writeln!(f, "  {:>6} | {:<16} | {:<16}", "Floor", "Down", "Up")?;

        for (floor, hall_request) in self.hall_requests.iter().enumerate().rev() {
            writeln!(
                f,
                "  {:>6} | {:<16} | {:<16}",
                floor + 1,
                hall_request.down,
                hall_request.up,
            )?;
        }

        Ok(())
    }
}

impl RequestStates {
    pub fn new(num_floors: usize) -> Self {
        Self {
            cab_requests: HashMap::new(),
            hall_requests: vec![HallRequest::default(); num_floors],
            num_floors,
            iteration: 0,
        }
    }
    pub fn set_pending(&mut self, floor: usize, name: String, request_type: RequestType) {
        match request_type {
            RequestType::Hall(Direction::Up) => self.hall_requests[floor].up = RequestStatus::Pending,
            RequestType::Hall(Direction::Down) => self.hall_requests[floor].down = RequestStatus::Pending,
            RequestType::Cab => self.cab_requests.entry(name).or_insert(vec![RequestStatus::Inactive; self.num_floors])[floor] = RequestStatus::Pending,
        }
    }

    pub fn set_inactive(&mut self, floor: usize, name: String, request_type: RequestType) {
        match request_type {
            RequestType::Hall(Direction::Up) => self.hall_requests[floor].up = RequestStatus::Inactive,
            RequestType::Hall(Direction::Down) => self.hall_requests[floor].down = RequestStatus::Inactive,
            RequestType::Cab => self.cab_requests.entry(name).or_insert(vec![RequestStatus::Inactive; self.num_floors])[floor] = RequestStatus::Inactive,
        }
    }

    pub fn actiave_all_confirmed(&mut self) {
        for cab_requests in self.cab_requests.values_mut() {
            for cab_request in cab_requests {
                if *cab_request == RequestStatus::Pending {
                    *cab_request = RequestStatus::Active;
                }
            }
        }

        for hall_request in &mut self.hall_requests {
            if hall_request.up == RequestStatus::Pending {
                hall_request.up = RequestStatus::Active;
            }

            if hall_request.down == RequestStatus::Pending {
                hall_request.down = RequestStatus::Active;
            }
        }
    }

    pub fn hall_requests_as_bools(&self) -> Vec<(bool, bool)> {
        self.hall_requests.iter().map(|request| (request.up == RequestStatus::Active, request.down == RequestStatus::Active)).collect()
    }

    pub fn cab_requests_as_bools(&self, name: &String) -> Vec<bool> {
        self.cab_requests.get(name)
            .map_or(vec![false; self.hall_requests.len()], |requessts| requessts.iter().map(|request| *request == RequestStatus::Active).collect())
    }
}
