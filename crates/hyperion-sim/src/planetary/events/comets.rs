//! Long-period comets: `system.comet` (plan 14, P14.T31.a; design note 15), each event a comet's
//! passage through perihelion on its own near-parabolic orbit about the halo's host.
//!
//! # The rate
//!
//! Comets reach perihelion inside 5 au × √(L ÷ L☉) of the host at the cometary halo's rate
//! (P14.T21.d,
//! [`CometaryHalo::comet_rate`](crate::planetary::halo::CometaryHalo::comet_rate)): every
//! long-period comet over 1 km, new and returning, three times those on their first passage
//! (ruling 84.5). The Solar System's 7.5 × 10¹¹ comets give 10.9 a year. Each comet is one event,
//! identified by its [`EventId`] and not by a body index (design note 15): a comet's next return,
//! centuries or millennia later, is another event.
//!
//! # A comet
//!
//! From each comet's own stream, [`BinEvent::marks`], at word 0:
//!
//! - **its perihelion distance q** (word 0), uniform in (0, 5 au × √(L ÷ L☉)]
//!   ([`halo::INNER_SYSTEM_AU`](crate::planetary::halo::INNER_SYSTEM_AU)), as Francis (2005, ApJ
//!   635, 1348) finds new comets' perihelia;
//! - **its eccentricity** (word 1): 1 − e uniform in [0, min(10⁻², q ÷ `a_min`)), with
//!   `a_min` = max(30 au × √(L ÷ L☉), the semi-major axis of a 200-year period)
//!   ([`CometModel::min_semi_major_axis`]). The plan's 1 − 10⁻⁴ to 1 is widened by ruling 84.5 to
//!   10⁻² ([`ECCENTRICITY_SPAN`]) so that the returning comets are counted, and held to
//!   q ÷ `a_min` by decision-p14-t31a-events (amendment (c)), so that no comet has a period under
//!   200 years, the long-period comets' conventional bound ([`MIN_PERIOD_YEARS`]), or an aphelion
//!   inside the listing radius: 34.2 au about the Sun, which moves only the 6.8% of comets with q
//!   under 0.342 au;
//! - **its orientation** (words 2–4), isotropic: the cosine of its inclination to the system frame
//!   uniform in (−1, 1], its ascending node and its argument of perihelion uniform in [0, 2π);
//! - **its nucleus** (word 5): its diameter from [`nuclei`], N(> D) ∝ D^(−1.92) normalised at
//!   1 km, from the floor up to [`LARGEST_NUCLEUS`];
//! - words 6 and 7 are reserved.
//!
//! The time of perihelion is the event's time. Its orbit is plan 14's T2.b's
//! ([`orbit::Orbit`](crate::orbit::Orbit)), about the host's gravitational parameter, so its
//! position at any time is a closed form: an [`OpenOrbit`] from
//! [`OpenOrbit::MIN_ECCENTRICITY`] up, and below it, where all but 1–2% of them lie, the same
//! orbit as a [`KeplerElements`], which `elements_from_state` would give.
//!
//! # Listing
//!
//! A comet is listed while it is inside 30 au × √(L ÷ L☉) ([`LISTING_RADIUS_AU`]), from the time it
//! comes in to the time it goes out ([`Comet::listed`]); its aphelion always lies beyond. A comet
//! whose perihelion lies inside its host's radius plunges into it, and is listed until it reaches
//! the host's surface. The times in and out are Kepler's, in a form that holds through e = 1.
//!
//! [`comets`] lists comets by their time of perihelion, which partitions; [`comets_listed_in`]
//! and [`comets_listed_at`] list those inside the radius during a window or at an instant, from
//! the bins [`CometModel::reach`] either side, the longest any comet stays listed.
//!
//! # The floor
//!
//! A comet is an event above its system's floor, [`CometModel::floor_diameter`]: the larger of
//! 1 km, the size the halo counts, and the diameter at which either the rate reaches
//! [`PoissonBins::MAX_MEAN_PER_BIN`] a bin (plan 06's flares' rule) or the comets listed at once,
//! the rate times twice the reach, reach [`MAX_LISTED_COMETS`] (decision-p14-t31a-events,
//! amendment (d)): a luminous host's comets stay listed for centuries, as L^(3⁄4), and would
//! otherwise number millions at once. The Sun's floor is 1 km, with about 300 listed at once.
//!
//! # Time
//!
//! Poisson bins of [`BIN_SECONDS`] = one Julian year on the system's own key, looking back none.
//! The rate is constant: its inputs are read once, at the epoch.

use core::f64::consts::{PI, TAU};
use std::error::Error;
use std::fmt;

use super::{SizeLaw, expected_count};
use crate::Seed;
use crate::coords::{SystemVector, SystemVelocity};
use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
    from_nanos_saturating, to_nanos,
};
use crate::id::SystemId;
use crate::id::event_tags::SYSTEM_COMET;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, OpenOrbit, Orbit, Orientation};
use crate::planetary::halo::INNER_SYSTEM_AU;
use crate::rng::PowerLaw;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
use crate::units::{GravitationalParameter, Metres, Radians, SolarLuminosities};

/// The length of a comet bin, s: one Julian year, 31,557,600 s.
pub const BIN_SECONDS: i64 = 31_557_600;

/// How many whole bins a comet's listing looks back by construction: none. A comet's listed span
/// reaches both before and after its perihelion, so the listings scan [`CometModel::reach`]
/// either side instead.
pub const LOOK_BACK_BINS: u32 = 0;

/// The radius inside which a comet is listed, au, at a host of the Sun's luminosity: 30, scaled
/// by √(L ÷ L☉) as the perihelia are (plan 14, P14.T31.a). Part of the generator version.
pub const LISTING_RADIUS_AU: f64 = 30.0;

/// The span of the eccentricities, 1 − e: 10⁻², ruling 84.5's widening of the plan's 10⁻⁴ so that
/// the returning long-period comets, which the halo's rate counts, are drawn too. Part of the
/// generator version.
pub const ECCENTRICITY_SPAN: f64 = 1e-2;

/// The lowest eccentricity a comet takes, 1 − [`ECCENTRICITY_SPAN`]: 0.99, excluded; one of small
/// perihelion takes a higher floor, 1 − q ÷ `a_min`.
pub const MIN_ECCENTRICITY: f64 = 1.0 - ECCENTRICITY_SPAN;

/// The shortest period a comet takes, Julian years: 200, the conventional bound between the
/// short-period and the long-period comets (decision-p14-t31a-events, amendment (c)). Part of the
/// generator version.
pub const MIN_PERIOD_YEARS: f64 = 200.0;

/// The most comets a system lists at once: 1,024 (decision-p14-t31a-events, amendment (d)). Part of
/// the generator version.
pub const MAX_LISTED_COMETS: f64 = 1_024.0;

/// The diameter the nuclei's size law is normalised at: 1 km, the size P14.T21.d's halo counts.
pub const NUCLEUS_REFERENCE_DIAMETER: Metres = Metres::new(1e3);

/// The slope q of the nuclei's size law, N(> D) ∝ D^(−q): 1.92, Snodgrass et al.'s (2011, MNRAS
/// 414, 458) cumulative slope of the Jupiter-family comets' nuclei of radius 1.25 km and above,
/// 1.92 ± 0.20. The long-period comets' is not agreed: Fernández and Sosa (2012, MNRAS 423, 1674)
/// find about 2.15 above a radius of 2.4 km, and Boe et al. (2019, Icarus 333, 252) about 3.6
/// above a diameter of 3.7 km, flattening below about 1 km. Part of the generator version.
pub const NUCLEUS_SIZE_SLOPE: f64 = 1.92;

/// The largest nucleus, m: 150 km, a little above the largest known, C/2014 UN271's 137 ± 17 km
/// (Lellouch et al. 2022, A&A 659, L1; Hale–Bopp's is about 74 km, Szabó et al. 2012,
/// arXiv:1210.2785). Part of the generator version.
pub const LARGEST_NUCLEUS: Metres = Metres::new(150e3);

/// [`BIN_SECONDS`] as a float, exactly.
const BIN_SECONDS_F64: f64 = 31_557_600.0;

/// The nuclei's size law: [`NUCLEUS_SIZE_SLOPE`] normalised at [`NUCLEUS_REFERENCE_DIAMETER`], up
/// to [`LARGEST_NUCLEUS`].
///
/// # Panics
///
/// Never: the constants are in range.
#[must_use]
pub fn nuclei() -> SizeLaw {
    SizeLaw::new(
        NUCLEUS_SIZE_SLOPE,
        NUCLEUS_REFERENCE_DIAMETER,
        LARGEST_NUCLEUS,
    )
    .expect("the nuclei's constants are in range")
}

/// The comet bins: a Julian year long, looking back none.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("a year lies between 16 s and 2^44 s")
}

/// What a [`CometModel`] reads of a system: its halo's rate, and its host's luminosity, mass and
/// radius.
///
/// Plain data with public fields: [`CometModel::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CometInputs {
    /// The rate of long-period comets over 1 km reaching perihelion inside 5 au × √(L ÷ L☉),
    /// events per second, finite and non-negative: the halo's
    /// [`comet_rate`](crate::planetary::halo::CometaryHalo::comet_rate) at `luminosity`.
    pub rate: EventsPerSecond,
    /// The host's luminosity, L☉, finite and positive: the perihelia's and the listing's scale.
    pub luminosity: SolarLuminosities,
    /// The host's gravitational parameter, m³ s⁻², finite and positive: a star's, or a pair's
    /// sum about its barycentre.
    pub host: GravitationalParameter,
    /// The host's radius, m, finite and non-negative: a comet whose perihelion lies inside it
    /// plunges into it.
    pub host_radius: Metres,
}

/// A [`CometModel`] could not be built, with the value refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCometModelError {
    /// The rate is not finite and non-negative.
    Rate(EventsPerSecond),
    /// The luminosity is not finite and positive.
    Luminosity(SolarLuminosities),
    /// The host's gravitational parameter is not finite and positive.
    HostParameter(GravitationalParameter),
    /// The host's radius is not finite and non-negative.
    HostRadius(Metres),
    /// The longest a comet stays listed is not a span the clock can hold, with room for the
    /// windows it widens: a host of vanishing mass or enormous luminosity.
    Reach {
        /// The span, s.
        seconds: f64,
    },
}

impl fmt::Display for BuildCometModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rate(rate) => write!(
                f,
                "comet rate {} per second is not finite and non-negative",
                rate.value()
            ),
            Self::Luminosity(l) => write!(
                f,
                "luminosity {} solar luminosities is not finite and positive",
                l.value()
            ),
            Self::HostParameter(mu) => write!(
                f,
                "host parameter {} m³ s⁻² is not finite and positive",
                mu.value()
            ),
            Self::HostRadius(r) => write!(
                f,
                "host radius {} m is not finite and non-negative",
                r.value()
            ),
            Self::Reach { seconds } => {
                write!(f, "a comet would stay listed {seconds} s, beyond the clock")
            }
        }
    }
}

impl Error for BuildCometModelError {}

/// The comets of one system: its series, its inputs, the scales of its perihelia and its listing,
/// its floor and its constant rate above the floor (see the [module documentation](self)).
///
/// It is the [`RateModel`] of the system's `system.comet` series, whose bound and rate are both
/// [`listed_rate`](Self::listed_rate) at every instant.
///
/// # Examples
///
/// The Solar System's comets, about eleven a year, and those inside 30 au at the epoch:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventsPerSecond, TimeWindow};
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::events::comets::{comets_listed_at, CometInputs, CometModel};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{GravitationalParameter, Metres, SolarLuminosities, SolarMasses};
///
/// let sun = CometInputs {
///     rate: EventsPerSecond::per_julian_year(10.9),
///     luminosity: SolarLuminosities::new(1.0),
///     host: GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     host_radius: Metres::new(6.957e8),
/// };
/// let model = CometModel::new(Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?, &sun)?;
/// let mut listed = Vec::new();
/// comets_listed_at(&model, UniverseTime::EPOCH, &mut listed);
/// // Each stays inside 30 au for two or three decades: some 300 at once.
/// assert!((200..400).contains(&listed.len()));
/// for comet in &listed {
///     let r = comet.position_at(UniverseTime::EPOCH).length();
///     assert!(r.value() <= 30.0 * 1.495_978_707e11 * 1.0001);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CometModel {
    series: EventSeries,
    inputs: CometInputs,
    perihelion_limit: Metres,
    listing_radius: Metres,
    min_semi_major_axis: Metres,
    reach: Span,
    floor: Metres,
    draws: Option<PowerLaw>,
    rate: EventsPerSecond,
}

impl CometModel {
    /// The comets of `system` in the universe of `seed`, from `inputs`: the series of
    /// [`SYSTEM_COMET`] with the system's ID as subject.
    ///
    /// # Errors
    ///
    /// The [`BuildCometModelError`] of the first input out of range, in the field order, then
    /// [`BuildCometModelError::Reach`] for a listing the clock cannot hold.
    pub fn new(
        seed: Seed,
        system: SystemId,
        inputs: &CometInputs,
    ) -> Result<Self, BuildCometModelError> {
        let rate = inputs.rate.value();
        if !(rate.is_finite() && rate >= 0.0) {
            return Err(BuildCometModelError::Rate(inputs.rate));
        }
        let luminosity = inputs.luminosity.value();
        if !(luminosity.is_finite() && luminosity > 0.0) {
            return Err(BuildCometModelError::Luminosity(inputs.luminosity));
        }
        let mu = inputs.host.value();
        if !(mu.is_finite() && mu > 0.0) {
            return Err(BuildCometModelError::HostParameter(inputs.host));
        }
        let host_radius = inputs.host_radius.value();
        if !(host_radius.is_finite() && host_radius >= 0.0) {
            return Err(BuildCometModelError::HostRadius(inputs.host_radius));
        }
        let scale = luminosity.sqrt() * METRES_PER_AU;
        let (perihelion_limit, listing_radius) =
            (INNER_SYSTEM_AU * scale, LISTING_RADIUS_AU * scale);
        let period = MIN_PERIOD_YEARS * SECONDS_PER_JULIAN_YEAR;
        let shortest = math::cbrt(mu * period * period / (4.0 * PI * PI));
        let min_semi_major_axis = if shortest > listing_radius {
            shortest
        } else {
            listing_radius
        };
        let reach_seconds =
            reach_seconds(perihelion_limit, min_semi_major_axis, listing_radius, mu);
        // Held to a quarter of the clock's range, so that a listing's window widened by the reach
        // either side stays within it before the clamp; any host with a halo lies far inside.
        let reach = Span::from_seconds_f64(reach_seconds)
            .filter(|span| span.seconds() < i64::MAX / 4)
            .ok_or(BuildCometModelError::Reach {
                seconds: reach_seconds,
            })?;
        let law = nuclei();
        // The highest rate listed: a bin's mean at the bins' limit, and the comets listed at once,
        // the rate times twice the reach, at theirs. With no comets at all the share is infinite
        // and the diameter zero, under the 1 km floor.
        let limit = (PoissonBins::MAX_MEAN_PER_BIN / BIN_SECONDS_F64)
            .min(MAX_LISTED_COMETS / (2.0 * reach_seconds));
        let capped = law.diameter_at_share(limit / rate);
        let floor = if capped > NUCLEUS_REFERENCE_DIAMETER {
            capped
        } else {
            NUCLEUS_REFERENCE_DIAMETER
        };
        let (floor, draws, listed) = match law.draws_above(floor) {
            // Clamped at the limit, which the floor already meets to rounding.
            Ok(draws) => (
                floor,
                Some(draws),
                (rate * law.share_above(floor)).min(limit),
            ),
            Err(_) => (law.largest(), None, 0.0),
        };
        Ok(Self {
            series: EventSeries::new(seed, SYSTEM_COMET, system.into()),
            inputs: *inputs,
            perihelion_limit: Metres::new(perihelion_limit),
            listing_radius: Metres::new(listing_radius),
            min_semi_major_axis: Metres::new(min_semi_major_axis),
            reach,
            floor,
            draws,
            rate: EventsPerSecond::new(listed),
        })
    }

    /// The series: the system's subject, [`SYSTEM_COMET`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The inputs it was built from.
    #[must_use]
    pub const fn inputs(&self) -> &CometInputs {
        &self.inputs
    }

    /// The largest perihelion distance, m: 5 au × √(L ÷ L☉).
    #[must_use]
    pub const fn perihelion_limit(&self) -> Metres {
        self.perihelion_limit
    }

    /// The radius inside which a comet is listed, m: 30 au × √(L ÷ L☉).
    #[must_use]
    pub const fn listing_radius(&self) -> Metres {
        self.listing_radius
    }

    /// `a_min`, the smallest semi-major axis a comet takes, m: the larger of the listing radius and
    /// the semi-major axis of a [`MIN_PERIOD_YEARS`] period about the host, 34.2 au about the Sun.
    #[must_use]
    pub const fn min_semi_major_axis(&self) -> Metres {
        self.min_semi_major_axis
    }

    /// The longest any comet stays listed either side of its perihelion, from the slowest orbits
    /// the draws allow, plus a second for rounding: about 15.2 years about the Sun.
    #[must_use]
    pub const fn reach(&self) -> Span {
        self.reach
    }

    /// The smallest nucleus listed, m: 1 km, or above it where the rate would overflow a bin or
    /// list more than [`MAX_LISTED_COMETS`] at once, or [`LARGEST_NUCLEUS`] for a system with no
    /// comet above the floor.
    #[must_use]
    pub const fn floor_diameter(&self) -> Metres {
        self.floor
    }

    /// The rate of the comets listed, those above the floor, events per second: constant.
    #[must_use]
    pub const fn listed_rate(&self) -> EventsPerSecond {
        self.rate
    }

    /// The rate of comets whose nucleus is over `diameter`, events per second, whatever the floor:
    /// the input rate times the nuclei's size law normalised at 1 km.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the diameter is positive ([`SizeLaw::share_above`]).
    #[must_use]
    pub fn rate_above(&self, diameter: Metres) -> EventsPerSecond {
        EventsPerSecond::new(self.inputs.rate.value() * nuclei().share_above(diameter))
    }

    /// The number of comets expected to reach perihelion in `window` above the floor: the rate
    /// times the window's span, not a draw.
    #[must_use]
    pub fn expected_count(&self, window: TimeWindow) -> f64 {
        expected_count(self.rate, window)
    }
}

impl RateModel for CometModel {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.rate
    }
}

/// The eccentricity of a comet of perihelion `q` whose eccentricity's uniform is `u`, in [0, 1):
/// 1 − e = u × min([`ECCENTRICITY_SPAN`], q ÷ `a_min`), so that its semi-major axis is at least
/// `min_semi_major_axis`.
#[must_use]
fn eccentricity_of(q: f64, min_semi_major_axis: f64, u: f64) -> f64 {
    1.0 - u * ECCENTRICITY_SPAN.min(q / min_semi_major_axis)
}

/// The longest a comet stays inside `listing_radius` either side of its perihelion, s, plus a
/// second: the time out to the radius of the slowest orbits the draws allow, the largest 1 − e at
/// each perihelion, at the larger of two perihelia, the largest (`perihelion_limit`) and the one
/// at which q ÷ `a_min` reaches [`ECCENTRICITY_SPAN`]. Between and below them the time is no
/// longer (checked on a fine grid over hosts of 10⁻⁴–10⁵ L☉ and 0.08–30 M☉ for P14.T31.a, and by
/// `no_comet_outstays_the_reach`).
#[must_use]
fn reach_seconds(
    perihelion_limit: f64,
    min_semi_major_axis: f64,
    listing_radius: f64,
    mu: f64,
) -> f64 {
    let bend = (ECCENTRICITY_SPAN * min_semi_major_axis).min(perihelion_limit);
    let slowest = |q: f64| {
        let e = eccentricity_of(q, min_semi_major_axis, 1.0);
        time_from_perihelion(q, e, mu, listing_radius)
    };
    slowest(bend).max(slowest(perihelion_limit)) + 1.0
}

/// The time a comet of perihelion distance `q` and eccentricity `e` about a host of gravitational
/// parameter `mu` takes from perihelion to the distance `r`, s: 0 inside the perihelion, half the
/// period if the aphelion lies inside `r`, Barker's equation for the parabola at e = 1, and
/// otherwise Kepler's equation, M = E − e sin E, in a form that holds to e = 1.
///
/// Near e = 1 the textbook cos E = (1 − r ÷ a) ÷ e cancels, so the eccentric anomaly comes from
/// sin²(E ÷ 2) = (1 − e)(r ÷ q − 1) ÷ 2e, which holds 1 − e exactly, and E − sin E from its series
/// below [`SMALL_ANOMALY`]. The time is then continuous into Barker's at e = 1, and agrees with
/// the orbit's own position on both sides of
/// [`NEAR_PARABOLIC_BAND`](crate::orbit::NEAR_PARABOLIC_BAND), where `OpenOrbit` changes
/// form; Barker's parabola applied across the band would be off by about 3(1 − e) r ÷ 20q for
/// r ≫ q.
#[must_use]
fn time_from_perihelion(q: f64, e: f64, mu: f64, r: f64) -> f64 {
    if r <= q {
        return 0.0;
    }
    // Exact for e in [0.5, 1] (Sterbenz).
    let below_one = 1.0 - e;
    if below_one <= 0.0 {
        // Barker's equation: t = √(2q³ ÷ μ) (D + D³ ÷ 3), D = tan(ν ÷ 2) = √(r ÷ q − 1).
        let d = (r / q - 1.0).sqrt();
        return (2.0 * q * q * q / mu).sqrt() * (d + d * d * d / 3.0);
    }
    let a = q / below_one;
    let inverse_mean_motion = a * (a / mu).sqrt();
    if r >= a * (1.0 + e) {
        return PI * inverse_mean_motion;
    }
    let half_versine = below_one * (r / q - 1.0) / (2.0 * e);
    let anomaly = 2.0 * math::asin(half_versine.sqrt().min(1.0));
    let sine = math::sin(anomaly);
    let excess = if anomaly < SMALL_ANOMALY {
        // E − sin E = E³/3! − E⁵/5! + E⁷/7! − E⁹/9!, the next term about 10⁻¹⁵ of the first.
        let e2 = anomaly * anomaly;
        anomaly * e2 / 6.0 * (1.0 - e2 / 20.0 * (1.0 - e2 / 42.0 * (1.0 - e2 / 72.0)))
    } else {
        anomaly - sine
    };
    (excess + below_one * sine) * inverse_mean_motion
}

/// Below this eccentric anomaly, rad, E − sin E is summed by its series rather than subtracted:
/// 0.1, where the subtraction would lose some 3 of 16 digits and the series' fifth term is about
/// 10⁻¹⁵ of its first.
const SMALL_ANOMALY: f64 = 0.1;

/// A span of `seconds`, a duration up to [`CometModel::reach`], in nanoseconds.
#[must_use]
fn nanos_of(seconds: f64) -> i128 {
    let span = Span::from_seconds_f64(seconds).expect("a comet's times lie within its reach");
    i128::from(span.seconds()) * 1_000_000_000 + i128::from(span.subsec_nanos())
}

/// One long-period comet's passage through perihelion: its ID, its orbit, its nucleus and when it
/// is listed (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Comet {
    id: EventId,
    perihelion_time: UniverseTime,
    perihelion: Metres,
    eccentricity: f64,
    orientation: Orientation,
    nucleus_diameter: Metres,
    orbit: Orbit,
    listed: TimeWindow,
    plunges: bool,
}

impl Comet {
    /// The comet's ID, from which [`comet`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// When it passes perihelion: its event's time.
    #[must_use]
    pub const fn perihelion_time(&self) -> UniverseTime {
        self.perihelion_time
    }

    /// Its perihelion distance from the host, m, in (0, 5 au × √(L ÷ L☉)].
    #[must_use]
    pub const fn perihelion(&self) -> Metres {
        self.perihelion
    }

    /// Its eccentricity, in (0.99, 1]: 1 − e below min(10⁻², q ÷ `a_min`).
    #[must_use]
    pub const fn eccentricity(&self) -> f64 {
        self.eccentricity
    }

    /// Its orbit's orientation in the system frame, isotropic.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    /// Its nucleus's diameter, m, from the model's floor up to [`LARGEST_NUCLEUS`].
    #[must_use]
    pub const fn nucleus_diameter(&self) -> Metres {
        self.nucleus_diameter
    }

    /// Its orbit about the host: open or nearly parabolic from [`OpenOrbit::MIN_ECCENTRICITY`],
    /// bound below it.
    #[must_use]
    pub const fn orbit(&self) -> &Orbit {
        &self.orbit
    }

    /// When it is listed: from when it comes inside the listing radius to when it leaves it, or
    /// reaches its host's surface if it plunges.
    #[must_use]
    pub const fn listed(&self) -> TimeWindow {
        self.listed
    }

    /// Whether it is listed at `t`.
    #[must_use]
    pub fn is_listed_at(&self, t: UniverseTime) -> bool {
        self.listed.contains(t)
    }

    /// Whether its perihelion lies inside its host, which it then strikes before perihelion.
    #[must_use]
    pub const fn plunges(&self) -> bool {
        self.plunges
    }

    /// Its position and velocity relative to its host at `t`, in the system frame's axes, m and
    /// m s⁻¹: its orbit's, a pure function of the time. A plunging comet's means nothing after it
    /// has reached its host.
    #[must_use]
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity) {
        self.orbit.relative_state_at(t)
    }

    /// Its position relative to its host at `t`, m
    /// ([`relative_state_at`](Self::relative_state_at)).
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> SystemVector {
        self.relative_state_at(t).0
    }
}

/// The orbit of perihelion distance `q`, eccentricity `e` and `orientation` about `mu`, at
/// perihelion at `perihelion_time`: an [`OpenOrbit`] from its lowest eccentricity, else the
/// [`KeplerElements`] of semi-major axis q ÷ (1 − e) whose mean anomaly at the epoch puts it at
/// perihelion then.
#[must_use]
fn orbit_of(
    q: f64,
    e: f64,
    orientation: Orientation,
    perihelion_time: UniverseTime,
    mu: GravitationalParameter,
) -> Orbit {
    if e >= OpenOrbit::MIN_ECCENTRICITY {
        return Orbit::Open(
            OpenOrbit::new(Metres::new(q), e, orientation, perihelion_time, mu)
                .expect("a comet's perihelion, eccentricity and host are in range"),
        );
    }
    // At perihelion, mean anomaly 0, at `perihelion_time`: the phase is reduced exactly from the
    // clock's integer seconds by the orbit's own period (`KeplerElements::rephased`).
    Orbit::Bound(
        KeplerElements::from_semi_major_axis(
            Metres::new(q / (1.0 - e)),
            mu,
            Eccentricity::new(e).expect("a bound comet's eccentricity lies in [0.99, 0.9999)"),
            orientation,
            Radians::ZERO,
        )
        .and_then(|elements| elements.rephased(perihelion_time, 0.0))
        .expect("a comet's semi-major axis, host and anomaly are in range"),
    )
}

/// The comet of `event`, its orbit and nucleus drawn from its own stream.
#[must_use]
fn comet_of(model: &CometModel, event: BinEvent) -> Comet {
    let draws = model
        .draws
        .as_ref()
        .expect("a model lists comets only when it has a size law above its floor");
    let (id, time) = (event.id(), event.time());
    let mut marks = event.into_marks();
    let q = model.perihelion_limit.value() * marks.uniform_open_low();
    let e = eccentricity_of(q, model.min_semi_major_axis.value(), marks.uniform());
    let cos_inclination = 1.0 - 2.0 * marks.uniform();
    let node = TAU * marks.uniform();
    let periapsis = TAU * marks.uniform();
    let nucleus = Metres::new(marks.power_law(draws));
    let orientation = Orientation::new(
        Radians::new(math::acos(cos_inclination)),
        Radians::new(node),
        Radians::new(periapsis),
    )
    .expect("an inclination in [0, π] and finite angles");
    let mu = model.inputs.host;
    let at = to_nanos(time);
    let radius = model.listing_radius.value();
    let host_radius = model.inputs.host_radius.value();
    let inward = nanos_of(time_from_perihelion(q, e, mu.value(), radius));
    let plunges = q <= host_radius;
    // A plunging comet's span ends where it reaches the host's surface, before its perihelion; a
    // host larger than the listing radius would swallow it before it is ever listed.
    let outward = if plunges {
        -nanos_of(time_from_perihelion(q, e, mu.value(), host_radius)).min(inward)
    } else {
        inward
    };
    let listed = TimeWindow::new(
        from_nanos_saturating(at - inward),
        from_nanos_saturating(at + outward),
    )
    .expect("a comet comes in before it goes out");
    Comet {
        id,
        perihelion_time: time,
        perihelion: Metres::new(q),
        eccentricity: e,
        orientation,
        nucleus_diameter: nucleus,
        orbit: orbit_of(q, e, orientation, time, mu),
        listed,
        plunges,
    }
}

/// Appends to `out` the system's comets whose perihelion falls in `window`, sorted by time of
/// perihelion and then by ID.
///
/// Every bin the window touches is generated whole, so a listing over a window is the union of the
/// listings over any partition of it, in any order of asking. `out` is only appended to.
pub fn comets(model: &CometModel, window: TimeWindow, out: &mut Vec<Comet>) {
    let mut events = Vec::new();
    bins().events_in(model.series(), window, model, &mut events);
    out.extend(events.into_iter().map(|event| comet_of(model, event)));
}

/// Appends to `out` the system's comets listed at some instant of `window`, those inside the
/// listing radius then, sorted by time of perihelion and then by ID; none for an empty window.
///
/// It reads the bins [`CometModel::reach`] either side of the window, so the result depends on the
/// model and the window alone.
///
/// # Panics
///
/// Never: a window widened at both ends, each end clamped to the clock, stays ordered.
pub fn comets_listed_in(model: &CometModel, window: TimeWindow, out: &mut Vec<Comet>) {
    if window.is_empty() {
        return;
    }
    let reach =
        i128::from(model.reach.seconds()) * 1_000_000_000 + i128::from(model.reach.subsec_nanos());
    let around = TimeWindow::new(
        from_nanos_saturating(to_nanos(window.start()) - reach),
        from_nanos_saturating(to_nanos(window.end()) + reach),
    )
    .expect("a window widened at both ends is still ordered");
    let mut events = Vec::new();
    bins().events_in(model.series(), around, model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(|event| comet_of(model, event))
            .filter(|comet| {
                comet.listed.start() < window.end() && window.start() < comet.listed.end()
            }),
    );
}

/// Appends to `out` the system's comets listed at `t`, those inside the listing radius then,
/// sorted by time of perihelion and then by ID; none at the clock's last instant.
///
/// # Panics
///
/// Never: the instant after `t`, clamped to the clock, is never before it.
pub fn comets_listed_at(model: &CometModel, t: UniverseTime, out: &mut Vec<Comet>) {
    // The nanosecond after `t`, or `t` itself at the clock's last instant, never before it, so
    // the window is ordered; at the last instant it is empty and lists nothing.
    let next = from_nanos_saturating(to_nanos(t) + 1);
    let instant = TimeWindow::new(t, next).expect("the next nanosecond is not before t");
    comets_listed_in(model, instant, out);
}

/// The comet `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject) or names a
/// number beyond its bin's count.
#[must_use]
pub fn comet(model: &CometModel, id: EventId) -> Option<Comet> {
    bins()
        .event(model.series(), id, model)
        .map(|event| comet_of(model, event))
}

#[cfg(test)]
mod tests;
