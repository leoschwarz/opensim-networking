//! Code for the DCT patch decompression.

use super::{PatchHeader, PatchGroupHeader};
use std::f32::consts::PI;

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
                    (2. * (i as f32) + 1.) * (j as f32) * PI / (2. * (SIZE::patches_per_edge() as f32)).cos(),
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

fn idct_patch<SIZE: PatchSize>(block: &mut Vec<f32>, tables: &PatchTables) {
    assert_eq!(block.len(), SIZE::patches_per_region() as usize);

    let mut temp: Vec<f32> = Vec::new();
    for _ in 0..SIZE::patches_per_region() {
        temp.push(0.)
    }

    for j in 0..SIZE::patches_per_edge() {
        idct_column::<SIZE>(&*block, &mut temp, j, tables);
    }
    for i in 0..SIZE::patches_per_edge() {
        idct_row::<SIZE>(&temp, block, i, tables);
    }

    assert_eq!(block.len(), SIZE::patches_per_region() as usize);
}

fn idct_column<SIZE: PatchSize>(
    data_in: &Vec<f32>,
    data_out: &mut Vec<f32>,
    column: u32,
    tables: &PatchTables,
) {
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in[column as usize];
        for x in 1..SIZE::patches_per_edge() {
            total += data_in[(column + x * SIZE::patches_per_edge()) as usize] *
                tables.icosines[(n + x * SIZE::patches_per_edge()) as usize];
        }
        data_out[(n * SIZE::patches_per_edge() + column) as usize] = total;
    }
}

fn idct_row<SIZE: PatchSize>(
    data_in: &Vec<f32>,
    data_out: &mut Vec<f32>,
    row: u32,
    tables: &PatchTables,
) {
    let row_offset = (row * SIZE::patches_per_edge()) as usize;
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in[row_offset];
        for x in 1..SIZE::patches_per_edge() {
            total += data_in[row_offset + x as usize] *
                tables.icosines[(n + x * SIZE::patches_per_edge()) as usize];
        }
        data_out[row_offset + n as usize] = total * (2. / (SIZE::patches_per_edge() as f32));
    }
}

pub(crate) fn decompress_patch<SIZE: PatchSize>(
    patch_in: &Vec<i32>,
    header: &PatchHeader,
    group_header: &PatchGroupHeader,
    tables: &PatchTables,
) -> Vec<f32> {
    let mut block: Vec<f32> = Vec::new();
    for k in 0..SIZE::patches_per_region() {
        block.push(
            patch_in[tables.decopy[k as usize]] as f32 * tables.dequantize[k as usize],
        );
    }

    idct_patch::<SIZE>(&mut block, tables);

    // TODO: make this cleaner here and in the spec
    let fact_mult: f32 = (header.range as f32) / ((1u32 << header.quant) as f32);
    let fact_add: f32 = fact_mult * ((1u32 << (header.quant - 1)) as f32) + (header.dc_offset as f32);

    let mut patch_out: Vec<f32> = Vec::new();
    for _ in 0..SIZE::patches_per_region() {
        patch_out.push(0.);
    }
    for j in 0..SIZE::patches_per_edge() {
        for i in 0..SIZE::patches_per_edge() {
            patch_out[(j * group_header.stride + i) as usize] =
                block[(j * SIZE::patches_per_edge() + i) as usize] * fact_mult + fact_add;
        }
    }
    patch_out
}
