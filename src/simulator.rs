use capabilities::Capabilities;
use circuit::{Circuit, CircuitConfig, SendMessage};
pub use circuit::MessageHandlers;
use futures::Future;
use logging::Logger;
use login::LoginResponse;
use messages::MessageInstance;
use messages::all::{CompleteAgentMovement, CompleteAgentMovement_AgentData, UseCircuitCode,
                    UseCircuitCode_CircuitCode};
use std::error::Error;
use types::Duration;

// TODO: Right now here we use LoginResponse, however we should define a struct
// only containing the relevant fields for sim connections so when jumping from
// sim to sim we can pass that data. And for LoginResponse a conversion to that
// representation would be provided.

/// This struct manages all connections from the viewer to a (single) simulator
/// instance.
pub struct Simulator {
    caps: Capabilities,
    circuit: Circuit,
}

impl Simulator {
    pub fn connect<L: Logger>(
        login: &LoginResponse,
        handlers: MessageHandlers,
        logger: &L,
    ) -> Result<Simulator, Box<Error>> {
        let circuit = Self::setup_circuit(login, handlers, logger)?;
        let capabilities = Self::setup_capabilities(login, logger)?;
        Ok(Simulator {
            caps: capabilities,
            circuit: circuit,
        })
    }

    pub fn send_message<M: Into<MessageInstance>>(
        &self,
        message: M,
        reliable: bool,
    ) -> SendMessage {
        self.circuit.send(message, reliable)
    }

    fn setup_circuit<L: Logger>(
        login: &LoginResponse,
        handlers: MessageHandlers,
        logger: &L,
    ) -> Result<Circuit, Box<Error>> {
        let config = CircuitConfig {
            send_timeout: Duration::from_millis(5000),
            send_attempts: 5,
        };
        let agent_id = login.agent_id.clone();
        let session_id = login.session_id.clone();
        let circuit_code = login.circuit_code.clone();

        let circuit = Circuit::initiate(login.clone(), config, handlers, logger.clone())?;

        let message = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: circuit_code,
                session_id: session_id,
                id: agent_id,
            },
        };
        circuit.send(message, true).wait().map_err(Box::new)?;

        let message = CompleteAgentMovement {
            agent_data: CompleteAgentMovement_AgentData {
                agent_id: agent_id,
                session_id: session_id,
                circuit_code: circuit_code,
            },
        };
        circuit.send(message, true).wait().map_err(Box::new)?;

        Ok(circuit)
    }

    fn setup_capabilities<L: Logger>(
        login: &LoginResponse,
        _: &L,
    ) -> Result<Capabilities, Box<Error>> {
        Ok(Capabilities::setup_capabilities(
            login.seed_capability.clone(),
        )?)
    }
}
