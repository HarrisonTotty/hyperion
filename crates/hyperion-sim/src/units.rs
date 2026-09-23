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

    /// The day, s: 86,400 SI seconds, the unit in which orbital periods are quoted. The Julian
    /// year is exactly 365.25 of them (IAU).
    pub const SECONDS_PER_DAY: f64 = 86_400.0;

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

    /// The Boltzmann constant, J K⁻¹. Exact in the 2019 SI, which defines the kelvin by it
    /// (CODATA 2018 and 2022: 1.380 649 × 10⁻²³ exactly).
    pub const BOLTZMANN_CONSTANT: f64 = 1.380_649e-23;

    /// The mass of a hydrogen atom (¹H), kg: about 1.673 533 × 10⁻²⁷ kg.
    ///
    /// The proton and the electron of CODATA 2022, 1.672 621 925 95 × 10⁻²⁷ kg and
    /// 9.109 383 7139 × 10⁻³¹ kg, less the 13.6 eV of binding, 2.42 × 10⁻³⁵ kg. The interstellar
    /// medium's mass density is quoted as 1.4 × this × the number density of hydrogen nuclei, the
    /// 1.4 standing for helium (plan 07, Design note 2); a model that counts nuclei rather than
    /// protons wants the atom's mass, which is 0.05% above the proton's.
    pub const HYDROGEN_MASS_KG: f64 = 1.673_532_84e-27;

    /// Metres per second in one kilometre per second.
    pub const METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND: f64 = 1e3;

    /// Radians in one degree, π ÷ 180.
    pub const RADIANS_PER_DEGREE: f64 = core::f64::consts::PI / 180.0;

    /// The Earth radius that planetary radii are quoted in, m: the volumetric mean radius,
    /// 6,371.000 km (NASA Earth Fact Sheet; IUGG's R₁ is 6,371.0088 km).
    ///
    /// It is the unit of Zeng, Sasselov and Jacobsen's (2016, ApJ 819, 127) mass–radius table
    /// (their footnote 3, R⊕ = 6.371 × 10⁶ m). Chen and Kipping (2017) and Lopez and Fortney (2014)
    /// quote radii in R⊕ without saying which; the 0.11% between the mean and IAU 2015's nominal
    /// equatorial radius (6,378.1 km) is below the precision of either.
    pub const EARTH_RADIUS_M: f64 = 6.371e6;

    /// The nominal equatorial radius of Jupiter, m (IAU 2015 Resolution B3: 7.1492 × 10⁷ m), the
    /// unit of giant-planet radii such as Fortney, Marley and Barnes's (2007, ApJ 659, 1661).
    pub const JUPITER_RADIUS_M: f64 = 7.1492e7;

    /// The Stefan–Boltzmann constant σ, W m⁻² K⁻⁴: 5.670 374 419 × 10⁻⁸ (CODATA 2018 and 2022;
    /// exact in the 2019 SI, which fixes h, c and k, to the digits given).
    pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

    /// The irradiance at 1 au from the nominal Sun, W m⁻²: L☉ ÷ 4π au² = 1,361.2 W m⁻², the unit
    /// [`EarthFluxes`](super::EarthFluxes) counts in.
    ///
    /// IAU 2015 Resolution B3 defines the nominal total solar irradiance as 1,361 W m⁻² and derives
    /// the nominal luminosity 3.828 × 10²⁶ W from it, rounded; this constant runs the other way, so
    /// that a host of L solar luminosities gives exactly L ÷ d² of it at d au.
    pub const SOLAR_CONSTANT_W_PER_M2: f64 =
        SOLAR_LUMINOSITY_W / (4.0 * core::f64::consts::PI * METRES_PER_AU * METRES_PER_AU);
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
unit!(
    /// A length in mean Earth radii ([`consts::EARTH_RADIUS_M`]), an edge unit: the unit of
    /// planetary radii in the mass–radius relations (plan 14).
    EarthRadii
);
unit!(
    /// A length in nominal equatorial Jupiter radii ([`consts::JUPITER_RADIUS_M`]), an edge unit.
    JupiterRadii
);
dimension!(Metres;
    LightYears = consts::METRES_PER_LIGHT_YEAR,
    Parsecs = consts::METRES_PER_PARSEC,
    Kiloparsecs = consts::METRES_PER_KILOPARSEC,
    AstronomicalUnits = consts::METRES_PER_AU,
    SolarRadii = consts::SOLAR_RADIUS_M,
    EarthRadii = consts::EARTH_RADIUS_M,
    JupiterRadii = consts::JUPITER_RADIUS_M,
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
unit!(
    /// A duration in days of 86,400 s, an edge unit: the unit of orbital periods in the
    /// multiplicity surveys (plan 11).
    Days
);
dimension!(Seconds;
    Years = consts::SECONDS_PER_JULIAN_YEAR,
    Megayears = consts::SECONDS_PER_MEGAYEAR,
    Gigayears = consts::SECONDS_PER_GIGAYEAR,
    Days = consts::SECONDS_PER_DAY,
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
    /// A gravitational parameter μ = GM in m³ s⁻², the SI unit used inside the sim: what every
    /// orbit takes in place of a mass.
    ///
    /// GM is known far better than G or M apart (IAU 2015 Resolution B3 gives the nominal solar,
    /// Jovian and terrestrial values, [`consts::GM_SUN`], [`consts::GM_JUPITER`] and
    /// [`consts::GM_EARTH`]), so a mass in one of those units converts through its own GM, never
    /// through kilograms. A relative orbit of two bodies takes the sum of their parameters. It
    /// stands outside the mass dimension's conversions: the constructors below name their unit.
    GravitationalParameter
);

impl GravitationalParameter {
    /// The parameter of a mass in nominal solar masses: GM☉ × m.
    #[must_use]
    pub fn from_solar_masses(mass: SolarMasses) -> Self {
        Self(consts::GM_SUN * mass.0)
    }

    /// The parameter of a mass in nominal Jovian masses: GM of Jupiter × m.
    #[must_use]
    pub fn from_jupiter_masses(mass: JupiterMasses) -> Self {
        Self(consts::GM_JUPITER * mass.0)
    }

    /// The parameter of a mass in nominal terrestrial masses: GM of Earth × m.
    #[must_use]
    pub fn from_earth_masses(mass: EarthMasses) -> Self {
        Self(consts::GM_EARTH * mass.0)
    }

    /// The parameter of a mass in kilograms: G × m, with CODATA's G, which is known only to
    /// 2 × 10⁻⁵. Prefer the constructors from the nominal mass units.
    #[must_use]
    pub fn from_kilograms(mass: Kilograms) -> Self {
        Self(consts::GRAVITATIONAL_CONSTANT * mass.0)
    }
}

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
unit!(
    /// A number density of hydrogen nuclei per cubic centimetre, the unit every measurement and
    /// every formula of the interstellar medium is quoted in (plan 07, Design note 2).
    ///
    /// The mass density it stands for is 1.4 × [`consts::HYDROGEN_MASS_KG`] × this, the 1.4
    /// standing for the helium that accompanies the hydrogen.
    HydrogenPerCm3
);
unit!(
    /// A thermal pressure divided by the Boltzmann constant, P ÷ k in K cm⁻³: the form
    /// interstellar pressures are measured and quoted in (plan 07, Design note 2).
    ///
    /// The pressure in pascals is this × [`consts::BOLTZMANN_CONSTANT`] × 10⁶ (cm⁻³ to m⁻³).
    KelvinPerCm3
);
unit!(
    /// A column density per square centimetre, such as the hydrogen column along a line of sight.
    PerCm2
);
unit!(
    /// An extinction or a colour excess in magnitudes, 2.5 log₁₀ of the ratio of two fluxes.
    Magnitudes
);
unit!(
    /// A wavelength in micrometres, the unit the interstellar extinction law is written in
    /// (Cardelli, Clayton and Mathis 1989, whose argument is 1 ÷ λ in µm⁻¹).
    ///
    /// It stands outside the length dimension's conversions on purpose: nothing turns a wavelength
    /// into metres, and the law's coefficients are fitted to µm⁻¹.
    Micrometres
);
unit!(
    /// A surface density in kilograms per square metre, such as a protoplanetary disc's column of
    /// gas or solids (plan 14). One gram per square centimetre, the unit the planet-formation
    /// literature quotes, is 10 kg m⁻².
    KilogramsPerSquareMetre
);
unit!(
    /// A mass density in kilograms per cubic metre, such as a body's bulk density in a Roche limit
    /// (plan 14). One gram per cubic centimetre is 1,000 kg m⁻³.
    KilogramsPerCubicMetre
);
unit!(
    /// An irradiance, the power arriving per unit area, in watts per square metre: the SI unit used
    /// inside the sim for the light a body receives from its hosts (plan 14).
    WattsPerSquareMetre
);
unit!(
    /// An irradiance in units of the flux at 1 au from the nominal Sun
    /// ([`consts::SOLAR_CONSTANT_W_PER_M2`], 1,361.2 W m⁻²), an edge unit: the S⊕ or F⊕ that
    /// habitable-zone fluxes (Kopparapu et al. 2013) and the envelope tables (Lopez and Fortney
    /// 2014) are quoted in. A host of L solar luminosities gives L ÷ d² of it at d au.
    EarthFluxes
);
dimension!(WattsPerSquareMetre; EarthFluxes = consts::SOLAR_CONSTANT_W_PER_M2);

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

    /// Two independent routes to the hydrogen atom's mass agree: its proton and electron
    /// (CODATA 2022) less the mass of its 13.6 eV of binding, and its relative atomic mass times
    /// the atomic mass constant. The Boltzmann constant is checked through the molar gas constant,
    /// `R = k N_A`, every one of the three exact in the 2019 SI.
    #[test]
    fn the_interstellar_medium_constants_are_codata_values() {
        let (proton, electron) = (1.672_621_925_95e-27, 9.109_383_713_9e-31);
        let binding = 13.598_434_599_702 * 1.602_176_634e-19 / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        assert_relative(HYDROGEN_MASS_KG, proton + electron - binding, 1e-9);
        let atomic_mass_constant = 1.660_539_068_92e-27;
        assert_relative(
            HYDROGEN_MASS_KG,
            1.007_825_031_90 * atomic_mass_constant,
            1e-8,
        );
        let avogadro = 6.022_140_76e23;
        assert_relative(BOLTZMANN_CONSTANT * avogadro, 8.314_462_618_153_24, 1e-15);
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
            round_trip!(Seconds, Days, value);
            round_trip!(Days, Years, value);
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
        assert_same_bits(Days::from(Years::new(1.0)).value(), 365.25);
        assert_same_bits(Seconds::from(Days::new(1.0)).value(), 86_400.0);
    }

    #[test]
    fn gravitational_parameters_come_from_the_nominal_mass_parameters() {
        assert_same_bits(
            GravitationalParameter::from_solar_masses(SolarMasses::new(2.0)).value(),
            2.0 * GM_SUN,
        );
        assert_same_bits(
            GravitationalParameter::from_jupiter_masses(JupiterMasses::new(1.0)).value(),
            GM_JUPITER,
        );
        assert_same_bits(
            GravitationalParameter::from_earth_masses(EarthMasses::new(0.5)).value(),
            0.5 * GM_EARTH,
        );
        assert_relative(
            GravitationalParameter::from_kilograms(Kilograms::new(SOLAR_MASS_KG)).value(),
            GM_SUN,
            1e-15,
        );
    }

    #[test]
    fn planetary_radii_and_irradiances_convert_through_their_si_units() {
        for value in [1.0, 3.7, 1e-6, 12_345.678] {
            round_trip!(Metres, EarthRadii, value);
            round_trip!(Metres, JupiterRadii, value);
            round_trip!(EarthRadii, JupiterRadii, value);
            round_trip!(WattsPerSquareMetre, EarthFluxes, value);
        }
        // Jupiter's nominal equatorial radius is 11.2 mean Earth radii.
        assert_relative(
            EarthRadii::from(JupiterRadii::new(1.0)).value(),
            11.221_5,
            1e-5,
        );
        // L☉ ÷ 4π au² rounds to IAU 2015's nominal total solar irradiance, 1,361 W m⁻².
        assert!((SOLAR_CONSTANT_W_PER_M2 - 1_361.0).abs() < 0.5);
        assert_relative(SOLAR_CONSTANT_W_PER_M2, 1_361.166_5, 1e-6);
        // σ from its defining constants, 2π⁵k⁴ ÷ 15h³c², with h exact in the 2019 SI.
        let h = 6.626_070_15e-34;
        let pi = core::f64::consts::PI;
        let sigma = 2.0 * crate::math::powi(pi, 5) * crate::math::powi(BOLTZMANN_CONSTANT, 4)
            / (15.0 * crate::math::powi(h, 3) * SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        assert_relative(STEFAN_BOLTZMANN, sigma, 1e-9);
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
