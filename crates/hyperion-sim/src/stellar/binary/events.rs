//! The binary events in time (plan 11's Provides; P11.T9.b): novae, dwarf novae, X-ray transients'
//! outbursts, Type I X-ray bursts, Be/X-ray binaries' giant outbursts, and the mergers' luminous
//! red novae and kilonovae, each on plan 06's constructions (`crate::events`) and keyed on its
//! host's own event series, with P11.T9.a's light curves.
//!
//! A host's events are a pure function of the seed, its marks, its event tag and time: nothing is
//! stored between calls, and no event leaves a state that a later one reads, so the state after n
//! eruptions is a closed form in n (each event radiates the host's one curve). The constructions:
//!
//! - **Novae** (`0x0300 NOVA`, P11.T8.a): the monotone phase an accreting white dwarf's scan
//!   marks carry, on their own nova clock of period `P_rec` ([`AwdScanMarks::clock`],
//!   [`AwdScanMarks::phase`]); no second clock. The curve is the white dwarf's
//!   ([`NovaCurve`]).
//! - **Dwarf novae** (`0x0301 DWARF_NOVA`): a dwarf nova's disc fills between outbursts, so its
//!   cycle is the fill time at the line, [`DWARF_NOVA_CYCLE_AT_LINE_DAYS`], times the line over
//!   its rate (`cv_line`, P11.T5's disc-instability line), held to [`DWARF_NOVA_CYCLES_DAYS`];
//!   a monotone phase on that period with a skip mark ([`DWARF_NOVA_SKIP_SHARE`]). The curve
//!   gathers the rate over the cycle ([`DwarfNovaCurve`]). Only an accreting white dwarf entry
//!   that `classify` names a dwarf nova at the epoch has them: the scan marks alone do not hold
//!   the orbit.
//! - **X-ray transients** (`0x0302 XRAY_TRANSIENT`): a low-mass transient's outbursts, a monotone
//!   phase at its recurrence ([`XrbScanMarks::recurrence`]); the curve gathers its mean rate over
//!   the recurrence ([`XrayTransientCurve`]).
//! - **Type I X-ray bursts** (`0x0303 XRAY_BURST`): a persistent neutron star's, a monotone phase
//!   at the ignition recurrence, the column [`BURST_IGNITION_COLUMN_G_CM2`] over the star's surface
//!   over its rate; the fuel by the rate's share of the Eddington rate ([`HELIUM_BURST_SHARES`]);
//!   none at or above it, where burning is stable. A transient's bursts in outburst are not
//!   modelled, nor a persistent black hole's or a supergiant host's events.
//! - **Be/X giant outbursts** (`0x0304 BEX_GIANT_OUTBURST`): Poisson bins of a Julian year at
//!   [`BEX_GIANT_OUTBURST_RATE_PER_YEAR`] ([`BeXOutburstCurve`]).
//! - **Luminous red novae and kilonovae** (`0x0305`, `0x0306`): one-shot at a merger's time T
//!   ([`MergerScanMarks::merger_time`], [`NsmScanMarks::merger_time`]), bin 0 and number 0 of
//!   their tags on the system, as plan 09's `SUPERNOVA` is. A red nova's curve is of the pair's
//!   total mass ([`LuminousRedNovaCurve`]), a kilonova's of its ejecta
//!   ([`NsmScanMarks::ejecta_mass`], [`KilonovaCurve`]): GW170817's for two neutron stars, and for
//!   a black hole what its tides leave outside it and unbind, so that a hole that swallows its
//!   neutron star whole, as most of the class's do, lists no kilonova. The remnant sets nothing in
//!   the kilonova: T9.a's curve has no term for it, and every merger P11.T8.d draws leaves a black
//!   hole of the pair's mass.
//!
//! The monotone phases other than the novae take the novae's noise (`scan::nova_phase`: 0.08
//! cycles on a one-cycle lattice, with octaves to the source horizon's cycles), and their clocks'
//! origins from their own series: the first word of event stream j = [`CLOCK_ORIGIN_STREAM`] of
//! bin 0, as a share of one period before the epoch. A phase reads its bins' streams (its skip
//! marks) and event streams j = 0 (its events' marks) to 48 (its noise's 48 octaves at most), and
//! Poisson bins event streams below their count, so that no construction reads it, and no
//! generation stream is read past what it was: nothing generated moves.
//!
//! An event is active from its onset until its light stays under [`ACTIVE_SHARE`] of its peak.
//! [`events_in`] and [`active_at`] append to a vector, in time order with ties by ID, as plan 06's
//! listings do. Nothing in generation calls them yet (plan 12's alerts read them through P12.T5.c
//! and P12.T5.e).

use crate::events::{
    BinEvent, CycleEvent, EventBin, EventId, EventSeries, EventSubject, EventsPerSecond,
    LinearClock, MonotonePhase, PoissonBins, RateModel, SkipMark, TimeWindow,
};
use crate::galaxy::catalogue_classes::binary::AwdEntry;
use crate::galaxy::gas::ccm::Band;
use crate::id::{BodyId, EventTag, SystemId, event_tags};
use crate::rng::Seed;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::structure::neutron_star_radius;
use crate::tables::binary::XrbHostKind;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{
    SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR, SOLAR_MASS_KG, SOLAR_RADIUS_M,
};
use crate::units::{Days, SolarMasses, SolarMassesPerYear, Watts};

use super::classify::cv_line;
use super::light_curves::{
    BeXOutburstCurve, BinaryLightCurve, BurstFuel, DwarfNovaCurve, KilonovaCurve,
    LuminousRedNovaCurve, NovaCurve, XrayAccretor, XrayBurstCurve, XrayTransientCurve,
};
use super::scan::{
    AwdScanMarks, MergerScanMarks, NsmScanMarks, XrbScanMarks, eddington_rate, nova_phase,
};
use super::{BinaryClass, CvKind};

/// The share of its peak under which an event's light stays once it is over: 0.01, five
/// magnitudes, ours. An event is active from its onset until then.
pub const ACTIVE_SHARE: f64 = 0.01;

/// The event stream j of bin 0 whose first word places a monotone phase's clock origin built
/// here: 255, past every stream a construction reads (module documentation).
pub const CLOCK_ORIGIN_STREAM: u8 = 255;

/// A dwarf nova's cycle at its line, days: 30, ours. The disc fills to the instability's
/// surface density, `Σ_max` ≈ 13.4 α^−0.83 M₁^−0.38 R₁₀^1.14 g cm⁻² (Lasota 2001, New Astron. Rev.
/// 45, 449; recalled, not read at source), so that half the disc's mass at `Σ_max`, about 1.7 ×
/// 10⁻¹⁰ M☉ for an outer radius of 2 × 10¹⁰ cm about 0.8 M☉ at α = 0.04, refills at the line's
/// 10⁻⁹ M☉ yr⁻¹ in about a month, SS Cygni's interval (49 days).
pub const DWARF_NOVA_CYCLE_AT_LINE_DAYS: f64 = 30.0;

/// A dwarf nova's cycle's limits, days: 10, ER Ursae Majoris stars' normal outbursts, to 33
/// years, WZ Sagittae's interval (recalled, not read at source).
pub const DWARF_NOVA_CYCLES_DAYS: (f64, f64) = (10.0, 33.0 * 365.25);

/// The share of a dwarf nova's cycles that pass without an outburst: 0.2, ours, for the
/// irregular intervals of dwarf novae.
pub const DWARF_NOVA_SKIP_SHARE: f64 = 0.2;

/// The column at which accreted matter ignites in a Type I X-ray burst, g cm⁻²: 10⁸, the
/// ignition columns of Strohmayer and Bildsten (2006, in Compact Stellar X-ray Sources, chapter
/// 3; recalled, not read at source).
pub const BURST_IGNITION_COLUMN_G_CM2: f64 = 1.0e8;

/// The shares of the Eddington rate between which a neutron star burns accreted helium alone in
/// its bursts: 0.01–0.1, ours, after the regimes Strohmayer and Bildsten (2006) describe: below,
/// mixed bursts that unstable hydrogen ignites; between, the hydrogen burns stably and helium
/// bursts; above, mixed bursts again, and at or above the Eddington rate no burst.
pub const HELIUM_BURST_SHARES: (f64, f64) = (0.01, 0.1);

/// A Be/X-ray binary's giant (Type II) outbursts a year: 0.1, ours, one a decade, the less
/// frequent, irregular outbursts of Reig (2011, Ap&SS 332, 1; recalled, not read at source).
pub const BEX_GIANT_OUTBURST_RATE_PER_YEAR: f64 = 0.1;

/// The hydrogen mass fraction of the matter a low-mass or Be donor gives: 0.7, a hydrogen-rich
/// surface (ruling 132.1's X from the transferred matter).
const DONOR_HYDROGEN: f64 = 0.7;

/// The bin of a Be/X host's giant outbursts, s: one Julian year.
const BEX_BIN_SECONDS: i64 = 31_557_600;

/// One kind of binary event (plan 11's Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BinaryEventKind {
    /// A classical nova.
    Nova,
    /// A dwarf nova's outburst.
    DwarfNova,
    /// A low-mass X-ray transient's outburst.
    XrayTransient,
    /// A Type I X-ray burst.
    XrayBurst,
    /// A Be/X-ray binary's giant (Type II) outburst.
    BeXGiantOutburst,
    /// A stellar merger's luminous red nova.
    LuminousRedNova,
    /// A neutron-star merger's kilonova.
    Kilonova,
}

impl BinaryEventKind {
    /// Every kind, in its tag's order.
    pub const ALL: [Self; 7] = [
        Self::Nova,
        Self::DwarfNova,
        Self::XrayTransient,
        Self::XrayBurst,
        Self::BeXGiantOutburst,
        Self::LuminousRedNova,
        Self::Kilonova,
    ];

    /// The kind's event tag, 0x0300–0x0306.
    #[must_use]
    pub const fn tag(self) -> EventTag {
        match self {
            Self::Nova => event_tags::NOVA,
            Self::DwarfNova => event_tags::DWARF_NOVA,
            Self::XrayTransient => event_tags::XRAY_TRANSIENT,
            Self::XrayBurst => event_tags::XRAY_BURST,
            Self::BeXGiantOutburst => event_tags::BEX_GIANT_OUTBURST,
            Self::LuminousRedNova => event_tags::LUMINOUS_RED_NOVA,
            Self::Kilonova => event_tags::KILONOVA,
        }
    }

    /// The kind of `tag`, or `None` for a tag of no binary kind.
    #[must_use]
    pub fn of_tag(tag: EventTag) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.tag() == tag)
    }
}

/// One binary event (plan 11's Provides): its kind, its ID, its onset and the curve of its light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryEvent {
    kind: BinaryEventKind,
    id: EventId,
    onset: UniverseTime,
    curve: BinaryLightCurve,
}

impl BinaryEvent {
    /// The kind.
    #[must_use]
    pub const fn kind(&self) -> BinaryEventKind {
        self.kind
    }

    /// The event's ID: its subject, its tag, its bin or cycle and its number there.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset: where its light begins to rise.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The curve of its light (P11.T9.a).
    #[must_use]
    pub const fn curve(&self) -> &BinaryLightCurve {
        &self.curve
    }

    /// How long it is active: from its onset until its light stays under [`ACTIVE_SHARE`] of its
    /// peak.
    ///
    /// # Panics
    ///
    /// Never: every curve here fades below a share of its peak within the clock's range.
    #[must_use]
    pub fn duration(&self) -> Span {
        active_duration(&self.curve)
    }

    /// Whether it is active at `t`: at or after its onset and before its light has faded.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        active_within(self.onset, self.duration(), t)
    }
}

/// How long an event of `curve` is active ([`BinaryEvent::duration`]), which plan 12's alerts read
/// as how long it gives light.
#[must_use]
pub(crate) fn active_duration(curve: &BinaryLightCurve) -> Span {
    curve
        .fades_below(ACTIVE_SHARE)
        .expect("a binary event's light fades below a hundredth of its peak within the clock")
}

/// Whether an event that began at `onset` and lasts `duration` is active at `t`.
#[must_use]
fn active_within(onset: UniverseTime, duration: Span, t: UniverseTime) -> bool {
    onset <= t && t.checked_since(onset).is_some_and(|dt| dt < duration)
}

/// A host of binary events (plan 11's Provides): what [`events_in`] and [`active_at`] list
/// events of, with the universe's seed that keys their series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinarySystem<'a> {
    seed: Seed,
    host: Host<'a>,
}

/// What a [`BinarySystem`] is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Host<'a> {
    AccretingWdScan(&'a AwdScanMarks),
    AccretingWd(&'a AwdEntry),
    XrayBinary(&'a XrbScanMarks),
    StellarMerger(&'a MergerScanMarks),
    NeutronStarMerger(&'a NsmScanMarks),
}

impl<'a> BinarySystem<'a> {
    /// An accreting white dwarf host of the universe of `seed` by its scan marks alone: its novae.
    #[must_use]
    pub const fn accreting_white_dwarf_scan(seed: Seed, scan: &'a AwdScanMarks) -> Self {
        Self {
            seed,
            host: Host::AccretingWdScan(scan),
        }
    }

    /// An accreting white dwarf entry of the universe of `seed`: its novae and, for a dwarf nova,
    /// its outbursts.
    #[must_use]
    pub const fn accreting_white_dwarf(seed: Seed, entry: &'a AwdEntry) -> Self {
        Self {
            seed,
            host: Host::AccretingWd(entry),
        }
    }

    /// An X-ray binary of the universe of `seed` by its scan marks: a low-mass transient's
    /// outbursts, a persistent neutron star's bursts or a Be/X host's giant outbursts.
    #[must_use]
    pub const fn xray_binary(seed: Seed, scan: &'a XrbScanMarks) -> Self {
        Self {
            seed,
            host: Host::XrayBinary(scan),
        }
    }

    /// A stellar merger of the universe of `seed` by its scan marks (P11.T8.c): its luminous red
    /// nova at its merger.
    #[must_use]
    pub const fn stellar_merger(seed: Seed, scan: &'a MergerScanMarks) -> Self {
        Self {
            seed,
            host: Host::StellarMerger(scan),
        }
    }

    /// A neutron-star merger of the universe of `seed` by its scan marks (P11.T8.d): its kilonova
    /// at its merger, or none where it ejects nothing ([`NsmScanMarks::ejecta_mass`]).
    #[must_use]
    pub const fn neutron_star_merger(seed: Seed, scan: &'a NsmScanMarks) -> Self {
        Self {
            seed,
            host: Host::NeutronStarMerger(scan),
        }
    }

    /// The host's sources of events, one a kind it has: at most two, an accreting white dwarf's
    /// novae and dwarf novae.
    #[must_use]
    fn sources(&self) -> [Option<Source>; 2] {
        match self.host {
            Host::AccretingWdScan(scan) => [Some(nova_source(scan)), None],
            Host::AccretingWd(entry) => [
                Some(nova_source(entry.scan())),
                dwarf_nova_source(self.seed, entry),
            ],
            Host::XrayBinary(scan) => [xray_source(self.seed, scan), None],
            Host::StellarMerger(scan) => [Some(red_nova_source(self.seed, scan)), None],
            Host::NeutronStarMerger(scan) => [kilonova_source(self.seed, scan), None],
        }
    }
}

/// One kind of a host's events: its series, its construction and the curve of each event.
#[derive(Debug, Clone, Copy)]
struct Source {
    kind: BinaryEventKind,
    series: EventSeries,
    construction: Construction,
    curve: BinaryLightCurve,
}

/// How a source's events fall in time.
#[derive(Debug, Clone, Copy)]
enum Construction {
    /// One a cycle of a monotone phase on a linear clock.
    Phase {
        clock: LinearClock,
        phase: MonotonePhase,
    },
    /// Poisson bins at a constant rate.
    Bins { bins: PoissonBins, rate: Constant },
    /// One event at `at`, bin 0 and number 0 of the series: a merger's.
    Once { at: UniverseTime },
}

/// A constant rate, its own bound.
#[derive(Debug, Clone, Copy)]
struct Constant(EventsPerSecond);

impl RateModel for Constant {
    fn bound(&self, _: TimeWindow) -> EventsPerSecond {
        self.0
    }

    fn rate(&self, _: UniverseTime) -> EventsPerSecond {
        self.0
    }
}

/// The construction's listings that one call reuses.
#[derive(Debug, Default)]
struct Scratch {
    cycles: Vec<CycleEvent>,
    bins: Vec<BinEvent>,
}

impl Source {
    /// Appends the source's events in `window` to `out`, in time order, listing through
    /// `scratch`.
    fn events_in(&self, window: TimeWindow, scratch: &mut Scratch, out: &mut Vec<BinaryEvent>) {
        let event = |id: EventId, onset: UniverseTime| BinaryEvent {
            kind: self.kind,
            id,
            onset,
            curve: self.curve,
        };
        match self.construction {
            Construction::Phase { clock, phase } => {
                scratch.cycles.clear();
                phase.events_in(&self.series, &clock, window, &mut scratch.cycles);
                out.extend(scratch.cycles.iter().map(|e| event(e.id(), e.time())));
            }
            Construction::Bins { bins, rate } => {
                scratch.bins.clear();
                bins.events_in(&self.series, window, &rate, &mut scratch.bins);
                out.extend(scratch.bins.iter().map(|e| event(e.id(), e.time())));
            }
            Construction::Once { at } => {
                if window.contains(at) {
                    let first = EventBin::new(0).expect("bin 0 is a bin");
                    out.push(event(self.series.event_id(first, 0), at));
                }
            }
        }
    }
}

/// The nova source of an accreting white dwarf's scan marks: its own clock and phase.
///
/// # Panics
///
/// Never: a host's white dwarf is of 0.55–1.36 M☉, a nova's range.
#[must_use]
fn nova_source(scan: &AwdScanMarks) -> Source {
    Source {
        kind: BinaryEventKind::Nova,
        series: *scan.series(),
        construction: Construction::Phase {
            clock: *scan.clock(),
            phase: *scan.phase(),
        },
        curve: NovaCurve::of(scan.white_dwarf_mass())
            .expect("a host's white dwarf is of 0.55–1.36 M☉")
            .into(),
    }
}

/// The clock of period `period` whose origin is a share of one period before the epoch, the
/// share the first word of `series`' event stream [`CLOCK_ORIGIN_STREAM`] of bin 0.
///
/// # Panics
///
/// If the period is under [`LinearClock::MIN_PERIOD`] or past the clock's range: a burst under
/// the Eddington rate recurs every 20 minutes or more, a dwarf nova every 10 days or more and a
/// transient every 56 years or more, all under the 18,000 years of the slowest transient.
#[must_use]
fn series_clock(series: &EventSeries, period: Span) -> LinearClock {
    let bin = EventBin::new(0).expect("bin 0 is a bin");
    let share = series
        .key()
        .event_stream(bin, CLOCK_ORIGIN_STREAM)
        .uniform();
    let back = Span::from_seconds_f64(share * period.as_seconds_f64())
        .expect("a share of a period is a span");
    let origin = UniverseTime::EPOCH
        .checked_sub(back)
        .expect("a period before the epoch is on the clock");
    LinearClock::new(period, origin)
        .expect("a period of 20 minutes or more exceeds the clock's least, 16 s")
}

/// The series of `tag` on body 0 of `system` in the universe of `seed`: the compact star's or
/// the white dwarf's.
#[must_use]
fn body_series(seed: Seed, tag: EventTag, system: SystemId) -> EventSeries {
    EventSeries::new(seed, tag, BodyId::new(system, 0).into())
}

/// A monotone phase's source of `kind` on `series` at `period`: its clock from the series, the
/// novae's noise, and `skip` if any.
#[must_use]
fn phase_source(
    kind: BinaryEventKind,
    series: EventSeries,
    period: Span,
    skip: Option<SkipMark>,
    curve: BinaryLightCurve,
) -> Source {
    let clock = series_clock(&series, period);
    let phase = nova_phase(&clock);
    Source {
        kind,
        series,
        construction: Construction::Phase {
            clock,
            phase: skip.map_or(phase, |skip| phase.with_skip(skip)),
        },
        curve,
    }
}

/// The dwarf nova source of an accreting white dwarf entry that `classify` names a dwarf nova at
/// the epoch, or `None` for any other.
///
/// # Panics
///
/// Never: the white dwarf, its period and the mass it gathers are a dwarf nova's.
#[must_use]
fn dwarf_nova_source(seed: Seed, entry: &AwdEntry) -> Option<Source> {
    let age = entry.record().age_at_epoch();
    if entry.timeline().class_at(age) != BinaryClass::CataclysmicVariable(CvKind::DwarfNova) {
        return None;
    }
    let binary = entry.binary();
    let white_dwarf = binary.white_dwarf().mass();
    let rate = binary.transfer_rate().value();
    let line = cv_line(
        white_dwarf,
        binary.donor().mass(),
        binary.orbit().semi_major_axis(),
    )
    .value();
    let (shortest, longest) = DWARF_NOVA_CYCLES_DAYS;
    let cycle_days = (DWARF_NOVA_CYCLE_AT_LINE_DAYS * line / rate).clamp(shortest, longest);
    let cycle = Span::from_seconds_f64(cycle_days * SECONDS_PER_DAY)
        .expect("a cycle of days to decades is a span");
    let gathered = SolarMasses::new(rate * cycle_days * SECONDS_PER_DAY / SECONDS_PER_JULIAN_YEAR);
    let period = Days::new(binary.orbit().period().value() / SECONDS_PER_DAY);
    let curve = DwarfNovaCurve::of(white_dwarf, period, gathered)
        .expect("a dwarf nova's white dwarf, orbit and gathered mass are in range");
    Some(phase_source(
        BinaryEventKind::DwarfNova,
        body_series(seed, event_tags::DWARF_NOVA, entry.record().id()),
        cycle,
        Some(SkipMark::from_probability(DWARF_NOVA_SKIP_SHARE)),
        curve.into(),
    ))
}

/// An X-ray binary's source by its kind: a low-mass transient's outbursts, a persistent neutron
/// star's bursts or a Be/X host's giant outbursts; `None` for a persistent black hole or a
/// supergiant host, whose X-rays are steady.
///
/// # Panics
///
/// Never: the scan's masses, recurrences and rates are in the curves' ranges.
#[must_use]
fn xray_source(seed: Seed, scan: &XrbScanMarks) -> Option<Source> {
    let system = SystemId::from(scan.id());
    let mass = scan.accretor_mass();
    match scan.kind() {
        XrbHostKind::NeutronStarTransient | XrbHostKind::BlackHoleTransient => {
            let recurrence = scan.recurrence()?;
            let rate = scan.transfer_rate()?;
            let period = Span::from_seconds_f64(recurrence.value() * SECONDS_PER_JULIAN_YEAR)
                .expect("a recurrence of years is a span");
            let gathered = SolarMasses::new(rate.value() * recurrence.value());
            let curve =
                XrayTransientCurve::of(scan.kind().accretor(), mass, gathered, DONOR_HYDROGEN)
                    .expect("a transient's accretor and gathered mass are in range");
            Some(phase_source(
                BinaryEventKind::XrayTransient,
                body_series(seed, event_tags::XRAY_TRANSIENT, system),
                period,
                None,
                curve.into(),
            ))
        }
        XrbHostKind::NeutronStarPersistent => {
            let (fuel, period) = burst_cycle(mass, scan.transfer_rate()?)?;
            let curve = XrayBurstCurve::of(mass, fuel).expect("a host's neutron star is in range");
            Some(phase_source(
                BinaryEventKind::XrayBurst,
                body_series(seed, event_tags::XRAY_BURST, system),
                period,
                None,
                curve.into(),
            ))
        }
        XrbHostKind::BeX => {
            let curve = BinaryLightCurve::from(
                BeXOutburstCurve::of(mass, DONOR_HYDROGEN)
                    .expect("a host's neutron star is in range"),
            );
            let bins_back = active_duration(&curve)
                .seconds()
                .div_euclid(BEX_BIN_SECONDS)
                + 1;
            let bins = PoissonBins::new(
                BEX_BIN_SECONDS,
                u32::try_from(bins_back).expect("an outburst of months spans a few bins"),
            )
            .expect("a year's bins looking back a few are Poisson bins");
            Some(Source {
                kind: BinaryEventKind::BeXGiantOutburst,
                series: body_series(seed, event_tags::BEX_GIANT_OUTBURST, system),
                construction: Construction::Bins {
                    bins,
                    rate: Constant(EventsPerSecond::per_julian_year(
                        BEX_GIANT_OUTBURST_RATE_PER_YEAR,
                    )),
                },
                curve,
            })
        }
        XrbHostKind::BlackHolePersistent | XrbHostKind::Supergiant => None,
    }
}

/// The luminous red nova of a stellar merger's scan marks: one-shot at its merger, of the pair's
/// total mass.
///
/// # Panics
///
/// Never: a merger's mass is two track stars', at most 300 M☉, the curve's range.
#[must_use]
fn red_nova_source(seed: Seed, scan: &MergerScanMarks) -> Source {
    let curve = LuminousRedNovaCurve::of(scan.total_mass())
        .expect("a merger's mass is two track stars', at most 300 M☉");
    one_shot_source(
        seed,
        BinaryEventKind::LuminousRedNova,
        SystemId::from(scan.id()),
        scan.merger_time(),
        curve.into(),
    )
}

/// The kilonova of a neutron-star merger's scan marks: one-shot at its merger, of its ejecta, or
/// `None` for a merger that ejects nothing.
///
/// # Panics
///
/// Never: a merger ejects at most GW170817's 0.05 M☉, in the curve's range.
#[must_use]
fn kilonova_source(seed: Seed, scan: &NsmScanMarks) -> Option<Source> {
    let ejecta = scan.ejecta_mass();
    (ejecta.value() > 0.0).then(|| {
        let curve = KilonovaCurve::of(ejecta)
            .expect("a merger ejects at most GW170817's 0.05 M☉, in the curve's range");
        one_shot_source(
            seed,
            BinaryEventKind::Kilonova,
            SystemId::from(scan.id()),
            scan.merger_time(),
            curve.into(),
        )
    })
}

/// A one-shot source of `kind` of `system` at `at`: bin 0, number 0 of its tag on the system.
#[must_use]
fn one_shot_source(
    seed: Seed,
    kind: BinaryEventKind,
    system: SystemId,
    at: UniverseTime,
    curve: BinaryLightCurve,
) -> Source {
    Source {
        kind,
        series: EventSeries::new(seed, kind.tag(), EventSubject::System(system)),
        construction: Construction::Once { at },
        curve,
    }
}

/// A neutron star of `mass` accreting at `rate`'s Type I bursts: their fuel and recurrence, the
/// ignition column [`BURST_IGNITION_COLUMN_G_CM2`] over the star's surface (plan 06's 12.2 km)
/// over the rate. `None` at or above the Eddington rate, where the burning is stable, for a rate
/// that is not positive, or for a recurrence past the clock's range.
#[must_use]
pub(crate) fn burst_cycle(
    mass: SolarMasses,
    rate: SolarMassesPerYear,
) -> Option<(BurstFuel, Span)> {
    let share = rate.value() / eddington_rate(XrayAccretor::NeutronStar, mass).value();
    let fuel = burst_fuel(share)?;
    let radius_cm = neutron_star_radius(RemnantRecipe::default()).value() * SOLAR_RADIUS_M * 100.0;
    let ignition_g =
        4.0 * core::f64::consts::PI * radius_cm * radius_cm * BURST_IGNITION_COLUMN_G_CM2;
    let ignition = ignition_g / 1.0e3 / SOLAR_MASS_KG;
    let years = ignition / rate.value();
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR).map(|period| (fuel, period))
}

/// The fuel of the bursts of a neutron star accreting at `share` of its Eddington rate
/// ([`HELIUM_BURST_SHARES`]), or `None` at or above 1 or for a share that is not positive.
#[must_use]
pub(crate) fn burst_fuel(share: f64) -> Option<BurstFuel> {
    let (helium_from, helium_to) = HELIUM_BURST_SHARES;
    if !(share > 0.0 && share < 1.0) {
        return None;
    }
    Some(if (helium_from..helium_to).contains(&share) {
        BurstFuel::Helium
    } else {
        BurstFuel::HydrogenHelium
    })
}

/// Appends to `out` the events of `system` whose onsets lie in `window`, in time order (ties by
/// ID), of every kind the host has (module documentation).
///
/// # Examples
///
/// A fast accreting white dwarf host's novae over its source horizon, listed once and over two
/// halves, the same; each is the white dwarf's one curve:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::AccretingWdFast;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::binary::events::{BinarySystem, events_in};
/// use hyperion_sim::stellar::binary::scan::scan_cell;
/// use hyperion_sim::time::{SourceHorizon, UniverseTime};
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = AccretingWdFast::new(&galaxy);
/// let mut hosts = Vec::new();
/// let key = CatalogueCellKey::new(ClassId::ACCRETING_WHITE_DWARF, [0, 20, 0])?;
/// scan_cell(&galaxy, process.hosts(), key, &mut hosts);
/// let host = hosts.first().ok_or("the inner disc has accreting white dwarfs")?;
/// let system = BinarySystem::accreting_white_dwarf_scan(galaxy.seed(), host);
/// let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END)?;
/// let mut whole = Vec::new();
/// events_in(&system, horizon, &mut whole);
/// assert!(!whole.is_empty());
/// let mut halves = Vec::new();
/// events_in(&system, TimeWindow::new(SourceHorizon::START, UniverseTime::EPOCH)?, &mut halves);
/// events_in(&system, TimeWindow::new(UniverseTime::EPOCH, SourceHorizon::END)?, &mut halves);
/// assert_eq!(whole, halves);
/// assert!(whole.iter().all(|e| e.curve() == whole[0].curve()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn events_in(system: &BinarySystem<'_>, window: TimeWindow, out: &mut Vec<BinaryEvent>) {
    let begin = out.len();
    let mut scratch = Scratch::default();
    for source in system.sources().iter().flatten() {
        source.events_in(window, &mut scratch, out);
    }
    out[begin..].sort_by(|a, b| a.onset.cmp(&b.onset).then(a.id.cmp(&b.id)));
}

/// Appends to `out` the events of `system` active at `t`, in time order (ties by ID): those whose
/// onset lies at or before `t` within their duration ([`BinaryEvent::is_active_at`]).
///
/// # Panics
///
/// Never: a nanosecond is a span, and the look-back's start comes at or before `t`.
pub fn active_at(system: &BinarySystem<'_>, t: UniverseTime, out: &mut Vec<BinaryEvent>) {
    let begin = out.len();
    let just_after = t
        .checked_add(Span::new(0, 1).expect("a nanosecond is a span"))
        .unwrap_or(t);
    let mut scratch = Scratch::default();
    let mut listed = Vec::new();
    for source in system.sources().iter().flatten() {
        let duration = active_duration(&source.curve);
        // An instant within a duration of the clock's own start looks back from `t` alone: no
        // event of these kinds lies there.
        let from = t.checked_sub(duration).unwrap_or(t);
        let window = TimeWindow::new(from, just_after)
            .expect("the look-back's start comes at or before t, before the window's end");
        listed.clear();
        source.events_in(window, &mut scratch, &mut listed);
        out.extend(
            listed
                .drain(..)
                .filter(|e| active_within(e.onset, duration, t)),
        );
    }
    out[begin..].sort_by(|a, b| a.onset.cmp(&b.onset).then(a.id.cmp(&b.id)));
}

/// The light of `event` in `band` `dt` after its onset, W (plan 11's Provides): its curve's
/// ([`BinaryLightCurve::in_band`]), zero before the onset.
#[must_use]
pub fn light_curve(event: &BinaryEvent, dt: Span, band: Band) -> Watts {
    event.curve.in_band(dt, band)
}

/// The 2–10 keV luminosity of `event` `dt` after its onset, W (plan 11's Provides): its curve's
/// ([`BinaryLightCurve::xray_luminosity`]), zero for a kind with no X-rays and before the onset.
#[must_use]
pub fn xray_luminosity(event: &BinaryEvent, dt: Span) -> Watts {
    event.curve.xray_luminosity(dt)
}

#[cfg(test)]
mod tests;
