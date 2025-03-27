use serde::{Deserialize, Serialize};
use std::iter::zip;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestDirection {
    Up,
    Down
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestType {
    Hall(RequestDirection),
    Cab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requests {
    cab: Vec<bool>,
    hall_down: Vec<bool>,
    hall_up: Vec<bool>,
}

impl Requests {
    /// Creates a new empty requests struct with given number of floors.
    pub fn new(num_floors: usize) -> Self {
        Self {
            cab: vec![false; num_floors],
            hall_down: vec![false; num_floors],
            hall_up: vec![false; num_floors],
        }
    }

    // Adds a upwards hall request at given floor.
    pub fn add(&mut self, floor: usize, request_type: RequestType) {
        match request_type {
            RequestType::Cab => self.cab[floor] = true,
            RequestType::Hall(RequestDirection::Up) => self.hall_up[floor] = true,
            RequestType::Hall(RequestDirection::Down) => self.hall_up[floor] = true,
        }
    }

    /// Clears upwards hall request and cab request at given floor.
    pub fn clear(&mut self, floor: usize, request_type: RequestType) {
        match request_type {
            RequestType::Cab => self.cab[floor] = false,
            RequestType::Hall(RequestDirection::Up) => self.hall_up[floor] = false,
            RequestType::Hall(RequestDirection::Down) => self.hall_up[floor] = false,
        }
    }

    /// Iterates over all requests from bottom to top floor.
    /// Each floor is given as a nested tuple of bools on the form `((up, down), cab)`.
    pub fn iter(&self) -> impl Iterator<Item = ((&bool, &bool), &bool)> {
        zip(&self.hall_up, &self.hall_down).zip(&self.cab)
    }

    /// Checks if an upwards hall request or a cab request exists at given floor.
    pub fn up_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.hall_up[floor]
    }

    /// Checks if a downwards hall request or a cab request exists at given floor.
    pub fn down_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.hall_down[floor]
    }

    /// Checks if there is any request for given floor.
    pub fn any_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.hall_up[floor] || self.hall_down[floor]
    }

    /// Checks if any request exists at any floor.
    pub fn any_exists(&self) -> bool {
        self.cab.contains(&true) || self.hall_up.contains(&true) || self.hall_down.contains(&true)
    }

    /// Checks if any requests exists below given floor.
    pub fn any_below_floor(&self, floor: usize) -> bool {
        self.cab[..floor].contains(&true)
            || self.hall_up[..floor].contains(&true)
            || self.hall_down[..floor].contains(&true)
    }

    /// Checks if any requests exists above given floor.
    pub fn any_above_floor(&self, floor: usize) -> bool {
        self.cab[floor + 1..].contains(&true)
            || self.hall_up[floor + 1..].contains(&true)
            || self.hall_down[floor + 1..].contains(&true)
    }
}
