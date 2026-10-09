//! `run binary_pair_light_c`, `binary_pair_light_d` and `binary_pair_light_e`: plan 11's
//! pair-light tables (P11.T17.b; `decision-p11-t16-hierarchy-bound.md` §§5, 6 and 10), what a
//! pair of two stars can hold at each age, measured on the binary engine, which the sim's
//! `stellar::binary::pair_light::PairLightTable` reads for the census's pair verdicts (P11.T17.c).
//!
//! For every cell of a layer's grid (`pair_light::PairGrid`) the task draws the sim's own samples
//! (`pair_light::sample_input`, under `pair_light::FIT_SEED`), runs each through the binary
//! engine and walks its timeline (`pair_light::pair_rows`, with the sky's photometry), and sums
//! them per cell. The first `samples_per_cell` make a validation table (`pair_light::assemble`);
//! the next `held_out_per_cell` of every cell are held out and checked against it, bin by bin. The
//! committed table then takes every sample of each cell, so that it bounds every sample drawn.
//! The table depends on no galaxy.
//!
//! The cells are mapped one a chunk; each cell's samples are summed by brightest values and
//! counts, which no order changes, so the table is the same for any thread count. One task a
//! layer, as R06.T5.d's `sky_binary_light`, so that each table stays under the repository's
//! 500 kB. The task's constants are the sim's; the manifest repeats them, and the task refuses one
//! that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::id::Layer;
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::binary::BinaryInput;
use hyperion_sim::stellar::binary::pair_light::{
    AGE_BINS, AGE_STEP_DEX, CORNER_SAMPLES, CellRows, ETA_EXTREME, FAINT_FROM_MAG,
    FAINT_TOLERANCE_SLOPE, FE_H_EDGES, FIRST_AGE_EDGE_YEARS, FIT_SEED, FORMAT, HIGH_ECCENTRICITY,
    LAST_AGE_YEARS, LOWEST_COMPANION_MSUN, MARGIN_MAG, MASS_CELLS, MAX_SAMPLED_ECCENTRICITY,
    MERGE_TOLERANCE_MAG, OPEN_PERIASTRON_REACH_RSUN, PERIASTRON_FIRST_RSUN, PERIASTRON_STEP_DEX,
    PackedPairLight, PairGrid, PairLightCells, PairLightTable, PairRows, Q_EDGES,
    SAMPLES_PER_PHASE, SEGMENT_SPLITS, UNSEEN_MAG, assemble, from_cmag, pair_rows, sample_input,
};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The most samples a cell may take, held out included: a stored count's limit is far above it.
pub const MAX_SAMPLES_PER_CELL: u32 = 4_096;

/// The characters of a packed text's line in the table file.
const LINE_CHARS: usize = 96;

/// A sample's rows as the fit reads them: through the sky's photometry
/// (`sky::photometry::absolute_v_of_state`, R06's interims for protostars and white dwarfs).
#[must_use]
pub fn rows(input: &BinaryInput) -> PairRows {
    pair_rows(input, &absolute_v_of_state)
}

/// The layer a task fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FitLayer {
    /// Layer C, 0.75–2.5 M☉.
    C,
    /// Layer D, 2.5–8 M☉.
    D,
    /// Layer E, 8–150 M☉.
    E,
}

impl FitLayer {
    /// The sim's layer.
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::C => Layer::C,
            Self::D => Layer::D,
            Self::E => Layer::E,
        }
    }

    /// The layer's grid.
    ///
    /// # Panics
    ///
    /// Never: each of C, D and E has a table.
    #[must_use]
    pub fn grid(self) -> PairGrid {
        PairGrid::of(self.layer()).expect("C, D and E have tables")
    }

    /// The fingerprint's probe cells, as [mass, mass ratio, metallicity, periastron] intervals:
    /// a light, a middle and a heavy primary of the layer, each at a modest and a near-equal mass
    /// ratio, at a close and a wide periastron, at the disc's metallicity: twelve pairs, which
    /// span the channels from mergers and common envelopes to pairs the pre-test passes over.
    #[must_use]
    pub const fn probe_cells() -> [[usize; 4]; 12] {
        let mut cells = [[0; 4]; 12];
        let masses = [1, 6, 11];
        let ratios = [2, 6];
        let periastra = [3, 9];
        let mut k = 0;
        while k < 12 {
            cells[k] = [masses[k / 4], ratios[(k / 2) % 2], 2, periastra[k % 2]];
            k += 1;
        }
        cells
    }
}

/// How many samples each cell takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sampling {
    samples_per_cell: u32,
    held_out_per_cell: u32,
    cell_stride: u32,
}

impl Sampling {
    /// `samples_per_cell` samples of each sampled cell for the validation table, then
    /// `held_out_per_cell` held out, every `cell_stride`-th cell sampled (1 for every cell; the
    /// smoke run samples a few, the others taking one sample of nothing).
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if a count is zero, or the two sample counts together pass
    /// [`MAX_SAMPLES_PER_CELL`] (overflow included).
    pub fn new(
        samples_per_cell: u32,
        held_out_per_cell: u32,
        cell_stride: u32,
    ) -> Result<Self, ManifestParamError> {
        for (key, n) in [
            ("samples_per_cell", samples_per_cell),
            ("held_out_per_cell", held_out_per_cell),
            ("cell_stride", cell_stride),
        ] {
            if n == 0 {
                return Err(ManifestParamError::new(key, "a count, at least 1"));
            }
        }
        if samples_per_cell
            .checked_add(held_out_per_cell)
            .is_none_or(|n| n > MAX_SAMPLES_PER_CELL)
        {
            return Err(ManifestParamError::new(
                "samples_per_cell",
                "with held_out_per_cell, at most 4,096",
            ));
        }
        Ok(Self {
            samples_per_cell,
            held_out_per_cell,
            cell_stride,
        })
    }

    /// The manifest's counts, checked ([`Sampling::new`]).
    fn of_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let count = |key: &'static str| {
            manifest
                .u64(key)
                .ok()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| ManifestParamError::new(key, "a count, at least 1"))
        };
        Self::new(
            count("samples_per_cell")?,
            count("held_out_per_cell")?,
            count("cell_stride")?,
        )
    }

    /// The samples of the validation table, a cell's first.
    #[must_use]
    pub const fn samples_per_cell(&self) -> u32 {
        self.samples_per_cell
    }

    /// The samples held out after them, checked against the validation table.
    #[must_use]
    pub const fn held_out_per_cell(&self) -> u32 {
        self.held_out_per_cell
    }

    /// Every `cell_stride`-th cell is sampled.
    #[must_use]
    pub const fn cell_stride(&self) -> u32 {
        self.cell_stride
    }

    /// Every sample of a sampled cell, held out included.
    #[must_use]
    pub const fn every_sample(&self) -> u32 {
        self.samples_per_cell + self.held_out_per_cell
    }

    /// Whether `cell` is sampled.
    #[must_use]
    pub fn samples(&self, cell: usize) -> bool {
        u32::try_from(cell).is_ok_and(|c| c.is_multiple_of(self.cell_stride))
    }
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// The sampling.
    pub sampling: Sampling,
    /// The grid's cells.
    pub cells: usize,
    /// The cells sampled.
    pub sampled_cells: usize,
    /// The samples drawn, held out included.
    pub samples: u64,
    /// The samples that hold a changed star at some age.
    pub changing: u64,
    /// The held-out samples' bins that hold a changed star.
    pub held_out_changed_bins: u64,
    /// The held-out samples' bins the validation table does not bound: a living or changed star
    /// brighter than its value, or one living where it says none does.
    pub violations: u64,
    /// The held-out samples' changed bins that their own cell's samples do not bound: what the
    /// cumulation over wider orbits, the dilation and the margin cover.
    pub beyond_own_cell: u64,
    /// The least of a held-out changed star's V less its validation value with the margin taken
    /// back, mag: negative where the margin was needed.
    pub least_room_mag: f64,
    /// The committed table's cells with a changed star at some age.
    pub changed_cells: usize,
    /// The committed table's brightest changed value, mag.
    pub brightest_changed_mag: f64,
    /// The packed table's distinct rows, segments and characters.
    pub packed: (usize, usize, usize),
}

/// One layer's fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerFit {
    /// The committed table, every sample of each cell.
    pub cells: PairLightCells,
    /// It packed.
    pub packed: PackedPairLight,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// One cell's samples: its index, the validation table's samples summed, those held out, and how
/// many of all hold a changed star at some age.
type CellSamples = (usize, CellRows, Vec<PairRows>, u64);

/// The samples of `cell` of `grid` under `sampling`.
#[must_use]
fn cell_samples(grid: &PairGrid, cell: usize, sampling: &Sampling) -> CellSamples {
    let mut fitted = CellRows::default();
    let mut held = Vec::new();
    let mut changing = 0;
    if sampling.samples(cell) {
        for index in 0..sampling.samples_per_cell {
            let sample = rows(&sample_input(grid, cell, index, FIT_SEED));
            changing += u64::from(sample.changes());
            fitted.add(&sample);
        }
        let held_out =
            sampling.samples_per_cell..sampling.samples_per_cell + sampling.held_out_per_cell;
        for index in held_out {
            let sample = rows(&sample_input(grid, cell, index, FIT_SEED));
            changing += u64::from(sample.changes());
            held.push(sample);
        }
    } else {
        fitted.add(&PairRows::empty());
    }
    (cell, fitted, held, changing)
}

/// How the held-out samples compare with the validation table.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Validation {
    changed_bins: u64,
    violations: u64,
    beyond_own_cell: u64,
    least_room_mag: f64,
}

/// Checks each cell's held-out samples, `held[cell]`, against the validation table `table`, as it
/// is stored (without a layer's reading, so that the figures are the fit's own), and against the
/// cell's own samples, `own[cell]`.
#[must_use]
fn validate(table: &PairLightCells, own: &[CellRows], held: &[Vec<PairRows>]) -> Validation {
    let mut v = Validation {
        changed_bins: 0,
        violations: 0,
        beyond_own_cell: 0,
        least_room_mag: f64::INFINITY,
    };
    for (cell, samples) in held.iter().enumerate() {
        for sample in samples {
            for bin in 0..AGE_BINS {
                let (living, changed) = (sample.living_mag()[bin], sample.changed_mag()[bin]);
                let t_living = from_cmag(table.stored_living_cmag(cell, bin));
                let t_changed = from_cmag(table.stored_changed_cmag(cell, bin));
                if living < t_living || changed < t_changed {
                    v.violations += 1;
                }
                if changed < f64::INFINITY {
                    v.changed_bins += 1;
                    if changed < own[cell].changed_mag()[bin] {
                        v.beyond_own_cell += 1;
                    }
                    if changed < UNSEEN_MAG && t_changed < UNSEEN_MAG {
                        v.least_room_mag = v.least_room_mag.min(changed - (t_changed + MARGIN_MAG));
                    }
                }
            }
        }
    }
    v
}

/// The table of `layer` from `sampling`'s samples, on `threads` threads: the same for any thread
/// count. Progress goes to standard error every twentieth of the cells.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// - If a cell's samples cannot be stored (a magnitude that is not finite, or one brighter than
///   −20.48), which the sim's photometry rules out.
/// - In debug builds only, where the binary engine's debug assertion in `rlof.rs` fires (about 1
///   in 3.9 × 10⁴ of E's samples, a deferred engine finding, `deferred-corrections.md`): run the
///   fit from a release build, as `hyperion-fit run` is.
pub fn fit(
    layer: FitLayer,
    sampling: &Sampling,
    threads: NonZeroUsize,
) -> Result<LayerFit, BuildThreadPoolError> {
    let grid = layer.grid();
    let n = grid.cells();
    let mut fitted = vec![CellRows::default(); n];
    let mut all = vec![CellRows::default(); n];
    let mut held: Vec<Vec<PairRows>> = vec![Vec::new(); n];
    let (mut done, mut report, mut changing) = (0_usize, n.div_ceil(20), 0_u64);
    map_reduce_chunks(
        u64::try_from(n).expect("a few thousand cells"),
        1,
        threads,
        |range| {
            range
                .map(|c| {
                    let cell = usize::try_from(c).expect("a cell index fits in usize");
                    cell_samples(&grid, cell, sampling)
                })
                .collect::<Vec<_>>()
        },
        |part| {
            for (cell, rows, held_out, changes) in part {
                changing += changes;
                let mut every = rows.clone();
                for sample in &held_out {
                    every.add(sample);
                }
                all[cell] = every;
                fitted[cell] = rows;
                held[cell] = held_out;
                done += 1;
            }
            if done >= report {
                eprintln!(
                    "binary_pair_light ({:?}): {done} of {n} cells",
                    layer.layer()
                );
                report += n.div_ceil(20);
            }
        },
    )?;
    let validation = assemble(&grid, &fitted).expect("the samples' magnitudes are storable");
    let checked = validate(&validation, &fitted, &held);
    let cells = assemble(&grid, &all).expect("the samples' magnitudes are storable");
    let packed = cells.pack();
    // The table's own figures, as stored: a layer's reading is the reader's, not the fit's.
    let changed_cells = (0..n)
        .filter(|&cell| {
            (0..AGE_BINS).any(|bin| from_cmag(cells.stored_changed_cmag(cell, bin)) < f64::INFINITY)
        })
        .count();
    let brightest_changed_mag = (0..n)
        .flat_map(|cell| (0..AGE_BINS).map(move |bin| (cell, bin)))
        .map(|(cell, bin)| from_cmag(cells.stored_changed_cmag(cell, bin)))
        .fold(f64::INFINITY, f64::min);
    let samples_per_cell = u64::from(sampling.every_sample());
    let sampled_cells = (0..n).filter(|&cell| sampling.samples(cell)).count();
    let chars = packed.rows().len() + packed.cell_rows().len();
    let figures = Figures {
        sampling: *sampling,
        cells: n,
        sampled_cells,
        samples: samples_per_cell * u64::try_from(sampled_cells).expect("a few thousand cells"),
        changing,
        held_out_changed_bins: checked.changed_bins,
        violations: checked.violations,
        beyond_own_cell: checked.beyond_own_cell,
        least_room_mag: checked.least_room_mag,
        changed_cells,
        brightest_changed_mag,
        packed: (packed.row_count(), packed.segments(), chars),
    };
    Ok(LayerFit {
        cells,
        packed,
        figures,
    })
}

/// The task's parameters: the sim's constants and the sampling, which its manifest must repeat.
///
/// # Panics
///
/// Never: the counts are small, and the seed is below 2⁶³.
#[must_use]
pub fn expected_params(layer: FitLayer, sampling: &Sampling) -> toml::Table {
    let floats = |values: &[f64]| {
        toml::Value::Array(values.iter().map(|&v| toml::Value::Float(v)).collect())
    };
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let grid = layer.grid();
    let mut table = toml::Table::new();
    table.insert(
        "samples_per_cell".to_owned(),
        toml::Value::Integer(i64::from(sampling.samples_per_cell)),
    );
    table.insert(
        "held_out_per_cell".to_owned(),
        toml::Value::Integer(i64::from(sampling.held_out_per_cell)),
    );
    table.insert(
        "cell_stride".to_owned(),
        toml::Value::Integer(i64::from(sampling.cell_stride)),
    );
    table.insert("mass_cells".to_owned(), count(MASS_CELLS));
    table.insert("q_edges".to_owned(), floats(&Q_EDGES));
    table.insert(
        "lowest_companion".to_owned(),
        toml::Value::Float(LOWEST_COMPANION_MSUN),
    );
    table.insert("fe_h_edges".to_owned(), floats(&FE_H_EDGES));
    table.insert(
        "periastron_first_rsun".to_owned(),
        toml::Value::Float(PERIASTRON_FIRST_RSUN),
    );
    table.insert(
        "periastron_step_dex".to_owned(),
        toml::Value::Float(PERIASTRON_STEP_DEX),
    );
    table.insert(
        "periastron_cells".to_owned(),
        count(grid.periastron_cells()),
    );
    table.insert(
        "open_periastron_reach_rsun".to_owned(),
        toml::Value::Float(OPEN_PERIASTRON_REACH_RSUN),
    );
    table.insert(
        "first_age_edge_years".to_owned(),
        toml::Value::Float(FIRST_AGE_EDGE_YEARS),
    );
    table.insert("age_step_dex".to_owned(), toml::Value::Float(AGE_STEP_DEX));
    table.insert(
        "last_age_years".to_owned(),
        toml::Value::Float(LAST_AGE_YEARS),
    );
    table.insert("age_bins".to_owned(), count(AGE_BINS));
    table.insert("margin_mag".to_owned(), toml::Value::Float(MARGIN_MAG));
    table.insert(
        "merge_tolerance_mag".to_owned(),
        toml::Value::Float(MERGE_TOLERANCE_MAG),
    );
    table.insert(
        "living_merge_tolerance_mag".to_owned(),
        toml::Value::Float(grid.living_merge_base_mag()),
    );
    table.insert(
        "faint_from_mag".to_owned(),
        toml::Value::Float(FAINT_FROM_MAG),
    );
    table.insert(
        "faint_tolerance_slope".to_owned(),
        toml::Value::Float(FAINT_TOLERANCE_SLOPE),
    );
    table.insert(
        "corner_samples".to_owned(),
        toml::Value::Integer(i64::from(CORNER_SAMPLES)),
    );
    table.insert(
        "samples_per_phase".to_owned(),
        toml::Value::Integer(i64::from(SAMPLES_PER_PHASE)),
    );
    table.insert(
        "segment_splits".to_owned(),
        toml::Value::Integer(i64::from(SEGMENT_SPLITS)),
    );
    table.insert(
        "high_eccentricity".to_owned(),
        toml::Value::Float(HIGH_ECCENTRICITY),
    );
    table.insert(
        "max_sampled_eccentricity".to_owned(),
        toml::Value::Float(MAX_SAMPLED_ECCENTRICITY),
    );
    table.insert("eta_extreme".to_owned(), toml::Value::Float(ETA_EXTREME));
    table.insert(
        "fit_seed".to_owned(),
        toml::Value::Integer(i64::try_from(FIT_SEED).expect("the seed is below 2^63")),
    );
    table
}

/// `text` as a Rust string literal of lines of [`LINE_CHARS`] characters: base64 needs no escape.
#[must_use]
fn string_literal(text: &str) -> String {
    let mut out = String::from("\"\\\n");
    for line in text.as_bytes().chunks(LINE_CHARS) {
        out.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        out.push('\n');
    }
    out.push('"');
    out
}

/// The table's body: its format, dimensions and samples per cell, then the packed cell rows and
/// rows.
#[must_use]
fn body(fit: &LayerFit) -> String {
    // Writing into a `String` cannot fail, so `writeln!`'s results are dropped here.
    let grid = fit.cells.grid();
    let dims = PairLightTable::dimensions_of(grid, fit.packed.row_count(), fit.packed.segments());
    let mut out = String::new();
    let _ = writeln!(
        out,
        "/// The table's format (`stellar::binary::pair_light::FORMAT`): the packed rows below.\n\
         pub const FORMAT: u32 = {FORMAT};\n"
    );
    let _ = writeln!(
        out,
        "/// The table's dimensions: mass, mass-ratio, metallicity and periastron intervals, age\n\
         /// bins, distinct rows and their segments.\n\
         pub const DIMENSIONS: [usize; 7] = {dims:?};\n"
    );
    let _ = writeln!(
        out,
        "/// The samples of each cell.\n\
         pub const SAMPLES_PER_CELL: u32 = {};\n",
        fit.cells.samples_per_cell()
    );
    out.push_str(
        "/// Per cell (mass-major, the periastron fastest), the indices of its living, changed and\n\
         /// count-class rows among [`ROWS`], three base64 characters each (RFC 4648, §4).\n",
    );
    let _ = writeln!(
        out,
        "pub static CELL_ROWS: &str = {};\n",
        string_literal(fit.packed.cell_rows())
    );
    out.push_str(
        "/// The distinct rows of 53 age bins, each as segments of a run of bins (6 bits) and a\n\
         /// value (12 bits, two's complement: hundredths of a magnitude rounded brighter, 2047\n\
         /// where nothing lives and 2046 where something lives that has no V, or a count's class),\n\
         /// three base64 characters a segment.\n",
    );
    let _ = writeln!(
        out,
        "pub static ROWS: &str = {};",
        string_literal(fit.packed.rows())
    );
    out
}

/// The table's contents.
#[must_use]
pub fn render(layer: FitLayer, fit: &LayerFit) -> RustTable {
    let f = &fit.figures;
    let grid = fit.cells.grid();
    let notes = format!(
        "\
{cells} cells: {m} intervals of the heavier star's mass even in ln m over layer {l:?}'s band, by
{q} of the mass ratio (the first from {lo} M☉ ÷ the band's top), by {z} of log₁₀(`Z_fit` ÷ 0.02),
by {p} of the drawn periastron ({step} dex from {p0} R☉, then one open bin sampled to {reach:e}
R☉), by {ages} age bins (one to 10⁵ years, then {astep} dex, to {last:e} years). Each cell takes
{n} samples ({c} at its corners, then R₄ points; eccentricities by Moe and Di Stefano's law and
circular and {hi_e} extremes; the generator's star draws under the fit's seed {seed:#x}, with η
at ±{eta}σ and the stripped mark and low kick mode set for some), each through the binary engine
to {last:e} years and walked at {spp} parts a phase and five points a part. Per age bin a cell
holds the brightest V of any living star, pair-evolved or its own model, and of any departing
star or product, each cumulated over wider periastra, dilated by one cell on every axis and one
age bin, brightened by {margin} mag, merged within {tol} mag (the living within {ltol}) of a
run's brightest, in hundredths of a magnitude rounded brighter; and the class of its own samples
holding a changed star. Validated on {h} held-out samples a cell against the table of the first
{fitn}; the table takes all.",
        cells = f.cells,
        m = MASS_CELLS,
        l = layer.layer(),
        q = Q_EDGES.len() - 1,
        lo = LOWEST_COMPANION_MSUN,
        z = FE_H_EDGES.len() - 1,
        p = grid.periastron_cells(),
        step = PERIASTRON_STEP_DEX,
        p0 = PERIASTRON_FIRST_RSUN,
        reach = OPEN_PERIASTRON_REACH_RSUN,
        ages = AGE_BINS,
        astep = AGE_STEP_DEX,
        last = LAST_AGE_YEARS,
        n = f.sampling.every_sample(),
        c = CORNER_SAMPLES,
        hi_e = HIGH_ECCENTRICITY,
        seed = FIT_SEED,
        eta = ETA_EXTREME,
        spp = SAMPLES_PER_PHASE,
        margin = MARGIN_MAG,
        tol = MERGE_TOLERANCE_MAG,
        ltol = grid.living_merge_base_mag(),
        h = f.sampling.held_out_per_cell,
        fitn = f.sampling.samples_per_cell,
    );
    RustTable {
        summary: vec![
            format!(
                "Plan 11's pair-light table of layer {:?} (P11.T17.b): what a pair of two stars",
                layer.layer()
            ),
            "can hold at each age, measured on the binary engine, which".to_owned(),
            "`stellar::binary::pair_light::PairLightTable` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task of one layer: P11.T17.b, slow, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BinaryPairLightTask {
    /// The layer it fits.
    pub layer: FitLayer,
}

/// Layer C's task.
pub static C_TASK: BinaryPairLightTask = BinaryPairLightTask { layer: FitLayer::C };

/// Layer D's task.
pub static D_TASK: BinaryPairLightTask = BinaryPairLightTask { layer: FitLayer::D };

/// Layer E's task.
pub static E_TASK: BinaryPairLightTask = BinaryPairLightTask { layer: FitLayer::E };

impl FitTask for BinaryPairLightTask {
    fn name(&self) -> &'static str {
        match self.layer {
            FitLayer::C => "binary_pair_light_c",
            FitLayer::D => "binary_pair_light_d",
            FitLayer::E => "binary_pair_light_e",
        }
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        match self.layer {
            FitLayer::C => "binary_pair_light_c.rs",
            FitLayer::D => "binary_pair_light_d.rs",
            FitLayer::E => "binary_pair_light_e.rs",
        }
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "FORMAT",
            "DIMENSIONS",
            "SAMPLES_PER_CELL",
            "CELL_ROWS",
            "ROWS",
        ]
    }

    /// The grid's cells, and at each of [`FitLayer::probe_cells`], one sample through the binary
    /// engine (a dozen pairs, sample 20 of each cell, an R₄ point): the sum of its finite living
    /// and changed magnitudes, its changed bins and its bins living with no V.
    fn fingerprint(&self) -> SimFingerprint {
        let grid = self.layer.grid();
        #[expect(clippy::cast_precision_loss, reason = "a few thousand cells")]
        let mut probes = vec![("cells".to_owned(), grid.cells() as f64)];
        for parts in FitLayer::probe_cells() {
            let cell = grid.cell_index(parts);
            let sample = rows(&sample_input(&grid, cell, 20, FIT_SEED));
            let finite = |row: &[f64; AGE_BINS]| {
                row.iter()
                    .filter(|v| v.is_finite() && **v < UNSEEN_MAG)
                    .fold(0.0, |sum, v| sum + v)
            };
            let count = |row: &[f64; AGE_BINS], unseen: bool| {
                let n = row
                    .iter()
                    .filter(|v| {
                        if unseen {
                            v.total_cmp(&UNSEEN_MAG).is_eq()
                        } else {
                            v.is_finite()
                        }
                    })
                    .count();
                f64::from(u32::try_from(n).expect("53 bins at most"))
            };
            let at = format!("cell({parts:?})");
            probes.push((format!("{at}.living"), finite(sample.living_mag())));
            probes.push((format!("{at}.changed"), finite(sample.changed_mag())));
            probes.push((
                format!("{at}.changed_bins"),
                count(sample.changed_mag(), false),
            ));
            probes.push((
                format!("{at}.unseen_bins"),
                count(sample.living_mag(), true),
            ));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let sampling = Sampling::of_manifest(manifest)?;
        manifest.expect_params(&expected_params(self.layer, &sampling))?;
        let fit = fit(self.layer, &sampling, threads)?;
        let f = &fit.figures;
        let (rows, segments, chars) = f.packed;
        Ok(TaskOutput {
            table: render(self.layer, &fit),
            source: "the generator's own binary engine (plan 11: Hurley, Tout and Pols 2002, \
                     MNRAS 329, 897, BSE; Moe and Di Stefano 2017, ApJS 230, 15, for the \
                     eccentricities) on plan 06's tracks (Hurley, Pols and Tout 2000, MNRAS 315, \
                     543), through R06's photometry (`sky::photometry::absolute_v_of_state`)"
                .to_owned(),
            acceptance: format!(
                "{} cells, {} sampled, {} samples ({} + {} held out a cell), {} holding a \
                 changed star; held out: {} of {} changed bins beyond their own cell's samples, \
                 {} bins not bounded by the table of the first {}, the least room {:.3} mag \
                 before the {MARGIN_MAG} mag margin; {} cells with a changed star, the brightest \
                 {:.2} mag; packed {rows} rows, {segments} segments, {chars} characters",
                f.cells,
                f.sampled_cells,
                f.samples,
                f.sampling.samples_per_cell,
                f.sampling.held_out_per_cell,
                f.changing,
                f.beyond_own_cell,
                f.held_out_changed_bins,
                f.violations,
                f.sampling.samples_per_cell,
                f.least_room_mag,
                f.changed_cells,
                f.brightest_changed_mag,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smoke run samples a few cells of every step, the same on one thread and on three, and
    /// renders the items the sim reads, which read back.
    #[test]
    fn the_smoke_run_is_the_same_on_any_thread_count_and_renders_its_items() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("manifests/binary_pair_light_c.smoke.toml");
        let manifest = Manifest::load(&path).expect("the committed smoke manifest loads");
        let output = C_TASK
            .run(&manifest, NonZeroUsize::MIN)
            .expect("the smoke run runs");
        let again = C_TASK
            .run(&manifest, NonZeroUsize::new(3).expect("three"))
            .expect("the smoke run runs");
        assert_eq!(output, again);
        let TableItem::Source(body) = &output.table.items[0] else {
            panic!("one verbatim item")
        };
        for item in [
            "pub const FORMAT: u32 = 1;",
            "pub const DIMENSIONS: [usize; 7] = [12, 8, 4, 16, 53, ",
            "pub const SAMPLES_PER_CELL: u32 = 1;",
            "pub static CELL_ROWS: &str = \"\\\n",
            "pub static ROWS: &str = \"\\\n",
        ] {
            assert!(body.contains(item), "{item}");
        }
        assert!(
            output
                .acceptance
                .starts_with("6144 cells, 13 sampled, 26 samples")
        );
    }

    /// A smoke fit's packed text reads back as the sim reads its table.
    #[test]
    fn a_fit_reads_back_as_the_sim_reads_it() {
        let sampling = Sampling::new(1, 1, 997).expect("small counts");
        let fit = fit(FitLayer::E, &sampling, NonZeroUsize::new(2).expect("two")).expect("a fit");
        let grid = FitLayer::E.grid();
        let dims =
            PairLightTable::dimensions_of(&grid, fit.packed.row_count(), fit.packed.segments());
        let read = PairLightTable::unpack(
            &grid,
            FORMAT,
            dims,
            fit.cells.samples_per_cell(),
            fit.packed.cell_rows(),
            fit.packed.rows(),
        )
        .expect("reads back");
        assert_eq!(read, PairLightTable::from_cells(&fit.cells));
        assert_eq!(fit.figures.sampled_cells, grid.cells().div_ceil(997));
    }

    /// Each layer's manifests repeat the sim's constants.
    #[test]
    fn the_manifests_repeat_the_sims_constants() {
        for task in [&C_TASK, &D_TASK, &E_TASK] {
            for suffix in ["", ".smoke"] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("manifests/{}{suffix}.toml", task.name()));
                let manifest = Manifest::load(&path).expect("the committed manifest loads");
                assert_eq!(manifest.task(), task.name());
                let sampling = Sampling::of_manifest(&manifest).expect("well-formed counts");
                manifest
                    .expect_params(&expected_params(task.layer, &sampling))
                    .expect("the manifest's parameters are the code's");
                if suffix.is_empty() {
                    assert_eq!(sampling.cell_stride, 1, "{}", task.name());
                }
            }
        }
    }

    /// A sampling of a zero count, or of sample counts past 4,096 together, overflow included, is
    /// refused.
    #[test]
    fn a_sampling_out_of_range_is_refused() {
        assert!(Sampling::new(64, 8, 1).is_ok());
        assert!(Sampling::new(4_000, 96, 1).is_ok());
        for (s, h, c) in [
            (0, 8, 1),
            (64, 0, 1),
            (64, 8, 0),
            (4_000, 97, 1),
            (u32::MAX, 2, 1),
        ] {
            assert!(Sampling::new(s, h, c).is_err(), "{s} {h} {c}");
        }
        let manifest = |text: &str| {
            Manifest::from_bytes(
                std::path::Path::new("sampling.toml"),
                format!("task = \"binary_pair_light_c\"\n[params]\n{text}\n").into_bytes(),
            )
            .expect("a manifest")
        };
        let of = |text: &str| Sampling::of_manifest(&manifest(text));
        assert!(of("samples_per_cell = 64\nheld_out_per_cell = 8\ncell_stride = 1").is_ok());
        assert!(of("samples_per_cell = 64\nheld_out_per_cell = 8").is_err());
        assert!(
            of("samples_per_cell = 4294967295\nheld_out_per_cell = 2\ncell_stride = 1").is_err()
        );
        assert!(of("samples_per_cell = 64\nheld_out_per_cell = 8\ncell_stride = -1").is_err());
    }

    #[test]
    fn the_probe_cells_lie_inside_every_grid() {
        for layer in [FitLayer::C, FitLayer::D, FitLayer::E] {
            let grid = layer.grid();
            for parts in FitLayer::probe_cells() {
                assert!(grid.cell_index(parts) < grid.cells());
            }
        }
    }

    #[test]
    fn a_long_text_is_a_string_literal_of_short_lines() {
        let text = "A".repeat(200);
        let literal = string_literal(&text);
        assert!(literal.starts_with("\"\\\n"));
        assert!(literal.ends_with("\n\""));
        assert_eq!(literal.lines().count(), 5);
        assert!(literal.lines().all(|l| l.len() <= LINE_CHARS + 2));
    }
}
