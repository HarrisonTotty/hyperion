//! The Type Ia explosion mark's yield (plan 15, P15.T9.b; consumer plan 11's
//! `stellar::binary::ia`, which reads `tables::binary::IA_YIELD`).
//!
//! The binary engine pools each pair's first event that could be a Type Ia supernova (P11.T6's
//! pool), and an explosion mark thins the pool towards the observed rate: a candidate that can
//! detonate (ruling 129.1's rule) explodes with probability η of its delay bin and channel. The
//! formulae do not set the rate, which they are known to underpredict several times over (Claeys
//! et al. 2014, A&A 563, A83, §4.3 and §7.2, against Maoz et al. 2011's rate), so η is fitted, not
//! computed (plan 15's record "T9.b's target", which replaces the task's first acceptance):
//!
//! - **The pool.** The detonatable candidates per M☉ formed, by delay bin of
//!   `tables::type_ia_delay::DELAY_EDGES` and by channel, from the engine run over a Hubble time on
//!   the grid's records of layers C–E about the solar circle as the generator draws them, each
//!   record's first attempt (the sim's `stellar::binary::testing::ia_pool_census_in`), each
//!   band's candidates a record weighted by the band's share of the systems and divided by the
//!   galaxy's mass formed a system. Layers A and B make none in a Hubble time: a primary under
//!   0.75 M☉ leaves its main sequence after it. The census galaxy is the Milky Way fixture under
//!   the manifest's generator version's rules, every hold that version turns on in force.
//! - **The target.** The bin's share of the observed yield, Maoz and Graur's (2017) 1.3 × 10⁻³ per
//!   M☉ formed (`tables::type_ia_delay::YIELD_PER_SOLAR_MASS`), for both channels together: the
//!   observed delay-time distribution does not split the channels, so the mark thins both
//!   channels of a bin alike, η = min(1, target ÷ pool) with the pool of both (lean, 2026-11-05).
//! - **Where the pool is short** η is 1, every detonatable candidate exploding, and the header
//!   reports the bin's pool against its target: the rest of the observed rate is what the
//!   formulae cannot make, which plan 09's delay-first class carries at 1.3 × 10⁻³ regardless.
//!
//! The run's acceptance, measured and stated in the header: η is 1 wherever the pool is short,
//! with each bin's pool ÷ target; the exploded yield, each candidate on its own pair's mark under
//! the fitted table (`IaExplosionMark`), is min(pool, target) in every bin to its Poisson error;
//! η fitted to the census's even parts and to its odd parts agree within their errors (the mark
//! thins a pool it does not shape); and, recorded and not asserted, the present rate of every
//! double-white-dwarf contact, stable ones included, over the last Gyr before each record's age
//! against the delay-time distribution's at the same ages, Maoz, Hallakoun and Badenes (2018,
//! MNRAS 476, 2584) measuring 4.5–7.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::imf::{BandShares, MassBand};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::stellar::binary::RedrawAttempt;
use hyperion_sim::stellar::binary::ia::IaExplosionMark;
use hyperion_sim::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, IaPoolCensus, ia_pool_census_in,
};
use hyperion_sim::tables::binary::{IA_DELAY_BINS, IaPoolChannel, IaYieldTable, ia_delay_bin};
use hyperion_sim::tables::type_ia_delay::{DELAY_EDGES, YIELD_PER_SOLAR_MASS};
use hyperion_sim::units::Years;
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion, Seed};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The block's items, in its order.
pub const ITEMS: [&str; 1] = ["IA_YIELD_ETA"];

/// The bands a census runs: those whose primaries make a candidate in a Hubble time.
pub const BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// The channels, in `IaYieldTable`'s order: the merger first.
pub const CHANNELS: [IaPoolChannel; 2] = [IaPoolChannel::Merger, IaPoolChannel::Accretion];

/// The fingerprint's galaxy's seed: the manifests' (`type_ia_yield.toml`).
const FINGERPRINT_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The generator version the fingerprint's galaxy is built at: the manifests' (a test holds both
/// manifests to it).
pub const FINGERPRINT_VERSION: GeneratorVersion = GeneratorVersion::new(30);

/// The fingerprint's census: the first 120 layer-D records of the solar circle's slab, part 0 of
/// 64, so that a change to the engine's pool moves the probes.
const FINGERPRINT_RECORDS: u64 = 120;

/// The largest distance, in standard errors, at which the acceptance passes a bin's exploded yield
/// against min(pool, target), or η of the census's even parts against its odd: 4.098, the
/// two-sided normal tail of α = 10⁻³ shared over the 24 bins (Bonferroni; the testkit's `ALPHA`),
/// erfc(z ÷ √2) = 10⁻³ ÷ 24.
pub const PASS_SIGMA: f64 = 4.098;

/// The records a census may run, exclusive: 2⁵³, below which a count is exact as a float.
const MAX_RECORDS: u64 = 1 << 53;

/// The run's parameters, from a manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YieldParams {
    /// The Milky Way fixture's seed, whose universe the explosion marks are drawn in.
    pub seed: u64,
    /// The generator version whose rules the census galaxy is built under.
    pub version: GeneratorVersion,
    /// The slab's records, layers C–E together, in the bands' shares of the systems.
    pub records: u64,
    /// The fit's parts, the first of `part_count`.
    pub parts: u64,
    /// The parts the slab's cells are cut into.
    pub part_count: u64,
    /// The age each pair is run to, years: the last edge of `DELAY_EDGES`, the Hubble time.
    pub until: Years,
}

impl YieldParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if one is missing, is not a number, or is out of range: a generator
    /// version under the sim's own or past `u32`, no records or 2⁵³ or more (the counts are summed
    /// as floats), fewer than two parts or more than `part_count`.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let version = u32::try_from(manifest.u64("generator_version")?)
            .ok()
            .filter(|&v| v >= GENERATOR_VERSION.get())
            .map(GeneratorVersion::new)
            .ok_or_else(|| {
                ManifestParamError::new("generator_version", "the sim's version or a later one")
            })?;
        let params = Self {
            seed: manifest.u64("seed")?,
            version,
            records: manifest.u64("records")?,
            parts: manifest.u64("parts")?,
            part_count: manifest.u64("part_count")?,
            until: Years::new(DELAY_EDGES[IA_DELAY_BINS]),
        };
        if params.records == 0 || params.records >= MAX_RECORDS {
            return Err(ManifestParamError::new("records", "positive and under 2⁵³"));
        }
        if params.parts < 2 || params.parts > params.part_count {
            return Err(ManifestParamError::new(
                "parts",
                "at least two, for the even and odd halves, and at most `part_count`",
            ));
        }
        Ok(params)
    }

    /// The census galaxy: the Milky Way fixture of the census's seed under its version's rules.
    ///
    /// # Panics
    ///
    /// If the Milky Way fixture does not build.
    #[must_use]
    pub fn galaxy(&self) -> Galaxy {
        Galaxy::from_params_at_version(
            Seed::new(self.seed),
            GalaxyParams::milky_way_like(),
            self.version,
        )
        .expect("the Milky Way fixture builds")
    }
}

/// One part of one band's census.
#[derive(Debug, Clone, PartialEq)]
pub struct PartCensus {
    /// The band's index in [`BANDS`].
    pub band: usize,
    /// The part's index.
    pub part: u64,
    /// What it found.
    pub census: IaPoolCensus,
}

/// Runs the slab's census in `galaxy` on `threads` threads: each band's records in its share of
/// the systems, each part's the band's over the parts, one job a band and part, in that order.
///
/// # Errors
///
/// [`BuildThreadPoolError`](crate::parallel::BuildThreadPoolError) if the thread pool cannot be
/// built.
///
/// # Panics
///
/// If the slab's cells leave the root cube (`ia_pool_census_in`), which they do not.
pub fn run_census(
    galaxy: &Galaxy,
    params: &YieldParams,
    threads: NonZeroUsize,
) -> Result<Vec<PartCensus>, crate::parallel::BuildThreadPoolError> {
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = BANDS
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let per_part = |band: MassBand| {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a record count under 2⁵³, rounded to a whole count"
        )]
        let in_band = (params.records as f64 * shares.share(band) / in_bands).round() as u64;
        in_band.div_ceil(params.parts)
    };
    let jobs: Vec<(usize, u64)> = (0..BANDS.len())
        .flat_map(|b| (0..params.parts).map(move |k| (b, k)))
        .collect();
    let n = u64::try_from(jobs.len()).expect("a few hundred jobs");
    let mut out = Vec::with_capacity(jobs.len());
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let (b, k) = jobs[usize::try_from(i).expect("a job's index")];
                    let census = ia_pool_census_in(
                        galaxy,
                        BANDS[b],
                        CensusSite::SOLAR_CIRCLE,
                        CensusRecords::All,
                        per_part(BANDS[b]),
                        CensusPart::new(k, params.part_count),
                        params.until,
                    );
                    PartCensus {
                        band: b,
                        part: k,
                        census,
                    }
                })
                .collect::<Vec<PartCensus>>()
        },
        |done| out.extend(done),
    )?;
    Ok(out)
}

/// The pool of some parts of a census: per M☉ formed, by delay bin and channel, with its
/// variance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pool {
    /// Detonatable candidates per M☉ formed, by bin and channel.
    pub yield_per_solar_mass: [[f64; 2]; IA_DELAY_BINS],
    /// Their Poisson variance.
    pub variance: [[f64; 2]; IA_DELAY_BINS],
}

impl Pool {
    /// Both channels' pool in bin `k`.
    #[must_use]
    pub fn both(&self, k: usize) -> f64 {
        self.yield_per_solar_mass[k][0] + self.yield_per_solar_mass[k][1]
    }

    /// Both channels' variance in bin `k`.
    #[must_use]
    pub fn both_variance(&self, k: usize) -> f64 {
        self.variance[k][0] + self.variance[k][1]
    }
}

/// The index of `channel` in [`CHANNELS`].
#[must_use]
fn channel_index(channel: IaPoolChannel) -> usize {
    match channel {
        IaPoolChannel::Merger => 0,
        IaPoolChannel::Accretion => 1,
    }
}

/// Each band's weight a candidate, per M☉ formed: the band's share of the systems over its records
/// in `parts` and the galaxy's mass formed a system.
#[must_use]
fn weights(galaxy: &Galaxy, parts: &[&PartCensus]) -> [f64; 3] {
    let shares = BandShares::of(galaxy.mass_function());
    let formed = galaxy.mean_formed_mass().value();
    core::array::from_fn(|b| {
        let records = count(
            parts
                .iter()
                .filter(|p| p.band == b)
                .map(|p| p.census.scanned())
                .sum(),
        );
        if records > 0.0 {
            shares.share(BANDS[b]) / records / formed
        } else {
            0.0
        }
    })
}

/// The detonatable candidates of `parts`, per M☉ formed, with their Poisson variance.
#[must_use]
fn pool_of(galaxy: &Galaxy, parts: &[&PartCensus]) -> Pool {
    let w = weights(galaxy, parts);
    let mut pool = Pool {
        yield_per_solar_mass: [[0.0; 2]; IA_DELAY_BINS],
        variance: [[0.0; 2]; IA_DELAY_BINS],
    };
    for p in parts {
        for m in p.census.members().iter().filter(|m| m.detonatable()) {
            let (k, c) = (
                ia_delay_bin(m.event().age()),
                channel_index(m.event().channel()),
            );
            pool.yield_per_solar_mass[k][c] += w[p.band];
            pool.variance[k][c] += w[p.band] * w[p.band];
        }
    }
    pool
}

/// η of bin `k` of `pool`: min(1, target ÷ pool), both channels together, and 1 for an empty bin.
#[must_use]
fn eta_of(pool: &Pool, k: usize) -> f64 {
    let both = pool.both(k);
    if both > 0.0 {
        (YIELD_PER_SOLAR_MASS[k] / both).min(1.0)
    } else {
        1.0
    }
}

/// The fitted yield and what the acceptance measured of it.
#[derive(Debug, Clone, PartialEq)]
pub struct YieldFit {
    /// The parameters.
    pub params: YieldParams,
    /// η by bin and channel.
    pub eta: [[f64; 2]; IA_DELAY_BINS],
    /// The fit's pool, every part.
    pub pool: Pool,
    /// The exploded yield per M☉ formed by bin, each candidate on its own pair's mark under the
    /// fitted table, with its thinning's variance.
    pub exploded: [(f64, f64); IA_DELAY_BINS],
    /// The largest distance, in its standard error, between the exploded yield and
    /// min(pool, target) over the bins, 0 where both are exact.
    pub worst_exploded: f64,
    /// The largest distance, in their standard error, between η of the even and of the odd parts
    /// over the bins, 0 where both are capped.
    pub worst_halves: f64,
    /// The records and pairs the census ran, by band.
    pub records: [(u64, u64); 3],
    /// The detonatable candidates found, by channel, and every candidate, detonatable or not.
    pub candidates: ([u64; 2], u64),
    /// The double white dwarfs' contacts over the last Gyr per M☉ formed a year, and the
    /// delay-time distribution's rate at the same ages.
    pub present: (f64, f64),
}

/// Fits the yield to `censuses` of `galaxy` under `params`.
///
/// # Panics
///
/// Never: the table's shares are in [0, 1] by construction.
#[must_use]
pub fn fit_yield(galaxy: &Galaxy, params: &YieldParams, censuses: &[PartCensus]) -> YieldFit {
    let all: Vec<&PartCensus> = censuses.iter().collect();
    let pool = pool_of(galaxy, &all);
    let eta: [[f64; 2]; IA_DELAY_BINS] = core::array::from_fn(|k| [eta_of(&pool, k); 2]);
    let table = IaYieldTable::new(eta);
    let seed = Seed::new(params.seed);
    let w = weights(galaxy, &all);
    let mut exploded = [(0.0, 0.0); IA_DELAY_BINS];
    for p in &all {
        for m in p.census.members().iter().filter(|m| m.detonatable()) {
            let k = ia_delay_bin(m.event().age());
            let e = eta[k][channel_index(m.event().channel())];
            let w = w[p.band];
            exploded[k].1 += w * w * e * (1.0 - e);
            if IaExplosionMark::of(seed, m.pair(), RedrawAttempt::FIRST).explodes(m.event(), &table)
            {
                exploded[k].0 += w;
            }
        }
    }
    let worst_exploded = (0..IA_DELAY_BINS)
        .map(|k| {
            let expected = pool.both(k).min(YIELD_PER_SOLAR_MASS[k]);
            distance(exploded[k].0 - expected, exploded[k].1, expected)
        })
        .fold(0.0, f64::max);
    let half =
        |odd: u64| -> Vec<&PartCensus> { censuses.iter().filter(|p| p.part % 2 == odd).collect() };
    let (even, odd) = (pool_of(galaxy, &half(0)), pool_of(galaxy, &half(1)));
    let worst_halves = (0..IA_DELAY_BINS)
        .map(|k| {
            let (a, b) = (eta_of(&even, k), eta_of(&odd, k));
            // η's relative error is the pool's where it is not capped.
            let relative = |pool: &Pool, eta: f64| {
                if eta < 1.0 && pool.both(k) > 0.0 {
                    eta * eta * pool.both_variance(k) / (pool.both(k) * pool.both(k))
                } else {
                    0.0
                }
            };
            distance(a - b, relative(&even, a) + relative(&odd, b), 1.0)
        })
        .fold(0.0, f64::max);
    let records = core::array::from_fn(|b| {
        all.iter()
            .filter(|p| p.band == b)
            .fold((0, 0), |(r, n), p| {
                (r + p.census.scanned(), n + p.census.pairs())
            })
    });
    let mut candidates = ([0_u64; 2], 0_u64);
    for p in &all {
        for m in p.census.members() {
            candidates.1 += 1;
            candidates.0[channel_index(m.event().channel())] += u64::from(m.detonatable());
        }
    }
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = BANDS.iter().fold(0.0, |sum, &b| sum + shares.share(b));
    let mut present = (0.0, 0.0);
    for p in &all {
        present.0 += w[p.band] * count(p.census.recent_contacts()) / 1.0e9;
        let records = count(
            all.iter()
                .filter(|q| q.band == p.band)
                .map(|q| q.census.scanned())
                .sum(),
        );
        present.1 += shares.share(BANDS[p.band]) / in_bands * p.census.dtd_rate_sum() / records;
    }
    YieldFit {
        params: *params,
        eta,
        pool,
        exploded,
        worst_exploded,
        worst_halves,
        records,
        candidates,
        present,
    }
}

/// `difference` between two figures of size `scale` in its standard error, the square root of
/// `variance`: 0 where it is within the sums' rounding, 10⁻¹² of `scale`, and infinite where it is
/// not and the variance is 0.
#[must_use]
fn distance(difference: f64, variance: f64, scale: f64) -> f64 {
    if difference.abs() <= 1e-12 * scale.abs() {
        0.0
    } else if variance > 0.0 {
        difference.abs() / variance.sqrt()
    } else {
        f64::INFINITY
    }
}

/// `n` as a float, for a count under 2⁵³.
#[must_use]
fn count(n: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a count under 2⁵³")]
    let n = n as f64;
    n
}

/// The fit's table: its summary, its notes and its item.
#[must_use]
pub fn render_yield(fit: &YieldFit) -> RustTable {
    RustTable {
        summary: vec![
            "The Type Ia explosion mark's yield η by delay bin and channel, fitted against plan 11's"
                .to_owned(),
            format!(
                "engine's pool at generator version {}'s rules (plan 15, P15.T9.b).",
                fit.params.version.get()
            ),
        ],
        notes: notes(fit),
        items: vec![TableItem::Nested {
            name: ITEMS[0].to_owned(),
            doc: vec![
                "η by delay bin of [`DELAY_EDGES`] and then by channel, the merger first: the share"
                    .to_owned(),
                "of the engine's detonatable pooled candidates that explode, min(1, target ÷ pool)."
                    .to_owned(),
            ],
            rows: fit.eta.iter().map(|row| row.to_vec()).collect(),
        }],
    }
}

/// The block's notes after its header, wrapped to the block's comment width.
#[must_use]
fn notes(fit: &YieldFit) -> Vec<String> {
    let p = &fit.params;
    let capped = fit.eta.iter().filter(|row| row[0] >= 1.0).count();
    let kept = if capped == IA_DELAY_BINS {
        "Every bin is capped, so η is the scratch's 1 everywhere and no generated value moves."
    } else {
        "Some bins are not capped: the mark thins their candidates towards the target."
    };
    let text = format!(
        "Read by `IA_YIELD`. Each bin's pool is the engine's detonatable candidates per solar mass \
         formed (hyperion-fit's `tasks::type_ia_yield`): each grid record's first attempt's pairs \
         of layers C–E about the solar circle run to {until:.4e} years under generator version \
         {version}'s rules, each band's candidates a record weighted by its share of the systems \
         over the mass formed a system. η is min(1, target ÷ pool), the target the bin's share of \
         the observed 1.3 × 10⁻³ per solar mass formed, both channels of a bin thinned alike; \
         {capped} of {bins} bins are capped at 1, the pool short there. {kept} The census ran \
         {records} records in parts 0–{last} of {count}.",
        until = p.until.value(),
        version = p.version.get(),
        bins = IA_DELAY_BINS,
        records = fit.records.iter().map(|r| r.0).sum::<u64>(),
        last = p.parts - 1,
        count = p.part_count,
    );
    wrapped(&text, NOTE_WIDTH)
}

/// The width of a note's line, characters: the block's 100 less its `// `.
const NOTE_WIDTH: usize = 97;

/// `text`'s words in lines of at most `width` characters, greedily.
#[must_use]
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The acceptance's figures, for the header and the lock file.
#[must_use]
fn acceptance(fit: &YieldFit) -> String {
    // Writing to a `String` cannot fail, so the results of `write!` are dropped.
    let mut out = String::new();
    let capped = fit.eta.iter().filter(|row| row[0] >= 1.0).count();
    let short_capped = (0..IA_DELAY_BINS)
        .filter(|&k| fit.pool.both(k) <= YIELD_PER_SOLAR_MASS[k])
        .all(|k| fit.eta[k].iter().all(|&e| e >= 1.0));
    let _ = write!(
        out,
        "{capped} of {IA_DELAY_BINS} bins capped at η 1, every short bin among them ({}), the pool \
         ÷ target by bin",
        verdict(short_capped)
    );
    for (k, target) in YIELD_PER_SOLAR_MASS.iter().enumerate() {
        let sep = if k == 0 { " " } else { "/" };
        let _ = write!(out, "{sep}{:.3}", fit.pool.both(k) / target);
    }
    let total: f64 = YIELD_PER_SOLAR_MASS.iter().sum();
    let by_channel = |c: usize| {
        (0..IA_DELAY_BINS)
            .map(|k| fit.pool.yield_per_solar_mass[k][c])
            .sum::<f64>()
    };
    let exploded: f64 = fit.exploded.iter().map(|e| e.0).sum();
    let _ = write!(
        out,
        "; the pool {:.4e} per M☉ formed, {:.3} of the yield (mergers {:.3}, accretions {:.3}); \
         exploded {:.4e}, min(pool, target) in every bin within {:.2} σ ({}); η of the even and \
         odd parts within {:.2} σ ({}; both under {PASS_SIGMA} σ, α = 10⁻³ over the bins); {} \
         detonatable candidates (mergers {}, accretions {}) of {}; \
         records C/D/E {}/{}/{}, pairs {}/{}/{}; recorded, not asserted: the double white dwarfs' \
         contacts over the last Gyr {:.3e} per M☉ formed a year, {:.3} × the delay-time \
         distribution's {:.3e} at the same ages (Maoz, Hallakoun and Badenes 2018: 4.5–7)",
        fit.pool.both_total(),
        fit.pool.both_total() / total,
        by_channel(0) / total,
        by_channel(1) / total,
        exploded,
        fit.worst_exploded,
        verdict(fit.worst_exploded < PASS_SIGMA),
        fit.worst_halves,
        verdict(fit.worst_halves < PASS_SIGMA),
        fit.candidates.0[0] + fit.candidates.0[1],
        fit.candidates.0[0],
        fit.candidates.0[1],
        fit.candidates.1,
        fit.records[0].0,
        fit.records[1].0,
        fit.records[2].0,
        fit.records[0].1,
        fit.records[1].1,
        fit.records[2].1,
        fit.present.0,
        fit.present.0 / fit.present.1,
        fit.present.1,
    );
    out
}

/// An acceptance check's word: `passes` or `FAILS`.
#[must_use]
const fn verdict(passes: bool) -> &'static str {
    if passes { "passes" } else { "FAILS" }
}

impl Pool {
    /// Both channels' pool over every bin.
    #[must_use]
    pub fn both_total(&self) -> f64 {
        (0..IA_DELAY_BINS).map(|k| self.both(k)).sum()
    }
}

/// The yield's task (`type_ia_yield`), one block of `tables/binary.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TypeIaYieldTask;

impl FitTask for TypeIaYieldTask {
    fn name(&self) -> &'static str {
        "type_ia_yield"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "binary.rs"
    }

    fn shares_its_file(&self) -> bool {
        true
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The galaxy's band shares of layers C–E and its mass formed a system, which the pool's
    /// weights read; the target's yield by bin; and the engine's pool among the fingerprint's
    /// records: its records, its pairs, and each candidate's delay, channel, masses and whether it
    /// can detonate.
    fn fingerprint(&self) -> SimFingerprint {
        let galaxy = Galaxy::from_params_at_version(
            Seed::new(FINGERPRINT_SEED),
            GalaxyParams::milky_way_like(),
            FINGERPRINT_VERSION,
        )
        .expect("the Milky Way fixture builds");
        let shares = BandShares::of(galaxy.mass_function());
        let mut probes: Vec<(String, f64)> = BANDS
            .iter()
            .map(|&b| (format!("band share {b:?}"), shares.share(b)))
            .collect();
        probes.push((
            "mean formed mass".to_owned(),
            galaxy.mean_formed_mass().value(),
        ));
        for (k, y) in YIELD_PER_SOLAR_MASS.iter().enumerate() {
            probes.push((format!("target {k}"), *y));
        }
        let census = ia_pool_census_in(
            &galaxy,
            MassBand::D,
            CensusSite::SOLAR_CIRCLE,
            CensusRecords::All,
            FINGERPRINT_RECORDS,
            CensusPart::new(0, 64),
            Years::new(DELAY_EDGES[IA_DELAY_BINS]),
        );
        probes.push(("records".to_owned(), count(census.scanned())));
        probes.push(("pairs".to_owned(), count(census.pairs())));
        let candidates = u64::try_from(census.members().len()).expect("a few candidates");
        probes.push(("candidates".to_owned(), count(candidates)));
        for (i, m) in census.members().iter().enumerate() {
            let [a, b] = m.event().masses();
            probes.push((format!("candidate {i} delay"), m.event().age().value()));
            let channel = u32::try_from(channel_index(m.event().channel())).expect("two channels");
            probes.push((format!("candidate {i} channel"), f64::from(channel)));
            probes.push((format!("candidate {i} masses"), a.value() + b.value()));
            probes.push((
                format!("candidate {i} detonatable"),
                f64::from(u8::from(m.detonatable())),
            ));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = YieldParams::of(manifest)?;
        let galaxy = params.galaxy();
        let censuses = run_census(&galaxy, &params, threads)?;
        let fit = fit_yield(&galaxy, &params, &censuses);
        Ok(TaskOutput {
            table: render_yield(&fit),
            source: "plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as \
                     P11.T4 builds it) and pool (P11.T6, ruling 129.1's detonatable rule), over \
                     plan 06's tracks, at the manifest's generator version's rules; the target \
                     Maoz and Graur 2017's 1.3 × 10⁻³ per M☉ formed (`type_ia_delay`), the known \
                     shortfall Claeys et al. 2014, A&A 563, A83, §4.3 and §7.2; the contacts \
                     against Maoz, Hallakoun and Badenes 2018, MNRAS 476, 2584"
                .to_owned(),
            acceptance: acceptance(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed manifest, with `edit` applied to its text.
    fn manifest(edit: impl Fn(&str) -> String) -> Manifest {
        let text = edit(include_str!("../../manifests/type_ia_yield.toml"));
        Manifest::from_bytes("manifests/type_ia_yield.toml".as_ref(), text.into_bytes()).unwrap()
    }

    /// The committed manifests read at the fingerprint's version, 30; a census with no records,
    /// one part or more parts than cuts is refused.
    #[test]
    fn the_manifest_s_parameters_are_checked() {
        let params = YieldParams::of(&manifest(str::to_owned)).unwrap();
        assert_eq!(params.version, FINGERPRINT_VERSION);
        assert_eq!(params.seed, FINGERPRINT_SEED);
        let smoke = Manifest::from_bytes(
            "manifests/type_ia_yield.smoke.toml".as_ref(),
            include_bytes!("../../manifests/type_ia_yield.smoke.toml").to_vec(),
        )
        .unwrap();
        assert_eq!(
            YieldParams::of(&smoke).unwrap().version,
            FINGERPRINT_VERSION
        );
        let refused = |from: &str, to: &str, key: &str| {
            let error = YieldParams::of(&manifest(|t| t.replacen(from, to, 1))).unwrap_err();
            assert!(error.to_string().contains(key), "{from} → {to}: {error}");
        };
        refused("records = ", "records = 0 #", "records");
        refused("parts = ", "parts = 1 #", "parts");
        refused("parts = ", "parts = 65 #", "parts");
    }

    /// A note wraps at its width, word by word, and keeps every word.
    #[test]
    fn a_note_wraps_at_its_width() {
        let lines = wrapped("aa bbb cc d eeee", 6);
        assert_eq!(lines, ["aa bbb", "cc d", "eeee"]);
        assert_eq!(wrapped("", 6), Vec::<String>::new());
    }

    /// η is min(1, target ÷ pool) for both channels of a bin, and 1 for an empty bin.
    #[test]
    fn eta_is_the_target_over_the_pool_capped_at_one() {
        let mut pool = Pool {
            yield_per_solar_mass: [[0.0; 2]; IA_DELAY_BINS],
            variance: [[0.0; 2]; IA_DELAY_BINS],
        };
        pool.yield_per_solar_mass[3] = [YIELD_PER_SOLAR_MASS[3], YIELD_PER_SOLAR_MASS[3]];
        pool.yield_per_solar_mass[4] = [0.25 * YIELD_PER_SOLAR_MASS[4], 0.0];
        assert!((eta_of(&pool, 3) - 0.5).abs() < 1e-15);
        assert!((eta_of(&pool, 4) - 1.0).abs() < 1e-15);
        assert!((eta_of(&pool, 5) - 1.0).abs() < 1e-15);
    }
}
