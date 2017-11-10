//! Code for the DCT patch decompression.

use super::{PatchHeader, PatchGroupHeader};
use std::f32::consts::PI;

#[derive(Clone)]
pub struct PatchMatrix {
    // col major matrix data, padded with `stride` zero rows
    data: Vec<f32>,
    stride: usize,
}

impl PatchMatrix {
    fn new(stride: usize) -> Self {
        PatchMatrix {
            data: vec![0.; LargePatch::patches_per_region() as usize],
            stride: stride,
        }
    }

    /// direct access to the entries of the matrix without any index transformation at all
    fn direct(&mut self, index: usize) -> &mut f32 {
        self.data.get_mut(index).unwrap_or_else(|| {
            panic!("invalid index: {}", index)
        })
    }

    /// projects (i,j) to the position in the absolute matrix without considering stride,
    /// i.e. this indexes the 32x32 matrix instead of what is contained.
    fn map_full(&mut self, i: usize, j: usize) -> &mut f32 {
        let index = j * (LargePatch::patches_per_edge() as usize) + i;
        self.data.get_mut(index).unwrap_or_else(|| {
            panic!("invalid index: ({},{})={}", i, j, index)
        })
    }

    fn get_full(&self, i: usize, j: usize) -> f32 {
        let index = j * (LargePatch::patches_per_edge() as usize) + i;
        self.data[index]
    }

    /// projects (i,j) to the actual data matrix (i.e. map to the matrix if the stride rows are
    /// removed from the matrix.)
    fn map_data(&mut self, i: usize, j: usize) -> &mut f32 {
        // TODO: what's the point of the stride header?
        // the value decoded is actually too large to be used as intended, so either our
        // header decoding is wrong or idk
        self.map_full(i, j)
        /*
        let index = j * self.stride + i;
        self.data
            .get_mut(index)
            .unwrap_or_else(|| panic!("invalid index: ({},{})={}", i, j, index))
            */
    }

    fn get_data(&self, i: usize, j: usize) -> f32 {
        self.get_full(i, j)
        /*
        let index = j * self.stride + i;
        self.data[index]
        */
    }
}

pub trait PatchSize {
    #[inline]
    fn patches_per_edge() -> u32;

    #[inline]
    fn patches_per_region() -> u32 {
        Self::patches_per_edge() * Self::patches_per_edge()
    }
}

/// Store these somewhere so they don't have to be computed every time,
/// this is expensive.
///
/// All tables are patches_per_edge x patches_per_edge square matrices stored
/// in column major fashion.
pub struct PatchTables {
    dequantize: Vec<f32>,
    icosines: Vec<f32>,
    decopy: Vec<usize>,
}

impl PatchTables {
    pub fn compute<SIZE: PatchSize>() -> Self {
        let mut dequantize = Vec::new();
        let mut icosines = Vec::new();
        let mut decopy = Vec::new();

        for j in 0..SIZE::patches_per_edge() {
            for i in 0..SIZE::patches_per_edge() {
                dequantize.push(1. + 2. * ((i + j) as f32));
                icosines.push(
                    (2. * (i as f32) + 1.) * (j as f32) * PI /
                        (2. * (SIZE::patches_per_edge() as f32)).cos(),
                );
            }
        }

        // TODO: Find a better way to build the decopy matrix.
        // My initial idea of using Cantor's pairing function for this are complicated by the fact,
        // that as soon as we reach the lower-right diagonal part of the matrix things get more
        // complicated.
        for _ in 0..SIZE::patches_per_region() {
            decopy.push(0);
        }

        let mut move_diag = false;
        let mut move_right = true;
        let mut i = 0;
        let mut j = 0;
        let mut count = 0;

        while i < SIZE::patches_per_edge() && j < SIZE::patches_per_region() {
            // Fill next field.
            decopy[(i + j * SIZE::patches_per_edge()) as usize] = count;
            count += 1;

            // Determine the next field.
            if !move_diag {
                if move_right {
                    if i < SIZE::patches_per_edge() - 1 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                    move_right = false;
                    move_diag = true;
                } else {
                    if j < SIZE::patches_per_edge() - 1 {
                        j += 1;
                    } else {
                        i += 1;
                    }
                    move_right = true;
                    move_diag = true;
                }
            } else {
                if move_right {
                    i += 1;
                    j -= 1;
                    if (i == SIZE::patches_per_edge() - 1) || (j == 0) {
                        move_diag = false;
                    }
                } else {
                    i -= 1;
                    j += 1;
                    if (i == 0) || (j == SIZE::patches_per_edge() - 1) {
                        move_diag = false;
                    }
                }
            }
        }

        assert_eq!(dequantize.len(), SIZE::patches_per_region() as usize);
        assert_eq!(icosines.len(), SIZE::patches_per_region() as usize);
        assert_eq!(decopy.len(), SIZE::patches_per_region() as usize);

        PatchTables {
            dequantize: dequantize,
            icosines: icosines,
            decopy: decopy,
        }
    }
}

pub struct LargePatch;

impl PatchSize for LargePatch {
    fn patches_per_edge() -> u32 {
        32
    }
}

pub struct NormalPatch;

impl PatchSize for NormalPatch {
    fn patches_per_edge() -> u32 {
        16
    }
}

fn idct_patch<SIZE: PatchSize>(block: &mut PatchMatrix, tables: &PatchTables) {
    let mut temp = block.clone();

    for j in 0..SIZE::patches_per_edge() {
        idct_column::<SIZE>(&*block, &mut temp, j, tables);
    }
    for i in 0..SIZE::patches_per_edge() {
        idct_row::<SIZE>(&temp, block, i, tables);
    }
}

fn idct_column<SIZE: PatchSize>(
    data_in: &PatchMatrix,
    data_out: &mut PatchMatrix,
    column: u32,
    tables: &PatchTables,
) {
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in.get_full(column as usize, 0);
        for x in 1..SIZE::patches_per_edge() {
            total += data_in.get_full(x as usize, column as usize) *
                tables.icosines[(n + x * SIZE::patches_per_edge()) as usize];
        }
        *data_out.map_full(n as usize, column as usize) = total;
    }
}

fn idct_row<SIZE: PatchSize>(
    data_in: &PatchMatrix,
    data_out: &mut PatchMatrix,
    row: u32,
    tables: &PatchTables,
) {
    //let row_offset = (row * SIZE::patches_per_edge()) as usize;
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in.get_full(0, row as usize);
        for x in 1..SIZE::patches_per_edge() {
            total += data_in.get_full(x as usize, row as usize) *
                tables.icosines[(n + x * SIZE::patches_per_edge()) as usize];
        }
        *data_out.map_full(n as usize, row as usize) = total *
            (2. / (SIZE::patches_per_edge() as f32));
    }
}

pub(crate) fn decompress_patch<SIZE: PatchSize>(
    patch_in: &Vec<i32>,
    header: &PatchHeader,
    group_header: &PatchGroupHeader,
    tables: &PatchTables,
) -> PatchMatrix {
    let mut block = PatchMatrix::new(group_header.stride as usize);
    for k in 0..SIZE::patches_per_region() {
        *block.direct(k as usize) = patch_in[tables.decopy[k as usize]] as f32 *
            tables.dequantize[k as usize];
    }

    idct_patch::<SIZE>(&mut block, tables);

    // TODO: make this cleaner here and in the spec
    let fact_mult: f32 = (header.range as f32) / ((1u32 << header.quant) as f32);
    let fact_add: f32 = fact_mult * ((1u32 << (header.quant - 1)) as f32) +
        (header.dc_offset as f32);

    let mut patch_out = PatchMatrix::new(group_header.stride as usize);
    for j in 0..SIZE::patches_per_edge() {
        for i in 0..SIZE::patches_per_edge() {
            *patch_out.map_data(i as usize, j as usize) =
                block.get_full(i as usize, j as usize) * fact_mult + fact_add;
        }
    }
    patch_out
}
