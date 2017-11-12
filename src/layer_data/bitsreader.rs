//! Implementation of the BitsReader we will use for our purposes.
//!
//! Maybe in the future the content of this module can be pushed upstream, for this reason the
//! functionality in here should be kept as general as possible.

use byteorder::{ByteOrder, BigEndian, LittleEndian};
use bitreader::BitReader;
pub use bitreader::BitReaderError as BitsReaderError;

pub struct BitsReader<'d> {
    reader: BitReader<'d>,
}

impl<'d> BitsReader<'d> {
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

    pub fn read_part_u8<P: Padding>(&mut self, num_bits: u8) -> Result<u8, BitsReaderError> {
        assert!(num_bits <= 8);
        Ok(P::pad_u8(self.reader.read_u8(num_bits)?, 8 - num_bits))
    }

    /// Read the specified number of bits, then apply padding, then reorder if needed.
    #[inline]
    pub fn read_part_u32<B: ByteOrder, P: Padding>(
        &mut self,
        num_bits: u8,
    ) -> Result<u32, BitsReaderError> {
        // TODO
        if num_bits <= 8 {
            Ok(self.reader.read_u32(num_bits)?)
        } else if num_bits <= 16 {
            Ok(B::read_u32(
                &[
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(num_bits - 8)?,
                    0,
                    0,
                ],
            ))
        } else if num_bits <= 24 {
            Ok(B::read_u32(
                &[
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(num_bits - 16)?,
                    0,
                ],
            ))
        } else if num_bits <= 32 {
            Ok(B::read_u32(
                &[
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(8)?,
                    self.reader.read_u8(num_bits - 24)?,
                ],
            ))
        } else {
            panic!("invalid num_bits: {}", num_bits);
        }
    }
}

/// Describes the padding of a value. Padding is applied before correcting endianness.
///
/// Generally you will only care whether to supply `PadOnLeft` or `PadOnRight`.
pub trait Padding {
    /// Pad the value `val` with `num_zeros` zeros.
    fn pad_u8(val: u8, num_zeros: u8) -> u8;

    /// Pad the value `val` with `num_zeros` zeros.
    fn pad_u16(val: u16, num_zeros: u8) -> u16;

    /// Pad the value `val` with `num_zeros` zeros.
    fn pad_u32(val: u32, num_zeros: u8) -> u32;

    /// Pad the value `val` with `num_zeros` zeros.
    fn pad_u64(val: u64, num_zeros: u8) -> u64;
}

/// Add padding on the left side of the number, i.e.
/// pad `11` to `0011`.
pub enum PadOnLeft {}

impl Padding for PadOnLeft {
    fn pad_u8(val: u8, _: u8) -> u8 {
        val
    }
    fn pad_u16(val: u16, _: u8) -> u16 {
        val
    }
    fn pad_u32(val: u32, _: u8) -> u32 {
        val
    }
    fn pad_u64(val: u64, _: u8) -> u64 {
        val
    }
}

/// Add padding on the right side of the number, i.e.
/// pad `11` to `1100`.
pub enum PadOnRight {}

impl Padding for PadOnRight {
    fn pad_u8(val: u8, num_zeros: u8) -> u8 {
        val << num_zeros
    }
    fn pad_u16(val: u16, num_zeros: u8) -> u16 {
        val << num_zeros
    }
    fn pad_u32(val: u32, num_zeros: u8) -> u32 {
        val << num_zeros
    }
    fn pad_u64(val: u64, num_zeros: u8) -> u64 {
        val << num_zeros
    }
}
