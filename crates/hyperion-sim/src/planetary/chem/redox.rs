//! A body's one redox figure, ΔIW (P14.T50.a; decision-composition §1.2, item 5).
//!
//! ΔIW is the present upper mantle's oxygen fugacity relative to the iron–wüstite buffer at the
//! same temperature and pressure, in log₁₀ units: ΔIW = log f(O₂) − log f(O₂)\_IW. It is not core
//! formation's, which differs: Sossi et al. (2025, arXiv:2512.00373, Table 1, read) put Earth's
//! core formation at IW−2.20, where its upper mantle now lies near FMQ, some IW+3.6. A body with
//! no mantle, undifferentiated, takes the figure for its bulk rock. The crust's [`RedoxClass`] is
//! a banding of it, and the outgassing speciation (P14.T55.a) will read it, so that crust and air
//! agree.
//!
//! # The model
//!
//! Its deterministic mean ([`mean_delta_iw`]) is the sum of three terms, held at most at
//! [`OXIDISED_CEILING_ABOVE_FMQ`] above FMQ, and a drawn scatter is added ([`delta_iw`]):
//!
//! - **The formation zone**, against the host's soot and snow lines. Planetary building blocks are
//!   more reduced the closer to their star they formed (Sossi et al. 2025, Fig. 1a: Mercury's core
//!   formed at IW−5.4, Vesta's at IW−1.2). In u = log₁₀(`d_form` ÷ `d_snow`) the term runs linearly
//!   from [`SOOT_LINE_DELTA_IW`] at the soot line ([`soot_line`]) to [`SNOW_LINE_DELTA_IW`] at the
//!   snow line, and is held at each end beyond it.
//! - **The water fraction**: water oxidises iron (Fe + H₂O → `FeO` + H₂), so a body that accreted
//!   ice is more oxidised ([`WATER_OXIDATION`], reached at [`ICY_WATER_FRACTION`]).
//! - **The mass**, standing for a deep magma ocean's Fe³⁺: in one, Fe²⁺ disproportionates to Fe³⁺
//!   and metal at high pressure, and the metal's descent to the core raises the upper mantle's
//!   oxidation state (Armstrong et al. 2019, Science 365, 903, abstract read). It is why Earth's
//!   mantle is more oxidised than Mars's ([`MAGMA_OCEAN_OXIDATION`]).
//!
//! It is calibrated on the Solar System table, each body at its own distance about the zero-age
//! Sun, whose snow line lies at 2.26 au:
//!
//! - Mercury, IW−2.6 to −6.3 (McCubbin et al. 2012, GRL 39, L09202, abstract read) and −4.5 to
//!   −7.3 (Zolotov et al. 2013, JGR Planets 118, 138, as decision-composition §9.1's checkers read
//!   it): the model's −5.0;
//! - the Moon, IW+0.2 to −2.5 (Fogel and Rutherford 1995, GCA 59, 201; Wadhwa 2008, Rev. Mineral.
//!   Geochem. 68, 493; both as Renggli, Steenstra and Saal 2024, arXiv:2410.18599, cite them,
//!   read): the model's −1.5;
//! - Mars's mantle, close to IW (Wadhwa 2001, Science 291, 1527, abstract read): the model's +0.01;
//! - Earth's upper mantle, within ±2 log units of FMQ (Frost and McCammon 2008, Annu. Rev. Earth
//!   Planet. Sci. 36, 389, abstract read): the model's FMQ − 0.04.
//!
//! Every coefficient but the soot line's is the lane's, fixed by that table and provisional, as the
//! volatiles' are; the water term has no calibrator in it.

use crate::math;
use crate::planetary::params::ICY_WATER_FRACTION;
use crate::stellar::draws::UnitUniform;
use crate::substance::MOLAR_GAS_CONSTANT;
use crate::units::{Dex, EarthMasses, Kelvin, Metres};

/// The iron–wüstite buffer's oxygen fugacity at `temperature`, log₁₀ of bar: O'Neill (1988,
/// Am. Mineral. 73, 470, abstract, read) for Fe + "`FeO`", μ(O₂) = −551,159 + 269.404 T −
/// 16.9484 T ln T J mol⁻¹, measured over 1,184–1,450 K with a reference pressure of 1 bar, and
/// log f(O₂) = μ(O₂) ÷ (R T ln 10).
///
/// Outside 1,184–1,450 K it is the same equation extrapolated.
#[must_use]
pub fn iw_log_fugacity(temperature: Kelvin) -> Dex {
    let t = temperature.value();
    let mu = -551_159.0 + 269.404 * t - 16.948_4 * t * math::ln(t);
    Dex::new(mu / (MOLAR_GAS_CONSTANT * t * core::f64::consts::LN_10))
}

/// The fayalite–magnetite–quartz buffer's oxygen fugacity at `temperature`, log₁₀ of bar:
/// O'Neill (1987, Am. Mineral. 72, 67, abstract, read) for quartz–fayalite–magnetite,
/// μ(O₂) = −587,474 + 1,584.427 T − 203.3164 T ln T + 0.092710 T² J mol⁻¹, measured over
/// 900–1,420 K with a reference pressure of 1 bar.
///
/// Outside 900–1,420 K it is the same equation extrapolated.
#[must_use]
pub fn fmq_log_fugacity(temperature: Kelvin) -> Dex {
    let t = temperature.value();
    let mu = -587_474.0 + 1_584.427 * t - 203.316_4 * t * math::ln(t) + 0.092_710 * t * t;
    Dex::new(mu / (MOLAR_GAS_CONSTANT * t * core::f64::consts::LN_10))
}

/// The temperature at which the FMQ–IW offset is taken: 1,400 K (1,127 °C), inside both buffers'
/// measured ranges ([`iw_log_fugacity`], [`fmq_log_fugacity`]), at 1 bar.
///
/// The offset is 3.593 there. Extrapolated, it is 3.519 at 1,200 °C and 3.445 at 1,300 °C, where
/// plan 14 wrote "about 3.5 at 1,200–1,300 °C" from memory.
pub const BUFFER_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(1_400.0);

/// How far the FMQ buffer lies above IW, log₁₀ units, at [`BUFFER_REFERENCE_TEMPERATURE`] and
/// 1 bar: 3.593 (O'Neill 1987; 1988).
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::redox::fmq_above_iw;
///
/// assert!((fmq_above_iw().value() - 3.593).abs() < 1e-3);
/// ```
#[must_use]
pub fn fmq_above_iw() -> Dex {
    fmq_log_fugacity(BUFFER_REFERENCE_TEMPERATURE) - iw_log_fugacity(BUFFER_REFERENCE_TEMPERATURE)
}

/// The disc temperature at which refractory carbon is destroyed, the soot line's: 500 K (Li et al.
/// 2021, Sci. Adv. 7, eabd3632, read in full: the major refractory carbon carriers convert to CO
/// "near a characteristic temperature of ~500 K").
pub const SOOT_LINE_TEMPERATURE: Kelvin = Kelvin::new(500.0);

/// The disc temperature at the snow line: 170 K, the temperature to which Hayashi's (1981)
/// profile falls at [`disc`](crate::planetary::disc)'s snow line of 2.7 au × √(L ÷ L☉).
pub const SNOW_LINE_TEMPERATURE: Kelvin = Kelvin::new(170.0);

/// The soot line of a host whose snow line is `snow_line`: where Hayashi's profile, T ∝ r^−½,
/// reaches [`SOOT_LINE_TEMPERATURE`], at (170 ÷ 500)² = 0.1156 of the snow line's distance;
/// 0.261 au about the zero-age Sun.
///
/// This is the passive disc's line at the host's zero-age luminosity, which decides where things
/// formed (design note 6). Li et al. place the early, accreting disc's soot line beyond 1 au for
/// its first million years; this one marks only the reduced end of [`mean_delta_iw`]'s zone term.
#[must_use]
pub fn soot_line(snow_line: Metres) -> Metres {
    let ratio = SNOW_LINE_TEMPERATURE.value() / SOOT_LINE_TEMPERATURE.value();
    snow_line * (ratio * ratio)
}

/// ΔIW of a body formed at the soot line or inside it, the most reduced the zone term gives: −6.5.
///
/// It is that of a gas of solar composition at 1,300 K and 10⁻⁴ bar (Grossman et al. 2008, Rev.
/// Mineral. Geochem. 68, 93, as Sossi et al. 2025 cite it beside their Fig. 1, read). Inside the
/// soot line its solids formed in equilibrium with the nebula's own hydrogen.
pub const SOOT_LINE_DELTA_IW: Dex = Dex::new(-6.5);

/// ΔIW of a dry body formed at the snow line or beyond it: +1.45.
///
/// It is fixed so that Mars, at 0.818 of the way from the zero-age Sun's soot line to its snow line
/// in log distance, lies at IW (the lane's, provisional). Mercury, at 0.183, then lies at IW−5.0,
/// inside both of its cited ranges.
pub const SNOW_LINE_DELTA_IW: Dex = Dex::new(1.45);

/// How much more oxidised a body is for the ice it accreted, log₁₀ units, reached at
/// [`ICY_WATER_FRACTION`] of water and held above it: 2.0, so that an icy body formed beyond the
/// snow line lies near FMQ, as the magnetite and Fe³⁺-bearing clays of the carbonaceous chondrites
/// that accreted ice suggest (the lane's, provisional: the table has no icy body whose mantle's
/// ΔIW is measured).
pub const WATER_OXIDATION: Dex = Dex::new(2.0);

/// The oxidation a deep magma ocean gives an Earth-mass body's upper mantle, log₁₀ units: 5.1,
/// fixed so that Earth lies at FMQ (the lane's, provisional; Armstrong et al. 2019).
pub const MAGMA_OCEAN_OXIDATION: Dex = Dex::new(5.1);

/// The masses between which the magma-ocean term rises from none to [`MAGMA_OCEAN_OXIDATION`].
///
/// They are Mars's, 0.107 M⊕, whose mantle lies near IW, and Earth's (the lane's, provisional),
/// and the term rises linearly in log mass between them. It is held above Earth's mass, so that a
/// super-Earth is no more oxidised than Earth by this term.
pub const MAGMA_OCEAN_MASSES: (EarthMasses, EarthMasses) =
    (EarthMasses::new(0.107), EarthMasses::new(1.0));

/// The most oxidised mean ΔIW, log₁₀ units above FMQ: 2, the top of Earth's upper mantle's range.
///
/// Frost and McCammon (2008) put Earth's upper mantle within ±2 of FMQ. The three terms' sum is
/// held to FMQ + 2, so that an Earth-mass body that accreted ice is no more oxidised than the most
/// oxidised terrestrial mantle.
pub const OXIDISED_CEILING_ABOVE_FMQ: Dex = Dex::new(2.0);

/// The scatter of ΔIW about its mean, log₁₀ units at one sigma: 0.5.
///
/// It is the lane's, provisional, the volatiles' scatter. Each table body's cited range is some
/// 2–3 units wide.
pub const DELTA_IW_SCATTER: Dex = Dex::new(0.5);

/// Everything [`mean_delta_iw`] reads of a body, as plain values.
///
/// The fields are public, as the volatiles' draws are. [`mean_delta_iw`] requires both distances
/// positive and finite and the water fraction and mass finite, and checks them in debug builds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RedoxInputs {
    /// Where the body formed from its host: its semi-major axis, unless it migrated.
    pub formation_distance: Metres,
    /// The host's snow line ([`disc::snow_line`](crate::planetary::disc::snow_line)).
    pub snow_line: Metres,
    /// The body's bulk water as a fraction of its mass, 0–1
    /// ([`MassFractions::water`](crate::planetary::derive::MassFractions::water)).
    pub water_fraction: f64,
    /// The body's mass.
    pub mass: EarthMasses,
}

/// The zone term of [`mean_delta_iw`]: [`SOOT_LINE_DELTA_IW`] to [`SNOW_LINE_DELTA_IW`], linear in
/// log₁₀(`d_form` ÷ `d_snow`) from the soot line to the snow line and held beyond each.
#[must_use]
fn zone_term(formation_distance: Metres, snow_line: Metres) -> f64 {
    let u = math::log10(formation_distance / snow_line);
    let u_soot = math::log10(soot_line(snow_line) / snow_line);
    let share = ((u - u_soot) / -u_soot).clamp(0.0, 1.0);
    SOOT_LINE_DELTA_IW.value() + (SNOW_LINE_DELTA_IW.value() - SOOT_LINE_DELTA_IW.value()) * share
}

/// The water term of [`mean_delta_iw`]: [`WATER_OXIDATION`] × min(`f_w` ÷
/// [`ICY_WATER_FRACTION`], 1).
#[must_use]
fn water_term(water_fraction: f64) -> f64 {
    let share = if water_fraction > 0.0 {
        water_fraction / ICY_WATER_FRACTION
    } else {
        0.0
    };
    WATER_OXIDATION.value() * if share < 1.0 { share } else { 1.0 }
}

/// The mass term of [`mean_delta_iw`]: [`MAGMA_OCEAN_OXIDATION`], scaled from 0 at the lower of
/// [`MAGMA_OCEAN_MASSES`] to 1 at the upper, linearly in log mass, and held outside them; none for
/// a body of no mass.
#[must_use]
fn mass_term(mass: EarthMasses) -> f64 {
    if mass.value().is_nan() || mass.value() <= 0.0 {
        return 0.0;
    }
    let (low, high) = MAGMA_OCEAN_MASSES;
    let share = math::log10(mass / low) / math::log10(high / low);
    MAGMA_OCEAN_OXIDATION.value() * share.clamp(0.0, 1.0)
}

/// The deterministic mean of a body's ΔIW, log₁₀ units (P14.T50.a): the zone, water and mass terms
/// of the module's model, summed in that order and held at most at FMQ +
/// [`OXIDISED_CEILING_ABOVE_FMQ`].
///
/// It is continuous in each input: every term is a clamped linear function, and so is the
/// ceiling.
///
/// # Panics
///
/// In debug builds, if a distance is not positive and finite, or the water fraction or the mass
/// is not finite.
///
/// # Examples
///
/// An Earth at Earth's distance from a Sun-like host lies near FMQ, and a Mercury-mass body at
/// Mercury's is strongly reduced:
///
/// ```
/// use hyperion_sim::planetary::chem::redox::{RedoxClass, RedoxInputs, fmq_above_iw, mean_delta_iw};
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{EarthMasses, Metres};
///
/// let snow_line = Metres::new(2.26 * METRES_PER_AU);
/// let at = |au: f64, mass: f64| RedoxInputs {
///     formation_distance: Metres::new(au * METRES_PER_AU),
///     snow_line,
///     water_fraction: 0.0,
///     mass: EarthMasses::new(mass),
/// };
/// let earth = mean_delta_iw(&at(1.0, 1.0));
/// assert!((earth - fmq_above_iw()).value().abs() < 0.1);
/// assert_eq!(RedoxClass::of(mean_delta_iw(&at(0.387, 0.055))), RedoxClass::Reduced);
/// ```
#[must_use]
pub fn mean_delta_iw(inputs: &RedoxInputs) -> Dex {
    let (d, snow) = (inputs.formation_distance.value(), inputs.snow_line.value());
    debug_assert!(
        d.is_finite() && d > 0.0 && snow.is_finite() && snow > 0.0,
        "a formation distance and a snow line are positive and finite, got {d} and {snow} m"
    );
    debug_assert!(
        inputs.water_fraction.is_finite() && inputs.mass.value().is_finite(),
        "a water fraction and a mass are finite"
    );
    let sum = zone_term(inputs.formation_distance, inputs.snow_line)
        + water_term(inputs.water_fraction)
        + mass_term(inputs.mass);
    let ceiling = fmq_above_iw().value() + OXIDISED_CEILING_ABOVE_FMQ.value();
    // A comparison rather than `min`, so that a NaN sum stays NaN rather than becoming the ceiling.
    Dex::new(if sum > ceiling { ceiling } else { sum })
}

/// A body's ΔIW, log₁₀ units (P14.T50.a): its [`mean_delta_iw`] plus a drawn scatter.
///
/// The scatter is [`DELTA_IW_SCATTER`] times the standard normal at `rank`, the body's redox rank
/// ([`VolatileDraws::redox`](crate::planetary::derive::atmosphere::VolatileDraws::redox), word 5
/// of `planet.volatiles`). At the median rank it is the mean, to the bit.
///
/// # Panics
///
/// As [`mean_delta_iw`], in debug builds.
#[must_use]
pub fn delta_iw(inputs: &RedoxInputs, rank: UnitUniform) -> Dex {
    mean_delta_iw(inputs) + DELTA_IW_SCATTER * math::normal_quantile(rank.value())
}

/// ΔIW below which a mantle is [`RedoxClass::Reduced`]: −3, below which lie the conditions
/// inferred for Mercury, the enstatite chondrites and the aubrite parent body (Keil 2010, Chem.
/// Erde 70, 295, as Renggli et al. 2024, §"Sulfides on Mercury", read, cite it: "very reducing
/// conditions below IW-3").
pub const REDUCED_BELOW: Dex = Dex::new(-3.0);

/// How far below FMQ a mantle becomes [`RedoxClass::Oxidised`], log₁₀ units: 2, the bottom of
/// Earth's upper mantle's range (Frost and McCammon 2008), so that every terrestrial upper mantle
/// they measure is oxidised: IW + 1.59 at [`BUFFER_REFERENCE_TEMPERATURE`].
pub const OXIDISED_FROM_BELOW_FMQ: Dex = Dex::new(2.0);

/// A banding of ΔIW, the crust's redox class (P14.T50.a).
///
/// Its edges are cited here and read by P14.T51.c's crust and chondrite choice. It is a closed
/// physics class, pinned with every variant (plan 14's Risks, "Composition").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RedoxClass {
    /// Below [`REDUCED_BELOW`]: Mercury, the enstatite chondrites and the aubrites.
    Reduced,
    /// Between the two edges: the Moon, Mars's mantle and the ordinary chondrites' range.
    Intermediate,
    /// From FMQ − [`OXIDISED_FROM_BELOW_FMQ`] up: Earth's upper mantle and the carbonaceous and
    /// R chondrites' oxidised range.
    Oxidised,
}

impl RedoxClass {
    /// Every class, most reduced first.
    pub const ALL: [Self; 3] = [Self::Reduced, Self::Intermediate, Self::Oxidised];

    /// The class of a mantle at `delta_iw`.
    #[must_use]
    pub fn of(delta_iw: Dex) -> Self {
        let oxidised_from = fmq_above_iw() - OXIDISED_FROM_BELOW_FMQ;
        if delta_iw.value() < REDUCED_BELOW.value() {
            Self::Reduced
        } else if delta_iw.value() < oxidised_from.value() {
            Self::Intermediate
        } else {
            Self::Oxidised
        }
    }
}

#[cfg(test)]
mod tests;
