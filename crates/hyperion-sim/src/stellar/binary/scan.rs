//! The scan marks of the binary catalogue classes (plan 11's Provides; P11.T8.a and P11.T8.b;
//! design notes 10 and 12): what a galaxy-wide scan needs of a host, drawn first and on draws of
//! its own, without the rest of the system and without the binary engine.
//!
//! Ten million accreting white dwarfs must be walked in seconds (design note 10), so a host's scan
//! marks are its first draws: its site and the thinning that places it, its nova recurrence period
//! and the white dwarf's mass it follows from, and its velocity. [`scan_marks`] draws them for one
//! candidate of a class cell, and is `Some` exactly when the class keeps the candidate as an entry
//! (`AccretingWdFast` and `AccretingWdSlow` in
//! [`catalogue_classes::binary`](crate::galaxy::catalogue_classes::binary) draw the same marks
//! first and then the rest). The catalogue's own test cannot refuse a candidate the scan keeps: its
//! one way of refusing one, its recurrence on the other side of L, is the scan's own last check.
//!
//! The X-ray binaries' marks ([`XrbScanMarks`], P11.T8.b) are their kind, their place and motion
//! at the epoch and a low-mass transient's outburst recurrence. [`scan_marks`] and [`scan_cell`]
//! take either class's process ([`ScanProcess`]). The class's test refuses an X-ray binary
//! candidate the scan keeps only if none of its Be/X donor's first
//! [`BE_DONOR_ATTEMPTS`](crate::galaxy::catalogue_classes::binary::BE_DONOR_ATTEMPTS) draws make
//! it plan 06's Be star, a chance under 10⁻⁴⁹ (the class's documentation).
//!
//! # The recurrence
//!
//! The recurrence period `P_rec` is the nova ignition mass over the transfer rate (P11.T5's
//! `nova_ignition_mass`, a fit to Yaron et al.'s 2005 grid). The sampler draws `P_rec` first, on
//! its class's side of L ([`AWD_SAMPLER`]'s ranks below or above L's), then the white dwarf's mass
//! given it, and the rate is the fit inverted at that mass (`transfer_rate_for`); `P_rec` is then
//! recomputed from the mass and the rate by the very function `carved_class` reads, so that the
//! scan and the class's test split the hosts on the same number.
//!
//! # Novae
//!
//! A host's novae are `0x0300 NOVA` events on its white dwarf, body 0 ([`nova_series`]): one
//! monotone phase (plan 06's `MonotonePhase`) on a [`LinearClock`] of period `P_rec` whose origin
//! the host draws in one period ([`nova_clock`]), with the phase noise of [`nova_phase`]. A slow
//! host recurs more slowly than L, so the source horizon, L + 2H long, holds at most two of its
//! eruptions; they are part of its scan marks ([`AwdScanMarks::eruptions`]), found by one call of
//! the phase when the marks are drawn, and a scan skips every slow host whose list misses its
//! interval. A fast host's are asked for over an interval ([`AwdScanMarks::eruptions_in`]). P11.T9
//! lists the same series.
//!
//! # The engine counter
//!
//! [`engine_calls`] (test builds and the `testing` feature) counts this thread's runs of the
//! binary engine, so that a test can show that a scan of 10⁶ hosts runs none.

#[cfg(any(test, feature = "testing"))]
use std::cell::Cell;

mod merger;
mod nsm;

pub use merger::{MergerScanMarks, horizon_time, merger_scan_marks, scan_merger_cell};
pub use nsm::{NsmScanMarks, nsm_scan_marks, scan_nsm_cell};

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::events::{
    CycleEvent, EventSeries, LinearClock, MonotonePhase, PhaseClock, TimeWindow, octaves_to_span,
};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide, XrayBinaries, XrbSide};
use crate::galaxy::fields::ComponentId;
use crate::id::{BodyId, CatalogueSystemId, SystemId, event_tags};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::isotropic;
use crate::tables::binary::{AWD_SAMPLER, AwdMark, XRB_SAMPLER, XrbHostKind, XrbMark, XrbPick};
use crate::time::{SourceHorizon, Span, UniverseTime};
use crate::units::consts::{
    SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR, SOLAR_MASS_KG, SPEED_OF_LIGHT,
};
use crate::units::{
    GravitationalParameter, Metres, PerCubicLightYear, Radians, Seconds, SolarMasses,
    SolarMassesPerYear, Years,
};

use super::classify::{nova_recurrence_period, recurrence_class, xrb_line};
use super::light_curves::{XrayAccretor, eddington_luminosity};
use super::sampler::ClassSampler;

/// The amplitude of a host's nova phase noise at its smallest octave, cycles: 0.08, ours,
/// provisional. It is near the largest a one-cycle lattice allows (10.3 × 0.08 = 0.82 of plan
/// 06's slope bound of 0.9), so that eruptions wander by some tens of per cent of a period, as
/// recurrent novae's intervals do: U Scorpii's ten eruptions of 1863–2010 fall 8–43 years apart
/// (Schaefer 2010, ApJS 187, 275, its eruption list).
pub const NOVA_NOISE_AMPLITUDE_CYCLES: f64 = 0.08;

/// The lattice spacing of a host's nova phase noise at its smallest octave, cycles: one, so that
/// each eruption has a value of its own, as plan 06's thermal pulses do.
pub const NOVA_NOISE_LATTICE_CYCLES: u32 = 1;

/// The fastest transfer a catalogue host is given, M☉ yr⁻¹: 10⁻⁷, under the steady-burning rate
/// of 1.03 × 10⁻⁷ above which a white dwarf burns steadily and erupts in no nova (Hurley, Tout and
/// Pols 2002, section 2.6.6; P11.T5's `STEADY_BURNING_RATE`), where the recurrence would be
/// infinite and a host's nova clock undefined. The recurrence falls as the rate rises
/// (Ṁ^−1.29), so a held rate lengthens it; at [`AWD_SAMPLER`]'s recurrences, 28,000 years and
/// more, the rate stays under 10⁻⁸ and the hold binds nowhere (a test holds it). Were it to bind,
/// a fast host could recur beyond L, which the scan and the class's test would both refuse.
pub const MAX_NOVA_TRANSFER_RATE: SolarMassesPerYear = SolarMassesPerYear::new(1.0e-7);

/// What a galaxy-wide scan reads of an accreting white dwarf host (plan 11's Provides; P11.T8.a):
/// its place and motion, its recurrence period and nova series, and a slow host's eruptions in the
/// source horizon.
///
/// # Examples
///
/// A scan of one cell of slow hosts near the Sun-like point, keeping those that erupted in the last
/// ten thousand years: a slow host's list is all a scan reads of its novae.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::AccretingWdSlow;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::binary::scan::scan_cell;
/// use hyperion_sim::time::{LIGHT_CROSSING_L, UniverseTime};
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = AccretingWdSlow::new(&galaxy);
/// let class = ClassId::ACCRETING_WHITE_DWARF_SLOW;
/// let mut hosts = Vec::new();
/// for x in -2..2 {
///     let key = CatalogueCellKey::new(class, [x, 50, 0])?;
///     scan_cell(&galaxy, process.hosts(), key, &mut hosts);
/// }
/// assert!(!hosts.is_empty());
/// let since = UniverseTime::from_julian_years(-10_000).ok_or("on the clock")?;
/// let recent: Vec<_> = hosts
///     .iter()
///     .filter(|h| h.eruptions().iter().any(|&t| since <= t && t < UniverseTime::EPOCH))
///     .collect();
/// assert!(recent.len() < hosts.len());
/// let l = LIGHT_CROSSING_L.as_julian_years_f64();
/// assert!(hosts.iter().all(|h| h.eruptions().len() <= 2 && h.recurrence().value() > l));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwdScanMarks {
    id: CatalogueSystemId,
    side: AwdSide,
    component: ComponentId,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    white_dwarf_mass: SolarMasses,
    transfer_rate: SolarMassesPerYear,
    recurrence: Years,
    series: EventSeries,
    clock: LinearClock,
    phase: MonotonePhase,
    eruptions: [UniverseTime; 2],
    eruption_count: u8,
}

impl AwdScanMarks {
    /// The host's ID: member 0 of its catalogue entry.
    #[must_use]
    pub const fn id(&self) -> CatalogueSystemId {
        self.id
    }

    /// Which of the two classes the host is in.
    #[must_use]
    pub const fn side(&self) -> AwdSide {
        self.side
    }

    /// The density component the host was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// Where the host is at the epoch: its site in its cell.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The host's velocity, galactic, the one its entry moves at (plan 09's rule 6).
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The host's position at `t`, on its straight line through its site at the epoch.
    ///
    /// # Panics
    ///
    /// As plan 09's `CatalogueMotion::position_at`, beyond any time the clock reaches.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(self.position, self.velocity)
            .position_at(t)
    }

    /// The white dwarf's mass, M☉.
    #[must_use]
    pub const fn white_dwarf_mass(&self) -> SolarMasses {
        self.white_dwarf_mass
    }

    /// The rate of transfer onto the white dwarf, M☉ yr⁻¹.
    #[must_use]
    pub const fn transfer_rate(&self) -> SolarMassesPerYear {
        self.transfer_rate
    }

    /// The nova recurrence period, Julian years: the ignition mass over the transfer rate, at most
    /// L for a fast host and beyond it for a slow one.
    #[must_use]
    pub const fn recurrence(&self) -> Years {
        self.recurrence
    }

    /// The host's nova series: `0x0300 NOVA` on its white dwarf, which keys the events.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The nova clock: period `P_rec` from the host's drawn origin.
    #[must_use]
    pub const fn clock(&self) -> &LinearClock {
        &self.clock
    }

    /// The nova phase noise ([`nova_phase`] of the clock).
    #[must_use]
    pub const fn phase(&self) -> &MonotonePhase {
        &self.phase
    }

    /// A slow host's eruptions in the source horizon, `[−(H + L), +H)`, in time order: none, one or
    /// two. A fast host's list is empty; ask [`eruptions_in`](Self::eruptions_in) instead.
    #[must_use]
    pub fn eruptions(&self) -> &[UniverseTime] {
        &self.eruptions[..usize::from(self.eruption_count)]
    }

    /// Appends the host's novae in `window` to `out`, in time order: the monotone phase's events
    /// of its series there, for a host of either class.
    pub fn eruptions_in(&self, window: TimeWindow, out: &mut Vec<CycleEvent>) {
        self.phase.events_in(&self.series, &self.clock, window, out);
    }
}

/// What a galaxy-wide scan reads of an X-ray binary host (plan 11's Provides; P11.T8.b): its kind,
/// its place and motion, a high-mass host's recoil from its supernova, and a low-mass transient's
/// outburst recurrence. The transients' and the Be/X outbursts' events are P11.T9's.
///
/// # Examples
///
/// The X-ray binaries about the centre and across the inner disc: the low-mass ones sit where they
/// were placed, and a high-mass one has moved from its supernova at its recoil since.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::XrayBinaries;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::binary::scan::scan_cell;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = XrayBinaries::new(&galaxy);
/// let mut hosts = Vec::new();
/// for x in -8..8 {
///     for y in [-1, 0, 11, 12] {
///         let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, 0])?;
///         scan_cell(&galaxy, &process, key, &mut hosts);
///     }
/// }
/// assert!(!hosts.is_empty());
/// for host in &hosts {
///     assert_eq!(host.kind().is_high_mass(), host.recoil().is_some());
///     assert_eq!(host.kind().is_transient(), host.recurrence().is_some());
///     if host.recoil().is_none() {
///         assert_eq!(host.position(), host.site());
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbScanMarks {
    id: CatalogueSystemId,
    kind: XrbHostKind,
    component: ComponentId,
    site: GalacticPosition,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    recoil: Option<XrbRecoil>,
    recurrence: Option<Years>,
    accretor_mass: SolarMasses,
    shape: XrbShape,
    transfer_rate: Option<SolarMassesPerYear>,
}

impl XrbScanMarks {
    /// The host's ID: member 0 of its catalogue entry.
    #[must_use]
    pub const fn id(&self) -> CatalogueSystemId {
        self.id
    }

    /// The host's kind.
    #[must_use]
    pub const fn kind(&self) -> XrbHostKind {
        self.kind
    }

    /// The density component the host was drawn from: the young disc's for a high-mass host.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// Where the host was placed in its cell: for a high-mass host, where its supernova was,
    /// relative to its component's field; for a low-mass one, its position at the epoch.
    #[must_use]
    pub const fn site(&self) -> &GalacticPosition {
        &self.site
    }

    /// Where the host is at the epoch: its site, moved by a high-mass host's recoil over the years
    /// since its supernova.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The host's velocity, galactic, the one its entry moves at (plan 09's rule 6): its
    /// component's draw at its site, plus a high-mass host's recoil.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The host's position at `t`, on its straight line through its position at the epoch.
    ///
    /// # Panics
    ///
    /// As plan 09's `CatalogueMotion::position_at`, beyond any time the clock reaches.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(self.position, self.velocity)
            .position_at(t)
    }

    /// A high-mass host's recoil from its supernova; `None` for a low-mass host.
    #[must_use]
    pub const fn recoil(&self) -> Option<&XrbRecoil> {
        self.recoil.as_ref()
    }

    /// A low-mass transient's outburst recurrence, Julian years; `None` for every other kind.
    #[must_use]
    pub const fn recurrence(&self) -> Option<Years> {
        self.recurrence
    }

    /// The compact star's mass, M☉: a neutron star's 1.2–2.2, a black hole's 5–15.
    #[must_use]
    pub const fn accretor_mass(&self) -> SolarMasses {
        self.accretor_mass
    }

    /// The orbital period.
    #[must_use]
    pub fn period(&self) -> Seconds {
        match self.shape {
            XrbShape::LowMass { period_hours, .. } => Seconds::new(period_hours * 3_600.0),
            XrbShape::BeX { period_days, .. } | XrbShape::Supergiant { period_days, .. } => {
                Seconds::new(period_days * SECONDS_PER_DAY)
            }
        }
    }

    /// The mean rate of transfer onto the compact star, M☉ yr⁻¹, held to its Eddington rate: a
    /// low-mass host's through its disc, on its kind's side of the irradiated disc's instability
    /// line by at least [`RATE_EXCESS_MARGIN_DEX`], or a supergiant host's; `None` for a Be/X
    /// host, marked detached.
    #[must_use]
    pub const fn transfer_rate(&self) -> Option<SolarMassesPerYear> {
        self.transfer_rate
    }

    /// What its binary is by its kind, as the scan drew it.
    #[must_use]
    pub(crate) const fn shape(&self) -> XrbShape {
        self.shape
    }
}

/// A high-mass X-ray binary's recoil (P11.T8.b): the systemic velocity its compact star's
/// supernova gave the pair (plan 11's P11.T4.e kick, drawn here from the class's table), and the
/// years since, over which it carried the pair from its site on a straight line, as plan 08's
/// `explosion_site` carries a record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbRecoil {
    velocity: GalacticVelocity,
    since_supernova: Years,
}

impl XrbRecoil {
    /// The recoil, galactic, against the host's component's field.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The Julian years from the supernova to the epoch.
    #[must_use]
    pub const fn since_supernova(&self) -> Years {
        self.since_supernova
    }

    /// The displacement from the site to the host at the epoch: the recoil over the years since.
    #[must_use]
    pub fn displacement(&self) -> GalacticDisplacement {
        self.velocity.displacement_over(Seconds::new(
            self.since_supernova.value() * SECONDS_PER_JULIAN_YEAR,
        ))
    }
}

/// The least distance of a low-mass host's transfer rate from the irradiated disc's instability
/// line, dex: 10⁻⁶, so that the rate falls on its kind's side of the line however `classify`
/// rounds the line (P11.T8.b).
pub const RATE_EXCESS_MARGIN_DEX: f64 = 1.0e-6;

/// What an X-ray binary host's binary is now by its kind, as its scan marks draw it (P11.T8.b):
/// the marks after its compact star's mass on `class.xrb.binary`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum XrbShape {
    /// A low-mass host: its period, hours, its donor's mass, and its rate over the irradiated
    /// disc's instability line, dex, on its kind's side by at least [`RATE_EXCESS_MARGIN_DEX`].
    LowMass {
        period_hours: f64,
        donor_mass: SolarMasses,
        rate_excess_dex: f64,
    },
    /// A Be/X host: its period, days, eccentricity, and its Be star's initial mass on its own
    /// track.
    BeX {
        period_days: f64,
        eccentricity: Eccentricity,
        donor_mass: SolarMasses,
    },
    /// A supergiant host: its period, days, its donor's mass and its transfer rate.
    Supergiant {
        period_days: f64,
        donor_mass: SolarMasses,
        rate: SolarMassesPerYear,
    },
}

impl XrbShape {
    /// The marks of a host of `kind` from `stream`, three words, the low-mass rate's ranks split at
    /// `rate_split`, the line's.
    #[must_use]
    fn draw(
        sampler: ClassSampler<XrbMark, XrbPick>,
        kind: XrbHostKind,
        rate_split: f64,
        stream: &mut Stream,
    ) -> Self {
        match kind {
            XrbHostKind::NeutronStarPersistent
            | XrbHostKind::NeutronStarTransient
            | XrbHostKind::BlackHolePersistent
            | XrbHostKind::BlackHoleTransient => {
                let log_period = sampler.draw(XrbMark::LowMassPeriod, 0.0, stream);
                let log_donor = sampler.draw(XrbMark::LowMassDonorMass, log_period, stream);
                let transient = kind.is_transient();
                let ranks = if transient {
                    (0.0, rate_split)
                } else {
                    (rate_split, 1.0)
                };
                let drawn = sampler.draw_in_ranks(XrbMark::LowMassRateExcess, 0.0, ranks, stream);
                Self::LowMass {
                    period_hours: math::exp10(log_period),
                    donor_mass: SolarMasses::new(math::exp10(log_donor)),
                    rate_excess_dex: if transient {
                        drawn.min(-RATE_EXCESS_MARGIN_DEX)
                    } else {
                        drawn.max(RATE_EXCESS_MARGIN_DEX)
                    },
                }
            }
            XrbHostKind::BeX => Self::BeX {
                period_days: math::exp10(sampler.draw(XrbMark::BexPeriod, 0.0, stream)),
                eccentricity: Eccentricity::new(sampler.draw(
                    XrbMark::BexEccentricity,
                    0.0,
                    stream,
                ))
                .expect("the table holds a Be/X host's eccentricity to 0–0.85"),
                donor_mass: SolarMasses::new(sampler.draw(XrbMark::BexDonorMass, 0.0, stream)),
            },
            XrbHostKind::Supergiant => Self::Supergiant {
                period_days: math::exp10(sampler.draw(XrbMark::SupergiantPeriod, 0.0, stream)),
                donor_mass: SolarMasses::new(sampler.draw(
                    XrbMark::SupergiantDonorMass,
                    0.0,
                    stream,
                )),
                rate: SolarMassesPerYear::new(math::exp10(sampler.draw(
                    XrbMark::SupergiantRate,
                    0.0,
                    stream,
                ))),
            },
        }
    }

    /// The rate onto a compact star `accretor` of `accretor_mass` of a host of this shape, M☉
    /// yr⁻¹, held to its Eddington rate ([`eddington_rate`]): a low-mass host's line ([`xrb_line`]
    /// at the circular orbit's separation) times its excess, a persistent host's never below the
    /// line, so that a line above the Eddington rate leaves it just above the line; a supergiant
    /// host's own; `None` for a Be/X host.
    #[must_use]
    fn transfer_rate(
        &self,
        accretor: XrayAccretor,
        accretor_mass: SolarMasses,
    ) -> Option<SolarMassesPerYear> {
        let eddington = eddington_rate(accretor, accretor_mass).value();
        match *self {
            Self::LowMass {
                period_hours,
                donor_mass,
                rate_excess_dex,
            } => {
                let separation = circular_separation(
                    Seconds::new(period_hours * 3_600.0),
                    accretor_mass + donor_mass,
                );
                let line = xrb_line(accretor_mass, donor_mass, separation).value();
                let held = (line * math::exp10(rate_excess_dex)).min(eddington);
                let rate = if rate_excess_dex > 0.0 {
                    held.max(line * math::exp10(RATE_EXCESS_MARGIN_DEX))
                } else {
                    held
                };
                Some(SolarMassesPerYear::new(rate))
            }
            Self::Supergiant { rate, .. } => {
                Some(SolarMassesPerYear::new(rate.value().min(eddington)))
            }
            Self::BeX { .. } => None,
        }
    }
}

/// The hydrogen mass fraction of the matter a compact star accretes, as its Eddington rate takes
/// it: 0.7, a hydrogen-rich donor's (Tauris, Langer and Kramer 2012, equation 6, as P11.T9.a's
/// mixed burst fuel takes it).
const ACCRETED_HYDROGEN: f64 = 0.7;

/// The Eddington rate of accretion onto `accretor` of `mass`, M☉ yr⁻¹: the Eddington luminosity
/// of matter of [`ACCRETED_HYDROGEN`] (P11.T9.a's `eddington_luminosity`) over the accretion
/// efficiency times c², 2.2 × 10⁻⁸ M☉ yr⁻¹ for a 1.45 M☉ neutron star of plan 06's 12.2 km.
#[must_use]
pub(crate) fn eddington_rate(accretor: XrayAccretor, mass: SolarMasses) -> SolarMassesPerYear {
    let luminosity = eddington_luminosity(mass, ACCRETED_HYDROGEN).value();
    let kg_per_s =
        luminosity / (accretor.efficiency(mass.value()) * SPEED_OF_LIGHT * SPEED_OF_LIGHT);
    SolarMassesPerYear::new(kg_per_s * SECONDS_PER_JULIAN_YEAR / SOLAR_MASS_KG)
}

/// The semi-major axis of an orbit of `period` about a total of `mass`: Kepler's third law, the
/// arithmetic of [`KeplerElements::from_period`], bit for bit.
#[must_use]
fn circular_separation(period: Seconds, mass: SolarMasses) -> Metres {
    KeplerElements::from_period(
        period,
        GravitationalParameter::from_solar_masses(mass),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)
            .expect("zero angles are an orientation"),
        Radians::ZERO,
    )
    .expect("a period of hours to days about a few M☉ is an orbit")
    .semi_major_axis()
}

/// [`scan_marks`] of an X-ray binary candidate.
#[must_use]
fn xrb_scan_marks(
    galaxy: &Galaxy,
    process: &XrayBinaries,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<XrbScanMarks> {
    let (site, component, host_side) = process.site(galaxy, cell, id, bound)?;
    let seed = galaxy.seed();
    let key = ObjectKey::from(SystemId::from(id));
    let sampler = ClassSampler::new(&XRB_SAMPLER);
    let mut stream = Stream::open(seed, tags::CLASS_XRB_KIND, key);
    let kind = match host_side {
        XrbSide::LowMass => XrbHostKind::LOW_MASS[sampler.pick(XrbPick::LowMassKind, &mut stream)],
        XrbSide::HighMass => {
            XrbHostKind::HIGH_MASS[sampler.pick(XrbPick::HighMassKind, &mut stream)]
        }
    };
    let recurrence_mark = match kind {
        XrbHostKind::NeutronStarTransient => Some(XrbMark::NeutronStarRecurrence),
        XrbHostKind::BlackHoleTransient => Some(XrbMark::BlackHoleRecurrence),
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::BlackHolePersistent
        | XrbHostKind::BeX
        | XrbHostKind::Supergiant => None,
    };
    let recurrence =
        recurrence_mark.map(|mark| Years::new(math::exp10(sampler.draw(mark, 0.0, &mut stream))));
    let mut stream = Stream::open(seed, tags::CLASS_XRB_BINARY, key);
    let accretor_mark = match kind.accretor() {
        XrayAccretor::NeutronStar => XrbMark::NeutronStarMass,
        XrayAccretor::BlackHole => XrbMark::BlackHoleMass,
    };
    let accretor_mass = SolarMasses::new(sampler.draw(accretor_mark, 0.0, &mut stream));
    let shape = XrbShape::draw(sampler, kind, process.rate_split(), &mut stream);
    let recoil_marks = match kind {
        XrbHostKind::BeX => Some((XrbMark::BexRecoil, XrbMark::BexSinceSupernova)),
        XrbHostKind::Supergiant => {
            Some((XrbMark::SupergiantRecoil, XrbMark::SupergiantSinceSupernova))
        }
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::NeutronStarTransient
        | XrbHostKind::BlackHolePersistent
        | XrbHostKind::BlackHoleTransient => None,
    };
    let recoil = recoil_marks.map(|(speed_mark, since_mark)| {
        let mut stream = Stream::open(seed, tags::CLASS_XRB_RECOIL, key);
        let metres_per_second = sampler.draw(speed_mark, 0.0, &mut stream) * 1.0e3;
        let direction = isotropic(&mut stream).components();
        let since = math::exp10(sampler.draw(since_mark, 0.0, &mut stream));
        XrbRecoil {
            velocity: GalacticVelocity::new(direction.map(|u| u * metres_per_second)),
            since_supernova: Years::new(since),
        }
    });
    let drawn = XrayBinaries::velocity(galaxy, component, &site, key);
    let (position, velocity) = match &recoil {
        Some(recoil) => (
            site.translated(recoil.displacement()).expect(
                "a finite recoil moving a host under 3,000 ly from its site carries its cell \
                 nowhere near the i32 range's end",
            ),
            drawn + recoil.velocity,
        ),
        None => (site, drawn),
    };
    Some(XrbScanMarks {
        id,
        kind,
        component,
        site,
        position,
        velocity,
        recoil,
        recurrence,
        accretor_mass,
        transfer_rate: shape.transfer_rate(kind.accretor(), accretor_mass),
        shape,
    })
}

/// A binary catalogue class as a galaxy-wide scan walks it (P11.T8): its bound over a cell and
/// each candidate's scan marks, which are the entry's own, bit for bit. The accreting white dwarfs'
/// [`AwdProcess`] (P11.T8.a) and the X-ray binaries' [`XrayBinaries`] (P11.T8.b) are built.
pub trait ScanProcess {
    /// What a scan reads of one host.
    type Marks;

    /// The class's bound over `cell`: its [`ClassProcess`]'s.
    ///
    /// [`ClassProcess`]: crate::galaxy::catalogue_classes::ClassProcess
    fn scan_bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear;

    /// The scan marks of candidate `id` of `cell` under `bound`, or `None` if the class keeps no
    /// entry there ([`scan_marks`]).
    fn marks_of(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<Self::Marks>;
}

impl ScanProcess for AwdProcess {
    type Marks = AwdScanMarks;

    fn scan_bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        self.bound(galaxy, cell)
    }

    fn marks_of(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<AwdScanMarks> {
        awd_scan_marks(galaxy, self, cell, id, bound)
    }
}

impl ScanProcess for XrayBinaries {
    type Marks = XrbScanMarks;

    fn scan_bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        self.bound(galaxy, cell)
    }

    fn marks_of(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<XrbScanMarks> {
        xrb_scan_marks(galaxy, self, cell, id, bound)
    }
}

/// The scan marks of candidate `id` of the class cell `cell` of `process`'s class under its
/// `bound`, or `None` if the class keeps no entry there. It never runs the binary engine.
///
/// - An accreting white dwarf's (P11.T8.a): the site and its thinning, the recurrence on the
///   class's side of L, the velocity, the nova series and clock, and a slow host's eruptions in
///   the source horizon.
/// - An X-ray binary's (P11.T8.b): the site and its thinning, the kind on the side its component
///   gives, a high-mass host's recoil from its supernova, the place and velocity at the epoch, and
///   a low-mass transient's outburst recurrence.
///
/// The marks are the entry's own, bit for bit: the class's candidate draws them through this
/// function first.
///
/// # Panics
///
/// If `cell` is not a cell of `process`'s class, as plan 09's cell; in debug builds, on a density
/// above the bound.
#[must_use]
pub fn scan_marks<P: ScanProcess>(
    galaxy: &Galaxy,
    process: &P,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<P::Marks> {
    process.marks_of(galaxy, cell, id, bound)
}

/// [`scan_marks`] of an accreting white dwarf candidate.
#[must_use]
fn awd_scan_marks(
    galaxy: &Galaxy,
    process: &AwdProcess,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<AwdScanMarks> {
    let (position, component) = process.site(galaxy, cell, id, bound)?;
    let seed = galaxy.seed();
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let sampler = ClassSampler::new(&AWD_SAMPLER);
    let mut stream = Stream::open(seed, tags::CLASS_AWD_RECURRENCE, key);
    let drawn = sampler.draw_in_ranks(
        AwdMark::Recurrence,
        0.0,
        process.recurrence_ranks(),
        &mut stream,
    );
    let white_dwarf_mass =
        SolarMasses::new(sampler.draw(AwdMark::WhiteDwarfMass, drawn, &mut stream));
    let transfer_rate = transfer_rate_for(white_dwarf_mass, drawn);
    let recurrence = nova_recurrence_period(white_dwarf_mass, transfer_rate);
    if recurrence_class(recurrence.value()) != process.side().carved() {
        return None;
    }
    let clock = nova_clock(recurrence, stream.uniform());
    let velocity = AwdProcess::velocity(galaxy, component, &position, key);
    let series = nova_series(seed, system);
    let phase = nova_phase(&clock);
    let (eruptions, eruption_count) = match process.side() {
        AwdSide::Slow => horizon_eruptions(&phase, &series, &clock),
        AwdSide::Fast => ([UniverseTime::EPOCH; 2], 0),
    };
    Some(AwdScanMarks {
        id,
        side: process.side(),
        component,
        position,
        velocity,
        white_dwarf_mass,
        transfer_rate,
        recurrence,
        series,
        clock,
        phase,
        eruptions,
        eruption_count,
    })
}

/// The margin on the base phase beyond the noise's bound within which a cycle's eruption is
/// sought, cycles: 0.01, far above the few units in the last place by which the phase as computed
/// strays from monotone, and far below a cycle.
const HORIZON_PHASE_MARGIN_CYCLES: f64 = 0.01;

/// A slow host's eruptions in the source horizon, in time order, and how many there are: the
/// monotone phase's events there, as [`MonotonePhase::events_in`] lists them, found by
/// regenerating only the cycles whose crossing can lie inside it.
///
/// Cycle n's eruption is where Φ = φ + N(φ) reaches n, and |N| is at most the noise's bound B, so
/// an eruption inside the horizon has its base phase φ there within B of n: only the cycles from
/// φ(start) − B to φ(end) + B (with [`HORIZON_PHASE_MARGIN_CYCLES`] more on each side) can erupt
/// inside it. For a slow host that is one or two cycles, where the listing's one cycle of margin
/// on each side of Φ at the horizon's ends regenerates three or four; a test holds the two to the
/// same events.
///
/// # Panics
///
/// In debug builds, on more than two eruptions, which a recurrence beyond L cannot fit into
/// L + 2H, since the phase's slope stays under 1.9.
fn horizon_eruptions(
    phase: &MonotonePhase,
    series: &EventSeries,
    clock: &LinearClock,
) -> ([UniverseTime; 2], u8) {
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END)
        .expect("the source horizon runs forward");
    let margin = phase.noise_bound_cycles() + HORIZON_PHASE_MARGIN_CYCLES;
    let first = clock.base_phase(horizon.start()).offset(-margin);
    let first = if first.fraction() > 0.0 {
        first.cycles().saturating_add(1)
    } else {
        first.cycles()
    };
    let last = clock.base_phase(horizon.end()).offset(margin).cycles();
    let mut eruptions = [UniverseTime::EPOCH; 2];
    let mut count = 0_u8;
    for cycle in first..=last {
        let Some(event) = phase.event(series, clock, cycle) else {
            continue;
        };
        if !horizon.contains(event.time()) {
            continue;
        }
        debug_assert!(count < 2, "a slow host erupts thrice in the source horizon");
        if let Some(slot) = eruptions.get_mut(usize::from(count)) {
            *slot = event.time();
            count += 1;
        }
    }
    (eruptions, count)
}

/// Appends to `out` the scan marks of every entry of the class cell `key` of `process`'s class,
/// in index order: one bound, one count and [`scan_marks`] of each candidate, the cell's entries
/// exactly, without the rest of their systems.
///
/// # Panics
///
/// As [`scan_marks`], and as plan 09's count on a bound past the Poisson sampler's limit.
pub fn scan_cell<P: ScanProcess>(
    galaxy: &Galaxy,
    process: &P,
    key: CatalogueCellKey,
    out: &mut Vec<P::Marks>,
) {
    let bound = process.scan_bound(galaxy, &key);
    let candidates =
        crate::galaxy::catalogue_classes::grid::candidate_count(galaxy.seed(), &key, bound);
    out.extend((0..candidates).filter_map(|index| {
        let id = key
            .candidate_id(index, 0)
            .expect("a count held under 2^24 numbers every candidate");
        scan_marks(galaxy, process, &key, id, bound)
    }));
}

/// The series of the novae of catalogue system `system` in the universe of `seed`: `0x0300 NOVA`
/// on its white dwarf, body 0 (P11.T8.a).
#[must_use]
pub fn nova_series(seed: Seed, system: SystemId) -> EventSeries {
    EventSeries::new(seed, event_tags::NOVA, BodyId::new(system, 0).into())
}

/// The nova clock of a host whose novae recur every `recurrence`, its phase 0 at `share` of one
/// period before the epoch, `share` in [0, 1).
///
/// # Panics
///
/// If the period is not a span of at least [`LinearClock::MIN_PERIOD`]: a catalogue host's
/// recurrence is at least 28,000 years ([`AWD_SAMPLER`]'s least).
#[must_use]
pub fn nova_clock(recurrence: Years, share: f64) -> LinearClock {
    let seconds = recurrence.value() * SECONDS_PER_JULIAN_YEAR;
    let period = Span::from_seconds_f64(seconds).expect("a recurrence of years is a span");
    let back = Span::from_seconds_f64(share * seconds).expect("a share of a period is a span");
    let origin = UniverseTime::EPOCH
        .checked_sub(back)
        .expect("a period before the epoch is on the clock");
    LinearClock::new(period, origin).expect("a recurrence of years exceeds the clock's 16 s")
}

/// The phase noise of the novae on `clock`: [`NOVA_NOISE_AMPLITUDE_CYCLES`] on a lattice of
/// [`NOVA_NOISE_LATTICE_CYCLES`], with as many octaves as span the source horizon's cycles, so
/// that the phase wanders across it as plan 06's thermal pulses' does.
///
/// # Panics
///
/// Never: the amplitude's slope bound is 0.82, under plan 06's 0.9, and a period of 28,000 years
/// or more puts few cycles in the horizon.
#[must_use]
pub fn nova_phase(clock: &LinearClock) -> MonotonePhase {
    let horizon = SourceHorizon::END
        .checked_since(SourceHorizon::START)
        .expect("the source horizon runs forward");
    let octaves = octaves_to_span(NOVA_NOISE_LATTICE_CYCLES, clock.cycles_in(horizon));
    MonotonePhase::new(
        NOVA_NOISE_AMPLITUDE_CYCLES,
        NOVA_NOISE_LATTICE_CYCLES,
        octaves,
    )
    .expect("the nova noise's parameters are inside plan 06's bounds")
}

/// The transfer rate onto a white dwarf of `mass` at which its novae recur every
/// 10^`log_recurrence_years` years, M☉ yr⁻¹, held to [`MAX_NOVA_TRANSFER_RATE`]: P11.T5's
/// ignition-mass fit, log₁₀ `M_ign` = −3.728 − 0.2934 (log₁₀ Ṁ + 9) + 1.920 log₁₀(1.44 − M),
/// inverted in closed form for `P_rec` = `M_ign` ÷ Ṁ, with the fit's own holds (M to 0.6–1.40 M☉,
/// Ṁ in `M_ign` to 10⁻¹¹–10⁻⁷).
///
/// A test holds the recurrence of its rate to the asked one where the hold binds nowhere.
#[must_use]
pub(crate) fn transfer_rate_for(
    mass: SolarMasses,
    log_recurrence_years: f64,
) -> SolarMassesPerYear {
    let m = mass.value().clamp(0.6, 1.40);
    // log₁₀ M_ign at Ṁ = 10⁻⁹, and the rate's slope in it.
    let at_nine = -3.728 + 1.920 * math::log10(1.44 - m);
    let slope = 0.2934;
    // Inside the fit's rates log₁₀ P = at_nine − slope (x + 9) − x; outside them M_ign is held at
    // the bound's.
    let mid = (at_nine - slope * 9.0 - log_recurrence_years) / (1.0 + slope);
    let log_rate = if mid < -11.0 {
        at_nine + 2.0 * slope - log_recurrence_years
    } else if mid > -7.0 {
        at_nine - 2.0 * slope - log_recurrence_years
    } else {
        mid
    };
    SolarMassesPerYear::new(math::exp10(log_rate).min(MAX_NOVA_TRANSFER_RATE.value()))
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// This thread's runs of the binary engine.
    static ENGINE_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// How many times this thread has run the binary engine (`evolve`, plan 11's P11.T4) since it
/// started: a counter of test builds and the `testing` feature, for the scan's test that it runs
/// none (P11.T8.a).
#[cfg(any(test, feature = "testing"))]
#[must_use]
pub fn engine_calls() -> u64 {
    ENGINE_CALLS.with(Cell::get)
}

/// Counts one run of the binary engine on this thread.
#[cfg(any(test, feature = "testing"))]
pub(crate) fn count_engine_call() {
    ENGINE_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
}

#[cfg(test)]
mod tests;
