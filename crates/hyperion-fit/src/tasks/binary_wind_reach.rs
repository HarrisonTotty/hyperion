//! `run binary_wind_reach`: plan 11's wind reach table (P11.T12's fix, held at 29), what bounds the
//! wind a star of any mass and metallicity interval can lose by an age as a wide pair's walk reads
//! it, which the census's pair bound (`stellar::binary::pair_light_bound`) reads for its `Detached`
//! from generator version 29.
//!
//! The table depends on no galaxy, only on the generator's tracks and binary parameters, so it is
//! fitted once and checked in. Each mass node of `reach::mass_node_msun` is the sim's own
//! `wind_reach::wind_reach_node` at every metallicity node of `reach::fe_h_node`, built in
//! parallel; `wind_reach::assemble` then takes each cell's largest over its nodes and rounds it
//! up, so the table is a pure function of the tracks.
//!
//! The task's parameters are the sim's constants. The manifest records them so that the inputs
//! hash covers them, and the task refuses a manifest that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::stellar::binary::reach::{
    AGE_BINS, AGE_SPREAD, AGE_STEP_DEX, CELLS, ETA_DRAWS, FE_H_CELLS, FE_H_NODES,
    FE_H_SUBDIVISIONS, FIRST_AGE_EDGE_YEARS, HIGHEST_MASS_MSUN, LOWEST_MASS_MSUN, MASS_CELLS,
    MASS_SUBDIVISIONS, fe_h_node, mass_node_msun,
};
use hyperion_sim::stellar::binary::wind_reach::{
    AGB_MARGIN, NO_WIND_CENTIDEX, SUM_MARGIN_DEX, WindReachCells, WindReachNode, assemble,
    centidex_to_rsun2_per_msun, wind_reach_node,
};
use hyperion_sim::stellar::fates::STRIPPED_WINDOW;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The masses, M☉, of the fingerprint's probe nodes: a star on the cooling fits, a red dwarf, the
/// Sun and two of the stars whose asymptotic giant branches feed the wide pairs, the
/// companion-stripped window's edges and two massive stars.
pub const FINGERPRINT_MASSES_MSUN: [f64; 9] = [0.09, 0.4, 1.0, 1.6, 3.0, 5.5, 11.0, 25.0, 100.0];

/// The metallicity coordinates of the fingerprint's probe nodes: the tracks' metal-poor clamp,
/// solar and the metal-rich clamp.
pub const FINGERPRINT_FE_H: [f64; 3] = [FE_H_NODES[0], 0.0, FE_H_NODES[FE_H_CELLS]];

/// The fitted table, in the table's units.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryWindReachFit {
    /// The cells.
    pub cells: WindReachCells,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// Samples: mass nodes × metallicity nodes.
    pub samples: usize,
    /// The largest stored Bondi–Hoyle sum, R☉² M☉⁻¹.
    pub largest_sum_rsun2_per_msun: f64,
    /// The cells that lose no wind by the last bin.
    pub windless: usize,
    /// The cells some of whose samples reach their asymptotic giant branch.
    pub reaching_the_agb: usize,
    /// The largest stored AGB loss, M☉.
    pub largest_agb_lost_msun: f64,
    /// The largest stored dust radius, R☉.
    pub largest_dust_rsun: f64,
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
/// If a node holds a sum the table cannot store, or a track gives a sum that is not a number
/// (`wind_reach_node`), which the tracks rule out.
pub fn fit(threads: NonZeroUsize) -> Result<BinaryWindReachFit, BuildThreadPoolError> {
    let masses = MASS_CELLS * MASS_SUBDIVISIONS + 1;
    let metallicities = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
    let n = u64::try_from(masses).expect("some four hundred nodes");
    let mut nodes: Vec<WindReachNode> = Vec::with_capacity(masses * metallicities);
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .flat_map(|k| {
                    let k = usize::try_from(k).expect("a node index fits in usize");
                    let m = mass_node_msun(k);
                    (0..metallicities).map(move |j| wind_reach_node(m, fe_h_node(j)))
                })
                .collect::<Vec<_>>()
        },
        |chunk| nodes.extend(chunk),
    )?;
    let cells = assemble(&nodes).expect("the tracks' sums are storable");
    let last = |row: &[i16; AGE_BINS]| row[AGE_BINS - 1];
    let largest = cells
        .log_sum_centidex
        .iter()
        .map(last)
        .max()
        .unwrap_or(NO_WIND_CENTIDEX);
    let figures = Figures {
        samples: nodes.len(),
        largest_sum_rsun2_per_msun: centidex_to_rsun2_per_msun(largest),
        windless: cells
            .log_sum_centidex
            .iter()
            .filter(|row| last(row) == NO_WIND_CENTIDEX)
            .count(),
        reaching_the_agb: cells
            .agb_start_years
            .iter()
            .filter(|t| t.is_finite())
            .count(),
        largest_agb_lost_msun: cells.agb_lost_msun.iter().copied().fold(0.0, f64::max),
        largest_dust_rsun: cells.agb_dust_rsun.iter().copied().fold(0.0, f64::max),
    };
    Ok(BinaryWindReachFit { cells, figures })
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
        "sum_margin_dex".to_owned(),
        toml::Value::Float(SUM_MARGIN_DEX),
    );
    table.insert("agb_margin".to_owned(), toml::Value::Float(AGB_MARGIN));
    table
}

/// The table's body: the sums as a `static`, one cell a line, too large for a `const`, then each
/// cell's AGB figures.
fn body(fit: &BinaryWindReachFit) -> String {
    // Writing into a `String` cannot fail, so `writeln!`'s results are dropped here.
    let cells = &fit.cells;
    let mut out = String::new();
    out.push_str(
        "/// Per cell (mass interval `k`, metallicity interval `j` at `k × 7 + j`), per age bin\n\
         /// (one to 10⁵ years, then 104 of 0.05 dex to 10^10.2 years), the largest Bondi–Hoyle\n\
         /// sum Σ ΔM (R ÷ (`β_W` M))² any star of the cell can have lost by the bin's end as a wide\n\
         /// pair's walk reads it, margin included, in hundredths of a dex of R☉² M☉⁻¹ rounded up;\n\
         /// `i16::MIN` for none.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static LOG_SUM_CENTIDEX: [[i16; {AGE_BINS}]; {}] = [",
        cells.log_sum_centidex.len()
    );
    for row in &cells.log_sum_centidex {
        let values: Vec<String> = row
            .iter()
            .map(|&k| {
                if k == NO_WIND_CENTIDEX {
                    "N".to_owned()
                } else {
                    k.to_string()
                }
            })
            .collect();
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n\n");
    out.push_str("/// The stored value of no wind.\nconst N: i16 = i16::MIN;\n\n");
    push_column(
        &mut out,
        "/// Per cell, the earliest age, years, at which a star of the cell may be on its\n\
         /// asymptotic giant branch; `f64::INFINITY` where none reaches it.\n",
        "AGB_START_YEARS",
        &cells.agb_start_years,
    );
    out.push('\n');
    push_column(
        &mut out,
        "/// Per cell, the most mass, M☉, a star of the cell may lose on its asymptotic giant\n\
         /// branch, margin included.\n",
        "AGB_LOST_MSUN",
        &cells.agb_lost_msun,
    );
    out.push('\n');
    push_column(
        &mut out,
        "/// Per cell, the least mass, M☉, of a star of the cell on its asymptotic giant branch,\n\
         /// margin included; `f64::INFINITY` where none reaches it.\n",
        "AGB_LEAST_MSUN",
        &cells.agb_least_msun,
    );
    out.push('\n');
    push_column(
        &mut out,
        "/// Per cell, the largest dust radius, R☉, of a star of the cell on its asymptotic giant\n\
         /// branch, margin included.\n",
        "AGB_DUST_RSUN",
        &cells.agb_dust_rsun,
    );
    out
}

/// One per-cell column `name` of `values`, under its doc comment `doc`, each value on a line of
/// its own, `f64::INFINITY` for +∞.
fn push_column(out: &mut String, doc: &str, name: &str, values: &[f64]) {
    // Writing into a `String` cannot fail, so `writeln!`'s results are dropped.
    out.push_str(doc);
    out.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(out, "pub static {name}: [f64; {}] = [", values.len());
    for &v in values {
        if v.is_finite() {
            let _ = writeln!(out, "    {},", literal(v));
        } else {
            out.push_str("    f64::INFINITY,\n");
        }
    }
    out.push_str("];\n");
}

/// The table's contents.
#[must_use]
pub fn render(fit: &BinaryWindReachFit) -> RustTable {
    let notes = format!(
        "\
{cells} cells: the reach table's, {m} mass intervals even in ln m over {lo}–{hi} M☉ by {z}
intervals of log₁₀(`Z_fit` ÷ 0.02). Each cell's stars are sampled at {ms} masses and {zs}
metallicities, edges included, each at {e} Reimers η draws (from η = 0 to +7σ), a full track at
the median draws but η and the companion-stripped mark (never set, and over {w0}–{w1} M☉ also
always set), walked from age 0 to its last living age in the wide pairs' steps (two between each
of its mass knots), with the generator's binary parameters. Each bin holds the largest sum over
the steps that start before its upper age × {spread}, made non-decreasing in age, plus {margin}
dex, rounded up to the hundredth of a dex. Each cell also keeps its samples' earliest start of an
AGB step ÷ {spread}, and their most AGB loss, least AGB mass and largest dust radius, each widened
by {agb}. The cooling fits below 0.1 M☉ lose no wind.",
        cells = CELLS,
        m = MASS_CELLS,
        lo = LOWEST_MASS_MSUN,
        hi = HIGHEST_MASS_MSUN,
        z = FE_H_CELLS,
        ms = MASS_SUBDIVISIONS + 1,
        zs = FE_H_SUBDIVISIONS + 1,
        e = ETA_DRAWS.len(),
        spread = AGE_SPREAD,
        margin = SUM_MARGIN_DEX,
        agb = AGB_MARGIN,
        w0 = STRIPPED_WINDOW.0,
        w1 = STRIPPED_WINDOW.1,
    );
    RustTable {
        summary: vec![
            "Plan 11's wind reach table (P11.T12's fix): what bounds the wind any star".to_owned(),
            "of a mass and metallicity interval can have lost by an age as a wide pair's"
                .to_owned(),
            "walk reads it, which `stellar::binary::wind_reach::WindReachTable` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The finite stand-in of a probe: `v`, or 0 where it is not finite.
fn finite_or_zero(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// The task (P11.T12's fix, held at 29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BinaryWindReachTask;

impl FitTask for BinaryWindReachTask {
    fn name(&self) -> &'static str {
        "binary_wind_reach"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "binary_wind_reach.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "LOG_SUM_CENTIDEX",
            "AGB_START_YEARS",
            "AGB_LOST_MSUN",
            "AGB_LEAST_MSUN",
            "AGB_DUST_RSUN",
        ]
    }

    /// The grid's counts and ends, and, at [`FINGERPRINT_MASSES_MSUN`] and each of
    /// [`FINGERPRINT_FE_H`], each node's largest sum, the sum of its bins' finite log sums and its
    /// AGB figures: some 300 walked tracks, which `just ci`'s fit-check can afford.
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
            let node = wind_reach_node(m, fe_h);
            let finite = node
                .log_sum_rsun2_per_msun
                .iter()
                .copied()
                .filter(|v| v.is_finite());
            let largest = finite.clone().fold(f64::NEG_INFINITY, f64::max);
            let sum = finite.fold(0.0, |a, v| a + v);
            let at = format!("wind_reach_node({m} M☉, {fe_h} dex)");
            probes.push((format!("{at}.largest"), finite_or_zero(largest)));
            probes.push((format!("{at}.sum"), sum));
            probes.push((
                format!("{at}.agb_start"),
                finite_or_zero(node.agb_start_years),
            ));
            probes.push((format!("{at}.agb_lost"), node.agb_lost_msun));
            probes.push((
                format!("{at}.agb_least"),
                finite_or_zero(node.agb_least_msun),
            ));
            probes.push((format!("{at}.agb_dust"), node.agb_dust_rsun));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        manifest.expect_params(&expected_params())?;
        let fit = fit(threads)?;
        let f = fit.figures;
        Ok(TaskOutput {
            table: render(&fit),
            source: "the generator's own tracks (plan 06: Hurley, Pols and Tout 2000, MNRAS 315, \
                     543; above 100 M☉ P06.T14's factors fitted to Yusof et al. 2013, MNRAS 433, \
                     1114), walked as plan 11's wide pairs' wind walks them (P11.T12's fix: \
                     Hurley, Tout and Pols 2002, MNRAS 329, 897, equations 6 and 9; the dust \
                     radius of Höfner 2007, ASP Conf. Ser. 378, 145, as Abate et al. 2013, A&A \
                     552, A26, take it)"
                .to_owned(),
            acceptance: format!(
                "{} cells × {} age bins from {} samples; the largest stored Bondi–Hoyle sum \
                 {:.4e} R☉² M☉⁻¹; {} cells lose no wind by 10^10.2 years; {} cells reach the \
                 asymptotic giant branch, the most AGB loss {:.3} M☉ and the largest dust radius \
                 {:.1} R☉, margins included; the sim's `light::tests` tallies hold no `Detached` \
                 pair wind-fed under the 29 arm",
                CELLS,
                AGE_BINS,
                f.samples,
                f.largest_sum_rsun2_per_msun,
                f.windless,
                f.reaching_the_agb,
                f.largest_agb_lost_msun,
                f.largest_dust_rsun,
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
        let fit = BinaryWindReachFit {
            cells: WindReachCells {
                log_sum_centidex: vec![[NO_WIND_CENTIDEX; AGE_BINS], [250; AGE_BINS]],
                agb_start_years: vec![f64::INFINITY, 1.5e9],
                agb_lost_msun: vec![0.0, 0.5],
                agb_least_msun: vec![f64::INFINITY, 0.6],
                agb_dust_rsun: vec![0.0, 900.0],
            },
            figures: Figures {
                samples: 0,
                largest_sum_rsun2_per_msun: 1.0,
                windless: 1,
                reaching_the_agb: 1,
                largest_agb_lost_msun: 0.5,
                largest_dust_rsun: 900.0,
            },
        };
        let TableItem::Source(body) = &render(&fit).items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains(&format!(
            "pub static LOG_SUM_CENTIDEX: [[i16; {AGE_BINS}]; 2] = ["
        )));
        assert!(body.contains("    [N,N,"));
        assert!(body.contains("    [250,250,"));
        assert!(body.contains("const N: i16 = i16::MIN;"));
        for name in [
            "AGB_START_YEARS",
            "AGB_LOST_MSUN",
            "AGB_LEAST_MSUN",
            "AGB_DUST_RSUN",
        ] {
            assert!(
                body.contains(&format!("pub static {name}: [f64; 2] = [")),
                "{name}"
            );
        }
        assert!(body.contains("    1_500_000_000.0,\n"));
        assert!(body.contains("    f64::INFINITY,\n"));
        assert!(body.contains("    900.0,\n"));
    }

    /// The manifest's parameters are the sim's constants.
    #[test]
    fn the_manifest_repeats_the_sims_constants() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("manifests/binary_wind_reach.toml");
        let manifest = Manifest::load(&path).expect("the committed manifest loads");
        assert_eq!(manifest.task(), "binary_wind_reach");
        manifest
            .expect_params(&expected_params())
            .expect("the manifest's parameters are the code's");
    }
}
