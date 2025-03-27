use super::hall::HallRequestDirection;

pub struct HallRequestAssignments {
    up: Vec<Option<String>>,
    down: Vec<Option<String>>,
}

impl HallRequestAssignments {
    pub fn new(num_floors: usize) -> Self {
        Self {
            up: vec![None; num_floors],
            down: vec![None; num_floors],
        }
    }

    pub fn assign(&mut self, floor: usize, direction: HallRequestDirection, name: String) {
        match direction {
            HallRequestDirection::Up => self.up[floor] = Some(name),
            HallRequestDirection::Down => self.down[floor] = Some(name),
        }
    }

    pub fn clear(&mut self, floor: usize, direction: HallRequestDirection) {
        match direction {
            HallRequestDirection::Up => self.up[floor] = None,
            HallRequestDirection::Down => self.down[floor] = None,
        }
    }
}
