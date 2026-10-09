//! `run binary_reach`: plan 11's reach table (P11.T17.a), an upper bound on the largest radius
//! any star of a mass and metallicity interval has had by an age, as the binary engine's pre-test
//! reads it, which the sim's `stellar::binary::largest_radius_bound` and the census's pair bound
//! (`stellar::binary::pair_light_bound`) read.
//!
//! The table depends on no galaxy, only on the generator's tracks, so it is fitted once and
//! checked in. Each mass node of `reach::mass_node_msun` is the sim's own `reach::reach_node` at every
//! metallicity node of `reach::fe_h_node`, built in parallel; `reach::assemble` then takes each
//! cell's largest over its nodes and rounds it up, so the table is a pure function of the tracks.
//!
//! The task's parameters are the sim's constants. The manifest records them so that the inputs
//! hash covers them, and the task refuses a manifest that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::stellar::binary::reach::{
    AGE_BINS, AGE_SPREAD, AGE_STEP_DEX, CELLS, DROP_ALLOWANCE_MAX_DEX, ETA_DRAWS, FE_H_CELLS,
    FE_H_NODES, FE_H_SUBDIVISIONS, FIRST_AGE_EDGE_YEARS, HIGHEST_MASS_MSUN, LOWEST_MASS_MSUN,
    MASS_CELLS, MASS_SUBDIVISIONS, RADIUS_MARGIN_DEX, ReachCells, ReachNode, assemble,
    centidex_to_rsun, fe_h_node, mass_node_msun, reach_node,
};
use hyperion_sim::stellar::fates::STRIPPED_WINDOW;
use hyperion_sim::stellar::sse::MIN_INITIAL_MASS;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The masses, M☉, of the fingerprint's probe nodes: a star on the cooling fits, the meeting of
/// the fits and the tracks, a red dwarf, the Sun, a B star, the companion-stripped window's edges
/// and middle, two massive stars and the tracks' top, which P06.T14's factors above 100 M☉ shape.
pub const FINGERPRINT_MASSES_MSUN: [f64; 11] =
    [0.09, 0.1, 0.4, 1.0, 3.0, 5.5, 8.0, 11.0, 25.0, 100.0, 150.0];

/// The metallicity coordinates of the fingerprint's probe nodes: the tracks' metal-poor clamp,
/// solar and the metal-rich clamp.
pub const FINGERPRINT_FE_H: [f64; 3] = [FE_H_NODES[0], 0.0, FE_H_NODES[FE_H_CELLS]];

/// The fitted table, in the table's units.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryReachFit {
    /// The cells.
    pub cells: ReachCells,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// Samples: mass nodes × metallicity nodes.
    pub samples: usize,
    /// Tracks built: one per sample from 0.1 M☉ up and η draw, two inside the companion-stripped
    /// window.
    pub tracks: usize,
    /// The largest stored radius, R☉.
    pub largest_rsun: f64,
    /// The smallest stored radius, R☉.
    pub smallest_rsun: f64,
    /// The cells whose stars may all still be on their main sequence at 10¹⁰ years.
    pub unevolved_at_ten_gyr: usize,
    /// The cells some of whose samples die suddenly.
    pub collapsing: usize,
    /// The lightest mass, M☉, of a cell some of whose samples die suddenly.
    pub lightest_collapsing_msun: f64,
}

/// The table built on `threads` threads, one mass node a chunk, assembled in node order: the same
/// bits for any thread count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If a node holds a radius the table cannot store, which the tracks' radii (under 10⁵ R☉) rule
/// out.
pub fn fit(threads: NonZeroUsize) -> Result<BinaryReachFit, BuildThreadPoolError> {
    let masses = MASS_CELLS * MASS_SUBDIVISIONS + 1;
    let metallicities = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
    let n = u64::try_from(masses).expect("some four hundred nodes");
    let mut nodes: Vec<ReachNode> = Vec::with_capacity(masses * metallicities);
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .flat_map(|k| {
                    let k = usize::try_from(k).expect("a node index fits in usize");
                    let m = mass_node_msun(k);
                    (0..metallicities).map(move |j| reach_node(m, fe_h_node(j)))
                })
                .collect::<Vec<_>>()
        },
        |chunk| nodes.extend(chunk),
    )?;
    let cells = assemble(&nodes).expect("the tracks' radii are storable");
    let stored = cells
        .log_radius_centidex
        .iter()
        .flat_map(|row| row.iter().copied());
    let (smallest, largest) =
        stored.fold((i16::MAX, i16::MIN), |(lo, hi), k| (lo.min(k), hi.max(k)));
    let tracked: usize = (0..masses)
        .map(mass_node_msun)
        .filter(|&m| m >= MIN_INITIAL_MASS.value())
        .map(|m| {
            if (STRIPPED_WINDOW.0..=STRIPPED_WINDOW.1).contains(&m) {
                2
            } else {
                1
            }
        })
        .sum();
    let collapsing = cells
        .earliest_collapse_years
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_finite());
    let lightest_collapsing_msun = collapsing
        .clone()
        .map(|(cell, _)| mass_node_msun((cell / FE_H_CELLS) * MASS_SUBDIVISIONS))
        .fold(f64::INFINITY, f64::min);
    let figures = Figures {
        samples: nodes.len(),
        tracks: tracked * metallicities * ETA_DRAWS.len(),
        largest_rsun: centidex_to_rsun(largest),
        smallest_rsun: centidex_to_rsun(smallest),
        unevolved_at_ten_gyr: cells
            .main_sequence_end_years
            .iter()
            .filter(|&&t| t > 1.0e10)
            .count(),
        collapsing: collapsing.count(),
        lightest_collapsing_msun,
    };
    Ok(BinaryReachFit { cells, figures })
}

/// The task's parameters: the sim's constants, which its manifest must repeat.
///
/// # Panics
///
/// Never: the counts are a few hundred, which fit in an `i64`.
#[must_use]
pub fn expected_params() -> toml::Table {
    let floats = |values: &[f64]| {
        toml::Value::Array(values.iter().map(|&v| toml::Value::Float(v)).collect())
    };
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let mut table = toml::Table::new();
    table.insert("mass_cells".to_owned(), count(MASS_CELLS));
    table.insert(
        "lowest_mass".to_owned(),
        toml::Value::Float(LOWEST_MASS_MSUN),
    );
    table.insert(
        "highest_mass".to_owned(),
        toml::Value::Float(HIGHEST_MASS_MSUN),
    );
    table.insert("mass_subdivisions".to_owned(), count(MASS_SUBDIVISIONS));
    table.insert("fe_h_nodes".to_owned(), floats(&FE_H_NODES));
    table.insert("fe_h_subdivisions".to_owned(), count(FE_H_SUBDIVISIONS));
    table.insert("eta_draws".to_owned(), floats(&ETA_DRAWS));
    table.insert(
        "first_age_edge_years".to_owned(),
        toml::Value::Float(FIRST_AGE_EDGE_YEARS),
    );
    table.insert("age_step_dex".to_owned(), toml::Value::Float(AGE_STEP_DEX));
    table.insert("age_bins".to_owned(), count(AGE_BINS));
    table.insert("age_spread".to_owned(), toml::Value::Float(AGE_SPREAD));
    table.insert(
        "radius_margin_dex".to_owned(),
        toml::Value::Float(RADIUS_MARGIN_DEX),
    );
    table.insert(
        "drop_allowance_max_dex".to_owned(),
        toml::Value::Float(DROP_ALLOWANCE_MAX_DEX),
    );
    table
}

/// The table's body: the radii as a `static`, one cell a line, too large for a `const`, then each
/// cell's earliest end of a main sequence and earliest sudden death.
fn body(fit: &BinaryReachFit) -> String {
    // Writing into a `String` cannot fail, so `writeln!`'s results are dropped here.
    let cells = &fit.cells;
    let mut out = String::new();
    out.push_str(
        "/// Per cell (mass interval `k`, metallicity interval `j` at `k × 7 + j`), per age bin\n\
         /// (one to 10⁵ years, then 104 of 0.05 dex to 10^10.2 years), the largest radius any\n\
         /// star of the cell can have had by the bin's end as the binary engine reads it, margin\n\
         /// included, in hundredths of a dex of R☉ rounded up.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static LOG_RADIUS_CENTIDEX: [[i16; {AGE_BINS}]; {}] = [",
        cells.log_radius_centidex.len()
    );
    for row in &cells.log_radius_centidex {
        let values: Vec<String> = row.iter().map(i16::to_string).collect();
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// Per cell, the earliest age, years, at which a star of the cell may have left its\n\
         /// main sequence and so hold a core; `f64::INFINITY` for the cooling fits.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static MAIN_SEQUENCE_END_YEARS: [f64; {}] = [",
        cells.main_sequence_end_years.len()
    );
    push_years(&mut out, &cells.main_sequence_end_years);
    out.push_str("];\n\n");
    out.push_str(
        "/// Per cell, the earliest age, years, at which a star of the cell may have died\n\
         /// suddenly (a core's collapse, an electron capture or a pair instability), whose kick\n\
         /// could bring a pair's periastron in; `f64::INFINITY` where none does.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static EARLIEST_COLLAPSE_YEARS: [f64; {}] = [",
        cells.earliest_collapse_years.len()
    );
    push_years(&mut out, &cells.earliest_collapse_years);
    out.push_str("];\n");
    out
}

/// Each of `ages` (years) on a line of its own, `f64::INFINITY` for +∞.
fn push_years(out: &mut String, ages: &[f64]) {
    // Writing into a `String` cannot fail, so `writeln!`'s result is dropped.
    for &t in ages {
        if t.is_finite() {
            let _ = writeln!(out, "    {},", literal(t));
        } else {
            out.push_str("    f64::INFINITY,\n");
        }
    }
}

/// The table's contents.
#[must_use]
pub fn render(fit: &BinaryReachFit) -> RustTable {
    let notes = format!(
        "\
{cells} cells: {m} mass intervals even in ln m over {lo}–{hi} M☉ by {z} intervals of
log₁₀(`Z_fit` ÷ 0.02). Each cell's stars are sampled at {ms} masses and {zs} metallicities, edges
included, each at {e} Reimers η draws (from η = 0 to +7σ), a full track at the median draws but η
and the companion-stripped mark (never set, and over {w0}–{w1} M☉ also always set); the cooling
fits at age 0 below 0.1 M☉. Each is read by `max_radius_until` at the engine's age (held to no
earlier than its arrival on the main sequence) of each bin's upper age × {spread}. The cell takes
the largest, with the rise into a drop between neighbouring mass samples (at most {drop} dex),
adds {margin} dex and rounds up to the hundredth of a dex. Each cell also keeps its samples'
earliest end of a main sequence and earliest sudden death, each ÷ {spread}.",
        cells = CELLS,
        m = MASS_CELLS,
        lo = LOWEST_MASS_MSUN,
        hi = HIGHEST_MASS_MSUN,
        z = FE_H_CELLS,
        ms = MASS_SUBDIVISIONS + 1,
        zs = FE_H_SUBDIVISIONS + 1,
        e = ETA_DRAWS.len(),
        spread = AGE_SPREAD,
        margin = RADIUS_MARGIN_DEX,
        drop = DROP_ALLOWANCE_MAX_DEX,
        w0 = STRIPPED_WINDOW.0,
        w1 = STRIPPED_WINDOW.1,
    );
    RustTable {
        summary: vec![
            "Plan 11's reach table (P11.T17.a): the largest radius any star of a mass".to_owned(),
            "and metallicity interval can have had by an age, as the binary engine's".to_owned(),
            "pre-test reads it, which `stellar::binary::reach::ReachTable` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task (P11.T17.a, R06's ask B, `decision-r06-census-cost.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BinaryReachTask;

impl FitTask for BinaryReachTask {
    fn name(&self) -> &'static str {
        "binary_reach"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "binary_reach.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "LOG_RADIUS_CENTIDEX",
            "MAIN_SEQUENCE_END_YEARS",
            "EARLIEST_COLLAPSE_YEARS",
        ]
    }

    /// The grid's counts and ends, and, at [`FINGERPRINT_MASSES_MSUN`] and each of
    /// [`FINGERPRINT_FE_H`], each node's largest radius, the sum of its bins' log radii, its
    /// earliest end of a main sequence and its earliest sudden death: some 400 tracks, which `just
    /// ci`'s fit-check can afford.
    fn fingerprint(&self) -> SimFingerprint {
        #[expect(clippy::cast_precision_loss, reason = "a few hundred nodes")]
        let mut probes = vec![
            ("mass_node_msun(0)".to_owned(), mass_node_msun(0)),
            (
                "mass_node_msun(last)".to_owned(),
                mass_node_msun(MASS_CELLS * MASS_SUBDIVISIONS),
            ),
            ("fe_h_node(1)".to_owned(), fe_h_node(1)),
            ("cells".to_owned(), CELLS as f64),
        ];
        for (m, fe_h) in FINGERPRINT_FE_H
            .iter()
            .flat_map(|&z| FINGERPRINT_MASSES_MSUN.map(|m| (m, z)))
        {
            let node = reach_node(m, fe_h);
            let largest = node
                .log_radius_rsun
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let sum = node.log_radius_rsun.iter().fold(0.0, |a, &v| a + v);
            // A star on the cooling fits never leaves a main sequence; its probe is 0, a finite
            // stand-in.
            let end = if node.main_sequence_end_years.is_finite() {
                node.main_sequence_end_years
            } else {
                0.0
            };
            let collapse = if node.earliest_collapse_years.is_finite() {
                node.earliest_collapse_years
            } else {
                0.0
            };
            let at = format!("reach_node({m} M☉, {fe_h} dex)");
            probes.push((format!("{at}.largest"), largest));
            probes.push((format!("{at}.sum"), sum));
            probes.push((format!("{at}.main_sequence_end"), end));
            probes.push((format!("{at}.earliest_collapse"), collapse));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        manifest.expect_params(&expected_params())?;
        let fit = fit(threads)?;
        let f = fit.figures;
        Ok(TaskOutput {
            table: render(&fit),
            source: "the generator's own tracks and cooling fits (plan 06: Hurley, Pols and Tout \
                     2000, MNRAS 315, 543; above 100 M☉ P06.T14's factors fitted to Yusof et al. \
                     2013, MNRAS 433, 1114; below 0.1 M☉ P06.T13's cooling fits, Burrows, \
                     Hubbard, Lunine and Liebert 2001, Rev. Mod. Phys. 73, 719, and Baraffe et al. \
                     2015, A&A 577, A42), read as plan 11's pre-test reads them (P11.T4.i)"
                .to_owned(),
            acceptance: format!(
                "{} cells × {} age bins from {} samples and {} tracks; stored radii {:.4} to \
                 {:.1} R☉; {} cells may all be on their main sequence at 10¹⁰ years; {} cells \
                 hold a sudden death, the lightest from {:.3} M☉; the slow test \
                 `the_reach_table_bounds_dense_tracks` checks the bound against dense tracks",
                CELLS,
                AGE_BINS,
                f.samples,
                f.tracks,
                f.smallest_rsun,
                f.largest_rsun,
                f.unevolved_at_ten_gyr,
                f.collapsing,
                f.lightest_collapsing_msun,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered body declares its items with the sim's shapes.
    #[test]
    fn the_body_declares_the_items_the_sim_reads() {
        let fit = BinaryReachFit {
            cells: ReachCells {
                log_radius_centidex: vec![[-3; AGE_BINS], [250; AGE_BINS]],
                main_sequence_end_years: vec![1.5e9, f64::INFINITY],
                earliest_collapse_years: vec![f64::INFINITY, 9.0e6],
            },
            figures: Figures {
                samples: 0,
                tracks: 0,
                largest_rsun: 1.0,
                smallest_rsun: 1.0,
                unevolved_at_ten_gyr: 0,
                collapsing: 1,
                lightest_collapsing_msun: 8.0,
            },
        };
        let TableItem::Source(body) = &render(&fit).items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains(&format!(
            "pub static LOG_RADIUS_CENTIDEX: [[i16; {AGE_BINS}]; 2] = ["
        )));
        assert!(body.contains("pub static MAIN_SEQUENCE_END_YEARS: [f64; 2] = ["));
        assert!(body.contains("    [-3,-3,"));
        assert!(body.contains("    1_500_000_000.0,\n"));
        assert!(body.contains("    f64::INFINITY,\n"));
        assert!(body.contains("pub static EARLIEST_COLLAPSE_YEARS: [f64; 2] = ["));
        assert!(body.contains("    9_000_000.0,\n"));
    }

    /// The manifest's parameters are the sim's constants.
    #[test]
    fn the_manifest_repeats_the_sims_constants() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/binary_reach.toml");
        let manifest = Manifest::load(&path).expect("the committed manifest loads");
        assert_eq!(manifest.task(), "binary_reach");
        manifest
            .expect_params(&expected_params())
            .expect("the manifest's parameters are the code's");
    }
}
