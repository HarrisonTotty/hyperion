//! Unit newtypes over `f64` and the physical constants that convert between them.
//!
//! Units inside the sim are SI: [`Metres`], [`Seconds`], [`Kilograms`], [`MetresPerSecond`],
//! [`Radians`], [`Kelvin`], [`Watts`]. Light-years, astronomical units, solar masses and the like
//! are conversions at the edges and in generation code where the source formulae use them, always
//! through a named newtype and a named constant in [`consts`].
//!
//! Every newtype has `new`, `value`, arithmetic within the unit (`+`, `-`, unary `-`), scaling by
//! `f64` (`*`, `/`), the ratio of two values as an `f64` (`/`), and `total_cmp`. None is `Eq` or
//! `Hash`: they hold floats. `From` converts in both directions between any two units of one
//! dimension, always by way of the SI unit, so `LightYears` to `Parsecs` is two multiplications.
//! Mixing dimensions, or two units of one dimension, does not compile:
//!
//! ```compile_fail
//! use hyperion_sim::units::{LightYears, Metres};
//! let _ = Metres::new(1.0) + LightYears::new(1.0);
//! ```

use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Physical constants, each with its source. SI units throughout.
pub mod consts {
    /// The speed of light in vacuum, m/s. Exact by definition of the metre (SI, 1983).
    pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;

    /// The Julian year, s: 365.25 days of 86,400 s. This is the year of the light-year's
    /// definition (IAU), so that one light-year is exactly `c` × one year.
    pub const SECONDS_PER_JULIAN_YEAR: f64 = 31_557_600.0;

    /// Seconds in a million Julian years.
    pub const SECONDS_PER_MEGAYEAR: f64 = SECONDS_PER_JULIAN_YEAR * 1e6;

    /// Seconds in a thousand million Julian years.
    pub const SECONDS_PER_GIGAYEAR: f64 = SECONDS_PER_JULIAN_YEAR * 1e9;

    /// The light-year, m: `c` × one Julian year = 9,460,730,472,580,800 m exactly, and exactly
    /// representable in `f64` (it is 147,823,913,634,075 × 2⁶).
    pub const METRES_PER_LIGHT_YEAR: f64 = 9_460_730_472_580_800.0;

    /// The astronomical unit, m. Exact (IAU 2012 Resolution B2).
    pub const METRES_PER_AU: f64 = 149_597_870_700.0;

    /// The parsec, m: 648,000 ÷ π au (IAU 2015 Resolution B2), about 3.0857 × 10¹⁶ m.
    pub const METRES_PER_PARSEC: f64 = 648_000.0 / core::f64::consts::PI * METRES_PER_AU;

    /// The kiloparsec, m.
    pub const METRES_PER_KILOPARSEC: f64 = METRES_PER_PARSEC * 1e3;

    /// The nominal solar radius, m (IAU 2015 Resolution B3).
    pub const SOLAR_RADIUS_M: f64 = 6.957e8;

    /// The nominal solar luminosity, W (IAU 2015 Resolution B3).
    pub const SOLAR_LUMINOSITY_W: f64 = 3.828e26;

    /// The nominal solar effective temperature, K (IAU 2015 Resolution B3).
    ///
    /// With [`SOLAR_LUMINOSITY_W`] and [`SOLAR_RADIUS_M`] it satisfies Stefan–Boltzmann,
    /// L = 4πR²σT⁴, to within the rounding of the three nominal values.
    pub const SOLAR_EFFECTIVE_TEMPERATURE_K: f64 = 5_772.0;

    /// The Newtonian constant of gravitation, m³ kg⁻¹ s⁻² (CODATA 2018: 6.674 30 × 10⁻¹¹, and
    /// unchanged in CODATA 2022).
    pub const GRAVITATIONAL_CONSTANT: f64 = 6.674_30e-11;

    /// The nominal solar mass parameter GM☉, m³ s⁻² (IAU 2015 Resolution B3).
    pub const GM_SUN: f64 = 1.327_124_4e20;

    /// The nominal Jovian mass parameter, m³ s⁻² (IAU 2015 Resolution B3).
    pub const GM_JUPITER: f64 = 1.266_865_3e17;

    /// The nominal terrestrial mass parameter, m³ s⁻² (IAU 2015 Resolution B3).
    pub const GM_EARTH: f64 = 3.986_004e14;

    /// The solar mass, kg: GM☉ ÷ G, about 1.988 41 × 10³⁰ kg.
    pub const SOLAR_MASS_KG: f64 = GM_SUN / GRAVITATIONAL_CONSTANT;

    /// The Jovian mass, kg: GM of Jupiter ÷ G, about 1.898 12 × 10²⁷ kg.
    pub const JUPITER_MASS_KG: f64 = GM_JUPITER / GRAVITATIONAL_CONSTANT;

    /// The terrestrial mass, kg: GM of Earth ÷ G, about 5.972 17 × 10²⁴ kg.
    pub const EARTH_MASS_KG: f64 = GM_EARTH / GRAVITATIONAL_CONSTANT;

    /// Metres per second in one kilometre per second.
    pub const METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND: f64 = 1e3;

    /// Radians in one degree, π ÷ 180.
    pub const RADIANS_PER_DEGREE: f64 = core::f64::consts::PI / 180.0;
}

/// Defines a unit newtype over `f64`.
macro_rules! unit {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
        pub struct $name(f64);

        impl $name {
            /// Zero of this unit.
            pub const ZERO: Self = Self(0.0);

            /// Wraps a value in this unit.
            #[must_use]
            pub const fn new(value: f64) -> Self {
                Self(value)
            }

            /// The value in this unit.
            #[must_use]
            pub const fn value(self) -> f64 {
                self.0
            }

            /// The absolute value.
            #[must_use]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }

            /// Total ordering of the values, NaN included, as [`f64::total_cmp`].
            #[must_use]
            pub fn total_cmp(&self, other: &Self) -> Ordering {
                self.0.total_cmp(&other.0)
            }
        }

        impl Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl Mul<f64> for $name {
            type Output = Self;
            fn mul(self, rhs: f64) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl Mul<$name> for f64 {
            type Output = $name;
            fn mul(self, rhs: $name) -> $name {
                $name(self * rhs.0)
            }
        }

        impl Div<f64> for $name {
            type Output = Self;
            fn div(self, rhs: f64) -> Self {
                Self(self.0 / rhs)
            }
        }

        impl Div for $name {
            type Output = f64;
            fn div(self, rhs: Self) -> f64 {
                self.0 / rhs.0
            }
        }
    };
}

/// Defines the conversions of one dimension: each edge unit to and from the SI unit through its
/// constant (edge × constant = SI), and each pair of edge units through the SI unit.
macro_rules! dimension {
    ($si:ident; $($edge:ident = $per_edge:expr),+ $(,)?) => {
        $(
            impl From<$edge> for $si {
                fn from(value: $edge) -> Self {
                    Self(value.0 * $per_edge)
                }
            }

            impl From<$si> for $edge {
                fn from(value: $si) -> Self {
                    Self(value.0 / $per_edge)
                }
            }
        )+
        dimension!(@pairs $si; $($edge),+);
    };
    (@pairs $si:ident; $head:ident $(, $tail:ident)*) => {
        $(
            impl From<$head> for $tail {
                fn from(value: $head) -> Self {
                    Self::from($si::from(value))
                }
            }

            impl From<$tail> for $head {
                fn from(value: $tail) -> Self {
                    Self::from($si::from(value))
                }
            }
        )*
        dimension!(@pairs $si; $($tail),*);
    };
    (@pairs $si:ident;) => {};
}

unit!(
    /// A length in metres, the SI unit used inside the sim.
    Metres
);
unit!(
    /// A length in light-years (Julian), an edge unit.
    LightYears
);
unit!(
    /// A length in parsecs, an edge unit.
    Parsecs
);
unit!(
    /// A length in kiloparsecs, an edge unit.
    Kiloparsecs
);
unit!(
    /// A length in astronomical units, an edge unit.
    AstronomicalUnits
);
unit!(
    /// A length in nominal solar radii, an edge unit.
    SolarRadii
);
dimension!(Metres;
    LightYears = consts::METRES_PER_LIGHT_YEAR,
    Parsecs = consts::METRES_PER_PARSEC,
    Kiloparsecs = consts::METRES_PER_KILOPARSEC,
    AstronomicalUnits = consts::METRES_PER_AU,
    SolarRadii = consts::SOLAR_RADIUS_M,
);

unit!(
    /// A duration in seconds, the SI unit used inside the sim.
    Seconds
);
unit!(
    /// A duration in Julian years, an edge unit.
    Years
);
unit!(
    /// A duration in millions of Julian years, an edge unit.
    Megayears
);
unit!(
    /// A duration in thousands of millions of Julian years, an edge unit.
    Gigayears
);
dimension!(Seconds;
    Years = consts::SECONDS_PER_JULIAN_YEAR,
    Megayears = consts::SECONDS_PER_MEGAYEAR,
    Gigayears = consts::SECONDS_PER_GIGAYEAR,
);

unit!(
    /// A mass in kilograms, the SI unit used inside the sim.
    Kilograms
);
unit!(
    /// A mass in nominal solar masses, an edge unit.
    SolarMasses
);
unit!(
    /// A mass in nominal Jovian masses, an edge unit.
    JupiterMasses
);
unit!(
    /// A mass in nominal terrestrial masses, an edge unit.
    EarthMasses
);
dimension!(Kilograms;
    SolarMasses = consts::SOLAR_MASS_KG,
    JupiterMasses = consts::JUPITER_MASS_KG,
    EarthMasses = consts::EARTH_MASS_KG,
);

unit!(
    /// A speed in metres per second, the SI unit used inside the sim.
    MetresPerSecond
);
unit!(
    /// A speed in kilometres per second, an edge unit.
    KilometresPerSecond
);
dimension!(MetresPerSecond;
    KilometresPerSecond = consts::METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND,
);

unit!(
    /// An angle in radians, the unit used inside the sim.
    Radians
);
unit!(
    /// An angle in degrees, an edge unit.
    Degrees
);
dimension!(Radians; Degrees = consts::RADIANS_PER_DEGREE);

unit!(
    /// A temperature in kelvin.
    Kelvin
);

unit!(
    /// A power in watts, the SI unit used inside the sim.
    Watts
);
unit!(
    /// A power in nominal solar luminosities, an edge unit.
    SolarLuminosities
);
dimension!(Watts; SolarLuminosities = consts::SOLAR_LUMINOSITY_W);

unit!(
    /// An angular frequency in radians per Julian year, such as the circular frequency Ω and the
    /// epicyclic frequency κ of a galactic orbit.
    PerYear
);
unit!(
    /// A number density per cubic light-year.
    PerCubicLightYear
);
unit!(
    /// A surface density per square light-year.
    PerSquareLightYear
);
unit!(
    /// A base-10 logarithmic quantity in dex (decades): an abundance such as \[Fe/H\], or the
    /// scatter of a quantity about a relation.
    Dex
);
unit!(
    /// A radial gradient of a logarithmic quantity in dex per kiloparsec, such as a disc's
    /// metallicity gradient.
    DexPerKiloparsec
);
unit!(
    /// A metal mass fraction Z, dimensionless, 0–1: the share of a star's mass in elements
    /// heavier than helium.
    ///
    /// The stellar evolution fits of Hurley, Pols and Tout (2000, MNRAS 315, 543) take Z = 0.02 as
    /// solar and are valid for 0.0001–0.03.
    MetalFraction
);
unit!(
    /// An excess ΔY of the helium mass fraction over the value a star's metallicity implies,
    /// dimensionless.
    ///
    /// Zero for every star the grid places, positive for the second-population members of
    /// globular clusters (up to about 0.18, the brainstorm's "Covering every class of star",
    /// helium row).
    HeliumExcess
);
unit!(
    /// A magnetic flux density in gauss (10⁻⁴ T), the unit in which stellar and neutron-star
    /// fields are quoted.
    Gauss
);
unit!(
    /// A rate of mass loss or gain in solar masses per Julian year, such as a stellar wind's.
    SolarMassesPerYear
);

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::consts::*;
    use super::*;

    fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
        let error = ((actual - expected) / expected).abs();
        assert!(
            error <= tolerance,
            "{actual} is {error:e} from {expected}, over {tolerance:e}"
        );
    }

    #[test]
    fn the_light_year_is_c_times_a_julian_year_exactly() {
        let metres: u64 = 299_792_458 * 31_557_600;
        assert_eq!(metres, 9_460_730_472_580_800);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the product is 147823913634075 × 2^6, exact in f64"
        )]
        let as_float = metres as f64;
        assert_same_bits(as_float, METRES_PER_LIGHT_YEAR);
        assert_same_bits(
            SPEED_OF_LIGHT * SECONDS_PER_JULIAN_YEAR,
            METRES_PER_LIGHT_YEAR,
        );
        assert_eq!(147_823_913_634_075_u64 << 6, metres);
    }

    #[test]
    fn a_parsec_is_three_point_two_six_light_years() {
        assert_relative(
            LightYears::from(Parsecs::new(1.0)).value(),
            3.261_563_777,
            1e-9,
        );
        assert_relative(METRES_PER_PARSEC, 3.085_677_581_491_367e16, 1e-15);
    }

    #[test]
    fn masses_in_kilograms_follow_from_the_mass_parameters() {
        assert_relative(SOLAR_MASS_KG, 1.988_41e30, 1e-5);
        assert_relative(JUPITER_MASS_KG, 1.898_12e27, 1e-5);
        assert_relative(EARTH_MASS_KG, 5.972_17e24, 1e-5);
    }

    /// IAU 2015 Resolution B3 derived the nominal temperature from best-estimate L and R through
    /// Stefan–Boltzmann and rounded it, so the nominal values agree to that rounding. σ is exact in
    /// the 2019 SI (5.670 374 419… × 10⁻⁸ W m⁻² K⁻⁴).
    #[test]
    fn the_nominal_solar_temperature_follows_from_luminosity_and_radius() {
        let sigma = 5.670_374_419e-8;
        let area = 4.0 * core::f64::consts::PI * SOLAR_RADIUS_M * SOLAR_RADIUS_M;
        let t4 = SOLAR_LUMINOSITY_W / (area * sigma);
        assert_relative(t4.sqrt().sqrt(), SOLAR_EFFECTIVE_TEMPERATURE_K, 1e-4);
    }

    macro_rules! round_trip {
        ($si:ident, $edge:ident, $value:expr) => {{
            let edge = $edge::new($value);
            let back = $edge::from($si::from(edge));
            assert_relative(back.value(), edge.value(), 1e-15);
            let si = $si::new($value);
            let back = $si::from($edge::from(si));
            assert_relative(back.value(), si.value(), 1e-15);
        }};
    }

    #[test]
    fn every_conversion_pair_round_trips() {
        for value in [1.0, 3.7, 1e-6, 12_345.678, 1e12] {
            round_trip!(Metres, LightYears, value);
            round_trip!(Metres, Parsecs, value);
            round_trip!(Metres, Kiloparsecs, value);
            round_trip!(Metres, AstronomicalUnits, value);
            round_trip!(Metres, SolarRadii, value);
            round_trip!(Seconds, Years, value);
            round_trip!(Seconds, Megayears, value);
            round_trip!(Seconds, Gigayears, value);
            round_trip!(Kilograms, SolarMasses, value);
            round_trip!(Kilograms, JupiterMasses, value);
            round_trip!(Kilograms, EarthMasses, value);
            round_trip!(MetresPerSecond, KilometresPerSecond, value);
            round_trip!(Radians, Degrees, value);
            round_trip!(Watts, SolarLuminosities, value);
            round_trip!(LightYears, Parsecs, value);
            round_trip!(Kiloparsecs, AstronomicalUnits, value);
            round_trip!(Years, Gigayears, value);
            round_trip!(SolarMasses, EarthMasses, value);
        }
    }

    #[test]
    fn edge_to_edge_goes_through_the_si_unit() {
        let kpc = Kiloparsecs::new(8.2);
        assert_same_bits(
            LightYears::from(kpc).value(),
            LightYears::from(Metres::from(kpc)).value(),
        );
        assert_relative(LightYears::from(kpc).value(), 26_744.8, 1e-5);
        assert_relative(
            Degrees::from(Radians::new(core::f64::consts::PI)).value(),
            180.0,
            1e-15,
        );
        assert_relative(Years::from(Gigayears::new(13.8)).value(), 1.38e10, 1e-15);
    }

    #[test]
    fn arithmetic_stays_within_the_unit() {
        let a = Metres::new(3.0);
        let b = Metres::new(1.5);
        assert_same_bits((a + b).value(), 4.5);
        assert_same_bits((a - b).value(), 1.5);
        assert_same_bits((-a).value(), -3.0);
        assert_same_bits((a * 2.0).value(), 6.0);
        assert_same_bits((2.0 * a).value(), 6.0);
        assert_same_bits((a / 2.0).value(), 1.5);
        assert_same_bits(a / b, 2.0);
        assert_same_bits(Metres::new(-2.0).abs().value(), 2.0);
        assert_eq!(a.total_cmp(&b), Ordering::Greater);
        assert_eq!(Metres::ZERO.total_cmp(&Metres::default()), Ordering::Equal);
    }
}
