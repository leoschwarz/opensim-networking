//! Code for the DCT patch decompression.

trait PatchSize {
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
struct PatchTables {
    dequantize: Vec<f32>,
    icosines: Vec<f32>,
    decopy: Vec<usize>,
}

impl PatchTables {
    fn compute<SIZE: PatchSize>() -> Self {
        let mut dequantize = Vec::new();
        let mut icosines = Vec::new();
        let mut decopy = Vec::new();

        for j in 0..SIZE::patches_per_edge() {
            for i in 0..SIZE::patches_per_edge() {
                dequantize.push(1. + 2. * (i + j));
                icosines.push(
                    (2. * i + 1.) * j * PI / (2. * SIZE::patches_per_edge()).cos(),
                );
            }
        }

        // TODO: decopy matrix

        assert_eq!(dequantize.len(), SIZE::patches_per_region());
        assert_eq!(icosines.len(), SIZE::patches_per_region());
        assert_eq!(decopy.len(), SIZE::patches_per_region());

        PatchTables {
            dequantize: dequantize,
            icosines: icosines,
            decopy: decopy,
        }
    }
}

struct LargePatch;

impl PatchSize for LargePatch {
    fn patches_per_edge() -> u32 {
        32
    }
}

struct NormalPatch;

impl PatchSize for NormalPatch {
    fn patches_per_edge() -> u32 {
        16
    }
}

fn idct_patch<SIZE: PatchSize>(block: &mut Vec<u8>, tables: &PatchTables) {
    assert_eq!(block.len(), SIZE::patches_per_region());

    let mut temp: Vec<f32> = Vec::new();
    for _ in SIZE::patches_per_region() {
        temp.push(0)
    }

    for j in 0..SIZE::patches_per_edge() {
        idct_column::<SIZE>(&*block, &mut temp, j);
    }
    for i in 0..SIZE::patches_per_edge() {
        idct_row::<SIZE>(&temp, block, i);
    }

    assert_eq!(block.len(), SIZE::patches_per_region());
}

fn idct_column<SIZE: PatchSize>(
    data_in: &Vec<u8>,
    data_out: &mut Vec<u8>,
    column: usize,
    tables: &PatchTables,
) {
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2.).sqrt() / 2. * data_in[column];
        for x in 1..SIZE::patches_per_edge() {
            total += data_in[column + x * SIZE::patches_per_edge()] *
                tables.icosines[n + x * SIZE::patches_per_edge()];
        }
        data_out[n * SIZE::patches_per_edge() + column] = total;
    }
}

fn idct_row<SIZE: PatchSize>(
    data_in: &Vec<u8>,
    data_out: &mut Vec<u8>,
    row: usize,
    tables: &PatchTables,
) {
    let line_offset = line * SIZE::patches_per_edge();
    for n in 0..SIZE::patches_per_edge() {
        let mut total: f32 = (2.).sqrt() / 2. * data_in[line_offset];
        for x in 1..SIZE::patches_per_edge() {
            total += data_in[line_offset + x] * tables.icosines[n + x * SIZE::patches_per_edge()];
        }
        data_out[line_offset + n] = total * (2. / SIZE::patches_per_edge());
    }
}

fn decompress_patch<SIZE: PatchSize>(patch_in: &Vec<i32>, tables: &PatchTables) {
    let mut block: Vec<f32> = Vec::new();
    for k in 0..SIZE::patches_per_region() {
        block.push(patch_in[tables.decopy[k]] * tables.dequantize[k]);
    }

    idct_patch::<SIZE>(&mut block, tables);

    // TODO
}
