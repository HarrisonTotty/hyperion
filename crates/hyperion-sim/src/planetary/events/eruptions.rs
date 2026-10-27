//! Volcanic eruptions on a body: `body.eruption` (plan 14, P14.T31.a; design note 15), from its
//! onset to its end.
//!
//! # The rate
//!
//! Eruptions start at a rate rising with the body's heat flow F (P14.T24.b's
//! [`GlobalFigures::heat_flow`](crate::planetary::hooks::figures::GlobalFigures::heat_flow), its
//! tidal heat included), and zero below the heat flow at which melt reaches the surface,
//! `F_c` = [`SURFACE_MELT_HEAT_FLOW`](crate::planetary::params::SURFACE_MELT_HEAT_FLOW) (0.025 W
//! m⁻²), as the present outgassing is:
//!
//! > Γ = Γ⊕ × O(F) × (R ÷ R⊕)², O(F) = max(0, (F − `F_c`) ÷ (F⊕ − `F_c`)),
//!
//! with O P14.T24.g's present outgassing per unit area over Earth's ([`outgassing`], 1 at the
//! generator's Earth,
//! [`GENERATED_EARTH_HEAT_FLOW`](crate::planetary::hooks::figures::interior::GENERATED_EARTH_HEAT_FLOW)),
//! the body's area over Earth's, and Γ⊕ =
//! [`EARTH_ERUPTIONS_PER_YEAR`], the eruptions that start on Earth in a year. An eruption is a
//! volcano's eruptive episode, as the Smithsonian's catalogue counts one.
//!
//! The bins hold at most [`PoissonBins::MAX_MEAN_PER_BIN`] onsets a week, about 3,300 a year,
//! some 95 Earths' worth. A body hotter than that, a lava world, lists onsets at that rate
//! ([`EruptionModel::is_saturated`]), which is recorded rather than hidden.
//!
//! # Durations
//!
//! Each eruption lasts a log-normal time of median [`MEDIAN_DURATION_SECONDS`] (7 weeks) and
//! log-width [`DURATION_LOG_SIGMA`], cut at [`MAX_DURATION`] (10 Julian years). An eruption that
//! would run longer, a volcano's persistent activity, is not modelled as one event: about 2% of
//! Earth's, so that some 27 eruptions run at once (Γ⊕ times the cut duration's mean, 0.77 years)
//! where the Global Volcanism Program's database has 37–44.
//!
//! Γ⊕ and σ are the Global Volcanism Program database's own figures, read for the science check
//! of P14.T31.a: 34.9–36.5 onsets a year, 16.8% of eruptions over a year and 1.9% over ten, about
//! Simkin's median (the database's eruptions of 1900–2015 give 53 days). decision-p14-t31a-events
//! ruled 40, 49 days and 1.6, fitted to about 20 running at once from memory; main accepted the
//! database's figures as a deviation (plan 14's Risks).
//!
//! # Time
//!
//! Poisson bins of [`BIN_SECONDS`] = one week, looking back [`LOOK_BACK_BINS`] = 522 bins, the
//! whole weeks that hold [`MAX_DURATION`], so that [`eruptions_active_at`] misses none. The rate
//! is constant: its inputs are read once, at the epoch.
//!
//! # Marks
//!
//! From each eruption's own stream, [`BinEvent::marks`], at word 0: its duration's standard
//! normal, words 0 and 1. Words 2 and 3 are reserved for its place on the body, and 4–7 for its
//! size.

use std::error::Error;
use std::fmt;

use super::expected_count;
use crate::Seed;
use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
};
use crate::id::BodyId;
use crate::id::event_tags::BODY_ERUPTION;
use crate::math;
use crate::planetary::hooks::figures::outgassing;
use crate::time::{Span, UniverseTime};
use crate::units::consts::EARTH_RADIUS_M;
use crate::units::{Metres, WattsPerSquareMetre};

/// The length of an eruption bin, s: one week, 604,800 s.
pub const BIN_SECONDS: i64 = 7 * 86_400;

/// The longest an eruption lasts: 10 Julian years, 315,576,000 s. Part of the generator version.
pub const MAX_DURATION: Span = Span::from_seconds(10 * 31_557_600);

/// How many whole bins [`eruptions_active_at`] looks back: 522, the fewest weeks that hold
/// [`MAX_DURATION`] (521.8 weeks).
pub const LOOK_BACK_BINS: u32 = 522;

/// Γ⊕, the eruptions that start on Earth in a year: 35. Part of the generator version.
///
/// The Smithsonian Institution's Global Volcanism Program's database of confirmed eruptions
/// (*Volcanoes of the World*, read through its web feature service for the science check of
/// P14.T31.a) has 34.9 starting a year over 1980–2019 and 36.5 over 2000–2024, while 66–72
/// volcanoes erupt in a year (Siebert, Simkin and Kimberly 2010, *Volcanoes of the World*, 3rd
/// ed.: 50–70).
pub const EARTH_ERUPTIONS_PER_YEAR: f64 = 35.0;

/// The median duration of an eruption, s: 7 weeks, 4,233,600 s (Simkin 1993, Annu. Rev. Earth
/// Planet. Sci. 21, 427: the median of the dated eruptions of the Smithsonian's catalogue; its
/// 2,322 eruptions of 1900–2015 dated to the day give 53 days). Part of the generator version.
pub const MEDIAN_DURATION_SECONDS: f64 = 49.0 * 86_400.0;

/// The log-width σ of an eruption's duration, in natural-log units: 2.08. Part of the generator
/// version.
///
/// Fitted to the Global Volcanism Program's dated eruptions of 1900–2015 (the science check of
/// P14.T31.a): 16.8% last over a year, which about the 7-week median gives σ = 2.09, and 1.9% over
/// ten years, which gives 2.08.
pub const DURATION_LOG_SIGMA: f64 = 2.08;

/// [`BIN_SECONDS`] as a float, exactly.
const BIN_SECONDS_F64: f64 = 604_800.0;

/// The eruption bins: a week long, looking back [`LOOK_BACK_BINS`].
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("a week lies between 16 s and 2^44 s")
}

/// What an [`EruptionModel`] reads of a body: its heat flow and its radius.
///
/// Plain data with public fields: [`EruptionModel::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EruptionInputs {
    /// Its mean surface heat flow, W m⁻², finite and non-negative: its global figures', tidal heat
    /// included
    /// ([`GlobalFigures::heat_flow`](crate::planetary::hooks::figures::GlobalFigures::heat_flow)).
    pub heat_flow: WattsPerSquareMetre,
    /// Its mean radius, m, finite and positive: its bulk section's.
    pub radius: Metres,
}

/// An [`EruptionModel`] could not be built, with the value refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildEruptionModelError {
    /// The heat flow is not finite and non-negative.
    HeatFlow(WattsPerSquareMetre),
    /// The radius is not finite and positive.
    Radius(Metres),
}

impl fmt::Display for BuildEruptionModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeatFlow(flow) => write!(
                f,
                "heat flow {} W m⁻² is not finite and non-negative",
                flow.value()
            ),
            Self::Radius(r) => write!(f, "radius {} m is not finite and positive", r.value()),
        }
    }
}

impl Error for BuildEruptionModelError {}

/// The eruptions of one body: its series, its inputs and its constant rate of onsets (see the
/// [module documentation](self)).
///
/// It is the [`RateModel`] of the body's `body.eruption` series, whose bound and rate are both
/// [`listed_rate`](Self::listed_rate) at every instant.
///
/// # Examples
///
/// An Earth erupts some 35 times a year, a Mars not at all:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::events::{EruptionInputs, EruptionModel};
/// use hyperion_sim::units::{Metres, WattsPerSquareMetre};
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let earth = EruptionInputs {
///     heat_flow: WattsPerSquareMetre::new(0.0925),
///     radius: Metres::new(6.371e6),
/// };
/// let model = EruptionModel::new(Seed::new(7), BodyId::new(system, 0x0103), &earth)?;
/// assert!((model.listed_rate().value() * 31_557_600.0 - 35.0).abs() < 0.1);
/// let mars = EruptionInputs {
///     heat_flow: WattsPerSquareMetre::new(0.022),
///     radius: Metres::new(3.3895e6),
/// };
/// let model = EruptionModel::new(Seed::new(7), BodyId::new(system, 0x0104), &mars)?;
/// assert_eq!(model.listed_rate().value(), 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EruptionModel {
    series: EventSeries,
    inputs: EruptionInputs,
    outgassing: f64,
    law: EventsPerSecond,
    rate: EventsPerSecond,
}

impl EruptionModel {
    /// The eruptions of `body` in the universe of `seed`, from `inputs`: the series of
    /// [`BODY_ERUPTION`] with the body's ID as subject.
    ///
    /// # Errors
    ///
    /// [`BuildEruptionModelError::HeatFlow`] unless the heat flow is finite and non-negative;
    /// [`BuildEruptionModelError::Radius`] unless the radius is finite and positive.
    pub fn new(
        seed: Seed,
        body: BodyId,
        inputs: &EruptionInputs,
    ) -> Result<Self, BuildEruptionModelError> {
        let flow = inputs.heat_flow.value();
        if !(flow.is_finite() && flow >= 0.0) {
            return Err(BuildEruptionModelError::HeatFlow(inputs.heat_flow));
        }
        let radius = inputs.radius.value();
        if !(radius.is_finite() && radius > 0.0) {
            return Err(BuildEruptionModelError::Radius(inputs.radius));
        }
        let outgassing = outgassing(inputs.heat_flow);
        let area = (radius / EARTH_RADIUS_M) * (radius / EARTH_RADIUS_M);
        let law = EventsPerSecond::per_julian_year(EARTH_ERUPTIONS_PER_YEAR * outgassing * area);
        let rate = EventsPerSecond::new(
            law.value()
                .min(PoissonBins::MAX_MEAN_PER_BIN / BIN_SECONDS_F64),
        );
        Ok(Self {
            series: EventSeries::new(seed, BODY_ERUPTION, body.into()),
            inputs: *inputs,
            outgassing,
            law,
            rate,
        })
    }

    /// The series: the body's subject, [`BODY_ERUPTION`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The inputs it was built from.
    #[must_use]
    pub const fn inputs(&self) -> &EruptionInputs {
        &self.inputs
    }

    /// O(F), the body's present outgassing per unit area over Earth's ([`outgassing`]): 0 at or
    /// below [`SURFACE_MELT_HEAT_FLOW`](crate::planetary::params::SURFACE_MELT_HEAT_FLOW).
    #[must_use]
    pub const fn outgassing(&self) -> f64 {
        self.outgassing
    }

    /// Γ, the law's rate of onsets, events per second, before the bins' limit.
    #[must_use]
    pub const fn law_rate(&self) -> EventsPerSecond {
        self.law
    }

    /// The rate of the onsets listed, events per second: the law's, held at the bins' limit of
    /// [`PoissonBins::MAX_MEAN_PER_BIN`] a week.
    #[must_use]
    pub const fn listed_rate(&self) -> EventsPerSecond {
        self.rate
    }

    /// Whether the law's rate exceeds the bins' limit, so that fewer onsets are listed than it
    /// gives: a lava world.
    #[must_use]
    pub fn is_saturated(&self) -> bool {
        self.law.value() > self.rate.value()
    }

    /// The number of onsets expected in `window`: the listed rate times the window's span, not a
    /// draw.
    #[must_use]
    pub fn expected_count(&self, window: TimeWindow) -> f64 {
        expected_count(self.rate, window)
    }
}

impl RateModel for EruptionModel {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.rate
    }
}

/// One eruption on a body: its ID, its onset and its duration (see the [module
/// documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Eruption {
    id: EventId,
    onset: UniverseTime,
    duration: Span,
    end: UniverseTime,
}

impl Eruption {
    /// The eruption's ID, from which [`eruption`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// When it starts: its event's time.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// How long it lasts: positive and at most [`MAX_DURATION`].
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// When it ends, the onset plus the duration: the first instant it is no longer running.
    #[must_use]
    pub const fn end(&self) -> UniverseTime {
        self.end
    }

    /// Whether it is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end
    }
}

/// The duration of an eruption whose duration's standard normal is `z`: the median ×
/// e^(σ z), cut at [`MAX_DURATION`], in whole nanoseconds.
#[must_use]
fn duration_of(z: f64) -> Span {
    let seconds = (MEDIAN_DURATION_SECONDS * math::exp(DURATION_LOG_SIGMA * z))
        .min(MAX_DURATION.as_seconds_f64());
    Span::from_seconds_f64(seconds).expect("a duration of at most 10 years is in range")
}

/// The eruption of `event`, its duration drawn from its own stream.
#[must_use]
fn eruption_of(event: BinEvent) -> Eruption {
    let (id, onset) = (event.id(), event.time());
    let duration = duration_of(event.into_marks().standard_normal());
    Eruption {
        id,
        onset,
        duration,
        end: onset
            .checked_add(duration)
            .expect("an onset inside the bins lies at least 10 years inside the clock's range"),
    }
}

/// Appends to `out` the body's eruptions whose onset lies in `window`, sorted by onset and then by
/// ID.
///
/// Every bin the window touches is generated whole, so a listing over a window is the union of the
/// listings over any partition of it, in any order of asking. `out` is only appended to.
pub fn eruptions(model: &EruptionModel, window: TimeWindow, out: &mut Vec<Eruption>) {
    let mut events = Vec::new();
    bins().events_in(model.series(), window, model, &mut events);
    out.extend(events.into_iter().map(eruption_of));
}

/// Appends to `out` the body's eruptions running at `t`, sorted by onset and then by ID.
///
/// They are those with an onset from the start of the bin [`LOOK_BACK_BINS`] before `t`'s up to
/// `t` whose end is after `t`; every eruption lasts at most [`MAX_DURATION`], which the look-back
/// holds, so none is missed. The result depends on the model and `t` alone.
pub fn eruptions_active_at(model: &EruptionModel, t: UniverseTime, out: &mut Vec<Eruption>) {
    let mut events = Vec::new();
    bins().active_at(model.series(), t, model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(eruption_of)
            .filter(|eruption| eruption.is_active_at(t)),
    );
}

/// The eruption `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject) or names a
/// number beyond its bin's count.
#[must_use]
pub fn eruption(model: &EruptionModel, id: EventId) -> Option<Eruption> {
    bins().event(model.series(), id, model).map(eruption_of)
}

#[cfg(test)]
mod tests;
