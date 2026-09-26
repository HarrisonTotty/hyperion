//! Rotation and magnetism of living stars: one draw each per star, giving the rotation period and
//! equatorial speed, fossil and dynamo fields, the activity level, and through them the Be, Ap, Bp
//! and Am stars (plan 06, P06.T25).
//!
//! Every quantity is a closed form in the star's state and its draws, so it is continuous in age
//! and the same whatever was asked before:
//!
//! - **Above 1.3 M☉** (hot stars, with radiative envelopes that barely brake) the rotation rank
//!   `u_rot` (`star.rotation`) maps onto the distribution of the equatorial speed as a fraction of
//!   the critical speed, v ÷ v(crit) with v(crit) = √(2GM ÷ 3R) of the Roche model (Huang et al.
//!   2010, ApJ 722, 605, equation 1). The fraction is held through the main sequence and
//!   the speed follows the state's mass and radius. The distribution is bimodal for late B and A
//!   stars (Zorec and Royer 2012, A&A 537, A120): a small slow mode below 0.3 and a broad fast mode
//!   whose centre follows their equation 5 and Huang et al.'s B stars ([`fast_fraction_mean`]).
//! - **Below 1.3 M☉** (the Kraft break) the rank maps onto the spread of birth periods, which
//!   magnetic braking erases: P(t)² = P₀² + P(g)(t)², the solution of Skumanich's torque
//!   dΩ ÷ dt ∝ −Ω³, with the gyrochronology law P(g) of Mamajek and Hillenbrand (2008, ApJ 687, 1264,
//!   table 10) as its clock, and with the torque saturated below the Rossby number 0.13 (Wright et
//!   al. 2011, ApJ 743, 48), where it falls only as Ω and the spin-down is exponential. Fully
//!   convective stars, whose turnover times are long, stay saturated and fast for a gigayear.
//! - After the main sequence a star keeps the specific angular momentum it ended it with, so that
//!   its period grows as its radius squared (the lane's addition to the plan).
//! - A protostar, still gaining mass, has no rotation here.
//! - `u_mag` (`star.magnetism`) gives a fossil field to a share of main-sequence stars above
//!   1.5 M☉, with a log-normal strength (Sikora et al. 2019, MNRAS 483, 3127); these are forced into
//!   the slow mode, as the Ap and Bp stars are. Stars below 1.3 M☉ carry a dynamo field and an
//!   X-ray activity from their Rossby number (Reiners et al. 2022, A&A 662, A41; Wright et al.
//!   2011).
//!
//! The spin axis is `star.spin_axis`, isotropic. The flare rate of P06.T28.a reads the activity.
//!
//! Every figure the plan gives is built as it gives it, and the ones the plan leaves to this task
//! are recorded with their sources on their constants; the lane's research found several of
//! them discrepant, and those are marked provisional pending a ruling (see the report of P06.T25).

use crate::coords::UnitVector;
use crate::math;
use crate::rng::Threshold;
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::photometry::colour_b_v;
use crate::stellar::sse::{ZCoeffs, zams};
use crate::stellar::{Composition, Phase, StarState};
use crate::units::consts::{
    GM_SUN, SECONDS_PER_DAY, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_RADIUS_M,
};
use crate::units::{
    Days, Gauss, Kelvin, KilometresPerSecond, Magnitudes, MetresPerSecond, SolarMasses,
};

/// The mass below which stars brake magnetically, M☉: the Kraft break at about F5, where the outer
/// convection zone becomes thin (plan 06, P06.T25).
pub const KRAFT_BREAK_MASS: SolarMasses = SolarMasses::new(1.3);

/// The mass above which a main-sequence star may carry a fossil field, M☉ (plan 06, P06.T25).
pub const FOSSIL_FIELD_MIN_MASS: SolarMasses = SolarMasses::new(1.5);

/// The share of main-sequence stars above [`FOSSIL_FIELD_MIN_MASS`] with a fossil field: 8%,
/// within the plan's 7–10% (the Magnetism in Massive Stars survey's 7 ± 1% of OB stars, Wade et al. 2014, IAUS 302,
/// 265). Provisional: Sikora et al. (2019, MNRAS 483, 2300, section 6.2) find the share rising
/// from 0.3% below 1.8 M☉ to 11% at 3.4–3.8 M☉.
pub const FOSSIL_FIELD_SHARE: f64 = 0.08;

/// The median dipole strength of a fossil field, G: 2.6 kG, the mean log B(d) = 3.4 of the
/// volume-limited Ap and Bp sample of Sikora et al. (2019, MNRAS 483, 3127, section 5.1).
pub const FOSSIL_FIELD_MEDIAN: Gauss = Gauss::new(2_600.0);

/// The log-normal spread of fossil-field strengths, dex: 0.4, from the range of 330 G–18 kG that
/// Sikora et al. (2019) find (the lane's estimate; provisional).
pub const FOSSIL_FIELD_SIGMA_DEX: f64 = 0.4;

/// The weakest fossil field, G: none below about 300 G is found (Aurière et al. 2007, A&A 475,
/// 1053, their critical field; Sikora et al. 2019).
pub const FOSSIL_FIELD_FLOOR: Gauss = Gauss::new(300.0);

/// The strongest fossil field, G: 30 kG, above every Ap star of Sikora et al. (2019).
pub const FOSSIL_FIELD_CEILING: Gauss = Gauss::new(30_000.0);

/// The fraction of critical speed above which a B-type main-sequence star is a Be star: 0.7 (plan
/// 06, P06.T25; Rivinius, Carciofi and Martayan 2013, A&ARv 21, 69, put it near 0.75 of the orbital
/// speed; provisional).
pub const BE_CRITICAL_FRACTION: f64 = 0.7;

/// The equatorial speed below which a non-magnetic A star is an Am star, km/s: 120 (Abt 2009, AJ
/// 138, 28, as a single-star stand-in for tidal braking; plan 06, P06.T25).
pub const AM_MAX_SPEED: KilometresPerSecond = KilometresPerSecond::new(120.0);

/// The effective temperatures of the Ap and Bp stars, K: 7,000–20,000 (plan 06, P06.T25).
pub const AP_TEMPERATURES: (Kelvin, Kelvin) = (Kelvin::new(7_000.0), Kelvin::new(20_000.0));

/// The effective temperatures of the Am stars, K: 7,000–10,000 (plan 06, P06.T25).
pub const AM_TEMPERATURES: (Kelvin, Kelvin) = (Kelvin::new(7_000.0), Kelvin::new(10_000.0));

/// The Rossby number below which activity and the braking torque saturate: 0.13 (Wright et al.
/// 2011, section 3.1: 0.13 ± 0.02).
pub const SATURATION_ROSSBY: f64 = 0.13;

/// log₁₀ L(X) ÷ L(bol) of a saturated star: −3.13 ± 0.08 (Wright et al. 2011, section 3.1; plan 06
/// rounds it to 10⁻³).
pub const SATURATED_LOG_LX_LBOL: f64 = -3.13;

/// The power of the Rossby number in L(X) ÷ L(bol) above saturation: −2.70 ± 0.13 (Wright et al.
/// 2011, from their unbiased subsample, valid 0.3 < Ro < 3).
pub const ACTIVITY_SLOPE: f64 = -2.70;

/// Mamajek and Hillenbrand's (2008, table 10) gyrochronology law, P = a [(B − V)₀ − c]^b t^n with
/// t in Myr and P in days: a = 0.407, b = 0.325, c = 0.495, n = 0.566, calibrated for 0.5 <
/// (B − V)₀ < 0.9 and forced through the Sun's 26.09 d. Plan 06 asks for Skumanich's t^½; the
/// source's own exponent is used (provisional).
const GYRO: (f64, f64, f64, f64) = (0.407, 0.325, 0.495, 0.566);

/// The median birth period of a star of 0.4–1.3 M☉, days: 4, between the peaks near 2 and 8 days
/// of the Orion Nebula Cluster (Herbst et al. 2007, Protostars and Planets V, 297, section 3),
/// whose range of about 0.6–20 d the spread [`BIRTH_PERIOD_SIGMA_DEX`] spans at 2σ. Below
/// 0.25 M☉ the median is half as long; between, it is interpolated in log period.
const BIRTH_PERIOD_MEDIAN_DAYS: f64 = 4.0;

/// The log-normal spread of birth periods, dex (after Herbst et al. 2007; the lane's choice).
const BIRTH_PERIOD_SIGMA_DEX: f64 = 0.35;

/// The fast mode's spread in v ÷ v(crit) (after Huang et al. 2010's B stars: 6% below 0.1, 52% at
/// 0.4–0.8, 1.3% above 0.9; the lane's fit).
const FAST_SIGMA: f64 = 0.2;

/// The slow mode's centre and spread in v ÷ v(crit) (Zorec and Royer 2012, table 4: modes of
/// 25–61 km/s).
const SLOW_MODE: (f64, f64) = (0.08, 0.04);

/// The fraction of critical speed a draw is held to: a star at break-up has no stable surface.
const FRACTION_RANGE: (f64, f64) = (0.01, 0.99);

/// The centre of the fast mode of v ÷ v(crit), by log₁₀ mass: rising from the F stars through the
/// A stars (Zorec and Royer 2012, equation 5, μ ≈ 41 M + 90 km/s at typical main-sequence radii)
/// to the late B stars, then falling to the early B and O stars (Huang et al. 2010, section 5:
/// 37%, 53% and 84% below 0.5 at 2–4, 4–8 and above 8 M☉). The lane's fit; provisional.
const FAST_MEAN_NODES: [(f64, f64); 7] = [
    (1.3, 0.20),
    (1.6, 0.40),
    (2.0, 0.47),
    (3.0, 0.56),
    (6.0, 0.49),
    (10.0, 0.33),
    (20.0, 0.30),
];

/// The share of the slow mode, by mass: none below 2 M☉, 3% at 2.1–2.5, 12–20% at 2.3–3.05 and
/// 5–8% above 2.95 M☉ (Zorec and Royer 2012, table 4). Held beyond the last node.
const SLOW_SHARE_NODES: [(f64, f64); 4] = [(2.0, 0.0), (2.3, 0.03), (2.7, 0.15), (3.2, 0.07)];

/// A star's spin axis, along the galactic axes: isotropic, from `star.spin_axis`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinAxis(UnitVector);

impl SpinAxis {
    /// The axis's direction, towards the pole from which the star turns anticlockwise.
    #[must_use]
    pub const fn direction(&self) -> UnitVector {
        self.0
    }
}

/// A living star's rotation (P06.T25).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    period: Days,
    equatorial_speed: KilometresPerSecond,
    critical_fraction: f64,
    axis: SpinAxis,
}

impl Rotation {
    /// The rotation period at the equator.
    #[must_use]
    pub const fn period(&self) -> Days {
        self.period
    }

    /// The equatorial speed, 2πR ÷ P.
    #[must_use]
    pub const fn equatorial_speed(&self) -> KilometresPerSecond {
        self.equatorial_speed
    }

    /// The equatorial speed as a fraction of the critical speed √(2GM ÷ 3R), 0–1.
    #[must_use]
    pub const fn critical_fraction(&self) -> f64 {
        self.critical_fraction
    }

    /// The spin axis.
    #[must_use]
    pub const fn axis(&self) -> SpinAxis {
        self.axis
    }
}

/// A star's surface magnetic field (P06.T25).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Magnetism {
    /// A fossil dipole field of a main-sequence star above 1.5 M☉, frozen in since its formation:
    /// the Ap and Bp stars' (Sikora et al. 2019). The field is the dipole's strength at the pole.
    Fossil {
        /// The polar dipole strength.
        field: Gauss,
    },
    /// A dynamo field of a cool star, the mean unsigned surface field ⟨B⟩ from the Rossby number
    /// (Reiners et al. 2022, table 2).
    Dynamo {
        /// The mean surface field.
        field: Gauss,
    },
}

impl Magnetism {
    /// The field strength.
    #[must_use]
    pub const fn field(&self) -> Gauss {
        match *self {
            Self::Fossil { field } | Self::Dynamo { field } => field,
        }
    }
}

/// How magnetically active a cool star is, from its Rossby number (P06.T25, Wright et al. 2011).
///
/// The bands below saturation are HYPERION's, a decade of L(X) ÷ L(bol) each: the Sun, at about
/// 10⁻⁶·², is [`Low`](Self::Low).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActivityLevel {
    /// Saturated: Ro ≤ 0.13, L(X) ÷ L(bol) ≈ 10⁻³·¹³.
    Saturated,
    /// High: L(X) ÷ L(bol) of 10⁻⁴ or more, below saturation.
    High,
    /// Moderate: 10⁻⁵ to 10⁻⁴.
    Moderate,
    /// Low: below 10⁻⁵, as the Sun.
    Low,
}

/// A cool star's magnetic activity: its Rossby number and coronal X-ray luminosity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Activity {
    rossby: f64,
    log_lx_lbol: f64,
}

impl Activity {
    /// The Rossby number P ÷ τ, with the convective turnover time τ of Wright et al. (2011,
    /// equation 11).
    #[must_use]
    pub const fn rossby(&self) -> f64 {
        self.rossby
    }

    /// log₁₀ of the X-ray to bolometric luminosity: −3.13 when saturated, falling as Ro^−2.70.
    #[must_use]
    pub const fn log_lx_lbol(&self) -> f64 {
        self.log_lx_lbol
    }

    /// The activity band.
    #[must_use]
    pub fn level(&self) -> ActivityLevel {
        if self.rossby <= SATURATION_ROSSBY {
            ActivityLevel::Saturated
        } else if self.log_lx_lbol >= -4.0 {
            ActivityLevel::High
        } else if self.log_lx_lbol >= -5.0 {
            ActivityLevel::Moderate
        } else {
            ActivityLevel::Low
        }
    }
}

/// Where a phase stands for rotation.
enum Stage {
    /// Before the end of the main sequence: the star's own law.
    MainSequence,
    /// After it, keeping the angular momentum it ended it with.
    Evolved,
    /// A protostar (still gaining mass, and so crossing the Kraft break), a stripped helium star,
    /// a post-AGB star, a remnant or a brown dwarf: not modelled here.
    Other,
}

/// Where `phase` stands.
#[must_use]
const fn stage(phase: Phase) -> Stage {
    match phase {
        Phase::PreMainSequence | Phase::MainSequence => Stage::MainSequence,
        Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb => Stage::Evolved,
        Phase::Protostar
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => Stage::Other,
    }
}

/// The rotation of a star in `state`, of `composition` and `draws`, or `None` where it is not
/// modelled: a protostar, a stripped helium star, a post-AGB star, a remnant (a neutron star's
/// spin is its pulsar's) or a brown dwarf.
///
/// `terminal_main_sequence` is the star's state at the end of its main sequence, which an evolved
/// star's rotation needs (its angular momentum then); an evolved star without it has none.
///
/// # Examples
///
/// The Sun at its present age, at the median draw, turns in about 26 days:
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::rotation::rotation;
/// use hyperion_sim::stellar::{Composition, Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// let sun = StarState::new(StarStateParts {
///     phase: Phase::MainSequence,
///     age: Years::new(4.57e9),
///     mass: SolarMasses::new(1.0),
///     core_mass: SolarMasses::ZERO,
///     luminosity: SolarLuminosities::new(1.0),
///     radius: SolarRadii::new(1.0),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.45,
/// });
/// let spin = rotation(&sun, &Composition::SOLAR, &StarDraws::median(), None).ok_or("modelled")?;
/// assert!((22.0..30.0).contains(&spin.period().value()));
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn rotation(
    state: &StarState,
    composition: &Composition,
    draws: &StarDraws,
    terminal_main_sequence: Option<&StarState>,
) -> Option<Rotation> {
    let axis = SpinAxis(draws.spin_axis());
    let (period_days, radius) = match stage(state.phase()) {
        Stage::MainSequence => (
            main_sequence_period(state, composition, draws),
            state.radius().value(),
        ),
        Stage::Evolved => {
            let end = terminal_main_sequence?;
            let at_end = main_sequence_period(end, composition, draws);
            let ratio = state.radius().value() / end.radius().value();
            (at_end * ratio * ratio, state.radius().value())
        }
        Stage::Other => return None,
    };
    let speed =
        2.0 * core::f64::consts::PI * radius * SOLAR_RADIUS_M / (period_days * SECONDS_PER_DAY);
    Some(Rotation {
        period: Days::new(period_days),
        equatorial_speed: KilometresPerSecond::from(MetresPerSecond::new(speed)),
        critical_fraction: speed / critical_speed(state.mass().value(), radius),
        axis,
    })
}

/// The fossil field of a main-sequence star in `state` with `draws`, if it has one: a share
/// [`FOSSIL_FIELD_SHARE`] of main-sequence stars above 1.5 M☉, by the `star.magnetism` mark, with
/// a log-normal strength about 2.6 kG from the mark's rank within the share, held to 300 G–30 kG.
#[must_use]
pub fn fossil_field(state: &StarState, draws: &StarDraws) -> Option<Gauss> {
    let eligible =
        matches!(state.phase(), Phase::MainSequence) && state.mass() >= FOSSIL_FIELD_MIN_MASS;
    let threshold = Threshold::from_probability(FOSSIL_FIELD_SHARE);
    if !eligible || !draws.magnetism().is_below(threshold) {
        return None;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a mark and a threshold are integers below 2^53, exact in f64"
    )]
    let rank = (draws.magnetism().get() as f64 + 0.5) / threshold.get() as f64;
    // A mark below the threshold ranks strictly inside (0, 1), whose quantile is finite.
    let z = StandardNormal::new(math::normal_quantile(rank)).unwrap_or(StandardNormal::ZERO);
    let log_field = z.truncated(
        math::log10(FOSSIL_FIELD_MEDIAN.value()),
        FOSSIL_FIELD_SIGMA_DEX,
        math::log10(FOSSIL_FIELD_FLOOR.value()),
        math::log10(FOSSIL_FIELD_CEILING.value()),
    );
    Some(Gauss::new(math::exp10(log_field)))
}

/// The magnetism of a star in `state` with `draws` and its `rotation`: a fossil field
/// ([`fossil_field`]), or for a cool star before the end of its main sequence the dynamo's mean
/// field from its Rossby number, or `None` (a hot star without a fossil field, an evolved star, or
/// what [`rotation`] does not model).
///
/// The dynamo field follows Reiners et al. (2022, table 2), ⟨B⟩ = 199 G × Ro^−1.26 above Ro = 0.13
/// and Ro^−0.11 below it, with their turnover time τ = 12.3 d × (L ÷ L☉)^−½; the saturated
/// branch is scaled to meet the other at 0.13 (their two fits miss by 1.5%), so the field is
/// continuous.
#[must_use]
pub fn magnetism(
    state: &StarState,
    draws: &StarDraws,
    rotation: Option<&Rotation>,
) -> Option<Magnetism> {
    if let Some(field) = fossil_field(state, draws) {
        return Some(Magnetism::Fossil { field });
    }
    let spin = rotation?;
    if !is_cool_dwarf(state) {
        return None;
    }
    let turnover = 12.3 / state.luminosity().value().sqrt();
    let rossby = spin.period().value() / turnover;
    let at_saturation = 199.0 * math::powf(SATURATION_ROSSBY, -1.26);
    let field = if rossby >= SATURATION_ROSSBY {
        199.0 * math::powf(rossby, -1.26)
    } else {
        at_saturation * math::powf(rossby / SATURATION_ROSSBY, -0.11)
    };
    Some(Magnetism::Dynamo {
        field: Gauss::new(field),
    })
}

/// The activity of a cool star in `state` with its `rotation`: its Rossby number with Wright et
/// al.'s turnover time, and L(X) ÷ L(bol) from it; `None` for a star above 1.3 M☉ or after its main
/// sequence.
#[must_use]
pub fn activity(state: &StarState, rotation: Option<&Rotation>) -> Option<Activity> {
    let spin = rotation?;
    if !is_cool_dwarf(state) {
        return None;
    }
    let rossby = spin.period().value() / wright_turnover_days(state.mass().value());
    let log_lx_lbol = if rossby <= SATURATION_ROSSBY {
        SATURATED_LOG_LX_LBOL
    } else {
        math::mul_add(
            ACTIVITY_SLOPE,
            math::log10(rossby / SATURATION_ROSSBY),
            SATURATED_LOG_LX_LBOL,
        )
    };
    Some(Activity {
        rossby,
        log_lx_lbol,
    })
}

/// Whether the star in `state` is a cool dwarf with a convective envelope: below 1.3 M☉ and
/// before the end of its main sequence.
#[must_use]
fn is_cool_dwarf(state: &StarState) -> bool {
    matches!(stage(state.phase()), Stage::MainSequence) && state.mass() < KRAFT_BREAK_MASS
}

/// The period, days, that the main-sequence law gives a star in `state`: by the fraction of
/// critical speed above the Kraft break, by the braked birth period below it.
#[must_use]
fn main_sequence_period(state: &StarState, composition: &Composition, draws: &StarDraws) -> f64 {
    let mass = state.mass().value();
    let radius = state.radius().value();
    if state.mass() >= KRAFT_BREAK_MASS {
        // Whether the star has a fossil field from its mass and mark alone, so that an evolved
        // star read at the start of its Hertzsprung gap keeps the slow mode it had.
        let magnetic = state.mass() >= FOSSIL_FIELD_MIN_MASS
            && draws
                .magnetism()
                .is_below(Threshold::from_probability(FOSSIL_FIELD_SHARE));
        let fraction = critical_fraction_of(mass, draws.rotation().value(), magnetic);
        let speed = fraction * critical_speed(mass, radius);
        2.0 * core::f64::consts::PI * radius * SOLAR_RADIUS_M / speed / SECONDS_PER_DAY
    } else {
        braked_period(mass, state.age().value() * 1e-6, composition, draws)
    }
}

/// The critical speed √(2GM ÷ 3R) of a star of `mass` M☉ and polar radius `radius` R☉, m/s (the
/// Roche model; Huang et al. 2010, equation 1).
#[must_use]
fn critical_speed(mass: f64, radius: f64) -> f64 {
    (2.0 * GM_SUN * mass / (3.0 * radius * SOLAR_RADIUS_M)).sqrt()
}

/// The fraction of critical speed of a star of `mass` M☉ at rotation rank `rank`, in the slow mode
/// if it is `magnetic`: the rank splits into the slow mode's share and the fast mode's, and within
/// each maps through a truncated normal.
#[must_use]
fn critical_fraction_of(mass: f64, rank: f64, magnetic: bool) -> f64 {
    let slow_share = interpolate_log_mass(&SLOW_SHARE_NODES, mass);
    let (mode, within) = if magnetic {
        (SLOW_MODE, rank)
    } else if rank < slow_share {
        (SLOW_MODE, rank / slow_share)
    } else {
        (
            (fast_fraction_mean(mass), FAST_SIGMA),
            (rank - slow_share) / (1.0 - slow_share),
        )
    };
    let within = within.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let z = StandardNormal::new(math::normal_quantile(within))
        .expect("a rank strictly inside (0, 1) has a finite quantile");
    z.truncated(mode.0, mode.1, FRACTION_RANGE.0, FRACTION_RANGE.1)
}

/// The centre of the fast mode of v ÷ v(crit) for `mass` M☉ ([`FAST_MEAN_NODES`]).
#[must_use]
pub(crate) fn fast_fraction_mean(mass: f64) -> f64 {
    interpolate_log_mass(&FAST_MEAN_NODES, mass)
}

/// The value of `nodes` (mass, value) at `mass`, linear in log mass, held beyond the ends.
#[must_use]
fn interpolate_log_mass(nodes: &[(f64, f64)], mass: f64) -> f64 {
    let first = nodes[0];
    let last = nodes[nodes.len() - 1];
    if mass <= first.0 {
        return first.1;
    }
    if mass >= last.0 {
        return last.1;
    }
    let x = math::log10(mass);
    nodes
        .windows(2)
        .find(|pair| mass <= pair[1].0)
        .map_or(last.1, |pair| {
            let (x0, x1) = (math::log10(pair[0].0), math::log10(pair[1].0));
            pair[0].1 + (pair[1].1 - pair[0].1) * (x - x0) / (x1 - x0)
        })
}

/// The period, days, of a star of `mass` M☉ below the Kraft break at `age_myr`, of `composition`
/// and `draws`: its birth period braked by Skumanich's torque with Mamajek and Hillenbrand's clock,
/// saturated below Ro = 0.13.
///
/// With G(t) = P(g)(t)² the gyrochronology law squared, the unsaturated torque gives
/// d(P²) ÷ dt = dG ÷ dt, so P² = P₀² + G, and the saturated one (torque ∝ Ω Ω(sat)²) gives
/// d(ln P) ÷ dt = (dG ÷ dt) ÷ 2P(sat)², so P = P₀ exp(G ÷ 2P(sat)²) until P reaches P(sat) = 0.13 τ,
/// at G₁ = 2P(sat)² ln(P(sat) ÷ P₀), and P² = P(sat)² + G − G₁ after. Both are continuous and rise
/// with age. The colour in P(g) is the star's at the zero-age main sequence, fixed, so that the
/// clock never runs backwards as the star evolves.
#[must_use]
fn braked_period(mass: f64, age_myr: f64, composition: &Composition, draws: &StarDraws) -> f64 {
    let birth = math::exp10(math::mul_add(
        BIRTH_PERIOD_SIGMA_DEX,
        math::normal_quantile(draws.rotation().value()),
        math::log10(birth_period_median(mass)),
    ));
    let (scale, colour_power, blue_edge, age_power) = GYRO;
    let excess = (zams_colour(mass, composition) - blue_edge).max(0.0);
    let clock = if excess > 0.0 && age_myr > 0.0 {
        let gyro = scale * math::powf(excess, colour_power) * math::powf(age_myr, age_power);
        gyro * gyro
    } else {
        0.0
    };
    let saturation = SATURATION_ROSSBY * wright_turnover_days(mass);
    if birth >= saturation {
        return (birth * birth + clock).sqrt();
    }
    let reach = 2.0 * saturation * saturation * math::ln(saturation / birth);
    if clock < reach {
        birth * math::exp(clock / (2.0 * saturation * saturation))
    } else {
        (saturation * saturation + clock - reach).sqrt()
    }
}

/// The median birth period of a star of `mass` M☉, days: 4 above 0.4 M☉, 2 below 0.25 M☉, and
/// between them interpolated in log period (Herbst et al. 2007).
#[must_use]
fn birth_period_median(mass: f64) -> f64 {
    let (low, high) = (0.25, 0.4);
    let t = ((mass - low) / (high - low)).clamp(0.0, 1.0);
    BIRTH_PERIOD_MEDIAN_DAYS * math::exp2(t - 1.0)
}

/// The convective turnover time of a star of `mass` M☉, days: log τ = 1.16 − 1.49 log M − 0.54
/// log² M (Wright et al. 2011, equation 11, valid 0.09–1.36 M☉, to which the mass is held).
#[must_use]
fn wright_turnover_days(mass: f64) -> f64 {
    let lm = math::log10(mass.clamp(0.09, 1.36));
    math::exp10(1.16 - 1.49 * lm - 0.54 * lm * lm)
}

/// The intrinsic B − V colour of a star of `mass` M☉ and `composition` at the zero-age main
/// sequence (Tout et al. 1996's radius and luminosity, the dwarf colours of Pecaut and Mamajek
/// 2013), with the mass held to 0.1–1.3 M☉, where the fits and the colour table both reach.
#[must_use]
fn zams_colour(mass: f64, composition: &Composition) -> f64 {
    let coeffs = ZCoeffs::new(composition.z_fit());
    let m = SolarMasses::new(mass.clamp(0.1, KRAFT_BREAK_MASS.value()));
    let l = zams::luminosity(m, &coeffs).value();
    let r = zams::radius(m, &coeffs).value();
    let teff = SOLAR_EFFECTIVE_TEMPERATURE_K * math::powf(l, 0.25) / r.sqrt();
    colour_b_v(Kelvin::new(teff)).map_or(2.0, Magnitudes::value)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::stellar::StarStateParts;
    use crate::stellar::draws::{StarDrawsParts, UnitUniform};
    use crate::units::{SolarLuminosities, SolarMassesPerYear, SolarRadii, Years};

    fn dwarf(mass: f64, luminosity: f64, radius: f64, age: f64) -> StarState {
        StarState::new(StarStateParts {
            phase: Phase::MainSequence,
            age: Years::new(age),
            mass: SolarMasses::new(mass),
            core_mass: SolarMasses::ZERO,
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    fn ranked(rank: f64) -> StarDraws {
        StarDraws::from_parts(StarDrawsParts {
            rotation: UnitUniform::new(rank).expect("open"),
            ..StarDrawsParts::MEDIAN
        })
    }

    /// The plan's test: the Sun's age and mass give a 22–30 day period and a low activity level
    /// for the median draw.
    #[test]
    fn the_median_sun_turns_in_about_four_weeks_and_is_quiet() {
        let sun = dwarf(1.0, 1.0, 1.0, 4.57e9);
        let spin = rotation(&sun, &Composition::SOLAR, &StarDraws::median(), None).unwrap();
        let p = spin.period().value();
        assert!((22.0..=30.0).contains(&p), "{p} d");
        assert!((spin.equatorial_speed().value() - 2.0).abs() < 0.4);
        let active = activity(&sun, Some(&spin)).unwrap();
        assert_eq!(active.level(), ActivityLevel::Low, "{active:?}");
        assert!((-7.0..-5.5).contains(&active.log_lx_lbol()), "{active:?}");
        let field = magnetism(&sun, &StarDraws::median(), Some(&spin)).unwrap();
        assert!(matches!(field, Magnetism::Dynamo { .. }));
        // Reiners et al.'s relation gives the Sun about 70 G at its Rossby number of 2.1.
        assert!((30.0..150.0).contains(&field.field().value()), "{field:?}");
    }

    /// The plan's test: the rotation period of a cool dwarf is continuous and rises with age, for
    /// every rank, the saturated fully convective stars included.
    #[test]
    fn a_cool_dwarfs_period_rises_continuously_with_age() {
        for mass in [0.12, 0.2, 0.35, 0.6, 0.9, 1.1, 1.25] {
            for rank in [0.01, 0.2, 0.5, 0.8, 0.99] {
                let draws = ranked(rank);
                let at = |myr: f64| braked_period(mass, myr, &Composition::SOLAR, &draws);
                let mut last = at(0.0);
                for i in 1..=4_000 {
                    let myr = f64::from(i) * 3.5;
                    let p = at(myr);
                    assert!(
                        p >= last,
                        "{mass} M☉, rank {rank}: {p} after {last} at {myr} Myr"
                    );
                    let next = at(myr + 1e-3);
                    assert!(
                        next >= p && next - p < 1e-4 * p,
                        "{mass}, {rank}: a jump at {myr} Myr, {p} to {next}"
                    );
                    last = p;
                }
            }
        }
    }

    /// Fully convective stars stay saturated and fast for longer than Sun-like ones.
    #[test]
    fn fully_convective_stars_spin_down_late() {
        let draws = ranked(0.3);
        let young_m = braked_period(0.2, 300.0, &Composition::SOLAR, &draws);
        let old_m = braked_period(0.2, 8_000.0, &Composition::SOLAR, &draws);
        assert!(young_m < 5.0, "{young_m}");
        assert!(old_m > 30.0, "{old_m}");
        let sun_like = braked_period(1.0, 300.0, &Composition::SOLAR, &draws);
        assert!(sun_like > young_m, "{sun_like} against {young_m}");
    }

    /// The draws of a B-type main-sequence star at a random rank and fossil mark.
    fn random_draws(rng: &mut Lcg) -> StarDraws {
        let open = |rng: &mut Lcg| (rng.next_f64() + f64::EPSILON) / (1.0 + 2.0 * f64::EPSILON);
        StarDraws::from_parts(StarDrawsParts {
            rotation: UnitUniform::new(open(rng)).expect("open"),
            magnetism: crate::rng::Mark::from_word(rng.next_u64()),
            ..StarDrawsParts::MEDIAN
        })
    }

    /// A main-sequence star of `mass` M☉ halfway through its main sequence, on rough mass–radius
    /// and mass–luminosity laws (R ∝ M^0.6, L ∝ M^3.5), enough for a share.
    fn b_star(mass: f64) -> StarState {
        dwarf(
            mass,
            math::powf(mass, 3.5),
            1.2 * math::powf(mass, 0.6),
            1e7,
        )
    }

    /// The plan's test: Be stars are 10–25% of B-type main-sequence stars, over an IMF of 2.5–18 M☉
    /// (Salpeter's slope, which the upper IMF follows).
    #[test]
    fn be_stars_are_a_tenth_to_a_quarter_of_b_stars() {
        let mut rng = Lcg::new(17);
        let (mut be, mut n) = (0_u32, 0_u32);
        for _ in 0..20_000 {
            let u = rng.next_f64();
            let mass = math::powf(
                math::powf(2.5, -1.35) + u * (math::powf(18.0, -1.35) - math::powf(2.5, -1.35)),
                -1.0 / 1.35,
            );
            let state = b_star(mass);
            let draws = random_draws(&mut rng);
            let spin = rotation(&state, &Composition::SOLAR, &draws, None).unwrap();
            n += 1;
            be += u32::from(
                fossil_field(&state, &draws).is_none()
                    && spin.critical_fraction() > BE_CRITICAL_FRACTION,
            );
        }
        let share = f64::from(be) / f64::from(n);
        assert!((0.10..=0.25).contains(&share), "{share}");
    }

    /// Fossil fields: the share and the strengths, and magnetic stars rotate slowly.
    #[test]
    fn fossil_fields_are_a_few_percent_and_slow_their_stars() {
        let mut rng = Lcg::new(23);
        let (mut fossil, mut n) = (0_u32, 0_u32);
        for _ in 0..40_000 {
            let draws = random_draws(&mut rng);
            let state = b_star(3.0);
            n += 1;
            if let Some(field) = fossil_field(&state, &draws) {
                fossil += 1;
                assert!((300.0..=30_000.0).contains(&field.value()), "{field:?}");
                let spin = rotation(&state, &Composition::SOLAR, &draws, None).unwrap();
                assert!(spin.critical_fraction() < 0.3, "{spin:?}");
            }
        }
        let share = f64::from(fossil) / f64::from(n);
        assert!((share - FOSSIL_FIELD_SHARE).abs() < 0.005, "{share}");
        assert_eq!(fossil_field(&b_star(1.4), &StarDraws::median()), None);
    }

    /// Above the Kraft break the fraction of critical speed is fixed through the main sequence, so
    /// a star that swells rotates more slowly; the saturated activity is 10⁻³·¹³.
    #[test]
    fn hot_stars_keep_their_fraction_of_critical_speed() {
        let draws = ranked(0.7);
        let young = rotation(&b_star(3.0), &Composition::SOLAR, &draws, None).unwrap();
        let swollen = dwarf(3.0, 150.0, 4.0, 3e8);
        let old = rotation(&swollen, &Composition::SOLAR, &draws, None).unwrap();
        assert!((young.critical_fraction() - old.critical_fraction()).abs() < 1e-12);
        assert!(old.period() > young.period());
        assert_eq!(activity(&swollen, Some(&old)), None);
    }

    /// An evolved star keeps its angular momentum: its period grows as the square of its radius.
    #[test]
    fn a_giant_keeps_its_angular_momentum() {
        let end = dwarf(1.0, 2.0, 1.4, 1.0e10);
        let mut giant = StarStateParts {
            phase: Phase::FirstGiantBranch,
            age: Years::new(1.1e10),
            mass: SolarMasses::new(1.0),
            core_mass: SolarMasses::new(0.3),
            luminosity: SolarLuminosities::new(30.0),
            radius: SolarRadii::new(14.0),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        };
        let draws = StarDraws::median();
        let at_end = rotation(&end, &Composition::SOLAR, &draws, None).unwrap();
        let spin = rotation(
            &StarState::new(giant),
            &Composition::SOLAR,
            &draws,
            Some(&end),
        )
        .unwrap();
        assert!((spin.period().value() / at_end.period().value() - 100.0).abs() < 1e-9);
        giant.phase = Phase::PostAgb;
        assert_eq!(
            rotation(
                &StarState::new(giant),
                &Composition::SOLAR,
                &draws,
                Some(&end)
            ),
            None
        );
    }

    /// A magnetic star keeps its slow rotation after its main sequence: the terminal state is read
    /// in the Hertzsprung gap, where it has no fossil field of its own.
    #[test]
    fn a_magnetic_star_stays_slow_after_its_main_sequence() {
        let draws = StarDraws::from_parts(StarDrawsParts {
            rotation: UnitUniform::new(0.9).expect("open"),
            magnetism: crate::rng::Mark::from_word(0),
            ..StarDrawsParts::MEDIAN
        });
        let ms = b_star(3.0);
        let mut gap = StarStateParts {
            phase: Phase::HertzsprungGap,
            age: ms.age(),
            mass: ms.mass(),
            core_mass: ms.core_mass(),
            luminosity: ms.luminosity(),
            radius: ms.radius(),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        };
        let on = rotation(&ms, &Composition::SOLAR, &draws, None).unwrap();
        let end = StarState::new(gap);
        let after = rotation(&end, &Composition::SOLAR, &draws, Some(&end)).unwrap();
        assert!((after.period().value() / on.period().value() - 1.0).abs() < 1e-12);
        assert!(after.critical_fraction() < 0.3, "{after:?}");
        gap.radius = SolarRadii::new(2.0 * ms.radius().value());
        let wider = rotation(
            &StarState::new(gap),
            &Composition::SOLAR,
            &draws,
            Some(&end),
        )
        .unwrap();
        assert!((wider.period().value() / on.period().value() - 4.0).abs() < 1e-9);
    }

    /// The activity law: saturated below Ro = 0.13, and Ro^−2.7 above.
    #[test]
    fn activity_saturates_below_a_rossby_number_of_0_13() {
        let young = dwarf(0.8, 0.35, 0.75, 5e7);
        let spin = rotation(&young, &Composition::SOLAR, &ranked(0.01), None).unwrap();
        let active = activity(&young, Some(&spin)).unwrap();
        assert_eq!(active.level(), ActivityLevel::Saturated, "{active:?}");
        assert!((active.log_lx_lbol() + 3.13).abs() < 1e-12);
    }
}
