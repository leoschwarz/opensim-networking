use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode, WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE};

use tokio_core::reactor::Core;
use tokio_core::net::{UdpCodec, UdpSocket};
use std::net::{SocketAddr, SocketAddrV4};
use std::collections::VecDeque;
use time::Timespec;

/// We only consider viewer <-> simulator circuits.
/// TODO: Once there is IPv6 support in the opensim server, implement support for both v4 and v6.
///       This was not done yet since we would have to abort the code if we got IPv6 addresses
///       anywhere and would make the API less realiable.
pub struct Circuit {
    core: Core,

    /// Local UDP socket used for communication.
    socket: UdpSocket,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,

    /// Maybe A 24 bit number (TODO: change type here???) created at the connection of any circuit.
    /// This number is stored in the packet header and incremented whenever a packet is sent
    /// from one end of the circuit to the other.
    ///
    /// For each connection and each direction messages are numbered with unique numbers in a
    /// sequential fashion. This number is incremented after sending each message.
    sequence_number: u32,
}

#[derive(Debug)]
pub enum CircuitInitiationError {
    IO(::std::io::Error),
}

impl From<::std::io::Error> for CircuitInitiationError {
    fn from(err: ::std::io::Error) -> Self {
        CircuitInitiationError::IO(err)
    }
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse,
                    local_socket_addr: SocketAddr)
                -> Result<Circuit, CircuitInitiationError> {

        // Create the eventloop.
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        // Create the socket.
        let socket = UdpSocket::bind(&local_socket_addr, &handle)?;

        // Create the circuit instance.
        let circuit = Circuit {
            core: core,
            socket: socket,
            sim_address: SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip,
                                                         login_res.sim_port)),
            sequence_number: 1
        };

        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone(),
            },
        };
        let mut packet = Packet::new(msg, 1);
        packet.enable_flags(PACKET_RELIABLE);
        circuit.send_packet(packet);

        // TODO: Wait for an ack.

        // Finished.
        Ok(circuit)
    }

    fn send_packet(&self, packet: Packet) {
        let mut buf = Vec::new();
        packet.write_to(&mut buf);
        self.socket.send_to(&buf, &self.sim_address);
    }

    // TODO: Move this method to the right location.
    fn send_message<M: Into<MessageInstance>>(&self, msg: M) {
        let packet = Packet::new(msg, self.sequence_number);
        self.send_packet(packet);
    }
}

struct OpensimCodec {
    circuit: Circuit
}

impl UdpCodec for OpensimCodec {
    type In = Packet;
    type Out = Packet;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> Result<Self::In, ::std::io::Error> {
        Packet::read(buf)
    }

    fn encode(&mut self, packet: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {
        packet.write_to(buf);
        self.circuit.sim_address
    }
}

