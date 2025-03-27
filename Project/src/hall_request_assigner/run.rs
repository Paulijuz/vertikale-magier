use std::{collections::HashMap, process::Command};

use crate::{
    elevator::controller::ElevatorState,
    requests::{assignments::HallRequestAssignments, cab::CabRequests, hall::HallRequests},
};

use super::data_format::{convert_from_hra_output, convert_to_hra_input};

/// Internal function used to actually execute the hall request assigner.
pub fn run_hall_request_assigner(
    num_floors: usize,
    hall_requests: &HallRequests,
    cab_requests: &CabRequests,
    elevator_states: &HashMap<String, ElevatorState>,
) -> Result<HallRequestAssignments, String> {
    let input_struct = convert_to_hra_input(hall_requests, cab_requests, elevator_states);
    let input_string = serde_json::to_string(&input_struct).unwrap();

    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&input_string)
        .output()
        .expect("Failed to start hall_request_assigner");

    if output.status.success() {
        let assignments = serde_json::from_slice(&output.stdout);

        match assignments {
            Ok(assignments) => Ok(convert_from_hra_output(num_floors, assignments)),
            Err(_) => Err(format!(
                "Invalid output from assigner: {}",
                String::from_utf8_lossy(&output.stdout)
            )
            .to_string()),
        }
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
