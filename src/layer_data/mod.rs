// TODO: This should probably moved to its own crate at a later time. (Maybe some decoding
// facilities could be combined together conveniently.)
//
// TODO: Cleanup the module, move everything out of the mod.rs file which doesn't have to reside in
// here.

use messages::all::LayerData;

use byteorder::{ByteOrder, LittleEndian};
use bitreader::BitReaderError;

mod idct;
mod reader;

use self::idct::{PatchTables, PatchSize, PatchMatrix};
use self::reader::{BitsReader, PadOnLeft};

const END_OF_PATCH: u8 = 97u8;

lazy_static! {
    static ref TABLES_NORMAL: PatchTables = PatchTables::compute::<idct::NormalPatch>();
    static ref TABLES_LARGE: PatchTables = PatchTables::compute::<idct::LargePatch>();
}


#[derive(Debug, ErrorChain)]
#[error_chain(error = "ExtractSurfaceError")]
#[error_chain(result = "")]
pub enum ExtractSurfaceErrorKind {
    #[error_chain(foreign)]
    BitReader(BitReaderError),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unknown layer type""#)]
    #[error_chain(display = r#"|code| write!(f, "unknown layer type: {}", code)"#)]
    UnknownLayerType(u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerKind {
    Land,
    Wind,
    Cloud,
    Water,
    AuroraLand,
    AuroraWind,
    AuroraCloud,
    AuroraWater,
}

impl LayerKind {
    fn from_code(c: u8) -> Result<Self, ExtractSurfaceError> {
        match c {
            b'L' => Ok(LayerKind::Land),
            b'7' => Ok(LayerKind::Wind),
            b'8' => Ok(LayerKind::Cloud),
            b'W' => Ok(LayerKind::Water),
            b'M' => Ok(LayerKind::AuroraLand),
            b'X' => Ok(LayerKind::AuroraWind),
            b'9' => Ok(LayerKind::AuroraCloud),
            b':' => Ok(LayerKind::AuroraWater),
            code => return Err(ExtractSurfaceErrorKind::UnknownLayerType(code).into()),
        }
    }
}

impl LayerKind {
    fn is_large_patch(&self) -> bool {
        match *self {
            LayerKind::Land => false,
            _ => unimplemented!(), // TODO
        }
    }
}

#[derive(Debug)]
pub struct Patch {
    /// Side length of the square shape patch.
    size: usize,

    /// Patch position in region.
    patch_x: u32,
    patch_y: u32,

    /// Decoded height map, square matrix of size `size`x`size`.
    /// TODO: (x,y)<->(i,j) ?
    data: Vec<f32>,
}

impl Patch {
    /// Side length of the square shape patch.
    pub fn side_length(&self) -> usize {
        self.size
    }

    /// Patch position (index, not meters) in the region.
    pub fn patch_position(&self) -> (u32, u32) {
        (self.patch_x, self.patch_y)
    }

    /// Return the value of at position (i,j).
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.data[i + j * self.size]
    }
}

pub struct Surface {
    cell_count_per_edge: u32,
    cell_width: f32,
    surface_width: f32,
}

#[derive(Debug)]
pub(crate) struct PatchHeader {
    quant: u32,
    word_bits: u32,
    dc_offset: u32,
    range: u16,
    patch_x: u32,
    patch_y: u32,
}

#[derive(Debug)]
pub(crate) struct PatchGroupHeader {
    stride: u32,
    patch_size: u32,
    layer_type: LayerKind,
}

impl Surface {
    pub fn extract_message(msg: &LayerData) -> Result<Vec<PatchMatrix>, ExtractSurfaceError> {
        let kind = LayerKind::from_code(msg.layer_id.type_)?;
        println!("kind : {:?}", kind);
        Self::extract(&msg.layer_data.data[..], kind.is_large_patch())
    }

    fn extract(data: &[u8], large_patch: bool) -> Result<Vec<Patch>, ExtractSurfaceError> {
        let mut reader = BitsReader::new(data);

        // Read patch_group_header
        let group_header = {
            // TODO: In the example reading a value of 264 indicates, that even for the Normal size
            // patch, the decoded matrix should be of the large size, i. e. for the small patches,
            // the resolution is lower.
            let stride = reader.read_full_u16::<LittleEndian>()?;
            // TODO: Can patch_i and patch_j be larger than this?
            // Because this is what's currently happening in the test, patch_size=16, but patch_i,j
            // are in the range {0,...LARGE_PATCH_SIZE-1=31}
            //
            // At this point I suspect (patch_x,patch_y) is not very relevant for decoding, i.e.
            // for large patches (patches_per_edge=32) patch_x being a u16 means it could go all
            // the way up to 65565 which is even worse than for normal size patches.
            let patch_size = reader.read_full_u8()?;
            let layer_type = reader.read_full_u8()?;

            PatchGroupHeader {
                stride: stride as u32,
                patch_size: patch_size as u32,
                layer_type: LayerKind::from_code(layer_type)?,
            }
        };

        println!("patch_group_header: {:?}", group_header);

        let mut decoded_patches = Vec::new();
        loop {
            // Read patch_header
            let header = {
                let quantity_wbits = reader.read_full_u8()?;
                if quantity_wbits == END_OF_PATCH {
                    break;
                }

                let quant = (quantity_wbits as u32 >> 4) + 2;
                let word_bits = (quantity_wbits as u32 & 0xf) + 2;

                // TODO: What is this variable? Funny values like
                // 1056964608 = 0b111111000000000000000000000000 are obtained?
                let dc_offset = reader.read_full_u32::<LittleEndian>()?;
                let range = reader.read_full_u16::<LittleEndian>()?;

                // TODO: figure out how byte order has to be handled for these
                let (patch_x, patch_y) = if large_patch {
                    let patchids = reader.read_full_u32::<LittleEndian>()?;
                    let x = patchids >> 16;
                    let y = patchids & 0xffff;
                    (x, y)
                } else {
                    let patchids = reader.read_part_u32::<LittleEndian, PadOnLeft>(10)?;
                    let x = patchids >> 5;
                    let y = patchids & 0x1f;
                    (x, y)
                };

                PatchHeader {
                    quant: quant,
                    word_bits: word_bits,
                    dc_offset: dc_offset,
                    range: range,
                    patch_x: patch_x,
                    patch_y: patch_y,
                }
            };

            //println!("decode patch, header: {:?}", header);

            let data = if large_patch {
                Self::decode_patch_data::<idct::LargePatch>(
                    &mut reader,
                    &header,
                    &group_header,
                    &TABLES_LARGE,
                )?
            } else {
                Self::decode_patch_data::<idct::NormalPatch>(
                    &mut reader,
                    &header,
                    &group_header,
                    &TABLES_NORMAL,
                )?
            };
            decoded_patches.push(data);
        }

        Ok(decoded_patches)
    }

    fn decode_patch_data<SIZE: PatchSize>(
        reader: &mut BitsReader,
        header: &PatchHeader,
        group_header: &PatchGroupHeader,
        tables: &PatchTables,
    ) -> Result<PatchMatrix, ExtractSurfaceError> {
        // Read patches.
        let mut patch_data = Vec::<i32>::new();
        'read_patch_data: for i in 0..SIZE::patches_per_region() {
            let exists = reader.read_bool()?;
            if exists {
                let not_eob = reader.read_bool()?;
                if not_eob {
                    // Read the item.
                    let sign = if reader.read_bool()? { -1 } else { 1 };
                    let value = reader.read_full_u8()? as i32;
                    patch_data.push(sign * value);
                } else {
                    for _ in i..SIZE::patches_per_region() {
                        patch_data.push(0);
                    }
                    break 'read_patch_data;
                }
            } else {
                patch_data.push(0);
            }
        }

        let tables = idct::PatchTables::compute::<SIZE>();
        Ok(idct::decompress_patch::<SIZE>(
            &patch_data,
            &header,
            &group_header,
            &tables,
        ))
    }
}
