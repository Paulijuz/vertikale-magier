use std::fs;
use std::{thread::sleep, time::Duration};
use std::fs::File;
use std::io::{Read, Write};
use serde_json::from_str;
use serde_json::to_string_pretty;
use log::{debug, error, info};

use crate::request_dispatch::AllElevatorStates;


pub fn load_state_from_file(file_path: &str) -> Result<AllElevatorStates, std::io::Error> {
    if !std::path::Path::new(file_path).exists() {
        info!("Backup-fil ikke funnet, starter med ny tilstand.");
        return Ok(AllElevatorStates::new());
    }
    let mut file = File::open(file_path)?;
    let mut json_string = String::new();
    file.read_to_string(&mut json_string)?;
    let state: AllElevatorStates = from_str(&json_string).unwrap();
    Ok(state)
}

pub fn save_state_to_file(state: &AllElevatorStates, file_path: &str) -> Result<(), std::io::Error> {
    let json_string = to_string_pretty(state).unwrap();
    let mut file = File::create(file_path)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}
    

