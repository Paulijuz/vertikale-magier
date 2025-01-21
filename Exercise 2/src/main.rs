use std::io::Error;

mod tcp;
mod udp;

fn main() -> std::io::Result<()> {
    let protocol = std::env::args().nth(1);

    if protocol.is_none() {
        return Err(Error::new(
            std::io::ErrorKind::InvalidInput,
            "No protocol specified.",
        ));
    }

    let protocol = protocol.unwrap();

    println!("Protocol: {}", protocol);

    match protocol.as_str() {
        "udp" => udp::udp()?,
        "tcp" => tcp::tcp()?,
        _ => {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid protocol specified.",
            ))
        }
    }

    return Ok(());
}
