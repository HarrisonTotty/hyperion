//! A body's interior as its surface reads it (plan 14, P14.T24.b; science-p14-t24b-laws Part 1):
//! its heat flow, its tectonic regime, its magnetic field and its present volcanic outgassing,
//! closed forms of its mass, composition, radius, age, host's \[Fe/H\], tides, surface state,
//! water and rotation, with no draws.
//!
//! # The heat flow
//!
//! F = `F_nt` + `F_tidal`, in W m⁻² at the surface, where `F_nt`, the non-tidal part, is what
//! P14.T56.d's tidal branches read:
//!
//! - **The radiogenic term**, `q_rad` = `M_rock` 10^\[Fe/H\] Σₖ Pₖ e^((A⊕ − A) ÷ τₖ) ÷ 4πR², over
//!   ²³⁸U, ²³⁵U, ²³²Th and ⁴⁰K ([`RadiogenicIsotope`]), each with its own mean life τₖ: the bulk
//!   silicate Earth's production per kilogram of rock today, Pₖ, at the generator's Earth's age
//!   A⊕ ([`RADIOGENIC_REFERENCE_AGE`]), scaled by the host's metal abundance and decayed to the
//!   body's age A ([`radiogenic_heat_flow`]). 4.92 × 10⁻¹² W kg⁻¹ today, so that Earth's
//!   0.675 M⊕ of rock make 19.8 TW.
//! - **The residual heat by interior** (decision-p14-moons §F-ocean F1, as corrected by
//!   science-p14-t24b-laws §1.4), so that the flow is more than the present production
//!   ([`Interior`]):
//!   - an `Icy` body whose rock is under [`CONVECTING_ROCK_RADIUS`] holds it under its ice, and
//!     it conducts: its radiogenic term is multiplied by the cold-started lag L of a uniformly
//!     heated sphere ([`conductive_lag`]), above 1 where the heat of an earlier, stronger
//!     production is still coming out (Pluto's 1.29) and under 1 where it is still accumulating
//!     in a large rock core (Ganymede's 0.92);
//!   - every other solid body convects, and its mantle carries its core's cooling with its own
//!     ([`convecting_heat_flow`]): F = `q_rad` \[1 + (1 ÷ `Ur_s` − 1)(1 + γ x ÷ (1 − x))\], x the
//!     iron share of the solids, γ the core's heat capacity over the mantle's
//!     ([`CORE_TO_MANTLE_HEAT_CAPACITY`]) and `Ur_s` its regime's silicate Urey ratio,
//!     [`MOBILE_LID_UREY_RATIO`] for a mobile lid and [`STAGNANT_LID_UREY_RATIO`] for every other.
//! - **Tidal heat**, added, not multiplied: a moon's, which its derivation computes
//!   (P14.T17.b's eccentricity tide at 22, P14.T56.d's from 22 → 23).
//!
//! A gas envelope takes no solid heat flow: its internal temperature is the giant cooling's
//! (ruling 112.7), and it has no figures. A young body's hotter start is not separately modelled.
//!
//! # The tectonic regime
//!
//! One of five ([`TectonicRegime`]), by the first clause that holds, from inputs that do not read
//! the residual, so that the Urey ratio reads the regime with no loop ([`tectonic_regime`];
//! science-p14-t24b-laws §1.1): an ice shell over a computed ocean; a heat pipe under strong
//! tides or over a magma ocean; a stagnant lid on a conducting body; a lid that fails, mobile
//! where it is wet and episodic where it is dry, on a convecting body massive and productive
//! enough, whose water's floor lies above the high-pressure ice; and otherwise a stagnant lid.
//! Earth's lid is mobile, Venus's episodic, and Mars's, Mercury's and the Moon's stagnant.
//!
//! # The magnetic field
//!
//! [`MagneticField`] `None`, `Weak` or `Strong`, by the first clause that holds
//! ([`magnetic_field`]; science-p14-t24b-laws §1.2): a gas envelope's is strong; a conducting
//! interior, a core under a tenth of the solids or a mantle heated by its tides more than by its
//! rock has none; a thermal dynamo, under a mobile lid or in the first 0.6 Gyr, is strong for a
//! fast rotator and weak for a slow one; a core of half the solids on a body of 0.03 M⊕ keeps a
//! deep, weak dynamo; and otherwise there is none. Earth's is strong and Mercury's weak.
//! Ganymede's intrinsic field is a recorded miss.
//!
//! # The present outgassing
//!
//! O = max(0, (F − `F_c`) ÷ (F⊕ − `F_c`)), relative to Earth's ([`outgassing`]; P14.T24.g): F the
//! heat flow above, `F_c` the heat flow below which no melt reaches the surface
//! ([`SURFACE_MELT_HEAT_FLOW`]), and F⊕ the generator's Earth's ([`GENERATED_EARTH_HEAT_FLOW`]).
//! The regime and the age enter through F alone. It is the present rate, not the volcanism level
//! that builds relief over a surface's history (P14.T48.a's), which Mars has and this does not.

use crate::math;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::{EARTH_REFERENCE_AGE, SurfaceState};
use crate::planetary::params::SURFACE_MELT_HEAT_FLOW;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    Dex, EarthMasses, Gigayears, Kilograms, Metres, MetresPerSecondSquared, Pascals, Seconds,
    WattsPerSquareMetre, Years,
};

#[cfg(test)]
mod tests;

/// The age at which [`RadiogenicIsotope::present_heat_production_w_per_kg`] is the bulk silicate
/// Earth's measured production: the generator's Earth's, [`EARTH_REFERENCE_AGE`], 4.57 Gyr.
pub const RADIOGENIC_REFERENCE_AGE: Years = EARTH_REFERENCE_AGE;

/// The bulk silicate Earth's uranium, kg per kg of rock: 20.3 ppb (McDonough and Sun 1995, Chem.
/// Geol. 120, 223, the pyrolite model; not re-read).
pub const BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION: f64 = 20.3e-9;

/// The bulk silicate Earth's thorium, kg per kg of rock: 79.5 ppb (McDonough and Sun 1995; not
/// re-read).
pub const BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION: f64 = 79.5e-9;

/// The bulk silicate Earth's potassium, kg per kg of rock: 240 ppm (McDonough and Sun 1995; not
/// re-read).
pub const BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION: f64 = 240e-6;

/// A long-lived isotope whose decay heats a body's rock (P14.T24.b): the four that hold the bulk
/// silicate Earth's production today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RadiogenicIsotope {
    /// ²³⁸U.
    Uranium238,
    /// ²³⁵U.
    Uranium235,
    /// ²³²Th.
    Thorium232,
    /// ⁴⁰K.
    Potassium40,
}

impl RadiogenicIsotope {
    /// The four, in the order every sum over them runs.
    pub const ALL: [Self; 4] = [
        Self::Uranium238,
        Self::Uranium235,
        Self::Thorium232,
        Self::Potassium40,
    ];

    /// The heat its decay chain leaves in rock, W per kilogram of the isotope: 9.46 × 10⁻⁵ for
    /// ²³⁸U, 5.69 × 10⁻⁴ for ²³⁵U, 2.64 × 10⁻⁵ for ²³²Th and 2.92 × 10⁻⁵ for ⁴⁰K (Turcotte and
    /// Schubert 2002, Geodynamics, 2nd ed., Table 4.2, as decision-p14-moons §F-ocean cites them;
    /// not re-read).
    #[must_use]
    pub const fn specific_heat_production_w_per_kg(self) -> f64 {
        match self {
            Self::Uranium238 => 9.46e-5,
            Self::Uranium235 => 5.69e-4,
            Self::Thorium232 => 2.64e-5,
            Self::Potassium40 => 2.92e-5,
        }
    }

    /// Its half-life: 4.47 × 10⁹ yr for ²³⁸U, 7.04 × 10⁸ for ²³⁵U, 1.40 × 10¹⁰ for ²³²Th and
    /// 1.25 × 10⁹ for ⁴⁰K (Turcotte and Schubert 2002, Table 4.2; not re-read).
    #[must_use]
    pub const fn half_life(self) -> Years {
        Years::new(match self {
            Self::Uranium238 => 4.47e9,
            Self::Uranium235 => 7.04e8,
            Self::Thorium232 => 1.40e10,
            Self::Potassium40 => 1.25e9,
        })
    }

    /// Its mean life τ, the half-life over ln 2, over which its production falls by e.
    #[must_use]
    pub fn mean_life(self) -> Years {
        Years::new(self.half_life().value() / core::f64::consts::LN_2)
    }

    /// Its mass fraction of the bulk silicate Earth today, kg per kg of rock: its element's
    /// ([`BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION`],
    /// [`BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION`],
    /// [`BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION`]) times its mass share of that element.
    ///
    /// The shares are Turcotte and Schubert's (2002, Table 4.2; not re-read), with which their
    /// natural uranium's and potassium's rates follow from the isotopes' to 0.15%: 0.9928 of
    /// uranium as ²³⁸U and 0.0071 as ²³⁵U, thorium's whole as ²³²Th, and 1.19 × 10⁻⁴ of potassium
    /// as ⁴⁰K, their 36.9 × 10⁻⁹ over 31.0 × 10⁻⁵ (science-p14-t24b-laws §1.4). IUPAC's atom
    /// fractions (Meija et al. 2016, Pure Appl. Chem. 88, 293; read by the science check) give
    /// 0.992 83, 0.007 114 and 1.196 × 10⁻⁴ by mass.
    #[must_use]
    pub const fn bulk_silicate_earth_mass_fraction(self) -> f64 {
        match self {
            Self::Uranium238 => BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION * 0.9928,
            Self::Uranium235 => BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION * 0.0071,
            Self::Thorium232 => BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION,
            Self::Potassium40 => BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION * 1.19e-4,
        }
    }

    /// Its heat production per kilogram of the bulk silicate Earth today, W kg⁻¹: its mass
    /// fraction times its specific heat. The four sum to 4.9213 × 10⁻¹² W kg⁻¹, in shares of
    /// 0.3874, 0.0167, 0.4265 and 0.1695 (science-p14-t24b-laws §1.4), against 4.9 × 10⁻¹² in
    /// decision-p14-moons §F-ocean.
    #[must_use]
    pub const fn present_heat_production_w_per_kg(self) -> f64 {
        self.bulk_silicate_earth_mass_fraction() * self.specific_heat_production_w_per_kg()
    }
}

/// The radiogenic heat production per kilogram of rock of a body about a host of \[Fe/H\] `fe_h`
/// at the age `age`, W kg⁻¹, by isotope in [`RadiogenicIsotope::ALL`]'s order:
/// 10^\[Fe/H\] Pₖ e^((A⊕ − A) ÷ τₖ), Pₖ the bulk silicate Earth's at the reference age A⊕
/// ([`RADIOGENIC_REFERENCE_AGE`]).
///
/// The host's metal abundance scales every isotope alike, as the plan writes it: its uranium,
/// thorium and potassium are not drawn apart from its iron.
#[must_use]
pub fn radiogenic_heat_production_w_per_kg(fe_h: Dex, age: Years) -> [f64; 4] {
    let scale = math::exp10(fe_h.value());
    let since = RADIOGENIC_REFERENCE_AGE.value() - age.value();
    RadiogenicIsotope::ALL.map(|isotope| {
        scale
            * isotope.present_heat_production_w_per_kg()
            * math::exp(since / isotope.mean_life().value())
    })
}

/// The radiogenic heat flow at the surface of a body of radius `radius` holding `rock` of rock,
/// about a host of \[Fe/H\] `fe_h` at the age `age`: the rock's production
/// ([`radiogenic_heat_production_w_per_kg`], summed in its order) over the surface area 4πR²,
/// before any residual heat. Zero for a radius that is not positive.
///
/// # Examples
///
/// An Earth's 0.675 M⊕ of rock at 4.57 Gyr make 19.8 TW, 38.9 mW m⁻², which its mobile lid's
/// residual heat, with its core's, raises to Earth's 0.092 W m⁻² ([`convecting_heat_flow`]):
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::interior::{convecting_heat_flow, radiogenic_heat_flow};
/// use hyperion_sim::planetary::hooks::figures::TectonicRegime;
/// use hyperion_sim::units::{Dex, EarthMasses, Kilograms, Metres, Years};
///
/// let (radius, age) = (Metres::new(6.371e6), Years::new(4.57e9));
/// let rock = Kilograms::from(EarthMasses::new(0.675));
/// let flow = radiogenic_heat_flow(rock, radius, Dex::new(0.0), age);
/// assert!((flow.value() - 0.0389).abs() < 0.0005);
/// // Its solids, iron and rock, as if they were all rock, and the iron's share of them.
/// let solids = radiogenic_heat_flow(Kilograms::from(EarthMasses::new(1.0)), radius, Dex::new(0.0), age);
/// let earth = convecting_heat_flow(solids, 0.325, TectonicRegime::MobileLid);
/// assert!((earth.value() / 0.092 - 1.0).abs() < 0.03);
/// ```
#[must_use]
pub fn radiogenic_heat_flow(
    rock: Kilograms,
    radius: Metres,
    fe_h: Dex,
    age: Years,
) -> WattsPerSquareMetre {
    let r = radius.value();
    if r.is_nan() || r <= 0.0 {
        return WattsPerSquareMetre::new(0.0);
    }
    let per_kg = radiogenic_heat_production_w_per_kg(fe_h, age)
        .iter()
        .fold(0.0, |sum, &production| sum + production);
    WattsPerSquareMetre::new(rock.value() * per_kg / (4.0 * core::f64::consts::PI * r * r))
}

/// The thermal diffusivity of an icy body's rock, κ: 10⁻⁶ m² s⁻¹ (decision-p14-moons §F-ocean F1,
/// from memory), a silicate's k ÷ ρ `c_p`, about 3 W m⁻¹ K⁻¹ over 3,300 kg m⁻³ and
/// 1,000 J kg⁻¹ K⁻¹.
pub const ROCK_THERMAL_DIFFUSIVITY_M2_PER_S: f64 = 1e-6;

/// The density at which an icy body's non-water mass is a rock sphere for its lag: 3,300 kg m⁻³, a
/// small moon's uncompressed rock
/// ([`MOON_ROCK_DENSITY`](crate::planetary::moons::regular::MOON_ROCK_DENSITY); decision-p14-moons
/// §F-ocean F1).
pub const ROCK_CORE_DENSITY: crate::units::KilogramsPerCubicMetre =
    crate::planetary::moons::regular::MOON_ROCK_DENSITY;

/// The conduction modes [`conductive_lag`] sums: 400, decision-p14-moons §F-ocean F1's.
///
/// The modes beyond hold 6 ÷ (400 π²) of the sum, 0.0015, at their steady state, so a sphere of
/// infinite diffusivity lags by 0.9985, not 1.
pub const LAG_MODES: u32 = 400;

/// The cold-started conductive lag L of a rock sphere of radius `rock_radius` and thermal
/// diffusivity `diffusivity_m2_per_s`, heated uniformly by the radiogenic isotopes from its birth
/// with its surface held cold, at the age `age`: its surface heat flow over its production then
/// (decision-p14-moons §F-ocean F1; science-p14-subsurface-ocean's closed form):
///
/// L = Σₖ wₖ Σ_{n ≤ 400} 6 ÷ (n²π²) × sₙ ÷ (sₙ − λₖ) × (1 − e^(−(sₙ − λₖ) t)),
///
/// with sₙ = n²π²κ ÷ `R_c`² each mode's decay rate, λₖ = 1 ÷ τₖ each isotope's, t the age and
/// wₖ each isotope's share of the production at that age; the term is sₙ t where sₙ = λₖ. It is
/// the conduction solution of a uniformly heated sphere in modes (Carslaw and Jaeger 1959, from
/// memory), as Hammond, Barr and Parmentier 2016 treat Pluto's rock. At 4.5 Gyr it is 1.29 for an
/// 850 km sphere (Pluto's rock), 1.01 for 190 km (Enceladus's) and 0.92 for 1,800 km (Ganymede's):
/// heat still leaving a small core from its stronger past, and still accumulating in a large one.
///
/// The shares are the production's own at the age, hₖ e^(−t ÷ τₖ) ÷ Σⱼ hⱼ e^(−t ÷ τⱼ)
/// ([`radiogenic_heat_production_w_per_kg`]), the exact solution for a decaying source, 0.3874,
/// 0.0167, 0.4265 and 0.1695 at 4.57 Gyr, so that the lag and the radiogenic term it multiplies
/// read one mixture of isotopes (science-p14-t24b-laws §1.4). Decision-p14-moons took the present
/// shares 0.386, 0.014, 0.43 and 0.17, whose ²³⁵U was 16% low; they gave Pluto 1.26.
///
/// 0 at no age and for a rock that does not conduct, and [`LAG_MODES`]' steady state, 0.9985, for
/// a sphere of no radius or of an infinite diffusivity. The diffusivity is finite or infinite and
/// not negative.
///
/// # Examples
///
/// Pluto's 850 km of rock, at its age, gives out more heat than it makes now:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::interior::{ROCK_THERMAL_DIFFUSIVITY_M2_PER_S, conductive_lag};
/// use hyperion_sim::units::{Metres, Years};
///
/// let lag = conductive_lag(Metres::new(850e3), Years::new(4.5e9), ROCK_THERMAL_DIFFUSIVITY_M2_PER_S);
/// assert!((lag - 1.285).abs() < 0.005);
/// ```
#[must_use]
pub fn conductive_lag(rock_radius: Metres, age: Years, diffusivity_m2_per_s: f64) -> f64 {
    let production = radiogenic_heat_production_w_per_kg(Dex::new(0.0), age);
    let total = production.iter().fold(0.0, |sum, &p| sum + p);
    lag_of(
        production.map(|p| p / total),
        rock_radius,
        age,
        diffusivity_m2_per_s,
    )
}

/// [`conductive_lag`] with the isotopes' shares of the production `shares`, in
/// [`RadiogenicIsotope::ALL`]'s order.
#[must_use]
pub(crate) fn lag_of(
    shares: [f64; 4],
    rock_radius: Metres,
    age: Years,
    diffusivity_m2_per_s: f64,
) -> f64 {
    let t = age.value() * SECONDS_PER_JULIAN_YEAR;
    if t.is_nan() || t <= 0.0 || diffusivity_m2_per_s.is_nan() || diffusivity_m2_per_s <= 0.0 {
        return 0.0;
    }
    let pi2 = core::f64::consts::PI * core::f64::consts::PI;
    let r = rock_radius.value();
    // A sphere of no radius, or of an infinite diffusivity, is at every mode's steady state.
    let rate = diffusivity_m2_per_s / (r * r);
    let steady = rate.is_infinite();
    RadiogenicIsotope::ALL
        .iter()
        .zip(shares)
        .fold(0.0, |lag, (isotope, share)| {
            let decay = 1.0 / (isotope.mean_life().value() * SECONDS_PER_JULIAN_YEAR);
            let modes = (1..=LAG_MODES).fold(0.0, |sum, n| {
                let n2 = f64::from(n * n);
                let weight = 6.0 / (n2 * pi2);
                if steady {
                    return sum + weight;
                }
                let s = n2 * pi2 * rate;
                sum + weight * s * t * held_share((s - decay) * t)
            });
            lag + share * modes
        })
}

/// (1 − e^(−x)) ÷ x, 1 at x = 0: the share of a mode's steady heat a source has built up over a
/// time x ÷ (sₙ − λₖ) ([`conductive_lag`]).
#[must_use]
fn held_share(x: f64) -> f64 {
    if x.abs() < f64::EPSILON {
        // Within 10⁻¹⁶ of its limit, where the quotient loses its digits.
        1.0
    } else {
        -math::exp_m1(-x) / x
    }
}

/// The silicate Urey ratio of a mobile lid, its mantle's present radiogenic production over the
/// mantle's share of its heat loss: 0.49 (science-p14-t24b-laws §1.4; [`convecting_heat_flow`]).
///
/// Calibrated on the Solar System table's Earth at Davies and Davies 2010's 0.0916 W m⁻² (Solid
/// Earth 1, 5, Table 7: 46.7 TW), the ruling's replica's 0.4917 rounded; with the core's heat its
/// production of 0.675 M⊕ of rock at 4.92 × 10⁻¹² W kg⁻¹, 19.8 TW, gives the generated Earth 0.0925
/// W m⁻² (Davies and Davies's Table 7 read by the science check). Jaupart, Labrosse and Mareschal
/// 2007 (Treatise on Geophysics, 253) give the bulk silicate Earth 20 ± 4 TW of 46 ± 3 TW (not
/// re-read); Korenaga 2008 (Rev. Geophys. 46, RG2007, abstract) puts the bulk Earth's Urey ratio at
/// about 0.35 and the convective one at about 0.2. Decision-p14-moons §F-ocean's bulk ratio, 0.42,
/// had no core term.
pub const MOBILE_LID_UREY_RATIO: f64 = 0.49;

/// The silicate Urey ratio of every lid but a mobile one: 0.79 (science-p14-t24b-laws §1.4;
/// [`convecting_heat_flow`]).
///
/// Calibrated on the table's Mars at about 22 mW m⁻² (the ruling's target), the ruling's replica's
/// 0.7892 rounded; it gives the table's Mars 22.5 mW m⁻², the low end of the 22.5–24.3 mW m⁻² of
/// the nine best-fit 3-D thermal-evolution models of Plesa et al. 2018 (GRL 45, 12,198, Table S3,
/// on pre-InSight constraints; read by the science check), whose reference case is 23.6. A
/// stagnant lid loses its heat less efficiently, so more of its loss is present production. An
/// episodic lid between its overturns, a heat pipe and a convecting body's ice shell take it too,
/// so that the heat flow steps only at a mobile lid's edge. It gives the Moon, as rock alone,
/// 12.1 mW m⁻², within the 9–13 mW m⁻² of mantle flux that "best approximate the observed heat
/// flux" at the Apollo sites (Siegler and Smrekar 2014, JGR Planets 119, 47, abstract; read by the
/// science check), Mercury 10.8 and Venus 49.6. Decision-p14-moons §F-ocean's bulk ratio, 0.75,
/// had no core term.
pub const STAGNANT_LID_UREY_RATIO: f64 = 0.79;

/// The core's heat capacity per unit mass over the mantle's, γ: 2/3, 800 ÷ 1,200 J kg⁻¹ K⁻¹ for
/// liquid iron over silicate, since the core cools with the mantle in parametrised convection
/// (Stevenson, Spohn and Schubert 1983, Icarus 54, 466; science-p14-t24b-laws §1.4; from memory,
/// checked at build).
pub const CORE_TO_MANTLE_HEAT_CAPACITY: f64 = 2.0 / 3.0;

/// The radius of an icy body's rock, its non-water mass at [`ROCK_CORE_DENSITY`], from which the
/// rock convects rather than conducts: 2,000 km (science-p14-t24b-laws §1.4, the plan's own).
///
/// It is above every moon's rock in the Solar System (Ganymede's 1,807 km; the generated Ganymedes
/// to about 1,910 km), and far below a water world's of a planet's mass, 3,000–8,000 km, whose
/// Rayleigh number, growing as the radius to the fourth, makes it convect at any plausible
/// viscosity. The heat flow steps at it, in mass, not in time.
pub const CONVECTING_ROCK_RADIUS: Metres = Metres::new(2.0e6);

/// How a solid body's interior loses its heat (decision-p14-moons §F-ocean F1, as corrected by
/// science-p14-t24b-laws §1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Interior {
    /// An `Icy` body's rock under [`CONVECTING_ROCK_RADIUS`], conducting: its radiogenic term
    /// times its lag L ([`conductive_lag`]).
    Conducting,
    /// A silicate mantle over its core, convecting: every `Rocky` body, and an `Icy` body from
    /// [`CONVECTING_ROCK_RADIUS`] ([`convecting_heat_flow`]).
    Convecting,
}

impl Interior {
    /// The interior of a body of class `class` whose rock, its non-water mass at
    /// [`ROCK_CORE_DENSITY`], is `rock_radius` in radius; `None` for a gas envelope, which takes no
    /// solid heat flow.
    #[must_use]
    pub fn of(class: PlanetClass, rock_radius: Metres) -> Option<Self> {
        match class {
            PlanetClass::Icy if rock_radius < CONVECTING_ROCK_RADIUS => Some(Self::Conducting),
            PlanetClass::Rocky | PlanetClass::Icy => Some(Self::Convecting),
            PlanetClass::SubNeptune | PlanetClass::IceGiant | PlanetClass::GasGiant => None,
        }
    }
}

/// The non-tidal heat flow of a convecting body of tectonic regime `regime`, whose solids, iron
/// and rock, would make `solid_radiogenic` were they all rock, of which `iron_share` is iron, W m⁻²
/// (science-p14-t24b-laws §1.4):
///
/// F = `q_rad` \[1 + (1 ÷ `Ur_s` − 1)(1 + γ x ÷ (1 − x))\], with `q_rad` = (1 − x) `q_solid`,
///
/// written as `q_solid` \[(1 − x) + (1 ÷ `Ur_s` − 1)((1 − x) + γ x)\] so that it is finite at
/// x = 1: the mantle's production with its secular cooling, which carries the core's cooling at
/// the core's heat capacity γ ([`CORE_TO_MANTLE_HEAT_CAPACITY`]), `Ur_s` the regime's silicate
/// Urey ratio ([`MOBILE_LID_UREY_RATIO`] for a mobile lid, [`STAGNANT_LID_UREY_RATIO`] for every
/// other). At x = 0 it is `q_rad` ÷ `Ur_s`.
#[must_use]
pub fn convecting_heat_flow(
    solid_radiogenic: WattsPerSquareMetre,
    iron_share: f64,
    regime: TectonicRegime,
) -> WattsPerSquareMetre {
    let urey = match regime {
        TectonicRegime::MobileLid => MOBILE_LID_UREY_RATIO,
        TectonicRegime::StagnantLid
        | TectonicRegime::HeatPipe
        | TectonicRegime::Episodic
        | TectonicRegime::IceShell => STAGNANT_LID_UREY_RATIO,
    };
    let x = iron_share;
    let mantle = 1.0 - x;
    WattsPerSquareMetre::new(
        solid_radiogenic.value()
            * (mantle + (1.0 / urey - 1.0) * (mantle + CORE_TO_MANTLE_HEAT_CAPACITY * x)),
    )
}

/// A body's tectonic regime, how its lithosphere carries its heat (P14.T24.b; science-p14-t24b-laws
/// §1.1), which rendering plan R09's coarse pass reads (re-exported as
/// [`planetary::surface::TectonicRegime`](crate::planetary::surface::TectonicRegime)).
///
/// A closed physics class, every variant of which [`tectonic_regime`] produces. Until R09.T11–T12
/// rule a morphology for each, R09's pass gives `Episodic`, `HeatPipe` and `IceShell` the
/// stagnant lid's: no plates, and provinces sized by the volcanism level (R09's Risks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TectonicRegime {
    /// Plates in relative motion, as on Earth: the pass draws plates (R09.T11).
    MobileLid,
    /// One lid, as on Mars, the Moon and Mercury: volcanic provinces and their flexure, no plate
    /// boundaries (R09's Design note 6).
    StagnantLid,
    /// Heat lost through volcanic conduits, as on Io, and any magma-ocean surface.
    HeatPipe,
    /// A lid that overturns in episodes, as Venus is modelled here (Armann and Tackley 2012, JGR
    /// 117, E12003).
    Episodic,
    /// An ice shell of at most [`ICE_SHELL_LIMIT`] over an ocean, as on Europa and Enceladus.
    IceShell,
}

/// The thickest ice shell over a computed ocean that deforms as Europa's and Enceladus's do: 60 km
/// (science-p14-t24b-laws §1.1, the plan's own), the geometric mean of the generator's Enceladus
/// (42.6 km) and Titan (87 km) (decision-p14-moons F2).
///
/// Europa's shell is 10–30 km and Enceladus's some 20–26 km on average, where Ganymede's and
/// Callisto's are 100–150 km, Titan's 50–200 km and Dione's about 100 km, whose surfaces are old
/// or shaped otherwise (Nimmo and Pappalardo 2016, JGR Planets 121, 1378; from memory).
pub const ICE_SHELL_LIMIT: Metres = Metres::new(60e3);

/// The tidal heat flux from which a convecting body is a heat pipe: 0.25 W m⁻²
/// (science-p14-t24b-laws §1.1, the plan's own value), about three times Earth's mean, between
/// Earth's 0.09 and Io's 2.2, which leaves mostly through eruptions that bury cold lava and build a
/// thick, cold lithosphere (O'Reilly and Davies 1981, GRL 8, 313; Moore and Webb 2013, Nature 501,
/// 501; from memory). A stagnant lid at 0.25 W m⁻² would be some 12 km thick, too thin for Io's 17
/// km mountains.
pub const HEAT_PIPE_TIDAL_FLUX: WattsPerSquareMetre = WattsPerSquareMetre::new(0.25);

/// The mass below which a lid does not fail: 0.3 M⊕ (science-p14-t24b-laws §1.1), the
/// habitability literature's floor for sustained tectonics: Raymond, Scalo and Meadows 2007 (ApJ
/// 669, 606, §2.1, read by the science check) find 0.3 M⊕ for 5 Gyr of plate tectonics at
/// 4.5 g cm⁻³, from Williams, Kasting and Wade 1997's radiogenic-flux limit (Nature 385, 234).
/// Mars, at 0.107 M⊕, lies below it. No upper mass law is made, since
/// the studies disagree (Valencia, O'Connell and Sasselov 2007; O'Neill and Lenardic 2007;
/// Korenaga 2010).
pub const LID_FAILURE_MASS: EarthMasses = EarthMasses::new(0.3);

/// The radiogenic heat flux, before the residual, below which a lid does not fail: 0.020 W m⁻²
/// (science-p14-t24b-laws §1.1, the plan's own), between the generator's Mars's 17.1 and Venus's
/// 37.0 mW m⁻², so that an Earth twin's plates end at 9.8 Gyr, "a window for plate tectonics"
/// (O'Neill et al. 2016, PEPI 255, 80; from memory).
pub const LID_FAILURE_FLUX: WattsPerSquareMetre = WattsPerSquareMetre::new(0.020);

/// The water a mobile lid needs, as a global-equivalent depth: 100 m (science-p14-t24b-laws §1.1,
/// the plan's own). A shallower inventory floods only the lowest few per cent of a relief of
/// `σ_h` ≈ 2.5 km, and cannot hydrate the plates' boundaries globally (Korenaga 2010, ApJ 725,
/// L43, needs a hydrated lithosphere; from memory).
pub const PLATE_WATER_DEPTH: Metres = Metres::new(100.0);

/// The water's floor pressure from which high-pressure ice parts an ocean from its rock, which can
/// then neither subduct nor overturn: 6.324 × 10⁸ Pa, the ice V–VI–liquid triple point at 273.31 K
/// (IAPWS R14-08(2011), Table 1, read by the science check; Wagner, Riethmann, Feistel and Harvey
/// 2011's melting curves; science-p14-t24b-laws §1.1).
pub const HIGH_PRESSURE_ICE_PRESSURE: Pascals = Pascals::new(632.4e6);

/// The density at which a body's water is spread into its global-equivalent depth for its
/// regime: 1,000 kg m⁻³, liquid water's (science-p14-t24b-laws §1.1).
pub const REGIME_WATER_DENSITY: crate::units::KilogramsPerCubicMetre =
    crate::units::KilogramsPerCubicMetre::new(1_000.0);

/// What decides a body's tectonic regime (P14.T24.b; science-p14-t24b-laws §1.1): fixed functions
/// of age + t, none of which reads the residual heat, as plain values.
///
/// The fields carry no invariant the rule relies on, so they are public: each clause compares
/// them with its edges, and a NaN fails every comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegimeInputs {
    /// How its interior loses its heat ([`Interior::of`]).
    pub interior: Interior,
    /// Its mass.
    pub mass: EarthMasses,
    /// Its radiogenic heat flux before the residual, `q_rad` ([`radiogenic_heat_flow`]).
    pub radiogenic: WattsPerSquareMetre,
    /// The tidal heat flux raised in it, `q_tid`: zero for a body that goes round a star.
    pub tidal: WattsPerSquareMetre,
    /// Its surface state (P14.T13.c).
    pub state: SurfaceState,
    /// Its water's global-equivalent depth, `d_w`: its water's mass at [`REGIME_WATER_DENSITY`]
    /// over its surface.
    pub water_depth: Metres,
    /// Its surface gravity, the bulk section's.
    pub surface_gravity: MetresPerSecondSquared,
    /// The ice shell over its subsurface ocean, where a rule computes one; `None` for none, or
    /// for a body no rule computes.
    pub ocean_shell: Option<Metres>,
}

/// The tectonic regime of the body `inputs` describe (science-p14-t24b-laws §1.1): the first
/// clause that holds decides.
///
/// 1. [`TectonicRegime::IceShell`]: a computed subsurface ocean under a shell of at most
///    [`ICE_SHELL_LIMIT`].
/// 2. [`TectonicRegime::HeatPipe`]: a `MagmaOcean` surface, or a convecting body whose tidal heat
///    flux is at least [`HEAT_PIPE_TIDAL_FLUX`].
/// 3. [`TectonicRegime::StagnantLid`]: a conducting body.
/// 4. A lid that fails, where the mass is at least [`LID_FAILURE_MASS`], the radiogenic flux at
///    least [`LID_FAILURE_FLUX`], and the water's floor pressure, `ρ_w` g `d_w`, under
///    [`HIGH_PRESSURE_ICE_PRESSURE`]: [`TectonicRegime::MobileLid`] if the state is `Temperate`
///    or `Snowball` and the water at least [`PLATE_WATER_DEPTH`] deep, a water-weakened
///    lithosphere (Korenaga 2010), and [`TectonicRegime::Episodic`] otherwise, a dry lid that
///    has the heat and size to fail and fails in episodes (Moresi and Solomatov 1998, GJI 133,
///    669; Armann and Tackley 2012; Venus).
/// 5. [`TectonicRegime::StagnantLid`] otherwise, an ocean world floored by high-pressure ice
///    among them.
///
/// A regime change, as the production falls through the edge or the surface's water goes, is a
/// state change of the record. The heat flow steps only at the mobile lid's edge, by the ratio of
/// the two Urey ratios' forms ([`convecting_heat_flow`]).
///
/// # Examples
///
/// A dry Venus fails episodically where a wet Earth's plates move:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::SurfaceState;
/// use hyperion_sim::planetary::hooks::figures::interior::{Interior, RegimeInputs, tectonic_regime};
/// use hyperion_sim::planetary::hooks::figures::TectonicRegime;
/// use hyperion_sim::units::{EarthMasses, Metres, MetresPerSecondSquared, WattsPerSquareMetre};
///
/// let earth = RegimeInputs {
///     interior: Interior::Convecting,
///     mass: EarthMasses::new(1.0),
///     radiogenic: WattsPerSquareMetre::new(0.039),
///     tidal: WattsPerSquareMetre::new(0.0),
///     state: SurfaceState::Temperate,
///     water_depth: Metres::new(2.7e3),
///     surface_gravity: MetresPerSecondSquared::new(9.8),
///     ocean_shell: None,
/// };
/// assert_eq!(tectonic_regime(&earth), TectonicRegime::MobileLid);
/// let venus = RegimeInputs { state: SurfaceState::RunawayGreenhouse, water_depth: Metres::new(0.0), ..earth };
/// assert_eq!(tectonic_regime(&venus), TectonicRegime::Episodic);
/// ```
#[must_use]
pub fn tectonic_regime(inputs: &RegimeInputs) -> TectonicRegime {
    if inputs
        .ocean_shell
        .is_some_and(|shell| shell <= ICE_SHELL_LIMIT)
    {
        return TectonicRegime::IceShell;
    }
    let convecting = match inputs.interior {
        Interior::Convecting => true,
        Interior::Conducting => false,
    };
    if inputs.state == SurfaceState::MagmaOcean
        || (convecting && inputs.tidal >= HEAT_PIPE_TIDAL_FLUX)
    {
        return TectonicRegime::HeatPipe;
    }
    if !convecting {
        return TectonicRegime::StagnantLid;
    }
    let floor =
        REGIME_WATER_DENSITY.value() * inputs.surface_gravity.value() * inputs.water_depth.value();
    let fails = inputs.mass >= LID_FAILURE_MASS
        && inputs.radiogenic >= LID_FAILURE_FLUX
        && floor < HIGH_PRESSURE_ICE_PRESSURE.value();
    if !fails {
        return TectonicRegime::StagnantLid;
    }
    let holds_water = match inputs.state {
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse => false,
    };
    let wet = holds_water && inputs.water_depth >= PLATE_WATER_DEPTH;
    if wet {
        TectonicRegime::MobileLid
    } else {
        TectonicRegime::Episodic
    }
}

/// A body's global intrinsic magnetic field, from a dynamo (P14.T24.b; science-p14-t24b-laws
/// §1.2). Crustal remanence (Mars's, the Moon's) and induced fields (Europa's, Callisto's) are not
/// global intrinsic fields. Readers that need a magnetosphere's size read the class with the
/// host's wind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MagneticField {
    /// No dynamo: remanent or induced fields only.
    None,
    /// A dynamo whose surface dipole is under about 1 μT, as Mercury's 0.2–0.3 μT, or a
    /// multipolar field whose dipole is a tenth or less of a dipolar dynamo's.
    Weak,
    /// A dipolar dynamo of order 10 μT or more at the surface, as Earth's 30 μT and the giants'
    /// 14–420 μT.
    Strong,
}

/// The iron share of the solids, iron ÷ (iron + rock), under which a body has no core of note for
/// a dynamo: 0.1 (science-p14-t24b-laws §1.2). The Moon's core is 1–2% of its mass, and the
/// generated Moon and every capture are rock alone.
pub const DYNAMO_CORE_FRACTION: f64 = 0.1;

/// The iron share of the solids from which a core keeps a deep compositional dynamo under a stable
/// layer, a weak field even under a stagnant lid: 0.5 (science-p14-t24b-laws §1.2; Mercury's,
/// Christensen 2006, Nature 444, 1056; from memory).
pub const COMPOSITIONAL_DYNAMO_FRACTION: f64 = 0.5;

/// The mass from which a core of [`COMPOSITIONAL_DYNAMO_FRACTION`] keeps its dynamo: 0.03 M⊕
/// (science-p14-t24b-laws §1.2, the plan's own), between the Moon (0.012 M⊕), whose small core
/// froze and whose dynamo, alive at about 1–2.5 Ga (Tikoo et al. 2017, Sci. Adv. 3, e1700207),
/// ceased between about 1.92 and 0.80 Ga (Mighani et al. 2020, Sci. Adv. 6, eaax0883; both
/// abstracts read by the science check), and Mercury (0.055).
pub const COMPOSITIONAL_DYNAMO_MASS: EarthMasses = EarthMasses::new(0.03);

/// The age under which every convecting core with iron of note drives a thermal dynamo, its
/// initial superheat draining: 0.6 Gyr (science-p14-t24b-laws §1.2). Mars's dynamo ceased 4.0–4.1
/// Ga by its large craters' magnetisation (Lillis et al. 2013, JGR Planets 118, 1488), or after
/// 3.7 Ga by MAVEN's Lucus Planum fields (Mittelholz et al. 2020, Sci. Adv. 6, eaba0513; both
/// abstracts read by the science check): 0.47 Gyr or more after the Solar System formed (Williams
/// and Nimmo 2004, Geology 32, 97, not re-read).
pub const EARLY_DYNAMO_AGE: Gigayears = Gigayears::new(0.6);

/// The sidereal rotation period up to which a thermal dynamo is dipolar: 1.3 d, 112,320 s
/// (science-p14-t24b-laws §1.2). Christensen and Aubert 2006 (GJI 166, 97, §3.1, read by the ruling
/// and the science check) find dynamos dipolar below a local Rossby number of 0.12 "irrespective of
/// the values of the Ekman number, Prandtl number and magnetic Prandtl number"; Earth's is about
/// 0.09 (Olson and Christensen 2006, from memory), and at a fixed buoyancy flux it scales close to
/// the inverse rotation rate, so 0.12 is reached at about 1.3 times Earth's period. The core's flux
/// and size, which also move it, are not modelled.
pub const DIPOLAR_ROTATION_PERIOD: Seconds = Seconds::new(112_320.0);

/// What decides a body's magnetic field (science-p14-t24b-laws §1.2), as plain values.
///
/// The fields carry no invariant the rule relies on, so they are public: the class alone decides
/// a gas envelope's field, whatever the others hold, and each other clause compares them with its
/// edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldInputs {
    /// Its class: a gas envelope's dynamo is its metallic hydrogen's or its water mantle's.
    pub class: PlanetClass,
    /// How its interior loses its heat ([`Interior::of`]); not read for a gas envelope.
    pub interior: Interior,
    /// Its mass.
    pub mass: EarthMasses,
    /// The iron share of its solids, iron ÷ (iron + rock).
    pub iron_share: f64,
    /// Its tectonic regime ([`tectonic_regime`]); not read for a gas envelope.
    pub regime: TectonicRegime,
    /// Its radiogenic heat flux before the residual.
    pub radiogenic: WattsPerSquareMetre,
    /// The tidal heat flux raised in it.
    pub tidal: WattsPerSquareMetre,
    /// Its age.
    pub age: Years,
    /// Its sidereal rotation period at the time, s, positive, infinite for a body that does not
    /// turn (P14.T14's, the orbital period for a 1:1 lock).
    pub rotation_period: Seconds,
}

/// The magnetic field of the body `inputs` describe (science-p14-t24b-laws §1.2): the first
/// clause that holds decides.
///
/// 1. A gas envelope (`SubNeptune`, `IceGiant`, `GasGiant`): [`MagneticField::Strong`], its
///    metallic hydrogen or ionic water convecting with the giant's internal heat (Stevenson 2003,
///    EPSL 208, 1; from memory); Uranus's and Neptune's multipolar fields are 14–23 μT at their
///    surfaces, and a sub-Neptune's is an extrapolation.
/// 2. A conducting interior, whose core does not cool, or an iron share under
///    [`DYNAMO_CORE_FRACTION`]: [`MagneticField::None`].
/// 3. A tidal heat flux at least the radiogenic: [`MagneticField::None`], since a mantle heated
///    from within keeps its core from cooling (Io; Wienbruch and Spohn 1995, PSS 43, 1045; from
///    memory).
/// 4. A thermal dynamo, under a [`TectonicRegime::MobileLid`], which cools the core efficiently
///    (Stevenson, Spohn and Schubert 1983; Nimmo and Stevenson 2000; Nimmo 2002 for Venus; from
///    memory), or at an age under [`EARLY_DYNAMO_AGE`]: [`MagneticField::Strong`] at a sidereal
///    rotation period of at most [`DIPOLAR_ROTATION_PERIOD`], and [`MagneticField::Weak`], a
///    multipolar field, when slower.
/// 5. An iron share of at least [`COMPOSITIONAL_DYNAMO_FRACTION`] on a body of at least
///    [`COMPOSITIONAL_DYNAMO_MASS`]: [`MagneticField::Weak`], a dynamo under a stable layer
///    (Mercury's).
/// 6. [`MagneticField::None`] otherwise.
///
/// Ganymede's intrinsic 720 nT is not reproduced, a recorded miss: its dynamo's source is
/// unresolved (Bland, Showman and Tobie 2008, Icarus 198, 384; from memory), and its near twins in
/// the generator's inputs, Callisto and Titan, have none.
///
/// # Examples
///
/// Earth's dipole is strong; turning once in ten days, the same dynamo would be multipolar:
///
/// ```
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::hooks::figures::interior::{FieldInputs, Interior, magnetic_field};
/// use hyperion_sim::planetary::hooks::figures::{MagneticField, TectonicRegime};
/// use hyperion_sim::units::consts::SECONDS_PER_DAY;
/// use hyperion_sim::units::{EarthMasses, Seconds, WattsPerSquareMetre, Years};
///
/// let earth = FieldInputs {
///     class: PlanetClass::Rocky,
///     interior: Interior::Convecting,
///     mass: EarthMasses::new(1.0),
///     iron_share: 0.325,
///     regime: TectonicRegime::MobileLid,
///     radiogenic: WattsPerSquareMetre::new(0.039),
///     tidal: WattsPerSquareMetre::new(0.0),
///     age: Years::new(4.57e9),
///     rotation_period: Seconds::new(86_164.1),
/// };
/// assert_eq!(magnetic_field(&earth), MagneticField::Strong);
/// let slow = FieldInputs { rotation_period: Seconds::new(10.0 * SECONDS_PER_DAY), ..earth };
/// assert_eq!(magnetic_field(&slow), MagneticField::Weak);
/// ```
#[must_use]
pub fn magnetic_field(inputs: &FieldInputs) -> MagneticField {
    match inputs.class {
        PlanetClass::SubNeptune | PlanetClass::IceGiant | PlanetClass::GasGiant => {
            return MagneticField::Strong;
        }
        PlanetClass::Rocky | PlanetClass::Icy => {}
    }
    let convecting = match inputs.interior {
        Interior::Convecting => true,
        Interior::Conducting => false,
    };
    if !convecting || inputs.iron_share < DYNAMO_CORE_FRACTION {
        return MagneticField::None;
    }
    // A mantle heated by its tides; a body with neither tide nor rock is not one.
    if inputs.tidal.value() > 0.0 && inputs.tidal >= inputs.radiogenic {
        return MagneticField::None;
    }
    let early = inputs.age.value() < Years::from(EARLY_DYNAMO_AGE).value();
    if inputs.regime == TectonicRegime::MobileLid || early {
        return if inputs.rotation_period <= DIPOLAR_ROTATION_PERIOD {
            MagneticField::Strong
        } else {
            MagneticField::Weak
        };
    }
    if inputs.iron_share >= COMPOSITIONAL_DYNAMO_FRACTION
        && inputs.mass >= COMPOSITIONAL_DYNAMO_MASS
    {
        MagneticField::Weak
    } else {
        MagneticField::None
    }
}

/// The heat flow of the generator's Earth, F⊕, W m⁻²: the Solar System table's Earth's (its own
/// rock and radius at 4.57 Gyr under its mobile lid, [`convecting_heat_flow`]), the unit of the
/// present outgassing ([`outgassing`]; P14.T24.g: "so Earth's O is 1"). `Fitted`: a test refits
/// it.
///
/// It is 0.0925 W m⁻², where Davies and Davies 2010 measure Earth's mean as 0.0916 (Solid Earth 1,
/// 5, Table 7).
pub const GENERATED_EARTH_HEAT_FLOW: WattsPerSquareMetre =
    WattsPerSquareMetre::new(0.092_547_910_476);

/// The present volcanic outgassing of a body of heat flow `heat_flow`, relative to Earth's:
/// O = max(0, (F − `F_c`) ÷ (F⊕ − `F_c`)), with `F_c` the heat flow below which no melt reaches
/// the surface ([`SURFACE_MELT_HEAT_FLOW`]) and F⊕ the generator's Earth's
/// ([`GENERATED_EARTH_HEAT_FLOW`]; P14.T24.g), so that Earth's is 1.
///
/// No regime factor is added: only Earth and Venus, one of each lid, constrain the sulphur per
/// unit of heat flow. Zero for a heat flow that is not a number.
#[must_use]
pub fn outgassing(heat_flow: WattsPerSquareMetre) -> f64 {
    let (f, threshold) = (heat_flow.value(), SURFACE_MELT_HEAT_FLOW.value());
    ((f - threshold) / (GENERATED_EARTH_HEAT_FLOW.value() - threshold)).max(0.0)
}
