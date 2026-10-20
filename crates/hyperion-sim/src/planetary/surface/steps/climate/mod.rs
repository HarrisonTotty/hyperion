//! Step 4, the climate (R09.T13, Design note 8).
//!
//! The model the climate regime names: the seasonal moist energy-balance model, in tidally locked
//! coordinates for a locked world, radiative equilibrium with thermal inertia, or the isothermal
//! surface; normalised to plan 14's mean and signed contrasts, with ice and liquids placed by
//! substance to plan 14's areas, and the labelled precipitation heuristic with each month's wind.
//!
//! # The pipeline
//!
//! 1. The land–sea state, from the working field: a field cell is under liquid where the relief
//!    step flooded it, its water surface strictly above its ground (`seas::under_sea`: R09.T12.e's
//!    ocean, decision-r09-t12-coarse R8, and later R09.T14's lakes), and each model grid point's
//!    sea share is the solid angle of its flooded cells over all of its cells', each cell binned
//!    by its centre's latitude and longitude in the model's coordinates; a point that holds no
//!    cell's centre takes the state of the cell that holds its own. Until R09.T12.e floods the
//!    ocean every cell's water surface is its ground, so every world is all land.
//! 2. The model that [`ClimateRegime::model`](crate::planetary::surface::ClimateRegime) names, on
//!    its own grid: the annual mean and each month's mean at every grid point. The energy-balance
//!    models read the relief (`hypsometry`, R09.T13.g): each grid point's eight equal-area
//!    quantiles of its cells' heights above the sea level, the water's surface over water, binned
//!    as the sea shares are, with its flooded cells' mean water-surface height; and on the zonal
//!    grid in the model's coordinates each band's quantiles over all of its cells, for τ's solve.
//!    Their albedo, outgoing radiation and sea-ice heat capacity read the ground's temperature,
//!    the sea-level temperature less Γ z at each quantile, and τ is solved for the surface mean
//!    (decision-r09-t13a-relief §1, decision-r09-t13ge §1).
//! 3. The normalisation on the model grid (`normalise`; decision-r09-t13d-normalisation §2):
//!    plan 14's contrasts as corrections a P₂(sin φ) + b P₁(cos γ) on the annual sea-level field,
//!    by an exact linear solve on the energy-balance and isothermal models; on the radiative model
//!    c, a and b together on its moments, in the brightness of its emitted flux. Every grid
//!    point's records then lie at or above the model's lower bound (the guard).
//! 4. The lapse rate (`lapse`), plan 14's ground gradient α g ÷ `c_p` on the energy-balance and
//!    isothermal models and none on the radiative one. The water surfaces are the relief step's
//!    (R8): this step reads them and writes none.
//! 5. The interpolation from the model grid to each climate cell, as sea-level temperatures,
//!    bicubic in the model's coordinates (Catmull and Rom's cubic in both directions, periodic in
//!    longitude and continued over the poles), every value held to the range of the values its
//!    stencil reads (the guard).
//! 6. On the energy-balance and isothermal models, the mean: the constant c that makes the field
//!    cells' area-weighted surface temperature, each its climate cell's sea-level temperature
//!    less Γ times its surface's height above the sea level, the input's, added last to every
//!    sea-level temperature (Design note 8). Every record then lies at or above the lower bound.
//! 7. The ices (`ice`): each placed per substance, most volatile first, its zonal part on the field
//!    cells coldest against its own frost point by their annual radiative mean surface
//!    temperature, read by the cells' own interpolant at their centres
//!    (decision-r09-t13d-normalisation §1), then its cold-trapped part
//!    ([`CoarseInputs::cold_traps`](crate::planetary::surface::CoarseInputs::cold_traps)) in
//!    proportion to each cell's cold-trap fraction at its centre's latitude, plan 14's closed form
//!    through its `ColdTrapGround`, scaled to plan 14's area and capped at the fraction, a cell
//!    naming one ice (decision-r09-t13f-cold-traps §5).
//! 8. The liquids (`seas`): the flooded cells shared among them, each to its share of the ocean
//!    fraction, basin by basin.
//! 9. The precipitation heuristic and the winds (`hydrology`, R09.T13.e; decision-r09-t13ge §2),
//!    on the energy-balance models from their own records before step 3's correction: each
//!    month's zonal moisture budget, Siler et al.'s Hadley-cell partition of the model's own flux
//!    centred on the month's energy-flux equator, held at Guendelman and Kaspi's ascending-branch
//!    cap on a rotating world (`partition`); Priestley and Taylor's evaporation over the climate
//!    condensable's own open liquid (`evaporation`); the sea's rate, each band's sea evaporation
//!    and its convergence, and the land's source, that convergence (R09.T13.j,
//!    decision-r09-t13e-revision §4.1–§4.2); each climate cell's 10 m wind from the Hadley cells'
//!    mass flux and a Ferrel and polar template under a surface drag, quantised and decoded into
//!    its record (`wind`); the orographic precipitation over land along the model's bands, each
//!    field cell's wind taken east, west or calm by its component along them (`lfpm`; §4.3); and
//!    one factor a month that balances the global water. The radiative and isothermal models write
//!    no precipitation and calm winds.
//!
//! A cell's surface temperature is its climate cell's sea-level temperature less
//! Γ (z − `z_sea`) at its surface height z, the water's surface over water: what R10's classifier
//! computes from the header's lapse rate and sea level, so that reduction and re-expansion share Γ
//! and `z_sea` and nothing else and are exact inverses (decision-r09-t13a-relief §2.1).
//!
//! # What is built
//!
//! R09.T13.a built the zonal moist energy-balance model (`ebm`) on 72 bands of 2.5° (`grid`), its
//! seasonal sun (`sun`) and its transport law (`transport`), private to this step. R09.T13.b added
//! the longitude: the geographic model on 72 × 72 points of 2.5° × 5°, each step's latitude solve
//! followed by a periodic solve in longitude, and the locked model on 72 × 144 points of 2.5° ×
//! 2.5° in Koll and Abbot's tidally locked coordinates about the substellar axis (`coordinates`),
//! lit by the host's beam rather than its diurnal mean, with its seasons wherever the orbit is
//! eccentric or the world tilted. [`run`] dispatches [`ClimateModelKind::EnergyBalance`] to the
//! geographic model and [`ClimateModelKind::LockedEnergyBalance`] to the locked one.
//! R09.T13.c added the other regimes, pointwise: [`ClimateModelKind::RadiativeEquilibrium`]'s
//! regolith columns with thermal inertia (`airless`), the day resolved and averaged over local time,
//! under a thin air's grey slab and its bulk gas's frost point, on the zonal grid or, on a world
//! locked 1:1 to its host, the tidally locked one; and [`ClimateModelKind::Isothermal`]'s uniform
//! surface (`isothermal`). On a locked lava world the field's melt is no record of its own: R10's
//! classifier reads it per cell from these temperatures against the cell's crust entry's
//! transition. R09.T13.d added the normalisation, the guard, the lapse rate, the mean, the ices and
//! the liquids (the pipeline's steps 3–8), and the energy-balance albedo's shift to the climate
//! condensable's freezing point by `SubstanceId` (`frost`). R09.T13.g gave the energy-balance
//! models the relief (`hypsometry`, and `ebm`'s relief terms, its surface-mean τ and the fold
//! guard): a world whose every height above the sea level is zero is T13.a's to the bit.
//! R09.T13.e built the precipitation heuristic and the winds (the pipeline's step 9): the
//! orographic precipitation (`lfpm`), Hergarten and Robl 2022's linear feedback model on the
//! field's cells, carried along a month's wind from the sea's rate, with the land's
//! runoff share, solved in the order of the month's upwind graph's strongly connected components,
//! the small ones exactly and the members of each larger one in its flow order (Eades, Lin and
//! Smyth's greedy order for the feedback arc set, the ruling's E-L8), by sweeps accelerated by
//! restarted GMRES until one further sweep moves no land cell's rate by more than 10⁻⁴ of it
//! (decision-r09-t13e-lfpm-convergence); the zonal
//! budget (`partition`), the evaporation (`evaporation`) and the winds
//! (`wind`); and their pipeline (`hydrology`), which writes each climate cell's monthly
//! precipitation and 10 m wind, and `annual_runoff_kg_m2_s`, the runoff R09.T14 reads. Each
//! month's record holds the wind whose component along the model's bands its orographic step
//! carried vapour along, as the record decodes it. R09.T13.j revised it for the land's water
//! (decision-r09-t13e-revision): the sea rains its own evaporation plus its band's convergence,
//! each land cell takes that convergence as a vapour source, the orographic model carries the
//! sea's air along the bands, and frozen ground recycles nothing. R09.T13.h revised the radiative
//! model's thin air (decision-r09-t13-thin-air §1.9): its slab is in balance with its column's
//! mean emission over each month of the orbit before, bounded at the bulk gas's frost point one
//! scale height up, its deficit falling as snow on every column, and a frost that runs out within
//! a step pays its latent heat. R09.T13.f added the cold traps
//! (`ice`, the pipeline's step 7): each ice's cold-trapped part,
//! the inputs' `cold_traps`, placed after the zonal parts in one function in each field cell's
//! cold-trap fraction, from one call of plan 14's `ColdTrapGround::fractions_at` a cell for every
//! trapped substance, scaled over every cell to plan 14's area and held to the fraction, the
//! trapped ice of largest share drawn where several lie and the rest counted as undrawn, its
//! shares written in whole codes coldest first; and the airless frost points the ices are ranked
//! by read plan 14's cold-trap threshold (`frost`). A world places liquids only on the cells the
//! relief step floods (R09.T12.e's ocean, decision-r09-t12-coarse R8).

mod airless;
mod coordinates;
mod ebm;
mod evaporation;
mod frost;
mod grid;
mod hydrology;
mod hypsometry;
pub(crate) mod ice;
mod isothermal;
mod lapse;
mod lfpm;
mod normalise;
mod partition;
pub(crate) mod seas;
mod sun;
#[cfg(test)]
mod tests;
mod transport;
mod wind;

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{ClimateModelKind, cell_index, month_edges};

use super::{Pass, Working};
use crate::planetary::hooks::surface::night_floor;
use crate::planetary::surface::grid::parent_index;
use crate::units::Kelvin;
use coordinates::Coordinates;
use ebm::{Ground, Relief};
#[cfg(test)]
use grid::LATITUDE_BANDS;
use grid::{GridClimate, ModelGrid};
use hypsometry::Hypsometry;
use ice::{IceKey, PlacedIce, PlacedTrap};
use normalise::Correction;
use seas::PlacedLiquid;
use sun::NOMINAL_STEP;

/// Runs the step on `working`: the model the inputs' climate regime names, normalised to plan
/// 14's figures and written to the climate cells, with the lapse rate, the ices and the liquids
/// (the [module documentation](self)).
///
/// # Panics
///
/// If the model leaves a value that is not finite: a fault of the model.
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    // The diagnostics are no output: the tests read them.
    let _diagnostics = run_diagnosed(pass, working);
}

/// What a run did that is no output, for its tests: the guard's counts, the corrections and the
/// ices, their cold-trapped parts and the liquids placed (decision-r09-t13d-normalisation §1.2,
/// §2.4; decision-r09-t13f-cold-traps §5.3).
#[derive(Debug, Clone, PartialEq, Default)]
struct Diagnostics {
    /// The correction the model's field took, K: c, a and b.
    correction: Correction,
    /// The radiative solve's residuals, K (its mean's, its ends', its day–night's); zero on the
    /// arithmetic models, whose solve is exact.
    residuals_k: [f64; 3],
    /// The grid's records the guard raised to the model's lower bound.
    floored_points: u32,
    /// The interpolated values the guard held to their stencil's range, the climate cells' and
    /// the field cells' alike.
    clamped_values: u64,
    /// The climate cells' records the guard raised to the lower bound after the mean's constant.
    floored_records: u32,
    /// The model's lower bound, K.
    lower_bound_k: f64,
    /// The ices' zonal parts placed, in order.
    ices: Vec<PlacedIce>,
    /// The ices' cold-trapped parts placed, in order (R09.T13.f).
    cold_traps: Vec<PlacedTrap>,
    /// Each field cell's ice key, in cell-index order, which a second placement reads
    /// (R09.T14.d's re-application).
    keys: Vec<IceKey>,
    /// The liquids placed, in order.
    liquids: Vec<PlacedLiquid>,
    /// The energy-balance models' surface means ([`SurfaceMeans`]); none on the other models.
    surface_means: Option<SurfaceMeans>,
    /// The energy-balance models' water cycle ([`hydrology`]); none on the other models, whose
    /// records are zero and calm.
    water: Option<hydrology::Diagnostics>,
}

/// An energy-balance model's surface means over its relief, K, which R09.T13.d's constant c
/// reconciles (decision-r09-t13ge §3, G3): its τ solve's, of the zonal grid's steady state over
/// each band's pooled quantiles, and its seasonal run's, of the annual field on its own grid over
/// each point's.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SurfaceMeans {
    /// The τ solve's steady state's, K.
    steady_k: f64,
    /// The seasonal run's annual field's, K.
    seasonal_k: f64,
}

/// The kind of mean a model's records are normalised in (decision-r09-t13d-normalisation §2.2),
/// and what it leaves for the steps after the guard.
#[derive(Debug, Clone, PartialEq)]
enum MeanKind {
    /// The arithmetic models' (the energy-balance and isothermal ones): the lapse term applies,
    /// the mean's constant is still to come after it, and the ices' keys come from the months.
    Arithmetic,
    /// The radiative model's, the brightness of its emitted flux: no lapse term and no later
    /// constant, and each grid point's ice key, its ⟨(T + s)⁴⟩^¼, K.
    Radiative {
        /// Each grid point's ⟨(T + s)⁴⟩^¼ over the converged orbit, K.
        keys_k: Vec<f64>,
    },
}

/// A model's normalised answer on its grid: its records, its coordinates, its lower bound, the
/// correction it took and the kind of its mean, and an energy-balance model's own records for the
/// water cycle.
#[derive(Debug, Clone, PartialEq)]
struct Normalised {
    grid: ModelGrid,
    coordinates: Coordinates,
    climate: GridClimate,
    lower_bound_k: f64,
    correction: Correction,
    residuals_k: [f64; 3],
    kind: MeanKind,
    surface_means: Option<SurfaceMeans>,
    /// The energy-balance models' own records before the correction, which the precipitation
    /// heuristic reads ([`hydrology`]); none on the radiative and isothermal models.
    water: Option<hydrology::OwnRecords>,
}

/// `t_k`, a climate temperature in kelvin, asserted finite: one that is not is a fault of the
/// model, and its bits (a NaN's payload) could differ between machines.
///
/// # Panics
///
/// If `t_k` is a NaN or infinite.
#[must_use]
#[track_caller]
fn finite_k(t_k: f64) -> f64 {
    assert!(t_k.is_finite(), "a temperature must be finite, got {t_k} K");
    t_k
}

/// The model `inputs`' regime names, run on `working`'s land and sea and normalised on its grid
/// (the pipeline's steps 2–3).
#[must_use]
fn normalised(pass: &Pass<'_>, working: &Working) -> Normalised {
    let inputs = pass.inputs();
    match inputs.climate().model {
        ClimateModelKind::EnergyBalance => energy_balance(
            pass,
            working,
            ModelGrid::geographic(),
            Coordinates::Geographic,
        ),
        ClimateModelKind::LockedEnergyBalance => energy_balance(
            pass,
            working,
            ModelGrid::locked(),
            Coordinates::TidallyLocked,
        ),
        ClimateModelKind::Isothermal => {
            let (grid, coordinates) = airless::frame(inputs);
            let climate = isothermal::solve(inputs, &grid);
            arithmetic(pass, grid, coordinates, climate)
        }
        ClimateModelKind::RadiativeEquilibrium => {
            let solution = airless::solve(inputs);
            let (correction, residuals_k) = normalise::radiative(inputs, &solution);
            let keys_k = normalise::radiative_keys(&solution, &correction);
            let lower_bound_k = solution.lower_bound_k;
            let (grid, coordinates) = (solution.grid, solution.coordinates);
            let mut climate = solution.climate;
            normalise::apply(&mut climate, &grid, coordinates, &correction);
            Normalised {
                grid,
                coordinates,
                climate,
                lower_bound_k,
                correction,
                residuals_k,
                kind: MeanKind::Radiative { keys_k },
                surface_means: None,
                water: None,
            }
        }
    }
}

/// An arithmetic model's records `climate` on `grid` in `coordinates`, normalised for the body of
/// `pass`'s inputs by the exact linear solve (decision-r09-t13d-normalisation §2), its lower bound
/// the night floor; no surface means or own records yet.
#[must_use]
fn arithmetic(
    pass: &Pass<'_>,
    grid: ModelGrid,
    coordinates: Coordinates,
    climate: GridClimate,
) -> Normalised {
    let inputs = pass.inputs();
    let correction = normalise::arithmetic(inputs, &grid, coordinates, &climate.annual_k);
    let mut climate = climate;
    normalise::apply(&mut climate, &grid, coordinates, &correction);
    Normalised {
        grid,
        coordinates,
        climate,
        lower_bound_k: night_floor(inputs.heat_flow()).value(),
        correction,
        residuals_k: [0.0; 3],
        kind: MeanKind::Arithmetic,
        surface_means: None,
        water: None,
    }
}

/// The energy-balance model on `grid` in `coordinates` (the geographic model in geographic
/// coordinates, the locked one in tidally locked ones, or in the tests the zonal grid) over
/// `working`'s land, sea and relief, normalised, with its surface means and its own records for
/// the water cycle: its monthly records before the correction, each month's insolation and each
/// point's water-surface height (R09.T13.e).
#[must_use]
fn energy_balance(
    pass: &Pass<'_>,
    working: &Working,
    grid: ModelGrid,
    coordinates: Coordinates,
) -> Normalised {
    let inputs = pass.inputs();
    let sea_share = point_sea_shares(pass, working, &grid, coordinates);
    let (on_grid, zonal) = reliefs(pass, working, &grid, coordinates);
    let ground = Ground::Relief {
        grid: &on_grid,
        zonal: &zonal,
    };
    let solution = match coordinates {
        Coordinates::Geographic => ebm::solve(inputs, &grid, &sea_share, NOMINAL_STEP, ground),
        Coordinates::TidallyLocked => {
            ebm::solve_locked(inputs, &grid, &sea_share, NOMINAL_STEP, ground)
        }
    };
    let surface_means = SurfaceMeans {
        steady_k: solution.steady_surface_mean_k,
        seasonal_k: on_grid.surface_mean_k(&grid, &solution.climate.annual_k),
    };
    let own = hydrology::OwnRecords {
        months_k: solution.climate.months_k.clone(),
        insolation_w_m2: solution.month_insolation_w_m2,
        water_m: on_grid
            .hypsometry()
            .all()
            .iter()
            .map(|point| point.water_m)
            .collect(),
    };
    let mut model = arithmetic(pass, grid, coordinates, solution.climate);
    model.surface_means = Some(surface_means);
    model.water = Some(own);
    model
}

/// The energy-balance model's relief from `working`'s field (R09.T13.g;
/// [`hypsometry`](self::hypsometry)), on `grid` in `coordinates` and on the zonal grid in the same
/// coordinates, whose bands pool all of their cells for τ's solve (decision-r09-t13ge §1.4, g5).
#[must_use]
fn reliefs(
    pass: &Pass<'_>,
    working: &Working,
    grid: &ModelGrid,
    coordinates: Coordinates,
) -> (Relief, Relief) {
    let inputs = pass.inputs();
    let of = |grid: &ModelGrid| {
        Relief::of(
            inputs,
            Hypsometry::of_field(pass, working, grid, coordinates),
        )
    };
    (of(grid), of(&ModelGrid::zonal()))
}

/// [`run`], with what it did that is no output.
fn run_diagnosed(pass: &Pass<'_>, working: &mut Working) -> Diagnostics {
    let model = normalised(pass, working);
    finish(pass, working, model)
}

/// The pipeline's steps from the guard on: `model`, the normalised answer on its grid, held at
/// its lower bound, written to `working` with the lapse rate and the mean, and the ices and the
/// liquids placed; what it did that is no output. The water surfaces are the relief step's.
fn finish(pass: &Pass<'_>, working: &mut Working, mut model: Normalised) -> Diagnostics {
    let inputs = pass.inputs();
    let floored_points = normalise::floor_records(&mut model.climate, model.lower_bound_k);
    working.lapse_rate_k_per_m = match model.kind {
        MeanKind::Arithmetic => lapse::lapse_rate_k_per_m(inputs),
        MeanKind::Radiative { .. } => 0.0,
    };
    let mut clamped_values = write_climate(
        pass,
        working,
        &model.grid,
        model.coordinates,
        &model.climate,
    );
    match model.kind {
        MeanKind::Arithmetic => {
            let mean = inputs.mean_surface_temperature().value() - surface_mean_k(pass, working);
            for state in &mut working.climate {
                state.sea_level_temperature =
                    Kelvin::new(state.sea_level_temperature.value() + mean);
            }
            model.correction.mean_k = mean;
        }
        MeanKind::Radiative { .. } => {}
    }
    let floored_records = floor_climate(working, model.lower_bound_k);
    let (annual_k, keys) = cell_temperatures(pass, working, &model, &mut clamped_values);
    let ices = ice::place_ices(
        inputs,
        pass.grid().solid_angles_sr(),
        pass.grid().centres(),
        &keys,
        &mut working.cells,
    );
    let liquids = seas::place_liquids(inputs, pass.grid(), &annual_k, &mut working.cells);
    let water = hydrology::run(pass, working, &model);
    Diagnostics {
        correction: model.correction,
        residuals_k: model.residuals_k,
        floored_points,
        clamped_values,
        floored_records,
        lower_bound_k: model.lower_bound_k,
        ices: ices.zonal,
        cold_traps: ices.traps,
        keys,
        liquids,
        surface_means: model.surface_means,
        water,
    }
}

/// The annual runoff-generating rate of field cell `cell` of `working`, kg m⁻² s⁻¹, for R09.T14's
/// erosion (decision-r09-t13ge §2.8, e8; Hergarten and Robl 2022, eqs. 44, 49): Σₘ sₘ (1 − εₘ) Pₘ
/// over the months, sₘ each month's share of the year, Pₘ its climate cell's month rate and εₘ the
/// orographic model's recycled share at its surface's height z above the sea level, ε₀ e^(−z/H₀)
/// where its month temperature at its surface lies at or above the climate condensable's freezing
/// point and none below it, where all that falls runs off ([`lfpm::runoff_kg_m2_s`],
/// [`hydrology::ground_of`]; decision-r09-t13e-revision §4.5); zero over liquid, and on a world
/// where nothing rains. T13.e writes no `runoff_m_per_year`: that is what the erosion took.
///
/// # Panics
///
/// If `cell` is not one of the field's cells, or the climate cell's rates are negative or not
/// finite (a fault of the step).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14's erosion reads the runoff the climate gives"
    )
)]
#[must_use]
pub(crate) fn annual_runoff_kg_m2_s(pass: &Pass<'_>, working: &Working, cell: u32) -> f64 {
    let state = &working.climate[usize::try_from(parent_index(cell)).expect("a cell fits a usize")];
    let months = usize::from(working.months);
    let field_cell = &working.cells[usize::try_from(cell).expect("a cell fits a usize")];
    let cover = if seas::under_sea(field_cell) {
        lfpm::Cover::Sea
    } else {
        lfpm::Cover::Land
    };
    let height_m = field_cell.water_surface.value() - working.sea_level.value();
    let freezing = ebm::Freezing::of(pass.inputs());
    month_shares(pass, months)
        .iter()
        .zip(&state.month_precipitation_kg_m2_s)
        .enumerate()
        .fold(0.0, |sum, (m, (&share, &rate))| {
            let ground = hydrology::ground_of(working, cell, m, freezing);
            sum + share * lfpm::runoff_kg_m2_s(rate, height_m, cover, ground)
        })
}

/// The field cells' area-weighted mean surface temperature of `working`, K: each cell's climate
/// cell's sea-level temperature less Γ times its water surface's height above the sea level, the
/// client's view of it (R10's classifier), summed in cell-index order.
#[must_use]
fn surface_mean_k(pass: &Pass<'_>, working: &Working) -> f64 {
    let (lapse, sea) = (working.lapse_rate_k_per_m, working.sea_level.value());
    let (sum, total) = (0_u32..)
        .zip(&working.cells)
        .zip(pass.grid().solid_angles_sr())
        .fold((0.0, 0.0), |(sum, total), ((n, cell), &omega)| {
            let parent = usize::try_from(parent_index(n)).expect("a cell index fits a usize");
            let t = working.climate[parent].sea_level_temperature.value()
                - lapse * (cell.water_surface.value() - sea);
            (sum + omega * t, total + omega)
        });
    sum / total
}

/// Holds every climate cell's records of `working` at least `lower_bound_k` (the guard): its
/// sea-level temperature, and in a twelve-month year each month's, its anomalies then taken from
/// the annual mean as held; a cell whose records all lie at or above it is left to the bit. The
/// number of records it raised, a diagnostic that is no output.
fn floor_climate(working: &mut Working, lower_bound_k: f64) -> u32 {
    let months = if working.months > 1 {
        usize::from(working.months)
    } else {
        0
    };
    let mut raised = 0_u32;
    for state in &mut working.climate {
        let annual = state.sea_level_temperature.value();
        let low = |t: f64| t < lower_bound_k;
        let lows = u32::from(low(annual))
            + state.month_anomaly_k[..months]
                .iter()
                .map(|a| u32::from(low(annual + a)))
                .sum::<u32>();
        if lows == 0 {
            continue;
        }
        raised = raised.saturating_add(lows);
        let held = if low(annual) { lower_bound_k } else { annual };
        for anomaly in &mut state.month_anomaly_k[..months] {
            let month = annual + *anomaly;
            *anomaly = if low(month) { lower_bound_k } else { month } - held;
        }
        state.sea_level_temperature = Kelvin::new(held);
    }
    raised
}

/// Each month's share of the year of a year of `months` months on the seasonal orbit of `pass`'s
/// inputs ([`month_edges`]): twelve shares of mean anomaly, or one.
#[must_use]
fn month_shares(pass: &Pass<'_>, months: usize) -> Vec<f64> {
    if months != 12 {
        return vec![1.0; months];
    }
    let e = pass
        .inputs()
        .seasonal_orbit()
        .map_or(0.0, |orbit| orbit.eccentricity);
    let edges = month_edges(e).expect("a seasonal orbit's eccentricity is in [0, 1)");
    edges
        .windows(2)
        .map(|pair| (pair[1].value() - pair[0].value()) / core::f64::consts::TAU)
        .collect()
}

/// Each field cell's annual mean surface temperature and ice key (the pipeline's step 7), from
/// `model`'s normalised records read at its centre by the cells' own interpolant, each value held
/// to its stencil's range (the guard, whose holds it counts into `clamped`), plus the mean's
/// constant, less the lapse term at its water surface's height above the sea level.
///
/// On the arithmetic models the key is its months' (Σₖ wₖ `T_ik`⁴)^¼ ([`IceKey::of_months`]); on the
/// radiative model, which has no lapse term, the interpolant of its points' ⟨(T + s)⁴⟩^¼, its
/// warmest month the tie-break.
#[must_use]
fn cell_temperatures(
    pass: &Pass<'_>,
    working: &Working,
    model: &Normalised,
    clamped: &mut u64,
) -> (Vec<f64>, Vec<IceKey>) {
    let shares = month_shares(pass, model.climate.months_k.len());
    let constant = match model.kind {
        MeanKind::Arithmetic => model.correction.mean_k,
        MeanKind::Radiative { .. } => 0.0,
    };
    let (lapse, sea) = (working.lapse_rate_k_per_m, working.sea_level.value());
    let mut months = vec![0.0; model.climate.months_k.len()];
    let mut annual = Vec::with_capacity(working.cells.len());
    let mut keys = Vec::with_capacity(working.cells.len());
    for (cell, centre) in working.cells.iter().zip(pass.grid().centres()) {
        let stencil = model.grid.stencil_at(model.coordinates.to_model(*centre));
        let mut at = |values: &[f64]| {
            let (value, held) = stencil.clamped(values);
            *clamped += u64::from(held);
            value
        };
        let offset = constant - lapse * (cell.water_surface.value() - sea);
        annual.push(at(&model.climate.annual_k) + offset);
        for (month, values) in months.iter_mut().zip(&model.climate.months_k) {
            *month = at(values) + offset;
        }
        keys.push(match &model.kind {
            MeanKind::Radiative { keys_k } => IceKey {
                radiative_mean_k: at(keys_k) + offset,
                warmest_k: months.iter().fold(f64::NEG_INFINITY, |w, &m| w.max(m)),
            },
            MeanKind::Arithmetic => IceKey::of_months(&months, &shares),
        });
    }
    (annual, keys)
}

/// Each point's sea share of the working field on `grid` in `coordinates`: the solid angle of its
/// cells under liquid ([`seas::under_sea`], the relief step's flood) over all of its cells', summed
/// in cell-index order, each cell in the point
/// of its centre's latitude and longitude in the model's frame ([`ModelGrid::point_of`]). A point
/// that holds no cell's centre, as points near the poles may and about a fifth of them do on the
/// shallowest level, 5, takes the state, 0 or 1, of the field cell that holds its own centre; a
/// field of no cells is all land.
///
/// # Panics
///
/// Never: the field's grid is at a coarse level, so a direction's cell is one of its cells.
#[must_use]
fn point_sea_shares(
    pass: &Pass<'_>,
    working: &Working,
    grid: &ModelGrid,
    coordinates: Coordinates,
) -> Vec<f64> {
    let points = grid.points();
    let (mut sea, mut all) = (vec![0.0; points], vec![0.0; points]);
    let field = pass.grid();
    for ((cell, centre), &solid_angle) in working
        .cells
        .iter()
        .zip(field.centres())
        .zip(field.solid_angles_sr())
    {
        let point = grid.point_of(coordinates.to_model(*centre));
        all[point] += solid_angle;
        if seas::under_sea(cell) {
            sea[point] += solid_angle;
        }
    }
    (0..points)
        .map(|point| {
            if all[point] > 0.0 {
                return sea[point] / all[point];
            }
            let direction = coordinates.to_body(grid.centre_direction(point));
            let key = PatchKey::containing(field.level(), direction)
                .expect("the field's grid is at a coarse level");
            let index = usize::try_from(cell_index(key)).expect("a cell index fits a usize");
            match working.cells.get(index) {
                Some(cell) if seas::under_sea(cell) => 1.0,
                Some(_) | None => 0.0,
            }
        })
        .collect()
}

/// Each latitude band's sea share of the working field, the zonal model's, which the step no
/// longer runs but its tests do: the solid angle of its cells under liquid
/// ([`seas::under_sea`]) over all of its cells', summed in cell-index order, each cell in the band
/// of its centre's latitude. A band that holds no cell's centre, as a polar band may on the
/// shallowest levels, takes the nearest band's that does, equatorward first; a field of no cells
/// is all land.
#[cfg(test)]
#[must_use]
fn sea_shares(pass: &Pass<'_>, working: &Working) -> Vec<f64> {
    let (mut sea, mut all) = (vec![0.0; LATITUDE_BANDS], vec![0.0; LATITUDE_BANDS]);
    let grid = pass.grid();
    for ((cell, centre), &solid_angle) in working
        .cells
        .iter()
        .zip(grid.centres())
        .zip(grid.solid_angles_sr())
    {
        let band = ModelGrid::band_of(centre[2]);
        all[band] += solid_angle;
        if seas::under_sea(cell) {
            sea[band] += solid_angle;
        }
    }
    let share = |band: usize| (all[band] > 0.0).then(|| sea[band] / all[band]);
    (0..LATITUDE_BANDS)
        .map(|band| {
            share(band)
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
                            .find_map(share)
                    })
                })
                .unwrap_or(0.0)
        })
        .collect()
}

/// Writes `climate`, a model's means on `grid` in `coordinates`, to every climate cell of
/// `working`: its sea-level temperature the annual mean at its centre, and in a twelve-month year
/// each month's mean there less it, each by the bicubic interpolation in the model's frame
/// ([`ModelGrid::stencil_at`]) held to the range of the values its stencil reads (the guard of
/// decision-r09-t13d-normalisation §2.4, [`Stencil::clamped`](grid::Stencil::clamped)); no anomaly
/// in a one-month year. The number of values the range held, a diagnostic that is no output.
fn write_climate(
    pass: &Pass<'_>,
    working: &mut Working,
    grid: &ModelGrid,
    coordinates: Coordinates,
    climate: &GridClimate,
) -> u64 {
    let twelve = working.months == 12 && climate.months_k.len() == 12;
    let mut clamped = 0_u64;
    for (state, centre) in working
        .climate
        .iter_mut()
        .zip(pass.climate_grid().centres())
    {
        let stencil = grid.stencil_at(coordinates.to_model(*centre));
        let mut at = |values: &[f64]| {
            let (value, held) = stencil.clamped(values);
            clamped += u64::from(held);
            value
        };
        let annual = at(&climate.annual_k);
        state.sea_level_temperature = Kelvin::new(annual);
        state.month_anomaly_k = [0.0; 12];
        if twelve {
            for (anomaly, month) in state.month_anomaly_k.iter_mut().zip(&climate.months_k) {
                *anomaly = at(month) - annual;
            }
        }
    }
    clamped
}
