use std::{net::UdpSocket, thread::sleep, time::Duration};

// 10.100.23.204

pub fn udp() -> std::io::Result<()> {
    // loop {
    //     let socket = UdpSocket::bind("0.0.0.0:30000")?;

    //     let mut buffer = [0; 256];
    //     let (amount, _source) = socket.recv_from(&mut buffer)?;

    //     let recieved_message = &buffer[0..amount];

    //     println!("Mottok melding: {}", std::str::from_utf8(recieved_message).unwrap())
    // }

    let receive_socket = UdpSocket::bind("0.0.0.0:20026")?;
    let send_socket = UdpSocket::bind("0.0.0.0:0")?;
    send_socket.set_broadcast(true)?;

    loop {
        let message = "Hei fra plass nr. 9!";

        println!("Sender melding: {}", message);
        send_socket.send_to(message.as_bytes(), "10.100.23.204:20026")?;

        let mut buffer = [0; 256];
        let (amount, _source) = receive_socket.recv_from(&mut buffer)?;

        let recieved_message = &buffer[0..amount];

        println!(
            "Mottok melding: {}",
            std::str::from_utf8(recieved_message).unwrap()
        );

        sleep(Duration::from_millis(1000));
    }
}
