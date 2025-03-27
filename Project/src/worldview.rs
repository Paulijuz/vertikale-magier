// use log::error;
// use serde::{Deserialize, Serialize};
// use std::{collections::HashMap, fmt, time::SystemTime};

// use crate::{
//     elevator::controller::{Behaviour, ElevatorDirection, ElevatorState},
//     requests::{
//         assigner::{self, HraBehaviour, HraDirection, HraState},
//         local::LocalRequests,
//     },
// };

// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
// pub struct ElevatorView {
//     pub state: ElevatorState,
//     pub cab_requests: Vec<bool>,
//     pub active: bool,
//     pub timestamp_last_event: SystemTime,
// }

// impl From<&ElevatorView> for HraState {
//     fn from(elevator_view: &ElevatorView) -> Self {
//         HraState {
//             behaviour: match elevator_view.state.behaviour {
//                 Behaviour::DoorOpen => HraBehaviour::DoorOpen,
//                 Behaviour::Moving => HraBehaviour::Moving,
//                 _ => HraBehaviour::Idle,
//             },
//             floor: elevator_view.state.floor,
//             direction: match elevator_view.state.direction {
//                 ElevatorDirection::Down => HraDirection::Down,
//                 ElevatorDirection::Stopped => HraDirection::Stop,
//                 ElevatorDirection::Up => HraDirection::Up,
//             },
//             cab_requests: elevator_view.cab_requests.clone(),
//         }
//     }
// }

// impl fmt::Display for ElevatorView {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let cab_requests_string = self
//             .cab_requests
//             .iter()
//             .map(|&v| if v { "*" } else { "-" })
//             .collect::<Vec<_>>()
//             .join(" ");

//         let age = match SystemTime::now().duration_since(self.timestamp_last_event) {
//             Ok(age) => age.as_secs().to_string(),
//             Err(_) => String::from("From the future"),
//         };

//         writeln!(
//             f,
//             "Age: {} s\nActive: {}\nState: {:?}\nDirection: {:?}\nFloor: {}\nInternal orders:\n  1 2 3 4\n  {}",
//             age,
//             self.active,
//             self.state.behaviour,
//             self.state.direction,
//             self.state.floor + 1,
//             cab_requests_string,
//         )
//     }
// }

// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub enum HallRequestState {
//     Inactive,
//     Requested,
//     Assigned(String),
// }

// impl fmt::Display for HallRequestState {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::Inactive => f.pad("-"),
//             Self::Assigned(assignee) => f.pad(&format!("* ({assignee})")),
//             Self::Requested => f.pad("* (-)"),
//         }
//     }
// }

// impl Default for HallRequestState {
//     fn default() -> Self {
//         Self::Inactive
//     }
// }

// #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
// pub struct HallRequest {
//     pub up: HallRequestState,
//     pub down: HallRequestState,
// }

// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub struct Worldview {
//     pub name: String,
//     pub elevators: HashMap<String, ElevatorView>, // List of all active elevators
//     pub hall_requests: Vec<HallRequest>,
//     pub iteration: i32,
//     num_floors: usize,
// }

// impl fmt::Display for Worldview {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         writeln!(f, "Iteration: {}", self.iteration)?;
//         writeln!(f, "Elevators:")?;

//         let mut sorted_elevators: Vec<(&String, &ElevatorView)> =
//             self.elevators.iter().collect::<Vec<_>>();
//         sorted_elevators.sort_by_key(|(name, _)| *name);

//         for (name, elevator_state) in sorted_elevators {
//             writeln!(f, "  {name}:")?;

//             for line in elevator_state.to_string().lines() {
//                 writeln!(f, "    {line}")?;
//             }
//         }

//         writeln!(f, "Orders:")?;
//         writeln!(f, "  {:>6} | {:<16} | {:<16}", "Floor", "Down", "Up")?;

//         for (floor, hall_request) in self.hall_requests.iter().enumerate().rev() {
//             writeln!(
//                 f,
//                 "  {:>6} | {:<16} | {:<16}",
//                 floor + 1,
//                 hall_request.down,
//                 hall_request.up,
//             )?;
//         }

//         Ok(())
//     }
// }

// impl Worldview {
//     pub fn new(name: String, num_floors: usize) -> Self {
//         Self {
//             name,
//             hall_requests: vec![HallRequest::default(); num_floors],
//             num_floors,
//             elevators: HashMap::new(),
//             iteration: 0,
//         }
//     }
//     pub fn add_request(&mut self, floor: usize, direction: ElevatorDirection) {
//         match direction {
//             ElevatorDirection::Up => self.hall_requests[floor].up = HallRequestState::Requested,
//             ElevatorDirection::Down => self.hall_requests[floor].down = HallRequestState::Requested,
//             _ => panic!("Tried to assign request with invalid direction"),
//         }
//     }

//     pub fn clear_request(&mut self, floor: usize, direction: ElevatorDirection) {
//         match direction {
//             ElevatorDirection::Up => self.hall_requests[floor].up = HallRequestState::Inactive,
//             ElevatorDirection::Down => self.hall_requests[floor].down = HallRequestState::Inactive,
//             _ => panic!("Tried to assign request with invalid direction"),
//         }
//     }
//     // Velger beste heis for en bestilling
//     pub fn assign_requests(&mut self) {
//         let hall_requests = self
//             .hall_requests
//             .iter()
//             .map(|request| {
//                 (
//                     request.up != HallRequestState::Inactive,
//                     request.down != HallRequestState::Inactive,
//                 )
//             })
//             .collect();
//         let states = self
//             .elevators
//             .iter()
//             .filter(|(_, v)| v.active)
//             .map(|(k, v)| (k.to_owned(), v.into()))
//             .collect();

//         let assignments = match assigner::run_hall_request_assigner(hall_requests, states) {
//             Ok(assignments) => assignments,
//             Err(message) => {
//                 error!("Could not assign requests: {message}");
//                 return;
//             }
//         };

//         for (name, assigned_hall_requests) in assignments.iter() {
//             for (floor, (up, down, _)) in assigned_hall_requests.iter().enumerate() {
//                 if *up {
//                     self.hall_requests[floor].up = HallRequestState::Assigned(name.to_string());
//                 }

//                 if *down {
//                     self.hall_requests[floor].down = HallRequestState::Assigned(name.to_string());
//                 }
//             }
//         }
//     }
//     pub fn requests_for_elevator(&self, name: &String) -> Option<LocalRequests> {
//         let mut requests = LocalRequests::new(self.num_floors);

//         for (floor, cab_request) in self.elevators.get(name)?.cab_requests.iter().enumerate() {
//             if *cab_request {
//                 requests.add_cab(floor);
//             }
//         }

//         for (floor, hall_request) in self.hall_requests.iter().enumerate() {
//             if hall_request.up == HallRequestState::Assigned(name.clone()) {
//                 requests.add_up(floor);
//             }

//             if hall_request.down == HallRequestState::Assigned(name.clone()) {
//                 requests.add_down(floor);
//             }
//         }

//         return Some(requests);
//     }
//     pub fn requests_for_local_elevator(&self) -> LocalRequests {
//         self.requests_for_elevator(&self.name)
//             .unwrap_or(LocalRequests::new(self.num_floors))
//     }
//     pub fn set_local_elevator_state(&mut self, local_elevator_state: ElevatorView) {
//         self.elevators
//             .insert(self.name.clone(), local_elevator_state.clone());
//     }
//     pub fn local_elevator_state(&mut self) -> &mut ElevatorView {
//         if !self.elevators.contains_key(&self.name) {
//             self.elevators.insert(
//                 self.name.clone(),
//                 ElevatorView {
//                     active: true,
//                     cab_requests: vec![false; self.num_floors],
//                     state: ElevatorState {
//                         direction: ElevatorDirection::Stopped,
//                         behaviour: Behaviour::Idle,
//                         obstruction: false,
//                         floor: 0,
//                     },
//                     timestamp_last_event: SystemTime::now(),
//                 },
//             );
//         }

//         self.elevators.get_mut(&self.name).unwrap()
//     }
//     pub fn sync_with_master(&mut self, master_state: Worldview) {
//         let local_elevator_state = self.local_elevator_state().to_owned();

//         *self = Self {
//             name: self.name.clone(),
//             ..master_state
//         };

//         self.elevators
//             .insert(self.name.clone(), local_elevator_state);
//     }
// }
