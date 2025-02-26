use std::fs;
use std::{thread::sleep, time::Duration};
use std::process::Command;
use std::fs::File;
use std::io::{Read, Write};
use serde_json::from_str;
use serde_json::to_string_pretty;
use log::{debug, error, info};

use crate::request_dispatch::AllElevatorStates;


pub fn load_state_from_file(file_path: &str) -> Result<AllElevatorStates, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut json_string = String::new();
    file.read_to_string(&mut json_string)?;

    let state: AllElevatorStates = match from_str(&json_string) {
        Ok(state) => state,
        Err(e) => {
            error!("Feil ved deserialisering av JSON: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "JSON deserialiseringsfeil"));
        }
    };

    Ok(state)
}


pub fn save_state_to_file(state: &AllElevatorStates, file_path: &str) -> Result<(), std::io::Error> {
    let json_string = to_string_pretty(state).unwrap();
    let mut file = File::create(file_path)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}




// fn start_backup() {

//     const INTERVAL_MS: u64 = 100;
//     const MAX_NUM: i32 = 1000;
//     const BACKUP_FILE_NAME: &str = "backup.txt";


//     println!("Slaven (back-up) startet");
//     fs::write(BACKUP_FILE_NAME, "").expect("kunne ikke lage fil");

//     let mut prev_contents = String::from_utf8(
//         fs::read(BACKUP_FILE_NAME).expect("Kunne ikke lese fra fil.")
//     ).expect("Filen inneholde ikke gyldig tekst.");


// loop {
//     println!("Backup waiting.");

//     sleep(Duration::from_millis(INTERVAL_MS));
    
//     let new_contents = String::from_utf8(
//         fs::read(BACKUP_FILE_NAME).expect("Kunne ikke lese fra fil.")
//     ).expect("Filen inneholde ikke gyldig tekst.");
    
//     if prev_contents == new_contents {
//         break;
//     };

//     prev_contents = new_contents.clone();
// }

// println!("New primary started!");

// let mut start_n = 0;
// if !prev_contents.is_empty(){
//     start_n = prev_contents.parse().expect("contents har feilet");
// }
// // så lenge verdien som back-upen tar over ikke er 0, alstå at den ikke tar over fra start,
// //vil den bruke den forrige stringen(prev_contents) som er lagret i tekst-filen og fortsette derifra
// //parse brukes til å konvertere stringen til en int i dette tilfellet

// if start_n >= MAX_NUM {
//     println!("Backup done!");
//     return;
// }
// //husker å slutte å telle^

// Command::new("gnome-terminal")
//     .args(["--", "cargo", "run"])
//     .spawn()
//     .expect("failed to execute child");
// //starter nytt program^

// for n in start_n..=MAX_NUM {
//     println!("number is: {}", n);

//     fs::write(BACKUP_FILE_NAME, n.to_string().as_bytes()).expect("Kunne ikke skrive til fil.");

//     sleep(Duration::from_millis(INTERVAL_MS));
// }
// //skriver til back-up-filen etter at primary er død^

// }


