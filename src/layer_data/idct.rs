//! Code for the DCT patch decompression.

use layer_data::land::PatchHeader;
use std::f32::consts::PI;
use types::nalgebra::DMatrix;

// TODO: When const generics are ever stabilized, this can be improved
//       and static size matrices be used in the following code.
//       Otherwise it turns out to be extremely hard to get the generic
//       code right!
pub trait PatchSize {
    /// Return the number of cells in one direction.
    fn per_direction() -> usize;

    fn per_patch() -> usize {
        Self::per_direction() * Self::per_direction()
    }
}

pub enum NormalPatch {}
pub enum LargePatch {}

impl PatchSize for NormalPatch {
    fn per_direction() -> usize {
        16
    }
}

impl PatchSize for LargePatch {
    fn per_direction() -> usize {
        32
    }
}

/// Store these somewhere so they don't have to be computed every time,
/// this is expensive.
///
/// All tables are per_direction x per_direction square matrices stored
/// in column major fashion.
pub struct PatchTables {
    dequantize: DMatrix<f32>,
    icosines: DMatrix<f32>,
    decopy: DMatrix<usize>,
}

impl PatchTables {
    pub fn compute<PS: PatchSize>() -> Self {
        let dequantize = DMatrix::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
            1. + 2. * ((i + j) as f32)
        });
        let icosines = DMatrix::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
            ((2. * (i as f32) + 1.) * (j as f32) * PI / (2. * (PS::per_direction() as f32))).cos()
        });

        // TODO: Find a better way to build the decopy matrix.
        // My initial idea of using Cantor's pairing function for this are complicated
        // by the fact, that as soon as we reach the lower-right diagonal part
        // of the matrix things get more complicated.
        let mut decopy = DMatrix::from_element(PS::per_direction(), PS::per_direction(), 0);
        let mut move_diag = false;
        let mut move_right = true;
        let mut i = 0;
        let mut j = 0;
        let mut count = 0;

        while i < PS::per_direction() && j < PS::per_direction() {
            // Fill next field.
            decopy[(i, j)] = count;
            count += 1;

            // Determine the next field.
            if !move_diag {
                if move_right {
                    if i < PS::per_direction() - 1 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                    move_right = false;
                    move_diag = true;
                } else {
                    if j < PS::per_direction() - 1 {
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
                    if (i == PS::per_direction() - 1) || (j == 0) {
                        move_diag = false;
                    }
                } else {
                    i -= 1;
                    j += 1;
                    if (i == 0) || (j == PS::per_direction() - 1) {
                        move_diag = false;
                    }
                }
            }
        }

        PatchTables {
            dequantize: dequantize,
            icosines: icosines,
            decopy: decopy,
        }
    }
}

fn idct_patch<PS: PatchSize>(block: &mut DMatrix<f32>, tables: &PatchTables) {
    let mut temp = block.clone();

    for j in 0..PS::per_direction() {
        idct_column::<PS>(&*block, &mut temp, j, tables);
    }
    for i in 0..PS::per_direction() {
        idct_row::<PS>(&temp, block, i, tables);
    }
}

fn idct_column<PS: PatchSize>(
    data_in: &DMatrix<f32>,
    data_out: &mut DMatrix<f32>,
    column: usize,
    tables: &PatchTables,
) {
    for n in 0..PS::per_direction() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in[(column, 0)];
        for x in 1..PS::per_direction() {
            total += data_in[(column, x)] * tables.icosines[(n, x)];
        }
        data_out[(column, n)] = total;
    }
}

fn idct_row<PS: PatchSize>(
    data_in: &DMatrix<f32>,
    data_out: &mut DMatrix<f32>,
    row: usize,
    tables: &PatchTables,
) {
    for n in 0..PS::per_direction() {
        let mut total: f32 = (2f32).sqrt() / 2. * data_in[(0, row)];
        for x in 1..PS::per_direction() {
            total += data_in[(x, row)] * tables.icosines[(n, x)];
        }
        data_out[(n, row)] = total * 2. / (PS::per_direction() as f32);
    }
}

pub(super) fn decompress_patch<PS: PatchSize>(
    patch_in: &Vec<i32>,
    header: &PatchHeader,
    tables: &PatchTables,
) -> DMatrix<f32> {
    let mut block: DMatrix<f32> =
        DMatrix::from_element(PS::per_direction(), PS::per_direction(), 0.);
    for k in 0..PS::per_patch() {
        block[k] = patch_in[tables.decopy[k]] as f32 * tables.dequantize[k];
    }

    idct_patch::<PS>(&mut block, tables);

    // Inverse the bijection applied before the DCT.
    let fact_mult: f32 = (header.range as f32) / 2f32.powi(header.quant as i32);
    let fact_add: f32 = (header.range as f32) / 2. + header.dc_offset;
    DMatrix::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
        block[(i, j)] * fact_mult + fact_add
    })
}
