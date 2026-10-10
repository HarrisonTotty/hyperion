//! The phase column: where a substance condenses, and in which phase it lies at a temperature and
//! pressure (decision-composition §1.1; P14.T49.a).
//!
//! A condensable row carries its triple and critical points, the enthalpies of sublimation and
//! vaporisation its curve takes below and above the triple point, and its saturated liquid's
//! density there ([`PhaseColumns`]). Methane's enthalpies are its triple point's; the four older
//! sets hold P14.T13.c's effective values, each source naming the reference value beside it.
//! Its saturation vapour pressure is Clausius–Clapeyron from the triple point, with the enthalpy
//! of the branch below or above it: the four constant sets P14.T13.c fixed for water, carbon
//! dioxide, nitrogen and argon, moved here verbatim so that no saturation pressure moves, and
//! methane's, for P14.T24.f. P14.T49.b replaces each with a fit to its substance's reference
//! equation of state, a deliberate change of output at the 22 → 23 bump.
//!
//! The phase diagram this gives ([`PhaseColumns::phase_at`]) is the saturation curve's:
//!
//! - at or above the critical temperature, supercritical at or above the critical pressure and
//!   vapour below it;
//! - below it, vapour below the saturation pressure, and at or above it solid below the triple
//!   temperature and liquid above;
//!
//! so the melting line is taken as vertical at the triple temperature. Real melting lines lean:
//! water's melting point falls by about 0.07 K per megapascal near its triple point (IAPWS's ice
//! Ih melting curve, Wagner, Riethmann, Feistel and Harvey 2011, JPCRD 40, 043103), under 1 K at
//! Venus's 92 bar, and the high-pressure ices are not modelled.
//! A constant enthalpy also leaves the liquid branch's end off the critical point: water's curve
//! passes the critical pressure at 604 K and methane's at 187 K, below their critical
//! temperatures, so between there and the critical temperature a fluid above the critical pressure
//! but below the curve reads vapour, while carbon dioxide's, nitrogen's and argon's curves end at
//! 81%, 74% and 85% of their critical pressures. Both go with P14.T49.b's fitted curves.

use super::{Source, SubstanceId};
use crate::math;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The molar gas constant, J mol⁻¹ K⁻¹ (exact in the 2019 SI, the Avogadro constant times
/// Boltzmann's: 8.314 462 618 153 24).
pub const MOLAR_GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// The phase a substance lies in at a temperature and pressure ([`phase_at`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// The solid: an ice, a frost or a crystal.
    Solid,
    /// The liquid.
    Liquid,
    /// The vapour, below the saturation pressure or above the critical temperature and below the
    /// critical pressure: the gas.
    Vapour,
    /// The supercritical fluid, at or above both the critical temperature and the critical
    /// pressure, which has no interface (Venus's carbon dioxide).
    Supercritical,
}

/// A condensable substance's phase data: the constants its saturation pressure and phase diagram
/// are drawn from, each named with its sources in [`sources`](Self::sources).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseColumns {
    pub(super) triple_temperature: Kelvin,
    pub(super) triple_pressure: Pascals,
    pub(super) critical_temperature: Kelvin,
    pub(super) critical_pressure: Pascals,
    pub(super) sublimation_enthalpy_j_per_mol: f64,
    pub(super) vaporisation_enthalpy_j_per_mol: f64,
    pub(super) triple_liquid_density: KilogramsPerCubicMetre,
    pub(super) sources: &'static [Source],
}

impl PhaseColumns {
    /// The triple point's temperature.
    #[must_use]
    pub const fn triple_temperature(&self) -> Kelvin {
        self.triple_temperature
    }

    /// The triple point's pressure.
    #[must_use]
    pub const fn triple_pressure(&self) -> Pascals {
        self.triple_pressure
    }

    /// The critical temperature, above which the substance never condenses.
    #[must_use]
    pub const fn critical_temperature(&self) -> Kelvin {
        self.critical_temperature
    }

    /// The critical pressure, above which, at or above the critical temperature, the fluid is
    /// supercritical.
    #[must_use]
    pub const fn critical_pressure(&self) -> Pascals {
        self.critical_pressure
    }

    /// The enthalpy of sublimation the saturation curve takes below the triple point, J mol⁻¹.
    #[must_use]
    pub const fn sublimation_enthalpy_j_per_mol(&self) -> f64 {
        self.sublimation_enthalpy_j_per_mol
    }

    /// The enthalpy of vaporisation the saturation curve takes above the triple point, J mol⁻¹.
    #[must_use]
    pub const fn vaporisation_enthalpy_j_per_mol(&self) -> f64 {
        self.vaporisation_enthalpy_j_per_mol
    }

    /// The saturated liquid's density at the triple point, the one liquid density the registry
    /// holds until P14.T49.b's `ρ_l(T)`: what a body's liquid inventory occupies (P14.T24.b reads
    /// it for the ocean's volume).
    #[must_use]
    pub const fn triple_liquid_density(&self) -> KilogramsPerCubicMetre {
        self.triple_liquid_density
    }

    /// Where each of these values comes from.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }

    /// The saturation vapour pressure at `temperature`, by Clausius–Clapeyron from the triple
    /// point with the enthalpy of the phase below or above it:
    /// `p_t` exp(−(L ÷ R)(1 ÷ T − 1 ÷ `T_t`)). Infinite at and above the critical temperature,
    /// where the substance never condenses, and zero at or below 0 K and for a temperature that is
    /// not a number.
    ///
    /// This is P14.T13.c's private function, moved verbatim: its branches and its arithmetic are
    /// output (a test holds it to the old one bit for bit).
    #[must_use]
    pub fn saturation_pressure(&self, temperature: Kelvin) -> Pascals {
        let t = temperature.value();
        if t >= self.critical_temperature.value() {
            return Pascals::new(f64::INFINITY);
        }
        if t.is_nan() || t <= 0.0 {
            return Pascals::ZERO;
        }
        let enthalpy = if t < self.triple_temperature.value() {
            self.sublimation_enthalpy_j_per_mol
        } else {
            self.vaporisation_enthalpy_j_per_mol
        };
        Pascals::new(
            self.triple_pressure.value()
                * math::exp(
                    -(enthalpy / MOLAR_GAS_CONSTANT)
                        * (1.0 / t - 1.0 / self.triple_temperature.value()),
                ),
        )
    }

    /// The phase at `temperature` and `pressure`, on the saturation curve's diagram of the
    /// [module documentation](self): a point on the curve itself is condensed. `None` for a
    /// temperature or pressure that is negative or not a number.
    #[must_use]
    pub fn phase_at(&self, temperature: Kelvin, pressure: Pascals) -> Option<Phase> {
        let (t, p) = (temperature.value(), pressure.value());
        if t.is_nan() || p.is_nan() || t < 0.0 || p < 0.0 {
            return None;
        }
        if t >= self.critical_temperature.value() {
            return Some(if p >= self.critical_pressure.value() {
                Phase::Supercritical
            } else {
                Phase::Vapour
            });
        }
        if p < self.saturation_pressure(temperature).value() {
            return Some(Phase::Vapour);
        }
        Some(if t < self.triple_temperature.value() {
            Phase::Solid
        } else {
            Phase::Liquid
        })
    }
}

/// The saturation vapour pressure of the substance `id` at `temperature`
/// ([`PhaseColumns::saturation_pressure`]), or `None` for a row with no phase data.
///
/// # Examples
///
/// Water boils near 373 K under one atmosphere, and Mars's 600 Pa of carbon dioxide freezes out
/// onto its polar caps below about 148 K:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::saturation_pressure;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let water = saturation_pressure(Gas::Water.substance(), Kelvin::new(373.15))?;
/// assert!((water.value() / 101_325.0 - 1.0).abs() < 0.1);
/// let frost = saturation_pressure(Gas::CarbonDioxide.substance(), Kelvin::new(148.0))?;
/// assert!((500.0..800.0).contains(&frost.value()));
/// // Hydrogen holds no phase data yet.
/// assert_eq!(saturation_pressure(Gas::Hydrogen.substance(), Kelvin::new(20.0)), None);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn saturation_pressure(id: SubstanceId, temperature: Kelvin) -> Option<Pascals> {
    id.substance()
        .phase()
        .map(|phase| phase.saturation_pressure(temperature))
}

/// The phase of the substance `id` at `temperature` and `pressure` ([`PhaseColumns::phase_at`]),
/// or `None` for a row with no phase data, or a temperature or pressure that is negative or not a
/// number.
///
/// # Examples
///
/// Venus's carbon dioxide is supercritical at its surface, and Titan's methane can pool:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::{Phase, phase_at};
/// use hyperion_sim::units::{Kelvin, Pascals};
///
/// let venus = phase_at(Gas::CarbonDioxide.substance(), Kelvin::new(737.0), Pascals::new(9.2e6));
/// assert_eq!(venus, Some(Phase::Supercritical));
/// let titan = phase_at(Gas::Methane.substance(), Kelvin::new(93.65), Pascals::new(1.467e5));
/// assert_eq!(titan, Some(Phase::Liquid));
/// ```
#[must_use]
pub fn phase_at(id: SubstanceId, temperature: Kelvin, pressure: Pascals) -> Option<Phase> {
    id.substance()
        .phase()
        .and_then(|phase| phase.phase_at(temperature, pressure))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::atmosphere::Gas;
    use crate::substance::condensables;

    fn columns(id: SubstanceId) -> &'static PhaseColumns {
        id.substance()
            .phase()
            .expect("a condensable has phase data")
    }

    #[test]
    fn the_saturation_pressures_meet_their_triple_points() {
        for id in condensables() {
            let c = columns(id);
            let t = c.triple_temperature().value();
            let below = c.saturation_pressure(Kelvin::new(t * (1.0 - 1e-9))).value();
            let above = c.saturation_pressure(Kelvin::new(t)).value();
            let p_t = c.triple_pressure().value();
            assert!((below / p_t - 1.0).abs() < 1e-6, "{id:?}");
            assert!((above / p_t - 1.0).abs() < 1e-12, "{id:?}");
            assert!(
                c.saturation_pressure(c.critical_temperature())
                    .value()
                    .is_infinite()
            );
            assert_same_bits(c.saturation_pressure(Kelvin::ZERO).value(), 0.0);
            assert_same_bits(c.saturation_pressure(Kelvin::new(f64::NAN)).value(), 0.0);
        }
    }

    /// The Clausius–Clapeyron curves meet the boiling and sublimation points the reference
    /// equations give under one atmosphere: water at 373.124 K (Wagner and Pruss 2002), CO₂'s
    /// frost at 194.69 K (Span and Wagner 1996's 194.686 K) and methane at 111.67 K (Setzmann and
    /// Wagner 1991), all as NIST's Chemistry Web Book evaluates them.
    #[test]
    fn the_curves_meet_their_normal_boiling_and_frost_points() {
        let atmosphere = 101_325.0;
        for (id, t, within) in [
            (Gas::Water, 373.124, 0.05),
            (Gas::CarbonDioxide, 194.69, 0.15),
            (Gas::Methane, 111.67, 0.03),
        ] {
            let p = saturation_pressure(id.substance(), Kelvin::new(t))
                .unwrap()
                .value();
            assert!(
                (p / atmosphere - 1.0).abs() < within,
                "{id:?} at {t} K: {p} Pa"
            );
        }
    }

    #[test]
    fn each_condensable_has_its_four_regions() {
        for id in condensables() {
            let c = columns(id);
            let (t_t, p_t) = (c.triple_temperature(), c.triple_pressure());
            let (t_c, p_c) = (c.critical_temperature(), c.critical_pressure());
            let mid = Kelvin::new(f64::midpoint(t_t.value(), t_c.value()));
            let cold = Kelvin::new(0.8 * t_t.value());
            let hot = Kelvin::new(1.5 * t_c.value());
            let sat = |t: Kelvin| c.saturation_pressure(t);
            for (t, p, phase) in [
                (cold, p_t, Phase::Solid),
                (cold, sat(cold) * 0.5, Phase::Vapour),
                (mid, sat(mid) * 1.5, Phase::Liquid),
                (mid, sat(mid) * 0.5, Phase::Vapour),
                (hot, p_c * 2.0, Phase::Supercritical),
                (hot, p_c * 0.5, Phase::Vapour),
                (t_c, p_c, Phase::Supercritical),
            ] {
                assert_eq!(c.phase_at(t, p), Some(phase), "{id:?} at {t:?}, {p:?}");
                assert_eq!(phase_at(id, t, p), Some(phase));
            }
            // On the curve itself the substance is condensed.
            assert_eq!(c.phase_at(mid, sat(mid)), Some(Phase::Liquid));
            assert_eq!(c.phase_at(cold, sat(cold)), Some(Phase::Solid));
            for (t, p) in [
                (Kelvin::new(f64::NAN), p_t),
                (mid, Pascals::new(f64::NAN)),
                (Kelvin::new(-1.0), p_t),
                (mid, Pascals::new(-1.0)),
            ] {
                assert_eq!(c.phase_at(t, p), None);
            }
        }
    }

    /// The Solar System's surfaces and a laboratory's: Earth's seas and ice and the steam above
    /// them, Mars's carbon dioxide frost and air (600 Pa), Venus's supercritical air (737 K,
    /// 92 bar), Titan's methane lakes (93.65 K, 1.467 bar; Fulchignoni et al. 2005, Nature 438,
    /// 785), nitrogen frost on a 30 K nightside, liquid nitrogen at 70 K under one atmosphere, and
    /// nitrogen and argon as Earth's air.
    #[test]
    fn the_solar_system_s_surfaces_lie_in_their_phases() {
        let earth = Pascals::new(101_325.0);
        for (gas, t, p, phase) in [
            (Gas::Water, 288.0, earth, Phase::Liquid),
            (Gas::Water, 250.0, earth, Phase::Solid),
            (Gas::Water, 400.0, earth, Phase::Vapour),
            (Gas::CarbonDioxide, 140.0, Pascals::new(600.0), Phase::Solid),
            (
                Gas::CarbonDioxide,
                210.0,
                Pascals::new(600.0),
                Phase::Vapour,
            ),
            (
                Gas::CarbonDioxide,
                737.0,
                Pascals::new(9.2e6),
                Phase::Supercritical,
            ),
            (Gas::Methane, 93.65, Pascals::new(1.467e5), Phase::Liquid),
            (Gas::Nitrogen, 30.0, Pascals::new(1.0), Phase::Solid),
            (Gas::Nitrogen, 70.0, earth, Phase::Liquid),
            (Gas::Nitrogen, 288.0, earth, Phase::Vapour),
            (Gas::Argon, 288.0, earth, Phase::Vapour),
        ] {
            assert_eq!(
                phase_at(gas.substance(), Kelvin::new(t), p),
                Some(phase),
                "{gas:?} at {t} K, {p:?}"
            );
        }
    }

    #[test]
    fn a_row_without_phase_data_has_no_curve_and_no_phase() {
        for gas in [Gas::Hydrogen, Gas::Helium, Gas::Ammonia, Gas::Oxygen] {
            let id = gas.substance();
            assert_eq!(saturation_pressure(id, Kelvin::new(100.0)), None);
            assert_eq!(phase_at(id, Kelvin::new(100.0), Pascals::new(1e5)), None);
        }
    }
}
