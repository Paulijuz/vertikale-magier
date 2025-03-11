use std::iter::zip;

use serde::{Deserialize, Serialize};

pub const NUMBER_OF_FLOORS: usize = 4;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Requests {
    cab: [bool; NUMBER_OF_FLOORS],
    down: [bool; NUMBER_OF_FLOORS],
    up: [bool; NUMBER_OF_FLOORS],
}

impl Requests {
    pub fn any_exists(&self) -> bool{
        self.cab.iter().any(|&r| r)
            || self.down.iter().any(|&r| r)
            || self.up.iter().any(|&r| r)
    }

    pub fn any_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.up[floor] || self.down[floor]
    }

    pub fn up_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.up[floor]
    }

    pub fn down_at_floor(&self, floor: usize) -> bool {
        self.cab[floor] || self.down[floor]
    }

    pub fn any_below_floor(&self, floor: usize) -> bool {
        self.cab[..floor].iter().any(|&r| r)
        || self.down[..floor].iter().any(|&r| r)
        || self.up[..floor].iter().any(|&r| r)
    }

    pub fn any_above_floor(&self, floor: usize) -> bool {
        self.cab[floor+1..].iter().any(|&r| r)
        || self.down[floor+1..].iter().any(|&r| r)
        || self.up[floor+1..].iter().any(|&r| r)
    }

    pub fn add_up(&mut self, floor: usize) {
        self.up[floor] = true;
    }

    pub fn add_down(&mut self, floor: usize) {
        self.down[floor] = true;
    }

    pub fn add_cab(&mut self, floor: usize) {
        self.cab[floor] = true;
    }

    pub fn clear_up(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.up[floor] = false;
    }

    pub fn clear_down(&mut self, floor: usize) {
        self.cab[floor] = false;
        self.down[floor] = false;
    }

    pub fn iter(&self) -> impl Iterator<Item = ((bool, bool), bool)> {
        zip(self.up, self.down).zip(self.cab)
    }
}
