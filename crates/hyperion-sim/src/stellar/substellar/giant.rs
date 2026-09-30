//! The cooling of giant planets from 0.3 to 13 Jupiter masses (plan 13, P13.T5.c): luminosity,
//! radius and effective temperature from mass, age and composition, continuous at 13 Jupiter
//! masses with plan 06's [`cooling`], for plan 14's giant planets and plan 13's rogue planets.
//!
//! # The model
//!
//! 1. **The table.** [`tables::giant_cooling`](crate::tables::giant_cooling) holds log₁₀ L and
//!    log₁₀ R at 12 masses and 12 ages, fitted by `hyperion-fit run giant_cooling` to the Sonora
//!    Bobcat evolution models (Marley et al. 2021, ApJ 920, 85): isolated, cloudless,
//!    hydrogen–helium objects of solar composition on a hot start. Burrows et al.'s (2001) power
//!    laws, which the brainstorm cites, were tried first and fail Jupiter in every quantity (the
//!    fitting task's documentation gives the figures and the reasons for the grid). The table is
//!    interpolated bilinearly in log₁₀ mass and log₁₀ age, so the state is continuous in both and
//!    has no step in age. Below 1 Myr ([`MIN_AGE`](super::MIN_AGE), where [`cooling`] holds too)
//!    the state is that of 1 Myr. Past the table's last age, 15 Gyr, the state goes on as
//!    Burrows et al.'s late-time laws have it, L ∝ t^−1.3 and R ∝ t^−0.056 (their equation 1,
//!    and equation 5 at the gravity of equation 3 and the temperature of equation 2), as
//!    [`cooling`] does there. The table's own last intervals fall as t^−1.02 to t^−1.29. So the
//!    luminosity keeps falling and never reaches zero, the masses keep their order, and the table
//!    keeps the difference from [`cooling`] it has at 15 Gyr.
//! 2. **Metallicity.** Bobcat's table is solar. The composition moves its luminosity by as much as
//!    it moves [`cooling`]'s at 13 Jupiter masses and the same age: once the object cools
//!    degenerately that is Burrows et al.'s opacity term, κ̂^0.35 (their equation 1, with κ̂ read
//!    from the metal fraction as [`cooling`] reads it), and while it still contracts it is less
//!    (at Z = 10⁻⁴, −0.12 dex at 1 Myr and −0.36 dex at 3 Myr, against the term's −0.57). The
//!    radius is the table's at every metallicity. Bobcat's own grids at \[M/H\] = ±0.5 move log L
//!    by 0–0.22 dex per dex at late times and by 0.02–0.09 at 10 Myr, against the term's 0.34 at
//!    solar metallicity, and make metal-rich objects 0.5–2.5% smaller per dex at late times
//!    (Marley et al. 2021), which the table does not follow. The luminosity takes [`cooling`]'s
//!    dependence so that the two sources differ at the join by the same amount at every
//!    metallicity: with Bobcat's weaker dependence a metal-poor object's luminosity would fall
//!    with mass across the join.
//! 3. **The join.** From 10 to 13 Jupiter masses the logarithms of luminosity and radius are
//!    blended, linearly in log mass, from the table's to those of [`cooling`]'s closed form,
//!    which is evaluated 5% below its own lower end of 0.01 M☉ (10.5 Jupiter masses) for the
//!    purpose. At 13 Jupiter masses and above the state is [`cooling`]'s, bit for bit in
//!    luminosity and radius. At 13 Jupiter masses the two differ by 0.16 dex at 10 Gyr and 0.20
//!    dex at 15 Gyr, Bobcat the brighter, and by about 0.1 dex either way below 1 Gyr. A smooth
//!    step in the weight would let the luminosity fall with mass at old ages; with the linear
//!    weight d ln L ÷ d ln m stays above 0.09 everywhere. Effective temperature follows from
//!    Stefan–Boltzmann, L = 4πR²σT⁴.
//!
//! # Against Jupiter and Saturn
//!
//! At 4.6 Gyr and solar composition a Jupiter mass has 104.6 K and 1.000 Jupiter radii (71,492 km
//! at 1 bar, the IAU nominal equatorial radius; 1.022 times the mean radius); Jupiter's internal
//! heat flux, 5.4–7.5 W m⁻² (Hanel et al. 1981; Li et al. 2018), is an effective temperature of
//! 99–107 K. At the fit's lower end of 0.3 Jupiter masses, just above Saturn's 0.2994, it has
//! 63.6 K and 65,270 km. Saturn's internal heat flux, 2.01 W m⁻² (Hanel et al. 1983) or 2.84 W m⁻²
//! (Wang et al. 2024), is 77–84 K, and its mean radius 58,232 km, so the model is 17–24% too cold,
//! because it has no helium rain, and 12% too large, because it has no core. Coreless models
//! agree: Fortney et al. (2007, ApJ 659, 1661) give 0.931 Jupiter radii, 66,560 km, at 0.3 Jupiter
//! masses and 4.5 Gyr.
//!
//! # What is not modelled
//!
//! - **Irradiation.** The state is the object's own cooling, as if isolated. Plan 14's P14.T12.a
//!   adds the host's flux: T⁴ = `T_eq`⁴ + L ÷ (4πR²σ).
//! - **Heavy elements.** Bobcat's objects have no core and a solar envelope, so giants below about
//!   1 Jupiter mass come out large for their mass (Saturn by 12%). Plan 14's P14.T11.d blends
//!   into Chen and Kipping's (2017) empirical radii over 0.3–0.414 Jupiter masses.
//! - **Deuterium burning**, as in [`cooling`]: the fit leaves out Bobcat's tracks above 11.5
//!   Jupiter masses, which burn it (plan 13's P13.T5.a may add it to both).
//! - **Clouds, and the formation.** The atmospheres are cloudless, and the early state depends on
//!   the object's initial entropy, which a hot start fixes high: for about 20 Myr at 1 Jupiter
//!   mass and up to 1 Gyr at 10 (Marley et al. 2007, ApJ 655, 541).
//! - **The coldest atmospheres.** Bobcat's atmosphere grid, its evolution's boundary condition,
//!   ends at 200 K and log g = 3 (Marley et al. 2021, §2.7), and its tables continue below that
//!   to 100 K. A Jupiter older than about 0.7 Gyr is in that continuation.

use core::f64::consts::LN_10;
use core::fmt;
use std::error::Error;

#[cfg(doc)]
use super::cooling;
use super::{
    BURROWS_G_POWERS, BURROWS_L_POWERS, BURROWS_R_POWERS, BURROWS_T_POWERS, LnState, MIN_AGE,
    ln_state,
};
use crate::math;
use crate::stellar::Composition;
use crate::stellar::composition::Z_SOLAR;
use crate::tables::giant_cooling::{LOG_AGE_NODES, LOG_LUMINOSITY, LOG_MASS_NODES, LOG_RADIUS};
use crate::units::{
    JupiterMasses, Kelvin, MetalFraction, SolarLuminosities, SolarMasses, SolarRadii, Years,
};

/// The lowest mass the fit covers, 0.3 Jupiter masses (about Saturn's), where plan 14's giant
/// planets begin.
pub const GIANT_MIN_MASS: JupiterMasses = JupiterMasses::new(0.3);

/// The highest mass the fit covers, 13 Jupiter masses, where plan 13's brown dwarfs begin and the
/// state is plan 06's [`cooling`]'s.
pub const GIANT_MAX_MASS: JupiterMasses = JupiterMasses::new(13.0);

/// Where the blend towards [`cooling`] begins, Jupiter masses.
const BLEND_START: JupiterMasses = JupiterMasses::new(10.0);

/// The power of age in Burrows et al.'s late-time radius, −0.056: equation 5's R ∝ g^−0.18
/// T^0.11 at equation 3's g ∝ T^(−0.23 ÷ 0.64) and equation 2's T ∝ t^−0.32, from the same
/// constants as [`cooling`]'s degenerate branch.
const RADIUS_AGE_POWER: f64 = (BURROWS_R_POWERS[1]
    - BURROWS_R_POWERS[0] * BURROWS_G_POWERS[1] / BURROWS_G_POWERS[0])
    * BURROWS_T_POWERS[0];

/// The state of a giant planet's interior: its own luminosity, radius and effective temperature,
/// as if it were isolated.
///
/// Plan 13's type for plan 14's giant planets (ruling 34 of 2026-09-22): a host star or brown dwarf
/// is a [`StarState`](crate::stellar::StarState), and there is no conversion between the two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolingState {
    luminosity: SolarLuminosities,
    radius: SolarRadii,
    effective_temperature: Kelvin,
}

impl CoolingState {
    /// The luminosity of the planet's own cooling and contraction, in nominal solar luminosities
    /// (IAU 2015). It excludes any sunlight the planet absorbs and re-emits.
    #[must_use]
    pub const fn luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// The radius, in nominal solar radii (IAU 2015): the radius of the photosphere, which for
    /// Jupiter is its radius at 1 bar.
    #[must_use]
    pub const fn radius(&self) -> SolarRadii {
        self.radius
    }

    /// The effective temperature of the planet's own flux, K: T = (L ÷ 4πR²σ)^¼. An irradiated
    /// planet's observed effective temperature is higher (plan 14, P14.T12.a).
    #[must_use]
    pub const fn effective_temperature(&self) -> Kelvin {
        self.effective_temperature
    }
}

/// Why [`giant_cooling`] has no state for its arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluateGiantCoolingError {
    /// The mass is outside [`GIANT_MIN_MASS`]–[`GIANT_MAX_MASS`], or not a number. Above 13
    /// Jupiter masses plan 06's [`cooling`] gives the state; below 0.3 plan 14 has no giant.
    MassOutsideFit(JupiterMasses),
    /// The age is negative or not finite: an object has no state before it forms.
    AgeOutsideLife(Years),
}

impl fmt::Display for EvaluateGiantCoolingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutsideFit(m) => write!(
                f,
                "a mass of {} Jupiter masses is outside the giant-planet cooling fit's {} to {} \
                 Jupiter masses",
                m.value(),
                GIANT_MIN_MASS.value(),
                GIANT_MAX_MASS.value()
            ),
            Self::AgeOutsideLife(t) => {
                write!(f, "an age of {} years is negative or not finite", t.value())
            }
        }
    }
}

impl Error for EvaluateGiantCoolingError {}

/// The interior of a giant planet of `mass` at `age` with composition `comp`: the planet's own
/// luminosity, radius and effective temperature, as if it were isolated.
///
/// `mass` is in Jupiter masses from [`GIANT_MIN_MASS`] to [`GIANT_MAX_MASS`] inclusive, and `age`
/// in Julian years since formation, below 1 Myr held at its state then. The fit reads the metal
/// fraction plan 06's fits read, [`Composition::z_fit`]. At 13 Jupiter masses the state is
/// [`cooling`]'s, and the two are continuous there in luminosity, radius and temperature at every
/// age and composition. Luminosity, radius and temperature never rise with age, and luminosity
/// never falls with mass. The model, its sources and its limits are in the
/// [module documentation](self).
///
/// # Errors
///
/// - [`EvaluateGiantCoolingError::MassOutsideFit`] if `mass` is below 0.3 or above 13 Jupiter
///   masses, or NaN.
/// - [`EvaluateGiantCoolingError::AgeOutsideLife`] if `age` is negative or not finite.
///
/// # Examples
///
/// A Jupiter at the Solar System's age glows at about 105 K of its own and is Jupiter's size;
/// a young one of ten Jupiter masses is a thousand times brighter:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::substellar::{EvaluateGiantCoolingError, giant_cooling};
/// use hyperion_sim::units::{JupiterMasses, Metres, Years};
///
/// let sun = Composition::SOLAR;
/// let jupiter = giant_cooling(JupiterMasses::new(1.0), Years::new(4.6e9), &sun)?;
/// assert!((100.0..110.0).contains(&jupiter.effective_temperature().value()));
/// let radius_km = Metres::from(jupiter.radius()).value() / 1e3;
/// assert!((radius_km / 71_492.0 - 1.0).abs() < 0.01);
///
/// let young = giant_cooling(JupiterMasses::new(10.0), Years::new(3e7), &sun)?;
/// assert!(young.luminosity() / jupiter.luminosity() > 1e3);
/// # Ok::<(), EvaluateGiantCoolingError>(())
/// ```
pub fn giant_cooling(
    mass: JupiterMasses,
    age: Years,
    comp: &Composition,
) -> Result<CoolingState, EvaluateGiantCoolingError> {
    let m = mass.value();
    if !(GIANT_MIN_MASS.value()..=GIANT_MAX_MASS.value()).contains(&m) {
        return Err(EvaluateGiantCoolingError::MassOutsideFit(mass));
    }
    let t = age.value();
    if !(t.is_finite() && t >= 0.0) {
        return Err(EvaluateGiantCoolingError::AgeOutsideLife(age));
    }
    let ln = giant_ln_state(m, t.max(MIN_AGE.value()), comp.z_fit());
    Ok(CoolingState {
        luminosity: SolarLuminosities::new(math::exp(ln.luminosity)),
        radius: SolarRadii::new(math::exp(ln.radius)),
        effective_temperature: Kelvin::new(math::exp(ln.temperature())),
    })
}

/// The state of `m` Jupiter masses at `t` years (at least 1 Myr) and metal fraction `z`: the
/// table below the blend, [`cooling`]'s closed form from 13 Jupiter masses, and the two blended
/// between (module documentation).
#[must_use]
fn giant_ln_state(m: f64, t: f64, z: MetalFraction) -> LnState {
    if m >= GIANT_MAX_MASS.value() {
        return cooling_ln_state(m, t, z);
    }
    let log_mass = math::log10(m);
    let table = table_ln_state(log_mass, t, z);
    if m <= BLEND_START.value() {
        return table;
    }
    let start = math::log10(BLEND_START.value());
    let weight = (log_mass - start) / (math::log10(GIANT_MAX_MASS.value()) - start);
    let cool = cooling_ln_state(m, t, z);
    LnState {
        luminosity: table.luminosity + weight * (cool.luminosity - table.luminosity),
        radius: table.radius + weight * (cool.radius - table.radius),
    }
}

/// [`cooling`]'s closed form at `m` Jupiter masses.
#[must_use]
fn cooling_ln_state(m: f64, t: f64, z: MetalFraction) -> LnState {
    ln_state(SolarMasses::from(JupiterMasses::new(m)).value(), t, z)
}

/// The table's state at log₁₀ mass `log_mass` (Jupiter masses) and `t` years (at least 1 Myr),
/// carried past its last age on Burrows et al.'s late-time laws, with its luminosity moved from
/// solar metallicity to `z` as [`metallicity_shift`] says.
#[must_use]
fn table_ln_state(log_mass: f64, t: f64, z: MetalFraction) -> LnState {
    let log_age = math::log10(t);
    let last_age = LOG_AGE_NODES[LOG_AGE_NODES.len() - 1];
    let mass = locate(&LOG_MASS_NODES, log_mass);
    let age = locate(&LOG_AGE_NODES, log_age.min(last_age));
    let ln_beyond = LN_10 * (log_age - last_age).max(0.0);
    LnState {
        luminosity: LN_10 * bilinear(&LOG_LUMINOSITY, mass, age)
            + BURROWS_L_POWERS[0] * ln_beyond
            + metallicity_shift(t, z),
        radius: LN_10 * bilinear(&LOG_RADIUS, mass, age) + RADIUS_AGE_POWER * ln_beyond,
    }
}

/// How much metal fraction `z` moves ln L from solar metallicity at `t` years: as much as it
/// moves [`cooling`]'s at 13 Jupiter masses and the same age.
#[must_use]
fn metallicity_shift(t: f64, z: MetalFraction) -> f64 {
    let top = GIANT_MAX_MASS.value();
    cooling_ln_state(top, t, z).luminosity - cooling_ln_state(top, t, Z_SOLAR).luminosity
}

/// The interval of `nodes` holding `value` and the fraction of the way along it; `value` is inside
/// the nodes.
#[must_use]
fn locate<const N: usize>(nodes: &[f64; N], value: f64) -> (usize, f64) {
    let i = nodes[1..N - 1]
        .iter()
        .take_while(|&&node| value >= node)
        .count();
    (i, (value - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// Bilinear interpolation of `values`, indexed by mass node then age node, in the interval and
/// fraction `mass` along the mass nodes and `age` along the age nodes: first along mass at the
/// interval's two ages, then along age.
#[must_use]
fn bilinear<const M: usize, const A: usize>(
    values: &[[f64; A]; M],
    (i, f): (usize, f64),
    (j, g): (usize, f64),
) -> f64 {
    let younger = values[i][j] + f * (values[i + 1][j] - values[i][j]);
    let older = values[i][j + 1] + f * (values[i + 1][j + 1] - values[i][j + 1]);
    younger + g * (older - younger)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::super::{BURROWS_T_KK, COOLING_OPACITY_FLOOR, burrows, cooling, opacity, power_law};
    use super::*;
    use crate::units::{Dex, HeliumExcess, Metres};

    /// A composition with metal fraction `z`.
    fn with_z(z: f64) -> Composition {
        Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
    }

    fn state(m: f64, gyr: f64, comp: &Composition) -> CoolingState {
        giant_cooling(JupiterMasses::new(m), Years::new(gyr * 1e9), comp).expect("inside the fit")
    }

    /// Jupiter's radius at 1 bar, km: the IAU 2015 nominal equatorial radius and the volumetric
    /// mean (Archinal et al. 2018).
    const JUPITER_EQUATORIAL_KM: f64 = 71_492.0;
    const JUPITER_MEAN_KM: f64 = 69_911.0;

    fn km(radius: SolarRadii) -> f64 {
        Metres::from(radius).value() / 1e3
    }

    /// Metal fractions across the formulae's range, 10⁻⁴ to 0.03.
    const METAL_FRACTIONS: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    #[test]
    fn a_jupiter_mass_at_4_6_gyr_has_jupiters_temperature_and_radius() {
        let jupiter = state(1.0, 4.6, &Composition::SOLAR);
        let teff = jupiter.effective_temperature().value();
        let r = km(jupiter.radius());
        eprintln!(
            "Jupiter: {teff:.1} K, {r:.0} km, log L {:.3}",
            math::log10(jupiter.luminosity().value())
        );
        assert!((100.0..=160.0).contains(&teff), "{teff} K");
        assert!((r / JUPITER_EQUATORIAL_KM - 1.0).abs() < 0.1, "{r} km");
        assert!((r / JUPITER_MEAN_KM - 1.0).abs() < 0.1, "{r} km");
        // Saturn, 0.2994 Jupiter masses, at the fit's lower end, for the record: a coreless model
        // is too cold and too large (module documentation).
        let saturn = state(GIANT_MIN_MASS.value(), 4.6, &Composition::SOLAR);
        eprintln!(
            "Saturn: {:.1} K, {:.0} km",
            saturn.effective_temperature().value(),
            km(saturn.radius())
        );
        assert!((55.0..80.0).contains(&saturn.effective_temperature().value()));
        assert!((58_232.0..70_000.0).contains(&km(saturn.radius())));
    }

    /// P13.T5.b's first step: Burrows et al.'s (2001) power laws, as plan 06 evaluates them, miss
    /// Jupiter, which is why the fit is a table.
    #[test]
    fn burrows_et_al_s_power_laws_miss_jupiter() {
        let m = SolarMasses::from(JupiterMasses::new(1.0)).value();
        let t = 4.6e9;
        let laws = burrows(m, t, Z_SOLAR);
        let logs = [math::ln(t / 1.0e9), math::ln(m / 0.05), 0.0];
        let teff = 1e3 * math::exp(power_law(BURROWS_T_KK, BURROWS_T_POWERS, logs));
        let r = km(SolarRadii::new(math::exp(laws.radius)));
        eprintln!("the power laws: {teff:.1} K, {r:.0} km");
        assert!(!(100.0..=160.0).contains(&teff), "{teff} K");
        assert!((r / JUPITER_EQUATORIAL_KM - 1.0).abs() > 0.1, "{r} km");
        // Equation 1 is 13–18% of Jupiter's internal luminosity, and a fifth of the table's.
        let table = state(1.0, 4.6, &Composition::SOLAR).luminosity().value();
        assert!(math::exp(laws.luminosity) < 0.2 * table);
    }

    /// 1 Myr to 100 Gyr.
    fn ages() -> impl Iterator<Item = f64> {
        (0..=350).map(|i| 1e6 * math::exp10(f64::from(i) * 5.0 / 350.0))
    }

    fn masses(n: u32) -> impl Iterator<Item = f64> {
        let span = math::log10(GIANT_MAX_MASS.value() / GIANT_MIN_MASS.value());
        (0..=n).map(move |i| {
            let m = GIANT_MIN_MASS.value() * math::exp10(f64::from(i) * span / f64::from(n));
            m.min(GIANT_MAX_MASS.value())
        })
    }

    #[test]
    fn luminosity_radius_and_temperature_fall_with_age_and_luminosity_rises_with_mass() {
        for z in METAL_FRACTIONS {
            let comp = with_z(z);
            for m in masses(120) {
                let mut previous: Option<CoolingState> = None;
                for t in ages() {
                    let s = giant_cooling(JupiterMasses::new(m), Years::new(t), &comp).unwrap();
                    if let Some(p) = previous {
                        assert!(
                            s.luminosity().value() <= p.luminosity().value() * (1.0 + 1e-12),
                            "L rises: {m} MJ at {t} yr, Z = {z}"
                        );
                        assert!(s.radius().value() <= p.radius().value() * (1.0 + 1e-12));
                        assert!(
                            s.effective_temperature().value()
                                <= p.effective_temperature().value() * (1.0 + 1e-12)
                        );
                    }
                    previous = Some(s);
                }
            }
            for t in ages().step_by(4) {
                let mut previous = 0.0;
                for m in masses(1_200) {
                    let l = giant_cooling(JupiterMasses::new(m), Years::new(t), &comp)
                        .unwrap()
                        .luminosity()
                        .value();
                    assert!(
                        l >= previous * (1.0 - 1e-12),
                        "L falls with mass: {m} MJ at {t} yr, Z = {z}"
                    );
                    previous = l;
                }
            }
        }
    }

    /// Continuity in age: in log–log the luminosity changes by at most twice and the radius by at
    /// most a third of the change of age (the table's steepest intervals, with a metal-poor
    /// object's shift as its contraction ends, and [`cooling`]'s laws), so there is no step
    /// anywhere, across the table's nodes included; over the clock window of ±1,000 years (plan
    /// 01's H) the state barely moves; and below 1 Myr it is held.
    #[test]
    fn it_is_continuous_in_age_across_the_clock_window() {
        let h = 1.0e3;
        for z in [1e-4, 0.02] {
            let comp = with_z(z);
            for m in masses(60) {
                let mass = JupiterMasses::new(m);
                let mut previous: Option<(f64, CoolingState)> = None;
                for i in 0..=2_000 {
                    let t = 1e6 * math::exp10(f64::from(i) * 4.4 / 2_000.0);
                    let s = giant_cooling(mass, Years::new(t), &comp).unwrap();
                    if let Some((tp, p)) = previous {
                        let d_ln_t = math::ln(t / tp);
                        let d_ln_l = math::ln(s.luminosity() / p.luminosity()).abs();
                        let d_ln_r = math::ln(s.radius() / p.radius()).abs();
                        assert!(d_ln_l <= 2.0 * d_ln_t, "{m} MJ at {t} yr: L jumps");
                        assert!(d_ln_r <= d_ln_t / 3.0, "{m} MJ at {t} yr: R jumps");
                    }
                    previous = Some((t, s));
                }
                for t in [1.001e6, 1e7, 4.6e9, 1.38e10] {
                    let now = giant_cooling(mass, Years::new(t), &comp).unwrap();
                    for dt in [-h, h] {
                        let then = giant_cooling(mass, Years::new(t + dt), &comp).unwrap();
                        assert!((then.luminosity() / now.luminosity() - 1.0).abs() < 2e-3);
                        assert!((then.radius() / now.radius() - 1.0).abs() < 1e-3);
                    }
                }
                let young = giant_cooling(mass, Years::new(2.0e5), &comp).unwrap();
                let at_floor = giant_cooling(mass, MIN_AGE, &comp).unwrap();
                assert_eq!(young, at_floor);
            }
        }
    }

    #[test]
    fn it_joins_plan_06s_cooling_at_13_jupiter_masses() {
        for z in METAL_FRACTIONS {
            let comp = with_z(z);
            for gyr in [0.1, 1.0, 10.0] {
                let age = Years::new(gyr * 1e9);
                // At 0.0124 M☉, the plan's figure for 13 Jupiter masses, to 5%.
                let brown = SolarMasses::new(0.0124);
                let star = cooling(brown, age, &comp).unwrap();
                let giant = giant_cooling(JupiterMasses::from(brown), age, &comp).unwrap();
                let d_l = giant.luminosity() / star.luminosity() - 1.0;
                let d_r = giant.radius() / star.radius() - 1.0;
                let d_t = giant.effective_temperature() / star.effective_temperature() - 1.0;
                eprintln!("Z = {z} at {gyr} Gyr: L {d_l:+.2e}, R {d_r:+.2e}, T {d_t:+.2e}");
                assert!(d_l.abs() < 0.05, "Z = {z} at {gyr} Gyr: L off by {d_l}");
                assert!(d_r.abs() < 0.05, "Z = {z} at {gyr} Gyr: R off by {d_r}");
                assert!(d_t.abs() < 0.05, "Z = {z} at {gyr} Gyr: T off by {d_t}");
                // At 13 Jupiter masses exactly, the same luminosity and radius.
                let top = cooling(SolarMasses::from(GIANT_MAX_MASS), age, &comp).unwrap();
                let at_top = giant_cooling(GIANT_MAX_MASS, age, &comp).unwrap();
                assert_same_bits(at_top.luminosity().value(), top.luminosity().value());
                assert_same_bits(at_top.radius().value(), top.radius().value());
                let d_t = at_top.effective_temperature() / top.effective_temperature() - 1.0;
                assert!(d_t.abs() < 1e-12, "{d_t}");
                // And the approach from below is continuous.
                let below = giant_cooling(JupiterMasses::new(13.0 - 1e-9), age, &comp).unwrap();
                assert!((below.luminosity() / top.luminosity() - 1.0).abs() < 1e-8);
                assert!((below.radius() / top.radius() - 1.0).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn it_is_continuous_in_mass() {
        let comp = Composition::SOLAR;
        let mut edges: Vec<f64> = LOG_MASS_NODES.iter().map(|&x| math::exp10(x)).collect();
        edges.push(BLEND_START.value());
        for gyr in [0.001, 0.03, 0.3, 3.0, 13.8] {
            for &m in &edges {
                let lo = (m * (1.0 - 1e-9)).max(GIANT_MIN_MASS.value());
                let hi = (m * (1.0 + 1e-9)).min(GIANT_MAX_MASS.value());
                let (a, b) = (state(lo, gyr, &comp), state(hi, gyr, &comp));
                assert!(
                    (b.luminosity() / a.luminosity() - 1.0).abs() < 1e-7,
                    "{m} MJ"
                );
                assert!((b.radius() / a.radius() - 1.0).abs() < 1e-7, "{m} MJ");
            }
        }
    }

    /// Below the blend the composition moves the luminosity as it moves [`cooling`]'s at 13
    /// Jupiter masses, which is Burrows et al.'s opacity term once the object is degenerate, and
    /// leaves the radius alone.
    #[test]
    fn metallicity_moves_the_luminosity_as_it_moves_coolings() {
        let sun = Composition::SOLAR;
        let top = SolarMasses::from(GIANT_MAX_MASS);
        for z in [1e-4, 2e-3, 0.03] {
            let comp = with_z(z);
            let kappa = opacity(comp.z_fit(), COOLING_OPACITY_FLOOR);
            for (m, gyr) in [
                (0.3, 0.001),
                (0.3, 0.01),
                (1.0, 4.6),
                (5.0, 13.8),
                (9.9, 0.1),
            ] {
                let age = Years::new(gyr * 1e9);
                let (poor, solar) = (state(m, gyr, &comp), state(m, gyr, &sun));
                let l = poor.luminosity() / solar.luminosity();
                let brown = cooling(top, age, &comp).unwrap().luminosity()
                    / cooling(top, age, &sun).unwrap().luminosity();
                assert!(
                    (l / brown - 1.0).abs() < 1e-12,
                    "{m} MJ at {gyr} Gyr, Z = {z}"
                );
                assert_same_bits(poor.radius().value(), solar.radius().value());
                if gyr >= 1.0 {
                    let law = math::powf(kappa, 0.35);
                    assert!(
                        (l / law - 1.0).abs() < 1e-3,
                        "{m} MJ at {gyr} Gyr: {l} against {law}"
                    );
                }
            }
        }
        // A contracting object barely feels it: at 1 Myr a thousandth of the Sun's metals moves
        // the luminosity by less than a third of the opacity term.
        let poor = with_z(2e-5);
        let l = state(1.0, 0.001, &poor).luminosity() / state(1.0, 0.001, &sun).luminosity();
        let law = math::powf(opacity(poor.z_fit(), COOLING_OPACITY_FLOOR), 0.35);
        assert!(math::ln(l) > math::ln(law) / 3.0, "{l} against {law}");
        // Equation 5 at equations 2 and 3: (0.11 + 0.18 × 0.23 ÷ 0.64) × −0.32.
        assert!((RADIUS_AGE_POWER + 0.055_9).abs() < 1e-12);
    }

    /// Past the table's 15 Gyr a giant below the blend cools on Burrows et al.'s late-time laws in
    /// age, and continuously at 15 Gyr.
    #[test]
    fn past_15_gyr_it_cools_on_burrows_et_al_s_late_time_laws() {
        let sun = Composition::SOLAR;
        for m in [0.3, 1.0, 9.0] {
            let (at, later) = (state(m, 15.0, &sun), state(m, 30.0, &sun));
            let l = later.luminosity() / at.luminosity();
            let r = later.radius() / at.radius();
            assert!(
                (l / math::powf(2.0, -1.3) - 1.0).abs() < 1e-12,
                "{m} MJ: {l}"
            );
            assert!(
                (r / math::powf(2.0, RADIUS_AGE_POWER) - 1.0).abs() < 1e-12,
                "{m} MJ: {r}"
            );
            let before = state(m, 15.0 * (1.0 - 1e-12), &sun);
            assert!((before.luminosity() / at.luminosity() - 1.0).abs() < 1e-10);
        }
    }

    /// No logarithm is taken of the luminosity or the temperature, so however old and cold the
    /// planet, the state is finite and never negative, and it stays positive for any age a galaxy
    /// can hold.
    #[test]
    fn the_old_cold_end_stays_finite() {
        for z in [1e-4, 0.02] {
            let comp = with_z(z);
            for m in [GIANT_MIN_MASS.value(), 1.0, 11.0] {
                for t in [1.38e10, 1e11, 1e13, 1e100, f64::MAX] {
                    let s = giant_cooling(JupiterMasses::new(m), Years::new(t), &comp).unwrap();
                    for value in [
                        s.luminosity().value(),
                        s.radius().value(),
                        s.effective_temperature().value(),
                    ] {
                        assert!(
                            value.is_finite() && value >= 0.0,
                            "{m} MJ at {t} yr: {value}"
                        );
                    }
                    if t <= 1e13 {
                        assert!(s.luminosity().value() > 0.0);
                        assert!(s.effective_temperature().value() > 0.0);
                    }
                }
            }
        }
    }

    #[test]
    fn it_refuses_masses_and_ages_outside_its_range() {
        let sun = Composition::SOLAR;
        for m in [0.299_999, 13.000_001, f64::NAN, -1.0, 20.0] {
            let mass = JupiterMasses::new(m);
            match giant_cooling(mass, Years::new(1e9), &sun) {
                Err(EvaluateGiantCoolingError::MassOutsideFit(got)) => {
                    assert!(got.value().total_cmp(&m).is_eq());
                }
                other => panic!("{m} MJ: {other:?}"),
            }
        }
        for t in [-1.0, f64::INFINITY, f64::NAN] {
            assert!(matches!(
                giant_cooling(JupiterMasses::new(1.0), Years::new(t), &sun),
                Err(EvaluateGiantCoolingError::AgeOutsideLife(_))
            ));
        }
        for m in [GIANT_MIN_MASS, GIANT_MAX_MASS] {
            assert!(giant_cooling(m, Years::ZERO, &sun).is_ok());
        }
        assert_eq!(
            EvaluateGiantCoolingError::MassOutsideFit(JupiterMasses::new(20.0)).to_string(),
            "a mass of 20 Jupiter masses is outside the giant-planet cooling fit's 0.3 to 13 Jupiter \
             masses"
        );
        assert_eq!(
            EvaluateGiantCoolingError::AgeOutsideLife(Years::new(-3.0)).to_string(),
            "an age of -3 years is negative or not finite"
        );
    }

    #[test]
    fn the_same_arguments_give_the_same_bits() {
        let comp = with_z(4e-3);
        for (m, gyr) in [(0.5, 0.002), (2.0, 1.0), (11.5, 0.1)] {
            let (a, b) = (state(m, gyr, &comp), state(m, gyr, &comp));
            assert_same_bits(a.luminosity().value(), b.luminosity().value());
            assert_same_bits(a.radius().value(), b.radius().value());
            assert_same_bits(
                a.effective_temperature().value(),
                b.effective_temperature().value(),
            );
        }
    }

    /// The table spans the fit's range: its mass nodes run from 0.3 to 13 Jupiter masses and its
    /// ages from 1 Myr, both ascending, so no mass inside the range is extrapolated.
    #[test]
    fn the_table_spans_the_range() {
        assert_same_bits(LOG_MASS_NODES[0], math::log10(GIANT_MIN_MASS.value()));
        assert_same_bits(LOG_MASS_NODES[11], math::log10(GIANT_MAX_MASS.value()));
        assert_same_bits(LOG_AGE_NODES[0], math::log10(MIN_AGE.value()));
        assert!(LOG_MASS_NODES.windows(2).all(|w| w[0] < w[1]));
        assert!(LOG_AGE_NODES.windows(2).all(|w| w[0] < w[1]));
        // The blend lies inside the table, and 13 Jupiter masses inside plan 06's fits.
        assert!(math::log10(BLEND_START.value()) > LOG_MASS_NODES[10]);
        assert!(SolarMasses::from(GIANT_MAX_MASS).value() > super::super::MIN_MASS.value());
    }
}
