extern crate opensim_networking;
extern crate toml;

use opensim_networking::login::{LoginRequest, hash_password};
use opensim_networking::circuit::Circuit;

use std::io::prelude::*;
use std::fs::File;

fn main() {
/*    // Read the configuration file.
    let mut file = match File::open("establish-circuit.toml") {
        Ok(f) => f,
        Err(_) => {
            panic!("Copy establish-circuit.toml.tpl to establisk-circuit.toml and populate it.")
        }
    };
    let mut raw_data = String::new();
    file.read_to_string(&mut raw_data).unwrap();
    let config = toml::Parser::new(&raw_data).parse().unwrap();

    // First we perform a login.
    let user = config.get("user").and_then(|t| t.as_table()).unwrap();
    let request = LoginRequest {
        first_name: user.get("first_name").and_then(|s| s.as_str()).unwrap().to_string(),
        last_name: user.get("last_name").and_then(|s| s.as_str()).unwrap().to_string(),
        password_hash: hash_password(user.get("password_plain").and_then(|s| s.as_str()).unwrap()),
        start: "last".to_string(),
    };

    println!("Performing login request: {:?}", request);

    let resp = match request.perform(config.get("sim")
        .and_then(|x| x.as_table())
        .and_then(|x| x.get("loginuri"))
        .and_then(|x| x.as_str())
        .unwrap()) {
        Err(e) => panic!("Login failed, err: {:?}", e),
        Ok(r) => {
            println!("Login successful: {:?}", r);
            r
        }
    };

    // Now establish the circuit.
    let circuit = match Circuit::initiate(resp) {
        Err(e) => panic!("Circuit establisment failed, err: {:?}", e),
        Ok(c) => c
    };
    println!("Established circuit successully.");
*/}
