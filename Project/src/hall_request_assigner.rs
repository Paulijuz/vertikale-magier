use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Command};

use crate::config::NUMBER_OF_FLOORS;

#[derive(Serialize, Deserialize)]
pub enum Behaviour {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "moving")]
    Moving,
    #[serde(rename = "doorOpen")]
    DoorOpen,
}

#[derive(Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "stop")]
    Stop,
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    #[serde(rename = "cabRequests")]
    pub cab_requests: [bool; NUMBER_OF_FLOORS],
}

pub type States = HashMap<String, State>;

pub type HallRequests = [(bool, bool); NUMBER_OF_FLOORS];

#[derive(Serialize, Deserialize)]
pub struct HallRequestsStates {
    #[serde(rename = "hallRequests")]
    pub hall_requests: HallRequests,
    pub states: States,
}

pub type HallRequestsAssignments = HashMap<String, HallRequests>;

pub fn run_hall_request_assigner(
    input: HallRequestsStates,
) -> Result<HallRequestsAssignments, String> {
    let input_json = serde_json::to_string(&input).unwrap();

    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&input_json)
        .output()
        .expect("Failed to start hall_request_assigner");

    let assignments = serde_json::from_slice(&output.stdout).unwrap();

    if output.status.success() {
        Ok(assignments)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
