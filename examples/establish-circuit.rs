extern crate futures;
extern crate num_traits;
extern crate opensim_networking;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use opensim_networking::login::{hash_password, LoginRequest};
use opensim_networking::circuit::{Circuit, CircuitConfig};
use opensim_networking::{Duration, Quaternion, Vector3};
use opensim_networking::systems::agent_update::{AgentState, Modality, MoveDirection};
use opensim_networking::messages::{AgentUpdate, AgentUpdate_AgentData};
use opensim_networking::logging::FullDebugLogger;

use num_traits::identities::{One, Zero};

use std::io::prelude::*;
use std::fs::File;
use std::thread;
use futures::future::Future;

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
    // Setup our logger.
    let logger = FullDebugLogger::new("output/logdir").unwrap();

    // Read the configuration file.
    let mut file = File::open("establish-circuit.toml")
        .expect("Copy establish-circuit.toml.tpl to establisk-circuit.toml and populate it.");
    let mut raw_data = String::new();
    file.read_to_string(&mut raw_data).unwrap();
    let config: Config = toml::from_str(raw_data.as_str()).expect("invalid TOML");

    // Perform the login.
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
        send_timeout: Duration::from_millis(2500),
        send_attempts: 5,
    };
    let agent_id = resp.agent_id.clone();
    let session_id = resp.session_id.clone();
    let circuit_code = resp.circuit_code.clone();
    let circuit = match Circuit::initiate(resp, config, logger.clone()) {
        Err(e) => panic!("Circuit establishment failed, err: {:?}", e),
        Ok(c) => c,
    };

    println!("Created circuit instance.");
    // Perform the last steps of the circuit initiation.
    opensim_networking::systems::initiation::initiate(
        &circuit,
        circuit_code,
        agent_id,
        session_id,
        logger,
    ).expect("circuit init sequence failed.");
    println!("Finish circuit initialization.");

    // Let the avatar walk back and forth.
    // TODO: extract position
    let mut state = AgentState {
        position: Vector3::zero(),
        move_direction: Some(MoveDirection::Forward),
        modality: Modality::Walking,
        body_rotation: Quaternion::one(),
        head_rotation: Quaternion::one(),
    };

    loop {
        for _ in 1..40 {
            let msg = state.to_update_message(agent_id, session_id);
            // TODO: change this to unreliable (false) after debugging
            circuit.send(msg, true).wait().unwrap();

            thread::sleep(std::time::Duration::from_millis(50));
        }
        thread::sleep(std::time::Duration::from_millis(200));
        state.move_direction = Some(state.move_direction.unwrap().inverse());
    }
}
