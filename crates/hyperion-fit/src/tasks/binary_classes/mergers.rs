//! Tasks `binary_merger` and `binary_nsm` (plan 15, P15.T10.b's merger step, held at 31): the
//! stellar mergers' and the neutron-star mergers' samplers and share rows, fitted to the engine's
//! own mergers through the sim's merger census (`stellar::binary::testing::merger_census_in`) of
//! the pairs the generator draws at each record's first attempt.
//!
//! # The censuses
//!
//! Each task runs the solar circle's slab and each scarce population's own site, as
//! `binary_awd`'s censuses are run (the parent module's), but over the merger class's own bands
//! (`merger_census_bands`: every stellar band for the stellar mergers, D and E for the
//! neutron-star mergers), each band's records in its share of the bands' systems, part by part. A
//! member is weighted by its age hazard at its merger age, so that the weighted members are the
//! mergers in the source horizon now ([`MergerMember::weight`]), and read as the class's sampler
//! reads it (`StellarMergerReading`, `NsmReading`).
//!
//! # The stellar mergers
//!
//! The class draws one of three channels ([`MergerChannel`]); an engine merger the channels do not
//! cover (a white dwarf's, two evolved stars', a heavier main-sequence donor's out of contact) is
//! left out, its share of the weight reported. The channel choice is the covered members' weighted
//! shares. The layer choice serves the contact and the evolved channels, and the scan restricts it
//! at each merger's T to the layers its channel opens there, so it is iterated to the fixed point
//! at which the class's realised layers, over the members' T, are the members' own: w ← w t ÷
//! r(w), t the members' weighted layer shares and r(w) the class's under w.
//!
//! A mark the class draws after the layer (the contact pair's primary rank and mass ratio, the
//! evolved donor's rank and its companion's ratio by the donor's kind) is conditional on the layer:
//! the condition is the layer's index, 0–4, across a median of zero on the span (0, 4), and the
//! node at 4b holds layer b's law where its members' Kish size is at least `min_members`, the
//! mark's own law otherwise; the sim's scan passes the layer as the condition. The age rank and the
//! dynamical pair's marks are unconditional. A mark whose members' Kish size is under
//! `min_members` keeps the scratch sampler's law.
//!
//! # The neutron-star mergers
//!
//! The class draws a pair of two neutron stars or a black hole's and a neutron star's; a merger of
//! two black holes is left out, its share reported. Each mark is unconditional: the neutron stars'
//! masses (both of a double neutron star), the black hole's log₁₀ mass, the progenitors' ranks in
//! the galaxy's mass function over the sim's fixed ranges (held at their ends: a progenitor outside
//! them is a finding, its share reported) and the age rank.
//!
//! # The rows
//!
//! Each class's row is its covered members' weighted rate a record, by population and band, its
//! bands weighted by their shares of the systems, scaled so that the galaxy's mergers in the source
//! horizon are the manifest's `total_hosts` (plan 11's design note 11: counts are observed, the
//! engine gives their shape).

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::imf::{BandShares, MassBand};
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::id::SystemId;
use hyperion_sim::math;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, MergerCensus, MergerCensusPlan, MergerMember,
    NsmReading, StellarMergerReading, merger_census_bands, merger_census_in,
};
use hyperion_sim::stellar::sse::MIN_INITIAL_MASS;
use hyperion_sim::tables::binary::{
    MERGER_SAMPLER, MergerChannel, MergerMark, MergerPick, NSM_SAMPLER, NsmMark, NsmPairKind,
    NsmPick,
};

use super::marks::{FittedMark, MarkPoint, NODES, unconditional};
use super::{CensusParams, MarkSource, RunCensusError, ShareRow, census_error};
use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// `binary_merger`'s revision.
pub const MERGER_VERSION: u32 = 0;

/// `binary_nsm`'s revision.
pub const NSM_VERSION: u32 = 0;

/// `binary_merger`'s block's items.
pub const MERGER_ITEMS: [&str; 9] = [
    "MERGER_RESIDUALS_31",
    "MERGER_NODE_RESIDUALS_31",
    "MERGER_MEDIAN_SPANS_31",
    "MERGER_MEDIANS_31",
    "MERGER_LIMITS_31",
    "MERGER_LAYERS_31",
    "MERGER_CHANNELS_31",
    "MERGER_HOSTS_PER_FORMED_MASS_31",
    "MERGER_LAYER_SHARES_31",
];

/// `binary_nsm`'s block's items.
pub const NSM_ITEMS: [&str; 5] = [
    "NSM_RESIDUALS_31",
    "NSM_LIMITS_31",
    "NSM_PAIR_KINDS_31",
    "NSM_HOSTS_PER_FORMED_MASS_31",
    "NSM_LAYER_SHARES_31",
];

/// The stellar merger the fingerprint's census reads: one of the merger census's at 31 (the
/// sim's `testing::mergers` tests pin it), whose cell in each layer the census takes.
const FINGERPRINT_MERGER: u64 = 0x81d8_b2e0_0000_0001;

/// The neutron-star merger the fingerprint's census reads, likewise.
const FINGERPRINT_NSM: u64 = 0x81db_b2e0_0000_000d;

/// The merger fits' parameters, from a manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerParams {
    /// The censuses' (`CensusParams`, whose records count the class's bands).
    pub census: CensusParams,
    /// The least Kish size a mark, or a layer's law, is fitted to.
    pub min_members: usize,
    /// The class's mergers in the source horizon at Milky Way parameters.
    pub total_hosts: f64,
}

impl MergerParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] as [`CensusParams::of`], or if the least members are under 20 or the
    /// total is not positive and finite.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let params = Self {
            census: CensusParams::of(manifest)?,
            min_members: usize::try_from(manifest.u64("min_members")?)
                .map_err(|_| ManifestParamError::new("min_members", "a count"))?,
            total_hosts: manifest.f64("total_hosts")?,
        };
        if params.min_members < 20 {
            return Err(ManifestParamError::new("min_members", "at least 20"));
        }
        if !(params.total_hosts.is_finite() && params.total_hosts > 0.0) {
            return Err(ManifestParamError::new(
                "total_hosts",
                "positive and finite",
            ));
        }
        Ok(params)
    }
}

/// One census's readings in one band, its parts joined in order: the records it ran by population,
/// in [`POPULATIONS`]' order, and each member's record's population and reading.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerBandCensus<R> {
    /// The band.
    pub band: MassBand,
    /// The records run, by population.
    pub scanned: [u64; 7],
    /// The members found, each with its record's population.
    pub readings: Vec<(Population, R)>,
}

/// What a merger fit's censuses found: the slab's, then each population's own, each by band in the
/// class's bands' order.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerCensuses<R> {
    /// The solar circle's slab, every population.
    pub slab: Vec<MergerBandCensus<R>>,
    /// Each population's own census.
    pub own: Vec<(Population, Vec<MergerBandCensus<R>>)>,
}

impl<R> MergerCensuses<R> {
    /// The census whose rates `population`'s are: its own, or the slab's.
    #[must_use]
    pub fn of(&self, population: Population) -> &[MergerBandCensus<R>] {
        self.own
            .iter()
            .find(|(p, _)| *p == population)
            .map_or(&self.slab, |(_, bands)| bands)
    }

    /// Every reading, the slab's first, as `(population, reading)`.
    pub fn readings(&self) -> impl Iterator<Item = &(Population, R)> {
        self.slab
            .iter()
            .chain(self.own.iter().flat_map(|(_, bands)| bands))
            .flat_map(|census| &census.readings)
    }

    /// The records run, every census's.
    #[must_use]
    pub fn records(&self) -> u64 {
        self.slab
            .iter()
            .chain(self.own.iter().flat_map(|(_, bands)| bands))
            .map(|c| c.scanned.iter().sum::<u64>())
            .sum()
    }
}

/// The censuses `params` runs: the slab's, of every population, then each population's own, each
/// with its site, whose records it takes and how many.
fn census_plans(params: &CensusParams) -> Vec<(CensusSite, CensusRecords, u64)> {
    std::iter::once((params.slab, CensusRecords::All, params.records))
        .chain(params.sites.iter().map(|&(population, site)| {
            let records = match population {
                Population::YoungThinDisc => params.young_records,
                Population::OldThinDisc
                | Population::ThickDisc
                | Population::Bulge
                | Population::LongBar
                | Population::NuclearDisc
                | Population::Halo => params.population_records,
            };
            (site, CensusRecords::Of(population), records)
        }))
        .collect()
}

/// Runs the slab's merger census of `class` and each population's own in `galaxy` on `threads`
/// threads, each of the class's bands' records in its share of their systems and each part's
/// records the band's over the parts, the parts in order, each member read by `read` (`None` drops
/// it).
///
/// # Errors
///
/// [`RunCensusError::Threads`] if the thread pool cannot be built, and
/// [`RunCensusError::TooFewCells`] if a site holds too few cells for its records.
///
/// # Panics
///
/// If a site's cells leave the root cube or its frame.
pub fn run_merger_censuses<R: Send>(
    galaxy: &Galaxy,
    class: CarvedClass,
    params: &CensusParams,
    threads: NonZeroUsize,
    read: impl Fn(&MergerMember) -> Option<R> + Sync,
) -> Result<MergerCensuses<R>, RunCensusError> {
    let bands = merger_census_bands(class);
    let plans = census_plans(params);
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = bands
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
        .flat_map(|p| (0..bands.len()).flat_map(move |b| (0..params.parts).map(move |k| (p, b, k))))
        .collect();
    let mut joined: Vec<Vec<MergerBandCensus<R>>> = plans
        .iter()
        .map(|_| {
            bands
                .iter()
                .map(|&band| MergerBandCensus {
                    band,
                    scanned: [0; 7],
                    readings: Vec::new(),
                })
                .collect()
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
                    let (site, whose, records) = plans[p];
                    let wanted = per_part(records, bands[b]);
                    let plan = MergerCensusPlan {
                        bands: &bands[b..=b],
                        site,
                        whose,
                        records: wanted,
                    };
                    let census: MergerCensus<Option<(Population, R)>> = merger_census_in(
                        galaxy,
                        class,
                        plan,
                        CensusPart::new(k, params.part_count),
                        |m| read(&m).map(|r| (m.record().population(), r)),
                    );
                    (p, b, wanted, census)
                })
                .collect::<Vec<_>>()
        },
        |done| {
            for (p, b, wanted, census) in done {
                let found = census.scanned_total();
                if found < wanted && short.is_none() {
                    short = Some(RunCensusError::TooFewCells {
                        census: p,
                        band: bands[b],
                        found,
                        wanted,
                    });
                }
                let slot = &mut joined[p][b];
                for (count, population) in slot.scanned.iter_mut().zip(POPULATIONS) {
                    *count += census.scanned(population);
                }
                slot.readings
                    .extend(census.into_members().into_iter().flatten());
            }
        },
    )?;
    if let Some(error) = short {
        return Err(error);
    }
    let mut plans = joined.into_iter();
    let slab = plans.next().expect("the slab's census is the first");
    let own = params.sites.iter().map(|s| s.0).zip(plans).collect();
    Ok(MergerCensuses { slab, own })
}

/// A class's share row from `censuses` in `galaxy`, its members weighted by `weight` (zero for one
/// the class leaves out), scaled so that the galaxy's mergers in the horizon are `total` (module
/// documentation).
///
/// # Panics
///
/// If no census found a weighted member, or a census ran no record of a population.
#[must_use]
pub fn merger_row<R>(
    galaxy: &Galaxy,
    censuses: &MergerCensuses<R>,
    weight: impl Fn(&R) -> f64,
    total: f64,
) -> ShareRow {
    let shares = BandShares::of(galaxy.mass_function()).as_array();
    let fields = galaxy.fields();
    let born: [f64; 7] = POPULATIONS.map(|population| {
        fields
            .components()
            .iter()
            .filter(|c| c.population() == population)
            .fold(0.0, |sum, c| sum + c.count())
    });
    // The weighted rate a record, per population and band.
    let rate: [[f64; 5]; 7] = POPULATIONS.map(|population| {
        let mut by_band = [0.0; 5];
        for census in censuses.of(population) {
            let records = census.scanned[population.index()];
            assert!(records > 0, "a census ran no record of {population:?}");
            let sum: f64 = census
                .readings
                .iter()
                .filter(|(p, _)| *p == population)
                .map(|(_, r)| weight(r))
                .sum();
            #[expect(clippy::cast_precision_loss, reason = "a record count under 2⁵³")]
            let per_record = sum / records as f64;
            by_band[census.band.index()] = per_record;
        }
        by_band
    });
    let per_system = |p: usize| {
        rate[p]
            .iter()
            .zip(shares)
            .fold(0.0, |sum, (r, s)| sum + r * s)
    };
    let hosts = (0..7).fold(0.0, |sum, p| sum + per_system(p) * born[p]);
    assert!(
        hosts > 0.0,
        "the censuses found weighted members of the class"
    );
    let scale = total / hosts;
    let formed = galaxy.mean_formed_mass().value();
    let rate_per_system: [f64; 7] = std::array::from_fn(per_system);
    let mut layer_shares: [f64; 5] =
        std::array::from_fn(|b| (0..7).fold(0.0, |sum, p| sum + rate[p][b] * shares[b] * born[p]));
    let sum = layer_shares.iter().sum::<f64>();
    for share in &mut layer_shares {
        *share /= sum;
    }
    ShareRow {
        hosts_per_formed_mass: rate_per_system.map(|r| scale * r / formed),
        layer_shares,
        rate_per_system,
    }
}

/// Whether a member of `weight` counts: a positive, finite weight. A merger whose component's age
/// density is zero at its merger age has none, and stands for no merger in the horizon.
#[must_use]
fn weighs(weight: f64) -> bool {
    weight.is_finite() && weight > 0.0
}

/// `points` whose values are finite, the rest dropped.
#[must_use]
fn finite(points: &[MarkPoint]) -> Vec<MarkPoint> {
    points
        .iter()
        .filter(|p| p.value.is_finite() && weighs(p.weight))
        .copied()
        .collect()
}

/// Kish's effective size of `points`' weights.
#[must_use]
fn kish(points: &[MarkPoint]) -> f64 {
    let (sum, squares) = points.iter().fold((0.0, 0.0), |(s, q), p| {
        (s + p.weight, q + p.weight * p.weight)
    });
    if squares > 0.0 {
        sum * sum / squares
    } else {
        0.0
    }
}

/// Whether `points` are enough to fit a law to: a Kish size of at least `min_members`.
#[must_use]
fn enough(points: &[MarkPoint], min_members: usize) -> bool {
    #[expect(clippy::cast_precision_loss, reason = "a member count, far under 2⁵³")]
    let min = min_members as f64;
    !points.is_empty() && kish(points) >= min
}

/// The layer whose law node `node` of 17 holds, for a mark conditional on the layer across the
/// span (0, 4): the nearest, node 4b being layer b's.
#[must_use]
const fn node_layer(node: usize) -> usize {
    let b = (node + 2) / 4;
    if b > 4 { 4 } else { b }
}

/// A mark conditional on the layer of `points`, each point's condition its layer's index, or
/// `None` if they are too few for a law (module documentation): the mark's own law over all of
/// them, and at the node of each layer whose members' Kish size is at least `min_members` that
/// layer's law, about a median of zero on the span (0, 4).
#[must_use]
fn by_layer(points: &[MarkPoint], min_members: usize) -> Option<FittedMark> {
    let points = &finite(points);
    if !enough(points, min_members) {
        return None;
    }
    let own = unconditional(points);
    let laws: [Option<[f64; NODES]>; 5] = std::array::from_fn(|b| {
        #[expect(clippy::cast_precision_loss, reason = "a layer's index, 0–4")]
        let layer = b as f64;
        let mine: Vec<MarkPoint> = points
            .iter()
            .filter(|p| (p.condition - layer).abs() < 0.5)
            .copied()
            .collect();
        enough(&mine, min_members).then(|| unconditional(&mine).residual)
    });
    let rows: [[f64; NODES]; NODES] =
        std::array::from_fn(|node| laws[node_layer(node)].unwrap_or(own.residual));
    let own_nodes = (0..NODES)
        .filter(|&n| laws[node_layer(n)].is_some())
        .count();
    Some(FittedMark {
        residual: own.residual,
        node_residuals: Some(rows),
        span: (0.0, 4.0),
        median: [0.0; NODES],
        limits: own.limits,
        members: points.len(),
        own_nodes,
    })
}

/// An unconditional mark of `points`, or `None` if they are too few for a law.
#[must_use]
fn plain(points: &[MarkPoint], min_members: usize) -> Option<FittedMark> {
    let points = &finite(points);
    enough(points, min_members).then(|| unconditional(points))
}

/// `fitted`, or `scratch`'s law where it is `None`, with where it came from.
#[must_use]
fn or_scratch(
    fitted: Option<&FittedMark>,
    scratch: &hyperion_sim::tables::binary::SamplerMark,
) -> (FittedMark, MarkSource) {
    fitted.map_or_else(
        || (FittedMark::of_scratch(scratch), MarkSource::Scratch),
        |m| (*m, MarkSource::Own),
    )
}

/// A point of `value` at the layer `layer`'s condition, of `weight`.
#[must_use]
fn at_layer(layer: MassBand, value: f64, weight: f64) -> MarkPoint {
    #[expect(clippy::cast_precision_loss, reason = "a layer's index, 0–4")]
    let condition = layer.index() as f64;
    MarkPoint {
        condition,
        value,
        weight,
    }
}

/// A point of `value` with no condition, of `weight`.
#[must_use]
const fn point(value: f64, weight: f64) -> MarkPoint {
    MarkPoint {
        condition: 0.0,
        value,
        weight,
    }
}

/// The stellar mergers' fitted sampler: its marks in [`MergerMark`]'s order and where each came
/// from, its layers' and channels' weights, and what the census's members gave them.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerSampler {
    /// The marks.
    pub marks: [FittedMark; 10],
    /// Where each came from.
    pub sources: [MarkSource; 10],
    /// The layer choice's weights, A–E, at the fixed point.
    pub layers: [f64; 5],
    /// The members' own weighted layer shares among the contact and evolved channels, the fixed
    /// point's target.
    pub target_layers: [f64; 5],
    /// The class's realised layers at the fixed point's weights.
    pub realised_layers: [f64; 5],
    /// The channel choice's weights, in [`MergerChannel::ALL`]'s order.
    pub channels: [f64; 3],
    /// The weight's shares of the members the channels do not cover and of each channel, in that
    /// order, over every member.
    pub covered: [f64; 4],
    /// The members read, every channel's and none's.
    pub members: usize,
    /// Their Kish size.
    pub kish: f64,
}

/// The fixed point of the layer choice (module documentation): weights `w` at which the class's
/// realised layers over `members` (each one's weight, its layers open at its T and its own layer)
/// are the members' own, from the target itself, 200 steps or until no weight moves by a part in
/// 10¹².
#[must_use]
fn layer_fixed_point(members: &[(f64, [bool; 5], usize)]) -> ([f64; 5], [f64; 5], [f64; 5]) {
    let total: f64 = members.iter().map(|m| m.0).sum();
    let mut target = [0.0; 5];
    for &(w, _, layer) in members {
        target[layer] += w / total;
    }
    let realised = |weights: &[f64; 5]| {
        let mut r = [0.0; 5];
        for &(w, open, _) in members {
            let sum: f64 = (0..5).filter(|&b| open[b]).map(|b| weights[b]).sum();
            if sum > 0.0 {
                for b in (0..5).filter(|&b| open[b]) {
                    r[b] += w / total * weights[b] / sum;
                }
            }
        }
        r
    };
    let mut weights = target;
    for _ in 0..200 {
        let r = realised(&weights);
        let next: [f64; 5] = std::array::from_fn(|b| {
            if r[b] > 0.0 {
                weights[b] * target[b] / r[b]
            } else {
                weights[b]
            }
        });
        let sum: f64 = next.iter().sum();
        let next = next.map(|w| w / sum);
        let moved = next
            .iter()
            .zip(&weights)
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        weights = next;
        if moved < 1e-12 {
            break;
        }
    }
    let r = realised(&weights);
    (weights, target, r)
}

/// Fits the stellar mergers' sampler to `readings` (module documentation).
#[must_use]
pub fn merger_sampler(
    galaxy: &Galaxy,
    readings: &[&StellarMergerReading],
    min_members: usize,
) -> MergerSampler {
    let channel_of = |r: &StellarMergerReading| r.channel;
    let of = |c: MergerChannel| readings.iter().filter(move |r| channel_of(r) == Some(c));
    let scratch = |m: MergerMark| MERGER_SAMPLER.mark(m);
    let mut marks: [FittedMark; 10] =
        std::array::from_fn(|k| FittedMark::of_scratch(scratch(MergerMark::ALL[k])));
    let mut sources = [MarkSource::Scratch; 10];
    let mut set = |marks: &mut [FittedMark; 10], mark: MergerMark, fitted: Option<FittedMark>| {
        let (m, s) = or_scratch(fitted.as_ref(), scratch(mark));
        marks[mark.index()] = m;
        sources[mark.index()] = s;
    };
    let covered: Vec<&&StellarMergerReading> =
        readings.iter().filter(|r| r.channel.is_some()).collect();
    set(
        &mut marks,
        MergerMark::AgeRank,
        plain(
            &covered
                .iter()
                .map(|r| point(r.age_rank, r.weight))
                .collect::<Vec<_>>(),
            min_members,
        ),
    );
    // The contact pair's mass ratio first: its largest sets the lightest primary, whose rank the
    // primary's mark reads.
    let contact_ratio: Vec<MarkPoint> = of(MergerChannel::Contact)
        .map(|r| at_layer(r.layer, r.mass_ratio, r.weight))
        .collect();
    set(
        &mut marks,
        MergerMark::ContactMassRatio,
        by_layer(&contact_ratio, min_members),
    );
    let lightest = MIN_INITIAL_MASS.value() / marks[MergerMark::ContactMassRatio.index()].limits.1;
    set(
        &mut marks,
        MergerMark::PrimaryRank,
        by_layer(
            &of(MergerChannel::Contact)
                .map(|r| at_layer(r.layer, r.contact_primary_rank(galaxy, lightest), r.weight))
                .collect::<Vec<_>>(),
            min_members,
        ),
    );
    set(
        &mut marks,
        MergerMark::EvolvedPrimaryRank,
        by_layer(
            &of(MergerChannel::EvolvedDonor)
                .map(|r| at_layer(r.layer, r.evolved_primary_rank(galaxy), r.weight))
                .collect::<Vec<_>>(),
            min_members,
        ),
    );
    for mark in [
        MergerMark::EvolvedMassRatio,
        MergerMark::GapMassRatio,
        MergerMark::HeliumBurningMassRatio,
        MergerMark::AgbMassRatio,
    ] {
        let points: Vec<MarkPoint> = of(MergerChannel::EvolvedDonor)
            .filter(|r| r.ratio_mark() == Some(mark))
            .map(|r| at_layer(r.layer, r.mass_ratio, r.weight))
            .collect();
        set(&mut marks, mark, by_layer(&points, min_members));
    }
    set(
        &mut marks,
        MergerMark::DynamicalPrimaryMass,
        plain(
            &of(MergerChannel::DynamicalMainSequence)
                .map(|r| point(r.primary_mass, r.weight))
                .collect::<Vec<_>>(),
            min_members,
        ),
    );
    set(
        &mut marks,
        MergerMark::DynamicalMassRatio,
        plain(
            &of(MergerChannel::DynamicalMainSequence)
                .map(|r| point(r.mass_ratio, r.weight))
                .collect::<Vec<_>>(),
            min_members,
        ),
    );
    let points: Vec<MarkPoint> = readings.iter().map(|r| point(0.0, r.weight)).collect();
    let picks = merger_picks(readings, lightest);
    MergerSampler {
        marks,
        sources,
        layers: picks.0,
        target_layers: picks.1,
        realised_layers: picks.2,
        channels: picks.3,
        covered: picks.4,
        members: readings.len(),
        kish: kish(&points),
    }
}

/// The stellar mergers' choices from `readings`, the contact pair's lightest primary `lightest`:
/// the layer choice's weights at their fixed point, its target and the class's realised layers
/// there; the channels' weights; and the weight's shares of the members no channel covers and of
/// each channel (module documentation).
#[expect(
    clippy::type_complexity,
    reason = "the five figures `MergerSampler` holds, in its order"
)]
fn merger_picks(
    readings: &[&StellarMergerReading],
    lightest: f64,
) -> ([f64; 5], [f64; 5], [f64; 5], [f64; 3], [f64; 4]) {
    let layered: Vec<(f64, [bool; 5], usize)> = readings
        .iter()
        .filter(|r| {
            matches!(
                r.channel,
                Some(MergerChannel::Contact | MergerChannel::EvolvedDonor)
            )
        })
        .map(|r| (r.weight, r.layers_open(lightest), r.layer.index()))
        .collect();
    let (layers, target, realised) = if layered.is_empty() {
        let w = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
        let w: [f64; 5] = std::array::from_fn(|b| w[b]);
        (w, w, w)
    } else {
        layer_fixed_point(&layered)
    };
    let total: f64 = readings.iter().map(|r| r.weight).sum();
    let mut covered = [0.0; 4];
    for r in readings {
        let k = r.channel.map_or(0, |c| {
            1 + MergerChannel::ALL.iter().position(|&a| a == c).unwrap_or(0)
        });
        covered[k] += r.weight / total;
    }
    let in_channels: f64 = covered[1..].iter().sum();
    let channels: [f64; 3] = std::array::from_fn(|c| covered[1 + c] / in_channels);
    (layers, target, realised, channels, covered)
}

/// The neutron-star mergers' fitted sampler: its marks in [`NsmMark`]'s order and where each came
/// from, its pair kinds' weights, and what the census's members gave them.
#[derive(Debug, Clone, PartialEq)]
pub struct NsmSampler {
    /// The marks.
    pub marks: [FittedMark; 5],
    /// Where each came from.
    pub sources: [MarkSource; 5],
    /// The pair kind choice's weights, in [`NsmPairKind::ALL`]'s order.
    pub pair_kinds: [f64; 2],
    /// The weight's shares of the pairs of two black holes, which the class leaves out, and of the
    /// neutron stars' and the black holes' progenitors outside the sim's ranges, over every member.
    pub left_out: [f64; 3],
    /// The members read.
    pub members: usize,
    /// Their Kish size.
    pub kish: f64,
}

/// Fits the neutron-star mergers' sampler to `readings` (module documentation).
#[must_use]
pub fn nsm_sampler(galaxy: &Galaxy, readings: &[&NsmReading], min_members: usize) -> NsmSampler {
    let scratch = |m: NsmMark| NSM_SAMPLER.mark(m);
    let drawn: Vec<&&NsmReading> = readings.iter().filter(|r| r.kind.is_some()).collect();
    let neutron: Vec<MarkPoint> = drawn
        .iter()
        .flat_map(|r| {
            r.neutron_star_masses
                .into_iter()
                .flatten()
                .map(|m| point(m, r.weight))
        })
        .collect();
    let hole: Vec<MarkPoint> = drawn
        .iter()
        .filter_map(|r| r.black_hole_mass.map(|m| point(math::log10(m), r.weight)))
        .collect();
    let neutron_ranks: Vec<MarkPoint> = drawn
        .iter()
        .flat_map(|r| {
            r.neutron_star_progenitor_ranks(galaxy)
                .into_iter()
                .flatten()
                .map(|q| point(q, r.weight))
        })
        .collect();
    let hole_ranks: Vec<MarkPoint> = drawn
        .iter()
        .filter_map(|r| {
            r.black_hole_progenitor_rank(galaxy)
                .map(|q| point(q, r.weight))
        })
        .collect();
    let ages: Vec<MarkPoint> = drawn.iter().map(|r| point(r.age_rank, r.weight)).collect();
    let fits = [
        (NsmMark::NeutronStarMass, plain(&neutron, min_members)),
        (NsmMark::LogBlackHoleMass, plain(&hole, min_members)),
        (
            NsmMark::NeutronStarProgenitorRank,
            plain(&neutron_ranks, min_members),
        ),
        (
            NsmMark::BlackHoleProgenitorRank,
            plain(&hole_ranks, min_members),
        ),
        (NsmMark::AgeRank, plain(&ages, min_members)),
    ];
    let mut marks = [FittedMark::of_scratch(scratch(NsmMark::AgeRank)); 5];
    let mut sources = [MarkSource::Scratch; 5];
    for (mark, fitted) in fits {
        let (m, s) = or_scratch(fitted.as_ref(), scratch(mark));
        marks[mark.index()] = m;
        sources[mark.index()] = s;
    }
    let total: f64 = readings.iter().map(|r| r.weight).sum();
    let mut kinds = [0.0; 2];
    let mut left_out = [0.0; 3];
    let (ns_range, bh_range) = (
        hyperion_sim::tables::binary::NSM_NEUTRON_STAR_PROGENITORS,
        hyperion_sim::tables::binary::NSM_BLACK_HOLE_PROGENITORS,
    );
    for r in readings {
        match r.kind {
            Some(kind) => {
                let k = NsmPairKind::ALL
                    .iter()
                    .position(|&a| a == kind)
                    .unwrap_or(0);
                kinds[k] += r.weight;
            }
            None => left_out[0] += r.weight / total,
        }
        if r.neutron_star_progenitors
            .into_iter()
            .flatten()
            .any(|m| !(ns_range.0..=ns_range.1).contains(&m))
        {
            left_out[1] += r.weight / total;
        }
        if r.black_hole_progenitor
            .is_some_and(|m| !(bh_range.0..=bh_range.1).contains(&m))
        {
            left_out[2] += r.weight / total;
        }
    }
    let in_kinds: f64 = kinds.iter().sum();
    let pair_kinds = if in_kinds > 0.0 {
        kinds.map(|k| k / in_kinds)
    } else {
        let w = NSM_SAMPLER.pick(NsmPick::PairKind).weights();
        [w[0], w[1]]
    };
    let points: Vec<MarkPoint> = readings.iter().map(|r| point(0.0, r.weight)).collect();
    NsmSampler {
        marks,
        sources,
        pair_kinds,
        left_out,
        members: readings.len(),
        kish: kish(&points),
    }
}

/// The stellar mergers' fit: the row and the sampler, and the records its censuses ran.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerFit {
    /// The parameters.
    pub params: MergerParams,
    /// The class's row.
    pub row: ShareRow,
    /// The sampler.
    pub sampler: MergerSampler,
    /// The records the censuses ran.
    pub records: u64,
}

/// The neutron-star mergers' fit, likewise.
#[derive(Debug, Clone, PartialEq)]
pub struct NsmFit {
    /// The parameters.
    pub params: MergerParams,
    /// The class's row.
    pub row: ShareRow,
    /// The sampler.
    pub sampler: NsmSampler,
    /// The records the censuses ran.
    pub records: u64,
}

/// Runs the stellar mergers' fit on `threads` threads.
///
/// # Errors
///
/// As [`run_merger_censuses`].
///
/// # Panics
///
/// If the Milky Way fixture does not build, or as [`merger_row`].
pub fn fit_merger(
    params: &MergerParams,
    threads: NonZeroUsize,
) -> Result<MergerFit, RunCensusError> {
    let galaxy = params.census.galaxy();
    let censuses = run_merger_censuses(
        &galaxy,
        CarvedClass::StellarMerger,
        &params.census,
        threads,
        |m| StellarMergerReading::of(&galaxy, m).filter(|r| weighs(r.weight)),
    )?;
    let row = merger_row(
        &galaxy,
        &censuses,
        |r| if r.channel.is_some() { r.weight } else { 0.0 },
        params.total_hosts,
    );
    let readings: Vec<&StellarMergerReading> = censuses
        .slab
        .iter()
        .flat_map(|c| &c.readings)
        .map(|(_, r)| r)
        .collect();
    Ok(MergerFit {
        params: params.clone(),
        row,
        sampler: merger_sampler(&galaxy, &readings, params.min_members),
        records: censuses.records(),
    })
}

/// Runs the neutron-star mergers' fit on `threads` threads.
///
/// # Errors
///
/// As [`run_merger_censuses`].
///
/// # Panics
///
/// If the Milky Way fixture does not build, or as [`merger_row`].
pub fn fit_nsm(params: &MergerParams, threads: NonZeroUsize) -> Result<NsmFit, RunCensusError> {
    let galaxy = params.census.galaxy();
    let censuses = run_merger_censuses(
        &galaxy,
        CarvedClass::NeutronStarMerger,
        &params.census,
        threads,
        |m| NsmReading::of(&galaxy, m).filter(|r| weighs(r.weight)),
    )?;
    let row = merger_row(
        &galaxy,
        &censuses,
        |r| if r.kind.is_some() { r.weight } else { 0.0 },
        params.total_hosts,
    );
    let readings: Vec<&NsmReading> = censuses
        .slab
        .iter()
        .flat_map(|c| &c.readings)
        .map(|(_, r)| r)
        .collect();
    Ok(NsmFit {
        params: params.clone(),
        row,
        sampler: nsm_sampler(&galaxy, &readings, params.min_members),
        records: censuses.records(),
    })
}

/// A `pub static` array `name` of 17-value `rows` under `doc`, one row a line.
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

/// A class's row's two items, named with `prefix`, for `what`.
fn row_items(prefix: &str, what: &str, row: &ShareRow) -> [TableItem; 2] {
    [
        TableItem::Array {
            name: format!("{prefix}_HOSTS_PER_FORMED_MASS_31"),
            doc: vec![format!(
                "The {what}' mergers in the source horizon per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order."
            )],
            values: row.hosts_per_formed_mass.to_vec(),
        },
        TableItem::Array {
            name: format!("{prefix}_LAYER_SHARES_31"),
            doc: vec![format!(
                "The share of the {what}' mergers whose primary lies in each layer, A–E."
            )],
            values: row.layer_shares.to_vec(),
        },
    ]
}

/// `binary_merger`'s block's items for `fit`.
#[must_use]
fn merger_items(fit: &MergerFit) -> Vec<TableItem> {
    let s = &fit.sampler;
    let mut items = vec![
        TableItem::Nested {
            name: "MERGER_RESIDUALS_31".to_owned(),
            doc: vec![
                "The stellar mergers' marks' residual quantiles at the ranks k ÷ 16 over all their members, in".to_owned(),
                "[`MergerMark`]'s order, each in its mark's unit.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.residual.to_vec()).collect(),
        },
        static_rows(
            "MERGER_NODE_RESIDUALS_31",
            &[
                "The stellar mergers' marks' laws at their medians' 17 nodes: each mark's 17 rows in".to_owned(),
                "[`MergerMark`]'s order, each its node's quantiles at the ranks k ÷ 16; node 4b is layer b's".to_owned(),
                "for a mark conditional on the layer, and an unconditional mark's are not read.".to_owned(),
            ],
            &s.marks.iter().flat_map(FittedMark::node_rows).collect::<Vec<_>>(),
        ),
        TableItem::Tuples {
            name: "MERGER_MEDIAN_SPANS_31".to_owned(),
            doc: vec![
                "The stellar mergers' marks' medians' spans of their conditions, first and last node, in".to_owned(),
                "[`MergerMark`]'s order: (0, 4) of the layer's index for a mark conditional on the layer.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| vec![m.span.0, m.span.1]).collect(),
        },
        TableItem::Nested {
            name: "MERGER_MEDIANS_31".to_owned(),
            doc: vec!["The stellar mergers' marks' medians at their 17 nodes, in [`MergerMark`]'s order: zero.".to_owned()],
            rows: s.marks.iter().map(|m| m.median.to_vec()).collect(),
        },
        TableItem::Tuples {
            name: "MERGER_LIMITS_31".to_owned(),
            doc: vec!["The stellar mergers' marks' least and greatest values, in [`MergerMark`]'s order.".to_owned()],
            rows: s.marks.iter().map(|m| vec![m.limits.0, m.limits.1]).collect(),
        },
        TableItem::Array {
            name: "MERGER_LAYERS_31".to_owned(),
            doc: vec!["The stellar mergers' layer choice's weights, A–E, at its fixed point.".to_owned()],
            values: s.layers.to_vec(),
        },
        TableItem::Array {
            name: "MERGER_CHANNELS_31".to_owned(),
            doc: vec!["The stellar mergers' channel choice's weights, in [`MergerChannel::ALL`]'s order.".to_owned()],
            values: s.channels.to_vec(),
        },
    ];
    items.extend(row_items("MERGER", "stellar mergers", &fit.row));
    items
}

/// `binary_nsm`'s block's items for `fit`.
#[must_use]
fn nsm_items(fit: &NsmFit) -> Vec<TableItem> {
    let s = &fit.sampler;
    let mut items = vec![
        TableItem::Nested {
            name: "NSM_RESIDUALS_31".to_owned(),
            doc: vec![
                "The neutron-star mergers' marks' quantiles at the ranks k ÷ 16 over all their members, in".to_owned(),
                "[`NsmMark`]'s order, each in its mark's unit.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.residual.to_vec()).collect(),
        },
        TableItem::Tuples {
            name: "NSM_LIMITS_31".to_owned(),
            doc: vec!["The neutron-star mergers' marks' least and greatest values, in [`NsmMark`]'s order.".to_owned()],
            rows: s.marks.iter().map(|m| vec![m.limits.0, m.limits.1]).collect(),
        },
        TableItem::Array {
            name: "NSM_PAIR_KINDS_31".to_owned(),
            doc: vec!["The neutron-star mergers' pair kind choice's weights, in [`NsmPairKind::ALL`]'s order.".to_owned()],
            values: s.pair_kinds.to_vec(),
        },
    ];
    items.extend(row_items("NSM", "neutron-star mergers", &fit.row));
    items
}

/// The names of `sources`' marks kept from the scratch table.
fn scratch_marks<M: std::fmt::Debug + Copy>(all: &[M], sources: &[MarkSource]) -> String {
    let names: Vec<String> = all
        .iter()
        .zip(sources)
        .filter(|(_, s)| **s == MarkSource::Scratch)
        .map(|(m, _)| format!("{m:?}"))
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// A row's figures for an acceptance line.
fn row_figures(row: &ShareRow) -> String {
    format!(
        "layers {} rates a system {}",
        row.layer_shares.map(|v| format!("{v:.3}")).join("/"),
        row.rate_per_system.map(|v| format!("{v:.2e}")).join("/")
    )
}

/// `binary_merger`'s acceptance line.
fn merger_acceptance(fit: &MergerFit) -> String {
    let s = &fit.sampler;
    format!(
        "{} members, Kish {:.0}; of the weight, uncovered {:.3}, contact {:.3}, evolved donor {:.3}, dynamical {:.3}; channels {}; layers {} (target {}, realised {}); {}; scratch marks: {}; the K–S tests against the other half of the census are the sim's slow `merger_tables::the_stellar_merger_sampler_against_the_engine`",
        s.members,
        s.kish,
        s.covered[0],
        s.covered[1],
        s.covered[2],
        s.covered[3],
        s.channels.map(|v| format!("{v:.3}")).join("/"),
        s.layers.map(|v| format!("{v:.3}")).join("/"),
        s.target_layers.map(|v| format!("{v:.3}")).join("/"),
        s.realised_layers.map(|v| format!("{v:.3}")).join("/"),
        row_figures(&fit.row),
        scratch_marks(&MergerMark::ALL, &s.sources),
    )
}

/// `binary_nsm`'s acceptance line.
fn nsm_acceptance(fit: &NsmFit) -> String {
    let s = &fit.sampler;
    format!(
        "{} members, Kish {:.0}; pair kinds {}; of the weight, two black holes {:.3}, a neutron star's progenitor outside 8–20 M☉ {:.3}, a black hole's outside 25–60 M☉ {:.3}; {}; scratch marks: {}; the K–S tests against the other half of the census are the sim's slow `merger_tables::the_neutron_star_merger_sampler_against_the_engine`",
        s.members,
        s.kish,
        s.pair_kinds.map(|v| format!("{v:.3}")).join("/"),
        s.left_out[0],
        s.left_out[1],
        s.left_out[2],
        row_figures(&fit.row),
        scratch_marks(&NsmMark::ALL, &s.sources),
    )
}

/// `text` wrapped at word breaks into lines of at most 97 characters, the emitter's `// ` after.
fn wrapped(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > 97 {
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

/// The block's notes after its header: the `class`' block of `params`, read by `read_by`, whose
/// marks `marks` describes, from censuses that ran `records` records.
fn notes(
    params: &MergerParams,
    records: u64,
    (class, read_by, marks): (&str, &str, &str),
) -> Vec<String> {
    wrapped(&format!(
        "Read by {read_by}, after the block. The {class}' marks are fitted to the engine's own \
         mergers at generator version {version}'s rules through the sim's merger census of the \
         pairs the generator draws at each record's first attempt (hyperion-fit's \
         `tasks::binary_classes::mergers`), each member weighted by its age hazard at its merger \
         age: {marks} A mark with a Kish size under {min} keeps the scratch sampler's law. The row \
         is the members' weighted rate a record by population and band, scaled to {total} mergers \
         in the source horizon at Milky Way parameters (plan 11's design note 11). The censuses \
         ran {records} records.",
        version = params.census.version.get(),
        min = params.min_members,
        total = params.total_hosts,
    ))
}

/// `binary_merger`'s block's contents for `fit`.
#[must_use]
pub fn render_merger(fit: &MergerFit) -> RustTable {
    RustTable {
        summary: vec![
            "The stellar mergers' sampler and share row fitted against plan 11's engine at generator"
                .to_owned(),
            "version 31 on the generator's own pairs (plan 15, P15.T10.b), held until 31.".to_owned(),
        ],
        notes: notes(
            &fit.params,
            fit.records,
            (
                "stellar mergers",
                "`MERGER_SAMPLER_31` and the class shares' rows from 31",
                &format!(
                    "a mark's weighted quantiles at the ranks k ÷ 16, a mark the class draws after the \
                     layer conditional on it, each layer's own law where its members' Kish size is {} or \
                     more, and the layer choice at its fixed point over the members' merger ages.",
                    fit.params.min_members
                ),
            ),
        ),
        items: merger_items(fit),
    }
}

/// `binary_nsm`'s block's contents for `fit`.
#[must_use]
pub fn render_nsm(fit: &NsmFit) -> RustTable {
    RustTable {
        summary: vec![
            "The neutron-star mergers' sampler and share row fitted against plan 11's engine at"
                .to_owned(),
            "generator version 31 on the generator's own pairs (plan 15, P15.T10.b), held until 31."
                .to_owned(),
        ],
        notes: notes(
            &fit.params,
            fit.records,
            (
                "neutron-star mergers",
                "`NSM_SAMPLER_31` and the class shares' rows from 31",
                "each mark's weighted quantiles at the ranks k ÷ 16, unconditional, the progenitors' \
                 ranks over the sim's fixed ranges held at their ends.",
            ),
        ),
        items: nsm_items(fit),
    }
}

/// The fingerprint of a merger task of `class`: the galaxy's band shares and born systems, and the
/// merger census at 31 of the cells that hold `pinned`'s record, every member's weight and merger
/// age.
fn fingerprint_of(class: CarvedClass, pinned: u64) -> SimFingerprint {
    let galaxy = super::fingerprint_galaxy();
    let shares = BandShares::of(galaxy.mass_function());
    let mut probes: Vec<(String, f64)> = merger_census_bands(class)
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
    let key = CellKey::of(SystemId::from_raw(pinned).expect("a grid ID")).expect("a grid cell");
    let size = i32::try_from(key.size_ly()).expect("a cell's size");
    let origin = key.origin_ly();
    let centre = [0, 1, 2].map(|k| origin[k] + size / 2);
    let plan = MergerCensusPlan {
        bands: merger_census_bands(class),
        site: CensusSite::new(centre, [0, 0, 0]),
        whose: CensusRecords::All,
        records: 1_000,
    };
    let census = merger_census_in(&galaxy, class, plan, CensusPart::new(0, 1), |m| {
        (m.weight(), m.merger_age().value())
    });
    #[expect(clippy::cast_precision_loss, reason = "a few cells' records")]
    probes.push(("cells' records".to_owned(), census.scanned_total() as f64));
    for (i, (w, t)) in census.members().iter().enumerate() {
        probes.push((format!("member {i} weight"), *w));
        probes.push((format!("member {i} merger age"), *t));
    }
    SimFingerprint::new(probes)
}

/// The stellar mergers' sampler and row (`binary_merger`).
#[derive(Debug, Clone, Copy)]
pub struct BinaryMergerTask;

impl FitTask for BinaryMergerTask {
    fn name(&self) -> &'static str {
        "binary_merger"
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
        MERGER_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &MERGER_ITEMS
    }

    /// The galaxy's band shares and born systems, and the merger census at 31 of the cells of one
    /// of the census's stellar mergers.
    fn fingerprint(&self) -> SimFingerprint {
        fingerprint_of(CarvedClass::StellarMerger, FINGERPRINT_MERGER)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = MergerParams::of(manifest)?;
        let fit = fit_merger(&params, threads).map_err(|e| census_error("binary_merger", e))?;
        Ok(TaskOutput {
            table: render_merger(&fit),
            source:
                "plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 \
                     builds it) and classes (P11.T5), over plan 06's tracks, at the manifest's \
                     generator version's rules; the total against the brainstorm's 9 × 10⁴ mergers \
                     in the source horizon (0.34 a year, Kochanek et al. 2014, MNRAS 443, 1319)"
                    .to_owned(),
            acceptance: merger_acceptance(&fit),
            provisional: None,
        })
    }
}

/// The neutron-star mergers' sampler and row (`binary_nsm`).
#[derive(Debug, Clone, Copy)]
pub struct BinaryNsmTask;

impl FitTask for BinaryNsmTask {
    fn name(&self) -> &'static str {
        "binary_nsm"
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
        NSM_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &NSM_ITEMS
    }

    /// The galaxy's band shares and born systems, and the merger census at 31 of the cells of one
    /// of the census's neutron-star mergers.
    fn fingerprint(&self) -> SimFingerprint {
        fingerprint_of(CarvedClass::NeutronStarMerger, FINGERPRINT_NSM)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = MergerParams::of(manifest)?;
        let fit = fit_nsm(&params, threads).map_err(|e| census_error("binary_nsm", e))?;
        Ok(TaskOutput {
            table: render_nsm(&fit),
            source:
                "plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 \
                     builds it) and classes (P11.T5), over plan 06's tracks, at the manifest's \
                     generator version's rules; the total against the brainstorm's ten \
                     neutron-star mergers in the source horizon"
                    .to_owned(),
            acceptance: nsm_acceptance(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node's layer is the nearest, node 4b layer b's.
    #[test]
    fn a_node_holds_its_nearest_layer() {
        let layers: Vec<usize> = (0..NODES).map(node_layer).collect();
        assert_eq!(layers, [0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4]);
        for b in 0..5 {
            assert_eq!(node_layer(4 * b), b);
        }
    }

    /// The layer choice's fixed point gives the class's realised layers the members' own: here two
    /// members open to every layer and one open to D alone.
    #[test]
    fn the_layer_fixed_point_realises_the_members_layers() {
        let members = [
            (1.0, [false, false, true, true, false], 2),
            (1.0, [false, false, true, true, false], 2),
            (1.0, [false, false, false, true, false], 3),
            (1.0, [false, false, true, true, false], 3),
        ];
        let (weights, target, realised) = layer_fixed_point(&members);
        assert!((target[2] - 0.5).abs() < 1e-12 && (target[3] - 0.5).abs() < 1e-12);
        for b in 0..5 {
            assert!((realised[b] - target[b]).abs() < 1e-9, "{realised:?}");
        }
        assert!(weights[2] > weights[3], "{weights:?}");
    }

    /// A mark conditional on the layer holds each well-supported layer's law at its node and the
    /// mark's own law at the others.
    #[test]
    fn a_layer_conditional_mark_holds_each_layer_s_law_at_its_node() {
        let points: Vec<MarkPoint> = (0..400)
            .map(|i| {
                let layer = if i < 300 { MassBand::C } else { MassBand::D };
                at_layer(
                    layer,
                    f64::from(i % 100) / 100.0 + f64::from(u8::from(i >= 300)),
                    1.0,
                )
            })
            .collect();
        let mark = by_layer(&points, 50).expect("enough members");
        let rows = mark.node_residuals.expect("node laws");
        assert!(
            rows[8][16] < 1.0 && rows[12][0] >= 1.0,
            "{:?} {:?}",
            rows[8],
            rows[12]
        );
        assert_eq!(rows[0], mark.residual);
        assert_eq!(mark.span, (0.0, 4.0));
        assert_eq!(mark.own_nodes, 8);
        assert!(by_layer(&points[..10], 50).is_none());
    }
}
