use serde::{Deserialize, Serialize};
use std::iter::zip;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requests {
    cab: Vec<bool>,
    down: Vec<bool>,
    up: Vec<bool>,
}

impl Requests {
    /// Creates a new empty requests struct with given number of floors.
    pub fn new(number_of_floors: usize) -> Self {
        Self {
            cab: vec![false; number_of_floors],
            down: vec![false; number_of_floors],
            up: vec![false; number_of_floors],
        }
    }

    // Adds a upwards hall request at given floor.
    pub fn add_up(&mut self, floor: usize) {
        self.up[floor] = true;
    }

    // Adds a downwards hall request at given floor.
    pub fn add_down(&mut self, floor: usize) {
        self.down[floor] = true;
    }

    /// Adds a cab request for given floor.
    pub fn add_cab(&mut self, floor: usize) {
        self.cab[floor] = true;
    }

    /// Clears upwards hall request and cab request at given floor.
    pub fn clear_up(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.up[floor] = false;
    }

    /// Clears downwards hall request and cab reqeust at given floor.
    pub fn clear_down(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.down[floor] = false;
    }

    /// Iterates over all requests from bottom to top floor.
    /// Each floor is given as a nested tuple of bools on the form `((up, down), cab)`.
    pub fn iter(&self) -> impl Iterator<Item = ((&bool, &bool), &bool)> {
        zip(&self.up, &self.down).zip(&self.cab)
    }

    /// Checks if an upwards hall request or a cab request exists at given floor.
    pub fn up_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.up[floor]
    }

    /// Checks if a downwards hall request or a cab request exists at given floor.
    pub fn down_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.down[floor]
    }

    /// Checks if there is any request for given floor.
    pub fn any_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.up[floor] || self.down[floor]
    }

    /// Checks if any request exists at any floor.
    pub fn any_exists(&self) -> bool {
        self.cab.contains(&true) || self.up.contains(&true) || self.down.contains(&true)
    }

    /// Checks if any requests exists below given floor.
    pub fn any_below_floor(&self, floor: usize) -> bool {
        self.cab[..floor].contains(&true)
            || self.up[..floor].contains(&true)
            || self.down[..floor].contains(&true)
    }

    /// Checks if any requests exists above given floor.
    pub fn any_above_floor(&self, floor: usize) -> bool {
        self.cab[floor + 1..].contains(&true)
            || self.up[floor + 1..].contains(&true)
            || self.down[floor + 1..].contains(&true)
    }
}
