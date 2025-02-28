use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

use crate::config::NUMBER_OF_FLOORS;
use crate::elevator_controller::{Direction, Request, Requests, State};
use crate::hall_request_assigner as hra;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevatorState {
    pub direction: Direction,
    pub state: State,
    pub floor: u8, // TOOD: Denne typen kan vel egentlig være usize?
    pub cab_requests: [bool; NUMBER_OF_FLOORS],
}

impl From<&ElevatorState> for hra::State {
    fn from(single_elevator_state: &ElevatorState) -> Self {
        hra::State {
            behaviour: match single_elevator_state.state {
                State::DoorOpen => hra::Behaviour::DoorOpen,
                State::Moving => hra::Behaviour::Moving,
                _ => hra::Behaviour::Idle,
            },
            floor: single_elevator_state.floor,
            direction: match single_elevator_state.direction {
                Direction::Down => hra::Direction::Down,
                Direction::Stopped => hra::Direction::Stop,
                Direction::Up => hra::Direction::Up,
            },
            cab_requests: single_elevator_state.cab_requests,
        }
    }
}

impl fmt::Display for ElevatorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Tilstand: {:?}\nRetning: {:?}\nEtasje: {}\nInterne bestillinger: {:?}",
            self.state,
            self.direction,
            self.floor + 1,
            self.cab_requests
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HallRequestState {
    Inactive,
    Requested,
    Assigned(String),
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SystemState {
    pub name: String,
    pub elevators: HashMap<String, ElevatorState>, //Liste over alle aktive heiser
    pub hall_requests: [HallRequest; NUMBER_OF_FLOORS],
    pub iteration: i32,
}

impl fmt::Display for SystemState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Heiser:")?;
        for (id, elevator_state) in &self.elevators {
            writeln!(f, "  {id}:")?;

            for line in elevator_state.to_string().lines() {
                writeln!(f, "    {line}")?;
            }
        }

        writeln!(f, "Bestillinger:")?;
        for (mut floor, hall_request) in self.hall_requests.iter().enumerate().rev() {
            floor += 1;

            writeln!(
                f,
                "  Etasje {floor} - Ned: {:?}, Opp: {:?}",
                hall_request.down, hall_request.up
            )?;
        }

        Ok(())
    }
}

impl SystemState {
    // Velger beste heis for en bestilling
    pub fn assign_request(&mut self, floor: u8, direction: Direction) {
        match direction {
            Direction::Up => self.hall_requests[floor as usize].up = HallRequestState::Requested,
            Direction::Down => {
                self.hall_requests[floor as usize].down = HallRequestState::Requested
            }
            _ => panic!("Tried to assign request with invalid direction"),
        }

        let hall_requests = self.hall_requests.clone().map(|request| {
            (
                request.up != HallRequestState::Inactive,
                request.down != HallRequestState::Inactive,
            )
        });
        let states = self
            .elevators
            .iter()
            .map(|(k, v)| (k.to_owned(), v.into()))
            .collect();

        let assignments = hra::run_hall_request_assigner(hra::HallRequestsStates {
            hall_requests,
            states,
        })
        .unwrap();

        for (id, assigned_hall_requests) in assignments.iter() {
            for (floor, (up, down)) in assigned_hall_requests.iter().enumerate() {
                if *up {
                    self.hall_requests[floor].up = HallRequestState::Assigned(id.to_string());
                }

                if *down {
                    self.hall_requests[floor].down = HallRequestState::Assigned(id.to_string());
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
        self.requests_for_elevator(&self.name).unwrap_or(Default::default())
    }
    pub fn set_local_elevator_state(&mut self, local_elevator_state: &ElevatorState) {
        self.elevators.insert(self.name.clone(), local_elevator_state.clone());
    }
}
