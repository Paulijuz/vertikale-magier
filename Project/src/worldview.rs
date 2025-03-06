use log::error;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, time::SystemTime};

use crate::{
    elevator::controller::Behaviour,
    requests::{
        assigner,
        requests::{Direction, Request, Requests, NUMBER_OF_FLOORS},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevatorState {
    pub direction: Direction,
    pub behaviour: Behaviour,
    pub floor: u8, // TOOD: Denne typen kan vel egentlig være usize?
    pub cab_requests: [bool; NUMBER_OF_FLOORS],
    pub active: bool,
    pub timestamp_last_event: SystemTime,
}

impl From<&ElevatorState> for assigner::State {
    fn from(single_elevator_state: &ElevatorState) -> Self {
        assigner::State {
            behaviour: match single_elevator_state.behaviour {
                Behaviour::DoorOpen => assigner::Behaviour::DoorOpen,
                Behaviour::Moving => assigner::Behaviour::Moving,
                _ => assigner::Behaviour::Idle,
            },
            floor: single_elevator_state.floor,
            direction: match single_elevator_state.direction {
                Direction::Down => assigner::Direction::Down,
                Direction::Stopped => assigner::Direction::Stop,
                Direction::Up => assigner::Direction::Up,
            },
            cab_requests: single_elevator_state.cab_requests,
        }
    }
}

impl fmt::Display for ElevatorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cab_requests_string = self
            .cab_requests
            .iter()
            .map(|&v| if v { "*" } else { "-" })
            .collect::<Vec<_>>()
            .join(" ");

        let age = match SystemTime::now().duration_since(self.timestamp_last_event) {
            Ok(age) => age.as_secs().to_string(),
            Err(_) => String::from("Fra fremtiden"),
        };

        writeln!(
            f,
            "Alder: {} s\nAktiv: {}\nTilstand: {:?}\nRetning: {:?}\nEtasje: {}\nInterne bestillinger:\n  1 2 3 4\n  {}",
            age,
            self.active,
            self.behaviour,
            self.direction,
            self.floor + 1,
            cab_requests_string,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worldview {
    pub name: String,
    pub elevators: HashMap<String, ElevatorState>, //Liste over alle aktive heiser
    pub hall_requests: [HallRequest; NUMBER_OF_FLOORS],
    pub iteration: i32,
}

impl fmt::Display for Worldview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Iterasjon: {}", self.iteration)?;
        writeln!(f, "Heiser:")?;

        let mut sorted_elevators: Vec<(&String, &ElevatorState)> =
            self.elevators.iter().collect::<Vec<_>>();
        sorted_elevators.sort_by_key(|(name, _)| *name);

        for (name, elevator_state) in sorted_elevators {
            writeln!(f, "  {name}:")?;

            for line in elevator_state.to_string().lines() {
                writeln!(f, "    {line}")?;
            }
        }

        writeln!(f, "Bestillinger:")?;
        writeln!(f, "  {:>6} | {:<16} | {:<16}", "Etasje", "Ned", "Opp")?;

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
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
    pub fn add_request(&mut self, floor: u8, direction: Direction) {
        match direction {
            Direction::Up => self.hall_requests[floor as usize].up = HallRequestState::Requested,
            Direction::Down => {
                self.hall_requests[floor as usize].down = HallRequestState::Requested
            }
            _ => panic!("Tried to assign request with invalid direction"),
        }
    }

    // Velger beste heis for en bestilling
    pub fn assign_requests(&mut self) {
        let hall_requests = self.hall_requests.clone().map(|request| {
            (
                request.up != HallRequestState::Inactive,
                request.down != HallRequestState::Inactive,
            )
        });
        let states = self
            .elevators
            .iter()
            .filter(|(_, v)| v.active)
            .map(|(k, v)| (k.to_owned(), v.into()))
            .collect();

        let assignments = match assigner::run_hall_request_assigner(assigner::HallRequestsStates {
            hall_requests,
            states,
        }) {
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
        let mut requests = [Request {
            cab: false,
            hall_down: false,
            hall_up: false,
        }; NUMBER_OF_FLOORS];

        for (floor, cab_request) in self.elevators.get(name)?.cab_requests.iter().enumerate() {
            requests[floor].cab = *cab_request;
        }

        for (floor, hall_request) in self.hall_requests.iter().enumerate() {
            requests[floor].hall_up = hall_request.up == HallRequestState::Assigned(name.clone());
            requests[floor].hall_down =
                hall_request.down == HallRequestState::Assigned(name.clone());
        }

        return Some(requests);
    }
    pub fn requests_for_local_elevator(&self) -> Requests {
        self.requests_for_elevator(&self.name)
            .unwrap_or(Default::default())
    }
    pub fn set_local_elevator_state(&mut self, local_elevator_state: ElevatorState) {
        self.elevators
            .insert(self.name.clone(), local_elevator_state.clone());
    }
    pub fn local_elevator_state(&mut self) -> &mut ElevatorState {
        if !self.elevators.contains_key(&self.name) {
            self.elevators.insert(
                self.name.clone(),
                ElevatorState {
                    active: true,
                    cab_requests: Default::default(),
                    direction: Direction::Stopped,
                    floor: 0,
                    behaviour: Behaviour::Idle,
                    timestamp_last_event: SystemTime::now(),
                },
            );
        }

        self.elevators.get_mut(&self.name).unwrap()
    }
    pub fn sync_with_master(&mut self, master_state: Worldview) {
        let local_elevator_state = self.local_elevator_state().to_owned();

        *self = Self {
            name: self.name.clone(),
            ..master_state
        };

        self.elevators
            .insert(self.name.clone(), local_elevator_state);
    }
}
