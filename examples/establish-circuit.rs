extern crate futures;
extern crate opensim_networking;
#[macro_use]
extern crate serde_derive;
extern crate tokio_core;
extern crate toml;

use opensim_networking::circuit::message_handlers;
use opensim_networking::logging::{Log, LogLevel};
use opensim_networking::login::{hash_password, LoginRequest};
use opensim_networking::simulator::{ConnectInfo, Simulator};
use opensim_networking::systems::agent_update::{AgentState, Modality, MoveDirection};
use opensim_networking::types::{Duration, UnitQuaternion, Vector3};

use std::io::prelude::*;
use std::fs::File;
use std::thread;
use futures::future::Future;
use tokio_core::reactor::Core;

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
    // Setup logging.
    let log = Log::new_dir("output/logdir", LogLevel::Debug).expect("Setting up log failed.");

    // Read the configuration file.
    let config = get_config();

    // Perform the login.
    let request = LoginRequest {
        first_name: config.user.first_name,
        last_name: config.user.last_name,
        password_hash: hash_password(config.user.password_plain.as_str()),
        start: "last".to_string(),
    };

    println!("Performing login request: {:?}", request);
    let resp = request
        .perform(config.sim.loginuri.as_str())
        .expect("Login failed.");
    // println!("Login success, response = {:?}", resp);
    println!("Login success.");
    let agent_id = resp.agent_id.clone();
    let session_id = resp.session_id.clone();

    let mut core = Core::new().unwrap();

    let message_handlers = message_handlers::Handlers::default();
    let sim_connect_info = ConnectInfo::from(resp);
    let sim = Simulator::connect(sim_connect_info, message_handlers, core.handle(), log)
        .wait()
        .unwrap();

    // Exemplary texture request.
    let texture_id = sim.region_info().terrain_detail[0].clone();
    let handle = core.handle();
    let texture = core.run(sim.get_texture(&texture_id, &handle)).unwrap();
    println!("texture: {:?}", texture);

    // Let the avatar walk back and forth.
    // TODO: extract position
    let z_axis = Vector3::z_axis();
    let mut state = AgentState {
        position: Vector3::new(0., 0., 0.),
        move_direction: Some(MoveDirection::Forward),
        modality: Modality::Walking,
        // TODO: This initialization is redundant as it was already done in simulator.rs
        body_rotation: UnitQuaternion::from_axis_angle(&z_axis, 0.),
        head_rotation: UnitQuaternion::from_axis_angle(&z_axis, 0.),
    };

    loop {
        for _ in 1..40 {
            let msg = state.to_update_message(agent_id, session_id);
            // TODO: change this to unreliable (false) after debugging
            sim.send_message(msg, true).wait().unwrap();

            thread::sleep(Duration::from_millis(50));
        }
        thread::sleep(Duration::from_millis(200));
        state.move_direction = Some(state.move_direction.unwrap().inverse());
    }
}

fn get_config() -> Config {
    let mut file = File::open("establish-circuit.toml")
        .expect("Copy establish-circuit.toml.tpl to establisk-circuit.toml and populate it.");
    let mut raw_data = String::new();
    file.read_to_string(&mut raw_data).unwrap();
    toml::from_str(raw_data.as_str()).expect("invalid TOML")
}
