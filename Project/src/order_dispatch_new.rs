use std::net::UdpSocket;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::thread;
use std::{net::{ TcpListener, TcpStream},io::{Read, Write}};
use log::{info, error, warn};
use serde::{Serialize, Deserialize};
use crossbeam_channel::{select};
use std::collections::{HashMap, HashSet};

use crate::elevator_controller::NUMBER_OF_FLOORS;
use crate::network::socket::{Host, Client};
use crate::elevator_controller::Order;
use crate::elevator_controller::{ States, Direction, ElevatorOrders};


const ADVERTISMENT_PORT: u16 = 52000;
const HOST_PORT: u16 = 44638;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SingleElevatorState {
    pub id: u8,
    pub direction: Direction,
    pub floor: u8, // TOOD: Denne typen kan vel egentlig være usize?
    pub cab_requests: [bool; NUMBER_OF_FLOORS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] //gir enumen den tilgangen (må stå over hver enum)
enum ElevatorRole {
    Master,
    Slave,
}
//vet ikke om vi trenger den over^

// MasterState: Holder oversikt over alle heiser og bestillinger

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum HallRequestState {
    Inactive,
    Requested,
    Assigned(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct HallRequest {
    up: HallRequestState,
    down: HallRequestState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllElevatorStates {
    pub elevators: HashMap<u8, SingleElevatorState>,   //Liste over alle aktive heiser
    pub hall_requests: [HallRequest; NUMBER_OF_FLOORS],
}
//endre til floor variabelen i vec^

//udp for å finne master 
impl AllElevatorStates {
    pub fn new() -> Self {
        Self {
            elevators: HashMap::new(),
            hall_requests: [HallRequest {
                up: HallRequestState::Inactive,
                down: HallRequestState::Inactive,
            }; NUMBER_OF_FLOORS],
        }
    }

    // Velger beste heis for en bestilling
    pub fn assign_order(&mut self, floor: u8, direction: Direction) {

        // TODO: Skriv om til å bruke utdelt program.

        //gir tilgang til å mutere heisen, tar også inn etasjen forespørselen skal til og retningen 
        //heisen skal.
        let mut best_elevator: Option<u8> = None;
        let mut best_distance = u8::MAX;

        for elevator in self.elevators.values() {
            let distance = (elevator.floor as i8 - floor as i8).abs() as u8;

            if distance < best_distance {
                best_distance = distance;
                best_elevator = Some(elevator.id);
            }
        }

        if let Some(id) = best_elevator {
            match direction {
                Direction::Down => self.hall_requests[floor as usize].down = HallRequestState::Assigned(id),
                Direction::Up => self.hall_requests[floor as usize].up = HallRequestState::Assigned(id),
                _ => panic!("Prøvde a tildele en bestilling med ugyldig rettning."),
            }

            info!("Tildelte oppdrag til heis {}: {}", id, floor);
        } else {
            error!("Kunne ikke tildele ordre til heis");
        }
    }
}
    
//Håndter forespørsel fra slave
/*
fn handle_slave_request(stream: &mut TcpStream, master_state: Arc<Mutex<AllElevatorStates>>) {
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
*/
//Starter TCP-server for Master
pub fn start_master_server() {
    let master: Host<AllElevatorStates> = Host::new_tcp_host(Some(HOST_PORT));
    info!("Master lytter på port {}", HOST_PORT);

    let all_elevator_states = Arc::new(Mutex::new(AllElevatorStates::new()));
    let mut slave_addresses: HashSet<SocketAddrV4> = HashSet::new();

    loop {
        select! {
            recv(master.receive_channel()) -> message => {
                let (address, single_elevator_state) = message.unwrap();
                slave_addresses.insert(address);

                info!("Master mottok melding fra slave: {:?}", single_elevator_state);

                let mut state = all_elevator_states.lock().unwrap();
                
                // Ta imot nye bestillinger
                for (floor, request) in single_elevator_state.hall_requests.iter().enumerate() {
                    if state.hall_requests[floor] != *request {
                        if request.up != HallRequestState::Inactive {
                            state.assign_order(floor as u8, Direction::Up)
                        }

                        if request.down != HallRequestState::Inactive {
                            state.assign_order(floor as u8, Direction::Down)
                        }

                        // TODO: Håndter situasjon hvor requests er ulike, men ikke inaktiv
                    }
                }

                // Informere alle slaver om nye bestillinger
                for slave_address in &slave_addresses {
                    master.send_channel().send((*slave_address, state.to_owned())).unwrap();
                }
            }
        }
    }

    // for stream in listener.incoming() {
    //     match stream {
    //         Ok(mut stream) => {
    //             info!("Ny slave tilkoblet.");
    //             let master_state = Arc::clone(&master_state);
    //             thread::spawn(move || handle_slave_request(&mut stream, master_state));
    //         }
    //         Err(e) => error!("Tilkobling feilet: {}", e),
    //     }
    // }
}

pub fn start_slave_server() {
    let slave: Client<AllElevatorStates> = Client::new_tcp_client([127, 0, 0, 1], HOST_PORT).unwrap();

    let mut all_elevator_states = AllElevatorStates::new();
    all_elevator_states.elevators.insert(0, SingleElevatorState {
        id: 0,
        cab_requests: [false; 4],
        direction: Direction::Up,
        floor: 0,
    });
    all_elevator_states.hall_requests[0].up = HallRequestState::Requested;

    slave.sender().send(all_elevator_states).unwrap();

    println!("Mottok: {:?}", slave.receiver().recv().unwrap());
}