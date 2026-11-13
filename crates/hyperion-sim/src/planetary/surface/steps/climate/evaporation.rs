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
//! rate s ÷ (s + γ) (R − G), 1.26 for saturated surfaces in advection-free data (their p. 91), and
//! its excess α − 1 stands for the aerodynamic, vapour-deficit term of the combination equation,
//! LE = s ÷ (s + γ) (R − G) + γ ÷ (s + γ) h (`q_s` − q) (their eq. 7a), the form of Penman, H. L.
//! 1948, "Natural Evaporation from Open Water, Bare Soil and Grass", Proc. R. Soc. Lond. A 193,
//! 120–145 (decision-r09-t13i §3.2; Penman from memory).
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
//!   cold air. The aerodynamic part keeps a summer evaporation where the storage takes all the
//!   radiation (over NCEP/NCAR R1's 40–60° N sea in summer the air heats the sea, and the sea still
//!   evaporates 22 W m⁻²; decision-r09-t13i §1.3). It is spread by the ruling's stand-in for the
//!   vapour deficit, (1 − RH) `q_s` with the air at the water's temperature, whose constant
//!   relative humidity cancels. The observed deficit peaks in winter instead (NCEP R1's 40–60° N
//!   sea: 1.28 g/kg in DJF against 0.51 in JJA, §1.3), so here the winter maximum comes from the
//!   storage's release, not from the deficit.
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
//!
//! # The month terms, shared within a climate cell (R09.T16.a)
//!
//! A month's coefficient, storage and `q_s` read only the climate cell's anomalies and the
//! liquid's temperature, the climate cell's sea-level temperature at the liquid's surface plus the
//! month's anomaly ([`month_terms`]). The water cycle's sea rates and the lakes' net loss ask at
//! every field cell in cell-index order, and a climate cell's four field cells are consecutive,
//! so a loop keeps each climate cell's terms by the surface temperatures asked
//! ([`ClimateCellTerms`]) and computes them once for the field cells of one climate cell at one
//! surface temperature, to the bit. A sea's cells stand at the sea level, so under the sea three
//! of four field cells ask again. Each cell's energy and ice share and every sum stay its own, in
//! their order.

use super::ebm::{ENERGY_SLOPE_STEP_K, EnergyBalance, Moisture};
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
        .phase_diagram()
        .map_or(0.0, |diagram| diagram.triple_temperature().value());
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
    /// The month's share of the year, `s_m`, in (0, 1].
    pub(super) share: f64,
    /// `c_PT` = min(1, α s ÷ (s + γ)) at the water's temperature ([`coefficient`]), in (0, 1].
    pub(super) coefficient: f64,
    /// A = β `S_abs` + `Q_O`, W m⁻² ([`energy_w_m2`]), of either sign.
    pub(super) energy_w_m2: f64,
    /// G, the sea's storage, W m⁻², of either sign.
    pub(super) storage_w_m2: f64,
    /// `q_s` at the water's temperature, the saturation specific humidity at the surface pressure,
    /// kg kg⁻¹, in [0, 1].
    pub(super) saturation_kg_per_kg: f64,
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
        saturation += open.share * open.saturation_kg_per_kg;
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
            open.saturation_kg_per_kg / saturation
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

/// The terms of one open month of a liquid's year that the liquid's temperature sets
/// ([`month_terms`]): its [`OpenMonth`] but for the month's share and the cell's energy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MonthTerms {
    /// `c_PT` at the liquid's temperature ([`coefficient`]).
    coefficient: f64,
    /// G, the sea's storage, W m⁻², of either sign; none in a one-month year.
    storage_w_m2: f64,
    /// `q_s` at the liquid's temperature, kg kg⁻¹; none in a one-month year.
    saturation_kg_per_kg: f64,
}

/// Each month's terms of an open liquid's year, one a month, `None` where it is frozen or past
/// the year's `months` (one, or twelve): the liquid's temperature `T_w` = `surface_k` + `a_m`, K,
/// its climate cell's anomaly `a_m` from `anomaly_k`, none in a one-month year; frozen below
/// `freezing_k`; else the coefficient ([`coefficient`]) of `moisture` in the air of `model`, the
/// storage `C_sea`(`T_w`) times the anomaly's centred change over the month's span from
/// `spans_s`, s, and `q_s`(`T_w`), neither of the last two in a one-month year.
///
/// It is the month loop of the water cycle's sea rates and the lakes' net loss
/// (`hydrology::Evaporation::rates`, `after_erosion::OpenLiquid::year_rates_kg_m2_s`), moved here
/// verbatim and less each cell's energy, so that a loop computes it once for the field cells of a
/// climate cell at one surface temperature ([`ClimateCellTerms`]; R09.T16.a).
///
/// # Panics
///
/// If `spans_s` holds fewer than twelve spans in a twelve-month year.
#[must_use]
pub(super) fn month_terms(
    model: &EnergyBalance,
    moisture: &Moisture,
    freezing_k: Option<f64>,
    spans_s: &[f64],
    anomaly_k: &[f64; 12],
    months: usize,
    surface_k: f64,
) -> [Option<MonthTerms>; 12] {
    let twelve = months == 12;
    let anomaly = |m: usize| {
        if twelve { anomaly_k[m] } else { 0.0 }
    };
    let mut terms = [None; 12];
    for (m, month) in terms.iter_mut().enumerate().take(months) {
        let t_w = surface_k + anomaly(m);
        if freezing_k.is_some_and(|f| t_w < f) {
            continue;
        }
        let c_pt = coefficient(moisture, model.specific_heat_j_per_kg_k, t_w);
        // A one-month year has no storage, and takes Priestley and Taylor's rate on its energy,
        // which reads no saturation.
        let (storage_w_m2, saturation_kg_per_kg) = if twelve {
            let (before, after) = ((m + 11) % 12, (m + 1) % 12);
            (
                model.heat_capacity(t_w, 1.0) * (anomaly(after) - anomaly(before)) / spans_s[m],
                moisture.saturation_specific_humidity(t_w),
            )
        } else {
            (0.0, 0.0)
        };
        *month = Some(MonthTerms {
            coefficient: c_pt,
            storage_w_m2,
            saturation_kg_per_kg,
        });
    }
    terms
}

/// Each month of an open liquid's year at a cell, from its month terms `terms` ([`month_terms`]):
/// the month's share from `shares` and the energy of the cell's grid point `point` from
/// `energy_w_m2` (one a month, each one a grid point) beside the terms; `None` where the terms
/// are.
///
/// # Panics
///
/// If a month that has terms is past `shares` or `energy_w_m2`, or `point` past its month's
/// energies.
#[must_use]
pub(super) fn open_months(
    terms: &[Option<MonthTerms>; 12],
    shares: &[f64],
    energy_w_m2: &[Vec<f64>],
    point: usize,
) -> [Option<OpenMonth>; 12] {
    let mut months = [None; 12];
    for (m, (month, kept)) in months.iter_mut().zip(terms).enumerate() {
        *month = kept.map(|t| OpenMonth {
            share: shares[m],
            coefficient: t.coefficient,
            energy_w_m2: energy_w_m2[m][point],
            storage_w_m2: t.storage_w_m2,
            saturation_kg_per_kg: t.saturation_kg_per_kg,
        });
    }
    months
}

/// A climate cell's field cells, consecutive in the field's cell-index order (`parent_index`).
const FIELD_CELLS_A_CLIMATE_CELL: usize = 4;

/// One climate cell's month terms ([`month_terms`]) by the surface temperatures its field cells
/// asked at, so that a loop over the field in cell-index order computes them once for the field
/// cells of one climate cell at one surface temperature (R09.T16.a; the [module
/// documentation](self)).
///
/// - **Why:** since the phase-data switch water's saturation pressure, IAPWS SR1-86's Wagner sum,
///   costs 196–337 ns a call by temperature band (312–337 ns over 273–310 K; R09.T16.a's probe,
///   test profile, 2026-10-23), against 25–30 ns for the held curve before `032a8c5f`, three calls
///   an open month. The reference Earth's erosion step makes 24.87 million of them in its two
///   balances, which rose from 0.9–1.3 s to 8.2–8.4 s, and the water cycle's sea rates rose from
///   0.4 s to 3.1–3.2 s (R09.T16.a's attribution, the test profile at level 8).
/// - **Exact:** within one loop over one working state, nothing changes, so a month's terms are
///   the same function of the same operands at the same climate cell and the same surface
///   temperature, to the bit: the surfaces are compared by their bits (`total_cmp`), so −0 and +0
///   are two, and a NaN matches only its own bits.
/// - **Owned by its loop:** each loop makes one and drops it after, and it forgets a climate cell
///   when the next begins. Its key holds only what varies within one loop, so no memo outlives
///   its loop: the entry points (`hydrology`'s `Evaporation::each_open_cell`,
///   `after_erosion`'s `NetLoss::at_water_surfaces`) each make a fresh one, and the functions
///   that take one are private to their modules, called by those entry points and the tests'
///   assertions alone, each with a fresh memo. It holds no map and nothing global, and no order
///   of its own reaches output.
#[derive(Debug, Clone, Default)]
pub(super) struct ClimateCellTerms {
    /// The climate cell whose terms it holds; `None` before the first ask.
    parent: Option<u32>,
    /// The slots held, at most [`FIELD_CELLS_A_CLIMATE_CELL`].
    len: usize,
    /// Each slot's surface temperature, K.
    surfaces_k: [f64; FIELD_CELLS_A_CLIMATE_CELL],
    /// Each slot's month terms.
    terms: [[Option<MonthTerms>; 12]; FIELD_CELLS_A_CLIMATE_CELL],
    /// The asks answered from a slot, which the tests count so that a lost sharing shows.
    #[cfg(test)]
    hits: u32,
}

impl ClimateCellTerms {
    /// The month terms at a field cell of climate cell `parent` whose surface temperature is
    /// `surface_k`, K: a slot's, where a field cell of `parent` asked at the same bits since
    /// `parent` began, else `compute`'s, which it keeps.
    pub(super) fn get_or_insert_with(
        &mut self,
        parent: u32,
        surface_k: f64,
        compute: impl FnOnce() -> [Option<MonthTerms>; 12],
    ) -> &[Option<MonthTerms>; 12] {
        if self.parent != Some(parent) {
            self.parent = Some(parent);
            self.len = 0;
        }
        let held = self.surfaces_k[..self.len]
            .iter()
            .position(|k| k.total_cmp(&surface_k).is_eq());
        let slot = if let Some(slot) = held {
            #[cfg(test)]
            {
                self.hits += 1;
            }
            slot
        } else {
            // A fifth surface comes only from a loop that asks at a field cell twice, and takes
            // the last slot.
            let slot = self.len.min(FIELD_CELLS_A_CLIMATE_CELL - 1);
            self.surfaces_k[slot] = surface_k;
            self.terms[slot] = compute();
            self.len = slot + 1;
            slot
        };
        &self.terms[slot]
    }

    /// The asks answered from a slot since it was made.
    #[cfg(test)]
    #[must_use]
    pub(super) fn hits(&self) -> u32 {
        self.hits
    }
}

/// The oracle of [`ClimateCellTerms`]' hits over a loop that asks at the field cells `visits` in
/// turn, each with its surface temperature, K: the visits whose climate cell's run of visits
/// asked at the same bits before; and the climate cells whose four field cells are all visited at
/// one surface temperature, each of which gives three.
#[cfg(test)]
#[must_use]
pub(super) fn expected_hits(visits: &[(u32, f64)]) -> (u32, u32) {
    use crate::planetary::surface::grid::parent_index;
    let (mut hits, mut whole) = (0_u32, 0_u32);
    for run in visits.chunk_by(|a, b| parent_index(a.0) == parent_index(b.0)) {
        for (i, &(_, surface_k)) in run.iter().enumerate() {
            hits += u32::from(
                run[..i]
                    .iter()
                    .any(|&(_, k)| k.total_cmp(&surface_k).is_eq()),
            );
        }
        whole += u32::from(
            run.len() == FIELD_CELLS_A_CLIMATE_CELL
                && run.iter().all(|&(_, k)| k.total_cmp(&run[0].1).is_eq()),
        );
    }
    (hits, whole)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::hooks::surface::EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
    use crate::planetary::surface::reference;

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
                    saturation_kg_per_kg: 0.008 + 0.004 * summer,
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
        // Each case's sunlight and ocean heat, W m⁻², `None` where no ocean heat reaches it.
        for (absorbed, ocean) in [
            (240.0, None),
            (240.0, Some(35.0)),
            (80.0, Some(-20.0)),
            (10.0, Some(-30.0)),
        ] {
            let energy = energy_w_m2(absorbed, ocean.unwrap_or(0.0));
            let open = OpenMonth {
                share: 1.0,
                coefficient: 0.83,
                energy_w_m2: energy,
                storage_w_m2: 0.0,
                saturation_kg_per_kg: 0.01,
            };
            let mut rate = [f64::NAN];
            year_rates_kg_m2_s(&[Some(open)], LATENT_J_PER_KG, &mut rate);
            hyperion_testkit::float::assert_same_bits(
                rate[0],
                rate_kg_m2_s(0.83, energy, LATENT_J_PER_KG),
            );
            if ocean.is_none() {
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
                        saturation_kg_per_kg: 0.012,
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

    /// The saturation specific humidity the aerodynamic part is spread by: on the reference
    /// Earth's air, ε `e_s` ÷ (p − (1 − ε) `e_s`) at the registry's saturation pressure to the bit,
    /// above the specific humidity at the model's relative humidity, and 1 (to 10⁻¹²) where the water
    /// boils at the surface pressure.
    #[test]
    fn the_saturation_specific_humidity_is_the_air_s_at_saturation() {
        let model = EnergyBalance::for_inputs(&reference::earth_like());
        let moisture = model.moisture.expect("the reference Earth is wet");
        let (p, epsilon) = (moisture.surface_pressure_pa, moisture.mass_ratio);
        for t in [260.0, 288.0, 300.0] {
            let e = saturation_pressure(moisture.condensable, Kelvin::new(t))
                .unwrap()
                .value();
            let q_s = moisture.saturation_specific_humidity(t);
            hyperion_testkit::float::assert_same_bits(q_s, epsilon * e / (p - (1.0 - epsilon) * e));
            assert!(q_s > moisture.specific_humidity(t), "{t} K");
        }
        let boiling = moisture.saturation_specific_humidity(400.0);
        assert!((boiling - 1.0).abs() < 1e-12, "{boiling}");
    }

    /// R09.T16.a's memo answers from a slot exactly where a field cell of the same climate cell
    /// asked at the same bits since that climate cell began, and otherwise computes and keeps: it
    /// tells −0 from +0, matches a NaN to its own bits, forgets a climate cell when the next
    /// begins, and gives a fifth surface the last slot. Each ask's terms are tagged by the ask
    /// that computed them, so the test sees which ask's terms come back.
    #[test]
    fn the_memo_answers_from_a_slot_only_within_a_climate_cell_at_the_same_bits() {
        let mut memo = ClimateCellTerms::default();
        let mut computed = 0_u32;
        let mut ask = |parent: u32, surface_k: f64, tag: f64| -> f64 {
            let terms = memo.get_or_insert_with(parent, surface_k, || {
                computed += 1;
                let mut terms = [None; 12];
                terms[0] = Some(MonthTerms {
                    coefficient: tag,
                    storage_w_m2: 0.0,
                    saturation_kg_per_kg: 0.0,
                });
                terms
            });
            terms[0]
                .expect("every ask's first month is open")
                .coefficient
        };
        // Each ask: its climate cell, its surface temperature, its tag, and the tag it gets back.
        let asks = [
            (7, 288.0, 1.0, 1.0),
            (7, 288.0, 2.0, 1.0),
            (7, 287.5, 3.0, 3.0),
            (7, 288.0, 4.0, 1.0),
            (8, 288.0, 5.0, 5.0),
            (8, 0.0, 6.0, 6.0),
            (8, -0.0, 7.0, 7.0),
            (8, 1.0, 8.0, 8.0),
            (8, 2.0, 9.0, 9.0),
            (8, 2.0, 10.0, 9.0),
            (8, 1.0, 11.0, 11.0),
            (8, 288.0, 12.0, 5.0),
            (7, 288.0, 13.0, 13.0),
            (9, f64::NAN, 14.0, 14.0),
            (9, f64::NAN, 15.0, 14.0),
        ];
        for (n, &(parent, surface_k, tag, expected)) in asks.iter().enumerate() {
            let got = ask(parent, surface_k, tag);
            assert_eq!(
                hyperion_testkit::float::bits(got),
                hyperion_testkit::float::bits(expected),
                "ask {n}: {got} against {expected}"
            );
        }
        assert_eq!(computed, 10, "the asks that computed");
        assert_eq!(memo.hits(), 5, "the asks answered from a slot");
        let visits = [
            (28, 288.0),
            (29, 288.0),
            (30, 287.5),
            (31, 288.0),
            (32, 288.0),
            (33, 0.0),
            (34, -0.0),
            (36, f64::NAN),
            (37, f64::NAN),
        ];
        assert_eq!(expected_hits(&visits), (3, 0), "the oracle's count");
        assert_eq!(
            expected_hits(&[(4, 300.0), (5, 300.0), (6, 300.0), (7, 300.0)]),
            (3, 1),
            "a whole climate cell at one surface"
        );
    }
}
