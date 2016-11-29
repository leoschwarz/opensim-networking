use {Ip4Addr, Ip4Port, Uuid};

use messages::{UseCircuitCode, UseCircuitCode_CircuitCode};
use login::LoginResponse;


// TODO: Generate these implementations automatically later.
use messages::Message;
use std::io::Write;
use byteorder::{LittleEndian, BigEndian, WriteBytesExt};
use messages::WriteMessageResult;
/*
impl Message for UseCircuitCode {
    fn write_to<W: Write>(&self, buffer: &mut W) -> WriteMessageResult {
        // Write message number.
        // 1, 2, or 4 bytes long for high, medium, low or fixed
        try!(buffer.write(&[0xff, 0xff, 0x00, 0x01]));

        // Write message body.
        try!(buffer.write_u32::<LittleEndian>(self.circuit_code.code));
        try!(buffer.write(self.circuit_code.session_id.as_bytes()));
        try!(buffer.write(self.circuit_code.id.as_bytes()));
        Ok(())
    }
}*/


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
    sequence_number: u32
}

impl<M: Message> Packet<M> {
    fn new(m: M, seq_number: u32) -> Packet<M> {
        Packet {
            message: m,
            flags: PacketFlags::empty(),
            sequence_number: seq_number
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
pub struct Circuit {
    ip: Ip4Addr,
    port: Ip4Port,
    sequence_number: u32
}

impl Circuit {
    fn initiate(login_res: &LoginResponse) {
        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone()
            }
        };

        // Send the packet and wait for ack.
        let mut packet = Packet::new(msg, 1);
        packet.enable_flags(PACKET_RELIABLE);

        // TODO send and wait
    }

    fn send_message<M: Message>(&self, msg: &M) {
        // TODO: which socket does this get written to?!
    }

}

