//! The precipitation heuristic and the winds, the climate step's last stage (R09.T13.e; Design
//! note 8, decision-r09-t13ge §2): each month's zonal moisture budget from the energy-balance
//! model's own records, evaporation over the climate condensable's open liquid, the large-scale
//! rate, the winds, the orographic precipitation over land along them and the global water balance,
//! written to the climate cells. Labelled a heuristic in the header
//! (`PrecipitationSource::Heuristic`).
//!
//! # Per month (one, or twelve)
//!
//! 1. **The zonal budget** ([`partition`](super::partition)): each band's mean h of the model's own
//!    monthly record, its meridional flux in the model's flux form, the energy-flux equator, the
//!    centre held at the ascending branch's cap (rotating worlds) or the substellar convergence
//!    (locked ones), Siler et al.'s partition and each band's E − P.
//! 2. **Evaporation** ([`evaporation`](super::evaporation)) over each field cell under the climate
//!    condensable's own liquid (its `liquid_entry` the palette's `Liquid` entry of the condensable),
//!    at or above the model's freezing point at its water surface: min(1, α s ÷ (s + γ)) ×
//!    max(0, β `S_abs` − `C_sea` dT/dt) ÷ L, times 1 − its ice fraction. Its temperature is the
//!    normalised field's, its climate cell's month at its water surface; `S_abs` its grid point's
//!    month insolation times one minus the albedo at the model's own record at the point's water
//!    surface, T − Γ `z̄_w` (decision-r09-t13ge §2.11), the point the cell's centre lies in, as the
//!    sea shares bin it; the storage the sea's heat capacity times the centred change of the
//!    cell's record over the neighbouring months' mid-times, none in a one-month year.
//! 3. **The large-scale rate:** each band's mean evaporation Ē over all of its cells (land none),
//!    `P_ls` = max(0, Ē − (E − P)), read at each sea cell by the zonal interpolant in the model's
//!    coordinates, held to its stencil's range. A band holding no cell takes the nearest band's Ē,
//!    equatorward first.
//! 4. **The winds** ([`wind`](super::wind)) at each climate cell, quantised and decoded into its
//!    record.
//! 5. **The orography** ([`lfpm`](super::lfpm)): the land cells' rate along each field cell's
//!    climate cell's decoded wind, the sea's `P_ls` the boundary.
//! 6. **The water balance** (decision-r09-t13ge §2.8, e8): one factor k a month, k = Σ E ÷
//!    (Σ `P_ls` over the sea + Σ (1 − ε) P over the land), solid-angle sums in cell-index order,
//!    applied to every cell's rate, so that the global precipitation equals the liquid's
//!    evaporation plus the land's recycled ε P, which eqs. 42–43 return to the vapour; exact,
//!    since the orographic model is linear in its inflow. With no evaporation the month rains
//!    nowhere (a dry world, or one whose liquid is all frozen), while ψ and the winds stand; its
//!    orographic solve is not run.
//! 7. **The records:** each climate cell's month rate the solid-angle mean of its four field
//!    cells'.
//!
//! A month whose every field cell's wind is calm (E-L6) carries nothing inland under the
//! orographic model's rule; there the land takes the zonal `P_ls` in place, as the sea does, with
//! its recycled share ε(H): with no wind the zonal budget's rain falls where it forms. Past the
//! year's months every record is zero and calm.
//!
//! The radiative and isothermal models get no precipitation and calm winds ([`calm`];
//! decision-r09-t13ge §2.7, e7): right for an airless world and close for a Venus, and missing a
//! thin air's winds and a liquid under either model (deferred, E4 and E5).
//!
//! Every loop runs in cell-index order (months outermost, then the field's or the climate layer's
//! cells), every sum in that order, in `f64`, every transcendental through [`crate::math`], with no
//! `HashMap` and no threads.

use hyperion_surface::field::{PaletteRole, Wind};

use super::coordinates::Coordinates;
use super::ebm::{EnergyBalance, Freezing, Moisture};
use super::grid::{BandStencil, LATITUDE_BANDS, ModelGrid};
use super::lfpm::{self, CellWind, Cover, Mesh, Month, Parameters};
use super::partition::{self, Bands, Budget, Circulation, DEGREE_RAD, Turning};
use super::seas;
use super::wind::{self, LOWER_BRANCH_DEPTH_SHARE};
use super::{Normalised, month_shares};
use crate::math;
use crate::planetary::surface::grid::parent_index;
use crate::planetary::surface::steps::{Pass, WindSample, Working};

/// An energy-balance model's own records, which the water cycle reads (decision-r09-t13ge §2.5,
/// §2.11): kept before R09.T13.d's correction, whose shifts are no energy budget.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OwnRecords {
    /// Each month's mean sea-level temperature at each grid point, K, before the normalisation.
    pub(super) months_k: Vec<Vec<f64>>,
    /// Each month's mean insolation at each grid point, W m⁻² ([`Sunlight::month_means`]).
    ///
    /// [`Sunlight::month_means`]: super::sun::Sunlight::month_means
    pub(super) insolation_w_m2: Vec<Vec<f64>>,
    /// Each grid point's `z̄_w`, its flooded cells' mean water-surface height above the sea level,
    /// m; zero where it has none.
    pub(super) water_m: Vec<f64>,
}

/// What one month's water cycle did that is no output.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MonthCycle {
    /// The month's zonal budget.
    pub(super) budget: Budget,
    /// Each band's large-scale rate before the balance, kg m⁻² s⁻¹.
    pub(super) large_scale_kg_m2_s: Vec<f64>,
    /// Each band's mean evaporation Ē, kg m⁻² s⁻¹.
    pub(super) band_evaporation_kg_m2_s: Vec<f64>,
    /// The balance's factor k; zero where nothing evaporates.
    pub(super) scale: f64,
    /// The open liquid's evaporation, the global mean, kg m⁻² s⁻¹.
    pub(super) liquid_evaporation_kg_m2_s: f64,
    /// The land's recycled ε P after the balance, the global mean, kg m⁻² s⁻¹.
    pub(super) recycled_kg_m2_s: f64,
    /// The precipitation after the balance, the global mean, kg m⁻² s⁻¹.
    pub(super) precipitation_kg_m2_s: f64,
    /// Whether every field cell's wind was calm (the month's land then took the zonal rate).
    pub(super) calm: bool,
    /// The orographic solve's diagnostics, or `None` where none ran (nothing evaporated, no land,
    /// or a wholly calm month).
    pub(super) orographic: Option<lfpm::Diagnostics>,
}

/// What a run's water cycle did that is no output, for its tests.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Diagnostics {
    /// The body's Hadley circulation: its edge, width and cap.
    pub(super) circulation: Circulation,
    /// The annual energy-flux equator's sine latitude, in the model's coordinates.
    pub(super) annual_efe_sine: f64,
    /// Each month's cycle.
    pub(super) months: Vec<MonthCycle>,
    /// Each month's winds, one a climate cell, as the record codes them: the winds that month's
    /// orographic step read.
    pub(super) winds: Vec<Vec<Wind>>,
}

/// Writes no precipitation and calm winds to every climate cell of `working`: the radiative and
/// isothermal models' (decision-r09-t13ge §2.7, e7).
pub(super) fn calm(working: &mut Working) {
    for state in &mut working.climate {
        state.month_precipitation_kg_m2_s = [0.0; 12];
        state.month_wind = [WindSample::CALM; 12];
    }
}

/// The water cycle of `model` (an energy-balance model's normalised answer with its own records)
/// on `working` (the [module documentation](self)): each month's precipitation and wind written to
/// every climate cell, and what it did. `None`, with every record zero and calm, for a model that
/// keeps no records of its own (the radiative and isothermal models).
///
/// # Panics
///
/// If a value comes out not finite (a fault of the model), or the model's months are not the
/// working state's.
pub(super) fn run(
    pass: &Pass<'_>,
    working: &mut Working,
    model: &Normalised,
) -> Option<Diagnostics> {
    let Some(own) = &model.water else {
        calm(working);
        return None;
    };
    let turning = match model.coordinates {
        Coordinates::Geographic => Turning::Rotating,
        Coordinates::TidallyLocked => Turning::Locked,
    };
    Some(water_cycle(
        pass,
        working,
        &model.grid,
        model.coordinates,
        own,
        turning,
    ))
}

/// Each field cell's band of `grid` in `coordinates`, by its centre's sine latitude in the model's
/// frame, and each band's solid angle, summed in cell-index order.
#[must_use]
fn cell_bands(pass: &Pass<'_>, coordinates: Coordinates) -> (Vec<usize>, Vec<f64>) {
    let field = pass.grid();
    let mut areas = vec![0.0; LATITUDE_BANDS];
    let bands = field
        .centres()
        .iter()
        .zip(field.solid_angles_sr())
        .map(|(centre, &omega)| {
            let band = ModelGrid::band_of(coordinates.to_model(*centre)[2]);
            areas[band] += omega;
            band
        })
        .collect();
    (bands, areas)
}

/// `sums` (one a band) over `areas`, each band's mean, a band of no area taking the nearest
/// band's that has one, equatorward first (T13.a's rule); zero where none has.
#[must_use]
fn band_means(sums: &[f64], areas: &[f64]) -> Vec<f64> {
    let mean = |band: usize| (areas[band] > 0.0).then(|| sums[band] / areas[band]);
    (0..LATITUDE_BANDS)
        .map(|band| {
            mean(band)
                .or_else(|| {
                    (1..LATITUDE_BANDS).find_map(|distance| {
                        let (equatorward, poleward) = if band < LATITUDE_BANDS / 2 {
                            (band.checked_add(distance), band.checked_sub(distance))
                        } else {
                            (band.checked_sub(distance), band.checked_add(distance))
                        };
                        [equatorward, poleward]
                            .into_iter()
                            .flatten()
                            .filter(|&b| b < LATITUDE_BANDS)
                            .find_map(mean)
                    })
                })
                .unwrap_or(0.0)
        })
        .collect()
}

/// The mass flux `edges` (one a band edge of `grid`) at the sine latitude `x`, linear in it
/// between the edges of the band that holds it.
#[must_use]
fn at_edges(grid: &ModelGrid, edges: &[f64], x: f64) -> f64 {
    let band = ModelGrid::band_of(x);
    let (south, north) = (grid.edge_sine(band), grid.edge_sine(band + 1));
    let t = ((x - south) / (north - south)).clamp(0.0, 1.0);
    edges[band] + (edges[band + 1] - edges[band]) * t
}

/// What a run's evaporation reads (the [module documentation](self)'s stage 2): the climate
/// condensable, its liquid's palette entry and the model's freezing point, each grid point's
/// absorbed sunlight at its water surface each month, and each month's centred span of time.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Evaporation {
    /// The model, whose heat capacity and air the rate reads.
    model: EnergyBalance,
    /// The climate condensable.
    moisture: Moisture,
    /// The palette's `Liquid` entry of the condensable.
    entry: u8,
    /// The freezing point below which the liquid gives nothing, K; `None` where no ice forms.
    freezing_k: Option<f64>,
    /// Each month's absorbed sunlight at each grid point's water surface, W m⁻²: the month's
    /// insolation × (1 − α(T − Γ `z̄_w`)) at the model's own record T.
    absorbed_w_m2: Vec<Vec<f64>>,
    /// Each month's span from the previous month's middle to the next's, s: (½ s₋ + s + ½ s₊) P of
    /// the shares s; none in a one-month year.
    spans_s: Vec<f64>,
}

impl Evaporation {
    /// The evaporation of `model` (the inputs' energy-balance model) over `working`'s liquids, from
    /// the model's own records `own`: `None` on a dry world, one whose condensable is not among its
    /// liquids or has no `Liquid` entry, which evaporates nothing.
    #[must_use]
    pub(super) fn new(
        pass: &Pass<'_>,
        working: &Working,
        model: &EnergyBalance,
        own: &OwnRecords,
    ) -> Option<Self> {
        let inputs = pass.inputs();
        let moisture = model.moisture?;
        let entry = inputs
            .climate()
            .condensable
            .and_then(|key| inputs.palette().find(PaletteRole::Liquid, key))?;
        let lapse = working.lapse_rate_k_per_m;
        let absorbed_w_m2 = own
            .months_k
            .iter()
            .zip(&own.insolation_w_m2)
            .map(|(t_k, insolation)| {
                t_k.iter()
                    .zip(insolation)
                    .zip(&own.water_m)
                    .map(|((&t, &q), &z)| q * (1.0 - model.albedo_at(t - lapse * z)))
                    .collect()
            })
            .collect();
        let months = own.months_k.len();
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
            model: *model,
            moisture,
            entry,
            freezing_k: match model.freezing {
                Freezing::At { point_k } => Some(point_k),
                Freezing::Never { .. } => None,
            },
            absorbed_w_m2,
            spans_s,
        })
    }

    /// Field cell `cell`'s evaporation rate each month, kg m⁻² s⁻¹, into `rates` (one a month),
    /// on `grid` in `coordinates`: whether it is under the condensable's own liquid, the rates zero
    /// where not. A month is zero where the liquid's temperature, its climate cell's month at its
    /// water surface, lies below the freezing point; else min(1, α s ÷ (s + γ)) × max(0, β `S_abs` −
    /// `C_sea` dT/dt) ÷ L × (1 − its ice fraction), `S_abs` its grid point's.
    pub(super) fn rates(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        grid: &ModelGrid,
        coordinates: Coordinates,
        cell: u32,
        rates: &mut [f64],
    ) -> bool {
        rates.fill(0.0);
        let field_cell = &working.cells[slot(cell)];
        if !seas::under_sea(field_cell) || field_cell.liquid_entry != Some(self.entry) {
            return false;
        }
        let point = grid.point_of(coordinates.to_model(pass.grid().centre(cell)));
        let state = &working.climate[slot(parent_index(cell))];
        let twelve = rates.len() == 12;
        let anomaly = |m: usize| {
            if twelve {
                state.month_anomaly_k[m]
            } else {
                0.0
            }
        };
        let surface = state.sea_level_temperature.value()
            - working.lapse_rate_k_per_m
                * (field_cell.water_surface.value() - working.sea_level.value());
        let open = 1.0 - field_cell.ice_fraction;
        let latent = self.moisture.latent_heat_j_per_kg;
        for (m, rate) in rates.iter_mut().enumerate() {
            let t_w = surface + anomaly(m);
            if self.freezing_k.is_some_and(|f| t_w < f) {
                continue;
            }
            let coefficient = super::evaporation::coefficient(
                &self.moisture,
                self.model.specific_heat_j_per_kg_k,
                t_w,
            );
            let storage = if twelve {
                let (before, after) = ((m + 11) % 12, (m + 1) % 12);
                self.model.heat_capacity(t_w, 1.0) * (anomaly(after) - anomaly(before))
                    / self.spans_s[m]
            } else {
                0.0
            };
            *rate = open
                * super::evaporation::rate_kg_m2_s(
                    coefficient,
                    self.absorbed_w_m2[m][point],
                    storage,
                    latent,
                );
        }
        true
    }
}

/// The precipitation heuristic's water cycle on `grid` in `coordinates` over the model's own
/// records `own`, a circulation that turns as `turning` says (the [module documentation](self)).
#[expect(
    clippy::too_many_lines,
    reason = "one month's pipeline of seven stages in order, each a few lines over the same cells; \
              split, its stages would pass the same dozen arrays between them"
)]
fn water_cycle(
    pass: &Pass<'_>,
    working: &mut Working,
    grid: &ModelGrid,
    coordinates: Coordinates,
    own: &OwnRecords,
    turning: Turning,
) -> Diagnostics {
    let inputs = pass.inputs();
    let field = pass.grid();
    let model = EnergyBalance::for_inputs(inputs);
    let circulation = Circulation::for_inputs(inputs, turning);
    let months = own.months_k.len();
    assert!(
        months == usize::from(working.months),
        "the model's {months} months are the working state's {}",
        working.months
    );
    let shares = month_shares(pass, months);

    // 1. The zonal budgets.
    let bands: Vec<Bands> = own
        .months_k
        .iter()
        .map(|t_k| Bands::of(&model, grid, t_k))
        .collect();
    let budgets: Vec<Budget> = bands
        .iter()
        .map(|b| partition::budget(grid, &model, &circulation, b))
        .collect();
    let annual = Bands::mean(&bands, &shares);
    let annual_flux = partition::edge_fluxes(grid, model.transport_kg_m2_s, &annual.energy);
    let annual_efe_sine = partition::energy_flux_equator(grid, &annual_flux, &annual.energy);

    // 2. The evaporation.
    let sea_level = working.sea_level.value();
    let (cell_band, band_area) = cell_bands(pass, coordinates);
    let mut band_evaporation = vec![vec![0.0; LATITUDE_BANDS]; months];
    let mut liquid_evaporation = vec![0.0; months];
    if let Some(evaporation) = Evaporation::new(pass, working, &model, own) {
        let mut rates = vec![0.0; months];
        for (n, &omega) in (0_u32..).zip(field.solid_angles_sr()) {
            if !evaporation.rates(pass, working, grid, coordinates, n, &mut rates) {
                continue;
            }
            for (m, &e) in rates.iter().enumerate() {
                band_evaporation[m][cell_band[slot(n)]] += omega * e;
                liquid_evaporation[m] += omega * e;
            }
        }
    }

    // 3–7, month by month.
    let sphere: f64 = field.solid_angles_sr().iter().sum();
    let radius_m = inputs.radius().value();
    let gravity = inputs.gravity().value();
    let depth_pa = LOWER_BRANCH_DEPTH_SHARE * inputs.surface_pressure().value();
    let rotation_s = inputs.spin().rotation_period.value();
    let pole = coordinates.to_body([0.0, 0.0, 1.0]);
    let parameters = Parameters::HERGARTEN_ROBL;
    let heights_m: Vec<f64> = working
        .cells
        .iter()
        .map(|cell| cell.water_surface.value() - sea_level)
        .collect();
    let cover: Vec<Cover> = working
        .cells
        .iter()
        .map(|cell| {
            if seas::under_sea(cell) {
                Cover::Sea
            } else {
                Cover::Land
            }
        })
        .collect();
    let land = cover.contains(&Cover::Land);
    // Each field cell's local frame and its zonal stencil in the model's coordinates, the same
    // every month.
    let frames: Vec<([f64; 3], [f64; 3])> = field
        .centres()
        .iter()
        .map(|&up| wind::east_north(up))
        .collect();
    let stencils: Vec<BandStencil> = field
        .centres()
        .iter()
        .map(|&centre| BandStencil::at(coordinates.to_model(centre)[2]))
        .collect();
    let mut mesh: Option<Mesh> = None;
    let mut cycles = Vec::with_capacity(months);
    let mut winds_used = Vec::with_capacity(months);
    let mut rates = vec![0.0; field.centres().len()];
    let mut sea_rates = vec![0.0; rates.len()];
    for (m, budget) in budgets.into_iter().enumerate() {
        let band_e = band_means(&band_evaporation[m], &band_area);
        let large_scale: Vec<f64> = band_e
            .iter()
            .zip(&budget.evaporation_less_precipitation_kg_m2_s)
            .map(|(&e, &emp)| {
                let rate = e - emp;
                if rate > 0.0 { rate } else { 0.0 }
            })
            .collect();

        // 4. The winds.
        let summer_edge_deg = {
            let reach = budget.centre_sine.abs() + circulation.edge_sine();
            if reach < 1.0 {
                math::asin(reach) / DEGREE_RAD
            } else {
                90.0
            }
        };
        let mut codes = Vec::with_capacity(working.climate.len());
        for (state, &up) in working
            .climate
            .iter_mut()
            .zip(pass.climate_grid().centres())
        {
            let x = coordinates.to_model(up)[2];
            let psi = at_edges(grid, &budget.mass_flux_kg_m2_s, x);
            let mut v = wind::lower_branch_m_s(psi, x, radius_m, gravity, depth_pa);
            if turning == Turning::Rotating && x.abs() > 0.0 {
                let summer =
                    budget.centre_sine.abs() > 0.0 && (x > 0.0) == (budget.centre_sine > 0.0);
                let edge = if summer {
                    summer_edge_deg
                } else {
                    circulation.edge_deg()
                };
                let poleward = wind::extratropical_m_s(math::asin(x.abs()) / DEGREE_RAD, edge);
                v += if x > 0.0 { poleward } else { -poleward };
            }
            let (u, _) = wind::drag_balance(v, wind::coriolis_per_s(rotation_s, up));
            let velocity = wind::towards_pole(pole, up).map_or([0.0; 3], |north| {
                let across = wind::cross(north, up);
                [
                    v * north[0] + u * across[0],
                    v * north[1] + u * across[1],
                    v * north[2] + u * across[2],
                ]
            });
            let (sample, code) = wind::recorded(velocity, up);
            state.month_wind[m] = sample;
            codes.push(code);
        }

        // 5. The large-scale rate over the sea, and the winds the orography reads: each field
        // cell's wind is its climate cell's record read at its own centre (`wind::cell_wind`, with
        // the cell's frame and the record's heading computed once).
        let headings: Vec<wind::Heading> = working
            .climate
            .iter()
            .map(|state| wind::Heading::of(&state.month_wind[m]))
            .collect();
        let cell_winds: Vec<CellWind> = (0_u32..)
            .zip(&frames)
            .map(|(n, &frame)| wind::cell_wind_in(&headings[slot(parent_index(n))], frame))
            .collect();
        let still = cell_winds.iter().all(|&w| w == CellWind::Calm);
        for ((rate, stencil), &c) in sea_rates.iter_mut().zip(&stencils).zip(&cover) {
            let reads = c == Cover::Sea || still;
            *rate = if reads {
                stencil.clamped(&large_scale)
            } else {
                0.0
            };
        }
        let evaporated = liquid_evaporation[m];
        let mut orographic = None;
        let mut recycled_shares: Option<Vec<f64>> = None;
        rates.fill(0.0);
        if evaporated > 0.0 && land {
            if still {
                recycled_shares = Some(
                    heights_m
                        .iter()
                        .zip(&cover)
                        .map(|(&z, &c)| match c {
                            Cover::Land => parameters.recycled_share(z),
                            Cover::Sea => 0.0,
                        })
                        .collect(),
                );
                for ((rate, &sea_rate), &c) in rates.iter_mut().zip(&sea_rates).zip(&cover) {
                    if c == Cover::Land {
                        *rate = sea_rate;
                    }
                }
            } else {
                let mesh = mesh.get_or_insert_with(|| Mesh::of_graph(field, inputs.radius()));
                let solved = lfpm::solve(
                    mesh,
                    &Month {
                        heights_m: &heights_m,
                        cover: &cover,
                        winds: &cell_winds,
                        sea_rates_kg_m2_s: &sea_rates,
                    },
                    &parameters,
                );
                rates.copy_from_slice(&solved.precipitation_kg_m2_s);
                recycled_shares = Some(solved.recycled_shares);
                orographic = Some(solved.diagnostics);
            }
        }

        // 6. The water balance.
        let mut delivered = 0.0;
        for (((&c, &rate), &sea_rate), (n, &omega)) in cover
            .iter()
            .zip(&rates)
            .zip(&sea_rates)
            .zip(field.solid_angles_sr().iter().enumerate())
        {
            delivered += omega
                * match c {
                    Cover::Sea => sea_rate,
                    Cover::Land => {
                        let share = recycled_shares.as_ref().map_or(0.0, |shares| shares[n]);
                        (1.0 - share) * rate
                    }
                };
        }
        let scale = if evaporated > 0.0 && delivered > 0.0 {
            evaporated / delivered
        } else {
            0.0
        };
        let (mut fallen, mut recycled) = (0.0, 0.0);
        for (((rate, &sea_rate), &c), (n, &omega)) in rates
            .iter_mut()
            .zip(&sea_rates)
            .zip(&cover)
            .zip(field.solid_angles_sr().iter().enumerate())
        {
            *rate = scale
                * match c {
                    Cover::Sea => sea_rate,
                    Cover::Land => *rate,
                };
            assert!(
                rate.is_finite(),
                "a precipitation rate must be finite, got {rate}"
            );
            fallen += omega * *rate;
            if c == Cover::Land {
                let share = recycled_shares.as_ref().map_or(0.0, |shares| shares[n]);
                recycled += omega * share * *rate;
            }
        }

        // 7. The records.
        let mut sums = vec![(0.0, 0.0); working.climate.len()];
        for ((n, &rate), &omega) in (0_u32..).zip(&rates).zip(field.solid_angles_sr()) {
            let sum = &mut sums[slot(parent_index(n))];
            sum.0 += omega * rate;
            sum.1 += omega;
        }
        for (state, &(rained, area)) in working.climate.iter_mut().zip(&sums) {
            state.month_precipitation_kg_m2_s[m] = if area > 0.0 { rained / area } else { 0.0 };
        }
        cycles.push(MonthCycle {
            budget,
            large_scale_kg_m2_s: large_scale,
            band_evaporation_kg_m2_s: band_e,
            scale,
            liquid_evaporation_kg_m2_s: evaporated / sphere,
            recycled_kg_m2_s: recycled / sphere,
            precipitation_kg_m2_s: fallen / sphere,
            calm: still,
            orographic,
        });
        winds_used.push(codes);
    }
    for state in &mut working.climate {
        for m in months..12 {
            state.month_precipitation_kg_m2_s[m] = 0.0;
            state.month_wind[m] = WindSample::CALM;
        }
    }
    Diagnostics {
        circulation,
        annual_efe_sine,
        months: cycles,
        winds: winds_used,
    }
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
