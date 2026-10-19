//! Design note 8's seasonal moist energy-balance model (R09.T13.a–b): North and Coakley's
//! diffusive equation in Spiegel, Menou and Scharf's form, diffusing near-surface moist static
//! energy as Siler, Roe and Armour do, in latitude and longitude, stepped implicitly over the
//! seasonal orbit.
//!
//! # The equation
//!
//! At each grid point, of sea share s, at the surface temperature T (at sea level):
//!
//! C(T) ∂T/∂t = S (1 − α(T)) + `F_int` − σT⁴ ÷ (1 + ¾τ) + Λ ∇²h,
//!
//! with ∇²h = (1 ÷ cos φ) ∂/∂φ (cos φ ∂h/∂φ) + (1 ÷ cos² φ) ∂²h/∂λ², the sphere's isotropic
//! Laplacian per unit of R⁻², in the grid's own coordinates (geographic, or tidally locked about
//! the substellar axis: [`Coordinates`](super::coordinates::Coordinates)):
//!
//! - S the insolation ([`Sunlight`]): the diurnal mean of the point's band on a world that turns
//!   under its sun, and the beam itself on one locked 1:1 to it; `F_int` the inputs' heat flow;
//! - α the ice albedo 0.525 − 0.245 tanh((T − (`T_f` − 5 K)) ÷ 5 K) ([`ALBEDO_MEAN`]), `T_f`
//!   the freezing point: water's ([`WATER_FREEZING_POINT_K`]) shifted by the climate
//!   condensable's freezing point less water's, by `SubstanceId` (R09.T13.d,
//!   [`EnergyBalance::for_inputs`]), and flat at [`ICE_FREE_ALBEDO`] where no ice forms;
//! - C the point's heat capacity, the area-weighted mix of land's ([`LAND_HEAT_CAPACITY_J_PER_M2_K`])
//!   and the sea's, 40 times land's above `T_f`, 9.2 times from 10 K below it, and 2 times
//!   colder, an ice's (40 times at every temperature where no ice forms);
//! - σT⁴ ÷ (1 + ¾τ) the grey outgoing radiation in Spiegel et al.'s Eddington form, with one τ
//!   for the body, solved once so that the global mean of the annual-mean steady state is plan
//!   14's mean ([`CoarseInputs::mean_surface_temperature`]);
//! - h the near-surface moist static energy `c_p` T + L q, with q the climate condensable's
//!   specific humidity at 80% relative humidity ([`RELATIVE_HUMIDITY`]), from its saturation
//!   pressure in the substance registry at the surface pressure, L its latent heat of
//!   vaporisation per kilogram, and `c_p` the air's mixed heat capacity: on a dry world, one whose
//!   condensable is not among its surface liquids or that has none, h is `c_p` T alone;
//! - Λ the transport's coefficient ([`transport`](super::transport)), no-flux at the poles.
//!
//! # The scheme
//!
//! Each step is backward Euler, linearised about the step's start: the emission, h and the absorbed
//! sunlight by their slopes in T. The heat capacity is taken at the step's start, and of the
//! absorbed sunlight's slope at most [`IMPLICIT_ALBEDO_SHARE`] of the point's storage and emission
//! slopes, which keeps the system diagonally dominant; the rest of it is taken at the step's
//! start, as most energy-balance models take the albedo. The change is in flux form, so the
//! scheme conserves energy.
//!
//! The step's system, (D − Λ Lφ − Λ Lλ) y = r for the change y = h′ δT, with Lφ and Lλ the
//! Laplacian's latitude and longitude terms, D the storage and radiative restoring over h′ and r
//! the net heating at the step's start, is split in a fixed order (an approximate factorisation,
//! (D − Λ Lλ) D⁻¹ (D − Λ Lφ)): a tridiagonal solve in latitude for each column,
//! (D − Λ Lφ) y₁ = r, then a periodic one in longitude for each band, (D − Λ Lλ) y = D y₁
//! ([`solve_cyclic_tridiagonal`]). The factorisation's error, Λ² Lλ D⁻¹ Lφ y, is of the step's
//! own order; where the step's change vanishes, so does r, so a steady state is the unsplit
//! equation's. A grid of one column, the zonal model, takes the latitude solve alone.
//! A step's change is scaled down, whole, so that no point's temperature moves by more than a
//! tenth ([`STEP_LIMIT`]): a guard that only the steady state's iteration from far off reaches.
//!
//! The run: the optical depth by bisection on the annual-mean steady state of the zonal model,
//! each from an isothermal start at the mean, under each band's mean insolation over its columns.
//! On the geographic grid that is the steady state itself, since only the heat capacity sees the
//! sea and the columns, and a steady state stores no heat; on the locked grid it is the steady
//! state of the tidally locked longitudes' mean sunlight, the steady state itself in a one-month
//! year, whose beam stands over the pole. Then, in a twelve-month year, whole orbits of steps
//! on the whole grid from that steady state, each column of it the zonal one ([`Sunlight`]),
//! relaxed first on the whole grid where the sunlight varies in longitude
//! ([`orbits`](EnergyBalance::orbits)), until the points' annual means move under
//! [`CONVERGENCE_K`] between orbits, at most [`MAX_ORBITS`]. A one-month year is the steady state
//! itself.

use super::coordinates::Coordinates;
use super::frost;
use super::grid::{
    CyclicWork, GridClimate, LATITUDE_BANDS, ModelGrid, solve_cyclic_tridiagonal, solve_tridiagonal,
};
use super::sun::Sunlight;
use super::transport::{Diffusing, REFERENCE_PRESSURE_PA, rotation_ratio, transport_kg_m2_s};
use crate::math;
use crate::planetary::hooks::surface::{
    EARTH_AIR_MOLAR_MASS_G_PER_MOL, EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
};
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{
    MOLAR_GAS_CONSTANT, SubstanceId, by_key, mean_molar_mass_g_per_mol,
    mixture_heat_capacity_over_r_at, saturation_pressure,
};
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{Kelvin, Seconds};

/// Water's freezing point, the model's `T_f` for water, which R09.T13.d shifts to the climate
/// condensable's ([`EnergyBalance::for_inputs`]), K: 273.15, ice Ih's melting point at 101,325 Pa
/// (273.152 519 K, IAPWS R10-06, to 0.01 K, as the registry's ice density takes it).
pub(super) const WATER_FREEZING_POINT_K: f64 = 273.15;

/// The heat capacity of a land column, J m⁻² K⁻¹: 5.25 × 10⁶ (Spiegel, Menou and Scharf 2008,
/// ApJ 681, 1609, §2.2, read: `C_l` = 5.25 × 10⁹ erg cm⁻² K⁻¹, "the same partial heat capacities
/// over land, ocean, and ice as WK97", Williams and Kasting 1997, Icarus 129, 254, not read).
pub(super) const LAND_HEAT_CAPACITY_J_PER_M2_K: f64 = 5.25e6;

/// The sea's heat capacity over land's at or above the freezing point: 40, Spiegel et al.'s
/// `C_o` = 40 `C_l` "above the wind-mixed surface layer of the ocean" (2008, §2.2).
pub(super) const SEA_HEAT_CAPACITY_RATIO: f64 = 40.0;

/// A frozen sea's heat capacity over land's from [`ICE_CAPACITY_SPAN_K`] below the freezing
/// point up to it: 9.2, Spiegel et al.'s `C_i` = 9.2 `C_l` "when 263 < T < 273 K" (2008, §2.2),
/// the latent heat of a sea freezing.
pub(super) const ICE_NEAR_FREEZING_HEAT_CAPACITY_RATIO: f64 = 9.2;

/// A frozen sea's heat capacity over land's colder than that: 2.0, Spiegel et al.'s `C_i` = 2.0
/// `C_l` "for T ≤ 263 K" (2008, §2.2).
pub(super) const ICE_HEAT_CAPACITY_RATIO: f64 = 2.0;

/// How far below the freezing point a frozen sea keeps its larger capacity, K: 10, Spiegel et
/// al.'s 263 K below their 273 K (2008, §2.2).
pub(super) const ICE_CAPACITY_SPAN_K: f64 = 10.0;

/// The ice albedo's mean, 0.525, half-range, 0.245, and width, 5 K: Spiegel, Menou and Scharf's
/// fiducial A₂(T) = 0.525 − 0.245 tanh((T − 268 K) ÷ 5 K) (2008, Table 1, model 2, read), from
/// about 0.77 under ice to 0.28 over open ground and water.
pub(super) const ALBEDO_MEAN: f64 = 0.525;

/// The ice albedo's half-range ([`ALBEDO_MEAN`]).
pub(super) const ALBEDO_HALF_RANGE: f64 = 0.245;

/// The ice albedo's width, K ([`ALBEDO_MEAN`]).
pub(super) const ALBEDO_WIDTH_K: f64 = 5.0;

/// How far below the freezing point the ice albedo's midpoint lies, K: 5, Spiegel et al.'s 268 K
/// below their 273 K ("at 273 K (water freezing)", 2008, §3), so that it shifts with the freezing
/// point ([`ALBEDO_MEAN`]).
pub(super) const ALBEDO_MIDPOINT_BELOW_FREEZING_K: f64 = 5.0;

/// The albedo where the climate has no condensable that freezes, or one that is supercritical:
/// 0.280, Spiegel et al.'s ice-free limit of their fiducial law, [`ALBEDO_MEAN`] −
/// [`ALBEDO_HALF_RANGE`] ("values ∼0.25–0.3 well above 273 K", 2008, §2.1, as the lane's
/// research-d §5 read it; the lane's lean, the optical depth's fit taking up its level).
pub(super) const ICE_FREE_ALBEDO: f64 = ALBEDO_MEAN - ALBEDO_HALF_RANGE;

/// The near-surface relative humidity of the moist static energy: 0.8 (Siler, Roe and Armour
/// 2018, J. Climate 31, 7481, §1, read: "Typically, relative humidity is assumed to be fixed at
/// 80% … close to the observed near-surface relative humidity over the oceans").
pub(super) const RELATIVE_HUMIDITY: f64 = 0.8;

/// The step of the centred difference that gives h's slope in T, K: 0.01, small against any
/// curvature of the saturation curve, whose form the registry owns.
const ENERGY_SLOPE_STEP_K: f64 = 0.01;

/// The largest emission factor 1 + ¾τ the optical depth's bisection reaches: 10⁶, which holds a
/// surface 31.6 times its equilibrium temperature.
pub(super) const EMISSION_FACTOR_MAX: f64 = 1e6;

/// The bisections of ln(1 + ¾τ) over [0, ln [`EMISSION_FACTOR_MAX`]]: 48, to 5 × 10⁻¹⁴ in the
/// logarithm, far below a kelvin.
pub(super) const OPTICAL_DEPTH_ITERATIONS: u32 = 48;

/// The most iterations of a steady state: 2,000, ten times what the reference Earth takes.
pub(super) const STEADY_ITERATIONS: u32 = 2_000;

/// The change under which a steady state's iteration stops, K: 10⁻⁹.
pub(super) const STEADY_TOLERANCE_K: f64 = 1e-9;

/// The most iterations of the relaxation on the whole grid that spins up a seasonal run whose
/// sunlight varies in longitude ([`EnergyBalance::orbits`]): 250 (`Adopted`). On the locked Earth
/// at 20° of obliquity on a 24-day orbit the state after 250 lies within 0.012 K of the state
/// after 2,000, whose last iterations still move it by 2 × 10⁻⁶ K each.
const SPIN_UP_ITERATIONS: u32 = 250;

/// The pseudo-heat capacity of the steady state's iteration, per step, over each band's emission
/// slope: 0.1, a Newton step damped by a tenth in the emission (`Adopted`: on the reference Earth
/// it converges in about 200 iterations, against about 500 at 1, the ice edge's slow mode setting
/// the pace).
const RELAXATION: f64 = 0.1;

/// The most of the absorbed sunlight's slope in T that a step takes implicitly, over the band's
/// storage and emission slopes: 0.9, so that the step's system stays diagonally dominant where
/// the ice albedo's ramp outweighs the emission (`Adopted`). The rest is taken at the step's
/// start.
const IMPLICIT_ALBEDO_SHARE: f64 = 0.9;

/// The largest change of any point's temperature in one step, over its temperature: 0.1.
pub(super) const STEP_LIMIT: f64 = 0.1;

/// The spacing of the nodes at which a seasonal run tabulates its temperature functions, K: 0.05
/// (`Adopted`): between nodes the cubic Hermite interpolant through each node's value and slope
/// is within about 10⁻¹¹ of the albedo and of h's latent part where they are smooth
/// ([`Tabulated`]).
const TABLE_SPACING_K: f64 = 0.05;

/// The largest departure of the tabulated h from the direct one, over h, and of its slope, over
/// the slope, at which an interval is tabulated: 10⁻¹⁰ and 10⁻⁷ (`Adopted`); and of the albedo
/// and its slope per kelvin, 10⁻¹⁰ and 10⁻⁸.
const TABLE_TOLERANCES: [f64; 4] = [1e-10, 1e-7, 1e-10, 1e-8];

/// How far the table reaches beyond the run's starting temperatures on either side, K: 200
/// (`Adopted`), beyond any season's swing; a temperature outside it is evaluated directly.
const TABLE_MARGIN_K: f64 = 200.0;

/// The most seasonal orbits a run takes after its spin-up: 40 (`Adopted`). An Earth's ocean
/// converges in about ten; an orbit much shorter than the ocean's relaxation time moves little
/// each orbit, so it stops early (R09's Risks, "Deferred findings (2026-10-13 rule)
/// (R09.T13.a)").
pub(super) const MAX_ORBITS: u32 = 40;

/// The move of the points' annual means between orbits under which a run has converged, K: 0.1
/// (Design note 8).
pub(super) const CONVERGENCE_K: f64 = 0.1;

/// Where the model's ice forms, and the albedo it takes: the freezing point lies in the one
/// variant that needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Freezing {
    /// Ice forms below the freezing point `point_k`: the ice albedo of Spiegel et al. turns about
    /// it ([`ALBEDO_MEAN`]), and so does the sea's heat capacity
    /// ([`EnergyBalance::heat_capacity`]).
    At {
        /// The freezing point `T_f`, K.
        point_k: f64,
    },
    /// No ice forms: the albedo is fixed at `albedo`, and the sea keeps its open water's heat
    /// capacity at every temperature. A world with no condensable, a supercritical one or one the
    /// registry has no diagram for takes [`ICE_FREE_ALBEDO`]; Kaspi and Showman's ice-free slab,
    /// the transport's fit, 0.35.
    Never {
        /// The fixed albedo.
        albedo: f64,
    },
}

impl Freezing {
    /// The freezing point, K, or `None` where no ice forms.
    #[cfg(test)]
    #[must_use]
    pub(super) fn point_k(&self) -> Option<f64> {
        match *self {
            Self::At { point_k } => Some(point_k),
            Self::Never { .. } => None,
        }
    }
}

/// An albedo at a temperature and its slope there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AlbedoAt {
    /// The albedo.
    albedo: f64,
    /// Its slope in T, K⁻¹, negative or zero.
    slope_per_k: f64,
}

/// The grey outgoing radiation at a temperature and its slope there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Emission {
    /// σT⁴ ÷ (1 + ¾τ), W m⁻².
    flux_w_m2: f64,
    /// Its slope in T, W m⁻² K⁻¹.
    slope_w_m2_k: f64,
}

/// A steady state's iteration: the points' temperatures and whether it met its tolerance.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Steady {
    /// Each point's temperature, K.
    pub(super) t_k: Vec<f64>,
    /// Whether the iteration met [`STEADY_TOLERANCE_K`] within its count.
    pub(super) converged: bool,
}

/// The optical depth's solve ([`EnergyBalance::emission_factor`]): the emission factor, the
/// annual-mean steady state there and how τ was found.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Fitted {
    /// The emission factor 1 + ¾τ.
    pub(super) emission_factor: f64,
    /// The annual-mean steady state at it.
    pub(super) steady: Steady,
    /// How τ was found.
    pub(super) optical_depth: OpticalDepth,
}

/// The air whose heat the model diffuses: its mean molar mass and specific heat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Air {
    /// The mean molar mass, g mol⁻¹.
    pub(super) molar_mass_g_per_mol: f64,
    /// The specific heat at constant pressure, J kg⁻¹ K⁻¹.
    pub(super) specific_heat_j_per_kg_k: f64,
}

impl Air {
    /// Earth's dry air: [`EARTH_AIR_MOLAR_MASS_G_PER_MOL`] and
    /// [`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`].
    pub(super) const EARTH: Self = Self {
        molar_mass_g_per_mol: EARTH_AIR_MOLAR_MASS_G_PER_MOL,
        specific_heat_j_per_kg_k: EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
    };

    /// The air of `inputs`' gases, by the substance registry: its mean molar mass and its mixed
    /// heat capacity at the mean surface temperature, `c_p` = (Σ xᵢ `c_p,ᵢ`) ÷ (Σ xᵢ Mᵢ), which
    /// needs no normalised fractions. Earth's dry air for a body with no gases, or one the
    /// registry cannot mix (a key it does not hold, or a row without gas columns).
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs) -> Self {
        let mixture: Option<Vec<(SubstanceId, f64)>> = inputs
            .gases()
            .iter()
            .map(|gas| by_key(gas.substance.as_str()).map(|id| (id, gas.mole_fraction)))
            .collect();
        let mixed = mixture.and_then(|mixture| {
            let mass = mean_molar_mass_g_per_mol(&mixture)?;
            let heat =
                mixture_heat_capacity_over_r_at(&mixture, inputs.mean_surface_temperature())?;
            Some((mass, heat))
        });
        match mixed {
            Some((mass, heat)) if positive(mass) && positive(heat) => Self {
                molar_mass_g_per_mol: mass,
                specific_heat_j_per_kg_k: heat * MOLAR_GAS_CONSTANT / (mass * 1e-3),
            },
            _ => Self::EARTH,
        }
    }
}

/// The climate condensable whose latent heat the moist static energy carries.
///
/// Its invariant, a positive and finite surface pressure, is [`Moisture::of`]'s: at no pressure
/// the specific humidity would be 0 ÷ 0. Its fields are open to the climate step's tests, which
/// build one by hand (Kaspi and Showman's configuration); a hand-built one must keep it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Moisture {
    /// The condensable's registry row.
    pub(super) condensable: SubstanceId,
    /// Its latent heat of vaporisation, J kg⁻¹: the registry's enthalpy over its molar mass.
    pub(super) latent_heat_j_per_kg: f64,
    /// Its molar mass over the air's, ε.
    pub(super) mass_ratio: f64,
    /// The surface pressure its specific humidity is taken at, Pa, positive.
    pub(super) surface_pressure_pa: f64,
}

impl Moisture {
    /// The moisture of the body of `inputs` in the air `air`: its climate condensable, where that
    /// is among its surface liquids, has phase columns and a molar mass, under a positive surface
    /// pressure; `None` for a dry world.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, air: &Air) -> Option<Self> {
        let key = inputs.climate().condensable?;
        let pressure = inputs.surface_pressure().value();
        if !inputs
            .liquids()
            .iter()
            .any(|liquid| liquid.substance == key)
            || !positive(pressure)
        {
            return None;
        }
        let condensable = by_key(key.as_str())?;
        let row = condensable.substance();
        let phase = row.phase()?;
        let mass = row.molar_mass_g_per_mol()?;
        Some(Self {
            condensable,
            latent_heat_j_per_kg: phase.vaporisation_enthalpy_j_per_mol() / (mass * 1e-3),
            mass_ratio: mass / air.molar_mass_g_per_mol,
            surface_pressure_pa: pressure,
        })
    }

    /// The specific humidity at the temperature `t_k`, K, and [`RELATIVE_HUMIDITY`]:
    /// q = ε e ÷ (p − (1 − ε) e), e the vapour pressure, its relative humidity of the saturation
    /// pressure but at most the surface pressure, where q is 1.
    #[must_use]
    fn specific_humidity(&self, t_k: f64) -> f64 {
        let saturation = saturation_pressure(self.condensable, Kelvin::new(t_k))
            .map_or(0.0, crate::units::Pascals::value);
        let wanted = RELATIVE_HUMIDITY * saturation;
        let p = self.surface_pressure_pa;
        let e = if wanted < p { wanted } else { p };
        let epsilon = self.mass_ratio;
        epsilon * e / (p - (1.0 - epsilon) * e)
    }
}

/// How the optical depth was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OpticalDepth {
    /// By bisection, so that the steady state's mean is the inputs'.
    Solved,
    /// Clamped at zero: the steady state is warmer than the mean even with no greenhouse, a mean
    /// colder than the albedo allows, which R09.T13.d's constant makes up.
    Transparent,
    /// Clamped at [`EMISSION_FACTOR_MAX`]: still colder than the mean.
    Opaque,
    /// None: the body has neither light nor internal heat, so its state is isothermal at the mean.
    Unforced,
}

/// The model's answer: the time means on its grid, and the emission factor it ran at.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Solution {
    /// The annual and monthly means on the grid, with the orbits run.
    pub(super) climate: GridClimate,
    /// The emission factor 1 + ¾τ.
    pub(super) emission_factor: f64,
    /// How τ was found.
    pub(super) optical_depth: OpticalDepth,
}

/// The model's parameters for one body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct EnergyBalance {
    /// Where the ice forms, the freezing point `T_f` the albedo and the heat capacity turn about,
    /// and the albedo.
    pub(super) freezing: Freezing,
    /// The air's specific heat `c_p`, J kg⁻¹ K⁻¹.
    pub(super) specific_heat_j_per_kg_k: f64,
    /// The moisture, or `None` on a dry world.
    pub(super) moisture: Option<Moisture>,
    /// The transport coefficient Λ, kg m⁻² s⁻¹.
    pub(super) transport_kg_m2_s: f64,
    /// The internal heat flow, W m⁻².
    pub(super) heat_flow_w_m2: f64,
}

/// The model's working arrays: h, its slope, the albedo, its slope, the step's right side, its
/// restoring D, the latitude
/// systems' diagonal and the solution at each grid point; the latitude systems' off-diagonals,
/// the same in every column, and one column's system, at each band; and one band's periodic
/// system in longitude, at each column.
#[derive(Debug, Clone, PartialEq)]
struct Work {
    energy: Vec<f64>,
    slope: Vec<f64>,
    albedo: Vec<f64>,
    albedo_slope: Vec<f64>,
    rhs: Vec<f64>,
    restoring: Vec<f64>,
    diagonal: Vec<f64>,
    solution: Vec<f64>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    column_diagonal: Vec<f64>,
    column_rhs: Vec<f64>,
    column_solution: Vec<f64>,
    column_scratch: Vec<f64>,
    row_lower: Vec<f64>,
    row_diagonal: Vec<f64>,
    row_upper: Vec<f64>,
    row_rhs: Vec<f64>,
    row_solution: Vec<f64>,
    cyclic: CyclicWork,
}

impl Work {
    /// The working arrays of a step on `grid`, every value zero.
    #[must_use]
    fn new(grid: &ModelGrid) -> Self {
        let points = vec![0.0; grid.points()];
        let bands = vec![0.0; LATITUDE_BANDS];
        let columns = vec![0.0; grid.columns()];
        Self {
            energy: points.clone(),
            slope: points.clone(),
            albedo: points.clone(),
            albedo_slope: points.clone(),
            rhs: points.clone(),
            restoring: points.clone(),
            diagonal: points.clone(),
            solution: points,
            lower: bands.clone(),
            upper: bands.clone(),
            column_diagonal: bands.clone(),
            column_rhs: bands.clone(),
            column_solution: bands.clone(),
            column_scratch: bands,
            row_lower: columns.clone(),
            row_diagonal: columns.clone(),
            row_upper: columns.clone(),
            row_rhs: columns.clone(),
            row_solution: columns,
            cyclic: CyclicWork::new(grid.columns().max(3)),
        }
    }
}

/// What a step stores heat in.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Storage<'a> {
    /// The steady state's pseudo-capacity ([`RELAXATION`]).
    Relaxation,
    /// The points' heat capacities over a step of `step_s` seconds, at each point's sea share.
    Seasonal { step_s: f64, sea_share: &'a [f64] },
}

/// A seasonal run's temperature functions, tabulated: h's latent part L q and the albedo, each
/// with its slope in T, at the temperatures k × [`TABLE_SPACING_K`] from a span's first node to
/// its last, read between nodes by the cubic Hermite interpolant (C¹, so the step's slopes are
/// continuous).
///
/// An interval is read from the table only if the interpolant at its quarter points meets
/// [`TABLE_TOLERANCES`] against the direct functions; one that does not, as where the saturation
/// curve has a kink (its triple point, where its branches meet, or where 80% of it reaches the
/// surface pressure and the vapour's share holds at one), is evaluated directly, whatever the
/// curve's form.
///
/// A step evaluates them at every point: the saturation pressure's exponential at T and
/// T ± 0.01 K, and the albedo's hyperbolic tangent. Directly, they took two thirds of a step's
/// time on the geographic grid, 167 ns a point against 52 ns for a dry world of fixed albedo, and
/// 72 ns with the table (measured as built on the reference Earth, at opt-level 2 on a shared
/// machine). The nodes lie on one absolute lattice, so a temperature's value does not depend on
/// the span, only on whether the span holds it; the steady states that set τ evaluate directly.
#[derive(Debug, Clone, PartialEq)]
struct Tabulated {
    first_node: f64,
    latent: Option<Vec<[f64; 2]>>,
    albedo: Vec<[f64; 2]>,
    smooth: Vec<bool>,
}

impl Tabulated {
    /// The table of `model`'s functions over the nodes from `low_k` − [`TABLE_MARGIN_K`] (but
    /// not below 1 K) to `high_k` + [`TABLE_MARGIN_K`].
    #[must_use]
    fn new(model: &EnergyBalance, low_k: f64, high_k: f64) -> Self {
        let lowest = ((low_k - TABLE_MARGIN_K) / TABLE_SPACING_K).floor();
        let floor = (1.0 / TABLE_SPACING_K).round();
        let first_node = if lowest > floor { lowest } else { floor };
        let last_node = ((high_k + TABLE_MARGIN_K) / TABLE_SPACING_K).ceil();
        let last_node = if last_node > first_node + 1.0 {
            last_node
        } else {
            first_node + 1.0
        };
        let count = finite_count(last_node - first_node) + 1;
        let temperatures: Vec<f64> = (0..count)
            .map(|i| (first_node + super::grid::index(i)) * TABLE_SPACING_K)
            .collect();
        let latent = model.moisture.as_ref().map(|moisture| {
            let part = |t: f64| moisture.latent_heat_j_per_kg * moisture.specific_humidity(t);
            temperatures
                .iter()
                .map(|&t| {
                    [
                        part(t),
                        (part(t + ENERGY_SLOPE_STEP_K) - part(t - ENERGY_SLOPE_STEP_K))
                            / (2.0 * ENERGY_SLOPE_STEP_K),
                    ]
                })
                .collect()
        });
        let albedo: Vec<[f64; 2]> = temperatures
            .iter()
            .map(|&t| {
                let at = model.albedo_and_slope(t);
                [at.albedo, at.slope_per_k]
            })
            .collect();
        let mut table = Self {
            first_node,
            latent,
            albedo,
            smooth: vec![true; count - 1],
        };
        let smooth = (0..count - 1)
            .map(|i| {
                [0.25, 0.5, 0.75].into_iter().all(|f| {
                    let t = temperatures[i] + f * TABLE_SPACING_K;
                    let (read, direct) = (table.read(model, i, f), model.functions(t, None));
                    let scale = [direct[0].abs(), direct[1].abs(), 1.0, 1.0];
                    (0..4).all(|k| (read[k] - direct[k]).abs() <= TABLE_TOLERANCES[k] * scale[k])
                })
            })
            .collect();
        table.smooth = smooth;
        table
    }

    /// The functions of `model` read from the table in interval `i` at fraction `f`: h, its
    /// slope, the albedo and its slope.
    #[must_use]
    fn read(&self, model: &EnergyBalance, i: usize, f: f64) -> [f64; 4] {
        let t = (self.first_node + super::grid::index(i) + f) * TABLE_SPACING_K;
        let sensible = model.specific_heat_j_per_kg_k * t;
        let (energy, slope) = match &self.latent {
            Some(latent) => {
                let (value, slope) = hermite(latent, i, f);
                (sensible + value, model.specific_heat_j_per_kg_k + slope)
            }
            None => (sensible, model.specific_heat_j_per_kg_k),
        };
        let (albedo, albedo_slope) = hermite(&self.albedo, i, f);
        [energy, slope, albedo, albedo_slope]
    }

    /// The interval holding `t` K, and the fraction of the way across it, or `None` outside
    /// the span, in an interval evaluated directly, or for a temperature that is not a number.
    #[must_use]
    fn locate(&self, t: f64) -> Option<(usize, f64)> {
        let x = t / TABLE_SPACING_K;
        let node = x.floor();
        let offset = node - self.first_node;
        // By comparison, so that a NaN is outside.
        if !(offset >= 0.0 && offset < super::grid::index(self.smooth.len())) {
            return None;
        }
        let i = finite_count(offset);
        self.smooth[i].then_some((i, x - node))
    }
}

/// The cubic Hermite interpolant of `nodes[i]` and `nodes[i + 1]`, each a value and a slope per
/// kelvin, at the fraction `f` of the way across an interval of [`TABLE_SPACING_K`]: the value
/// and its slope.
#[must_use]
fn hermite(nodes: &[[f64; 2]], i: usize, f: f64) -> (f64, f64) {
    let ([v0, s0], [v1, s1]) = (nodes[i], nodes[i + 1]);
    let (d0, d1) = (TABLE_SPACING_K * s0, TABLE_SPACING_K * s1);
    let (f2, f3) = (f * f, f * f * f);
    let value = (2.0 * f3 - 3.0 * f2 + 1.0) * v0
        + (f3 - 2.0 * f2 + f) * d0
        + (3.0 * f2 - 2.0 * f3) * v1
        + (f3 - f2) * d1;
    let slope = ((6.0 * f2 - 6.0 * f) * v0
        + (3.0 * f2 - 4.0 * f + 1.0) * d0
        + (6.0 * f - 6.0 * f2) * v1
        + (3.0 * f2 - 2.0 * f) * d1)
        / TABLE_SPACING_K;
    (value, slope)
}

/// `x`, a non-negative integer-valued `f64` below 2³², as a `usize`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is a node count or offset of a table of under 2³² nodes, non-negative and \
              integer-valued, so the cast holds it exactly"
)]
fn finite_count(x: f64) -> usize {
    let held = if x > 0.0 { x.min(4.0e9) } else { 0.0 };
    held as usize
}

impl EnergyBalance {
    /// The model of the body of `inputs` (the [module documentation](self)): Kaspi and Showman's
    /// transport at its sidereal rotation and surface pressure, its air's molar mass and heat
    /// capacity, its moisture and heat flow, under the ice albedo about its climate condensable's
    /// freezing point.
    ///
    /// The freezing point is decision-composition (e)'s shift by `SubstanceId`: Spiegel et al.'s
    /// law and Williams and Kasting's capacities are calibrated on water, so every temperature of
    /// theirs moves from [`WATER_FREEZING_POINT_K`] by the condensable's freezing point less
    /// water's, both from the registry's diagrams ([`frost::freezing_point_k`],
    /// [`frost::water_freezing_point_k`]). Water's own shift is exactly zero, so a water world's
    /// model is T13.a's to the bit (research-d §5). Where no ice forms (no condensable, a
    /// supercritical one, or one the registry has no diagram for) the albedo is
    /// [`ICE_FREE_ALBEDO`] and the sea never freezes.
    #[must_use]
    pub(super) fn for_inputs(inputs: &CoarseInputs) -> Self {
        let air = Air::of(inputs);
        let moisture = Moisture::of(inputs, &air);
        let diffusing = if moisture.is_some() {
            Diffusing::MoistStaticEnergy
        } else {
            Diffusing::SensibleHeat
        };
        let freezing = match frost::freezing_point_k(inputs) {
            Some(point) => Freezing::At {
                point_k: WATER_FREEZING_POINT_K + (point - frost::water_freezing_point_k()),
            },
            None => Freezing::Never {
                albedo: ICE_FREE_ALBEDO,
            },
        };
        Self {
            freezing,
            specific_heat_j_per_kg_k: air.specific_heat_j_per_kg_k,
            moisture,
            transport_kg_m2_s: transport_kg_m2_s(
                rotation_ratio(inputs.spin().rotation_period.value()),
                inputs.surface_pressure().value() / REFERENCE_PRESSURE_PA,
                air.molar_mass_g_per_mol,
                air.specific_heat_j_per_kg_k,
                diffusing,
            ),
            heat_flow_w_m2: inputs.heat_flow().value(),
        }
    }

    /// The moist static energy h at the temperature `t_k`, K, J kg⁻¹: `c_p` T + L q, or `c_p` T
    /// dry.
    #[must_use]
    pub(super) fn energy(&self, t_k: f64) -> f64 {
        let sensible = self.specific_heat_j_per_kg_k * t_k;
        match &self.moisture {
            Some(moisture) => {
                sensible + moisture.latent_heat_j_per_kg * moisture.specific_humidity(t_k)
            }
            None => sensible,
        }
    }

    /// h's slope in T at `t_k`, K, J kg⁻¹ K⁻¹, by a centred difference: at least `c_p`, since q
    /// does not fall with T.
    #[must_use]
    fn energy_slope(&self, t_k: f64) -> f64 {
        match &self.moisture {
            Some(_) => {
                (self.energy(t_k + ENERGY_SLOPE_STEP_K) - self.energy(t_k - ENERGY_SLOPE_STEP_K))
                    / (2.0 * ENERGY_SLOPE_STEP_K)
            }
            None => self.specific_heat_j_per_kg_k,
        }
    }

    /// The albedo at `t_k`, K.
    #[cfg(test)]
    #[must_use]
    pub(super) fn albedo_at(&self, t_k: f64) -> f64 {
        self.albedo_and_slope(t_k).albedo
    }

    /// The albedo at `t_k`, K, and its slope in T.
    #[must_use]
    fn albedo_and_slope(&self, t_k: f64) -> AlbedoAt {
        match self.freezing {
            Freezing::At { point_k } => {
                let midpoint = point_k - ALBEDO_MIDPOINT_BELOW_FREEZING_K;
                let ramp = math::tanh((t_k - midpoint) / ALBEDO_WIDTH_K);
                AlbedoAt {
                    albedo: ALBEDO_MEAN - ALBEDO_HALF_RANGE * ramp,
                    slope_per_k: -ALBEDO_HALF_RANGE * (1.0 - ramp * ramp) / ALBEDO_WIDTH_K,
                }
            }
            Freezing::Never { albedo } => AlbedoAt {
                albedo,
                slope_per_k: 0.0,
            },
        }
    }

    /// h at `t_k`, K, its slope, the albedo and its slope, from `table` where it holds `t_k` and
    /// directly otherwise ([`energy`](Self::energy), [`energy_slope`](Self::energy_slope),
    /// [`albedo_and_slope`](Self::albedo_and_slope)).
    #[must_use]
    fn functions(&self, t_k: f64, table: Option<&Tabulated>) -> [f64; 4] {
        if let Some((table, (i, f))) = table.and_then(|table| Some((table, table.locate(t_k)?))) {
            return table.read(self, i, f);
        }
        let albedo = self.albedo_and_slope(t_k);
        [
            self.energy(t_k),
            self.energy_slope(t_k),
            albedo.albedo,
            albedo.slope_per_k,
        ]
    }

    /// The heat capacity, J m⁻² K⁻¹, of a grid point of sea share `sea_share` at `t_k`, K:
    /// (1 − s) `C_l` + s `C_sea`(T), the sea's open water's at every temperature where no ice
    /// forms.
    #[must_use]
    pub(super) fn heat_capacity(&self, t_k: f64, sea_share: f64) -> f64 {
        let sea_ratio = match self.freezing {
            Freezing::At { point_k } if t_k < point_k => {
                if t_k > point_k - ICE_CAPACITY_SPAN_K {
                    ICE_NEAR_FREEZING_HEAT_CAPACITY_RATIO
                } else {
                    ICE_HEAT_CAPACITY_RATIO
                }
            }
            Freezing::At { .. } | Freezing::Never { .. } => SEA_HEAT_CAPACITY_RATIO,
        };
        LAND_HEAT_CAPACITY_J_PER_M2_K * ((1.0 - sea_share) + sea_share * sea_ratio)
    }

    /// The net heating of every grid point at temperatures `t_k`, K, under insolation
    /// `insolation_w_m2` and emission factor `emission_factor`, W m⁻²: absorbed, internal and
    /// emitted, then the diffusion's convergence, each per point.
    #[cfg(test)]
    #[must_use]
    pub(super) fn heating(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        t_k: &[f64],
    ) -> (Vec<f64>, Vec<f64>) {
        let energy: Vec<f64> = t_k.iter().map(|&v| self.energy(v)).collect();
        let radiative = (0..grid.points())
            .map(|p| {
                insolation_w_m2[p] * (1.0 - self.albedo_at(t_k[p])) + self.heat_flow_w_m2
                    - emission(t_k[p], emission_factor).flux_w_m2
            })
            .collect();
        let diffusive = (0..grid.points())
            .map(|p| {
                let (band, column) = (p / grid.columns(), p % grid.columns());
                self.transport_kg_m2_s * convergence(grid, &energy, band, column)
            })
            .collect();
        (radiative, diffusive)
    }

    /// One linearised backward-Euler step of the points' temperatures `t_k`, K, under insolation
    /// `insolation_w_m2` (one value per point) and emission factor `emission_factor`, storing heat
    /// in `storage`, split into the latitude solve and then the longitude one (the
    /// [module documentation](self)); the largest change, K.
    #[expect(
        clippy::too_many_arguments,
        reason = "a step reads the grid, the forcing, the storage, the table and the state, and \
                  writes the work arrays; bundling them would only rename them"
    )]
    fn advance(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        storage: Storage<'_>,
        table: Option<&Tabulated>,
        t_k: &mut [f64],
        work: &mut Work,
    ) -> f64 {
        let lambda = self.transport_kg_m2_s;
        let columns = grid.columns();
        for (p, &v) in t_k.iter().enumerate() {
            let [energy, slope, albedo, albedo_slope] = self.functions(v, table);
            work.energy[p] = energy;
            work.slope[p] = slope;
            work.albedo[p] = albedo;
            work.albedo_slope[p] = albedo_slope;
        }
        for k in 0..LATITUDE_BANDS {
            let (below, above, w) = (
                grid.edge_conductance(k),
                grid.edge_conductance(k + 1),
                grid.weight(k),
            );
            // The unknown is the change of h, y = h′ δT, which keeps the system diagonally
            // dominant whatever h′ is.
            work.lower[k] = -lambda * below / w;
            work.upper[k] = -lambda * above / w;
            for column in 0..columns {
                let p = grid.point(k, column);
                let tk = t_k[p];
                let Emission {
                    flux_w_m2: emitted,
                    slope_w_m2_k: emission_slope,
                } = emission(tk, emission_factor);
                let stored = match storage {
                    Storage::Relaxation => RELAXATION * emission_slope,
                    Storage::Seasonal { step_s, sea_share } => {
                        self.heat_capacity(tk, sea_share[p]) / step_s
                    }
                };
                let (albedo, albedo_slope) = (work.albedo[p], work.albedo_slope[p]);
                work.rhs[p] = insolation_w_m2[p] * (1.0 - albedo) + self.heat_flow_w_m2 - emitted
                    + lambda * convergence(grid, &work.energy, k, column);
                let restoring = stored + emission_slope;
                let absorbed_slope = -insolation_w_m2[p] * albedo_slope;
                let implicit = if absorbed_slope < IMPLICIT_ALBEDO_SHARE * restoring {
                    absorbed_slope
                } else {
                    IMPLICIT_ALBEDO_SHARE * restoring
                };
                work.restoring[p] = (restoring - implicit) / work.slope[p];
                work.diagonal[p] = work.restoring[p] + lambda * (below + above) / w;
            }
        }
        // The latitude solve, column by column from the west.
        for column in 0..columns {
            for k in 0..LATITUDE_BANDS {
                let p = grid.point(k, column);
                work.column_diagonal[k] = work.diagonal[p];
                work.column_rhs[k] = work.rhs[p];
            }
            solve_tridiagonal(
                &work.lower,
                &work.column_diagonal,
                &work.upper,
                &work.column_rhs,
                &mut work.column_solution,
                &mut work.column_scratch,
            );
            for k in 0..LATITUDE_BANDS {
                work.solution[grid.point(k, column)] = work.column_solution[k];
            }
        }
        if columns > 1 {
            solve_longitude(lambda, grid, work);
        }
        let worst = (0..grid.points()).fold(0.0, |worst, p| {
            let relative = (work.solution[p] / work.slope[p]).abs() / t_k[p];
            if relative > worst { relative } else { worst }
        });
        let scale = if worst > STEP_LIMIT {
            STEP_LIMIT / worst
        } else {
            1.0
        };
        let mut largest = 0.0;
        for (p, v) in t_k.iter_mut().enumerate() {
            let change = scale * work.solution[p] / work.slope[p];
            *v += change;
            if change.abs() > largest {
                largest = change.abs();
            }
        }
        largest
    }

    /// The steady state under constant insolation `insolation_w_m2` at emission factor
    /// `emission_factor`, iterated from the temperatures `start_k`, K, and whether its iteration
    /// met [`STEADY_TOLERANCE_K`] within [`STEADY_ITERATIONS`].
    #[must_use]
    pub(super) fn steady_state(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        start_k: Vec<f64>,
    ) -> Steady {
        self.relax(
            grid,
            emission_factor,
            insolation_w_m2,
            start_k,
            None,
            STEADY_ITERATIONS,
        )
    }

    /// [`steady_state`](Self::steady_state) for at most `iterations`, its functions read from
    /// `table` where it holds them.
    #[must_use]
    fn relax(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        start_k: Vec<f64>,
        table: Option<&Tabulated>,
        iterations: u32,
    ) -> Steady {
        let mut t_k = start_k;
        let mut work = Work::new(grid);
        for _ in 0..iterations {
            let change = self.advance(
                grid,
                emission_factor,
                insolation_w_m2,
                Storage::Relaxation,
                table,
                &mut t_k,
                &mut work,
            );
            if change < STEADY_TOLERANCE_K {
                return Steady {
                    t_k,
                    converged: true,
                };
            }
        }
        Steady {
            t_k,
            converged: false,
        }
    }

    /// The emission factor 1 + ¾τ at which the annual-mean steady state under `insolation_w_m2`,
    /// each solve from an isothermal start at `mean_k`, has the global mean `mean_k`, K; the
    /// steady state there, and how τ was found.
    #[must_use]
    pub(super) fn emission_factor(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
    ) -> Fitted {
        let solve = |factor: f64| {
            let steady =
                self.steady_state(grid, factor, insolation_w_m2, vec![mean_k; grid.points()]);
            (grid.mean(&steady.t_k), steady)
        };
        let fitted = |emission_factor: f64, steady: Steady, optical_depth: OpticalDepth| Fitted {
            emission_factor,
            steady,
            optical_depth,
        };
        let (clear_mean, clear) = solve(1.0);
        if clear_mean >= mean_k {
            return fitted(1.0, clear, OpticalDepth::Transparent);
        }
        let (opaque_mean, opaque) = solve(EMISSION_FACTOR_MAX);
        if opaque_mean < mean_k {
            return fitted(EMISSION_FACTOR_MAX, opaque, OpticalDepth::Opaque);
        }
        let (mut low, mut high) = (0.0, math::ln(EMISSION_FACTOR_MAX));
        for _ in 0..OPTICAL_DEPTH_ITERATIONS {
            let middle = f64::midpoint(low, high);
            if solve(math::exp(middle)).0 < mean_k {
                low = middle;
            } else {
                high = middle;
            }
        }
        let factor = math::exp(f64::midpoint(low, high));
        let (_, steady) = solve(factor);
        fitted(factor, steady, OpticalDepth::Solved)
    }

    /// Whole seasonal orbits of `sunlight` at emission factor `emission_factor` from `start_k`, K,
    /// over points of sea share `sea_share`, until the annual means move under [`CONVERGENCE_K`]
    /// between orbits, at most [`MAX_ORBITS`]: the last orbit's annual and monthly means.
    ///
    /// Where the sunlight varies in the grid's longitude, as a locked world's beam does on an
    /// eccentric or tilted orbit, `start`, the zonal model's steady state under each band's mean
    /// sunlight, is first relaxed towards the whole grid's annual-mean steady state, for
    /// [`SPIN_UP_ITERATIONS`]: the split's error slows the modes that vary in both directions
    /// under the relaxation's small storage, so it does not meet [`STEADY_TOLERANCE_K`], but it
    /// spares the orbits most of the ocean's slow relaxation from the zonal start (on the locked
    /// Earth at 20° of obliquity, all sea, 17 orbits fall to 2, its annual mean then within 0.2 K
    /// of that state).
    ///
    /// # Panics
    ///
    /// If `sunlight` is constant, or a month holds no step, which [`Sunlight::seasonal`]'s step
    /// count rules out.
    #[must_use]
    pub(super) fn orbits(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        emission_factor: f64,
        start_k: Vec<f64>,
        sea_share: &[f64],
    ) -> GridClimate {
        let steps = sunlight.steps();
        assert!(steps > 0, "a seasonal run needs a seasonal orbit");
        let points = grid.points();
        let storage = Storage::Seasonal {
            step_s: sunlight.step_s(),
            sea_share,
        };
        let (low, high) = start_k
            .iter()
            .fold((f64::INFINITY, 0.0), |(low, high), &v| {
                (
                    if v < low { v } else { low },
                    if v > high { v } else { high },
                )
            });
        let table = Tabulated::new(self, low, high);
        let start_k = if sunlight.varies_in_longitude() {
            self.relax(
                grid,
                emission_factor,
                sunlight.annual(),
                start_k,
                Some(&table),
                SPIN_UP_ITERATIONS,
            )
            .t_k
        } else {
            start_k
        };
        let mut t = start_k;
        let mut work = Work::new(grid);
        let mut insolation = vec![0.0; points];
        // The last orbit's annual means, `None` before the first, its buffer reused orbit to orbit.
        let mut previous: Option<Vec<f64>> = None;
        let (mut annual, mut months) = (vec![0.0; points], vec![vec![0.0; points]; 12]);
        let mut counts = [0_u32; 12];
        let (mut orbits, mut converged) = (0, false);
        while orbits < MAX_ORBITS {
            annual.fill(0.0);
            for month in &mut months {
                month.fill(0.0);
            }
            counts = [0; 12];
            for step in 0..steps {
                sunlight.fill(step, grid, &mut insolation);
                self.advance(
                    grid,
                    emission_factor,
                    &insolation,
                    storage,
                    Some(&table),
                    &mut t,
                    &mut work,
                );
                let month = usize::from(sunlight.month_of(step));
                counts[month] += 1;
                for ((sum, month_sum), &v) in annual.iter_mut().zip(&mut months[month]).zip(&t) {
                    *sum += v;
                    *month_sum += v;
                }
            }
            orbits += 1;
            let n = f64::from(steps);
            for sum in &mut annual {
                *sum /= n;
            }
            if let Some(last) = &previous {
                let moved = annual.iter().zip(last).fold(0.0, |most: f64, (a, b)| {
                    let d = (a - b).abs();
                    if d > most { d } else { most }
                });
                if moved < CONVERGENCE_K {
                    converged = true;
                    break;
                }
            }
            previous.get_or_insert_with(Vec::new).clone_from(&annual);
        }
        for (month, count) in months.iter_mut().zip(counts) {
            assert!(count > 0, "every month holds a step");
            let n = f64::from(count);
            for sum in month.iter_mut() {
                *sum /= n;
            }
        }
        GridClimate {
            annual_k: annual,
            months_k: months,
            orbits,
            converged,
        }
    }

    /// The model's run over `sunlight` for a body of mean surface temperature `mean_k`, over the
    /// points of `grid` of sea share `sea_share` (the [module documentation](self)).
    ///
    /// # Panics
    ///
    /// If a value of the answer is not finite: a fault of the model.
    #[must_use]
    pub(super) fn run(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        mean_k: f64,
        sea_share: &[f64],
    ) -> Solution {
        let months = usize::from(sunlight.months());
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let forcing = zonal.mean(&annual) + self.heat_flow_w_m2;
        let solution = if positive(forcing) {
            let Fitted {
                emission_factor: factor,
                steady,
                optical_depth: depth,
            } = self.emission_factor(&zonal, &annual, mean_k);
            let converged = steady.converged;
            let steady = grid.broadcast(&steady.t_k);
            let climate = if sunlight.steps() == 0 {
                GridClimate {
                    annual_k: steady.clone(),
                    months_k: vec![steady; months],
                    orbits: 0,
                    converged,
                }
            } else {
                self.orbits(grid, sunlight, factor, steady, sea_share)
            };
            Solution {
                climate,
                emission_factor: factor,
                optical_depth: depth,
            }
        } else {
            let isothermal = vec![mean_k; grid.points()];
            Solution {
                climate: GridClimate {
                    annual_k: isothermal.clone(),
                    months_k: vec![isothermal; months],
                    orbits: 0,
                    converged: true,
                },
                emission_factor: 1.0,
                optical_depth: OpticalDepth::Unforced,
            }
        };
        let climate = &solution.climate;
        for v in climate
            .annual_k
            .iter()
            .chain(climate.months_k.iter().flatten())
        {
            let _finite = super::finite_k(*v);
        }
        solution
    }
}

/// A step's longitude solve on `grid` of more than one column, under the transport coefficient
/// `lambda`, kg m⁻² s⁻¹: band by band from the south, (D − Λ Lλ) y = D y₁, from the latitude
/// solve's y₁ in `work`'s solution, into it ([`solve_cyclic_tridiagonal`]).
fn solve_longitude(lambda: f64, grid: &ModelGrid, work: &mut Work) {
    let columns = grid.columns();
    for k in 0..LATITUDE_BANDS {
        let coupling = lambda * grid.longitude_coupling(k);
        for column in 0..columns {
            let p = grid.point(k, column);
            work.row_lower[column] = -coupling;
            work.row_upper[column] = -coupling;
            work.row_diagonal[column] = work.restoring[p] + 2.0 * coupling;
            work.row_rhs[column] = work.restoring[p] * work.solution[p];
        }
        solve_cyclic_tridiagonal(
            &work.row_lower,
            &work.row_diagonal,
            &work.row_upper,
            &work.row_rhs,
            &mut work.row_solution,
            &mut work.cyclic,
        );
        for column in 0..columns {
            work.solution[grid.point(k, column)] = work.row_solution[column];
        }
    }
}

/// The grey outgoing radiation σT⁴ ÷ (1 + ¾τ) at `t_k`, K, under emission factor
/// `emission_factor`, and its slope in T (Spiegel, Menou and Scharf 2008, Table 1, models 1–2's
/// Eddington form, read).
#[must_use]
fn emission(t_k: f64, emission_factor: f64) -> Emission {
    let cube = t_k * t_k * t_k;
    Emission {
        flux_w_m2: STEFAN_BOLTZMANN * cube * t_k / emission_factor,
        slope_w_m2_k: 4.0 * STEFAN_BOLTZMANN * cube / emission_factor,
    }
}

/// The diffusion's convergence at the point of band `band` and column `column` of the field
/// `energy`, J kg⁻¹ per unit of Λ: the point's average of ∇²h in flux form ([`ModelGrid`]), the
/// latitude term no-flux at the poles, and on a grid of more than one column the longitude term,
/// periodic round the band.
#[must_use]
fn convergence(grid: &ModelGrid, energy: &[f64], band: usize, column: usize) -> f64 {
    let at = |k: usize| energy[grid.point(k, column)];
    let here = at(band);
    let mut flux = 0.0;
    if band + 1 < LATITUDE_BANDS {
        flux += grid.edge_conductance(band + 1) * (at(band + 1) - here);
    }
    if band > 0 {
        flux -= grid.edge_conductance(band) * (here - at(band - 1));
    }
    let latitude = flux / grid.weight(band);
    let columns = grid.columns();
    if columns == 1 {
        return latitude;
    }
    // Round the band without an integer division, the step's costliest operation per point.
    let east = if column + 1 < columns { column + 1 } else { 0 };
    let west = if column > 0 { column - 1 } else { columns - 1 };
    let (east, west) = (
        energy[grid.point(band, east)],
        energy[grid.point(band, west)],
    );
    latitude + grid.longitude_coupling(band) * ((east - here) - (here - west))
}

/// The equator–pole contrast of a zonal field `t`, K: the cubic's value at the equator less its
/// mean at the two poles ([`ModelGrid::stencil`]).
#[cfg(test)]
#[must_use]
pub(super) fn contrast(t: &[f64]) -> f64 {
    let at = |z: f64| ModelGrid::stencil(z).apply(t);
    at(0.0) - f64::midpoint(at(1.0), at(-1.0))
}

/// The model's answer for the body of `inputs`, which turns under its sun, over the points of
/// `grid` (the zonal or the geographic grid) of sea share `sea_share`, under the diurnal-mean
/// sunlight, at a nominal step of `step` ([`NOMINAL_STEP`](super::sun::NOMINAL_STEP) in the
/// pass).
#[must_use]
pub(super) fn solve(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
    )
}

/// The model's answer for the body of `inputs`, locked 1:1 to its seasonal host, over the points
/// of `grid` (the locked grid) in tidally locked coordinates, of sea share `sea_share`, under the
/// host's beam ([`Sunlight::locked`]), at a nominal step of `step`.
#[must_use]
pub(super) fn solve_locked(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::locked(inputs, grid, Coordinates::TidallyLocked, step);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
    )
}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::surface::reference;

    fn earth() -> (EnergyBalance, Sunlight, ModelGrid) {
        let inputs = reference::earth_like();
        let grid = ModelGrid::zonal();
        let sun = Sunlight::of(&inputs, &grid, super::super::sun::NOMINAL_STEP);
        (EnergyBalance::for_inputs(&inputs), sun, grid)
    }

    /// The reference Earth's air is the registry's, dry air's 1,004.7 J kg⁻¹ K⁻¹ and 28.96 g mol⁻¹
    /// with its water vapour; its water is moist static energy at Earth's transport.
    #[test]
    fn the_reference_earth_diffuses_moist_static_energy_in_its_own_air() {
        let inputs = reference::earth_like();
        let air = Air::of(&inputs);
        // Dry air's 28.96 g mol⁻¹ and 1,004.7 J kg⁻¹ K⁻¹ with 1.67% of water vapour (18.02 g mol⁻¹,
        // about 1,860 J kg⁻¹ K⁻¹), the reference's own (`reference::earth_air`).
        assert!((air.molar_mass_g_per_mol - 28.78).abs() < 0.02, "{air:?}");
        assert!(
            (air.specific_heat_j_per_kg_k - 1_012.8).abs() < 2.0,
            "{air:?}"
        );
        let model = EnergyBalance::for_inputs(&inputs);
        let moisture = model.moisture.expect("water is among Earth's liquids");
        assert!(
            (moisture.latent_heat_j_per_kg - 2.415e6).abs() < 1e3,
            "{moisture:?}"
        );
        // h rises with T, by more than c_p, and q at 288 K and 80% is about 8.5 g kg⁻¹.
        let q = moisture.specific_humidity(288.0);
        assert!((q - 8.5e-3).abs() < 0.5e-3, "{q}");
        assert!(model.energy_slope(288.0) > 2.0 * air.specific_heat_j_per_kg_k);
        let lambda = model.transport_kg_m2_s / super::super::transport::EARTH_TRANSPORT_KG_M2_S;
        assert!((lambda - 1.0).abs() < 0.05, "{lambda}");
    }

    /// The steady state's net heating sums to nothing over the sphere, the diffusion's to
    /// rounding, and its mean is the inputs' 288 K.
    #[test]
    fn the_steady_state_conserves_energy_and_holds_the_mean() {
        let (model, sun, grid) = earth();
        let Fitted {
            emission_factor: factor,
            steady: Steady {
                t_k: steady,
                converged,
            },
            optical_depth: depth,
        } = model.emission_factor(&grid, sun.annual(), 288.0);
        assert!(converged);
        assert_eq!(depth, OpticalDepth::Solved);
        assert!(
            (grid.mean(&steady) - 288.0).abs() < 1e-6,
            "{}",
            grid.mean(&steady)
        );
        let tau = (factor - 1.0) / 0.75;
        assert!((0.3..2.0).contains(&tau), "{tau}");
        let (radiative, diffusive) = model.heating(&grid, factor, sun.annual(), &steady);
        let absorbed = grid.mean(sun.annual());
        assert!(
            grid.mean(&diffusive).abs() < 1e-12 * absorbed,
            "{}",
            grid.mean(&diffusive)
        );
        assert!(
            grid.mean(&radiative).abs() < 1e-6 * absorbed,
            "{}",
            grid.mean(&radiative)
        );
        for k in 0..LATITUDE_BANDS {
            let net = radiative[k] + diffusive[k];
            assert!(net.abs() < 1e-5, "band {k}: {net}");
        }
        // Warm at the equator, cold at the poles, and symmetric.
        assert!(contrast(&steady) > 20.0);
        assert!((steady[0] - steady[LATITUDE_BANDS - 1]).abs() < 1e-6);
    }

    /// A mean colder than the albedo allows clamps τ at zero; one that no greenhouse within the
    /// bound reaches clamps it there.
    #[test]
    fn the_optical_depth_is_clamped_where_no_greenhouse_holds_the_mean() {
        let (model, sun, grid) = earth();
        let clear = model.emission_factor(&grid, sun.annual(), 150.0);
        assert_eq!(clear.optical_depth, OpticalDepth::Transparent);
        assert!((clear.emission_factor - 1.0).abs() < 1e-15);
        let hot = model.emission_factor(&grid, sun.annual(), 1e5);
        assert_eq!(hot.optical_depth, OpticalDepth::Opaque);
        assert!((hot.emission_factor - EMISSION_FACTOR_MAX).abs() < 1e-6);
        assert!(hot.steady.t_k.iter().all(|t| t.is_finite() && *t > 288.0));
    }

    /// A body with neither light nor heat is isothermal at its mean; one lit only by its own heat
    /// holds a finite, uniform mean.
    #[test]
    fn a_body_without_light_still_gives_a_finite_answer() {
        let (mut model, _, grid) = earth();
        let dark = Sunlight::constant(vec![0.0; LATITUDE_BANDS]);
        model.heat_flow_w_m2 = 0.0;
        let none = model.run(&grid, &dark, 40.0, &[0.0; LATITUDE_BANDS]);
        assert_eq!(none.optical_depth, OpticalDepth::Unforced);
        assert!(
            none.climate
                .annual_k
                .iter()
                .all(|t| (t - 40.0).abs() < 1e-12)
        );
        model.heat_flow_w_m2 = 0.0916;
        let warm = model.run(&grid, &dark, 60.0, &[0.0; LATITUDE_BANDS]);
        assert_eq!(warm.optical_depth, OpticalDepth::Solved);
        assert!(
            warm.climate
                .annual_k
                .iter()
                .all(|t| (t - 60.0).abs() < 1e-3),
            "{warm:?}"
        );
    }

    /// One split step on the geographic grid, over a continent and from a state that varies in
    /// latitude and longitude, conserves energy: the area-weighted sum of its change times the
    /// restoring, Σ w D y, is the net heating's, Σ w r, since each solve's diffusion sums to
    /// nothing; and the diffusion's convergence itself sums to nothing over the sphere.
    #[test]
    fn the_split_step_conserves_energy() {
        let (model, sun, _) = earth();
        let grid = ModelGrid::geographic();
        let t: Vec<f64> = (0..grid.points())
            .map(|p| {
                let d = grid.centre_direction(p);
                260.0 + 30.0 * (1.0 - d[2] * d[2]) + 5.0 * d[0] - 3.0 * d[1] * d[2]
            })
            .collect();
        let sea: Vec<f64> = (0..grid.points())
            .map(|p| {
                if grid.centre_direction(p)[0] > 0.3 {
                    0.0
                } else {
                    1.0
                }
            })
            .collect();
        let mut insolation = vec![0.0; grid.points()];
        sun.fill(300, &grid, &mut insolation);
        let factor = 1.65;
        let mut work = Work::new(&grid);
        let mut state = t.clone();
        model.advance(
            &grid,
            factor,
            &insolation,
            Storage::Seasonal {
                step_s: sun.step_s(),
                sea_share: &sea,
            },
            None,
            &mut state,
            &mut work,
        );
        let weighted = |f: &dyn Fn(usize) -> f64| {
            (0..grid.points()).fold(0.0, |sum, p| sum + grid.weight(p / grid.columns()) * f(p))
        };
        let stored = weighted(&|p| work.restoring[p] * work.solution[p]);
        let heating = weighted(&|p| work.rhs[p]);
        let scale = weighted(&|p| work.rhs[p].abs());
        assert!(
            (stored - heating).abs() < 1e-12 * scale,
            "{stored} against {heating} of {scale}"
        );
        assert!(state.iter().zip(&t).any(|(a, b)| (a - b).abs() > 1e-3));
        let (_, diffusive) = model.heating(&grid, factor, &insolation, &t);
        let total = weighted(&|p| diffusive[p]);
        let size = weighted(&|p| diffusive[p].abs());
        assert!(total.abs() < 1e-12 * size, "{total} of {size}");
    }

    /// The tabulated functions are the direct ones to the interpolant's error, h within 10⁻⁹ of
    /// itself and its slope within 10⁻⁶, the albedo and its slope within 10⁻⁹, everywhere: the
    /// intervals holding the saturation curve's kinks, at the triple point and where the vapour
    /// reaches the surface pressure (377.7 K under the reference Earth's 98,550 Pa), are
    /// evaluated directly, as is a temperature outside the span.
    #[test]
    fn the_table_is_the_direct_functions_to_its_interpolant_s_error() {
        let (model, _, _) = earth();
        let table = Tabulated::new(&model, 250.0, 300.0);
        let (mut worst_h, mut worst_slope) = (0.0_f64, 0.0_f64);
        let (mut worst_albedo, mut worst_albedo_slope) = (0.0_f64, 0.0_f64);
        for i in 0..40_000 {
            let t = 60.0 + 0.012_345 * f64::from(i);
            let tabulated = model.functions(t, Some(&table));
            let direct = model.functions(t, None);
            worst_h = worst_h.max((tabulated[0] - direct[0]).abs() / direct[0]);
            worst_slope = worst_slope.max((tabulated[1] - direct[1]).abs() / direct[1]);
            worst_albedo = worst_albedo.max((tabulated[2] - direct[2]).abs());
            worst_albedo_slope = worst_albedo_slope.max((tabulated[3] - direct[3]).abs());
        }
        let direct = table.smooth.iter().filter(|s| !**s).count();
        let runs: Vec<(f64, f64)> = table
            .smooth
            .iter()
            .enumerate()
            .filter(|(_, s)| !**s)
            .map(|(i, _)| {
                let t = (table.first_node + super::super::grid::index(i)) * TABLE_SPACING_K;
                (t, t + TABLE_SPACING_K)
            })
            .collect();
        eprintln!(
            "h {worst_h:e}, slope {worst_slope:e}, albedo {worst_albedo:e}, its slope \
             {worst_albedo_slope:e}; {direct} of {} intervals direct: {:?} … {:?}",
            table.smooth.len(),
            runs.first(),
            runs.last()
        );
        assert!(worst_h < 1e-9 && worst_slope < 1e-6);
        assert!(worst_albedo < 1e-9 && worst_albedo_slope < 1e-8);
        for kink in [273.16, 377.7] {
            assert!(table.locate(kink).is_none(), "{kink}");
        }
        assert!(direct < table.smooth.len() / 20, "{direct}");
        // Within a climate's range only the triple point's interval is direct.
        let within: Vec<&(f64, f64)> = runs
            .iter()
            .filter(|(low, high)| *high > 200.0 && *low < 340.0)
            .collect();
        eprintln!("direct within 200–340 K: {within:?}");
        assert!(
            within
                .iter()
                .all(|(low, high)| *low <= 273.16 && *high >= 273.16)
        );
        let node = 280.0;
        let (at_node, direct) = (
            model.functions(node, Some(&table)),
            model.functions(node, None),
        );
        assert!((at_node[0] - direct[0]).abs() < 1e-9 * direct[0]);
        assert!(table.locate(30.0).is_none() && table.locate(f64::NAN).is_none());
        for (a, b) in model
            .functions(30.0, Some(&table))
            .into_iter()
            .zip(model.functions(30.0, None))
        {
            hyperion_testkit::float::assert_same_bits(a, b);
        }
    }

    /// A dry world diffuses `c_p` T alone at half the transport; its steady contrast is larger.
    #[test]
    fn a_dry_world_diffuses_sensible_heat_at_half_the_transport() {
        let inputs = reference::earth_like();
        let wet = EnergyBalance::for_inputs(&inputs);
        let mut model = wet;
        model.moisture = None;
        model.transport_kg_m2_s *= 0.5;
        assert!((model.energy(288.0) - model.specific_heat_j_per_kg_k * 288.0).abs() < 1e-9);
        let grid = ModelGrid::zonal();
        let sun = Sunlight::of(&inputs, &grid, super::super::sun::NOMINAL_STEP);
        let wet_state = wet.emission_factor(&grid, sun.annual(), 288.0).steady.t_k;
        let dry_state = model.emission_factor(&grid, sun.annual(), 288.0).steady.t_k;
        assert!(contrast(&dry_state) > contrast(&wet_state) + 5.0);
        // A world whose condensable is not among its liquids is dry, as is one with no pressure.
        let mars = reference::mars_like();
        assert!(EnergyBalance::for_inputs(&mars).moisture.is_none());
    }
}
