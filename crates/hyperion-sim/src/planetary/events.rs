//! Events on bodies (plan 14, P14.T31; design note 15): what happens to a system's bodies during
//! play, as a pure function of the seed, the subject, the event tag and the time.
//!
//! Every kind is one of plan 06's two constructions ([`crate::events`]), on an [`EventSeries`]
//! whose subject is the body's
//! [`BodyId`](crate::id::BodyId), or the system's [`SystemId`](crate::id::SystemId) for its
//! comets, under the kind's own event tag in plan 06's block 0x0400–0x04FF. A series goes to one
//! construction only. The Poisson bins (P14.T31.a):
//!
//! - [`impacts`]: `body.impact` (0x0400), impacts above a stated energy at the belts' number flux
//!   at the body times its gravitationally focused cross-section, each with an energy from the
//!   size law and a place on the body;
//! - [`eruptions`]: `body.eruption` (0x0401), eruptions at a rate rising with P14.T24's heat flow
//!   and zero below the heat flow at which melt reaches the surface, each with a duration found by
//!   looking back a fixed number of bins;
//! - [`comets`]: `system.comet` (0x0404), the perihelion passages of long-period comets at
//!   P14.T21.d's rate, each a comet on its own near-parabolic orbit, listed while it is inside
//!   30 au × √(L ÷ L☉).
//!
//! The monotone phases (P14.T31.b), each on a [`LinearClock`] whose period is the body's orbital
//! period, from a perihelion passage for the dust storms and from a plain origin for the storms,
//! which P14.T31.c sets at one:
//!
//! - [`storms`]: `body.storm` (0x0402), the great storms of giant planets, Saturn's great white
//!   spots: one a cycle, on a phase that diffuses across the source horizon, each with a duration
//!   and a latitude band;
//! - [`dust_storms`]: `body.duststorm` (0x0403), the global dust storms of dry, thin-aired planets,
//!   Mars's: one in about three cycles, on a phase locked to the orbit so that every onset falls in
//!   the perihelion season, each with a duration.
//!
//! P14.T31.c merges every kind into `PlanetarySystem::events_between`. Until then nothing on the
//! record calls these modules: each takes what a body's record or hooks give as plain values,
//! which P14.T31.c reads once, at the epoch (design note 23 of plan 06), so that a rate never
//! depends on the time asked.
//!
//! No event leaves a mark (design note 15): an impact made during play is below the resolution of
//! the global figures, and no event changes any `body_at` result.
//!
//! # Shared pieces
//!
//! - [`SizeLaw`]: a cumulative power law of diameter, N(> D) ∝ D^(−q), normalised at a
//!   reference diameter and truncated at a largest one. The impacts draw their projectiles from
//!   one by their composition, and the comets their nuclei from another.
//! - [`SurfacePoint`]: a place on a body, uniform over its sphere.

pub mod comets;
pub mod dust_storms;
pub mod eruptions;
pub mod impacts;
pub mod storms;

use core::f64::consts::TAU;
use std::error::Error;
use std::fmt;

pub use comets::{BuildCometModelError, Comet, CometInputs, CometModel};
pub use dust_storms::{BuildDustStormModelError, DustStorm, DustStormInputs, DustStormModel};
pub use eruptions::{BuildEruptionModelError, Eruption, EruptionInputs, EruptionModel};
pub use impacts::{BuildImpactModelError, Impact, ImpactInputs, ImpactModel};
pub use storms::{BuildStormModelError, Storm, StormBand, StormInputs, StormModel};

use crate::events::{
    CycleEvent, EventSeries, EventsPerSecond, LinearClock, MonotonePhase, TimeWindow,
    from_nanos_saturating, to_nanos,
};
use crate::math;
use crate::rng::{BuildPowerLawError, PowerLaw, Stream};
use crate::time::{Span, UniverseTime};
use crate::units::{Metres, Radians};

/// A [`SizeLaw`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum BuildSizeLawError {
    /// The slope is not finite and positive.
    Slope(f64),
    /// The reference diameter is not finite and positive.
    Reference(Metres),
    /// The largest diameter is not finite or does not exceed the reference.
    Largest(Metres),
}

impl fmt::Display for BuildSizeLawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slope(slope) => write!(f, "size slope {slope} is not finite and positive"),
            Self::Reference(reference) => write!(
                f,
                "reference diameter {} m is not finite and positive",
                reference.value()
            ),
            Self::Largest(largest) => write!(
                f,
                "largest diameter {} m is not finite or not above the reference",
                largest.value()
            ),
        }
    }
}

impl Error for BuildSizeLawError {}

/// A cumulative power law of diameter, N(> D) ∝ D^(−q), normalised at a reference diameter
/// `D_ref` and truncated at a largest diameter `D_max`: the number of bodies over D per body over
/// the reference is S(D) = ((D ÷ `D_ref`)^(−q) − x) ÷ (1 − x), x = (`D_max` ÷ `D_ref`)^(−q), so
/// that S is 1 at the reference, 0 at `D_max`, and above 1 below the reference.
///
/// Its differential density is ∝ D^(−(q + 1)), from which each event draws its diameter in one
/// word ([`Stream::power_law`]). The laws the kinds use: [`impacts::rocky_projectiles`],
/// [`impacts::icy_projectiles`] and [`comets::nuclei`].
///
/// # Examples
///
/// A law of slope 2.25 has 10^2.25 times as many bodies over 100 m as over 1 km:
///
/// ```
/// use hyperion_sim::planetary::events::SizeLaw;
/// use hyperion_sim::units::Metres;
///
/// let law = SizeLaw::new(2.25, Metres::new(1e3), Metres::new(450e3))?;
/// assert!((law.share_above(Metres::new(100.0)) / 177.83 - 1.0).abs() < 1e-4);
/// assert!((law.diameter_at_share(177.83).value() - 100.0).abs() < 0.01);
/// # Ok::<(), hyperion_sim::planetary::events::BuildSizeLawError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SizeLaw {
    slope: f64,
    reference: Metres,
    largest: Metres,
}

impl SizeLaw {
    /// The law of slope `slope` (q, in N(> D) ∝ D^(−q)) normalised at `reference` and truncated at
    /// `largest`.
    ///
    /// # Errors
    ///
    /// - [`BuildSizeLawError::Slope`] unless the slope is finite and positive;
    /// - [`BuildSizeLawError::Reference`] unless the reference is finite and positive;
    /// - [`BuildSizeLawError::Largest`] unless the largest diameter is finite and above the
    ///   reference.
    pub fn new(slope: f64, reference: Metres, largest: Metres) -> Result<Self, BuildSizeLawError> {
        if !(slope.is_finite() && slope > 0.0) {
            return Err(BuildSizeLawError::Slope(slope));
        }
        if !(reference.value().is_finite() && reference.value() > 0.0) {
            return Err(BuildSizeLawError::Reference(reference));
        }
        if !(largest.value().is_finite() && largest > reference) {
            return Err(BuildSizeLawError::Largest(largest));
        }
        Ok(Self {
            slope,
            reference,
            largest,
        })
    }

    /// The slope q.
    #[must_use]
    pub const fn slope(&self) -> f64 {
        self.slope
    }

    /// The reference diameter `D_ref`, at which S is 1.
    #[must_use]
    pub const fn reference(&self) -> Metres {
        self.reference
    }

    /// The largest diameter, `D_max`.
    #[must_use]
    pub const fn largest(&self) -> Metres {
        self.largest
    }

    /// x = (`D_max` ÷ `D_ref`)^(−q), the truncation's share, in (0, 1).
    #[must_use]
    fn truncation(&self) -> f64 {
        math::powf(self.largest / self.reference, -self.slope)
    }

    /// S(D), the number of bodies over `diameter` per body over the reference: 1 at the reference
    /// exactly, 0 at and above the largest diameter, and above 1 below the reference.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the diameter is positive.
    #[must_use]
    pub fn share_above(&self, diameter: Metres) -> f64 {
        debug_assert!(
            diameter.value() > 0.0,
            "a diameter is positive, got {} m",
            diameter.value()
        );
        if diameter >= self.largest {
            return 0.0;
        }
        let x = self.truncation();
        (math::powf(diameter / self.reference, -self.slope) - x) / (1.0 - x)
    }

    /// The diameter over which there are `share` bodies per body over the reference, the inverse
    /// of [`share_above`](Self::share_above): the reference at a share of 1, approaching 0 as the
    /// share grows, and the largest diameter at a share of 0.
    ///
    /// # Panics
    ///
    /// In debug builds, if the share is negative or NaN.
    #[must_use]
    pub fn diameter_at_share(&self, share: f64) -> Metres {
        debug_assert!(share >= 0.0, "a share is not negative, got {share}");
        if share <= 0.0 {
            return self.largest;
        }
        let x = self.truncation();
        self.reference * math::powf(share * (1.0 - x) + x, -1.0 / self.slope)
    }

    /// The law's diameters from `floor` up, as the density ∝ D^(−(q + 1)) on [`floor`, `D_max`]
    /// each event draws one from.
    ///
    /// # Errors
    ///
    /// As [`PowerLaw::new`]: a floor at or above the largest diameter, or not positive.
    fn draws_above(&self, floor: Metres) -> Result<PowerLaw, BuildPowerLawError> {
        PowerLaw::new(self.slope + 1.0, floor.value(), self.largest.value())
    }
}

/// A place on a body's surface: its planetocentric latitude, in [−π ÷ 2, π ÷ 2], and its east
/// longitude, in [0, 2π), in the body's own body-fixed frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct SurfacePoint {
    latitude: Radians,
    longitude: Radians,
}

impl SurfacePoint {
    /// The planetocentric latitude, rad, in [−π ÷ 2, π ÷ 2].
    #[must_use]
    pub const fn latitude(&self) -> Radians {
        self.latitude
    }

    /// The east longitude, rad, in [0, 2π).
    #[must_use]
    pub const fn longitude(&self) -> Radians {
        self.longitude
    }

    /// A point uniform over the sphere, from the stream's next two words: the sine of the latitude
    /// uniform in [−1, 1) from the first, the longitude uniform in [0, 2π) from the second.
    #[must_use]
    fn uniform(stream: &mut Stream) -> Self {
        let sine = 2.0 * stream.uniform() - 1.0;
        let longitude = TAU * stream.uniform();
        Self {
            latitude: Radians::new(math::asin(sine)),
            longitude: Radians::new(longitude),
        }
    }
}

/// The number of events a constant `rate` gives in `window`: the rate times the window's span. A
/// rate times a span, not a draw.
#[must_use]
fn expected_count(rate: EventsPerSecond, window: TimeWindow) -> f64 {
    // In 128-bit nanoseconds, which no window of the clock overflows.
    let nanos = to_nanos(window.end()) - to_nanos(window.start());
    #[expect(
        clippy::cast_precision_loss,
        reason = "a span of at most 2^64 s in nanoseconds, to f64's relative 1.1e-16"
    )]
    let seconds = nanos as f64 / 1e9;
    rate.value() * seconds
}

/// The least share of the period between two events of a monotone phase whose noise passes
/// [`MonotonePhase::new`]'s slope check: 1 ÷ 1.9, since dΦ/dφ is at most 1 + 0.9.
///
/// A phase kind whose events last a smaller share of the period than this never has two running
/// at once, and [`events_about`] holds every event running at an instant.
const LEAST_INTERVAL_SHARE: f64 = 1.0 / (1.0 + MonotonePhase::MAX_SLOPE);

/// The events of `noise` on `series` and `clock` from the cycle before the one Φ is in at `t` to
/// the cycle after it, in cycle order: the margin [`MonotonePhase::events_in`] takes at a window's
/// ends, so that a crossing a nanosecond from `t` is not missed.
///
/// An event that lasts under [`LEAST_INTERVAL_SHARE`] of the period and is running at `t` began
/// at or before `t` and after the crossing of the cycle before Φ(t)'s, so it is among them. Cycles
/// beyond the 40-bit cycle numbers and skipped cycles give none.
fn events_about<'a>(
    noise: &'a MonotonePhase,
    series: &'a EventSeries,
    clock: &'a LinearClock,
    t: UniverseTime,
) -> impl Iterator<Item = CycleEvent> + 'a {
    let now = noise.phase_at(series, clock, t).cycles();
    (now.saturating_sub(1)..=now.saturating_add(1))
        .filter_map(move |cycle| noise.event(series, clock, cycle))
}

/// `share` of `period`, a share in [0, 1), floored to whole nanoseconds.
///
/// # Panics
///
/// Never for a share in [0, 1): such a share of a span is a span.
#[must_use]
fn share_of(period: Span, share: f64) -> Span {
    debug_assert!(
        (0.0..1.0).contains(&share),
        "a share of a period lies in [0, 1), got {share}"
    );
    Span::from_seconds_f64(share * period.as_seconds_f64())
        .expect("a share in [0, 1) of a span is a span")
}

/// `onset` plus `duration`, a span that is not negative, held at the clock's last instant: the
/// end of an event whose onset the bisection saturated at the clock's end.
#[must_use]
fn end_of(onset: UniverseTime, duration: Span) -> UniverseTime {
    onset
        .checked_add(duration)
        .unwrap_or_else(|| from_nanos_saturating(i128::MAX))
}

#[cfg(test)]
mod tests;
