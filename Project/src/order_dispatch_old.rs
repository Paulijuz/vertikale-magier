use crate::{
    elevator_controller::{Direction, ElevatorEvent, ElevatorOrders, Order},
    inputs,
    light_sync::sync_call_lights,
};
use crossbeam_channel as cbc;
use driver_rust::elevio;

const NUMBER_OF_FLOORS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct Elevator {
    pub direction: Direction,
    pub floor: u8, // TOOD: Denne typen kan vel egentlig være usize?
    pub orders: ElevatorOrders,
}

impl Elevator {
    pub fn init() -> Elevator {
        return Elevator {
            direction: Direction::Stopped,
            floor: 0,
            orders: [Order {
                outside_call_down: false,
                outside_call_up: false,
                inside_call: false,
            }; NUMBER_OF_FLOORS],
        };
    }
    pub fn clear_orders_here(&mut self) {
        self.orders[self.floor as usize].inside_call = false;
        self.orders[self.floor as usize].outside_call_up = false;
        self.orders[self.floor as usize].outside_call_down = false;
    }
}

pub fn dispatch_loop(
    elevio_elevator: &elevio::elev::Elevator,
    elevator_command_tx: cbc::Sender<ElevatorOrders>,
    elevator_event_rx: cbc::Receiver<ElevatorEvent>,
) {
    let mut elevator = Elevator::init();
    let rx_channels = inputs::get_input_channels(&elevio_elevator);

    loop {
        cbc::select! {
            recv(elevator_event_rx) -> elevator_event => {
                let elevator_event = elevator_event.unwrap();

                elevator.floor = elevator_event.floor;
                elevator.direction = elevator_event.direction;

                elevator.clear_orders_here();

                elevator_command_tx.send(elevator.orders).unwrap();
                sync_call_lights(&elevio_elevator, &elevator.orders);
            },
            recv(rx_channels.call_button_rx) -> call_button => {
                let call_button = call_button.unwrap();

                match call_button.call {
                    0 => {
                        elevator.orders[call_button.floor as usize].outside_call_up = true;
                    }
                    1 => {
                        elevator.orders[call_button.floor as usize].outside_call_down = true;
                    }
                    2=> {
                        elevator.orders[call_button.floor as usize].inside_call = true;
                    }
                    _ => {
                        panic!("Fikk ukjent knapp.");
                    }
                }

                elevator_command_tx.send(elevator.orders).unwrap();
                sync_call_lights(&elevio_elevator, &elevator.orders);
            },
        }
    }
}

// Master-slave state struct
//master looper over sine kontroll områder
//prioriteringsstruct, evt et random valg av hvem som tar over som master
// med hvem som looper over kontroll området
/*
master er heisen med høyest verdi 1-3, velger alltid en lovlig master
sjekke hvilke heiser som er i livet før master velges (orders dispatcher)
setter default state på resterende heiser blir slave
lage to lister av structs for å ha oversikt over states og en for bestillinger
master sjekker listen hele tiden, deler ut ordre basert på tid/avstand til den ordren
oppdaterer ordren dersom en heis som allerede har mottatt en annen kan fullføre flere ordre

*/
