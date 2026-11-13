//! `run binary_cooling_floor`: plan 11's cooling floor (P11.T17.b's first widening at read time),
//! the brightest V a star on P06.T13's cooling fits of 0.08–0.1 M☉ can have in each metallicity
//! interval and age bin of the pair-light tables, margin included, which the sim's
//! `stellar::binary::pair_light::cooling_floor_cmag` reads for a pair whose lighter star lies below
//! 0.1 M☉ (`PairLightTable::living_cmag_for` and `changed_cmag_for`).
//!
//! The table depends on no galaxy, only on the cooling fits and the photometry, so it is fitted
//! once and checked in (decided 2026-10-11, `decision-test-coverage.md` §3.2, N7): the sim built
//! it lazily before, some 10⁶ evaluations of the fits and 2.6 s in a test build, in every test
//! process that read it. Each value is the sim's own `pair_light::build_cooling_floor_cmag`, so the
//! table is a pure function of the fits, and the sim's fast test `the_lazy_values_are_fresh_builds`
//! holds the committed table to a fresh build on every run, natively and on wasm32-wasip1.
//!
//! The task's parameters are the sim's constants. The manifest records them so that the inputs
//! hash covers them, and the task refuses a manifest that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::stellar::binary::pair_light::{
    AGE_BINS, AGE_STEP_DEX, COOLING_FLOOR_AGES, COOLING_FLOOR_FE_H, COOLING_FLOOR_MASSES,
    FE_H_CELLS, FE_H_EDGES, FIRST_AGE_EDGE_YEARS, LAST_AGE_YEARS, LOWEST_COMPANION_MSUN,
    MARGIN_MAG, UNSEEN_CMAG, build_cooling_floor_cmag, cooling_brightest_mag, from_cmag,
};
use hyperion_sim::stellar::substellar;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The age bins of the fingerprint's probes, in each metallicity interval: the first, where the
/// fits are brightest, two on the fits' dimming, and the last.
pub const FINGERPRINT_BINS: [usize; 4] = [0, 12, 30, AGE_BINS - 1];

/// The fitted table: per metallicity interval, per age bin, the floor in hundredths of a
/// magnitude.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryCoolingFloorFit {
    /// The floor, `[fe_cell][bin]`.
    pub cmag: Vec<[i16; AGE_BINS]>,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Figures {
    /// The evaluations of the cooling fits: per value, the masses × metallicities × ages read.
    pub evaluations: u64,
    /// The brightest stored value, hundredths of a magnitude.
    pub brightest_cmag: i16,
    /// The faintest stored value other than [`UNSEEN_CMAG`], hundredths of a magnitude.
    pub faintest_cmag: i16,
    /// The values where no star of the interval and bin has a V.
    pub unseen: usize,
    /// Whether every interval's floor rises with age or holds, as the fits dim.
    pub rises_with_age: bool,
}

/// The table built on `threads` threads, one value a chunk, assembled in order: the same bits for
/// any thread count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If a value is past what the table stores, which the cooling fits' V (some +10 to +16 mag)
/// rules out (`build_cooling_floor_cmag`).
pub fn fit(threads: NonZeroUsize) -> Result<BinaryCoolingFloorFit, BuildThreadPoolError> {
    let n = u64::try_from(FE_H_CELLS * AGE_BINS).expect("a few hundred values");
    let mut values: Vec<i16> = Vec::with_capacity(FE_H_CELLS * AGE_BINS);
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let i = usize::try_from(i).expect("a value's index fits in usize");
                    build_cooling_floor_cmag(i / AGE_BINS, i % AGE_BINS)
                })
                .collect::<Vec<i16>>()
        },
        |chunk| values.extend(chunk),
    )?;
    let (rows, rest) = values.as_chunks::<AGE_BINS>();
    debug_assert!(rest.is_empty(), "whole rows of AGE_BINS values");
    let cmag = rows.to_vec();
    let seen = || values.iter().copied().filter(|&k| k != UNSEEN_CMAG);
    let per_value = u64::from(COOLING_FLOOR_MASSES + 1)
        * u64::from(COOLING_FLOOR_FE_H + 1)
        * u64::from(COOLING_FLOOR_AGES + 1);
    let figures = Figures {
        evaluations: n * per_value,
        brightest_cmag: seen().min().unwrap_or(UNSEEN_CMAG),
        faintest_cmag: seen().max().unwrap_or(UNSEEN_CMAG),
        unseen: values.len() - seen().count(),
        rises_with_age: cmag.iter().all(|row| row.windows(2).all(|w| w[0] <= w[1])),
    };
    Ok(BinaryCoolingFloorFit { cmag, figures })
}

/// The task's parameters: the sim's constants, which its manifest must repeat.
///
/// # Panics
///
/// Never: the counts are a few dozen, which fit in an `i64`.
#[must_use]
pub fn expected_params() -> toml::Table {
    let floats = |values: &[f64]| {
        toml::Value::Array(values.iter().map(|&v| toml::Value::Float(v)).collect())
    };
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let mut table = toml::Table::new();
    table.insert("fe_h_edges".to_owned(), floats(&FE_H_EDGES));
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
    table.insert(
        "lowest_mass".to_owned(),
        toml::Value::Float(LOWEST_COMPANION_MSUN),
    );
    table.insert(
        "highest_mass".to_owned(),
        toml::Value::Float(substellar::MAX_MASS.value()),
    );
    table.insert(
        "mass_steps".to_owned(),
        toml::Value::Integer(i64::from(COOLING_FLOOR_MASSES)),
    );
    table.insert(
        "fe_h_steps".to_owned(),
        toml::Value::Integer(i64::from(COOLING_FLOOR_FE_H)),
    );
    table.insert(
        "age_steps".to_owned(),
        toml::Value::Integer(i64::from(COOLING_FLOOR_AGES)),
    );
    table.insert("margin_mag".to_owned(), toml::Value::Float(MARGIN_MAG));
    table
}

/// The table's body: the floor as a `static`, one metallicity interval a line.
fn body(fit: &BinaryCoolingFloorFit) -> String {
    // Writing into a `String` cannot fail, so `writeln!`'s results are dropped here.
    let mut out = String::new();
    out.push_str(
        "/// Per metallicity interval of `stellar::binary::pair_light::FE_H_EDGES`, per age bin of\n\
         /// `pair_light::age_edge_years`, the brightest V a star on P06.T13's cooling fits of\n\
         /// 0.08–0.1 M☉ can have there, brightened by `pair_light::MARGIN_MAG`, in hundredths of\n\
         /// a magnitude rounded brighter (`pair_light::to_cmag`); `pair_light::UNSEEN_CMAG` where\n\
         /// none has a V.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static COOLING_FLOOR_CMAG: [[i16; {AGE_BINS}]; {}] = [",
        fit.cmag.len()
    );
    for row in &fit.cmag {
        let values: Vec<String> = row.iter().map(i16::to_string).collect();
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n");
    out
}

/// The table's contents.
#[must_use]
pub fn render(fit: &BinaryCoolingFloorFit) -> RustTable {
    let notes = format!(
        "\
{cells} metallicity intervals of log₁₀(`Z_fit` ÷ 0.02) by {bins} age bins, the pair-light
tables' own. Each value reads P06.T13's cooling fits at {masses} masses even over {lo}–{hi} M☉,
{fe_h} metallicities even over the interval and {ages} ages even over the bin, edges included,
through plan 06's photometry, takes the brightest V, brightens it by {margin} mag and rounds it
brighter to the hundredth of a magnitude, as `pair_light::build_cooling_floor_cmag` does.",
        cells = FE_H_CELLS,
        bins = AGE_BINS,
        masses = COOLING_FLOOR_MASSES + 1,
        lo = LOWEST_COMPANION_MSUN,
        hi = substellar::MAX_MASS.value(),
        fe_h = COOLING_FLOOR_FE_H + 1,
        ages = COOLING_FLOOR_AGES + 1,
        margin = MARGIN_MAG,
    );
    RustTable {
        summary: vec![
            "Plan 11's cooling floor (P11.T17.b): the brightest V a star on the cooling".to_owned(),
            "fits can have in each metallicity interval and age bin of the pair-light".to_owned(),
            "tables, which `stellar::binary::pair_light::cooling_floor_cmag` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task (P11.T17.b's cooling floor, fitted since `decision-test-coverage.md` §3.2, N7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BinaryCoolingFloorTask;

impl FitTask for BinaryCoolingFloorTask {
    fn name(&self) -> &'static str {
        "binary_cooling_floor"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "binary_cooling_floor.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["COOLING_FLOOR_CMAG"]
    }

    /// The grid's counts, and in each metallicity interval at each of [`FINGERPRINT_BINS`] the
    /// brightest V on the cooling fits before its margin and rounding (0, a finite stand-in, where
    /// none has a V) and the value the table stores there, so that a change to the fits, to the
    /// margin, to the rounding or to the path of no V names the table stale: some 170,000
    /// evaluations of the fits, which `just ci`'s fit-check can afford.
    fn fingerprint(&self) -> SimFingerprint {
        #[expect(clippy::cast_precision_loss, reason = "a few dozen values")]
        let mut probes = vec![
            ("fe_h_cells".to_owned(), FE_H_CELLS as f64),
            ("age_bins".to_owned(), AGE_BINS as f64),
        ];
        for fe_cell in 0..FE_H_CELLS {
            for bin in FINGERPRINT_BINS {
                let v = cooling_brightest_mag(fe_cell, bin);
                probes.push((
                    format!("cooling_brightest_mag({fe_cell}, {bin})"),
                    if v.is_finite() { v } else { 0.0 },
                ));
                probes.push((
                    format!("build_cooling_floor_cmag({fe_cell}, {bin})"),
                    f64::from(build_cooling_floor_cmag(fe_cell, bin)),
                ));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        manifest.expect_params(&expected_params())?;
        let fit = fit(threads)?;
        let f = fit.figures;
        Ok(TaskOutput {
            table: render(&fit),
            source: "the generator's own cooling fits below 0.1 M☉ (P06.T13: Burrows, Hubbard, \
                     Lunine and Liebert 2001, Rev. Mod. Phys. 73, 719, and Baraffe et al. 2015, \
                     A&A 577, A42) through plan 06's photometry, read as plan 11's pair-light \
                     reader reads them (P11.T17.b)"
                .to_owned(),
            acceptance: format!(
                "{} values ({} metallicity intervals × {} age bins) from {} evaluations of the \
                 cooling fits; stored V {:+.2} to {:+.2} mag, margin included, {} with no V; every \
                 interval's floor {} with age; the sim's `no_cooling_star_is_brighter_than_its_floor` \
                 checks 20,000 random cooling stars against it and `the_lazy_values_are_fresh_builds` \
                 holds it to a fresh build",
                FE_H_CELLS * AGE_BINS,
                FE_H_CELLS,
                AGE_BINS,
                f.evaluations,
                from_cmag(f.brightest_cmag),
                from_cmag(f.faintest_cmag),
                f.unseen,
                if f.rises_with_age {
                    "rises or holds"
                } else {
                    "falls somewhere"
                },
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered body declares the item the sim reads, with its shape.
    #[test]
    fn the_body_declares_the_item_the_sim_reads() {
        let fit = BinaryCoolingFloorFit {
            cmag: vec![[1_027; AGE_BINS], [UNSEEN_CMAG; AGE_BINS]],
            figures: Figures {
                evaluations: 0,
                brightest_cmag: 1_027,
                faintest_cmag: 1_027,
                unseen: AGE_BINS,
                rises_with_age: true,
            },
        };
        let TableItem::Source(body) = &render(&fit).items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains(&format!(
            "pub static COOLING_FLOOR_CMAG: [[i16; {AGE_BINS}]; 2] = ["
        )));
        assert!(body.contains("    [1027,1027,"));
        assert!(body.contains(&format!("    [{UNSEEN_CMAG},")));
    }

    /// The manifest's parameters are the sim's constants.
    #[test]
    fn the_manifest_repeats_the_sims_constants() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("manifests/binary_cooling_floor.toml");
        let manifest = Manifest::load(&path).expect("the committed manifest loads");
        assert_eq!(manifest.task(), "binary_cooling_floor");
        manifest
            .expect_params(&expected_params())
            .expect("the manifest's parameters are the code's");
    }
}
