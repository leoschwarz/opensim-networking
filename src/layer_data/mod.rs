// TODO: This should probably moved to its own crate at a later time. (Maybe some decoding
// facilities could be combined together conveniently.)

mod idct;
mod bitsreader;
mod extractor;

use nalgebra::DMatrix;

use messages::all::LayerData;
pub use self::extractor::{ExtractSurfaceError, ExtractSurfaceErrorKind};

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
    data: DMatrix<f32>,
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

    pub fn data(&self) -> &DMatrix<f32> {
        &self.data
    }

    /*
    /// Return the value of at position (i,j).
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.data[i + j * self.size]
    }
    */
}

/*
pub struct Surface {
    cell_count_per_edge: u32,
    cell_width: f32,
    surface_width: f32,
}
*/

pub struct Surface {}

impl Surface {
    pub fn extract_message(msg: &LayerData) -> Result<Vec<Patch>, ExtractSurfaceError> {
        let kind = LayerKind::from_code(msg.layer_id.type_)?;
        println!("kind : {:?}", kind);
        extractor::extract_patches(&msg.layer_data.data[..], kind.is_large_patch())
    }
}
