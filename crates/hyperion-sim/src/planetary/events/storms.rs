//! The great storms of giant planets: `body.storm` (plan 14, P14.T31.b; design note 15), after
//! Saturn's great white spots.
//!
//! # The phase
//!
//! One storm falls where the phase Φ = φ + N(φ) of the body's `body.storm` series crosses each
//! whole cycle (plan 06, design note 16; [`MonotonePhase`]), with φ on a [`LinearClock`] whose
//! period P is the body's orbital period and whose origin is a plain input; P14.T31.c passes a
//! perihelion passage.
//!
//! - **Noise:** octaves of value noise of amplitude [`NOISE_AMPLITUDE_CYCLES`] (0.087) on a lattice
//!   of [`NOISE_LATTICE_CYCLES`] (1) cycle, as many octaves as span the source horizon
//!   ([`octaves_to_span`]), as `GlitchCycle::of` builds a Vela-like pulsar's: at Saturn's
//!   29.447 years, J = 15 and the noise's bound is 37.8 cycles. The slope bound, 10.3 a ÷ ℓ, is
//!   0.896, as steep as [`MonotonePhase::MAX_SLOPE`] allows, which sets the scatter of the
//!   intervals between storms: a coefficient of variation of about 0.09 (the glitches measured
//!   0.093 on this lattice), with no interval under P ÷ 1.9, 15.5 years at Saturn. Saturn's five
//!   intervals since 1876 give 0.15, or 0.06 without 2010's early storm.
//! - **No skip and no season** (decision-p14-t31b-phases §2): six storms over 1876–2010 is 1.10 a
//!   Saturn year, and no Saturn year has lacked one. The mechanism is internal, which is a monotone
//!   phase exactly: the water above its cloud base suppresses moist convection until the
//!   troposphere above has cooled radiatively, and then releases it (Li and Ingersoll 2015, Nature
//!   Geosci. 8, 398; Sugiyama et al. 2014, Icarus 229, 71, a model of Jupiter's whose interval is
//!   likewise set by radiative cooling; both reviewed by Sánchez-Lavega et al. 2024, arXiv
//!   2401.13294, which lists the season's part in triggering a storm as open).
//!   Five of the six storms came near northern summer, at Ls ~90–180, and 2010's in northern
//!   spring; holding storms to the summer would need the pole as an input, and is deferred.
//!
//! # Which bodies
//!
//! [`has_great_storms`]: every giant, gas or ice ([`PlanetClass::is_giant`]), a rule P14.T31.c
//! applies to the record. Li and Ingersoll's condition, that the troposphere's water vapour
//! exceeds a mixing ratio of about 1.0% (their abstract; the ruling's 0.9% is the density minimum
//! of Sánchez-Lavega et al. 2024, Fig. 14.9), which Jupiter's lower water may not meet, is not
//! modelled.
//!
//! # Marks
//!
//! From each storm's own stream, [`CycleEvent::marks`], at word 0:
//!
//! - **word 0**, its duration as a share of the period, uniform in [[`MIN_DURATION_SHARE`],
//!   [`MAX_DURATION_SHARE`]), so that no storm outlasts its cycle;
//! - **word 1**, its latitude band ([`StormBand`]), at the shares of Saturn's six storms: three
//!   equatorial, two at mid-latitudes and one polar, all northern.
//!
//! Words 2–7 are reserved, for a latitude within the band, a longitude and a size, in that order.
//! A phase kind opens no other event stream of a cycle: streams 1 and up hold the noise's lattice.
//!
//! No storm leaves a mark (design note 15), and none changes any `body_at` result.

use std::error::Error;
use std::fmt;

use super::{LEAST_INTERVAL_SHARE, end_of, events_about, share_of};
use crate::Seed;
use crate::events::{
    CycleEvent, EventId, EventSeries, LinearClock, MonotonePhase, Phase, TimeWindow,
    octaves_to_span,
};
use crate::id::BodyId;
use crate::id::event_tags::BODY_STORM;
use crate::planetary::derive::PlanetClass;
use crate::time::{SourceHorizon, Span, UniverseTime};

/// The noise's amplitude at its finest octave, cycles: 0.087, which with
/// [`NOISE_LATTICE_CYCLES`] puts the slope bound, 10.3 a ÷ ℓ, at 0.896, as near the bound of 0.9
/// as two figures allow (decision-p14-t31b-phases §2, the Vela-like glitches' choice). Part of the
/// generator version.
pub const NOISE_AMPLITUDE_CYCLES: f64 = 0.087;

/// The noise's lattice spacing at its finest octave, whole cycles: 1, so that consecutive
/// intervals take nearly independent lattice values. Part of the generator version.
pub const NOISE_LATTICE_CYCLES: u32 = 1;

/// The shortest storm, as a share of the period: 0.0024, 26 days at Saturn, the 1876 storm's.
/// Part of the generator version.
///
/// Saturn's six great white spots lasted 26 (1876), 150 (1903), 44 (1933), 41 (1960) and 55 days
/// (1990), from Alexander (1962) and Sánchez-Lavega (1994, Chaos 4, 341) as Sayanagi et al.
/// (2013, Icarus 223, 460, §1) give them, and 201 days (2010–11, the storm head's activity,
/// Sayanagi et al. §4), over Saturn's sidereal period of 10,755.70 days (NASA's Saturn fact sheet).
/// The four short ones were seen from Earth with gaps in coverage, and may be too short.
pub const MIN_DURATION_SHARE: f64 = 0.0024;

/// The longest storm, as a share of the period: 0.0187, 201 days at Saturn, the 2010–11 storm's
/// (see [`MIN_DURATION_SHARE`]). Part of the generator version.
pub const MAX_DURATION_SHARE: f64 = 0.0187;

// Every storm lasts a positive share of the period under the least interval between two, so that
// no two run at once and `events_about` holds every storm running at an instant.
const _: () = assert!(0.0 < MIN_DURATION_SHARE && MIN_DURATION_SHARE < MAX_DURATION_SHARE);
const _: () = assert!(MAX_DURATION_SHARE < LEAST_INTERVAL_SHARE);

/// The weights of the latitude bands, equatorial, mid-latitude and polar: Saturn's six great white
/// spots, three equatorial (1876 at 8° N, 1933 at 2° N and 1990 at 12° N, planetographic), two at
/// mid-latitudes (1903 at 36° N and 2010 at 38° N) and one polar (1960, at 58° N), all northern
/// (Sánchez-Lavega et al. 2018, *Saturn in the 21st Century*, Cambridge University Press, Table
/// 13.1; the bands as Sánchez-Lavega et al. 2024, arXiv 2401.13294, §14.2.3, name them). Part of
/// the generator version.
pub const BAND_WEIGHTS: [f64; 3] = [3.0, 2.0, 1.0];

/// Whether a planet of class `class` has great storms: every giant, gas or ice (the rule
/// P14.T31.c applies to a body's record; see the [module documentation](self)).
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::events::storms::has_great_storms;
///
/// assert!(has_great_storms(PlanetClass::GasGiant));
/// assert!(!has_great_storms(PlanetClass::Rocky));
/// ```
#[must_use]
pub const fn has_great_storms(class: PlanetClass) -> bool {
    class.is_giant()
}

/// What a [`StormModel`] reads of a giant: its orbital period and the clock's origin.
///
/// Plain data with public fields: [`StormModel::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StormInputs {
    /// Its orbital period, P: the clock's period, at least [`LinearClock::MIN_PERIOD`].
    pub orbital_period: Span,
    /// The instant of the clock's phase 0; P14.T31.c passes a perihelion passage.
    pub origin: UniverseTime,
}

/// A [`StormModel`] could not be built, with the value refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildStormModelError {
    /// The orbital period is shorter than [`LinearClock::MIN_PERIOD`].
    PeriodTooShort(Span),
}

impl fmt::Display for BuildStormModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodTooShort(period) => write!(
                f,
                "a storm period of {period} is shorter than the {} minimum",
                LinearClock::MIN_PERIOD
            ),
        }
    }
}

impl Error for BuildStormModelError {}

/// The great storms of one giant: its series, its clock and the phase's noise (see the [module
/// documentation](self)).
///
/// # Examples
///
/// A Saturn has a great storm about once an orbit, each lasting one to seven months:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::events::storms::storms;
/// use hyperion_sim::planetary::events::{StormInputs, StormModel};
/// use hyperion_sim::time::{Span, UniverseTime};
///
/// let saturn = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0x0106);
/// let inputs = StormInputs {
///     orbital_period: Span::from_seconds(10_756 * 86_400),
///     origin: UniverseTime::EPOCH,
/// };
/// let model = StormModel::new(Seed::new(7), saturn, &inputs)?;
/// let millennium = TimeWindow::new(
///     UniverseTime::EPOCH,
///     UniverseTime::from_julian_years(1_000).ok_or("in range")?,
/// )?;
/// let mut listed = Vec::new();
/// storms(&model, millennium, &mut listed);
/// // 33.9 periods, within the noise's reach of 37.8 cycles at each end.
/// assert!((20..=48).contains(&listed.len()));
/// assert!(listed.iter().all(|storm| storm.duration() < Span::from_seconds(202 * 86_400)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StormModel {
    series: EventSeries,
    inputs: StormInputs,
    clock: LinearClock,
    noise: MonotonePhase,
}

impl StormModel {
    /// The great storms of `body` in the universe of `seed`, from `inputs`.
    ///
    /// The series, `EventSeries::new(seed, BODY_STORM, body.into())`, is opened here from `seed`
    /// and `body`, so that the two cannot come from two universes.
    ///
    /// # Errors
    ///
    /// [`BuildStormModelError::PeriodTooShort`] for an orbital period under
    /// [`LinearClock::MIN_PERIOD`].
    ///
    /// # Panics
    ///
    /// Only on a broken invariant: the noise's constants pass its slope check at any number of
    /// octaves.
    pub fn new(
        seed: Seed,
        body: BodyId,
        inputs: &StormInputs,
    ) -> Result<Self, BuildStormModelError> {
        let clock = LinearClock::new(inputs.orbital_period, inputs.origin)
            .map_err(|_| BuildStormModelError::PeriodTooShort(inputs.orbital_period))?;
        let horizon = SourceHorizon::END
            .checked_since(SourceHorizon::START)
            .expect("the source horizon is a span the clock holds");
        let noise = MonotonePhase::new(
            NOISE_AMPLITUDE_CYCLES,
            NOISE_LATTICE_CYCLES,
            octaves_to_span(NOISE_LATTICE_CYCLES, clock.cycles_in(horizon)),
        )
        .expect("the noise's constants keep its slope bound at 0.896, under 0.9");
        Ok(Self {
            series: EventSeries::new(seed, BODY_STORM, body.into()),
            inputs: *inputs,
            clock,
            noise,
        })
    }

    /// The series: the body's subject, [`BODY_STORM`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The inputs it was built from.
    #[must_use]
    pub const fn inputs(&self) -> &StormInputs {
        &self.inputs
    }

    /// The clock: the orbital period from the inputs' origin.
    #[must_use]
    pub const fn clock(&self) -> &LinearClock {
        &self.clock
    }

    /// The phase's noise, which has no skip.
    #[must_use]
    pub const fn noise(&self) -> &MonotonePhase {
        &self.noise
    }

    /// The phase Φ(t) of the body's series at `t`: storm n falls where it reaches n, so the storms
    /// in a window are the whole cycles it crosses there.
    #[must_use]
    pub fn phase_at(&self, t: UniverseTime) -> Phase {
        self.noise.phase_at(&self.series, &self.clock, t)
    }

    /// The storm of `event`, a cycle of this model's series: its duration and band from words 0
    /// and 1 of its marks.
    #[must_use]
    fn storm_of(&self, event: CycleEvent) -> Storm {
        let (id, cycle, onset) = (event.id(), event.cycle(), event.time());
        let mut marks = event.into_marks();
        let share = marks.uniform_in(MIN_DURATION_SHARE, MAX_DURATION_SHARE);
        let band = StormBand::of_index(
            marks
                .mark()
                .pick_weighted(&BAND_WEIGHTS, BAND_TOTAL)
                .expect("the band weights sum to their bound, so no mark is rejected"),
        );
        let duration = share_of(self.clock.period(), share);
        Storm {
            id,
            cycle,
            onset,
            duration,
            end: end_of(onset, duration),
            band,
        }
    }
}

/// The total of [`BAND_WEIGHTS`], the bound its pick is made against, exactly: whole numbers
/// summed.
const BAND_TOTAL: f64 = BAND_WEIGHTS[0] + BAND_WEIGHTS[1] + BAND_WEIGHTS[2];

/// The latitude band of a great storm, in the northern hemisphere of the body's own frame, as
/// Saturn's six were (see [`BAND_WEIGHTS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StormBand {
    /// Near the equator, 2–12° N planetographic at Saturn: its storms of 1876, 1933 and 1990.
    Equatorial,
    /// At mid-latitudes, 36–38° N planetographic at Saturn: its storms of 1903 and 2010.
    MidLatitude,
    /// At high latitudes, 58° N planetographic at Saturn (52.5° planetocentric, subpolar rather
    /// than polar): its storm of 1960.
    Polar,
}

impl StormBand {
    /// The band of index `index` in [`BAND_WEIGHTS`]' order.
    ///
    /// # Panics
    ///
    /// For an index beyond the three bands, which a pick among [`BAND_WEIGHTS`] never gives.
    #[must_use]
    fn of_index(index: usize) -> Self {
        match index {
            0 => Self::Equatorial,
            1 => Self::MidLatitude,
            2 => Self::Polar,
            _ => unreachable!("a pick among three band weights gives an index below 3"),
        }
    }
}

/// One great storm on a giant: its ID, its cycle, its onset, its duration and its latitude band
/// (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Storm {
    id: EventId,
    cycle: i64,
    onset: UniverseTime,
    duration: Span,
    end: UniverseTime,
    band: StormBand,
}

impl Storm {
    /// The storm's ID, from which [`storm`] regenerates it: the cycle as its bin, number 0.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The cycle whose start the storm is.
    #[must_use]
    pub const fn cycle(&self) -> i64 {
        self.cycle
    }

    /// When it breaks out: where the phase reaches its cycle.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// How long it lasts: [`MIN_DURATION_SHARE`] to [`MAX_DURATION_SHARE`] of the period.
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// When it ends, the onset plus the duration: the first instant it is no longer running.
    #[must_use]
    pub const fn end(&self) -> UniverseTime {
        self.end
    }

    /// Its latitude band.
    #[must_use]
    pub const fn band(&self) -> StormBand {
        self.band
    }

    /// Whether it is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end
    }
}

/// Appends to `out` the giant's storms whose onset lies in `window`, in time order, which is cycle
/// order.
///
/// Each storm's time is a function of its cycle alone, so a listing over a window is the union of
/// the listings over any partition of it, in any order of asking. `out` is only appended to.
pub fn storms(model: &StormModel, window: TimeWindow, out: &mut Vec<Storm>) {
    let mut events = Vec::new();
    model
        .noise
        .events_in(&model.series, &model.clock, window, &mut events);
    out.extend(events.into_iter().map(|event| model.storm_of(event)));
}

/// Appends to `out` the giant's storms running at `t`: at most one, since a storm lasts at most
/// [`MAX_DURATION_SHARE`] of the period, under the least interval between two.
///
/// It is found among the cycles about the one the phase is in at `t`. The result depends on the
/// model and `t` alone.
pub fn storms_active_at(model: &StormModel, t: UniverseTime, out: &mut Vec<Storm>) {
    out.extend(
        events_about(&model.noise, &model.series, &model.clock, t)
            .map(|event| model.storm_of(event))
            .filter(|storm| storm.is_active_at(t)),
    );
}

/// The storm `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject), its number
/// is not 0, or its cycle lies beyond the 40-bit cycle numbers.
#[must_use]
pub fn storm(model: &StormModel, id: EventId) -> Option<Storm> {
    model
        .noise
        .event_by_id(&model.series, &model.clock, id)
        .map(|event| model.storm_of(event))
}

#[cfg(test)]
mod tests;
