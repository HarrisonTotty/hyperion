//! The scan marks of the binary catalogue classes (plan 11's Provides; P11.T8.a; design notes 10
//! and 12): what a galaxy-wide scan needs of a host, drawn first and on draws of its own, without
//! the rest of the system and without the binary engine.
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

pub use merger::{MergerScanMarks, horizon_time, merger_scan_marks, scan_merger_cell};

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::events::{
    CycleEvent, EventSeries, LinearClock, MonotonePhase, PhaseClock, TimeWindow, octaves_to_span,
};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide};
use crate::galaxy::fields::ComponentId;
use crate::id::{BodyId, CatalogueSystemId, SystemId, event_tags};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::tables::binary::{AWD_SAMPLER, AwdMark};
use crate::time::{SourceHorizon, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{PerCubicLightYear, SolarMasses, SolarMassesPerYear, Years};

use super::classify::{nova_recurrence_period, recurrence_class};
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

/// The scan marks of candidate `id` of the accreting white dwarf class cell `cell` under its
/// `bound`, or `None` if the class keeps no entry there (P11.T8.a): the site and its thinning,
/// the recurrence on the class's side of L, the velocity, the nova series and clock, and a slow
/// host's eruptions in the source horizon. It never runs the binary engine.
///
/// The marks are the entry's own, bit for bit: the class's candidate draws them through this
/// function first.
///
/// # Panics
///
/// If `cell` is not a cell of `process`'s class, as plan 09's cell; in debug builds, on a density
/// above the bound.
#[must_use]
pub fn scan_marks(
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

/// Appends to `out` the scan marks of every entry of the accreting white dwarf class cell `key`,
/// in index order: one bound, one count and [`scan_marks`] of each candidate, the cell's entries
/// exactly, without the rest of their systems.
///
/// # Panics
///
/// As [`scan_marks`], and as plan 09's count on a bound past the Poisson sampler's limit.
pub fn scan_cell(
    galaxy: &Galaxy,
    process: &AwdProcess,
    key: CatalogueCellKey,
    out: &mut Vec<AwdScanMarks>,
) {
    let bound = process.bound(galaxy, &key);
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
