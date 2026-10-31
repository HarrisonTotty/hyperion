//! The giant eruptions of luminous blue variables, the fifth single-star event kind (plan 06,
//! P06.T28.e).
//!
//! # Who erupts
//!
//! A star erupts inside its LBV window, the clock interval of [`StarModel::lbv_window`]
//! (P06.T24.a): the hull from the first to the last age at which the star lies beyond the
//! Humphreys–Davidson limit and is hotter than 8,000 K ([`is_luminous_blue_variable`]), in any
//! living phase, within the part of its life the track is built for. Below version 26 eruptions
//! are not thinned inside the hull, so a star whose track leaves the region and comes back erupts
//! in the gap too: at [Fe/H] = −1 the windows of 45–100 M☉ span some 2 × 10⁵ years with a
//! red-supergiant stretch near 4,000 K inside them, and a 150 M☉ star's starts on its main
//! sequence (both deferred findings in plan 06, P06.T28.e). From version 26, behind the stellar
//! rules' `hd_limit` hold (P06.T42.b's swap, which enforces the limit in the winds;
//! `wind::HD_LIMIT_VERSION`), each candidate is thinned by [`is_luminous_blue_variable`] at its
//! onset, so a star erupts only while it is one (research-p06 §8): the bound stays the constant,
//! so the bins' words and every other eruption keep their places. The window is found once per
//! call on the star's track. A star with no window (every star below about 40 M☉, whose track
//! never reaches 6 × 10⁵ L☉) has no eruptions.
//!
//! # The rate
//!
//! One eruption per 12,000 years per star in the window ([`ERUPTIONS_PER_YEAR`]), a generator
//! default and ours. The plan's provisional one per 30,000 years assumed some 500 such stars, about 30
//! eruptions galaxy-wide in ±H; P06.T31's sampler finds 181 luminous blue variables at Milky Way
//! values below version 26, so the rate is retuned to keep that figure: 181 × 2,000 years ÷ 12,000
//! years = 30.2 eruptions in ±H, inside the plan's 15–60 (a slow test in `stellar::testing`
//! measures it). Under the enforced Humphreys–Davidson limit (the 26 hold) the same sampler finds
//! 48.8 luminous blue variables and 7.9 eruptions in ±H once thinned, both below the plan's
//! windows: the 26 train retunes the rate (`lbv_eruptions_galaxy_wide_under_the_enforced_limit`). The
//! rate is a constant inside the window and zero outside it, and each bin's bound is the constant
//! where the bin meets the window.
//!
//! The mean mass of the eruptions is taken to be in the track's wind (plan 06, design note 14): at
//! the marks' mean ejecta of 2.15 M☉ the eruptions shed 1.8 × 10⁻⁴ M☉ a year on average, 1.2 times
//! the modern recipe's luminous-blue-variable wind of 1.5 × 10⁻⁴ M☉ a year (Belczynski et al.
//! 2010, ApJ 714, 1217, equation 8, below version 26 the whole of an evolved star's wind beyond
//! the limit), which a star of the window's main-sequence part does not blow (a deferred finding in
//! plan 06, P06.T28.e). Under the enforced limit the wind strips tens of solar masses inside most
//! windows and the eruptions' expected mass is a small share of it, but in the longest windows of
//! the most massive stars, whose main-sequence part blows Vink's rate alone (up to 1.3 times the
//! track's loss for 150 M☉ at solar metallicity). Nothing here changes the track.
//!
//! # Time
//!
//! Eruptions are Poisson bins of [`BIN_SECONDS`] = 100 Julian years that look back
//! [`LOOK_BACK_BINS`] = one bin (plan 06, design note 15); a bin's mean is 100 ÷ 12,000, far under
//! 64. The window's ends are clock times, the epoch plus (age − the age at the epoch) × 31,557,600
//! s floored to the nanosecond, as [`StarModel`]'s mass breaks place an age, so no age is read at
//! t (design note 23).
//!
//! # Marks
//!
//! Each eruption's marks come from its own stream, [`BinEvent::marks`], from word 0, so marks
//! never move a time. The layout is fixed, each mark log-uniform from one `uniform()` and
//! independent of the others:
//!
//! - **word 0:** the ejected mass, 0.1–10 M☉ ([`EJECTED_MASS_SOLAR_MASSES`]);
//! - **word 1:** the peak luminosity, 10^6.5–10^7.5 L☉ ([`LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES`]);
//! - **word 2:** the duration, 1–20 years ([`DURATION_YEARS`]).
//!
//! Scatter added later takes words 3, 4, … in a fixed order.
//!
//! # The light curve
//!
//! An eruption's luminosity factor is `F^x`, F the peak luminosity over the star's quiescent one
//! (at least 1): x rises linearly in time from 0 at the onset to 1 at the peak, a quarter of the
//! duration in ([`RISE_SHARE`]), and falls linearly to 0 at the end, so the light curve is linear in
//! bolometric magnitude through the rise and through the decline. The shape is this module's
//! choice, the form of the FU Orionis outbursts' (P06.T28.d); the plan fixes only the peak and the
//! duration. Giant eruptions exceed the Eddington luminosity (η Carinae's Great Eruption did, for
//! about a decade, at five to eight times it: Davidson and Humphreys 1997, ARA&A 35, 1; Owocki,
//! Gayley and Shaviv 2004, ApJ 616, 525; recalled, not read at source in this build), so no cap
//! applies. The quiescent luminosity is the track's at the onset (or at +H,
//! beyond which a living star's track is not built), read once per eruption.
//!
//! An event may change what a console reads, never what the star's track is (design note 14):
//! nothing here changes the star.
//!
//! [`is_luminous_blue_variable`]: crate::stellar::classify::is_luminous_blue_variable

use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
    from_nanos_saturating, tags,
};
use crate::math;
use crate::stellar::classify::is_luminous_blue_variable;
use crate::stellar::sse::wind::HdLimitRules;
use crate::stellar::system::StarModel;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{SolarLuminosities, SolarMasses};

/// Seconds in one Julian year, as an integer.
const SECONDS_PER_JULIAN_YEAR_I64: i64 = 31_557_600;

/// The length of an eruption bin, s: 100 Julian years, 3,155,760,000 s (plan 06, P06.T28.e).
pub const BIN_SECONDS: i64 = 100 * SECONDS_PER_JULIAN_YEAR_I64;

/// How many whole bins [`eruptions_active_at`] looks back: one, 100 years, longer than any
/// eruption ([`MAX_DURATION`]).
pub const LOOK_BACK_BINS: u32 = 1;

/// The longest an eruption lasts, 20 Julian years, the top of [`DURATION_YEARS`]: under the
/// look-back's 100 years, so that [`eruptions_active_at`] finds every eruption still running.
pub const MAX_DURATION: Span = Span::from_seconds(20 * SECONDS_PER_JULIAN_YEAR_I64);

/// The rate of giant eruptions of a star inside its LBV window, per Julian year: one per 12,000
/// years, a generator default and ours (plan 06, P06.T28.e and design note 22), retuned here.
///
/// Retuned from the plan's provisional one per 30,000 years, which with "some 500 such stars"
/// gave a Milky Way galaxy about 30 eruptions in ±H, the research figure. P06.T31 measures 181
/// luminous blue variables galaxy-wide at Milky Way values (plan 06's Risks, "T31, measured at
/// Milky Way parameters"), so 181 stars × 2,000 years × this rate gives 30.2 in ±H, inside the
/// plan's 15–60. The slow test `lbv_eruptions_galaxy_wide_at_milky_way_values` measures it.
pub const ERUPTIONS_PER_YEAR: f64 = 1.0 / 12_000.0;

/// The range of an eruption's ejected mass, M☉, log-uniform: 0.1–10 (plan 06, P06.T28.e).
///
/// The bottom is P Cygni's eruption of 1600, which shed about 0.1 M☉ (Smith and Hartigan 2006, ApJ
/// 638, 1045), and the top lies just below η Carinae's Great Eruption of the 1840s, 12 M☉ or more
/// in the Homunculus (Smith et al. 2003, AJ 125, 1458); both are recalled, not read at source in
/// this build. The mean, 9.9 ÷ ln 100 = 2.15 M☉, is taken to be in the track's mean wind (design
/// note 14), though at the retuned rate it comes to 1.2 times the wind beyond the limit (see the
/// [module documentation](self)): the mark changes nothing the track holds.
pub const EJECTED_MASS_SOLAR_MASSES: (f64, f64) = (0.1, 10.0);

/// The range of log₁₀ of an eruption's peak bolometric luminosity in L☉, uniform: 6.5–7.5, so
/// 3.2 × 10⁶ to 3.2 × 10⁷ L☉ (plan 06, P06.T28.e).
///
/// η Carinae's Great Eruption radiated about 10^49.7 erg over two decades and reached a
/// bolometric magnitude of about −14 at its peaks, some 3 × 10⁷ L☉ (Davidson and Humphreys 1997,
/// ARA&A 35, 1; recalled, not read at source in this build), the range's top.
pub const LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES: (f64, f64) = (6.5, 7.5);

/// The range of an eruption's duration, Julian years, log-uniform: 1–20 (plan 06, P06.T28.e).
///
/// η Carinae's Great Eruption lasted about 20 years, 1838–1858 (Davidson and Humphreys 1997), and
/// P Cygni's of 1600 about six at third magnitude before its fading (Humphreys and Davidson 1994,
/// PASP 106, 1025); both recalled, not read at source in this build.
pub const DURATION_YEARS: (f64, f64) = (1.0, 20.0);

/// The share of an eruption's duration over which its light rises to the peak: a quarter, this
/// module's choice (η Carinae rose from its brightening of 1837–1838 to its peak of 1843 over
/// about a quarter of its Great Eruption, 0.27 of 1838–1858: Davidson and Humphreys 1997,
/// recalled, not read at source in this build).
pub const RISE_SHARE: f64 = 0.25;

/// The rate as an [`EventsPerSecond`].
#[must_use]
fn eruption_rate() -> EventsPerSecond {
    EventsPerSecond::per_julian_year(ERUPTIONS_PER_YEAR)
}

/// `lo` × (`hi` ÷ `lo`)^`u`: log-uniform between `lo` and `hi` for a uniform `u` in [0, 1).
#[must_use]
fn log_uniform((lo, hi): (f64, f64), u: f64) -> f64 {
    lo * math::powf(hi / lo, u)
}

/// `years` Julian years as a span, floored to the nanosecond.
#[must_use]
fn years_span(years: f64) -> Span {
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .expect("a mark of at most 20 years is a finite span")
}

/// The clock time `t` plus `span`, held to the clock's end.
#[must_use]
fn saturating_add(t: UniverseTime, span: Span) -> UniverseTime {
    t.checked_add(span)
        .unwrap_or_else(|| from_nanos_saturating(i128::MAX))
}

/// The giant eruptions of one luminous blue variable: its LBV window and the rate inside it
/// (plan 06, P06.T28.e; see the [module documentation](self)).
///
/// It is the [`RateModel`] of the star's `star.ev.lbv_eruption` series: the rate is
/// [`ERUPTIONS_PER_YEAR`] inside the window, T24.a's hull of the star's LBV ages, and zero outside
/// it, and a bin's bound is the rate if the bin meets the window and zero otherwise. Under the 26
/// hold of the Humphreys–Davidson limit the rate is also zero where the star is not a luminous blue
/// variable, the bound unchanged (the module documentation's thinning). Its methods list from the
/// window found once; the free functions find it on each call.
///
/// # Examples
///
/// A 60 M☉ star is a luminous blue variable for some tens of thousands of years after its main
/// sequence. Built at an age inside that window, which its full track gives, it erupts about once
/// in 12,000 years while the window lasts:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::lbv::LbvModel;
/// use hyperion_sim::stellar::sse::{PhasePredicate, Track};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let (mass, solar, draws) = (SolarMasses::new(60.0), Composition::SOLAR, StarDraws::median());
/// let window = Track::full(mass, &solar, &draws)
///     .window_where(PhasePredicate::Lbv)
///     .ok_or("a 60 M☉ star is an LBV")?;
/// let middle = (window.start().value() + window.end().value()) / 2.0;
/// let lbv = StarModel::new(mass, solar, draws, Years::new(middle))?;
/// let model = LbvModel::of(&lbv).ok_or("an LBV erupts")?;
/// assert!(model.start() < UniverseTime::EPOCH && UniverseTime::EPOCH < model.end());
/// assert!((1.0 / (model.rate_inside().value() * 31_557_600.0) - 12_000.0).abs() < 1e-6);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvModel<'a> {
    star: &'a StarModel,
    start: UniverseTime,
    end: UniverseTime,
}

impl<'a> LbvModel<'a> {
    /// The giant eruptions of `star`, or `None` for a star that is never a luminous blue variable
    /// in the part of its life its track is built for ([`StarModel::lbv_window`]), or whose window
    /// lies beyond the clock's range.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let (start, end) = star.lbv_window()?;
        Some(Self { star, start, end })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The first instant of the LBV window, as a clock time.
    #[must_use]
    pub const fn start(&self) -> UniverseTime {
        self.start
    }

    /// The end of the LBV window, as a clock time: the last instant at which the star is a luminous
    /// blue variable, which the eruptions' half-open window `[start, end)` leaves out.
    #[must_use]
    pub const fn end(&self) -> UniverseTime {
        self.end
    }

    /// The rate inside the window: [`ERUPTIONS_PER_YEAR`] as a rate per second, 2.64 × 10⁻¹² s⁻¹.
    #[must_use]
    pub fn rate_inside(&self) -> EventsPerSecond {
        eruption_rate()
    }

    /// Whether the star can erupt at `t`: whether `t` lies in `[start, end)`.
    #[must_use]
    pub fn erupts_at(&self, t: UniverseTime) -> bool {
        self.start <= t && t < self.end
    }

    /// Appends to `out` the giant eruptions of `series` with onsets in `window`, sorted by onset
    /// and then by ID: [`eruptions_in`] from this model, without finding the window again.
    ///
    /// # Panics
    ///
    /// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
    pub fn eruptions_in(&self, series: &EventSeries, window: TimeWindow, out: &mut Vec<Eruption>) {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
        let Some(window) = active_part(self, window) else {
            return;
        };
        let mut events = Vec::new();
        bins().events_in(series, window, self, &mut events);
        out.extend(events.into_iter().map(|event| self.eruption_of(event)));
    }

    /// Appends to `out` the giant eruptions of `series` running at `t`, sorted by onset and then
    /// by ID.
    ///
    /// They are those with an onset from the start of the bin before `t`'s up to `t` whose end is
    /// after `t`; every eruption lasts at most 20 years, under the look-back, so none is missed.
    ///
    /// # Panics
    ///
    /// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
    pub fn eruptions_active_at(
        &self,
        series: &EventSeries,
        t: UniverseTime,
        out: &mut Vec<Eruption>,
    ) {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
        if t < self.start || t >= saturating_add(self.end, MAX_DURATION) {
            return;
        }
        let mut events = Vec::new();
        bins().active_at(series, t, self, &mut events);
        out.extend(
            events
                .into_iter()
                .map(|event| self.eruption_of(event))
                .filter(|eruption| eruption.is_active_at(t)),
        );
    }

    /// The giant eruption `id` names, regenerated on its own; `None` if `id` is not of `series`,
    /// or names a candidate that was rejected or a number beyond its bin's count.
    ///
    /// # Panics
    ///
    /// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
    #[must_use]
    pub fn eruption(&self, series: &EventSeries, id: EventId) -> Option<Eruption> {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
        bins()
            .event(series, id, self)
            .map(|event| self.eruption_of(event))
    }

    /// The eruption of `event`, an event of this model's series: its ejected mass, peak and
    /// duration from words 0–2 of its marks, and its peak factor from the star's state.
    #[must_use]
    fn eruption_of(&self, event: BinEvent) -> Eruption {
        let id = event.id();
        let onset = event.time();
        let mut marks = event.into_marks();
        let ejected_mass = log_uniform(EJECTED_MASS_SOLAR_MASSES, marks.uniform());
        let (lo, hi) = LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES;
        let peak_luminosity = math::exp10(math::mul_add(hi - lo, marks.uniform(), lo));
        let duration = years_span(log_uniform(DURATION_YEARS, marks.uniform()));
        debug_assert!(duration <= MAX_DURATION, "an eruption lasts {duration}");
        let rise = Span::from_seconds_f64(duration.as_seconds_f64() * RISE_SHARE)
            .expect("a quarter of at most 20 years is a finite span");
        // The window lies within the built track, but an onset after +H reads the state at +H,
        // beyond which a living star's track is not built.
        let quiescent = self
            .star
            .state_at(onset.min(ClockWindow::END))
            .expect("an onset in the LBV window, or +H after it, comes after the star forms")
            .luminosity();
        let peak_factor = (peak_luminosity / quiescent.value()).max(1.0);
        Eruption {
            id,
            onset,
            rise,
            duration,
            ejected_mass: SolarMasses::new(ejected_mass),
            peak_luminosity: SolarLuminosities::new(peak_luminosity),
            quiescent_luminosity: quiescent,
            peak_factor,
        }
    }
}

impl RateModel for LbvModel<'_> {
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
        if bin.start() < self.end && self.start < bin.end() {
            eruption_rate()
        } else {
            EventsPerSecond::ZERO
        }
    }

    /// The constant inside the window; under the enforced Humphreys–Davidson limit only where the
    /// star is a luminous blue variable at `t` as well (the module documentation's thinning).
    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        let erupts = self.erupts_at(t)
            && match self.star.rules().hd_limit {
                HdLimitRules::Belczynski => true,
                HdLimitRules::Enforced => self
                    .star
                    .state_at(t)
                    .is_some_and(|state| is_luminous_blue_variable(&state)),
            };
        if erupts {
            eruption_rate()
        } else {
            EventsPerSecond::ZERO
        }
    }
}

/// One giant eruption of a luminous blue variable: its ID, onset, duration, ejected mass and peak,
/// and its light curve.
///
/// Its luminosity factor is 1 before the onset and from the end, and in between rises to its peak
/// factor and falls back, linearly in magnitude
/// ([`luminosity_factor_at`](Self::luminosity_factor_at)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Eruption {
    id: EventId,
    onset: UniverseTime,
    rise: Span,
    duration: Span,
    ejected_mass: SolarMasses,
    peak_luminosity: SolarLuminosities,
    quiescent_luminosity: SolarLuminosities,
    peak_factor: f64,
}

impl Eruption {
    /// The eruption's ID, from which [`eruption`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset, where the factor starts to rise from 1.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The peak, the onset plus the rise, where the factor is [`peak_factor`](Self::peak_factor).
    #[must_use]
    pub fn peak(&self) -> UniverseTime {
        saturating_add(self.onset, self.rise)
    }

    /// The first instant after the eruption, its onset plus its duration (held to the clock's end).
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        saturating_add(self.onset, self.duration)
    }

    /// The rise, from the onset to the peak: [`RISE_SHARE`] of the duration, floored to the
    /// nanosecond.
    #[must_use]
    pub const fn rise(&self) -> Span {
        self.rise
    }

    /// The duration, 1–20 Julian years, floored to the nanosecond: at most [`MAX_DURATION`].
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// The mass the eruption ejects, 0.1–10 M☉: a mark only, which the track's mean wind stands
    /// for (design note 14).
    #[must_use]
    pub const fn ejected_mass(&self) -> SolarMasses {
        self.ejected_mass
    }

    /// The peak bolometric luminosity as drawn, 10^6.5–10^7.5 L☉.
    #[must_use]
    pub const fn peak_luminosity(&self) -> SolarLuminosities {
        self.peak_luminosity
    }

    /// The star's quiescent luminosity, its track's at the onset (or at +H for an onset after it).
    #[must_use]
    pub const fn quiescent_luminosity(&self) -> SolarLuminosities {
        self.quiescent_luminosity
    }

    /// The peak factor, the peak luminosity over the quiescent one, at least 1.
    #[must_use]
    pub const fn peak_factor(&self) -> f64 {
        self.peak_factor
    }

    /// Whether the eruption is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end()
    }

    /// The star's bolometric luminosity at `t` as this eruption makes it: the quiescent luminosity
    /// times [`luminosity_factor_at`](Self::luminosity_factor_at), so the quiescent luminosity
    /// itself outside the eruption.
    #[must_use]
    pub fn luminosity_at(&self, t: UniverseTime) -> SolarLuminosities {
        SolarLuminosities::new(self.quiescent_luminosity.value() * self.luminosity_factor_at(t))
    }

    /// The star's bolometric luminosity at `t` over its quiescent one, as this eruption makes it.
    ///
    /// It is 1 before the onset and from the end, and in between `F^x`, F the peak factor, with
    /// x = (t − onset) ÷ rise through the rise and x = (end − t) ÷ (duration − rise) through the
    /// decline: linear in magnitude through both, 1 exactly at the onset and the end, and the peak
    /// factor exactly at the peak. It is continuous in `t`. P06.T28.g composes it into the star's
    /// summary.
    #[must_use]
    pub fn luminosity_factor_at(&self, t: UniverseTime) -> f64 {
        let Some(elapsed) = t.checked_since(self.onset) else {
            return 1.0;
        };
        if elapsed.is_negative() || elapsed >= self.duration {
            return 1.0;
        }
        let x = if elapsed < self.rise {
            elapsed.as_seconds_f64() / self.rise.as_seconds_f64()
        } else {
            let decline = self
                .duration
                .checked_sub(self.rise)
                .map_or(0.0, Span::as_seconds_f64);
            // Under the duration, so the difference is positive and cannot overflow.
            let left = self
                .duration
                .checked_sub(elapsed)
                .map_or(0.0, Span::as_seconds_f64);
            left / decline
        };
        if x >= 1.0 {
            self.peak_factor
        } else {
            math::powf(self.peak_factor, x)
        }
    }
}

/// The eruption bins: 100 years long, looking back one.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("100 years lie between 16 s and 2^44 s")
}

/// `window` cut to `model`'s LBV window, or `None` where they do not meet. Outside the LBV window
/// every bin's bound is zero, so the cut drops nothing and spares the empty bins.
#[must_use]
fn active_part(model: &LbvModel<'_>, window: TimeWindow) -> Option<TimeWindow> {
    let start = window.start().max(model.start());
    let end = window.end().min(model.end());
    (start < end).then(|| TimeWindow::new(start, end).expect("start before end"))
}

/// Appends to `out` the giant eruptions of `star` with onsets in `window`, sorted by onset and
/// then by ID.
///
/// A star that is never a luminous blue variable ([`LbvModel::of`]) has none. `series` is the
/// star's `star.ev.lbv_eruption` series, `EventSeries::new(seed, tags::STAR_LBV_ERUPTION,
/// body.into())`: [`StarModel`] holds no seed or ID, so it comes in beside the star, as for the
/// flares. Every bin the window touches is generated whole, so a listing over a window is the
/// union of the listings over any partition of it, in any order of asking. `out` is only appended
/// to; nothing in it is read. Each call finds the star's LBV window on its track, a millisecond or
/// so for a star that has one; [`LbvModel::eruptions_in`] lists from a model already built.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::lbv::{LbvModel, eruption, eruptions_in};
/// use hyperion_sim::stellar::sse::{PhasePredicate, Track};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A 60 M☉ star at the end of its LBV window, so that its track holds the whole window.
/// let (mass, solar, draws) = (SolarMasses::new(60.0), Composition::SOLAR, StarDraws::median());
/// let window = Track::full(mass, &solar, &draws)
///     .window_where(PhasePredicate::Lbv)
///     .ok_or("a 60 M☉ star is an LBV")?;
/// let star = StarModel::new(mass, solar, draws, Years::new(window.end().value()))?;
/// let model = LbvModel::of(&star).ok_or("an LBV erupts")?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = EventSeries::new(Seed::new(7), tags::STAR_LBV_ERUPTION, body.into());
/// let mut eruptions = Vec::new();
/// eruptions_in(&star, &series, TimeWindow::new(model.start(), model.end())?, &mut eruptions);
/// let first = eruptions.first().ok_or("tens of thousands of years at one per 12,000")?;
/// assert_eq!(eruption(&star, &series, first.id()).as_ref(), Some(first));
/// assert!(first.peak_factor() >= 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn eruptions_in(
    star: &StarModel,
    series: &EventSeries,
    window: TimeWindow,
    out: &mut Vec<Eruption>,
) {
    if let Some(model) = LbvModel::of(star) {
        model.eruptions_in(series, window, out);
    } else {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
    }
}

/// Appends to `out` the giant eruptions of `star` running at `t`, sorted by onset and then by ID:
/// [`LbvModel::eruptions_active_at`] of its model, and none for a star that is never a luminous
/// blue variable.
///
/// The result depends on `star`, `series` and `t` alone, never on what was asked before.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
pub fn eruptions_active_at(
    star: &StarModel,
    series: &EventSeries,
    t: UniverseTime,
    out: &mut Vec<Eruption>,
) {
    if let Some(model) = LbvModel::of(star) {
        model.eruptions_active_at(series, t, out);
    } else {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
    }
}

/// The giant eruption `id` names, regenerated on its own: [`LbvModel::eruption`] of the star's
/// model, and `None` for a star that is never a luminous blue variable.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_LBV_ERUPTION`].
#[must_use]
pub fn eruption(star: &StarModel, series: &EventSeries, id: EventId) -> Option<Eruption> {
    let Some(model) = LbvModel::of(star) else {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_LBV_ERUPTION,
            "a star's giant eruptions are its star.ev.lbv_eruption series"
        );
        return None;
    };
    model.eruption(series, id)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, ks_one_sample, poisson_interval,
    };

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::events::{EventBin, event_id, to_nanos};
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::{Galaxy, Population};
    use crate::id::{BodyId, SystemId, event_tags};
    use crate::rng::Seed;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::{PhasePredicate, Track};
    use crate::stellar::testing::{Sampling, reference_position, sample_inputs};
    use crate::time::SourceHorizon;
    use crate::units::{Dex, HeliumExcess, Years};

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000e);
    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// The ages of the LBV window of a star of `mass` M☉ and `composition` at the median draws,
    /// from its full track.
    fn window_ages(mass: f64, composition: &Composition) -> (f64, f64) {
        let window = Track::full(SolarMasses::new(mass), composition, &StarDraws::median())
            .window_where(PhasePredicate::Lbv)
            .unwrap();
        (window.start().value(), window.end().value())
    }

    /// A star of `mass` M☉ and `composition` at the median draws whose age at the epoch is its
    /// LBV window's start plus `share` of the window, plus `years`.
    fn star_in(mass: f64, composition: Composition, share: f64, years: f64) -> StarModel {
        let (start, end) = window_ages(mass, &composition);
        StarModel::new(
            SolarMasses::new(mass),
            composition,
            StarDraws::median(),
            Years::new(start + share * (end - start) + years),
        )
        .unwrap()
    }

    /// A solar-composition star of `mass` M☉ ten years past its LBV window at the epoch, so that
    /// its track holds the whole window.
    fn past_window(mass: f64) -> StarModel {
        star_in(mass, Composition::SOLAR, 1.0, 10.0)
    }

    /// [Fe/H] = −1, Z = 0.002, at which the LBV windows of 45–100 M☉ last some 2 × 10⁵ years.
    fn low_z() -> Composition {
        Composition::from_fe_h(Dex::new(-1.0), HeliumExcess::ZERO)
    }

    /// A star of `mass` M☉ at the median draws, `age` years old at the epoch, of solar
    /// composition.
    fn star(mass: f64, age: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(mass),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(age),
        )
        .unwrap()
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    fn series(index: u16) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_LBV_ERUPTION, body(index).into())
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// The instant `n` nanoseconds from `t`.
    fn shifted(t: UniverseTime, n: i64) -> UniverseTime {
        from_nanos_saturating(to_nanos(t) + i128::from(n))
    }

    /// The bins `from..to` as one window.
    fn bins_window(from: i64, to: i64) -> TimeWindow {
        TimeWindow::new(
            UniverseTime::new(from * BIN_SECONDS, 0).unwrap(),
            UniverseTime::new(to * BIN_SECONDS, 0).unwrap(),
        )
        .unwrap()
    }

    /// The whole bins about `model`'s window, a bin either side.
    fn around(model: &LbvModel<'_>) -> TimeWindow {
        let bins = bins();
        let first = bins.bin_of(model.start()).unwrap().get() - 1;
        let last = bins.bin_of(model.end()).unwrap().get() + 2;
        bins_window(first, last)
    }

    fn list(model: &LbvModel<'_>, series: &EventSeries, w: TimeWindow) -> Vec<Eruption> {
        let mut out = Vec::new();
        model.eruptions_in(series, w, &mut out);
        out
    }

    /// The eruptions of `n` series of `model`'s star over `w`, one series after another.
    fn list_many(model: &LbvModel<'_>, n: u16, w: TimeWindow) -> Vec<Eruption> {
        let mut out = Vec::new();
        for i in 0..n {
            model.eruptions_in(&series(i), w, &mut out);
        }
        out
    }

    fn running(model: &LbvModel<'_>, series: &EventSeries, t: UniverseTime) -> Vec<Eruption> {
        let mut out = Vec::new();
        model.eruptions_active_at(series, t, &mut out);
        out
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    /// The window of a 60 M☉ solar star, 1,633 years of its Hertzsprung gap; a 150 M☉ one, some
    /// 7 × 10⁵ years from its main sequence; and a 60 M☉ star at [Fe/H] = −1, 2.1 × 10⁵ years.
    fn cases() -> [StarModel; 3] {
        [
            past_window(60.0),
            past_window(150.0),
            star_in(60.0, low_z(), 1.0, 10.0),
        ]
    }

    /// The rate is 1 ÷ 12,000 years exactly in `[start, end)` and zero outside it, and a bin's
    /// bound is that rate exactly when the bin meets the window; the window is the full track's.
    #[test]
    fn the_rate_is_the_constant_inside_the_window_and_zero_outside_it() {
        let rate = eruption_rate();
        assert!(relative(rate.value() * YEAR, 1.0 / 12_000.0) < 1e-15);
        let bins = bins();
        for (s, (mass, z)) in cases().iter().zip([
            (60.0, Composition::SOLAR),
            (150.0, Composition::SOLAR),
            (60.0, low_z()),
        ]) {
            let model = LbvModel::of(s).unwrap();
            let (start_age, end_age) = window_ages(mass, &z);
            // The clock window is the track's: its length in years, to the nanoseconds' rounding.
            let length = model.end().checked_since(model.start()).unwrap();
            assert!((length.as_julian_years_f64() - (end_age - start_age)).abs() < 1e-6);
            for (t, inside) in [
                (model.start(), true),
                (shifted(model.start(), -1), false),
                (shifted(model.end(), -1), true),
                (model.end(), false),
                (after(model.start(), -1e3 * YEAR), false),
            ] {
                assert_eq!(model.erupts_at(t), inside, "{t}");
                assert_eq!(
                    model.rate(t),
                    if inside { rate } else { EventsPerSecond::ZERO }
                );
                let bin = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
                let meets = bin.start() < model.end() && model.start() < bin.end();
                assert_eq!(
                    model.bound(bin),
                    if meets { rate } else { EventsPerSecond::ZERO }
                );
            }
            assert_eq!(model.rate_inside(), rate);
        }
    }

    /// No eruption without an LBV window in the built track: stars of 1, 20 and 40 M☉ at solar
    /// metallicity, a brown dwarf, and a 60 M☉ star at 1 Myr, whose track is built only to +H,
    /// before its window; a 60 M☉ star dead for 10 Myr still has its window, long past.
    #[test]
    fn a_star_without_a_window_has_no_eruptions() {
        let window = TimeWindow::new(SourceHorizon::START, ClockWindow::END).unwrap();
        for s in [
            star(1.0, 4.6e9),
            star(20.0, 9e6),
            star(40.0, 5e6),
            star(0.05, 1e9),
            star(60.0, 1e6),
        ] {
            assert!(LbvModel::of(&s).is_none());
            let mut out = Vec::new();
            eruptions_in(&s, &series(0), window, &mut out);
            eruptions_active_at(&s, &series(0), UniverseTime::EPOCH, &mut out);
            assert_eq!(out, []);
            assert_eq!(
                eruption(
                    &s,
                    &series(0),
                    series(0).event_id(EventBin::new(0).unwrap(), 0)
                ),
                None
            );
        }
        let dead = star(60.0, 1.456e7);
        let model = LbvModel::of(&dead).unwrap();
        assert!(model.end() < SourceHorizon::START);
        assert_eq!(list(&model, &series(0), window), []);
    }

    /// The bound holds at 43 instants of each bin, its ends included, from a bin before the window
    /// to a bin after it, for 40–150 M☉ at solar metallicity and at [Fe/H] = −1.
    #[test]
    fn the_bound_holds_across_mass_composition_and_the_window() {
        let bins = bins();
        let mut checked = 0_u32;
        for z in [Composition::SOLAR, low_z()] {
            for mass in [40.0, 45.0, 50.0, 60.0, 80.0, 100.0, 150.0] {
                let Some(window) = Track::full(SolarMasses::new(mass), &z, &StarDraws::median())
                    .window_where(PhasePredicate::Lbv)
                else {
                    continue;
                };
                let s = StarModel::new(
                    SolarMasses::new(mass),
                    z,
                    StarDraws::median(),
                    Years::new(window.end().value() + 10.0),
                )
                .unwrap();
                let model = LbvModel::of(&s).unwrap();
                let w = around(&model);
                let first = bins.bin_of(w.start()).unwrap().get();
                let last = bins.bin_of(shifted(w.end(), -1)).unwrap().get();
                // At most 64 bins at the window's two ends and in between.
                let edges = bins.bin_of(model.start()).unwrap().get();
                let ends = bins.bin_of(model.end()).unwrap().get();
                for k in
                    (first..=last).filter(|&k| (k - edges).abs() <= 32 || (k - ends).abs() <= 32)
                {
                    let bin = bins.bin_window(EventBin::new(k).unwrap()).unwrap();
                    let bound = model.bound(bin);
                    for i in 0..=42_i64 {
                        let t = if i == 42 {
                            shifted(bin.end(), -1)
                        } else {
                            UniverseTime::new(k * BIN_SECONDS + i * (BIN_SECONDS / 42), 0).unwrap()
                        };
                        assert!(model.rate(t) <= bound, "{mass} M☉ at {t}");
                        checked += 1;
                    }
                    let seconds = bin
                        .end()
                        .checked_since(bin.start())
                        .unwrap()
                        .as_seconds_f64();
                    assert!(bound.value() * seconds <= PoissonBins::MAX_MEAN_PER_BIN);
                }
            }
        }
        assert!(checked > 20_000, "{checked} instants");
    }

    /// A rate model that accepts every candidate of a model's bins: the same bound, and the rate
    /// equal to it.
    struct EveryCandidate<'a>(&'a LbvModel<'a>);

    impl RateModel for EveryCandidate<'_> {
        fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
            self.0.bound(bin)
        }

        fn rate(&self, t: UniverseTime) -> EventsPerSecond {
            let bins = bins();
            self.0
                .bound(bins.bin_window(bins.bin_of(t).unwrap()).unwrap())
        }
    }

    /// `eruption(id)` regenerates every listed eruption of the bins about a 60 M☉ star's short
    /// window, and returns `None` for a candidate rejected outside the window, a number beyond
    /// its bin's count, an ID of another series and a star with no window.
    #[test]
    fn an_id_regenerates_its_eruption_and_names_nothing_else() {
        let s = past_window(60.0);
        let model = LbvModel::of(&s).unwrap();
        let every = EveryCandidate(&model);
        let bins = bins();
        let w = around(&model);
        let first = bins.bin_of(w.start()).unwrap().get();
        let last = bins.bin_of(shifted(w.end(), -1)).unwrap().get();
        let (mut rejected, mut beyond, mut found) = (0, 0, 0);
        for index in 0..2_000 {
            let sr = series(index);
            let listed = list(&model, &sr, w);
            assert!(listed.iter().all(|e| model.erupts_at(e.onset())));
            for bin in first..=last {
                for j in 0..4 {
                    let id = sr.event_id(EventBin::new(bin).unwrap(), j);
                    let expected = listed.iter().find(|e| e.id() == id);
                    assert_eq!(
                        model.eruption(&sr, id).as_ref(),
                        expected,
                        "series {index}, bin {bin}, number {j}"
                    );
                    match (expected, bins.event(&sr, id, &every)) {
                        (None, Some(_)) => rejected += 1,
                        (None, None) => beyond += 1,
                        (Some(_), _) => found += 1,
                    }
                }
            }
        }
        assert!(
            rejected > 3 && beyond > 100_000 && found > 100,
            "{rejected} rejected, {beyond} beyond, {found} found"
        );
        let (index, listed) = (0..2_000)
            .map(|i| (i, list(&model, &series(i), w)))
            .find(|(_, l)| !l.is_empty())
            .unwrap();
        let sr = series(index);
        assert_eq!(eruption(&s, &sr, listed[0].id()).as_ref(), Some(&listed[0]));
        let word = listed[0].id().word();
        for foreign in [
            event_id(
                body(9_999).into(),
                tags::STAR_LBV_ERUPTION,
                word.bin(),
                word.number(),
            ),
            event_id(
                body(index).into(),
                event_tags::SELF_TEST,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(model.eruption(&sr, foreign), None);
        }
        assert_eq!(eruption(&star(20.0, 9e6), &sr, listed[0].id()), None);
    }

    /// The K–S test of `values` against the log-uniform law on `(lo, hi)`.
    fn assert_log_uniform(what: &str, mut values: Vec<f64>, (lo, hi): (f64, f64)) {
        assert!(values.len() > 200, "{what}: {}", values.len());
        assert!(
            values.iter().all(|&v| (lo..=hi).contains(&v)),
            "{what} outside {lo}–{hi}"
        );
        let span = math::ln(hi / lo);
        let ks = ks_one_sample(&mut values, |x| (math::ln(x / lo) / span).clamp(0.0, 1.0));
        assert_p_value(what, ks.p_value, ALPHA);
    }

    /// Every mark lies in its range and follows its log-uniform law (K–S); the rise is a quarter
    /// of the duration, every eruption ends within 20 years, the quiescent luminosity is the
    /// track's at the onset and the peak factor the peak over it, at least 1.
    #[test]
    fn every_mark_follows_its_law() {
        let s = past_window(150.0);
        let model = LbvModel::of(&s).unwrap();
        let eruptions = list_many(&model, 12, around(&model));
        assert!(eruptions.len() > 500, "{}", eruptions.len());
        assert_log_uniform(
            "ejected mass",
            eruptions.iter().map(|e| e.ejected_mass().value()).collect(),
            EJECTED_MASS_SOLAR_MASSES,
        );
        assert_log_uniform(
            "peak luminosity",
            eruptions
                .iter()
                .map(|e| e.peak_luminosity().value())
                .collect(),
            (
                math::exp10(LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES.0),
                math::exp10(LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES.1),
            ),
        );
        assert_log_uniform(
            "duration",
            eruptions
                .iter()
                .map(|e| e.duration().as_julian_years_f64())
                .collect(),
            DURATION_YEARS,
        );
        let mut binding = 0;
        for e in &eruptions {
            assert!(e.duration() <= MAX_DURATION);
            let quarter = e.duration().as_seconds_f64() * RISE_SHARE;
            // Floored to the nanosecond, against an `f64` of seconds good to about 30 ns here.
            assert!((e.rise().as_seconds_f64() - quarter).abs() < 1e-7);
            let quiescent = s.state_at(e.onset()).unwrap().luminosity();
            assert_same_bits(e.quiescent_luminosity().value(), quiescent.value());
            let ratio = e.peak_luminosity().value() / quiescent.value();
            assert_same_bits(e.peak_factor(), ratio.max(1.0));
            binding += u32::from(ratio < 1.0);
        }
        // A 150 M☉ star of over 10⁶ L☉ still sits below every drawn peak.
        assert_eq!(binding, 0);
    }

    /// The factor is 1 outside an eruption and at its ends, its peak factor at the peak, F^½
    /// half-way through the rise and the decline (linear in magnitude), continuous, and rises,
    /// then falls.
    #[test]
    fn the_factor_is_one_outside_an_eruption_and_its_peak_factor_at_the_peak() {
        let s = past_window(150.0);
        let model = LbvModel::of(&s).unwrap();
        let eruptions = list_many(&model, 2, around(&model));
        assert!(eruptions.len() > 20);
        for e in eruptions.iter().take(20) {
            let (peak, top) = (e.peak(), e.peak_factor());
            assert!(top > 1.0);
            assert_same_bits(e.luminosity_factor_at(shifted(e.onset(), -1)), 1.0);
            assert_same_bits(e.luminosity_factor_at(e.onset()), 1.0);
            assert_same_bits(e.luminosity_factor_at(e.end()), 1.0);
            assert_same_bits(e.luminosity_factor_at(peak), top);
            assert_same_bits(
                e.luminosity_at(peak).value(),
                e.quiescent_luminosity().value() * top,
            );
            assert_same_bits(
                e.luminosity_at(e.end()).value(),
                e.quiescent_luminosity().value(),
            );
            assert!(e.is_active_at(e.onset()) && !e.is_active_at(e.end()));
            assert!(!e.is_active_at(shifted(e.onset(), -1)));
            let excess = top - 1.0;
            for t in [
                e.onset(),
                shifted(peak, -1_000_000_000),
                peak,
                shifted(e.end(), -1_000_000_000),
            ] {
                let step =
                    (e.luminosity_factor_at(after(t, 1.0)) - e.luminosity_factor_at(t)).abs();
                assert!(step < 1e-6 * excess, "{step} at {t}");
            }
            let decline = e.duration().as_seconds_f64() - e.rise().as_seconds_f64();
            for t in [
                after(e.onset(), e.rise().as_seconds_f64() / 2.0),
                after(peak, decline / 2.0),
            ] {
                assert!(
                    relative(e.luminosity_factor_at(t), top.sqrt()) < 1e-9,
                    "{t}"
                );
            }
            let samples = 2_000_u32;
            let (mut last, mut past_peak) = (1.0, false);
            for i in 1..=samples {
                let t = after(
                    e.onset(),
                    e.duration().as_seconds_f64() * f64::from(i) / f64::from(samples),
                );
                let factor = e.luminosity_factor_at(t);
                assert!((1.0..=top).contains(&factor));
                if t <= peak {
                    assert!(factor >= last);
                } else {
                    if !past_peak {
                        (past_peak, last) = (true, top);
                    }
                    assert!(factor <= last);
                }
                last = factor;
            }
        }
    }

    /// Under the 26 hold of the Humphreys–Davidson limit (P06.T42.b) a 150 M☉ star at \[Fe/H\] −2,
    /// whose window crosses states that are not luminous blue variables, erupts at exactly the
    /// candidates of the same bins inside its window whose onset is one: each eruption is the
    /// candidate's, its ID regenerates it and a dropped candidate's names none, and the listing is
    /// the same over a partition of the window's edges in any order of asking.
    #[test]
    fn the_thinned_eruptions_are_the_windows_candidates_at_lbv_states() {
        use crate::stellar::rules::StellarRules;
        let rules = StellarRules {
            hd_limit: HdLimitRules::Enforced,
            ..StellarRules::of_version(GENERATOR_VERSION)
        };
        let s = StarModel::new_under(
            SolarMasses::new(150.0),
            Composition::from_fe_h(Dex::new(-2.0), HeliumExcess::ZERO),
            StarDraws::median(),
            Years::new(2.6e6),
            rules,
        )
        .unwrap();
        let model = LbvModel::of(&s).unwrap();
        let every = EveryCandidate(&model);
        let bins = bins();
        let w = around(&model);
        let first = bins.bin_of(w.start()).unwrap().get();
        let last = bins.bin_of(shifted(w.end(), -1)).unwrap().get();
        let (mut kept, mut thinned) = (0, 0);
        for index in 0..100 {
            let sr = series(index);
            let listed = list(&model, &sr, w);
            let mut expected = 0;
            for bin in first..=last {
                for j in 0..4 {
                    let id = sr.event_id(EventBin::new(bin).unwrap(), j);
                    let Some(candidate) = bins.event(&sr, id, &every) else {
                        continue;
                    };
                    let onset = candidate.time();
                    let inside = model.erupts_at(onset);
                    let lbv = inside
                        && s.state_at(onset)
                            .is_some_and(|state| is_luminous_blue_variable(&state));
                    let want = lbv.then(|| model.eruption_of(candidate));
                    assert_eq!(
                        model.eruption(&sr, id),
                        want,
                        "series {index}, bin {bin}, {j}"
                    );
                    if let Some(eruption) = want {
                        assert!(listed.contains(&eruption), "series {index}: {eruption:?}");
                        expected += 1;
                        kept += 1;
                    } else if inside {
                        thinned += 1;
                    }
                }
            }
            assert_eq!(listed.len(), expected, "series {index}");
        }
        assert!(kept > 100 && thinned > 10, "{kept} kept, {thinned} thinned");
        let listing = |w: TimeWindow| list_many(&model, 20, w);
        let cuts = vec![
            model.start(),
            shifted(model.start(), 1),
            shifted(model.end(), -1),
            model.end(),
        ];
        assert_partition_independent(w, &cuts, listing);
        assert_order_independent(&partition(w, &cuts), |w| listing(*w));
    }

    /// The partition rule at the window's edges: cuts at its start, a nanosecond after it, a
    /// nanosecond before its end and at it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_partitions_at_the_windows_edges() {
        for s in cases() {
            let model = LbvModel::of(&s).unwrap();
            let sr = series(5);
            let whole = around(&model);
            let listing = |w: TimeWindow| list(&model, &sr, w);
            let cuts = vec![
                model.start(),
                shifted(model.start(), 1),
                shifted(model.end(), -1),
                model.end(),
            ];
            assert_partition_independent(whole, &cuts, listing);
            assert_order_independent(&partition(whole, &cuts), |w| listing(*w));
        }
    }

    /// The window and cut sets of the partition tests, on the 150 M☉ star.
    fn partition_case(
        model: &LbvModel<'_>,
        listing: impl Fn(TimeWindow) -> Vec<Eruption>,
    ) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let bins = bins();
        let k = bins.bin_of(model.start()).unwrap().get() + 10;
        let whole = TimeWindow::new(
            UniverseTime::new(k * BIN_SECONDS - 1_234, 5).unwrap(),
            UniverseTime::new((k + 1_500) * BIN_SECONDS + 77, 0).unwrap(),
        )
        .unwrap();
        let eruptions = listing(whole);
        assert!(eruptions.len() > 6, "{} eruptions", eruptions.len());
        let at = |n: i64, nanos: u32| UniverseTime::new(n, nanos).unwrap();
        let cuts = vec![
            vec![at((k + 700) * BIN_SECONDS, 0)],
            vec![
                at((k + 3) * BIN_SECONDS, 0),
                at((k + 3) * BIN_SECONDS, 0),
                at((k + 3) * BIN_SECONDS + BIN_SECONDS / 2, 1),
                at((k + 4) * BIN_SECONDS, 3),
            ],
            vec![eruptions[2].onset(), after(eruptions[3].onset(), 1e-9)],
            (1..60)
                .map(|n| at((k + 25 * n) * BIN_SECONDS + 3_600 * n, 0))
                .collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test through the public function: a listing over a window is the
    /// union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let s = past_window(150.0);
        let model = LbvModel::of(&s).unwrap();
        let sr = series(2);
        let listing = |w: TimeWindow| list(&model, &sr, w);
        let (whole, cut_sets) = partition_case(&model, listing);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
        // The free function, which finds the window itself, lists the same.
        let mut free = Vec::new();
        eruptions_in(&s, &sr, whole, &mut free);
        assert_eq!(free, listing(whole));
    }

    /// The pieces listed on threads of their own give the same eruptions. Not on wasm32-wasip1,
    /// which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let s = past_window(150.0);
        let model = LbvModel::of(&s).unwrap();
        let sr = series(2);
        let listing = |w: TimeWindow| list(&model, &sr, w);
        let (whole, cut_sets) = partition_case(&model, listing);
        let eruptions = listing(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<Eruption>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), eruptions);
        }
    }

    /// `eruptions_active_at` gives exactly the listed eruptions running at t.
    #[test]
    fn eruptions_active_at_gives_the_eruptions_running_then() {
        let s = past_window(150.0);
        let model = LbvModel::of(&s).unwrap();
        let w = around(&model);
        let span = w.end().checked_since(w.start()).unwrap().as_seconds_f64();
        let mut seen = 0;
        for index in 0..40 {
            let sr = series(index);
            let listed = list(&model, &sr, w);
            // Each eruption's onset, a nanosecond before it, its middle and its end, and 500
            // instants spread over the window.
            let mut instants: Vec<UniverseTime> = listed
                .iter()
                .flat_map(|e| {
                    [
                        e.onset(),
                        shifted(e.onset(), -1),
                        after(e.onset(), e.duration().as_seconds_f64() / 2.0),
                        e.end(),
                    ]
                })
                .collect();
            instants
                .extend((0..500).map(|i| after(w.start(), span * (f64::from(i) + 0.5) / 500.0)));
            for t in instants {
                let expected: Vec<Eruption> = listed
                    .iter()
                    .copied()
                    .filter(|e| e.is_active_at(t))
                    .collect();
                let got = running(&model, &sr, t);
                assert_eq!(got, expected, "series {index} at {t}");
                seen += got.len();
            }
        }
        assert!(seen > 1_000, "{seen} running eruptions seen");
        let mut free = Vec::new();
        eruptions_active_at(&s, &series(0), model.start(), &mut free);
        assert_eq!(free, running(&model, &series(0), model.start()));
    }

    /// The parent's no-history test for this kind: the eruptions running at +900 years, asked
    /// cold, are the same after asking about 100 earlier times.
    #[test]
    fn the_eruptions_running_at_900_years_need_no_history() {
        // A 150 M☉ star in the middle of its window at the epoch.
        let probe = star_in(150.0, Composition::SOLAR, 0.5, 0.0);
        let model = LbvModel::of(&probe).unwrap();
        let base = after(UniverseTime::EPOCH, 900.0 * YEAR);
        assert!(model.erupts_at(base));
        let (index, t) = (0..2_000)
            .flat_map(|i| (0..10).map(move |k| (i, after(base, f64::from(k) * 10.0 * YEAR))))
            .find(|&(i, t)| !running(&model, &series(i), t).is_empty())
            .unwrap();
        let sr = series(index);
        let s = star_in(150.0, Composition::SOLAR, 0.5, 0.0);
        let mut cold = Vec::new();
        eruptions_active_at(&s, &sr, t, &mut cold);
        let fresh = LbvModel::of(&s).unwrap();
        let mut lcg = Lcg::new(0x0928_e900);
        for _ in 0..100 {
            let earlier = after(ClockWindow::START, lcg.next_f64() * 1.899e3 * YEAR);
            assert!(earlier < t);
            let _ = running(&fresh, &sr, earlier);
            let _ = list(
                &fresh,
                &sr,
                TimeWindow::new(earlier, after(earlier, 50.0 * YEAR)).unwrap(),
            );
        }
        assert_eq!(running(&fresh, &sr, t), cold);
        assert_ne!(cold, []);
    }

    /// Pinned: three luminous blue variables, their windows, the rate and the bounds of the bins
    /// at the window's ends, and each eruption's ID, onset, marks, quiescent luminosity, peak
    /// factor and factor at four instants, over at most 100,000 years of each window.
    #[test]
    fn lbv_eruptions_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.f64("rate inside a window (per s)", eruption_rate().value());
        let bins = bins();
        let cases = [
            (
                "a 60 M☉ star of solar composition ten years past its window, 40 series",
                past_window(60.0),
                0..40,
            ),
            (
                "a 150 M☉ star of solar composition ten years past its window, the first 100,000 \
                 years of its window, one series",
                past_window(150.0),
                40..41,
            ),
            (
                "a 60 M☉ star at [Fe/H] = −1 ten years past its window, the first 100,000 years of \
                 its window, one series",
                star_in(60.0, low_z(), 1.0, 10.0),
                42..43,
            ),
        ];
        for (name, s, indices) in cases {
            let model = LbvModel::of(&s).unwrap();
            w.line("");
            w.line(&format!("# {name}"));
            w.line(&format!("window from {} to {}", model.start(), model.end()));
            for (label, t) in [("start", model.start()), ("end", model.end())] {
                let bin = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
                w.f64(
                    &format!("bound of the {label} bin (per s)"),
                    model.bound(bin).value(),
                );
            }
            // At most the 1,000 bins from the one before the window's start.
            let whole = around(&model);
            let first = bins.bin_of(whole.start()).unwrap().get();
            let last = bins.bin_of(whole.end()).unwrap().get().min(first + 1_000);
            for index in indices {
                for e in list(&model, &series(index), bins_window(first, last)) {
                    w.line(&format!("{} at {}", e.id(), e.onset()));
                    w.f64("  ejected mass (M☉)", e.ejected_mass().value());
                    w.f64("  peak luminosity (L☉)", e.peak_luminosity().value());
                    w.line(&format!("  rise {}", e.rise()));
                    w.line(&format!("  duration {}", e.duration()));
                    w.f64(
                        "  quiescent luminosity (L☉)",
                        e.quiescent_luminosity().value(),
                    );
                    w.f64("  peak factor", e.peak_factor());
                    let decline = e.duration().as_seconds_f64() - e.rise().as_seconds_f64();
                    for (label, t) in [
                        (
                            "half the rise",
                            after(e.onset(), e.rise().as_seconds_f64() / 2.0),
                        ),
                        ("the peak", e.peak()),
                        ("half the decline", after(e.peak(), decline / 2.0)),
                        ("a day before the end", after(e.end(), -86_400.0)),
                    ] {
                        w.f64(&format!("  factor at {label}"), e.luminosity_factor_at(t));
                    }
                }
            }
        }
        golden!("stellar/lbv_eruptions", w.as_str());
    }

    /// The parent's statistical test for this kind: 10⁴ stars, 100 series each of the first 100
    /// luminous blue variables of T31's sampler (band E of the young thin disc at Milky Way
    /// values, at their own masses, compositions and draws), each built ten years past its window
    /// so that its track holds it: the eruptions listed over the bins about each window number the
    /// rate integral, Σ rate × window, within a Poisson interval.
    #[test]
    #[ignore = "slow: about 1,400 massive tracks, and 10⁴ series over their windows"]
    fn the_mean_count_over_10_000_stars_matches_the_rate_integral() {
        const STARS: u16 = 100;
        const SERIES: u16 = 100;
        let seed = 0x0628_e000_0000_5eed_u64;
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like()).unwrap();
        let component = galaxy
            .fields()
            .components()
            .iter()
            .position(|c| c.population() == Population::YoungThinDisc)
            .unwrap();
        let sampling = Sampling {
            component,
            position: reference_position(Population::YoungThinDisc),
            band: Some(MassBand::E),
        };
        let (mut expected, mut counted, mut drawn) = (0.0, 0_u64, 0_u64);
        let (mut shortest, mut longest) = (f64::INFINITY, 0.0_f64);
        let mut lbvs = 0_u16;
        while lbvs < STARS {
            let inputs = sample_inputs(&galaxy, &sampling, seed, drawn);
            drawn += 1;
            let Some(window) = Track::full(inputs.initial_mass, &inputs.composition, &inputs.draws)
                .window_where(PhasePredicate::Lbv)
            else {
                continue;
            };
            let s = StarModel::new(
                inputs.initial_mass,
                inputs.composition,
                inputs.draws,
                Years::new(window.end().value() + 10.0),
            )
            .unwrap();
            let model = LbvModel::of(&s).unwrap();
            let length = model
                .end()
                .checked_since(model.start())
                .unwrap()
                .as_seconds_f64();
            shortest = shortest.min(length / YEAR);
            longest = longest.max(length / YEAR);
            for k in 0..SERIES {
                let sr = EventSeries::new(
                    Seed::new(seed),
                    tags::STAR_LBV_ERUPTION,
                    BodyId::new(
                        SystemId::from_raw(0x0200_0800_2000_0000).unwrap(),
                        lbvs * SERIES + k,
                    )
                    .into(),
                );
                counted += u64::try_from(list(&model, &sr, around(&model)).len()).unwrap();
                expected += eruption_rate().value() * length;
            }
            lbvs += 1;
        }
        eprintln!(
            "{STARS} LBVs of {drawn} band-E stars, windows of {shortest:.0}–{longest:.0} yr, \
             {SERIES} series each: expected {expected:.1}, counted {counted}, interval {:?} at \
             alpha {ALPHA:e}",
            poisson_interval(expected, ALPHA),
        );
        assert!(expected > 1_000.0, "{expected}");
        assert_poisson_count("eruptions of 10⁴ stars", counted, expected, ALPHA);
    }
}
