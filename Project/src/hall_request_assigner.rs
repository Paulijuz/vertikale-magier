use log::error;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::zip, process::Command};

use crate::{
    elevator::{
        controller::Behaviour,
        requests::{Direction, RequestType},
    },
    worldview::{ElevatorView, RequestStates},
};

#[derive(Debug, Serialize, Deserialize)]
pub enum HraBehaviour {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "moving")]
    Moving,
    #[serde(rename = "doorOpen")]
    DoorOpen,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HraDirection {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "stop")]
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HraState {
    pub behaviour: HraBehaviour,
    pub floor: usize,
    pub direction: HraDirection,
    #[serde(rename = "cabRequests")]
    pub cab_requests: Vec<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestAssignments {
    pub hall_up: Vec<Option<String>>,
    pub hall_down: Vec<Option<String>>,
    pub cab: HashMap<String, Vec<bool>>,
}

fn assignees_to_requests(
    name: &String,
    assignees: &Vec<Option<String>>,
    request_type: RequestType,
) -> Vec<(bool, usize, RequestType)> {
    assignees
        .iter()
        .enumerate()
        .map(|(floor, assignee)| (assignee.as_ref() == Some(name), floor, request_type))
        .collect()
}

impl RequestAssignments {
    pub fn new(num_floors: usize) -> Self {
        Self {
            hall_up: vec![None; num_floors],
            hall_down: vec![None; num_floors],
            cab: HashMap::new(),
        }
    }

    pub fn requests(&self, name: &String) -> Vec<(bool, usize, RequestType)> {
        let hall_up = assignees_to_requests(name, &self.hall_up, RequestType::Hall(Direction::Up));
        let hall_down =
            assignees_to_requests(name, &self.hall_down, RequestType::Hall(Direction::Down));

        let requests = hall_up.into_iter().chain(hall_down);

        if let Some(cab) = self.cab.get(name) {
            requests
                .chain(
                    cab.iter()
                        .enumerate()
                        .map(|(floor, &active)| (active, floor, RequestType::Cab)),
                )
                .collect()
        } else {
            requests.collect()
        }
    }

    pub fn new_requests(&self, new: &Self, name: &String) -> Vec<(bool, usize, RequestType)> {
        zip(self.requests(name), new.requests(name))
            .filter(|(old_request, new_request)| old_request.0 != new_request.0)
            .map(|(_, new_request)| new_request)
            .collect()
    }

    pub fn has_assignment(&self, name: &String) -> bool {
        let assignee = Some(name.clone());

        self.cab
            .get(name)
            .map_or(false, |cab_requests| cab_requests.contains(&true))
            || self.hall_up.contains(&assignee)
            || self.hall_down.contains(&assignee)
    }
}

pub fn assign_requests(
    request_states: &RequestStates,
    elevator_views: &HashMap<String, ElevatorView>,
) -> Option<RequestAssignments> {
    let num_floors = request_states.num_floors();

    let hall_requests = request_states.hall_requests_as_bools();
    let cab_requests = request_states.all_cab_requests_as_bools();

    let states = elevator_views
        .iter()
        .filter(|(_, v)| v.active)
        .map(|(k, v)| {
            (
                k.to_owned(),
                create_hra_state(v, &cab_requests.get(k).unwrap_or(&vec![false; num_floors])),
            )
        })
        .collect();

    let assignments = match run_hall_request_assigner(hall_requests, states) {
        Ok(assignments) => assignments,
        Err(message) => {
            error!("Could not assign requests: {message}");
            return None;
        }
    };

    let mut request_assignments = RequestAssignments::new(num_floors);

    for (name, requests) in assignments {
        let mut cab_requests = vec![false; num_floors];

        for (floor, &(up, down, cab)) in requests.iter().enumerate() {
            if up {
                request_assignments.hall_up[floor] = Some(name.clone());
            }

            if down {
                request_assignments.hall_down[floor] = Some(name.clone());
            }

            if cab {
                cab_requests[floor] = true;
            }
        }

        request_assignments.cab.insert(name, cab_requests);
    }

    Some(request_assignments)
}
