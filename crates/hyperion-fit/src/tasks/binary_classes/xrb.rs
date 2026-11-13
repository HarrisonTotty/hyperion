//! The X-ray binaries' sampler and share row (plan 15, P15.T10.b), task `binary_xrb`, fitted
//! against plan 11's engine as the accreting white dwarfs' are (the parent module): the same
//! censuses, the young thin disc's larger, since its high-mass hosts are a few in a hundred of its
//! layer-E records.
//!
//! From the refit at 27 the class's kinds ([`XrbHostKind`]) are all nine the carving counts, the
//! engine's members' kinds by `XrayMarks::member_kind`: a neutron star or a black hole fed by a
//! hydrogen-rich low-mass donor, persistent or transient; an ultracompact or a symbiotic X-ray
//! binary; a neutron star beside a Be star or a supergiant, and a black hole beside a supergiant.
//! The kinds' weights are the galaxy's shares of its hosts of each kind, every population's rate
//! of the kind times its systems, but for the black-hole transients', pinned at the observed
//! [`BLACK_HOLE_TRANSIENT_SHARE`] (plan 11's design note 11, research's ruling of 2026-11-02),
//! the other kinds keeping their ratios in the rest. A mark's members are weighted so that their
//! kinds are in those weights' shares (`weighted_to_shares`), so that the marks the kinds share
//! are fitted to them as the class draws them. Each mark is fitted to the members of the kinds the class draws it for, as the class
//! draws it: the low-mass and history marks to the slab's members and the high-mass marks to the
//! young thin disc's own census. A mark with fewer than the manifest's `min_members` keeps
//! `XRB_SAMPLER`'s law, the outburst recurrences among them, which the engine does not draw, and
//! the symbiotic giant's filling of its lobe, which the members' marks do not carry. The low-mass
//! donor's mass follows the period, with a residual law at each node; every other mark is
//! unconditional.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::imf::{BandShares, MassBand};
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::math;
use hyperion_sim::orbit::Eccentricity;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::light_curves::XrayAccretor;
use hyperion_sim::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, XrayMarks, class_census_in,
};
use hyperion_sim::tables::binary::{BLACK_HOLE_TRANSIENT_SHARE, XRB_SAMPLER, XrbHostKind, XrbMark};
use hyperion_sim::units::consts::SECONDS_PER_DAY;

use super::{
    CensusParams, Censuses, FittedMark, MarkPoint, MarkSource, RunCensusError, ShareRow,
    census_error, conditional, fingerprint_galaxy, run_censuses, share_rows, unconditional,
    weighted_to_shares,
};
use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The X-ray binaries' task's revision.
pub const XRB_VERSION: u32 = 0;

/// The X-ray binaries' block's items, in its order: the sampler, then the share row.
pub const XRB_ITEMS: [&str; 9] = [
    "XRB_RESIDUALS_29",
    "XRB_DONOR_MASS_NODE_RESIDUALS_29",
    "XRB_MEDIAN_SPANS_29",
    "XRB_MEDIANS_29",
    "XRB_LIMITS_29",
    "XRB_LOW_MASS_KINDS_29",
    "XRB_HIGH_MASS_KINDS_29",
    "XRB_HOSTS_PER_FORMED_MASS_29",
    "XRB_LAYER_SHARES_29",
];

/// The marks of the high-mass kinds, fitted to the young thin disc's own census.
const HIGH_MASS_MARKS: [XrbMark; 14] = [
    XrbMark::BexPeriod,
    XrbMark::BexEccentricity,
    XrbMark::BexDonorMass,
    XrbMark::SupergiantPeriod,
    XrbMark::SupergiantDonorMass,
    XrbMark::SupergiantRate,
    XrbMark::BexRecoil,
    XrbMark::SupergiantRecoil,
    XrbMark::BexSinceSupernova,
    XrbMark::SupergiantSinceSupernova,
    XrbMark::BlackHoleHighMassAccretorMass,
    XrbMark::BlackHoleHighMassPeriod,
    XrbMark::BlackHoleHighMassDonorMass,
    XrbMark::BlackHoleHighMassRate,
];

/// The fingerprint's census: one layer-E cell of the solar circle's slab, of 30 records three of
/// whose pairs the engine makes X-ray binaries at 27, so that a change to the engine or to the
/// classes moves the probes.
const FINGERPRINT_CELL: CensusSite = CensusSite::new([5_376, 26_000, 0], [0, 0, 0]);

/// The X-ray binaries' fit parameters, from the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbParams {
    /// The censuses'.
    pub census: CensusParams,
    /// The galaxy's X-ray binaries at Milky Way parameters: the observed count the row is scaled
    /// to.
    pub total_hosts: f64,
    /// The least members a mark, or a node's law, is fitted to.
    pub min_members: usize,
}

impl XrbParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] as [`CensusParams::of`], or if the total is not positive or the
    /// least members under 20.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let params = Self {
            census: CensusParams::of(manifest)?,
            total_hosts: manifest.f64("total_hosts")?,
            min_members: usize::try_from(manifest.u64("min_members")?)
                .map_err(|_| ManifestParamError::new("min_members", "a count"))?,
        };
        if !(params.total_hosts.is_finite() && params.total_hosts > 0.0) {
            return Err(ManifestParamError::new("total_hosts", "positive"));
        }
        if params.min_members < 20 {
            return Err(ManifestParamError::new("min_members", "at least 20"));
        }
        Ok(params)
    }
}

/// What one X-ray binary member gives `mark`, as the class draws the mark for the member's kind
/// (`XrayMarks::member_kind`): its condition (the period's log₁₀, hours, for the low-mass donor's
/// mass, zero for every other mark) and value, in the mark's unit. `None` where the class draws no
/// such mark for the kind; where the mark is the compact star's and the member's compact star is
/// not its kind's (the few black holes the census finds beside ultracompact donors, whose kind's
/// compact star is a neutron star); where the engine draws no such mark (the outburst
/// recurrences, and the symbiotic giant's filling, which the marks do not carry); or where a
/// value is not finite.
#[must_use]
pub fn xrb_pair(x: &XrayMarks, mark: XrbMark) -> Option<(f64, f64)> {
    use XrbHostKind::{
        BeX, BlackHoleHighMass, BlackHolePersistent, BlackHoleTransient, NeutronStarPersistent,
        NeutronStarTransient, Supergiant, Symbiotic, Ultracompact,
    };
    let kind = x.member_kind()?;
    let log = |v: f64| (v > 0.0).then(|| math::log10(v));
    let own_compact = match kind.accretor() {
        XrayAccretor::NeutronStar => x.accretor_phase == Phase::NeutronStar,
        XrayAccretor::BlackHole => x.accretor_phase == Phase::BlackHole,
    };
    let hydrogen_low = matches!(
        kind,
        NeutronStarPersistent | NeutronStarTransient | BlackHolePersistent | BlackHoleTransient
    );
    let log_minutes = x.period.and_then(|p| log(p.value() / 60.0));
    let log_hours = x.period.and_then(|p| log(p.value() / 3_600.0));
    let log_days = x.period.and_then(|p| log(p.value() / SECONDS_PER_DAY));
    let log_rate = x.transfer_rate.and_then(|r| log(r.value()));
    let recoil = x.recoil_speed.map(|v| v.value() / 1.0e3);
    let log_since = x.since_supernova.and_then(|y| log(y.value()));
    let only = |holds: bool, value: Option<f64>| if holds { Some((0.0, value?)) } else { None };
    let accretor = Some(x.accretor_mass.value());
    let pair = match mark {
        XrbMark::NeutronStarMass => only(
            own_compact
                && matches!(
                    kind,
                    NeutronStarPersistent
                        | NeutronStarTransient
                        | BeX
                        | Supergiant
                        | Ultracompact
                        | Symbiotic
                ),
            accretor,
        ),
        XrbMark::BlackHoleMass => only(
            own_compact && matches!(kind, BlackHolePersistent | BlackHoleTransient),
            accretor,
        ),
        XrbMark::BlackHoleHighMassAccretorMass => {
            only(own_compact && kind == BlackHoleHighMass, accretor)
        }
        XrbMark::LowMassPeriod => only(hydrogen_low, log_hours),
        XrbMark::LowMassDonorMass => {
            if hydrogen_low {
                Some((log_hours?, log(x.donor_mass.value())?))
            } else {
                None
            }
        }
        XrbMark::LowMassRateExcess => only(hydrogen_low, x.rate_excess_dex),
        // The engine draws no recurrence, and the marks carry no giant's radius.
        XrbMark::NeutronStarRecurrence
        | XrbMark::BlackHoleRecurrence
        | XrbMark::UltracompactRecurrence
        | XrbMark::SymbioticFilling => None,
        XrbMark::BexPeriod => only(kind == BeX, log_days),
        XrbMark::BexEccentricity => only(kind == BeX, x.eccentricity.map(Eccentricity::value)),
        XrbMark::BexDonorMass => only(kind == BeX, Some(x.donor_mass.value())),
        XrbMark::BexRecoil => only(kind == BeX, recoil),
        XrbMark::BexSinceSupernova => only(kind == BeX, log_since),
        XrbMark::SupergiantPeriod => only(kind == Supergiant, log_days),
        XrbMark::SupergiantDonorMass => only(kind == Supergiant, Some(x.donor_mass.value())),
        XrbMark::SupergiantRate => only(kind == Supergiant, log_rate),
        // The class draws a black hole's supergiant's recoil by the neutron stars' marks.
        XrbMark::SupergiantRecoil => only(matches!(kind, Supergiant | BlackHoleHighMass), recoil),
        XrbMark::SupergiantSinceSupernova => {
            only(matches!(kind, Supergiant | BlackHoleHighMass), log_since)
        }
        XrbMark::BlackHoleHighMassPeriod => only(kind == BlackHoleHighMass, log_days),
        XrbMark::BlackHoleHighMassDonorMass => {
            only(kind == BlackHoleHighMass, Some(x.donor_mass.value()))
        }
        XrbMark::BlackHoleHighMassRate => only(kind == BlackHoleHighMass, log_rate),
        XrbMark::UltracompactPeriod => only(kind == Ultracompact, log_minutes),
        XrbMark::UltracompactDonorInitialMass => {
            only(kind == Ultracompact, log(x.donor_initial_mass.value()))
        }
        XrbMark::SymbioticPeriod => only(kind == Symbiotic, log_days),
        XrbMark::SymbioticDonorMass => only(kind == Symbiotic, log(x.donor_mass.value())),
        XrbMark::NeutronStarProgenitor => only(
            own_compact && kind.accretor() == XrayAccretor::NeutronStar,
            log(x.accretor_initial_mass.value()),
        ),
        XrbMark::BlackHoleProgenitor => only(
            own_compact && kind.accretor() == XrayAccretor::BlackHole,
            log(x.accretor_initial_mass.value()),
        ),
        XrbMark::DonorInitialExcess => only(
            hydrogen_low || kind == Symbiotic,
            log(x.donor_initial_mass.value() / x.donor_mass.value()),
        ),
    };
    pair.filter(|(c, v)| c.is_finite() && v.is_finite())
}

/// The X-ray marks of the census's members of the class.
#[must_use]
fn xray_members<'a>(members: impl IntoIterator<Item = &'a ClassMember>) -> Vec<XrayMarks> {
    members
        .into_iter()
        .filter(|m| m.class() == CarvedClass::XrayBinary)
        .filter_map(|m| m.xray().copied())
        .collect()
}

/// `kind`'s place in [`XrbHostKind::ALL`].
#[must_use]
fn kind_index(kind: XrbHostKind) -> usize {
    XrbHostKind::ALL
        .iter()
        .position(|&each| each == kind)
        .expect("every kind is in `XrbHostKind::ALL`")
}

/// The galaxy's shares of its X-ray binaries of each kind, in [`XrbHostKind::ALL`]'s order
/// (`XrayMarks::member_kind`): every population's rate of the kind a system, its bands weighted by
/// their shares of the systems, times its systems, over the same sum for every kind.
///
/// # Panics
///
/// If no census found a member of the class, or one ran no record of a population.
#[must_use]
pub fn census_kind_shares(galaxy: &Galaxy, censuses: &Censuses) -> [f64; 9] {
    let shares = BandShares::of(galaxy.mass_function()).as_array();
    let fields = galaxy.fields();
    let mut totals = [0.0; 9];
    for population in POPULATIONS {
        let born = fields
            .components()
            .iter()
            .filter(|c| c.population() == population)
            .fold(0.0, |sum, c| sum + c.count());
        for census in censuses.of(population) {
            let records = census.scanned[population.index()];
            assert!(records > 0, "a census ran no record of {population:?}");
            let mut counts = [0_u32; 9];
            for m in census
                .members
                .iter()
                .filter(|m| m.record().population() == population)
            {
                if let Some(kind) = m.xray().and_then(XrayMarks::member_kind) {
                    counts[kind_index(kind)] += 1;
                }
            }
            for (total, count) in totals.iter_mut().zip(counts) {
                #[expect(clippy::cast_precision_loss, reason = "record counts under 2⁵³")]
                let rate = f64::from(count) / records as f64;
                *total += rate * shares[census.band.index()] * born;
            }
        }
    }
    let sum: f64 = totals.iter().sum();
    assert!(sum > 0.0, "the censuses found members of the class");
    totals.map(|t| t / sum)
}

/// The kinds' weights the class draws, in [`XrbHostKind::ALL`]'s order: the black-hole transients'
/// [`BLACK_HOLE_TRANSIENT_SHARE`], the others the `census` shares scaled together to the rest.
///
/// # Panics
///
/// If the census holds black-hole transients alone.
#[must_use]
pub fn kind_weights(census: &[f64; 9]) -> [f64; 9] {
    let pinned = kind_index(XrbHostKind::BlackHoleTransient);
    let rest = 1.0 - census[pinned];
    assert!(rest > 0.0, "the census holds kinds besides the pinned one");
    std::array::from_fn(|k| {
        if k == pinned {
            BLACK_HOLE_TRANSIENT_SHARE
        } else {
            census[k] * (1.0 - BLACK_HOLE_TRANSIENT_SHARE) / rest
        }
    })
}

/// The X-ray binaries' fitted sampler: its marks in [`XrbMark`]'s order, where each came from,
/// and the two kind choices' weights.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbSampler {
    /// The marks.
    pub marks: [FittedMark; 30],
    /// Where each mark's members came from: its own class's, or none.
    pub sources: [MarkSource; 30],
    /// The galaxy's census shares of the kinds, in [`XrbHostKind::ALL`]'s order.
    pub census_kinds: [f64; 9],
    /// The low-mass kinds' weights, in [`XrbHostKind::LOW_MASS`]' order.
    pub low_mass_kinds: [f64; 6],
    /// The high-mass kinds' weights, in [`XrbHostKind::HIGH_MASS`]' order.
    pub high_mass_kinds: [f64; 3],
    /// The slab's members of each kind, in [`XrbHostKind::ALL`]'s order.
    pub slab_members: [usize; 9],
    /// The young thin disc's own census's members of each kind.
    pub young_members: [usize; 9],
}

/// Each kind's members among `members`, in [`XrbHostKind::ALL`]'s order.
#[must_use]
fn by_kind(members: &[XrayMarks]) -> [usize; 9] {
    let mut counts = [0; 9];
    for kind in members.iter().filter_map(XrayMarks::member_kind) {
        counts[kind_index(kind)] += 1;
    }
    counts
}

/// Fits the sampler from `censuses` in `galaxy` (module documentation).
///
/// # Panics
///
/// As [`census_kind_shares`] and [`kind_weights`].
#[must_use]
pub fn xrb_sampler(galaxy: &Galaxy, censuses: &Censuses, params: &XrbParams) -> XrbSampler {
    let slab = xray_members(censuses.slab.iter().flat_map(|c| &c.members));
    let young = xray_members(
        censuses
            .of(Population::YoungThinDisc)
            .iter()
            .flat_map(|c| &c.members),
    );
    let census = census_kind_shares(galaxy, censuses);
    let weights = kind_weights(&census);
    let mut sources = [MarkSource::Scratch; 30];
    let marks = std::array::from_fn(|k| {
        let mark = XrbMark::ALL[k];
        let pool = if HIGH_MASS_MARKS.contains(&mark) {
            &young
        } else {
            &slab
        };
        let raw: Vec<(MarkPoint, usize)> = pool
            .iter()
            .filter_map(|x| {
                let (condition, value) = xrb_pair(x, mark)?;
                let point = MarkPoint {
                    condition,
                    value,
                    weight: 1.0,
                };
                Some((point, kind_index(x.member_kind()?)))
            })
            .collect();
        let points = weighted_to_shares(&raw, &weights);
        if points.len() < params.min_members {
            return FittedMark::of_scratch(XRB_SAMPLER.mark(mark));
        }
        sources[k] = MarkSource::Own;
        if mark == XrbMark::LowMassDonorMass {
            conditional(&points, params.min_members)
        } else {
            unconditional(&points)
        }
    });
    XrbSampler {
        marks,
        sources,
        census_kinds: census,
        low_mass_kinds: XrbHostKind::LOW_MASS.map(|kind| weights[kind_index(kind)]),
        high_mass_kinds: XrbHostKind::HIGH_MASS.map(|kind| weights[kind_index(kind)]),
        slab_members: by_kind(&slab),
        young_members: by_kind(&young),
    }
}

/// The X-ray binaries' fit: its row and sampler.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbFit {
    /// The parameters.
    pub params: XrbParams,
    /// The class's row.
    pub row: ShareRow,
    /// The class's sampler.
    pub sampler: XrbSampler,
}

/// Runs the X-ray binaries' fit on `threads` threads.
///
/// # Errors
///
/// As [`run_censuses`].
///
/// # Panics
///
/// If the Milky Way fixture does not build, or as [`share_rows`] and [`census_kind_shares`]: the
/// censuses find no member of the class, or run no record of a population.
pub fn fit_xrb(params: &XrbParams, threads: NonZeroUsize) -> Result<XrbFit, RunCensusError> {
    let galaxy = params.census.galaxy();
    let censuses = run_censuses(&galaxy, &params.census, threads)?;
    let row = share_rows(
        &galaxy,
        &censuses,
        &[CarvedClass::XrayBinary],
        params.total_hosts,
    )[0];
    Ok(XrbFit {
        params: params.clone(),
        row,
        sampler: xrb_sampler(&galaxy, &censuses, params),
    })
}

/// The limits' item `name`, as [`TableItem::Tuples`] writes it, under its one-line `doc`, with
/// Clippy's `approx_constant` allowed on it: a limit may land near a constant, and is data (the
/// black-hole high-mass period's, kept from `XRB_SAMPLER`, is log₁₀ 2 days).
///
/// # Panics
///
/// If a limit is not finite, which a fitted mark's never is.
#[must_use]
fn limits_item(name: &str, doc: &str, limits: &[(f64, f64)]) -> TableItem {
    assert!(
        limits
            .iter()
            .all(|(lo, hi)| lo.is_finite() && hi.is_finite()),
        "item `{name}` holds a value that is not finite"
    );
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(out, "/// {doc}");
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(
        out,
        "#[allow(clippy::approx_constant, reason = \"fitted values that may land near a constant are data\")]"
    );
    let _ = writeln!(out, "pub const {name}: [(f64, f64); {}] = [", limits.len());
    for &(lo, hi) in limits {
        let _ = writeln!(out, "    ({}, {}),", literal(lo), literal(hi));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The block's contents for `fit`.
#[must_use]
pub fn render_xrb(fit: &XrbFit) -> RustTable {
    let s = &fit.sampler;
    let items = vec![
        TableItem::Nested {
            name: XRB_ITEMS[0].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' residual quantiles at the ranks k ÷ 16 over all their members, in".to_owned(),
                "[`XrbMark`]'s order, each in its mark's unit.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.residual.to_vec()).collect(),
        },
        TableItem::Nested {
            name: XRB_ITEMS[1].to_owned(),
            doc: vec![
                "The low-mass donor's mass's residual laws at its median's 17 nodes, each row its node's".to_owned(),
                "quantiles at the ranks k ÷ 16, log₁₀ M☉.".to_owned(),
            ],
            rows: s.marks[XrbMark::LowMassDonorMass.index()]
                .node_rows()
                .iter()
                .map(|row| row.to_vec())
                .collect(),
        },
        TableItem::Tuples {
            name: XRB_ITEMS[2].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' medians' spans of their conditions, first and last node, in".to_owned(),
                "[`XrbMark`]'s order; only the low-mass donor's mass's is read.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| vec![m.span.0, m.span.1]).collect(),
        },
        TableItem::Nested {
            name: XRB_ITEMS[3].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' medians at their 17 nodes, in [`XrbMark`]'s order; only the low-mass".to_owned(),
                "donor's mass's is read.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.median.to_vec()).collect(),
        },
        limits_item(
            XRB_ITEMS[4],
            "The X-ray binaries' marks' least and greatest values, in [`XrbMark`]'s order.",
            &s.marks.iter().map(|m| m.limits).collect::<Vec<_>>(),
        ),
        TableItem::Array {
            name: XRB_ITEMS[5].to_owned(),
            doc: vec![
                "The low-mass kinds' weights, as shares of all the class's hosts, in".to_owned(),
                "[`XrbHostKind::LOW_MASS`]' order: the black-hole transients' pinned at".to_owned(),
                "[`BLACK_HOLE_TRANSIENT_SHARE`].".to_owned(),
            ],
            values: s.low_mass_kinds.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[6].to_owned(),
            doc: vec![
                "The high-mass kinds' weights, as shares of all the class's hosts, in".to_owned(),
                "[`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.".to_owned(),
            ],
            values: s.high_mass_kinds.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[7].to_owned(),
            doc: vec![
                "The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order."
                    .to_owned(),
            ],
            values: fit.row.hosts_per_formed_mass.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[8].to_owned(),
            doc: vec![
                "The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.".to_owned(),
            ],
            values: fit.row.layer_shares.to_vec(),
        },
    ];
    RustTable {
        summary: vec![
            "The X-ray binaries' sampler and share row refitted against plan 11's engine at generator"
                .to_owned(),
            "version 29 on the generator's own pairs (plan 15, P15.T10.b).".to_owned(),
        ],
        notes: xrb_notes(fit),
        items,
    }
}

/// The block's notes after its header.
#[must_use]
fn xrb_notes(fit: &XrbFit) -> Vec<String> {
    let p = &fit.params;
    let text = format!(
        "\
Read by `XRB_SAMPLER` and `CLASS_SHARES`, before the block. The marks are fitted to the
engine's members of the nine kinds `XrbHostKind` holds at generator version {version}'s rules
(hyperion-fit's `tasks::binary_classes::xrb`), each to the kinds the class draws it for: the
low-mass and history marks to the solar circle's slab, the high-mass marks to the young thin
disc's own census of {young} records, each unconditional but the low-mass donor's mass, which
follows the period with a law at each node, from the members under the node's triangle where
their Kish size is {min} or more. A mark with under {min} members keeps `XRB_SAMPLER`'s law. The
kinds' weights are the galaxy's shares of its hosts of each kind, the black-hole transients'
pinned at `BLACK_HOLE_TRANSIENT_SHARE` and the rest keeping their ratios; each mark's members
are weighted so that their kinds are in those shares. The row counts every member of the class and is
scaled to {total:e} hosts at Milky Way parameters (plan 11's design note 11).",
        version = p.census.version.get(),
        young = p.census.young_records,
        min = p.min_members,
        total = p.total_hosts,
    );
    text.lines().map(str::to_owned).collect()
}

/// The acceptance's figures for `fit`.
#[must_use]
fn xrb_acceptance(fit: &XrbFit) -> String {
    let s = &fit.sampler;
    let scratch: Vec<String> = s
        .sources
        .iter()
        .zip(XrbMark::ALL)
        .filter(|(source, _)| **source == MarkSource::Scratch)
        .map(|(_, mark)| format!("{mark:?}"))
        .collect();
    let joined = |v: &[usize]| {
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/")
    };
    let shares = |v: &[f64]| {
        v.iter()
            .map(|w| format!("{w:.4}"))
            .collect::<Vec<_>>()
            .join("/")
    };
    let high: f64 = s.high_mass_kinds.iter().sum();
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "slab members by kind {}; young members by kind {}; census kinds {}; weights {} and {}; \
         black-hole transients {:.0} of {:e} (the census's {:.0}); high-mass hosts {:.0}; donor \
         mass's own nodes {}; layers C {:.3} D {:.3} E {:.3}; rates a system {}; scratch marks: \
         {}; the K–S tests against the other half of the census are the sim's slow \
         `fitted_tables::the_x_ray_binary_sampler_against_the_engine`",
        joined(&s.slab_members),
        joined(&s.young_members),
        shares(&s.census_kinds),
        shares(&s.low_mass_kinds),
        shares(&s.high_mass_kinds),
        s.low_mass_kinds[3] * fit.params.total_hosts,
        fit.params.total_hosts,
        s.census_kinds[3] * fit.params.total_hosts,
        high * fit.params.total_hosts,
        s.marks[XrbMark::LowMassDonorMass.index()].own_nodes,
        fit.row.layer_shares[2],
        fit.row.layer_shares[3],
        fit.row.layer_shares[4],
        fit.row
            .rate_per_system
            .iter()
            .map(|r| format!("{r:.2e}"))
            .collect::<Vec<_>>()
            .join("/"),
        scratch.join(", "),
    );
    out
}

/// The task: P15.T10.b's X-ray binaries, slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BinaryXrbTask;

impl FitTask for BinaryXrbTask {
    fn name(&self) -> &'static str {
        "binary_xrb"
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
        XRB_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &XRB_ITEMS
    }

    /// The galaxy's band shares of layers D and E and its systems born by population, which the
    /// row and the kinds' weights read; and the engine on one layer-E cell's records: its
    /// records, its members, and each X-ray member's compact star's mass, donor's mass and rate
    /// over the line.
    fn fingerprint(&self) -> SimFingerprint {
        let galaxy = fingerprint_galaxy();
        let shares = BandShares::of(galaxy.mass_function());
        let mut probes: Vec<(String, f64)> = [MassBand::D, MassBand::E]
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
        let census = class_census_in(
            &galaxy,
            MassBand::E,
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
            if let Some(x) = m.xray() {
                probes.push((format!("member {i} compact star"), x.accretor_mass.value()));
                probes.push((format!("member {i} donor"), x.donor_mass.value()));
                probes.push((
                    format!("member {i} rate excess"),
                    x.rate_excess_dex.unwrap_or(-99.0),
                ));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = XrbParams::of(manifest)?;
        let fit = fit_xrb(&params, threads).map_err(|e| census_error("binary_xrb", e))?;
        Ok(TaskOutput {
            table: render_xrb(&fit),
            source:
                "plan 11's binary engine (Hurley, Tout and Pols 2002, MNRAS 329, 897, as P11.T4 \
                     builds it) and classes (P11.T5), over plan 06's tracks, at the manifest's \
                     generator version's rules; the total against the brainstorm's 10⁴, the \
                     black-hole transients pinned at Corral-Santana et al.'s (2016, A&A 587, A61) \
                     observed 1,300"
                    .to_owned(),
            acceptance: xrb_acceptance(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::stellar::binary::{BinaryClass, XrbKind};
    use hyperion_sim::units::Seconds;
    use hyperion_sim::units::{SolarMasses, SolarMassesPerYear, Years};

    use super::*;

    /// A low-mass neutron-star transient with a main-sequence donor of 0.5 M☉ on a 10-hour
    /// orbit, its rate a dex under the line.
    fn transient() -> XrayMarks {
        XrayMarks {
            age: Years::new(5.0e9),
            class: BinaryClass::LowMassXrayBinary(XrbKind::Transient),
            accretor_phase: Phase::NeutronStar,
            accretor_mass: SolarMasses::new(1.4),
            donor_mass: SolarMasses::new(0.5),
            donor_phase: Phase::MainSequence,
            period: Some(Seconds::new(36_000.0)),
            eccentricity: None,
            transfer_rate: Some(SolarMassesPerYear::new(1.0e-10)),
            recoil_speed: None,
            since_supernova: Some(Years::new(1.0e9)),
            accretor_initial_mass: SolarMasses::new(10.0),
            donor_initial_mass: SolarMasses::new(1.0),
            rate_excess_dex: Some(-1.0),
        }
    }

    /// A low-mass member gives its kind's marks and none of the high-mass kinds' or the other
    /// accretor's; an ultracompact one, whose donor is a helium white dwarf, gives the
    /// ultracompact marks and its neutron star's, and none of the hydrogen-rich donors'; one beside
    /// a black hole gives no compact star's mark.
    #[test]
    fn a_member_gives_the_marks_its_kind_draws() {
        let x = transient();
        assert_eq!(x.member_kind(), Some(XrbHostKind::NeutronStarTransient));
        let pair = |mark| xrb_pair(&x, mark);
        assert_eq!(pair(XrbMark::NeutronStarMass), Some((0.0, 1.4)));
        assert_eq!(pair(XrbMark::BlackHoleMass), None);
        assert_eq!(pair(XrbMark::LowMassPeriod), Some((0.0, 1.0)));
        assert_eq!(pair(XrbMark::LowMassDonorMass).map(|p| p.0), Some(1.0));
        assert_eq!(pair(XrbMark::LowMassRateExcess), Some((0.0, -1.0)));
        assert_eq!(pair(XrbMark::NeutronStarProgenitor), Some((0.0, 1.0)));
        let excess = pair(XrbMark::DonorInitialExcess)
            .expect("a low-mass donor's excess")
            .1;
        assert!((excess - math::log10(2.0)).abs() < 1e-12, "{excess}");
        for mark in HIGH_MASS_MARKS {
            assert_eq!(pair(mark), None, "{mark:?}");
        }
        assert_eq!(pair(XrbMark::NeutronStarRecurrence), None);
        assert_eq!(pair(XrbMark::UltracompactPeriod), None);
        let ultracompact = XrayMarks {
            donor_phase: Phase::HeliumWhiteDwarf,
            donor_mass: SolarMasses::new(0.012),
            period: Some(Seconds::new(3_600.0)),
            ..transient()
        };
        assert_eq!(ultracompact.member_kind(), Some(XrbHostKind::Ultracompact));
        let period = xrb_pair(&ultracompact, XrbMark::UltracompactPeriod).expect("its period");
        assert!((period.1 - math::log10(60.0)).abs() < 1e-12);
        assert_eq!(
            xrb_pair(&ultracompact, XrbMark::UltracompactDonorInitialMass),
            Some((0.0, 0.0))
        );
        assert_eq!(
            xrb_pair(&ultracompact, XrbMark::NeutronStarMass),
            Some((0.0, 1.4))
        );
        for mark in [
            XrbMark::LowMassPeriod,
            XrbMark::LowMassDonorMass,
            XrbMark::DonorInitialExcess,
        ] {
            assert_eq!(xrb_pair(&ultracompact, mark), None, "{mark:?}");
        }
        let beside_a_black_hole = XrayMarks {
            accretor_phase: Phase::BlackHole,
            ..ultracompact
        };
        assert_eq!(
            beside_a_black_hole.member_kind(),
            Some(XrbHostKind::Ultracompact)
        );
        assert_eq!(
            xrb_pair(&beside_a_black_hole, XrbMark::NeutronStarMass),
            None
        );
        assert_eq!(xrb_pair(&beside_a_black_hole, XrbMark::BlackHoleMass), None);
        assert_eq!(
            xrb_pair(&beside_a_black_hole, XrbMark::NeutronStarProgenitor),
            None
        );
        assert!(xrb_pair(&beside_a_black_hole, XrbMark::UltracompactPeriod).is_some());
    }

    /// The weights pin the black-hole transients and keep the other kinds' ratios, summing to
    /// one.
    #[test]
    fn the_weights_pin_the_black_hole_transients() {
        let census = [0.005, 0.58, 0.002, 0.02, 0.008, 0.0001, 0.38, 0.001, 0.0039];
        let weights = kind_weights(&census);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((weights[3] - BLACK_HOLE_TRANSIENT_SHARE).abs() < 1e-15);
        assert!((weights[1] / weights[6] - census[1] / census[6]).abs() < 1e-12);
    }
}
