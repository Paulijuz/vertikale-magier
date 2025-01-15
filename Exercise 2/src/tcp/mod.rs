use std::{io::{Read, Write}, net::TcpStream, thread::sleep, time::Duration};

const IP: &str = "10.100.23.204";
const PORT: u32 = 33546;

pub fn tcp() -> std::io::Result<()> {
    let mut stream: TcpStream = TcpStream::connect(format!("{}:{}", IP, PORT))?;
    loop {
        let message = "Hello from Vertical Magics!\0";

        println!("Sender melding: {}", message);

        stream.write(message.as_bytes())?;
        
        let mut buffer = [0; 1024];

        stream.read(&mut buffer)?;

        println!("Mottok melding: {}", std::str::from_utf8(&buffer).unwrap());
        
        sleep(Duration::from_secs(1));
    }
} // the stream is closed here