//! Design note 8's seasonal moist energy-balance model (R09.T13.a–b, g): North and Coakley's
//! diffusive equation in Spiegel, Menou and Scharf's form, diffusing near-surface moist static
//! energy as Siler, Roe and Armour do, in latitude and longitude, stepped implicitly over the
//! seasonal orbit, over each grid point's relief.
//!
//! # The equation
//!
//! At each grid point, of sea share s, at the surface temperature T at sea level, written here
//! for a point at the sea level (the relief's terms follow):
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
//! - σT⁴ ÷ (1 + ¾τ) the grey outgoing radiation in Spiegel et al.'s Eddington form, its τ solved
//!   once so that the surface mean of the annual-mean steady state is plan 14's mean
//!   ([`CoarseInputs::mean_surface_temperature`]): where the body's gases hold water, plan 14's
//!   own grey law at the column's own temperature, τ(T) = m [`τ_d` + `τ_w`(T)], `τ_d` plan 14's terms
//!   of the other gases, `τ_w` = `k_w` √(`p_w`(T) ÷ 1 bar) its water term and m one multiplier for
//!   the body ([`Greenhouse::Vapour`]; R09.T13.i, R1 of decision-r09-t13i §2.2), which thickens the
//!   moist tropics' column and thins the dry poles': `p_w` = `h_w` `p_sat`(T) at the local
//!   saturation where liquid water lies at the surface, and otherwise the lesser of that and the
//!   air's own water, `x_w` `p_s` (its §2.4 follow-up 2 §4, [`Reservoir`]); where the gases hold no
//!   water, and in Kaspi and Showman's fit, one τ ([`Greenhouse::Grey`]);
//! - h the near-surface moist static energy `c_p` T + L q, with q the climate condensable's
//!   specific humidity at 80% relative humidity ([`RELATIVE_HUMIDITY`]), from its saturation
//!   pressure in the substance registry at the surface pressure, L its latent heat of
//!   vaporisation per kilogram, and `c_p` the air's mixed heat capacity: on a dry world, one whose
//!   condensable is not among its surface liquids or that has none, h is `c_p` T alone;
//! - Λ the transport's coefficient ([`transport`](super::transport)), no-flux at the poles.
//!
//! # The relief (R09.T13.g)
//!
//! Each point holds its surface heights above the sea level, the water's surface over water, as
//! eight equal-area quantiles `z_q` of the field cells it holds, and `z̄_w`, its flooded cells' mean
//! water-surface height ([`hypsometry`](super::hypsometry)). The prognostic T stays the sea-level
//! temperature, and h and the transport stay at it (decision-r09-t13a-relief §1.1); at each
//! quantile the ground is at `T_g` = T − Γ `z_q`, Γ plan 14's ground gradient α g ÷ `c_p`
//! ([`lapse`](super::lapse)), held at least the night floor ([`Relief`]):
//!
//! - α and its slope are the means over the quantiles of the albedo law at `T_g`;
//! - the outgoing radiation is the mean over the quantiles of
//!   max(σT⁴ ÷ (1 + ¾τ) − e^(−(3/2)`τ_z`) σ(T⁴ − `T_g`⁴), e^(−(3/2)`τ_z`) σ`T_g`⁴), with
//!   `τ_z` = τ e^(−z ÷ 2H), plan 14's τ ∝ √p over the air's scale height H, and its diffusivity
//!   3/2 ([`quantile_emission`]); under the water term `τ_z` = m [`τ_d` e^(−z ÷ 2H) + `τ_w`(`T_g`)],
//!   the dry gases thinning over 2H and the vapour following the ground's own temperature, over
//!   its Clausius–Clapeyron height (R2; [`vapour_quantile_emission`]);
//! - the sea's heat capacity reads its water's surface, T − Γ `z̄_w` (decision-r09-t13ge §1.4, g2);
//! - τ is solved so that the steady state's surface mean, of each point's quantiles' `T_g`, is
//!   plan 14's (decision-r09-t13a-relief §1.3), with a guard where the bisection closes on a fold
//!   ([`OpticalDepth::Folded`]).
//!
//! A point whose heights are all zero takes the arithmetic above, so a world at the sea level is
//! T13.a's to the bit; a point whose heights are all equal is evaluated once.
//!
//! # The scheme
//!
//! Each step is backward Euler, linearised about the step's start: the emission, h and the absorbed
//! sunlight by their slopes in T, the emission's with its water term's, held at least
//! [`EMISSION_SLOPE_FLOOR`] of its slope at a fixed τ. The heat capacity is taken at the step's
//! start, and of the absorbed sunlight's slope at most [`IMPLICIT_ALBEDO_SHARE`] of the point's
//! storage and emission slopes, which keeps the system diagonally dominant; the rest of it is
//! taken at the step's start, as most energy-balance models take the albedo. The change is in
//! flux form, so the scheme conserves energy.
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
//! each from an isothermal start at the mean, under each band's mean insolation over its columns
//! and over each band's quantiles pooled over all of its cells (decision-r09-t13ge §1.4, g5).
//! On the geographic grid at the sea level that is the steady state itself, since only the heat
//! capacity sees the sea and the columns, and a steady state stores no heat; over relief, whose
//! quantiles vary in longitude, it is not quite (R09.T13.d's constant takes up the difference,
//! decision-r09-t13ge §3, G3); on the locked grid it is the steady
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
use super::hypsometry::{Hypsometry, PointHeights, QUANTILES, Shape};
use super::lapse;
use super::ocean::{Currents, OceanGrid, OceanTransport, Returning, SeaLiquid};
use super::partition::{Circulation, Turning};
use super::sun::Sunlight;
use super::transport::{Diffusing, REFERENCE_PRESSURE_PA, rotation_ratio, transport_kg_m2_s};
use crate::math;
use crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY;
use crate::planetary::derive::atmosphere::{
    Gas, GreenhouseConstants, PartialPressures, SnapshotRules,
};
use crate::planetary::hooks::surface::{
    EARTH_AIR_MOLAR_MASS_G_PER_MOL, EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
    FLUX_OPTICAL_DEPTH_PER_GREY, night_floor,
};
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{
    MOLAR_GAS_CONSTANT, SubstanceId, by_key, mean_molar_mass_g_per_mol,
    mixture_heat_capacity_over_r_at, saturation_pressure,
};
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{Kelvin, Pascals, Seconds};

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
pub(super) const ENERGY_SLOPE_STEP_K: f64 = 0.01;

/// The coefficient of the optical depth in the emission factor 1 + ¾τ: ¾, Spiegel, Menou and
/// Scharf's Eddington form σT⁴ ÷ (1 + ¾τ) (2008, Table 1, models 1–2, read, as [`emission`]
/// cites it), by which the relief's transmission recovers τ = (x − 1) ÷ ¾ from the fitted factor
/// x (decision-r09-t13a-relief §1.3).
pub(super) const EDDINGTON_COEFFICIENT: f64 = 0.75;

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

/// The largest departure of the tabulated water term `τ_w` and of the relief's water transmission
/// e^(−(3/2) m `τ_w`) from the direct ones, and of their slopes per kelvin, at which an interval is
/// tabulated under the water term (decision-r09-t13i §2.2): 10⁻¹⁰ and 10⁻⁸ each, as the albedo's
/// (`Adopted`; both are of order one).
const WATER_TABLE_TOLERANCES: [f64; 4] = [1e-10, 1e-8, 1e-10, 1e-8];

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
    /// Where the model of the body of `inputs` forms its ice: about its climate condensable's
    /// freezing point shifted onto water's calibration, or nowhere ([`EnergyBalance::for_inputs`]'s
    /// rule, which it reads; alone, for the frozen ground's test, decision-r09-t13e-revision §4.4).
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs) -> Self {
        match frost::freezing_point_k(inputs) {
            Some(point) => Self::At {
                point_k: WATER_FREEZING_POINT_K + (point - frost::water_freezing_point_k()),
            },
            None => Self::Never {
                albedo: ICE_FREE_ALBEDO,
            },
        }
    }

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

/// The albedo at `t_k`, K, where the ice forms as `freezing` says, and its slope in T: Spiegel et
/// al.'s law about the freezing point ([`ALBEDO_MEAN`]), or the fixed albedo where no ice forms.
#[must_use]
fn albedo_of(freezing: Freezing, t_k: f64) -> AlbedoAt {
    match freezing {
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

/// The grey outgoing radiation at a temperature and its slope there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Emission {
    /// σT⁴ ÷ (1 + ¾τ), W m⁻².
    flux_w_m2: f64,
    /// Its slope in T, W m⁻² K⁻¹, which the step reads: under a water term that thickens with T
    /// (decision-r09-t13i §2.2) the whole slope, held at least [`EMISSION_SLOPE_FLOOR`] of
    /// [`fixed_slope_w_m2_k`](Self::fixed_slope_w_m2_k); under one τ that slope itself.
    slope_w_m2_k: f64,
    /// The slope at a fixed τ, 4σT³ ÷ (1 + ¾τ) at the sea level, which the steady state's
    /// pseudo-capacity reads ([`RELAXATION`]).
    fixed_slope_w_m2_k: f64,
}

/// The share of the fixed-τ slope 4σT³ ÷ (1 + ¾τ) at which the emission's slope in T is held:
/// 0.1, decision-r09-t13i §2.2's guard, which keeps each step diagonally dominant where the water
/// term's thickening with T outruns the column's own rise (∂ln(1 + ¾τ) ÷ ∂T near 4 ÷ T, a
/// runaway). The slope only conditions the step: the outgoing radiation itself is exact, so a
/// converged state is the equation's.
pub(super) const EMISSION_SLOPE_FLOOR: f64 = 0.1;

/// `slope`, a slope of the outgoing radiation in T, held at least [`EMISSION_SLOPE_FLOOR`] of the
/// fixed-τ slope `fixed`.
#[must_use]
fn floored_slope(slope: f64, fixed: f64) -> f64 {
    let floor = EMISSION_SLOPE_FLOOR * fixed;
    if slope > floor { slope } else { floor }
}

/// A steady state's iteration: the points' temperatures and whether it met its tolerance.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Steady {
    /// Each point's temperature, K.
    pub(super) t_k: Vec<f64>,
    /// Whether the iteration met [`STEADY_TOLERANCE_K`] within its count.
    pub(super) converged: bool,
}

/// The optical depth's solve ([`EnergyBalance::fit`]): the emission factor, the annual-mean
/// steady state there, its surface mean and how τ was found.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Fitted {
    /// The emission factor 1 + ¾τ, at plan 14's mean under the water term.
    pub(super) emission_factor: f64,
    /// The radiation at it.
    pub(super) radiation: Radiation,
    /// The annual-mean steady state at it.
    pub(super) steady: Steady,
    /// The steady state's surface mean, K: the solid-angle mean of each point's ground
    /// temperature over its hypsometry ([`Relief::surface_mean_k`]), its mean at sea level where
    /// the model reads no relief.
    pub(super) surface_mean_k: f64,
    /// How τ was found.
    pub(super) optical_depth: OpticalDepth,
}

/// A probe of τ's bisection, and its steady state, which the fold guard keeps where it converged
/// ([`EnergyBalance::fit`]; decision-r09-t13ge §1.4, g4).
#[derive(Debug, Clone, PartialEq)]
struct Probe {
    /// The probe's emission factor.
    emission_factor: f64,
    /// Its radiation.
    radiation: Radiation,
    /// Its steady state's surface mean, K: not a number where the steady state is not finite, a
    /// probe the bisection takes as warm (decision-r09-t13i §2.2).
    surface_mean_k: f64,
    /// Its steady state.
    steady: Steady,
}

/// The fold guard's choice between the last converged probe on the cold side of the bracket,
/// `cold`, and on the warm side, `warm`: the one whose surface mean lies nearer `target_k`, K, ties
/// to the warm side (decision-r09-t13ge §1.4, g4); `None` where neither side kept one.
#[must_use]
fn nearer(cold: Option<Probe>, warm: Option<Probe>, target_k: f64) -> Option<Probe> {
    match (cold, warm) {
        (Some(cold), Some(warm)) => {
            if (warm.surface_mean_k - target_k).abs() <= (cold.surface_mean_k - target_k).abs() {
                Some(warm)
            } else {
                Some(cold)
            }
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
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
    pub(super) fn specific_humidity(&self, t_k: f64) -> f64 {
        let saturation = saturation_pressure(self.condensable, Kelvin::new(t_k))
            .map_or(0.0, crate::units::Pascals::value);
        let wanted = RELATIVE_HUMIDITY * saturation;
        let p = self.surface_pressure_pa;
        let e = if wanted < p { wanted } else { p };
        let epsilon = self.mass_ratio;
        epsilon * e / (p - (1.0 - epsilon) * e)
    }
}

/// One bar, Pa: the unit of the partial pressures plan 14's grey terms take their square roots
/// of (P14.T13.c, `derive::atmosphere`'s τ = Σ k (p ÷ 1 bar)^½).
const BAR_PA: f64 = 1e5;

/// Plan 14's grey greenhouse under the generator version's rules in force, as the energy balance
/// reads its water term (R1, decision-r09-t13i §2.2): the constants, and the share of water's
/// saturation pressure, by weight, that the water term's partial pressure takes, for the air
/// `air`.
///
/// - Under [`SnapshotRules::Held`] (version 22): P14.T13.c's constants
///   ([`GreenhouseConstants::VERSION_22`], water 2.89 per √bar), the water's column weight held at
///   its saturation pressure, a share of 1.
/// - Under [`SnapshotRules::Batch`] (from 23): P14.T57.a's fits ([`GreenhouseConstants::FITTED`],
///   water 4.47 per √bar) with the water held at [`WATER_SURFACE_HUMIDITY`] of its saturation
///   pressure by mole (Manabe and Wetherald's 0.77, P14.T51.a's partition), so
///   [`WATER_SURFACE_HUMIDITY`] × `M_w` ÷ μ of it by weight, μ the air's mean molar mass: 0.496 on
///   an Earth (`GREENHOUSE_WATER`'s documentation).
///
/// P14.T54.a, the 22 → 23 bump, deletes the snapshot lane's hold and its `SnapshotRules`: the
/// batch's arm is then the only one, and this is the one function to change (main's ruling on
/// decision-r09-t13i, 2026-10-20). Whichever of R09.T13.i and P14.T54.a lands second re-measures
/// the climate (decision-r09-t13i §4).
#[must_use]
fn greenhouse_in_force(air: &Air) -> (GreenhouseConstants, f64) {
    match SnapshotRules::in_force() {
        SnapshotRules::Held => (GreenhouseConstants::VERSION_22, 1.0),
        SnapshotRules::Batch => (
            GreenhouseConstants::FITTED,
            WATER_SURFACE_HUMIDITY * Gas::Water.molar_mass_g_per_mol() / air.molar_mass_g_per_mol,
        ),
    }
}

/// Plan 14's water term of the grey optical depth at a temperature (R1, decision-r09-t13i §2.2):
/// `τ_w`(T) = `k_w` √(`p_w`(T) ÷ 1 bar), `k_w` plan 14's water constant, and `p_w` the vapour's
/// partial pressure as its [`Reservoir`] holds it, at most `h_w` `p_sat`(T), `h_w` plan 14's
/// humidity convention under the rules in force ([`greenhouse_in_force`]) and `p_sat` the
/// registry's water curve (its ice branch below the triple point): plan 14's own term, which sets
/// the mean R09.T13.d imposes, taken at the column's own temperature rather than the body's mean,
/// so that it thickens the moist tropics' column and thins the dry poles'.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WaterVapour {
    /// The registry's water row.
    water: SubstanceId,
    /// `k_w`, per √bar.
    constant: f64,
    /// `h_w`, the water's partial pressure by weight over its saturation pressure.
    humidity: f64,
    /// What holds the vapour.
    reservoir: Reservoir,
}

/// What holds the water term's vapour (decision-r09-t13i's §2.4 follow-up 2 §4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Reservoir {
    /// Liquid water at the surface, whatever the climate condensable: the sea holds the vapour at
    /// `h_w` of the local saturation, as the model's moist static energy takes it at a fixed
    /// relative humidity, `p_w` = `h_w` `p_sat`(T).
    Surface,
    /// No liquid water at the surface (a desert, or a world whose water is ice): the air holds the
    /// water it has, plan 14's listed `x_w` `p_s`, and the saturation caps it where the air is
    /// colder than that vapour's dew or frost point, the cold trap: `p_w` = min(`h_w` `p_sat`(T),
    /// `x_w` `p_s`). Uncapped, the term grows with T without limit, and the reference Earth with
    /// no liquid ran away to a 53,093 K mean.
    Air {
        /// `x_w` `p_s`, Pa, the inputs' water mole fraction times their surface pressure, as
        /// `τ_d` takes the other gases' partial pressures.
        inventory_pa: f64,
    },
}

impl WaterVapour {
    /// `τ_w` at `t_k`, K: zero where the registry has no saturation pressure there.
    #[must_use]
    pub(super) fn depth(&self, t_k: f64) -> f64 {
        let saturation =
            saturation_pressure(self.water, Kelvin::new(t_k)).map_or(0.0, Pascals::value);
        let saturated = self.humidity * saturation;
        let pressure = match self.reservoir {
            Reservoir::Surface => saturated,
            // By comparison, so that a NaN stays one.
            Reservoir::Air { inventory_pa } => {
                if saturated > inventory_pa {
                    inventory_pa
                } else {
                    saturated
                }
            }
        };
        // By comparison, so that a NaN gives no term, as plan 14's root of a NaN's max(0) does.
        if pressure > 0.0 {
            self.constant * (pressure / BAR_PA).sqrt()
        } else {
            0.0
        }
    }

    /// `τ_w` at `t_k`, K, and its slope in T by a centred difference of [`ENERGY_SLOPE_STEP_K`], as
    /// h's slope is taken, so that the registry may own the curve's form.
    #[must_use]
    fn depth_and_slope(&self, t_k: f64) -> [f64; 2] {
        [
            self.depth(t_k),
            (self.depth(t_k + ENERGY_SLOPE_STEP_K) - self.depth(t_k - ENERGY_SLOPE_STEP_K))
                / (2.0 * ENERGY_SLOPE_STEP_K),
        ]
    }
}

/// What sets the model's grey optical depth (decision-r09-t13i §2.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Greenhouse {
    /// One τ for every column at every temperature, solved per body as R09.T13.a built it: a world
    /// whose gases hold no water (its τ is plan 14's dry terms', fixed), and Kaspi and Showman's
    /// fit, whose GCM's grey radiation holds its optical depth fixed with no vapour feedback (their
    /// §2, §3.3), so that the transport's tables stay its transport efficiency.
    Grey,
    /// Plan 14's grey law at each column's own temperature (R1 and R2):
    /// τ(T, z) = m [`τ_d` e^(−z ÷ 2H) + `τ_w`(T − Γz)], m one multiplier solved per body.
    Vapour {
        /// `τ_d`, plan 14's terms of the gases other than water at their partial pressures
        /// `x_i p_s`: carbon dioxide's and nitrogen's, nitrogen's collision factor at the body's
        /// equilibrium temperature ([`Greenhouse::of`]); methane and the other condensables enter
        /// no term, as in plan 14's law.
        dry_depth: f64,
        /// `τ_w`.
        water: WaterVapour,
    },
}

impl Greenhouse {
    /// The greenhouse of the body of `inputs` in the air `air`, whose albedo law is `freezing`'s:
    /// [`Greenhouse::Vapour`] where its gases hold water, else [`Greenhouse::Grey`]
    /// (decision-r09-t13i §2.2). Its vapour's [`Reservoir`] is the surface where water is among
    /// its surface liquids ([`CoarseInputs::liquids`]), pure or as an aqueous solution's
    /// component, and otherwise the air's own water, `x_w` `p_s` (decision-r09-t13i's §2.4
    /// follow-up 2 §4). A solution's lowering of the vapour pressure is not taken: `h_w` stands
    /// for it as for the humidity.
    ///
    /// `τ_d` takes plan 14's grey terms ([`GreenhouseConstants::terms`]) of the inputs' gases at
    /// `x_i p_s`, water's left out, and nitrogen's collision factor at the body's equilibrium
    /// temperature, here σ`T_eq`⁴ = (1 − α(T\*)) F ÷ 4 + `F_int`: F the light of its hosts
    /// ([`CoarseInputs::host_flux`]), α the model's albedo law at plan 14's mean T\* and `F_int`
    /// its heat flow, the model's own equilibrium with the sunlight at its mean (the lane's choice;
    /// the ruling names the body's equilibrium temperature, which the inputs do not carry). On an
    /// Earth that is 256.4 K against plan 14's 255.1 K at its Bond albedo 0.294, which moves
    /// nitrogen's term by 1%.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, air: &Air, freezing: Freezing) -> Self {
        let water = Gas::Water.substance();
        let fraction = |gas: Gas| {
            inputs
                .gases()
                .iter()
                .filter(|share| by_key(share.substance.as_str()) == Some(gas.substance()))
                .fold(0.0, |sum, share| sum + share.mole_fraction)
        };
        let water_fraction = fraction(Gas::Water);
        if !positive(water_fraction) {
            return Self::Grey;
        }
        let (constants, humidity) = greenhouse_in_force(air);
        let pressure = inputs.surface_pressure().value();
        // Water is among a surface liquid's components where the liquid is water or an aqueous
        // solution, a brine's water as much a reservoir as a fresh sea's.
        let holds_water = |key: &str| {
            by_key(key)
                .and_then(|liquid| liquid.substance().liquid())
                .is_some_and(|columns| columns.components().contains(&water))
        };
        let reservoir = if inputs
            .liquids()
            .iter()
            .any(|liquid| holds_water(liquid.substance.as_str()))
        {
            Reservoir::Surface
        } else {
            Reservoir::Air {
                inventory_pa: water_fraction * pressure,
            }
        };
        let mean_k = inputs.mean_surface_temperature().value();
        let absorbed = (1.0 - albedo_of(freezing, mean_k).albedo) * inputs.host_flux().value()
            / 4.0
            + inputs.heat_flow().value();
        let equilibrium = Kelvin::new((absorbed.max(0.0) / STEFAN_BOLTZMANN).sqrt().sqrt());
        let dry = PartialPressures::from_fn(|gas| match gas {
            Gas::Water => Pascals::ZERO,
            _ => Pascals::new(fraction(gas) * pressure),
        });
        let dry_depth = constants
            .terms(&dry, equilibrium)
            .iter()
            .fold(0.0, |sum, &(_, term)| sum + term);
        let unit = PartialPressures::from_fn(|gas| match gas {
            Gas::Water => Pascals::new(BAR_PA),
            _ => Pascals::ZERO,
        });
        let constant = constants
            .terms(&unit, equilibrium)
            .iter()
            .find(|(gas, _)| *gas == Gas::Water)
            .map_or(0.0, |&(_, term)| term);
        Self::Vapour {
            dry_depth,
            water: WaterVapour {
                water,
                constant,
                humidity,
                reservoir,
            },
        }
    }
}

/// One run's radiation: its greenhouse at one emission factor (decision-r09-t13i §2.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Radiation {
    /// One emission factor 1 + ¾τ at every column and temperature.
    Grey {
        /// 1 + ¾τ.
        emission_factor: f64,
    },
    /// τ(T, z) = m [`τ_d` e^(−z ÷ 2H) + `τ_w`(T − Γz)].
    Vapour {
        /// m.
        multiplier: f64,
        /// `τ_d`.
        dry_depth: f64,
        /// `τ_w`.
        water: WaterVapour,
    },
}

impl Radiation {
    /// The radiation of `greenhouse` whose emission factor at the temperature `reference_k`, K,
    /// is `emission_factor`: that factor itself under one τ; under the water term,
    /// m = (x − 1) ÷ (¾ (`τ_d` + `τ_w`(T\*))), or one τ where the gases' terms vanish there.
    #[must_use]
    pub(super) fn at(greenhouse: Greenhouse, emission_factor: f64, reference_k: f64) -> Self {
        match greenhouse {
            Greenhouse::Grey => Self::Grey { emission_factor },
            Greenhouse::Vapour { dry_depth, water } => {
                let depth = dry_depth + water.depth(reference_k);
                if positive(depth) {
                    Self::Vapour {
                        multiplier: (emission_factor - 1.0) / (EDDINGTON_COEFFICIENT * depth),
                        dry_depth,
                        water,
                    }
                } else {
                    Self::Grey { emission_factor }
                }
            }
        }
    }

    /// The column's optical depth above the sea level that thins with height alone, the τ of the
    /// relief's dry transmission e^(−(3/2) τ e^(−z ÷ 2H)) ([`Relief::transmissions_through`]):
    /// (x − 1) ÷ ¾ under one τ, as R09.T13.g took it, and m `τ_d` under the water term, whose
    /// own part follows the ground's temperature instead (R2).
    #[must_use]
    fn thinning_depth(&self) -> f64 {
        match *self {
            Self::Grey { emission_factor } => (emission_factor - 1.0) / EDDINGTON_COEFFICIENT,
            Self::Vapour {
                multiplier,
                dry_depth,
                ..
            } => multiplier * dry_depth,
        }
    }
}

/// How the optical depth was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OpticalDepth {
    /// By bisection, so that the steady state's surface mean is the inputs'. Its steady state
    /// converged, but where the final probe's did not and the fold guard kept no converged probe
    /// on either side of the bracket: then the state is the final probe's, unconverged, as before
    /// the guard ([`Solution::steady_converged`] says which).
    Solved,
    /// By bisection, but the final probe's steady state did not converge within
    /// [`STEADY_ITERATIONS`]: the bisection closed on a fold, a jump of the steady states' mean
    /// between two branches (the relief Earth's small Arctic cap), where the iteration crawls. The
    /// fold guard took the last converged probe on either side of the bracket whose surface mean
    /// lies nearer the inputs', ties to the warm side (decision-r09-t13ge §1.4, g4); R09.T13.d's
    /// constant takes up the rest.
    Folded,
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
    /// The emission factor 1 + ¾τ, at plan 14's mean under the water term.
    pub(super) emission_factor: f64,
    /// The radiation the orbits ran under.
    pub(super) radiation: Radiation,
    /// How τ was found.
    pub(super) optical_depth: OpticalDepth,
    /// Whether the steady state τ was solved on met [`STEADY_TOLERANCE_K`]: true for a body
    /// with neither light nor heat, which has none. It is false with [`OpticalDepth::Solved`]
    /// where the bisection's final probe did not converge and the fold guard kept no converged
    /// probe on either side: the state is then the final probe's, as built before the guard.
    pub(super) steady_converged: bool,
    /// That steady state's surface mean, K ([`Fitted::surface_mean_k`]): the inputs' mean for a
    /// body with neither light nor heat.
    pub(super) steady_surface_mean_k: f64,
    /// Each month's mean insolation at each point of the grid, W m⁻²
    /// ([`Sunlight::month_means`]): over the steps each month's record averages, one month in a
    /// one-month year. The precipitation heuristic's absorbed sunlight reads it (R09.T13.e).
    pub(super) month_insolation_w_m2: Vec<Vec<f64>>,
    /// The sea's heat transport over the last orbit, or at the steady state in a one-month year:
    /// each month's ocean heat convergence at each point and the transports at each edge
    /// (decision-r09-t13i §2.3.3); `None` with no sea's transport, or none to carry.
    pub(super) ocean: Option<OceanRecord>,
}

/// The ground a run of the model reads (R09.T13.g).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Ground<'a> {
    /// Every point at the sea level, as the model was before R09.T13.g: no hypsometry is read.
    /// The step always reads the field's relief, at the sea level wherever its heights are zero;
    /// the tests compare it with this.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the model before R09.T13.g, which the tests hold the step's zero relief to"
        )
    )]
    SeaLevel,
    /// The relief of the run's grid, and of the zonal grid in its coordinates, whose bands pool
    /// all of their cells (decision-r09-t13ge §1.4, g5), on which τ is solved.
    Relief {
        /// The run's grid's relief.
        grid: &'a Relief,
        /// The zonal grid's relief.
        zonal: &'a Relief,
    },
}

/// The ground one solve reads on one grid (R09.T13.g), `R` the grid's relief ([`Relief`]) or that
/// relief at one emission factor ([`ReliefAt`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum GridGround<R> {
    /// Every point at the sea level: T13.a's arithmetic, no hypsometry read.
    SeaLevel,
    /// The grid's relief.
    Relief(R),
}

impl<'a> GridGround<&'a Relief> {
    /// Each point-quantile's transmission through the part of the column above it that thins with
    /// height alone under `radiation` ([`Relief::transmissions_through`] at
    /// [`Radiation::thinning_depth`]): the whole column's under one τ, the dry gases' under the
    /// water term; none at the sea level.
    #[must_use]
    fn transmissions(self, radiation: &Radiation) -> Vec<[f64; QUANTILES]> {
        match self {
            Self::SeaLevel => Vec::new(),
            Self::Relief(relief) => relief.transmissions_through(radiation.thinning_depth()),
        }
    }

    /// The ground at one emission factor, whose transmissions are `transmission`
    /// ([`transmissions`](Self::transmissions) at that factor).
    #[must_use]
    fn at<'b>(self, transmission: &'b [[f64; QUANTILES]]) -> GridGround<ReliefAt<'b>>
    where
        'a: 'b,
    {
        match self {
            Self::SeaLevel => GridGround::SeaLevel,
            Self::Relief(relief) => GridGround::Relief(ReliefAt {
                relief,
                transmission,
            }),
        }
    }
}

/// The relief the energy-balance model reads on one grid (R09.T13.g; decision-r09-t13a-relief
/// §1, decision-r09-t13ge §1.4): its points' hypsometry, the body's lapse rate Γ and its air's
/// scale height H, and the ground's floor.
///
/// The prognostic temperature T stays the sea-level one, and h and the transport stay at it; at
/// each quantile height z the ground is at `T_g` = T − Γz, held at least the floor. The albedo and
/// its slope are the means over the quantiles of the albedo law at `T_g`; the outgoing radiation
/// is the sea-level column's less the change at the raised ground transmitted through the column
/// above it, never below the ground's emission transmitted ([`quantile_emission`]); the sea's
/// heat capacity reads the water's own surface, T − Γ `z̄_w`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Relief {
    /// Γ, K m⁻¹.
    lapse_rate_k_per_m: f64,
    /// The floor of the ground's temperature, K.
    floor_k: f64,
    /// The points' heights.
    hypsometry: Hypsometry,
    /// Each point-quantile's share of the column's optical depth above it, e^(−z ÷ 2H).
    thinning: Vec<[f64; QUANTILES]>,
}

impl Relief {
    /// The relief of `hypsometry` under the lapse rate `lapse_rate_k_per_m`, K m⁻¹, and the scale
    /// height `scale_height_m`, m, the ground held at least `floor_k`, K. A scale height that is
    /// not positive or not finite thins nothing.
    #[must_use]
    pub(super) fn new(
        lapse_rate_k_per_m: f64,
        scale_height_m: f64,
        floor_k: f64,
        hypsometry: Hypsometry,
    ) -> Self {
        let thinning = hypsometry
            .all()
            .iter()
            .map(|point| {
                point.quantiles_m.map(|z| {
                    if positive(scale_height_m) {
                        math::exp(-z / (2.0 * scale_height_m))
                    } else {
                        1.0
                    }
                })
            })
            .collect();
        Self {
            lapse_rate_k_per_m,
            floor_k,
            hypsometry,
            thinning,
        }
    }

    /// The relief of `hypsometry` on the body of `inputs`: plan 14's ground gradient
    /// ([`lapse::lapse_rate_k_per_m`], the one Γ T13.d writes to the header), the air's scale height
    /// at the mean surface temperature ([`scale_height_m`]) and the night floor (`night_floor`,
    /// decision-r09-t13a-relief §1.2's guard).
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, hypsometry: Hypsometry) -> Self {
        Self::new(
            lapse::lapse_rate_k_per_m(inputs),
            scale_height_m(inputs),
            night_floor(inputs.heat_flow()).value(),
            hypsometry,
        )
    }

    /// The points' heights.
    #[must_use]
    pub(super) fn hypsometry(&self) -> &Hypsometry {
        &self.hypsometry
    }

    /// The ground's temperature `z_m` above the sea level under the sea-level temperature `t_k`,
    /// K, and its slope in T: T − Γz and 1, or the floor and 0 where T − Γz lies below it.
    #[must_use]
    fn ground_k(&self, t_k: f64, z_m: f64) -> (f64, f64) {
        let ground = t_k - self.lapse_rate_k_per_m * z_m;
        if ground < self.floor_k {
            (self.floor_k, 0.0)
        } else {
            (ground, 1.0)
        }
    }

    /// The temperature of the water's surface of a point of heights `heights` under the sea-level
    /// temperature `t_k`, K, which the sea's ice heat capacity reads: T − Γ `z̄_w`, T itself over
    /// R09.T12.e's ocean (decision-r09-t13ge §1.4, g2).
    #[must_use]
    fn water_k(&self, t_k: f64, heights: &PointHeights) -> f64 {
        t_k - self.lapse_rate_k_per_m * heights.water_m
    }

    /// Point `point`'s surface temperature at the sea-level temperature `t_k`, K: the mean over its
    /// quantiles of the ground's temperature, held at least the floor; `t_k` itself at the sea
    /// level, so that a world at the sea level keeps its mean to the bit.
    #[must_use]
    pub(super) fn surface_k(&self, point: usize, t_k: f64) -> f64 {
        let heights = self.hypsometry.point(point);
        match heights.shape {
            Shape::SeaLevel => t_k,
            Shape::Level => self.ground_k(t_k, heights.quantiles_m[0]).0,
            Shape::Varied => {
                heights
                    .quantiles_m
                    .iter()
                    .fold(0.0, |sum, &z| sum + self.ground_k(t_k, z).0)
                    * QUANTILE_WEIGHT
            }
        }
    }

    /// The surface mean of the sea-level temperatures `t_k`, K, one per point of `grid`: the
    /// solid-angle mean over the grid of each point's surface temperature
    /// ([`surface_k`](Self::surface_k)), [`ModelGrid::mean`]'s, which it is to the bit at the sea
    /// level (decision-r09-t13a-relief §1.3).
    #[must_use]
    pub(super) fn surface_mean_k(&self, grid: &ModelGrid, t_k: &[f64]) -> f64 {
        let surface: Vec<f64> = t_k
            .iter()
            .enumerate()
            .map(|(point, &t)| self.surface_k(point, t))
            .collect();
        grid.mean(&surface)
    }

    /// Each point-quantile's transmission through the column above it whose optical depth at the
    /// sea level is `tau` and thins with height as e^(−z ÷ 2H): e^(−(3/2) `τ_z`) with
    /// `τ_z` = τ e^(−z ÷ 2H) (plan 14's τ ∝ √p, `optical_depth`, and its diffusivity
    /// [`FLUX_OPTICAL_DEPTH_PER_GREY`]), each through [`math::exp`]. Under one τ, τ is the model's
    /// own, (x − 1) ÷ ¾, as R09.T13.g built it; under the water term the dry gases' m `τ_d`
    /// ([`Radiation::thinning_depth`]).
    #[must_use]
    fn transmissions_through(&self, tau: f64) -> Vec<[f64; QUANTILES]> {
        self.thinning
            .iter()
            .map(|point| point.map(|share| math::exp(-FLUX_OPTICAL_DEPTH_PER_GREY * tau * share)))
            .collect()
    }

    /// How far a seasonal run's table must reach below and above its starting temperatures for
    /// every quantile's ground, K: Γ times the highest quantile above the sea level, and times the
    /// deepest below it, each at least zero.
    #[must_use]
    fn table_reach_k(&self) -> (f64, f64) {
        let (deepest, highest) = self.hypsometry.extremes_m();
        let reach = |z: f64| {
            if z > 0.0 {
                self.lapse_rate_k_per_m * z
            } else {
                0.0
            }
        };
        (reach(highest), reach(-deepest))
    }
}

/// The weight of each quantile in a point's mean over its hypsometry: 1 ÷ [`QUANTILES`], exact.
const QUANTILE_WEIGHT: f64 = 1.0 / 8.0;

// The weight is the quantiles' count's reciprocal.
const _: () = assert!(QUANTILES == 8, "QUANTILE_WEIGHT is 1 ÷ QUANTILES");

/// The scale height of the air of `inputs`, m: H = R `T_s` ÷ (μ g), at plan 14's mean surface
/// temperature `T_s` and the air's mean molar mass μ ([`Air::of`]), on which plan 14's grey τ,
/// whose every term goes as the square root of its partial pressure, thins as e^(−z ÷ 2H) above
/// raised ground (decision-r09-t13a-relief §1.3: 8.42 km on an Earth; the reference Earth's moist
/// air, 8.47 km).
#[must_use]
pub(super) fn scale_height_m(inputs: &CoarseInputs) -> f64 {
    MOLAR_GAS_CONSTANT * inputs.mean_surface_temperature().value()
        / (Air::of(inputs).molar_mass_g_per_mol * 1e-3 * inputs.gravity().value())
}

/// A grid's relief at one emission factor, what a step reads: the relief, and each
/// point-quantile's transmission at that factor ([`Relief::transmissions`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReliefAt<'a> {
    relief: &'a Relief,
    transmission: &'a [[f64; QUANTILES]],
}

impl<'a> ReliefAt<'a> {
    /// Point `point`'s heights and transmissions, or `None` at the sea level, where the model
    /// takes T13.a's arithmetic.
    #[must_use]
    fn raised(self, point: usize) -> Option<(&'a PointHeights, &'a [f64; QUANTILES])> {
        let heights = self.relief.hypsometry.point(point);
        (heights.shape != Shape::SeaLevel).then(|| (heights, &self.transmission[point]))
    }
}

impl<'a> GridGround<ReliefAt<'a>> {
    /// Point `point`'s relief, heights and transmissions ([`ReliefAt::raised`]), or `None` where
    /// the ground or the point is at the sea level, where the model takes T13.a's arithmetic.
    #[must_use]
    fn raised(
        self,
        point: usize,
    ) -> Option<(&'a Relief, (&'a PointHeights, &'a [f64; QUANTILES]))> {
        match self {
            Self::SeaLevel => None,
            Self::Relief(at) => Some((at.relief, at.raised(point)?)),
        }
    }
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
    /// What sets the grey optical depth (decision-r09-t13i §2.2).
    pub(super) greenhouse: Greenhouse,
    /// `r_l`, the sea's heat capacities' factor on water's, its liquid's `ρ_l c_l` ÷ (`ρ_w c_w`)
    /// ([`SeaLiquid::of`]; decision-r09-t13i §2.1): exactly 1 for water.
    pub(super) sea_capacity_ratio: f64,
}

/// The model's working arrays: h, its slope, the albedo, its slope, the step's right side, its
/// restoring D, the latitude
/// systems' diagonal and the solution at each grid point; the latitude systems' off-diagonals,
/// the same in every column, and one column's system, at each band; and one band's periodic
/// system in longitude, at each column. With the sea's transport (decision-r09-t13i §2.3), the
/// latitude systems' off-diagonals at each point, each point's water-surface temperature, the
/// bands' mean h and sea temperature, the Ekman cells' gain at each edge, and the atmosphere's,
/// the Ekman cells' and the gyres' northward flux at each point's southern edge.
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
    point_lower: Vec<f64>,
    point_upper: Vec<f64>,
    water: Vec<f64>,
    band_energy: Vec<f64>,
    band_sea: Vec<f64>,
    gains: Vec<f64>,
    south_atmosphere: Vec<f64>,
    south_ekman: Vec<f64>,
    south_gyres: Vec<f64>,
    column_lower: Vec<f64>,
    column_upper: Vec<f64>,
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
            point_lower: points.clone(),
            point_upper: points.clone(),
            water: points.clone(),
            south_atmosphere: points.clone(),
            south_ekman: points.clone(),
            south_gyres: points.clone(),
            solution: points,
            lower: bands.clone(),
            upper: bands.clone(),
            band_energy: bands.clone(),
            band_sea: bands.clone(),
            gains: vec![0.0; LATITUDE_BANDS + 1],
            column_lower: bands.clone(),
            column_upper: bands.clone(),
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
///
/// Under the water term (decision-r09-t13i §2.2) it also holds `τ_w` and its slope, and the relief's
/// water transmission e^(−(3/2) m `τ_w`) and its slope at the run's multiplier m, which each raised
/// quantile reads at its ground's temperature.
#[derive(Debug, Clone, PartialEq)]
struct Tabulated {
    first_node: f64,
    latent: Option<Vec<[f64; 2]>>,
    albedo: Vec<[f64; 2]>,
    /// `τ_w` and its slope, under the water term.
    water: Option<Vec<[f64; 2]>>,
    /// The multiplier m, and e^(−(3/2) m `τ_w`) and its slope at it, under the water term.
    wet: Option<(f64, Vec<[f64; 2]>)>,
    smooth: Vec<bool>,
}

impl Tabulated {
    /// The table of `model`'s functions under `radiation` over the nodes from `low_k` −
    /// [`TABLE_MARGIN_K`] (but not below 1 K) to `high_k` + [`TABLE_MARGIN_K`].
    #[must_use]
    fn new(model: &EnergyBalance, radiation: &Radiation, low_k: f64, high_k: f64) -> Self {
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
        let vapour = match *radiation {
            Radiation::Vapour {
                multiplier, water, ..
            } => Some((multiplier, water)),
            Radiation::Grey { .. } => None,
        };
        let water: Option<Vec<[f64; 2]>> = vapour.map(|(_, water)| {
            temperatures
                .iter()
                .map(|&t| water.depth_and_slope(t))
                .collect()
        });
        let wet = vapour.zip(water.as_ref()).map(|((multiplier, _), water)| {
            (
                multiplier,
                water.iter().map(|&w| wet_of(multiplier, w)).collect(),
            )
        });
        let mut table = Self {
            first_node,
            latent,
            albedo,
            water,
            wet,
            smooth: vec![true; count - 1],
        };
        let smooth = (0..count - 1)
            .map(|i| {
                [0.25, 0.5, 0.75].into_iter().all(|f| {
                    let t = temperatures[i] + f * TABLE_SPACING_K;
                    let (read, direct) = (table.read(model, i, f), model.functions(t, None));
                    let scale = [direct[0].abs(), direct[1].abs(), 1.0, 1.0];
                    let functions = (0..4)
                        .all(|k| (read[k] - direct[k]).abs() <= TABLE_TOLERANCES[k] * scale[k]);
                    let vapour = vapour.is_none_or(|(multiplier, water)| {
                        let direct_water = water.depth_and_slope(t);
                        let direct_wet = wet_of(multiplier, direct_water);
                        let read = table.read_water(i, f);
                        let direct = [
                            direct_water[0],
                            direct_water[1],
                            direct_wet[0],
                            direct_wet[1],
                        ];
                        read.is_some_and(|read| {
                            (0..4).all(|k| (read[k] - direct[k]).abs() <= WATER_TABLE_TOLERANCES[k])
                        })
                    });
                    functions && vapour
                })
            })
            .collect();
        table.smooth = smooth;
        table
    }

    /// `τ_w`, its slope, the water transmission and its slope read from the table in interval `i` at
    /// fraction `f`, or `None` where the table holds no water term.
    #[must_use]
    fn read_water(&self, i: usize, f: f64) -> Option<[f64; 4]> {
        let (water, (_, wet)) = (self.water.as_ref()?, self.wet.as_ref()?);
        let (depth, slope) = hermite(water, i, f);
        let (transmission, transmission_slope) = hermite(wet, i, f);
        Some([depth, slope, transmission, transmission_slope])
    }

    /// `τ_w` and its slope at `t_k`, K, from the table where it holds `t_k` and a water term.
    #[must_use]
    fn water_at(&self, t_k: f64) -> Option<[f64; 2]> {
        let water = self.water.as_ref()?;
        let (i, f) = self.locate(t_k)?;
        let (depth, slope) = hermite(water, i, f);
        Some([depth, slope])
    }

    /// The water transmission e^(−(3/2) m `τ_w`) and its slope at `t_k`, K, from the table where it
    /// holds `t_k` and the water term at the multiplier `multiplier`, to the bit.
    #[must_use]
    fn wet_at(&self, multiplier: f64, t_k: f64) -> Option<[f64; 2]> {
        let (held, wet) = self.wet.as_ref()?;
        if held.total_cmp(&multiplier).is_ne() {
            return None;
        }
        let (i, f) = self.locate(t_k)?;
        let (transmission, slope) = hermite(wet, i, f);
        Some([transmission, slope])
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

/// The water's share of a raised quantile's transmission, e^(−(3/2) m `τ_w`), and its slope in T,
/// −(3/2) m `τ_w′` e^(−(3/2) m `τ_w`), at the multiplier `multiplier` from `τ_w` and its slope `water`
/// (decision-r09-t13i §2.2, R2: the vapour above the ground follows the ground's temperature).
#[must_use]
fn wet_of(multiplier: f64, water: [f64; 2]) -> [f64; 2] {
    let [depth, slope] = water;
    let transmission = math::exp(-FLUX_OPTICAL_DEPTH_PER_GREY * multiplier * depth);
    [
        transmission,
        -FLUX_OPTICAL_DEPTH_PER_GREY * multiplier * slope * transmission,
    ]
}

/// `τ_w` of `water` and its slope at `t_k`, K, from `table` where it holds `t_k`, and directly
/// otherwise.
#[must_use]
fn water_terms(water: &WaterVapour, t_k: f64, table: Option<&Tabulated>) -> [f64; 2] {
    table
        .and_then(|table| table.water_at(t_k))
        .unwrap_or_else(|| water.depth_and_slope(t_k))
}

/// The water transmission of `water` at the multiplier `multiplier` and its slope at the ground's
/// temperature `ground_k`, K ([`wet_of`]), from `table` where it holds `ground_k` at that
/// multiplier, and directly otherwise.
#[must_use]
fn wet_terms(
    water: &WaterVapour,
    multiplier: f64,
    ground_k: f64,
    table: Option<&Tabulated>,
) -> [f64; 2] {
    table
        .and_then(|table| table.wet_at(multiplier, ground_k))
        .unwrap_or_else(|| wet_of(multiplier, water.depth_and_slope(ground_k)))
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
        let freezing = Freezing::of(inputs);
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
            greenhouse: Greenhouse::of(inputs, &air, freezing),
            sea_capacity_ratio: SeaLiquid::of(inputs, freezing).capacity_ratio(),
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
    #[must_use]
    pub(super) fn albedo_at(&self, t_k: f64) -> f64 {
        self.albedo_and_slope(t_k).albedo
    }

    /// The albedo at `t_k`, K, and its slope in T ([`albedo_of`] under the model's freezing).
    #[must_use]
    fn albedo_and_slope(&self, t_k: f64) -> AlbedoAt {
        albedo_of(self.freezing, t_k)
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

    /// h at `t_k`, K, and its slope, from `table` where it holds `t_k` and directly otherwise:
    /// [`functions`](Self::functions)' first two, to the bit.
    #[must_use]
    fn energies(&self, t_k: f64, table: Option<&Tabulated>) -> [f64; 2] {
        if let Some((table, (i, f))) = table.and_then(|table| Some((table, table.locate(t_k)?))) {
            let t = (table.first_node + super::grid::index(i) + f) * TABLE_SPACING_K;
            let sensible = self.specific_heat_j_per_kg_k * t;
            return match &table.latent {
                Some(latent) => {
                    let (value, slope) = hermite(latent, i, f);
                    [sensible + value, self.specific_heat_j_per_kg_k + slope]
                }
                None => [sensible, self.specific_heat_j_per_kg_k],
            };
        }
        [self.energy(t_k), self.energy_slope(t_k)]
    }

    /// The albedo at the ground's temperature `ground_k`, K, and its slope, from `table` where it
    /// holds `ground_k` and directly otherwise (the nodes lie on one lattice, so the value does
    /// not depend on the table's span).
    #[must_use]
    fn ground_albedo(&self, ground_k: f64, table: Option<&Tabulated>) -> AlbedoAt {
        match table.and_then(|table| Some((table, table.locate(ground_k)?))) {
            Some((table, (i, f))) => {
                let (albedo, slope_per_k) = hermite(&table.albedo, i, f);
                AlbedoAt {
                    albedo,
                    slope_per_k,
                }
            }
            None => self.albedo_and_slope(ground_k),
        }
    }

    /// The albedo of a raised point of heights `heights` under `relief` at the sea-level
    /// temperature `t_k`, K, and its slope in T: the mean over its quantiles of the albedo law at
    /// each ground's temperature, and of its slope times the ground's in T (zero where the floor
    /// holds it), once at a level point (decision-r09-t13a-relief §1.2).
    #[must_use]
    fn point_albedo(
        &self,
        t_k: f64,
        heights: &PointHeights,
        relief: &Relief,
        table: Option<&Tabulated>,
    ) -> AlbedoAt {
        let at = |z: f64| {
            let (ground, rate) = relief.ground_k(t_k, z);
            let albedo = self.ground_albedo(ground, table);
            (albedo.albedo, albedo.slope_per_k * rate)
        };
        match heights.shape {
            Shape::SeaLevel | Shape::Level => {
                let (albedo, slope_per_k) = at(heights.quantiles_m[0]);
                AlbedoAt {
                    albedo,
                    slope_per_k,
                }
            }
            Shape::Varied => {
                let (albedo, slope) =
                    heights
                        .quantiles_m
                        .iter()
                        .fold((0.0, 0.0), |(albedo, slope), &z| {
                            let (a, s) = at(z);
                            (albedo + a, slope + s)
                        });
                AlbedoAt {
                    albedo: albedo * QUANTILE_WEIGHT,
                    slope_per_k: slope * QUANTILE_WEIGHT,
                }
            }
        }
    }

    /// The heat capacity, J m⁻² K⁻¹, of a grid point of sea share `sea_share` at `t_k`, K:
    /// (1 − s) `C_l` + s `C_sea`(T), the sea's open water's at every temperature where no ice
    /// forms. Over relief the sea's T is its water surface's, T − Γ `z̄_w` (decision-r09-t13ge
    /// §1.4, g2), which the caller gives; land's capacity reads no temperature. The sea's three
    /// capacities are its liquid's: water's times `r_l` ([`sea_capacity_ratio`](Self::sea_capacity_ratio);
    /// decision-r09-t13i §2.1).
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
        } * self.sea_capacity_ratio;
        LAND_HEAT_CAPACITY_J_PER_M2_K * ((1.0 - sea_share) + sea_share * sea_ratio)
    }

    /// The net heating of every grid point at the sea level at temperatures `t_k`, K, under
    /// insolation `insolation_w_m2` and the radiation `radiation`, W m⁻²: absorbed, internal and
    /// emitted, then the diffusion's convergence, each per point.
    #[cfg(test)]
    #[must_use]
    pub(super) fn heating(
        &self,
        grid: &ModelGrid,
        radiation: &Radiation,
        insolation_w_m2: &[f64],
        t_k: &[f64],
    ) -> (Vec<f64>, Vec<f64>) {
        let energy: Vec<f64> = t_k.iter().map(|&v| self.energy(v)).collect();
        let radiative = (0..grid.points())
            .map(|p| {
                insolation_w_m2[p] * (1.0 - self.albedo_at(t_k[p])) + self.heat_flow_w_m2
                    - sea_level_emission(radiation, t_k[p], None).flux_w_m2
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
    /// in `storage`, over the ground `ground` at that factor, split into the latitude solve and
    /// then the longitude one (the [module documentation](self)); the largest change, K.
    #[expect(
        clippy::too_many_arguments,
        reason = "a step reads the grid, the forcing, the storage, the table, the relief and the \
                  state, and writes the work arrays; bundling them would only rename them"
    )]
    fn advance(
        &self,
        grid: &ModelGrid,
        radiation: &Radiation,
        insolation_w_m2: &[f64],
        storage: Storage<'_>,
        table: Option<&Tabulated>,
        ground: GridGround<ReliefAt<'_>>,
        ocean: Option<(&OceanGrid, Returning)>,
        t_k: &mut [f64],
        work: &mut Work,
    ) -> f64 {
        let lambda = self.transport_kg_m2_s;
        let columns = grid.columns();
        let raised = |p: usize| ground.raised(p);
        for (p, &v) in t_k.iter().enumerate() {
            let [energy, slope, albedo, albedo_slope] = match raised(p) {
                // At the sea level, T13.a's arithmetic to the bit.
                None => self.functions(v, table),
                Some((relief, (heights, _))) => {
                    let [energy, slope] = self.energies(v, table);
                    let ground = self.point_albedo(v, heights, relief, table);
                    [energy, slope, ground.albedo, ground.slope_per_k]
                }
            };
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
                let (
                    Emission {
                        flux_w_m2: emitted,
                        slope_w_m2_k: emission_slope,
                        fixed_slope_w_m2_k: fixed_slope,
                    },
                    sea_k,
                ) = emission_and_sea(radiation, t_k[p], raised(p), table);
                // The pseudo-capacity reads the slope at a fixed τ, so that the relaxation stays
                // conditioned as R09.T13.a built it where the water term weakens the restoring
                // (decision-r09-t13i §2.2); under one τ the two are the same.
                let stored = match storage {
                    Storage::Relaxation => RELAXATION * fixed_slope,
                    Storage::Seasonal { step_s, sea_share } => {
                        self.heat_capacity(sea_k, sea_share[p]) / step_s
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
        // The sea's terms (decision-r09-t13i §2.3), on the latitude systems of the points whose
        // edges carry them; elsewhere the systems are as built.
        let coupled = match ocean {
            Some((ocean, returning)) if ocean.any() => {
                self.couple_ocean(grid, ocean, returning, t_k, ground, work);
                true
            }
            Some(_) | None => false,
        };
        solve_latitudes(grid, coupled, work);
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

    /// The sea's terms of a step at the points' starting temperatures `t_k`, K, over the ground
    /// `ground`, on the latitude systems in `work` (whose h, slopes, right sides and diagonals the
    /// step has filled), under `ocean` with the return temperatures `returning` (decision-r09-t13i
    /// §2.3): each point's off-diagonals the band's, then at a point whose edges carry a term the
    /// Ekman cells' conductance Λ A γ on its h and the gyres' K on its water-surface temperature,
    /// implicit in the step's change y = h′ δT (δT = y ÷ h′ at each point), with their fluxes at
    /// the step's start on its right side. Each point's northward fluxes at its southern edge, the
    /// atmosphere's, the Ekman cells' and the gyres', go to `work` for the record.
    fn couple_ocean(
        &self,
        grid: &ModelGrid,
        ocean: &OceanGrid,
        returning: Returning,
        t_k: &[f64],
        ground: GridGround<ReliefAt<'_>>,
        work: &mut Work,
    ) {
        let lambda = self.transport_kg_m2_s;
        let columns = grid.columns();
        for (p, &t) in t_k.iter().enumerate() {
            work.water[p] = match ground.raised(p) {
                None => t,
                Some((relief, (heights, _))) => relief.water_k(t, heights),
            };
        }
        let bands = grid.band_means(&work.energy);
        work.band_energy.copy_from_slice(&bands);
        ocean.band_sea_k(grid, &work.water, &mut work.band_sea);
        ocean.ekman_gains(
            grid,
            lambda,
            &work.band_energy,
            &work.band_sea,
            returning,
            &mut work.gains,
        );
        for k in 0..LATITUDE_BANDS {
            let (below, above, w) = (
                grid.edge_conductance(k),
                grid.edge_conductance(k + 1),
                grid.weight(k),
            );
            for column in 0..columns {
                let p = grid.point(k, column);
                work.point_lower[p] = work.lower[k];
                work.point_upper[p] = work.upper[k];
                let south = (k > 0).then(|| p - columns);
                let north = (k + 1 < LATITUDE_BANDS).then_some(p + columns);
                let gamma_below = ocean.edge_share(p) * work.gains[k];
                let gamma_above = north.map_or(0.0, |q| ocean.edge_share(q) * work.gains[k + 1]);
                let gyres_below = ocean.diffusive(p);
                let gyres_above = north.map_or(0.0, |q| ocean.diffusive(q));
                let atmosphere =
                    south.map_or(0.0, |q| -lambda * below * (work.energy[p] - work.energy[q]));
                work.south_atmosphere[p] = atmosphere;
                work.south_ekman[p] = gamma_below * atmosphere;
                work.south_gyres[p] =
                    south.map_or(0.0, |q| -gyres_below * (work.water[p] - work.water[q]));
                // By comparison, so that a point whose edges carry nothing is stepped as built.
                if !(gamma_below > 0.0
                    || gamma_above > 0.0
                    || gyres_below > 0.0
                    || gyres_above > 0.0)
                {
                    continue;
                }
                let ekman_below = lambda * below * gamma_below;
                let ekman_above = lambda * above * gamma_above;
                let (energy, slope, water) = (work.energy[p], work.slope[p], work.water[p]);
                let mut flux = 0.0;
                let mut lower = ekman_below;
                let mut upper = ekman_above;
                if let Some(q) = north {
                    flux += ekman_above * (work.energy[q] - energy)
                        + gyres_above * (work.water[q] - water);
                    upper += gyres_above / work.slope[q];
                }
                if let Some(q) = south {
                    flux -= ekman_below * (energy - work.energy[q])
                        + gyres_below * (water - work.water[q]);
                    lower += gyres_below / work.slope[q];
                }
                work.rhs[p] += flux / w;
                work.diagonal[p] +=
                    (ekman_below + ekman_above + (gyres_below + gyres_above) / slope) / w;
                work.point_lower[p] -= lower / w;
                work.point_upper[p] -= upper / w;
            }
        }
    }

    /// The steady state under constant insolation `insolation_w_m2` under one τ of emission factor
    /// `emission_factor`, iterated from the temperatures `start_k`, K, and whether its iteration
    /// met [`STEADY_TOLERANCE_K`] within [`STEADY_ITERATIONS`].
    #[cfg(test)]
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
            &Radiation::Grey { emission_factor },
            insolation_w_m2,
            start_k,
            None,
            GridGround::SeaLevel,
            None,
            STEADY_ITERATIONS,
        )
    }

    /// [`steady_state`](Self::steady_state) over the ground `relief` (R09.T13.g).
    #[cfg(test)]
    #[must_use]
    pub(super) fn steady_state_over(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        start_k: Vec<f64>,
        relief: &Relief,
    ) -> Steady {
        self.relax(
            grid,
            &Radiation::Grey { emission_factor },
            insolation_w_m2,
            start_k,
            None,
            GridGround::Relief(relief),
            None,
            STEADY_ITERATIONS,
        )
    }

    /// The steady state under constant insolation `insolation_w_m2` and the radiation `radiation`,
    /// iterated from the temperatures `start_k`, K, for at most `iterations`, its functions read
    /// from `table` where it holds them, over the ground `ground`, its transmissions taken once at
    /// the radiation, with the sea's transport `ocean` at its own state's return temperatures
    /// (decision-r09-t13i §2.3.3); and whether its iteration met [`STEADY_TOLERANCE_K`]. A state
    /// that is no longer finite, a runaway's under the water term, stops it, unconverged.
    #[expect(
        clippy::too_many_arguments,
        reason = "the steady state's grid, forcing and start, and the step's table, relief, ocean \
                  and count; bundling them would only rename them"
    )]
    #[must_use]
    fn relax(
        &self,
        grid: &ModelGrid,
        radiation: &Radiation,
        insolation_w_m2: &[f64],
        start_k: Vec<f64>,
        table: Option<&Tabulated>,
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
        iterations: u32,
    ) -> Steady {
        let transmission = ground.transmissions(radiation);
        let at = ground.at(&transmission);
        let mut t_k = start_k;
        let mut work = Work::new(grid);
        for _ in 0..iterations {
            let change = self.advance(
                grid,
                radiation,
                insolation_w_m2,
                Storage::Relaxation,
                table,
                at,
                ocean.map(|ocean| (ocean, Returning::Own)),
                &mut t_k,
                &mut work,
            );
            if !t_k.iter().all(|t| t.is_finite()) {
                break;
            }
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
    /// steady state there, and how τ was found ([`fit`](Self::fit), at the sea level).
    #[cfg(test)]
    #[must_use]
    pub(super) fn emission_factor(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
    ) -> Fitted {
        self.fit(grid, insolation_w_m2, mean_k, GridGround::SeaLevel, None)
    }

    /// The emission factor 1 + ¾τ at which the annual-mean steady state under `insolation_w_m2`
    /// over the ground `ground`, each solve from an isothermal start at `mean_k`, has the surface
    /// mean `mean_k`, K ([`Relief::surface_mean_k`], decision-r09-t13a-relief §1.3): 48 bisections
    /// of ln(1 + ¾τ); the steady state there, and how τ was found.
    ///
    /// Under the water term the bisected factor is the one at `mean_k`, plan 14's T\*,
    /// u = ln(1 + ¾ m (`τ_d` + `τ_w`(T\*))), so that m solves as τ did ([`Radiation::at`];
    /// decision-r09-t13i §2.2). A τ that grows with T as √`p_sat` makes the outgoing radiation
    /// fall with T where ∂ln(1 + ¾τ) ÷ ∂T exceeds 4 ÷ T, a runaway: a probe whose steady state is
    /// not finite counts as warm, its surface mean not being below the target.
    ///
    /// The fold guard (decision-r09-t13ge §1.4, g4): the bisection keeps, on each side of its
    /// bracket, the last probe whose steady state converged, the bracket's two ends among them. If
    /// the final midpoint's steady state has not converged within [`STEADY_ITERATIONS`], it
    /// answers whichever kept probe's surface mean lies nearer `mean_k`, ties to the warm side
    /// ([`OpticalDepth::Folded`]), or, where it kept none, the final midpoint, unconverged and
    /// labelled [`OpticalDepth::Solved`] as before the guard; otherwise the midpoint, as before, to
    /// the bit. On a world of one month the steady state is the climate, and a steady state that
    /// had not converged would be a transient between two branches.
    #[must_use]
    fn fit(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
    ) -> Fitted {
        let solve = |factor: f64| {
            let radiation = Radiation::at(self.greenhouse, factor, mean_k);
            let steady = self.relax(
                grid,
                &radiation,
                insolation_w_m2,
                vec![mean_k; grid.points()],
                None,
                ground,
                ocean,
                STEADY_ITERATIONS,
            );
            let surface = match ground {
                GridGround::Relief(relief) => relief.surface_mean_k(grid, &steady.t_k),
                GridGround::SeaLevel => grid.mean(&steady.t_k),
            };
            Probe {
                emission_factor: factor,
                radiation,
                surface_mean_k: surface,
                steady,
            }
        };
        let fitted = |probe: Probe, optical_depth: OpticalDepth| Fitted {
            emission_factor: probe.emission_factor,
            radiation: probe.radiation,
            steady: probe.steady,
            surface_mean_k: probe.surface_mean_k,
            optical_depth,
        };
        let clear = solve(1.0);
        if clear.surface_mean_k >= mean_k {
            return fitted(clear, OpticalDepth::Transparent);
        }
        let opaque = solve(EMISSION_FACTOR_MAX);
        if opaque.surface_mean_k < mean_k {
            return fitted(opaque, OpticalDepth::Opaque);
        }
        let kept = |probe: Probe| probe.steady.converged.then_some(probe);
        let (mut cold, mut warm) = (kept(clear), kept(opaque));
        let (mut low, mut high) = (0.0, math::ln(EMISSION_FACTOR_MAX));
        for _ in 0..OPTICAL_DEPTH_ITERATIONS {
            let middle = f64::midpoint(low, high);
            let probe = solve(math::exp(middle));
            if probe.surface_mean_k < mean_k {
                low = middle;
                cold = kept(probe).or(cold);
            } else {
                high = middle;
                warm = kept(probe).or(warm);
            }
        }
        let last = solve(math::exp(f64::midpoint(low, high)));
        if last.steady.converged {
            return fitted(last, OpticalDepth::Solved);
        }
        match nearer(cold, warm, mean_k) {
            Some(probe) => fitted(probe, OpticalDepth::Folded),
            None => fitted(last, OpticalDepth::Solved),
        }
    }

    /// Whole seasonal orbits of `sunlight` under the radiation `radiation` from `start_k`, K,
    /// over points of sea share `sea_share` and the ground `ground`, with the sea's transport
    /// `ocean`, until the annual means move under [`CONVERGENCE_K`] between orbits, at most
    /// [`MAX_ORBITS`]: the last orbit's annual and monthly means, its ocean record, and the state
    /// at its end. The table reaches as far below and above the start as the highest and deepest
    /// quantiles' grounds need.
    ///
    /// The Ekman cells' return temperature is the state's own in the first orbit, and from the
    /// second the previous orbit's coldest month's band-mean sea temperature at the return
    /// latitude in each hemisphere (decision-r09-t13i §2.3.1).
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
    #[expect(
        clippy::too_many_arguments,
        reason = "the orbits' grid, sunlight, radiation and start, and the points' sea, relief and \
                  ocean; bundling them would only rename them"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "one orbit loop: the steps, the months' sums, the ocean's record and return \
                  temperatures and the stop, over the same arrays"
    )]
    #[must_use]
    fn orbits(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        radiation: &Radiation,
        start_k: Vec<f64>,
        sea_share: &[f64],
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
    ) -> Orbited {
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
        let (below, above) = match ground {
            GridGround::SeaLevel => (0.0, 0.0),
            GridGround::Relief(relief) => relief.table_reach_k(),
        };
        let table = Tabulated::new(self, radiation, low - below, high + above);
        let start_k = if sunlight.varies_in_longitude() {
            self.relax(
                grid,
                radiation,
                sunlight.annual(),
                start_k,
                Some(&table),
                ground,
                ocean,
                SPIN_UP_ITERATIONS,
            )
            .t_k
        } else {
            start_k
        };
        let transmission = ground.transmissions(radiation);
        let at = ground.at(&transmission);
        let ocean = ocean.filter(|ocean| ocean.any());
        let mut t = start_k;
        let mut work = Work::new(grid);
        let mut insolation = vec![0.0; points];
        // The last orbit's annual means, `None` before the first, its buffer reused orbit to orbit.
        let mut previous: Option<Vec<f64>> = None;
        let (mut annual, mut months) = (vec![0.0; points], vec![vec![0.0; points]; 12]);
        let mut counts = [0_u32; 12];
        let mut recorder = ocean.map(|_| Recorder::new(points, 12));
        let mut returning = Returning::Own;
        let mut water = vec![0.0; points];
        let mut band_sea = vec![0.0; LATITUDE_BANDS];
        let (mut orbits, mut converged) = (0, false);
        while orbits < MAX_ORBITS {
            annual.fill(0.0);
            for month in &mut months {
                month.fill(0.0);
            }
            counts = [0; 12];
            if let Some(recorder) = &mut recorder {
                recorder.clear();
            }
            for step in 0..steps {
                sunlight.fill(step, grid, &mut insolation);
                self.advance(
                    grid,
                    radiation,
                    &insolation,
                    storage,
                    Some(&table),
                    at,
                    ocean.map(|ocean| (ocean, returning)),
                    &mut t,
                    &mut work,
                );
                let month = usize::from(sunlight.month_of(step));
                counts[month] += 1;
                for ((sum, month_sum), &v) in annual.iter_mut().zip(&mut months[month]).zip(&t) {
                    *sum += v;
                    *month_sum += v;
                }
                if let Some(recorder) = &mut recorder {
                    recorder.add(grid, &work, month);
                }
            }
            orbits += 1;
            let n = f64::from(steps);
            for sum in &mut annual {
                *sum /= n;
            }
            if let Some(ocean) = ocean {
                returning = Returning::Coldest(coldest_return_k(
                    grid,
                    ocean,
                    at,
                    &months,
                    &counts,
                    &mut water,
                    &mut band_sea,
                ));
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
        Orbited {
            climate: GridClimate {
                annual_k: annual,
                months_k: months,
                orbits,
                converged,
            },
            ocean: recorder.map(|recorder| recorder.finish(&counts, steps)),
            final_k: t,
            returning,
        }
    }

    /// The ocean's record of one state `t_k`, K, a steady state's, the year's one month: the
    /// sea's fluxes at that state, its own return temperatures (decision-r09-t13i §2.3.3).
    #[must_use]
    fn steady_ocean_record(
        &self,
        grid: &ModelGrid,
        radiation: &Radiation,
        t_k: &[f64],
        ground: GridGround<&Relief>,
        ocean: &OceanGrid,
    ) -> OceanRecord {
        let transmission = ground.transmissions(radiation);
        let at = ground.at(&transmission);
        let mut work = Work::new(grid);
        for (p, &t) in t_k.iter().enumerate() {
            let [energy, slope] = self.energies(t, None);
            work.energy[p] = energy;
            work.slope[p] = slope;
        }
        self.couple_ocean(grid, ocean, Returning::Own, t_k, at, &mut work);
        let mut recorder = Recorder::new(grid.points(), 1);
        recorder.add(grid, &work, 0);
        recorder.finish(&[1], 1)
    }

    /// The model's run over `sunlight` for a body of mean surface temperature `mean_k`, over the
    /// points of `grid` of sea share `sea_share` and the ground `ground`, with the sea's transport
    /// `currents` (the [module documentation](self)): τ, or under the water term its multiplier,
    /// solved on the zonal grid in the grid's coordinates, over the zonal relief and its bands' sea
    /// shares, so that the steady state's surface mean is `mean_k`, then the orbits over the grid's
    /// own relief and sea.
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
        ground: Ground<'_>,
        currents: Option<&Currents>,
    ) -> Solution {
        let (on_grid, on_zonal) = match ground {
            Ground::SeaLevel => (GridGround::SeaLevel, GridGround::SeaLevel),
            Ground::Relief { grid, zonal } => (GridGround::Relief(grid), GridGround::Relief(zonal)),
        };
        let months = usize::from(sunlight.months());
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let forcing = zonal.mean(&annual) + self.heat_flow_w_m2;
        let ocean = currents.map(|currents| OceanGrid::new(grid, sea_share, currents));
        let zonal_ocean =
            currents.map(|currents| OceanGrid::new(&zonal, &grid.band_means(sea_share), currents));
        let solution = if positive(forcing) {
            let Fitted {
                emission_factor: factor,
                radiation,
                steady,
                surface_mean_k,
                optical_depth: depth,
            } = self.fit(&zonal, &annual, mean_k, on_zonal, zonal_ocean.as_ref());
            let converged = steady.converged;
            let steady = grid.broadcast(&steady.t_k);
            let (climate, record) = if sunlight.steps() == 0 {
                let record = ocean.as_ref().filter(|ocean| ocean.any()).map(|ocean| {
                    self.steady_ocean_record(grid, &radiation, &steady, on_grid, ocean)
                });
                (
                    GridClimate {
                        annual_k: steady.clone(),
                        months_k: vec![steady; months],
                        orbits: 0,
                        converged,
                    },
                    record,
                )
            } else {
                let orbited = self.orbits(
                    grid,
                    sunlight,
                    &radiation,
                    steady,
                    sea_share,
                    on_grid,
                    ocean.as_ref(),
                );
                (orbited.climate, orbited.ocean)
            };
            Solution {
                climate,
                emission_factor: factor,
                radiation,
                optical_depth: depth,
                steady_converged: converged,
                steady_surface_mean_k: surface_mean_k,
                month_insolation_w_m2: sunlight.month_means(grid),
                ocean: record,
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
                radiation: Radiation::Grey {
                    emission_factor: 1.0,
                },
                optical_depth: OpticalDepth::Unforced,
                steady_converged: true,
                steady_surface_mean_k: mean_k,
                month_insolation_w_m2: sunlight.month_means(grid),
                ocean: None,
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

/// What a seasonal run's orbits leave: the last orbit's means, the ocean's record over it, the
/// state at its end and the return temperatures its successor would read.
#[derive(Debug, Clone, PartialEq)]
struct Orbited {
    /// The last orbit's annual and monthly means, with the orbits run.
    climate: GridClimate,
    /// The ocean's record over the last orbit, where the sea's transport ran.
    ocean: Option<OceanRecord>,
    /// The state at the last orbit's end, K.
    final_k: Vec<f64>,
    /// The return temperatures after the last orbit.
    returning: Returning,
}

/// The northward transports at each band edge of a grid ([`LATITUDE_BANDS`] + 1 values, zero at
/// the poles), each the mean over the edge's columns of its points' southern-edge fluxes, W m⁻²
/// per 2πR² (times 2πR² the zonal transport in W; decision-r09-t13i §2.3.3): the atmosphere's
/// moist static energy, which the partition reads, the Ekman cells' and the gyres'.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is one carrier's flux, its unit in its name as the crate's rules ask"
)]
pub(super) struct Transports {
    /// The atmosphere's.
    pub(super) atmosphere_w_m2: Vec<f64>,
    /// The Ekman cells'.
    pub(super) ekman_w_m2: Vec<f64>,
    /// The gyres' and eddies'.
    pub(super) gyres_w_m2: Vec<f64>,
}

impl Transports {
    /// Every edge's at zero.
    #[must_use]
    fn zero() -> Self {
        Self {
            atmosphere_w_m2: vec![0.0; LATITUDE_BANDS + 1],
            ekman_w_m2: vec![0.0; LATITUDE_BANDS + 1],
            gyres_w_m2: vec![0.0; LATITUDE_BANDS + 1],
        }
    }

    /// Each part's sum at each edge with `other`'s times `weight`.
    fn add(&mut self, other: &Self, weight: f64) {
        for (mine, theirs) in [
            (&mut self.atmosphere_w_m2, &other.atmosphere_w_m2),
            (&mut self.ekman_w_m2, &other.ekman_w_m2),
            (&mut self.gyres_w_m2, &other.gyres_w_m2),
        ] {
            for (a, b) in mine.iter_mut().zip(theirs) {
                *a += weight * b;
            }
        }
    }
}

/// The sea's heat transport over a run (decision-r09-t13i §2.3.3), which the water cycle's
/// evaporation reads (its batch 2) and the tests compare with Trenberth and Caron: no output.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OceanRecord {
    /// Each month's mean ocean heat convergence at each point, W m⁻² of the point's area: the
    /// Ekman cells' and the gyres' fluxes into it less out of it, over its area's share of 2πR².
    pub(super) convergence_w_m2: Vec<Vec<f64>>,
    /// Each month's mean transports at each band edge.
    pub(super) months: Vec<Transports>,
    /// The year's.
    pub(super) annual: Transports,
}

/// A run's sums of the ocean's fluxes, month by month, over one orbit's steps.
#[derive(Debug, Clone, PartialEq)]
struct Recorder {
    convergence: Vec<Vec<f64>>,
    months: Vec<Transports>,
}

impl Recorder {
    /// Sums of every point's convergence and every edge's transports for `months` months of a grid
    /// of `points` points, each zero.
    #[must_use]
    fn new(points: usize, months: usize) -> Self {
        Self {
            convergence: vec![vec![0.0; points]; months],
            months: vec![Transports::zero(); months],
        }
    }

    /// Every sum back to zero, for the next orbit.
    fn clear(&mut self) {
        for month in &mut self.convergence {
            month.fill(0.0);
        }
        for month in &mut self.months {
            *month = Transports::zero();
        }
    }

    /// Adds one step's fluxes in `work` (its points' southern-edge fluxes, [`EnergyBalance`]'s
    /// `couple_ocean`) on `grid` to month `month`'s sums, edges from the south and columns from
    /// the west.
    fn add(&mut self, grid: &ModelGrid, work: &Work, month: usize) {
        let columns = grid.columns();
        let n = super::grid::index(columns);
        let mean = |values: &[f64], k: usize| {
            values[k * columns..(k + 1) * columns]
                .iter()
                .fold(0.0, |sum, v| sum + v)
                / n
        };
        let transports = &mut self.months[month];
        for k in 1..LATITUDE_BANDS {
            transports.atmosphere_w_m2[k] += mean(&work.south_atmosphere, k);
            transports.ekman_w_m2[k] += mean(&work.south_ekman, k);
            transports.gyres_w_m2[k] += mean(&work.south_gyres, k);
        }
        let convergence = &mut self.convergence[month];
        for k in 0..LATITUDE_BANDS {
            let w = grid.weight(k);
            for column in 0..columns {
                let p = grid.point(k, column);
                let into = work.south_ekman[p] + work.south_gyres[p];
                let out = if k + 1 < LATITUDE_BANDS {
                    work.south_ekman[p + columns] + work.south_gyres[p + columns]
                } else {
                    0.0
                };
                convergence[p] += (into - out) / w;
            }
        }
    }

    /// The record: each month's sums over its `counts` steps, and the year's over all `steps`.
    #[must_use]
    fn finish(self, counts: &[u32], steps: u32) -> OceanRecord {
        let mut annual = Transports::zero();
        let total = f64::from(steps);
        for month in &self.months {
            annual.add(month, 1.0 / total);
        }
        let months = self
            .months
            .iter()
            .zip(counts)
            .map(|(sums, &count)| {
                let mut mean = Transports::zero();
                mean.add(sums, 1.0 / f64::from(count));
                mean
            })
            .collect();
        let convergence_w_m2 = self
            .convergence
            .into_iter()
            .zip(counts)
            .map(|(sums, &count)| {
                let n = f64::from(count);
                sums.into_iter().map(|sum| sum / n).collect()
            })
            .collect();
        OceanRecord {
            convergence_w_m2,
            months,
            annual,
        }
    }
}

/// The return temperatures, south and north, K, that the next orbit's Ekman cells read: each
/// hemisphere's least over the months of its return band's sea temperature ([`OceanGrid::return_k`]),
/// from each month's sums `months` of the points' sea-level temperatures over its `counts` steps,
/// each point's water surface under the ground `ground`, with `water` and `band_sea` the scratch
/// of one month's points and bands.
#[must_use]
fn coldest_return_k(
    grid: &ModelGrid,
    ocean: &OceanGrid,
    ground: GridGround<ReliefAt<'_>>,
    months: &[Vec<f64>],
    counts: &[u32; 12],
    water: &mut [f64],
    band_sea: &mut [f64],
) -> [f64; 2] {
    let mut coldest = [f64::INFINITY; 2];
    for (sums, &count) in months.iter().zip(counts) {
        if count == 0 {
            continue;
        }
        let n = f64::from(count);
        for (p, (&sum, out)) in sums.iter().zip(water.iter_mut()).enumerate() {
            let t = sum / n;
            *out = match ground.raised(p) {
                None => t,
                Some((relief, (heights, _))) => relief.water_k(t, heights),
            };
        }
        ocean.band_sea_k(grid, water, band_sea);
        for (least, value) in coldest.iter_mut().zip(ocean.return_k(band_sea)) {
            if value < *least {
                *least = value;
            }
        }
    }
    coldest
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
    let slope = 4.0 * STEFAN_BOLTZMANN * cube / emission_factor;
    Emission {
        flux_w_m2: STEFAN_BOLTZMANN * cube * t_k / emission_factor,
        slope_w_m2_k: slope,
        fixed_slope_w_m2_k: slope,
    }
}

/// The outgoing radiation at the sea level at `t_k`, K, under `radiation`, its water term read
/// from `table` where it holds it: [`emission`] under one τ, [`Column::vapour`]'s under the water
/// term.
#[must_use]
fn sea_level_emission(radiation: &Radiation, t_k: f64, table: Option<&Tabulated>) -> Emission {
    match *radiation {
        Radiation::Grey { emission_factor } => emission(t_k, emission_factor),
        Radiation::Vapour {
            multiplier,
            dry_depth,
            water,
        } => Column::vapour(t_k, multiplier, dry_depth, water_terms(&water, t_k, table)).emission,
    }
}

/// The sea-level column of a point at one temperature, what each of its quantiles' outgoing
/// radiation reads: σT⁴ ÷ (1 + ¾τ) and its slope ([`emission`]), T³ and T⁴; under the water
/// term, also the slope before its floor.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Column {
    emission: Emission,
    /// The emission's slope before [`floored_slope`]: under one τ, its slope.
    whole_slope_w_m2_k: f64,
    cube: f64,
    fourth: f64,
}

impl Column {
    /// The column at the sea-level temperature `t_k`, K, under emission factor `emission_factor`.
    #[must_use]
    fn at(t_k: f64, emission_factor: f64) -> Self {
        let cube = t_k * t_k * t_k;
        let emission = emission(t_k, emission_factor);
        Self {
            emission,
            whole_slope_w_m2_k: emission.slope_w_m2_k,
            cube,
            fourth: cube * t_k,
        }
    }

    /// The column at the sea-level temperature `t_k`, K, under the water term of multiplier
    /// `multiplier` and dry depth `dry_depth`, its `τ_w` and slope at `t_k` `water`
    /// (decision-r09-t13i §2.2): x = 1 + ¾ m (`τ_d` + `τ_w`(T)), the flux σT⁴ ÷ x, the fixed-τ slope
    /// 4σT³ ÷ x, and the whole slope 4σT³ ÷ x − σT⁴ (¾ m ∂`τ_w` ÷ ∂T) ÷ x², held at
    /// [`EMISSION_SLOPE_FLOOR`] of the fixed one.
    #[must_use]
    fn vapour(t_k: f64, multiplier: f64, dry_depth: f64, water: [f64; 2]) -> Self {
        let [depth, depth_slope] = water;
        let factor = 1.0 + EDDINGTON_COEFFICIENT * multiplier * (dry_depth + depth);
        let cube = t_k * t_k * t_k;
        let fourth = cube * t_k;
        let fixed = 4.0 * STEFAN_BOLTZMANN * cube / factor;
        let whole = fixed
            - STEFAN_BOLTZMANN * fourth * (EDDINGTON_COEFFICIENT * multiplier * depth_slope)
                / (factor * factor);
        Self {
            emission: Emission {
                flux_w_m2: STEFAN_BOLTZMANN * fourth / factor,
                slope_w_m2_k: floored_slope(whole, fixed),
                fixed_slope_w_m2_k: fixed,
            },
            whole_slope_w_m2_k: whole,
            cube,
            fourth,
        }
    }
}

/// The outgoing radiation of one quantile of a raised point and its slope in T, W m⁻² and
/// W m⁻² K⁻¹, over the sea-level column `column` (the point's temperature T and emission factor,
/// computed once for its quantiles), its ground at `ground_k`, K, of slope `ground_rate` in T (1,
/// or 0 where the floor holds it), seen through the column above it of transmission
/// `transmission`, e^(−(3/2)`τ_z`) (decision-r09-t13a-relief §1.3, decision-r09-t13ge §1.4, g5):
///
/// OLR = max(σT⁴ ÷ (1 + ¾τ) − e^(−(3/2)`τ_z`) σ(T⁴ − `T_g`⁴), e^(−(3/2)`τ_z`) σ`T_g`⁴),
///
/// the sea-level column's less the change at the raised ground transmitted through the column
/// above it, to first order in the removed layer's own optical depth, and never below the ground's
/// emission transmitted, since the air's own emission is not negative; its slope from whichever
/// term binds. At z = 0 it is σT⁴ ÷ (1 + ¾τ); as τ → 0 it is σ`T_g`⁴, the ground's own; the floor
/// binds only where the transmission exceeds 1 ÷ (1 + ¾τ), above 16.2 km on an Earth and 3.89
/// scale heights up under τ = 10.
#[must_use]
fn quantile_emission(
    column: &Column,
    ground_k: f64,
    ground_rate: f64,
    transmission: f64,
) -> Emission {
    let (cube, fourth) = (column.cube, column.fourth);
    let column = column.emission;
    let ground_cube = ground_k * ground_k * ground_k;
    let ground_fourth = ground_cube * ground_k;
    let through = column.flux_w_m2 - transmission * STEFAN_BOLTZMANN * (fourth - ground_fourth);
    let ground = transmission * STEFAN_BOLTZMANN * ground_fourth;
    let (flux_w_m2, slope_w_m2_k) = if through >= ground {
        (
            through,
            column.slope_w_m2_k
                - transmission * 4.0 * STEFAN_BOLTZMANN * (cube - ground_cube * ground_rate),
        )
    } else {
        (
            ground,
            transmission * 4.0 * STEFAN_BOLTZMANN * ground_cube * ground_rate,
        )
    };
    Emission {
        flux_w_m2,
        slope_w_m2_k,
        fixed_slope_w_m2_k: slope_w_m2_k,
    }
}

/// [`quantile_emission`] under the water term (decision-r09-t13i §2.2, R2): the quantile's
/// transmission is `dry_transmission`, e^(−(3/2) m `τ_d` e^(−z ÷ 2H)), times the water's
/// e^(−(3/2) m `τ_w`(`T_g`)) at the ground's temperature, `wet` with its slope in `T_g`
/// ([`wet_of`]), so that the vapour above raised ground thins over its Clausius–Clapeyron height
/// with the ground's temperature rather than over the air's 2H. The flux is
/// max(σT⁴ ÷ x(T) − tr σ(T⁴ − `T_g`⁴), tr σ`T_g`⁴) as [`quantile_emission`]'s; its whole slope adds
/// the transmission's own slope in T, and is held at [`EMISSION_SLOPE_FLOOR`] of the slope at a
/// fixed τ and transmission. At z = 0 it is the sea-level column's to the bit.
#[must_use]
fn vapour_quantile_emission(
    column: &Column,
    ground_k: f64,
    ground_rate: f64,
    dry_transmission: f64,
    wet: [f64; 2],
) -> Emission {
    let (cube, fourth) = (column.cube, column.fourth);
    let ground_cube = ground_k * ground_k * ground_k;
    let ground_fourth = ground_cube * ground_k;
    let transmission = dry_transmission * wet[0];
    let transmission_slope = dry_transmission * wet[1] * ground_rate;
    let through =
        column.emission.flux_w_m2 - transmission * STEFAN_BOLTZMANN * (fourth - ground_fourth);
    let ground = transmission * STEFAN_BOLTZMANN * ground_fourth;
    let (flux_w_m2, whole, fixed) = if through >= ground {
        let held = transmission * 4.0 * STEFAN_BOLTZMANN * (cube - ground_cube * ground_rate);
        (
            through,
            column.whole_slope_w_m2_k
                - transmission_slope * STEFAN_BOLTZMANN * (fourth - ground_fourth)
                - held,
            column.emission.fixed_slope_w_m2_k - held,
        )
    } else {
        let held = transmission * 4.0 * STEFAN_BOLTZMANN * ground_cube * ground_rate;
        (
            ground,
            held + transmission_slope * STEFAN_BOLTZMANN * ground_fourth,
            held,
        )
    };
    Emission {
        flux_w_m2,
        slope_w_m2_k: floored_slope(whole, fixed),
        fixed_slope_w_m2_k: fixed,
    }
}

/// The outgoing radiation of a raised point of heights `heights` under `relief` and its slope in
/// T at the sea-level temperature `t_k`, K, under emission factor `emission_factor`, each
/// quantile seen through its transmission `transmission` ([`quantile_emission`]): the mean over
/// its quantiles, once at a level point.
#[must_use]
fn point_emission(
    t_k: f64,
    emission_factor: f64,
    heights: &PointHeights,
    transmission: &[f64; QUANTILES],
    relief: &Relief,
) -> Emission {
    let column = Column::at(t_k, emission_factor);
    let at = |q: usize| {
        let (ground, rate) = relief.ground_k(t_k, heights.quantiles_m[q]);
        quantile_emission(&column, ground, rate, transmission[q])
    };
    quantile_mean(heights.shape, at)
}

/// The mean over a point's quantiles of `at`'s emission, quantile by quantile in order, or the
/// first quantile's alone where the point is level (`shape`).
#[must_use]
fn quantile_mean(shape: Shape, at: impl Fn(usize) -> Emission) -> Emission {
    match shape {
        Shape::SeaLevel | Shape::Level => at(0),
        Shape::Varied => {
            let (flux, slope, fixed) =
                (0..QUANTILES).fold((0.0, 0.0, 0.0), |(flux, slope, fixed), q| {
                    let quantile = at(q);
                    (
                        flux + quantile.flux_w_m2,
                        slope + quantile.slope_w_m2_k,
                        fixed + quantile.fixed_slope_w_m2_k,
                    )
                });
            Emission {
                flux_w_m2: flux * QUANTILE_WEIGHT,
                slope_w_m2_k: slope * QUANTILE_WEIGHT,
                fixed_slope_w_m2_k: fixed * QUANTILE_WEIGHT,
            }
        }
    }
}

/// The outgoing radiation of a raised point of heights `heights` under `relief` and its slopes in
/// T at the sea-level temperature `t_k`, K, under the water term of `radiation`'s multiplier
/// `multiplier`, dry depth `dry_depth` and water `water`, each quantile seen through its dry
/// transmission `transmission` and its water's at its ground ([`vapour_quantile_emission`]),
/// read from `table` where it holds them: the mean over its quantiles, once at a level point.
#[expect(
    clippy::too_many_arguments,
    reason = "the point's temperature, its relief and heights, the water term's three parts and the \
              table; bundling them would only rename them"
)]
#[must_use]
fn vapour_point_emission(
    t_k: f64,
    multiplier: f64,
    dry_depth: f64,
    water: &WaterVapour,
    heights: &PointHeights,
    transmission: &[f64; QUANTILES],
    relief: &Relief,
    table: Option<&Tabulated>,
) -> Emission {
    let column = Column::vapour(t_k, multiplier, dry_depth, water_terms(water, t_k, table));
    let at = |q: usize| {
        let (ground, rate) = relief.ground_k(t_k, heights.quantiles_m[q]);
        let wet = wet_terms(water, multiplier, ground, table);
        vapour_quantile_emission(&column, ground, rate, transmission[q], wet)
    };
    quantile_mean(heights.shape, at)
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

/// The currents of the body of `inputs` under the model `model` that `ocean` asks for, turning
/// as `turning` says: the Ekman cells under its Hadley circulation on a rotating world, none on a
/// locked one (decision-r09-t13i §2.3.1).
#[must_use]
fn currents_of(
    inputs: &CoarseInputs,
    model: &EnergyBalance,
    ocean: OceanTransport,
    turning: Turning,
) -> Option<Currents> {
    match ocean {
        OceanTransport::None => None,
        OceanTransport::Currents { mean_depth_m } => {
            let circulation = match turning {
                Turning::Rotating => Some(Circulation::for_inputs(inputs, turning)),
                Turning::Locked => None,
            };
            Some(Currents::of(
                inputs,
                model.freezing,
                mean_depth_m,
                circulation,
            ))
        }
    }
}

/// The model's answer for the body of `inputs`, which turns under its sun, over the points of
/// `grid` (the zonal or the geographic grid) of sea share `sea_share` and the ground `ground`,
/// with the sea's transport `ocean`, under the diurnal-mean sunlight, at a nominal step of `step`
/// ([`NOMINAL_STEP`](super::sun::NOMINAL_STEP) in the pass).
#[must_use]
pub(super) fn solve(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
    ocean: OceanTransport,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    let currents = currents_of(inputs, &model, ocean, Turning::Rotating);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
        currents.as_ref(),
    )
}

/// The model's answer for the body of `inputs`, locked 1:1 to its seasonal host, over the points
/// of `grid` (the locked grid) in tidally locked coordinates, of sea share `sea_share` and the
/// ground `ground` (its zonal relief the tidally locked bands'), with the sea's transport `ocean`
/// (the gyres' diffusion alone, unwalled), under the host's beam ([`Sunlight::locked`]), at a
/// nominal step of `step`.
#[must_use]
pub(super) fn solve_locked(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
    ocean: OceanTransport,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::locked(inputs, grid, Coordinates::TidallyLocked, step);
    let currents = currents_of(inputs, &model, ocean, Turning::Locked);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
        currents.as_ref(),
    )
}

/// A point's outgoing radiation at `t_k`, K, under `radiation`, at the sea level where `raised` is
/// `None` (T13.a's arithmetic to the bit) and otherwise over its relief, heights and
/// transmissions; and the temperature its sea's capacity reads, its water surface's, K.
#[must_use]
fn emission_and_sea(
    radiation: &Radiation,
    t_k: f64,
    raised: Option<(&Relief, (&PointHeights, &[f64; QUANTILES]))>,
    table: Option<&Tabulated>,
) -> (Emission, f64) {
    match (*radiation, raised) {
        (_, None) => (sea_level_emission(radiation, t_k, table), t_k),
        (Radiation::Grey { emission_factor }, Some((relief, (heights, transmission)))) => (
            point_emission(t_k, emission_factor, heights, transmission, relief),
            relief.water_k(t_k, heights),
        ),
        (
            Radiation::Vapour {
                multiplier,
                dry_depth,
                water,
            },
            Some((relief, (heights, transmission))),
        ) => (
            vapour_point_emission(
                t_k,
                multiplier,
                dry_depth,
                &water,
                heights,
                transmission,
                relief,
                table,
            ),
            relief.water_k(t_k, heights),
        ),
    }
}

/// The step's latitude solve, column by column from the west, of `work`'s systems on `grid`: each
/// column's own sub- and super-diagonals where the sea's terms are `coupled` into them, and the
/// diffusion's shared ones otherwise; into `work.solution`.
fn solve_latitudes(grid: &ModelGrid, coupled: bool, work: &mut Work) {
    for column in 0..grid.columns() {
        for k in 0..LATITUDE_BANDS {
            let p = grid.point(k, column);
            work.column_diagonal[k] = work.diagonal[p];
            work.column_rhs[k] = work.rhs[p];
            if coupled {
                work.column_lower[k] = work.point_lower[p];
                work.column_upper[k] = work.point_upper[p];
            }
        }
        let (lower, upper) = if coupled {
            (&work.column_lower, &work.column_upper)
        } else {
            (&work.lower, &work.upper)
        };
        solve_tridiagonal(
            lower,
            &work.column_diagonal,
            upper,
            &work.column_rhs,
            &mut work.column_solution,
            &mut work.column_scratch,
        );
        for k in 0..LATITUDE_BANDS {
            work.solution[grid.point(k, column)] = work.column_solution[k];
        }
    }
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
    use hyperion_testkit::float::assert_same_bits;

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
            radiation,
            steady: Steady {
                t_k: steady,
                converged,
            },
            surface_mean_k,
            optical_depth: depth,
        } = model.emission_factor(&grid, sun.annual(), 288.0);
        assert!(converged);
        assert_same_bits(surface_mean_k, grid.mean(&steady));
        assert_eq!(depth, OpticalDepth::Solved);
        assert!(
            (grid.mean(&steady) - 288.0).abs() < 1e-6,
            "{}",
            grid.mean(&steady)
        );
        let tau = (factor - 1.0) / 0.75;
        assert!((0.3..2.0).contains(&tau), "{tau}");
        let (radiative, diffusive) = model.heating(&grid, &radiation, sun.annual(), &steady);
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
        let none = model.run(
            &grid,
            &dark,
            40.0,
            &[0.0; LATITUDE_BANDS],
            Ground::SeaLevel,
            None,
        );
        assert_eq!(none.optical_depth, OpticalDepth::Unforced);
        assert!(
            none.climate
                .annual_k
                .iter()
                .all(|t| (t - 40.0).abs() < 1e-12)
        );
        model.heat_flow_w_m2 = 0.0916;
        let warm = model.run(
            &grid,
            &dark,
            60.0,
            &[0.0; LATITUDE_BANDS],
            Ground::SeaLevel,
            None,
        );
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
        let radiation = Radiation::Grey {
            emission_factor: 1.65,
        };
        let mut work = Work::new(&grid);
        let mut state = t.clone();
        model.advance(
            &grid,
            &radiation,
            &insolation,
            Storage::Seasonal {
                step_s: sun.step_s(),
                sea_share: &sea,
            },
            None,
            GridGround::SeaLevel,
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
        let (_, diffusive) = model.heating(&grid, &radiation, &insolation, &t);
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
        let table = Tabulated::new(
            &model,
            &Radiation::at(model.greenhouse, 1.65, 288.0),
            250.0,
            300.0,
        );
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
            assert_same_bits(a, b);
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

    /// A point half at the sea level and half 3 km up, and one wholly at their mean height.
    fn split_and_mean(inputs: &CoarseInputs) -> Relief {
        let split = [0.0, 0.0, 0.0, 0.0, 3_000.0, 3_000.0, 3_000.0, 3_000.0];
        Relief::of(
            inputs,
            Hypsometry::new(vec![
                PointHeights::new(split, 0.0),
                PointHeights::level(1_500.0, 0.0),
            ]),
        )
    }

    /// R09.T13.g (decision-r09-t13ge §1.6): a band half at sea level and half 3 km up is brighter
    /// than one wholly at their mean height where that height's ground lies above the albedo's
    /// midpoint, and darker below it, the tanh law being convex above its midpoint and concave
    /// below; read through the table, the same. At 275, 280 and 285 K at sea level the two read
    /// decision-r09-t13a-relief §1.2's 0.537 against 0.615, 0.504 against 0.391 and 0.420
    /// against 0.299 (its Γ 5.85 K km⁻¹; the reference Earth's 5.818), to 0.01.
    #[test]
    fn a_band_half_3_km_up_is_brighter_than_its_mean_height_above_the_albedo_s_midpoint() {
        let inputs = reference::earth_like();
        let model = EnergyBalance::for_inputs(&inputs);
        let relief = split_and_mean(&inputs);
        let (split, mean) = (relief.hypsometry().point(0), relief.hypsometry().point(1));
        let gamma = lapse::lapse_rate_k_per_m(&inputs);
        let point = model.freezing.point_k().expect("water freezes");
        let midpoint = point - ALBEDO_MIDPOINT_BELOW_FREEZING_K;
        let table = Tabulated::new(
            &model,
            &Radiation::Grey {
                emission_factor: 1.65,
            },
            200.0,
            320.0,
        );
        for offset in [-15.0, -8.0, -4.0, -1.0, -0.25, 0.25, 1.0, 4.0, 8.0, 15.0] {
            // The sea level at which the mean height's ground lies `offset` from the midpoint.
            let t = midpoint + offset + gamma * 1_500.0;
            let (a, b) = (
                model.point_albedo(t, split, &relief, None),
                model.point_albedo(t, mean, &relief, None),
            );
            eprintln!(
                "ground at 1.5 km {:+6.2} K from the midpoint: split {:.4}, mean height {:.4}",
                offset, a.albedo, b.albedo
            );
            if offset > 0.0 {
                assert!(a.albedo > b.albedo, "{offset}: {a:?} against {b:?}");
            } else {
                assert!(a.albedo < b.albedo, "{offset}: {a:?} against {b:?}");
            }
            let read = model.point_albedo(t, split, &relief, Some(&table));
            assert!((read.albedo - a.albedo).abs() < 1e-9);
            assert!((read.slope_per_k - a.slope_per_k).abs() < 1e-8);
        }
        for (t, ruled_split, ruled_mean) in [
            (275.0, 0.537, 0.615),
            (280.0, 0.504, 0.391),
            (285.0, 0.420, 0.299),
        ] {
            let (a, b) = (
                model.point_albedo(t, split, &relief, None).albedo,
                model.point_albedo(t, mean, &relief, None).albedo,
            );
            eprintln!("{t} K: split {a:.3} ({ruled_split}), mean height {b:.3} ({ruled_mean})");
            assert!((a - ruled_split).abs() < 0.01 && (b - ruled_mean).abs() < 0.01);
        }
    }

    /// R09.T13.g (decision-r09-t13ge §1.6, g2): the heat capacity of a point half sea and half a
    /// 3 km plateau, just above the freezing point at sea level, carries the open sea's capacity
    /// on its sea share, its water's surface at the sea level, where the plateau's quantiles would
    /// read ice; a lake whose surface is 3 km up reads that surface's ice.
    #[test]
    fn a_sea_beside_a_plateau_keeps_the_open_sea_s_heat_capacity() {
        let inputs = reference::earth_like();
        let model = EnergyBalance::for_inputs(&inputs);
        let relief = split_and_mean(&inputs);
        let split = relief.hypsometry().point(0);
        let point = model.freezing.point_k().expect("water freezes");
        let t = point + 0.5;
        let capacity = model.heat_capacity(relief.water_k(t, split), 0.5);
        let open = LAND_HEAT_CAPACITY_J_PER_M2_K * (0.5 + 0.5 * SEA_HEAT_CAPACITY_RATIO);
        assert_same_bits(capacity, open);
        // The plateau's ground, 17 K colder, would read the frozen sea's 2.0.
        let gamma = lapse::lapse_rate_k_per_m(&inputs);
        let frozen = LAND_HEAT_CAPACITY_J_PER_M2_K * (0.5 + 0.5 * ICE_HEAT_CAPACITY_RATIO);
        assert_same_bits(model.heat_capacity(t - gamma * 3_000.0, 0.5), frozen);
        let lake = PointHeights::new(split.quantiles_m, 3_000.0);
        assert_same_bits(model.heat_capacity(relief.water_k(t, &lake), 0.5), frozen);
    }

    /// R09.T13.g (decision-r09-t13ge §1.6): with no transport and τ = 0, a column's ground
    /// temperature at a given sunlight is the same at every height: its outgoing radiation is the
    /// ground's own, σ`T_g`⁴, and its albedo reads `T_g`, so the steady state of each band, every
    /// one under the same sunlight and each at its own height from 0 to 7.1 km, holds the same
    /// ground. Under the reference Earth's τ, with an albedo that cannot flip, the higher ground is
    /// colder, by less than Γz.
    #[test]
    fn with_no_transport_and_no_greenhouse_the_ground_s_temperature_is_independent_of_height() {
        let inputs = reference::earth_like();
        let mut model = EnergyBalance::for_inputs(&inputs);
        model.transport_kg_m2_s = 0.0;
        let grid = ModelGrid::zonal();
        let height = |k: usize| 100.0 * super::super::grid::index(k);
        let relief = Relief::of(
            &inputs,
            Hypsometry::new(
                (0..LATITUDE_BANDS)
                    .map(|k| PointHeights::level(height(k), 0.0))
                    .collect(),
            ),
        );
        let gamma = lapse::lapse_rate_k_per_m(&inputs);
        let sunlight = vec![300.0; LATITUDE_BANDS];
        for (factor, ground_k) in [(1.0, 250.0), (1.0, 290.0), (1.65, 270.0)] {
            let start = (0..LATITUDE_BANDS)
                .map(|k| ground_k + gamma * height(k))
                .collect();
            // Under a greenhouse, an albedo that cannot flip, so that the contrast is the column's.
            let model = if factor > 1.0 {
                EnergyBalance {
                    freezing: Freezing::Never {
                        albedo: ICE_FREE_ALBEDO,
                    },
                    ..model
                }
            } else {
                model
            };
            let steady = model.steady_state_over(&grid, factor, &sunlight, start, &relief);
            assert!(steady.converged, "{factor}");
            let grounds: Vec<f64> = (0..LATITUDE_BANDS)
                .map(|k| steady.t_k[k] - gamma * height(k))
                .collect();
            let (low, high) = grounds
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &g| {
                    (l.min(g), h.max(g))
                });
            eprintln!(
                "x {factor}: ground {:.6} K at sea level, {:.6} K at 7.1 km; spread {:e} K",
                grounds[0],
                grounds[LATITUDE_BANDS - 1],
                high - low
            );
            if factor > 1.0 {
                let fall = grounds[0] - grounds[LATITUDE_BANDS - 1];
                assert!(
                    fall > 1.0 && fall < gamma * height(LATITUDE_BANDS - 1),
                    "{fall}"
                );
            } else {
                assert!(high - low < 1e-6, "{grounds:?}");
            }
        }
    }

    /// R09.T13.g (decision-r09-t13ge §1.6, g5): the outgoing radiation of every quantile is
    /// positive and rises with T, over τ from 0 to 20, ground from sea level to four scale heights
    /// up and grounds of 50–300 K, and is never below the ground's emission transmitted through the
    /// column above it; its slope is the derivative of the branch that binds. A τ = 10 column four
    /// scale heights up takes that floor; at the sea level the floor never binds, and the form is
    /// σT⁴ ÷ (1 + ¾τ) to the bit.
    #[test]
    fn the_outgoing_radiation_rises_with_t_and_never_falls_below_the_transmitted_ground() {
        let inputs = reference::earth_like();
        let (gamma, h) = (lapse::lapse_rate_k_per_m(&inputs), scale_height_m(&inputs));
        eprintln!("Γ {:.4} K km⁻¹, H {:.1} m", gamma * 1e3, h);
        assert!((h - 8_470.0).abs() < 20.0, "{h}");
        let transmission = |tau: f64, z: f64| {
            math::exp(-FLUX_OPTICAL_DEPTH_PER_GREY * tau * math::exp(-z / (2.0 * h)))
        };
        let mut floors = 0;
        for tau in [0.0, 0.3, 0.876, 2.0, 5.0, 10.0, 20.0] {
            let factor = 1.0 + EDDINGTON_COEFFICIENT * tau;
            // The first height is the sea level's.
            for (height, scale_heights) in
                [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0].into_iter().enumerate()
            {
                let z = scale_heights * h;
                let tr = transmission(tau, z);
                for ground in [50.0, 100.0, 150.0, 200.0, 250.0, 300.0] {
                    let t = ground + gamma * z;
                    let at = |dt: f64| {
                        quantile_emission(&Column::at(t + dt, factor), ground + dt, 1.0, tr)
                    };
                    let e = at(0.0);
                    let through = STEFAN_BOLTZMANN * math::powi(t, 4) / factor
                        - tr * STEFAN_BOLTZMANN * (math::powi(t, 4) - math::powi(ground, 4));
                    let floor = tr * STEFAN_BOLTZMANN * math::powi(ground, 4);
                    assert!(e.flux_w_m2 > 0.0, "τ {tau}, z {z}, {ground} K: {e:?}");
                    assert!(e.slope_w_m2_k > 0.0, "τ {tau}, z {z}, {ground} K: {e:?}");
                    assert!(at(0.5).flux_w_m2 > e.flux_w_m2);
                    assert!(e.flux_w_m2 >= floor * (1.0 - 1e-12));
                    assert!((e.flux_w_m2 - through.max(floor)).abs() < 1e-9 * e.flux_w_m2);
                    if (through - floor).abs() > 1e-3 * floor {
                        let numeric = (at(1e-3).flux_w_m2 - at(-1e-3).flux_w_m2) / 2e-3;
                        assert!(
                            (numeric - e.slope_w_m2_k).abs() < 1e-6 * e.slope_w_m2_k,
                            "τ {tau}, z {z}, {ground} K: {numeric} against {e:?}"
                        );
                    }
                    if through < floor {
                        floors += 1;
                    }
                    if height == 0 {
                        let sea = emission(t, factor);
                        assert_same_bits(e.flux_w_m2, sea.flux_w_m2);
                        assert_same_bits(e.slope_w_m2_k, sea.slope_w_m2_k);
                    }
                }
            }
        }
        eprintln!("the floor bound {floors} cases");
        // Four scale heights under τ = 10: e^(−15 e^(−2)) = 0.131 > 1 ÷ 8.5.
        let (tau, z, ground) = (10.0, 4.0 * h, 220.0);
        let tr = transmission(tau, z);
        assert!(tr > 1.0 / (1.0 + EDDINGTON_COEFFICIENT * tau));
        let factor = 1.0 + EDDINGTON_COEFFICIENT * tau;
        let e = quantile_emission(&Column::at(ground + gamma * z, factor), ground, 1.0, tr);
        assert_same_bits(e.flux_w_m2, tr * STEFAN_BOLTZMANN * math::powi(ground, 4));
        // A floored ground's emission holds still.
        let held = quantile_emission(&Column::at(400.0, factor), 30.0, 0.0, tr);
        assert!(held.flux_w_m2 > 0.0);
        assert_same_bits(held.slope_w_m2_k, 0.0);
    }

    /// The fold guard's choice (decision-r09-t13ge §1.4, g4): the kept probe whose surface mean
    /// lies nearer the target, ties to the warm side, either side alone, or none; on the relief
    /// Earth's fold, 287.34 K on the cold branch and 288.46 K on the warm one, the warm.
    #[test]
    fn the_fold_guard_takes_the_nearer_converged_probe_ties_to_the_warm_side() {
        let probe = |factor: f64, mean: f64| Probe {
            emission_factor: factor,
            radiation: Radiation::Grey {
                emission_factor: factor,
            },
            surface_mean_k: mean,
            steady: Steady {
                t_k: vec![mean],
                converged: true,
            },
        };
        let chosen = |cold: Option<Probe>, warm: Option<Probe>| {
            nearer(cold, warm, 288.0).map(|probe| probe.emission_factor)
        };
        assert_eq!(
            chosen(Some(probe(1.656_70, 287.34)), Some(probe(1.656_80, 288.46))),
            Some(1.656_80)
        );
        assert_eq!(
            chosen(Some(probe(1.0, 287.9)), Some(probe(2.0, 288.5))),
            Some(1.0)
        );
        assert_eq!(
            chosen(Some(probe(1.0, 287.5)), Some(probe(2.0, 288.5))),
            Some(2.0)
        );
        assert_eq!(chosen(Some(probe(1.0, 280.0)), None), Some(1.0));
        assert_eq!(chosen(None, Some(probe(2.0, 300.0))), Some(2.0));
        assert_eq!(chosen(None, None), None);
    }
}
