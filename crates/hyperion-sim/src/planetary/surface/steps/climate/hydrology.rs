//! The precipitation heuristic and the winds, the climate step's last stage (R09.T13.e; Design
//! note 8, decision-r09-t13ge §2): each month's zonal moisture budget from the energy-balance
//! model's own records, evaporation over the climate condensable's open liquid, the sea's rate and
//! the land's source, the winds, the orographic precipitation over land along the model's bands
//! and the global water balance, written to the climate cells (R09.T13.j's revision,
//! decision-r09-t13e-revision, gives the sea its own evaporation and the land the zonal budget's
//! convergence). Labelled a heuristic in the header
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
//!    over its open months, those at or above the model's freezing point at its water surface:
//!    Priestley and Taylor's year with no storage on β `S_abs` + `Q_O`, its months the Penman
//!    split's shape (decision-r09-t13i §3.2), times 1 − its ice fraction. Its temperature is the
//!    normalised field's, its climate cell's month at its water surface; `S_abs` its grid point's
//!    month insolation times one minus the albedo at the model's own record at the point's water
//!    surface, T − Γ `z̄_w` (decision-r09-t13ge §2.11), the point the cell's centre lies in, as the
//!    sea shares bin it; `Q_O` that point's ocean heat convergence per unit of its sea; the
//!    storage the sea's heat capacity times the centred change of the cell's record over the
//!    neighbouring months' mid-times, none in a one-month year. The field cells of a climate cell
//!    at one water surface share their months' coefficient, storage and `q_s`, to the bit
//!    ([`ClimateCellTerms`]; R09.T16.a).
//! 3. **The sea's rate and the land's source** ([`SeaBands`]; decision-r09-t13e-revision §4.1,
//!    §4.2). Each band's evaporation per unit of its sea's solid angle, `e_b` = Σ Ω E over its
//!    cells ÷ Σ Ω over its sea cells (`seas::under_sea`'s, every liquid), and its sea share `s_b`,
//!    a band with no sea taking its nearest band's that has one, equatorward first; its
//!    convergence `C_b` = −(E − P); and its sea's rate `P_sea` = max(0, `e_b` + `C_b`) where
//!    `C_b` ≥ 0, else max(0, `e_b` + `C_b` ÷ `s_b`): a sea column rains its own evaporation plus
//!    the band's convergence, and a divergent band's export falls on its sea alone, since its land
//!    exports nothing. `P_sea` is read at each sea cell, and max(0, `C_b`) at each land cell, by
//!    the zonal interpolant in the model's coordinates, held to its stencil's range. On an
//!    aquaplanet `e_b` is the band mean Ē and `s_b` is 1, so `P_sea` is max(0, Ē − (E − P)) to the
//!    bit.
//! 4. **The winds** ([`wind`](super::wind)) at each climate cell, quantised and decoded into its
//!    record.
//! 5. **The orography** ([`lfpm`](super::lfpm)): the land cells' rate along each field cell's
//!    climate cell's decoded wind's component along the model's bands, east, west or calm by its
//!    sign ([`wind::along_bands`]; decision-r09-t13e-revision §4.3), since the zonal budget
//!    already carries the meridional transport, which the wind's meridional part would count
//!    again over land: its E − P is the divergence of the meridional water flux, Hadley and eddy
//!    (Siler, Roe and Armour 2018, J. Climate 31, 7481, the text after their eq. 9, and their
//!    Fig. 2f). The sea's `P_sea` is the boundary, each land cell taking its
//!    convergence as a vapour source scaled by e^(−H/H₀), and recycling nothing in a month whose
//!    temperature at its surface, its climate cell's normalised month at its height, lies below
//!    the condensable's freezing point ([`ground_of`]; §4.4). A wholly calm month needs no rule of
//!    its own: every land cell is then a sink with its source (§4.5).
//! 6. **The water balance** (decision-r09-t13ge §2.8, e8): one factor k a month, k = Σ E ÷
//!    (Σ `P_sea` over the sea + Σ (1 − ε) P over the land), ε the month's on each land cell,
//!    solid-angle sums in cell-index order, applied to every cell's rate, so that the global
//!    precipitation equals the liquid's evaporation plus the land's recycled ε P, which eqs. 42–43
//!    return to the vapour; exact, since the orographic model is linear in its inflow and its
//!    source. With no evaporation the month rains nowhere (a dry world, or one whose liquid is all
//!    frozen), while ψ and the winds stand; its orographic solve is not run.
//! 7. **The records:** each climate cell's month rate the solid-angle mean of its four field
//!    cells'.
//!
//! Past the year's months every record is zero and calm.
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
use super::evaporation::ClimateCellTerms;
#[cfg(test)]
use super::evaporation::OpenMonth;
use super::grid::{BandStencil, LATITUDE_BANDS, ModelGrid};
use super::lfpm::{self, CellWind, Cover, GroundState, Mesh, Month, Parameters};
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
    /// Each month's ocean heat convergence at each grid point per unit of its sea's area, W m⁻²
    /// (R09.T13.i; decision-r09-t13i §3.2's `Q_O`): the kept run's record of its Ekman cells and
    /// gyres ([`OceanRecord`]) over the point's sea share, zero at a point with no sea; `None`
    /// where no sea carries the term.
    ///
    /// [`OceanRecord`]: super::ebm::OceanRecord
    pub(super) ocean_w_m2: Option<Vec<Vec<f64>>>,
}

/// What one month's water cycle did that is no output.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MonthCycle {
    /// The month's zonal budget.
    pub(super) budget: Budget,
    /// Each band's sea: its evaporation per sea area, its sea share, its rate before the balance
    /// and its convergence, the land's source (decision-r09-t13e-revision §4.7).
    pub(super) sea: SeaBands,
    /// Each band's mean evaporation Ē over all of its cells, kg m⁻² s⁻¹.
    pub(super) band_evaporation_kg_m2_s: Vec<f64>,
    /// The balance's factor k; zero where nothing evaporates.
    pub(super) scale: f64,
    /// The open liquid's evaporation, the global mean, kg m⁻² s⁻¹.
    pub(super) liquid_evaporation_kg_m2_s: f64,
    /// The land's recycled ε P after the balance, the global mean, kg m⁻² s⁻¹.
    pub(super) recycled_kg_m2_s: f64,
    /// The precipitation after the balance, the global mean, kg m⁻² s⁻¹.
    pub(super) precipitation_kg_m2_s: f64,
    /// The land's frozen share of its solid angle: its cells whose ground is frozen this month;
    /// zero on a world of no land.
    pub(super) frozen_land_share: f64,
    /// The field cells' orographic winds by their sense along the model's bands
    /// ([`wind::along_bands`]): towards local east, towards local west, and calm.
    pub(super) band_winds: BandWinds,
    /// Whether every field cell's orographic wind was calm (its land then a sink each, taking its
    /// source in place).
    pub(super) calm: bool,
    /// The orographic solve's diagnostics, or `None` where none ran (nothing evaporated, or no
    /// land).
    pub(super) orographic: Option<lfpm::Diagnostics>,
}

/// A month's field cells by the sense of their orographic wind along the model's bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) struct BandWinds {
    /// The cells whose orographic wind blows towards their local east about the model's pole.
    pub(super) eastward: u32,
    /// The cells whose orographic wind blows towards their local west.
    pub(super) westward: u32,
    /// The cells whose orographic wind is calm: a calm record, a record across the bands, or a
    /// cell at the model's pole.
    pub(super) calm: u32,
}

impl BandWinds {
    /// The count of the field cells' orographic `winds`, each cell's local east `easts`
    /// ([`wind::model_east`]).
    #[must_use]
    fn of(winds: &[CellWind], easts: &[Option<[f64; 3]>]) -> Self {
        let mut counts = Self::default();
        for (wind, east) in winds.iter().zip(easts) {
            match (wind, east) {
                (CellWind::Towards(t), Some(e))
                    if t[0] * e[0] + t[1] * e[1] + t[2] * e[2] > 0.0 =>
                {
                    counts.eastward += 1;
                }
                (CellWind::Towards(_), Some(_)) => counts.westward += 1,
                (CellWind::Towards(_), None) => {
                    unreachable!("along_bands is calm where the bands have no east")
                }
                (CellWind::Calm, _) => counts.calm += 1,
            }
        }
        counts
    }
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
    /// Each month's winds, one a climate cell, as the record codes them: the winds whose
    /// components along the model's bands that month's orographic step read.
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
    run_at(pass, working, model, &Parameters::HERGARTEN_ROBL)
}

/// [`run`] with the orographic model's `parameters` in place of [`Parameters::HERGARTEN_ROBL`]:
/// for the tests' deep solves (decision-r09-t13e-lfpm-convergence §3.5).
#[cfg(test)]
pub(super) fn run_with(
    pass: &Pass<'_>,
    working: &mut Working,
    model: &Normalised,
    parameters: &Parameters,
) -> Option<Diagnostics> {
    run_at(pass, working, model, parameters)
}

/// [`run`] at the orographic model's `parameters`.
fn run_at(
    pass: &Pass<'_>,
    working: &mut Working,
    model: &Normalised,
    parameters: &Parameters,
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
        parameters,
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

/// The band nearest `band` for which `holds` holds, `band` itself first, then outward, equatorward
/// before poleward at each distance (T13.a's rule); `None` where none does.
#[must_use]
pub(super) fn nearest_band(band: usize, holds: impl Fn(usize) -> bool) -> Option<usize> {
    if holds(band) {
        return Some(band);
    }
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
            .find(|&b| holds(b))
    })
}

/// `sums` (one a band) over `areas`, each band's mean, a band of no area taking the nearest
/// band's that has one, equatorward first (T13.a's rule); zero where none has.
#[must_use]
fn band_means(sums: &[f64], areas: &[f64]) -> Vec<f64> {
    (0..LATITUDE_BANDS)
        .map(|band| nearest_band(band, |b| areas[b] > 0.0).map_or(0.0, |b| sums[b] / areas[b]))
        .collect()
}

/// A month's sea and land source by band (decision-r09-t13e-revision §4.1, §4.2), one value a band
/// of the model's grid.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SeaBands {
    /// `e_b`, each band's evaporation per unit of its sea's solid angle, kg m⁻² s⁻¹: Σ Ω E over
    /// its cells (as Ē sums it) ÷ Σ Ω over its sea cells; a band with no sea its nearest band's
    /// that has one, equatorward first; zero where none has.
    pub(super) evaporation_kg_m2_s: Vec<f64>,
    /// `s_b`, each band's sea share of its solid angle; a band with no sea its nearest band's that
    /// has one, as `e_b`, so that the rate never divides by no sea; zero where none has.
    pub(super) sea_shares: Vec<f64>,
    /// `P_sea`, each band's sea's rate before the balance, kg m⁻² s⁻¹: max(0, `e_b` + `C_b`) where
    /// the convergence `C_b` = −(E − P) ≥ 0, else max(0, `e_b` + `C_b` ÷ `s_b`), the divergent
    /// band's export falling on its sea alone; zero where it diverges and no band has a sea.
    pub(super) rates_kg_m2_s: Vec<f64>,
    /// Each band's convergence max(0, `C_b`), kg m⁻² s⁻¹: the land's vapour source at the sea
    /// level.
    pub(super) convergence_kg_m2_s: Vec<f64>,
}

impl SeaBands {
    /// The month's bands from each band's evaporation `evaporation_sums_kg_m2_s_sr` (Σ Ω E over
    /// its cells, kg m⁻² s⁻¹ × sr), its solid angle `areas_sr`, its sea's `sea_areas_sr` and the
    /// zonal budget's E − P `evaporation_less_precipitation_kg_m2_s`, one each a band. Where every
    /// cell is sea, `e_b` is Ē and `s_b` is 1 to the bit, so that the rate is max(0, Ē − (E − P)).
    ///
    /// # Panics
    ///
    /// If a slice does not hold one value for each of the model's [`LATITUDE_BANDS`] bands.
    #[must_use]
    pub(super) fn of(
        evaporation_sums_kg_m2_s_sr: &[f64],
        areas_sr: &[f64],
        sea_areas_sr: &[f64],
        evaporation_less_precipitation_kg_m2_s: &[f64],
    ) -> Self {
        for (name, values) in [
            ("evaporation sums", evaporation_sums_kg_m2_s_sr),
            ("solid angles", areas_sr),
            ("sea solid angles", sea_areas_sr),
            ("E − P", evaporation_less_precipitation_kg_m2_s),
        ] {
            assert_eq!(
                values.len(),
                LATITUDE_BANDS,
                "the bands' {name} hold one value a band"
            );
        }
        let mut bands = Self {
            evaporation_kg_m2_s: Vec::with_capacity(LATITUDE_BANDS),
            sea_shares: Vec::with_capacity(LATITUDE_BANDS),
            rates_kg_m2_s: Vec::with_capacity(LATITUDE_BANDS),
            convergence_kg_m2_s: Vec::with_capacity(LATITUDE_BANDS),
        };
        for (band, &emp) in evaporation_less_precipitation_kg_m2_s.iter().enumerate() {
            let (e, s) = nearest_band(band, |b| sea_areas_sr[b] > 0.0).map_or((0.0, 0.0), |b| {
                (
                    evaporation_sums_kg_m2_s_sr[b] / sea_areas_sr[b],
                    sea_areas_sr[b] / areas_sr[b],
                )
            });
            let c = -emp;
            let rate = if c >= 0.0 {
                e + c
            } else if s > 0.0 {
                e + c / s
            } else {
                0.0
            };
            bands.evaporation_kg_m2_s.push(e);
            bands.sea_shares.push(s);
            bands
                .rates_kg_m2_s
                .push(if rate > 0.0 { rate } else { 0.0 });
            bands
                .convergence_kg_m2_s
                .push(if c > 0.0 { c } else { 0.0 });
        }
        bands
    }
}

/// Field cell `cell`'s month `m` temperature at its surface in `working`, K: its climate cell's
/// normalised month at its water surface (the ground, where it is dry), `T_sl` − Γ (`z_w` −
/// `z_sea`) + `a_m`, no anomaly in a one-month year; the temperature evaporation reads at a
/// liquid's surface and the frozen ground's test reads at the land's.
#[must_use]
fn surface_temperature_k(working: &Working, cell: u32, m: usize) -> f64 {
    let state = &working.climate[slot(parent_index(cell))];
    let surface = annual_surface_k(working, cell);
    let anomaly = if working.months == 12 {
        state.month_anomaly_k[m]
    } else {
        0.0
    };
    surface + anomaly
}

/// Field cell `cell`'s annual mean temperature at its surface in `working`, K: its climate cell's
/// at its water surface (the ground, where it is dry), `T_sl` − Γ (`z_w` − `z_sea`); a month's is
/// this plus its anomaly ([`surface_temperature_k`]).
#[must_use]
fn annual_surface_k(working: &Working, cell: u32) -> f64 {
    let field_cell = &working.cells[slot(cell)];
    let state = &working.climate[slot(parent_index(cell))];
    state.sea_level_temperature.value()
        - working.lapse_rate_k_per_m
            * (field_cell.water_surface.value() - working.sea_level.value())
}

/// Field cell `cell`'s ground in month `m` of `working`, where the model's ice forms as `freezing`
/// says (decision-r09-t13e-revision §4.4): frozen where its month temperature at its surface
/// ([`surface_temperature_k`]) lies below the freezing point, else thawed; never frozen where no
/// ice forms.
#[must_use]
pub(super) fn ground_of(working: &Working, cell: u32, m: usize, freezing: Freezing) -> GroundState {
    match freezing {
        Freezing::At { point_k } if surface_temperature_k(working, cell, m) < point_k => {
            GroundState::Frozen
        }
        Freezing::At { .. } | Freezing::Never { .. } => GroundState::Thawed,
    }
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
/// energy at its water surface each month, and each month's share and centred span of time.
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
    /// Each month's energy at each grid point's water surface, W m⁻², A = β `S_abs` + `Q_O`
    /// ([`energy_w_m2`](super::evaporation::energy_w_m2)): `S_abs` the month's insolation ×
    /// (1 − α(T − Γ `z̄_w`)) at the model's own record T, and `Q_O` the point's ocean heat per
    /// unit of its sea ([`OwnRecords::ocean_w_m2`]), none where no sea carries the term.
    energy_w_m2: Vec<Vec<f64>>,
    /// Each month's span from the previous month's middle to the next's, s: (½ s₋ + s + ½ s₊) P of
    /// the shares s; none in a one-month year.
    spans_s: Vec<f64>,
    /// Each month's share of the year.
    shares: Vec<f64>,
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
                        super::evaporation::energy_w_m2(absorbed, ocean)
                    })
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
            energy_w_m2,
            spans_s,
            shares,
        })
    }

    /// Field cell `cell`'s evaporation rate each month, kg m⁻² s⁻¹, into `rates` (one a month),
    /// on `grid` in `coordinates`: whether it is under the condensable's own liquid, the rates zero
    /// where not. A month is zero where the liquid's temperature, its climate cell's month at its
    /// water surface, lies below the freezing point; the open months take the year of
    /// [`year_rates_kg_m2_s`](super::evaporation::year_rates_kg_m2_s) on β `S_abs` + `Q_O`, the
    /// coefficient min(1, α s ÷ (s + γ)), the storage `C_sea` dT/dt and the saturation specific
    /// humidity at that temperature, times (1 − its ice fraction); `S_abs` and `Q_O` its grid
    /// point's.
    ///
    /// The oracle of [`each_open_cell`](Self::each_open_cell), which the water cycle runs: this
    /// cell alone, its month terms its own (R09.T16.a).
    ///
    /// # Panics
    ///
    /// If `rates` does not hold one value for each of `working`'s months: the storage term reads
    /// a twelve-month year from `rates` and the surface temperature from `working`, which must
    /// agree.
    #[cfg(test)]
    pub(super) fn rates(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        grid: &ModelGrid,
        coordinates: Coordinates,
        cell: u32,
        rates: &mut [f64],
    ) -> bool {
        assert_eq!(
            rates.len(),
            usize::from(working.months),
            "the evaporation's rates hold one value for each of the working state's months"
        );
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
        let open = 1.0 - field_cell.ice_fraction;
        let latent = self.moisture.latent_heat_j_per_kg;
        let mut months: [Option<OpenMonth>; 12] = [None; 12];
        for (m, month) in months.iter_mut().enumerate().take(rates.len()) {
            let t_w = surface_temperature_k(working, cell, m);
            if self.freezing_k.is_some_and(|f| t_w < f) {
                continue;
            }
            let coefficient = super::evaporation::coefficient(
                &self.moisture,
                self.model.specific_heat_j_per_kg_k,
                t_w,
            );
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
        super::evaporation::year_rates_kg_m2_s(&months[..rates.len()], latent, rates);
        for rate in rates.iter_mut() {
            *rate *= open;
        }
        true
    }

    /// Each field cell of `working` under the condensable's own liquid, in cell-index order, given
    /// to `each` with its evaporation rate each month, kg m⁻² s⁻¹, one a month, on `grid` in
    /// `coordinates`; the other cells are skipped. Each cell's rates are those of `rates`, the
    /// tests' oracle, to the bit: the field cells of one climate cell at one water surface share
    /// their months' coefficient, storage and `q_s` ([`ClimateCellTerms`]; R09.T16.a), and each
    /// cell's energy and open share stay its own.
    ///
    /// # Panics
    ///
    /// As [`each_open_cell_sharing`](Self::each_open_cell_sharing).
    fn each_open_cell(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        grid: &ModelGrid,
        coordinates: Coordinates,
        each: impl FnMut(u32, &[f64]),
    ) {
        self.each_open_cell_sharing(
            pass,
            working,
            grid,
            coordinates,
            &mut ClimateCellTerms::default(),
            each,
        );
    }

    /// [`each_open_cell`](Self::each_open_cell) with the month terms kept in `terms`, a memo made
    /// fresh for this one loop over `working`. Private to this module, so that its callers are
    /// `each_open_cell` and the tests' `assert_the_sea_rates_share_their_month_terms` alone, each
    /// with a fresh memo, which the tests read: none carries one across loops or working states,
    /// which its key does not tell apart.
    ///
    /// # Panics
    ///
    /// If `working` has more months than the model's records (`Evaporation::new`'s).
    fn each_open_cell_sharing(
        &self,
        pass: &Pass<'_>,
        working: &Working,
        grid: &ModelGrid,
        coordinates: Coordinates,
        terms: &mut ClimateCellTerms,
        mut each: impl FnMut(u32, &[f64]),
    ) {
        let months = usize::from(working.months);
        let latent = self.moisture.latent_heat_j_per_kg;
        let mut year = [0.0; 12];
        let rates = &mut year[..months];
        for (cell, field_cell) in (0_u32..).zip(&working.cells) {
            if !seas::under_sea(field_cell) || field_cell.liquid_entry != Some(self.entry) {
                continue;
            }
            let point = grid.point_of(coordinates.to_model(pass.grid().centre(cell)));
            let parent = parent_index(cell);
            let anomaly_k = &working.climate[slot(parent)].month_anomaly_k;
            let surface_k = annual_surface_k(working, cell);
            let month_terms = terms.get_or_insert_with(parent, surface_k, || {
                super::evaporation::month_terms(
                    &self.model,
                    &self.moisture,
                    self.freezing_k,
                    &self.spans_s,
                    anomaly_k,
                    months,
                    surface_k,
                )
            });
            let open_months = super::evaporation::open_months(
                month_terms,
                &self.shares,
                &self.energy_w_m2,
                point,
            );
            super::evaporation::year_rates_kg_m2_s(&open_months[..months], latent, rates);
            let open = 1.0 - field_cell.ice_fraction;
            for rate in rates.iter_mut() {
                *rate *= open;
            }
            each(cell, rates);
        }
    }
}

/// Asserts R09.T16.a's sharing of the water cycle's sea rates over `working` on `pass`'s world,
/// whose sea evaporates, from the climate records `working` holds, and prints what it found under
/// `what`. `working` may read those records as a one-month year (its `months` 1), so that
/// [`month_terms`](super::evaporation::month_terms)' one-month branch meets the oracle's:
///
/// - **To the bit:** the water cycle's loop (`Evaporation::each_open_cell`, its memo a fresh one)
///   visits exactly the cells the oracle ([`Evaporation::rates`]) opens, in cell-index order, each
///   month's rate the oracle's.
/// - **Shared:** its memo's hits are the oracle's count
///   ([`expected_hits`](super::evaporation::expected_hits)) over the cells visited at their
///   surface temperatures, `T_sl` − Γ (`z_w` − ℓ).
/// - **A floor:** that is at least three hits for each climate cell whose four field cells stand
///   at one water surface, and such climate cells are half of them at least (on the small Earth
///   after its climate step 957 of 1,536, with 3,150 hits over 4,362 sea cells, as built), so that
///   a loss of the sharing, or of the sea's one surface, shows.
///
/// The memo's hits.
///
/// # Panics
///
/// If `working` holds no records of the model's own, its sea evaporates nothing, or an assertion
/// above fails.
#[cfg(test)]
pub(crate) fn assert_the_sea_rates_share_their_month_terms(
    pass: &Pass<'_>,
    working: &Working,
    what: &str,
) -> u32 {
    use hyperion_testkit::float::bits;
    let model = &working
        .climate_records
        .as_ref()
        .expect("the climate step keeps its records")
        .model;
    let own = model
        .water
        .as_ref()
        .expect("the energy-balance model's own");
    let sea = Evaporation::new(
        pass,
        working,
        &EnergyBalance::for_inputs(pass.inputs()),
        own,
    )
    .expect("the sea evaporates");
    let mut terms = ClimateCellTerms::default();
    let mut oracle = vec![0.0; usize::from(working.months)];
    let mut visits = Vec::new();
    sea.each_open_cell_sharing(
        pass,
        working,
        &model.grid,
        model.coordinates,
        &mut terms,
        |cell, rates| {
            assert!(
                sea.rates(
                    pass,
                    working,
                    &model.grid,
                    model.coordinates,
                    cell,
                    &mut oracle
                ),
                "{what}, cell {cell}: the oracle's sea cell too"
            );
            assert_eq!(
                rates.len(),
                oracle.len(),
                "{what}, cell {cell}: one rate a month"
            );
            for (m, (&shared, &alone)) in rates.iter().zip(&oracle).enumerate() {
                assert_eq!(
                    bits(shared),
                    bits(alone),
                    "{what}, cell {cell}, month {m}: the sea's rate against the oracle's"
                );
            }
            let state = &working.cells[slot(cell)];
            let climate = &working.climate[slot(parent_index(cell))];
            visits.push((
                cell,
                climate.sea_level_temperature.value()
                    - working.lapse_rate_k_per_m
                        * (state.water_surface.value() - working.sea_level.value()),
            ));
        },
    );
    let mut visited = visits.iter().map(|&(cell, _)| cell).peekable();
    for cell in 0..pass.grid().len() {
        let open = sea.rates(
            pass,
            working,
            &model.grid,
            model.coordinates,
            cell,
            &mut oracle,
        );
        assert_eq!(
            open,
            visited.next_if_eq(&cell).is_some(),
            "{what}, cell {cell}: visited exactly where the oracle opens it"
        );
    }
    assert!(
        visited.next().is_none(),
        "{what}: every visit is a cell, in order"
    );
    let (expected, whole) = super::evaporation::expected_hits(&visits);
    let climate_cells = u32::try_from(working.climate.len()).expect("the climate cells fit a u32");
    let hits = terms.hits();
    eprintln!(
        "{what}: the sea's rates at {} cells to the bit; the month terms shared {hits} times \
         ({expected} expected), {whole} of {climate_cells} climate cells at one water surface",
        visits.len()
    );
    assert_eq!(hits, expected, "{what}: the water cycle's memo's hits");
    assert!(
        hits >= 3 * whole && 2 * whole >= climate_cells,
        "{what}: {hits} hits, {whole} of {climate_cells} climate cells at one water surface"
    );
    hits
}

/// The precipitation heuristic's water cycle on `grid` in `coordinates` over the model's own
/// records `own`, a circulation that turns as `turning` says, the orographic model at
/// `parameters` (the [module documentation](self)).
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
    parameters: &Parameters,
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
    let annual_flux =
        partition::edge_fluxes(grid, model.transport_kg_m2_s, &annual.energy_j_per_kg);
    let annual_efe_sine =
        partition::energy_flux_equator(grid, &annual_flux, &annual.energy_j_per_kg);

    // 2. The evaporation.
    let sea_level = working.sea_level.value();
    let (cell_band, band_area) = cell_bands(pass, coordinates);
    let mut band_evaporation = vec![vec![0.0; LATITUDE_BANDS]; months];
    let mut liquid_evaporation = vec![0.0; months];
    if let Some(evaporation) = Evaporation::new(pass, working, &model, own) {
        let omegas = field.solid_angles_sr();
        evaporation.each_open_cell(pass, working, grid, coordinates, |n, rates| {
            let omega = omegas[slot(n)];
            for (m, &e) in rates.iter().enumerate() {
                band_evaporation[m][cell_band[slot(n)]] += omega * e;
                liquid_evaporation[m] += omega * e;
            }
        });
    }

    // 3–7, month by month.
    let sphere: f64 = field.solid_angles_sr().iter().sum();
    let radius_m = inputs.radius().value();
    let gravity = inputs.gravity().value();
    let depth_pa = LOWER_BRANCH_DEPTH_SHARE * inputs.surface_pressure().value();
    let rotation_s = inputs.spin().rotation_period.value();
    let pole = coordinates.to_body([0.0, 0.0, 1.0]);
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
    // Each band's sea, and the land's solid angle, summed in cell-index order.
    let (mut sea_area, mut land_area) = (vec![0.0; LATITUDE_BANDS], 0.0);
    for ((&c, &band), &omega) in cover.iter().zip(&cell_band).zip(field.solid_angles_sr()) {
        match c {
            Cover::Sea => sea_area[band] += omega,
            Cover::Land => land_area += omega,
        }
    }
    // Each field cell's local frame, its local east about the model's pole (none at the pole)
    // and its zonal stencil in the model's coordinates, the same every month.
    let frames: Vec<([f64; 3], [f64; 3])> = field
        .centres()
        .iter()
        .map(|&up| wind::east_north(up))
        .collect();
    let easts: Vec<Option<[f64; 3]>> = field
        .centres()
        .iter()
        .map(|&up| wind::model_east(pole, up))
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
    let mut convergence = vec![0.0; rates.len()];
    let mut ground = vec![GroundState::Thawed; rates.len()];
    // Each month's headings, field-cell winds and record sums, allocated once and refilled.
    let mut headings: Vec<wind::Heading> = Vec::with_capacity(working.climate.len());
    let mut cell_winds: Vec<CellWind> = Vec::with_capacity(frames.len());
    let mut sums: Vec<(f64, f64)> = Vec::with_capacity(working.climate.len());
    for (m, budget) in budgets.into_iter().enumerate() {
        let band_e = band_means(&band_evaporation[m], &band_area);
        let sea = SeaBands::of(
            &band_evaporation[m],
            &band_area,
            &sea_area,
            &budget.evaporation_less_precipitation_kg_m2_s,
        );

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
            let velocity_m_s = wind::towards_pole(pole, up).map_or([0.0; 3], |north| {
                let across = wind::cross(north, up);
                [
                    v * north[0] + u * across[0],
                    v * north[1] + u * across[1],
                    v * north[2] + u * across[2],
                ]
            });
            let (sample, code) = wind::recorded(velocity_m_s, up);
            state.month_wind[m] = sample;
            codes.push(code);
        }

        // 5. The sea's rate, the land's source and ground, and the winds the orography reads: each
        // field cell's wind is its climate cell's record read at its own centre
        // (`wind::cell_wind`, with the cell's frame and the record's heading computed once),
        // taken along the model's bands (`wind::along_bands`).
        headings.clear();
        headings.extend(
            working
                .climate
                .iter()
                .map(|state| wind::Heading::of(&state.month_wind[m])),
        );
        cell_winds.clear();
        cell_winds.extend(
            (0_u32..)
                .zip(&frames)
                .zip(&easts)
                .map(|((n, &frame), &east)| {
                    wind::along_bands(
                        wind::cell_wind_in(&headings[slot(parent_index(n))], frame),
                        east,
                    )
                }),
        );
        let band_winds = BandWinds::of(&cell_winds, &easts);
        let still = cell_winds.iter().all(|&w| w == CellWind::Calm);
        let mut frozen_area = 0.0;
        for (n, (((((rate, source), state), stencil), &c), &omega)) in (0_u32..).zip(
            sea_rates
                .iter_mut()
                .zip(convergence.iter_mut())
                .zip(ground.iter_mut())
                .zip(&stencils)
                .zip(&cover)
                .zip(field.solid_angles_sr()),
        ) {
            match c {
                Cover::Sea => {
                    *rate = stencil.clamped(&sea.rates_kg_m2_s);
                    *source = 0.0;
                    *state = GroundState::Thawed;
                }
                Cover::Land => {
                    *rate = 0.0;
                    *source = stencil.clamped(&sea.convergence_kg_m2_s);
                    *state = ground_of(working, n, m, model.freezing);
                    if *state == GroundState::Frozen {
                        frozen_area += omega;
                    }
                }
            }
        }
        let evaporated = liquid_evaporation[m];
        let mut orographic = None;
        let mut recycled_shares: Option<Vec<f64>> = None;
        rates.fill(0.0);
        if evaporated > 0.0 && land {
            let mesh = mesh.get_or_insert_with(|| Mesh::of_graph(field, inputs.radius()));
            let solved = lfpm::solve(
                mesh,
                &Month {
                    heights_m: &heights_m,
                    cover: &cover,
                    ground: &ground,
                    winds: &cell_winds,
                    sea_rates_kg_m2_s: &sea_rates,
                    convergence_kg_m2_s: &convergence,
                },
                parameters,
            );
            rates.copy_from_slice(&solved.precipitation_kg_m2_s);
            recycled_shares = Some(solved.recycled_shares);
            orographic = Some(solved.diagnostics);
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
        sums.clear();
        sums.resize(working.climate.len(), (0.0, 0.0));
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
            sea,
            band_evaporation_kg_m2_s: band_e,
            scale,
            liquid_evaporation_kg_m2_s: evaporated / sphere,
            recycled_kg_m2_s: recycled / sphere,
            precipitation_kg_m2_s: fallen / sphere,
            frozen_land_share: if land_area > 0.0 {
                frozen_area / land_area
            } else {
                0.0
            },
            band_winds,
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
