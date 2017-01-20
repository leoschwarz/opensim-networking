use byteorder::{LittleEndian, BigEndian, WriteBytesExt, ReadBytesExt};
use std::io::{Read, Write};
use tokio_core::net::UdpSocket;

use messages::{MessageInstance, read_message};

bitflags! {
    pub flags PacketFlags: u8 {
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

/// One packet either sent to or received from a sim.
pub struct Packet {
    message: MessageInstance,
    flags: PacketFlags,
    sequence_number: u32,
    appended_acks: Option<Vec<u32>>,
}

impl Packet {
    pub fn new<M: Into<MessageInstance>>(m: M, seq_number: u32) -> Packet {
        Packet {
            message: m.into(),
            flags: PacketFlags::empty(),
            sequence_number: seq_number,
            appended_acks: None,
        }
    }

    /// Write the packet (including both body and header) to a buffer
    /// in its current form.
    ///
    /// # Protocol documentation
    /// * http://lib.openmetaverse.co/wiki/Protocol_(network)
    /// * http://wiki.secondlife.com/wiki/Packet_Layout
    ///
    /// TODO: Implement zero encoded writing.
    fn write_to<W: Write>(&self, buffer: &mut W) {
        buffer.write_u8(self.flags.bits());
        buffer.write_u32::<BigEndian>(self.sequence_number);
        buffer.write(&[0]);
        self.message.write_to(buffer);
        match self.appended_acks {
            Some(ref acks) => {
                for ack in acks {
                    buffer.write_u32::<BigEndian>(*ack);
                }
            }
            None => {}
        };
    }

    fn read<'a>(buf: &'a [u8]) -> Result<Packet, ::std::io::Error> {
        let mut reader = PacketReader::new(buf);

        let flags = PacketFlags::from_bits(reader.read_u8()?).unwrap();
        let sequence_num = reader.read_u32::<BigEndian>()?;

        // TODO: Skip extra header if there is any.
        reader.read_u8()?;

        // Read message.
        let message_num = reader.read_message_number()?;
        if flags.contains(PACKET_ZEROCODED) {
            reader.zerocoding_enabled = true;
        }

        let message = read_message(&mut reader, message_num)?;

        // Read appended ACKs if there are supposed to be any.
        if flags.contains(PACKET_APPENDED_ACKS) {
            // TODO
        }

        Ok(Packet {
            message: message,
            flags: flags,
            sequence_number: sequence_num,
            appended_acks: None
        })
    }

    // TODO: Optimize this in the future.
    //       This function will potentially get a lot of use.
    // fn send(&self, socket: &UdpSocket) {
    // let mut buf = Vec::new();
    // self.write_to(&mut buf);
    // socket.send(&buf[..]);
    // }
    //

    /// Enable the provided flags.
    pub fn enable_flags(&mut self, flags: PacketFlags) {
        self.flags.insert(flags);
    }

    /// Disable the provided flags.
    pub fn disable_flags(&mut self, flags: PacketFlags) {
        self.flags.remove(flags);
    }
}

/// Used internally to read the content of packages.
/// Provides transparent reading of zerocoded content.
/// Memory: O(1)
struct PacketReader<'a> {
    buf: &'a [u8],
    pointer: usize,
    zerocoding_enabled: bool,
    /// It is possible that with zerocoding enabled a pair of zero byte and count is encountered,
    /// but not all such bytes can be read into the destination buffer.
    /// In that case this variable will be set to the number of zerobytes which are yet pending
    /// to be read on the next invocation of read.
    pending_zerobytes: u8,
}

impl<'a> PacketReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        PacketReader {
            buf: buf,
            pointer: 0,
            zerocoding_enabled: false,
            pending_zerobytes: 0,
        }
    }

    #[inline]
    fn has_index(&self, index: usize) -> bool {
        (self.buf.len() - index) > 0
    }
    
    fn read_message_number(&mut self) -> Result<u32, ::std::io::Error> {
        let b1 = self.read_u8()?;
        let bytes = if b1 != 0xff {
            // High frequency messages.
            [b1,0,0,0]
        } else {
            let b2 = self.read_u8()?;
            if b2 != 0xff {
                // Medium frequency messages.
                [b1, b2,0,0]
            } else {
                // Low and fixed frequency messages.
                let b3 = self.read_u8()?;
                let b4 = self.read_u8()?;
                [b1, b2, b3, b4]
            }
        };

        // Convert into u32
        (&mut bytes.as_ref()).read_u32::<BigEndian>()
    }
}

impl<'a> Read for PacketReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ::std::io::Error> {
        use ::std::cmp::min;

        if self.zerocoding_enabled {
            let mut read_bytes: usize = 0;

            // Try to read as many bytes as requested.
            while buf.len() - read_bytes > 0 {
                if self.pending_zerobytes > 0 {
                    buf[read_bytes] = 0;
                    self.pending_zerobytes -= 1;
                    read_bytes += 1;
                } else if !self.has_index(self.pointer) {
                    // The reader has read through all of the buffer.
                    return Ok(read_bytes);
                } else {
                    if self.buf[self.pointer] == 0 {
                        if self.has_index(self.pointer + 1) {
                            self.pending_zerobytes = self.buf[self.pointer + 1];
                            self.pointer += 2;
                        } else {
                            return Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData,
                                                             "Zerocoding enabled, but found a \
                                                              zero byte at EOF without \
                                                              repetition quantity."));
                        }
                    } else {
                        buf[read_bytes] = self.buf[self.pointer];
                        read_bytes += 1;
                        self.pointer += 1;
                    }
                }
            }

            Ok(read_bytes)
        } else {
            // Determine the number of bytes to be read.
            let requested = buf.len();
            let available = self.buf.len() - self.pointer;
            let bytes = min(requested, available);

            // Now read that number of bytes into the buffer.
            buf[..bytes].copy_from_slice(&self.buf[self.pointer..self.pointer + bytes]);
            self.pointer += bytes;

            // Return success status.
            Ok(bytes)
        }
    }

}

#[test]
fn read_simple() {
    let data: [u8; 6] = [2, 4, 6, 8, 10, 12];
    let mut reader = PacketReader::new(&data);

    let mut buffer: [u8; 6] = [0, 0, 0, 0, 0, 0];
    let bytes = reader.read(&mut buffer).unwrap();
    assert_eq!(bytes, 6);
    assert_eq!(buffer, [2, 4, 6, 8, 10, 12]);
}

#[test]
fn read_in_chunks() {
    let data: [u8; 5] = [2, 4, 6, 8, 10];
    let mut reader = PacketReader::new(&data);

    let mut buffer: [u8; 2] = [0, 0];
    let b1 = reader.read(&mut buffer).unwrap();
    assert_eq!(b1, 2);
    assert_eq!(buffer, [2, 4]);
    let b2 = reader.read(&mut buffer).unwrap();
    assert_eq!(b2, 2);
    assert_eq!(buffer, [6, 8]);
    let b3 = reader.read(&mut buffer).unwrap();
    assert_eq!(b3, 1);
    assert_eq!(buffer, [10, 8]);
}

#[test]
fn read_zerocoded_easy() {
    let data: [u8; 2] = [0, 5];
    let mut reader = PacketReader::new(&data);
    reader.zerocoding_enabled = true;

    let mut buffer: [u8; 10] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let bytes = reader.read(&mut buffer).unwrap();
    assert_eq!(bytes, 5);
    assert_eq!(buffer, [0, 0, 0, 0, 0, 1, 1, 1, 1, 1]);
}

#[test]
fn read_zerocoded_hard() {
    let data: [u8; 2] = [0, 5];
    let mut reader = PacketReader::new(&data);
    reader.zerocoding_enabled = true;

    let mut buffer: [u8; 3] = [1, 1, 1];
    let b1 = reader.read(&mut buffer).unwrap();
    assert_eq!(b1, 3);
    assert_eq!(buffer, [0, 0, 0]);

    buffer = [1, 1, 1];
    let b2 = reader.read(&mut buffer).unwrap();
    assert_eq!(b2, 2);
    assert_eq!(buffer, [0, 0, 1]);
}
