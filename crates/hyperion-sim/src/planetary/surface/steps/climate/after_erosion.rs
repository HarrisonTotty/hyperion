//! The climate after the erosion (R09.T14.d; decision-r09-t14-choices Q6(b), Q3 and Q7): the
//! climate step's own records kept in the working state, the lapse term and the mean re-applied
//! at the heights and the sea level the erosion leaves, with the ices and the liquids placed
//! again, and the open liquid's net loss that the erosion's lakes are balanced by.
//!
//! A child of the climate step, so that it reads the step's own records and functions and
//! widens none of them (Q6(b)).
//!
//! # The records (`ClimateRecords`)
//!
//! Each field cell's ice key carries −Γ times its water surface's height above the sea level ℓ,
//! and the erosion moves both, so keys stored by the climate step would place the ice at the old
//! heights (Q6(b)). The step keeps its normalised model instead (`Working::climate_records`, set
//! at the end of its `finish`):
//!
//! - the model's grid and coordinates;
//! - its records after the guard: the annual and monthly sea-level means at each grid point, and
//!   on the radiative model each point's ice key;
//! - its kind, its lower bound and its correction, with the mean's constant c;
//! - on the energy-balance models, its own records before the correction, which the evaporation
//!   reads: each month's means, each month's insolation, each point's water-surface height and,
//!   where a sea carries it, each month's ocean heat convergence per unit of each point's sea.
//!
//! It is held per grid point, not per field cell: on the reference Earth's 72 × 72 points,
//! 259,200 values, 2.07 MB. It is owned by the climate step and is no output: the quantiser does
//! not read it.
//!
//! # The re-application (`reapply_after_erosion`)
//!
//! It is Q7's step 4, after the re-match, `sea::place` and the lakes:
//!
//! 1. **The mean**, on the arithmetic models (the energy-balance models and the isothermal
//!    surface). Every climate cell's records are written again from the model's, and c′, the
//!    input's mean surface temperature less the field cells' area-weighted surface mean at the
//!    final water surfaces and ℓ, is added to every sea-level temperature and stored as the
//!    model's constant, then the guard holds every record at the lower bound.
//!    - This is the ruling's shift of every sea-level temperature by δ = c′ − c, the constant's
//!      change, taken from the records rather than added to the field the climate step left.
//!    - So unchanged heights give back the climate step's state to the bit, which a shift by a δ
//!      of rounding would not.
//!    - A record the guard held is held again from its own value, not shifted from the bound.
//!    - The radiative model has no lapse term and no constant, and its records stand.
//! 2. **The temperatures and keys:** each field cell's annual mean surface temperature and ice key
//!    at its final water surface (`cell_temperatures`).
//! 3. **The ices**, placed again with those keys (`ice::place_ices`, which takes back every ice it
//!    places first). On the radiative model the keys are the climate step's, and so is every
//!    cell's ice.
//! 4. **The liquids**, shared on the final flood, the sea and the lakes (`seas::place_liquids`).
//!
//! Neither the model nor the water cycle runs again (R6). The precipitation, the winds and the
//! runoff stand, and a depression that was dry land in the climate keeps that climate's.
//!
//! # The lakes' net loss (`NetLoss`)
//!
//! Q3 balances a closed lake's inflow, the runoff from the land outside it, against its net loss,
//! Σ (E − P) A over its cells; the erosion's balance adds each cell's own runoff to the rate this
//! module gives, so that a flooded cell's runoff leaves its inflow (`erosion::branches`'
//! `WaterBalance`). At one cell whose liquid would stand at the surface z, its rate E − P is in
//! metres of the main liquid a Julian year:
//!
//! - **E** is the climate's Priestley–Taylor evaporation of the climate condensable's own open
//!   liquid, the formula the sea's evaporation takes in the water cycle (`hydrology`'s
//!   `Evaporation::rates`, from `evaporation::coefficient`, `evaporation::energy_w_m2` and
//!   `evaporation::year_rates_kg_m2_s`; decision-r09-t13i §3.2): Σₘ sₘ Eₘ of the cell's year,
//!   Priestley and Taylor's with no storage on the energy β `S_abs` + `Q_O` over its open months,
//!   its months in the Penman split's shape.
//!   - It reads the model's own records: each grid point's absorbed sunlight at its water surface
//!     and its ocean heat convergence per unit of its sea each month, and the sea's storage over
//!     the month's centred span.
//!   - The liquid's month temperature is its climate cell's at z, `T_sl` − Γ (z − ℓ) + `a_m`,
//!     which sets each month's coefficient, storage and saturation specific humidity.
//!   - A month in which that lies below the model's freezing point evaporates nothing, and the
//!     open months' year is the year's amount.
//!   - E is zero on the models that keep no own records (the radiative and isothermal ones), and
//!     where the main liquid, the lake's, is not the condensable's own.
//! - **P** is the cell's climate cell's annual precipitation, Σₘ sₘ Pₘ over the months' shares of
//!   the year.
//! - **The units:** E − P, in kg m⁻² s⁻¹, times a Julian year's seconds over the main liquid's
//!   density, its palette entry's. A world with no main liquid has no lake, and its net loss is
//!   zero.
//!
//! A balance takes it at every field cell at its own water surface, in cell-index order
//! (`NetLoss::at_water_surfaces`), where the field cells of one climate cell at one water surface
//! (under the sea, all four) share their months' coefficient, storage and `q_s`, to the bit
//! (`evaporation::ClimateCellTerms`; R09.T16.a: since the phase-data switch the reference Earth's
//! two balances in its erosion step took 8.2–8.4 s, against 0.9–1.3 s before, nearly all of it
//! water's saturation pressure).
//!
//! **Labelled:**
//! - The cell's ice share is not applied, where the sea's evaporation takes its open share, 1 −
//!   its ice fraction: a lake candidate's ice is the climate step's placement at the old heights,
//!   which the re-application replaces.
//! - E reads the climate before the re-application's constant. Q7's lakes come before it, an
//!   error of Γ times the change of the mean height, a few tenths of a kelvin.
//! - A lake candidate takes its grid point's ocean heat `Q_O`, as the sea's cells there do: none
//!   at a point with no sea, and at a coastal point the sea's currents' heat per unit of that sea,
//!   which no inland lake receives.
//! - Where the main liquid is not the condensable, P is the condensable's mass, taken at the main
//!   liquid's density.

use hyperion_surface::field::PaletteRole;

use super::coordinates::Coordinates;
use super::ebm::{EnergyBalance, Freezing, Moisture};
use super::evaporation::ClimateCellTerms;
#[cfg(test)]
use super::evaporation::OpenMonth;
use super::grid::ModelGrid;
use super::{
    MeanKind, Normalised, cell_temperatures, evaporation, floor_climate, ice, month_shares, seas,
    surface_mean_k, write_climate,
};
use crate::planetary::surface::grid::parent_index;
use crate::planetary::surface::steps::{Pass, Working};
use crate::units::Kelvin;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The climate step's normalised model, kept for the re-application after the erosion and for
/// the lakes' net loss (the [module documentation](self); decision-r09-t14-choices Q6(b)).
///
/// It is opaque outside the climate step, and no output. On the reference Earth it holds 259,200
/// values, 2.07 MB.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClimateRecords {
    /// The model as the climate step finished it, its correction's constant the mean's.
    pub(super) model: Normalised,
}

/// Re-applies the climate's lapse term and mean to `working` at the heights and the sea level the
/// erosion leaves, then places the ices and the liquids again (the [module documentation](self);
/// decision-r09-t14-choices Q6(b), and Q7's step 4).
///
/// On the arithmetic models the constant is re-solved so that the field cells' area-weighted
/// surface mean is the input's mean surface temperature again. On the radiative model only the
/// placements run, at its unchanged keys. Neither the model nor the water cycle runs again.
///
/// It does nothing where the climate step did not run, so that `working` holds no records: a test
/// that skips that step.
///
/// # Panics
///
/// If a temperature or a key comes out not finite, a fault of the model, or as the placements
/// (`ice::place_ices`, `seas::place_liquids`).
pub(crate) fn reapply_after_erosion(pass: &Pass<'_>, working: &mut Working) {
    let Some(ClimateRecords { mut model }) = working.climate_records.take() else {
        return;
    };
    let inputs = pass.inputs();
    // The guard's holds are diagnostics, which only the climate step's own run reports.
    let mut clamped = match model.kind {
        MeanKind::Arithmetic => {
            let clamped = write_climate(
                pass,
                working,
                &model.grid,
                model.coordinates,
                &model.climate,
            );
            let mean = inputs.mean_surface_temperature().value() - surface_mean_k(pass, working);
            for state in &mut working.climate {
                state.sea_level_temperature =
                    Kelvin::new(state.sea_level_temperature.value() + mean);
            }
            model.correction.mean_k = mean;
            let _floored = floor_climate(working, model.lower_bound_k);
            clamped
        }
        MeanKind::Radiative { .. } => 0,
    };
    let (annual_k, keys) = cell_temperatures(pass, working, &model, &mut clamped);
    let _ices = ice::place_ices(
        inputs,
        pass.grid().solid_angles_sr(),
        pass.grid().centres(),
        &keys,
        &mut working.cells,
    );
    let _liquids = seas::place_liquids(inputs, pass.grid(), &annual_k, &mut working.cells);
    working.climate_records = Some(ClimateRecords { model });
}

/// Q3's net loss of an open liquid at field cell `cell` of `working`, were its liquid to stand at
/// the surface `surface_m` above the datum, m: E − P in metres of the main liquid a Julian year
/// (the [module documentation](self); decision-r09-t14-choices Q3), positive where the liquid
/// loses more than it gains.
///
/// It builds a `NetLoss` for the one call. A caller that asks at many cells builds one and asks
/// it (`NetLoss::at`, or at every cell `NetLoss::at_water_surfaces`).
///
/// # Panics
///
/// As `NetLoss::new` and `NetLoss::at`.
#[cfg(test)]
#[must_use]
pub(crate) fn open_liquid_net_loss_m_per_year(
    pass: &Pass<'_>,
    working: &Working,
    cell: u32,
    surface_m: f64,
) -> f64 {
    NetLoss::new(pass, working).at(pass, working, cell, surface_m)
}

/// What Q3's net loss reads that is the same at every cell of one working state, built once for
/// all of a field's lake candidates (the [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NetLoss {
    /// The evaporation of the climate condensable's own liquid, or `None` where none evaporates:
    /// no records, a model with no records of its own, a dry world, or a main liquid that is not
    /// the condensable's.
    evaporation: Option<OpenLiquid>,
    /// Each month's share of the year.
    shares: Vec<f64>,
    /// A Julian year's seconds over the main liquid's density, from kg m⁻² s⁻¹ to m a⁻¹, m³ s
    /// kg⁻¹ a⁻¹; `None` on a world with no main liquid.
    scale: Option<f64>,
}

impl NetLoss {
    /// The net loss over `working`'s climate on `pass`'s world, from the climate step's records
    /// that `working` holds: no evaporation where it holds none.
    ///
    /// # Panics
    ///
    /// If the model's months are not `working`'s, or the main liquid's palette entry carries no
    /// liquid record, which the inputs' builder refuses.
    #[must_use]
    pub(crate) fn new(pass: &Pass<'_>, working: &Working) -> Self {
        let inputs = pass.inputs();
        let scale = inputs.main_liquid().map(|entry| {
            let density_kg_m3 = inputs
                .palette()
                .get(entry)
                .and_then(|entry| entry.liquid.as_ref())
                .expect("the inputs' builder gives the main liquid's entry its liquid record")
                .density_kg_m3;
            SECONDS_PER_JULIAN_YEAR / density_kg_m3
        });
        Self {
            evaporation: OpenLiquid::of(pass, working),
            shares: month_shares(pass, usize::from(working.months)),
            scale,
        }
    }

    /// E − P at every field cell of `working`, its liquid at the cell's own water surface, metres
    /// of the main liquid a Julian year, one a cell in cell-index order (the [module
    /// documentation](self)); zero on a world with no main liquid.
    ///
    /// Each cell's is `at`'s there, the tests' oracle, to the bit: the field cells of one climate
    /// cell at one water surface share their months' coefficient, storage and `q_s`
    /// ([`ClimateCellTerms`]; R09.T16.a), and each cell's energy, precipitation and sums stay its
    /// own, in their order.
    ///
    /// # Panics
    ///
    /// If a loss is not finite (a fault of the climate, or a water surface that is not finite).
    #[must_use]
    pub(crate) fn at_water_surfaces(&self, pass: &Pass<'_>, working: &Working) -> Vec<f64> {
        self.at_water_surfaces_sharing(pass, working, &mut ClimateCellTerms::default())
    }

    /// [`at_water_surfaces`](Self::at_water_surfaces) with the month terms kept in `terms`, a
    /// memo made fresh for this one loop over `working`. Private to this module, so that its
    /// callers are `at_water_surfaces` and the tests' `assert_the_net_loss_shares_its_month_terms`
    /// alone, each with a fresh memo, which the tests read: none carries one across loops or
    /// working states, which its key does not tell apart.
    ///
    /// # Panics
    ///
    /// As [`at_water_surfaces`](Self::at_water_surfaces).
    #[must_use]
    fn at_water_surfaces_sharing(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        terms: &mut ClimateCellTerms,
    ) -> Vec<f64> {
        let Some(scale) = self.scale else {
            return vec![0.0; working.cells.len()];
        };
        (0_u32..)
            .zip(&working.cells)
            .map(|(cell, state)| {
                let evaporated = self.evaporation.as_ref().map_or(0.0, |open| {
                    let point = open.point_of(pass, cell);
                    let mut year = [0.0; 12];
                    let rates = &mut year[..usize::from(working.months)];
                    open.year_rates_sharing(
                        working,
                        cell,
                        point,
                        state.water_surface.value(),
                        rates,
                        terms,
                    );
                    self.shares
                        .iter()
                        .zip(rates.iter())
                        .fold(0.0, |sum, (&share, &rate)| sum + share * rate)
                });
                let loss = (evaporated - self.precipitation_kg_m2_s(working, cell)) * scale;
                assert!(
                    loss.is_finite(),
                    "an open liquid's net loss must be finite, got {loss} m a⁻¹ at cell {cell}"
                );
                loss
            })
            .collect()
    }

    /// E − P at field cell `cell` of `working`, its liquid at the surface `surface_m` above the
    /// datum, metres of the main liquid a Julian year (`open_liquid_net_loss_m_per_year`); zero
    /// on a world with no main liquid.
    ///
    /// The oracle of [`at_water_surfaces`](Self::at_water_surfaces): this cell alone, its month
    /// terms its own (R09.T16.a).
    ///
    /// # Panics
    ///
    /// If `cell` is not one of the field's cells, or the loss is not finite (a fault of the
    /// climate, or a `surface_m` that is not finite).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn at(&self, pass: &Pass<'_>, working: &Working, cell: u32, surface_m: f64) -> f64 {
        let Some(scale) = self.scale else {
            return 0.0;
        };
        let loss = (self.evaporation_kg_m2_s(pass, working, cell, surface_m)
            - self.precipitation_kg_m2_s(working, cell))
            * scale;
        assert!(
            loss.is_finite(),
            "an open liquid's net loss must be finite, got {loss} m a⁻¹ at cell {cell}"
        );
        loss
    }

    /// E, the annual evaporation of the condensable's own liquid at field cell `cell` of
    /// `working`, its surface at `surface_m` above the datum, kg m⁻² s⁻¹: Σₘ sₘ Eₘ of its year
    /// (`OpenLiquid::year_rates_kg_m2_s`), summed in month order; zero where none evaporates.
    ///
    /// # Panics
    ///
    /// As `OpenLiquid::year_rates_kg_m2_s`.
    #[cfg(test)]
    #[must_use]
    fn evaporation_kg_m2_s(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        cell: u32,
        surface_m: f64,
    ) -> f64 {
        let Some(open) = &self.evaporation else {
            return 0.0;
        };
        let point = open.point_of(pass, cell);
        let mut year = [0.0; 12];
        let rates = &mut year[..usize::from(working.months)];
        open.year_rates_kg_m2_s(working, cell, point, surface_m, rates);
        self.shares
            .iter()
            .zip(rates.iter())
            .fold(0.0, |sum, (&share, &rate)| sum + share * rate)
    }

    /// P, field cell `cell`'s climate cell's annual precipitation in `working`, kg m⁻² s⁻¹:
    /// Σₘ sₘ Pₘ.
    #[must_use]
    fn precipitation_kg_m2_s(&self, working: &Working, cell: u32) -> f64 {
        let state = &working.climate[slot(parent_index(cell))];
        self.shares
            .iter()
            .zip(&state.month_precipitation_kg_m2_s)
            .fold(0.0, |sum, (&share, &rate)| sum + share * rate)
    }
}

/// The evaporation of the climate condensable's own open liquid as the water cycle takes it over
/// the sea (`hydrology`'s `Evaporation`, which reads each cell's own water surface and liquid, so
/// that a lake candidate's needs its own): the model, the condensable, the freezing point, each
/// grid point's energy each month, and each month's share and centred span.
#[derive(Debug, Clone, PartialEq)]
struct OpenLiquid {
    /// The model, whose heat capacity and air the rate reads.
    model: EnergyBalance,
    /// The climate condensable.
    moisture: Moisture,
    /// The freezing point below which the liquid gives nothing, K; `None` where no ice forms.
    freezing_k: Option<f64>,
    /// The model's grid, whose points the energy is held at.
    grid: ModelGrid,
    /// The model's coordinates.
    coordinates: Coordinates,
    /// Each month's energy at each grid point's water surface, W m⁻², A = β `S_abs` + `Q_O`
    /// (`evaporation::energy_w_m2`), as the water cycle takes it: `S_abs` the month's insolation
    /// × (1 − α(T − Γ `z̄_w`)) at the model's own record T, and `Q_O` the point's ocean heat
    /// convergence per unit of its sea, none where no sea carries the term.
    energy_w_m2: Vec<Vec<f64>>,
    /// Each month's span from the previous month's middle to the next's, s: (½ s₋ + s + ½ s₊) P
    /// of the shares s, as the water cycle takes it.
    spans_s: Vec<f64>,
    /// Each month's share of the year, as the water cycle takes it.
    shares: Vec<f64>,
}

impl OpenLiquid {
    /// The open liquid of `pass`'s world over `working`'s climate records, or `None` where none
    /// evaporates: `working` holds no records, the model keeps no records of its own, the world
    /// is dry, or its main liquid is not the condensable's own `Liquid` entry.
    ///
    /// # Panics
    ///
    /// If the model's months are not `working`'s.
    #[must_use]
    fn of(pass: &Pass<'_>, working: &Working) -> Option<Self> {
        let records = &working.climate_records.as_ref()?.model;
        let own = records.water.as_ref()?;
        let inputs = pass.inputs();
        let model = EnergyBalance::for_inputs(inputs);
        let moisture = model.moisture?;
        let entry = inputs
            .climate()
            .condensable
            .and_then(|key| inputs.palette().find(PaletteRole::Liquid, key))?;
        if inputs.main_liquid() != Some(entry) {
            return None;
        }
        let months = own.months_k.len();
        assert_eq!(
            months,
            usize::from(working.months),
            "the model's months are the working state's"
        );
        // The water cycle's own arithmetic (`hydrology`'s `Evaporation::new`), in its order, so
        // that the sea's rates come out to the bit.
        let lapse = working.lapse_rate_k_per_m;
        let energy_w_m2 = own
            .months_k
            .iter()
            .zip(&own.insolation_w_m2)
            .enumerate()
            .map(|(m, (t_k, insolation))| {
                t_k.iter()
                    .zip(insolation)
                    .zip(&own.water_m)
                    .enumerate()
                    .map(|(p, ((&t, &q), &z))| {
                        let absorbed = q * (1.0 - model.albedo_at(t - lapse * z));
                        let ocean = own.ocean_w_m2.as_ref().map_or(0.0, |ocean| ocean[m][p]);
                        evaporation::energy_w_m2(absorbed, ocean)
                    })
                    .collect()
            })
            .collect();
        let shares = month_shares(pass, months);
        let period_s = inputs
            .seasonal_orbit()
            .map_or(0.0, |orbit| orbit.period.value());
        let spans_s = (0..months)
            .map(|m| {
                let (before, after) = ((m + months - 1) % months, (m + 1) % months);
                period_s * (0.5 * shares[before] + shares[m] + 0.5 * shares[after])
            })
            .collect();
        Some(Self {
            model,
            moisture,
            freezing_k: match model.freezing {
                Freezing::At { point_k } => Some(point_k),
                Freezing::Never { .. } => None,
            },
            grid: records.grid.clone(),
            coordinates: records.coordinates,
            energy_w_m2,
            spans_s,
            shares,
        })
    }

    /// The grid point that holds field cell `cell`'s centre, as the sea shares bin it.
    #[must_use]
    fn point_of(&self, pass: &Pass<'_>, cell: u32) -> usize {
        self.grid
            .point_of(self.coordinates.to_model(pass.grid().centre(cell)))
    }

    /// The open liquid's evaporation rate each month at field cell `cell` of `working`, its
    /// surface at `surface_m` above the datum and its centre in grid point `point`, kg m⁻² s⁻¹,
    /// into `rates` (one a month): the water cycle's sea rates before its ice share. A month is
    /// zero where the liquid's temperature, its climate cell's month at that surface, lies below
    /// the freezing point; the open months take the year of `evaporation::year_rates_kg_m2_s` on
    /// β `S_abs` + `Q_O`, the coefficient min(1, α s ÷ (s + γ)), the storage `C_sea` dT/dt and
    /// the saturation specific humidity at that temperature, none of the last two in a one-month
    /// year.
    ///
    /// The oracle of [`year_rates_sharing`](Self::year_rates_sharing): this cell alone, its month
    /// terms its own (R09.T16.a).
    ///
    /// # Panics
    ///
    /// If `rates` does not hold one value for each of `working`'s months, or `working` has more
    /// months than the model.
    #[cfg(test)]
    fn year_rates_kg_m2_s(
        &self,
        working: &Working,
        cell: u32,
        point: usize,
        surface_m: f64,
        rates: &mut [f64],
    ) {
        assert_eq!(
            rates.len(),
            usize::from(working.months),
            "the open liquid's rates hold one value for each of the working state's months"
        );
        let state = &working.climate[slot(parent_index(cell))];
        let twelve = working.months == 12;
        let anomaly = |m: usize| {
            if twelve {
                state.month_anomaly_k[m]
            } else {
                0.0
            }
        };
        let surface_k = state.sea_level_temperature.value()
            - working.lapse_rate_k_per_m * (surface_m - working.sea_level.value());
        let mut months: [Option<OpenMonth>; 12] = [None; 12];
        for (m, month) in months.iter_mut().enumerate().take(rates.len()) {
            let t_w = surface_k + anomaly(m);
            if self.freezing_k.is_some_and(|f| t_w < f) {
                continue;
            }
            let coefficient =
                evaporation::coefficient(&self.moisture, self.model.specific_heat_j_per_kg_k, t_w);
            // A one-month year has no storage, and takes Priestley and Taylor's rate on its
            // energy, which reads no saturation.
            let (storage, saturation_kg_per_kg) = if twelve {
                let (before, after) = ((m + 11) % 12, (m + 1) % 12);
                (
                    self.model.heat_capacity(t_w, 1.0) * (anomaly(after) - anomaly(before))
                        / self.spans_s[m],
                    self.moisture.saturation_specific_humidity(t_w),
                )
            } else {
                (0.0, 0.0)
            };
            *month = Some(OpenMonth {
                share: self.shares[m],
                coefficient,
                energy_w_m2: self.energy_w_m2[m][point],
                storage_w_m2: storage,
                saturation_kg_per_kg,
            });
        }
        evaporation::year_rates_kg_m2_s(
            &months[..rates.len()],
            self.moisture.latent_heat_j_per_kg,
            rates,
        );
    }

    /// The open liquid's evaporation rate each month at field cell `cell` of `working`, its
    /// surface at `surface_m` above the datum and its centre in grid point `point`, kg m⁻² s⁻¹,
    /// into `rates` (one a month): `year_rates_kg_m2_s`'s, the tests' oracle, to the bit, with the
    /// month terms ([`evaporation::month_terms`]) from `terms`, where a field cell of the same
    /// climate cell asked at the same surface temperature (R09.T16.a): the memo of the one loop
    /// that calls it, [`NetLoss::at_water_surfaces_sharing`]'s over `working`'s cells.
    ///
    /// # Panics
    ///
    /// If `rates` does not hold one value for each of `working`'s months, or `working` has more
    /// months than the model.
    fn year_rates_sharing(
        &self,
        working: &Working,
        cell: u32,
        point: usize,
        surface_m: f64,
        rates: &mut [f64],
        terms: &mut ClimateCellTerms,
    ) {
        let months = rates.len();
        assert_eq!(
            months,
            usize::from(working.months),
            "the open liquid's rates hold one value for each of the working state's months"
        );
        let parent = parent_index(cell);
        let state = &working.climate[slot(parent)];
        let surface_k = state.sea_level_temperature.value()
            - working.lapse_rate_k_per_m * (surface_m - working.sea_level.value());
        let month_terms = terms.get_or_insert_with(parent, surface_k, || {
            evaporation::month_terms(
                &self.model,
                &self.moisture,
                self.freezing_k,
                &self.spans_s,
                &state.month_anomaly_k,
                months,
                surface_k,
            )
        });
        let open_months =
            evaporation::open_months(month_terms, &self.shares, &self.energy_w_m2, point);
        evaporation::year_rates_kg_m2_s(
            &open_months[..months],
            self.moisture.latent_heat_j_per_kg,
            rates,
        );
    }
}

/// Asserts R09.T16.a's sharing of the lakes' net loss `net` over `working` on `pass`'s world,
/// whose open liquid evaporates, and prints what it found under `what`. `net` is
/// [`NetLoss::new`]'s over `working`, or over the twelve-month state whose climate records
/// `working` reads as a one-month year (its `months` 1), which `NetLoss::new` refuses, so that
/// [`evaporation::month_terms`]' one-month branch meets the oracle's:
///
/// - **To the bit:** at every field cell, [`NetLoss::at_water_surfaces`] is [`NetLoss::at`] at the
///   cell's water surface, the oracle.
/// - **Shared:** its memo's hits are the oracle's count ([`evaporation::expected_hits`]) over every
///   cell at its surface temperature, `T_sl` − Γ (`z_w` − ℓ).
/// - **A floor:** that is at least three hits for each climate cell whose four field cells stand
///   at one surface temperature, and such climate cells, the sea's, are half of them at least
///   (on the small Earth 957 of 1,536 after its climate step and 955 in its final balance, with
///   3,150 and 3,153 hits), so that a loss of the sharing, or of the sea's one surface, shows.
///
/// The memo's hits.
#[cfg(test)]
pub(crate) fn assert_the_net_loss_shares_its_month_terms(
    pass: &Pass<'_>,
    net: &NetLoss,
    working: &Working,
    what: &str,
) -> u32 {
    use hyperion_testkit::float::bits;
    assert!(
        net.evaporation.is_some() && net.scale.is_some(),
        "{what}: the open liquid evaporates"
    );
    let mut terms = ClimateCellTerms::default();
    let shared = net.at_water_surfaces_sharing(pass, working, &mut terms);
    assert_eq!(shared.len(), working.cells.len(), "{what}: one loss a cell");
    let mut visits = Vec::with_capacity(shared.len());
    for ((cell, state), &loss) in (0_u32..).zip(&working.cells).zip(&shared) {
        let surface_m = state.water_surface.value();
        assert_eq!(
            bits(loss),
            bits(net.at(pass, working, cell, surface_m)),
            "{what}: cell {cell}'s net loss against the oracle's"
        );
        let climate = &working.climate[slot(parent_index(cell))];
        visits.push((
            cell,
            climate.sea_level_temperature.value()
                - working.lapse_rate_k_per_m * (surface_m - working.sea_level.value()),
        ));
    }
    let (expected, whole) = evaporation::expected_hits(&visits);
    let climate_cells = u32::try_from(working.climate.len()).expect("the climate cells fit a u32");
    let hits = terms.hits();
    eprintln!(
        "{what}: the net loss at {} field cells to the bit; the month terms shared {hits} times \
         ({expected} expected), {whole} of {climate_cells} climate cells at one surface \
         temperature",
        shared.len()
    );
    assert_eq!(hits, expected, "{what}: the memo's hits");
    assert!(
        hits >= 3 * whole && 2 * whole >= climate_cells,
        "{what}: {hits} hits, {whole} of {climate_cells} climate cells at one surface temperature"
    );
    hits
}

/// The index into a vector of cell `cell`.
///
/// # Panics
///
/// Never on the targets the sim builds for: a `u32` fits a `usize` on 32- and 64-bit targets.
#[must_use]
fn slot(cell: u32) -> usize {
    usize::try_from(cell).expect("a cell index fits a usize on every target the sim builds for")
}

#[cfg(test)]
mod tests {
    use hyperion_surface::field::{ClimateModelKind, coarse_level};

    use super::super::{Diagnostics, hydrology, run_diagnosed};
    use super::*;
    use crate::planetary::surface::grid::CellGraph;
    use crate::planetary::surface::inputs::CoarseInputs;
    use crate::planetary::surface::reference::{self, small_earth};
    use crate::planetary::surface::steps::{craters, plates, relief};
    use crate::units::Metres;
    use hyperion_testkit::float::bits;

    /// A world's working state staged to the end of its climate step under the reference seed, at
    /// its own level: the pass's steps before the erosion, in the pass's order, with what the
    /// climate step did that is no output.
    struct Staged {
        inputs: CoarseInputs,
        grid: CellGraph,
        climate_grid: CellGraph,
        working: Working,
        diagnostics: Diagnostics,
    }

    impl Staged {
        /// `inputs`' world through plates, relief, craters, the relief's second entry and the
        /// climate.
        fn of(inputs: CoarseInputs) -> Self {
            let level = coarse_level(inputs.radius());
            let grid = CellGraph::for_field(level);
            let climate_grid = CellGraph::for_climate(level);
            let (working, diagnostics) = {
                let pass = Pass::new(reference::SEED, &inputs, &grid, &climate_grid);
                let mut working = Working::initial(&pass);
                assert!(
                    working.climate_records.is_none(),
                    "the initial state holds no climate records"
                );
                plates::run(&pass, &mut working);
                relief::run(&pass, &mut working);
                craters::run(&pass, &mut working);
                relief::run_after_craters(&pass, &mut working);
                let diagnostics = run_diagnosed(&pass, &mut working);
                (working, diagnostics)
            };
            Self {
                inputs,
                grid,
                climate_grid,
                working,
                diagnostics,
            }
        }

        /// The pass over the staged world.
        fn pass(&self) -> Pass<'_> {
            Pass::new(
                reference::SEED,
                &self.inputs,
                &self.grid,
                &self.climate_grid,
            )
        }
    }

    /// The model `working` keeps.
    fn model_of(working: &Working) -> &Normalised {
        &working
            .climate_records
            .as_ref()
            .expect("the climate step keeps its records")
            .model
    }

    /// Each palette entry's ice over `working`'s cells, Σ Ω f, sr, one value an entry.
    fn ice_areas(pass: &Pass<'_>, working: &Working) -> [f64; 256] {
        let mut areas = [0.0; 256];
        for (cell, &omega) in working.cells.iter().zip(pass.grid().solid_angles_sr()) {
            if let Some(entry) = cell.ice_entry {
                areas[usize::from(entry)] += omega * cell.ice_fraction;
            }
        }
        areas
    }

    /// The point arrays `records` holds, their values' count: the annual and monthly records,
    /// the radiative keys and the energy-balance models' own records, the ocean's heat among
    /// them (the grid's own few hundred values aside).
    fn record_values(records: &ClimateRecords) -> usize {
        let model = &records.model;
        let rows = |rows: &[Vec<f64>]| rows.iter().map(Vec::len).sum::<usize>();
        let keys = match &model.kind {
            MeanKind::Radiative { keys_k } => keys_k.len(),
            MeanKind::Arithmetic => 0,
        };
        let own = model.water.as_ref().map_or(0, |own| {
            rows(&own.months_k)
                + rows(&own.insolation_w_m2)
                + own.water_m.len()
                + own.ocean_w_m2.as_deref().map_or(0, rows)
        });
        model.climate.annual_k.len() + rows(&model.climate.months_k) + keys + own
    }

    /// Moves `working`'s land cells by hand, in their index order: every sixth from the first
    /// raised 800 m, from the third lowered 400 m, both dry, and from the fifth lowered 150 m
    /// under a pond at its old ground. The land cells, and the moved ones' counts: raised,
    /// lowered and ponded.
    fn move_land(working: &mut Working) -> (Vec<usize>, [u32; 3]) {
        let land: Vec<usize> = (0..working.cells.len())
            .filter(|&i| !seas::under_sea(&working.cells[i]))
            .collect();
        let mut moved = [0_u32; 3];
        for (k, &i) in land.iter().enumerate() {
            let cell = &mut working.cells[i];
            let z = cell.elevation.value();
            let (ground, surface, kind) = match k % 6 {
                0 => (z + 800.0, z + 800.0, 0),
                2 => (z - 400.0, z - 400.0, 1),
                4 => (z - 150.0, z, 2),
                _ => continue,
            };
            cell.elevation = Metres::new(ground);
            cell.water_surface = Metres::new(surface);
            moved[kind] += 1;
        }
        (land, moved)
    }

    /// Asserts that `working`'s liquids and ices are placed consistently with its flood: a liquid
    /// on exactly the flooded cells, each a palette `Liquid` entry, and every iced cell naming a
    /// palette `Ice` entry. The flooded and iced cells' counts.
    fn assert_placed_consistently(pass: &Pass<'_>, working: &Working) -> (u32, u32) {
        let palette = pass.inputs().palette();
        let (mut flooded, mut iced) = (0_u32, 0_u32);
        for (n, cell) in working.cells.iter().enumerate() {
            assert_eq!(
                cell.liquid_entry.is_some(),
                seas::under_sea(cell),
                "cell {n}: a liquid exactly where it is flooded"
            );
            if let Some(entry) = cell.liquid_entry {
                assert_eq!(
                    palette.get(entry).map(|e| e.role),
                    Some(PaletteRole::Liquid),
                    "cell {n}"
                );
                flooded += 1;
            }
            if cell.ice_fraction > 0.0 {
                let entry = cell.ice_entry.expect("an iced cell names its ice");
                assert_eq!(
                    palette.get(entry).map(|e| e.role),
                    Some(PaletteRole::Ice),
                    "cell {n}"
                );
                iced += 1;
            }
        }
        (flooded, iced)
    }

    /// On the small Earth, with some land cells raised, some lowered and some lowered under a
    /// pond of their old height by hand ([`move_land`]), the re-application brings the field
    /// cells' surface mean back to the input's mean, to 10⁻⁹ relative, the stored constant moved
    /// by the change, and places the ices and the liquids again: every flooded cell, the ponds
    /// included, names a liquid and no dry one does, every iced cell names a palette `Ice` entry,
    /// and each ice covers its area again to one cell.
    #[test]
    fn the_re_application_holds_the_mean_and_places_the_ices_and_liquids_again() {
        let staged = Staged::of(small_earth());
        let pass = staged.pass();
        let mut working = staged.working.clone();
        let (land, [raised, lowered, ponds]) = move_land(&mut working);
        let t_in = pass.inputs().mean_surface_temperature().value();
        let moved = surface_mean_k(&pass, &working);
        let constant = model_of(&working).correction.mean_k;
        eprintln!(
            "small Earth: {} land cells, {raised} raised 800 m, {lowered} lowered 400 m, {ponds} \
             under ponds; surface mean {moved:.6} K against {t_in} K",
            land.len()
        );
        assert!(
            (moved - t_in).abs() > 1e-3,
            "the moved heights move the mean: {moved} against {t_in}"
        );
        reapply_after_erosion(&pass, &mut working);
        let mean = surface_mean_k(&pass, &working);
        assert!(
            (mean - t_in).abs() <= 1e-9 * t_in,
            "the surface mean {mean} K is the input's {t_in} K"
        );
        let shift = model_of(&working).correction.mean_k - constant;
        assert!(
            (shift - (t_in - moved)).abs() <= 1e-9 * t_in,
            "the constant moved by {shift} K, the change {} K",
            t_in - moved
        );
        let (flooded, iced) = assert_placed_consistently(&pass, &working);
        for &i in land.iter().skip(4).step_by(6) {
            assert!(
                working.cells[i].liquid_entry.is_some(),
                "pond cell {i} names a liquid"
            );
        }
        assert!(flooded > 0 && iced > 0, "{flooded} flooded, {iced} iced");
        let largest = pass
            .grid()
            .solid_angles_sr()
            .iter()
            .fold(0.0, |w: f64, &o| w.max(o));
        let (before, after) = (
            ice_areas(&pass, &staged.working),
            ice_areas(&pass, &working),
        );
        for (entry, (&b, &a)) in before.iter().zip(&after).enumerate() {
            assert!(
                (a - b).abs() <= largest,
                "entry {entry}: the ice covers {a} sr again, against {b} sr"
            );
        }
        let moved_ice = working
            .cells
            .iter()
            .zip(&staged.working.cells)
            .filter(|(a, b)| {
                a.ice_entry != b.ice_entry || bits(a.ice_fraction) != bits(b.ice_fraction)
            })
            .count();
        eprintln!(
            "small Earth: {flooded} cells flooded, {iced} iced, {moved_ice} cells' ice moved"
        );
    }

    /// `staged`'s world re-applied at the heights its climate step saw: every cell's ice and
    /// liquid and every climate cell's state as that step left them, to the bit, and the whole
    /// working state, its records included, equal.
    fn assert_unchanged_heights_change_nothing(staged: &Staged, what: &str) {
        let pass = staged.pass();
        let mut again = staged.working.clone();
        reapply_after_erosion(&pass, &mut again);
        let (mut iced, mut flooded) = (0_u32, 0_u32);
        for (n, (a, b)) in again.cells.iter().zip(&staged.working.cells).enumerate() {
            assert_eq!(
                bits(a.ice_fraction),
                bits(b.ice_fraction),
                "{what}: cell {n}'s ice fraction"
            );
            assert_eq!(a.ice_entry, b.ice_entry, "{what}: cell {n}'s ice");
            assert_eq!(a.liquid_entry, b.liquid_entry, "{what}: cell {n}'s liquid");
            iced += u32::from(a.ice_fraction > 0.0);
            flooded += u32::from(a.liquid_entry.is_some());
        }
        for (n, (a, b)) in again
            .climate
            .iter()
            .zip(&staged.working.climate)
            .enumerate()
        {
            assert_eq!(
                bits(a.sea_level_temperature.value()),
                bits(b.sea_level_temperature.value()),
                "{what}: climate cell {n}'s sea-level temperature"
            );
            for m in 0..12 {
                for (x, y, name) in [
                    (a.month_anomaly_k[m], b.month_anomaly_k[m], "anomaly"),
                    (
                        a.month_precipitation_kg_m2_s[m],
                        b.month_precipitation_kg_m2_s[m],
                        "precipitation",
                    ),
                    (
                        a.month_wind[m].speed.value(),
                        b.month_wind[m].speed.value(),
                        "wind speed",
                    ),
                    (
                        a.month_wind[m].azimuth.value(),
                        b.month_wind[m].azimuth.value(),
                        "wind azimuth",
                    ),
                ] {
                    assert_eq!(
                        bits(x),
                        bits(y),
                        "{what}: climate cell {n}'s month {m} {name}"
                    );
                }
            }
        }
        assert_eq!(
            bits(model_of(&again).correction.mean_k),
            bits(model_of(&staged.working).correction.mean_k),
            "{what}: the constant"
        );
        // The catch-all over every other member, the records among them, after the bits above:
        // `assert_eq!` would print two working states' `Debug`, megabytes, so the checks above
        // name what differs and this one only that something else does.
        assert!(again == staged.working, "{what}: the whole working state");
        eprintln!("{what}: {iced} iced and {flooded} flooded cells, each to the bit");
        assert!(iced > 0, "{what}: some cell holds ice");
    }

    /// With the heights the climate step saw, the re-application changes nothing, to the bit: on
    /// the small Earth (the energy-balance model, its mean re-solved) and on the reference Mars
    /// (the radiative model, its keys unchanged), each staged to the end of its climate step.
    #[test]
    fn the_re_application_at_unchanged_heights_changes_nothing() {
        let earth = Staged::of(small_earth());
        assert!(matches!(
            model_of(&earth.working).kind,
            MeanKind::Arithmetic
        ));
        assert_unchanged_heights_change_nothing(&earth, "small Earth");
        let mars = Staged::of(reference::mars_like());
        assert!(matches!(
            model_of(&mars.working).kind,
            MeanKind::Radiative { .. }
        ));
        assert_unchanged_heights_change_nothing(&mars, "reference Mars");
    }

    /// Asserts that `staged`'s open liquid is the sea's evaporation: at each cell under the
    /// condensable's own liquid, at its water surface, each month's E times its open share is the
    /// water cycle's rate there to the bit (`hydrology`'s `Evaporation::rates`), and nothing in a
    /// month whose temperature there lies below the freezing point. The cell-months compared,
    /// frozen and evaporating.
    fn assert_the_sea_s_rates(staged: &Staged, open: &OpenLiquid) -> [u32; 3] {
        let pass = staged.pass();
        let working = &staged.working;
        let model = model_of(working);
        let own = model
            .water
            .as_ref()
            .expect("the energy-balance model's own");
        let sea = hydrology::Evaporation::new(
            &pass,
            working,
            &EnergyBalance::for_inputs(pass.inputs()),
            own,
        )
        .expect("the sea evaporates");
        let freezing = open.freezing_k.expect("the liquid freezes");
        let mut rates = vec![0.0; usize::from(working.months)];
        let mut ours = rates.clone();
        let mut counts = [0_u32; 3];
        for cell in 0..pass.grid().len() {
            if !sea.rates(
                &pass,
                working,
                &model.grid,
                model.coordinates,
                cell,
                &mut rates,
            ) {
                continue;
            }
            let state = &working.cells[slot(cell)];
            let surface = state.water_surface.value();
            let point = open.point_of(&pass, cell);
            let climate = &working.climate[slot(parent_index(cell))];
            open.year_rates_kg_m2_s(working, cell, point, surface, &mut ours);
            for (m, (&rate, &e)) in rates.iter().zip(&ours).enumerate() {
                assert_eq!(
                    bits((1.0 - state.ice_fraction) * e),
                    bits(rate),
                    "cell {cell}, month {m}: the sea's rate"
                );
                counts[0] += 1;
                let t = climate.sea_level_temperature.value()
                    - working.lapse_rate_k_per_m * (surface - working.sea_level.value())
                    + climate.month_anomaly_k[m];
                if t < freezing {
                    assert_eq!(bits(e), bits(0.0), "cell {cell}, month {m}");
                    counts[1] += 1;
                } else if e > 0.0 {
                    counts[2] += 1;
                }
            }
        }
        counts
    }

    /// Q3's net loss is finite at every cell of the small Earth at its water surface, and its
    /// evaporation is the sea's ([`assert_the_sea_s_rates`]), nothing in a frozen month. At 20 km
    /// above the sea level every month freezes, so the loss is −P. On the reference Mars, whose
    /// radiative model keeps no records of its own and which has no main liquid, it is zero at
    /// every cell.
    #[test]
    fn the_net_loss_is_finite_on_the_small_earth_zero_on_mars_and_frozen_months_evaporate_nothing()
    {
        let staged = Staged::of(small_earth());
        let pass = staged.pass();
        let working = &staged.working;
        let net = NetLoss::new(&pass, working);
        let open = net
            .evaporation
            .as_ref()
            .expect("the small Earth's sea evaporates");
        let scale = net.scale.expect("the small Earth has a main liquid");
        let (mut least, mut most) = (f64::INFINITY, f64::NEG_INFINITY);
        for cell in 0..pass.grid().len() {
            let surface = working.cells[slot(cell)].water_surface.value();
            let loss = net.at(&pass, working, cell, surface);
            assert!(loss.is_finite(), "cell {cell}: {loss}");
            least = least.min(loss);
            most = most.max(loss);
            if cell % 97 == 0 {
                assert_eq!(
                    bits(open_liquid_net_loss_m_per_year(
                        &pass, working, cell, surface
                    )),
                    bits(loss),
                    "cell {cell}: the one call is the context's"
                );
            }
        }
        eprintln!("small Earth: E − P from {least:.4} to {most:.4} m a⁻¹ at the water surfaces");
        let [compared, frozen, positive] = assert_the_sea_s_rates(&staged, open);
        eprintln!(
            "small Earth: {compared} sea cell-months against the water cycle's, {frozen} frozen, \
             {positive} evaporating"
        );
        assert!(compared > 0 && frozen > 0 && positive > 0);
        let high = working.sea_level.value() + 20_000.0;
        for cell in 0..pass.grid().len() {
            assert_eq!(
                bits(net.evaporation_kg_m2_s(&pass, working, cell, high)),
                bits(0.0),
                "cell {cell}: nothing evaporates frozen"
            );
            assert_eq!(
                bits(net.at(&pass, working, cell, high)),
                bits((0.0 - net.precipitation_kg_m2_s(working, cell)) * scale),
                "cell {cell}: the loss is −P"
            );
        }

        let mars = Staged::of(reference::mars_like());
        let pass = mars.pass();
        let net = NetLoss::new(&pass, &mars.working);
        assert!(net.evaporation.is_none() && net.scale.is_none());
        assert!(model_of(&mars.working).water.is_none());
        for (cell, state) in (0_u32..).zip(&mars.working.cells) {
            let surface = state.water_surface.value();
            assert_eq!(
                bits(open_liquid_net_loss_m_per_year(
                    &pass,
                    &mars.working,
                    cell,
                    surface
                )),
                bits(0.0),
                "Mars cell {cell}"
            );
        }
    }

    /// `staged`'s climate step left its records: none in the initial state (`Staged::of`), and
    /// after the step the model of the world's regime, its constant the step's and its keys, read
    /// again, the keys the step placed the ices by, to the bit. Their size, bytes, which it
    /// prints.
    fn assert_records_kept(staged: &Staged, what: &str) -> usize {
        let pass = staged.pass();
        let records = staged
            .working
            .climate_records
            .as_ref()
            .expect("the climate step keeps its records");
        let model = &records.model;
        let kind = staged.inputs.climate().model;
        let arithmetic = match kind {
            ClimateModelKind::EnergyBalance
            | ClimateModelKind::LockedEnergyBalance
            | ClimateModelKind::Isothermal => true,
            ClimateModelKind::RadiativeEquilibrium => false,
        };
        assert_eq!(
            matches!(model.kind, MeanKind::Arithmetic),
            arithmetic,
            "{what}: the kind of {kind:?}"
        );
        assert_eq!(
            model.water.is_some(),
            matches!(
                kind,
                ClimateModelKind::EnergyBalance | ClimateModelKind::LockedEnergyBalance
            ),
            "{what}: own records exactly on the energy-balance models"
        );
        assert_eq!(model.correction, staged.diagnostics.correction, "{what}");
        let (_, keys) = cell_temperatures(&pass, &staged.working, model, &mut 0);
        assert_eq!(keys.len(), staged.diagnostics.keys.len(), "{what}");
        for (n, (a, b)) in keys.iter().zip(&staged.diagnostics.keys).enumerate() {
            assert_eq!(
                (bits(a.radiative_mean_k), bits(a.warmest_k)),
                (bits(b.radiative_mean_k), bits(b.warmest_k)),
                "{what}: cell {n}'s key"
            );
        }
        let values = record_values(records);
        let bytes = values * size_of::<f64>() + size_of::<ClimateRecords>();
        eprintln!(
            "{what}: {kind:?} on {} points, {values} values in the records' point arrays, {bytes} B \
             ({:.3} MB)",
            model.grid.points(),
            f64::from(u32::try_from(bytes).expect("the records' bytes fit a u32")) / 1e6
        );
        bytes
    }

    /// Every reference world's climate step keeps its records, and the initial state none:
    /// Ceres, the Moon, the reference Mars and the small Earth, in the reference Earth's place
    /// (decision-r09-t13-test-cost §2, (e); the reference Earth itself is
    /// [`the_reference_earth_s_records_are_kept_and_measured`]'s).
    #[test]
    fn the_climate_step_keeps_its_records_on_every_reference_world() {
        for (what, inputs) in [
            ("Ceres", reference::ceres_like()),
            ("Moon", reference::moon_like()),
            ("reference Mars", reference::mars_like()),
            ("small Earth", small_earth()),
        ] {
            let _bytes = assert_records_kept(&Staged::of(inputs), what);
        }
    }

    /// The reference Earth's climate step, at level 8, keeps its records, and they are the size
    /// the module documents: 259,200 values on its 72 × 72 points, 2.07 MB.
    #[test]
    #[ignore = "slow: the reference Earth's coarse pass to the end of its climate step at level 8, \
                about 20 s"]
    fn the_reference_earth_s_records_are_kept_and_measured() {
        let staged = Staged::of(reference::earth_like());
        let bytes = assert_records_kept(&staged, "reference Earth");
        let records = staged
            .working
            .climate_records
            .as_ref()
            .expect("the climate step keeps its records");
        assert_eq!(record_values(records), 259_200);
        assert!((2_070_000..2_090_000).contains(&bytes), "{bytes} B");
    }
}
