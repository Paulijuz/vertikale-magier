use UdpSocket;
use Ipv4Addr;
use Duration, Instant;
use std::thread;
use Arc, Mutex;
use std::{net::{UdpSocket, TcpListener, TcpStream}io::{Read, Write}, time::Duration};
use log::{info, error, warn};


mod elevator_controller;
mod inputs;
mod light_sync;
mod order_dispatch;
mod timer;

use crate::elevator::{Elevator, States, Direction, OrderArray};


const UDP_PORT: &str = "20026";
const TCP_PORT: &str = "33546";
//const BROADCAST_ADDR: &str = "10.100.23.255:20026"; // Sender master IP her


// Struktur for å lagre informasjon om hver heis
#[derive(Debug, Clone, Copy)]
pub struct ElevatorInfo {
    pub id: u8,
    pub status: Elevator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] //gir enumen den tilgangen (må stå over hver enum)
enum ElevatorRole {
    Master,
    Slave,
}
//vet ikke om vi trenger den over^

// MasterState: Holder oversikt over alle heiser og bestillinger
#[derive(Debug, Clone, Copy)]
pub struct MasterState {
    pub elevators: Vec<ElevatorInfo>,   //Liste over alle aktive heiser
    pub orders: Vec<(u8, Direction)>,  //Bestillinger (etasje, retning) 
}
//endre til floor variabelen i vec^

//udp for å finne master 



impl MasterState {
    // Oppretter en ny MasterState
    pub fn new() -> Self {
        Self {
            elevators: Vec::new(),
            orders: Vec::new(),
        }
    }

    // Oppdaterer status for en heis
    pub fn update_elevator_status(&mut self, elevator_id: u8, status: Elevator) {
        if let Some(elevator) = self.elevators.iter_mut().find(|e| e.id == elevator_id) {
            elevator.status = status;
        } else {
            self.elevators.push(ElevatorInfo { id: elevator_id, status });
        }

        info!("Oppdatert status for heis {}: {:?}", elevator_id, status);
    }

    // Velger beste heis for en bestilling
    pub fn assign_order(&mut self, floor: u8, direction: Direction) -> Option<u8> {
        //gir tilgang til å mutere heisen, tar også inn etasjen forespørselen skal til og retningen 
        //heisen skal.
        let mut best_elevator: Option<u8> = None;
        let mut best_distance = u8::MAX;

        for elevator in &self.elevators {
            if elevator.status.state == States::OutOfOrder { continue; }  // Ignorer ødelagte heiser
            let distance = (elevator.status.floor as i8 - floor as i8).abs() as u8;

            if distance < best_distance {
                best_distance = distance;
                best_elevator = Some(elevator.id);
            }
        }

        best_elevator
    }
}

//Starter TCP-server for Master
pub fn start_master_server() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).expect("Kunne ikke binde master-server.");
    let master_state = Arc::new(Mutex::new(MasterState::new()));

    info!("Master lytter på port {}", TCP_PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                info!("Ny slave tilkoblet.");
                let master_state = Arc::clone(&master_state);
                thread::spawn(move || handle_slave_request(&mut stream, master_state));
            }
            Err(e) => error!("Tilkobling feilet: {}", e),
        }
    }
}


//Håndter forespørsel fra slave
fn handle_slave_request(stream: &mut TcpStream, master_state: Arc<Mutex<MasterState>>) {
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                info!("Slave frakoblet");
                break;
            }
            Ok(n) => {
                let received_data = String::from_utf8_lossy(&buffer[..n]);
                info!("Mottatt melding: {}", received_data);

                let mut state = master_state.lock().unwrap();

                if received_data.contains("STATUS") {
                    handle_status_update(&received_data, &mut state);
                } else if received_data.contains("CALL") {
                    handle_new_order(&received_data, &mut state, stream);
                }
            }
            Err(e) => {
                error!("Feil ved lesing fra stream: {}", e);
                break;
            }
        }
    }
}


// Håndter statusoppdatering fra en slave
fn handle_status_update(received_data: &str, state: &mut MasterState) {
    let parts: Vec<&str> = received_data.split_whitespace().collect();
    if parts.len() >= 6 {
        let elevator_id: u8 = parts[1].parse().unwrap_or(0);
        let floor: u8 = parts[2].parse().unwrap_or(0);
        let direction = match parts[3] {
            "up" => Direction::Up,
            "down" => Direction::Down,
            _ => Direction::Stopped,
        };
        let state_enum = match parts[4] {
            "idle" => States::Idle,
            "moving" => States::Moving,
            "door_open" => States::DoorOpen,
            _ => States::OutOfOrder,
        };

        let orders: OrderArray = [Order {
            outside_call_up: parts[5] == "1",
            outside_call_down: parts[6] == "1",
            inside_call: parts[7] == "1",
        }; 4];

        let elevator = Elevator {
            state: state_enum,
            direction,
            obstruction: false,
            floor,
            orders,
        };

        state.update_elevator_status(elevator_id, elevator);
    }
}


// Håndter ny bestilling
fn handle_new_order(received_data: &str, state: &mut MasterState, stream: &mut TcpStream) {
    let parts: Vec<&str> = received_data.split_whitespace().collect();
    if parts.len() >= 3 {
        let floor: u8 = parts[1].parse().unwrap_or(0);
        let direction = match parts[2] {
            "up" => Direction::Up,
            "down" => Direction::Down,
            _ => Direction::Stopped,
        };

        if let Some(elevator_id) = state.assign_order(floor, direction) {
            let message = format!("Dispatch {} {}\n", floor, parts[2]);
            if let Err(e) = stream.write_all(message.as_bytes()) {
                error!("Feil ved sending av ordre til heis {}: {}", elevator_id, e);
            } else {
                info!("Sendt oppdrag til heis {}: {}", elevator_id, message);
            }
        } else {
            state.orders.push((floor, direction));
        }
    }
}

