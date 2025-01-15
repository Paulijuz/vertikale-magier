mod tcp;

fn main() -> std::io::Result<()>  {

    tcp::tcp()?;

    return Ok(());
}