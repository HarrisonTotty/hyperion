//! The light a body receives from its hosts, and its equilibrium temperature (plan 14, P14.T12.a).
//!
//! A host is read as plain values of luminosity, effective temperature and radius, which the
//! caller takes from its [`StarState`](crate::stellar::StarState) at the body's age plus the
//! clock (ruling 34: a host is always a `StarState`; a giant planet's own cooling is not a host).
//! A black hole's luminosity is zero (ruling 40), and it irradiates nothing: nothing here takes
//! the logarithm of a luminosity.
//!
//! The flux is the time average over a Keplerian orbit of L ÷ 4πr², which is L ÷ (4π a² √(1 −
//! e²)), the average that Kopparapu et al. (2013, ApJ 765, 131, eq. 4) apply to the habitable
//! zone after Williams and Pollard (2002), who show that it is what sets the climate of an
//! eccentric planet with an ocean. The equilibrium temperature of a body that returns what it absorbs, spread over its whole
//! surface, is then `T_eq` = [S (1 − A) ÷ 4σ]^¼; for a single host with L = 4πR★²σT★⁴ this is plan
//! 14's T★ √(R★ ÷ 2a) (1 − A)^¼ (1 − e²)^(−⅛) exactly. In a multiple system fluxes add
//! ([`total_flux`]): a planet about one star sees its companion at the binary's own orbit, which
//! averages 1 ÷ r² the same way, and a planet about a pair sees both at its own distance from their
//! barycentre. A moon takes its planet's orbit.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{
    AstronomicalUnits, EarthFluxes, Kelvin, Metres, SolarLuminosities, SolarRadii, Watts,
    WattsPerSquareMetre,
};

/// A host's light as the derivation reads it: its luminosity, effective temperature and radius
/// at one time (ruling 34).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HostLight {
    luminosity: SolarLuminosities,
    effective_temperature: Kelvin,
    radius: SolarRadii,
}

impl HostLight {
    /// A host of luminosity `luminosity`, effective temperature `effective_temperature` and
    /// radius `radius`, as a [`StarState`](crate::stellar::StarState) gives them. A dark host (a
    /// black hole, or no remnant) has zero luminosity, and then any temperature and radius.
    ///
    /// # Errors
    ///
    /// The [`BuildHostLightError`] naming the first value that is negative or not finite, or
    /// [`BuildHostLightError::LuminousWithoutSurface`] for a luminous host with no radius or no
    /// temperature.
    pub fn new(
        luminosity: SolarLuminosities,
        effective_temperature: Kelvin,
        radius: SolarRadii,
    ) -> Result<Self, BuildHostLightError> {
        let valid = |x: f64| x.is_finite() && x >= 0.0;
        if !valid(luminosity.value()) {
            return Err(BuildHostLightError::LuminosityNotValid);
        }
        if !valid(effective_temperature.value()) {
            return Err(BuildHostLightError::TemperatureNotValid);
        }
        if !valid(radius.value()) {
            return Err(BuildHostLightError::RadiusNotValid);
        }
        if luminosity.value() > 0.0
            && (effective_temperature.value() <= 0.0 || radius.value() <= 0.0)
        {
            return Err(BuildHostLightError::LuminousWithoutSurface);
        }
        Ok(Self {
            luminosity,
            effective_temperature,
            radius,
        })
    }

    /// The host's bolometric luminosity.
    #[must_use]
    pub const fn luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// The host's effective temperature.
    #[must_use]
    pub const fn effective_temperature(&self) -> Kelvin {
        self.effective_temperature
    }

    /// The host's radius.
    #[must_use]
    pub const fn radius(&self) -> SolarRadii {
        self.radius
    }
}

/// A [`HostLight`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildHostLightError {
    /// The luminosity was negative or not finite.
    LuminosityNotValid,
    /// The effective temperature was negative or not finite.
    TemperatureNotValid,
    /// The radius was negative or not finite.
    RadiusNotValid,
    /// The host shines but has no radius or no temperature.
    LuminousWithoutSurface,
}

impl fmt::Display for BuildHostLightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LuminosityNotValid => "a host's luminosity must be finite and not negative",
            Self::TemperatureNotValid => "a host's temperature must be finite and not negative",
            Self::RadiusNotValid => "a host's radius must be finite and not negative",
            Self::LuminousWithoutSurface => "a luminous host must have a radius and a temperature",
        })
    }
}

impl Error for BuildHostLightError {}

/// One host's light on a body: the host, and the semi-major axis and eccentricity of the orbit
/// that separates them (the body's own orbit about the host or about a pair it belongs to, or the
/// binary's orbit for a companion of the body's host).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Illumination {
    host: HostLight,
    semi_major_axis: Metres,
    eccentricity: f64,
}

impl Illumination {
    /// The light of `host` along an orbit of semi-major axis `semi_major_axis` and eccentricity
    /// `eccentricity`, or `None` unless the axis is positive and finite and 0 ≤ e < 1.
    #[must_use]
    pub fn new(host: HostLight, semi_major_axis: Metres, eccentricity: f64) -> Option<Self> {
        let a = semi_major_axis.value();
        (a.is_finite() && a > 0.0 && (0.0..1.0).contains(&eccentricity)).then_some(Self {
            host,
            semi_major_axis,
            eccentricity,
        })
    }

    /// The host.
    #[must_use]
    pub const fn host(&self) -> &HostLight {
        &self.host
    }

    /// The semi-major axis of the orbit between host and body.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// That orbit's eccentricity, 0 ≤ e < 1.
    #[must_use]
    pub const fn eccentricity(&self) -> f64 {
        self.eccentricity
    }

    /// The flux the body receives from this host, averaged over the orbit: L ÷ (4π a² √(1 −
    /// e²)), in units of the flux at 1 au from the Sun.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::derive::irradiation::{HostLight, Illumination};
    /// use hyperion_sim::units::{AstronomicalUnits, Kelvin, Metres, SolarLuminosities, SolarRadii};
    ///
    /// let sun = HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?;
    /// let orbit = Metres::from(AstronomicalUnits::new(0.723_3));
    /// let venus = Illumination::new(sun, orbit, 0.006_8).expect("a bound orbit").flux().value();
    /// assert!((venus - 1.911).abs() < 1e-3);
    /// # Ok::<(), hyperion_sim::planetary::derive::irradiation::BuildHostLightError>(())
    /// ```
    #[must_use]
    pub fn flux(&self) -> EarthFluxes {
        flux(&self.host, self.semi_major_axis, self.eccentricity)
    }
}

/// The flux a body receives from `host` along an orbit of semi-major axis `a` and eccentricity
/// `e`, averaged over the orbit: L ÷ (4π a² √(1 − e²)), in units of the flux at 1 au from the Sun.
/// The public way in is [`Illumination::flux`], whose constructor checks `a` and `e`.
///
/// # Panics
///
/// In debug builds, if `a` is not positive or `e` is outside 0–1.
#[must_use]
pub(crate) fn flux(host: &HostLight, a: Metres, e: f64) -> EarthFluxes {
    luminosity_flux(host.luminosity, a, e)
}

/// The flux from a luminosity `luminosity` along an orbit of semi-major axis `a` and eccentricity
/// `e`, averaged over the orbit: [`flux`] for a bare luminosity, such as a host's zero-age one.
///
/// # Panics
///
/// In debug builds, if `a` is not positive or `e` is outside 0–1.
#[must_use]
pub(crate) fn luminosity_flux(luminosity: SolarLuminosities, a: Metres, e: f64) -> EarthFluxes {
    debug_assert!(a.value() > 0.0, "a semi-major axis is positive, got {a:?}");
    debug_assert!(
        (0.0..1.0).contains(&e),
        "a bound orbit's eccentricity, got {e}"
    );
    let a_au = AstronomicalUnits::from(a).value();
    EarthFluxes::new(luminosity.value() / (a_au * a_au * (1.0 - e * e).sqrt()))
}

/// The flux a body receives from every host in `sources`, added in the order given.
#[must_use]
pub fn total_flux<'a>(sources: impl IntoIterator<Item = &'a Illumination>) -> EarthFluxes {
    sources
        .into_iter()
        .fold(EarthFluxes::ZERO, |sum, source| sum + source.flux())
}

/// A Bond albedo: the fraction of the light arriving at a body that it reflects, 0 ≤ A < 1.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BondAlbedo(f64);

impl BondAlbedo {
    /// The albedo every body has until P14.T13 derives one from its surface and clouds: 0.3
    /// ([`BOND_ALBEDO_BEFORE_ATMOSPHERES`](crate::planetary::params::BOND_ALBEDO_BEFORE_ATMOSPHERES)).
    pub const BEFORE_ATMOSPHERES: Self = crate::planetary::params::BOND_ALBEDO_BEFORE_ATMOSPHERES;

    /// An albedo the caller has already confined to 0 ≤ `value` < 1.
    #[must_use]
    pub(crate) const fn from_fraction(value: f64) -> Self {
        Self(value)
    }

    /// The albedo `value`, or `None` unless 0 ≤ `value` < 1.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (0.0..1.0).contains(&value).then_some(Self(value))
    }

    /// The albedo, 0–1.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The equilibrium temperature of a body receiving `flux` with Bond albedo `albedo`, which
/// re-radiates what it absorbs from its whole surface: `T_eq` = [S (1 − A) ÷ 4σ]^¼.
///
/// Zero flux gives zero, as about a black hole.
///
/// # Panics
///
/// In debug builds, if `flux` is negative or not finite.
///
/// # Examples
///
/// Earth, with its Bond albedo of 0.294 (NASA's fact sheet), is at 255 K:
///
/// ```
/// use hyperion_sim::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
/// use hyperion_sim::units::EarthFluxes;
///
/// let albedo = BondAlbedo::new(0.294).expect("inside 0 to 1");
/// let t = equilibrium_temperature(EarthFluxes::new(1.000_14), albedo).value();
/// assert!((t - 255.1).abs() < 0.1);
/// ```
#[must_use]
pub fn equilibrium_temperature(flux: EarthFluxes, albedo: BondAlbedo) -> Kelvin {
    let s = WattsPerSquareMetre::from(flux).value();
    debug_assert!(s.is_finite() && s >= 0.0, "a flux is not negative, got {s}");
    let absorbed = s * (1.0 - albedo.0) / (4.0 * STEFAN_BOLTZMANN);
    Kelvin::new(absorbed.sqrt().sqrt())
}

/// The temperature of a body at equilibrium temperature `t_eq` that also radiates an internal
/// luminosity `internal` from its surface of radius `radius`: T⁴ = `T_eq`⁴ + `L_int` ÷ (4πR²σ).
///
/// # Panics
///
/// In debug builds, if `internal` is negative or `radius` is not positive.
#[must_use]
pub fn with_internal_heat(t_eq: Kelvin, internal: Watts, radius: Metres) -> Kelvin {
    debug_assert!(
        internal.value() >= 0.0,
        "an internal luminosity is not negative"
    );
    debug_assert!(radius.value() > 0.0, "a radius is positive");
    let r = radius.value();
    let area = 4.0 * core::f64::consts::PI * r * r;
    let t4 = math::powi(t_eq.value(), 4) + internal.value() / (area * STEFAN_BOLTZMANN);
    Kelvin::new(t4.sqrt().sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::{METRES_PER_AU, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_RADIUS_M};

    fn sun() -> HostLight {
        HostLight::new(
            SolarLuminosities::new(1.0),
            Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
            SolarRadii::new(1.0),
        )
        .unwrap()
    }

    fn t_eq(a_au: f64, e: f64, albedo: f64) -> f64 {
        let s = flux(&sun(), Metres::new(a_au * METRES_PER_AU), e);
        equilibrium_temperature(s, BondAlbedo::new(albedo).unwrap()).value()
    }

    #[test]
    fn the_planets_equilibrium_temperatures_follow_from_their_albedos() {
        // Semi-major axes (au) and eccentricities from NASA's fact sheets; Bond albedos and
        // black-body temperatures from the same sheets (fetched 2026-09-23), except Venus's 0.76,
        // plan 14's, where the sheet has 0.77 and 226.6 K.
        for (name, a, e, albedo, expected) in [
            ("Earth", 1.000_000_11, 0.016_7, 0.294, 255.0),
            ("Venus", 0.723_331_99, 0.006_8, 0.76, 229.0),
            ("Mars", 1.523_662_31, 0.093_5, 0.250, 210.0),
            ("Jupiter", 5.203_363_01, 0.048_7, 0.343, 110.0),
            ("Venus, fact sheet", 0.723_331_99, 0.006_8, 0.77, 226.6),
            ("Mars, fact sheet", 1.523_662_31, 0.093_5, 0.250, 209.8),
            ("Jupiter, fact sheet", 5.203_363_01, 0.048_7, 0.343, 109.9),
        ] {
            let t = t_eq(a, e, albedo);
            assert!((t - expected).abs() < 2.0, "{name}: {t} K");
        }
    }

    #[test]
    fn the_flux_form_is_plan_14_s_stellar_form() {
        // T★ √(R★ ÷ 2a) (1 − A)^¼ (1 − e²)^(−⅛), with the nominal Sun, which satisfies
        // Stefan–Boltzmann to the rounding of its values.
        for (a, e, albedo) in [(1.0, 0.0, 0.3), (0.05, 0.2, 0.1), (30.0, 0.6, 0.5)] {
            let ours = t_eq(a, e, albedo);
            let r_over_2a = SOLAR_RADIUS_M / (2.0 * a * METRES_PER_AU);
            let theirs = SOLAR_EFFECTIVE_TEMPERATURE_K
                * r_over_2a.sqrt()
                * math::powf(1.0 - albedo, 0.25)
                * math::powf(1.0 - e * e, -0.125);
            assert!(
                (ours / theirs - 1.0).abs() < 1e-4,
                "{a} au: {ours} against {theirs}"
            );
        }
    }

    #[test]
    fn two_equal_stars_warm_a_circumbinary_planet_by_the_fourth_root_of_two() {
        let at = |host| Illumination::new(host, Metres::new(2.0 * METRES_PER_AU), 0.1).unwrap();
        let albedo = BondAlbedo::BEFORE_ATMOSPHERES;
        let one = equilibrium_temperature(total_flux(&[at(sun())]), albedo).value();
        let two = equilibrium_temperature(total_flux(&[at(sun()), at(sun())]), albedo).value();
        assert!((two / one - math::powf(2.0, 0.25)).abs() < 1e-12);
        // A companion at 50 au adds its own share, averaged over the binary's orbit.
        let companion = Illumination::new(sun(), Metres::new(50.0 * METRES_PER_AU), 0.5).unwrap();
        assert_eq!(Illumination::new(sun(), Metres::ZERO, 0.5), None);
        assert_eq!(Illumination::new(sun(), Metres::new(1.0), 1.0), None);
        let with = total_flux(&[at(sun()), companion]).value();
        let expected = 1.0 / (4.0 * 0.99_f64.sqrt()) + 1.0 / (2_500.0 * 0.75_f64.sqrt());
        assert!((with - expected).abs() < 1e-12);
    }

    #[test]
    fn a_black_hole_irradiates_nothing() {
        let hole = HostLight::new(
            SolarLuminosities::ZERO,
            Kelvin::ZERO,
            SolarRadii::new(4.2e-5),
        )
        .unwrap();
        let s = flux(&hole, Metres::new(METRES_PER_AU), 0.3);
        assert!(s.value().abs() < f64::MIN_POSITIVE);
        let t = equilibrium_temperature(s, BondAlbedo::BEFORE_ATMOSPHERES);
        assert!(t.value().abs() < f64::MIN_POSITIVE);
        // Internal heat alone: what the surface radiates of its own.
        let (area, t4) = (4.0 * core::f64::consts::PI * 1e14, math::powi(99.0, 4));
        let own = with_internal_heat(
            t,
            Watts::new(t4 * area * STEFAN_BOLTZMANN),
            Metres::new(1e7),
        );
        assert!((own.value() - 99.0).abs() < 1e-9, "{own:?}");
    }

    #[test]
    fn internal_heat_adds_in_the_fourth_power() {
        // As much internal flux as absorbed doubles T⁴.
        let radius = Metres::new(7e7);
        let area = 4.0 * core::f64::consts::PI * radius.value() * radius.value();
        let internal = Watts::new(math::powi(110.0, 4) * STEFAN_BOLTZMANN * area);
        let t = with_internal_heat(Kelvin::new(110.0), internal, radius);
        assert!(
            (t.value() / 110.0 - math::powf(2.0, 0.25)).abs() < 1e-12,
            "{t:?}"
        );
        let none = with_internal_heat(Kelvin::new(200.0), Watts::ZERO, Metres::new(1e7));
        assert!((none.value() - 200.0).abs() < 1e-12);
    }

    #[test]
    fn hosts_and_albedos_are_validated_once() {
        let (l, t, r) = (
            SolarLuminosities::new(1.0),
            Kelvin::new(5_772.0),
            SolarRadii::new(1.0),
        );
        assert!(HostLight::new(l, t, r).is_ok());
        assert_eq!(
            HostLight::new(SolarLuminosities::new(-1.0), t, r),
            Err(BuildHostLightError::LuminosityNotValid)
        );
        assert_eq!(
            HostLight::new(l, Kelvin::new(f64::NAN), r),
            Err(BuildHostLightError::TemperatureNotValid)
        );
        assert_eq!(
            HostLight::new(l, t, SolarRadii::new(f64::INFINITY)),
            Err(BuildHostLightError::RadiusNotValid)
        );
        assert_eq!(
            HostLight::new(l, Kelvin::ZERO, r),
            Err(BuildHostLightError::LuminousWithoutSurface)
        );
        assert_eq!(BondAlbedo::new(1.0), None);
        assert_eq!(BondAlbedo::new(-0.1), None);
        assert!((BondAlbedo::BEFORE_ATMOSPHERES.value() - 0.3).abs() < 1e-15);
        let text = BuildHostLightError::LuminousWithoutSurface.to_string();
        assert!(text.starts_with('a') && !text.ends_with('.'));
    }
}
