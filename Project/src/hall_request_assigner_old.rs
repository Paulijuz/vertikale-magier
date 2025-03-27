use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Command};

use crate::elevator::controller;

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
    pub behaviour: Behaviour,
    pub floor: usize,
    pub direction: Direction,
    #[serde(rename = "cabRequests")]
    pub cab_requests: Vec<bool>,
}

impl From<(ElevatorState, Vec<bool>)> for State {
    fn from((state, cab_requests): (ElevatorState, Vec<bool>)) -> Self {
        Self {
            behaviour: state.behaviour.into(),
            direction: state.direction.into(),
            floor: state.floor,
            cab_requests,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct HraInput {
    #[serde(rename = "hallRequests")]
    pub hall_requests: Vec<(bool, bool)>,
    pub states: HashMap<String, State>,
}

type HraOutput = HashMap<String, Vec<(bool, bool, bool)>>;

/// Internal function used to actually execute the hall request assigner.
fn run_hall_request_assigner(input: HraInput) -> Result<HraOutput, String> {
    let input = serde_json::to_string(&input).unwrap();

    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&input)
        .output()
        .expect("Failed to start hall_request_assigner");

    if output.status.success() {
        let assignments = serde_json::from_slice(&output.stdout);

        match assignments {
            Ok(assignments) => Ok(assignments),
            Err(_) => Err(String::from(format!(
                "Invalid output from assigner: {}",
                String::from_utf8_lossy(&output.stdout)
            ))),
        }
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
