use std::process::Command;

fn run_hall_request_assigner(hall_requests: &str, states: &str) -> Result<String, String> {
    // Construct the input JSON
    let input_json = format!(
        r#"{{"hallRequests":{},"states":{}}}"#,
        hall_requests, states
    );

    // Run the hall_request_assigner program with the provided input
    let output = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&input_json)
        .output()
        .expect("Failed to start hall_request_assigner");

    // Return the output of the program
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn main() {
    // Example input
    let hall_requests = r#"[[false,false],[true,false],[false,false],[false,true]]"#;
    let states = r#"{"one":{"behaviour":"moving","floor":2,"direction":"up","cabRequests":[false,false,true,true]},"two":{"behaviour":"idle","floor":0,"direction":"stop","cabRequests":[false,false,false,false]}}"#;

    // Run the hall_request_assigner function and print the result
    match run_hall_request_assigner(hall_requests, states) {
        Ok(output) => println!("{}", output),
        Err(error) => eprintln!("Error: {}", error),
    }
}