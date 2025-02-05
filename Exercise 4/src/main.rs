use std::fs;
use std::{thread::sleep, time::Duration};
use std::process::Command;

const INTERVAL_MS: u64 = 100;
const MAX_NUM: i32 = 1000;
const BACKUP_FILE_NAME: &str = "backup.txt";

fn main() {
    println!("New backup started!");
    
    let mut prev_contents = String::from_utf8(
        fs::read(BACKUP_FILE_NAME).expect("Kunne ikke lese fra fil.")
    ).expect("Filen inneholde ikke gyldig tekst.");
    
    loop {
        println!("Backup waiting.");

        sleep(Duration::from_millis(INTERVAL_MS));
        
        let new_contents = String::from_utf8(
            fs::read(BACKUP_FILE_NAME).expect("Kunne ikke lese fra fil.")
        ).expect("Filen inneholde ikke gyldig tekst.");
        
        if prev_contents == new_contents {
            break;
        };

        prev_contents = new_contents.clone();
    }

    println!("New primary started!");
    
    let mut start_n = 0;
    if !prev_contents.is_empty(){
        start_n = prev_contents.parse().expect("contents har feilet");
    }
    
    if start_n >= MAX_NUM {
        println!("Backup done!");
        return;
    }

    Command::new("gnome-terminal")
        .args(["--", "cargo", "run"])
        .spawn()
        .expect("failed to execute child");

    for n in start_n..=MAX_NUM {
        println!("number is: {}", n);

        fs::write(BACKUP_FILE_NAME, n.to_string().as_bytes()).expect("Kunne ikke skrive til fil.");

        sleep(Duration::from_millis(INTERVAL_MS));
    }

    sleep(Duration::from_millis(2*INTERVAL_MS));

    fs::write(BACKUP_FILE_NAME, "").expect("Kunne ikke skrive til fil.");

    println!("Primary done!");

}
