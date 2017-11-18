//! Implementation of the BitsReader we will use for our purposes.
//!
//! Maybe in the future the content of this module can be pushed upstream, for this reason the
//! functionality in here should be kept as general as possible.

use byteorder::ByteOrder;
use bitreader::BitReader;
pub use bitreader::BitReaderError as BitsReaderError;

pub struct BitsReader<'d> {
    reader: BitReader<'d>,
}

impl<'d> BitsReader<'d> {
    #[inline]
    fn read_bytes_partial<'r>(&mut self, n_bits: u8) -> Result<[u8; 8], BitsReaderError> {
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
        BitsReader { reader: BitReader::new(data) }
    }

    #[inline]
    pub fn read_full_u8(&mut self) -> Result<u8, BitsReaderError> {
        Ok(self.reader.read_u8(8)?)
    }

    #[inline]
    pub fn read_full_u16<B: ByteOrder>(&mut self) -> Result<u16, BitsReaderError> {
        Ok(B::read_u16(
            &[self.reader.read_u8(8)?, self.reader.read_u8(8)?],
        ))
    }

    #[inline]
    pub fn read_full_u32<B: ByteOrder>(&mut self) -> Result<u32, BitsReaderError> {
        Ok(B::read_u32(
            &[
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
            ],
        ))
    }

    #[inline]
    pub fn read_full_f32<B: ByteOrder>(&mut self) -> Result<f32, BitsReaderError> {
        Ok(B::read_f32(
            &[
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
            ],
        ))
    }

    #[inline]
    pub fn read_full_u64<B: ByteOrder>(&mut self) -> Result<u64, BitsReaderError> {
        Ok(B::read_u64(
            &[
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
                self.reader.read_u8(8)?,
            ],
        ))
    }

    #[inline]
    pub fn read_bool(&mut self) -> Result<bool, BitsReaderError> {
        self.reader.read_bool()
    }

    /// Read a u8 from the next num_bits bits of the reader.
    #[inline]
    pub fn read_part_u8<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u8, BitsReaderError> {
        Ok(self.read_bytes_partial(num_bits)?[0])
    }

    /// Read a u16 from the next num_bits bits of the reader.
    #[inline]
    pub fn read_part_u16<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u16, BitsReaderError> {
        Ok(B::read_u16(&self.read_bytes_partial(num_bits)?[0..4]))
    }

    /// Read a u32 from the next num_bits bits of the reader.
    #[inline]
    pub fn read_part_u32<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u32, BitsReaderError> {
        Ok(B::read_u32(&self.read_bytes_partial(num_bits)?[0..4]))
    }

    /// Read a u64 from the next num_bits bits of the reader.
    #[inline]
    pub fn read_part_u64<B: ByteOrder>(&mut self, num_bits: u8) -> Result<u64, BitsReaderError> {
        Ok(B::read_u64(&self.read_bytes_partial(num_bits)?[0..8]))
    }
}
