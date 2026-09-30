//! Before the main sequence: the protostar phase (Class 0 and I, with a closed form for its growth
//! in mass) and the contraction tracks ahead of the zero-age main sequence, which make the T Tauri
//! and Herbig Ae/Be stars (plan 06, P06.T15; the brainstorm's "Covering every class of star",
//! row "Before the main sequence").
//!
//! A star's age is counted from the onset of its collapse (plan 06, design note 4). Its track
//! opens with two segments, built by `stellar::sse`'s integrator from the closed forms here:
//!
//! - **The protostar** ([`Phase::Protostar`](crate::stellar::Phase)), from age zero to
//!   [`PROTOSTAR_DURATION`], `t_p` = 0.5 Myr, while it accretes its final mass `m_f` as
//!   `m_f` (1 − (1 − t ÷ `t_p`)²) ([`protostar_mass`]). It sits on the birthline of its current mass
//!   and shines by its photosphere and by accretion, G m ṁ ÷ R. It is Class 0 while its envelope,
//!   `m_f` − m, outweighs it, and Class I after ([`protostar_class`]).
//! - **The contraction** ([`Phase::PreMainSequence`](crate::stellar::Phase)), from `t_p` to the
//!   arrival on the zero-age main sequence, [`t_zams`]: Hayashi contraction at the birthline's
//!   temperature with R ∝ t^−⅓, then a Henyey segment, blended so that L, R and their first
//!   derivatives meet the main sequence's at the arrival.
//!
//! A star whose arrival falls before `t_p` (above about 6 M☉) is on the main sequence when
//! accretion ends: its protostar hands over to the main sequence at `t_p`, at the fractional age it
//! has reached by then, and it has no contraction segment.
//!
//! The disc-lifetime law of P06.T15.c, [`disc_lifetime`], was built first, because plan 14's disc
//! is its first caller (ruling 33 of 2026-09-22, "One disc lifetime per circumstellar disc"): a
//! star's T Tauri class (P06.T24) and its planets' formation (P14.T3) read the same disc, so there
//! is one lifetime, of the star's own [`StarDraws::disc_lifetime`] rank. It draws nothing.
//!
//! [`StarDraws::disc_lifetime`]: crate::stellar::draws::StarDraws::disc_lifetime

use crate::math;
use crate::stellar::Composition;
use crate::stellar::draws::UnitUniform;
use crate::stellar::sse::PhasePoint;
use crate::units::consts::{
    GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, SOLAR_RADIUS_M,
};
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii, Years};

/// The mean lifetime of a solar-mass star's protostellar disc: 2.5 Myr.
///
/// Mamajek (2009, AIP Conf. Proc. 1158, 3), Fig. 1 and eq. 1: the fraction of young stars in 22
/// clusters and groups with optically thick primordial discs, or with spectroscopic signs of
/// accretion, falls with age as exp(−t ÷ τ) with a best-fit e-folding time τ = 2.5 Myr (half-life
/// 1.7 Myr), to about 10% (dropping any one cluster moves τ by less than that). A disc fraction
/// that decays exponentially is the survival function of an exponential distribution of
/// lifetimes, which is why the law is exponential and why its mean is τ (ruling 33).
pub const DISC_LIFETIME_MEAN_SOLAR: Megayears = Megayears::new(2.5);

/// The shortest disc lifetime: 0.3 Myr.
///
/// The age of NGC 2024, the youngest cluster of Mamajek's (2009) Fig. 1, where nearly every star
/// still has its disc. Plan 06's interim bound (P06.T15.c), kept by ruling 33.
pub const DISC_LIFETIME_MIN: Megayears = Megayears::new(0.3);

/// The longest disc lifetime: 15 Myr.
///
/// Within the ages of the oldest known accretors Mamajek (2009) lists, of about 7–17 Myr and
/// 8–25 Myr. Plan 06's interim bound (P06.T15.c), kept by ruling 33.
pub const DISC_LIFETIME_MAX: Megayears = Megayears::new(15.0);

/// How slowly the mean disc lifetime falls with mass up to a solar mass: τ ∝ m^−0.1 (ruling 38).
///
/// Luhman et al. (2005, ApJ 631, L69, abstract) find disc fractions of 42 ± 13% and 50 ± 17% for
/// the brown dwarfs of IC 348 and Chamaeleon I (below about 0.08 M☉) against 33 ± 4% and 45 ± 7%
/// for their M0–M6 stars (0.1–0.7 M☉). At one age an exponential's τ goes as −1 ÷ ln f, so the
/// brown dwarfs' discs live 1.28 and 1.15 times as long across a factor of about six in mass, an
/// exponent of 0.14 and 0.08. With it a 0.05 M☉ brown dwarf's mean is 3.4 Myr, against the
/// 2.4–5.9 Myr (typically 3) Mamajek (2009) derives for four clusters' brown dwarfs.
pub const DISC_LIFETIME_LOW_MASS_EXPONENT: f64 = 0.1;

/// How fast the mean disc lifetime falls with mass above a solar mass: τ ∝ m^−1.06 (ruling 38).
///
/// Ribas, Bouy and Merín (2015, A&A 576, A52, Table 3) measure protoplanetary disc fractions of
/// 63 ± 2% and 38 ± 6% for stars below and above 2 M☉ at 1–3 Myr, and 17 ± 2% and 2% at 3–11 Myr:
/// at one age the lifetimes on the two sides of 2 M☉ differ by ln 0.38 ÷ ln 0.63 = 2.09 and
/// ln 0.02 ÷ ln 0.17 = 2.2. Mamajek (2009) gives τ ≈ 1.2 Myr for stars above 1.3 M☉. The exponent
/// halves the lifetime between 1 and 2 M☉, 2.5 Myr × 2^−1.06 = 1.20 Myr, so the law meets both.
/// Above about 7 M☉ the mean is below the 0.3 Myr floor.
pub const DISC_LIFETIME_HIGH_MASS_EXPONENT: f64 = 1.06;

/// The mean lifetime of the protostellar disc of a star of initial mass `mass`, before
/// [`disc_lifetime`] holds a lifetime to 0.3–15 Myr: 2.5 Myr × (m ÷ M☉)^−0.1 up to a solar mass
/// and 2.5 Myr × (m ÷ M☉)^−1.06 above it (ruling 38).
///
/// Nearly flat from brown dwarfs to the Sun, as Luhman et al. (2005) find
/// ([`DISC_LIFETIME_LOW_MASS_EXPONENT`]), and halving by 2 M☉, as Ribas et al. (2015) and Mamajek
/// (2009) find ([`DISC_LIFETIME_HIGH_MASS_EXPONENT`]). The normalisation is Mamajek's 2.5 Myr at a
/// solar mass ([`DISC_LIFETIME_MEAN_SOLAR`]); Ribas et al.'s own fit to all their stars, 2.7 ±
/// 0.7 Myr for inner-disc excesses (their Table A.2), agrees within its error. The two pieces meet
/// at 1 M☉, where both are 2.5 Myr exactly, and the mean never rises with mass.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
#[must_use]
pub fn disc_lifetime_mean(mass: SolarMasses) -> Megayears {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a star's mass is positive and finite, got {}",
        mass.value()
    );
    let m = mass.value();
    let exponent = if m <= 1.0 {
        DISC_LIFETIME_LOW_MASS_EXPONENT
    } else {
        DISC_LIFETIME_HIGH_MASS_EXPONENT
    };
    DISC_LIFETIME_MEAN_SOLAR * math::powf(m, -exponent)
}

/// The lifetime of the protostellar disc of a star of initial mass `mass` whose disc-lifetime rank
/// is `rank`: exponential with mean [`disc_lifetime_mean`], held to [`DISC_LIFETIME_MIN`]–
/// [`DISC_LIFETIME_MAX`] (plan 06, P06.T15.c; ruling 33).
///
/// The lifetime is the exponential quantile of the rank, −τ ln(1 − u), so it rises with the rank,
/// and the median rank gives τ ln 2 (1.73 Myr at 1 M☉, Mamajek's half-life). The rank is the
/// star's own [`StarDraws::disc_lifetime`] for a circumstellar disc; a circumbinary disc's comes
/// from plan 14's `planet.disc` stream, at the pair's total mass. The function draws nothing.
///
/// [`StarDraws::disc_lifetime`]: crate::stellar::draws::StarDraws::disc_lifetime
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
///
/// # Examples
///
/// The median star's disc lasts τ ln 2; an M dwarf's lasts a little longer, and an A star's half
/// as long:
///
/// ```
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::stellar::premain::disc_lifetime;
/// use hyperion_sim::units::SolarMasses;
///
/// let median = |m: f64| disc_lifetime(SolarMasses::new(m), UnitUniform::HALF).value();
/// assert!((median(1.0) - 2.5 * core::f64::consts::LN_2).abs() < 1e-12);
/// assert!(median(0.3) > median(1.0) && median(0.3) < 1.2 * median(1.0));
/// assert!((median(2.0) / median(1.0) - 0.48).abs() < 0.01);
/// ```
#[must_use]
pub fn disc_lifetime(mass: SolarMasses, rank: UnitUniform) -> Megayears {
    let mean = disc_lifetime_mean(mass).value();
    // −ln(1 − u) through ln_1p keeps the short lifetimes of small ranks accurate.
    let lifetime = -mean * math::ln_1p(-rank.value());
    Megayears::new(lifetime.clamp(DISC_LIFETIME_MIN.value(), DISC_LIFETIME_MAX.value()))
}

// -------------------------------------------------------------------------------------------------
// The protostar (P06.T15.a).

/// How long a star accretes, `t_p`: 0.5 Myr from the onset of collapse.
///
/// Dunham et al. (2014, Protostars and Planets VI, 195, Table 1 and section 2.4) find the Class 0
/// and I phases together last about 0.5 Myr (0.42–0.54 Myr across four surveys), and Class 0
/// alone 0.15–0.16 Myr (their Table 3). With the growth law of [`protostar_mass`] the envelope
/// stops outweighing the star at `t_p` (1 − 1 ÷ √2) = 0.146 Myr, which is that Class 0 lifetime.
pub const PROTOSTAR_DURATION: Megayears = Megayears::new(0.5);

/// [`PROTOSTAR_DURATION`] in years.
pub(crate) const PROTOSTAR_YEARS: f64 = 5e5;

/// The lightest a protostar is taken to be, M☉: a guard that keeps the state finite at the onset
/// of collapse, where the growth law gives no mass at all. No other state reads it.
const MIN_PROTOSTAR_MASS: f64 = 1e-3;

/// A protostar's class by what outweighs what (Lada 1987; André, Ward-Thompson and Barsony 1993).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProtostarClass {
    /// Class 0: the envelope still outweighs the star.
    Class0,
    /// Class I: the star outweighs what is left of its envelope.
    ClassI,
}

/// The mass a star of final mass `final_mass` has accreted by `age` (years since the onset of
/// collapse): nothing before the onset, `m_f` (1 − (1 − t ÷ `t_p`)²) up to `t_p`
/// ([`PROTOSTAR_DURATION`]), `m_f` after it.
///
/// The rate, 2 `m_f` (1 − t ÷ `t_p`) ÷ `t_p`, falls linearly to zero at `t_p`, so the mass is continuous and
/// smooth there. Half the final mass is reached at 0.146 Myr.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::premain::protostar_mass;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let m = |years: f64| protostar_mass(SolarMasses::new(1.0), Years::new(years)).value();
/// assert!((m(146_446.609_406_726_24) - 0.5).abs() < 1e-12);
/// assert_eq!(m(1e6), 1.0);
/// ```
#[must_use]
pub fn protostar_mass(final_mass: SolarMasses, age: Years) -> SolarMasses {
    SolarMasses::new(accreted_mass(final_mass.value(), age.value()))
}

/// [`protostar_mass`] in M☉ for `m_f` M☉ at `age` years: none before the onset.
#[must_use]
fn accreted_mass(m_f: f64, age: f64) -> f64 {
    if age >= PROTOSTAR_YEARS {
        return m_f;
    }
    if age.is_nan() || age <= 0.0 {
        return 0.0;
    }
    let x = 1.0 - age.max(0.0) / PROTOSTAR_YEARS;
    m_f * (1.0 - x * x)
}

/// The accretion rate of [`protostar_mass`], M☉ per year: 2 `m_f` (1 − t ÷ `t_p`) ÷ `t_p`, and zero from
/// `t_p` on.
#[must_use]
fn accretion_rate(m_f: f64, age: f64) -> f64 {
    if age >= PROTOSTAR_YEARS {
        return 0.0;
    }
    2.0 * m_f * (1.0 - age.max(0.0) / PROTOSTAR_YEARS) / PROTOSTAR_YEARS
}

/// The class of a protostar of final mass `final_mass` at `age` (years since the onset of
/// collapse), or `None` once it has stopped accreting at [`PROTOSTAR_DURATION`]: Class 0 while the
/// envelope still to fall in, `m_f` − m, outweighs the star, Class I after (P06.T15.a).
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::premain::{ProtostarClass, protostar_class};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let class = |years: f64| protostar_class(SolarMasses::new(1.0), Years::new(years));
/// assert_eq!(class(1e5), Some(ProtostarClass::Class0));
/// assert_eq!(class(2e5), Some(ProtostarClass::ClassI));
/// assert_eq!(class(1e6), None);
/// ```
#[must_use]
pub fn protostar_class(final_mass: SolarMasses, age: Years) -> Option<ProtostarClass> {
    let (m_f, t) = (final_mass.value(), age.value());
    // A star of negative age is not yet born (P06.T29's `SystemExistence::NotYetBorn`).
    if !(0.0..PROTOSTAR_YEARS).contains(&t) {
        return None;
    }
    let m = accreted_mass(m_f, t);
    Some(if m_f - m > m {
        ProtostarClass::Class0
    } else {
        ProtostarClass::ClassI
    })
}

/// G M☉ (M☉ per year) ÷ R☉, in L☉: the accretion luminosity G m ṁ ÷ R of a solar mass accreting a
/// solar mass a year at a solar radius, 3.14 × 10⁷ L☉.
const ACCRETION_LUMINOSITY: f64 =
    GM_SUN * SOLAR_MASS_KG / SECONDS_PER_JULIAN_YEAR / SOLAR_RADIUS_M / SOLAR_LUMINOSITY_W;

/// log₁₀ of the Sun's effective temperature, K.
const LOG_SOLAR_TEMPERATURE: f64 = 3.761_326_322_421_456_6;

// -------------------------------------------------------------------------------------------------
// The birthline.

/// The birthline: where a star of each final mass stands when it stops accreting at `t_p`, as (mass,
/// M☉; radius, R☉; effective temperature, K), increasing in mass. Between the nodes log R and
/// log `T_eff` are linear in log m; beyond the ends they are held.
///
/// - **0.2–1.4 M☉:** Baraffe et al. (2015, A&A 577, A42; "BHAC15") at 0.5 Myr, the age at which
///   their tracks start and at which [`PROTOSTAR_DURATION`] ends accretion. 0.1 and 0.15 M☉,
///   whose tracks start later, take their 1 Myr temperatures and their 1 Myr radii times 1.150,
///   the ratio of BHAC15's 0.5 and 1 Myr radii at 0.2 M☉.
/// - **2–6 M☉:** Palla and Stahler's (1993, ApJ 418, 414, Table 1, "initial" rows) birthline for an
///   accretion rate of 10⁻⁵ M☉ per year, with its swelling near 4 M☉ where deuterium burns in a
///   shell. Their 1.5 M☉ star (4.97 R☉) is left out: it appears 0.15 Myr after the onset of
///   collapse and has contracted towards BHAC15's radius by `t_p`.
/// - **8 M☉:** the zero-age main sequence of Hurley, Pols and Tout at Z = 0.02, which Palla and
///   Stahler's (1990, ApJ 360, L47) birthline meets "at a mass of about 8 M☉". Above it the star
///   is on the main sequence when accretion ends, and a protostar blends to its main sequence (see
///   [`Protostar`]).
///
/// Palla and Stahler (1999, ApJ 525, 772), the plan's starting point, could not be fetched; their
/// 1990 and 1993 papers give the same birthline. The BHAC15 radii are smaller than Palla and
/// Stahler's at the same mass because they are read at `t_p` rather than where each star first
/// appears.
const BIRTHLINE: [(f64, f64, f64); 21] = [
    (0.1, 1.155, 2_935.0),
    (0.15, 1.527, 3_078.0),
    (0.2, 1.729, 3_220.0),
    (0.3, 2.204, 3_460.0),
    (0.4, 2.341, 3_670.0),
    (0.5, 2.447, 3_848.0),
    (0.6, 2.534, 3_987.0),
    (0.7, 2.670, 4_110.0),
    (0.8, 2.808, 4_221.0),
    (0.9, 2.945, 4_315.0),
    (1.0, 3.081, 4_397.0),
    (1.1, 3.214, 4_471.0),
    (1.2, 3.346, 4_537.0),
    (1.3, 3.475, 4_596.0),
    (1.4, 3.603, 4_647.0),
    (2.0, 4.41, 4_660.0),
    (3.0, 3.98, 4_990.0),
    (3.5, 4.23, 5_140.0),
    (4.0, 7.82, 6_920.0),
    (5.0, 6.40, 11_700.0),
    (8.0, 3.457, 22_343.0),
];

/// log₁₀ R (R☉) and log₁₀ `T_eff` (K) on the birthline ([`BIRTHLINE`]) at `m` M☉.
#[must_use]
fn birthline(mass: f64) -> [f64; 2] {
    let count = BIRTHLINE.len();
    let (first, last) = (BIRTHLINE[0], BIRTHLINE[count - 1]);
    let node =
        |(_, radius, temperature): (f64, f64, f64)| [math::log10(radius), math::log10(temperature)];
    if mass <= first.0 {
        return node(first);
    }
    if mass >= last.0 {
        return node(last);
    }
    let index = BIRTHLINE[1..count - 1]
        .iter()
        .take_while(|&&(node_mass, ..)| mass >= node_mass)
        .count();
    let (below, above) = (BIRTHLINE[index], BIRTHLINE[index + 1]);
    let share = math::log10(mass / below.0) / math::log10(above.0 / below.0);
    let ([r0, t0], [r1, t1]) = (node(below), node(above));
    [r0 + share * (r1 - r0), t0 + share * (t1 - t0)]
}

/// log₁₀ L (L☉) of a photosphere of log₁₀ R (R☉) and log₁₀ `T_eff` (K): Stefan–Boltzmann.
#[must_use]
fn photosphere_log_l(log_r: f64, log_t: f64) -> f64 {
    2.0 * log_r + 4.0 * (log_t - LOG_SOLAR_TEMPERATURE)
}

/// A protostar's closed form: its mass, radius and luminosity from the onset of collapse to `t_p`
/// (P06.T15.a).
///
/// Its photosphere is the birthline's at its current mass, shifted by the share it has accreted,
/// w = m ÷ `m_f`, times the offset in log R and log `T_eff` between its own birthline point and the
/// state its next phase starts in. For a star that contracts after accretion that offset is zero,
/// since the contraction starts on the birthline, and the shift is instead the contraction's own
/// −⅓ log₁₀(t ÷ `t_p`) in log R: at `t_p`, where w's rate vanishes with the accretion's, the
/// photosphere then meets the contraction in value and in slope. For a star that is on the main
/// sequence at `t_p` the offset is the step from the birthline to the main sequence, so the
/// photosphere reaches it exactly at `t_p`, with the slope of the birthline at constant mass, zero,
/// against the main sequence's own slow change.
///
/// The luminosity is the photosphere's plus the accretion's, G m ṁ ÷ R, which falls to zero at
/// `t_p` with the accretion rate; its slope there is not the next phase's, since accretion stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Protostar {
    final_mass: f64,
    /// The offsets in log₁₀ R and log₁₀ `T_eff` from the birthline at the final mass to the next
    /// phase's first state.
    offset: [f64; 2],
    /// Whether the contraction follows, whose Hayashi law the photosphere takes on as it accretes.
    contracts: bool,
}

impl Protostar {
    /// The protostar of final mass `final_mass` (M☉) whose next phase starts at `t_p` with log₁₀ R
    /// and log₁₀ `T_eff` `next`, or on its own birthline with `None`.
    #[must_use]
    pub(crate) fn new(final_mass: f64, next: Option<[f64; 2]>) -> Self {
        let offset = next.map_or([0.0; 2], |[log_r, log_t]| {
            let [r, t] = birthline(final_mass);
            [log_r - r, log_t - t]
        });
        Self {
            final_mass,
            offset,
            contracts: next.is_none(),
        }
    }

    /// The mass at `age` years, M☉: [`protostar_mass`], above a guard of 10⁻³ M☉.
    #[must_use]
    pub(crate) fn mass(&self, age: f64) -> f64 {
        accreted_mass(self.final_mass, age).max(MIN_PROTOSTAR_MASS)
    }

    /// The luminosity and radius at `age` years (no core).
    #[must_use]
    pub(crate) fn at(&self, age: f64) -> PhasePoint {
        let m = self.mass(age);
        // The accreted share from the unguarded mass, so that w times the contraction's
        // logarithm vanishes at the onset.
        let w = (accreted_mass(self.final_mass, age) / self.final_mass).min(1.0);
        let [r, t] = birthline(m);
        // The contraction's R ∝ t^−⅓, taken on with the accreted share: it vanishes at the onset,
        // and at t_p, where w's rate vanishes, it gives the photosphere the contraction's slope.
        let hayashi = if self.contracts && age > 0.0 {
            -math::log10(age / PROTOSTAR_YEARS) / 3.0
        } else {
            0.0
        };
        let (log_r, log_t) = (r + w * (self.offset[0] + hayashi), t + w * self.offset[1]);
        let radius = math::exp10(log_r);
        let accretion = ACCRETION_LUMINOSITY * m * accretion_rate(self.final_mass, age) / radius;
        PhasePoint {
            luminosity: SolarLuminosities::new(
                math::exp10(photosphere_log_l(log_r, log_t)) + accretion,
            ),
            radius: SolarRadii::new(radius),
            core_mass: SolarMasses::ZERO,
        }
    }

    /// log₁₀ R and log₁₀ `T_eff` of the contraction's first state for a star of `final_mass` M☉:
    /// its birthline point.
    #[must_use]
    pub(crate) fn birthline_start(final_mass: f64) -> [f64; 2] {
        birthline(final_mass)
    }
}

// -------------------------------------------------------------------------------------------------
// The contraction (P06.T15.b).

/// Kelvin and Helmholtz's time for a solar mass of a solar radius shining at a solar luminosity,
/// G M☉² ÷ (R☉ L☉), years: 31.4 Myr.
const KELVIN_HELMHOLTZ_YEARS: f64 =
    GM_SUN * SOLAR_MASS_KG / (SOLAR_RADIUS_M * SOLAR_LUMINOSITY_W * SECONDS_PER_JULIAN_YEAR);

/// The arrival time over the Kelvin–Helmholtz time as a cubic in log₁₀ m, coefficients from the
/// cube down: log₁₀ g = c₃ x³ + c₂ x² + c₁ x + c₀, x = log₁₀(m ÷ M☉), up to [`ARRIVAL_FIT_MASS`].
///
/// Fitted by least squares to Baraffe et al.'s (2015) arrival on the zero-age main sequence at
/// 0.1, 0.15, 0.2, 0.3–1.4 M☉ by 0.1 (the age at which log R first comes within 0.01 dex of its
/// minimum: 554, 294, 230, 191, 163, 140, 108, 80, 61, 48, 38.4, 31.0, 25.9, 21.0 and 17.4 Myr),
/// over G m² ÷ (R L) at Hurley, Pols and Tout's zero-age main sequence at Z = 0.02: g runs from
/// 0.23 at 0.1 M☉ to 0.76 at 1 M☉ and 1.32 at 1.4 M☉; the fit's residuals are 0.017 dex rms and
/// 0.037 at most.
const ARRIVAL_FIT: [f64; 4] = [
    0.741_102_005_096_876,
    1.936_110_407_099_685,
    1.702_411_841_206_539_7,
    -0.135_492_064_498_989_68,
];

/// The mass above which the arrival time is the Kelvin–Helmholtz time times the fit's value
/// here, M☉: the top of Baraffe et al.'s (2015) grid used.
const ARRIVAL_FIT_MASS: f64 = 1.4;

/// The arrival on the zero-age main sequence, years since the onset of collapse, of a star of
/// `m` M☉ whose zero-age main sequence has luminosity `l` (L☉) and radius `r` (R☉): g(m) times the
/// Kelvin–Helmholtz time G m² ÷ (R L) at that main sequence (P06.T15.b).
///
/// g is Baraffe et al.'s (2015) arrival over the Kelvin–Helmholtz time up to 1.4 M☉
/// ([`ARRIVAL_FIT`]) and is held at its value there, 1.435, above: the arrival of a heavier star is
/// its Kelvin–Helmholtz time, as the plan asks, scaled to meet the fit. Against MIST (Choi et al.
/// 2016, Dotter 2016, EEP 202, solar) this gives 15.2 against 13.9 Myr at 1.5 M☉, 7.0 against
/// 9.0 at 2, 0.81 against 1.0 at 5 and 0.21 against 0.19 at 10 M☉. The metallicity enters through
/// the zero-age main sequence alone: a 1 M☉ star at Z = 0.002 arrives at 19.6 Myr, against MIST's
/// 21.1 at \[Fe/H\] = −1. The arrival falls below [`PROTOSTAR_DURATION`] at about 6 M☉ (the plan
/// said about 8).
#[must_use]
pub(crate) fn arrival_years(mass: f64, luminosity: f64, radius: f64) -> f64 {
    let log_m = math::log10(mass.min(ARRIVAL_FIT_MASS));
    let [c3, c2, c1, c0] = ARRIVAL_FIT;
    let ratio = math::exp10(((c3 * log_m + c2) * log_m + c1) * log_m + c0);
    ratio * KELVIN_HELMHOLTZ_YEARS * mass * mass / (radius * luminosity)
}

/// The arrival on the zero-age main sequence of a star of initial mass `mass` and `composition`,
/// counted from the onset of its collapse: `t_zams`(m, Z) (plan 06, P06.T15.b, design note 4).
///
/// See [`arrival_years`] for the law. It is where the formulae of Hurley, Pols and Tout start
/// their clock: every phase from the main sequence on is offset by it, and a star's lifetime
/// includes it.
///
/// # Panics
///
/// In debug builds, if `mass` lies outside the tracks' range, 0.1–150 M☉.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::premain::t_zams;
/// use hyperion_sim::units::SolarMasses;
///
/// // The Sun took about 40 Myr to settle on the main sequence; a 0.2 M☉ star a few hundred.
/// let arrival = |m: f64| t_zams(SolarMasses::new(m), &Composition::SOLAR).value();
/// assert!((30.0..50.0).contains(&arrival(1.0)));
/// assert!((150.0..400.0).contains(&arrival(0.2)));
/// ```
#[must_use]
pub fn t_zams(mass: SolarMasses, composition: &Composition) -> Megayears {
    Megayears::new(crate::stellar::sse::arrival_time(mass, composition) * 1e-6)
}

/// The share of the contraction's span in log age that the Hayashi segment takes, at `m` M☉:
/// 0.687 − 0.7 log₁₀ m, held to 0.3–0.9.
///
/// Fitted to where Baraffe et al.'s (2015) tracks start to heat (their temperature 50 K above its
/// minimum) between `t_p` and the arrival: 0.83 of the span in log age at 0.6 M☉, 0.69 at 1 M☉ and
/// 0.57 at 1.4 M☉. Below 0.5 M☉ the stars stay convective and contract at nearly fixed temperature
/// to the end, so the Henyey segment is only the last 10% of the span, where the contraction
/// slows as hydrogen ignites.
#[must_use]
fn hayashi_share(m: f64) -> f64 {
    (0.687 - 0.7 * math::log10(m)).clamp(0.3, 0.9)
}

/// A cubic Hermite blend of y in u from (y0, slope s0) at u = 0 to (y1, s1) at u = 1, the slopes
/// per unit u.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hermite {
    y0: f64,
    s0: f64,
    y1: f64,
    s1: f64,
}

impl Hermite {
    /// The blend at `u`, 0–1.
    #[must_use]
    fn at(&self, u: f64) -> f64 {
        let (u2, u3) = (u * u, u * u * u);
        (2.0 * u3 - 3.0 * u2 + 1.0) * self.y0
            + (u3 - 2.0 * u2 + u) * self.s0
            + (3.0 * u2 - 2.0 * u3) * self.y1
            + (u3 - u2) * self.s1
    }
}

/// The contraction's closed form, from `t_p` to the arrival (P06.T15.b).
///
/// From `t_p` the star contracts down its Hayashi track: at the birthline's temperature, fixed, with
/// R = `R_birthline` (t ÷ `t_p`)^−⅓, the Kelvin–Helmholtz contraction of a fully convective star at
/// constant temperature, L ∝ R². Baraffe et al.'s (2015) 1 M☉ star shrinks by 0.593 from 1 to
/// 5 Myr, against 0.585 by this law, and cools by 50–150 K meanwhile, which the law leaves out.
/// From the Henyey onset ([`hayashi_share`]) to the arrival, log L and log R are cubic Hermite
/// blends in ln t from the Hayashi track's values and slopes to the main sequence's, so that L, R
/// and their first derivatives in age meet the main sequence's at the arrival.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Contraction {
    /// log₁₀ R at `t_p`, R☉.
    log_r_p: f64,
    /// log₁₀ `T_eff` on the Hayashi track, K.
    log_t: f64,
    /// The Henyey onset, years.
    onset: f64,
    /// ln of the onset, and the span in ln t from it to the arrival.
    ln_onset: f64,
    ln_span: f64,
    log_l: Hermite,
    log_r: Hermite,
}

impl Contraction {
    /// The contraction of a star of `m` M☉ from its birthline point `start` (log₁₀ R, log₁₀ `T_eff`)
    /// at `t_p` to its arrival at `arrival` years, where the main sequence starts with log₁₀ L and
    /// log₁₀ R `zams` and their rates in age `rates` (per year).
    #[must_use]
    pub(crate) fn new(
        m: f64,
        start: [f64; 2],
        arrival: f64,
        zams: [f64; 2],
        rates: [f64; 2],
    ) -> Self {
        let [log_r_p, log_t] = start;
        let (ln_p, ln_z) = (math::ln(PROTOSTAR_YEARS), math::ln(arrival));
        let ln_onset = ln_p + hayashi_share(m) * (ln_z - ln_p);
        let onset = math::exp(ln_onset);
        let ln_span = ln_z - ln_onset;
        let hayashi_r = log_r_p - (ln_onset - ln_p) / (3.0 * core::f64::consts::LN_10);
        // d log₁₀ R ÷ d ln t on the Hayashi track, and twice that for L ∝ R².
        let slope_r = -1.0 / (3.0 * core::f64::consts::LN_10);
        Self {
            log_r_p,
            log_t,
            onset,
            ln_onset,
            ln_span,
            log_l: Hermite {
                y0: photosphere_log_l(hayashi_r, log_t),
                s0: 2.0 * slope_r * ln_span,
                y1: zams[0],
                s1: rates[0] * arrival * ln_span,
            },
            log_r: Hermite {
                y0: hayashi_r,
                s0: slope_r * ln_span,
                y1: zams[1],
                s1: rates[1] * arrival * ln_span,
            },
        }
    }

    /// The luminosity and radius at `age` years (no core).
    #[must_use]
    pub(crate) fn at(&self, age: f64) -> PhasePoint {
        let (log_l, log_r) = if age <= self.onset {
            let log_r =
                self.log_r_p - math::log10(age.max(PROTOSTAR_YEARS) / PROTOSTAR_YEARS) / 3.0;
            (photosphere_log_l(log_r, self.log_t), log_r)
        } else {
            let u = ((math::ln(age) - self.ln_onset) / self.ln_span).clamp(0.0, 1.0);
            (self.log_l.at(u), self.log_r.at(u))
        };
        PhasePoint {
            luminosity: SolarLuminosities::new(math::exp10(log_l)),
            radius: SolarRadii::new(math::exp10(log_r)),
            core_mass: SolarMasses::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::LN_2;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// A star of negative age is not yet born (P06.T15's "rejected by the type"): it has no
    /// protostar class and no mass, as `StarModel` has no state then (P06.T29,
    /// `SystemExistence::NotYetBorn`); and the protostar is Class 0 until 0.146 Myr, Class I to
    /// `t_p`.
    #[test]
    fn a_protostar_is_class_0_then_class_i_and_nothing_before_its_onset() {
        let m = SolarMasses::new(1.0);
        for years in [-1.0, -1e6, f64::NAN] {
            assert_eq!(protostar_class(m, Years::new(years)), None);
            assert_same_bits(protostar_mass(m, Years::new(years)).value(), 0.0);
        }
        let half = PROTOSTAR_YEARS * (1.0 - core::f64::consts::FRAC_1_SQRT_2);
        assert_eq!(
            protostar_class(m, Years::new(0.0)),
            Some(ProtostarClass::Class0)
        );
        assert_eq!(
            protostar_class(m, Years::new(half * 0.999)),
            Some(ProtostarClass::Class0)
        );
        assert_eq!(
            protostar_class(m, Years::new(half * 1.001)),
            Some(ProtostarClass::ClassI)
        );
        assert_eq!(protostar_class(m, Years::new(PROTOSTAR_YEARS)), None);
        assert!((protostar_mass(m, Years::new(half)).value() - 0.5).abs() < 1e-12);
        assert!((half - 146_446.609_406_726_2).abs() < 1e-6);
    }

    fn rank(u: f64) -> UnitUniform {
        UnitUniform::new(u).expect("a rank in (0, 1)")
    }

    fn mean(m: f64) -> f64 {
        disc_lifetime_mean(SolarMasses::new(m)).value()
    }

    #[test]
    fn the_mean_is_mamajek_s_two_and_a_half_megayears_at_a_solar_mass() {
        assert_same_bits(mean(1.0), 2.5);
        // Continuous where the two exponents meet.
        assert!((mean(1.0 - 1e-12) - 2.5).abs() < 1e-11);
        assert!((mean(1.0 + 1e-12) - 2.5).abs() < 1e-11);
    }

    #[test]
    fn the_mean_follows_the_measured_mass_dependence() {
        // Mamajek's (2009) 1.2 Myr above 1.3 M☉, reached at 2 M☉.
        assert!((mean(2.0) - 1.2).abs() < 0.005, "{}", mean(2.0));
        // Ribas et al.'s (2015) factor of 2.09–2.2 across 2 M☉.
        let ratio = mean(1.0) / mean(2.0);
        assert!((2.05..2.25).contains(&ratio), "{ratio}");
        // Luhman et al.'s (2005) brown dwarfs outlast their M stars by 1.15–1.28 across a factor
        // of six in mass.
        let dwarfs = mean(0.05) / mean(0.3);
        assert!((1.15..1.28).contains(&dwarfs), "{dwarfs}");
        // Mamajek's brown dwarfs, about 3 Myr (2.4–5.9 over four clusters).
        assert!((2.4..5.9).contains(&mean(0.05)), "{}", mean(0.05));
        // The mean never rises with mass.
        let mut previous = f64::INFINITY;
        for k in 1..2_000_u32 {
            let m = 0.01 * f64::from(k);
            assert!(mean(m) <= previous, "{m} M☉");
            previous = mean(m);
        }
    }

    #[test]
    fn the_median_rank_gives_the_half_life() {
        for m in [0.3, 1.0, 2.0] {
            let mean = disc_lifetime_mean(SolarMasses::new(m)).value();
            let median = disc_lifetime(SolarMasses::new(m), UnitUniform::HALF).value();
            assert!(
                (median - mean * LN_2).abs() < 1e-12 * mean,
                "{m} M☉: {median} against {}",
                mean * LN_2
            );
        }
        // Mamajek's (2009) half-life of 1.7 Myr, at a solar mass.
        let sun = disc_lifetime(SolarMasses::new(1.0), UnitUniform::HALF).value();
        assert!((sun - 1.733).abs() < 1e-3, "{sun}");
    }

    #[test]
    fn a_rank_maps_to_its_exponential_quantile() {
        for u in [0.2, 0.5, 0.9, 0.99] {
            let t = disc_lifetime(SolarMasses::new(1.0), rank(u)).value();
            // The survival function of the lifetime at t is the rank's complement.
            let survival = math::exp(-t / 2.5);
            assert!((survival - (1.0 - u)).abs() < 1e-12, "rank {u}: {t} Myr");
        }
    }

    #[test]
    fn lifetimes_are_held_to_the_clamps() {
        let sun = SolarMasses::new(1.0);
        assert_same_bits(disc_lifetime(sun, rank(1e-12)).value(), 0.3);
        assert_same_bits(disc_lifetime(sun, rank(1.0 - 1e-12)).value(), 15.0);
        // The rank at which a solar-mass disc reaches each clamp: 1 − e^(−0.12) and 1 − e^(−6).
        let low = 1.0 - math::exp(-0.3 / 2.5);
        assert!(disc_lifetime(sun, rank(low - 1e-9)).value() <= 0.3);
        assert!(disc_lifetime(sun, rank(low + 1e-9)).value() > 0.3);
        let high = 1.0 - math::exp(-15.0 / 2.5);
        assert!(disc_lifetime(sun, rank(high - 1e-9)).value() < 15.0);
        assert!(disc_lifetime(sun, rank(high + 1e-9)).value() >= 15.0);
        // A massive star's mean is below the floor at every rank below the floor's quantile.
        let massive = disc_lifetime(SolarMasses::new(100.0), UnitUniform::HALF);
        assert_same_bits(massive.value(), DISC_LIFETIME_MIN.value());
        for m in [0.08, 0.1, 0.5, 1.0, 3.0, 20.0, 150.0] {
            for u in [1e-15, 0.001, 0.3, 0.7, 0.999, 1.0 - 1e-15] {
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!((0.3..=15.0).contains(&t), "{m} M☉, rank {u}: {t} Myr");
            }
        }
    }

    #[test]
    fn the_lifetime_never_falls_as_the_rank_rises() {
        for m in [0.1, 1.0, 8.0] {
            let mut previous = 0.0;
            for k in 1..10_000_u32 {
                let u = f64::from(k) / 10_000.0;
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!(t >= previous, "{m} M☉: {t} at rank {u} after {previous}");
                previous = t;
            }
        }
    }

    #[test]
    fn a_heavier_star_never_keeps_its_disc_longer_at_the_same_rank() {
        for u in [0.05, 0.5, 0.95] {
            let mut previous = f64::INFINITY;
            for m in [0.08, 0.2, 0.5, 1.0, 2.0, 5.0, 20.0] {
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!(t <= previous, "rank {u}: {t} Myr at {m} M☉");
                previous = t;
            }
        }
    }
}
