// TODO: This should probably moved to its own crate at a later time.

use bitreader::{BitReader, BitReaderError};

use messages::all::LayerData;

mod idct;

const END_OF_PATCH: u8 = 97u8;

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
    pub fn extract_message(msg: &LayerData) -> Result<(), ExtractSurfaceError> {
        let kind = LayerKind::from_code(msg.layer_id.type_)?;
        println!("kind : {:?}", kind);
        Self::extract(&msg.layer_data.data[..], kind.is_large_patch())
    }

    fn extract(data: &[u8], large_patch: bool) -> Result<(), ExtractSurfaceError> {
        let mut reader = BitReader::new(data);

        // Read patch_group_header
        let group_header = {
            // TODO We read a value of 2049 for the patch, this is way too much!!!
            let stride = reader.read_u16(16)?; // TODO byte order
            // TODO: Can patch_i and patch_j be larger than this?
            // Because this is what's currently happening in the test, patch_size=16, but patch_i,j
            // are in the range {0,...LARGE_PATCH_SIZE-1=31}
            //
            // At this point I suspect (patch_x,patch_y) is not very relevant for decoding, i.e.
            // for large patches (patches_per_edge=32) patch_x being a u16 means it could go all
            // the way up to 65565 which is even worse than for normal size patches.
            let patch_size = reader.read_u8(8)?;
            let layer_type = reader.read_u8(8)?;

            PatchGroupHeader {
                stride: stride as u32,
                patch_size: patch_size as u32,
                layer_type: LayerKind::from_code(layer_type)?,
            }
        };

        println!("patch_group_header: {:?}", group_header);

        loop {
            // Read patch_header
            let header = {
                let quantity_wbits = reader.read_u8(8)?;
                if quantity_wbits == END_OF_PATCH {
                    break;
                }

                let quant = (quantity_wbits as u32 >> 4) + 2;
                let word_bits = (quantity_wbits as u32 & 0xf) + 2;

                let dc_offset = reader.read_u32(32)?; // TODO byte order
                let range = reader.read_u16(16)?; // TODO byte order

                let (patch_x, patch_y) = if large_patch {
                    let x = reader.read_u32(16)?;
                    let y = reader.read_u32(16)?;
                    (x, y)
                } else {
                    let x = reader.read_u32(5)?;
                    let y = reader.read_u32(5)?;
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

            println!("=============== new patch ================ ");
            println!("patch_header: {:?}", header);

            // Read patches.
            // TODO: don't depend on group_header.patch_size but make this generic code too.
            let mut patch_data = Vec::<i32>::new();
            'read_patch_data: for i in 0..group_header.patch_size * group_header.patch_size {
                let exists = reader.read_bool()?;
                if exists {
                    let not_eob = reader.read_bool()?;
                    if not_eob {
                        // Read the item.
                        let sign = if reader.read_bool()? { -1 } else { 1 };
                        let value = reader.read_u8(8)? as i32;
                        patch_data.push(sign * value);
                    } else {
                        for _ in i..group_header.patch_size * group_header.patch_size {
                            patch_data.push(0);
                        }
                        break 'read_patch_data;
                    }
                } else {
                    patch_data.push(0);
                }
            }

            println!("patch_data.len(): {}", patch_data.len());

            if large_patch {
                let tables = idct::PatchTables::compute::<idct::LargePatch>();
                idct::decompress_patch::<idct::LargePatch>(&patch_data, &header, &group_header, &tables);
            } else {
                let tables = idct::PatchTables::compute::<idct::NormalPatch>();
                idct::decompress_patch::<idct::NormalPatch>(&patch_data, &header, &group_header, &tables);
            }

        }

        Ok(())
    }
}
