//! The binary classes' samplers and share rows (plan 15, P15.T10.b), fitted against plan 11's
//! engine: one task a class, each writing its block of the sim's `tables/binary.rs`, whose shapes
//! and scratch tables are plan 11's. Built so far: the accreting white dwarfs' (`binary_awd`) and
//! the X-ray binaries' (`binary_xrb`, [`xrb`]).
//!
//! # The census
//!
//! The engine's members are the sim's own `stellar::binary::testing::class_census_in`: grid
//! records of layers C–E, each band's count in the galaxy's band shares, so that the members are
//! an unweighted sample of the systems that can make the classes (a primary under 0.75 M☉, layers
//! A and B, leaves the main sequence after the Galaxy's age and can make none), each record's
//! first attempt run through the engine with no carve-out and kept where `carved_class` puts a
//! pair in a class. The census of every population is a slab of the disc about the solar circle;
//! the populations scarce there, the young thin disc, the halo, the bulge, the long bar and the
//! nuclear disc, each have a census of their own at a site where they lie, of their records
//! alone. A census is cut into parts of disjoint cells, and the fit takes the first half of them,
//! so that the sim's slow tests, which take the other half, check the table against members it
//! was not fitted to.
//!
//! # The share rows
//!
//! A class's row holds its hosts per solar mass formed by population and the share of them whose
//! primary lies in each layer (`ClassShareRow`). Each population's rate is its census's members of
//! the class a record, each band's count weighted by the band's share of the systems; the rows are
//! then scaled together so that the Milky Way fixture's hosts of the task's classes are the
//! brainstorm's observed total (plan 11's design note 11: counts are observed, the engine gives
//! their shape). Each layer's share is the galaxy's hosts with a primary in it: every population's
//! rate in the layer times its systems there.
//!
//! From the refit at 27 the accreting white dwarfs' rows are by donor kind (P11.T5's fix,
//! [`donor_rows`]): each kind scaled to its own observed count, its split between the classes the
//! engine's, so `binary_awd` reads no total of its own.
//!
//! # The galaxy
//!
//! Each task fits at its manifest's `generator_version`, 29 for the refit on the generator's own
//! pairs (the census's first attempts as the generator draws them, train 27's finding), into its
//! block's `_29` items, which `tables::binary`'s samplers and share rows read. Its census galaxy is the version's, every hold
//! that version turns on in force, with the binary classes' scratch share rows in its class shares
//! (`Galaxy::from_params_for_binary_refit`), so that the field the census's records come from does
//! not read the rows the task fits (plan 15's lean of 2026-11-01).
//!
//! # A sampler
//!
//! Each mark of a class's sampler is fitted to its members' values (`marks`): a median that
//! follows its condition, the residual about it, and from the refit at 27 a residual law at each
//! node of the median. A mark is fitted to its own class's members where they number at least the
//! manifest's `min_members`, to both classes' members pooled where they do not, and keeps the
//! sim's sampler's law where neither does, which the fitted tables hold as P11.T8's scratch had it
//! where the refit at 27 kept it. A mark's members are weighted so that their kinds are
//! in the shares the class's choice of kind draws ([`weighted_to_shares`]: the rows' donor kinds,
//! or the X-ray kinds' weights), so that a mark the kinds share is fitted to them in the rows'
//! mix and not in the census's.

pub mod donor_rows;
mod marks;
pub mod xrb;

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::imf::{BandShares, MassBand};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::math;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::testing::{
    AccretingMarks, CensusPart, CensusRecords, CensusSite, ClassCensus, ClassMember,
    class_census_in,
};
use hyperion_sim::tables::binary::{
    ACCRETING_WHITE_DWARF_MAX_MASS, AWD_FAST_SAMPLER, AWD_SLOW_SAMPLER, AwdDonorKind, AwdMark,
    AwdPick,
};
use hyperion_sim::time::LIGHT_CROSSING_L;
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion, Seed};

use donor_rows::{OBSERVED_DONOR_KINDS, awd_share_rows};
pub use marks::{
    FittedMark, MIN_NODE_WEIGHT, MarkPoint, NODES, conditional, unconditional, weighted_quantile,
};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The accreting white dwarfs' task's revision.
pub const AWD_VERSION: u32 = 0;

/// The accreting white dwarfs' block's items, in its order: each side's sampler, then each side's
/// share row.
pub const AWD_ITEMS: [&str; 16] = [
    "AWD_FAST_RESIDUALS_29",
    "AWD_FAST_NODE_RESIDUALS_29",
    "AWD_FAST_MEDIAN_SPANS_29",
    "AWD_FAST_MEDIANS_29",
    "AWD_FAST_LIMITS_29",
    "AWD_FAST_DONOR_KINDS_29",
    "AWD_SLOW_RESIDUALS_29",
    "AWD_SLOW_NODE_RESIDUALS_29",
    "AWD_SLOW_MEDIAN_SPANS_29",
    "AWD_SLOW_MEDIANS_29",
    "AWD_SLOW_LIMITS_29",
    "AWD_SLOW_DONOR_KINDS_29",
    "AWD_FAST_HOSTS_PER_FORMED_MASS_29",
    "AWD_FAST_LAYER_SHARES_29",
    "AWD_SLOW_HOSTS_PER_FORMED_MASS_29",
    "AWD_SLOW_LAYER_SHARES_29",
];

/// The bands a census runs: the three whose primaries can make a class's member.
pub const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// The populations with a census of their own, in [`POPULATIONS`]' order, and the manifest key of
/// each one's site.
pub const OWN_SITES: [(Population, &str); 5] = [
    (Population::YoungThinDisc, "young_thin_disc_site"),
    (Population::Bulge, "bulge_site"),
    (Population::LongBar, "long_bar_site"),
    (Population::NuclearDisc, "nuclear_disc_site"),
    (Population::Halo, "halo_site"),
];

/// The fingerprint's galaxy's seed: the manifests' (`binary_awd.toml`).
const FINGERPRINT_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The generator version the fingerprints' galaxy is built at: the refit's, the manifests'
/// (a test holds the four manifests to it).
const FINGERPRINT_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// The fingerprints' galaxy: the Milky Way fixture of [`FINGERPRINT_SEED`] at
/// [`FINGERPRINT_VERSION`], with the binary classes' scratch share rows, as a census galaxy is
/// built.
///
/// # Panics
///
/// If the Milky Way fixture does not build.
#[must_use]
fn fingerprint_galaxy() -> Galaxy {
    Galaxy::from_params_for_binary_refit(
        Seed::new(FINGERPRINT_SEED),
        GalaxyParams::milky_way_like(),
        FINGERPRINT_VERSION,
    )
    .expect("the Milky Way fixture builds")
}

/// The fingerprint's census: one layer-D cell of the solar circle's slab, of 21 records two of
/// whose pairs the engine makes accreting white dwarfs at 27, so that a change to the engine or to
/// the classes moves the probes.
const FINGERPRINT_CELL: CensusSite = CensusSite::new([2_048, 26_128, 0], [0, 0, 0]);

/// The censuses' parameters, from a manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct CensusParams {
    /// The Milky Way fixture's seed.
    pub seed: u64,
    /// The generator version whose rules the census galaxy is built under, with the binary
    /// classes' scratch share rows (module documentation).
    pub version: GeneratorVersion,
    /// The slab's records, layers C–E together.
    pub records: u64,
    /// Each population's own census's records, layers C–E together.
    pub population_records: u64,
    /// The young thin disc's own census's records: the manifest's `young_thin_disc_records`, or
    /// `population_records` without it.
    pub young_records: u64,
    /// The fit's parts, the first of `part_count`.
    pub parts: u64,
    /// The parts each census's cells are cut into.
    pub part_count: u64,
    /// Each population's own site, in [`OWN_SITES`]' order.
    pub sites: [(Population, CensusSite); 5],
}

impl CensusParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if one is missing, is not a number, or is out of range: a generator
    /// version under the sim's own or past `u32`, no records, no parts, more parts than
    /// `part_count`, or a site that is not six whole numbers, its centre within `i32` light-years
    /// and its three half-widths not negative.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let version = u32::try_from(manifest.u64("generator_version")?)
            .ok()
            .filter(|&v| v >= GENERATOR_VERSION.get())
            .map(GeneratorVersion::new)
            .ok_or_else(|| {
                ManifestParamError::new("generator_version", "the sim's version or a later one")
            })?;
        let population_records = manifest.u64("population_records")?;
        let young_records = if manifest.params().contains_key("young_thin_disc_records") {
            manifest.u64("young_thin_disc_records")?
        } else {
            population_records
        };
        let params = Self {
            seed: manifest.u64("seed")?,
            version,
            records: manifest.u64("records")?,
            population_records,
            young_records,
            parts: manifest.u64("parts")?,
            part_count: manifest.u64("part_count")?,
            sites: own_sites(manifest)?,
        };
        if params.records == 0 || params.population_records == 0 || params.young_records == 0 {
            return Err(ManifestParamError::new("records", "positive"));
        }
        if params.parts == 0 || params.parts > params.part_count {
            return Err(ManifestParamError::new(
                "parts",
                "positive and at most `part_count`",
            ));
        }
        Ok(params)
    }

    /// The census galaxy: the Milky Way fixture of the census's seed under its version's rules,
    /// with the binary classes' scratch share rows (module documentation).
    ///
    /// # Panics
    ///
    /// If the Milky Way fixture does not build.
    #[must_use]
    pub fn galaxy(&self) -> Galaxy {
        Galaxy::from_params_for_binary_refit(
            Seed::new(self.seed),
            GalaxyParams::milky_way_like(),
            self.version,
        )
        .expect("the Milky Way fixture builds")
    }
}

/// Each population's own site of [`OWN_SITES`] in `manifest`, as `[x, y, z, half_x, half_y,
/// half_z]` light-years.
fn own_sites(manifest: &Manifest) -> Result<[(Population, CensusSite); 5], ManifestParamError> {
    let mut out = [(Population::Halo, CensusSite::SOLAR_CIRCLE); 5];
    for (slot, &(population, key)) in out.iter_mut().zip(&OWN_SITES) {
        let bad = || ManifestParamError::new(key, "six whole numbers of light-years");
        let numbers: Vec<i64> = manifest
            .params()
            .get(key)
            .and_then(toml::Value::as_array)
            .and_then(|a| a.iter().map(toml::Value::as_integer).collect())
            .ok_or_else(bad)?;
        let [x, y, z, hx, hy, hz]: [i64; 6] = numbers.try_into().map_err(|_| bad())?;
        let centre = [x, y, z].map(|v| i32::try_from(v).map_err(|_| bad()));
        let half = [hx, hy, hz].map(|v| u32::try_from(v).map_err(|_| bad()));
        let [Ok(cx), Ok(cy), Ok(cz)] = centre else {
            return Err(bad());
        };
        let [Ok(hx), Ok(hy), Ok(hz)] = half else {
            return Err(bad());
        };
        *slot = (population, CensusSite::new([cx, cy, cz], [hx, hy, hz]));
    }
    Ok(out)
}

/// One census's members and records in one band, its parts joined in order.
#[derive(Debug, Clone, PartialEq)]
pub struct BandCensus {
    /// The band.
    pub band: MassBand,
    /// The records run, by population in [`POPULATIONS`]' order.
    pub scanned: [u64; 7],
    /// The members found.
    pub members: Vec<ClassMember>,
}

impl BandCensus {
    /// The members of `class` whose records are of `population`.
    #[must_use]
    fn count(&self, class: CarvedClass, population: Population) -> u64 {
        let n = self
            .members
            .iter()
            .filter(|m| m.class() == class && m.record().population() == population)
            .count();
        u64::try_from(n).expect("a census's members fit in u64")
    }
}

/// What the fit's censuses found: the slab's, then each population's own, each by band in
/// [`CENSUS_BANDS`]' order.
#[derive(Debug, Clone, PartialEq)]
pub struct Censuses {
    /// The solar circle's slab, every population.
    pub slab: [BandCensus; 3],
    /// Each population's own census, in [`OWN_SITES`]' order.
    pub own: Vec<(Population, [BandCensus; 3])>,
}

impl Censuses {
    /// The census whose rates `population`'s are: its own, or the slab's.
    #[must_use]
    pub fn of(&self, population: Population) -> &[BandCensus; 3] {
        self.own
            .iter()
            .find(|(p, _)| *p == population)
            .map_or(&self.slab, |(_, bands)| bands)
    }
}

/// One census to run: its site, whose records it takes, and its records in layers C–E together.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CensusPlan {
    site: CensusSite,
    whose: CensusRecords,
    records: u64,
}

/// A census could not run.
#[derive(Debug, thiserror::Error)]
pub enum RunCensusError {
    /// The thread pool could not be built.
    #[error(transparent)]
    Threads(#[from] BuildThreadPoolError),
    /// A part of a census ran out of its site's cells before it had its records.
    #[error(
        "census {census}'s site holds too few cells of layer {band:?}: a part found {found} of \
         its {wanted} records"
    )]
    TooFewCells {
        /// The census, 0 for the slab and then the populations' own in `OWN_SITES`' order.
        census: usize,
        /// The band.
        band: MassBand,
        /// The records the part found.
        found: u64,
        /// The records it wanted.
        wanted: u64,
    },
}

/// `task`'s run's error for a census's `error`: the thread pool's, or the manifest's sites'.
#[must_use]
fn census_error(task: &'static str, error: RunCensusError) -> RunTaskError {
    match error {
        RunCensusError::Threads(e) => RunTaskError::Threads(e),
        too_few @ RunCensusError::TooFewCells { .. } => RunTaskError::Input {
            task,
            source: Box::new(too_few),
        },
    }
}

/// Runs the slab's census and each population's own in `galaxy` on `threads` threads, each band's
/// records in its share of the systems and each part's records the band's over the parts, the
/// parts in order.
///
/// # Errors
///
/// [`RunCensusError::Threads`] if the thread pool cannot be built, and
/// [`RunCensusError::TooFewCells`] if a site holds too few cells for its records.
///
/// # Panics
///
/// If a site's cells leave the root cube or its frame (`class_census_in`).
pub fn run_censuses(
    galaxy: &Galaxy,
    params: &CensusParams,
    threads: NonZeroUsize,
) -> Result<Censuses, RunCensusError> {
    let plans: Vec<CensusPlan> = std::iter::once(CensusPlan {
        site: CensusSite::SOLAR_CIRCLE,
        whose: CensusRecords::All,
        records: params.records,
    })
    .chain(params.sites.iter().map(|&(population, site)| CensusPlan {
        site,
        whose: CensusRecords::Of(population),
        records: match population {
            Population::YoungThinDisc => params.young_records,
            Population::OldThinDisc
            | Population::ThickDisc
            | Population::Bulge
            | Population::LongBar
            | Population::NuclearDisc
            | Population::Halo => params.population_records,
        },
    }))
    .collect();
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = CENSUS_BANDS
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let per_part = |records: u64, band: MassBand| {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a record count under 2⁵³, rounded to a whole count"
        )]
        let in_band = (records as f64 * shares.share(band) / in_bands).round() as u64;
        in_band.div_ceil(params.parts)
    };
    // One job a plan, band and part, in that order, so that the joined censuses are the same on
    // any number of threads.
    let jobs: Vec<(usize, usize, u64)> = (0..plans.len())
        .flat_map(|p| {
            (0..CENSUS_BANDS.len()).flat_map(move |b| (0..params.parts).map(move |k| (p, b, k)))
        })
        .collect();
    let mut joined: Vec<[BandCensus; 3]> = plans
        .iter()
        .map(|_| {
            CENSUS_BANDS.map(|band| BandCensus {
                band,
                scanned: [0; 7],
                members: Vec::new(),
            })
        })
        .collect();
    let n = u64::try_from(jobs.len()).expect("a few hundred jobs");
    let mut short = None;
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let (p, b, k) = jobs[usize::try_from(i).expect("a job's index")];
                    let plan = plans[p];
                    let band = CENSUS_BANDS[b];
                    let wanted = per_part(plan.records, band);
                    let census = class_census_in(
                        galaxy,
                        band,
                        plan.site,
                        plan.whose,
                        wanted,
                        CensusPart::new(k, params.part_count),
                    );
                    (p, b, wanted, census)
                })
                .collect::<Vec<(usize, usize, u64, ClassCensus)>>()
        },
        |done| {
            for (p, b, wanted, census) in done {
                let found = census.scanned_total();
                if found < wanted && short.is_none() {
                    short = Some(RunCensusError::TooFewCells {
                        census: p,
                        band: CENSUS_BANDS[b],
                        found,
                        wanted,
                    });
                }
                let slot = &mut joined[p][b];
                for (count, population) in slot.scanned.iter_mut().zip(POPULATIONS) {
                    *count += census.scanned(population);
                }
                slot.members.extend(census.into_members());
            }
        },
    )?;
    if let Some(error) = short {
        return Err(error);
    }
    let mut plans = joined.into_iter();
    let slab = plans.next().expect("the slab's census is the first");
    let own = params.sites.iter().map(|s| s.0).zip(plans).collect();
    Ok(Censuses { slab, own })
}

/// One class's fitted share row: hosts per solar mass formed by population, in [`POPULATIONS`]'
/// order, and the share of the hosts whose primary lies in each of layers A–E.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShareRow {
    /// Hosts per solar mass formed, by population.
    pub hosts_per_formed_mass: [f64; 7],
    /// The layers' shares, A–E; they sum to 1.
    pub layer_shares: [f64; 5],
    /// The members a record each population's census found, its bands weighted by their shares
    /// of the systems.
    pub rate_per_system: [f64; 7],
}

/// The rows of `classes` from `censuses` in `galaxy`, scaled together so that the galaxy's
/// expected hosts of all of them are `total` (module documentation).
///
/// # Panics
///
/// If no census found a member of any of `classes`, or a census ran no record of a population.
#[must_use]
pub fn share_rows(
    galaxy: &Galaxy,
    censuses: &Censuses,
    classes: &[CarvedClass],
    total: f64,
) -> Vec<ShareRow> {
    let shares = BandShares::of(galaxy.mass_function()).as_array();
    let fields = galaxy.fields();
    let born: [f64; 7] = POPULATIONS.map(|population| {
        fields
            .components()
            .iter()
            .filter(|c| c.population() == population)
            .fold(0.0, |sum, c| sum + c.count())
    });
    // Each class's rate a system, per population and band: the census's members a record.
    let rates: Vec<[[f64; 5]; 7]> = classes
        .iter()
        .map(|&class| {
            POPULATIONS.map(|population| {
                let bands = censuses.of(population);
                let mut by_band = [0.0; 5];
                for census in bands {
                    let records = census.scanned[population.index()];
                    assert!(records > 0, "a census ran no record of {population:?}");
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "member and record counts under 2⁵³"
                    )]
                    let rate = census.count(class, population) as f64 / records as f64;
                    by_band[census.band.index()] = rate;
                }
                by_band
            })
        })
        .collect();
    let per_system = |rate: &[[f64; 5]; 7], p: usize| {
        rate[p]
            .iter()
            .zip(shares)
            .fold(0.0, |sum, (r, s)| sum + r * s)
    };
    let hosts = rates.iter().fold(0.0, |sum, rate| {
        (0..7).fold(sum, |sum, p| sum + per_system(rate, p) * born[p])
    });
    assert!(hosts > 0.0, "the censuses found members of the classes");
    let scale = total / hosts;
    let formed = galaxy.mean_formed_mass().value();
    rates
        .iter()
        .map(|rate| {
            let rate_per_system: [f64; 7] = std::array::from_fn(|p| per_system(rate, p));
            let mut layer_shares: [f64; 5] = std::array::from_fn(|b| {
                (0..7).fold(0.0, |sum, p| sum + rate[p][b] * shares[b] * born[p])
            });
            let sum = layer_shares.iter().sum::<f64>();
            for share in &mut layer_shares {
                *share /= sum;
            }
            ShareRow {
                hosts_per_formed_mass: rate_per_system.map(|r| scale * r / formed),
                layer_shares,
                rate_per_system,
            }
        })
        .collect()
}

/// Where a fitted mark's members came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkSource {
    /// Its own class's members.
    Own,
    /// Both classes' members, its own being too few.
    Pooled,
    /// None: the sim's sampler's law, the scratch one's where the refit at 27 kept it.
    Scratch,
}

/// One accreting white dwarf class's fitted sampler: its marks in [`AwdMark`]'s order, where each
/// came from, and the donor's kinds' shares in [`AwdDonorKind`]'s order.
#[derive(Debug, Clone, PartialEq)]
pub struct AwdSampler {
    /// The marks.
    pub marks: [FittedMark; 11],
    /// Where each mark's members came from.
    pub sources: [MarkSource; 11],
    /// The donor's kinds' shares.
    pub donor_kinds: [f64; 3],
    /// The class's members.
    pub members: usize,
}

/// The accreting white dwarfs' fit parameters, from the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct AwdParams {
    /// The censuses'.
    pub census: CensusParams,
    /// The least members a mark, or a node's law, is fitted to.
    pub min_members: usize,
    /// The greatest log₁₀ of a recurrence, years, that a slow host draws, under the clock's reach.
    pub max_log_recurrence: f64,
}

impl AwdParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] as [`CensusParams::of`], or if the least members are under 20 or
    /// the recurrence's cap not above L's or above 11.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let params = Self {
            census: CensusParams::of(manifest)?,
            min_members: usize::try_from(manifest.u64("min_members")?)
                .map_err(|_| ManifestParamError::new("min_members", "a count"))?,
            max_log_recurrence: manifest.f64("max_log_recurrence")?,
        };
        if params.min_members < 20 {
            return Err(ManifestParamError::new("min_members", "at least 20"));
        }
        if !(params.max_log_recurrence > log_l() && params.max_log_recurrence <= 11.0) {
            return Err(ManifestParamError::new(
                "max_log_recurrence",
                "above L's and at most 11",
            ));
        }
        Ok(params)
    }
}

/// log₁₀ of L, years.
#[must_use]
fn log_l() -> f64 {
    math::log10(LIGHT_CROSSING_L.as_julian_years_f64())
}

/// The two classes, fast then slow.
pub const AWD_CLASSES: [CarvedClass; 2] =
    [CarvedClass::AccretingWdFast, CarvedClass::AccretingWdSlow];

/// What one accreting white dwarf member, its system `age` years old at the epoch, gives `mark`:
/// its condition and value, or `None` where it defines no such mark (a donor of another kind, a
/// value that is not finite). Its recurrence is held at 10^`cap` years.
#[must_use]
fn awd_pair(m: &AccretingMarks, age: f64, mark: AwdMark, cap: f64) -> Option<(f64, f64)> {
    let log = |v: f64| (v > 0.0).then(|| math::log10(v));
    let log_recurrence = log(m.recurrence.value()).map(|r| r.min(cap));
    let log_rate = log(m.transfer_rate.value());
    let log_period = m.period.and_then(|p| log(p.value() / 3_600.0));
    let log_donor = log(m.donor_mass.value());
    let log_m0 = log(m.white_dwarf_initial_mass.value());
    let kind = m.donor_kind();
    let of_kind = |k: AwdDonorKind, x: Option<f64>, y: Option<f64>| {
        if kind == k { Some((x?, y?)) } else { None }
    };
    let pair = match mark {
        AwdMark::Recurrence => Some((0.0, log_recurrence?)),
        AwdMark::WhiteDwarfMass => Some((log_recurrence?, m.white_dwarf_mass.value())),
        AwdMark::MainSequencePeriod => of_kind(AwdDonorKind::MainSequence, log_rate, log_period),
        AwdMark::HeliumPeriod => of_kind(AwdDonorKind::Helium, log_rate, log_period),
        AwdMark::GiantPeriod => of_kind(AwdDonorKind::Giant, log_rate, log_period),
        AwdMark::MainSequenceDonorMass => {
            of_kind(AwdDonorKind::MainSequence, log_period, log_donor)
        }
        AwdMark::HeliumDonorMass => of_kind(AwdDonorKind::Helium, log_period, log_donor),
        AwdMark::GiantDonorMass => of_kind(AwdDonorKind::Giant, log_period, log_donor),
        AwdMark::PrimaryInitialMass => Some((m.white_dwarf_mass.value(), log_m0?)),
        AwdMark::DonorInitialMass => Some((log_donor?, log(m.donor_initial_mass.value())?)),
        AwdMark::Age => Some((log_m0?, log(age)?)),
    };
    pair.filter(|(x, y)| x.is_finite() && y.is_finite())
}

/// The accreting white dwarfs' members of `class` among `members`, with each one's system's age at
/// the epoch, Julian years.
#[must_use]
fn awd_members<'a>(
    members: impl IntoIterator<Item = &'a ClassMember>,
    class: CarvedClass,
) -> Vec<(AccretingMarks, f64)> {
    members
        .into_iter()
        .filter(|m| m.class() == class)
        .filter_map(|m| {
            m.accreting()
                .map(|a| (*a, m.record().age_at_epoch().value()))
        })
        .collect()
}

/// `points`, each of a kind (its place in `shares`), weighted so that each kind's share of their
/// total weight is its share in `shares` among the kinds present (module documentation): each
/// point of kind k weighs `shares[k]` over its kind's share of the points. A kind whose share is
/// zero drops out, since the class never draws it.
///
/// # Panics
///
/// If a kind is past `shares`, or there are 2³² points or more.
#[must_use]
pub fn weighted_to_shares(points: &[(MarkPoint, usize)], shares: &[f64]) -> Vec<MarkPoint> {
    let mut counts = vec![0_u32; shares.len()];
    for &(_, k) in points {
        counts[k] += 1;
    }
    let n = f64::from(u32::try_from(points.len()).expect("a mark's points number under 2³²"));
    points
        .iter()
        .filter(|(_, k)| shares[*k] > 0.0)
        .map(|&(point, k)| MarkPoint {
            weight: shares[k] * n / f64::from(counts[k]),
            ..point
        })
        .collect()
}

/// Fits each side's sampler from the slab's members, takes its donor kinds' shares from
/// `donor_kinds`, the rows' (module documentation), and weights each mark's members so that their
/// kinds are in those shares ([`weighted_to_shares`]). The fast side's recurrence keeps its
/// members' rate in its lowest sixteenth (`FittedMark::with_rate_preserving_floor`): its 1 ÷ P is
/// the galaxy's novae.
///
/// # Panics
///
/// Never: every donor's kind is one of [`AwdDonorKind::ALL`].
#[must_use]
pub fn awd_samplers(
    censuses: &Censuses,
    params: &AwdParams,
    donor_kinds: &[[f64; 3]; 2],
) -> [AwdSampler; 2] {
    let sides =
        AWD_CLASSES.map(|class| awd_members(censuses.slab.iter().flat_map(|c| &c.members), class));
    let pooled: Vec<(AccretingMarks, f64)> = sides.iter().flatten().copied().collect();
    let cap = params.max_log_recurrence;
    // A mark too few members define keeps the side's sampler's law as committed: P15.T10.b's
    // refit at 29, itself the earlier refit's or the scratch law where a refit kept it.
    let kept = [&AWD_FAST_SAMPLER, &AWD_SLOW_SAMPLER];
    // A side with no member, which only a smoke run's census can have, keeps the side's sampler's
    // shares.
    let side_kinds: [[f64; 3]; 2] = std::array::from_fn(|side| {
        if donor_kinds[side].iter().sum::<f64>() > 0.0 {
            donor_kinds[side]
        } else {
            let kinds = kept[side].pick(AwdPick::DonorKind).weights();
            std::array::from_fn(|k| kinds[k])
        }
    });
    let points = |members: &[(AccretingMarks, f64)], mark: AwdMark, side: usize| {
        let raw: Vec<(MarkPoint, usize)> = members
            .iter()
            .filter_map(|(m, age)| {
                let (condition, value) = awd_pair(m, *age, mark, cap)?;
                let kind = AwdDonorKind::ALL
                    .iter()
                    .position(|&k| k == m.donor_kind())
                    .expect("every kind is in `AwdDonorKind::ALL`");
                let point = MarkPoint {
                    condition,
                    value,
                    weight: 1.0,
                };
                Some((point, kind))
            })
            .collect();
        weighted_to_shares(&raw, &side_kinds[side])
    };
    std::array::from_fn(|side| {
        let own = &sides[side];
        let mut sources = [MarkSource::Scratch; 11];
        let mut marks: [FittedMark; 11] = std::array::from_fn(|k| {
            let mark = AwdMark::ALL[k];
            let fit = |points: &[MarkPoint]| match mark {
                AwdMark::Recurrence if AWD_CLASSES[side] == CarvedClass::AccretingWdFast => {
                    unconditional(points).with_rate_preserving_floor(points)
                }
                AwdMark::Recurrence => unconditional(points),
                AwdMark::WhiteDwarfMass
                | AwdMark::MainSequencePeriod
                | AwdMark::HeliumPeriod
                | AwdMark::GiantPeriod
                | AwdMark::MainSequenceDonorMass
                | AwdMark::HeliumDonorMass
                | AwdMark::GiantDonorMass
                | AwdMark::PrimaryInitialMass
                | AwdMark::DonorInitialMass
                | AwdMark::Age => conditional(points, params.min_members),
            };
            let mine = points(own, mark, side);
            if mine.len() >= params.min_members {
                sources[k] = MarkSource::Own;
                return fit(&mine);
            }
            let all = points(&pooled, mark, side);
            // The recurrence is a side's alone: pooled, it would cross L.
            if all.len() >= params.min_members && mark != AwdMark::Recurrence {
                sources[k] = MarkSource::Pooled;
                return fit(&all);
            }
            FittedMark::of_scratch(kept[side].mark(mark))
        });
        // No draw reaches the Chandrasekhar mass, which a census of an engine without its dwarf
        // limit could pass (the refit at 27 before P11.T4's `dwarf_limit`, to 2.12 M☉).
        let dwarf = &mut marks[AwdMark::WhiteDwarfMass.index()].limits.1;
        *dwarf = dwarf.min(ACCRETING_WHITE_DWARF_MAX_MASS);
        AwdSampler {
            marks,
            sources,
            donor_kinds: side_kinds[side],
            members: own.len(),
        }
    })
}

/// The accreting white dwarfs' fit: the censuses, each side's row and sampler.
#[derive(Debug, Clone, PartialEq)]
pub struct AwdFit {
    /// The parameters.
    pub params: AwdParams,
    /// The fast and the slow classes' rows.
    pub rows: [ShareRow; 2],
    /// Each donor kind's scale, hosts in the galaxy per census member a record and system born,
    /// in [`AwdDonorKind`]'s order (`donor_rows::DonorKindRows::scales`).
    pub scales: [Option<f64>; 3],
    /// The fast and the slow classes' samplers.
    pub samplers: [AwdSampler; 2],
    /// The records the slab's census ran, and each population's own.
    pub records: (u64, u64),
}

/// Runs the accreting white dwarfs' fit on `threads` threads: the rows by donor kind
/// ([`awd_share_rows`] at [`OBSERVED_DONOR_KINDS`]) and each side's sampler, its members weighted
/// to their side's kinds' shares.
///
/// # Errors
///
/// As [`run_censuses`].
///
/// # Panics
///
/// If the Milky Way fixture does not build, or as [`awd_share_rows`]: the censuses find no member
/// of the classes, or run no record of a population.
pub fn fit_awd(params: &AwdParams, threads: NonZeroUsize) -> Result<AwdFit, RunCensusError> {
    let galaxy = params.census.galaxy();
    let censuses = run_censuses(&galaxy, &params.census, threads)?;
    let by_kind = awd_share_rows(&galaxy, &censuses, &OBSERVED_DONOR_KINDS);
    let records = (
        censuses
            .slab
            .iter()
            .map(|c| c.scanned.iter().sum::<u64>())
            .sum(),
        censuses
            .own
            .iter()
            .flat_map(|(_, bands)| bands.iter().map(|c| c.scanned.iter().sum::<u64>()))
            .sum(),
    );
    Ok(AwdFit {
        params: params.clone(),
        rows: by_kind.rows,
        scales: by_kind.scales,
        samplers: awd_samplers(&censuses, params, &by_kind.donor_kinds),
        records,
    })
}

/// A side's prefix in its items' names.
const SIDES: [&str; 2] = ["FAST", "SLOW"];

/// A side's name in its items' docs.
const SIDE_NAMES: [&str; 2] = ["fast", "slow"];

/// A `pub static` array `name` of 17-value `rows` under `doc`, one row a line: as
/// [`TableItem::Nested`] writes a `const`, since Clippy refuses so large a `const` array
/// (`large_const_arrays`).
///
/// # Panics
///
/// If a value is not finite, which a fitted law's never is (`marks`' points are checked).
#[must_use]
fn static_rows(name: &str, doc: &[String], rows: &[[f64; NODES]]) -> TableItem {
    assert!(
        rows.iter().flatten().all(|v| v.is_finite()),
        "item `{name}` holds a value that is not finite"
    );
    let mut out = String::new();
    // Writing to a String cannot fail.
    for line in doc {
        let _ = writeln!(out, "/// {line}");
    }
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(
        out,
        "pub static {name}: [[f64; {NODES}]; {}] = [",
        rows.len()
    );
    for row in rows {
        let _ = writeln!(out, "    [{}],", row.map(literal).join(", "));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The block's items for `fit`.
#[must_use]
fn awd_items(fit: &AwdFit) -> Vec<TableItem> {
    let mut items = Vec::with_capacity(AWD_ITEMS.len());
    for (side, sampler) in fit.samplers.iter().enumerate() {
        let (prefix, name) = (SIDES[side], SIDE_NAMES[side]);
        items.push(TableItem::Nested {
            name: format!("AWD_{prefix}_RESIDUALS_29"),
            doc: vec![
                format!("The {name} accreting white dwarfs' marks' residual quantiles at the ranks k ÷ 16 over all their"),
                "members, in [`AwdMark`]'s order, each in its mark's unit.".to_owned(),
            ],
            rows: sampler.marks.iter().map(|m| m.residual.to_vec()).collect(),
        });
        items.push(static_rows(
            &format!("AWD_{prefix}_NODE_RESIDUALS_29"),
            &[
                format!("The {name} accreting white dwarfs' marks' residual laws at their medians' 17 nodes: each mark's"),
                "17 rows in [`AwdMark`]'s order, each row its node's quantiles at the ranks k ÷ 16; the".to_owned(),
                "recurrence's, which is unconditional, are not read.".to_owned(),
            ],
            &sampler.marks.iter().flat_map(FittedMark::node_rows).collect::<Vec<_>>(),
        ));
        items.push(TableItem::Tuples {
            name: format!("AWD_{prefix}_MEDIAN_SPANS_29"),
            doc: vec![
                format!("The {name} accreting white dwarfs' marks' medians' spans of their conditions, first and last"),
                "node, in [`AwdMark`]'s order; the recurrence's, which is unconditional, is not read.".to_owned(),
            ],
            rows: sampler.marks.iter().map(|m| vec![m.span.0, m.span.1]).collect(),
        });
        items.push(TableItem::Nested {
            name: format!("AWD_{prefix}_MEDIANS_29"),
            doc: vec![
                format!("The {name} accreting white dwarfs' marks' medians at their 17 nodes, in [`AwdMark`]'s order;"),
                "the recurrence's is zero.".to_owned(),
            ],
            rows: sampler.marks.iter().map(|m| m.median.to_vec()).collect(),
        });
        items.push(TableItem::Tuples {
            name: format!("AWD_{prefix}_LIMITS_29"),
            doc: vec![format!(
                "The {name} accreting white dwarfs' marks' least and greatest values, in [`AwdMark`]'s order."
            )],
            rows: sampler.marks.iter().map(|m| vec![m.limits.0, m.limits.1]).collect(),
        });
        items.push(TableItem::Array {
            name: format!("AWD_{prefix}_DONOR_KINDS_29"),
            doc: vec![format!(
                "The {name} accreting white dwarfs' donors' kinds' shares, in [`AwdDonorKind`]'s order."
            )],
            values: sampler.donor_kinds.to_vec(),
        });
    }
    for (side, row) in fit.rows.iter().enumerate() {
        let (prefix, name) = (SIDES[side], SIDE_NAMES[side]);
        items.push(TableItem::Array {
            name: format!("AWD_{prefix}_HOSTS_PER_FORMED_MASS_29"),
            doc: vec![format!(
                "The {name} accreting white dwarfs' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order."
            )],
            values: row.hosts_per_formed_mass.to_vec(),
        });
        items.push(TableItem::Array {
            name: format!("AWD_{prefix}_LAYER_SHARES_29"),
            doc: vec![format!(
                "The share of the {name} accreting white dwarfs' hosts whose primary lies in each layer, A–E."
            )],
            values: row.layer_shares.to_vec(),
        });
    }
    items
}

/// The block's contents for `fit`.
#[must_use]
pub fn render_awd(fit: &AwdFit) -> RustTable {
    RustTable {
        summary: vec![
            "The accreting white dwarfs' samplers and share rows refitted against plan 11's engine at"
                .to_owned(),
            "generator version 29 on the generator's own pairs (plan 15, P15.T10.b).".to_owned(),
        ],
        notes: awd_notes(fit),
        items: awd_items(fit),
    }
}

/// The block's notes after its header.
#[must_use]
fn awd_notes(fit: &AwdFit) -> Vec<String> {
    let p = &fit.params;
    let text = format!(
        "\
Read by `AWD_FAST_SAMPLER`, `AWD_SLOW_SAMPLER` and `CLASS_SHARES`, after the block. Each
class's marks are fitted to its own members of the engine near the solar circle at generator
version {version}'s rules (hyperion-fit's `tasks::binary_classes`): a median that follows the
mark's condition on 17 nodes, the weighted median under a triangle two node spacings wide, the
residual's quantiles about it, and its law at each node, from the members under the node's
triangle where their Kish size is {min} or more. Each mark's members are weighted so that their
donor kinds are in the class's shares of them. A mark with under {min} members of its class
takes both classes' members, and with under {min} of both keeps `AWD_SAMPLER`'s law. The fast
recurrence's lowest quantile keeps its members' mean of 1 ÷ P over their lowest sixteenth. A
slow host's recurrence is held at 10^{cap} years, under the universe clock's reach; the
engine's longest, beyond it, never erupt in the Galaxy's age. The rows scale each donor kind to
its observed figure (`donor_rows::OBSERVED_DONOR_KINDS`, plan 11's design note 11), each kind's
rates each population's members a record, its bands weighted by their shares of the systems.
The slab's census ran {slab} records and the populations' own {own}.",
        version = p.census.version.get(),
        min = p.min_members,
        cap = p.max_log_recurrence,
        slab = fit.records.0,
        own = fit.records.1,
    );
    text.lines().map(str::to_owned).collect()
}

/// The acceptance's figures for `fit`: the kinds' scales, then each side's members, the marks'
/// sources and nodes with laws of their own, the rows' layer shares and rates.
#[must_use]
fn awd_acceptance(fit: &AwdFit) -> String {
    let mut out = format!(
        "kind scales {}; ",
        fit.scales
            .iter()
            .map(|s| s.map_or_else(|| "none".to_owned(), |s| format!("{s:.4e}")))
            .collect::<Vec<_>>()
            .join("/")
    );
    for (side, (sampler, row)) in fit.samplers.iter().zip(&fit.rows).enumerate() {
        let pooled = sampler
            .sources
            .iter()
            .zip(AwdMark::ALL)
            .filter(|(s, _)| **s != MarkSource::Own)
            .map(|(s, m)| format!("{m:?} {s:?}"))
            .collect::<Vec<_>>();
        let nodes = sampler
            .marks
            .iter()
            .map(|m| m.own_nodes.to_string())
            .collect::<Vec<_>>()
            .join("/");
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "{}{}: {} members, own nodes {nodes}, donors {:.3}/{:.3}/{:.3}, layers C {:.3} D {:.3} \
             E {:.3}, rates a system {}{}",
            if side == 0 { "" } else { "; " },
            SIDE_NAMES[side],
            sampler.members,
            sampler.donor_kinds[0],
            sampler.donor_kinds[1],
            sampler.donor_kinds[2],
            row.layer_shares[2],
            row.layer_shares[3],
            row.layer_shares[4],
            row.rate_per_system
                .iter()
                .map(|r| format!("{r:.2e}"))
                .collect::<Vec<_>>()
                .join("/"),
            if pooled.is_empty() {
                String::new()
            } else {
                format!(", not its own: {}", pooled.join(", "))
            },
        );
    }
    out.push_str(
        "; the K–S tests against the other half of the census are the sim's slow \
         `fitted_tables::the_accreting_white_dwarf_samplers_against_the_engine`",
    );
    out
}

/// The task: P15.T10.b's accreting white dwarfs, slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BinaryAwdTask;

impl FitTask for BinaryAwdTask {
    fn name(&self) -> &'static str {
        "binary_awd"
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
        AWD_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &AWD_ITEMS
    }

    /// The galaxy's band shares of layers C–E, its systems born by population and its mass
    /// formed per system, which the rows read; and the engine on one layer-D cell's records: its
    /// records, its members, and each accreting member's white dwarf mass, donor mass and rate.
    fn fingerprint(&self) -> SimFingerprint {
        let galaxy = fingerprint_galaxy();
        let shares = BandShares::of(galaxy.mass_function());
        let mut probes: Vec<(String, f64)> = CENSUS_BANDS
            .iter()
            .map(|&b| (format!("band share {b:?}"), shares.share(b)))
            .collect();
        for population in POPULATIONS {
            let born = galaxy
                .fields()
                .components()
                .iter()
                .filter(|c| c.population() == population)
                .fold(0.0, |sum, c| sum + c.count());
            probes.push((format!("born {population:?}"), born));
        }
        probes.push((
            "mean formed mass".to_owned(),
            galaxy.mean_formed_mass().value(),
        ));
        let census = class_census_in(
            &galaxy,
            MassBand::D,
            FINGERPRINT_CELL,
            CensusRecords::All,
            u64::MAX,
            CensusPart::new(0, 1),
        );
        #[expect(clippy::cast_precision_loss, reason = "a cell's few records")]
        probes.push(("cell records".to_owned(), census.scanned_total() as f64));
        #[expect(clippy::cast_precision_loss, reason = "a cell's few members")]
        probes.push(("cell members".to_owned(), census.members().len() as f64));
        for (i, m) in census.members().iter().enumerate() {
            if let Some(a) = m.accreting() {
                probes.push((
                    format!("member {i} white dwarf"),
                    a.white_dwarf_mass.value(),
                ));
                probes.push((format!("member {i} donor"), a.donor_mass.value()));
                probes.push((format!("member {i} rate"), a.transfer_rate.value()));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = AwdParams::of(manifest)?;
        let fit = fit_awd(&params, threads).map_err(|e| census_error("binary_awd", e))?;
        Ok(TaskOutput {
            table: render_awd(&fit),
            source:
                "plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 \
                     builds it) and classes (P11.T5), over plan 06's tracks, at the manifest's \
                     generator version's rules; the rows by donor kind against Pala et al. 2020, \
                     MNRAS 494, 3799, Carter et al. 2013, MNRAS 429, 2143, and Lü, Yungelson and \
                     Han 2006, MNRAS 372, 1389, the total against the brainstorm's 6–12 × 10⁶"
                    .to_owned(),
            acceptance: awd_acceptance(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed manifest, with `edit` applied to its text.
    fn manifest(edit: impl Fn(&str) -> String) -> Manifest {
        let text = edit(include_str!("../../manifests/binary_awd.toml"));
        Manifest::from_bytes("manifests/binary_awd.toml".as_ref(), text.into_bytes()).unwrap()
    }

    /// The committed manifest reads, the young thin disc's census taking the populations' records
    /// without a count of its own; a census with no records or more parts than cuts, a site with a
    /// negative half-width or of five numbers, and a cap on the recurrence beyond the clock are
    /// refused.
    #[test]
    fn the_manifest_s_parameters_are_checked() {
        let params = AwdParams::of(&manifest(str::to_owned)).unwrap();
        assert_eq!(params.census.records, 10_000_000);
        assert_eq!(
            params.census.young_records,
            params.census.population_records
        );
        assert_eq!(params.census.sites[0].0, Population::YoungThinDisc);
        let refused = |from: &str, to: &str, key: &str| {
            let error = AwdParams::of(&manifest(|t| t.replacen(from, to, 1))).unwrap_err();
            assert!(error.to_string().contains(key), "{from} → {to}: {error}");
        };
        assert_eq!(params.census.version, GeneratorVersion::new(29));
        refused(
            "generator_version = 29",
            "generator_version = 23",
            "generator_version",
        );
        refused("records = 10_000_000", "records = 0", "records");
        refused("parts = 32", "parts = 65", "parts");
        refused(
            "[0, 0, 0, 800, 800, 200]",
            "[0, 0, 0, 800, -800, 200]",
            "nuclear_disc_site",
        );
        refused(
            "[0, 0, 0, 800, 800, 200]",
            "[0, 0, 800, 800, 200]",
            "nuclear_disc_site",
        );
        refused(
            "max_log_recurrence = 10.0",
            "max_log_recurrence = 12.0",
            "max_log_recurrence",
        );
        refused("min_members = 50", "min_members = 5", "min_members");
    }

    /// The four binary manifests fit at the fingerprints' version, so that `check`'s probes are
    /// the census galaxy's.
    #[test]
    fn the_manifests_fit_at_the_fingerprints_version() {
        for (path, text) in [
            (
                "binary_awd.toml",
                include_str!("../../manifests/binary_awd.toml"),
            ),
            (
                "binary_awd.smoke.toml",
                include_str!("../../manifests/binary_awd.smoke.toml"),
            ),
            (
                "binary_xrb.toml",
                include_str!("../../manifests/binary_xrb.toml"),
            ),
            (
                "binary_xrb.smoke.toml",
                include_str!("../../manifests/binary_xrb.smoke.toml"),
            ),
        ] {
            let manifest = Manifest::from_bytes(path.as_ref(), text.as_bytes().to_vec()).unwrap();
            let params = CensusParams::of(&manifest).unwrap();
            assert_eq!(params.version, FINGERPRINT_VERSION, "{path}");
        }
    }

    /// The points are weighted so that each kind present takes its share of their weight: kinds
    /// of three points, one and one under shares of a half, a half and none come out as equal
    /// halves, and the kind of no share drops out.
    #[test]
    fn points_are_weighted_to_their_kinds_shares() {
        let point = |value: f64| MarkPoint {
            condition: 0.0,
            value,
            weight: 1.0,
        };
        let points = [
            (point(1.0), 0),
            (point(2.0), 0),
            (point(3.0), 0),
            (point(4.0), 1),
            (point(5.0), 2),
        ];
        let weighted = weighted_to_shares(&points, &[0.5, 0.5, 0.0]);
        assert_eq!(weighted.len(), 4);
        let of = |range: std::ops::Range<usize>| {
            weighted[range].iter().fold(0.0, |sum, p| sum + p.weight)
        };
        assert!((of(0..3) - of(3..4)).abs() < 1e-12, "{weighted:?}");
        assert!(weighted.iter().all(|p| p.value < 5.0));
    }
}
