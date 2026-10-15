//! The central black hole's X-ray flares: Sgr A*-like flares, about one a day, each with a mean
//! luminosity, a duration, an energy and a light curve (plan 09, P09.T30.a).
//!
//! # The band
//!
//! Every luminosity here is the unabsorbed 2–10 keV X-ray luminosity, in watts (erg/s in the
//! sources: 1 W = 10⁷ erg/s), and every energy the 2–10 keV energy, in joules (erg in the
//! sources). A flare's mean luminosity is the one Neilsen et al. (2013, ApJ 774, 42; section 3 and
//! Table 1's note) give each of their 39 Chandra HETGS flares of the 2012 campaign (3 Ms): its
//! pileup-corrected mean unabsorbed 2–10 keV luminosity, the fluence over the duration. Its
//! luminosity at t ([`CentreFlare::luminosity_watts_at`]) is the flare's own, above whatever the
//! black hole emits in quiescence, which P09.T30.b adds.
//!
//! Not modelled: the near-infrared and submillimetre counterparts (every X-ray flare has a
//! near-infrared one, and near-infrared flares are about four times as frequent: Ponti et al.
//! 2015, MNRAS 454, 1525, section 1; GRAVITY Collaboration 2020, A&A 638, A2, section 4, and
//! Witzel et al. 2018, ApJ 863, 15, describe the near infrared as a flux distribution, not as a
//! rate above a threshold), flares below the threshold or shorter than Neilsen et al.'s
//! 400 s selection limit, and any dependence of the flares on the black hole's mass. Plan 09
//! records each as a deferred finding (P09.T30.a).
//!
//! # The rate
//!
//! A constant [`RATE_PER_DAY`] = 1.1 flares a day, 401.775 a Julian year, of flares whose mean
//! luminosity is at least [`MIN_MEAN_LUMINOSITY_WATTS`] = 10³⁴ erg/s: Neilsen et al.'s (section 5)
//! "1.1 (+0.2 −0.1) flares per day above a luminosity of 10³⁴ erg s⁻¹". The threshold is the
//! luminosity law's floor, so the rate is the rate of exactly the flares drawn. It is constant in
//! time, as Ponti et al. (2015, section 5.4) find the bright flares' rate from 1999 to the summer
//! of 2014, and the same for every galaxy's black hole, though Neilsen et al. measured Sgr A*
//! alone.
//!
//! # Time
//!
//! Flares are plan 06's Poisson bins of [`BIN_SECONDS`] = one day that look back
//! [`LOOK_BACK_BINS`] = one bin (Design note 15), on the black hole's own event key
//! ([`subject`](super::subject)) under [`CENTRE_FLARE`]. The bound is the constant rate, so a bin's
//! mean is 1.1, far under [`PoissonBins::MAX_MEAN_PER_BIN`], and nothing is thinned. Every flare
//! lasts at most [`MAX_DURATION`], under the look-back, so [`flares_active_at`] finds every flare
//! still running.
//!
//! # Marks
//!
//! Each flare's marks come from its own stream, [`BinEvent::marks`], so marks never move a time.
//! The layout is fixed:
//!
//! - **word 0:** the mean luminosity, by inverse transform of Neilsen et al.'s (Table 2) fitted
//!   power law dN/dL ∝ L^−1.9 on [10³⁴, 1.925 × 10³⁵] erg/s, from one uniform in [0, 1);
//! - **word 1:** the duration, by inverse transform of their dN/dT ∝ T^−0.9 on [400, 8,175] s,
//!   from one uniform in [0, 1).
//!
//! The two are drawn independently, and the energy is their product, E = L̄ × T, since the mean
//! luminosity is the fluence over the duration. Neilsen et al. (section 4.2) find the fluence
//! correlated with the peak rate (ρ ≈ 0.89) and with the duration (ρ ≈ 0.54), and the peak rate
//! and the duration almost uncorrelated (ρ ≈ 0.27); section 5 reads the flares' properties as
//! following "from the independence of the flare intensity on duration". The energies run from
//! 4.0 × 10³⁶ to 1.574 × 10³⁹ erg. Scatter added later takes words 2, 3, … in a fixed order.
//!
//! # The light curve
//!
//! Neilsen et al.'s own flare shape (section 3): a Gaussian in time, whose start and stop are its
//! 2σ limits, so that a flare lasts 4σ. Here it is centred at the onset plus half the duration with
//! σ a quarter of the duration, its value at ±2σ is subtracted so that it is continuous and zero
//! outside the flare, and it is scaled so that ∫ ΔL dt over the flare is the energy. Its peak is
//! then [`peak_over_mean`] = 1.8683 times the mean luminosity, so no flare's peak exceeds
//! [`MAX_PEAK_LUMINOSITY_WATTS`], 3.5965 × 10³⁵ erg/s. One Gaussian is one flare: Neilsen et al.
//! fit each subflare with σ of 100–1,600 s and a flare of several subflares as their sum, so a
//! flare here longer than 6,400 s, whose σ exceeds 1,600 s, stands for what they saw as several.
//!
//! The rate, the two laws' slopes and ranges, the cap on the duration and the light-curve template
//! are part of the generator version (plan 09, "Generator version"): a change to any of them moves
//! generated output. An event may change what a console reads, never what the centre is (plan 06,
//! Design note 14): nothing here changes a member.

use std::f64::consts::{PI, SQRT_2};

use crate::Seed;
use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
};
use crate::galaxy::Galaxy;
use crate::id::event_tags::CENTRE_FLARE;
use crate::math;
use crate::time::{Span, UniverseTime};

/// The length of a flare bin, s: one day (plan 09, P09.T30.a, as plan 06's star flares).
pub const BIN_SECONDS: i64 = 86_400;

/// How many whole bins [`flares_active_at`] looks back: one, a day, longer than any flare
/// ([`MAX_DURATION`]).
pub const LOOK_BACK_BINS: u32 = 1;

/// The rate of flares whose mean luminosity is at least [`MIN_MEAN_LUMINOSITY_WATTS`], per day of
/// 86,400 s: 1.1, constant in time and the same for every galaxy's black hole. Part of the
/// generator version.
///
/// Neilsen et al. (2013, ApJ 774, 42, section 5): "a flare frequency of 1.1 (+0.2 −0.1) flares per
/// day above a luminosity of 10³⁴ erg s⁻¹", the mean unabsorbed 2–10 keV luminosity of section 3
/// and Table 1's note. It is 401.775 a Julian year. Ponti et al.'s (2015, MNRAS 454, 1525, Table
/// 3) count of the same 2012 data, 1.08 ± 0.10 a day, agrees.
pub const RATE_PER_DAY: f64 = 1.1;

/// The least mean luminosity of a flare, W: 10³⁴ erg/s, unabsorbed 2–10 keV. Part of the generator
/// version.
///
/// It is the threshold of Neilsen et al.'s (2013, section 5) rate, [`RATE_PER_DAY`], and so the
/// floor of the luminosity law: the rate is the rate of exactly the flares drawn.
pub const MIN_MEAN_LUMINOSITY_WATTS: f64 = 1e27;

/// The greatest mean luminosity of a flare, W: 1.925 × 10³⁵ erg/s, unabsorbed 2–10 keV. Part of
/// the generator version.
///
/// It is where Neilsen et al.'s (2013, section 4.3) test of the fitted law's extrapolation starts,
/// "at our highest bins", just above their brightest flare's 1.92 × 10³⁵ erg/s (Table 1): above
/// it the extrapolated law predicts luminous flares that were not seen.
pub const MAX_MEAN_LUMINOSITY_WATTS: f64 = 1.925e28;

/// Γ of the mean luminosity's law, dN/dL ∝ L^Γ: −1.9 (+0.3 −0.4) (Neilsen et al. 2013, Table 2).
/// Part of the generator version.
pub const LUMINOSITY_SLOPE: f64 = -1.9;

/// The shortest flare, s: 400, Neilsen et al.'s (2013, section 3) "shortest allowed time", a
/// selection limit and not a physical one. Part of the generator version.
pub const MIN_DURATION_SECONDS: f64 = 400.0;

/// The longest flare, s: 8,175, where Neilsen et al.'s (2013, section 4.3) test of the fitted
/// law's extrapolation in duration starts, "at our highest bins"; their longest flare lasted about
/// 7,800 s (section 3). Part of the generator version.
pub const MAX_DURATION_SECONDS: f64 = 8_175.0;

/// Γ of the duration's law, dN/dT ∝ T^Γ: −0.9 ± 0.2 (Neilsen et al. 2013, Table 2). Part of the
/// generator version.
pub const DURATION_SLOPE: f64 = -0.9;

/// The longest a flare lasts: [`MAX_DURATION_SECONDS`], 8,175 s, under the look-back of a day, so
/// that [`flares_active_at`] finds every flare still running.
pub const MAX_DURATION: Span = Span::from_seconds(8_175);

/// A bound on every flare's peak luminosity, W: 3.5965 × 10²⁸ W, 3.5965 × 10³⁵ erg/s, unabsorbed
/// 2–10 keV.
///
/// It is [`peak_over_mean`] times [`MAX_MEAN_LUMINOSITY_WATTS`], 3.596474 × 10²⁸ W, rounded up at
/// the fifth figure, so that no flare exceeds it. The brightest flare Neilsen et al. (2013, section
/// 5) saw peaked near 5 × 10³⁵ erg/s, with four subflares (Table 1), which one template per flare
/// does not reach.
pub const MAX_PEAK_LUMINOSITY_WATTS: f64 = 3.5965e28;

/// How many σ of the Gaussian lie each side of its centre within the flare: 2, Neilsen et al.'s
/// (2013, section 3) start and stop. Part of the generator version.
const EDGE_SIGMAS: f64 = 2.0;

// No flare outlives the look-back: one that starts before its first bin ends before the bin that
// holds the instant asked about.
const _: () = assert!(LOOK_BACK_BINS == 1 && MAX_DURATION.seconds() < BIN_SECONDS);

/// The flare bins: a day long, looking back one.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("a day lies between 16 s and 2^44 s")
}

/// The quantile at `u` of the truncated power law dN/dx ∝ x^slope on [lo, hi], slope ≠ −1:
/// x = lo × [1 + u ((hi ÷ lo)^(1 + slope) − 1)]^(1 ÷ (1 + slope)), held to [lo, hi].
#[must_use]
fn power_law_quantile(u: f64, lo: f64, hi: f64, slope: f64) -> f64 {
    debug_assert!((0.0..1.0).contains(&u), "a uniform lies in [0, 1), got {u}");
    let k = 1.0 + slope;
    let span = math::powf(hi / lo, k) - 1.0;
    (lo * math::powf(1.0 + u * span, 1.0 / k)).clamp(lo, hi)
}

/// The mean luminosity of the flare whose word 0 is `u`, W.
#[must_use]
pub(super) fn mean_luminosity_watts_at(u: f64) -> f64 {
    power_law_quantile(
        u,
        MIN_MEAN_LUMINOSITY_WATTS,
        MAX_MEAN_LUMINOSITY_WATTS,
        LUMINOSITY_SLOPE,
    )
}

/// The duration of the flare whose word 1 is `u`, s.
#[must_use]
pub(super) fn duration_seconds_at(u: f64) -> f64 {
    power_law_quantile(
        u,
        MIN_DURATION_SECONDS,
        MAX_DURATION_SECONDS,
        DURATION_SLOPE,
    )
}

/// The template's value at its edges, e^−2: the Gaussian at ±2σ, subtracted from it.
#[must_use]
fn pedestal() -> f64 {
    math::exp(-0.5 * EDGE_SIGMAS * EDGE_SIGMAS)
}

/// The template's area over the flare, in σ times the Gaussian's amplitude before its pedestal is
/// subtracted: √(2π) erf(√2) − 4 e^−2 = 1.851235. The template's peak is then 1 − e^−2 times that
/// amplitude.
#[must_use]
fn template_area_sigmas() -> f64 {
    (2.0 * PI).sqrt() * math::erf(EDGE_SIGMAS / SQRT_2) - 2.0 * EDGE_SIGMAS * pedestal()
}

/// A flare's peak luminosity over its mean luminosity: 1.868298, the same for every flare. Part of
/// the generator version.
///
/// A flare of mean luminosity L̄ and duration T = 4σ radiates E = L̄ × 4σ. The template, a
/// Gaussian of amplitude A with its value at ±2σ subtracted, has an area of 1.851235 σA and a peak
/// of A (1 − e^−2). So the peak is 4 (1 − e^−2) ÷ 1.851235 = 1.8683 times L̄, against 1.6718 for
/// a plain Gaussian cut at ±2σ.
#[must_use]
pub fn peak_over_mean() -> f64 {
    2.0 * EDGE_SIGMAS * (1.0 - pedestal()) / template_area_sigmas()
}

/// The X-ray flares of the central black hole of one universe: their series, at a constant rate
/// (see the [module documentation](self)).
///
/// It is the [`RateModel`] of the black hole's `centre.ev.flare` series, whose bound and rate are
/// both [`RATE_PER_DAY`] at every instant. The flares read nothing of the galaxy but its seed.
///
/// # Examples
///
/// About 401.8 flares are expected in a Julian year:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::features::centre::events::CentreFlareModel;
///
/// let model = CentreFlareModel::new(Seed::new(7));
/// let per_year = model.listed_rate().value() * 31_557_600.0;
/// assert!((per_year - 401.775).abs() < 1e-9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CentreFlareModel {
    series: EventSeries,
}

impl CentreFlareModel {
    /// The flares of the central black hole in the universe of `seed`: the series of
    /// [`CENTRE_FLARE`] with the black hole's own ID as subject (Design note 23).
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self {
            series: EventSeries::new(seed, CENTRE_FLARE, super::subject()),
        }
    }

    /// The flares of `galaxy`'s central black hole: [`new`](Self::new) at the galaxy's seed.
    #[must_use]
    pub fn of(galaxy: &Galaxy) -> Self {
        Self::new(galaxy.seed())
    }

    /// The series: the black hole's subject, [`CENTRE_FLARE`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The rate of listed flares, [`RATE_PER_DAY`] in events per second: 1.27 × 10⁻⁵ s⁻¹.
    #[must_use]
    pub fn listed_rate(&self) -> EventsPerSecond {
        EventsPerSecond::per_day(RATE_PER_DAY)
    }
}

impl RateModel for CentreFlareModel {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.listed_rate()
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.listed_rate()
    }
}

/// The flare of `event`, an event of a [`CentreFlareModel`]'s series: its mean luminosity from word
/// 0 of its marks, its duration from word 1, and its energy and peak from the two.
///
/// # Panics
///
/// In debug builds, if the duration exceeds [`MAX_DURATION`], which the law's range rules out.
#[must_use]
fn flare_of(event: BinEvent) -> CentreFlare {
    let id = event.id();
    let onset = event.time();
    let mut marks = event.into_marks();
    let mean_luminosity_watts = mean_luminosity_watts_at(marks.uniform());
    let duration_seconds = duration_seconds_at(marks.uniform());
    let duration = Span::from_seconds_f64(duration_seconds).expect("a flare lasts at most 8,175 s");
    debug_assert!(
        duration <= MAX_DURATION,
        "a flare lasts {duration}, beyond the longest, {MAX_DURATION}"
    );
    CentreFlare {
        id,
        onset,
        mean_luminosity_watts,
        duration_seconds,
        duration,
        energy_joules: mean_luminosity_watts * duration_seconds,
        peak_luminosity_watts: mean_luminosity_watts * peak_over_mean(),
    }
}

/// One X-ray flare of the central black hole: its ID, onset, mean luminosity, duration, energy and
/// peak, and its light curve (see the [module documentation](self)).
///
/// Luminosities are unabsorbed 2–10 keV, in watts, and the energy in joules.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CentreFlare {
    id: EventId,
    onset: UniverseTime,
    mean_luminosity_watts: f64,
    duration_seconds: f64,
    duration: Span,
    energy_joules: f64,
    peak_luminosity_watts: f64,
}

impl CentreFlare {
    /// The flare's ID, from which [`flare`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset, the template's start, 2σ before its peak.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The mean luminosity, W (erg/s in the sources: 1 W = 10⁷ erg/s), unabsorbed 2–10 keV: the
    /// energy over the duration, from [`MIN_MEAN_LUMINOSITY_WATTS`] up to
    /// [`MAX_MEAN_LUMINOSITY_WATTS`].
    #[must_use]
    pub const fn mean_luminosity_watts(&self) -> f64 {
        self.mean_luminosity_watts
    }

    /// The duration as drawn, s: from [`MIN_DURATION_SECONDS`] up to [`MAX_DURATION_SECONDS`].
    #[must_use]
    pub const fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    /// The duration from the onset to the end, [`duration_seconds`](Self::duration_seconds)
    /// floored to the nanosecond, at most [`MAX_DURATION`].
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// The first instant after the flare, its onset plus its duration.
    ///
    /// # Panics
    ///
    /// If that lies beyond the universe clock, which no flare listed inside it reaches.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        self.onset
            .checked_add(self.duration)
            .expect("a flare ends within 8,175 s of an onset the clock holds")
    }

    /// The energy radiated, J (erg in the sources: 1 J = 10⁷ erg), unabsorbed 2–10 keV: the mean
    /// luminosity times the duration as drawn, from 4.0 × 10²⁹ J (4.0 × 10³⁶ erg) up to
    /// 1.574 × 10³² J (1.574 × 10³⁹ erg).
    #[must_use]
    pub const fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    /// The peak luminosity, W, unabsorbed 2–10 keV, at the middle of the flare: [`peak_over_mean`]
    /// times the mean luminosity, at most [`MAX_PEAK_LUMINOSITY_WATTS`].
    #[must_use]
    pub const fn peak_luminosity_watts(&self) -> f64 {
        self.peak_luminosity_watts
    }

    /// Whether the flare is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        t.checked_since(self.onset)
            .is_some_and(|elapsed| !elapsed.is_negative() && elapsed < self.duration)
    }

    /// The flare's own luminosity at `t`, ΔL in W, unabsorbed 2–10 keV, above the black hole's
    /// quiescent emission (P09.T30.b's).
    ///
    /// It is 0 before the onset and from the end, and in between
    ///
    /// > peak × (e^(−x² ÷ 2) − e^−2) ÷ (1 − e^−2), x = (t − onset − T ÷ 2) ÷ σ, σ = T ÷ 4,
    ///
    /// Neilsen et al.'s (2013, section 3) Gaussian with its value at ±2σ subtracted: continuous in
    /// `t`, its peak at the middle of the flare, and its integral over the flare the energy.
    #[must_use]
    pub fn luminosity_watts_at(&self, t: UniverseTime) -> f64 {
        let Some(elapsed) = t.checked_since(self.onset) else {
            return 0.0;
        };
        if elapsed.is_negative() || elapsed >= self.duration {
            return 0.0;
        }
        let sigma = self.duration_seconds / (2.0 * EDGE_SIGMAS);
        let x = (elapsed.as_seconds_f64() - 0.5 * self.duration_seconds) / sigma;
        let floor = pedestal();
        let level = (math::exp(-0.5 * x * x) - floor) / (1.0 - floor);
        if level > 0.0 {
            self.peak_luminosity_watts * level
        } else {
            0.0
        }
    }
}

/// Appends to `out` the black hole's flares with onsets in `window`, sorted by onset and then by
/// ID.
///
/// Every bin the window touches is generated whole, so a listing over a window is the union of the
/// listings over any partition of it, in any order of asking. `out` is only appended to; nothing in
/// it is read.
///
/// # Examples
///
/// A week of flares, and the brightest of them regenerated from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::features::centre::events::{CentreFlareModel, flare, flares};
/// use hyperion_sim::time::{Span, UniverseTime};
///
/// let model = CentreFlareModel::new(Seed::new(7));
/// let week_end = UniverseTime::EPOCH.checked_add(Span::from_seconds(7 * 86_400)).ok_or("in range")?;
///
/// let mut week = Vec::new();
/// flares(&model, TimeWindow::new(UniverseTime::EPOCH, week_end)?, &mut week);
/// // About 1.1 a day above 10³⁴ erg/s (10²⁷ W).
/// assert!((1..25).contains(&week.len()));
/// let brightest = week
///     .iter()
///     .max_by(|a, b| a.peak_luminosity_watts().total_cmp(&b.peak_luminosity_watts()))
///     .ok_or("a week has a flare")?;
/// assert_eq!(flare(&model, brightest.id()).as_ref(), Some(brightest));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn flares(model: &CentreFlareModel, window: TimeWindow, out: &mut Vec<CentreFlare>) {
    let mut events = Vec::new();
    bins().events_in(model.series(), window, model, &mut events);
    out.extend(events.into_iter().map(flare_of));
}

/// Appends to `out` the black hole's flares running at `t`, sorted by onset and then by ID.
///
/// They are those with an onset from the start of the bin before `t`'s up to `t` whose end is
/// after `t`; every flare lasts under the look-back, so none is missed. The result depends on the
/// model and `t` alone, never on what was asked before.
pub fn flares_active_at(model: &CentreFlareModel, t: UniverseTime, out: &mut Vec<CentreFlare>) {
    let mut events = Vec::new();
    bins().active_at(model.series(), t, model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(flare_of)
            .filter(|flare| flare.is_active_at(t)),
    );
}

/// The flare `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject) or names a
/// number beyond its bin's count.
#[must_use]
pub fn flare(model: &CentreFlareModel, id: EventId) -> Option<CentreFlare> {
    bins().event(model.series(), id, model).map(flare_of)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, ks_one_sample, poisson_interval,
    };

    use super::*;
    use crate::events::testing::{ConstantRate, assert_partition_independent, partition};
    use crate::events::{EventBin, event_id};
    use crate::id::{SystemId, event_tags};
    use crate::time::{ClockWindow, SourceHorizon};
    use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR};

    const SEED: Seed = Seed::new(0x0e7e_0009_0030_000a);
    const DAY: i64 = BIN_SECONDS;

    fn model() -> CentreFlareModel {
        CentreFlareModel::new(SEED)
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn days(from: i64, to: i64) -> TimeWindow {
        TimeWindow::new(at(from * DAY, 0), at(to * DAY, 0)).unwrap()
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    fn list(model: &CentreFlareModel, w: TimeWindow) -> Vec<CentreFlare> {
        let mut out = Vec::new();
        flares(model, w, &mut out);
        out
    }

    fn running(model: &CentreFlareModel, t: UniverseTime) -> Vec<CentreFlare> {
        let mut out = Vec::new();
        flares_active_at(model, t, &mut out);
        out
    }

    /// The CDF of the truncated power law dN/dx ∝ x^slope on [lo, hi].
    fn power_law_cdf(x: f64, lo: f64, hi: f64, slope: f64) -> f64 {
        let k = 1.0 + slope;
        let x = x.clamp(lo, hi);
        (math::powf(x / lo, k) - 1.0) / (math::powf(hi / lo, k) - 1.0)
    }

    /// The plan's figure, as §3.5 item 2 of issue #20's brief rules it: Neilsen et al.'s 1.1 a day,
    /// 401.775 flares expected in a Julian year (1.1 × 365.25), the rate times the span.
    #[test]
    fn a_julian_year_expects_401_775_flares() {
        let model = model();
        let expected = model.listed_rate().value() * SECONDS_PER_JULIAN_YEAR;
        assert!(
            (expected / 401.775 - 1.0).abs() < 1e-12,
            "{expected} flares a year"
        );
        assert!((expected / (RATE_PER_DAY * 365.25) - 1.0).abs() < 1e-12);
        let mean_per_bin = model.listed_rate().value() * SECONDS_PER_DAY;
        assert!((mean_per_bin - 1.1).abs() < 1e-12, "{mean_per_bin}");
    }

    /// The bound holds at every instant: the rate is the bound in every bin, across the source
    /// horizon, and a bin's mean is far under 64. The law is the same for every black hole, so a
    /// sweep of masses has nothing to vary here; the disruptions' bound test sweeps them.
    #[test]
    fn the_bound_holds_across_the_source_horizon() {
        let model = model();
        let bins = bins();
        for t in [
            SourceHorizon::START,
            ClockWindow::START,
            UniverseTime::EPOCH,
            ClockWindow::END,
        ] {
            let window = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
            let bound = model.bound(window);
            assert!(bound.value() * SECONDS_PER_DAY <= PoissonBins::MAX_MEAN_PER_BIN);
            for fraction in [0.0, 0.25, 0.5, 0.999_999] {
                let instant = after(window.start(), fraction * SECONDS_PER_DAY);
                assert!(model.rate(instant) <= bound, "{instant}");
            }
        }
    }

    /// The law's figures that issue #20's brief (§3.5 items 2 and 3) checks the built law against,
    /// computed from the built constants: the share above 10³⁵ erg/s, the mean duration and the duty
    /// cycle, the energy range, the share above 10³⁸ erg and the peak bound.
    #[test]
    fn the_law_reproduces_neilsen_et_al_s_figures() {
        let luminosity_cdf = |x: f64| {
            power_law_cdf(
                x,
                MIN_MEAN_LUMINOSITY_WATTS,
                MAX_MEAN_LUMINOSITY_WATTS,
                LUMINOSITY_SLOPE,
            )
        };
        let bright = 1.0 - luminosity_cdf(1e28);
        assert!((bright - 0.0603).abs() < 5e-5, "{bright} above 10³⁵ erg/s");
        let interval_days = 1.0 / (RATE_PER_DAY * bright);
        assert!((interval_days - 15.08).abs() < 0.01, "{interval_days} d");
        // The mean of T^−0.9 on [400, 8,175] s: [T^1.1 ÷ 1.1] ÷ [T^0.1 ÷ 0.1].
        let (shortest, longest) = (MIN_DURATION_SECONDS, MAX_DURATION_SECONDS);
        let mean = (math::powf(longest, 1.1) - math::powf(shortest, 1.1))
            / 1.1
            / ((math::powf(longest, 0.1) - math::powf(shortest, 0.1)) / 0.1);
        let duty = RATE_PER_DAY * mean / SECONDS_PER_DAY;
        assert!((duty - 0.0350).abs() < 5e-5, "a duty cycle of {duty}");
        let lowest = MIN_MEAN_LUMINOSITY_WATTS * MIN_DURATION_SECONDS;
        let highest = MAX_MEAN_LUMINOSITY_WATTS * MAX_DURATION_SECONDS;
        assert!((lowest / 4.0e29 - 1.0).abs() < 1e-12, "{lowest} J");
        assert!(
            (highest / 1.573_687_5e32 - 1.0).abs() < 1e-12,
            "{highest} J"
        );
        // P(E > 10³⁸ erg) = ∫ f(L) P(T > E ÷ L) dL, by the midpoint rule in ln L, where the
        // density in ln L is L f(L) = k L^k ÷ (L(max)^k − L(min)^k), k = 1 + Γ.
        let threshold_joules = 1e31;
        let steps = 200_000_u32;
        let exponent = 1.0 + LUMINOSITY_SLOPE;
        let norm = math::powf(MAX_MEAN_LUMINOSITY_WATTS, exponent)
            - math::powf(MIN_MEAN_LUMINOSITY_WATTS, exponent);
        let ln_low = math::ln(MIN_MEAN_LUMINOSITY_WATTS);
        let step = (math::ln(MAX_MEAN_LUMINOSITY_WATTS) - ln_low) / f64::from(steps);
        let mut share = 0.0;
        for i in 0..steps {
            let luminosity = math::exp(ln_low + (f64::from(i) + 0.5) * step);
            let density = exponent * math::powf(luminosity, exponent) / norm;
            let shorter = power_law_cdf(
                threshold_joules / luminosity,
                shortest,
                longest,
                DURATION_SLOPE,
            );
            share += density * (1.0 - shorter) * step;
        }
        assert!((share - 0.253).abs() < 1e-3, "{share} above 10³⁸ erg");
        let peak = peak_over_mean();
        assert!((peak - 1.8683).abs() < 5e-5, "{peak}");
        assert!((template_area_sigmas() - 1.851_23).abs() < 5e-6);
        let bound = peak * MAX_MEAN_LUMINOSITY_WATTS;
        assert!(bound <= MAX_PEAK_LUMINOSITY_WATTS, "{bound} W");
        assert!(MAX_PEAK_LUMINOSITY_WATTS / bound - 1.0 < 1e-4, "{bound} W");
        eprintln!(
            "flare law: {bright:.4} above 10^35 erg/s (one in {interval_days:.2} d), mean \
             duration {mean:.1} s, duty cycle {:.3}%, {share:.4} above 10^38 erg ({:.3} a day), \
             peak/mean {peak:.6}, bound {:.6e} erg/s",
            100.0 * duty,
            RATE_PER_DAY * share,
            bound * 1e7,
        );
    }

    /// The inverse transforms' ends are the laws' ends, and a year's marks lie in their ranges,
    /// every duration under the cap, and follow their truncated laws (Kolmogorov–Smirnov).
    #[test]
    fn a_year_s_marks_lie_in_their_ranges_and_follow_their_laws() {
        assert_same_bits(mean_luminosity_watts_at(0.0), MIN_MEAN_LUMINOSITY_WATTS);
        assert_same_bits(duration_seconds_at(0.0), MIN_DURATION_SECONDS);
        // The cap is the law's top, whole seconds of it.
        assert_same_bits(MAX_DURATION.as_seconds_f64(), MAX_DURATION_SECONDS);
        let top = 1.0 - f64::EPSILON / 2.0;
        let l = mean_luminosity_watts_at(top);
        assert!(l <= MAX_MEAN_LUMINOSITY_WATTS && l > 0.999 * MAX_MEAN_LUMINOSITY_WATTS);
        let d = duration_seconds_at(top);
        assert!(d <= MAX_DURATION_SECONDS && d > 0.999 * MAX_DURATION_SECONDS);
        let year = list(&model(), days(0, 365));
        assert!(year.len() > 300, "{} flares", year.len());
        for f in &year {
            assert!(
                (MIN_MEAN_LUMINOSITY_WATTS..=MAX_MEAN_LUMINOSITY_WATTS)
                    .contains(&f.mean_luminosity_watts()),
                "{f:?}"
            );
            assert!(
                (MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&f.duration_seconds()),
                "{f:?}"
            );
            assert!(f.duration() <= MAX_DURATION, "{f:?}");
            assert!(
                (f.energy_joules() / (f.mean_luminosity_watts() * f.duration_seconds()) - 1.0)
                    .abs()
                    < 1e-15
            );
            assert!(f.peak_luminosity_watts() <= MAX_PEAK_LUMINOSITY_WATTS);
        }
        let mut lums: Vec<f64> = year
            .iter()
            .map(CentreFlare::mean_luminosity_watts)
            .collect();
        let ks = ks_one_sample(&mut lums, |x| {
            power_law_cdf(
                x,
                MIN_MEAN_LUMINOSITY_WATTS,
                MAX_MEAN_LUMINOSITY_WATTS,
                LUMINOSITY_SLOPE,
            )
        });
        assert_p_value("mean luminosities against L^−1.9", ks.p_value, ALPHA);
        let mut durations: Vec<f64> = year.iter().map(CentreFlare::duration_seconds).collect();
        let ks = ks_one_sample(&mut durations, |x| {
            power_law_cdf(
                x,
                MIN_DURATION_SECONDS,
                MAX_DURATION_SECONDS,
                DURATION_SLOPE,
            )
        });
        assert_p_value("durations against T^−0.9", ks.p_value, ALPHA);
    }

    /// The light curve is 0 outside the flare, continuous, peaks at the middle at the peak
    /// luminosity, and integrates to the energy.
    #[test]
    fn the_light_curve_is_zero_outside_continuous_and_integrates_to_the_energy() {
        let mut month = list(&model(), days(0, 30));
        month.sort_by(|a, b| a.energy_joules().total_cmp(&b.energy_joules()));
        let picks = [month[0], month[month.len() / 2], month[month.len() - 1]];
        for f in picks {
            let before =
                UniverseTime::new(f.onset().seconds() - 1, f.onset().subsec_nanos()).unwrap();
            assert_same_bits(f.luminosity_watts_at(before), 0.0);
            assert_same_bits(f.luminosity_watts_at(f.end()), 0.0);
            assert_same_bits(f.luminosity_watts_at(after(f.end(), 1.0)), 0.0);
            assert!(f.luminosity_watts_at(f.onset()) < 1e-12 * f.peak_luminosity_watts());
            assert!(!f.is_active_at(f.end()) && f.is_active_at(f.onset()));
            let middle = after(f.onset(), 0.5 * f.duration_seconds());
            let peak = f.peak_luminosity_watts();
            assert!((f.luminosity_watts_at(middle) / peak - 1.0).abs() < 1e-9);
            // Continuous: a microsecond moves it by a sliver of its peak anywhere, the ends
            // included.
            for share in [1e-9, 0.1, 0.25, 0.5, 0.8, 1.0 - 1e-6] {
                let t = after(f.onset(), share * f.duration_seconds());
                let step = (f.luminosity_watts_at(after(t, 1e-6)) - f.luminosity_watts_at(t)).abs();
                assert!(step < 1e-6 * peak, "{step} W at {share} of the flare");
            }
            // The trapezoid rule over the whole flare, 100,000 steps.
            let n = 100_000_u32;
            let h = f.duration_seconds() / f64::from(n);
            let mut sum = 0.0;
            for i in 0..=n {
                let weight = if i == 0 || i == n { 0.5 } else { 1.0 };
                sum += weight * f.luminosity_watts_at(after(f.onset(), h * f64::from(i)));
            }
            let integral = sum * h;
            assert!(
                (integral / f.energy_joules() - 1.0).abs() < 1e-5,
                "{integral} J against {} J",
                f.energy_joules()
            );
        }
    }

    /// The window and cut sets of the partition tests.
    fn partition_case(model: &CentreFlareModel) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let whole = TimeWindow::new(at(-20 * DAY - 1_234, 5), at(25 * DAY + 77, 0)).unwrap();
        let listed = list(model, whole);
        assert!(listed.len() > 20, "{} flares", listed.len());
        let cuts = vec![
            vec![at(0, 0)],
            vec![at(-DAY, 0), at(-DAY, 0), at(DAY, 1), at(4 * DAY + 999, 3)],
            vec![listed[10].onset(), after(listed[11].onset(), 1e-9)],
            (-19..25)
                .map(|d| at(d * DAY + 3_600 * (d % 7), 0))
                .collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test through the public function: a listing over a window is the
    /// union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let model = model();
        let listing = |w: TimeWindow| list(&model, w);
        let (whole, cut_sets) = partition_case(&model);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
    }

    /// The pieces listed on threads of their own give the same flares. Not on wasm32-wasip1, which
    /// has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let model = model();
        let listing = |w: TimeWindow| list(&model, w);
        let (whole, cut_sets) = partition_case(&model);
        let listed = listing(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<CentreFlare>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), listed);
        }
    }

    /// `flare(id)` regenerates every listed flare and returns `None` for a number beyond its bin's
    /// count and for an ID of another series, the other tag or another subject. Nothing is
    /// thinned: the listing is the bins' at a constant rate.
    #[test]
    fn an_id_regenerates_its_flare_and_names_nothing_else() {
        let model = model();
        let listed = list(&model, days(-5, 5));
        let mut bare = Vec::new();
        let constant = ConstantRate::new(model.listed_rate());
        bins().events_in(model.series(), days(-5, 5), &constant, &mut bare);
        assert_eq!(
            bare.iter().map(BinEvent::id).collect::<Vec<_>>(),
            listed.iter().map(CentreFlare::id).collect::<Vec<_>>()
        );
        let mut beyond = 0;
        for k in -5..5 {
            for j in 0..=u8::MAX {
                let id = model.series().event_id(EventBin::new(k).unwrap(), j);
                let found = listed.iter().find(|f| f.id() == id);
                assert_eq!(flare(&model, id).as_ref(), found, "bin {k}, number {j}");
                if found.is_none() {
                    beyond += 1;
                }
            }
        }
        assert!(beyond > 2_000, "{beyond} beyond the counts");
        let word = listed[0].id().word();
        let other_member = SystemId::from_raw(0xF000_0007_0000_0001).unwrap();
        for foreign in [
            event_id(
                super::super::subject(),
                event_tags::TIDAL_DISRUPTION,
                word.bin(),
                word.number(),
            ),
            event_id(other_member.into(), CENTRE_FLARE, word.bin(), word.number()),
        ] {
            assert_eq!(flare(&model, foreign), None);
        }
        assert_eq!(bins().bin_of(listed[0].onset()), Some(word.bin()));
    }

    /// `flares_active_at` gives exactly the listed flares running at t.
    #[test]
    fn flares_active_at_gives_the_flares_running_then() {
        let model = model();
        let listed = list(&model, days(-2, 40));
        let mut seen = 0;
        for i in 0..20_000 {
            let t = at(i64::from(i) * 157 + 3, 17);
            let expected: Vec<CentreFlare> = listed
                .iter()
                .copied()
                .filter(|f| f.is_active_at(t))
                .collect();
            let got = running(&model, t);
            assert_eq!(got, expected, "at {t}");
            seen += got.len();
        }
        assert!(seen > 100, "{seen} running flares seen");
    }

    /// The parent's no-history test for this kind: the flares running at +900 years, asked cold,
    /// are the same after asking about 100 earlier times.
    #[test]
    fn the_flares_running_at_900_years_need_no_history() {
        let base = UniverseTime::from_julian_years(900).unwrap();
        // Find an instant with a flare running on a model of its own, so that the cold call below
        // is the first on its model.
        let t = (0..20_000)
            .map(|i| after(base, f64::from(i) * 157.0))
            .find(|&t| !running(&model(), t).is_empty())
            .unwrap();
        let model = model();
        let cold = running(&model, t);
        let mut lcg = Lcg::new(0x0930_0900);
        for _ in 0..100 {
            let earlier = after(
                ClockWindow::START,
                lcg.next_f64() * 1.899e3 * SECONDS_PER_JULIAN_YEAR,
            );
            assert!(earlier < t);
            let _ = running(&model, earlier);
            let _ = list(
                &model,
                TimeWindow::new(earlier, after(earlier, 2.0 * SECONDS_PER_DAY)).unwrap(),
            );
        }
        assert_eq!(running(&model, t), cold);
        assert!(!cold.is_empty());
    }

    /// The same seed gives the same flares, another seed others.
    #[test]
    fn the_same_seed_gives_the_same_flares() {
        let a = list(&model(), days(0, 10));
        assert_eq!(list(&CentreFlareModel::new(SEED), days(0, 10)), a);
        assert_ne!(list(&CentreFlareModel::new(Seed::new(1)), days(0, 10)), a);
    }

    /// The rate integral: over 10³ seeds, the flares listed in one Julian year from the epoch
    /// number 10³ × 401.775 within a Poisson interval. Under half a second in a debug build, so not
    /// marked slow.
    #[test]
    fn a_thousand_seeds_years_of_flares_match_the_rate_integral() {
        const SEEDS: u32 = 1_000;
        let year = TimeWindow::new(
            UniverseTime::EPOCH,
            UniverseTime::from_julian_years(1).unwrap(),
        )
        .unwrap();
        let mut counted = 0_u64;
        let mut out = Vec::new();
        for n in 0..SEEDS {
            out.clear();
            flares(
                &CentreFlareModel::new(Seed::new(0x0930_a000_0000_0000 | u64::from(n))),
                year,
                &mut out,
            );
            counted += u64::try_from(out.len()).unwrap();
        }
        let expected = f64::from(SEEDS) * model().listed_rate().value() * SECONDS_PER_JULIAN_YEAR;
        eprintln!(
            "{SEEDS} seeds over a Julian year: expected {expected:.3}, counted {counted}, \
             interval {:?} at alpha {ALPHA:e}",
            poisson_interval(expected, ALPHA)
        );
        assert_poisson_count("flares of 10³ seeds in a year", counted, expected, ALPHA);
    }
}
