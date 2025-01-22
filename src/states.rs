const NUMBER_OF_FLOORS: usize = 4;

#[derive(Clone, Copy)]
pub enum States {
    Idle, Moving, DoorOpen, OutOfOrder
}

#[derive(Clone, Copy)]
pub enum Direction {
    Up, Down, Stopped,
}

#[derive(Clone, Copy)]
pub struct Order {
    pub outside_call_up: bool,
    pub outside_call_down: bool,
    pub inside_call: bool,
}

#[derive(Clone, Copy)]
pub struct Elevator{
    pub state: States,
    pub direction: Direction,
    pub floor: u8,
    pub orders: [Order; NUMBER_OF_FLOORS],
}

impl Elevator {
    pub fn init() -> Elevator {
        return Elevator {
            state: States::Idle,
            direction: Direction::Stopped,
            floor: 0,
            orders: [Order { outside_call_down: false, outside_call_up: false, inside_call: false }; NUMBER_OF_FLOORS],
        }
    }
    pub fn orders_below(self) -> bool {
        self.orders[..self.floor as usize]
            .iter()
            .any(|order| order.inside_call || order.outside_call_down || order.outside_call_up)
    }
    pub fn orders_above(self) -> bool {
        self.orders[self.floor as usize + 1..]
            .iter()
            .any(|order| order.inside_call || order.outside_call_down || order.outside_call_up)
    }
    pub fn should_stop(self) -> bool {
        match self.direction {
            Direction::Down => return 
                self.orders[self.floor as usize].outside_call_down ||
                self.orders[self.floor as usize].inside_call ||
                !self.orders_below(),
            Direction::Up => return 
                self.orders[self.floor as usize].outside_call_up ||
                self.orders[self.floor as usize].inside_call ||
                !self.orders_above(),
            _ => return true,
        }
    }
}
