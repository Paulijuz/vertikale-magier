use serde::{Deserialize, Serialize};
use std::iter::zip;

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// pub enum RequestDirection {
//     Up,
//     Down,
// }

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// pub enum RequestType {
//     Hall(RequestDirection),
//     Cab
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRequests {
    cab: Vec<bool>,
    hall_down: Vec<bool>,
    hall_up: Vec<bool>,
}

impl LocalRequests {
    /// Creates a new empty requests struct with given number of floors.
    pub fn new(num_floors: usize) -> Self {
        Self {
            cab: vec![false; num_floors],
            hall_up: vec![false; num_floors],
            hall_down: vec![false; num_floors],
        }
    }

    pub fn from_vectors(cab: Vec<bool>, hall: &Vec<(bool, bool)>) -> Self {
        Self {
            cab,
            hall_up: hall.iter().map(|&(up, _)| up).collect(),
            hall_down: hall.iter().map(|&(_, down)| down).collect(),
        }
    }

    // Adds a upwards hall request at given floor.
    pub fn add_up(&mut self, floor: usize) {
        self.hall_up[floor] = true;
    }

    // Adds a downwards hall request at given floor.
    pub fn add_down(&mut self, floor: usize) {
        self.hall_down[floor] = true;
    }

    /// Adds a cab request for given floor.
    pub fn add_cab(&mut self, floor: usize) {
        self.cab[floor] = true;
    }

    /// Clears upwards hall request and cab request at given floor.
    pub fn clear_up(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.hall_up[floor] = false;
    }

    /// Clears downwards hall request and cab reqeust at given floor.
    pub fn clear_down(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.hall_down[floor] = false;
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


//     // Adds a request of given type at given floor.
//     pub fn add(&mut self, floor: usize, request_type: RequestType) {
//         match request_type {
//             RequestType::Cab => self.cab[floor] = true,
//             RequestType::Hall(direction) => {
//                 match direction {
//                     RequestDirection::Up => self.hall_up[floor] = true,
//                     RequestDirection::Down => self.hall_down[floor] = true,
//                 }
//             }
//         }
//     }

//     /// Clears hall request and cab request at given floor.
//     pub fn clear(&mut self, floor: usize, direction: Option<RequestDirection>) {
//         self.cab[floor] = false;

//         if direction == Some(RequestDirection::Up) || direction.is_none() {
//             self.hall_up[floor] = false;
//         }

//         if direction == Some(RequestDirection::Down) || direction.is_none() {
//             self.hall_down[floor] = false;
//         }
//     }

//     /// Iterates over all requests from bottom to top floor.
//     /// Each floor is given as a nested tuple of bools on the form `((up, down), cab)`.
//     pub fn iter(&self) -> impl Iterator<Item = ((&bool, &bool), &bool)> {
//         zip(&self.hall_up, &self.hall_down).zip(&self.cab)
//     }

//     /// Checks if a hall request or a cab request exists at given floor.
//     pub fn at_floor_in_direction(&self, floor: usize, direction: Option<RequestDirection>) -> bool {
//         self.cab[floor] || match direction {
//             Some(RequestDirection::Down) => self.hall_down[floor],
//             Some(RequestDirection::Up) => self.hall_up[floor],
//             None => self.hall_down[floor] || self.hall_up[floor],
//         }
//     }

//     /// Checks if any request exists at any floor.
//     pub fn any_exists(&self) -> bool {
//         self.cab.contains(&true) || self.hall_up.contains(&true) || self.hall_down.contains(&true)
//     }

//     /// Checks if any requests exists after a floor in the given direction.
//     pub fn after_floor_in_direction(&self, floor: usize, direction: Option<RequestDirection>) -> bool {
//         match direction {
//             Some(RequestDirection::Down) => {
//                 self.cab[floor + 1..].contains(&true)
//                 || self.hall_up[floor + 1..].contains(&true)
//                 || self.hall_down[floor + 1..].contains(&true)
//             },
//             Some(RequestDirection::Up) => {
//                 self.cab[..floor].contains(&true)
//                 || self.hall_up[..floor].contains(&true)
//                 || self.hall_down[..floor].contains(&true)
//             },
//             None => false,
//         }
        
//     }
// }
