//! The global dust storms of dry, thin-aired planets: `body.duststorm` (plan 14, P14.T31.b;
//! design note 15), after Mars's planet-encircling storms.
//!
//! # The phase
//!
//! At most one storm a cycle, where the phase Φ = φ + N(φ) of the body's `body.duststorm` series
//! crosses each whole cycle (plan 06, design note 16; [`MonotonePhase`]), with φ on a
//! [`LinearClock`] whose period P is the body's orbital period and whose origin is a perihelion
//! passage, so that φ's fraction is the mean anomaly over 2π.
//!
//! - **Skip:** each cycle passes without a storm with probability [`SKIP_PROBABILITY`], two in
//!   three ([`SkipMark`]): Zurek and Martin (1993, J. Geophys. Res. 98, 3247) find the record
//!   random between years with odds of about one in three (18–55% at 95% confidence), and the
//!   orbiters' record, Mars years 23–34, holds three in twelve (MY 25, 28 and 34).
//! - **Season:** a storm breaks out in the perihelion season, the half of the orbit within ±90° of
//!   true anomaly of perihelion ([`SEASON_HALF_WIDTH`]): for Mars (e = 0.0934, perihelion at
//!   Ls 251.0° at J2000; Allison and McEwen 2000, Planet. Space Sci. 48, 215) Ls ≈ 161–341°, 0.44
//!   of the year, whose start is the dust season's nominal start at Ls ~160, the season Ls 160–340
//!   of the southern spring and summer (Shirley and Mischna 2017, §3 and §7, after Zurek and Martin
//!   1993; Shirley 2015, Icarus 251, 128). It holds every catalogued
//!   onset: Ls 185–310 in Shirley's catalogue (as Table 1 of Shirley and Mischna 2017, Planet.
//!   Space Sci. 139, 37, gives it), true anomaly −66° to +59°, and 2018's first signature at
//!   Ls 186.2 (Kass et al. 2020, Geophys. Res. Lett. 47, e2019GL083931). Its half-width in cycles
//!   is h(e) = (arccos e − e √(1 − e²)) ÷ 2π ([`season_half_width_cycles`]): 0.2203 at Mars's e,
//!   0.25 at e = 0, 0.156 at e = 0.3.
//! - **Noise:** J = [`NOISE_OCTAVES`] (2) octaves on a lattice of [`NOISE_LATTICE_CYCLES`] (1)
//!   cycle, of amplitude a(e) = min([`SEASON_MARGIN`] × h(e) ÷ (1 + √2),
//!   [`MAX_NOISE_AMPLITUDE_CYCLES`]) ([`noise_amplitude_cycles`]). The noise's bound, a (1 + √2),
//!   then stays at or under 0.9 h(e), so every onset, which lies the noise's value from its
//!   perihelion in mean anomaly, falls in the season. For Mars a = 0.0821 and the bound is 0.198
//!   cycles, about ±82° of true anomaly. The cap 0.087 is the slope check's (10.3 a ÷ ℓ at 0.896),
//!   and binds only below e ≈ 0.05. One octave cannot work: the slope check would cap the bound at
//!   ±31° of mean anomaly, which excludes the onsets at Ls 185–208 and 300–310.
//!
//! **A departure from plan 06's design note 16 and the brainstorm's "a plain jittered lattice is
//! wrong, because its phase never diffuses"** ("Events in time"; decision-p14-t31b-phases §1): the
//! largest octave does not span the source horizon, so the phase does not diffuse. Seasonal
//! forcing locks the phase to the orbit; note 16's diffusion is for internally timed cycles. A
//! phase that diffused at P = 1.88 years would spread the storms over every season within a few
//! centuries.
//!
//! **Near-circular orbits** have no cut-off in e, so the output never jumps at a threshold: as
//! e → 0 the season is the half-year about the elements' pericentre, though the forcing it stands
//! for, the perihelion's extra sunlight, fades with the contrast (1 + e)² ÷ (1 − e)² − 1
//! ([`perihelion_contrast`]): 45% at Mars, 7% at Earth's e.
//!
//! # Which bodies
//!
//! [`has_dust_storms`]: the worlds whose background dust, P14.T24.c's
//! [`dust_optical_depth`](crate::planetary::hooks::aerosols::dust_optical_depth), is above 0: dry,
//! with air below [`DUST_AIR_LIMIT`](crate::planetary::hooks::aerosols::DUST_AIR_LIMIT), and able
//! to lift dust. A storm then raises a column that exists (plan 14, P14.T24.c); the raise is not
//! built here. P14.T31.c applies the rule to the record.
//!
//! # Marks
//!
//! From each storm's own stream, [`CycleEvent::marks`], at word 0:
//!
//! - **word 0**, its duration as a share of the period, uniform in [[`MIN_DURATION_SHARE`],
//!   [`MAX_DURATION_SHARE`]), so that no storm outlasts its cycle.
//!
//! Words 1–7 are reserved, for its peak optical depth and its place of onset, in that order. A
//! phase kind opens no other event stream of a cycle: the cycle's own stream holds its skip, and
//! streams 1 and up the noise's lattice.
//!
//! Not modelled: the two global storms of 1977, of which the construction gives one; orbit–spin
//! coupling (Shirley 2015), since the skip is independent between years.
//!
//! No storm leaves a mark (design note 15), and none changes any `body_at` result.

use core::f64::consts::{FRAC_PI_2, SQRT_2, TAU};
use std::error::Error;
use std::fmt;

use super::{LEAST_INTERVAL_SHARE, end_of, events_about, share_of};
use crate::Seed;
use crate::events::{
    CycleEvent, EventId, EventSeries, LinearClock, MonotonePhase, Phase, PhaseClock, SkipMark,
    TimeWindow,
};
use crate::id::BodyId;
use crate::id::event_tags::BODY_DUSTSTORM;
use crate::math;
use crate::orbit::{Eccentricity, solve_kepler};
use crate::time::{Span, UniverseTime};
use crate::units::Radians;

/// The probability that a cycle passes without a global dust storm: two in three (Zurek and Martin
/// 1993; see the [module documentation](self)). Part of the generator version.
pub const SKIP_PROBABILITY: f64 = 2.0 / 3.0;

/// The season's half-width in true anomaly about perihelion: 90°, the half of the orbit nearer the
/// star (decision-p14-t31b-phases §1). Part of the generator version.
pub const SEASON_HALF_WIDTH: Radians = Radians::new(FRAC_PI_2);

/// The share of the season's half-width the noise's bound may reach: 0.9, a margin of a tenth.
/// Part of the generator version.
pub const SEASON_MARGIN: f64 = 0.9;

/// The noise's largest amplitude at its finest octave, cycles: 0.087, which puts the slope bound,
/// 10.3 a ÷ ℓ, at 0.896 under [`MonotonePhase::MAX_SLOPE`]'s 0.9, as the Vela-like glitches' does.
/// Part of the generator version.
pub const MAX_NOISE_AMPLITUDE_CYCLES: f64 = 0.087;

/// The noise's octaves: 2, the fewest whose bound can reach most of the season (see the [module
/// documentation](self)). Part of the generator version.
pub const NOISE_OCTAVES: u32 = 2;

/// The noise's lattice spacing at its finest octave, whole cycles: 1. Part of the generator
/// version.
pub const NOISE_LATTICE_CYCLES: u32 = 1;

/// The shortest global dust storm, as a share of the period: 0.16, 110 days at Mars. Part of the
/// generator version.
///
/// The three global storms seen from orbit lasted from 110 to over 154 days, onset to decay, 0.16
/// to 0.224 of the Mars year of 686.980 days (NASA's Mars fact sheet), and the upper bound rounds
/// 0.224 up: MY 25 (2001), between June and October 2001 from its start near Ls 177, 110–140 days
/// (Strausberg et al. 2005, J. Geophys. Res. 110, E02006, read from its abstract); MY 28 (2007),
/// from Ls 262 to its steady decline after Ls 295 and normal levels by Ls ~330–335, about 114–123
/// days (Lemmon et al. 2015, Icarus 251, 96, §4.1 and Fig. 8); and MY 34 (2018), from Ls 186.2 to
/// background conditions about the southern summer solstice, about 136 days (Kass et al. 2020,
/// read from its abstract), or more than 150 sols, about 154 days, of dust over the planet
/// (Montabone et al. 2020, J. Geophys. Res. Planets 125, e2019JE006111). The days are the Ls
/// converted by Allison and McEwen's (2000) Mars24 algorithm. The shares are carried to other
/// periods as they are.
pub const MIN_DURATION_SHARE: f64 = 0.16;

/// The longest global dust storm, as a share of the period: 0.23, 158 days at Mars, the longest
/// seen (about 154 days) rounded up (see [`MIN_DURATION_SHARE`]). Part of the generator version.
pub const MAX_DURATION_SHARE: f64 = 0.23;

// Every storm lasts a positive share of the period under the least interval between two, so that
// no two run at once and `events_about` holds every storm running at an instant.
const _: () = assert!(0.0 < MIN_DURATION_SHARE && MIN_DURATION_SHARE < MAX_DURATION_SHARE);
const _: () = assert!(MAX_DURATION_SHARE < LEAST_INTERVAL_SHARE);

/// The noise's bound over its finest octave's amplitude for [`NOISE_OCTAVES`] octaves:
/// 2⁰ + 2^(1/2) = 1 + √2.
const BOUND_OVER_AMPLITUDE: f64 = 1.0 + SQRT_2;

/// h(e), the perihelion season's half-width in cycles of mean anomaly for an orbit of eccentricity
/// `e`: (arccos e − e √(1 − e²)) ÷ 2π, the mean anomaly at a true anomaly of 90° over 2π.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::events::dust_storms::season_half_width_cycles;
///
/// // A circular orbit's season is half of it.
/// assert!((season_half_width_cycles(Eccentricity::CIRCULAR) - 0.25).abs() < 1e-15);
/// let mars = season_half_width_cycles(Eccentricity::new(0.0934)?);
/// assert!((mars - 0.2203).abs() < 1e-4);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
///
/// Near e = 1 the two terms nearly cancel, so the difference is held at 0 or above.
#[must_use]
pub fn season_half_width_cycles(e: Eccentricity) -> f64 {
    let e = e.value();
    ((math::acos(e) - e * (1.0 - e * e).sqrt()) / TAU).max(0.0)
}

/// a(e), the noise's amplitude at its finest octave for an orbit of eccentricity `e`, cycles:
/// min([`SEASON_MARGIN`] × h(e) ÷ (1 + √2), [`MAX_NOISE_AMPLITUDE_CYCLES`]), so that the noise's
/// bound stays inside the season with a margin and its slope passes the check; in
/// [0, [`MAX_NOISE_AMPLITUDE_CYCLES`]].
#[must_use]
pub fn noise_amplitude_cycles(e: Eccentricity) -> f64 {
    (SEASON_MARGIN * season_half_width_cycles(e) / BOUND_OVER_AMPLITUDE)
        .clamp(0.0, MAX_NOISE_AMPLITUDE_CYCLES)
}

/// The perihelion's extra sunlight over the aphelion's for an orbit of eccentricity `e`,
/// (1 + e)² ÷ (1 − e)² − 1: the forcing the season stands for, 0.455 at Mars's e and 0.069 at
/// Earth's.
#[must_use]
pub fn perihelion_contrast(e: Eccentricity) -> f64 {
    let ratio = (1.0 + e.value()) / (1.0 - e.value());
    ratio * ratio - 1.0
}

/// Whether a planet whose background dust has τ(550) `background_optical_depth` has global dust
/// storms: exactly where P14.T24.c's
/// [`dust_optical_depth`](crate::planetary::hooks::aerosols::dust_optical_depth) is above 0 (the
/// rule P14.T31.c applies to a body's record; see the [module documentation](self)). A NaN has
/// none.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::events::dust_storms::has_dust_storms;
/// use hyperion_sim::planetary::hooks::aerosols::MARS_DUST_OPTICAL_DEPTH;
///
/// assert!(has_dust_storms(MARS_DUST_OPTICAL_DEPTH));
/// assert!(!has_dust_storms(0.0));
/// ```
#[must_use]
pub fn has_dust_storms(background_optical_depth: f64) -> bool {
    background_optical_depth > 0.0
}

/// What a [`DustStormModel`] reads of a planet: its orbital period, a perihelion passage and its
/// orbit's eccentricity.
///
/// Plain data with public fields: [`DustStormModel::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DustStormInputs {
    /// Its orbital period, P: the clock's period, at least [`LinearClock::MIN_PERIOD`].
    pub orbital_period: Span,
    /// A perihelion passage: the instant of the clock's phase 0.
    pub perihelion: UniverseTime,
    /// Its orbit's eccentricity, in [0, 1) by construction, which sets the season's width.
    pub eccentricity: Eccentricity,
}

/// A [`DustStormModel`] could not be built, with the value refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildDustStormModelError {
    /// The orbital period is shorter than [`LinearClock::MIN_PERIOD`].
    PeriodTooShort(Span),
}

impl fmt::Display for BuildDustStormModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodTooShort(period) => write!(
                f,
                "a dust storm period of {period} is shorter than the {} minimum",
                LinearClock::MIN_PERIOD
            ),
        }
    }
}

impl Error for BuildDustStormModelError {}

/// The global dust storms of one planet: its series, its clock and the phase's noise and skip (see
/// the [module documentation](self)).
///
/// # Examples
///
/// A Mars has a global dust storm in about one year in three, each breaking out in the perihelion
/// season:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::events::dust_storms::{SEASON_HALF_WIDTH, dust_storms};
/// use hyperion_sim::planetary::events::{DustStormInputs, DustStormModel};
/// use hyperion_sim::time::{Span, UniverseTime};
///
/// let mars = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0x0104);
/// let inputs = DustStormInputs {
///     orbital_period: Span::from_seconds(59_355_072),
///     perihelion: UniverseTime::EPOCH,
///     eccentricity: Eccentricity::new(0.0934)?,
/// };
/// let model = DustStormModel::new(Seed::new(7), mars, &inputs)?;
/// let window = TimeWindow::new(
///     UniverseTime::EPOCH,
///     UniverseTime::from_julian_years(300).ok_or("in range")?,
/// )?;
/// let mut listed = Vec::new();
/// dust_storms(&model, window, &mut listed);
/// // 159.5 Mars years, a third of them with a storm.
/// assert!((35..=72).contains(&listed.len()));
/// let half_width = SEASON_HALF_WIDTH.value();
/// assert!(listed.iter().all(|storm| storm.onset_true_anomaly().value().abs() <= half_width));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DustStormModel {
    series: EventSeries,
    inputs: DustStormInputs,
    clock: LinearClock,
    noise: MonotonePhase,
}

impl DustStormModel {
    /// The global dust storms of `body` in the universe of `seed`, from `inputs`.
    ///
    /// The series, `EventSeries::new(seed, BODY_DUSTSTORM, body.into())`, is opened here from
    /// `seed` and `body`, so that the two cannot come from two universes.
    ///
    /// # Errors
    ///
    /// [`BuildDustStormModelError::PeriodTooShort`] for an orbital period under
    /// [`LinearClock::MIN_PERIOD`].
    ///
    /// # Panics
    ///
    /// Only on a broken invariant: an amplitude in [0, [`MAX_NOISE_AMPLITUDE_CYCLES`]] passes the
    /// noise's checks.
    pub fn new(
        seed: Seed,
        body: BodyId,
        inputs: &DustStormInputs,
    ) -> Result<Self, BuildDustStormModelError> {
        let clock = LinearClock::new(inputs.orbital_period, inputs.perihelion)
            .map_err(|_| BuildDustStormModelError::PeriodTooShort(inputs.orbital_period))?;
        let noise = MonotonePhase::new(
            noise_amplitude_cycles(inputs.eccentricity),
            NOISE_LATTICE_CYCLES,
            NOISE_OCTAVES,
        )
        .expect("an amplitude in [0, 0.087] is valid and keeps the slope bound at 0.896, under 0.9")
        .with_skip(SkipMark::from_probability(SKIP_PROBABILITY));
        Ok(Self {
            series: EventSeries::new(seed, BODY_DUSTSTORM, body.into()),
            inputs: *inputs,
            clock,
            noise,
        })
    }

    /// The series: the body's subject, [`BODY_DUSTSTORM`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The inputs it was built from.
    #[must_use]
    pub const fn inputs(&self) -> &DustStormInputs {
        &self.inputs
    }

    /// The clock: the orbital period from the perihelion passage.
    #[must_use]
    pub const fn clock(&self) -> &LinearClock {
        &self.clock
    }

    /// The phase's noise, with its skip.
    #[must_use]
    pub const fn noise(&self) -> &MonotonePhase {
        &self.noise
    }

    /// The phase Φ(t) of the body's series at `t`: cycle n's storm, unless skipped, falls where it
    /// reaches n.
    #[must_use]
    pub fn phase_at(&self, t: UniverseTime) -> Phase {
        self.noise.phase_at(&self.series, &self.clock, t)
    }

    /// The planet's true anomaly at `t`, rad, in [−π, π]: from the mean anomaly the clock gives,
    /// through [`solve_kepler`]. The season is where its magnitude is at most
    /// [`SEASON_HALF_WIDTH`].
    #[must_use]
    pub fn true_anomaly_at(&self, t: UniverseTime) -> Radians {
        let e = self.inputs.eccentricity;
        let half = 0.5 * solve_kepler(mean_anomaly_at(&self.clock, t), e).value();
        let (sine, cosine) = math::sin_cos(half);
        Radians::new(
            2.0 * math::atan2(
                (1.0 + e.value()).sqrt() * sine,
                (1.0 - e.value()).sqrt() * cosine,
            ),
        )
    }

    /// The storm of `event`, a cycle of this model's series that is not skipped: its duration from
    /// word 0 of its marks.
    #[must_use]
    fn dust_storm_of(&self, event: CycleEvent) -> DustStorm {
        let (id, cycle, onset) = (event.id(), event.cycle(), event.time());
        let share = event
            .into_marks()
            .uniform_in(MIN_DURATION_SHARE, MAX_DURATION_SHARE);
        let duration = share_of(self.clock.period(), share);
        DustStorm {
            id,
            cycle,
            onset,
            duration,
            end: end_of(onset, duration),
            onset_true_anomaly: self.true_anomaly_at(onset),
        }
    }
}

/// The base phase of `clock` at `t` as a mean anomaly, rad, in [−π, π): the fraction of the cycle
/// since the clock's origin, a perihelion passage, taken about the nearer passage.
#[must_use]
fn mean_anomaly_at(clock: &LinearClock, t: UniverseTime) -> Radians {
    let fraction = clock.base_phase(t).fraction();
    let about = if fraction >= 0.5 {
        fraction - 1.0
    } else {
        fraction
    };
    Radians::new(TAU * about)
}

/// One global dust storm on a planet: its ID, its cycle, its onset, its duration and the true
/// anomaly at its onset (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DustStorm {
    id: EventId,
    cycle: i64,
    onset: UniverseTime,
    duration: Span,
    end: UniverseTime,
    onset_true_anomaly: Radians,
}

impl DustStorm {
    /// The storm's ID, from which [`dust_storm`] regenerates it: the cycle as its bin, number 0.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The cycle, counted in perihelion passages from the inputs' own, whose storm this is.
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

    /// The planet's true anomaly at the onset, rad: within [`SEASON_HALF_WIDTH`] of perihelion.
    #[must_use]
    pub const fn onset_true_anomaly(&self) -> Radians {
        self.onset_true_anomaly
    }

    /// Whether it is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end
    }
}

/// Appends to `out` the planet's global dust storms whose onset lies in `window`, in time order,
/// which is cycle order.
///
/// Each storm's time and skip are functions of its cycle alone, so a listing over a window is the
/// union of the listings over any partition of it, in any order of asking. `out` is only appended
/// to.
pub fn dust_storms(model: &DustStormModel, window: TimeWindow, out: &mut Vec<DustStorm>) {
    let mut events = Vec::new();
    model
        .noise
        .events_in(&model.series, &model.clock, window, &mut events);
    out.extend(events.into_iter().map(|event| model.dust_storm_of(event)));
}

/// Appends to `out` the planet's global dust storms running at `t`: at most one, since a storm
/// lasts at most [`MAX_DURATION_SHARE`] of the period, under the least interval between two.
///
/// It is found among the cycles about the one the phase is in at `t`. The result depends on the
/// model and `t` alone.
pub fn dust_storms_active_at(model: &DustStormModel, t: UniverseTime, out: &mut Vec<DustStorm>) {
    out.extend(
        events_about(&model.noise, &model.series, &model.clock, t)
            .map(|event| model.dust_storm_of(event))
            .filter(|storm| storm.is_active_at(t)),
    );
}

/// The global dust storm `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject), its number
/// is not 0, its cycle is skipped, or it lies beyond the 40-bit cycle numbers.
#[must_use]
pub fn dust_storm(model: &DustStormModel, id: EventId) -> Option<DustStorm> {
    model
        .noise
        .event_by_id(&model.series, &model.clock, id)
        .map(|event| model.dust_storm_of(event))
}

#[cfg(test)]
mod tests;
