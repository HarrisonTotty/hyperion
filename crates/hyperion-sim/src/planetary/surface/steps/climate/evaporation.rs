//! The precipitation heuristic's evaporation over open liquid (R09.T13.e; Design note 8,
//! decision-r09-t13ge §2.6, e6, as decision-r09-t13i §3.2 revises it): each sea cell's year
//! evaporates Priestley and Taylor's rate on its energy with no storage, the sea's share of its
//! absorbed sunlight and the heat the ocean brings it, over its open months; its months take the
//! shape of Priestley and Taylor's rate split as Penman's combination equation splits it.
//!
//! The source is Priestley, C. H. B., and Taylor, R. J. 1972, "On the Assessment of Surface Heat
//! Flux and Evaporation Using Large-Scale Parameters", Mon. Wea. Rev. 100, 81–92, read: the
//! evaporation of a horizontally uniform saturated surface in energy units is α s ÷ (s + γ)
//! (R − G), α = 1.26 (their eq. 14), s the slope of the saturation specific humidity with the
//! temperature and γ = `c_p` ÷ L (their text after eq. 3; α is eq. 5), R the net radiation and G
//! the heat into the ground, here the sea's storage. α is the actual rate over the equilibrium
//! rate s ÷ (s + γ) (R − G), and its excess α − 1 is the large-scale advection's term of Penman's
//! combination equation, the aerodynamic part (Penman, H. L. 1948, "Natural Evaporation from Open
//! Water, Bare Soil and Grass", Proc. R. Soc. Lond. A 193, 120–145, from memory, as
//! decision-r09-t13i §3.2 reads it).
//!
//! # A sea cell's year (decision-r09-t13i §3.2)
//!
//! Over its open months m, those whose water surface lies at or above the model's freezing point
//! (frozen months give none), with ⟨·⟩ their mean weighted by the months' shares `s_m`:
//!
//! ```text
//! c_PT,m = min(1, α s ÷ (s + γ))                 at the water's temperature T_w,m
//! c_eq,m = c_PT,m ÷ α
//! A_m    = β S_abs,m + Q_O,m                     the energy: net radiation and the ocean's heat
//! Ē      = max(0, ⟨c_PT,m A_m ÷ L⟩)              Priestley and Taylor's year, with no storage
//! ē_eq   = max(0, ⟨c_eq,m A_m ÷ L⟩)
//! D_m    = c_eq,m max(0, A_m − G_m) ÷ L + (α − 1) ē_eq q_s(T_w,m) ÷ ⟨q_s(T_w)⟩
//! E_m    = Ē D_m ÷ ⟨D⟩                           zero where ⟨D⟩ is zero
//! ```
//!
//! - β = 0.47, the ocean's net surface radiation over its absorbed sunlight
//!   ([`OCEAN_NET_RADIATION_SHARE`]): the model has no surface energy budget of its own, so R is
//!   that share of the sunlight the sea absorbs, `S_abs`, the month's insolation × (1 − albedo) at
//!   the water's surface from the model's own records (decision-r09-t13ge §2.11).
//! - `Q_O` the ocean's heat convergence per unit of the sea's area, the energy balance's own record
//!   of its Ekman cells and gyres (decision-r09-t13i §2.3): the heat the Gulf Stream's and
//!   Kuroshio's winter evaporation spends (their evaporation "exceeds 300 cm yr⁻¹ in the boreal
//!   winter": Yu, L. 2007, J. Climate 20, 5376, as the ruling read it), and the heat the equatorial
//!   upwelling takes.
//! - G = `C_sea` dT/dt the storage, the sea's heat capacity times the month's centred change of its
//!   temperature. It sets the months' shape and not the year's amount: the year's storage is zero.
//!   The equilibrium part carries the winter maximum, where the sea releases its heat (G < 0) to
//!   cold air; the aerodynamic part, spread by the vapour deficit (1 − RH) `q_s`, whose constant
//!   relative humidity cancels, keeps a summer evaporation where the storage takes all the
//!   radiation (over NCEP/NCAR R1's 40–60° N sea in summer the air heats the sea, and the sea still
//!   evaporates 22 W m⁻²; decision-r09-t13i §1.3).
//! - the clamp at 1: a wet surface in equilibrium cannot spend more than its available energy on
//!   evaporation; it binds above about 304 K for water and at Titan's 93.7 K for methane
//!   (decision-r09-t13ge §2.6).
//! - s from the saturation specific humidity `q_s` = ε `e_s` ÷ (p − (1 − ε) `e_s`) at the surface
//!   pressure, `e_s` the registry's saturation pressure of the condensable, by a centred difference
//!   of 0.01 K as the model takes h's slope, over the liquid's branch: within that step of the
//!   registry's triple point, where the difference would read the solid's branch below it, at a
//!   step above the triple point; where `e_s` reaches the surface pressure on either side the
//!   liquid boils, and the clamp holds.
//!
//! No new constant, no wind and no air temperature: the model has neither. A one-month year has no
//! storage, and takes Priestley and Taylor's rate on its energy directly, as built to the bit where
//! no ocean heat reaches it.
//!
//! None evaporates from frozen liquid (below the model's freezing point) or from any liquid but the
//! condensable's own: R09.T13.d's other liquids (a Titan's ethane) give the air no vapour of the
//! condensable.

use super::ebm::{ENERGY_SLOPE_STEP_K, Moisture};
use crate::substance::saturation_pressure;
use crate::units::{Kelvin, Pascals};

/// Priestley and Taylor's α, the equilibrium evaporation's multiplier over a saturated surface:
/// 1.26, "the overall mean (land and water)" of their sites (1972, §6 and eq. 14, read; 1.28 without
/// the fluxatron). Medium confidence for Earth's oceans; the same for every condensable, a figure
/// measured on water.
pub(super) const PRIESTLEY_TAYLOR_COEFFICIENT: f64 = 1.26;

/// β, the ocean's net surface radiation over the sunlight it absorbs, the share of the absorbed
/// sunlight the heuristic gives its evaporation: 0.47, Wild et al. 2015's (Clim. Dyn. 44, 3393,
/// abstract read) ocean net surface radiation, 117 W m⁻², over the ocean's absorbed sunlight at
/// the top of the air, about 245–250 W m⁻² (its surface 170 plus the air's ~77, from memory;
/// decision-r09-t13ge §2.6, e6, which amends the lane's global 0.44). It is the Earth's ocean's,
/// named apart from the orographic model's re-evaporation β ([`lfpm`](super::lfpm)); a Titan's haze
/// would want its own (decision-r09-t13ge §3, E2). Medium confidence on the Earth.
pub(super) const OCEAN_NET_RADIATION_SHARE: f64 = 0.47;

/// The equilibrium evaporation's coefficient min(1, α s ÷ (s + γ)) of `moisture` in an air of
/// specific heat `specific_heat_j_per_kg_k`, J kg⁻¹ K⁻¹, at the liquid's temperature `t_k`, K (the
/// [module documentation](self)): 1 where the liquid boils within the difference's step.
#[must_use]
pub(super) fn coefficient(moisture: &Moisture, specific_heat_j_per_kg_k: f64, t_k: f64) -> f64 {
    let p = moisture.surface_pressure_pa;
    let epsilon = moisture.mass_ratio;
    let saturated = |t: f64| -> Option<f64> {
        let e =
            saturation_pressure(moisture.condensable, Kelvin::new(t)).map_or(0.0, Pascals::value);
        // By comparison, so that an infinite pressure (above the critical point) boils.
        (e < p).then(|| epsilon * e / (p - (1.0 - epsilon) * e))
    };
    // The liquid's slope: within a step of the triple point the difference would read the solid's
    // branch below it, so it is taken a step above.
    let triple_k = moisture
        .condensable
        .substance()
        .phase()
        .map_or(0.0, |phase| phase.triple_temperature().value());
    let at_k = if t_k < triple_k + ENERGY_SLOPE_STEP_K {
        triple_k + ENERGY_SLOPE_STEP_K
    } else {
        t_k
    };
    let (Some(above), Some(below)) = (
        saturated(at_k + ENERGY_SLOPE_STEP_K),
        saturated(at_k - ENERGY_SLOPE_STEP_K),
    ) else {
        return 1.0;
    };
    let slope = (above - below) / (2.0 * ENERGY_SLOPE_STEP_K);
    let gamma = specific_heat_j_per_kg_k / moisture.latent_heat_j_per_kg;
    let equilibrium = PRIESTLEY_TAYLOR_COEFFICIENT * slope / (slope + gamma);
    if equilibrium < 1.0 { equilibrium } else { 1.0 }
}

/// Priestley and Taylor's rate, kg m⁻² s⁻¹, of an open liquid of coefficient `coefficient`
/// ([`coefficient`]) on the energy `energy_w_m2`, under the latent heat `latent_heat_j_per_kg`:
/// `coefficient` × max(0, A) ÷ L.
#[must_use]
pub(super) fn rate_kg_m2_s(coefficient: f64, energy_w_m2: f64, latent_heat_j_per_kg: f64) -> f64 {
    if energy_w_m2 > 0.0 {
        coefficient * energy_w_m2 / latent_heat_j_per_kg
    } else {
        0.0
    }
}

/// The energy an open liquid spends on its evaporation in a month, W m⁻²: A = β `S_abs` + `Q_O`,
/// the sea's share of the sunlight it absorbs, `absorbed_w_m2`, and the ocean's heat convergence
/// per unit of the sea's area, `ocean_w_m2` (the [module documentation](self)).
#[must_use]
pub(super) fn energy_w_m2(absorbed_w_m2: f64, ocean_w_m2: f64) -> f64 {
    OCEAN_NET_RADIATION_SHARE * absorbed_w_m2 + ocean_w_m2
}

/// One open month of a sea cell's year, as its evaporation reads it (the [module
/// documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct OpenMonth {
    /// The month's share of the year, `s_m`.
    pub(super) share: f64,
    /// `c_PT` = min(1, α s ÷ (s + γ)) at the water's temperature ([`coefficient`]).
    pub(super) coefficient: f64,
    /// A = β `S_abs` + `Q_O`, W m⁻² ([`energy_w_m2`]).
    pub(super) energy_w_m2: f64,
    /// G, the sea's storage, W m⁻².
    pub(super) storage_w_m2: f64,
    /// `q_s` at the water's temperature, the saturation specific humidity at the surface pressure.
    pub(super) saturation: f64,
}

/// A sea cell's evaporation in each month of its year, kg m⁻² s⁻¹, into `rates` (one a month),
/// from its open months `months` (one a month, `None` where its liquid is frozen), under the latent
/// heat `latent_heat_j_per_kg` (the [module documentation](self)): Priestley and Taylor's year with
/// no storage, Ē = max(0, ⟨`c_PT` A ÷ L⟩), in the Penman split's shape, `E_m` = Ē `D_m` ÷ ⟨D⟩. A
/// one-month year takes [`rate_kg_m2_s`] on its energy, to the bit; a frozen month and a year
/// with no open month or no energy, none. Every mean is summed in month order.
///
/// # Panics
///
/// If `rates` does not hold one value a month.
pub(super) fn year_rates_kg_m2_s(
    months: &[Option<OpenMonth>],
    latent_heat_j_per_kg: f64,
    rates: &mut [f64],
) {
    assert_eq!(rates.len(), months.len(), "one rate a month");
    rates.fill(0.0);
    if let [month] = months {
        if let Some(open) = month {
            rates[0] = rate_kg_m2_s(open.coefficient, open.energy_w_m2, latent_heat_j_per_kg);
        }
        return;
    }
    let (mut weight, mut year, mut equilibrium, mut saturation) = (0.0, 0.0, 0.0, 0.0);
    for open in months.iter().flatten() {
        weight += open.share;
        year += open.share * open.coefficient * open.energy_w_m2;
        equilibrium +=
            open.share * (open.coefficient / PRIESTLEY_TAYLOR_COEFFICIENT) * open.energy_w_m2;
        saturation += open.share * open.saturation;
    }
    // By comparison, so that no open month, no energy (and a NaN) evaporate nothing.
    let positive = |value: f64| value > 0.0;
    if !positive(weight) {
        return;
    }
    let year = year / weight / latent_heat_j_per_kg;
    if !positive(year) {
        return;
    }
    let equilibrium = equilibrium / weight / latent_heat_j_per_kg;
    let equilibrium = if positive(equilibrium) {
        equilibrium
    } else {
        0.0
    };
    let saturation = saturation / weight;
    let mut shape = 0.0;
    for (rate, open) in rates.iter_mut().zip(months) {
        let Some(open) = open else {
            continue;
        };
        let spare = open.energy_w_m2 - open.storage_w_m2;
        let radiative = (open.coefficient / PRIESTLEY_TAYLOR_COEFFICIENT)
            * if spare > 0.0 { spare } else { 0.0 }
            / latent_heat_j_per_kg;
        // Where no open month holds any vapour the deficit is spread evenly.
        let deficit = if saturation > 0.0 {
            open.saturation / saturation
        } else {
            1.0
        };
        *rate = radiative + (PRIESTLEY_TAYLOR_COEFFICIENT - 1.0) * equilibrium * deficit;
        shape += open.share * *rate;
    }
    let shape = shape / weight;
    if !positive(shape) {
        rates.fill(0.0);
        return;
    }
    for rate in rates.iter_mut() {
        *rate = year * *rate / shape;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::hooks::surface::EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
    use crate::planetary::surface::reference;
    use crate::planetary::surface::steps::climate::ebm::EnergyBalance;

    /// Over the reference Earth's water, α s ÷ (s + γ) is the lane's exact form's (scratch
    /// `e/pt-ratios.txt`, Murray's saturation curve at 1,013 hPa, `c_p` 1,005 and L 2.465 MJ kg⁻¹):
    /// 0.511 at 273.15 K, 0.837 at 291.15 K and 0.994 at 303.15 K, to within 0.015, the difference
    /// of the registry's constant-enthalpy curve, its L of 2.415 MJ kg⁻¹, the moist air's `c_p` and
    /// the reference Earth's 985.5 hPa (0.498, 0.825 and 0.987 as built; at the triple point the
    /// liquid's slope is taken); the clamp binds above about 304 K; the rate is the coefficient
    /// times the share of the sunlight plus the ocean's heat, and nothing where the ocean takes it all.
    #[test]
    fn the_coefficient_is_priestley_and_taylor_s_and_clamped_at_one() {
        let model = EnergyBalance::for_inputs(&reference::earth_like());
        let moisture = model.moisture.expect("the reference Earth is wet");
        let cp = model.specific_heat_j_per_kg_k;
        assert!(
            (cp - EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K).abs() < 60.0,
            "{cp}"
        );
        eprintln!(
            "c_p {cp:.2} J kg⁻¹ K⁻¹, L {:.4e} J kg⁻¹, ε {:.4}, p {} Pa, γ {:.4e} K⁻¹",
            moisture.latent_heat_j_per_kg,
            moisture.mass_ratio,
            moisture.surface_pressure_pa,
            cp / moisture.latent_heat_j_per_kg
        );
        for (t, expected) in [(273.15, 0.511), (291.15, 0.837), (303.15, 0.994)] {
            let c = coefficient(&moisture, cp, t);
            let e = saturation_pressure(moisture.condensable, Kelvin::new(t))
                .unwrap()
                .value();
            eprintln!("T {t} K: e_s {e:.2} Pa, α s ÷ (s + γ) {c:.4}, the lane's {expected}");
            assert!((c - expected).abs() < 0.015, "{t}: {c}");
        }
        hyperion_testkit::float::assert_same_bits(coefficient(&moisture, cp, 310.0), 1.0);
        hyperion_testkit::float::assert_same_bits(coefficient(&moisture, cp, 400.0), 1.0);
        let c = coefficient(&moisture, cp, 291.15);
        let energy = energy_w_m2(200.0, -10.0);
        let rate = rate_kg_m2_s(c, energy, moisture.latent_heat_j_per_kg);
        let expected = c * (0.47 * 200.0 - 10.0) / moisture.latent_heat_j_per_kg;
        assert!((rate - expected).abs() <= 1e-15 * expected);
        hyperion_testkit::float::assert_same_bits(
            rate_kg_m2_s(c, energy_w_m2(200.0, -95.0), moisture.latent_heat_j_per_kg),
            0.0,
        );
    }

    /// Earth's water's latent heat for the year's tests, J kg⁻¹ (any positive figure serves).
    const LATENT_J_PER_KG: f64 = 2.45e6;

    /// A year of twelve months of varying shares, coefficients, energies, storages and
    /// saturations, with two frozen months: the shares of the reference Earth's calendar, the
    /// energies of a midlatitude sea (positive in summer, negative in winter with the ocean's heat
    /// out of it), its storage in summer, its release in winter.
    fn varied_year() -> Vec<Option<OpenMonth>> {
        let days = [
            31.0, 28.25, 31.0, 30.0, 31.0, 30.0, 31.0, 31.0, 30.0, 31.0, 30.0, 31.0,
        ];
        let total: f64 = days.iter().sum();
        (0_u32..12)
            .zip(days)
            .map(|(m, d)| {
                let phase = f64::from(m) / 12.0 * 2.0 * core::f64::consts::PI;
                let summer = -crate::math::cos(phase);
                (m != 0 && m != 1).then(|| OpenMonth {
                    share: d / total,
                    coefficient: 0.7 + 0.2 * summer,
                    energy_w_m2: 60.0 + 90.0 * summer,
                    storage_w_m2: 150.0 * crate::math::sin(phase),
                    saturation: 0.008 + 0.004 * summer,
                })
            })
            .collect()
    }

    /// decision-r09-t13i §3.5, 2: a year's Σ `s_m` `E_m` over its open months is Σ `s_m` `c_PT` A ÷
    /// L to 10⁻¹² of it; its frozen months give exactly nothing.
    #[test]
    fn the_year_evaporates_priestley_and_taylor_s_amount_with_no_storage() {
        let months = varied_year();
        let mut rates = [0.0; 12];
        year_rates_kg_m2_s(&months, LATENT_J_PER_KG, &mut rates);
        let (mut evaporated, mut expected) = (0.0, 0.0);
        for (open, &rate) in months.iter().zip(&rates) {
            match open {
                Some(open) => {
                    evaporated += open.share * rate;
                    expected += open.share * open.coefficient * open.energy_w_m2 / LATENT_J_PER_KG;
                    assert!(rate >= 0.0, "{rate}");
                }
                None => hyperion_testkit::float::assert_same_bits(rate, 0.0),
            }
        }
        eprintln!(
            "the year: {evaporated:e} against {expected:e} kg m⁻² s⁻¹; by month {:?}",
            rates.map(|r| r * 86_400.0)
        );
        assert!(expected > 0.0);
        assert!(
            (evaporated - expected).abs() <= 1e-12 * expected,
            "{evaporated} against {expected}"
        );
        // A year with no energy at all evaporates nothing.
        let dark: Vec<Option<OpenMonth>> = months
            .iter()
            .map(|m| {
                m.map(|open| OpenMonth {
                    energy_w_m2: -10.0,
                    ..open
                })
            })
            .collect();
        year_rates_kg_m2_s(&dark, LATENT_J_PER_KG, &mut rates);
        for rate in rates {
            hyperion_testkit::float::assert_same_bits(rate, 0.0);
        }
    }

    /// decision-r09-t13i §3.5, 2: a one-month year is Priestley and Taylor's rate on its energy to
    /// the bit, and where no ocean heat reaches it the as-built rate on its sunlight to the bit; a
    /// frozen one evaporates nothing.
    #[test]
    fn a_one_month_year_is_priestley_and_taylor_s_rate_to_the_bit() {
        for (absorbed, ocean) in [(240.0, 0.0), (240.0, 35.0), (80.0, -20.0), (10.0, -30.0)] {
            let open = OpenMonth {
                share: 1.0,
                coefficient: 0.83,
                energy_w_m2: energy_w_m2(absorbed, ocean),
                storage_w_m2: 0.0,
                saturation: 0.01,
            };
            let mut rate = [f64::NAN];
            year_rates_kg_m2_s(&[Some(open)], LATENT_J_PER_KG, &mut rate);
            hyperion_testkit::float::assert_same_bits(
                rate[0],
                rate_kg_m2_s(0.83, energy_w_m2(absorbed, ocean), LATENT_J_PER_KG),
            );
            if ocean == 0.0 {
                // As built: the coefficient × max(0, β S_abs − G) ÷ L with no storage.
                let as_built =
                    0.83 * (OCEAN_NET_RADIATION_SHARE * absorbed - 0.0) / LATENT_J_PER_KG;
                hyperion_testkit::float::assert_same_bits(rate[0], as_built);
            }
        }
        let mut rate = [f64::NAN];
        year_rates_kg_m2_s(&[None], LATENT_J_PER_KG, &mut rate);
        hyperion_testkit::float::assert_same_bits(rate[0], 0.0);
    }

    /// decision-r09-t13i §3.5, 2: with no storage and no ocean heat, at a constant temperature and
    /// sunlight (one coefficient, energy and saturation), every open month is equal and is
    /// Priestley and Taylor's rate, to 10⁻¹²; frozen months give nothing. (With a sunlight that
    /// varies, the months differ from Priestley and Taylor's month by month, the aerodynamic part
    /// being spread by `q_s` and not by the energy, and their year is still its amount, the test
    /// above.)
    #[test]
    fn uniform_months_with_no_storage_each_evaporate_priestley_and_taylor_s_rate() {
        let pt = rate_kg_m2_s(0.8, energy_w_m2(200.0, 0.0), LATENT_J_PER_KG);
        for frozen in [None, Some(3), Some(9)] {
            let months: Vec<Option<OpenMonth>> = (0..12)
                .map(|m| {
                    (Some(m) != frozen).then_some(OpenMonth {
                        share: 1.0 / 12.0,
                        coefficient: 0.8,
                        energy_w_m2: energy_w_m2(200.0, 0.0),
                        storage_w_m2: 0.0,
                        saturation: 0.012,
                    })
                })
                .collect();
            let mut rates = [0.0; 12];
            year_rates_kg_m2_s(&months, LATENT_J_PER_KG, &mut rates);
            let first = rates[usize::from(frozen == Some(0))];
            for (m, &rate) in rates.iter().enumerate() {
                if Some(m) == frozen {
                    hyperion_testkit::float::assert_same_bits(rate, 0.0);
                } else {
                    hyperion_testkit::float::assert_same_bits(rate, first);
                    assert!((rate - pt).abs() <= 1e-12 * pt, "{rate} against {pt}");
                }
            }
        }
    }

    /// decision-r09-t13i §3.5, 2: a month whose storage exceeds its energy still evaporates its
    /// aerodynamic share, positive, (α − 1) `ē_eq` `q_s` ÷ ⟨`q_s`⟩ rescaled to the year.
    #[test]
    fn a_month_whose_storage_takes_all_its_energy_still_evaporates_its_aerodynamic_share() {
        let mut months = varied_year();
        let june = months[5].as_mut().expect("June is open");
        june.storage_w_m2 = june.energy_w_m2 + 50.0;
        let mut rates = [0.0; 12];
        year_rates_kg_m2_s(&months, LATENT_J_PER_KG, &mut rates);
        eprintln!(
            "June, its storage over its energy: {:e} kg m⁻² s⁻¹",
            rates[5]
        );
        assert!(rates[5] > 0.0, "{}", rates[5]);
        // Its rate is the aerodynamic part alone, rescaled: against a June with its storage at
        // its energy exactly, the same.
        let mut at = months.clone();
        let june = at[5].as_mut().expect("June is open");
        june.storage_w_m2 = june.energy_w_m2;
        let mut equal = [0.0; 12];
        year_rates_kg_m2_s(&at, LATENT_J_PER_KG, &mut equal);
        hyperion_testkit::float::assert_same_bits(rates[5], equal[5]);
    }
}
