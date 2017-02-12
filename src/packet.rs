use byteorder::{LittleEndian, BigEndian, WriteBytesExt, ReadBytesExt};
use std::io::{Read, Write};
use tokio_core::net::UdpSocket;

use messages::{MessageInstance, read_message};

bitflags! {
    pub flags PacketFlags: u8 {
        /// There are acks appended to the packet.
        const PACKET_APPENDED_ACKS = 0b0001_0000,
        /// Resending a packet that was sent (with PACKET_RELIABLE) but not ackd.
        const PACKET_RESENT        = 0b0010_0000,
        /// Ack for packet is requested.
        const PACKET_RELIABLE      = 0b0100_0000,
        /// If enabled:
        /// Multiple consecutive zero bytes (but also single zero bytes) are coded into one zero
        /// byte and a following byte specifying the number of zero bytes.
        const PACKET_ZEROCODED     = 0b1000_0000,
    }
}

pub type SequenceNumber = u32;

/// One packet either sent to or received from a sim.
pub struct Packet {
    /// The contained message.
    message: MessageInstance,

    /// Flags of the packet.
    flags: PacketFlags,

    /// The sequence number of the packet. This number is unique for each packet in each
    /// circuit and each direction. It is incremented one after one for each message.
    sequence_number: SequenceNumber,

    /// Packet acknowledgments appended to this packet. Can contain any number (including zero)
    /// of elements.
    appended_acks: Vec<SequenceNumber>,
}

impl Packet {
    pub fn new<M: Into<MessageInstance>>(m: M, seq_number: u32) -> Packet {
        Packet {
            message: m.into(),
            flags: PacketFlags::empty(),
            sequence_number: seq_number,
            appended_acks: Vec::new(),
        }
    }

    /// Write the packet (including both body and header) to a buffer
    /// in its current form.
    ///
    /// # Protocol documentation
    /// * http://lib.openmetaverse.co/wiki/Protocol_(network)
    /// * http://wiki.secondlife.com/wiki/Packet_Layout
    pub fn write_to<W: Write>(&self, buffer: &mut W) -> Result<(), ::std::io::Error> {
        // Assert: PACKET_APPENDED_ACKS flag set <-> self.appended_acks is empty.
        debug_assert!(!(self.flags.contains(PACKET_APPENDED_ACKS) ^ self.appended_acks.is_empty()));
        // TODO: Zero coded writing not implemented yet.
        assert!(!self.flags.contains(PACKET_ZEROCODED));

        buffer.write_u8(self.flags.bits())?;
        buffer.write_u32::<BigEndian>(self.sequence_number)?;
        buffer.write(&[0])?;
        self.message.write_to(buffer)?;
        for ack in &self.appended_acks {
            buffer.write_u32::<BigEndian>(*ack)?;
        };
        if !self.appended_acks.is_empty() {
            buffer.write_u8(self.appended_acks.len() as u8)?;
        }
        Ok(())
    }

    pub fn read<'a>(buf: &'a [u8]) -> Result<Packet, ::std::io::Error> {
        let mut reader = PacketReader::new(buf);

        let flags = PacketFlags::from_bits(reader.read_u8()?).unwrap();
        let sequence_num = reader.read_u32::<BigEndian>()?;

        // Skip extra header if present, since we don't expect it.
        let extra_bytes = reader.read_u8()? as usize;
        if extra_bytes > 0 {
            reader.skip_bytes(extra_bytes);
        }

        // Read message.
        let message_num = reader.read_message_number()?;
        if flags.contains(PACKET_ZEROCODED) {
            reader.zerocoding_enabled = true;
        }
        let message = read_message(&mut reader, message_num)?;

        // Read appended ACKs if there are supposed to be any.
        let mut acks = Vec::new();
        if flags.contains(PACKET_APPENDED_ACKS) {
            let n_acks = reader.peek_last_byte() as usize;
            acks.reserve(n_acks);
            for _ in 0..n_acks {
                acks.push(reader.read_u32::<BigEndian>()?);
            }
        }

        Ok(Packet {
            message: message,
            flags: flags,
            sequence_number: sequence_num,
            appended_acks: acks,
        })
    }

    /// Enable the provided flags.
    pub fn enable_flags(&mut self, flags: PacketFlags) {
        self.flags.insert(flags);
    }

    /// Disable the provided flags.
    pub fn disable_flags(&mut self, flags: PacketFlags) {
        self.flags.remove(flags);
    }

    /// Set the reliable flack for a packet.
    pub fn set_reliable(&mut self, value: bool) {
        if value {
            self.enable_flags(PACKET_RELIABLE);
        } else {
            self.disable_flags(PACKET_RELIABLE);
        }
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

    /// Just return the content of the very last byte of the message,
    /// without changing the reader's state in any way.
    fn peek_last_byte(&self) -> u8 {
        self.buf[self.buf.len() - 1]
    }

    #[inline]
    fn has_index(&self, index: usize) -> bool {
        (self.buf.len() - index) > 0
    }

    /// Skips the provided number of bytes.
    /// If such a number is not available an error will be returned.
    fn skip_bytes(&mut self, bytes: usize) -> Result<(), ::std::io::Error> {
        let new_pointer = self.pointer + bytes;
        if self.buf.len() >= new_pointer {
            self.pointer = new_pointer;
            Ok(())
        } else {
            Err(::std::io::Error::new(::std::io::ErrorKind::UnexpectedEof, "Tried skipping behind EOF in PacketReader."))
        }
    }

    fn read_message_number(&mut self) -> Result<u32, ::std::io::Error> {
        let b1 = self.read_u8()?;
        let bytes = if b1 != 0xff {
            // High frequency messages.
            [b1, 0, 0, 0]
        } else {
            let b2 = self.read_u8()?;
            if b2 != 0xff {
                // Medium frequency messages.
                [b1, b2, 0, 0]
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

#[test]
fn reader_skip() {
    let data: [u8; 6] = [0, 1, 2, 3, 4, 5];
    let mut reader = PacketReader::new(&data);
    assert!(reader.skip_bytes(2).is_ok());
    assert_eq!(reader.read_u8().unwrap(), 2);
    assert!(reader.skip_bytes(3).is_ok());
    assert!(reader.skip_bytes(1).is_err());
}
