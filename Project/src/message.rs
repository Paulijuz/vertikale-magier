use serde::{Deserialize, Serialize};

use crate::{
    elevator::controller::ElevatorState,
    requests::{
        cab::CabRequests,
        hall::{HallRequestDirection, HallRequests},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    SynchronizeRequests(HallRequests, CabRequests),
    NewHallRequest(usize, HallRequestDirection),
    NewCabRequest(usize, String),
    HallRequestAssignments(/* TODO */),
    ClearHallRequest(usize, HallRequestDirection, u32),
    ClearCabRequest(usize, String, u32),
    ElevatorState(String, ElevatorState),
}
