//! The scan marks of the binary catalogue classes (plan 11's Provides; P11.T8.a and P11.T8.b;
//! design notes 10 and 12): what a galaxy-wide scan needs of a host, drawn first and on draws of
//! its own, without the rest of the system and without running the binary engine (from 26 a
//! stellar merger's evolved donor asks one onset of an engine built at its merger, `merger`).
//!
//! Ten million accreting white dwarfs must be walked in seconds (design note 10), so a host's scan
//! marks are its first draws: its site and the thinning that places it, its nova recurrence period
//! and the white dwarf's mass it follows from, and its velocity. [`scan_marks`] draws them for one
//! candidate of a class cell, and is `Some` exactly when the class keeps the candidate as an entry
//! (`AccretingWdFast` and `AccretingWdSlow` in
//! [`catalogue_classes::binary`](crate::galaxy::catalogue_classes::binary) draw the same marks
//! first and then the rest). The catalogue's own test cannot refuse a candidate the scan keeps: its
//! ways of refusing one, its recurrence on the other side of L and, from 27, a fast host fed helium,
//! are the scan's own last checks.
//!
//! The X-ray binaries' marks ([`XrbScanMarks`], P11.T8.b) are their kind, their place and motion
//! at the epoch and a low-mass transient's outburst recurrence. [`scan_marks`] and [`scan_cell`]
//! take either class's process ([`ScanProcess`]). From 27 the scan refuses a candidate fed under
//! the floor of P11.T8.b's fix, as the class's test does. The class's test refuses an X-ray binary
//! candidate the scan keeps only if none of its Be/X donor's first
//! [`BE_DONOR_ATTEMPTS`](crate::galaxy::catalogue_classes::binary::BE_DONOR_ATTEMPTS) draws make
//! it plan 06's Be star, a chance under 10⁻⁴⁹ (the class's documentation).
//!
//! A catalogue candidate and a feature's list entry (P11.T8.f) draw the same marks after their site,
//! each keyed by its own system ([`BinaryHost`]): [`AwdScanMarks::id`] and the other classes' are a
//! catalogue entry's member 0 or a list entry's member.
//!
//! # The recurrence
//!
//! The recurrence period `P_rec` is the nova ignition mass over the transfer rate (P11.T5's
//! `nova_ignition_mass`, a fit to Yaron et al.'s 2005 grid). The sampler draws `P_rec` first, on
//! its class's side of L (the class's sampler's ranks below or above L's), then the white dwarf's
//! mass given it, and the rate is the fit inverted at that mass (`transfer_rate_for`); `P_rec` is
//! then recomputed from the mass and the rate by the very function `carved_class` reads, so that
//! the scan and the class's test split the hosts on the same number. From 27 (P11.T5's fix) a
//! white dwarf fed helium has no nova, and the test puts it on the slow side whatever its rate:
//! the fast side's scan reads its donor's kind, its binary's first draw, and refuses it.
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
//! `engine_calls` (test builds and the `testing` feature) counts this thread's runs of the
//! binary engine, so that a test can show that a scan of 10⁶ hosts runs none.

#[cfg(any(test, feature = "testing"))]
use std::cell::Cell;

mod merger;
mod nsm;

pub(crate) use merger::donor_product;
pub(crate) use merger::merger_marks_at;
pub use merger::{MergerScanMarks, horizon_time, merger_scan_marks, scan_merger_cell};
pub(crate) use nsm::nsm_marks_at;
#[cfg(test)]
pub(crate) use nsm::remnant_mass_under;
pub use nsm::{NsbhOutflow, NsmScanMarks, nsm_scan_marks, scan_nsm_cell};

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::events::{
    CycleEvent, EventSeries, LinearClock, MonotonePhase, PhaseClock, TimeWindow, octaves_to_span,
};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{
    AwdProcess, AwdSide, BinaryHost, XrayBinaries, XrbSide, compact_radius, symbiotic_giant_state,
};
use crate::galaxy::fields::ComponentId;
use crate::id::{BodyId, CatalogueSystemId, SystemId, event_tags};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::{Seed, Stream, tags};
use crate::stellar::draws::isotropic;
use crate::tables::binary::{AwdMark, XrbHostKind, XrbMark, XrbPick};
use crate::time::{SourceHorizon, Span, UniverseTime};
use crate::units::consts::{
    GM_SUN, SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR, SOLAR_MASS_KG, SOLAR_RADIUS_M, SPEED_OF_LIGHT,
};
use crate::units::{
    GravitationalParameter, Metres, PerCubicLightYear, Radians, Seconds, SolarMasses,
    SolarMassesPerYear, SolarRadii, Watts, Years,
};

use super::classify::{
    BinaryClass, SYMBIOTIC_XRB_MIN_LUMINOSITY, XrbKind, nova_recurrence_period, recurrence_class,
    wind_fed_luminosity, xrb_line,
};
use super::light_curves::{XrayAccretor, eddington_luminosity};
use super::params::BinaryParams;
use super::sampler::ClassSampler;
use super::xrb_class::{XRB_MIN_TRANSFER_RATE, helium_disc_line};

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
/// (Ṁ^−1.29), so a held rate lengthens it; at the scratch sampler's recurrences of version 26,
/// 28,000 years and more, the rate stays under 10⁻⁸ and the hold binds nowhere (a test holds it).
/// Were it to bind, as it may on plan 15's fitted fast sampler's recurrent novae from version 27
/// ([`AWD_FAST_SAMPLER`](crate::tables::binary::AWD_FAST_SAMPLER)), a fast host could recur beyond
/// L, which the scan and the class's test would both refuse.
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
    host: BinaryHost,
    side: AwdSide,
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
    /// The host's ID: member 0 of its catalogue entry, or its feature's list entry (P11.T8.f).
    #[must_use]
    pub const fn id(&self) -> SystemId {
        self.host.system()
    }

    /// Where the host sits and what it shares with its surroundings.
    #[must_use]
    pub const fn host(&self) -> &BinaryHost {
        &self.host
    }

    /// Which of the two classes the host is in.
    #[must_use]
    pub const fn side(&self) -> AwdSide {
        self.side
    }

    /// The density component the host was drawn from, for a catalogue entry.
    #[must_use]
    pub const fn component(&self) -> Option<ComponentId> {
        self.host.component()
    }

    /// Where the host is at the epoch: its site in its cell, or its list entry's position.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        self.host.position()
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
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(*self.position(), self.velocity)
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
///     assert!(!host.kind().is_transient() || host.recurrence().is_some());
///     if host.recoil().is_none() {
///         assert_eq!(host.position(), host.site());
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbScanMarks {
    host: BinaryHost,
    kind: XrbHostKind,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    recoil: Option<XrbRecoil>,
    recurrence: Option<Years>,
    accretor_mass: SolarMasses,
    shape: XrbShape,
    transfer_rate: Option<SolarMassesPerYear>,
}

impl XrbScanMarks {
    /// The host's ID: member 0 of its catalogue entry, or its feature's list entry (P11.T8.f).
    #[must_use]
    pub const fn id(&self) -> SystemId {
        self.host.system()
    }

    /// Where the host sits and what it shares with its surroundings.
    #[must_use]
    pub const fn host(&self) -> &BinaryHost {
        &self.host
    }

    /// The host's kind.
    #[must_use]
    pub const fn kind(&self) -> XrbHostKind {
        self.kind
    }

    /// The density component the host was drawn from, for a catalogue entry: the young disc's for
    /// a high-mass host.
    #[must_use]
    pub const fn component(&self) -> Option<ComponentId> {
        self.host.component()
    }

    /// Where the host was placed: in its cell, for a catalogue high-mass host where its supernova
    /// was, relative to its component's field, and for a low-mass one its position at the epoch;
    /// a list entry's position (P11.T8.f).
    #[must_use]
    pub const fn site(&self) -> &GalacticPosition {
        self.host.position()
    }

    /// Where the host is at the epoch: its site, moved by a catalogue high-mass host's recoil over
    /// the years since its supernova.
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

    /// A catalogue high-mass host's recoil from its supernova; `None` for a low-mass host and a
    /// list entry, which moves with its feature (P11.T8.f).
    #[must_use]
    pub const fn recoil(&self) -> Option<&XrbRecoil> {
        self.recoil.as_ref()
    }

    /// A low-mass transient's outburst recurrence, Julian years; `None` for every other host.
    #[must_use]
    pub const fn recurrence(&self) -> Option<Years> {
        self.recurrence
    }

    /// Whether the host is a low-mass transient, whose outbursts recur: a host of a transient
    /// kind, or an ultracompact one fed below the irradiated disc's instability line.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        self.recurrence.is_some()
    }

    /// The binary class `classify` gives the pair at the epoch: its kind's, an ultracompact
    /// host's persistent or transient by its rate.
    #[must_use]
    pub const fn class(&self) -> BinaryClass {
        if matches!(self.kind, XrbHostKind::Ultracompact) && self.is_transient() {
            BinaryClass::LowMassXrayBinary(XrbKind::Transient)
        } else {
            self.kind.class()
        }
    }

    /// The compact star's mass, M☉: a neutron star's 1.2–2.2, a black hole's 5–15, and a
    /// black-hole high-mass host's 15–30.
    #[must_use]
    pub const fn accretor_mass(&self) -> SolarMasses {
        self.accretor_mass
    }

    /// The orbital period.
    #[must_use]
    pub fn period(&self) -> Seconds {
        self.shape.period()
    }

    /// The mean rate of transfer onto the compact star, M☉ yr⁻¹, held to its Eddington rate: a
    /// low-mass host's through its disc, on its kind's side of the irradiated disc's instability
    /// line by at least [`RATE_EXCESS_MARGIN_DEX`], an ultracompact host's driven by gravitational
    /// radiation, or a supergiant or black-hole high-mass host's; `None` for a Be/X or symbiotic
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
    /// A supergiant host or a black-hole high-mass one: its period, days, its donor's mass and its
    /// transfer rate.
    Supergiant {
        period_days: f64,
        donor_mass: SolarMasses,
        rate: SolarMassesPerYear,
    },
    /// An ultracompact host: its period, minutes, and the mass of the helium white dwarf filling
    /// its Roche lobe at that period ([`ultracompact_donor_mass`]).
    Ultracompact {
        period_minutes: f64,
        donor_mass: SolarMasses,
    },
    /// A symbiotic host, detached: its period, days, its giant's mass, and its giant's radius over
    /// its Roche lobe's.
    Symbiotic {
        period_days: f64,
        donor_mass: SolarMasses,
        filling: f64,
    },
}

impl XrbShape {
    /// The marks of a host of `kind` beside a compact star of `accretor_mass` from `stream`, three
    /// words, or one for an ultracompact host, the low-mass rate's ranks split at `rate_split`, the
    /// line's.
    #[must_use]
    fn draw(
        sampler: ClassSampler<XrbMark, XrbPick>,
        (kind, accretor_mass): (XrbHostKind, SolarMasses),
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
            XrbHostKind::Supergiant | XrbHostKind::BlackHoleHighMass => {
                let (period, donor, rate) = if kind == XrbHostKind::Supergiant {
                    (
                        XrbMark::SupergiantPeriod,
                        XrbMark::SupergiantDonorMass,
                        XrbMark::SupergiantRate,
                    )
                } else {
                    (
                        XrbMark::BlackHoleHighMassPeriod,
                        XrbMark::BlackHoleHighMassDonorMass,
                        XrbMark::BlackHoleHighMassRate,
                    )
                };
                Self::Supergiant {
                    period_days: math::exp10(sampler.draw(period, 0.0, stream)),
                    donor_mass: SolarMasses::new(sampler.draw(donor, 0.0, stream)),
                    rate: SolarMassesPerYear::new(math::exp10(sampler.draw(rate, 0.0, stream))),
                }
            }
            XrbHostKind::Ultracompact => {
                let period_minutes =
                    math::exp10(sampler.draw(XrbMark::UltracompactPeriod, 0.0, stream));
                Self::Ultracompact {
                    period_minutes,
                    donor_mass: ultracompact_donor_mass(
                        Seconds::new(period_minutes * 60.0),
                        accretor_mass,
                    ),
                }
            }
            XrbHostKind::Symbiotic => Self::Symbiotic {
                period_days: math::exp10(sampler.draw(XrbMark::SymbioticPeriod, 0.0, stream)),
                donor_mass: SolarMasses::new(math::exp10(sampler.draw(
                    XrbMark::SymbioticDonorMass,
                    0.0,
                    stream,
                ))),
                filling: sampler.draw(XrbMark::SymbioticFilling, 0.0, stream),
            },
        }
    }

    /// The orbital period.
    #[must_use]
    fn period(&self) -> Seconds {
        match *self {
            Self::LowMass { period_hours, .. } => Seconds::new(period_hours * 3_600.0),
            Self::Ultracompact { period_minutes, .. } => Seconds::new(period_minutes * 60.0),
            Self::BeX { period_days, .. }
            | Self::Supergiant { period_days, .. }
            | Self::Symbiotic { period_days, .. } => Seconds::new(period_days * SECONDS_PER_DAY),
        }
    }

    /// The rate onto a compact star `accretor` of `accretor_mass` of a host of this shape, M☉
    /// yr⁻¹, held to its Eddington rate ([`eddington_rate`]): a low-mass host's line ([`xrb_line`]
    /// at the circular orbit's separation) times its excess, a persistent host's never below the
    /// line, so that a line above the Eddington rate leaves it just above the line; an
    /// ultracompact host's driven by gravitational radiation ([`gravitational_wave_rate`]); a
    /// supergiant host's own; `None` for a Be/X or symbiotic host.
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
            Self::Ultracompact { donor_mass, .. } => {
                let separation = circular_separation(self.period(), accretor_mass + donor_mass);
                let rate = gravitational_wave_rate(accretor_mass, donor_mass, separation);
                Some(SolarMassesPerYear::new(rate.value().min(eddington)))
            }
            Self::BeX { .. } | Self::Symbiotic { .. } => None,
        }
    }

    /// Whether a host of this shape, fed at `rate`, is fed below its disc's instability line as
    /// `classify` splits a low-mass X-ray binary on it: an ultracompact host's transience, by its
    /// helium disc's line ([`helium_disc_line`], P11.T5's fix). `false` for every other shape,
    /// whose kind says it.
    #[must_use]
    pub(crate) fn below_the_line(&self, rate: Option<SolarMassesPerYear>) -> bool {
        match (*self, rate) {
            (Self::Ultracompact { .. }, Some(rate)) => rate < helium_disc_line(self.period()),
            (
                Self::LowMass { .. }
                | Self::BeX { .. }
                | Self::Supergiant { .. }
                | Self::Ultracompact { .. }
                | Self::Symbiotic { .. },
                _,
            ) => false,
        }
    }

    /// The luminosity at which a neutron star of `accretor_mass` takes a symbiotic host's giant's
    /// wind, as `classify` reads it (its accretor's radius, [`compact_radius`], the giant
    /// [`symbiotic_giant_state`]'s and the engine's default parameters, a timeline's from its
    /// marks); `None` for every other shape.
    #[must_use]
    fn wind_fed_luminosity(&self, accretor_mass: SolarMasses) -> Option<Watts> {
        let Self::Symbiotic {
            donor_mass,
            filling,
            ..
        } = *self
        else {
            return None;
        };
        let separation = circular_separation(self.period(), accretor_mass + donor_mass);
        let lobe = SolarRadii::new(
            roche_lobe_radius(donor_mass.value() / accretor_mass.value(), separation).value()
                / SOLAR_RADIUS_M,
        );
        let giant = symbiotic_giant_state(donor_mass, (lobe, filling), Years::ZERO);
        let radius = compact_radius(XrayAccretor::NeutronStar, accretor_mass);
        Some(wind_fed_luminosity(
            (accretor_mass, radius),
            &giant,
            separation,
            &BinaryParams::default(),
        ))
    }
}

/// Eggleton's cold white dwarf's Chandrasekhar mass: 1.44 M☉.
const EGGLETON_CHANDRASEKHAR_MASS: SolarMasses = SolarMasses::new(1.44);

/// Eggleton's cold white dwarf's mass where its relation turns to a planet's: 0.000 57 M☉.
const EGGLETON_PLANET_MASS: SolarMasses = SolarMasses::new(0.000_57);

/// The radius of a cold white dwarf of `mass`: Eggleton's mass–radius relation as Verbunt and
/// Rappaport (1988, ApJ 332, 193) give it,
/// `R = 0.0114 [(M/Mch)^(-2/3) - (M/Mch)^(2/3)]^(1/2) [1 + 3.5 (M/Mp)^(-2/3) + Mp/M]^(-2/3)` R☉,
/// with `Mch` [`EGGLETON_CHANDRASEKHAR_MASS`] and `Mp` [`EGGLETON_PLANET_MASS`] (recalled, not
/// read at source).
#[must_use]
fn cold_white_dwarf_radius(mass: SolarMasses) -> SolarRadii {
    let (m, planet_mass) = (mass.value(), EGGLETON_PLANET_MASS.value());
    let x = m / EGGLETON_CHANDRASEKHAR_MASS.value();
    let degenerate = math::powf_positive(x, -2.0 / 3.0) - math::powf_positive(x, 2.0 / 3.0);
    let planet = 1.0 + 3.5 * math::powf_positive(m / planet_mass, -2.0 / 3.0) + planet_mass / m;
    SolarRadii::new(0.0114 * degenerate.max(0.0).sqrt() * math::powf_positive(planet, -2.0 / 3.0))
}

/// The least and the greatest mass an ultracompact host's helium white dwarf is solved on.
const ULTRACOMPACT_DONOR_MASSES: (SolarMasses, SolarMasses) =
    (SolarMasses::new(0.001), SolarMasses::new(0.45));

/// The mass of a helium white dwarf filling its Roche lobe (Eggleton 1983) on a circular orbit of
/// `period` about a compact star of `accretor_mass`: where [`cold_white_dwarf_radius`] meets the
/// lobe, found by 60 bisections on [`ULTRACOMPACT_DONOR_MASSES`], from the lightest, which
/// overfills its lobe at every period of 10–80 minutes, to the heaviest, which fits inside it;
/// held at the ends. About 0.057 M☉, a cold dwarf's, at 4U 1820−30's 11.4 minutes beside a 1.4 M☉
/// neutron star, and 0.0047 M☉ at 80 minutes.
#[must_use]
pub(crate) fn ultracompact_donor_mass(period: Seconds, accretor_mass: SolarMasses) -> SolarMasses {
    let overfills = |mass: f64| {
        let donor = SolarMasses::new(mass);
        let separation = circular_separation(period, accretor_mass + donor);
        let lobe = roche_lobe_radius(mass / accretor_mass.value(), separation).value();
        cold_white_dwarf_radius(donor).value() * SOLAR_RADIUS_M > lobe
    };
    let (mut lo, mut hi) = (
        ULTRACOMPACT_DONOR_MASSES.0.value(),
        ULTRACOMPACT_DONOR_MASSES.1.value(),
    );
    if !overfills(lo) {
        return SolarMasses::new(lo);
    }
    if overfills(hi) {
        return SolarMasses::new(hi);
    }
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if overfills(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    SolarMasses::new(f64::midpoint(lo, hi))
}

/// The mass–radius exponent ζ = d ln R ÷ d ln M of a cold white dwarf of `mass`: the slope of
/// [`cold_white_dwarf_radius`],
/// `ζ = -(1/3)(u + v)/(u - v) + (2/3)[(7/3)(M/Mp)^(-2/3) + Mp/M]/B`, with `u = (M/Mch)^(-2/3)`,
/// `v = (M/Mch)^(2/3)` and `B` the relation's second bracket, as van Haaften et al. (2012, A&A
/// 537, A104) take an ultracompact donor's: −0.275 at 0.057 M☉ and −0.21 at 0.02 M☉, against a
/// degenerate gas's −1/3 (research's ruling of 2026-11-02). Valid below the Chandrasekhar mass,
/// where the relation's first bracket vanishes; an ultracompact donor is under 0.45 M☉.
#[must_use]
fn cold_white_dwarf_zeta(mass: SolarMasses) -> f64 {
    let (m, planet_mass) = (mass.value(), EGGLETON_PLANET_MASS.value());
    let x = m / EGGLETON_CHANDRASEKHAR_MASS.value();
    let (lower, upper) = (
        math::powf_positive(x, -2.0 / 3.0),
        math::powf_positive(x, 2.0 / 3.0),
    );
    let planet_term = math::powf_positive(m / planet_mass, -2.0 / 3.0);
    let planet = 1.0 + 3.5 * planet_term + planet_mass / m;
    -(lower + upper) / (3.0 * (lower - upper))
        + 2.0 / 3.0 * (7.0 / 3.0 * planet_term + planet_mass / m) / planet
}

/// The rate at which gravitational radiation drives a cold white dwarf of `donor` onto an
/// accretor of `accretor` on a circular orbit of semi-major axis `separation`, M☉ yr⁻¹, the
/// transfer conservative: −Ṁ₂ ÷ M₂ = (−J̇ ÷ J) ÷ (5/6 + ζ ÷ 2 − q), q = M₂ ÷ M₁ and ζ the dwarf's
/// own ([`cold_white_dwarf_zeta`]), with J̇ ÷ J = −(32/5) G³ M₁ M₂ (M₁ + M₂) ÷ (c⁵ a⁴) (Paczyński
/// 1967, Acta Astron. 17, 287; Rappaport, Joss and Webbink 1982, ApJ 254, 616; recalled, not
/// read at source): about 7 × 10⁻⁹ M☉ yr⁻¹ at 11 minutes beside a 1.4 M☉ neutron star, and
/// 10⁻¹² M☉ yr⁻¹ near an hour. Valid for q under 5/6 + ζ ÷ 2, where the transfer is stable: 0.70 at
/// 0.057 M☉ and 0.63 at 0.3 M☉, where an ultracompact host's q is under 0.4.
#[must_use]
pub(crate) fn gravitational_wave_rate(
    accretor: SolarMasses,
    donor: SolarMasses,
    separation: Metres,
) -> SolarMassesPerYear {
    let (m1, m2) = (accretor.value(), donor.value());
    let a = separation.value();
    let c2 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    let a2 = a * a;
    let loss = 32.0 / 5.0 * GM_SUN * GM_SUN * GM_SUN * m1 * m2 * (m1 + m2)
        / (c2 * c2 * SPEED_OF_LIGHT * a2 * a2);
    let response = 5.0 / 6.0 + cold_white_dwarf_zeta(donor) / 2.0 - m2 / m1;
    debug_assert!(
        response > 0.0,
        "a stable transfer: q = {} under 5/6 + zeta/2",
        m2 / m1
    );
    SolarMassesPerYear::new(m2 * loss / response * SECONDS_PER_JULIAN_YEAR)
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
    xrb_marks_at(
        galaxy,
        process,
        BinaryHost::field(id, site, component),
        host_side,
    )
}

/// The scan marks of an X-ray binary on `host_side` at `host`, after its site: its kind and a
/// transient's recurrence, its compact star's mass and its binary's shape, a catalogue high-mass
/// host's recoil, and its place and velocity at the epoch. A catalogue candidate's
/// ([`scan_marks`]) and a feature's list entry's (P11.T8.f), on the same draws keyed by each one's
/// system; a list entry takes no recoil and moves with its feature. `None` from 27 for a pair fed
/// under the floor of P11.T8.b's fix, and for a symbiotic host whose neutron star takes its
/// giant's wind under [`SYMBIOTIC_XRB_MIN_LUMINOSITY`], which the class's test refuses.
#[must_use]
pub(crate) fn xrb_marks_at(
    galaxy: &Galaxy,
    process: &XrayBinaries,
    host: BinaryHost,
    host_side: XrbSide,
) -> Option<XrbScanMarks> {
    let seed = galaxy.seed();
    let key = host.key();
    let sampler = process.sampler();
    // The held tables weigh each side's first kinds alone (`XrbHostKind::LOW_MASS_HELD`), so that
    // they never pick the carving's three.
    let mut kind_stream = Stream::open(seed, tags::CLASS_XRB_KIND, key);
    let kind = match host_side {
        XrbSide::LowMass => {
            XrbHostKind::LOW_MASS[sampler.pick(XrbPick::LowMassKind, &mut kind_stream)]
        }
        XrbSide::HighMass => {
            XrbHostKind::HIGH_MASS[sampler.pick(XrbPick::HighMassKind, &mut kind_stream)]
        }
    };
    let recurrence_mark = match kind {
        XrbHostKind::NeutronStarTransient => Some(XrbMark::NeutronStarRecurrence),
        XrbHostKind::BlackHoleTransient => Some(XrbMark::BlackHoleRecurrence),
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::BlackHolePersistent
        | XrbHostKind::BeX
        | XrbHostKind::Supergiant
        | XrbHostKind::Ultracompact
        | XrbHostKind::Symbiotic
        | XrbHostKind::BlackHoleHighMass => None,
    };
    let recurrence = recurrence_mark
        .map(|mark| Years::new(math::exp10(sampler.draw(mark, 0.0, &mut kind_stream))));
    let mut stream = Stream::open(seed, tags::CLASS_XRB_BINARY, key);
    let accretor_mark = accretor_mark_of(kind);
    let accretor_mass = SolarMasses::new(sampler.draw(accretor_mark, 0.0, &mut stream));
    let shape = XrbShape::draw(
        sampler,
        (kind, accretor_mass),
        process.rate_split(),
        &mut stream,
    );
    let transfer_rate = shape.transfer_rate(kind.accretor(), accretor_mass);
    // The class's test refuses a pair fed under the floor (`xrb_class`).
    if transfer_rate.is_some_and(|rate| rate < XRB_MIN_TRANSFER_RATE) {
        return None;
    }
    // In every version it refuses a symbiotic host whose neutron star takes its giant's wind
    // under the symbiotic X-ray binaries' floor.
    if shape
        .wind_fed_luminosity(accretor_mass)
        .is_some_and(|luminosity| luminosity < SYMBIOTIC_XRB_MIN_LUMINOSITY)
    {
        return None;
    }
    // An ultracompact host is a transient by its rate against the line the class's test reads, its
    // recurrence the kind's next word.
    let recurrence = recurrence.or_else(|| {
        shape.below_the_line(transfer_rate).then(|| {
            let mark = XrbMark::UltracompactRecurrence;
            Years::new(math::exp10(sampler.draw(mark, 0.0, &mut kind_stream)))
        })
    });
    let recoil_marks = recoil_marks_of(kind).filter(|_| host.listed_host().is_none());
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
    let site = *host.position();
    let drawn = host.velocity(galaxy, tags::CLASS_XRB_VELOCITY);
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
        host,
        kind,
        position,
        velocity,
        recoil,
        recurrence,
        accretor_mass,
        shape,
        transfer_rate,
    })
}

/// The mark an X-ray binary host of `kind` draws its compact star's mass by.
#[must_use]
const fn accretor_mark_of(kind: XrbHostKind) -> XrbMark {
    match kind {
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::NeutronStarTransient
        | XrbHostKind::BeX
        | XrbHostKind::Supergiant
        | XrbHostKind::Ultracompact
        | XrbHostKind::Symbiotic => XrbMark::NeutronStarMass,
        XrbHostKind::BlackHolePersistent | XrbHostKind::BlackHoleTransient => {
            XrbMark::BlackHoleMass
        }
        XrbHostKind::BlackHoleHighMass => XrbMark::BlackHoleHighMassAccretorMass,
    }
}

/// The marks of a high-mass host of `kind`'s recoil and its years since the supernova: a
/// black-hole high-mass host's the supergiants'; `None` for a low-mass kind.
#[must_use]
const fn recoil_marks_of(kind: XrbHostKind) -> Option<(XrbMark, XrbMark)> {
    match kind {
        XrbHostKind::BeX => Some((XrbMark::BexRecoil, XrbMark::BexSinceSupernova)),
        XrbHostKind::Supergiant | XrbHostKind::BlackHoleHighMass => {
            Some((XrbMark::SupergiantRecoil, XrbMark::SupergiantSinceSupernova))
        }
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::NeutronStarTransient
        | XrbHostKind::BlackHolePersistent
        | XrbHostKind::BlackHoleTransient
        | XrbHostKind::Ultracompact
        | XrbHostKind::Symbiotic => None,
    }
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
    awd_marks_at(galaxy, process, BinaryHost::field(id, position, component))
}

/// The scan marks of an accreting white dwarf of `process`'s class at `host`, after its site:
/// its recurrence on the class's side of L, the white dwarf's mass and the rate, its velocity, its
/// nova series and clock, and a slow host's eruptions in the source horizon; `None` if the
/// recurrence lands on the other side or, from 27, a fast host is fed helium. A catalogue
/// candidate's ([`scan_marks`]) and a feature's list entry's (P11.T8.f), on the same draws keyed by
/// each one's system.
#[must_use]
pub(crate) fn awd_marks_at(
    galaxy: &Galaxy,
    process: &AwdProcess,
    host: BinaryHost,
) -> Option<AwdScanMarks> {
    let seed = galaxy.seed();
    let system = host.system();
    let key = host.key();
    let sampler = process.sampler();
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
    // From 27 a white dwarf fed helium has no nova, so the class's test puts it on the slow side.
    if process.side() == AwdSide::Fast && process.has_no_nova(galaxy, key) {
        return None;
    }
    let clock = nova_clock(recurrence, stream.uniform());
    let velocity = host.velocity(galaxy, tags::CLASS_AWD_VELOCITY);
    let series = nova_series(seed, system);
    let phase = nova_phase(&clock);
    let (eruptions, eruption_count) = match process.side() {
        AwdSide::Slow => horizon_eruptions(&phase, &series, &clock),
        AwdSide::Fast => ([UniverseTime::EPOCH; 2], 0),
    };
    Some(AwdScanMarks {
        host,
        side: process.side(),
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
/// recurrence was at least 28,000 years (version 26's scratch sampler's least), and from version 27
/// some years (plan 15's fitted fast sampler's,
/// [`AWD_FAST_SAMPLER`](crate::tables::binary::AWD_FAST_SAMPLER)).
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
