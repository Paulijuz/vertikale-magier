use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Command};

#[derive(Serialize, Deserialize)]
pub enum HraBehaviour {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "moving")]
    Moving,
    #[serde(rename = "doorOpen")]
    DoorOpen,
}

#[derive(Serialize, Deserialize)]
pub enum HraDirection {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "stop")]
    Stop,
}

#[derive(Serialize, Deserialize)]
pub struct HraState {
    pub behaviour: HraBehaviour,
    pub floor: usize,
    pub direction: HraDirection,
    #[serde(rename = "cabRequests")]
    pub cab_requests: Vec<bool>,
}

#[derive(Serialize, Deserialize)]
struct HraInput {
    #[serde(rename = "hallRequests")]
    pub hall_requests: Vec<(bool, bool)>,
    pub states: HashMap<String, HraState>,
}

type HraOutput = HashMap<String, Vec<(bool, bool, bool)>>;

pub fn run_hall_request_assigner(
    hall_requests: Vec<(bool, bool)>,
    states: HashMap<String, HraState>,
) -> Result<HraOutput, String> {
    let input_struct = HraInput {
        hall_requests,
        states,
    };

    let input_json = serde_json::to_string(&input_struct).unwrap();

    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&input_json)
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
