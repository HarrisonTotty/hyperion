//! The kick law's rank table (plan 06, P06.T19.b; plan 15, P15.T5.a): the 257 quantiles of Mandel
//! and Müller's ordinary kick score over the reference population, which
//! `hyperion_sim::stellar::remnant::KickRankTable` interpolates.
//!
//! The committed table is P06.T19.b's provisional one, made by the sim's own [`score_quantiles`]
//! with 10⁶ scores, and `manifests/kick_rank.toml` describes it. Plan 15's P15.T5.a takes the task
//! over: [`KickRankTask`] runs the same computation in chunks through
//! [`map_reduce_chunks`](crate::parallel::map_reduce_chunks) ([`fit_parallel`]), bit for bit the
//! same as [`score_quantiles`] for every chunking and thread count, and the production run of 10⁷
//! scores is `manifests/kick_rank.production.toml`, written with `--out` to a scratch path. Plan
//! 06's P06.T19.e is the commit that swaps it in, with the bumped generator version. Every score
//! costs a full track of a massive star, about a millisecond, so the task is slow; its tests run
//! it on a small sample.
//!
//! The run's acceptance figures are the moments of ln(speed ÷ km/s) of fresh reference scores
//! mapped through the new table ([`acceptance`]): uniform ranks give 5.60 and 0.68 before the
//! kick law's truncation at 1,000 km/s, its rank clamp to 0.001–0.999 taking a few thousandths
//! off the standard deviation (0.6758 over the committed table's 10⁵ fresh scores).

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::remnant::reference::{
    KROUPA_HIGH_MASS_SLOPE, MASS_RANGE, QUANTILES, ReferencePopulation, quantiles_of,
    score_quantiles,
};
use hyperion_sim::stellar::remnant::{KickLawParams, KickRankTable};
use hyperion_sim::stellar::sse::Track;
use hyperion_sim::tables::kick_rank;
use hyperion_sim::units::SolarMasses;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The scores the committed table is made from: 10⁶ (P06.T19.b).
pub const DRAWS: u64 = 1_000_000;

/// The seed of the committed table's sample.
pub const SEED: u64 = 0x0619_b000_0000_0000;

/// Sample stars per chunk of [`fit_parallel`]. The chunking cannot change the result.
pub const CHUNK: u64 = 1_024;

/// The initial masses, M☉, of the fingerprint's probes: the reference population's range, from the
/// lightest core collapses to the heaviest track the generator builds.
pub const FINGERPRINT_MASSES: [f64; 8] = [8.0, 10.0, 12.0, 15.0, 20.0, 30.0, 50.0, 100.0];

/// The fitted table: the quantiles and what they were made from.
#[derive(Debug, Clone, PartialEq)]
pub struct KickRankFit {
    /// The score's quantiles at ranks i ÷ 256.
    pub quantiles: [f64; QUANTILES],
    /// The number of scores.
    pub draws: u64,
    /// The sample's seed.
    pub seed: u64,
}

/// The committed table's fit: [`DRAWS`] scores under [`SEED`], on one thread.
#[must_use]
pub fn fit() -> KickRankFit {
    fit_with(DRAWS, SEED)
}

/// The quantiles of `draws` scores of the reference population under `seed`, by the sim's
/// [`score_quantiles`] on one thread.
///
/// # Panics
///
/// If `draws` is below 2.
#[must_use]
pub fn fit_with(draws: u64, seed: u64) -> KickRankFit {
    let population = ReferencePopulation::default();
    KickRankFit {
        quantiles: score_quantiles(&population, draws, Seed::new(seed)),
        draws,
        seed,
    }
}

/// The scores of the first `n` members of `population` under `seed`, in sample order, scored in
/// chunks of [`CHUNK`] sample stars on `threads` threads.
///
/// Sample stars are taken in rounds, each of twice the members still wanted (about three sample
/// stars in five are members), so the last round overshoots by little; each round's chunks are
/// scored in parallel and joined in index order, and the scores are cut at the n-th. So the result
/// is the first `n` members in order whatever the chunking and threads: exactly the scores
/// [`score_quantiles`] sorts.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `n` scores do not fit in memory.
pub fn scores_parallel(
    population: &ReferencePopulation,
    n: u64,
    seed: Seed,
    threads: NonZeroUsize,
) -> Result<Vec<f64>, BuildThreadPoolError> {
    let n_usize = usize::try_from(n).expect("a sample that fits in memory has a usize length");
    let mut scores = Vec::with_capacity(n_usize);
    let mut start = 0;
    while scores.len() < n_usize {
        let wanted = n - u64::try_from(scores.len()).expect("a usize fits in 64 bits");
        let round = (2 * wanted).max(16);
        map_reduce_chunks(
            round,
            CHUNK,
            threads,
            |items| population.scores(seed, start + items.start..start + items.end),
            |chunk| scores.extend(chunk),
        )?;
        start += round;
    }
    scores.truncate(n_usize);
    Ok(scores)
}

/// The quantiles of `draws` scores under `seed` on `threads` threads: bit for bit
/// [`fit_with`]'s.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `draws` is below 2.
pub fn fit_parallel(
    draws: u64,
    seed: u64,
    threads: NonZeroUsize,
) -> Result<KickRankFit, BuildThreadPoolError> {
    assert!(draws >= 2, "quantiles need at least two scores");
    let population = ReferencePopulation::default();
    let mut scores = scores_parallel(&population, draws, Seed::new(seed), threads)?;
    Ok(KickRankFit {
        quantiles: quantiles_of(&mut scores),
        draws,
        seed,
    })
}

/// The acceptance figures of a rank table: fresh reference scores mapped through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KickRankAcceptance {
    /// The fresh scores.
    pub draws: u64,
    /// The mean of ln(speed ÷ km/s), speed exp(μ + σ Φ⁻¹(r)) with the rank r clamped to
    /// 0.001–0.999 and the truncation at 1,000 km/s not applied.
    pub mean_ln_speed: f64,
    /// Its standard deviation.
    pub sd_ln_speed: f64,
    /// The largest change from the committed table, as a shift of rank: the largest
    /// |`F_committed`(qᵢ) − i ÷ 256| over the new table's knots qᵢ.
    pub largest_rank_shift: f64,
}

/// The acceptance figures of `fit`: `draws` fresh scores under `seed` mapped through it.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `fit`'s quantiles do not make a rank table, which [`quantiles_of`] rules out.
pub fn acceptance(
    fit: &KickRankFit,
    draws: u64,
    seed: u64,
    threads: NonZeroUsize,
) -> Result<KickRankAcceptance, BuildThreadPoolError> {
    let table = KickRankTable::new(fit.quantiles.to_vec())
        .expect("quantiles_of returns strictly increasing finite quantiles");
    let params = KickLawParams::default();
    let (lo, hi) = params.rank_clamp;
    let scores = scores_parallel(
        &ReferencePopulation::default(),
        draws,
        Seed::new(seed),
        threads,
    )?;
    // Welford's running mean and variance, in sample order.
    let (mut count, mut mean, mut m2) = (0.0_f64, 0.0_f64, 0.0_f64);
    for score in scores {
        let r = table.rank(score).clamp(lo, hi);
        let x = math::mul_add(params.ln_sigma, math::normal_quantile(r), params.ln_mu);
        count += 1.0;
        let delta = x - mean;
        mean += delta / count;
        m2 += delta * (x - mean);
    }
    let committed = KickRankTable::generator();
    let last = u32::try_from(QUANTILES - 1).expect("257 fits in u32");
    let largest_rank_shift = fit
        .quantiles
        .iter()
        .zip(0..=last)
        .map(|(&q, i)| (committed.rank(q) - f64::from(i) / f64::from(last)).abs())
        .fold(0.0, f64::max);
    Ok(KickRankAcceptance {
        draws,
        mean_ln_speed: mean,
        sd_ln_speed: (m2 / (count - 1.0)).sqrt(),
        largest_rank_shift,
    })
}

/// The table's contents: its summary, its notes and its items, which are the committed bytes.
#[must_use]
pub fn render(fit: &KickRankFit) -> RustTable {
    let params = KickLawParams::default();
    let notes = format!(
        "\
The four defaults are the constants `stellar::remnant::KickLawParams::default` reads.

**Provisional** (P06.T19.b): plan 15's P15.T5.a replaces the quantiles with a run of 10⁷
scores and takes the file over, with a generator-version bump (P06.T19.e).

Inputs: `stellar::remnant::reference::score_quantiles` over {draws} scores of seed
{seed:#018x}: Kroupa primaries of {lo}–{hi} M☉ (slope {slope}) at Z = 0.02 on the
generator's tracks, the iron-core collapses of single and wind-stripped progenitors that
leave a neutron star, each scored (`M_CO` − `M_rem`) ÷ `M_rem` × ξ with ξ normal about 1 of
relative scatter {scatter} redrawn until positive.",
        draws = fit.draws,
        seed = fit.seed,
        lo = MASS_RANGE.0,
        hi = MASS_RANGE.1,
        slope = KROUPA_HIGH_MASS_SLOPE,
        scatter = params.score_scatter,
    );
    let mut out = String::new();
    out.push_str(
        "/// The ordinary kick score at ranks i ÷ 256 over the reference population, strictly\n\
         /// increasing.\n",
    );
    let _ = writeln!(out, "pub const SCORE_QUANTILES: [f64; {QUANTILES}] = [");
    for q in fit.quantiles {
        let _ = writeln!(out, "    {},", literal(q));
    }
    out.push_str("];\n\n");
    let (lo, hi) = kick_rank::LOW_RAMP;
    let _ = writeln!(
        out,
        "/// The carbon–oxygen cores, M☉, over which a companion-stripped progenitor's chance of the\n\
         /// low kick mode falls from 1 to 0 (the brainstorm's \"Open questions\").\n\
         pub const LOW_RAMP: (f64, f64) = ({}, {});\n\n\
         /// The factor on a black hole's ordinary kick: HYPERION's calibration against Nagarajan\n\
         /// and El-Badry 2025 and Atri et al. 2019 (Mandel and Müller 2020 have 0.5; ruling 96.4).\n\
         pub const BH_FACTOR: f64 = {};\n\n\
         /// The single star's electron-capture window, M☉ of the mass `m_c_bagb` reads (the\n\
         /// brainstorm's 0.1; ruling 45.1).\n\
         pub const EC_WINDOW_SINGLE: f64 = {};\n\n\
         /// A companion-stripped star's electron-capture window, M☉ of the same mass (the\n\
         /// brainstorm's \"about 1\").\n\
         pub const EC_WINDOW_STRIPPED: f64 = {};",
        literal(lo),
        literal(hi),
        literal(kick_rank::BH_FACTOR),
        literal(kick_rank::EC_WINDOW_SINGLE),
        literal(kick_rank::EC_WINDOW_STRIPPED),
    );
    RustTable {
        summary: vec![
            "The kick law's rank table and its four defaults (plan 06, P06.T19.b): the quantiles of"
                .to_owned(),
            "Mandel and Müller's ordinary kick score over the reference population, which"
                .to_owned(),
            "`stellar::remnant::KickRankTable` interpolates.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(out)],
    }
}

/// The task: P06.T19.b's provisional table, taken over by plan 15 (P15.T5.a); slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct KickRankTask;

impl FitTask for KickRankTask {
    fn name(&self) -> &'static str {
        "kick_rank"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "kick_rank.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "SCORE_QUANTILES",
            "LOW_RAMP",
            "BH_FACTOR",
            "EC_WINDOW_SINGLE",
            "EC_WINDOW_STRIPPED",
        ]
    }

    /// Core and remnant masses at eight initial masses on the generator's tracks (Design note
    /// 7), with the median draws at Z = 0.02, and the score's scatter.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::with_capacity(2 * FINGERPRINT_MASSES.len() + 1);
        for m0 in FINGERPRINT_MASSES {
            let track = Track::full(
                SolarMasses::new(m0),
                &Composition::SOLAR,
                &StarDraws::median(),
            );
            let core = track
                .death()
                .map_or(0.0, |d| d.progenitor().co_core_mass().value());
            let remnant = track.remnant().map_or(0.0, |r| r.mass().value());
            probes.push((format!("co_core_mass({m0} M☉)"), core));
            probes.push((format!("remnant_mass({m0} M☉)"), remnant));
        }
        probes.push((
            "KickLawParams::default().score_scatter".to_owned(),
            KickLawParams::default().score_scatter,
        ));
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let draws = manifest.u64("draws")?;
        let seed = manifest.u64("seed")?;
        let acceptance_draws = manifest.u64("acceptance_draws")?;
        let acceptance_seed = manifest.u64("acceptance_seed")?;
        if draws < 2 || acceptance_draws < 2 {
            return Err(crate::manifest::ManifestParamError::new("draws", "at least 2").into());
        }
        let fit = fit_parallel(draws, seed, threads)?;
        let figures = acceptance(&fit, acceptance_draws, acceptance_seed, threads)?;
        let increasing = fit.quantiles.windows(2).all(|q| q[0] < q[1]);
        Ok(TaskOutput {
            table: render(&fit),
            source: "Disberg and Mandel (2025, ApJ Letters 989, L8) for the log-normal of young \
                     isolated pulsars; Mandel and Müller (2020, MNRAS 499, 3214) for the score; Disberg, \
                     Mandel and Hirai (2026) for its 45% scatter; plan 06's tracks"
                .to_owned(),
            acceptance: format!(
                "{} fresh scores through the table give ln(v ÷ km/s) of mean {:.4} and standard \
                 deviation {:.4}, rank clamped and untruncated; knots {}; largest shift of a knot's \
                 rank from the committed table {:.5}",
                figures.draws,
                figures.mean_ln_speed,
                figures.sd_ln_speed,
                if increasing {
                    "strictly increasing"
                } else {
                    "NOT strictly increasing"
                },
                figures.largest_rank_shift,
            ),
            provisional: Some("P06.T19.b"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_fit_is_strictly_increasing_and_repeats_bit_for_bit() {
        let a = fit_with(200, SEED);
        let b = fit_with(200, SEED);
        assert_eq!(a, b);
        assert!(a.quantiles.windows(2).all(|q| q[0] < q[1]));
        assert!(
            a.quantiles[0] > 0.0,
            "a neutron star's core outweighs it: {}",
            a.quantiles[0]
        );
        let table = render(&a);
        let TableItem::Source(body) = &table.items[0] else {
            panic!("the body is one verbatim item");
        };
        assert!(body.contains("pub const SCORE_QUANTILES: [f64; 257] = ["));
        assert!(table.notes.iter().any(|l| l.contains("Provisional")));
        assert!(table.notes.iter().any(|l| l.contains("200 scores of seed")));
        assert!(table.notes.iter().any(|l| l.contains("0x0619b00000000000")));
        assert_eq!(body.matches(",\n").count(), QUANTILES);
    }

    /// P15.T5.a's chunked, threaded run is [`score_quantiles`] bit for bit, on one thread and on
    /// four, and so cannot depend on how it is cut up.
    #[test]
    fn the_parallel_fit_is_score_quantiles_for_any_thread_count() {
        let serial = fit_with(1_500, SEED);
        for threads in [1, 4] {
            let parallel = fit_parallel(1_500, SEED, NonZeroUsize::new(threads).unwrap()).unwrap();
            assert!(
                parallel
                    .quantiles
                    .iter()
                    .zip(&serial.quantiles)
                    .all(|(a, b)| a.total_cmp(b).is_eq()),
                "{threads} threads"
            );
        }
    }
}
