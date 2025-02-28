use crate::system_state::SystemState;
use serde_json::from_str;
use serde_json::to_string_pretty;
use std::fs::File;
use std::io::{Read, Write};

pub fn load_state_from_file(file_path: &str) -> Result<SystemState, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut json_string = String::new();
    file.read_to_string(&mut json_string)?;
    let state: SystemState = from_str(&json_string).unwrap();
    Ok(state)
}

pub fn save_state_to_file(state: &SystemState, file_path: &str) -> Result<(), std::io::Error> {
    let json_string = to_string_pretty(state).unwrap();
    let mut file = File::create(file_path)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}
