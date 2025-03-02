use serde::{Deserialize, Serialize};

pub const NUMBER_OF_FLOORS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Stopped,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Request {
    pub hall_up: bool,
    pub hall_down: bool,
    pub cab: bool,
}

pub type Requests = [Request; NUMBER_OF_FLOORS];

pub fn requests_below_floor(requests: &Requests, floor: usize) -> bool {
    requests[..floor]
        .iter()
        .any(|request| request.cab || request.hall_down || request.hall_up)
}

pub fn requests_above_floor(requests: &Requests, floor: usize) -> bool {
    requests[floor + 1..]
        .iter()
        .any(|request| request.cab || request.hall_down || request.hall_up)
}

pub fn requests_at_floor(requests: &Requests, floor: usize, direction: Option<Direction>) -> bool {
    let request = requests[floor];

    request.cab
        || match direction {
            Some(Direction::Up) => request.hall_up,
            Some(Direction::Down) => request.hall_down,
            _ => request.hall_up || request.hall_down,
        }
}
