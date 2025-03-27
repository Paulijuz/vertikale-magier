use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    elevator::controller::{self, ElevatorState},
    requests::{
        assignments::HallRequestAssignments,
        cab::CabRequests,
        hall::{HallRequestDirection, HallRequests},
    },
};

#[derive(Serialize, Deserialize)]
enum Behaviour {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "moving")]
    Moving,
    #[serde(rename = "doorOpen")]
    DoorOpen,
}

impl From<controller::Behaviour> for Behaviour {
    fn from(behaviour: controller::Behaviour) -> Self {
        match behaviour {
            controller::Behaviour::DoorOpen => Behaviour::DoorOpen,
            controller::Behaviour::Idle => Behaviour::Idle,
            controller::Behaviour::Moving => Behaviour::Moving,
            controller::Behaviour::OutOfOrder => Behaviour::Idle,
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Direction {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "stop")]
    Stop,
}

impl From<controller::ElevatorDirection> for Direction {
    fn from(direction: controller::ElevatorDirection) -> Self {
        match direction {
            controller::ElevatorDirection::Up => Direction::Up,
            controller::ElevatorDirection::Down => Direction::Down,
            controller::ElevatorDirection::Stopped => Direction::Down,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct State {
    behaviour: Behaviour,
    floor: usize,
    direction: Direction,
    #[serde(rename = "cabRequests")]
    cab_requests: Vec<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct Input {
    #[serde(rename = "hallRequests")]
    hall_requests: Vec<(bool, bool)>,
    states: HashMap<String, State>,
}

pub fn convert_to_hra_input(
    hall_requests: &HallRequests,
    cab_requests: &CabRequests,
    elevator_states: &HashMap<String, ElevatorState>,
) -> Input {
    let mut states: HashMap<String, State> = HashMap::new();

    for (name, state) in elevator_states {
        states.insert(
            name.clone(),
            State {
                floor: state.floor,
                direction: state.direction.into(),
                behaviour: state.behaviour.into(),
                cab_requests: cab_requests.as_bools(name),
            },
        );
    }

    Input {
        hall_requests: hall_requests.as_bools(),
        states,
    }
}

pub type Output = HashMap<String, Vec<(bool, bool, bool)>>;

pub fn convert_from_hra_output(num_floors: usize, output: Output) -> HallRequestAssignments {
    let mut hall_request_assignments = HallRequestAssignments::new(num_floors);

    for (name, assignments) in output {
        for (floor, &(up, down, _)) in assignments.iter().enumerate() {
            if up {
                hall_request_assignments.assign(floor, HallRequestDirection::Up, name.clone());
            }

            if down {
                hall_request_assignments.assign(floor, HallRequestDirection::Up, name.clone());
            }
        }
    }

    hall_request_assignments
}
