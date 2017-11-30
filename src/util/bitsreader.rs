//! Implementation of the BitsReader we will use for our purposes.
//!
//! Maybe in the future the content of this module can be pushed upstream, for
//! this reason the functionality in here should be kept as general as possible.

// TODO: This should be moved to its own crate and then ReadBytesExt etc should
//       not be used in my code anymore, as this provides a cleaner way around
// that functionality. (Always making it explicit whether it's about
// reading
//       bits or bytes.)

use byteorder::{ByteOrder, ReadBytesExt};
pub use byteorder::{BigEndian, LittleEndian};
use bitreader::BitReader;
use bitreader::BitReaderError;
use std::io::Read as IoRead;
use std::io::Error as IoError;

// TODO: This should be improved in the future,
// especially the BitReader variant should fall away.
#[derive(Debug)]
pub enum ReadError {
    UnexpectedEnd,
    IoError(IoError),
    BitReader(BitReaderError),
}

impl From<IoError> for ReadError {
    fn from(e: IoError) -> ReadError {
        ReadError::IoError(e)
    }
}

impl From<BitReaderError> for ReadError {
    fn from(e: BitReaderError) -> Self {
        ReadError::BitReader(e)
    }
}

impl ::std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        use std::error::Error;
        write!(f, "{}", self.description())
    }
}

impl ::std::error::Error for ReadError {
    fn description(&self) -> &str {
        match *self {
            ReadError::UnexpectedEnd => "unexpected eof",
            ReadError::IoError(_) => "io error",
            ReadError::BitReader(_) => "bit reader error",
        }
    }
}

/// This trait defines a reader which can read values **byte by byte**.
pub trait BytesReader {
    /// Read the next byte as u8.
    fn read_bytes_u8(&mut self) -> Result<u8, ReadError>;
    /// Read the next 2 bytes as u16.
    fn read_bytes_u16<B: ByteOrder>(&mut self) -> Result<u16, ReadError>;
    /// Read the next 4 bytes as u32.
    fn read_bytes_u32<B: ByteOrder>(&mut self) -> Result<u32, ReadError>;
    /// Read the next 8 bytes as u64.
    fn read_bytes_u64<B: ByteOrder>(&mut self) -> Result<u64, ReadError>;

    /// Read the next 4 bytes as f32.
    fn read_bytes_f32<B: ByteOrder>(&mut self) -> Result<f32, ReadError>;
    /// Read the next 8 bytes as f64.
    fn read_bytes_f64<B: ByteOrder>(&mut self) -> Result<f64, ReadError>;

    /// Read the next byte as bool.
    fn read_bytes_bool(&mut self) -> Result<bool, ReadError> {
        self.read_bytes_u8().map(|num| num != 0)
    }
}

/// This trait defines a reader which can read values **bit by bit**.
pub trait BitsReader: BytesReader {
    /// Read a bool from the next num_bits.
    fn read_bits_bool(&mut self, num_bits: u8) -> Result<bool, ReadError>;
    /// Read the next bit as a bool.
    fn read_bit_bool(&mut self) -> Result<bool, ReadError> {
        self.read_bits_bool(1)
    }

    /// Read a u8 from the next num_bits bits of the reader.
    fn read_bits_u8<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u8, ReadError>;
    /// Read a u16 from the next num_bits bits of the reader.
    fn read_bits_u16<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u16, ReadError>;
    /// Read a u32 from the next num_bits bits of the reader.
    fn read_bits_u32<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u32, ReadError>;
    /// Read a u64 from the next num_bits bits of the reader.
    fn read_bits_u64<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u64, ReadError>;
}

pub struct BufBitsReader<'d> {
    // TODO: Drop the dependency on an external crate and write an implementation which is the best
    // for our specific needs. For now I'm leaving it here since there are no unit tests yet, which
    // we'll want as soon as we make the shift to our more optimized version.
    reader: BitReader<'d>,
}

impl<'d> BufBitsReader<'d> {
    #[inline]
    fn read_bytes_partial<'r>(&mut self, n_bits: u8) -> Result<[u8; 8], ReadError> {
        debug_assert!(n_bits <= 64);

        let mut result = [0u8; 8];
        let n_full_bytes: u8 = n_bits / 8;
        let n_remaining_bits: u8 = n_bits - n_full_bytes * 8;

        for i in 0..n_full_bytes {
            result[i as usize] = self.reader.read_u8(8)?;
        }
        if n_remaining_bits > 0 {
            result[n_full_bytes as usize] = self.reader.read_u8(n_remaining_bits)?;
        }
        Ok(result)
    }

    pub fn new(data: &'d [u8]) -> Self {
        BufBitsReader {
            reader: BitReader::new(data),
        }
    }
}

impl<'d> BytesReader for BufBitsReader<'d> {
    #[inline]
    fn read_bytes_u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.reader.read_u8(8)?)
    }

    #[inline]
    fn read_bytes_u16<B: ByteOrder>(&mut self) -> Result<u16, ReadError> {
        Ok(B::read_u16(
            &[self.reader.read_u8(8)?, self.reader.read_u8(8)?],
        ))
    }

    #[inline]
    fn read_bytes_u32<B: ByteOrder>(&mut self) -> Result<u32, ReadError> {
        Ok(B::read_u32(&[
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?,
        ]))
    }

    #[inline]
    fn read_bytes_u64<B: ByteOrder>(&mut self) -> Result<u64, ReadError> {
        Ok(B::read_u64(&[
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?, self.reader.read_u8(8)?,
        ]))
    }

    #[inline]
    fn read_bytes_f32<B: ByteOrder>(&mut self) -> Result<f32, ReadError> {
        Ok(B::read_f32(&[
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?,
        ]))
    }

    #[inline]
    fn read_bytes_f64<B: ByteOrder>(&mut self) -> Result<f64, ReadError> {
        Ok(B::read_f64(&[
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?, self.reader.read_u8(8)?, self.reader.read_u8(8)?,
            self.reader.read_u8(8)?, self.reader.read_u8(8)?,
        ]))
    }
}

impl<'d> BitsReader for BufBitsReader<'d> {
    #[inline]
    fn read_bits_bool(&mut self, num_bits: u8) -> Result<bool, ReadError> {
        let num = self.read_bits_u32::<LittleEndian>(num_bits)?;
        match num {
            0 => Ok(false),
            _ => Ok(true),
        }
    }

    #[inline]
    fn read_bit_bool(&mut self) -> Result<bool, ReadError> {
        Ok(self.reader.read_bool()?)
    }

    /// Read a u8 from the next num_bits bits of the reader.
    #[inline]
    fn read_bits_u8<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u8, ReadError> {
        Ok(self.read_bytes_partial(num_bits)?[0])
    }

    /// Read a u16 from the next num_bits bits of the reader.
    #[inline]
    fn read_bits_u16<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u16, ReadError> {
        Ok(B::read_u16(&self.read_bytes_partial(num_bits)?[0..4]))
    }

    /// Read a u32 from the next num_bits bits of the reader.
    #[inline]
    fn read_bits_u32<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u32, ReadError> {
        Ok(B::read_u32(&self.read_bytes_partial(num_bits)?[0..4]))
    }

    /// Read a u64 from the next num_bits bits of the reader.
    #[inline]
    fn read_bits_u64<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u64, ReadError> {
        Ok(B::read_u64(&self.read_bytes_partial(num_bits)?[0..8]))
    }
}

impl<T> BytesReader for T
where
    T: IoRead,
{
    #[inline]
    fn read_bytes_u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.read_u8()?)
    }

    #[inline]
    fn read_bytes_u16<B: ByteOrder>(&mut self) -> Result<u16, ReadError> {
        Ok(self.read_u16::<B>()?)
    }

    #[inline]
    fn read_bytes_u32<B: ByteOrder>(&mut self) -> Result<u32, ReadError> {
        Ok(self.read_u32::<B>()?)
    }

    #[inline]
    fn read_bytes_u64<B: ByteOrder>(&mut self) -> Result<u64, ReadError> {
        Ok(self.read_u64::<B>()?)
    }

    #[inline]
    fn read_bytes_f32<B: ByteOrder>(&mut self) -> Result<f32, ReadError> {
        Ok(self.read_f32::<B>()?)
    }

    #[inline]
    fn read_bytes_f64<B: ByteOrder>(&mut self) -> Result<f64, ReadError> {
        Ok(self.read_f64::<B>()?)
    }
}
