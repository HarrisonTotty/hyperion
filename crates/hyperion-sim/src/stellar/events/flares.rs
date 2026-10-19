//! Flares: the white-light flares of stars with convective envelopes, the first single-star event
//! kind (plan 06, P06.T28.a).
//!
//! # Who flares
//!
//! A star flares where T25 gives it an [`Activity`]: a pre-main-sequence or main-sequence star
//! below the Kraft break, 1.3 M☉ ([`KRAFT_BREAK_MASS`]), which is the plan's "convective
//! envelopes (below about 1.3 M☉, and pre-main-sequence stars)" as far as
//! [`rotation::activity`] models them. Protostars, pre-main-sequence stars from 1.3 M☉, giants,
//! remnants and objects below 0.1 M☉ have no activity there and so no flares here; plan 06 records
//! them as deferred findings (P06.T28.a), with the sources that would model them.
//!
//! # The rate
//!
//! The cumulative rate of flares of bolometric energy above E, while the star is in a flaring
//! phase, is a power law truncated at [`MAX_ENERGY_JOULES`]:
//!
//! > ν(>E) = ν₀ × [(E ÷ E₀)^−β − (E(max) ÷ E₀)^−β],
//!
//! with β = [`FLARE_INDEX`] = 0.98 and E₀ = [`REFERENCE_ENERGY_JOULES`] = 1.51 × 10³² erg, both
//! from Medina et al. (2020, ApJ 905, 107), and
//!
//! > ν₀ = ν(sat)(M) × (L(X) ÷ L(bol)) ÷ (L(X) ÷ L(bol))(sat),
//!
//! the saturated rate at E₀ for the star's mass times its X-ray activity relative to saturation,
//! 10^(log L(X) ÷ L(bol) + 3.13) from T25's continuous [`Activity::log_lx_lbol`] (Wright et al.
//! 2011): 1 below the Rossby number 0.13, falling as Ro^−2.70 above it, with no step at an
//! [`ActivityLevel`](crate::stellar::rotation::ActivityLevel) band's edge. That the rate of flares
//! above a fixed energy scales with the X-ray luminosity, and saturates where it saturates, is
//! Audard et al.'s (2000, ApJ 541, 396, sections 5.2.1–5.2.2) relation: log N(>10³² erg) =
//! −26.7 + (0.95 ± 0.10) log L(X), "compatible with proportionality", with the rate normalised to
//! the saturation turn-on constant at saturation. Maehara et al. (2015, Earth, Planets and Space
//! 67, 59) find the rate of solar-type stars above 10³³ erg falling about as P^−3 over 2.5–20 d,
//! against the law's P^−2.70 at a fixed mass.
//!
//! The saturated rate ν(sat)(M) comes from two surveys, blended in mass (no one survey gives both
//! ends; see [`saturated_rate`]):
//!
//! - **0.1–0.3 M☉:** e^−1.30 a day above 10^31.5 erg in the TESS band (Medina et al. 2020,
//!   section 6.1, the weighted mean of every star below Ro = 0.1; their "log" is natural, section
//!   5.2.3), held below 0.3 M☉, where they find no dependence on mass;
//! - **1 M☉ and above:** 7.50 a year above 10³³ erg ([`SOLAR_TYPE_SATURATED_PER_YEAR`]), fitted
//!   here to Maehara et al.'s (2015) Fig. 2 with the law's own decline in Rossby number;
//! - between them log ν(sat) is linear in log M.
//!
//! The rate is normalised at the fixed E₀, never at the star's own floor, so that raising a floor
//! removes small flares and leaves the large ones as they were.
//!
//! # The band
//!
//! Energies are bolometric, in joules (erg in the sources). Medina et al.'s TESS-band energies are
//! made bolometric by their own stated method, a 9,000 K blackbody of which the TESS band holds
//! 20.9% ([`TESS_BAND_SHARE`]); Maehara et al.'s are bolometric already (Shibayama et al. 2013,
//! ApJS 209, 5, section 2.3: a 10,000 K blackbody). The two temperatures differ: at 9,000 K
//! Shibayama et al.'s method gives 66% of its 10,000 K energies, so the two anchors' energy scales
//! may differ by up to that.
//!
//! # The floor
//!
//! A flare is an event above its star's floor, [`FlareModel::floor_joules`]: the larger of
//! [`FIXED_FLOOR_JOULES`], the lowest energy Medina et al.'s distribution is fitted to, so that
//! nothing is extrapolated below their data, and the energy at which ν(>E) is 64 a day,
//! [`PoissonBins::MAX_MEAN_PER_BIN`] over a bin of one day. With these constants the fixed floor
//! is the larger for every star (a saturated M dwarf lists 8 flares a day above it); the other
//! keeps the bound at or under 64 a day whatever the constants become.
//!
//! # Time
//!
//! Flares are Poisson bins of [`BIN_SECONDS`] = one day that look back [`LOOK_BACK_BINS`] = one
//! bin (plan 06, design note 15). The activity, and with it ν₀ and the floor, is read once, at the
//! epoch (design note 23): a star with no activity at the epoch has no flares at all, and a star
//! with one flares at the constant rate wherever its phase at t is a flaring one, which
//! [`StarModel::state_at`] gives. The age is read only to find that phase, never to form a fast
//! quantity. A star that enters or leaves the flaring phases inside the source horizon therefore
//! flares only on the epoch's side of the change. Beyond the clock window's end, where a living star's
//! track is not built, the phase is the one at +H.
//!
//! # Marks
//!
//! Each flare's marks come from its own stream, [`BinEvent::marks`], at word 0, so marks never move
//! a time. The layout is fixed:
//!
//! - **word 0:** the energy, by inverse transform of the truncated power law above the floor, from
//!   one uniform in [0, 1) ([`FlareModel::energy_joules_at`]).
//!
//! Nothing else is drawn: the duration and the peak factor are functions of the energy. Scatter
//! added later takes words 1, 2, … in a fixed order.
//!
//! # Duration and light curve
//!
//! The light curve is Davenport et al.'s (2014, ApJ 797, 122) empirical template, a quartic rise
//! (their equation 1) and a two-exponential decay (equation 4) in units of the full width at half
//! maximum t½, from one t½ before the peak to twenty after it, the template's own span; each piece
//! is rescaled so that the curve is continuous, 0 at the onset and the end and 1 at the peak (see
//! [`Flare::luminosity_factor_at`]). t½ follows the energy as Maehara et al.'s (2015, equation 7
//! and Fig. 6) e-folding decay time, τ = 11.1 min × (E ÷ 10³⁴ erg)^0.39, so a flare lasts 21 t½;
//! the relation is fitted to G dwarfs from about 2 × 10³² erg up and is carried, unsourced, to
//! cooler dwarfs and smaller flares.
//! The peak factor, the bolometric luminosity at the peak over the star's quiescent one, is
//! 1 + E ÷ (L(bol) × t½ × A), with A the template's area in t½, so that ∫ ΔL dt = E. The
//! quiescent luminosity is the epoch's, as the activity is.
//!
//! An event may change what a console reads, never what the star's track is (design note 14):
//! nothing here changes the star.

use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow, tags,
};
use crate::math;
use crate::stellar::rotation::{self, Activity, KRAFT_BREAK_MASS, SATURATED_LOG_LX_LBOL};
use crate::stellar::system::StarModel;
use crate::stellar::{Phase, StarState};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR, SOLAR_LUMINOSITY_W};
use crate::units::{SolarLuminosities, SolarMasses};

/// One erg in joules.
const JOULES_PER_ERG: f64 = 1e-7;

/// The length of a flare bin, s: one day (plan 06, P06.T28.a).
pub const BIN_SECONDS: i64 = 86_400;

/// How many whole bins [`flares_active_at`] looks back: one, a day, at least as long as any flare
/// ([`MAX_DURATION`]).
pub const LOOK_BACK_BINS: u32 = 1;

/// The longest a flare may last, one day: the look-back, so that [`flares_active_at`] finds every
/// flare still running.
///
/// A flare that starts in the bin before the look-back's first ends before the bin that holds the
/// instant asked about begins.
pub const MAX_DURATION: Span = Span::from_seconds(BIN_SECONDS);

/// The share of a 9,000 K blackbody's energy in the TESS band, 600–1,000 nm: 20.9%.
///
/// Medina et al. (2020, ApJ 905, 107, section 5.2.2) convert their TESS-band energies to
/// bolometric ones by it. (A blackbody integral over the band at 9,000 K gives 0.2097.)
pub const TESS_BAND_SHARE: f64 = 0.209;

/// β, the index of the cumulative distribution ν(>E) ∝ E^−β: 0.98.
///
/// It is the differential index α = 1.98 ± 0.02 less one, which Medina et al. (2020, section
/// 5.2.4) fit to all 38 of their mid-to-late M dwarfs with five or more flares above 30%
/// completeness, by maximum likelihood with the completeness folded in. Kepler gives GJ 1243 the
/// same: a cumulative slope of −0.98 (Davenport 2016, ApJ 829, 23, Fig. 6) and −1.01 (Hawley et
/// al. 2014, ApJ 797, 121, table 3).
pub const FLARE_INDEX: f64 = 0.98;

/// E₀, the energy at which the rate is normalised, J: 1.51 × 10³² erg bolometric.
///
/// It is Medina et al.'s (2020, section 5.2.2) 10^31.5 erg in the TESS band over
/// [`TESS_BAND_SHARE`], the threshold of their rate R31.5, at which every one of their stars is at
/// least 50% complete.
pub const REFERENCE_ENERGY_JOULES: f64 = 1.51e32 * JOULES_PER_ERG;

/// The fixed floor, J: 4.78 × 10³⁰ erg bolometric, 10³⁰ erg in the TESS band over
/// [`TESS_BAND_SHARE`].
///
/// It is where the nearest star of Medina et al. (2020, Fig. 5), Proxima Centauri, reaches 30%
/// completeness, the threshold above which they fit [`FLARE_INDEX`] (section 5.2.4), and so the
/// lowest energy their fit reaches (read from the figure; Proxima's 50% point is 10^30.16 erg, and
/// every other star's lies at 10^30.8–10^31.5 erg).
pub const FIXED_FLOOR_JOULES: f64 = 1e30 / TESS_BAND_SHARE * JOULES_PER_ERG;

/// The largest flare, J: 10³⁶ erg bolometric.
///
/// It is the top of the ranges to which Maehara et al. (2015, Earth, Planets and Space 67, 59:
/// 4 × 10³³–10³⁶ erg) and Yang and Liu (2019, ApJS 241, 29, Fig. 3: 10^34.5–10³⁶ erg for F and G
/// stars) fit their distributions. The most energetic Kepler superflares on G dwarfs are
/// 1.1–1.3 × 10³⁶ erg, on stars rotating in 0.8–3.9 d, and those on Sun-like stars (P > 10 d)
/// reach 5.2 × 10³⁵ erg (Shibayama et al. 2013, ApJS 209, 5, Figs. 3 and 4 and table 3). It also
/// keeps every flare under [`MAX_DURATION`].
pub const MAX_ENERGY_JOULES: f64 = 1e36 * JOULES_PER_ERG;

/// ln of the saturated rate of mid-to-late M dwarfs above [`REFERENCE_ENERGY_JOULES`], per day:
/// −1.30 ± 0.08, the weighted mean of Medina et al.'s (2020, section 6.1) stars below Ro = 0.1
/// (0.27 a day). Their "log" is the natural logarithm (section 5.2.3).
const LN_SATURATED_RATE_PER_DAY_LOW_MASS: f64 = -1.30;

/// The mass at and below which the saturated rate is Medina et al.'s: 0.3 M☉, the top of their
/// 0.1–0.3 M☉ sample, within which they find no dependence of the rate on mass (section 6.1).
const LOW_ANCHOR_MASS: SolarMasses = SolarMasses::new(0.3);

/// The mass at and above which the saturated rate is the solar-type stars': 1.0 M☉, the middle of
/// Maehara et al.'s (2015) 5,300–6,300 K main-sequence sample.
const HIGH_ANCHOR_MASS: SolarMasses = SolarMasses::new(1.0);

/// The saturated rate of a 1 M☉ star above [`SOLAR_TYPE_ANCHOR_JOULES`], per Julian year: 7.50.
///
/// Fitted here to Maehara et al.'s (2015, Fig. 2) rates of flares above 10³³ erg on solar-type
/// stars by rotation period, read from the figure as 2.21, 0.149 and 0.0279 a star a year at
/// 2.5–5, 5–10 and 10–20 d: each over the law's decline (P ÷ 0.13 τ)^−2.70 at the bin's
/// geometric-mean period, with Wright et al.'s (2011, equation 11) τ = 14.45 d at 1 M☉, gives
/// 12.2, 5.34 and 6.49, whose geometric mean this is. Their bin below 2.5 d (0.68) is not used:
/// it lies below the 2.5–5 d bin, which no saturating law can follow. The figure is an
/// extrapolation to saturation, 3.4 times their highest bin; no bin observes it.
pub const SOLAR_TYPE_SATURATED_PER_YEAR: f64 = 7.50;

/// The energy of [`SOLAR_TYPE_SATURATED_PER_YEAR`], J: 10³³ erg (Maehara et al. 2015, Fig. 2).
pub const SOLAR_TYPE_ANCHOR_JOULES: f64 = 1e33 * JOULES_PER_ERG;

/// Maehara et al.'s (2015, equation 7 and Fig. 6) e-folding decay time at 10³⁴ erg, s: 11.1 min.
///
/// They print only the slope, [`DURATION_INDEX`]; the normalisation is read from the figure's
/// regression line (1.9 min at 10³² erg, 160 min at 10³⁷ erg). The relation is fitted to G dwarfs
/// (5,300–6,300 K) from about 2 × 10³² to 10³⁶ erg; it is carried here to K and M dwarfs and down
/// to the floor without a source of its own.
const DECAY_TIME_AT_1E34_ERG_SECONDS: f64 = 666.0;

/// The energy of [`DECAY_TIME_AT_1E34_ERG_SECONDS`], J: 10³⁴ erg.
const DECAY_TIME_ANCHOR_JOULES: f64 = 1e34 * JOULES_PER_ERG;

/// The power of the energy in the duration: 0.39 ± 0.03 (Maehara et al. 2015, equation 7), close
/// to the 1/3 that reconnection gives and solar flares show.
pub const DURATION_INDEX: f64 = 0.39;

/// Where the template, from its peak, has fallen to 1/e: 1.00995 t½. Maehara et al.'s duration is
/// the e-folding decay time, so t½ is it over this.
const E_FOLD_HALF_WIDTHS: f64 = 1.00995;

/// The rise, t½: the template starts one full width at half maximum before its peak (Davenport et
/// al. 2014, equation 1, valid for −1 < t ÷ t½ ≤ 0).
const RISE_HALF_WIDTHS: f64 = 1.0;

/// The decay, t½: the template is followed to twenty full widths after its peak, the span over
/// which Davenport et al. (2014, section 3) built it.
const DECAY_HALF_WIDTHS: f64 = 20.0;

/// The longest t½, s, that keeps a flare of 21 t½ within [`MAX_DURATION`].
const MAX_HALF_WIDTH_SECONDS: f64 = 86_400.0 / (RISE_HALF_WIDTHS + DECAY_HALF_WIDTHS);

/// The rise's coefficients of x, x², x³ and x⁴, x = t ÷ t½ from the peak: Davenport et al. (2014,
/// equation 1), F = 1 + 1.941x − 0.175x² − 2.246x³ − 1.125x⁴.
const RISE: [f64; 4] = [1.941, -0.175, -2.246, -1.125];

/// The decay's two exponentials, (amplitude, rate in 1 ÷ t½): Davenport et al. (2014, equation 4),
/// F = 0.6890 e^(−1.600x) + 0.3030 e^(−0.2783x).
const DECAY: [(f64, f64); 2] = [(0.6890, 1.600), (0.3030, 0.2783)];

/// The rise polynomial at `x` half widths from the peak, in Horner's form.
#[must_use]
const fn rise_polynomial(x: f64) -> f64 {
    1.0 + x * (RISE[0] + x * (RISE[1] + x * (RISE[2] + x * RISE[3])))
}

/// The rise polynomial at the onset, 0.005: the printed fit misses zero there by half a percent.
const RISE_AT_ONSET: f64 = rise_polynomial(-RISE_HALF_WIDTHS);

/// The decay's two exponentials at `x` half widths after the peak.
#[must_use]
fn decay_sum(x: f64) -> f64 {
    DECAY[0].0 * math::exp(-DECAY[0].1 * x) + DECAY[1].0 * math::exp(-DECAY[1].1 * x)
}

/// The decay at the peak, 0.992: the printed fit misses one there by under a percent.
const DECAY_AT_PEAK: f64 = DECAY[0].0 + DECAY[1].0;

/// The template's profile at `x` half widths from its peak: 0 at and before the onset (x = −1), 1
/// at the peak, 0 at and after the end (x = 20).
///
/// Each of Davenport et al.'s pieces is shifted and scaled to meet its ends exactly, the rise as
/// (F − F(−1)) ÷ (1 − F(−1)) and the decay as (F − F(20)) ÷ (F(0) − F(20)), which moves neither by
/// more than 0.8% of the peak.
#[must_use]
fn profile(x: f64) -> f64 {
    if x <= -RISE_HALF_WIDTHS || x >= DECAY_HALF_WIDTHS {
        0.0
    } else if x <= 0.0 {
        (rise_polynomial(x) - RISE_AT_ONSET) / (1.0 - RISE_AT_ONSET)
    } else {
        let end = decay_sum(DECAY_HALF_WIDTHS);
        let level = (decay_sum(x) - end) / (DECAY_AT_PEAK - end);
        if level > 0.0 { level } else { 0.0 }
    }
}

/// The area under [`profile`], in t½: 1.810, of which the rise holds 0.304.
#[must_use]
fn profile_area() -> f64 {
    // ∫ from −1 to 0 of the rise polynomial: −[x + 1.941x²/2 − 0.175x³/3 − 2.246x⁴/4 − 1.125x⁵/5]
    // at −1.
    let x = -RISE_HALF_WIDTHS;
    let antiderivative = x
        * (1.0
            + x * (RISE[0] / 2.0 + x * (RISE[1] / 3.0 + x * (RISE[2] / 4.0 + x * RISE[3] / 5.0))));
    let rise = (-antiderivative - RISE_AT_ONSET * RISE_HALF_WIDTHS) / (1.0 - RISE_AT_ONSET);
    let span = DECAY_HALF_WIDTHS;
    let end = decay_sum(span);
    let exponentials = DECAY[0].0 / DECAY[0].1 * -math::exp_m1(-DECAY[0].1 * span)
        + DECAY[1].0 / DECAY[1].1 * -math::exp_m1(-DECAY[1].1 * span);
    let decay = (exponentials - end * span) / (DECAY_AT_PEAK - end);
    rise + decay
}

/// The template's full width at half maximum for a flare of `energy_joules`, s: Maehara et al.'s
/// e-folding decay time over [`E_FOLD_HALF_WIDTHS`], held to the longest that keeps the flare
/// within [`MAX_DURATION`].
///
/// # Panics
///
/// In debug builds, if the relation gives a flare longer than [`MAX_DURATION`], which no energy up
/// to [`MAX_ENERGY_JOULES`] does: at the cap the flare would keep its energy and its template, its
/// time scale held at the cap and its peak raised to keep ∫ ΔL dt = E.
#[must_use]
fn half_width_seconds(energy_joules: f64) -> f64 {
    let decay = DECAY_TIME_AT_1E34_ERG_SECONDS
        * math::powf(energy_joules / DECAY_TIME_ANCHOR_JOULES, DURATION_INDEX);
    let half_width = decay / E_FOLD_HALF_WIDTHS;
    debug_assert!(
        half_width <= MAX_HALF_WIDTH_SECONDS,
        "a flare of {energy_joules:e} J lasts 21 × {half_width} s, beyond the look-back of a day"
    );
    half_width.min(MAX_HALF_WIDTH_SECONDS)
}

/// The saturated rate of flares above [`REFERENCE_ENERGY_JOULES`] of a star of `mass`, the rate
/// of a star whose Rossby number is at most 0.13.
///
/// It is Medina et al.'s (2020) e^−1.30 a day at and below 0.3 M☉, and the solar-type stars'
/// [`SOLAR_TYPE_SATURATED_PER_YEAR`] above 10³³ erg, carried to E₀ by the power law (0.131 a
/// day), at and above 1 M☉. Between them log ν(sat) is linear in log M: this model's own
/// interpolation between its two sourced anchors (plan 06, P06.T28.a; provisional), from 0.27 to
/// 0.13 a day, a factor of 2.1.
#[must_use]
pub fn saturated_rate(mass: SolarMasses) -> EventsPerSecond {
    let low = math::exp(LN_SATURATED_RATE_PER_DAY_LOW_MASS);
    let high = SOLAR_TYPE_SATURATED_PER_YEAR * SECONDS_PER_DAY / SECONDS_PER_JULIAN_YEAR
        * math::powf(
            SOLAR_TYPE_ANCHOR_JOULES / REFERENCE_ENERGY_JOULES,
            FLARE_INDEX,
        );
    let (lo, hi) = (LOW_ANCHOR_MASS.value(), HIGH_ANCHOR_MASS.value());
    let m = mass.value().clamp(lo, hi);
    let share = math::log10(m / lo) / math::log10(hi / lo);
    EventsPerSecond::per_day(math::exp10(math::mul_add(
        share,
        math::log10(high) - math::log10(low),
        math::log10(low),
    )))
}

/// The floor, J, and the rate of flares above it, of a star whose untruncated rate above
/// [`REFERENCE_ENERGY_JOULES`] is `normalisation`, per second.
///
/// The floor is [`FIXED_FLOOR_JOULES`] unless the rate above it would exceed 64 a day
/// ([`PoissonBins::MAX_MEAN_PER_BIN`] over a day's bin); then it is the energy at which the rate is
/// 64 a day, and the rate is held to that.
#[must_use]
fn floor_and_rate(normalisation: f64) -> (f64, EventsPerSecond) {
    let tail = shape(MAX_ENERGY_JOULES);
    let cap = PoissonBins::MAX_MEAN_PER_BIN / SECONDS_PER_DAY;
    let fixed = normalisation * (shape(FIXED_FLOOR_JOULES) - tail);
    if fixed <= cap {
        return (FIXED_FLOOR_JOULES, EventsPerSecond::new(fixed));
    }
    let floor =
        REFERENCE_ENERGY_JOULES * math::powf(cap / normalisation + tail, -1.0 / FLARE_INDEX);
    let listed = (normalisation * (shape(floor) - tail)).min(cap);
    (floor, EventsPerSecond::new(listed))
}

/// Whether a star in `state` is in a phase in which T25 gives it an activity: before the end of
/// its main sequence (pre-main-sequence or main-sequence) and below the Kraft break.
#[must_use]
fn in_flaring_phase(state: &StarState) -> bool {
    matches!(state.phase(), Phase::PreMainSequence | Phase::MainSequence)
        && state.mass() < KRAFT_BREAK_MASS
}

/// `(E ÷ E₀)^−β`, the untruncated cumulative shape at `energy_joules`.
#[must_use]
fn shape(energy_joules: f64) -> f64 {
    math::powf(energy_joules / REFERENCE_ENERGY_JOULES, -FLARE_INDEX)
}

/// The flare bins: a day long, looking back one.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("a day lies between 16 s and 2^44 s")
}

/// The flares of one star: its rate, its floor and what its flares are drawn from, read at the
/// epoch (plan 06, P06.T28.a; see the [module documentation](self)).
///
/// It is the [`RateModel`] of the star's `star.ev.flare` series: its [`bound`](RateModel::bound)
/// is the constant [`listed_rate`](Self::listed_rate) in every bin, and its
/// [`rate`](RateModel::rate) at t is that where the star is in a flaring phase at t and 0
/// elsewhere.
///
/// # Examples
///
/// A Sun-like star flares above its floor about once a year, and the large flares a console would
/// call superflares are rare:
///
/// ```
/// use hyperion_sim::events::EventsPerSecond;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::flares::FlareModel;
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = StarModel::new(SolarMasses::new(1.0), Composition::SOLAR, StarDraws::median(), Years::new(4.57e9))?;
/// let flares = FlareModel::of(&sun).ok_or("the Sun has a convective envelope")?;
/// let per_year = |r: EventsPerSecond| r.value() * 31_557_600.0;
/// assert!((0.3..3.0).contains(&per_year(flares.listed_rate())));
/// // Above 10³⁴ erg (10²⁷ J): once in one or two thousand years.
/// assert!((1e-4..1e-3).contains(&per_year(flares.rate_above(1e27))));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlareModel<'a> {
    star: &'a StarModel,
    activity: Activity,
    luminosity: SolarLuminosities,
    /// ν₀, the untruncated rate above E₀.
    normalisation: EventsPerSecond,
    floor_joules: f64,
    listed: EventsPerSecond,
}

impl<'a> FlareModel<'a> {
    /// The flares of `star`, or `None` for a star with no activity at the epoch (see "Who flares"
    /// in the [module documentation](self)): not formed by then, a protostar, a star from 1.3 M☉,
    /// one past its main sequence, a remnant or an object below 0.1 M☉.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let state = star.state_at(UniverseTime::EPOCH)?;
        if !in_flaring_phase(&state) {
            return None;
        }
        let activity = rotation::activity(&state, star.rotation_at(UniverseTime::EPOCH).as_ref())?;
        let relative = math::exp10(activity.log_lx_lbol() - SATURATED_LOG_LX_LBOL);
        let normalisation = saturated_rate(state.mass()).value() * relative;
        let (floor_joules, listed) = floor_and_rate(normalisation);
        Some(Self {
            star,
            activity,
            luminosity: state.luminosity(),
            normalisation: EventsPerSecond::new(normalisation),
            floor_joules,
            listed,
        })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The star's activity at the epoch, which sets its rate (T25).
    #[must_use]
    pub const fn activity(&self) -> Activity {
        self.activity
    }

    /// The star's quiescent bolometric luminosity at the epoch, L☉, which the peak factors are
    /// relative to.
    #[must_use]
    pub const fn quiescent_luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// β, the index of the cumulative distribution: [`FLARE_INDEX`], the same for every star.
    #[must_use]
    pub const fn index(&self) -> f64 {
        FLARE_INDEX
    }

    /// The floor, J: the bolometric energy above which a flare is listed.
    ///
    /// Flares below it happen in the law but are left out of every listing; plan 12 reads this to
    /// say so.
    #[must_use]
    pub const fn floor_joules(&self) -> f64 {
        self.floor_joules
    }

    /// The rate of listed flares, those above the floor, while the star is in a flaring phase:
    /// ν(>floor), at most 64 a day.
    #[must_use]
    pub const fn listed_rate(&self) -> EventsPerSecond {
        self.listed
    }

    /// ν(>E), the law's rate of flares of bolometric energy above `energy_joules` while the star
    /// is in a flaring phase, whatever the floor; zero from [`MAX_ENERGY_JOULES`].
    ///
    /// # Panics
    ///
    /// In debug builds, unless `energy_joules` is finite and positive.
    #[must_use]
    pub fn rate_above(&self, energy_joules: f64) -> EventsPerSecond {
        debug_assert!(
            energy_joules.is_finite() && energy_joules > 0.0,
            "a flare energy is finite and positive, got {energy_joules}"
        );
        if energy_joules >= MAX_ENERGY_JOULES {
            return EventsPerSecond::ZERO;
        }
        EventsPerSecond::new(
            self.normalisation.value() * (shape(energy_joules) - shape(MAX_ENERGY_JOULES)),
        )
    }

    /// Whether the star is in a flaring phase at `t`: a pre-main-sequence or main-sequence star
    /// below 1.3 M☉ by [`StarModel::state_at`], read at +H for a `t` beyond the clock window.
    #[must_use]
    pub fn flares_at(&self, t: UniverseTime) -> bool {
        self.star
            .state_at(t.min(ClockWindow::END))
            .is_some_and(|state| in_flaring_phase(&state))
    }

    /// The energy of the flare whose word 0 is `u`, J, by the inverse of the truncated power law
    /// above the floor.
    ///
    /// E = floor × [1 − u (1 − (E(max) ÷ floor)^−β)]^(−1 ÷ β), from the floor at u = 0 towards
    /// [`MAX_ENERGY_JOULES`] as u nears 1.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `u` lies in [0, 1).
    #[must_use]
    pub fn energy_joules_at(&self, u: f64) -> f64 {
        debug_assert!((0.0..1.0).contains(&u), "a uniform lies in [0, 1), got {u}");
        let kept = math::powf(MAX_ENERGY_JOULES / self.floor_joules, -FLARE_INDEX);
        let energy = self.floor_joules * math::powf(1.0 - u * (1.0 - kept), -1.0 / FLARE_INDEX);
        energy.min(MAX_ENERGY_JOULES)
    }

    /// The flare of `event`, an event of this model's series: its energy from word 0 of its marks,
    /// and its duration and peak factor from the energy.
    #[must_use]
    fn flare_of(&self, event: BinEvent) -> Flare {
        let id = event.id();
        let onset = event.time();
        let mut marks = event.into_marks();
        let energy_joules = self.energy_joules_at(marks.uniform());
        let half_width = half_width_seconds(energy_joules);
        let duration = Span::from_seconds_f64((RISE_HALF_WIDTHS + DECAY_HALF_WIDTHS) * half_width)
            .expect("a flare lasts under a day");
        debug_assert!(duration <= MAX_DURATION, "a flare lasts {duration}");
        let luminosity_watts = self.luminosity.value() * SOLAR_LUMINOSITY_W;
        Flare {
            id,
            onset,
            energy_joules,
            half_width_seconds: half_width,
            duration,
            peak_factor: 1.0 + energy_joules / (luminosity_watts * half_width * profile_area()),
        }
    }
}

impl RateModel for FlareModel<'_> {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.listed
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        if self.flares_at(t) {
            self.listed
        } else {
            EventsPerSecond::ZERO
        }
    }
}

/// One flare: its ID, onset, bolometric energy, duration and peak, and its light curve.
///
/// Its luminosity factor is 1 before the onset and from the end, and in between follows Davenport
/// et al.'s (2014) template ([`luminosity_factor_at`](Self::luminosity_factor_at)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Flare {
    id: EventId,
    onset: UniverseTime,
    energy_joules: f64,
    half_width_seconds: f64,
    duration: Span,
    peak_factor: f64,
}

impl Flare {
    /// The flare's ID, from which [`flare`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset, the template's start, one t½ before the peak.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The bolometric energy radiated, J (erg in the sources: 1 J = 10⁷ erg), from the star's
    /// floor up to [`MAX_ENERGY_JOULES`].
    #[must_use]
    pub const fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    /// The template's full width at half maximum t½, s.
    #[must_use]
    pub const fn half_width_seconds(&self) -> f64 {
        self.half_width_seconds
    }

    /// The duration from the onset to the end, 21 t½ floored to the nanosecond, under
    /// [`MAX_DURATION`].
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
            .expect("a flare ends within a day of an onset the clock holds")
    }

    /// The peak factor, the bolometric luminosity at the peak over the star's quiescent luminosity
    /// at the epoch: 1 + ΔL ÷ L, above 1.
    #[must_use]
    pub const fn peak_factor(&self) -> f64 {
        self.peak_factor
    }

    /// Whether the flare is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        t.checked_since(self.onset)
            .is_some_and(|elapsed| !elapsed.is_negative() && elapsed < self.duration)
    }

    /// The star's bolometric luminosity at `t` over its quiescent one, as this flare makes it.
    ///
    /// It is 1 before the onset and from the end, and in between 1 + (peak − 1) × p(x), p
    /// Davenport et al.'s (2014) template at x = (t − onset) ÷ t½ − 1 half widths from the peak,
    /// rescaled to be continuous (0 at the onset and the end, 1 at the peak). It is continuous in
    /// `t`, and ∫ (factor − 1) dt over the flare is E ÷ L(bol). P06.T28.g composes it into the
    /// star's summary.
    #[must_use]
    pub fn luminosity_factor_at(&self, t: UniverseTime) -> f64 {
        let Some(elapsed) = t.checked_since(self.onset) else {
            return 1.0;
        };
        if elapsed.is_negative() || elapsed >= self.duration {
            return 1.0;
        }
        let x = elapsed.as_seconds_f64() / self.half_width_seconds - RISE_HALF_WIDTHS;
        1.0 + (self.peak_factor - 1.0) * profile(x)
    }
}

/// Appends to `out` the flares of `star` with onsets in `window`, sorted by onset and then by ID.
///
/// A star that does not flare ([`FlareModel::of`]) has none. `series` is the star's `star.ev.flare` series, `EventSeries::new(seed, tags::STAR_FLARE,
/// body.into())`: [`StarModel`] holds no seed or ID, so it comes in beside the star, as for
/// [`light_factor_at`](crate::stellar::variability::light_factor_at). Every bin the window touches
/// is generated whole, so a listing over a window is the union of the listings over any partition
/// of it, in any order of asking. `out` is only appended to; nothing in it is read.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FLARE`].
///
/// # Examples
///
/// A saturated M dwarf's flares over a week, and the largest of them regenerated from its ID
/// alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::flares::{flare, flares_in};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A 0.2 M☉ dwarf at 300 Myr, fully convective and still spinning fast enough to saturate.
/// let dwarf = StarModel::new(SolarMasses::new(0.2), Composition::SOLAR, StarDraws::median(), Years::new(3e8))?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = EventSeries::new(Seed::new(7), tags::STAR_FLARE, body.into());
/// let week_end = UniverseTime::EPOCH.checked_add(Span::from_seconds(7 * 86_400)).ok_or("in range")?;
///
/// let mut flares = Vec::new();
/// flares_in(&dwarf, &series, TimeWindow::new(UniverseTime::EPOCH, week_end)?, &mut flares);
/// // About eight a day above the floor of 4.8 × 10³⁰ erg.
/// assert!((25..110).contains(&flares.len()));
/// let largest = flares
///     .iter()
///     .max_by(|a, b| a.energy_joules().total_cmp(&b.energy_joules()))
///     .ok_or("a saturated dwarf flares in a week")?;
/// assert_eq!(flare(&dwarf, &series, largest.id()).as_ref(), Some(largest));
/// assert!(largest.peak_factor() > 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn flares_in(star: &StarModel, series: &EventSeries, window: TimeWindow, out: &mut Vec<Flare>) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FLARE,
        "a star's flares are its star.ev.flare series"
    );
    let Some(model) = FlareModel::of(star) else {
        return;
    };
    let mut events = Vec::new();
    bins().events_in(series, window, &model, &mut events);
    out.extend(events.into_iter().map(|event| model.flare_of(event)));
}

/// Appends to `out` the flares of `star` running at `t`, sorted by onset and then by ID.
///
/// They are those with an onset from the start of the bin before `t`'s up to `t` whose end is
/// after `t`; every flare lasts under the look-back, so none is missed. `series` is as for
/// [`flares_in`]. The result depends on `star`, `series` and `t` alone, never on what was asked
/// before.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FLARE`].
pub fn flares_active_at(
    star: &StarModel,
    series: &EventSeries,
    t: UniverseTime,
    out: &mut Vec<Flare>,
) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FLARE,
        "a star's flares are its star.ev.flare series"
    );
    let Some(model) = FlareModel::of(star) else {
        return;
    };
    let mut events = Vec::new();
    bins().active_at(series, t, &model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(|event| model.flare_of(event))
            .filter(|flare| flare.is_active_at(t)),
    );
}

/// The flare `id` names, regenerated on its own.
///
/// `None` if `id` is not of `series`, names a candidate that was rejected (one outside the star's
/// flaring phases) or a number beyond its bin's count, or `star` does not flare.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FLARE`].
#[must_use]
pub fn flare(star: &StarModel, series: &EventSeries, id: EventId) -> Option<Flare> {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FLARE,
        "a star's flares are its star.ev.flare series"
    );
    let model = FlareModel::of(star)?;
    bins()
        .event(series, id, &model)
        .map(|event| model.flare_of(event))
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
    use crate::events::testing::{ConstantRate, assert_partition_independent, partition};
    use crate::events::{EventBin, event_id};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::{Galaxy, POPULATIONS};
    use crate::id::{BodyId, SystemId, event_tags};
    use crate::rng::Seed;
    use crate::stellar::Composition;
    use crate::stellar::draws::{StarDraws, StarDrawsParts, UnitUniform};
    use crate::stellar::premain::PROTOSTAR_YEARS;
    use crate::stellar::rotation::{ACTIVITY_SLOPE, SATURATION_ROSSBY};
    use crate::stellar::sse::Track;
    use crate::stellar::testing::sample_population_star;
    use crate::time::SourceHorizon;
    use crate::units::{Dex, HeliumExcess, Years};

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000a);
    const DAY: i64 = BIN_SECONDS;

    fn star(mass: f64, age: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(mass),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(age),
        )
        .unwrap()
    }

    /// A saturated M dwarf: 0.2 M☉ at 300 Myr, at the median draws.
    fn m_dwarf() -> StarModel {
        star(0.2, 3e8)
    }

    /// The Sun's mass and age at the median draws.
    fn sun() -> StarModel {
        star(1.0, 4.57e9)
    }

    /// A pre-main-sequence K star: 0.8 M☉ at 10 Myr.
    fn young_k() -> StarModel {
        star(0.8, 1e7)
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    fn series(index: u16) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_FLARE, body(index).into())
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

    fn list(star: &StarModel, series: &EventSeries, w: TimeWindow) -> Vec<Flare> {
        let mut out = Vec::new();
        flares_in(star, series, w, &mut out);
        out
    }

    fn running(star: &StarModel, series: &EventSeries, t: UniverseTime) -> Vec<Flare> {
        let mut out = Vec::new();
        flares_active_at(star, series, t, &mut out);
        out
    }

    fn per_day(rate: EventsPerSecond) -> f64 {
        rate.value() * SECONDS_PER_DAY
    }

    /// The earliest instant in `(from, to]` at which the star flares, to the nanosecond, for a
    /// star that does not flare at `from` and does at `to`.
    fn first_flaring(model: &FlareModel<'_>, from: UniverseTime, to: UniverseTime) -> UniverseTime {
        let nanos = |t: UniverseTime| {
            i128::from(t.seconds()) * 1_000_000_000 + i128::from(t.subsec_nanos())
        };
        let time = |n: i128| {
            at(
                i64::try_from(n.div_euclid(1_000_000_000)).unwrap(),
                u32::try_from(n.rem_euclid(1_000_000_000)).unwrap(),
            )
        };
        assert!(!model.flares_at(from) && model.flares_at(to));
        let (mut lo, mut hi) = (nanos(from), nanos(to));
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if model.flares_at(time(mid)) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        time(hi)
    }

    /// The template meets 0 at the onset and the end and 1 at the peak, its area is the closed
    /// form's, and it falls to 1/e of its peak at [`E_FOLD_HALF_WIDTHS`].
    #[test]
    fn the_template_meets_its_ends_and_has_its_own_area_and_e_fold() {
        assert!((RISE_AT_ONSET - 0.005).abs() < 1e-12, "{RISE_AT_ONSET}");
        assert!((DECAY_AT_PEAK - 0.992).abs() < 1e-15, "{DECAY_AT_PEAK}");
        assert_same_bits(profile(-RISE_HALF_WIDTHS), 0.0);
        assert_same_bits(profile(0.0), 1.0);
        assert_same_bits(profile(DECAY_HALF_WIDTHS), 0.0);
        assert!(profile(-1.0 + 1e-9) < 1e-8 && profile(1e-9) > 1.0 - 1e-8);
        assert!(profile(DECAY_HALF_WIDTHS - 1e-9) < 1e-9);
        // Rising, then falling, and never outside [0, 1].
        let xs: Vec<f64> = (0..=21_000)
            .map(|i| -1.0 + f64::from(i) / 1_000.0)
            .collect();
        for pair in xs.windows(2) {
            let (a, b) = (profile(pair[0]), profile(pair[1]));
            assert!((0.0..=1.0).contains(&b));
            if pair[1] <= 0.0 {
                assert!(b >= a, "the rise falls at {}", pair[1]);
            } else if pair[0] >= 0.0 {
                assert!(b <= a, "the decay rises at {}", pair[1]);
            }
        }
        // The area by Simpson's rule on each piece.
        let simpson = |lo: f64, hi: f64, n: u32| {
            let h = (hi - lo) / f64::from(n);
            let mut sum = profile(lo) + profile(hi);
            for i in 1..n {
                let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
                sum += weight * profile(lo + h * f64::from(i));
            }
            sum * h / 3.0
        };
        let numeric = simpson(-1.0, 0.0, 2_000) + simpson(0.0, DECAY_HALF_WIDTHS, 40_000);
        let area = profile_area();
        assert!((numeric - area).abs() < 1e-10, "{numeric} against {area}");
        assert!((area - 1.810).abs() < 5e-4, "{area}");
        // The e-fold, by bisection.
        let (mut lo, mut hi) = (0.0_f64, 5.0_f64);
        for _ in 0..80 {
            let mid = f64::midpoint(lo, hi);
            if profile(mid) > math::exp(-1.0) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        assert!(
            (lo - E_FOLD_HALF_WIDTHS).abs() < 1e-4,
            "the template's e-fold is at {lo} t½"
        );
    }

    /// [`SOLAR_TYPE_SATURATED_PER_YEAR`] is the fit to Maehara et al.'s (2015) Fig. 2 that its
    /// documentation describes, with T25's constants; the blend runs between its two anchors.
    #[test]
    fn the_solar_type_rate_is_the_fit_to_maehara_et_al_s_fig_2() {
        let tau = math::exp10(1.16);
        let p_sat = SATURATION_ROSSBY * tau;
        let bins: [(f64, f64, f64); 3] =
            [(2.5, 5.0, 2.21), (5.0, 10.0, 0.149), (10.0, 20.0, 0.0279)];
        let mean_log = bins
            .iter()
            .map(|&(a, b, nu)| math::log10(nu / math::powf((a * b).sqrt() / p_sat, ACTIVITY_SLOPE)))
            .sum::<f64>()
            / 3.0;
        let fit = math::exp10(mean_log);
        assert!((fit - SOLAR_TYPE_SATURATED_PER_YEAR).abs() < 0.01, "{fit}");
        // The blend's two ends, at E₀: Medina et al.'s 0.27 a day, the solar-type 0.13 a day.
        let rate = |mass: f64| per_day(saturated_rate(SolarMasses::new(mass)));
        let low = rate(0.2);
        assert!((low / math::exp(-1.30) - 1.0).abs() < 1e-12, "{low}");
        assert_same_bits(low, rate(0.1));
        let high = rate(1.0);
        assert!((high - 0.1309).abs() < 5e-4, "{high}");
        assert_same_bits(high, rate(1.29));
        let mid = rate(0.6);
        assert!(high < mid && mid < low, "{mid}");
    }

    /// The floor is the fixed one while the rate above it stays at or under 64 a day; above that
    /// it rises to the energy of 64 a day, and the rate is held there.
    #[test]
    fn the_floor_rises_to_hold_64_flares_a_day() {
        let tail = shape(MAX_ENERGY_JOULES);
        let at_fixed = |nu0: f64| nu0 * (shape(FIXED_FLOOR_JOULES) - tail) * SECONDS_PER_DAY;
        let quiet = 1e-6;
        let (floor, listed) = floor_and_rate(quiet);
        assert_same_bits(floor, FIXED_FLOOR_JOULES);
        assert_same_bits(per_day(listed), at_fixed(quiet));
        // Ten times the rate that gives exactly 64 a day above the fixed floor.
        let busy = 10.0 * 64.0 / at_fixed(1.0);
        let (floor, listed) = floor_and_rate(busy);
        assert!(floor > FIXED_FLOOR_JOULES, "{floor}");
        assert!((per_day(listed) - 64.0).abs() < 1e-9, "{listed:?}");
        assert!(per_day(listed) <= PoissonBins::MAX_MEAN_PER_BIN);
        // A tenth of the rate is 10^(1/β) times the energy.
        let ratio = floor / FIXED_FLOOR_JOULES / math::powf(10.0, 1.0 / FLARE_INDEX);
        assert!((ratio - 1.0).abs() < 1e-3, "{ratio}");
    }

    /// Cool dwarfs flare, pre-main-sequence ones included; a star above 1.3 M☉, a
    /// pre-main-sequence star above it, a giant, a white dwarf, a brown dwarf, a protostar and an
    /// unformed star do not.
    #[test]
    fn cool_dwarfs_flare_and_hot_evolved_dead_and_substellar_objects_do_not() {
        for (mass, age) in [
            (0.1, 1e9),
            (0.2, 3e8),
            (0.5, 2e9),
            (1.0, 4.57e9),
            (1.29, 1e9),
            (0.8, 1e7),
        ] {
            let model = star(mass, age);
            assert!(FlareModel::of(&model).is_some(), "{mass} M☉ at {age} yr");
        }
        // The Sun's model is built only to +H, so its main sequence's end comes from a full track.
        let ms_end = Track::full(
            SolarMasses::new(1.0),
            &Composition::SOLAR,
            &StarDraws::median(),
        )
        .main_sequence_end()
        .unwrap()
        .value();
        let cases = [
            (
                "above the Kraft break",
                star(1.5, 1e9),
                &[Phase::MainSequence][..],
            ),
            (
                "pre-main-sequence above it",
                star(2.0, 1e6),
                &[Phase::PreMainSequence][..],
            ),
            (
                "a giant",
                star(1.0, ms_end + 5e8),
                &[
                    Phase::HertzsprungGap,
                    Phase::FirstGiantBranch,
                    Phase::CoreHeliumBurning,
                ][..],
            ),
            (
                "a white dwarf",
                star(3.0, 1e9),
                &[Phase::CarbonOxygenWhiteDwarf, Phase::OxygenNeonWhiteDwarf][..],
            ),
            ("a brown dwarf", star(0.05, 1e9), &[Phase::Substellar][..]),
            ("a protostar", star(1.0, 1e5), &[Phase::Protostar][..]),
        ];
        for (name, model, phases) in cases {
            let phase = model.state_at(UniverseTime::EPOCH).unwrap().phase();
            assert!(phases.contains(&phase), "{name} is {phase:?}");
            assert_eq!(FlareModel::of(&model), None, "{name}");
            assert!(list(&model, &series(0), days(0, 30)).is_empty(), "{name}");
        }
        assert_eq!(
            FlareModel::of(&star(0.5, -10.0)),
            None,
            "a star that forms after the epoch"
        );
    }

    /// The phase rule is T25's: a star is in a flaring phase at t exactly where
    /// [`rotation::activity`] gives it an activity then.
    #[test]
    fn the_flaring_phase_is_where_t25_gives_an_activity() {
        let times = [
            SourceHorizon::START,
            ClockWindow::START,
            UniverseTime::EPOCH,
            ClockWindow::END,
        ];
        let activity_at = |model: &StarModel, t: UniverseTime| {
            model
                .state_at(t)
                .and_then(|state| rotation::activity(&state, model.rotation_at(t).as_ref()))
        };
        let mut seen = [0_u32; 2];
        for mass in [0.08, 0.1, 0.3, 0.7, 1.0, 1.25, 1.3, 1.35, 2.0, 5.0] {
            for age in [3e4, 4.999e5, 5.01e5, 3e6, 3e7, 1e9, 6e9, 1.3e10] {
                let model = star(mass, age);
                for t in times {
                    let Some(state) = model.state_at(t) else {
                        continue;
                    };
                    let active = activity_at(&model, t).is_some();
                    assert_eq!(
                        in_flaring_phase(&state),
                        active,
                        "{mass} M☉ at {age} yr, t = {t}"
                    );
                    seen[usize::from(active)] += 1;
                }
                assert_eq!(
                    FlareModel::of(&model).is_some(),
                    activity_at(&model, UniverseTime::EPOCH).is_some(),
                    "{mass} M☉ at {age} yr"
                );
            }
        }
        assert!(seen[0] > 20 && seen[1] > 20, "{seen:?}");
    }

    /// The parent's test: the bound is never exceeded, across 0.1–1.3 M☉, from the pre-main
    /// sequence to the end of the main sequence, the rotation draw's range and metallicity; and a
    /// bin's mean is at most 64.
    #[test]
    fn the_bound_holds_across_mass_phase_rotation_and_metallicity() {
        let bins = bins();
        let probes: Vec<EventBin> = [
            SourceHorizon::START,
            ClockWindow::START,
            UniverseTime::EPOCH,
            ClockWindow::END,
        ]
        .into_iter()
        .map(|t| bins.bin_of(t).unwrap())
        .collect();
        let mut models = 0;
        for mass in [0.1, 0.2, 0.4, 0.7, 1.0, 1.29] {
            for age in [1e6, 3e7, 1e9, 8e9] {
                for rank in [1e-3, 0.5, 0.999] {
                    for fe_h in [-1.5, 0.0, 0.4] {
                        let draws = StarDraws::from_parts(StarDrawsParts {
                            rotation: UnitUniform::new(rank).unwrap(),
                            ..StarDrawsParts::MEDIAN
                        });
                        let composition =
                            Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                        let model = StarModel::new(
                            SolarMasses::new(mass),
                            composition,
                            draws,
                            Years::new(age),
                        )
                        .unwrap();
                        let Some(flares) = FlareModel::of(&model) else {
                            continue;
                        };
                        models += 1;
                        assert!(flares.floor_joules() >= FIXED_FLOOR_JOULES);
                        for &bin in &probes {
                            let window = bins.bin_window(bin).unwrap();
                            let bound = flares.bound(window);
                            assert!(
                                per_day(bound) <= PoissonBins::MAX_MEAN_PER_BIN,
                                "{mass} M☉ at {age} yr"
                            );
                            let span = window
                                .end()
                                .checked_since(window.start())
                                .unwrap()
                                .as_seconds_f64();
                            for fraction in [0.0, 0.25, 0.5, 0.999_999] {
                                let t = after(window.start(), fraction * span);
                                assert!(flares.rate(t) <= bound, "{mass} M☉ at {age} yr, {t}");
                            }
                        }
                    }
                }
            }
        }
        assert!(models > 100, "{models} flaring models");
    }

    /// The saturated M dwarf's rate is Medina et al.'s (2020) by construction, and its floor the
    /// fixed one, far above the 64-a-day energy.
    #[test]
    fn a_saturated_m_dwarf_flares_at_medina_et_al_s_rate() {
        let dwarf = m_dwarf();
        let model = FlareModel::of(&dwarf).unwrap();
        assert!(
            model.activity().rossby() <= SATURATION_ROSSBY,
            "{:?}",
            model.activity()
        );
        let at_reference = per_day(model.rate_above(REFERENCE_ENERGY_JOULES));
        assert!(
            (at_reference / math::exp(-1.30) - 1.0).abs() < 1e-3,
            "{at_reference} a day"
        );
        assert_same_bits(model.floor_joules(), FIXED_FLOOR_JOULES);
        let listed = per_day(model.listed_rate());
        assert!((7.5..8.5).contains(&listed), "{listed} a day");
        let e64 = REFERENCE_ENERGY_JOULES
            * math::powf(
                PoissonBins::MAX_MEAN_PER_BIN / SECONDS_PER_DAY / model.normalisation.value()
                    + shape(MAX_ENERGY_JOULES),
                -1.0 / FLARE_INDEX,
            );
        assert!((per_day(model.rate_above(e64)) - 64.0).abs() < 1e-9);
        // Nothing from E(max) up, and a little just below it.
        assert_same_bits(model.rate_above(MAX_ENERGY_JOULES).value(), 0.0);
        assert_same_bits(model.rate_above(2.0 * MAX_ENERGY_JOULES).value(), 0.0);
        assert!(model.rate_above(MAX_ENERGY_JOULES * (1.0 - 1e-9)).value() > 0.0);
        eprintln!(
            "saturated 0.2 M☉ dwarf: Ro {:.4}, L {:.5} L☉, {at_reference:.4} a day above E₀, \
             {listed:.3} a day above the floor {:.4e} J; 64 a day above {e64:.4e} J ({:.4e} erg)",
            model.activity().rossby(),
            model.quiescent_luminosity().value(),
            model.floor_joules(),
            e64 / JOULES_PER_ERG,
        );
    }

    /// The Sun-like check against Maehara et al. (2015): the Sun at its period, which they put at
    /// once in 500–600 years above 10³³ erg, and their Fig. 2 bins.
    #[test]
    fn the_sun_flares_about_as_often_as_maehara_et_al_find() {
        let sun = sun();
        let model = FlareModel::of(&sun).unwrap();
        let per_year = |e: f64| model.rate_above(e).value() * SECONDS_PER_JULIAN_YEAR;
        let above_33 = per_year(SOLAR_TYPE_ANCHOR_JOULES);
        let period = sun
            .rotation_at(UniverseTime::EPOCH)
            .unwrap()
            .period()
            .value();
        eprintln!(
            "the median Sun: P {period:.2} d, Ro {:.3}, log L(X) / L(bol) {:.3}; above 10^33 erg \
             once in {:.0} yr, 10^34 once in {:.0}, 10^35 once in {:.0}; {:.3} a year above the \
             floor",
            model.activity().rossby(),
            model.activity().log_lx_lbol(),
            1.0 / above_33,
            1.0 / per_year(1e34 * JOULES_PER_ERG),
            1.0 / per_year(1e35 * JOULES_PER_ERG),
            model.listed_rate().value() * SECONDS_PER_JULIAN_YEAR,
        );
        // Within a factor of five of once in 550 years.
        assert!(
            (1.0 / 2_750.0..5.0 / 550.0).contains(&above_33),
            "{above_33} a year"
        );
        // Each of Fig. 2's bins within a factor of 2.5 of the law at its geometric-mean period.
        let tau = math::exp10(1.16);
        let saturated = saturated_rate(SolarMasses::new(1.0)).value()
            * (shape(SOLAR_TYPE_ANCHOR_JOULES) - shape(MAX_ENERGY_JOULES))
            * SECONDS_PER_JULIAN_YEAR;
        for (a, b, observed) in [
            (2.5_f64, 5.0_f64, 2.21),
            (5.0, 10.0, 0.149),
            (10.0, 20.0, 0.0279),
        ] {
            let ro = (a * b).sqrt() / tau;
            let law = saturated * math::powf(ro / SATURATION_ROSSBY, ACTIVITY_SLOPE);
            assert!(
                (observed / 2.5..observed * 2.5).contains(&law),
                "{a}–{b} d: {law} a year"
            );
        }
    }

    /// Energies lie in [floor, E(max)) and follow the truncated power law; every flare ends within
    /// a day; the inverse transform's ends are the floor and E(max).
    #[test]
    fn every_energy_lies_above_the_floor_and_every_flare_ends_within_a_day() {
        let dwarf = m_dwarf();
        let model = FlareModel::of(&dwarf).unwrap();
        let year = list(&dwarf, &series(0), days(0, 365));
        // A star that flares throughout accepts every candidate: the same bins at a constant rate
        // give the same events.
        let mut bare = Vec::new();
        let constant = ConstantRate::new(model.listed_rate());
        bins().events_in(&series(0), days(0, 365), &constant, &mut bare);
        assert_eq!(
            bare.iter().map(BinEvent::id).collect::<Vec<_>>(),
            year.iter().map(Flare::id).collect::<Vec<_>>()
        );
        assert_poisson_count(
            "flares in a year",
            u64::try_from(year.len()).unwrap(),
            per_day(model.listed_rate()) * 365.0,
            ALPHA,
        );
        for f in &year {
            assert!(
                (model.floor_joules()..MAX_ENERGY_JOULES).contains(&f.energy_joules()),
                "{f:?}"
            );
            assert!(
                f.duration() < MAX_DURATION && f.peak_factor() > 1.0,
                "{f:?}"
            );
        }
        let kept = math::powf(MAX_ENERGY_JOULES / model.floor_joules(), -FLARE_INDEX);
        let mut energies: Vec<f64> = year.iter().map(Flare::energy_joules).collect();
        let ks = ks_one_sample(&mut energies, |e| {
            (1.0 - math::powf(e / model.floor_joules(), -FLARE_INDEX)) / (1.0 - kept)
        });
        assert_p_value(
            "energies against the truncated power law",
            ks.p_value,
            ALPHA,
        );
        assert_same_bits(model.energy_joules_at(0.0), model.floor_joules());
        let top = model.energy_joules_at(1.0 - f64::EPSILON / 2.0);
        assert!(
            top <= MAX_ENERGY_JOULES && top > 0.999 * MAX_ENERGY_JOULES,
            "{top}"
        );
        let longest =
            (RISE_HALF_WIDTHS + DECAY_HALF_WIDTHS) * half_width_seconds(MAX_ENERGY_JOULES);
        assert!(longest < MAX_DURATION.as_seconds_f64(), "{longest} s");
        eprintln!("the longest flare, at E(max): {:.2} h", longest / 3_600.0);
    }

    /// The factor is 1 outside a flare, continuous in time, reaches its peak one t½ after the
    /// onset, and ∫ (factor − 1) dt is E ÷ L(bol).
    #[test]
    fn the_factor_is_one_outside_a_flare_and_integrates_to_its_energy_over_the_luminosity() {
        let dwarf = m_dwarf();
        let model = FlareModel::of(&dwarf).unwrap();
        let luminosity = model.quiescent_luminosity().value() * SOLAR_LUMINOSITY_W;
        let mut flares = list(&dwarf, &series(0), days(0, 30));
        flares.sort_by(|a, b| a.energy_joules().total_cmp(&b.energy_joules()));
        let picks = [
            flares[0],
            flares[flares.len() / 2],
            flares[flares.len() - 1],
        ];
        for f in picks {
            let before =
                UniverseTime::new(f.onset().seconds() - 1, f.onset().subsec_nanos()).unwrap();
            assert_same_bits(f.luminosity_factor_at(before), 1.0);
            assert_same_bits(f.luminosity_factor_at(f.onset()), 1.0);
            assert_same_bits(f.luminosity_factor_at(f.end()), 1.0);
            assert!(!f.is_active_at(f.end()) && f.is_active_at(f.onset()));
            let peak = after(f.onset(), f.half_width_seconds());
            let excess = f.peak_factor() - 1.0;
            assert!((f.luminosity_factor_at(peak) - f.peak_factor()).abs() < 1e-6 * excess);
            // Continuous: a microsecond moves the factor by a sliver of its excess anywhere.
            for x in [-0.999, -0.5, 0.0, 1.6, 19.99] {
                let t = after(f.onset(), (x + 1.0) * f.half_width_seconds());
                let step =
                    (f.luminosity_factor_at(after(t, 1e-6)) - f.luminosity_factor_at(t)).abs();
                assert!(step < 1e-4 * excess, "{step} at x = {x}");
            }
            // The trapezoid rule over the whole flare, 10,000 steps a half width.
            let n = 210_000_u32;
            let h = f.duration().as_seconds_f64() / f64::from(n);
            let mut sum = 0.0;
            for i in 0..=n {
                let weight = if i == 0 || i == n { 0.5 } else { 1.0 };
                sum += weight * (f.luminosity_factor_at(after(f.onset(), h * f64::from(i))) - 1.0);
            }
            let integral = sum * h;
            let expected = f.energy_joules() / luminosity;
            assert!(
                (integral / expected - 1.0).abs() < 1e-5,
                "{integral} s against {expected} s"
            );
        }
    }

    /// `flare(id)` regenerates every listed flare and returns `None` for a rejected candidate, a
    /// number beyond its bin's count, an ID of another series and a star that does not flare.
    #[test]
    fn an_id_regenerates_its_flare_and_names_nothing_else() {
        // A 0.5 M☉ star that leaves its protostar phase half a day into a bin a century before the
        // epoch: the candidates before that are rejected.
        let young = star(0.5, PROTOSTAR_YEARS + 100.0 + 0.5 / 365.25);
        let model = FlareModel::of(&young).unwrap();
        let start = first_flaring(
            &model,
            UniverseTime::from_julian_years(-101).unwrap(),
            UniverseTime::from_julian_years(-99).unwrap(),
        );
        let k = start.seconds().div_euclid(DAY);
        let s = series(1);
        let listed = list(&young, &s, days(k - 3, k + 3));
        assert!(
            listed.len() > 5 && listed.iter().all(|f| f.onset() >= start),
            "{} flares",
            listed.len()
        );
        let every = ConstantRate::new(model.listed_rate());
        let (mut rejected, mut beyond) = (0, 0);
        for bin in k - 3..k + 3 {
            for j in 0..=u8::MAX {
                let id = s.event_id(EventBin::new(bin).unwrap(), j);
                let found = listed.iter().find(|f| f.id() == id);
                assert_eq!(
                    flare(&young, &s, id).as_ref(),
                    found,
                    "bin {bin}, number {j}"
                );
                match (found, bins().event(&s, id, &every)) {
                    (None, Some(_)) => rejected += 1,
                    (None, None) => beyond += 1,
                    (Some(_), _) => {}
                }
            }
        }
        assert!(
            rejected > 5 && beyond > 1_000,
            "{rejected} rejected, {beyond} beyond"
        );
        let word = listed[0].id().word();
        for foreign in [
            event_id(body(9).into(), tags::STAR_FLARE, word.bin(), word.number()),
            event_id(
                body(1).into(),
                event_tags::SELF_TEST,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(flare(&young, &s, foreign), None);
        }
        assert_eq!(flare(&star(1.5, 1e9), &s, listed[0].id()), None);
    }

    /// The window and cut sets of the partition tests, on a saturated M dwarf.
    fn partition_case(
        listing: impl Fn(TimeWindow) -> Vec<Flare>,
    ) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let whole = TimeWindow::new(at(-3 * DAY - 1_234, 5), at(5 * DAY + 77, 0)).unwrap();
        let flares = listing(whole);
        assert!(flares.len() > 30, "{} flares", flares.len());
        let cuts = vec![
            vec![at(0, 0)],
            vec![at(-DAY, 0), at(-DAY, 0), at(DAY, 1), at(4 * DAY + 999, 3)],
            vec![flares[10].onset(), after(flares[11].onset(), 1e-9)],
            (-2..5).map(|d| at(d * DAY + 3_600 * d, 0)).collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test through the public function: a listing over a window is the
    /// union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let dwarf = m_dwarf();
        let s = series(2);
        let listing = |w: TimeWindow| list(&dwarf, &s, w);
        let (whole, cut_sets) = partition_case(listing);
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
        let dwarf = m_dwarf();
        let s = series(2);
        let listing = |w: TimeWindow| list(&dwarf, &s, w);
        let (whole, cut_sets) = partition_case(listing);
        let flares = listing(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<Flare>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), flares);
        }
    }

    /// `flares_active_at` gives exactly the listed flares running at t.
    #[test]
    fn flares_active_at_gives_the_flares_running_then() {
        let dwarf = m_dwarf();
        let s = series(4);
        let listed = list(&dwarf, &s, days(-2, 12));
        let mut seen = 0;
        for i in 0..2_000 {
            let t = at(i64::from(i) * 431 + 3, 17);
            let expected: Vec<Flare> = listed
                .iter()
                .copied()
                .filter(|f| f.is_active_at(t))
                .collect();
            let got = running(&dwarf, &s, t);
            assert_eq!(got, expected, "at {t}");
            seen += got.len();
        }
        assert!(seen > 10, "{seen} running flares seen");
    }

    /// The parent's no-history test for this kind: the flares running at +900 years, asked cold,
    /// are the same after asking about 100 earlier times.
    #[test]
    fn the_flares_running_at_900_years_need_no_history() {
        let s = series(3);
        // Find an instant with a flare running on a model of its own, so that the cold call below
        // is the first on its star.
        let base = UniverseTime::from_julian_years(900).unwrap();
        let probe = m_dwarf();
        let t = (0..5_000)
            .map(|i| after(base, f64::from(i) * 157.0))
            .find(|&t| !running(&probe, &s, t).is_empty())
            .unwrap();
        let dwarf = m_dwarf();
        let cold = running(&dwarf, &s, t);
        let mut lcg = Lcg::new(0x0928_0900);
        for _ in 0..100 {
            let earlier = after(
                ClockWindow::START,
                lcg.next_f64() * 1.899e3 * SECONDS_PER_JULIAN_YEAR,
            );
            assert!(earlier < t);
            let _ = running(&dwarf, &s, earlier);
            let _ = list(
                &dwarf,
                &s,
                TimeWindow::new(earlier, after(earlier, 2.0 * SECONDS_PER_DAY)).unwrap(),
            );
        }
        assert_eq!(running(&dwarf, &s, t), cold);
        assert_ne!(cold, []);
    }

    /// Pinned: the template's profile and area; three stars at the median draws, their floors and
    /// rates, and each flare's ID, onset, energy, duration, half width, peak factor and luminosity
    /// factor at four instants, over three days (and the G dwarf's over a year, since it seldom
    /// flares in three); and a star whose flaring phase begins inside a bin, whose rejected
    /// candidates leave gaps in its numbers.
    #[test]
    fn flares_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.line("");
        w.line("# the template");
        for x in [-0.5, 0.5, 1.6, 10.0] {
            w.f64(&format!("profile at {x} half widths"), profile(x));
        }
        w.f64("profile area (half widths)", profile_area());
        let young = star(0.5, PROTOSTAR_YEARS + 100.0 + 0.5 / 365.25);
        let start = first_flaring(
            &FlareModel::of(&young).unwrap(),
            UniverseTime::from_julian_years(-101).unwrap(),
            UniverseTime::from_julian_years(-99).unwrap(),
        );
        let k = start.seconds().div_euclid(DAY);
        let boundary = format!(
            "a 0.5 M☉ star that leaves its protostar phase at {start}, over the bin before, its \
             bin and the bin after"
        );
        let cases = [
            (
                "a saturated M dwarf, 0.2 M☉ at 300 Myr, over three days",
                m_dwarf(),
                0,
                days(0, 3),
            ),
            (
                "a Sun-like G dwarf, 1 M☉ at 4.57 Gyr, over three days",
                sun(),
                1,
                days(0, 3),
            ),
            ("the same G dwarf over a year", sun(), 1, days(0, 365)),
            (
                "a pre-main-sequence K star, 0.8 M☉ at 10 Myr, over three days",
                young_k(),
                2,
                days(0, 3),
            ),
            (boundary.as_str(), young, 5, days(k - 1, k + 2)),
        ];
        for (name, model, index, window) in cases {
            let flares = FlareModel::of(&model).unwrap();
            let phase = model.state_at(UniverseTime::EPOCH).unwrap().phase();
            w.line("");
            w.line(&format!("# {name}"));
            w.line(&format!("phase at the epoch: {phase:?}"));
            w.f64("Rossby number at the epoch", flares.activity().rossby());
            w.f64(
                "log L(X) / L(bol) at the epoch",
                flares.activity().log_lx_lbol(),
            );
            w.f64("floor (J)", flares.floor_joules());
            w.f64("listed rate (per s)", flares.listed_rate().value());
            w.f64(
                "rate above E0 (per s)",
                flares.rate_above(REFERENCE_ENERGY_JOULES).value(),
            );
            for f in list(&model, &series(index), window) {
                w.line(&format!("{} at {}", f.id(), f.onset()));
                w.f64("  energy (J)", f.energy_joules());
                w.line(&format!("  duration {}", f.duration()));
                w.f64("  half width (s)", f.half_width_seconds());
                w.f64("  peak factor", f.peak_factor());
                for half_widths in [0.5, 1.0, 3.0, 15.0] {
                    let t = after(f.onset(), half_widths * f.half_width_seconds());
                    w.f64(
                        &format!("  factor {half_widths} half widths after the onset"),
                        f.luminosity_factor_at(t),
                    );
                }
            }
        }
        golden!("stellar/flares", w.as_str());
    }

    /// The parent's statistical test for this kind: over 10⁴ stars of T31's sampler, across its
    /// populations at Milky Way parameters, the flares listed in 30 days from the epoch number the
    /// rate integral, the sum of each star's constant rate times the window, within a Poisson
    /// interval.
    ///
    /// A star with a model flares at the epoch, the window's start, and the test asserts that none
    /// leaves its flaring phase within the window. It also counts the exposure of the epoch rule:
    /// stars with no activity at the epoch that are in a flaring phase at an end of the source
    /// horizon, and stars with one that are not.
    #[test]
    #[ignore = "slow: 10⁴ sampled stars, each with its track (2–3 s under slow-test)"]
    fn the_mean_count_over_10_000_stars_matches_the_rate_integral() {
        const STARS: u16 = 10_000;
        let seed = 0x0628_a000_0000_5eed_u64;
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like()).unwrap();
        let window = days(0, 30);
        let last = UniverseTime::new(30 * DAY - 1, 999_999_999).unwrap();
        let ends = [SourceHorizon::START, ClockWindow::END];
        let (mut expected, mut counted) = (0.0, 0_u64);
        let (mut inactive, mut dormant, mut changing) = (0_u32, 0_u32, 0_u32);
        for i in 0..STARS {
            let population = POPULATIONS[usize::from(i) % POPULATIONS.len()];
            let sampled = sample_population_star(&galaxy, population, seed, u64::from(i));
            let Some(model) = FlareModel::of(&sampled.model) else {
                inactive += 1;
                let flaring_at = |t: UniverseTime| {
                    sampled
                        .model
                        .state_at(t)
                        .is_some_and(|state| in_flaring_phase(&state))
                };
                if ends.into_iter().any(flaring_at) {
                    dormant += 1;
                }
                continue;
            };
            assert!(
                model.flares_at(window.start()) && model.flares_at(last),
                "star {i}"
            );
            if !ends.into_iter().all(|t| model.flares_at(t)) {
                changing += 1;
            }
            expected += model.listed_rate().value() * 30.0 * SECONDS_PER_DAY;
            counted += u64::try_from(list(&sampled.model, &series(i), window).len()).unwrap();
        }
        eprintln!(
            "{STARS} stars over 30 days: expected {expected:.1}, counted {counted}, interval \
             {:?} at alpha {ALPHA:e}; {inactive} without activity at the epoch ({:.1}%), of which \
             {dormant} are in a flaring phase at an end of the source horizon; {changing} with one \
             are not at an end",
            poisson_interval(expected, ALPHA),
            100.0 * f64::from(inactive) / f64::from(STARS),
        );
        assert_poisson_count("flares of 10⁴ stars in 30 days", counted, expected, ALPHA);
    }
}
