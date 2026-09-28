//! The direct companions' period correction (plan 11, ruling 81.3): the factors on Moe and Di
//! Stefano's period law under which the periods of the direct companions that survive rejection
//! follow the law itself, which `hyperion_sim::stellar::multiplicity` draws with.
//!
//! The fit ([`fit`]) starts from a table of ones and, twelve times over, at each row's primary
//! mass, draws the hierarchies of a fixed sample of systems at the Sun-like point of the Milky Way
//! fixture with the table so far, counts the direct companions' periods into the eight bins, and
//! multiplies each bin's factor by its target share over its measured share. The sample and the
//! measurements are the sim's own ([`period_fit`]); the draws are cut into chunks of
//! [`CHUNK`] systems on any number of threads and their counts summed as integers, so the table is
//! the same for every chunking and thread count, and the same as the sim's former ignored test
//! `fit_the_direct_period_correction` bit for bit. It is slow: some 2 × 10⁶ hierarchies.
//!
//! **Not yet registered.** The committed table is still the sim's own constant
//! (`period_fit::COMMITTED`), and its 7 M☉ row is stale at version 13: this fit moves it, and so
//! the generator's output. [`PeriodCorrectionTask`] joins the registry in the commit that runs it
//! into `tables/period_correction.rs` with a generator-version bump (plan 11, P11.T1.d's refit),
//! and the sim then reads the table from there. The sim's slow test
//! `the_period_correction_gives_its_bin_shares` holds the committed table to its shares
//! meanwhile.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::stellar::multiplicity::period_fit::{
    self, CORRECTION_MASSES, CorrectionTable, count_bins, direct_log_periods, direct_tries,
    draw_with, record, shares, target_shares,
};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The iterations of the committed fit.
pub const ITERATIONS: u32 = 12;

/// The systems drawn at each row's mass: 20,000.
pub const SAMPLE: u32 = 20_000;

/// Systems per chunk of the draws. The chunking cannot change the result.
pub const CHUNK: u64 = 250;

/// The systems of each row the fingerprint draws under a table of ones.
pub const FINGERPRINT_SAMPLE: u32 = 100;

/// A row's figures from the fit's last iteration, measured with the table before its last update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowFigures {
    /// The row's primary mass, M☉.
    pub mass: f64,
    /// The largest relative miss of a bin's share from its target.
    pub worst_miss: f64,
    /// The share of the direct companions' tried periods that rejection turned down.
    pub rejected: f64,
    /// The direct companions placed.
    pub placed: u64,
    /// The direct companions dropped once their tries ran out.
    pub dropped: u64,
}

/// The fitted table and how it was made.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodCorrectionFit {
    /// The factors, a row for each of `CORRECTION_MASSES`.
    pub table: CorrectionTable,
    /// The iterations run.
    pub iterations: u32,
    /// The systems drawn at each row's mass.
    pub sample: u32,
    /// Each row's figures from the last iteration; empty for no iterations.
    pub last: Vec<RowFigures>,
}

/// The direct companions' bin counts over the first `sample` systems of mass `m` under `table`.
fn bin_counts(
    galaxy: &Galaxy,
    m: f64,
    sample: u32,
    table: &CorrectionTable,
    threads: NonZeroUsize,
) -> Result<[u64; 8], BuildThreadPoolError> {
    let mut counts = [0_u64; 8];
    map_reduce_chunks(
        u64::from(sample),
        CHUNK,
        threads,
        |items| {
            let mut part = [0_u64; 8];
            for index in items {
                let index = u32::try_from(index).expect("the sample's indices are u32s");
                let h = draw_with(galaxy, &record(galaxy, index, m), table);
                count_bins(&direct_log_periods(&h), &mut part);
            }
            part
        },
        |part| {
            for (sum, add) in counts.iter_mut().zip(part) {
                *sum += add;
            }
        },
    )?;
    Ok(counts)
}

/// The direct companions placed, their tries and those dropped, over the first `sample` systems
/// of mass `m` under `table`.
fn tries(
    galaxy: &Galaxy,
    m: f64,
    sample: u32,
    table: &CorrectionTable,
    threads: NonZeroUsize,
) -> Result<(u64, u64, u64), BuildThreadPoolError> {
    let mut sums = (0_u64, 0_u64, 0_u64);
    map_reduce_chunks(
        u64::from(sample),
        CHUNK,
        threads,
        |items| {
            let mut part = (0_u64, 0_u64, 0_u64);
            for index in items {
                let index = u32::try_from(index).expect("the sample's indices are u32s");
                if let Some((n, t, d)) = direct_tries(galaxy, &record(galaxy, index, m), table) {
                    let n = u64::try_from(n).expect("at most three companions");
                    part = (part.0 + n, part.1 + t, part.2 + u64::from(d));
                }
            }
            part
        },
        |part| sums = (sums.0 + part.0, sums.1 + part.1, sums.2 + part.2),
    )?;
    Ok(sums)
}

/// The fit: `iterations` rounds of `c ← c × target ÷ measured` from a table of ones, over the
/// first `sample` systems at each row's mass, on `threads` threads.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
pub fn fit(
    iterations: u32,
    sample: u32,
    threads: NonZeroUsize,
) -> Result<PeriodCorrectionFit, BuildThreadPoolError> {
    let galaxy = period_fit::galaxy();
    let mut table = [[1.0; 8]; 4];
    let mut last = Vec::new();
    for iteration in 0..iterations {
        let mut figures = Vec::with_capacity(CORRECTION_MASSES.len());
        for (row, m) in CORRECTION_MASSES.into_iter().enumerate() {
            let target = target_shares(m);
            let measured = shares(&bin_counts(&galaxy, m, sample, &table, threads)?);
            let (placed, tried, dropped) = tries(&galaxy, m, sample, &table, threads)?;
            for bin in 0..8 {
                if measured[bin] > 0.0 {
                    table[row][bin] *= target[bin] / measured[bin];
                }
            }
            #[expect(clippy::cast_precision_loss, reason = "counts of a few 10⁴")]
            let rejected = 1.0 - (placed as f64) / (tried as f64);
            figures.push(RowFigures {
                mass: m,
                worst_miss: (0..8)
                    .map(|b| (measured[b] / target[b] - 1.0).abs())
                    .fold(0.0, f64::max),
                rejected,
                placed,
                dropped,
            });
        }
        if iteration + 1 == iterations {
            last = figures;
        }
    }
    Ok(PeriodCorrectionFit {
        table,
        iterations,
        sample,
        last,
    })
}

/// The table's contents: its summary, its notes and its item.
#[must_use]
pub fn render(fit: &PeriodCorrectionFit) -> RustTable {
    let notes = format!(
        "\
Inputs: {iterations} iterations of `c ← c × target ÷ measured` from a table of ones, over
{sample} systems at each row's mass (`stellar::multiplicity::period_fit::record`: the Sun-like
point of the Milky Way fixture, free, first attempt); the targets are the bin shares of Moe
and Di Stefano's eqs. 20–23 law, normalised over log₁₀(P ÷ 1 d) = 0.2–8.",
        iterations = fit.iterations,
        sample = fit.sample,
    );
    let mut out = String::new();
    out.push_str(
        "/// The factors on the direct companions' period law, a row for each primary mass of\n\
         /// `CORRECTION_MASSES` and a column for each period bin 0.2–1, 1–2, …, 7–8.\n",
    );
    out.push_str("pub const PERIOD_CORRECTION: [[f64; 8]; 4] = [\n");
    for row in fit.table {
        out.push_str("    [\n");
        for factor in row {
            let _ = writeln!(out, "        {},", literal(factor));
        }
        out.push_str("    ],\n");
    }
    out.push_str("];");
    RustTable {
        summary: vec![
            "The direct companions' period correction (plan 11, ruling 81.3): the factors under"
                .to_owned(),
            "which the periods that survive rejection follow Moe and Di Stefano's law.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(out)],
    }
}

/// The task: slow, and not yet in the registry (see the module's documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PeriodCorrectionTask;

impl FitTask for PeriodCorrectionTask {
    fn name(&self) -> &'static str {
        "period_correction"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "period_correction.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["PERIOD_CORRECTION"]
    }

    /// At each row's mass, the direct companions of the first [`FINGERPRINT_SAMPLE`] systems under
    /// a table of ones, and the sum of their log periods: the draw that the fit measures.
    fn fingerprint(&self) -> SimFingerprint {
        let galaxy = period_fit::galaxy();
        let ones = [[1.0; 8]; 4];
        let mut probes = Vec::with_capacity(2 * CORRECTION_MASSES.len());
        for m in CORRECTION_MASSES {
            let periods: Vec<f64> = (0..FINGERPRINT_SAMPLE)
                .flat_map(|i| {
                    direct_log_periods(&draw_with(&galaxy, &record(&galaxy, i, m), &ones))
                })
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "a few hundred companions")]
            let count = periods.len() as f64;
            probes.push((format!("direct_companions({m} M☉)"), count));
            probes.push((
                format!("sum_log_period({m} M☉)"),
                periods.iter().sum::<f64>(),
            ));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let iterations = u32::try_from(manifest.u64("iterations")?)
            .map_err(|_| ManifestParamError::new("iterations", "a u32"))?;
        let sample = u32::try_from(manifest.u64("sample")?)
            .map_err(|_| ManifestParamError::new("sample", "a u32"))?;
        if iterations == 0 || sample == 0 {
            return Err(ManifestParamError::new("iterations and sample", "at least 1").into());
        }
        let fit = fit(iterations, sample, threads)?;
        let mut acceptance = format!(
            "after {} iterations over {} systems a row, the worst bin's relative miss of its share \
             before the last update:",
            fit.iterations, fit.sample
        );
        for row in &fit.last {
            let _ = write!(
                acceptance,
                " {:.4} at {} M☉ ({:.2}% of tries rejected, {} of {} direct companions dropped);",
                row.worst_miss,
                row.mass,
                100.0 * row.rejected,
                row.dropped,
                row.placed + row.dropped
            );
        }
        acceptance.pop();
        Ok(TaskOutput {
            table: render(&fit),
            source:
                "Moe and Di Stefano (2017, ApJS 230, 15), eqs. 20–23 and Table 13; Mardling and \
                     Aarseth (2001, MNRAS 321, 398) for the rejection"
                    .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small fit is the same on one thread and on four, and moves the table off its ones.
    #[test]
    fn a_small_fit_is_the_same_on_any_thread_count() {
        let one = fit(2, 300, NonZeroUsize::MIN).unwrap();
        let four = fit(2, 300, NonZeroUsize::new(4).unwrap()).unwrap();
        assert!(
            one.table
                .iter()
                .flatten()
                .zip(four.table.iter().flatten())
                .all(|(a, b)| a.total_cmp(b).is_eq())
        );
        assert_eq!(one.last, four.last);
        assert_eq!(one.last.len(), 4);
        assert!(one.table.iter().flatten().any(|&c| (c - 1.0).abs() > 1e-3));
        let table = render(&one);
        let TableItem::Source(body) = &table.items[0] else {
            panic!("the body is one verbatim item");
        };
        assert!(body.starts_with("/// The factors"));
        assert!(body.contains("pub const PERIOD_CORRECTION: [[f64; 8]; 4] = ["));
        assert_eq!(body.matches(",\n").count(), 32 + 4);
    }

    /// The task runs its smoke manifest, and turns down an empty run.
    #[test]
    fn the_task_runs_its_smoke_manifest() {
        let task = PeriodCorrectionTask;
        let empty = Manifest::from_bytes(
            std::path::Path::new("empty.toml"),
            b"task = \"period_correction\"\n[params]\niterations = 0\nsample = 10\n".to_vec(),
        )
        .unwrap();
        assert!(matches!(
            task.run(&empty, NonZeroUsize::MIN),
            Err(RunTaskError::Param(_))
        ));
        let smoke = Manifest::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("manifests/period_correction.smoke.toml"),
        )
        .unwrap();
        let output = task.run(&smoke, NonZeroUsize::new(2).unwrap()).unwrap();
        assert!(
            output
                .acceptance
                .starts_with("after 2 iterations over 300 systems a row")
        );
        assert!(output.acceptance.contains("at 28 M☉"));
        assert_eq!(output.table.items.len(), 1);
        assert_eq!(task.fingerprint().probes().len(), 8);
    }
}
