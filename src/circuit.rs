use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode, WriteMessageResult};
use login::LoginResponse;

use byteorder::{LittleEndian, BigEndian, WriteBytesExt};
use std::io::Write;
use tokio_core::reactor::Core;
use tokio_core::net::UdpSocket;
use std::net::{SocketAddr, SocketAddrV4};

bitflags! {
    pub flags PacketFlags: u16 {
        /// There are acks appended to the packet. TODO: implement
        const PACKET_APPENDED_ACKS = 0b0001_0000,
        /// Resending a packet that was sent (with PACKET_RELIABLE) but not ackd.
        const PACKET_RESENT        = 0b0010_0000,
        /// Ack for packet is requested. TODO: implement
        const PACKET_RELIABLE      = 0b0100_0000,
        /// If enabled:
        /// Multiple consecutive zero bytes (but also single zero bytes) are coded into one zero
        /// byte and a following byte specifying the number of zero bytes.
        /// TODO: implement
        const PACKET_ZEROCODED     = 0b1000_0000,
    }
}

// TODO: Figure out if packages with multiple messages in the body are also possible.
pub struct Packet {
    message: MessageInstance,
    flags: PacketFlags,
    sequence_number: u32,
}

pub enum ReadPacketError {

}

impl Packet {
    fn new<M: Into<MessageInstance>>(m: M, seq_number: u32) -> Packet {
        Packet {
            message: m.into(),
            flags: PacketFlags::empty(),
            sequence_number: seq_number,
        }
    }

    /// Write the packet (including both body and header) to a buffer
    /// in its current form.
    ///
    /// # Protocol documentation
    /// * http://lib.openmetaverse.co/wiki/Protocol_(network)
    /// * http://wiki.secondlife.com/wiki/Packet_Layout
    fn write_to<W: Write>(&self, buffer: &mut W) {
        // Flags
        buffer.write_u16::<LittleEndian>(self.flags.bits());

        // Sequence number.
        buffer.write_u32::<BigEndian>(self.sequence_number);

        // No extra header information specified.
        buffer.write(&[0]);

        // Message body
        self.message.write_to(buffer);
    }

    /* TODO
    fn read_from<R: Read>(buffer: &mut R) -> Result<Packet, ReadPacketError> {
        // Read packet header.
        let flags = try!(buffer.read_u16::<LittleEndian>());
        let seq_num = try!(buffer.read_u32::<BigEndian>());
        try!(buffer.read_u8());

    }*/

    /// TODO: Optimize this in the future.
    ///       This function will potentially get a lot of use.
    fn send(&self, socket: &UdpSocket) {
        let mut buf = Vec::new();
        self.write_to(&mut buf);
        socket.send(&buf[..]);
    }

    /// Enable the provided flags.
    fn enable_flags(&mut self, flags: PacketFlags) {
        self.flags.insert(flags);
    }

    /// Disable the provided flags.
    fn disable_flags(&mut self, flags: PacketFlags) {
        self.flags.remove(flags);
    }
}

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

    /// A 24 bit number (TODO: change type here???) created at the connection of any circuit.
    /// This number is stored in the packet header and incremented whenever a packet is sent
    /// from one end of the circuit to the other.
    ///
    /// TODO: Figure out how important this sequence number is. If we end up having two
    /// packets with the same sequence number, do we need to discard some packages?
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
        let socket = try!(UdpSocket::bind(&local_socket_addr, &handle));

        // Create the circuit instance.
        let circuit = Circuit {
            core: core,
            socket: socket,
            sim_address: SocketAddr::V4(SocketAddr4::new(login_res.sim_ip,
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
        circuit.send_packet(remote_socket);

        // TODO: Wait for an ack.

        // Finished.
        Ok(circuit)
    }

    fn send_packet(&self, packet: Packet) {

    }

    fn send_message<M: Into<MessageInstance>>(&self, msg: M) {
        let packet = Packet::new(msg, self.sequence_number);
        
    }
}
