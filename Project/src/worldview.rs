use log::error;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, time::SystemTime};

use crate::{
    elevator::{
        controller::{Behaviour, ElevatorState},
        requests::{Direction, RequestType, Requests},
    },
    hall_request_assigner::{run_hall_request_assigner, HraBehaviour, HraDirection, HraState},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevatorView {
    pub state: ElevatorState,
    pub active: bool,
    pub timestamp_last_event: SystemTime,
}

fn create_hra_state(elevator_view: &ElevatorView, cab_requests: &Vec<bool>) -> HraState {
    HraState {
        behaviour: match elevator_view.state.behaviour {
            Behaviour::DoorOpen => HraBehaviour::DoorOpen,
            Behaviour::Moving => HraBehaviour::Moving,
            _ => HraBehaviour::Idle,
        },
        floor: elevator_view.state.floor,
        direction: match elevator_view.state.direction {
            Some(Direction::Down) => HraDirection::Down,
            None => HraDirection::Stop,
            Some(Direction::Up) => HraDirection::Up,
        },
        cab_requests: cab_requests.clone(),
    }
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
pub enum HallRequestState {
    Inactive,
    Requested,
    Assigned(String),
}

impl fmt::Display for HallRequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inactive => f.pad("-"),
            Self::Assigned(assignee) => f.pad(&format!("* ({assignee})")),
            Self::Requested => f.pad("* (-)"),
        }
    }
}

impl Default for HallRequestState {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HallRequest {
    pub up: HallRequestState,
    pub down: HallRequestState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worldview {
    pub cab_requests: HashMap<String, Vec<bool>>, // List of all active elevators
    pub hall_requests: Vec<HallRequest>,
    pub iteration: i32,
    num_floors: usize,
}

impl fmt::Display for Worldview {
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

impl Worldview {
    pub fn new(num_floors: usize) -> Self {
        Self {
            cab_requests: HashMap::new(),
            hall_requests: vec![HallRequest::default(); num_floors],
            num_floors,
            iteration: 0,
        }
    }
    pub fn add_request(&mut self, floor: usize, name: String, request_type: RequestType) {
        match request_type {
            RequestType::Hall(Direction::Up) => self.hall_requests[floor].up = HallRequestState::Requested,
            RequestType::Hall(Direction::Down) => self.hall_requests[floor].down = HallRequestState::Requested,
            RequestType::Cab => self.cab_requests.entry(name).or_insert(vec![false; self.num_floors])[floor] = true,
        }
    }

    pub fn clear_request(&mut self, floor: usize, name: String, request_type: RequestType) {
        match request_type {
            RequestType::Hall(Direction::Up) => self.hall_requests[floor].up = HallRequestState::Inactive,
            RequestType::Hall(Direction::Down) => self.hall_requests[floor].down = HallRequestState::Inactive,
            RequestType::Cab => self.cab_requests.entry(name).or_insert(vec![false; self.num_floors])[floor] = false,
        }
    }
    // Velger beste heis for en bestilling
    pub fn assign_requests(&mut self, elevator_views: &HashMap<String, ElevatorView>) {
        let hall_requests = self
            .hall_requests
            .iter()
            .map(|request| {
                (
                    request.up != HallRequestState::Inactive,
                    request.down != HallRequestState::Inactive,
                )
            })
            .collect();

        let states = elevator_views
            .iter()
            .filter(|(_, v)| v.active)
            .map(|(k, v)| (k.to_owned(), create_hra_state(v, self.cab_requests.get(k).unwrap_or(&vec![false; self.num_floors]))))
            .collect();

        let assignments = match run_hall_request_assigner(hall_requests, states) {
            Ok(assignments) => assignments,
            Err(message) => {
                error!("Could not assign requests: {message}");
                return;
            }
        };

        for (name, assigned_hall_requests) in assignments.iter() {
            for (floor, (up, down, _)) in assigned_hall_requests.iter().enumerate() {
                if *up {
                    self.hall_requests[floor].up = HallRequestState::Assigned(name.to_string());
                }

                if *down {
                    self.hall_requests[floor].down = HallRequestState::Assigned(name.to_string());
                }
            }
        }
    }
    pub fn requests_for_elevator(&self, name: &String) -> Option<Requests> {
        let mut requests = Requests::new(self.num_floors);

        if let Some(cab_requests) = self.cab_requests.get(name) {
            for (floor, cab_request) in cab_requests.iter().enumerate() {
                if *cab_request {
                    requests.add(floor, RequestType::Cab);
                }
            }
        }

        for (floor, hall_request) in self.hall_requests.iter().enumerate() {
            if hall_request.up == HallRequestState::Assigned(name.clone()) {
                requests.add(floor, RequestType::Hall(Direction::Up));
            }

            if hall_request.down == HallRequestState::Assigned(name.clone()) {
                requests.add(floor, RequestType::Hall(Direction::Down));
            }
        }

        return Some(requests);
    }
}
