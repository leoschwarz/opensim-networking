use {Ip4Addr, IpPort, Uuid};

use messages::{Message, UseCircuitCode, UseCircuitCode_CircuitCode, WriteMessageResult};
use login::LoginResponse;

use byteorder::{LittleEndian, BigEndian, WriteBytesExt};
use std::io::Write;
use std::net::UdpSocket;

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
pub struct Packet<M: Message> {
    message: M,
    flags: PacketFlags,
    sequence_number: u32,
}

impl<M: Message> Packet<M> {
    fn new(m: M, seq_number: u32) -> Packet<M> {
        Packet {
            message: m,
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
    sim_ip: Ip4Addr,
    sim_port: IpPort,
    sim_socket: UdpSocket,
    //local_ip: Ip4Addr,
    //local_port: IpPort,
    sequence_number: u32,
}

pub enum CircuitInitiationError {
    IO(::std::io::Error),
}

impl From<::std::io::Error> for CircuitInitiationError {
    fn from(err: ::std::io::Error) -> Self {
        CircuitInitiationError::IO(err)
    }
}

impl Circuit {
    fn initiate(login_res: LoginResponse,
          /*      local_ip: Ip4Addr,
                local_port: IpPort*/)
                -> Result<Circuit, CircuitInitiationError> {
        // Open the sockets.
        //let mut local_socket = try!(UdpSocket::bind((local_ip, local_port)));
        let mut remote_socket = try!(UdpSocket::bind((login_res.sim_ip, login_res.sim_port)));

        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone(),
            },
        };

        // Send the packet and wait for ack.
        let mut packet = Packet::new(msg, 1);
        packet.enable_flags(PACKET_RELIABLE);
        packet.send(&remote_socket);

        Ok(Circuit {
            sim_ip: login_res.sim_ip,
            sim_port: login_res.sim_port,
            sim_socket: remote_socket,
            sequence_number: 1
        })
    }

    fn send_message<M: Message>(&self, msg: &M) {
        // TODO
    }
}
