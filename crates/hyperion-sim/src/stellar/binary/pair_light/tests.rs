//! The pair-light tables' tests (plan 11, P11.T17.b).

use super::*;

use crate::sky::photometry::absolute_v_of_state;
use crate::stellar::binary::testing::{Mix, generated_pairs, milky_way};
use crate::units::consts::METRES_PER_AU;
use hyperion_testkit::float::assert_same_bits;

/// The sky's photometry, as the fit reads it.
fn sky(state: &StarState) -> Option<Magnitudes> {
    absolute_v_of_state(state)
}

/// Every layer's grid.
fn grids() -> [PairGrid; 3] {
    LAYERS.map(|layer| PairGrid::of(layer).expect("a layer with a table"))
}

/// A circular pair of `m1` and `m2` (M☉) at solar metallicity of separation `a_au` (au), at the
/// median draws.
fn circular(m1: f64, m2: f64, a_au: f64) -> BinaryInput {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(a_au * METRES_PER_AU),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.5), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(0.0),
    )
    .expect("an orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::SOLAR,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(1.0e10),
    )
    .expect("a pair")
}

#[test]
fn the_grid_numbers_its_cells_both_ways() {
    for grid in grids() {
        assert_eq!(
            grid.cells(),
            MASS_CELLS * Q_CELLS * FE_H_CELLS * grid.periastron_cells()
        );
        for cell in 0..grid.cells() {
            assert_eq!(grid.cell_index(grid.cell_parts(cell)), cell);
        }
        let band = MassBand::of_layer(grid.layer());
        assert_same_bits(grid.mass_edges_msun(0).0, band.lo());
        assert_same_bits(grid.mass_edges_msun(MASS_CELLS - 1).1, band.hi());
        for k in 1..MASS_CELLS {
            assert_same_bits(grid.mass_edges_msun(k - 1).1, grid.mass_edges_msun(k).0);
        }
        assert_same_bits(grid.q_edges(0).0, LOWEST_COMPANION_MSUN / band.hi());
        let open = grid.periastron_edges_rsun(grid.periastron_cells() - 1);
        assert_same_bits(open.1, OPEN_PERIASTRON_REACH_RSUN);
    }
    assert!(PairGrid::of(Layer::A).is_none() && PairGrid::of(Layer::BrownDwarf).is_none());
}

/// Each cell's samples lie inside it, its first sixteen at its corners, and the reader finds the
/// cell of every sample inside it.
#[test]
fn a_cells_samples_lie_inside_it_and_its_corners_on_its_edges() {
    for grid in grids() {
        let cells = [0, grid.cells() / 3, grid.cells() / 2 + 7, grid.cells() - 1];
        for cell in cells {
            let [mi, qi, fi, pi] = grid.cell_parts(cell);
            let (m_lo, m_hi) = grid.mass_edges_msun(mi);
            let (q_lo, q_hi) = grid.q_edges(qi);
            let (f_lo, f_hi) = grid.fe_h_edges(fi);
            let (p_lo, p_hi) = grid.periastron_edges_rsun(pi);
            for index in 0..48 {
                let input = sample_input(&grid, cell, index, FIT_SEED);
                let [m1, m2] = input.masses().map(SolarMasses::value);
                assert!((m_lo..=m_hi).contains(&m1), "{cell} {index}: {m1}");
                assert!(
                    m2 <= m1 && m2 >= LOWEST_COMPANION_MSUN,
                    "{cell} {index}: {m2}"
                );
                let q = m2 / m1;
                let clamped = m2.total_cmp(&LOWEST_COMPANION_MSUN).is_eq();
                assert!(
                    clamped || (q >= q_lo * (1.0 - 1e-12) && q <= q_hi * (1.0 + 1e-12)),
                    "{cell} {index}: q {q} of {q_lo}-{q_hi}"
                );
                let fe_h = super::super::reach::fe_h_of(input.composition());
                assert!((f_lo - 1e-9..=f_hi + 1e-9).contains(&fe_h), "{fe_h}");
                let p = input.orbit().periapsis().value() / SOLAR_RADIUS_M;
                assert!(
                    p >= p_lo * (1.0 - 1e-9) && p <= p_hi * (1.0 + 1e-9),
                    "{cell} {index}: periastron {p} of {p_lo}-{p_hi}"
                );
                let e = input.orbit().eccentricity().value();
                assert!((0.0..=MAX_SAMPLED_ECCENTRICITY).contains(&e), "{e}");
                if index < CORNER_SAMPLES {
                    let corner = |bit: u32, lo: f64, hi: f64| {
                        if (index >> bit) & 1 == 1 { hi } else { lo }
                    };
                    assert_same_bits(m1, corner(0, m_lo, m_hi));
                } else if !clamped {
                    let found = grid.cell_of(m1, m2, fe_h, p);
                    assert_eq!(found, Some(cell), "{cell} {index}: {m1} {m2} {fe_h} {p}");
                }
            }
        }
    }
}

#[test]
fn the_reader_finds_no_cell_outside_the_table() {
    let grid = PairGrid::of(Layer::C).expect("C's grid");
    assert_eq!(grid.cell_of(0.7, 0.5, 0.0, 100.0), None, "below the band");
    assert_eq!(grid.cell_of(2.6, 0.5, 0.0, 100.0), None, "above the band");
    assert_eq!(grid.cell_of(1.0, 0.07, 0.0, 100.0), None, "a brown dwarf");
    assert_eq!(
        grid.cell_of(1.0, 1.1, 0.0, 100.0),
        None,
        "the lighter first"
    );
    assert_eq!(
        grid.cell_of(1.0, 0.5, 0.0, 0.5),
        None,
        "inside the first bin"
    );
    assert_eq!(grid.cell_of(1.0, 0.5, f64::NAN, 100.0), None);
    assert_eq!(grid.cell_of(1.0, 0.5, 0.0, f64::NAN), None);
    // Past the open bin's samples, and past the metallicity clamps, the edge cells.
    let wide = grid.cell_of(1.0, 0.5, 0.0, 1.0e9).expect("the open bin");
    assert_eq!(grid.cell_parts(wide)[3], grid.periastron_cells() - 1);
    let rich = grid
        .cell_of(1.0, 0.5, 0.5, 100.0)
        .expect("the metal-rich cell");
    assert_eq!(grid.cell_parts(rich)[2], FE_H_CELLS - 1);
    let edge = grid.cell_of(2.5, 2.5, -3.0, 1.0).expect("every upper edge");
    assert_eq!(grid.cell_parts(edge), [MASS_CELLS - 1, Q_CELLS - 1, 0, 0]);
}

#[test]
fn age_bins_and_their_edges_agree() {
    assert_eq!(age_bin(0.0), Some(0));
    assert_eq!(age_bin(-1.0), Some(0));
    assert_eq!(age_bin(9.9e4), Some(0));
    assert_eq!(age_bin(LAST_AGE_YEARS), Some(AGE_BINS - 1));
    assert_eq!(age_bin(1.6e10), None);
    assert_eq!(age_bin(f64::NAN), None);
    for k in 1..AGE_BINS {
        let (lo, hi) = (age_edge_years(k), age_edge_years(k + 1));
        assert!(hi > lo, "{k}");
        assert_eq!(age_bin(lo * (1.0 + 1e-12)), Some(k), "{k}");
        assert_eq!(age_bin(f64::midpoint(lo, hi)), Some(k), "{k}");
    }
}

/// A pair too wide to interact holds its stars' own models only: nothing changed, and a living
/// Sun-like star until about 10¹⁰ years.
#[test]
fn a_pair_the_pre_test_passes_over_holds_only_its_own_models() {
    let rows = pair_rows(&circular(1.0, 0.5, 5.0e3), &sky);
    assert!(!rows.changes(), "{:?}", rows.changed);
    let sun_now = age_bin(4.6e9).expect("a bin");
    assert!(
        (3.5..5.5).contains(&rows.living[sun_now]),
        "{}",
        rows.living[sun_now]
    );
    // The red dwarf outlives the table; protostars are dark.
    assert!(rows.living[AGE_BINS - 1].is_finite());
    assert_same_bits(rows.living[0], UNSEEN_MAG);
}

/// An Algol (Hurley, Tout and Pols 2002, section 3.1) holds a changed star from its transfer on,
/// the gainer of more than 2 M☉ at 5 × 10⁸ years (`evolve`'s example), and none before.
#[test]
fn an_algol_holds_a_changed_star_from_its_transfer_on() {
    // A 3-day orbit: a³ = M P² in au, M☉ and years.
    let a_au = math::cbrt(3.8 * (3.0 / 365.25) * (3.0 / 365.25));
    let rows = pair_rows(&circular(2.9, 0.9, a_au), &sky);
    let young = age_bin(1.0e8).expect("a bin");
    let after = age_bin(5.0e8).expect("a bin");
    assert!(rows.changed[young].is_infinite(), "{:?}", rows.changed);
    assert!(rows.changed[after].is_finite(), "{:?}", rows.changed);
    assert!(rows.living[after] <= rows.changed[after]);
}

/// The rows of a pair are a pure function: the same twice, in any order.
#[test]
fn a_pairs_rows_are_the_same_twice() {
    let grid = PairGrid::of(Layer::D).expect("D's grid");
    let inputs: Vec<BinaryInput> = [5, 900, 3_000]
        .map(|cell| sample_input(&grid, cell, 20, FIT_SEED))
        .into();
    let once: Vec<PairRows> = inputs.iter().map(|i| pair_rows(i, &sky)).collect();
    let reversed: Vec<PairRows> = inputs.iter().rev().map(|i| pair_rows(i, &sky)).collect();
    for (a, b) in once.iter().zip(reversed.iter().rev()) {
        for k in 0..AGE_BINS {
            assert_same_bits(a.living[k], b.living[k]);
            assert_same_bits(a.changed[k], b.changed[k]);
        }
    }
}

#[test]
fn cell_rows_merge_in_any_order() {
    let mut rows = Vec::new();
    for k in 0..5 {
        let mut r = PairRows::empty();
        r.living[k] = 1.0 + f64::from(u8::try_from(k).expect("small"));
        r.changed[10 + k] = 2.0;
        r.changed[3] = UNSEEN_MAG;
        rows.push(r);
    }
    let mut forward = CellRows::default();
    for r in &rows {
        forward.add(r);
    }
    let mut halves = [CellRows::default(), CellRows::default()];
    for (i, r) in rows.iter().enumerate().rev() {
        halves[i % 2].add(r);
    }
    let mut merged = halves[1].clone();
    merged.merge(&halves[0]);
    assert_eq!(forward, merged);
    assert_eq!(forward.samples(), 5);
    assert_eq!(forward.departing()[3], 5);
    assert_eq!(forward.departing()[10], 1);
    assert_same_bits(forward.living_mag()[0], 1.0);
}

#[test]
fn stored_values_round_brighter_and_keep_their_order() {
    assert_eq!(to_cmag(f64::INFINITY), Some(DARK_CMAG));
    assert_eq!(to_cmag(UNSEEN_MAG), Some(UNSEEN_CMAG));
    assert_eq!(to_cmag(f64::NAN), None);
    assert_eq!(to_cmag(f64::NEG_INFINITY), None);
    assert_eq!(to_cmag(-21.0), None);
    assert_eq!(to_cmag(1.234_5), Some(123));
    assert_eq!(to_cmag(-1.234_5), Some(-124));
    assert_eq!(to_cmag(-20.48), Some(BRIGHTEST_CMAG));
    assert_eq!(to_cmag(40.0), Some(FAINTEST_CMAG));
    let mut mix = Mix(0x7e57_0017_b0b0_0001);
    for _ in 0..10_000 {
        let v = -20.0 + 45.0 * mix.unit();
        let k = to_cmag(v).expect("storable");
        assert!(from_cmag(k) <= v, "{v} {k}");
        assert!(v - from_cmag(k) < 0.02 || k == FAINTEST_CMAG, "{v} {k}");
    }
    assert!(from_cmag(FAINTEST_CMAG) < from_cmag(UNSEEN_CMAG));
    assert!(from_cmag(UNSEEN_CMAG) < from_cmag(DARK_CMAG));
    assert_eq!(
        [0, 1, 2, 3, 4, 7, 8, 64, 65].map(count_class),
        [0, 1, 2, 2, 3, 3, 4, 7, 7]
    );
}

#[test]
fn runs_within_the_tolerance_merge_at_their_brightest() {
    let mut row = [f64::INFINITY; AGE_BINS];
    row[10] = 3.0;
    row[11] = 3.1;
    row[12] = 3.15;
    row[13] = 5.0;
    row[14] = UNSEEN_MAG;
    row[20] = 12.0;
    row[21] = 12.5;
    let stored = stored_row(&row, &merge_tolerance_mag).expect("storable");
    let m = |v: f64| to_cmag(v - MARGIN_MAG).expect("storable");
    assert_eq!(stored[9], DARK_CMAG);
    assert_eq!(&stored[10..13], &[m(3.0); 3]);
    assert_eq!(stored[13], m(5.0));
    assert_eq!(stored[14], UNSEEN_CMAG);
    // Past +10 the tolerance grows: 0.2 + 0.2 × 1.7.
    assert_eq!(&stored[20..22], &[m(12.0); 2]);
    // The living rows merge within a magnitude, and within two in E.
    let grid = PairGrid::of(Layer::C).expect("C's grid");
    let tolerance = |v: f64| grid.living_merge_tolerance_mag(v);
    let living = stored_row(&row, &tolerance).expect("storable");
    assert_eq!(&living[10..13], &[m(3.0); 3]);
    assert_eq!(living[13], m(5.0));
    row[13] = 3.9;
    let living = stored_row(&row, &tolerance).expect("storable");
    assert_eq!(&living[10..14], &[m(3.0); 4]);
    row[13] = 4.9;
    let e = PairGrid::of(Layer::E).expect("E's grid");
    let living = stored_row(&row, &|v| e.living_merge_tolerance_mag(v)).expect("storable");
    assert_eq!(&living[10..14], &[m(3.0); 4]);
    let living = stored_row(&row, &tolerance).expect("storable");
    assert_eq!(living[13], m(4.9));
}

/// One sample's changed star in one cell reaches every closer periastron, its neighbours one
/// cell along each axis and one age bin, brightened by the margin; its count stays its own.
#[test]
fn the_assembly_takes_wider_orbits_neighbours_and_the_margin() {
    let grid = PairGrid::of(Layer::C).expect("C's grid");
    let parts = [5, 3, 2, 6];
    let at = grid.cell_index(parts);
    let mut cells = vec![CellRows::default(); grid.cells()];
    for cell in &mut cells {
        cell.add(&PairRows::empty());
    }
    let mut sample = PairRows::empty();
    sample.changed[30] = 2.0;
    sample.living[30] = 2.0;
    cells[at].add(&sample);
    let table = assemble(&grid, &cells).expect("a table");
    let read = PairLightTable::from_cells(&table);
    let value = to_cmag(2.0 - MARGIN_MAG).expect("storable");
    let changed = |p: [usize; 4], bin: usize| read.changed_cmag(grid.cell_index(p), bin);
    assert_eq!(changed(parts, 30), value);
    assert_eq!(changed(parts, 29), value);
    assert_eq!(changed(parts, 31), value);
    assert_eq!(changed(parts, 28), DARK_CMAG);
    // Every closer orbit, one wider by the dilation, not two.
    for p in 0..=7 {
        assert_eq!(changed([5, 3, 2, p], 30), value, "periastron bin {p}");
    }
    assert_eq!(changed([5, 3, 2, 8], 30), DARK_CMAG);
    for (axis, len) in [(0, MASS_CELLS), (1, Q_CELLS), (2, FE_H_CELLS)] {
        let mut near = parts;
        near[axis] += 1;
        assert_eq!(changed(near, 30), value, "axis {axis}");
        if parts[axis] + 2 < len {
            near[axis] += 1;
            assert_eq!(changed(near, 30), DARK_CMAG, "axis {axis}");
        }
        let mut below = parts;
        below[axis] -= 1;
        assert_eq!(changed(below, 30), value, "axis {axis}");
    }
    assert_eq!(read.departing_class(at, 30), 1);
    assert_eq!(read.departing_at_least(at, 30), 1);
    assert_eq!(read.departing_class(grid.cell_index([5, 3, 2, 5]), 30), 0);
    assert_eq!(read.living_cmag(at, 30), value);
    assert_eq!(read.living_cmag(at, 0), DARK_CMAG);
    assert_eq!(read.samples_per_cell(), 1);
    assert_same_bits(table.changed_unmargined_mag(at)[30], 2.0);
}

#[test]
fn the_assembly_refuses_unsound_cells() {
    let grid = PairGrid::of(Layer::E).expect("E's grid");
    let filled = || {
        let mut cells = vec![CellRows::default(); grid.cells()];
        for cell in &mut cells {
            cell.add(&PairRows::empty());
        }
        cells
    };
    assert_eq!(
        assemble(&grid, &filled()[1..]),
        Err(AssemblePairLightError::WrongShape {
            expected: grid.cells(),
            found: grid.cells() - 1
        })
    );
    let mut empty = filled();
    empty[17] = CellRows::default();
    assert_eq!(
        assemble(&grid, &empty),
        Err(AssemblePairLightError::EmptyCell { cell: 17 })
    );
    let mut unsound = filled();
    let mut nan = PairRows::empty();
    nan.changed[4] = f64::NAN;
    unsound[9].add(&nan);
    assert_eq!(
        assemble(&grid, &unsound),
        Err(AssemblePairLightError::UnsoundCell { cell: 9 })
    );
    let mut bright = filled();
    let mut too_bright = PairRows::empty();
    too_bright.living[4] = -40.0;
    bright[3].add(&too_bright);
    assert!(matches!(
        assemble(&grid, &bright),
        Err(AssemblePairLightError::UnsoundCell { .. })
    ));
}

/// A packed table reads back to the cells it was packed from, and a malformed one is refused.
#[test]
fn a_table_packs_reads_back_and_refuses_what_is_malformed() {
    let grid = PairGrid::of(Layer::D).expect("D's grid");
    let mut mix = Mix(0x7e57_0017_b0b0_0002);
    let mut cells = vec![CellRows::default(); grid.cells()];
    for cell in &mut cells {
        for _ in 0..3 {
            let mut rows = PairRows::empty();
            for k in 0..AGE_BINS {
                if mix.unit() < 0.3 {
                    rows.living[k] = -8.0 + 20.0 * mix.unit();
                }
                if mix.unit() < 0.05 {
                    rows.changed[k] = if mix.unit() < 0.1 {
                        UNSEEN_MAG
                    } else {
                        -8.0 + 20.0 * mix.unit()
                    };
                }
            }
            cell.add(&rows);
        }
    }
    let table = assemble(&grid, &cells).expect("a table");
    let packed = table.pack();
    let dims = PairLightTable::dimensions_of(&grid, packed.row_count, packed.segments);
    let read = PairLightTable::unpack(&grid, FORMAT, dims, 3, &packed.cell_rows, &packed.rows)
        .expect("reads back");
    assert_eq!(read, PairLightTable::from_cells(&table));
    for cell in (0..grid.cells()).step_by(37) {
        for bin in 0..AGE_BINS {
            assert_eq!(read.living_cmag(cell, bin), table.living[cell][bin]);
            assert_eq!(read.changed_cmag(cell, bin), table.changed[cell][bin]);
            assert_eq!(
                i16::from(read.departing_class(cell, bin)),
                table.departing[cell][bin]
            );
        }
    }
    // Whitespace is ignored; a wrong format, grid or text is refused.
    let mut spaced = String::new();
    for line in packed.rows.as_bytes().chunks(64) {
        spaced.push_str(std::str::from_utf8(line).expect("ASCII"));
        spaced.push('\n');
    }
    assert!(PairLightTable::unpack(&grid, FORMAT, dims, 3, &packed.cell_rows, &spaced).is_ok());
    let unpack = |format: u32, dims: Dimensions, cell_rows: &str, rows: &str| {
        PairLightTable::unpack(&grid, format, dims, 3, cell_rows, rows)
    };
    assert_eq!(
        unpack(2, dims, &packed.cell_rows, &packed.rows),
        Err(ReadPairLightError::WrongFormat)
    );
    let other = PairGrid::of(Layer::C).expect("C's grid");
    let other_dims = PairLightTable::dimensions_of(&other, packed.row_count, packed.segments);
    assert_eq!(
        unpack(FORMAT, other_dims, &packed.cell_rows, &packed.rows),
        Err(ReadPairLightError::WrongDimensions)
    );
    assert_eq!(
        unpack(FORMAT, [0; 7], "", ""),
        Err(ReadPairLightError::WrongDimensions)
    );
    let cut = &packed.rows[..packed.rows.len() - 3];
    assert_eq!(
        unpack(FORMAT, dims, &packed.cell_rows, cut),
        Err(ReadPairLightError::Malformed)
    );
    let bad = format!("!{}", &packed.cell_rows[1..]);
    assert_eq!(
        unpack(FORMAT, dims, &bad, &packed.rows),
        Err(ReadPairLightError::Malformed)
    );
}

/// E's reader brightens every finite value by its read margin and takes each bin's brightest
/// over its read smear, and C's and D's do neither; DARK and UNSEEN stay.
#[test]
fn es_reader_takes_its_read_margin_and_smear() {
    for (layer, margin, smear) in [
        (Layer::C, 0, 0),
        (Layer::D, 0, 0),
        (Layer::E, E_READ_MARGIN_CMAG, E_READ_SMEAR_BINS),
    ] {
        let grid = PairGrid::of(layer).expect("a grid");
        assert_eq!(grid.read_margin_cmag(), margin);
        assert_eq!(grid.read_smear_bins(), smear);
        let mut cells = vec![CellRows::default(); grid.cells()];
        let mut rows = PairRows::empty();
        rows.changed[20] = 1.0;
        rows.living[20] = 1.0;
        rows.living[21] = UNSEEN_MAG;
        for cell in &mut cells {
            cell.add(&rows);
        }
        let read = PairLightTable::from_cells(&assemble(&grid, &cells).expect("a table"));
        let stored = to_cmag(1.0 - MARGIN_MAG).expect("storable");
        // The assembly's dilation holds bins 19-21; the smear takes them `smear` bins further.
        assert_eq!(read.changed_cmag(0, 20), stored - margin);
        assert_eq!(read.living_cmag(0, 20), stored - margin);
        assert_eq!(read.changed_cmag(0, 21 + smear), stored - margin);
        assert_eq!(read.changed_cmag(0, 22 + smear), DARK_CMAG);
        assert_eq!(read.living_cmag(0, 22 + smear), UNSEEN_CMAG);
        assert_eq!(read.changed_cmag(0, 40), DARK_CMAG);
    }
}

/// No star on the cooling fits (0.08–0.1 M☉) is brighter than its cooling floor, over random
/// masses, metallicities and ages, and the floor rises with age, as the fits dim; DARK stays
/// DARK and a fainter value is brightened to the floor.
#[test]
fn no_cooling_star_is_brighter_than_its_floor() {
    let mut mix = Mix(0x7e57_0017_b0b0_0003);
    for _ in 0..20_000 {
        let m = 0.08 + 0.02 * mix.unit();
        let fe_h = FE_H_EDGES[0] + (FE_H_EDGES[FE_H_CELLS] - FE_H_EDGES[0]) * mix.unit();
        let age = math::exp10(9.0 + 1.17 * mix.unit()) * mix.unit();
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let state = substellar::cooling(SolarMasses::new(m), Years::new(age), &comp)
            .expect("inside the fits");
        // A star cooler than the photometry's tables has no V, and is no brighter than any floor.
        let Some(v) = absolute_magnitude_v(&state).map(Magnitudes::value) else {
            continue;
        };
        let grid = PairGrid::of(Layer::E).expect("E's grid");
        let fe_cell = grid.cell_parts(grid.cell_of(20.0, 1.0, fe_h, 10.0).expect("a cell"))[2];
        let bin = age_bin(age).expect("a bin");
        let floor = from_cmag(cooling_floor_cmag(fe_cell, bin));
        assert!(
            floor <= v - MARGIN_MAG + 1e-9,
            "{m} {fe_h} {age}: {v} against {floor}"
        );
    }
    for f in 0..FE_H_CELLS {
        for k in 1..AGE_BINS {
            assert!(
                cooling_floor_cmag(f, k - 1) <= cooling_floor_cmag(f, k),
                "{f} {k}"
            );
        }
    }
    assert_eq!(with_cooling_floor(DARK_CMAG, 0, 30), DARK_CMAG);
    assert_eq!(with_cooling_floor(-500, 0, 30), -500);
    assert_eq!(with_cooling_floor(2_000, 0, 30), cooling_floor_cmag(0, 30));
    assert!(reads_cooling_floor(0.099) && !reads_cooling_floor(0.1));
}

/// The fitted tables have their grids' shapes (the readers' shapes): every cell sampled, the
/// open bin of every cell no brighter than a closer one, and living wherever something changed.
#[test]
fn the_fitted_tables_have_their_grids_shapes() {
    for grid in grids() {
        let table = PairLightTable::generator(grid.layer())
            .unwrap_or_else(|| panic!("layer {:?}'s fitted table", grid.layer()));
        assert_eq!(table.grid(), &grid);
        assert!(table.samples_per_cell() >= CORNER_SAMPLES);
        for cell in 0..grid.cells() {
            let p = grid.cell_parts(cell)[3];
            for bin in 0..AGE_BINS {
                let changed = table.changed_cmag(cell, bin);
                assert!(table.living_cmag(cell, bin) <= changed, "{cell} {bin}");
                assert!(table.departing_at_least(cell, bin) <= table.samples_per_cell());
                if p > 0 {
                    assert!(table.changed_cmag(cell - 1, bin) <= changed, "{cell} {bin}");
                }
            }
        }
    }
}

// --- The tables against pairs of their own ----------------------------------------------------

/// The seed of the slow test's own draws, independent of [`FIT_SEED`].
const TEST_SEED: u64 = 0x7e57_0017_b0b0_5eed;

/// How a layer's pairs compare with its table.
#[derive(Debug, Default)]
struct Bounded {
    pairs: u64,
    /// Pairs whose own cell exists.
    read: u64,
    /// Bins where a pair holds a changed star.
    changed_bins: u64,
    /// Bins where a pair is brighter than its table, or holds what the table says is dark.
    violations: Vec<String>,
    /// The least of each changed bin's table value less the pair's, before the margin, mag:
    /// negative where the margin was needed.
    least_room: f64,
    /// Pairs with a star below 0.1 M☉, on the cooling fits, which the engine lets accrete its
    /// companion's wind (BSE equation 6), and those of them holding a changed star at some age
    /// (R06.T8.g's science check, 2026-10-07, for P11.T17.c's `Detached`).
    light_member_pairs: u64,
    light_member_changed: u64,
    /// Pairs on which the engine's own debug assertions fired (release builds, the fit's and
    /// the game's, skip them): plan 11's finding, recorded in its Risks.
    engine_panics: Vec<String>,
}

impl Bounded {
    fn check(&mut self, table: &PairLightTable, input: &BinaryInput, label: &str) {
        self.pairs += 1;
        let grid = table.grid();
        let [m1, m2] = input.masses().map(SolarMasses::value);
        let (heavier, lighter) = if m1 >= m2 { (m1, m2) } else { (m2, m1) };
        let fe_h = super::super::reach::fe_h_of(input.composition());
        let p = input.orbit().periapsis().value() / SOLAR_RADIUS_M;
        let Some(cell) = grid.cell_of(heavier, lighter, fe_h, p) else {
            return;
        };
        self.read += 1;
        let Ok(rows) = std::panic::catch_unwind(|| pair_rows(input, &sky)) else {
            self.engine_panics.push(format!(
                "{label}: {heavier:.4} + {lighter:.4} M_sun, [Fe/H] {fe_h:.3}, periastron \
                 {p:.4e} R_sun, e {:.3}",
                input.orbit().eccentricity().value()
            ));
            return;
        };
        if lighter < crate::stellar::sse::MIN_INITIAL_MASS.value() {
            self.light_member_pairs += 1;
            self.light_member_changed += u64::from(rows.changes());
        }
        for bin in 0..AGE_BINS {
            let (living, changed) = (rows.living[bin], rows.changed[bin]);
            let (t_living, t_changed) = (
                from_cmag(table.living_cmag_for(cell, bin, lighter)),
                from_cmag(table.changed_cmag_for(cell, bin, lighter)),
            );
            if changed < f64::INFINITY {
                self.changed_bins += 1;
                if changed < UNSEEN_MAG && t_changed < UNSEEN_MAG {
                    let margin = MARGIN_MAG + f64::from(grid.read_margin_cmag()) / 100.0;
                    self.least_room = self.least_room.min(changed - (t_changed + margin));
                }
            }
            if living < t_living || changed < t_changed {
                self.violations.push(format!(
                    "{label}: {heavier:.4} + {lighter:.4} M_sun, [Fe/H] {fe_h:.3}, periastron \
                     {p:.4e} R_sun, e {:.3}, cell {cell}, bin {bin}: living {living} against \
                     {t_living}, changed {changed} against {t_changed}",
                    input.orbit().eccentricity().value()
                ));
            }
        }
    }

    fn merge(&mut self, other: Self) {
        self.pairs += other.pairs;
        self.read += other.read;
        self.changed_bins += other.changed_bins;
        self.violations.extend(other.violations);
        self.least_room = self.least_room.min(other.least_room);
        self.engine_panics.extend(other.engine_panics);
        self.light_member_pairs += other.light_member_pairs;
        self.light_member_changed += other.light_member_changed;
    }
}

/// Checks `inputs` against `table` in eight shares (in turn on wasm32-wasip1).
fn bounded(table: &PairLightTable, inputs: &[BinaryInput], label: &str) -> Bounded {
    const SHARES: usize = 8;
    let share = |k: usize| {
        let mut b = Bounded {
            least_room: f64::INFINITY,
            ..Bounded::default()
        };
        for input in inputs.iter().skip(k).step_by(SHARES) {
            b.check(table, input, label);
        }
        b
    };
    #[cfg(not(target_family = "wasm"))]
    let parts: Vec<Bounded> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..SHARES)
            .map(|k| {
                let share = &share;
                scope.spawn(move || share(k))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a share's thread"))
            .collect()
    });
    #[cfg(target_family = "wasm")]
    let parts: Vec<Bounded> = (0..SHARES).map(share).collect();
    let mut total = Bounded {
        least_room: f64::INFINITY,
        ..Bounded::default()
    };
    for part in parts {
        total.merge(part);
    }
    total
}

/// P11.T17.b's slow test: each layer's table bounds, at every age bin, `per_layer` pairs sampled
/// as the fit samples them but on [`TEST_SEED`] (each at a random cell, at an R₄ point far past
/// the fit's, with draws of their own), and `per_layer` of the generator's own pairs near the Sun
/// whose heavier star lies in the layer's band: no living star brighter than the table's living
/// value, and no changed star brighter than its changed value, DARK included.
#[test]
#[ignore = "slow: 6 × 10⁴ pairs through the binary engine, each walked"]
fn the_pair_light_tables_bound_evolved_pairs() {
    let per_layer = 10_000;
    let galaxy = milky_way(TEST_SEED);
    let mut violations = 0;
    for (salt, grid) in (0_u64..).zip(grids()) {
        let table = PairLightTable::generator(grid.layer()).expect("a fitted table");
        let mut mix = Mix(TEST_SEED ^ salt);
        let cells = u64::try_from(grid.cells()).expect("a few thousand cells");
        let sampled: Vec<BinaryInput> = (0..per_layer)
            .map(|_| {
                let cell = usize::try_from(mix.word() % cells).expect("a cell");
                let index = 1_000 + u32::try_from(mix.word() % 100_000).expect("small");
                sample_input(&grid, cell, index, TEST_SEED)
            })
            .collect();
        let mut realised = Vec::new();
        for layer in [Layer::C, Layer::D, Layer::E] {
            realised.extend(
                generated_pairs(&galaxy, layer, per_layer, TEST_SEED ^ salt ^ 0x9e11)
                    .into_iter()
                    .map(|(input, _)| input)
                    .filter(|input| {
                        let [a, b] = input.masses().map(SolarMasses::value);
                        let band = MassBand::of_layer(grid.layer());
                        (band.lo()..=band.hi()).contains(&a.max(b))
                    }),
            );
        }
        realised.truncate(per_layer);
        for (label, inputs) in [("sampled", &sampled), ("realised", &realised)] {
            let b = bounded(table, inputs, label);
            #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³")]
            let share = b.changed_bins as f64 / (b.read.max(1) * AGE_BINS as u64) as f64;
            println!(
                "layer {:?}, {label}: {} pairs, {} read; changed bins {} ({share:.4}); least \
                 room {:.4} mag before the margin; {} violations; {} engine panics; \
                 {} pairs with a star below 0.1 M_sun, {} of them changed",
                grid.layer(),
                b.pairs,
                b.read,
                b.changed_bins,
                b.least_room,
                b.violations.len(),
                b.engine_panics.len(),
                b.light_member_pairs,
                b.light_member_changed
            );
            for line in b.violations.iter().take(20) {
                println!("  {line}");
            }
            for line in &b.engine_panics {
                println!("  the engine's assertion fired: {line}");
            }
            violations += b.violations.len();
            // The engine's debug assertion fires for about 1 in 3.9 × 10⁴ of E's samples; a rise
            // would shrink what the test covers, so it fails past a few.
            assert!(
                b.engine_panics.len() <= 5,
                "the engine's assertion fired {} times",
                b.engine_panics.len()
            );
        }
    }
    assert_eq!(violations, 0, "bins the tables do not bound");
}

// --- The lazily built values ---------------------------------------------------------------------

/// FNV-1a over 64 bits, for a digest of a reader's values.
struct Fnv(u64);

impl Fnv {
    fn add(&mut self, value: i16) {
        for byte in value.to_le_bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// The cooling floor and each layer's decoded reader are pinned bit for bit (the determinism
/// audit): the floor's values, and per layer a digest of every cell's living, changed and class
/// values as the reader gives them, with three cells' rows in full.
#[test]
fn the_cooling_floor_and_the_readers_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let joined = |row: &[i16]| row.iter().map(i16::to_string).collect::<Vec<_>>().join(" ");
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    for f in 0..FE_H_CELLS {
        let row: Vec<i16> = (0..AGE_BINS).map(|k| cooling_floor_cmag(f, k)).collect();
        w.line(&format!(
            "cooling floor, metallicity interval {f}: {}",
            joined(&row)
        ));
    }
    for grid in grids() {
        let table = PairLightTable::generator(grid.layer()).expect("a fitted table");
        let mut digest = Fnv(0xcbf2_9ce4_8422_2325);
        for cell in 0..grid.cells() {
            for bin in 0..AGE_BINS {
                digest.add(table.living_cmag(cell, bin));
                digest.add(table.changed_cmag(cell, bin));
                digest.add(i16::from(table.departing_class(cell, bin)));
            }
        }
        let layer = grid.layer();
        w.u64_hex(&format!("layer {layer:?} read values"), digest.0);
        for cell in [0, grid.cells() / 2 + 3, grid.cells() - 1] {
            let read = |f: &dyn Fn(usize) -> i16| (0..AGE_BINS).map(f).collect::<Vec<i16>>();
            let living = read(&|bin| table.living_cmag(cell, bin));
            let changed = read(&|bin| table.changed_cmag(cell, bin));
            let classes = read(&|bin| i16::from(table.departing_class(cell, bin)));
            w.line(&format!(
                "layer {layer:?} cell {cell} living: {}",
                joined(&living)
            ));
            w.line(&format!(
                "layer {layer:?} cell {cell} changed: {}",
                joined(&changed)
            ));
            w.line(&format!(
                "layer {layer:?} cell {cell} classes: {}",
                joined(&classes)
            ));
        }
    }
    golden!("stellar/pair_light_tables", w.as_str());
}

/// The lazily built values are what fresh builds give: the cooling floor, and each layer's
/// decoded table against a fresh decoding of the same fitted constants.
#[test]
fn the_lazy_values_are_fresh_builds() {
    use crate::tables::binary_pair_light_e as e;
    use crate::tables::{binary_pair_light_c as c, binary_pair_light_d as d};

    for f in 0..FE_H_CELLS {
        for k in 0..AGE_BINS {
            let v = cooling_brightest(f, k);
            let fresh = to_cmag(if v.is_finite() {
                v - MARGIN_MAG
            } else {
                UNSEEN_MAG
            });
            assert_eq!(Some(cooling_floor_cmag(f, k)), fresh, "{f} {k}");
        }
    }
    let fitted = [
        (
            c::FORMAT,
            c::DIMENSIONS,
            c::SAMPLES_PER_CELL,
            c::CELL_ROWS,
            c::ROWS,
        ),
        (
            d::FORMAT,
            d::DIMENSIONS,
            d::SAMPLES_PER_CELL,
            d::CELL_ROWS,
            d::ROWS,
        ),
        (
            e::FORMAT,
            e::DIMENSIONS,
            e::SAMPLES_PER_CELL,
            e::CELL_ROWS,
            e::ROWS,
        ),
    ];
    for (grid, (format, dims, samples, cell_rows, rows)) in grids().iter().zip(fitted) {
        let fresh = PairLightTable::unpack(grid, format, dims, samples, cell_rows, rows)
            .expect("a fitted table reads");
        assert_eq!(PairLightTable::generator(grid.layer()), Some(&fresh));
    }
}

/// A cell of more samples than a stored count holds is refused.
#[test]
fn a_cell_of_too_many_samples_is_refused() {
    let grid = PairGrid::of(Layer::C).expect("C's grid");
    let mut cells = vec![CellRows::default(); grid.cells()];
    for cell in &mut cells {
        cell.add(&PairRows::empty());
    }
    cells[5].samples = 40_000;
    assert_eq!(
        assemble(&grid, &cells),
        Err(AssemblePairLightError::TooManySamples { cell: 5 })
    );
}

/// A count row holding a class past 32, which `count_class` never gives, is malformed.
#[test]
fn a_count_class_past_32_is_malformed() {
    let grid = PairGrid::of(Layer::C).expect("C's grid");
    let mut cells = vec![CellRows::default(); grid.cells()];
    for cell in &mut cells {
        cell.add(&PairRows::empty());
    }
    let mut table = assemble(&grid, &cells).expect("a table");
    table.departing[0][0] = 40;
    let packed = table.pack();
    let dims = PairLightTable::dimensions_of(&grid, packed.row_count(), packed.segments());
    assert_eq!(
        PairLightTable::unpack(&grid, FORMAT, dims, 1, packed.cell_rows(), packed.rows()),
        Err(ReadPairLightError::Malformed)
    );
    table.departing[0][0] = -1;
    let packed = table.pack();
    let dims = PairLightTable::dimensions_of(&grid, packed.row_count(), packed.segments());
    assert_eq!(
        PairLightTable::unpack(&grid, FORMAT, dims, 1, packed.cell_rows(), packed.rows()),
        Err(ReadPairLightError::Malformed)
    );
}
