extern crate opensim_networking;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use opensim_networking::login::{LoginRequest, hash_password};
use opensim_networking::circuit::{Circuit, CircuitConfig};
use opensim_networking::Duration;

use std::io::prelude::*;
use std::fs::File;

#[derive(Deserialize)]
struct Config {
    user: ConfigUser,
    sim: ConfigSim,
}

#[derive(Deserialize)]
struct ConfigUser {
    first_name: String,
    last_name: String,
    password_plain: String,
}

#[derive(Deserialize)]
struct ConfigSim {
    loginuri: String,
}

fn main() {
    // Read the configuration file.
    let mut file = File::open("establish-circuit.toml").expect(
        "Copy establish-circuit.toml.tpl to establisk-circuit.toml and populate it.",
    );
    let mut raw_data = String::new();
    file.read_to_string(&mut raw_data).unwrap();
    let config: Config = toml::from_str(raw_data.as_str()).expect("invalid TOML");

    // First we perform a login.
    let request = LoginRequest {
        first_name: config.user.first_name,
        last_name: config.user.last_name,
        password_hash: hash_password(config.user.password_plain.as_str()),
        start: "last".to_string(),
    };

    println!("Performing login request: {:?}", request);

    let resp = match request.perform(config.sim.loginuri.as_str()) {
        Err(e) => panic!("Login failed, err: {:?}", e),
        Ok(r) => {
            println!("Login successful: {:?}", r);
            r
        }
    };

    // Now establish the circuit.
    let config = CircuitConfig {
        send_timeout: Duration::milliseconds(2500),
        send_attempts: 5,
    };
    let circuit = match Circuit::initiate(resp, config) {
        Err(e) => panic!("Circuit establishment failed, err: {:?}", e),
        Ok(c) => c,
    };
    println!("Established circuit successully.");
}
