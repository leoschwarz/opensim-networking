//! Contains some code to finish the initiation for a avatar to enter a new region.
// TODO: include stuff like session id somehow with the Circuit struct so it doesn't have to be
// passed around all the time.
use Uuid;
use circuit::Circuit;
use messages::{UseCircuitCode, UseCircuitCode_CircuitCode};
use messages::{CompleteAgentMovement, CompleteAgentMovement_AgentData};

pub fn initiate(circuit: &Circuit, circuit_code: u32, agent_id: Uuid, session_id: Uuid) {
    println!("using circuit code: {}", circuit_code);
    println!("session_id: {}", session_id);
    println!("agent id: {}", agent_id);
    let msg1 = UseCircuitCode {
        circuit_code: UseCircuitCode_CircuitCode {
            code: circuit_code,
            session_id: session_id,
            id: agent_id,
        },
    };
    let msg2 = CompleteAgentMovement {
        agent_data: CompleteAgentMovement_AgentData {
            agent_id: agent_id,
            session_id: session_id,
            circuit_code: circuit_code,
        },
    };

    circuit.send(msg1, true);
    circuit.send(msg2, true);
}
