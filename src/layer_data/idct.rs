//! Code for the DCT patch decompression.

use layer_data::PatchMatrix;
use layer_data::land::PatchHeader;
use std::f32::consts::PI;
use std::ops::Mul;
use typenum;
use types::{Matrix};
use types::nalgebra::core::{MatrixArray, NamedDim};
use types::nalgebra::core::dimension::{Dim, DimName, U16, U32};
use generic_array;

type MatrixD<S, PS: PatchSize> =
    Matrix<S, PS::DimName, PS::DimName, MatrixArray<
    <<<PS as PatchSize>::DimName as DimName>::Value as Mul>::Output, PS::DimName, PS::DimName>>;

pub trait PatchSize where
{
    // e.g. typenum::U16
    type DimValue: Mul + NamedDim<Name = Self::DimName>;
    // e.g. nalgebra::core::dimension::U16
    type DimName: DimName<Value=Self::DimValue>;
    //type DimName: DimName;

    /// Return the number of cells in one direction.
    fn per_direction() -> usize {
        // Note: Should never fail for U16, U32.
        Self::MatDim::try_to_usize().unwrap()
    }

    fn per_patch() -> usize {
        Self::per_direction() * Self::per_direction()
    }
}

pub enum NormalPatch {}
impl PatchSize for NormalPatch {
    type DimValue = typenum::U16;
    type DimName = U16;
}

pub enum LargePatch {}
impl PatchSize for LargePatch {
    type DimValue = typenum::U32;
    type DimName = U32;
}

/// Store these somewhere so they don't have to be computed every time,
/// this is expensive.
///
/// All tables are per_direction x per_direction square matrices stored
/// in column major fashion.
pub struct PatchTables<PS: PatchSize>
{
    dequantize: MatrixD<f32, PS>,
    icosines: MatrixD<f32, PS>,
    decopy: MatrixD<usize, PS>,
}

impl<PS: PatchSize> PatchTables<PS>
{
    pub fn compute() -> Self {
        let dequantize = MatrixD::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
            1. + 2. * ((i + j) as f32)
        });
        let icosines = MatrixD::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
            ((2. * (i as f32) + 1.) * (j as f32) * PI / (2. * (PS::per_direction() as f32))).cos()
        });

        // TODO: Find a better way to build the decopy matrix.
        // My initial idea of using Cantor's pairing function for this are complicated
        // by the fact, that as soon as we reach the lower-right diagonal part
        // of the matrix things get more complicated.
        let mut decopy = MatrixD::from_element(PS::per_direction(), PS::per_direction(), 0);
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

fn idct_patch<PS: PatchSize>(block: &mut MatrixD<f32, PS>, tables: &PatchTables<PS>) {
    let mut temp = block.clone();

    for j in 0..PS::per_direction() {
        idct_column::<PS>(&*block, &mut temp, j, tables);
    }
    for i in 0..PS::per_direction() {
        idct_row::<PS>(&temp, block, i, tables);
    }
}

fn idct_column<PS: PatchSize>(
    data_in: &MatrixD<f32, PS>,
    data_out: &mut MatrixD<f32, PS>,
    column: usize,
    tables: &PatchTables<PS>,
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
    data_in: &MatrixD<f32, PS>,
    data_out: &mut MatrixD<f32, PS>,
    row: usize,
    tables: &PatchTables<PS>,
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
    tables: &PatchTables<PS>,
) -> MatrixD<f32, PS> {
    let mut block: MatrixD<f32, PS> =
        MatrixD::from_element(PS::per_direction(), PS::per_direction(), 0.);
    for k in 0..PS::per_patch() {
        block[k] = patch_in[tables.decopy[k]] as f32 * tables.dequantize[k];
    }

    idct_patch::<PS>(&mut block, tables);

    // Inverse the bijection applied before the DCT.
    let fact_mult: f32 = (header.range as f32) / 2f32.powi(header.quant as i32);
    let fact_add: f32 = (header.range as f32) / 2. + header.dc_offset;
    MatrixD::from_fn(PS::per_direction(), PS::per_direction(), |i, j| {
        block[(i, j)] * fact_mult + fact_add
    })
}
