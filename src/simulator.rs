use capabilities::Capabilities;
use circuit::{Circuit, CircuitConfig, MessageHandlerError, SendMessage};
pub use circuit::MessageHandlers;
use data::RegionInfo;
use futures::Future;
use logging::Log;
use login::LoginResponse;
use messages::{MessageInstance, MessageType};
use messages::all::{CompleteAgentMovement, CompleteAgentMovement_AgentData, CompletePingCheck,
                    CompletePingCheck_PingID, UseCircuitCode, UseCircuitCode_CircuitCode};
use systems::agent_update::{AgentState, Modality};
use types::{Duration, UnitQuaternion, Vector3};

// TODO: Right now here we use LoginResponse, however we should define a struct
// only containing the relevant fields for sim connections so when jumping from
// sim to sim we can pass that data. And for LoginResponse a conversion to that
// representation would be provided.

/// This struct manages all connections from the viewer to a (single) simulator
/// instance.
pub struct Simulator {
    caps: Capabilities,
    circuit: Circuit,
    // TODO: (future) can this be updated remotely somehow, i.e. by the estate manager?
    region_info: RegionInfo,
}

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ConnectError")]
#[error_chain(result = "")]
pub enum ConnectErrorKind {
    #[error_chain(foreign)] CapabilitiesError(::capabilities::CapabilitiesError),

    #[error_chain(foreign)] IoError(::std::io::Error),

    #[error_chain(foreign)] MpscError(::std::sync::mpsc::RecvError),

    #[error_chain(foreign)] ReadMessageError(::circuit::ReadMessageError),

    #[error_chain(foreign)] SendMessageError(::circuit::SendMessageError),

    #[error_chain(custom)] Msg(String),
}

impl Simulator {
    pub fn connect(
        login: &LoginResponse,
        mut handlers: MessageHandlers,
        logger: &Log,
    ) -> Result<Simulator, ConnectError> {
        // Setup default handlers (TODO move to right place and make more transparent
        // to user?)
        handlers.insert(
            MessageType::StartPingCheck,
            Box::new(|msg, circuit| {
                let start_ping_check = match msg {
                    MessageInstance::StartPingCheck(m) => Ok(m),
                    _ => Err(MessageHandlerError::WrongHandler),
                }?;
                let response = CompletePingCheck {
                    ping_id: CompletePingCheck_PingID {
                        ping_id: start_ping_check.ping_id.ping_id,
                    },
                };
                circuit.send(response, false);
                Ok(())
            }),
        );

        let capabilities = Self::setup_capabilities(login, logger)?;
        let (circuit, region_info) = Self::setup_circuit(login, handlers, logger)?;
        Ok(Simulator {
            caps: capabilities,
            circuit: circuit,
            region_info: region_info,
        })
    }

    pub fn region_info(&self) -> &RegionInfo {
        &self.region_info
    }

    pub fn send_message<M: Into<MessageInstance>>(
        &self,
        message: M,
        reliable: bool,
    ) -> SendMessage {
        self.circuit.send(message, reliable)
    }

    fn setup_circuit(
        login: &LoginResponse,
        handlers: MessageHandlers,
        log: &Log,
    ) -> Result<(Circuit, RegionInfo), ConnectError> {
        let config = CircuitConfig {
            send_timeout: Duration::from_millis(5000),
            send_attempts: 5,
        };
        let agent_id = login.agent_id.clone();
        let session_id = login.session_id.clone();
        let circuit_code = login.circuit_code.clone();

        let circuit = Circuit::initiate(login.clone(), config, handlers, log.clone())?;

        let message = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: circuit_code,
                session_id: session_id,
                id: agent_id,
            },
        };
        circuit.send(message, true).wait()?;

        // Now wait for the RegionHandshake message.
        let timeout = Duration::from_millis(15_000);
        let region_info = match circuit.read(Some(timeout))? {
            MessageInstance::RegionHandshake(handshake) => {
                Ok(RegionInfo::extract_message(handshake))
            }
            _ => Err(ConnectError::from("Did not receive RegionHandshake")),
        }?;

        let message = CompleteAgentMovement {
            agent_data: CompleteAgentMovement_AgentData {
                agent_id: agent_id.clone(),
                session_id: session_id.clone(),
                circuit_code: circuit_code,
            },
        };
        circuit.send(message, true).wait()?;

        // let region_x = 256000.;
        // let region_y = 256000.;
        let local_x = 10.;
        let local_y = 10.;

        let z_axis = Vector3::z_axis();
        let agent_state = AgentState {
            position: Vector3::new(local_x, local_y, 0.),
            move_direction: None,
            modality: Modality::Walking,
            body_rotation: UnitQuaternion::from_axis_angle(&z_axis, 0.),
            head_rotation: UnitQuaternion::from_axis_angle(&z_axis, 0.),
        };
        let message = agent_state.to_update_message(agent_id, session_id);
        circuit.send(message, true).wait()?;

        Ok((circuit, region_info))
    }

    fn setup_capabilities(login: &LoginResponse, _: &Log) -> Result<Capabilities, ConnectError> {
        Ok(Capabilities::setup_capabilities(
            login.seed_capability.clone(),
        )?)
    }
}
