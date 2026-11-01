//! The X-ray binaries' sampler and share row (plan 15, P15.T10.b), task `binary_xrb`, fitted
//! against plan 11's engine as the accreting white dwarfs' are (the parent module): the same
//! censuses, the young thin disc's larger, since its high-mass hosts are a few in a hundred of its
//! layer-E records.
//!
//! The class's kinds ([`XrbHostKind`]) are the engine's members that `XrayMarks::host_kind` gives
//! one: a neutron star or a black hole fed by a hydrogen-rich low-mass donor, persistent or
//! transient, and a neutron star beside a Be star or a supergiant. The engine's members of no kind
//! (a symbiotic X-ray binary, a black hole beside a massive donor, and the ultracompact low-mass
//! systems, whose donors are helium stars or white dwarfs) are counted in the share row, which is
//! what the grid gives up, but fit no mark. The kinds' weights are the galaxy's shares of its
//! hosts of each kind: every population's rate of the kind times its systems. The low-mass and
//! history marks are fitted to the slab's members and the high-mass marks to the young thin
//! disc's own census; a mark with fewer than the manifest's `min_members`, the transients'
//! outburst recurrences among them, which the engine does not draw, keeps `XRB_SAMPLER`'s law.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::imf::{BandShares, MassBand};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::math;
use hyperion_sim::orbit::Eccentricity;
use hyperion_sim::stellar::binary::CarvedClass;
use hyperion_sim::stellar::binary::light_curves::XrayAccretor;
use hyperion_sim::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, XrayMarks, class_census_in,
};
use hyperion_sim::tables::binary::{XRB_SAMPLER, XrbHostKind, XrbMark};
use hyperion_sim::units::consts::SECONDS_PER_DAY;

use super::{
    CensusParams, Censuses, FINGERPRINT_SEED, FittedMark, MarkSource, RunCensusError, ShareRow,
    census_error, conditional, run_censuses, share_rows, unconditional,
};
use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The X-ray binaries' task's revision.
pub const XRB_VERSION: u32 = 0;

/// The X-ray binaries' block's items, in its order: the sampler, then the share row.
pub const XRB_ITEMS: [&str; 8] = [
    "XRB_RESIDUALS_26",
    "XRB_MEDIAN_SPANS_26",
    "XRB_MEDIANS_26",
    "XRB_LIMITS_26",
    "XRB_LOW_MASS_KINDS_26",
    "XRB_HIGH_MASS_KINDS_26",
    "XRB_HOSTS_PER_FORMED_MASS_26",
    "XRB_LAYER_SHARES_26",
];

/// The marks of the high-mass kinds, fitted to the young thin disc's own census.
const HIGH_MASS_MARKS: [XrbMark; 10] = [
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
];

/// The fingerprint's census: one layer-E cell of the solar circle's slab, of 33 records one of
/// whose pairs the engine makes an X-ray binary of a kind the class holds, so that a change to the
/// engine or to the classes moves the probes.
const FINGERPRINT_CELL: CensusSite = CensusSite::new([-384, 26_000, 0], [0, 0, 0]);

/// The X-ray binaries' fit parameters, from the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbParams {
    /// The censuses'.
    pub census: CensusParams,
    /// The galaxy's X-ray binaries at Milky Way parameters: the observed count the row is scaled
    /// to.
    pub total_hosts: f64,
    /// The least members a mark is fitted to.
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

/// What one X-ray binary member gives `mark`: its condition (the period's log₁₀, hours, for the
/// low-mass donor's mass, zero for every other mark) and value, in the mark's unit, or `None`
/// where it defines no such mark: a member of no catalogue kind, of another kind or accretor, or
/// a value that is not finite.
#[must_use]
pub fn xrb_pair(x: &XrayMarks, mark: XrbMark) -> Option<(f64, f64)> {
    let kind = x.host_kind()?;
    let log = |v: f64| (v > 0.0).then(|| math::log10(v));
    let low = !kind.is_high_mass();
    let neutron = matches!(kind.accretor(), XrayAccretor::NeutronStar);
    let (bex, supergiant) = (kind == XrbHostKind::BeX, kind == XrbHostKind::Supergiant);
    let log_hours = x.period.and_then(|p| log(p.value() / 3_600.0));
    let log_days = x.period.and_then(|p| log(p.value() / SECONDS_PER_DAY));
    let recoil = x.recoil_speed.map(|v| v.value() / 1.0e3);
    let log_since = x.since_supernova.and_then(|y| log(y.value()));
    let only = |holds: bool, value: Option<f64>| if holds { Some((0.0, value?)) } else { None };
    let pair = match mark {
        XrbMark::NeutronStarMass => only(neutron, Some(x.accretor_mass.value())),
        XrbMark::BlackHoleMass => only(!neutron, Some(x.accretor_mass.value())),
        XrbMark::LowMassPeriod => only(low, log_hours),
        XrbMark::LowMassDonorMass => {
            if low {
                Some((log_hours?, log(x.donor_mass.value())?))
            } else {
                None
            }
        }
        XrbMark::LowMassRateExcess => only(low, x.rate_excess_dex),
        XrbMark::NeutronStarRecurrence | XrbMark::BlackHoleRecurrence => None,
        XrbMark::BexPeriod => only(bex, log_days),
        XrbMark::BexEccentricity => only(bex, x.eccentricity.map(Eccentricity::value)),
        XrbMark::BexDonorMass => only(bex, Some(x.donor_mass.value())),
        XrbMark::SupergiantPeriod => only(supergiant, log_days),
        XrbMark::SupergiantDonorMass => only(supergiant, Some(x.donor_mass.value())),
        XrbMark::SupergiantRate => only(supergiant, x.transfer_rate.and_then(|r| log(r.value()))),
        XrbMark::BexRecoil => only(bex, recoil),
        XrbMark::SupergiantRecoil => only(supergiant, recoil),
        XrbMark::BexSinceSupernova => only(bex, log_since),
        XrbMark::SupergiantSinceSupernova => only(supergiant, log_since),
        XrbMark::NeutronStarProgenitor => only(neutron, log(x.accretor_initial_mass.value())),
        XrbMark::BlackHoleProgenitor => only(!neutron, log(x.accretor_initial_mass.value())),
        XrbMark::DonorInitialExcess => only(
            low,
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

/// The galaxy's shares of its X-ray binaries of each kind, in [`XrbHostKind::ALL`]'s order: every
/// population's rate of the kind a system, its bands weighted by their shares of the systems,
/// times its systems, over the same sum for every kind. Members of no kind are left out.
///
/// # Panics
///
/// If no census found a member of a kind, or one ran no record of a population.
#[must_use]
pub fn kind_shares(galaxy: &Galaxy, censuses: &Censuses) -> [f64; 6] {
    let shares = BandShares::of(galaxy.mass_function()).as_array();
    let fields = galaxy.fields();
    let mut totals = [0.0; 6];
    for population in POPULATIONS {
        let born = fields
            .components()
            .iter()
            .filter(|c| c.population() == population)
            .fold(0.0, |sum, c| sum + c.count());
        for census in censuses.of(population) {
            let records = census.scanned[population.index()];
            assert!(records > 0, "a census ran no record of {population:?}");
            let mut counts = [0_u32; 6];
            for m in census
                .members
                .iter()
                .filter(|m| m.record().population() == population)
            {
                if let Some(kind) = m.xray().and_then(XrayMarks::host_kind) {
                    let k = XrbHostKind::ALL
                        .iter()
                        .position(|&each| each == kind)
                        .expect("every kind is in `XrbHostKind::ALL`");
                    counts[k] += 1;
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
    assert!(sum > 0.0, "the censuses found members of the class's kinds");
    totals.map(|t| t / sum)
}

/// The X-ray binaries' fitted sampler: its marks in [`XrbMark`]'s order, where each came from,
/// and the two kind choices' weights.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbSampler {
    /// The marks.
    pub marks: [FittedMark; 20],
    /// Where each mark's members came from: its own class's, or none.
    pub sources: [MarkSource; 20],
    /// The low-mass kinds' weights, in [`XrbHostKind::LOW_MASS`]' order.
    pub low_mass_kinds: [f64; 4],
    /// The high-mass kinds' weights, in [`XrbHostKind::HIGH_MASS`]' order.
    pub high_mass_kinds: [f64; 2],
    /// The slab's members of a kind, and of none.
    pub slab_members: (usize, usize),
    /// The young thin disc's own census's members of a high-mass kind.
    pub young_high_mass_members: usize,
}

/// Fits the sampler from `censuses` in `galaxy` (module documentation).
///
/// # Panics
///
/// As [`kind_shares`].
#[must_use]
pub fn xrb_sampler(galaxy: &Galaxy, censuses: &Censuses, params: &XrbParams) -> XrbSampler {
    let slab = xray_members(censuses.slab.iter().flat_map(|c| &c.members));
    let young = xray_members(
        censuses
            .of(Population::YoungThinDisc)
            .iter()
            .flat_map(|c| &c.members),
    );
    let mut sources = [MarkSource::Scratch; 20];
    let marks = std::array::from_fn(|k| {
        let mark = XrbMark::ALL[k];
        let pool = if HIGH_MASS_MARKS.contains(&mark) {
            &young
        } else {
            &slab
        };
        let pairs: Vec<(f64, f64)> = pool.iter().filter_map(|x| xrb_pair(x, mark)).collect();
        if pairs.len() < params.min_members {
            return FittedMark::of_scratch(XRB_SAMPLER.mark(mark));
        }
        sources[k] = MarkSource::Own;
        if mark == XrbMark::LowMassDonorMass {
            conditional(&pairs)
        } else {
            unconditional(&pairs.iter().map(|p| p.1).collect::<Vec<_>>())
        }
    });
    let kinds = kind_shares(galaxy, censuses);
    let of_kind = slab.iter().filter(|x| x.host_kind().is_some()).count();
    XrbSampler {
        marks,
        sources,
        low_mass_kinds: [kinds[0], kinds[1], kinds[2], kinds[3]],
        high_mass_kinds: [kinds[4], kinds[5]],
        slab_members: (of_kind, slab.len() - of_kind),
        young_high_mass_members: young
            .iter()
            .filter(|x| x.host_kind().is_some_and(XrbHostKind::is_high_mass))
            .count(),
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
/// If the Milky Way fixture does not build, or as [`share_rows`] and [`kind_shares`]: the
/// censuses find no member of the class or of its kinds, or run no record of a population.
pub fn fit_xrb(params: &XrbParams, threads: NonZeroUsize) -> Result<XrbFit, RunCensusError> {
    let galaxy = Galaxy::from_params(
        Seed::new(params.census.seed),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture builds");
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

/// The block's contents for `fit`.
#[must_use]
pub fn render_xrb(fit: &XrbFit) -> RustTable {
    let s = &fit.sampler;
    let items = vec![
        TableItem::Nested {
            name: XRB_ITEMS[0].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' residual quantiles at the ranks k ÷ 16, in [`XrbMark`]'s order, each".to_owned(),
                "in its mark's unit.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.residual.to_vec()).collect(),
        },
        TableItem::Tuples {
            name: XRB_ITEMS[1].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' medians' spans of their conditions, first and last node, in".to_owned(),
                "[`XrbMark`]'s order; only the low-mass donor's mass's is read.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| vec![m.span.0, m.span.1]).collect(),
        },
        TableItem::Nested {
            name: XRB_ITEMS[2].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' medians at their 17 nodes, in [`XrbMark`]'s order; only the low-mass".to_owned(),
                "donor's mass's is read.".to_owned(),
            ],
            rows: s.marks.iter().map(|m| m.median.to_vec()).collect(),
        },
        TableItem::Tuples {
            name: XRB_ITEMS[3].to_owned(),
            doc: vec![
                "The X-ray binaries' marks' least and greatest values, in [`XrbMark`]'s order."
                    .to_owned(),
            ],
            rows: s.marks.iter().map(|m| vec![m.limits.0, m.limits.1]).collect(),
        },
        TableItem::Array {
            name: XRB_ITEMS[4].to_owned(),
            doc: vec![
                "The low-mass kinds' weights, as shares of all the class's hosts, in".to_owned(),
                "[`XrbHostKind::LOW_MASS`]' order.".to_owned(),
            ],
            values: s.low_mass_kinds.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[5].to_owned(),
            doc: vec![
                "The high-mass kinds' weights, as shares of all the class's hosts, in".to_owned(),
                "[`XrbHostKind::HIGH_MASS`]' order: their sum is the young thin disc's share.".to_owned(),
            ],
            values: s.high_mass_kinds.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[6].to_owned(),
            doc: vec![
                "The X-ray binaries' hosts per solar mass formed, M☉⁻¹, by population in [`POPULATIONS`]' order."
                    .to_owned(),
            ],
            values: fit.row.hosts_per_formed_mass.to_vec(),
        },
        TableItem::Array {
            name: XRB_ITEMS[7].to_owned(),
            doc: vec![
                "The share of the X-ray binaries' hosts whose primary lies in each layer, A–E.".to_owned(),
            ],
            values: fit.row.layer_shares.to_vec(),
        },
    ];
    RustTable {
        summary: vec![
            "The X-ray binaries' sampler and share row fitted against plan 11's engine (plan 15, P15.T10.b),"
                .to_owned(),
            "held until generator version 27 (`catalogue_classes::binary`'s hold).".to_owned(),
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
Read by `XRB_SAMPLER_26` and `CLASS_SHARES_26`, before the block. The marks are fitted to the
engine's members of the kinds `XrbHostKind` holds (hyperion-fit's `tasks::binary_classes::xrb`):
the low-mass and history marks to the solar circle's slab, the high-mass marks to the young thin
disc's own census of {young} records, each unconditional but the low-mass donor's mass, which
follows the period. A mark with under {min} members keeps `XRB_SAMPLER`'s law. The kinds' weights
are the galaxy's shares of its hosts of each kind; the row counts every member of the class,
those of no kind among them, and is scaled to {total:e} hosts at Milky Way parameters (plan 11's
design note 11).",
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
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "slab members {} of a kind and {} of none; young high-mass members {}; kinds {}; \
         black-hole transients {:.0} of {:e}; layers C {:.3} D {:.3} E {:.3}; rates a system {}; \
         scratch marks: {}; the K–S tests against the other half of the census are the sim's \
         slow `tables_hold::the_x_ray_binary_sampler_against_the_engine`",
        s.slab_members.0,
        s.slab_members.1,
        s.young_high_mass_members,
        s.low_mass_kinds
            .iter()
            .chain(&s.high_mass_kinds)
            .map(|w| format!("{w:.4}"))
            .collect::<Vec<_>>()
            .join("/"),
        s.low_mass_kinds[3] * fit.params.total_hosts,
        fit.params.total_hosts,
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
        let galaxy =
            Galaxy::from_params(Seed::new(FINGERPRINT_SEED), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture builds");
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
                     builds it) and classes (P11.T5), over plan 06's tracks, at the generator \
                     version's rules; the total against the brainstorm's 10⁴ (Corral-Santana et \
                     al. 2016, A&A 587, A61)"
                    .to_owned(),
            acceptance: xrb_acceptance(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::stellar::Phase;
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
    /// accretor's; an ultracompact one, whose donor is a helium white dwarf, gives none.
    #[test]
    fn a_member_gives_its_own_kind_s_marks() {
        let x = transient();
        assert_eq!(x.host_kind(), Some(XrbHostKind::NeutronStarTransient));
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
        let ultracompact = XrayMarks {
            donor_phase: Phase::HeliumWhiteDwarf,
            ..transient()
        };
        assert_eq!(ultracompact.host_kind(), None);
        assert!(
            XrbMark::ALL
                .iter()
                .all(|&m| xrb_pair(&ultracompact, m).is_none())
        );
    }
}
