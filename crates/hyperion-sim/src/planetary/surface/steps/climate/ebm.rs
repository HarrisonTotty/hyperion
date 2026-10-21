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
//! C(T) ∂T/∂t = S (1 − α(T)) + `F_int` − σT⁴ ÷ (1 + ¾τ) + Λ ∇²h + `Q_O`,
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
//!   air's own water, both by weight as plan 14 takes them (its §2.4 follow-up 2 §4 and follow-up 5
//!   §2, [`Reservoir`]); where the gases hold no water, and in Kaspi and Showman's fit, one τ
//!   ([`Greenhouse::Grey`]);
//! - h the near-surface moist static energy `c_p` T + L q, with q the climate condensable's
//!   specific humidity at 80% relative humidity ([`RELATIVE_HUMIDITY`]), from its saturation
//!   pressure in the substance registry at the surface pressure, L its latent heat of
//!   vaporisation per kilogram, and `c_p` the air's mixed heat capacity: on a dry world, one whose
//!   condensable is not among its surface liquids or that has none, h is `c_p` T alone;
//! - Λ the transport's coefficient ([`transport`](super::transport)), no-flux at the poles;
//! - `Q_O` the ocean's heat convergence over the sea (R09.T13.i, decision-r09-t13i §2.3,
//!   [`ocean`](super::ocean)): the Ekman cells' share of the latitude flux of h inside the Hadley
//!   cell and the gyres' diffusion of the sea's water-surface temperature, none on a world with no
//!   sea.
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
//! ([`orbits`](EnergyBalance::orbits)), each run until its stop holds: the move of the points'
//! annual means between orbits under [`CONVERGENCE_K`], and the remainder the shrinking moves
//! extrapolate to under it too, at least [`MIN_ORBITS`] and at most [`MAX_ORBITS`], its slow mode
//! extrapolated once the moves shrink steadily ([`Extrapolation`]). The surface mean is corrected
//! on the seasonal climate (decision-r09-t13i's §2.4 follow-up 4): a search for the greenhouse on
//! the zonal model's seasonal runs, from the fit's steady state, then at most two runs on the
//! whole grid, the first from the zonal answer broadcast to every column
//! ([`corrected`](EnergyBalance::corrected)). A one-month year is the steady state itself.

use super::coordinates::Coordinates;
use super::frost;
use super::grid::{
    CyclicWork, GridClimate, LATITUDE_BANDS, ModelGrid, solve_cyclic_tridiagonal, solve_tridiagonal,
};
use super::hypsometry::{Hypsometry, PointHeights, QUANTILES, Shape};
use super::lapse;
use super::ocean::{Currents, OceanGrid, OceanTransport, Returning, SeaLiquid};
use super::partition::Turning;
use super::sun::Sunlight;
use super::transport::{Diffusing, REFERENCE_PRESSURE_PA, rotation_ratio, transport_kg_m2_s};
use crate::math;
use crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY;
use crate::planetary::derive::atmosphere::{Gas, GreenhouseConstants, PartialPressures};
use crate::planetary::hooks::surface::{
    EARTH_AIR_MOLAR_MASS_G_PER_MOL, EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
    FLUX_OPTICAL_DEPTH_PER_GREY, night_floor,
};
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{
    MOLAR_GAS_CONSTANT, PhaseDiagram, SubstanceId, by_key, mean_molar_mass_g_per_mol,
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
/// converges in about ten. A run that reaches it is flagged as not converged.
pub(super) const MAX_ORBITS: u32 = 40;

/// The fewest seasonal orbits a run takes: 3, so that its stop reads two moves of the annual
/// means (decision-r09-t13i §2.4).
pub(super) const MIN_ORBITS: u32 = 3;

/// The bound of the stop, K: 0.1 (Design note 8). A run has converged when the move of the
/// points' annual means between its last two orbits, `m_n` (the largest over the points), is
/// under it, the moves are shrinking, r = `m_n` ÷ `m_n−1` < 1, and the remainder they extrapolate
/// to, `m_n` r ÷ (1 − r), is under it too (decision-r09-t13i §2.4, T13.a's deferred finding: an
/// orbit much shorter than the ocean's relaxation time, C ÷ B of about two years on an Earth under
/// one τ and 3.7 under the water term's weaker restoring, moves little each orbit, and the move
/// alone stopped an orbit of 0.1 year several kelvin short). The zonal search's runs take the same
/// rule at [`SEARCH_CONVERGENCE_K`].
pub(super) const CONVERGENCE_K: f64 = 0.1;

/// The change of u = ln(1 + ¾τ(T\*)) on either side of the fitted one at which the zonal search
/// takes the zonal steady states' two-sided slope of the surface mean in u for its first step: 0.05
/// (decision-r09-t13i §2.4, as its §2.4 follow-ups keep it).
pub(super) const CORRECTION_STEP: f64 = 0.05;

/// The bound of the zonal search's stop, and its extrapolation's trigger, K: 0.01 (`Adopted`,
/// decision-r09-t13i's §2.4 follow-up 4 §4): the stop's rule at a tenth of its bound, so that the
/// search's secant reads differences clean of its runs' own convergence (its steps are 0.1–2 K), at
/// the zonal model's cost, about a seventy-second of a geographic orbit's. A search ends when one
/// of its converged runs lies within it of its target.
pub(super) const SEARCH_CONVERGENCE_K: f64 = 0.01;

/// The most runs one zonal search makes: 6 (`Adopted`, decision-r09-t13i's §2.4 follow-up 4 §4).
pub(super) const SEARCH_RUNS: usize = 6;

/// The most the search's secant step moves u where its two points do not straddle the target,
/// over the step between them: 4 (`Adopted`, decision-r09-t13i's §2.4 follow-up §2, kept by its
/// follow-up 4: the lane's sea-level Earth needed about 2.2 times, and a 5.2-times step ran into
/// the seasonal branch's own collapse).
pub(super) const SECANT_STEP_CAP: f64 = 4.0;

/// The range, open at both ends, of the ratio r of an orbit's move to the one before within which
/// the spin-up extrapolates the run's slow mode: (0.3, 0.97) (`Adopted`, decision-r09-t13i's §2.4
/// follow-up 4 §3). Below 0.3 the mode shrinks tenfold in two orbits anyway, and the ratio of small
/// moves is noise; above 0.97, a gain over 32, the run is in a fold's critical slowing or on a short
/// orbit, where one linear mode no longer holds (in the ruling's prototype a gain of 37.7 past the
/// fold was rejected by its first fresh move, 0.338 against 0.160 K).
pub(super) const JUMP_RATIO_RANGE: (f64, f64) = (0.3, 0.97);

/// The fewest orbits between two of a run's extrapolations: 2 (`Adopted`, decision-r09-t13i's §2.4
/// follow-up 4 §3).
pub(super) const JUMP_SPACING_ORBITS: u32 = 2;

/// How near two successive ratios of a run's moves must agree for it to jump again after a reject:
/// 0.05 (`Adopted`, decision-r09-t13i's §2.4 follow-up 6 §1: the ruling's prototype's stricter
/// trigger, so that a reject mid-transient, as the deep aquaplanet's at orbit 3, does not leave the
/// run's steady tail marching).
pub(super) const JUMP_AGREEMENT: f64 = 0.05;

/// The most jumps a run may have rejected before it only marches: 2 (`Adopted`, decision-r09-t13i's
/// §2.4 follow-up 7 §2: a reject wastes an orbit, and ten of them used up the deep aquaplanet's
/// 40).
pub(super) const MAX_REJECTS: u32 = 2;

/// The bound of G1's probe stop on its surface mean's remainder, K: 0.05 (`Adopted`,
/// decision-r09-t13i's §2.4 follow-up 8 §1). G1 measures d and G2's start, not the climate, so it
/// needs only d's precision: half the climate's bound, so that d's error (at most 0.05 K, moving
/// G2's mean by at most about 0.035 K at `S_g` ÷ `S_Z` ≈ 0.67) and G2's own stop together stay
/// within the climate stop's 0.1 K of the ideal G2.
pub(super) const PROBE_STOP_K: f64 = 0.05;

/// The fewest fresh orbits, counted since a run's start or its last jump that stood, before G1's
/// probe stop may end it: 4 (`Adopted`, decision-r09-t13i's §2.4 follow-up 9 §1: the surface
/// mean's last two ratios need three fresh changes).
pub(super) const PROBE_FRESH_ORBITS: u32 = 4;

/// How near the surface mean's last two ratios must agree for G1's probe stop to read its
/// remainder: 0.05 (`Adopted`, decision-r09-t13i's §2.4 follow-up 9 §1). While a faster component
/// remains, the mean's ratio approaches its slow rate from below (`δ_n` = a `λ_f`ⁿ + b `λ_s`ⁿ
/// gives ratios between `λ_f` and `λ_s`, rising), so two that agree are the sign that the faster
/// part has gone; a ratio at or above the tail's rate then brackets the limit, |limit − T̂| ≤ R̄.
pub(super) const PROBE_AGREEMENT: f64 = 0.05;

/// How far from the mean τ's fit's final probe may lie and still be `Solved`, K: 0.1 (`Adopted`,
/// decision-r09-t13i's §2.4 follow-up §4 (a)): farther, converged or not, the bisection has
/// closed on a jump, and the fit is labelled a fold.
pub(super) const FOLD_MISS_K: f64 = 0.1;

/// The difference of two probes' ice shares, of the sphere, above which a fold is snowball-class:
/// 0.5 (`Adopted`, decision-r09-t13i's §2.4 follow-up §4 (b)). There the fold guard takes the probe
/// whose ice share lies nearer plan 14's surface ice, so that plan 14's history, not a few kelvin
/// of mean, picks the branch.
pub(super) const SNOWBALL_FOLD_ICE_SHARE: f64 = 0.5;

/// Whether a run's moves `moved` and `before`, K, the move of its last orbit's annual means and
/// the one before (`None` before its second move), stop it under the bound `bound_k`, K:
/// [`CONVERGENCE_K`]'s rule. A move of nothing stops it, its remainder being nothing.
#[cfg(test)]
#[must_use]
fn settled(moved: f64, before: Option<f64>, bound_k: f64) -> bool {
    settled_after(moved, before, bound_k, None)
}

/// [`settled`] after a jump that stood, triggered by the ratio `jump_ratio`: the remainder reads
/// r* = max(r, `jump_ratio`), so that the jump's own faster transient is not read as the slow
/// mode's convergence (decision-r09-t13i's §2.4 follow-up 7 §2.3, the prototype's false pass 0.23 K
/// short); with no jump, [`settled`] to the bit.
#[must_use]
fn settled_after(moved: f64, before: Option<f64>, bound_k: f64, jump_ratio: Option<f64>) -> bool {
    // A move that is not a number does not stop the run; a move of nothing does.
    if moved.is_nan() || moved >= bound_k {
        return false;
    }
    if moved <= 0.0 {
        return true;
    }
    let Some(before) = before else {
        return false;
    };
    let ratio = stop_ratio(moved, before, jump_ratio);
    ratio < 1.0 && moved * ratio / (1.0 - ratio) < bound_k
}

/// The stop's ratio r\* of a run's last move `moved` to the one before `before`, K, after a jump
/// that stood triggered by the ratio `jump_ratio`: the fresh ratio, or the jump's where larger
/// (decision-r09-t13i's §2.4 follow-up 7 §2.3), which [`settled_after`] and G1's probe stop
/// ([`probe_end`]) both read.
#[must_use]
fn stop_ratio(moved: f64, before: f64, jump_ratio: Option<f64>) -> f64 {
    let fresh = moved / before;
    match jump_ratio {
        Some(jump) if jump > fresh => jump,
        Some(_) | None => fresh,
    }
}

/// G1's probe of its surface mean's limit (decision-r09-t13i's §2.4 follow-ups 8 §1 and 9 §1): the
/// orbit it read, the mean, its change over that orbit, the mean's own last two ratios and r\*, and
/// from them its remainder and its extrapolated limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MeanProbe {
    /// The orbit it read.
    pub(super) orbits: u32,
    /// `T̄_n`, that orbit's annual field's surface mean, K.
    pub(super) mean_k: f64,
    /// `δ_n`, the mean's change over that orbit, K.
    pub(super) change_k: f64,
    /// (`ρ_n`, `ρ_n−1`) = (`δ_n` ÷ `δ_n−1`, `δ_n−1` ÷ `δ_n−2`), the mean's own last two ratios,
    /// where it had three fresh changes.
    pub(super) mean_ratios: Option<[f64; 2]>,
    /// r\* = max(the stop's ratio, `ρ_n`), below 1 ([`stop_ratio`]; `ρ_n` where it lies in (0, 1)).
    pub(super) ratio: f64,
    /// R̄ = |`δ_n`| r\* ÷ (1 − r\*), K.
    pub(super) remainder_k: f64,
    /// T̂ = `T̄_n` + `δ_n` r\* ÷ (1 − r\*), the mean's extrapolated limit, K.
    pub(super) limit_k: f64,
}

impl MeanProbe {
    /// The probe after orbit `orbits` of surface mean and change `(mean_k, change_k)`, K, with the
    /// mean's ratios `mean_ratios`, at r\* = `ratio`, below 1.
    #[must_use]
    fn of(
        orbits: u32,
        (mean_k, change_k): (f64, f64),
        mean_ratios: Option<[f64; 2]>,
        ratio: f64,
    ) -> Self {
        // The extrapolation's form, the gain r ÷ (1 − r) times the change, as the jump's: d reads
        // T̂ to the bit, so an algebraically equal rewrite (the stop's m r ÷ (1 − r)) moves G2.
        let gain = ratio / (1.0 - ratio);
        Self {
            orbits,
            mean_k,
            change_k,
            mean_ratios,
            ratio,
            remainder_k: change_k.abs() * gain,
            limit_k: mean_k + change_k * gain,
        }
    }
}

/// G1's count of fresh orbits and its surface mean's fresh changes (decision-r09-t13i's §2.4
/// follow-ups 8 §1 and 9 §1): both since the run's start or its last jump that stood, and both as
/// they were at the last jump, which a reject restores.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct MeanTrail {
    /// The orbits since the start or the last jump.
    fresh: u32,
    /// The mean's fresh changes before this orbit, `(δ_n−1, δ_n−2)`, K.
    changes: [Option<f64>; 2],
    /// Both as they were at the last jump.
    at_jump: (u32, [Option<f64>; 2]),
}

impl MeanTrail {
    /// An orbit has run.
    fn orbit(&mut self) {
        self.fresh += 1;
    }

    /// That orbit's mean changed by `change_k`, K, and the run carries on.
    fn push(&mut self, change_k: f64) {
        self.changes = [Some(change_k), self.changes[0]];
    }

    /// The run jumped after that orbit: its count and its changes start again.
    fn jumped(&mut self) {
        self.at_jump = (self.fresh, self.changes);
        (self.fresh, self.changes) = (0, [None; 2]);
    }

    /// The jump was rejected: the count and the changes go back to theirs at it.
    fn rejected(&mut self) {
        (self.fresh, self.changes) = self.at_jump;
    }
}

/// The surface mean's own last two ratios (`ρ_n`, `ρ_n−1`) of its change `change_k` over the last
/// orbit and the fresh changes before it `(δ_n−1, δ_n−2)`, K, where it has both.
#[must_use]
fn mean_ratios(change_k: f64, [last, before]: [Option<f64>; 2]) -> Option<[f64; 2]> {
    let (last, before) = (last?, before?);
    Some([change_k / last, last / before])
}

/// Whether G1's probe stop ends its run after orbit `orbits`, `fresh` orbits since its start or its
/// last jump that stood, its surface mean and its change over that orbit `(mean_k, change_k)` and
/// its fresh changes before `earlier`, K, at the stop's ratio `stop` (`None` before two fresh
/// moves), under the bound `bound_k`, K (decision-r09-t13i's §2.4 follow-up 9 §1): after at least
/// [`PROBE_FRESH_ORBITS`] fresh orbits, where the mean's last two ratios each lie in (0, 1) and agree
/// within [`PROBE_AGREEMENT`], at r\* = max(the stop's ratio, `ρ_n`) below 1 and a remainder R̄ at
/// most the bound; the probe where it does.
#[must_use]
fn probe_end(
    (orbits, fresh): (u32, u32),
    (mean_k, change_k): (f64, f64),
    earlier: [Option<f64>; 2],
    stop: Option<f64>,
    bound_k: f64,
) -> Option<MeanProbe> {
    let (stop, ratios) = (stop?, mean_ratios(change_k, earlier)?);
    let [rho, rho_before] = ratios;
    // By comparison, so that a ratio, a mean or a remainder that is not a number never ends it.
    let within = |ratio: f64| ratio > 0.0 && ratio < 1.0;
    if fresh < PROBE_FRESH_ORBITS
        || !within(rho)
        || !within(rho_before)
        || (rho - rho_before).abs() > PROBE_AGREEMENT
        || stop.is_nan()
        || stop >= 1.0
    {
        return None;
    }
    let ratio = if rho > stop { rho } else { stop };
    let probe = MeanProbe::of(orbits, (mean_k, change_k), Some(ratios), ratio);
    (probe.remainder_k <= bound_k).then_some(probe)
}

/// G1's probe where the climate's stop ended its run first (decision-r09-t13i's §2.4 follow-up 9
/// §1.2), after orbit `orbits`, its surface mean and change `(mean_k, change_k)` and fresh changes
/// before `earlier`, K, at the stop's ratio `stop`: T̂ at r\* = max(the stop's ratio, `ρ_n`), `ρ_n`
/// read only where it lies in (0, 1), and the stop's ratio nothing where a move of nothing stopped
/// the run before it had one, or two moves of nothing made it 0 ÷ 0, its remainder being nothing
/// as [`settled_after`]'s is.
#[must_use]
fn settled_probe(
    orbits: u32,
    (mean_k, change_k): (f64, f64),
    earlier: [Option<f64>; 2],
    stop: Option<f64>,
) -> MeanProbe {
    let ratios = mean_ratios(change_k, earlier);
    // By comparison, so that a ratio that is not a number reads as none.
    let stop = stop.filter(|ratio| *ratio < 1.0).unwrap_or(0.0);
    let ratio = match ratios {
        Some([rho, _]) if rho > stop && rho < 1.0 => rho,
        Some(_) | None => stop,
    };
    MeanProbe::of(orbits, (mean_k, change_k), ratios, ratio)
}

/// A seasonal run's stop (decision-r09-t13i §2.4 and its §2.4 follow-ups 4 and 8).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stop {
    /// Its bound, K ([`settled`]), and the extrapolation's trigger.
    bound_k: f64,
    /// Whether the run extrapolates its slow mode ([`Extrapolation`]).
    jumps: Jumps,
    /// The most orbits it runs: [`MAX_ORBITS`] but in the tests' fixed runs.
    max_orbits: u32,
    /// G1's probe stop's bound on its surface mean's remainder, K ([`probe_end`]), where the run is
    /// G1; `None` for every other run.
    probe_k: Option<f64>,
}

/// The climate's runs' stop: [`CONVERGENCE_K`], extrapolating.
const CLIMATE_STOP: Stop = Stop {
    bound_k: CONVERGENCE_K,
    jumps: Jumps::Allowed,
    max_orbits: MAX_ORBITS,
    probe_k: None,
};

/// G1's stop: the climate's, or its probe stop at [`PROBE_STOP_K`] where that comes first
/// (decision-r09-t13i's §2.4 follow-up 8 §1).
const G1_STOP: Stop = Stop {
    probe_k: Some(PROBE_STOP_K),
    ..CLIMATE_STOP
};

/// The zonal search's runs' stop: [`SEARCH_CONVERGENCE_K`], extrapolating.
const SEARCH_STOP: Stop = Stop {
    bound_k: SEARCH_CONVERGENCE_K,
    jumps: Jumps::Allowed,
    max_orbits: MAX_ORBITS,
    probe_k: None,
};

/// Whether a run may extrapolate its slow mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Jumps {
    /// It may, as every run of the model does (decision-r09-t13i's §2.4 follow-up 4 §3).
    Allowed,
    /// It marches every orbit: the tests' comparison.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the model always extrapolates; the tests compare the marched run"
        )
    )]
    Never,
}

/// The spin-up's extrapolation of one run's slow mode (decision-r09-t13i's §2.4 follow-up 4 §3,
/// as its follow-up 6 §1 amends it; I13's remedy). After an orbit whose stop fails on its
/// remainder, `m_n` r ÷ (1 − r) at least the stop's bound, r = `m_n` ÷ `m_n−1` in
/// [`JUMP_RATIO_RANGE`] (no floor on `m_n` itself), each point's end-of-orbit state and its stored
/// annual mean, the one the next move is taken from, shift by that orbit's annual-mean change times
/// r ÷ (1 − r), the geometric remainder of a mode shrinking by r an orbit, at most once every
/// [`JUMP_SPACING_ORBITS`] orbits and only where two more orbits fit under [`MAX_ORBITS`]. The stop
/// then reads only the moves after the jump, so it needs two fresh ones. If the first of them
/// exceeds `m_n`, the jump is rejected: the state, the stored annual means and the return
/// temperatures go back to theirs before it, and the run jumps again only once two successive
/// ratios agree within [`JUMP_AGREEMENT`], each in the range. The records are always a marched
/// orbit's: a jump moves the next orbit's start, never an orbit's means. On a mixed layer relaxing
/// over years (r about 0.87 an orbit on the reference Earth under the water term) it spares most of
/// the orbits the march takes (decision-r09-t13i's §2.4 follow-up 4 §1).
#[derive(Debug, Clone, PartialEq)]
struct Extrapolation {
    /// Whether the run may jump at all: not where its stop forbids it.
    allowed: bool,
    /// Whether a reject requires two agreeing ratios before the next jump.
    rejected: bool,
    /// The ratio of the last orbit's move to the one before, where both were marched.
    last_ratio: Option<f64>,
    /// The orbit of the run's last jump.
    last_jump: Option<u32>,
    /// What the last jump displaced, until its first fresh move accepts or rejects it.
    pending: Option<Jumped>,
    /// The jumps that stood.
    jumps: u32,
    /// The jumps rejected; at [`MAX_REJECTS`] the run only marches.
    rejects: u32,
    /// The ratio that triggered the last jump that stood, which the stop's remainder reads.
    jump_ratio: Option<f64>,
    /// The ratio that triggered the jump [`gain`](Self::gain) last found due.
    due_ratio: f64,
}

/// What a jump displaced, to restore on a reject.
#[derive(Debug, Clone, PartialEq)]
struct Jumped {
    /// The state at the jump's orbit's end, K.
    state_k: Vec<f64>,
    /// That orbit's annual means, K.
    annual_k: Vec<f64>,
    /// The return temperatures after it.
    returning: Returning,
    /// Its move, `m_n`, K.
    move_k: f64,
    /// The ratio that triggered it.
    ratio: f64,
}

/// What a jump's first fresh move says.
#[derive(Debug, Clone, PartialEq)]
enum Review {
    /// No jump awaited it.
    None,
    /// The move was no larger than the jump's own: the jump stands.
    Accepted,
    /// The move was larger: the displaced state, means and return temperatures, to restore.
    Rejected(Jumped),
}

impl Extrapolation {
    /// A run's extrapolation under `jumps`.
    #[must_use]
    const fn new(jumps: Jumps) -> Self {
        Self {
            allowed: matches!(jumps, Jumps::Allowed),
            rejected: false,
            last_ratio: None,
            last_jump: None,
            pending: None,
            jumps: 0,
            rejects: 0,
            jump_ratio: None,
            due_ratio: 0.0,
        }
    }

    /// The gain r ÷ (1 − r) of a jump due after orbit `orbits`, whose move is `moved` and the move
    /// before `before`, K, under the bound `bound_k`, K, or `None` where none is due; it notes the
    /// orbit's ratio for the agreement a reject requires.
    #[must_use]
    fn gain(&mut self, orbits: u32, moved: f64, before: Option<f64>, bound_k: f64) -> Option<f64> {
        let (low, high) = JUMP_RATIO_RANGE;
        let within = |ratio: f64| ratio > low && ratio < high;
        let ratio = before.map(|before| moved / before);
        let earlier = core::mem::replace(&mut self.last_ratio, ratio);
        if !self.allowed
            || self.rejects >= MAX_REJECTS
            || self.pending.is_some()
            || orbits + 2 > MAX_ORBITS
            || self
                .last_jump
                .is_some_and(|last| orbits < last + JUMP_SPACING_ORBITS)
        {
            return None;
        }
        let ratio = ratio?;
        // By comparison, so that a move or a ratio that is not a number makes no jump.
        if !within(ratio) {
            return None;
        }
        if self.rejected {
            let agreeing = earlier.is_some_and(|earlier| {
                within(earlier) && (ratio - earlier).abs() <= JUMP_AGREEMENT
            });
            if !agreeing {
                return None;
            }
        }
        let gain = ratio / (1.0 - ratio);
        // The stop fails on its remainder: a move at least the bound whose remainder is under it
        // only marches.
        if moved * gain >= bound_k {
            self.due_ratio = ratio;
            Some(gain)
        } else {
            None
        }
    }

    /// Jumps after orbit `orbits`, whose move was `moved`, K, by `gain`: each point's state
    /// `state_k` and stored annual mean `stored_k` shift by its annual mean `annual_k` less the
    /// stored one times `gain`, K; the displaced state, means and return temperatures `returning`
    /// are kept for a reject.
    fn jump(
        &mut self,
        orbits: u32,
        moved: f64,
        gain: f64,
        (state_k, annual_k, stored_k): (&mut [f64], &[f64], &mut [f64]),
        returning: Returning,
    ) {
        self.pending = Some(Jumped {
            state_k: state_k.to_vec(),
            annual_k: annual_k.to_vec(),
            returning,
            move_k: moved,
            ratio: self.due_ratio,
        });
        for ((state, &annual), stored) in state_k.iter_mut().zip(annual_k).zip(stored_k) {
            let shift = (annual - *stored) * gain;
            *state += shift;
            *stored = annual + shift;
        }
        self.last_jump = Some(orbits);
        self.last_ratio = None;
    }

    /// The review of the last jump by its first fresh move `moved`, K: a move larger than the
    /// jump's own, or one that is not a number, rejects it, and the run makes no more.
    #[must_use]
    fn review(&mut self, moved: f64) -> Review {
        let Some(jumped) = self.pending.take() else {
            return Review::None;
        };
        if moved <= jumped.move_k {
            self.jumps += 1;
            self.jump_ratio = Some(jumped.ratio);
            Review::Accepted
        } else {
            self.rejected = true;
            self.rejects += 1;
            Review::Rejected(jumped)
        }
    }
}

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
    /// At a fold, the fold guard's candidates, the cold side's and the warm side's last converged
    /// probes (decision-r09-t13ge §1.4, g4); none elsewhere.
    pub(super) candidates: [Option<Candidate>; 2],
}

/// One of the fold guard's candidates: its emission factor, its steady state's surface mean, K, and
/// its ice share.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Candidate {
    /// The probe's emission factor.
    pub(super) emission_factor: f64,
    /// Its steady state's surface mean, K.
    pub(super) surface_mean_k: f64,
    /// Its steady state's ice share of the sphere.
    pub(super) ice_share: f64,
}

/// A side of the fold guard's bracket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Side {
    /// The cold side's probe.
    Cold,
    /// The warm side's.
    Warm,
}

/// A probe of τ's bisection, and its steady state, which the fold guard keeps where it converged
/// ([`EnergyBalance::fit`]; decision-r09-t13ge §1.4, g4).
#[derive(Debug, Clone, PartialEq)]
struct Probe {
    /// The probe's emission factor.
    emission_factor: f64,
    /// Its radiation.
    radiation: Radiation,
    /// Its steady state's ice share of the sphere ([`ice_share`]).
    ice_share: f64,
    /// Its steady state's surface mean, K: not a number where the steady state is not finite, a
    /// probe the bisection takes as warm (decision-r09-t13i §2.2).
    surface_mean_k: f64,
    /// Its steady state.
    steady: Steady,
}

/// The fold guard's choice between the last converged probe on the cold side of the bracket,
/// `cold`, and on the warm side, `warm`: the one whose surface mean lies nearer `target_k`, K, ties
/// to the warm side (decision-r09-t13ge §1.4, g4); `None` where neither side kept one. At a
/// snowball-class fold, where the two probes' ice shares differ by more than
/// [`SNOWBALL_FOLD_ICE_SHARE`], the one whose ice share lies nearer `target_ice`, plan 14's surface
/// ice share, ties to the nearer mean, then warm (decision-r09-t13i's §2.4 follow-up §4 (b)).
#[must_use]
fn nearer(
    cold: Option<Probe>,
    warm: Option<Probe>,
    target_k: f64,
    target_ice: f64,
) -> Option<Probe> {
    let candidate = |probe: &Probe| Candidate {
        emission_factor: probe.emission_factor,
        surface_mean_k: probe.surface_mean_k,
        ice_share: probe.ice_share,
    };
    let side = nearer_side(
        [cold.as_ref().map(candidate), warm.as_ref().map(candidate)],
        target_k,
        target_ice,
    )?;
    match side {
        Side::Cold => cold,
        Side::Warm => warm,
    }
}

/// [`nearer`]'s rule on the candidates `[cold, warm]`: the side it keeps.
#[must_use]
fn nearer_side(
    [cold, warm]: [Option<Candidate>; 2],
    target_k: f64,
    target_ice: f64,
) -> Option<Side> {
    match (cold, warm) {
        (Some(cold), Some(warm)) => {
            let by_mean = if (warm.surface_mean_k - target_k).abs()
                <= (cold.surface_mean_k - target_k).abs()
            {
                Side::Warm
            } else {
                Side::Cold
            };
            if (cold.ice_share - warm.ice_share).abs() > SNOWBALL_FOLD_ICE_SHARE {
                let (cold_miss, warm_miss) = (
                    (cold.ice_share - target_ice).abs(),
                    (warm.ice_share - target_ice).abs(),
                );
                Some(if cold_miss < warm_miss {
                    Side::Cold
                } else if warm_miss < cold_miss {
                    Side::Warm
                } else {
                    by_mean
                })
            } else {
                Some(by_mean)
            }
        }
        (Some(_), None) => Some(Side::Cold),
        (None, Some(_)) => Some(Side::Warm),
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

/// Watson's exponent, 0.38: the latent heat of vaporisation falls as ((`T_c` − T) ÷ (`T_c` −
/// `T_ref`))^0.38 from its value at `T_ref` (Watson 1943, Ind. Eng. Chem. 35, 398; Poling,
/// Prausnitz and O'Connell 2001, The Properties of Gases and Liquids, 5th ed., §7-12, not
/// re-read; [`latent_heat_j_per_mol`]).
pub(super) const WATSON_EXPONENT: f64 = 0.38;

/// The latent heat of vaporisation h″ − h′ of the condensable of phase diagram `diagram` at
/// `temperature`, J mol⁻¹: Watson's scaling of the enthalpy of vaporisation at its triple point,
/// L = `L_t` ((`T_c` − T) ÷ (`T_c` − `T_t`))^0.38 ([`WATSON_EXPONENT`]), held at `L_t` at and below
/// the triple temperature; `None` at or above the critical temperature, where it falls to zero,
/// and for a temperature that is not a number.
///
/// It is the latent heat itself, which the moist static energy carries, not the curve's
/// Clausius–Clapeyron enthalpy ([`PhaseDiagram::clausius_clapeyron_enthalpy_j_per_mol`]), which
/// exceeds it by the vapour's 1 ÷ (z″ − z′), increasingly away from the triple point: water's by
/// 1.6% at 373 K and 17% at 500 K, methane's by 28% at 150 K. Against IAPWS-95's h″ − h′ (Wagner
/// and Pruss 2002, J. Phys. Chem. Ref. Data 31, 387) water's is about 0.13% low at 288 K (44.37
/// against 44.42 kJ mol⁻¹, 2,465.9 kJ kg⁻¹), 1.5% at 373 K and 4% at 500 K.
#[must_use]
pub(super) fn latent_heat_j_per_mol(diagram: &PhaseDiagram, temperature: Kelvin) -> Option<f64> {
    let t = temperature.value();
    let triple = diagram.triple_temperature().value();
    let critical = diagram.critical_temperature().value();
    if t.is_nan() || t >= critical {
        return None;
    }
    let liquid = if t > triple { t } else { triple };
    Some(
        diagram.triple_vaporisation_enthalpy_j_per_mol()
            * math::powf((critical - liquid) / (critical - triple), WATSON_EXPONENT),
    )
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
    /// Its latent heat of vaporisation, J kg⁻¹: Watson's scaling of its triple point's at the
    /// body's mean surface temperature, over its molar mass ([`Moisture::of`]).
    pub(super) latent_heat_j_per_kg: f64,
    /// Its molar mass over the air's, ε.
    pub(super) mass_ratio: f64,
    /// The surface pressure its specific humidity is taken at, Pa, positive.
    pub(super) surface_pressure_pa: f64,
}

impl Moisture {
    /// The moisture of the body of `inputs` in the air `air`: its climate condensable, where that
    /// is among its surface liquids, has a phase diagram and a molar mass, under a positive surface
    /// pressure; `None` for a dry world.
    ///
    /// Its latent heat is the latent heat of vaporisation at the body's mean surface temperature,
    /// the temperature [`Air::of`]'s `c_p` is taken at, by Watson's scaling of the triple point's
    /// enthalpy on the row's phase diagram ([`latent_heat_j_per_mol`]): the liquid's, whose vapour
    /// the moist static energy carries from the surface liquid it requires, and for a mean below
    /// the triple temperature the triple point's. Water's is 44.37 kJ mol⁻¹ at 288 K,
    /// 2.4628 × 10⁶ J kg⁻¹, about 0.13% below IAPWS-95's 2,465.9 kJ kg⁻¹ (44.42 kJ mol⁻¹; Wagner
    /// and Pruss 2002), and about 4% below it at 500 K. `None` too for a mean at or above the
    /// critical temperature, where the latent heat falls to zero.
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
        let diagram = row.phase_diagram()?;
        let mass = row.molar_mass_g_per_mol()?;
        let latent = latent_heat_j_per_mol(diagram, inputs.mean_surface_temperature())?;
        Some(Self {
            condensable,
            latent_heat_j_per_kg: latent / (mass * 1e-3),
            mass_ratio: mass / air.molar_mass_g_per_mol,
            surface_pressure_pa: pressure,
        })
    }

    /// The specific humidity at the temperature `t_k`, K, and [`RELATIVE_HUMIDITY`]:
    /// q = ε e ÷ (p − (1 − ε) e), e the vapour pressure, its relative humidity of the saturation
    /// pressure but at most the surface pressure, where q is 1.
    #[must_use]
    pub(super) fn specific_humidity(&self, t_k: f64) -> f64 {
        self.humidity_at(t_k, RELATIVE_HUMIDITY)
    }

    /// The saturation specific humidity `q_s` at the temperature `t_k`, K: q at a relative
    /// humidity of 1, the vapour pressure the saturation pressure but at most the surface pressure,
    /// where `q_s` is 1. The sea's evaporation spreads its aerodynamic part by it
    /// (decision-r09-t13i §3.2).
    #[must_use]
    pub(super) fn saturation_specific_humidity(&self, t_k: f64) -> f64 {
        self.humidity_at(t_k, 1.0)
    }

    /// q = ε e ÷ (p − (1 − ε) e) at the temperature `t_k`, K, e the relative humidity
    /// `relative_humidity` of the saturation pressure but at most the surface pressure.
    #[must_use]
    fn humidity_at(&self, t_k: f64, relative_humidity: f64) -> f64 {
        let saturation = saturation_pressure(self.condensable, Kelvin::new(t_k))
            .map_or(0.0, crate::units::Pascals::value);
        let wanted = relative_humidity * saturation;
        let p = self.surface_pressure_pa;
        let e = if wanted < p { wanted } else { p };
        let epsilon = self.mass_ratio;
        epsilon * e / (p - (1.0 - epsilon) * e)
    }
}

/// The column weight of the gas `gas`, of mole fraction `mole_fraction` in the air `air` under the
/// surface pressure `surface_pressure_pa`, Pa: `w_i` = `x_i` (`M_i` ÷ μ) `p_s`, μ the air's mean
/// molar mass, the airborne mass's weight over the surface M g ÷ 4πR² that plan 14's grey law reads
/// (`derive::atmosphere`; decision-r09-t13i's §2.2/§2.3 follow-up 5 §2), for the water term's `τ_d`
/// and the vapour cap alike, so that each is plan 14's own term. On the reference Earth `τ_d` is
/// 0.513 by weight against 0.440 at mole partials (carbon dioxide's term 1.23 times, nitrogen's 0.98).
#[must_use]
fn partial_pressure_pa(gas: Gas, mole_fraction: f64, surface_pressure_pa: f64, air: &Air) -> f64 {
    mole_fraction * (gas.molar_mass_g_per_mol() / air.molar_mass_g_per_mol) * surface_pressure_pa
}

/// One bar, Pa: the unit of the partial pressures plan 14's grey terms take their square roots
/// of (P14.T13.c, `derive::atmosphere`'s τ = Σ k (p ÷ 1 bar)^½).
const BAR_PA: f64 = 1e5;

/// Plan 14's grey greenhouse, as the energy balance reads its water term (R1, decision-r09-t13i
/// §2.2): the constants, and the share of water's saturation pressure, by weight, that the water
/// term's partial pressure takes, for the air `air`.
///
/// P14.T57.a's fits ([`GreenhouseConstants::FITTED`], water 4.47 per √bar) with the water held at
/// [`WATER_SURFACE_HUMIDITY`] of its saturation pressure by mole (Manabe and Wetherald's 0.77,
/// P14.T51.a's partition), so [`WATER_SURFACE_HUMIDITY`] × `M_w` ÷ μ of it by weight, μ the air's
/// mean molar mass: 0.481 on the reference Earth's air (μ 28.82 g mol⁻¹; plan 14's
/// `GREENHOUSE_WATER` documents 0.496 from its own conversion). P14.T54.a's deletion of the
/// snapshot lane's hold left this, the batch's arm, the only one (main's ruling on
/// decision-r09-t13i, 2026-10-20).
#[must_use]
fn greenhouse_in_force(air: &Air) -> (GreenhouseConstants, f64) {
    (
        GreenhouseConstants::FITTED,
        WATER_SURFACE_HUMIDITY * Gas::Water.molar_mass_g_per_mol() / air.molar_mass_g_per_mol,
    )
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
    /// water it has, its column weight `x_w` (`M_w` ÷ μ) `p_s`, and the saturation caps it where the
    /// air is colder than the crossover, the cold trap: `p_w` = min(`h_w` `p_sat`(T), `x_w` (`M_w` ÷
    /// μ) `p_s`), both sides by weight (decision-r09-t13i's §2.4 follow-up 2 §4, with its follow-up
    /// 5 §2). Uncapped, the term grows with T without limit, and the reference Earth with no liquid
    /// ran away to a 53,093 K mean.
    Air {
        /// `x_w` (`M_w` ÷ μ) `p_s`, Pa, the air's water by weight ([`partial_pressure_pa`]).
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
        /// `τ_d`, plan 14's terms of the gases other than water at their column weights
        /// ([`partial_pressure_pa`]): carbon dioxide's and nitrogen's, nitrogen's collision factor at
        /// the body's
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
    /// component, and otherwise the air's own water by weight (decision-r09-t13i's §2.4 follow-up 2
    /// §4 and follow-up 5 §2). A solution's lowering of the vapour pressure is not taken until
    /// P14.T52 makes seas of solutions (follow-up 5 §4 rules its form, `a_w` `h_w` `p_sat`, `a_w` the
    /// solution's water activity).
    ///
    /// `τ_d` takes plan 14's grey terms ([`GreenhouseConstants::terms`]) of the inputs' gases at
    /// their column weights ([`partial_pressure_pa`]), water's left out, and nitrogen's collision
    /// factor at the body's equilibrium
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
                inventory_pa: partial_pressure_pa(Gas::Water, water_fraction, pressure, air),
            }
        };
        let mean_k = inputs.mean_surface_temperature().value();
        let absorbed = (1.0 - albedo_of(freezing, mean_k).albedo) * inputs.host_flux().value()
            / 4.0
            + inputs.heat_flow().value();
        let equilibrium = Kelvin::new((absorbed.max(0.0) / STEFAN_BOLTZMANN).sqrt().sqrt());
        let dry = PartialPressures::from_fn(|gas| match gas {
            Gas::Water => Pascals::ZERO,
            Gas::CarbonDioxide
            | Gas::Nitrogen
            | Gas::Oxygen
            | Gas::Argon
            | Gas::Methane
            | Gas::Ammonia
            | Gas::Hydrogen
            | Gas::Helium => Pascals::new(partial_pressure_pa(gas, fraction(gas), pressure, air)),
        });
        let dry_depth = constants
            .terms(&dry, equilibrium)
            .iter()
            .fold(0.0, |sum, &(_, term)| sum + term);
        let unit = PartialPressures::from_fn(|gas| match gas {
            Gas::Water => Pascals::new(BAR_PA),
            Gas::CarbonDioxide
            | Gas::Nitrogen
            | Gas::Oxygen
            | Gas::Argon
            | Gas::Methane
            | Gas::Ammonia
            | Gas::Hydrogen
            | Gas::Helium => Pascals::ZERO,
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

    /// The multiplier m, or `None` under one τ.
    #[cfg(test)]
    #[must_use]
    pub(super) fn multiplier(&self) -> Option<f64> {
        match *self {
            Self::Grey { .. } => None,
            Self::Vapour { multiplier, .. } => Some(multiplier),
        }
    }
}

/// How the optical depth was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OpticalDepth {
    /// By bisection, so that the steady state's surface mean is the inputs': its final probe
    /// converged within [`FOLD_MISS_K`] of the mean; or, where neither side of the bracket kept a
    /// converged probe, the final probe within it, unconverged, as before the guard
    /// ([`Solution::steady_converged`] says which).
    Solved,
    /// By bisection, but the final probe's surface mean misses the mean by more than
    /// [`FOLD_MISS_K`], converged or not: the bisection closed on a fold, a jump of the steady
    /// states' mean between two branches (under the water term the reference Earth's, between a
    /// snowball near 207 K and its warm branch's end near 290 K). The fold guard took, of the last
    /// converged probes on either side of the bracket and the final probe where it converged, the
    /// one whose surface mean lies nearer the inputs', ties to the warm side (decision-r09-t13ge
    /// §1.4, g4), or at a snowball-class fold, its candidates' ice shares more than
    /// [`SNOWBALL_FOLD_ICE_SHARE`] apart, the one whose ice share lies nearer plan 14's
    /// (decision-r09-t13i's §2.4 follow-up §4); with none, the final probe, unconverged. The mean's
    /// correction and R09.T13.d's constant take up the rest.
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
    /// What the mean's correction did (decision-r09-t13i §2.4).
    pub(super) correction: MeanCorrection,
    /// The kept seasonal run's moves of its annual means between orbits, K, in order (its stop's
    /// record, [`CONVERGENCE_K`]); none for a steady state.
    pub(super) moves_k: Vec<f64>,
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

impl<'a> Ground<'a> {
    /// The ground on the run's grid and on the zonal grid τ is solved on.
    #[must_use]
    fn on_grids(self) -> (GridGround<&'a Relief>, GridGround<&'a Relief>) {
        match self {
            Self::SeaLevel => (GridGround::SeaLevel, GridGround::SeaLevel),
            Self::Relief { grid, zonal } => (GridGround::Relief(grid), GridGround::Relief(zonal)),
        }
    }
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

    /// The quantiles' grounds of a point of heights `heights` under the sea-level temperature
    /// `t_k`, K ([`ground_k`](Self::ground_k)), and their places in `table`: the first alone at a
    /// level point, the rest left nothing.
    #[must_use]
    fn quantiles_at(
        &self,
        t_k: f64,
        heights: &PointHeights,
        table: Option<&Tabulated>,
    ) -> [Quantile; QUANTILES] {
        let count = match heights.shape {
            Shape::Varied => QUANTILES,
            Shape::SeaLevel | Shape::Level => 1,
        };
        let mut quantiles = [Quantile {
            ground_k: 0.0,
            rate: 0.0,
            located: None,
        }; QUANTILES];
        for (quantile, &z) in quantiles.iter_mut().zip(&heights.quantiles_m).take(count) {
            let (ground_k, rate) = self.ground_k(t_k, z);
            *quantile = Quantile {
                ground_k,
                rate,
                located: table.and_then(|table| table.locate(ground_k)),
            };
        }
        quantiles
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
    /// Plan 14's surface ice share of the sphere ([`CoarseInputs::ice_fraction`]), by which τ's fit
    /// picks a snowball-class fold's branch (decision-r09-t13i's §2.4 follow-up §4 (b)).
    pub(super) ice_fraction: f64,
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
    /// Each point's quantiles' grounds at the step's start, K, their slopes in T and their places
    /// in the run's table, computed once a step for the albedo and the emission alike (the first
    /// quantile's alone at a level point).
    quantiles: Vec<[Quantile; QUANTILES]>,
}

/// One quantile's ground at a step's start: its temperature, K, its slope in T (1, or 0 where the
/// floor holds it), and its interval and fraction in the run's table where it holds them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Quantile {
    ground_k: f64,
    rate: f64,
    located: Option<(usize, f64)>,
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
            quantiles: vec![
                [Quantile {
                    ground_k: 0.0,
                    rate: 0.0,
                    located: None,
                }; QUANTILES];
                grid.points()
            ],
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
/// quantile reads at its ground's temperature. An interval is then tabulated only where these meet
/// [`WATER_TABLE_TOLERANCES`] too, so whether h and the albedo are read from the table or directly
/// at a temperature depends on the run's radiation as well as the span (deterministically: each run
/// builds its own table, and the water's reads are keyed to its multiplier's bits).
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
    #[cfg(test)]
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
#[cfg(test)]
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
            ice_fraction: inputs.ice_fraction(),
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
    #[cfg(test)]
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
    #[cfg(test)]
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

    /// [`point_albedo`](Self::point_albedo) from a step's quantiles `quantiles`, their grounds and
    /// places in `table` computed once: the same operations on the same operands, to the bit.
    #[must_use]
    fn located_albedo(
        &self,
        shape: Shape,
        quantiles: &[Quantile; QUANTILES],
        table: Option<&Tabulated>,
    ) -> AlbedoAt {
        let at = |quantile: &Quantile| {
            let albedo = match table.zip(quantile.located) {
                Some((table, (i, f))) => {
                    let (albedo, slope_per_k) = hermite(&table.albedo, i, f);
                    AlbedoAt {
                        albedo,
                        slope_per_k,
                    }
                }
                None => self.albedo_and_slope(quantile.ground_k),
            };
            (albedo.albedo, albedo.slope_per_k * quantile.rate)
        };
        match shape {
            Shape::SeaLevel | Shape::Level => {
                let (albedo, slope_per_k) = at(&quantiles[0]);
                AlbedoAt {
                    albedo,
                    slope_per_k,
                }
            }
            Shape::Varied => {
                let (albedo, slope) = quantiles.iter().fold((0.0, 0.0), |(albedo, slope), q| {
                    let (a, s) = at(q);
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
        ocean: Option<(&OceanGrid, &Returning)>,
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
                    work.quantiles[p] = relief.quantiles_at(v, heights, table);
                    let ground = self.located_albedo(heights.shape, &work.quantiles[p], table);
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
                ) = emission_and_sea(radiation, t_k[p], raised(p), &work.quantiles[p], table);
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
        returning: &Returning,
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
        grid.band_means_into(&work.energy, &mut work.band_energy);
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
                ocean.map(|ocean| (ocean, &Returning::Own)),
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
    /// The fold guard (decision-r09-t13ge §1.4, g4, as decision-r09-t13i's §2.4 follow-up §4 amends
    /// it): the bisection keeps, on each side of its bracket, the last probe whose steady state
    /// converged, the bracket's two ends among them. A final midpoint that converged within
    /// [`FOLD_MISS_K`] of `mean_k` is the answer, [`OpticalDepth::Solved`], to the bit as before.
    /// Otherwise the fit is labelled [`OpticalDepth::Folded`]: a converged final midpoint replaces its
    /// side's kept probe, and the answer is the candidate [`nearer`] picks (the nearer surface mean,
    /// ties to the warm side, or at a snowball-class fold the nearer ice share); where no candidate
    /// converged, the final midpoint, unconverged, labelled `Solved` only within [`FOLD_MISS_K`]. On a
    /// world of one month the steady state is the climate, and a steady state that had not converged
    /// would be a transient between two branches.
    #[must_use]
    fn fit(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
    ) -> Fitted {
        let solve = |factor: f64| self.probe(grid, insolation_w_m2, mean_k, ground, ocean, factor);
        let fitted = |probe: Probe, optical_depth: OpticalDepth| Fitted {
            emission_factor: probe.emission_factor,
            radiation: probe.radiation,
            steady: probe.steady,
            surface_mean_k: probe.surface_mean_k,
            optical_depth,
            candidates: [None; 2],
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
        // A converged final probe within FOLD_MISS_K of the mean is solved; one farther, converged
        // or not, means the bisection closed on a jump (decision-r09-t13i's §2.4 follow-up §4 (a)),
        // and joins its side's candidates if it converged.
        if last.steady.converged && (last.surface_mean_k - mean_k).abs() <= FOLD_MISS_K {
            return fitted(last, OpticalDepth::Solved);
        }
        let last = if last.steady.converged {
            if last.surface_mean_k < mean_k {
                cold = Some(last);
            } else {
                warm = Some(last);
            }
            None
        } else {
            Some(last)
        };
        let candidates = [&cold, &warm].map(|side| {
            side.as_ref().map(|probe| Candidate {
                emission_factor: probe.emission_factor,
                surface_mean_k: probe.surface_mean_k,
                ice_share: probe.ice_share,
            })
        });
        match (nearer(cold, warm, mean_k, self.ice_fraction), last) {
            (Some(probe), _) => Fitted {
                candidates,
                ..fitted(probe, OpticalDepth::Folded)
            },
            // By comparison, so that a mean that is not a number is a fold.
            (None, Some(last)) if (last.surface_mean_k - mean_k).abs() <= FOLD_MISS_K => {
                fitted(last, OpticalDepth::Solved)
            }
            (None, Some(last)) => fitted(last, OpticalDepth::Folded),
            (None, None) => unreachable!("a converged final probe is a candidate"),
        }
    }

    /// The seasonal climate of `run` at the fitted emission factor and radiation `fitted`, corrected
    /// on its surface mean against `mean_k`, K, T\* (decision-r09-t13i's §2.4 follow-up 4 §4; the
    /// water cycle reads the raw records):
    ///
    /// 1. **The zonal search** Z(T\*) on `zonal`, the fit's own zonal model, each run under the
    ///    search's stop ([`SEARCH_STOP`]), recording both stops' flags: Z₀ at the fit's u₀ from its
    ///    zonal steady state `steady_k`, then by [`search`], its first step on the zonal steady
    ///    states' two-sided slope in u at u₀ ± [`CORRECTION_STEP`] (on `fit`'s set-up), as ruled
    ///    before.
    /// 2. **On the zonal grid** (where `run` is the zonal model), the search's answer is the climate,
    ///    its convergence the climate's stop's.
    /// 3. **G1**, a measurement of d and of G2's start (decision-r09-t13i's §2.4 follow-up 8 §1):
    ///    the run at the answer's u from its end state broadcast to every column, with its return
    ///    temperatures, under [`G1_STOP`]: the climate's stop, or its probe stop where that comes
    ///    first, once the surface mean's own last two ratios agree and its remainder is at most
    ///    [`PROBE_STOP_K`] ([`probe_end`], as follow-up 9 §1 amends it).
    /// 4. **G2**, always: where G1's probe measured its mean's extrapolated limit T̂, at u = `u_Z` −
    ///    d ÷ `S_Z`, d = T̂ − `T̄_Z` the geographic run's departure from the zonal one at the same u
    ///    and `S_Z` the search's own slope ([`defect_slope`]), held to the fit's bracket, from G1's
    ///    end plus the zonal predictor's change ([`g2_start`]), under the climate's stop (a defect
    ///    correction, follow-up 6 §2, no second search); where it did not, G1 having reached its
    ///    last orbit under neither stop, run A, at u₀ from the fit's steady state broadcast
    ///    ([`second_run`]).
    /// 5. **Kept** ([`kept_run`]): G2 where it converged; G1 where only it met the climate's stop;
    ///    the nearer where neither did, flagged.
    ///
    /// Where no zonal run's mean is finite, G1 is run A under its own stop. Every run's u is held to
    /// the fit's bracket (the slope's two steady probes at u₀ ± [`CORRECTION_STEP`] are not runs).
    /// The kept run, its emission factor and radiation, and the record of every zonal and
    /// geographic run's u, surface mean, orbits and convergence, G1's probe, and d.
    #[must_use]
    fn corrected(
        &self,
        run: &Seasonal<'_>,
        zonal: &Seasonal<'_>,
        fit: &ZonalFit<'_>,
        mean_k: f64,
        fitted: (f64, Radiation),
        steady_k: &[f64],
    ) -> (Orbited, f64, Radiation, MeanCorrection) {
        let at = FitAt {
            u0: math::ln(fitted.0),
            factor: fitted.0,
            radiation: fitted.1,
            greenhouse: self.greenhouse,
            mean_k,
        };
        let mut zonal_search = ZonalSearch::begin(self, zonal, fit, at, steady_k);
        let answer = zonal_search.toward(mean_k, SEARCH_RUNS - 1);
        if run.grid.columns() == 1 {
            return zonal_search.into_climate(answer);
        }
        let geographic = |u: f64, start: (Vec<f64>, Start), stop: Stop| {
            let (x, radiation) = at.at(u);
            let orbited = self.orbits(
                run.grid,
                run.sunlight,
                &radiation,
                start,
                run.sea_share,
                run.ground,
                run.ocean,
                stop,
            );
            let record = CorrectionRun::of(u, run, &orbited);
            (orbited, x, radiation, record)
        };
        let steady = || (run.grid.broadcast(steady_k), Start::Steady);
        let first = match answer {
            Some(i) => {
                let (record, from) = &zonal_search.runs[i];
                geographic(
                    record.run.u,
                    (
                        run.grid.broadcast(&from.final_k),
                        Start::Continued(from.returning.clone()),
                    ),
                    G1_STOP,
                )
            }
            None => geographic(at.u0, steady(), G1_STOP),
        };
        let (g1, g1_probe) = (first.3, first.0.mean_probe);
        let search = zonal_search.records();
        let (second, defect, g2_start_k) = match second_run(answer, &search, g1_probe) {
            SecondRun::Defect {
                answer: i,
                defect_k,
            } => {
                let zonal_answer = zonal_search.runs[i].0.run;
                let slope = zonal_search.defect_slope();
                let u = (zonal_answer.u - defect_k / slope.slope_k_per_u)
                    .clamp(0.0, math::ln(EMISSION_FACTOR_MAX));
                let (state, returning, begun) = g2_start(
                    run.grid,
                    &first.0,
                    &zonal_search.runs,
                    slope.pair,
                    (zonal_answer.u, u),
                );
                let start_k = surface_mean(run.ground, run.grid, &state);
                let (orbited, x, radiation, record) =
                    geographic(u, (state, Start::Continued(returning)), CLIMATE_STOP);
                let record = CorrectionRun {
                    start: begun,
                    ..record
                };
                (
                    (orbited, x, radiation, record),
                    Some((defect_k, slope)),
                    Some(start_k),
                )
            }
            SecondRun::RunA => (geographic(at.u0, steady(), CLIMATE_STOP), None, None),
        };
        let g2 = second.3;
        let kept = kept_run(&g1, &g2, mean_k);
        let correction = MeanCorrection::Searched {
            search,
            answer,
            g1,
            g1_probe: g1_probe.map(Box::new),
            defect,
            g2_start_k,
            g2,
            kept,
        };
        match kept {
            KeptRun::G2 => (second.0, second.1, second.2, correction),
            KeptRun::G1 => (first.0, first.1, first.2, correction),
        }
    }

    /// The seasonal run `run` continued from the last state of `from`, whose return temperatures
    /// it carries on, at u = ln x, x the emission factor at `mean_k`, K ([`Radiation::at`]), under
    /// the stop `stop`: the run, its emission factor and radiation, and its record.
    #[cfg(test)]
    #[must_use]
    fn continued(
        &self,
        run: &Seasonal<'_>,
        from: &Orbited,
        u: f64,
        mean_k: f64,
        stop: Stop,
    ) -> (Orbited, f64, Radiation, CorrectionRun) {
        let factor = math::exp(u);
        let radiation = Radiation::at(self.greenhouse, factor, mean_k);
        let orbited = self.orbits(
            run.grid,
            run.sunlight,
            &radiation,
            (
                from.final_k.clone(),
                Start::Continued(from.returning.clone()),
            ),
            run.sea_share,
            run.ground,
            run.ocean,
            stop,
        );
        let record = CorrectionRun::of(u, run, &orbited);
        (orbited, factor, radiation, record)
    }

    /// One probe of τ's solve: the annual-mean steady state under `insolation_w_m2` over the
    /// ground `ground` with the sea's transport `ocean`, from an isothermal start at `mean_k`, K,
    /// at the emission factor `emission_factor` at `mean_k` ([`Radiation::at`]), and its surface
    /// mean ([`Relief::surface_mean_k`]).
    #[must_use]
    fn probe(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
        emission_factor: f64,
    ) -> Probe {
        let radiation = Radiation::at(self.greenhouse, emission_factor, mean_k);
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
            emission_factor,
            radiation,
            ice_share: ice_share(self.freezing, grid, ground, &steady.t_k),
            surface_mean_k: surface,
            steady,
        }
    }

    /// Whole seasonal orbits of `sunlight` under the radiation `radiation` from `start_k`, K,
    /// begun as `start` says, over points of sea share `sea_share` and the ground `ground`, with
    /// the sea's transport `ocean`, at least [`MIN_ORBITS`] and at most [`MAX_ORBITS`], until the
    /// stop `stop` holds ([`settled`] at its bound), its slow mode extrapolated where it allows
    /// ([`Extrapolation`]): the last orbit's annual and monthly means, its ocean record, and the
    /// state at its end. A run whose state is no longer finite stops, unconverged. The table
    /// reaches as far below and above the start as the highest and deepest quantiles' grounds
    /// need.
    ///
    /// G1's stop also ends it at its probe stop ([`probe_end`]), whichever comes first, the probe
    /// reading the same orbit's moves and ratio as the stop, after them; G1's run records its
    /// probe at the orbit it ended, by either stop (decision-r09-t13i's §2.4 follow-up 8 §1).
    ///
    /// The Ekman cells' return temperature is the state's own in a steady start's first orbit,
    /// and after each orbit that orbit's coldest month's band-mean sea temperature at the return
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
        (start_k, start): (Vec<f64>, Start),
        sea_share: &[f64],
        ground: GridGround<&Relief>,
        ocean: Option<&OceanGrid>,
        stop: Stop,
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
        let start_k = if start == Start::Steady && sunlight.varies_in_longitude() {
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
        let begun = match start {
            Start::Steady => RunStart::Steady,
            Start::Continued(_) => RunStart::Continued,
        };
        let mut returning = match start {
            Start::Steady => Returning::Own,
            Start::Continued(returning) => returning,
        };
        #[cfg(test)]
        let started = (t.clone(), returning.clone());
        let mut before: Option<f64> = None;
        let mut history = Vec::new();
        let mut water = vec![0.0; points];
        let mut band_sea = vec![0.0; LATITUDE_BANDS];
        let mut orbits = 0;
        // Whether the last orbit met the climate's stop and the search's (decision-r09-t13i's
        // §2.4 follow-up 6 §3): a climate reads the first, the search's secant the second.
        let mut met = (false, false);
        let mut extrapolation = Extrapolation::new(stop.jumps);
        // G1's fresh orbits and its surface mean's fresh changes, and its probe where its run
        // ended (follow-ups 8 §1 and 9 §1).
        let mut trail = MeanTrail::default();
        let mut mean_probe = None;
        while orbits < stop.max_orbits {
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
                    ocean.map(|ocean| (ocean, &returning)),
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
            trail.orbit();
            let n = f64::from(steps);
            for sum in &mut annual {
                *sum /= n;
            }
            if let Some(ocean) = ocean {
                returning = Returning::Coldest(Box::new(coldest_return_k(
                    grid,
                    ocean,
                    at,
                    &months,
                    &counts,
                    &mut water,
                    &mut band_sea,
                )));
            }
            if !annual.iter().all(|t| t.is_finite()) {
                break;
            }
            let moved = previous.as_ref().map(|last| {
                annual.iter().zip(last).fold(0.0, |most: f64, (a, b)| {
                    let d = (a - b).abs();
                    if d > most { d } else { most }
                })
            });
            if let Some(moved) = moved {
                history.push(moved);
                match extrapolation.review(moved) {
                    Review::Rejected(jumped) => {
                        t.copy_from_slice(&jumped.state_k);
                        returning = jumped.returning;
                        before = Some(jumped.move_k);
                        previous = Some(jumped.annual_k);
                        met = (false, false);
                        trail.rejected();
                        continue;
                    }
                    // The stop reads only the moves after a jump.
                    Review::Accepted => before = None,
                    Review::None => {}
                }
                let enough = orbits >= MIN_ORBITS;
                let jump_ratio = extrapolation.jump_ratio;
                met = (
                    enough && settled_after(moved, before, CONVERGENCE_K, jump_ratio),
                    enough && settled_after(moved, before, SEARCH_CONVERGENCE_K, jump_ratio),
                );
                // G1's probe reads the surface mean of this orbit and its change from the last
                // (follow-ups 8 §1 and 9 §1).
                let probed = stop
                    .probe_k
                    .zip(previous.as_deref())
                    .map(|(bound_k, last)| {
                        let mean = surface_mean(ground, grid, &annual);
                        (bound_k, (mean, mean - surface_mean(ground, grid, last)))
                    });
                let ratio = before.map(|before| stop_ratio(moved, before, jump_ratio));
                if enough && settled_after(moved, before, stop.bound_k, jump_ratio) {
                    mean_probe =
                        probed.map(|(_, means)| settled_probe(orbits, means, trail.changes, ratio));
                    break;
                }
                if let Some((bound_k, means)) = probed {
                    let fresh = (orbits, trail.fresh);
                    if let Some(probe) = probe_end(fresh, means, trail.changes, ratio, bound_k) {
                        mean_probe = Some(probe);
                        break;
                    }
                    trail.push(means.1);
                }
                if let (Some(gain), Some(stored)) = (
                    extrapolation.gain(orbits, moved, before, stop.bound_k),
                    previous.as_mut(),
                ) {
                    extrapolation.jump(
                        orbits,
                        moved,
                        gain,
                        (&mut t, &annual, stored),
                        returning.clone(),
                    );
                    before = None;
                    trail.jumped();
                    continue;
                }
                before = Some(moved);
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
                converged: met.0,
            },
            ocean: recorder.map(|recorder| recorder.finish(&counts, steps)),
            final_k: t,
            returning,
            moves_k: history,
            searched: met.1,
            jumps: extrapolation.jumps,
            rejects: extrapolation.rejects,
            start: begun,
            mean_probe,
            #[cfg(test)]
            started,
        }
    }

    /// The zonal search's first run, Z₀, that [`run`](Self::run) makes over the same zonal set-up
    /// (`grid` the zonal grid), of a world that is lit, continued from its last state at u = `u`
    /// under the stop (decision-r09-t13i's §2.4 follow-up 2 §3, as its follow-up 4 §7 restates it:
    /// the sea-level Earth at the relief Earth's kept u, so that the relief's comparison holds the
    /// radiation fixed).
    ///
    /// # Panics
    ///
    /// If `sunlight` is constant.
    #[cfg(test)]
    #[expect(
        clippy::too_many_arguments,
        reason = "run's own set-up, and the u to continue at"
    )]
    #[must_use]
    pub(super) fn run_continued(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        mean_k: f64,
        sea_share: &[f64],
        ground: Ground<'_>,
        currents: Option<&Currents>,
        u: f64,
    ) -> ContinuedRun {
        let (on_grid, on_zonal) = ground.on_grids();
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let ocean = currents.map(|currents| OceanGrid::new(grid, sea_share.to_vec(), currents));
        let zonal_ocean =
            currents.map(|currents| OceanGrid::new(&zonal, grid.band_means(sea_share), currents));
        let fitted = self.fit(&zonal, &annual, mean_k, on_zonal, zonal_ocean.as_ref());
        let run = Seasonal {
            grid,
            sunlight,
            sea_share,
            ground: on_grid,
            ocean: ocean.as_ref(),
        };
        let first = self.orbits(
            grid,
            sunlight,
            &fitted.radiation,
            (grid.broadcast(&fitted.steady.t_k), Start::Steady),
            sea_share,
            on_grid,
            ocean.as_ref(),
            SEARCH_STOP,
        );
        let a = CorrectionRun::of(math::ln(fitted.emission_factor), &run, &first);
        let (orbited, _, _, continued) = self.continued(&run, &first, u, mean_k, CLIMATE_STOP);
        ContinuedRun {
            a,
            continued,
            climate: orbited.climate,
        }
    }

    /// One seasonal run at the fit's u from its steady state broadcast, over [`run`](Self::run)'s
    /// set-up, of a world that is lit, with the extrapolation as `jumps` says: under the climate's
    /// stop, or for exactly `orbits` orbits where it is given (decision-r09-t13i's §2.4 follow-up 4
    /// §7, the extrapolation against the march; its follow-up 7 §3, the geographic model against
    /// the zonal one).
    #[cfg(test)]
    #[expect(
        clippy::too_many_arguments,
        reason = "run's own set-up, and whether the run may jump"
    )]
    #[must_use]
    pub(super) fn run_at_fit(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        mean_k: f64,
        sea_share: &[f64],
        ground: Ground<'_>,
        currents: Option<&Currents>,
        jumps: Jumps,
        orbits: Option<u32>,
    ) -> GridClimate {
        let (on_grid, on_zonal) = ground.on_grids();
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let ocean = currents.map(|currents| OceanGrid::new(grid, sea_share.to_vec(), currents));
        let zonal_ocean =
            currents.map(|currents| OceanGrid::new(&zonal, grid.band_means(sea_share), currents));
        let fitted = self.fit(&zonal, &annual, mean_k, on_zonal, zonal_ocean.as_ref());
        self.orbits(
            grid,
            sunlight,
            &fitted.radiation,
            (grid.broadcast(&fitted.steady.t_k), Start::Steady),
            sea_share,
            on_grid,
            ocean.as_ref(),
            Stop {
                // A bound of nothing never stops the run.
                bound_k: if orbits.is_some() { 0.0 } else { CONVERGENCE_K },
                jumps,
                max_orbits: orbits.unwrap_or(MAX_ORBITS),
                probe_k: None,
            },
        )
        .climate
    }

    /// The zonal search [`run`](Self::run) makes over the zonal set-up of `grid`, of a world that
    /// is lit and whose τ's fit is solved or a fold, with each run's start and end states and
    /// return temperatures (decision-r09-t13i's §2.4 follow-up 8 §2, the search's test of its
    /// predicted starts).
    #[cfg(test)]
    #[must_use]
    pub(super) fn search_runs(
        &self,
        grid: &ModelGrid,
        sunlight: &Sunlight,
        mean_k: f64,
        sea_share: &[f64],
        ground: Ground<'_>,
        currents: Option<&Currents>,
    ) -> Vec<SearchState> {
        let (_, on_zonal) = ground.on_grids();
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let zonal_sea = grid.band_means(sea_share);
        let zonal_ocean =
            currents.map(|currents| OceanGrid::new(&zonal, zonal_sea.clone(), currents));
        let fitted = self.fit(&zonal, &annual, mean_k, on_zonal, zonal_ocean.as_ref());
        let banded = sunlight.band_means(grid);
        let zonal_run = Seasonal {
            grid: &zonal,
            sunlight: if grid.columns() == 1 {
                sunlight
            } else {
                &banded
            },
            sea_share: &zonal_sea,
            ground: on_zonal,
            ocean: zonal_ocean.as_ref(),
        };
        let fit = ZonalFit {
            insolation_w_m2: &annual,
            ground: on_zonal,
            ocean: zonal_ocean.as_ref(),
        };
        let at = FitAt {
            u0: math::ln(fitted.emission_factor),
            factor: fitted.emission_factor,
            radiation: fitted.radiation,
            greenhouse: self.greenhouse,
            mean_k,
        };
        let mut search = ZonalSearch::begin(self, &zonal_run, &fit, at, &fitted.steady.t_k);
        let _answer = search.toward(mean_k, SEARCH_RUNS - 1);
        search
            .runs
            .into_iter()
            .map(|(record, orbited)| SearchState {
                record,
                start_k: orbited.started.0,
                start_returning: orbited.started.1,
                final_k: orbited.final_k,
                final_returning: orbited.returning,
            })
            .collect()
    }

    /// The Ekman cells of the zonal `climate` over sea shares `sea_share` with `currents` at band
    /// edge `edge` ([`OceanGrid::ekman_at`]), from its annual-mean h and sea temperatures and its
    /// coldest month's: ψ, kg s⁻¹ per 2πR², and `ΔT_D`, K. A print's, at the annual mean.
    #[cfg(test)]
    #[must_use]
    pub(super) fn ekman_at(
        &self,
        sea_share: &[f64],
        currents: &Currents,
        climate: &GridClimate,
        edge: usize,
    ) -> Option<(f64, f64)> {
        let grid = ModelGrid::zonal();
        let ocean = OceanGrid::new(&grid, sea_share.to_vec(), currents);
        let energy: Vec<f64> = climate
            .annual_k
            .iter()
            .map(|&t| self.energies(t, None)[0])
            .collect();
        let mut sea = vec![0.0; LATITUDE_BANDS];
        ocean.band_sea_k(&grid, &climate.annual_k, &mut sea);
        let mut coldest = [f64::INFINITY; LATITUDE_BANDS];
        let mut month = vec![0.0; LATITUDE_BANDS];
        for means in &climate.months_k {
            ocean.band_sea_k(&grid, means, &mut month);
            for (least, &t) in coldest.iter_mut().zip(&month) {
                if t < *least {
                    *least = t;
                }
            }
        }
        ocean.ekman_at(
            &grid,
            self.transport_kg_m2_s,
            &energy,
            &sea,
            &Returning::Coldest(Box::new(coldest)),
            edge,
        )
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
        self.couple_ocean(grid, ocean, &Returning::Own, t_k, at, &mut work);
        let mut recorder = Recorder::new(grid.points(), 1);
        recorder.add(grid, &work, 0);
        recorder.finish(&[1], 1)
    }

    /// The model's run over `sunlight` for a body of mean surface temperature `mean_k`, over the
    /// points of `grid` of sea share `sea_share` and the ground `ground`, with the sea's transport
    /// `currents` (the [module documentation](self)): τ, or under the water term its multiplier,
    /// solved on the zonal grid in the grid's coordinates, over the zonal relief and its bands' sea
    /// shares, so that the steady state's surface mean is `mean_k`, then the orbits over the grid's
    /// own relief and sea, the mean corrected by the zonal search ([`solved`](Self::solved)).
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
        let (on_grid, on_zonal) = ground.on_grids();
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let forcing = zonal.mean(&annual) + self.heat_flow_w_m2;
        let zonal_sea = grid.band_means(sea_share);
        let ocean = currents.map(|currents| OceanGrid::new(grid, sea_share.to_vec(), currents));
        let zonal_ocean =
            currents.map(|currents| OceanGrid::new(&zonal, zonal_sea.clone(), currents));
        let solution = if positive(forcing) {
            let fitted = self.fit(&zonal, &annual, mean_k, on_zonal, zonal_ocean.as_ref());
            // The zonal search reads each step's band mean of the grid's sunlight.
            let banded;
            let zonal_sunlight = if grid.columns() == 1 {
                sunlight
            } else {
                banded = sunlight.band_means(grid);
                &banded
            };
            let run = Seasonal {
                grid,
                sunlight,
                sea_share,
                ground: on_grid,
                ocean: ocean.as_ref(),
            };
            let zonal_run = Seasonal {
                grid: &zonal,
                sunlight: zonal_sunlight,
                sea_share: &zonal_sea,
                ground: on_zonal,
                ocean: zonal_ocean.as_ref(),
            };
            let zonal_fit = ZonalFit {
                insolation_w_m2: &annual,
                ground: on_zonal,
                ocean: zonal_ocean.as_ref(),
            };
            self.solved(&run, &zonal_run, &zonal_fit, mean_k, fitted)
        } else {
            unforced(grid, sunlight, mean_k)
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

    /// The answer of `run` for a body of mean surface temperature `mean_k`, K, τ's fit `fitted` made
    /// on `fit`'s zonal set-up: in a one-month year the steady state itself; in a twelve-month one
    /// the seasonal climate the correction keeps ([`corrected`](Self::corrected), on the zonal model
    /// `zonal`), or, where the fit was clamped, the run at its u from its steady state.
    #[must_use]
    fn solved(
        &self,
        run: &Seasonal<'_>,
        zonal: &Seasonal<'_>,
        fit: &ZonalFit<'_>,
        mean_k: f64,
        fitted: Fitted,
    ) -> Solution {
        let Fitted {
            emission_factor: factor,
            radiation,
            steady,
            surface_mean_k,
            optical_depth: depth,
            candidates: _,
        } = fitted;
        let converged = steady.converged;
        let broadcast = run.grid.broadcast(&steady.t_k);
        let (climate, record, factor, radiation, correction, moves) = if run.sunlight.steps() == 0 {
            let record = run.ocean.filter(|ocean| ocean.any()).map(|ocean| {
                self.steady_ocean_record(run.grid, &radiation, &broadcast, run.ground, ocean)
            });
            let climate = GridClimate {
                annual_k: broadcast.clone(),
                months_k: vec![broadcast; usize::from(run.sunlight.months())],
                orbits: 0,
                converged,
            };
            (
                climate,
                record,
                factor,
                radiation,
                MeanCorrection::None,
                Vec::new(),
            )
        } else {
            let (kept, factor, radiation, correction) = match depth {
                OpticalDepth::Solved | OpticalDepth::Folded => {
                    self.corrected(run, zonal, fit, mean_k, (factor, radiation), &steady.t_k)
                }
                OpticalDepth::Transparent | OpticalDepth::Opaque | OpticalDepth::Unforced => {
                    let first = self.orbits(
                        run.grid,
                        run.sunlight,
                        &radiation,
                        (broadcast, Start::Steady),
                        run.sea_share,
                        run.ground,
                        run.ocean,
                        CLIMATE_STOP,
                    );
                    (first, factor, radiation, MeanCorrection::Clamped)
                }
            };
            (
                kept.climate,
                kept.ocean,
                factor,
                radiation,
                correction,
                kept.moves_k,
            )
        };
        Solution {
            climate,
            emission_factor: factor,
            radiation,
            optical_depth: depth,
            steady_converged: converged,
            steady_surface_mean_k: surface_mean_k,
            month_insolation_w_m2: run.sunlight.month_means(run.grid),
            ocean: record,
            correction,
            moves_k: moves,
        }
    }
}

/// The zonal search's Z₀ continued at another u (decision-r09-t13i's §2.4 follow-up 2 §3).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ContinuedRun {
    /// Z₀'s record.
    pub(super) a: CorrectionRun,
    /// The continued run's u and surface mean.
    pub(super) continued: CorrectionRun,
    /// The continued run's climate.
    pub(super) climate: GridClimate,
}

/// One run of the zonal search with the states it began and ended in
/// ([`EnergyBalance::search_runs`]).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SearchState {
    /// Its record.
    pub(super) record: SearchRun,
    /// The state it began from, K.
    pub(super) start_k: Vec<f64>,
    /// The return temperatures it began with.
    pub(super) start_returning: Returning,
    /// The state at its last orbit's end, K.
    pub(super) final_k: Vec<f64>,
    /// The return temperatures after its last orbit.
    pub(super) final_returning: Returning,
}

/// The answer for a body with neither light nor heat over the points of `grid` under `sunlight`:
/// isothermal at `mean_k`, K, at no greenhouse.
#[must_use]
fn unforced(grid: &ModelGrid, sunlight: &Sunlight, mean_k: f64) -> Solution {
    let isothermal = vec![mean_k; grid.points()];
    Solution {
        climate: GridClimate {
            annual_k: isothermal.clone(),
            months_k: vec![isothermal; usize::from(sunlight.months())],
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
        correction: MeanCorrection::None,
        moves_k: Vec::new(),
    }
}

/// The ice share of the steady state `t_k`, K, one a point of `grid`, over the ground `ground`, where
/// the ice forms as `freezing` says: the solid angle whose ground, each quantile's T − Γz, lies below
/// the albedo's midpoint `T_f` − [`ALBEDO_MIDPOINT_BELOW_FREEZING_K`], over the sphere's; none where
/// no ice forms (decision-r09-t13i's §2.4 follow-up §4 (b)).
#[must_use]
fn ice_share(
    freezing: Freezing,
    grid: &ModelGrid,
    ground: GridGround<&Relief>,
    t_k: &[f64],
) -> f64 {
    let Freezing::At { point_k } = freezing else {
        return 0.0;
    };
    let midpoint = point_k - ALBEDO_MIDPOINT_BELOW_FREEZING_K;
    let iced: Vec<f64> = t_k
        .iter()
        .enumerate()
        .map(|(p, &t)| match ground {
            GridGround::SeaLevel => f64::from(u8::from(t < midpoint)),
            GridGround::Relief(relief) => {
                relief
                    .hypsometry()
                    .point(p)
                    .quantiles_m
                    .iter()
                    .fold(0.0, |share, &z| {
                        if relief.ground_k(t, z).0 < midpoint {
                            share + QUANTILE_WEIGHT
                        } else {
                            share
                        }
                    })
            }
        })
        .collect();
    grid.mean(&iced)
}

/// The surface mean of the temperatures `t_k`, K, one a point of `grid`, over the ground `ground`
/// ([`Relief::surface_mean_k`]; [`ModelGrid::mean`] at the sea level).
#[must_use]
fn surface_mean(ground: GridGround<&Relief>, grid: &ModelGrid, t_k: &[f64]) -> f64 {
    match ground {
        GridGround::Relief(relief) => relief.surface_mean_k(grid, t_k),
        GridGround::SeaLevel => grid.mean(t_k),
    }
}

/// The run kept of the runs `runs` against the surface mean `target_k`, K (decision-r09-t13i's
/// §2.4 follow-up 3, as its follow-up 4 keeps it): the converged run nearest the target, the earlier
/// on a tie; where none converged, the nearest run, the earlier on a tie, its climate flagged
/// unconverged; `None` where no run's mean is finite. An unconverged run's mean is not a climate:
/// on the continent aquaplanet the old secant's run C, at 40 orbits still falling about 0.1 K an
/// orbit, lay 1.67 K from the mean against the converged B's 2.45 K, and by its moves would have
/// ended 3.0–3.6 K from it.
#[must_use]
fn kept_index(runs: &[CorrectionRun], target_k: f64) -> Option<usize> {
    let any_converged = runs
        .iter()
        .any(|run| run.converged && run.mean_k.is_finite());
    let mut kept: Option<(usize, f64)> = None;
    for (i, run) in runs.iter().enumerate() {
        if any_converged && !run.converged {
            continue;
        }
        let miss = (run.mean_k - target_k).abs();
        // A run whose mean is not finite is never kept, and a later run only where strictly
        // nearer.
        if !miss.is_finite() {
            continue;
        }
        if kept.is_none_or(|(_, best)| miss < best) {
            kept = Some((i, miss));
        }
    }
    kept.map(|(i, _)| i)
}

/// The run the correction keeps of G1 `g1` and G2 `g2` against the surface mean `target_k`, K
/// (decision-r09-t13i's §2.4 follow-up 8 §1.3): G2 where it converged; G1 where only it met the
/// climate's stop (follow-up 3's fallback); where neither did, the nearer, the earlier on a tie,
/// its climate flagged unconverged ([`kept_index`]), and G2 where neither mean is finite.
#[must_use]
fn kept_run(g1: &CorrectionRun, g2: &CorrectionRun, target_k: f64) -> KeptRun {
    if g2.converged {
        KeptRun::G2
    } else if g1.converged {
        KeptRun::G1
    } else {
        match kept_index(&[*g1, *g2], target_k) {
            Some(0) => KeptRun::G1,
            Some(_) | None => KeptRun::G2,
        }
    }
}

/// G2's run (decision-r09-t13i's §2.4 follow-up 8 §1.2–§1.4).
#[derive(Debug, Clone, Copy, PartialEq)]
enum SecondRun {
    /// The defect's step from the zonal search's answer, run `answer` of its runs, d = `defect_k`,
    /// K.
    Defect {
        /// The answer's index among the search's runs.
        answer: usize,
        /// d = T̂ − `T̄_Z`, K.
        defect_k: f64,
    },
    /// Run A, at u₀ from the fit's steady state broadcast.
    RunA,
}

/// G2's run after the zonal search's runs `search`, its answer `answer`, and G1's probe `probe`:
/// the defect's step, d = T̂ − `T̄_Z` with T̂ the probe's extrapolated limit, where G1 measured it
/// and the search has an answer; run A where G1 measured no d, having reached its last orbit
/// under neither of its stops or its state having stopped being finite, or where no zonal run's
/// mean is finite (decision-r09-t13i's §2.4 follow-up 8 §1.4).
#[must_use]
fn second_run(answer: Option<usize>, search: &[SearchRun], probe: Option<MeanProbe>) -> SecondRun {
    match (answer, probe) {
        (Some(i), Some(probe)) => SecondRun::Defect {
            answer: i,
            defect_k: probe.limit_k - search[i].run.mean_k,
        },
        (Some(_), None) | (None, _) => SecondRun::RunA,
    }
}

/// One zonal search toward the surface mean `target_k`, K, Z(target) (decision-r09-t13i's §2.4
/// follow-up 4 §4, as its follow-ups 6 and 7 amend it), over the runs so far `runs`, each its record
/// and its state: until one of its runs that met the search's stop lies within
/// [`SEARCH_CONVERGENCE_K`] of the target, or `limit` more runs, each `run(u, step, runs)`, a run at
/// u started as its step `step` says, its u chosen by [`next_search_step`] under the zonal steady
/// states' slope `slope_k`, K per unit of u, within [0, `bracket`]. Its answer: the run nearest the target
/// that met the climate's stop, the earlier on a tie, or the nearest where none did
/// ([`kept_index`]); `None` where no run's mean is finite.
#[must_use]
fn search<S>(
    runs: &mut Vec<(SearchRun, S)>,
    target_k: f64,
    slope_k: f64,
    bracket: f64,
    limit: usize,
    run: &impl Fn(f64, &SearchStep, &[(SearchRun, S)]) -> (CorrectionRun, S),
) -> Option<usize> {
    let records = |runs: &[(SearchRun, S)]| -> Vec<CorrectionRun> {
        runs.iter().map(|(record, _)| record.run).collect()
    };
    for _ in 0..limit {
        let within = runs.iter().any(|(record, _)| {
            record.run.searched && (record.run.mean_k - target_k).abs() <= SEARCH_CONVERGENCE_K
        });
        if within {
            break;
        }
        let Some((u, step, _)) = next_search_step(&records(runs), target_k, slope_k, bracket)
        else {
            break;
        };
        let (record, state) = run(u, &step, runs);
        runs.push((
            SearchRun {
                run: record,
                target_k,
                step,
            },
            state,
        ));
    }
    kept_index(&records(runs), target_k)
}

/// The zonal search's next u toward the surface mean `target_k`, K, after the runs `runs`, how it
/// was chosen, and the run it continues from (decision-r09-t13i's §2.4 follow-up 4 §4, as its
/// follow-up 6 §3 amends it), held to [0, `bracket`]; `None` where no run met the climate's stop,
/// the slope `slope_k` (K per unit of u) is not positive where the step needs it, or the secant's
/// two points have one mean:
///
/// - where the latest run did not meet the climate's stop, halfway from the nearest run that did to
///   its u (such a run is never a secant point);
/// - else, with two runs or more that met the search's stop, the secant on the latest two, held
///   inside them where they straddle the target and otherwise to [`SECANT_STEP_CAP`] times the step
///   between them;
/// - else, from the nearest run that met the climate's stop, the slope's step, (target − T̄) ÷ S
///   (so Z₁ may read a Z₀ that met only the climate's stop).
///
/// Each continues from the nearest run that met the climate's stop.
#[must_use]
fn next_search_step(
    runs: &[CorrectionRun],
    target_k: f64,
    slope_k: f64,
    bracket: f64,
) -> Option<(f64, SearchStep, usize)> {
    let miss = |i: usize| (runs[i].mean_k - target_k).abs();
    let nearest = (0..runs.len())
        .filter(|&i| runs[i].converged && runs[i].mean_k.is_finite())
        .fold(None, |best: Option<usize>, i| match best {
            Some(b) if miss(b) <= miss(i) => Some(b),
            _ => Some(i),
        })?;
    let searched: Vec<usize> = (0..runs.len())
        .filter(|&i| runs[i].searched && runs[i].mean_k.is_finite())
        .collect();
    let latest = runs.len() - 1;
    let (u, step) = if !runs[latest].converged {
        (
            f64::midpoint(runs[nearest].u, runs[latest].u),
            SearchStep::Halfway {
                from: nearest,
                toward: latest,
            },
        )
    } else if let [.., a, b] = searched[..] {
        let (first, second) = (runs[a], runs[b]);
        let rise = second.mean_k - first.mean_k;
        // By comparison, so that two points of one mean, or of means that are not numbers, end
        // the search.
        if rise.is_nan() || rise.abs() <= 0.0 {
            return None;
        }
        let secant = (target_k - second.mean_k) * (second.u - first.u) / rise;
        let straddle = (first.mean_k - target_k) * (second.mean_k - target_k) < 0.0;
        let u = if straddle {
            let (low, high) = if first.u < second.u {
                (first.u, second.u)
            } else {
                (second.u, first.u)
            };
            (second.u + secant).clamp(low, high)
        } else {
            let cap = SECANT_STEP_CAP * (second.u - first.u).abs();
            second.u + secant.clamp(-cap, cap)
        };
        (
            u,
            SearchStep::Secant {
                points: [a, b],
                from: nearest,
            },
        )
    } else {
        if !positive(slope_k) {
            return None;
        }
        let from = runs[nearest];
        (
            from.u + (target_k - from.mean_k) / slope_k,
            SearchStep::Slope { from: nearest },
        )
    };
    u.is_finite()
        .then(|| (u.clamp(0.0, bracket), step, nearest))
}

/// The zonal slope `S_Z` of the defect's step, K per unit of u (decision-r09-t13i's §2.4 follow-up 6
/// §2): the secant of the search's two runs nearest the target `target_k`, K, that met the search's
/// stop and whose means differ by at least [`DEFECT_PAIR_SPREAD_K`], with the pair; the steady
/// states' slope `steady_k` where no such pair has a positive one.
#[must_use]
fn defect_slope(runs: &[CorrectionRun], target_k: f64, steady_k: f64) -> DefectSlope {
    let mut order: Vec<usize> = (0..runs.len())
        .filter(|&i| runs[i].searched && runs[i].mean_k.is_finite())
        .collect();
    // Nearest first, the earlier on a tie (the sort is stable).
    order.sort_by(|&a, &b| {
        (runs[a].mean_k - target_k)
            .abs()
            .total_cmp(&(runs[b].mean_k - target_k).abs())
    });
    let pair = order.first().and_then(|&a| {
        order[1..]
            .iter()
            .find(|&&b| (runs[b].mean_k - runs[a].mean_k).abs() >= DEFECT_PAIR_SPREAD_K)
            .map(|&b| [a, b])
    });
    let secant = pair.map(|[a, b]| (runs[b].mean_k - runs[a].mean_k) / (runs[b].u - runs[a].u));
    match secant {
        Some(slope) if positive(slope) => DefectSlope {
            slope_k_per_u: slope,
            pair,
        },
        Some(_) | None => DefectSlope {
            slope_k_per_u: steady_k,
            pair: None,
        },
    }
}

/// The defect step's zonal slope (decision-r09-t13i's §2.4 follow-up 6 §2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DefectSlope {
    /// `S_Z`, K per unit of u.
    pub(super) slope_k_per_u: f64,
    /// The search's two runs it is the secant of, indices in its runs, nearer first; `None` where
    /// it is the steady states' slope.
    pub(super) pair: Option<[usize; 2]>,
}

/// The secant predictor of a run's start (decision-r09-t13i's §2.4 follow-up 7 §1, natural-parameter
/// continuation: Allgower and Georg 1990, from the ruling's memory): each point's state of the two
/// runs' end states `states`, K, at the share `share` of the way from the first's u to the second's,
/// `x_a` + (`x_b` − `x_a`) share, and their return temperatures `returning` the same way where both are
/// the coldest month's (the first's otherwise).
#[must_use]
fn predicted(
    (first, second): (&[f64], &[f64]),
    (first_returning, second_returning): (&Returning, &Returning),
    share: f64,
) -> (Vec<f64>, Returning) {
    let state = first
        .iter()
        .zip(second)
        .map(|(&a, &b)| a + (b - a) * share)
        .collect();
    let returning = match (first_returning, second_returning) {
        (Returning::Coldest(a), Returning::Coldest(b)) => {
            let mut bands = [0.0; LATITUDE_BANDS];
            for ((out, &a), &b) in bands.iter_mut().zip(a.iter()).zip(b.iter()) {
                *out = a + (b - a) * share;
            }
            Returning::Coldest(Box::new(bands))
        }
        (Returning::Own | Returning::Coldest(_), _) => first_returning.clone(),
    };
    (state, returning)
}

/// G2's start (decision-r09-t13i's §2.4 follow-up 7 §1): G1's end `g1` on `grid` plus the broadcast
/// change of the zonal predictor over `S_Z`'s pair `pair` of the search's runs `runs`, from the
/// answer's u to G2's, `(u_Z, u_2)`, and its return temperatures likewise; G1's end itself where
/// `S_Z` is the steady slope, with no pair.
#[must_use]
fn g2_start(
    grid: &ModelGrid,
    g1: &Orbited,
    runs: &[(SearchRun, Orbited)],
    pair: Option<[usize; 2]>,
    (answer_u, u): (f64, f64),
) -> (Vec<f64>, Returning, RunStart) {
    let Some([nearer, other]) = pair else {
        return (
            g1.final_k.clone(),
            g1.returning.clone(),
            RunStart::Continued,
        );
    };
    let (first, second) = (&runs[nearer], &runs[other]);
    let share = (u - answer_u) / (second.0.run.u - first.0.run.u);
    let change: Vec<f64> = first
        .1
        .final_k
        .iter()
        .zip(&second.1.final_k)
        .map(|(x_a, x_b)| (x_b - x_a) * share)
        .collect();
    let state = g1
        .final_k
        .iter()
        .zip(grid.broadcast(&change))
        .map(|(g1_k, change)| g1_k + change)
        .collect();
    let returning = shifted_returning(
        &g1.returning,
        (&first.1.returning, &second.1.returning),
        share,
    );
    (state, returning, RunStart::Predicted)
}

/// G1's return temperatures `g1` shifted by the zonal pair's change of theirs, (second − first)
/// times `share` (decision-r09-t13i's §2.4 follow-up 7 §1): band by band where all are the coldest
/// month's, G1's own otherwise.
#[must_use]
fn shifted_returning(
    g1: &Returning,
    (first, second): (&Returning, &Returning),
    share: f64,
) -> Returning {
    match (g1, first, second) {
        (Returning::Coldest(g1), Returning::Coldest(first), Returning::Coldest(second)) => {
            let mut bands = [0.0; LATITUDE_BANDS];
            for (((out, &g1), &first), &second) in bands
                .iter_mut()
                .zip(g1.iter())
                .zip(first.iter())
                .zip(second.iter())
            {
                *out = g1 + (second - first) * share;
            }
            Returning::Coldest(Box::new(bands))
        }
        (Returning::Own | Returning::Coldest(_), _, _) => g1.clone(),
    }
}

/// How a run of the correction began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RunStart {
    /// From the fit's steady state.
    Steady,
    /// From an earlier run's end.
    Continued,
    /// From the secant predictor of two earlier runs (decision-r09-t13i's §2.4 follow-up 7 §1).
    Predicted,
}

/// The fit's radiation at any u (decision-r09-t13i §2.2): the fit's own at its u₀, so that Z₀ and
/// run A are the fit's to the bit, and [`Radiation::at`] the emission factor e^u at T\* elsewhere.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FitAt {
    /// The fit's u₀ = ln x.
    u0: f64,
    /// The fit's emission factor x.
    factor: f64,
    /// The fit's radiation.
    radiation: Radiation,
    /// The model's greenhouse.
    greenhouse: Greenhouse,
    /// T\*, K.
    mean_k: f64,
}

impl FitAt {
    /// The emission factor and radiation at u = `u`.
    #[must_use]
    fn at(&self, u: f64) -> (f64, Radiation) {
        if u.total_cmp(&self.u0).is_eq() {
            (self.factor, self.radiation)
        } else {
            let x = math::exp(u);
            (x, Radiation::at(self.greenhouse, x, self.mean_k))
        }
    }
}

/// The zonal search of a run's correction over its fit's zonal model (decision-r09-t13i's §2.4
/// follow-up 4 §4): its runs so far, each its record and its orbits, and what its steps read.
struct ZonalSearch<'a> {
    /// The model.
    model: &'a EnergyBalance,
    /// The fit's zonal model.
    zonal: &'a Seasonal<'a>,
    /// The radiation at any u.
    at: FitAt,
    /// The zonal steady states' two-sided slope of the surface mean in u at u₀ ±
    /// [`CORRECTION_STEP`], K per unit of u.
    slope_k: f64,
    /// The runs so far, in order.
    runs: Vec<(SearchRun, Orbited)>,
}

impl<'a> ZonalSearch<'a> {
    /// The search's first run, Z₀, at the fit's u₀ from its zonal steady state `steady_k`, K, under
    /// the search's stop, and the steady states' slope on `fit`'s set-up.
    #[must_use]
    fn begin(
        model: &'a EnergyBalance,
        zonal: &'a Seasonal<'a>,
        fit: &ZonalFit<'_>,
        at: FitAt,
        steady_k: &[f64],
    ) -> Self {
        let first = model.orbits(
            zonal.grid,
            zonal.sunlight,
            &at.radiation,
            (steady_k.to_vec(), Start::Steady),
            zonal.sea_share,
            zonal.ground,
            zonal.ocean,
            SEARCH_STOP,
        );
        let runs = vec![(
            SearchRun {
                run: CorrectionRun::of(at.u0, zonal, &first),
                target_k: at.mean_k,
                step: SearchStep::Fit,
            },
            first,
        )];
        let probe = |u: f64| {
            model
                .probe(
                    zonal.grid,
                    fit.insolation_w_m2,
                    at.mean_k,
                    fit.ground,
                    fit.ocean,
                    math::exp(u),
                )
                .surface_mean_k
        };
        let slope_k = (probe(at.u0 + CORRECTION_STEP) - probe(at.u0 - CORRECTION_STEP))
            / (2.0 * CORRECTION_STEP);
        Self {
            model,
            zonal,
            at,
            slope_k,
            runs,
        }
    }

    /// The search carried on toward the surface mean `target_k`, K, for at most `limit` more runs
    /// ([`search`]), each under the search's stop from the run its step names: its answer.
    fn toward(&mut self, target_k: f64, limit: usize) -> Option<usize> {
        let (model, zonal, at) = (self.model, self.zonal, self.at);
        let run = |u: f64, step: &SearchStep, runs: &[(SearchRun, Orbited)]| {
            let (state, returning, start) = match *step {
                // From Z₂ on, the secant's two runs predict the start (follow-up 7 §1).
                SearchStep::Secant { points: [a, b], .. } => {
                    let (first, second) = (&runs[a], &runs[b]);
                    let share = (u - first.0.run.u) / (second.0.run.u - first.0.run.u);
                    let (state, returning) = predicted(
                        (&first.1.final_k, &second.1.final_k),
                        (&first.1.returning, &second.1.returning),
                        share,
                    );
                    (state, returning, RunStart::Predicted)
                }
                SearchStep::Fit | SearchStep::Slope { .. } | SearchStep::Halfway { .. } => {
                    let from = match *step {
                        SearchStep::Slope { from } | SearchStep::Halfway { from, .. } => from,
                        SearchStep::Fit | SearchStep::Secant { .. } => 0,
                    };
                    let from = &runs[from].1;
                    (
                        from.final_k.clone(),
                        from.returning.clone(),
                        RunStart::Continued,
                    )
                }
            };
            let orbited = model.orbits(
                zonal.grid,
                zonal.sunlight,
                &at.at(u).1,
                (state, Start::Continued(returning)),
                zonal.sea_share,
                zonal.ground,
                zonal.ocean,
                SEARCH_STOP,
            );
            let record = CorrectionRun {
                start,
                ..CorrectionRun::of(u, zonal, &orbited)
            };
            (record, orbited)
        };
        search(
            &mut self.runs,
            target_k,
            self.slope_k,
            math::ln(EMISSION_FACTOR_MAX),
            limit,
            &run,
        )
    }

    /// The runs' records, in order.
    #[must_use]
    fn records(&self) -> Vec<SearchRun> {
        self.runs.iter().map(|(record, _)| *record).collect()
    }

    /// The defect step's slope over the search's runs toward T\* ([`defect_slope`]).
    #[must_use]
    fn defect_slope(&self) -> DefectSlope {
        let runs: Vec<CorrectionRun> = self.runs.iter().map(|(record, _)| record.run).collect();
        defect_slope(&runs, self.at.mean_k, self.slope_k)
    }

    /// The zonal set-up's climate: the answer's run `answer` (the first where none is), its
    /// emission factor and radiation, and the search's record.
    #[must_use]
    fn into_climate(mut self, answer: Option<usize>) -> (Orbited, f64, Radiation, MeanCorrection) {
        let kept = answer.unwrap_or(0);
        let (x, radiation) = self.at.at(self.runs[kept].0.run.u);
        let search = self.records();
        let orbited = self.runs.swap_remove(kept).1;
        (
            orbited,
            x,
            radiation,
            MeanCorrection::Zonal {
                search,
                answer: kept,
            },
        )
    }
}

/// A seasonal run's set-up: its grid, sunlight, sea shares, ground and sea's transport.
#[derive(Debug, Clone, Copy)]
struct Seasonal<'a> {
    grid: &'a ModelGrid,
    sunlight: &'a Sunlight,
    sea_share: &'a [f64],
    ground: GridGround<&'a Relief>,
    ocean: Option<&'a OceanGrid>,
}

/// τ's solve on the zonal grid: its annual insolation, ground and sea's transport.
#[derive(Debug, Clone, Copy)]
struct ZonalFit<'a> {
    insolation_w_m2: &'a [f64],
    ground: GridGround<&'a Relief>,
    ocean: Option<&'a OceanGrid>,
}

/// One seasonal run of the mean's correction: its u = ln x, its surface mean, its orbits and
/// whether it converged (decision-r09-t13i's §2.4 follow-up 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CorrectionRun {
    /// u = ln(1 + ¾τ(T\*)).
    pub(super) u: f64,
    /// Its annual field's surface mean, K.
    pub(super) mean_k: f64,
    /// The orbits it ran.
    pub(super) orbits: u32,
    /// The extrapolation's jumps that stood.
    pub(super) jumps: u32,
    /// The extrapolation's jumps rejected.
    pub(super) rejects: u32,
    /// How it began.
    pub(super) start: RunStart,
    /// Whether its last orbit met the climate's stop, [`CONVERGENCE_K`], within [`MAX_ORBITS`]: a
    /// climate reads it (decision-r09-t13i's §2.4 follow-up 6 §3).
    pub(super) converged: bool,
    /// Whether its last orbit met the search's stop, [`SEARCH_CONVERGENCE_K`]: the search's secant
    /// and the defect's slope read it.
    pub(super) searched: bool,
}

impl CorrectionRun {
    /// The record of `orbited`, a run of `run`'s set-up at u = `u`.
    #[must_use]
    fn of(u: f64, run: &Seasonal<'_>, orbited: &Orbited) -> Self {
        Self {
            u,
            mean_k: surface_mean(run.ground, run.grid, &orbited.climate.annual_k),
            orbits: orbited.climate.orbits,
            jumps: orbited.jumps,
            rejects: orbited.rejects,
            start: orbited.start,
            converged: orbited.climate.converged,
            searched: orbited.searched,
        }
    }
}

/// The least spread of the means of the two search runs whose secant is the defect step's slope, K:
/// 0.1 (`Adopted`, decision-r09-t13i's §2.4 follow-up 6 §2): two reads nearer than that differ by
/// little more than the runs' own reproducibility (the coarse Earth's second search: two runs
/// 1.3 × 10⁻⁶ apart in u differed by 0.037 K).
pub(super) const DEFECT_PAIR_SPREAD_K: f64 = 0.1;

/// How a run of the zonal search took its u (decision-r09-t13i's §2.4 follow-up 4 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SearchStep {
    /// Z₀: at the fit's u, from its zonal steady state.
    Fit,
    /// By the zonal steady states' slope from run `from`.
    Slope {
        /// The run it continued from.
        from: usize,
    },
    /// The secant on the converged runs `points`, continued from run `from`.
    Secant {
        /// The secant's two runs, earlier first.
        points: [usize; 2],
        /// The run it continued from.
        from: usize,
    },
    /// Halfway from the converged run `from`'s u to the unconverged run `toward`'s.
    Halfway {
        /// The converged run it continued from.
        from: usize,
        /// The unconverged run.
        toward: usize,
    },
}

/// One run of the zonal search: its record, the target it served, K, and how it took its u.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SearchRun {
    /// Its u, surface mean, orbits and convergence.
    pub(super) run: CorrectionRun,
    /// Its search's target surface mean, K: T\*, or T\* − d for the second search.
    pub(super) target_k: f64,
    /// How it took its u.
    pub(super) step: SearchStep,
}

/// Which geographic run the mean's correction kept ([`kept_run`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum KeptRun {
    /// G1, at the zonal search's answer (run A at u₀ where the search has none): only where G2 did
    /// not converge and G1 met the climate's stop, or where neither converged and G1 lies nearer,
    /// or as near.
    G1,
    /// G2, the defect's step, or run A where G1 measured no d or the search has no answer.
    G2,
}

/// What the mean's correction did (decision-r09-t13i's §2.4 follow-up 4 §4).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MeanCorrection {
    /// None: a one-month year, whose steady state is its climate, or a body with neither light
    /// nor heat.
    None,
    /// None: τ's solve was clamped, so no greenhouse within its bracket holds the mean; the run is
    /// at the fit's u from its steady state.
    Clamped,
    /// On the zonal grid: the zonal search, its answer the climate.
    Zonal {
        /// The search's runs, in order.
        search: Vec<SearchRun>,
        /// The answer's index in `search`.
        answer: usize,
    },
    /// On the geographic or the locked grid: the zonal search, G1 and G2.
    Searched {
        /// The search's runs, in order.
        search: Vec<SearchRun>,
        /// Z(T\*)'s answer, its index in `search`.
        answer: Option<usize>,
        /// G1.
        g1: CorrectionRun,
        /// G1's probe at the orbit it ended: its R̄ and T̂ (decision-r09-t13i's §2.4 follow-up 8
        /// §1); `None` where it reached its last orbit under neither of its stops, or its state
        /// stopped being finite.
        g1_probe: Option<Box<MeanProbe>>,
        /// d = T̂ − `T̄_Z`, K, and the defect step's zonal slope, where G2 took it.
        defect: Option<(f64, DefectSlope)>,
        /// The surface mean of G2's start, K, where G2 took the defect's step: G1's end, predicted
        /// along the zonal search's pair where `S_Z` has one (decision-r09-t13i's §2.4 follow-up
        /// 7 §1).
        g2_start_k: Option<f64>,
        /// G2, which always runs (follow-up 8 §1.3).
        g2: CorrectionRun,
        /// The run kept.
        kept: KeptRun,
    },
}

/// How a seasonal run begins (decision-r09-t13i §2.4).
#[derive(Debug, Clone, PartialEq)]
enum Start {
    /// From the annual-mean steady state, relaxed first on the whole grid where the sunlight varies
    /// in longitude, the Ekman cells' first return temperatures the state's own.
    Steady,
    /// From a seasonal run's last state, carrying on with its return temperatures `0`.
    Continued(Returning),
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
    /// Each orbit's move of the points' annual means from the orbit before, K, the largest over
    /// the points, in order (none after the first).
    moves_k: Vec<f64>,
    /// Whether its last orbit met the search's stop, [`SEARCH_CONVERGENCE_K`]; its climate's
    /// `converged` says whether it met the climate's, [`CONVERGENCE_K`].
    searched: bool,
    /// The extrapolation's jumps that stood.
    jumps: u32,
    /// The extrapolation's jumps rejected.
    rejects: u32,
    /// How it began: from a steady state, or continued from a run's end.
    start: RunStart,
    /// G1's probe at the orbit its run ended, by its probe stop or the climate's; `None` for every
    /// other run, and for a G1 that reached its last orbit under neither or whose state stopped
    /// being finite (decision-r09-t13i's §2.4 follow-up 8 §1).
    mean_probe: Option<MeanProbe>,
    /// The state it began from, K, and the return temperatures it began with: the search's test of
    /// its predicted starts reads them.
    #[cfg(test)]
    started: (Vec<f64>, Returning),
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
            month.atmosphere_w_m2.fill(0.0);
            month.ekman_w_m2.fill(0.0);
            month.gyres_w_m2.fill(0.0);
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

/// The bands' return temperatures, K, that the next orbit's Ekman cells read: each band's least over
/// the months of its sea temperature ([`OceanGrid::band_sea_k`]), from each month's sums `months`
/// of the points' sea-level temperatures over its `counts` steps, each point's water surface under
/// the ground `ground`, with `water` and `band_sea` the scratch of one month's points and bands
/// (decision-r09-t13i §2.3.1 and its §2.2/§2.3 follow-up 5 §1: the return band's is `T_R`, and the
/// bands' between the edge and it the subducted water's).
#[must_use]
fn coldest_return_k(
    grid: &ModelGrid,
    ocean: &OceanGrid,
    ground: GridGround<ReliefAt<'_>>,
    months: &[Vec<f64>],
    counts: &[u32; 12],
    water: &mut [f64],
    band_sea: &mut [f64],
) -> [f64; LATITUDE_BANDS] {
    let mut coldest = [f64::INFINITY; LATITUDE_BANDS];
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
        for (least, &value) in coldest.iter_mut().zip(band_sea.iter()) {
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
#[cfg(test)]
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
#[cfg(test)]
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
            Some(Currents::of(inputs, model.freezing, mean_depth_m, turning))
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

/// The seasonal run A of [`solve`] continued at u = `u` ([`EnergyBalance::run_continued`]).
#[cfg(test)]
#[must_use]
pub(super) fn solve_continued(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
    ocean: OceanTransport,
    u: f64,
) -> ContinuedRun {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    let currents = currents_of(inputs, &model, ocean, Turning::Rotating);
    model.run_continued(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
        currents.as_ref(),
        u,
    )
}

/// One seasonal run of [`solve`]'s at the fit's u from its steady state, extrapolating as `jumps`
/// says ([`EnergyBalance::run_at_fit`]).
#[cfg(test)]
#[must_use]
pub(super) fn solve_at_fit(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
    ocean: OceanTransport,
    (jumps, orbits): (Jumps, Option<u32>),
) -> GridClimate {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    let currents = currents_of(inputs, &model, ocean, Turning::Rotating);
    model.run_at_fit(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
        currents.as_ref(),
        jumps,
        orbits,
    )
}

/// The zonal search of [`solve`]'s, with each run's states ([`EnergyBalance::search_runs`]).
#[cfg(test)]
#[must_use]
pub(super) fn search_states(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
    ocean: OceanTransport,
) -> Vec<SearchState> {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    let currents = currents_of(inputs, &model, ocean, Turning::Rotating);
    model.search_runs(
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
/// transmissions, its quantiles' grounds and table places `quantiles` the step's own; and the
/// temperature its sea's capacity reads, its water surface's, K.
#[must_use]
fn emission_and_sea(
    radiation: &Radiation,
    t_k: f64,
    raised: Option<(&Relief, (&PointHeights, &[f64; QUANTILES]))>,
    quantiles: &[Quantile; QUANTILES],
    table: Option<&Tabulated>,
) -> (Emission, f64) {
    match (*radiation, raised) {
        (_, None) => (sea_level_emission(radiation, t_k, table), t_k),
        (Radiation::Grey { emission_factor }, Some((relief, (heights, transmission)))) => {
            let column = Column::at(t_k, emission_factor);
            let at = |q: usize| {
                let quantile = quantiles[q];
                quantile_emission(&column, quantile.ground_k, quantile.rate, transmission[q])
            };
            (
                quantile_mean(heights.shape, at),
                relief.water_k(t_k, heights),
            )
        }
        (
            Radiation::Vapour {
                multiplier,
                dry_depth,
                water,
            },
            Some((relief, (heights, transmission))),
        ) => {
            let column =
                Column::vapour(t_k, multiplier, dry_depth, water_terms(&water, t_k, table));
            // The table's water transmission holds only at its own multiplier, to the bit.
            let wet_table = table.and_then(|table| {
                table
                    .wet
                    .as_ref()
                    .filter(|(held, _)| held.total_cmp(&multiplier).is_eq())
                    .map(|(_, wet)| wet)
            });
            let at = |q: usize| {
                let quantile = quantiles[q];
                let wet = match wet_table.zip(quantile.located) {
                    Some((wet, (i, f))) => {
                        let (transmission, slope) = hermite(wet, i, f);
                        [transmission, slope]
                    }
                    None => wet_of(multiplier, water.depth_and_slope(quantile.ground_k)),
                };
                vapour_quantile_emission(
                    &column,
                    quantile.ground_k,
                    quantile.rate,
                    transmission[q],
                    wet,
                )
            };
            (
                quantile_mean(heights.shape, at),
                relief.water_k(t_k, heights),
            )
        }
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
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::surface::reference;

    fn earth() -> (EnergyBalance, Sunlight, ModelGrid) {
        let inputs = reference::earth_like();
        let grid = ModelGrid::zonal();
        let sun = Sunlight::of(&inputs, &grid, super::super::sun::NOMINAL_STEP);
        (EnergyBalance::for_inputs(&inputs), sun, grid)
    }

    /// The reference Earth's air is the registry's, dry air's 1,004.7 J kg⁻¹ K⁻¹ and 28.96 g mol⁻¹
    /// with its water vapour; its water is moist static energy at Earth's transport, its latent
    /// heat Watson's at the mean 288 K, 44.37 kJ mol⁻¹ over 18.015 g mol⁻¹: 2.4628 × 10⁶ J kg⁻¹,
    /// about 0.13% below IAPWS-95's h″ − h′ there, 2,465.9 kJ kg⁻¹ (44.42 kJ mol⁻¹; Wagner and
    /// Pruss 2002, J. Phys. Chem. Ref. Data 31, 387). The diagram's Clausius–Clapeyron enthalpy
    /// gave 2.4686 × 10⁶, about 0.1% above it, and the held curve's constant 43.5 kJ mol⁻¹
    /// 2.415 × 10⁶.
    #[test]
    fn the_reference_earth_diffuses_moist_static_energy_in_its_own_air() {
        let inputs = reference::earth_like();
        let air = Air::of(&inputs);
        // Dry air's 28.96 g mol⁻¹ and 1,004.7 J kg⁻¹ K⁻¹ with 1.32% of water vapour (18.02 g mol⁻¹,
        // about 1,860 J kg⁻¹ K⁻¹), the reference's own (`reference::earth_air`, its water at 0.77 of
        // its saturation by mole since P14.T54.a; 28.820 g mol⁻¹ and 1,010.91 J kg⁻¹ K⁻¹ measured).
        assert!((air.molar_mass_g_per_mol - 28.82).abs() < 0.02, "{air:?}");
        assert!(
            (air.specific_heat_j_per_kg_k - 1_010.9).abs() < 2.0,
            "{air:?}"
        );
        let model = EnergyBalance::for_inputs(&inputs);
        let moisture = model.moisture.expect("water is among Earth's liquids");
        assert!(
            (moisture.latent_heat_j_per_kg - 2.4628e6).abs() < 1e3,
            "{moisture:?}"
        );
        // h rises with T, by more than c_p, and q at 288 K and 80% is about 8.5 g kg⁻¹.
        let q = moisture.specific_humidity(288.0);
        assert!((q - 8.5e-3).abs() < 0.5e-3, "{q}");
        assert!(model.energy_slope(288.0) > 2.0 * air.specific_heat_j_per_kg_k);
        let lambda = model.transport_kg_m2_s / super::super::transport::EARTH_TRANSPORT_KG_M2_S;
        assert!((lambda - 1.0).abs() < 0.05, "{lambda}");
    }

    /// Watson's latent heat lies below IAPWS-95's h″ − h′ for water (Wagner and Pruss 2002): by
    /// about 0.13% at 288 K (44.42 kJ mol⁻¹, 2,465.9 kJ kg⁻¹), 1.5% at 373.15 K (40.65 kJ mol⁻¹)
    /// and 4% at 500 K (32.92 kJ mol⁻¹, 1,827.1 kJ kg⁻¹); and it is its triple point's enthalpy,
    /// exactly, at and below the triple temperature.
    #[test]
    fn the_latent_heat_is_watson_s_scaling_of_the_triple_point_s() {
        let water = by_key("H2O").unwrap().substance().phase_diagram().unwrap();
        let at = |t: f64| latent_heat_j_per_mol(water, Kelvin::new(t)).unwrap();
        for (t, iapws, within) in [
            (288.0, 44_423.8, 0.002),
            (373.15, 40_649.7, 0.02),
            (500.0, 32_915.2, 0.045),
        ] {
            let error = at(t) / iapws - 1.0;
            assert!(error < 0.0 && error > -within, "{t} K: {} J/mol", at(t));
        }
        let triple = water.triple_vaporisation_enthalpy_j_per_mol();
        assert_same_bits(at(water.triple_temperature().value()), triple);
        assert_same_bits(at(200.0), triple);
        assert!(latent_heat_j_per_mol(water, water.critical_temperature()).is_none());
        assert!(latent_heat_j_per_mol(water, Kelvin::new(f64::NAN)).is_none());
    }

    /// The moisture's latent heat is Watson's at the mean surface temperature: the triple point's
    /// for a mean below the triple temperature, whose vapour the surface liquid gives; and a mean
    /// at or above the critical temperature, where no latent heat is left, has no moisture.
    #[test]
    fn the_moisture_s_latent_heat_is_the_triple_point_s_below_it_and_none_above_the_critical() {
        let earth = reference::earth_like();
        let row = by_key("H2O").unwrap().substance();
        let water = row.phase_diagram().unwrap();
        let mass = row.molar_mass_g_per_mol().unwrap();
        let at = |t: f64| {
            let inputs = super::super::tests::builder_of(&earth)
                .mean_surface_temperature(Kelvin::new(t))
                .build()
                .expect("the Earth's inputs at another mean are valid");
            Moisture::of(&inputs, &Air::of(&inputs))
        };
        let cold = at(250.0).expect("a mean below the triple point keeps its moisture");
        assert_same_bits(
            cold.latent_heat_j_per_kg,
            water.triple_vaporisation_enthalpy_j_per_mol() / (mass * 1e-3),
        );
        let warm = at(288.0).expect("Earth's mean has its moisture");
        assert_same_bits(
            warm.latent_heat_j_per_kg,
            latent_heat_j_per_mol(water, Kelvin::new(288.0)).unwrap() / (mass * 1e-3),
        );
        for t in [water.critical_temperature().value(), 700.0] {
            assert!(at(t).is_none(), "{t} K");
        }
    }

    /// The steady state's net heating sums to nothing over the sphere, the diffusion's to
    /// rounding, under one τ and under the water term. Under one τ its mean is the inputs' 288 K;
    /// under the water term the all-land Earth's annual-mean steady states jump across the mean
    /// (decision-r09-t13i's §2.4 follow-up §1; under the 22 rules by weight the cold side ends at a
    /// capped state near 285.3 K, ice share 0.157, its follow-up 6 §5), so its fit is labelled a
    /// fold and keeps the guard's choice among its converged candidates (follow-up 6 §4).
    #[test]
    fn the_steady_state_conserves_energy_and_holds_the_mean() {
        let (vapour, sun, grid) = earth();
        let grey = EnergyBalance {
            greenhouse: Greenhouse::Grey,
            ..vapour
        };
        for (name, model) in [("one τ", grey), ("the water term", vapour)] {
            let Fitted {
                emission_factor: factor,
                radiation,
                steady:
                    Steady {
                        t_k: steady,
                        converged,
                    },
                surface_mean_k,
                optical_depth: depth,
                candidates,
            } = model.emission_factor(&grid, sun.annual(), 288.0);
            eprintln!(
                "{name}: x {factor:.5} ({depth:?}), the steady state's mean {surface_mean_k:.3} K, \
                 its ice share {:.3}; the fold guard's candidates {candidates:?}",
                ice_share(model.freezing, &grid, GridGround::SeaLevel, &steady)
            );
            assert!(converged, "{name}");
            assert_same_bits(surface_mean_k, grid.mean(&steady));
            if model.greenhouse == Greenhouse::Grey {
                assert_eq!(depth, OpticalDepth::Solved);
                assert!(
                    (grid.mean(&steady) - 288.0).abs() < 1e-6,
                    "{}",
                    grid.mean(&steady)
                );
            } else {
                // The branch by g4's ruled rule (decision-r09-t13i's §2.4 follow-up 6 §4): a fold,
                // and the kept probe the guard's choice among its converged candidates.
                assert_eq!(depth, OpticalDepth::Folded);
                let side = nearer_side(candidates, 288.0, model.ice_fraction)
                    .expect("a fold keeps a candidate");
                let chosen = match side {
                    Side::Cold => candidates[0],
                    Side::Warm => candidates[1],
                }
                .expect("the chosen side kept a probe");
                assert_same_bits(factor, chosen.emission_factor);
                assert_same_bits(surface_mean_k, chosen.surface_mean_k);
            }
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
                assert!(net.abs() < 1e-5, "{name}, band {k}: {net}");
            }
            // Warm at the equator, cold at the poles, and symmetric.
            assert!(contrast(&steady) > 20.0);
            assert!((steady[0] - steady[LATITUDE_BANDS - 1]).abs() < 1e-6);
        }
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

    /// The step's shared quantiles (a bit-exact cost lever, decision-r09-t13i's §2.4 follow-up 6
    /// §3): each quantile's ground and table place computed once a step give the albedo and the
    /// outgoing radiation of the point-by-point functions to the bit, under one τ and under the
    /// water term, through the table and directly, at a level point, a split one and one whose
    /// highest quantiles the night floor holds.
    #[test]
    fn the_step_s_shared_quantiles_give_the_point_functions_to_the_bit() {
        let inputs = reference::earth_like();
        let model = EnergyBalance::for_inputs(&inputs);
        let relief = Relief::of(
            &inputs,
            Hypsometry::new(vec![
                PointHeights::new(
                    [0.0, 0.0, 120.0, 900.0, 1_800.0, 3_000.0, 4_500.0, 6_000.0],
                    0.0,
                ),
                PointHeights::level(1_500.0, 0.0),
                PointHeights::new([0.0; QUANTILES].map(|_| 0.0), 0.0),
                PointHeights::new(
                    [0.0, 0.0, 0.0, 0.0, 9_000.0, 12_000.0, 15_000.0, 20_000.0],
                    0.0,
                ),
            ]),
        );
        for radiation in [
            Radiation::Grey {
                emission_factor: 1.65,
            },
            Radiation::at(model.greenhouse, 1.65, 288.0),
        ] {
            let table = Tabulated::new(&model, &radiation, 180.0, 330.0);
            let transmissions = GridGround::Relief(&relief).transmissions(&radiation);
            for table in [Some(&table), None] {
                for point in [0, 1, 3] {
                    let heights = relief.hypsometry().point(point);
                    for t in [150.0, 205.0, 250.0, 270.0, 273.16, 288.0, 305.0, 330.0] {
                        let quantiles = relief.quantiles_at(t, heights, table);
                        let shared = model.located_albedo(heights.shape, &quantiles, table);
                        let direct = model.point_albedo(t, heights, &relief, table);
                        assert_same_bits(shared.albedo, direct.albedo);
                        assert_same_bits(shared.slope_per_k, direct.slope_per_k);
                        let raised = Some((&relief, (heights, &transmissions[point])));
                        let (emission, sea) =
                            emission_and_sea(&radiation, t, raised, &quantiles, table);
                        let reference = match radiation {
                            Radiation::Grey { emission_factor } => point_emission(
                                t,
                                emission_factor,
                                heights,
                                &transmissions[point],
                                &relief,
                            ),
                            Radiation::Vapour {
                                multiplier,
                                dry_depth,
                                water,
                            } => vapour_point_emission(
                                t,
                                multiplier,
                                dry_depth,
                                &water,
                                heights,
                                &transmissions[point],
                                &relief,
                                table,
                            ),
                        };
                        assert_same_bits(emission.flux_w_m2, reference.flux_w_m2);
                        assert_same_bits(emission.slope_w_m2_k, reference.slope_w_m2_k);
                        assert_same_bits(emission.fixed_slope_w_m2_k, reference.fixed_slope_w_m2_k);
                        assert_same_bits(sea, relief.water_k(t, heights));
                    }
                }
            }
        }
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
        // The water term's τ_w and the relief's water transmission, under its multiplier
        // (decision-r09-t13i §2.2): within the table's tolerances wherever it holds them.
        let Radiation::Vapour {
            multiplier, water, ..
        } = Radiation::at(model.greenhouse, 1.65, 288.0)
        else {
            panic!("the reference Earth's air holds water");
        };
        let (mut worst_water, mut worst_wet) = (0.0_f64, 0.0_f64);
        for i in 0..40_000 {
            let t = 60.0 + 0.012_345 * f64::from(i);
            let direct = water.depth_and_slope(t);
            let read = water_terms(&water, t, Some(&table));
            let wet = wet_terms(&water, multiplier, t, Some(&table));
            let wet_direct = wet_of(multiplier, direct);
            worst_water = worst_water
                .max((read[0] - direct[0]).abs())
                .max((read[1] - direct[1]).abs() * 1e-2);
            worst_wet = worst_wet
                .max((wet[0] - wet_direct[0]).abs())
                .max((wet[1] - wet_direct[1]).abs() * 1e-2);
        }
        eprintln!("τ_w {worst_water:e}, its transmission {worst_wet:e}");
        assert!(worst_water < 1e-9 && worst_wet < 1e-9);
        // Another multiplier's transmission is never read from this table.
        assert!(table.wet_at(multiplier * 1.01, 288.0).is_none());
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
    /// against 0.299 (its Γ 5.85 K km⁻¹; the reference Earth's 5.829), to 0.01.
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
        let iced = |factor: f64, mean: f64, ice_share: f64| Probe {
            emission_factor: factor,
            radiation: Radiation::Grey {
                emission_factor: factor,
            },
            ice_share,
            surface_mean_k: mean,
            steady: Steady {
                t_k: vec![mean],
                converged: true,
            },
        };
        let probe = |factor: f64, mean: f64| iced(factor, mean, 0.05);
        let chosen = |cold: Option<Probe>, warm: Option<Probe>| {
            nearer(cold, warm, 288.0, 0.041).map(|probe| probe.emission_factor)
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
        // decision-r09-t13i's §2.4 follow-up §4 (b): at a snowball-class fold the branch whose ice
        // share lies nearer plan 14's, though its mean lies farther: the warm one for a world plan
        // 14 makes capped, the snowball for one it makes a snowball; ties to the nearer mean.
        let (snowball, capped) = (iced(1.6545, 287.9, 1.0), iced(1.6595, 293.0, 0.03));
        assert_eq!(
            nearer(Some(snowball.clone()), Some(capped.clone()), 288.0, 0.041)
                .map(|probe| probe.emission_factor),
            Some(1.6595)
        );
        assert_eq!(
            nearer(Some(snowball), Some(capped), 288.0, 0.9).map(|probe| probe.emission_factor),
            Some(1.6545)
        );
        assert_eq!(
            nearer(
                Some(iced(1.0, 287.0, 0.9)),
                Some(iced(2.0, 288.5, 0.1)),
                288.0,
                0.5
            )
            .map(|probe| probe.emission_factor),
            Some(2.0)
        );
    }

    /// R09.T13.i's stop (decision-r09-t13i §2.4): a move under 0.1 K stops a run only where the
    /// moves shrink and the remainder they extrapolate to, `m_n` r ÷ (1 − r), is under 0.1 K too;
    /// a move of nothing stops it; a first move, with none before it, never does.
    #[test]
    fn the_stop_reads_the_move_and_its_extrapolated_remainder() {
        assert!(settled(0.05, Some(0.2), CONVERGENCE_K));
        // r = 0.8: the remainder 0.2 K.
        assert!(!settled(0.05, Some(0.0625), CONVERGENCE_K));
        // Growing moves.
        assert!(!settled(0.05, Some(0.04), CONVERGENCE_K));
        assert!(!settled(0.15, Some(1.0), CONVERGENCE_K));
        assert!(!settled(0.05, None, CONVERGENCE_K));
        assert!(settled(0.0, None, CONVERGENCE_K));
        assert!(settled(0.0, Some(0.0), CONVERGENCE_K));
        assert!(!settled(f64::NAN, Some(1.0), CONVERGENCE_K));
        // At the edge: r = 0.5, the remainder equal to the move.
        assert!(settled(0.099, Some(0.198), CONVERGENCE_K));
        assert!(!settled(0.1, Some(0.2), CONVERGENCE_K));
        // The search's stop is the same rule at its own bound.
        assert!(!settled(0.05, Some(0.2), SEARCH_CONVERGENCE_K));
        assert!(settled(0.005, Some(0.02), SEARCH_CONVERGENCE_K));
    }

    /// A run's record for the tests: converged at the climate's stop as `converged` says, and at
    /// the search's too where it did.
    fn record(u: f64, mean_k: f64, converged: bool) -> CorrectionRun {
        CorrectionRun {
            u,
            mean_k,
            orbits: if converged { 12 } else { MAX_ORBITS },
            jumps: 0,
            rejects: 0,
            start: RunStart::Continued,
            converged,
            searched: converged,
        }
    }

    /// The correction's choice (decision-r09-t13i's §2.4 follow-up 3, as its follow-ups 4 and 6
    /// keep it): the run nearest the mean that met the climate's stop, the earlier on a tie, over a
    /// nearer run that did not; the nearest of all where none did; never a run whose mean is not
    /// finite.
    #[test]
    fn the_correction_keeps_the_converged_run_nearest_the_mean() {
        // The continent aquaplanet's old runs: C nearer, unconverged; B kept.
        let (a, b, c) = (
            record(0.506_49, 292.03, true),
            record(0.502_29, 290.45, true),
            record(0.495_77, 286.33, false),
        );
        assert_eq!(kept_index(&[a, b, c], 288.0), Some(1));
        let c_converged = record(c.u, c.mean_k, true);
        assert_eq!(kept_index(&[a, b, c_converged], 288.0), Some(2));
        let unconverged = |r: CorrectionRun| record(r.u, r.mean_k, false);
        assert_eq!(
            kept_index(&[unconverged(a), unconverged(b), c], 288.0),
            Some(2)
        );
        // A tie keeps the earlier.
        assert_eq!(
            kept_index(
                &[record(0.5, 289.0, true), record(0.49, 287.0, true)],
                288.0
            ),
            Some(0)
        );
        // A run whose mean is not finite is never kept, and none finite keeps none.
        assert_eq!(
            kept_index(
                &[unconverged(a), record(0.5, f64::NAN, false), unconverged(c)],
                288.0
            ),
            Some(2)
        );
        assert_eq!(kept_index(&[record(0.5, f64::NAN, true)], 288.0), None);
    }

    /// The correction's choice of G1 and G2 (decision-r09-t13i's §2.4 follow-up 8 §1.3): G2 where
    /// it converged, even where a converged G1 lies nearer the mean; G1 where only it met the
    /// climate's stop; the nearer where neither did; G2 where neither mean is finite.
    #[test]
    fn the_correction_keeps_g2_unless_only_g1_converged() {
        let (near, far) = (record(0.5, 288.01, true), record(0.49, 288.3, true));
        let unconverged = |r: CorrectionRun| record(r.u, r.mean_k, false);
        assert_eq!(kept_run(&near, &far, 288.0), KeptRun::G2);
        assert_eq!(kept_run(&near, &unconverged(far), 288.0), KeptRun::G1);
        assert_eq!(
            kept_run(&unconverged(near), &unconverged(far), 288.0),
            KeptRun::G1
        );
        assert_eq!(
            kept_run(&unconverged(far), &unconverged(near), 288.0),
            KeptRun::G2
        );
        let lost = record(0.5, f64::NAN, false);
        assert_eq!(kept_run(&lost, &lost, 288.0), KeptRun::G2);
    }

    /// The orbit at which G1's probe stop ends a hand-built run of surface means `mean_k(n)` after
    /// orbit n, K, from `mean_k(0)`, every orbit fresh, at the stop's ratio `stop`, its fresh
    /// changes kept by the model's own [`MeanTrail`]; `None` within [`MAX_ORBITS`].
    fn probe_of(mean_k: impl Fn(u32) -> f64, stop: f64) -> Option<MeanProbe> {
        let mut trail = MeanTrail::default();
        for orbit in 1..=MAX_ORBITS {
            trail.orbit();
            let (mean, last) = (mean_k(orbit), mean_k(orbit - 1));
            let ended = probe_end(
                (orbit, trail.fresh),
                (mean, mean - last),
                trail.changes,
                Some(stop),
                PROBE_STOP_K,
            );
            if ended.is_some() {
                return ended;
            }
            trail.push(mean - last);
        }
        None
    }

    /// `x` to the power `n`, by repeated product.
    fn power(x: f64, n: u32) -> f64 {
        (0..n).fold(1.0, |product, _| product * x)
    }

    /// The probe's remainder and limit are the extrapolation's form to the bit: |`δ_n`| times the
    /// gain r\* ÷ (1 − r\*), and `T̄_n` plus `δ_n` times it.
    fn assert_the_probe_s_form(probe: &MeanProbe) {
        let gain = probe.ratio / (1.0 - probe.ratio);
        assert_same_bits(probe.remainder_k, probe.change_k.abs() * gain);
        assert_same_bits(probe.limit_k, probe.mean_k + probe.change_k * gain);
    }

    /// G1's probe stop (decision-r09-t13i's §2.4 follow-ups 8 §1 and 9 §2, its unit tests).
    ///
    /// On a hand-built geometric sequence of surface means, ratio 0.9, 1 K below its limit at the
    /// start, the probe ends at the first orbit whose remainder R̄ = |`δ_n`| r ÷ (1 − r) = 0.9ⁿ is
    /// at most 0.05 K, the 29th (0.9²⁸ = 0.0523, 0.9²⁹ = 0.0471), its own ratios agreeing at 0.9,
    /// and its extrapolated limit T̂ is the sequence's within 10⁻⁹ K.
    ///
    /// On two modes, `δ_n` = 0.3ⁿ + 0.02 × 0.9ⁿ K, with the stop's ratio at the fast mode's 0.3, the
    /// lane's failure in small: follow-up 8's probe ended it at orbit 3, R̄ 0.027 K against a true
    /// remainder of 0.143 K; this one waits for the mean's two ratios to agree, ending it at orbit
    /// 13 with T̂ within 3 × 10⁻⁵ K of the limit (the ruling's scratch figure; −2.73 × 10⁻⁵).
    ///
    /// It never ends a run before four fresh orbits or three changes, where the mean's two ratios
    /// disagree by more than 0.05 or either lies outside (0, 1), at a stop's ratio of 1 or more, or
    /// where a ratio is not a number; r\* is the larger of the stop's ratio and the mean's; and R̄
    /// and T̂ are the extrapolation's form to the bit.
    #[test]
    fn g1_s_probe_ends_at_the_first_orbit_within_its_bound_and_reads_the_limit() {
        let limit = 288.0;
        let probe = probe_of(|n| limit - power(0.9, n), 0.9).expect("one mode ends the probe");
        eprintln!("one mode: {probe:?}");
        assert_eq!(probe.orbits, 29);
        assert!(probe.remainder_k <= PROBE_STOP_K);
        assert!((probe.limit_k - limit).abs() <= 1e-9, "{}", probe.limit_k);
        assert_the_probe_s_form(&probe);
        // Two modes: each orbit's change 0.3ⁿ + 0.02 × 0.9ⁿ, the remainder after orbit n
        // 0.3ⁿ⁺¹ ÷ 0.7 + 0.02 × 0.9ⁿ⁺¹ ÷ 0.1.
        let remainder = |n: u32| power(0.3, n + 1) / 0.7 + 0.02 * power(0.9, n + 1) / 0.1;
        let two = probe_of(|n| limit - remainder(n), 0.3).expect("two modes end the probe");
        eprintln!(
            "two modes: {two:?}, the true remainder at orbit 3 {:.4} K",
            remainder(3)
        );
        assert_eq!(two.orbits, 13);
        let [rho, rho_before] = two.mean_ratios.expect("the probe read the mean's ratios");
        assert!((rho - rho_before).abs() <= PROBE_AGREEMENT);
        assert!((two.limit_k - limit).abs() <= 3e-5, "{}", two.limit_k);
        assert!((two.limit_k - limit).abs() <= PROBE_STOP_K);
        assert_the_probe_s_form(&two);
        // The edges, at the mean's ratios 0.5 and 0.5.
        let (means, agreeing) = ((287.99, 0.01), [Some(0.02), Some(0.04)]);
        let end = |fresh: u32, means: (f64, f64), earlier: [Option<f64>; 2], stop: Option<f64>| {
            probe_end((5, fresh), means, earlier, stop, PROBE_STOP_K)
        };
        assert!(end(4, means, agreeing, Some(0.5)).is_some());
        assert!(end(3, means, agreeing, Some(0.5)).is_none());
        assert!(end(4, means, [Some(0.02), None], Some(0.5)).is_none());
        assert!(end(4, means, [Some(0.02), Some(0.03)], Some(0.5)).is_none());
        assert!(end(4, (287.99, 0.03), agreeing, Some(0.5)).is_none());
        assert!(end(4, (287.99, -0.01), agreeing, Some(0.5)).is_none());
        assert!(end(4, means, agreeing, None).is_none());
        assert!(end(4, means, agreeing, Some(1.0)).is_none());
        assert!(end(4, means, agreeing, Some(f64::NAN)).is_none());
        let larger = end(4, means, agreeing, Some(0.8)).expect("R̄ 0.04 K at r* 0.8");
        assert_same_bits(larger.ratio, 0.8);
        assert!(end(4, means, agreeing, Some(0.9)).is_none());
    }

    /// The mean's changes `a` and `b` are the same, each to the bit.
    fn assert_same_changes(a: [Option<f64>; 2], b: [Option<f64>; 2]) {
        for (x, y) in a.into_iter().zip(b) {
            match (x, y) {
                (Some(x), Some(y)) => assert_same_bits(x, y),
                (None, None) => {}
                (x, y) => panic!("{x:?} against {y:?}"),
            }
        }
    }

    /// G1's fresh count and the mean's changes (decision-r09-t13i's §2.4 follow-ups 8 §1 and 9 §1.3):
    /// a jump restarts both; a reject restores both to the bit, the jump orbit's change included;
    /// and after a jump that stood even a perfect geometric run's probe cannot end it before
    /// [`PROBE_FRESH_ORBITS`] fresh orbits, ending it at the fourth.
    #[test]
    fn the_mean_s_trail_restarts_at_a_jump_and_a_reject_restores_it() {
        let mut trail = MeanTrail::default();
        for change in [0.04, 0.02, 0.01] {
            trail.orbit();
            trail.push(change);
        }
        assert_eq!(trail.fresh, 3);
        let before = trail;
        trail.jumped();
        assert_eq!(trail.fresh, 0);
        assert_same_changes(trail.changes, [None, None]);
        trail.orbit();
        trail.rejected();
        assert_eq!(trail.fresh, before.fresh);
        assert_same_changes(trail.changes, before.changes);
        let mut trail = before;
        trail.jumped();
        let (mut mean, mut change) = (287.9, 0.02);
        for n in 1..=PROBE_FRESH_ORBITS {
            trail.orbit();
            mean += change;
            let ended = probe_end(
                (10 + n, trail.fresh),
                (mean, change),
                trail.changes,
                Some(0.5),
                PROBE_STOP_K,
            );
            assert_eq!(ended.is_some(), n == PROBE_FRESH_ORBITS, "fresh orbit {n}");
            trail.push(change);
            change *= 0.5;
        }
    }

    /// G1's probe where the climate's stop ends its run (decision-r09-t13i's §2.4 follow-up 9
    /// §1.2): r\* is the mean's ratio where it lies above the stop's and below 1, the stop's ratio
    /// otherwise, and nothing where the stop has no ratio or two moves of nothing made it 0 ÷ 0, T̂
    /// then the mean itself.
    #[test]
    fn a_climate_stop_s_probe_reads_the_larger_ratio_and_a_still_run_none() {
        let (means, agreeing) = ((287.99, 0.01), [Some(0.02), Some(0.04)]);
        let from_mean = settled_probe(7, means, agreeing, Some(0.3));
        assert_same_bits(from_mean.ratio, 0.5);
        assert!(
            (from_mean.limit_k - 288.0).abs() <= 1e-9,
            "{}",
            from_mean.limit_k
        );
        assert_the_probe_s_form(&from_mean);
        assert_same_bits(settled_probe(7, means, agreeing, Some(0.8)).ratio, 0.8);
        assert_same_bits(
            settled_probe(7, (287.99, 0.03), agreeing, Some(0.3)).ratio,
            0.3,
        );
        for stop in [None, Some(f64::NAN)] {
            let still = settled_probe(7, (288.0, 0.0), [None, None], stop);
            assert_same_bits(still.ratio, 0.0);
            assert_same_bits(still.remainder_k, 0.0);
            assert_same_bits(still.limit_k, 288.0);
        }
    }

    /// G1's probe reads its run without changing it (decision-r09-t13i's §2.4 follow-up 8 §1's
    /// "the probe reads the same sums as the stop"): on the reference Earth's zonal model at its
    /// fitted u over a sea share of 0.7, from its steady state, a run
    /// under G1's stop whose probe cannot fire (a bound of nothing) is the climate's run to the
    /// bit, its probe recorded only where the climate's stop ended it; and on the marched run the
    /// probe with no bound on its remainder ends it at the first fresh orbit whose mean's two
    /// ratios agree, long before the climate's stop (at 0.05 K it ends that run nowhere before its
    /// climate stop at orbit 27, the mean's last ratios there 0.60 and 0.92).
    #[test]
    fn g1_s_probe_reads_its_run_without_changing_it() {
        let (model, sun, grid) = earth();
        let fitted = model.emission_factor(&grid, sun.annual(), 288.0);
        let sea = vec![0.7; grid.points()];
        let run = |stop| {
            model.orbits(
                &grid,
                &sun,
                &fitted.radiation,
                (fitted.steady.t_k.clone(), Start::Steady),
                &sea,
                GridGround::SeaLevel,
                None,
                stop,
            )
        };
        let (plain, probed) = (
            run(CLIMATE_STOP),
            run(Stop {
                probe_k: Some(0.0),
                ..G1_STOP
            }),
        );
        let same = |name: &str, a: &[f64], b: &[f64]| {
            assert_eq!(a.len(), b.len(), "{name}");
            for (x, y) in a.iter().zip(b) {
                assert_same_bits(*x, *y);
            }
        };
        same("annual", &plain.climate.annual_k, &probed.climate.annual_k);
        for (a, b) in plain.climate.months_k.iter().zip(&probed.climate.months_k) {
            same("month", a, b);
        }
        same("state", &plain.final_k, &probed.final_k);
        same("moves", &plain.moves_k, &probed.moves_k);
        assert_eq!(
            (
                plain.climate.orbits,
                plain.climate.converged,
                plain.searched,
                plain.jumps,
                plain.rejects
            ),
            (
                probed.climate.orbits,
                probed.climate.converged,
                probed.searched,
                probed.jumps,
                probed.rejects
            )
        );
        assert_eq!(plain.mean_probe, None);
        assert_eq!(
            probed.mean_probe.map(|probe| probe.orbits),
            probed.climate.converged.then_some(probed.climate.orbits)
        );
        let marched = Stop {
            jumps: Jumps::Never,
            ..CLIMATE_STOP
        };
        let (climate, measured) = (
            run(marched),
            run(Stop {
                probe_k: Some(f64::INFINITY),
                ..marched
            }),
        );
        let probe = measured.mean_probe.expect("the probe ends a marched run");
        eprintln!(
            "marched: {} orbits, converged {}; probed: {probe:?}",
            climate.climate.orbits, climate.climate.converged
        );
        assert_eq!(probe.orbits, measured.climate.orbits);
        assert!(!measured.climate.converged);
        assert!(measured.climate.orbits < climate.climate.orbits);
        let [rho, rho_before] = probe.mean_ratios.expect("the probe read the mean's ratios");
        assert!((rho - rho_before).abs() <= PROBE_AGREEMENT);
    }

    /// The probe stop's fallback (decision-r09-t13i's §2.4 follow-up 8 §1.4, its test): a G1 that
    /// reaches its last orbit under neither of its stops, here the reference Earth's zonal model
    /// for two orbits, fewer than either stop needs, records no probe, and G2 is then run A; where
    /// G1's probe measured T̂ and the search has an answer, G2 takes the defect's step, d = T̂ −
    /// `T̄_Z`.
    #[test]
    fn a_g1_that_never_meets_its_probe_stop_makes_g2_run_a() {
        let (model, sun, grid) = earth();
        let radiation = Radiation::at(model.greenhouse, 1.6, 288.0);
        let g1 = model.orbits(
            &grid,
            &sun,
            &radiation,
            (vec![288.0; grid.points()], Start::Steady),
            &vec![0.7; grid.points()],
            GridGround::SeaLevel,
            None,
            Stop {
                max_orbits: 2,
                ..G1_STOP
            },
        );
        assert_eq!(g1.climate.orbits, 2);
        assert!(!g1.climate.converged);
        assert_eq!(g1.mean_probe, None);
        let search = [SearchRun {
            run: record(0.5, 288.2, true),
            target_k: 288.0,
            step: SearchStep::Fit,
        }];
        assert_eq!(second_run(Some(0), &search, g1.mean_probe), SecondRun::RunA);
        let probe = MeanProbe::of(5, (287.9, -0.01), None, 0.8);
        assert_eq!(second_run(None, &search, Some(probe)), SecondRun::RunA);
        let SecondRun::Defect { answer, defect_k } = second_run(Some(0), &search, Some(probe))
        else {
            panic!("a measured G1 takes the defect's step");
        };
        assert_eq!(answer, 0);
        assert_same_bits(defect_k, probe.limit_k - 288.2);
        assert!((defect_k + 0.34).abs() < 1e-9, "{defect_k}");
    }

    /// The spin-up's extrapolation (decision-r09-t13i's §2.4 follow-up 4 §3, as its follow-up 6 §1
    /// amends it): a jump is due only while the stop fails on its remainder, `m_n` r ÷ (1 − r) at
    /// least the bound, with r in (0.3, 0.97), at most once every two orbits and where two more
    /// orbits fit; it shifts the state and the stored annual means by the orbit's change times
    /// r ÷ (1 − r); a first fresh move larger than the jump's own rejects it, restoring the state,
    /// the means and the return temperatures to the bit, after which a jump needs two successive
    /// ratios that agree within 0.05; a smaller first fresh move accepts it.
    #[test]
    fn a_jump_whose_first_fresh_move_grows_is_rejected_and_restored_to_the_bit() {
        let mut spin = Extrapolation::new(Jumps::Allowed);
        let bound = CONVERGENCE_K;
        // Not due: the remainder under the bound (0.05 at r 0.5), outside the range, with no move
        // before, not a number, at the cap.
        assert!(spin.gain(5, 0.05, Some(0.1), bound).is_none());
        assert!(spin.gain(5, 0.5, Some(2.0), bound).is_none());
        assert!(spin.gain(5, 0.5, Some(0.5), bound).is_none());
        assert!(spin.gain(5, 0.5, None, bound).is_none());
        assert!(spin.gain(5, f64::NAN, Some(1.0), bound).is_none());
        assert!(spin.gain(MAX_ORBITS - 1, 0.5, Some(1.0), bound).is_none());
        // Due with no floor on the move: 0.005 at r 0.95 leaves 0.095 K, over the search's bound.
        assert!(
            spin.gain(5, 0.005, Some(0.005 / 0.95), SEARCH_CONVERGENCE_K)
                .is_some()
        );
        let gain = spin.gain(5, 0.8, Some(1.0), bound).expect("r 0.8 is due");
        assert!((gain - 4.0).abs() < 1e-12, "{gain}");
        assert!(
            Extrapolation::new(Jumps::Never)
                .gain(5, 0.8, Some(1.0), bound)
                .is_none()
        );
        let mut state = vec![290.0, 280.5, 270.25];
        let annual = vec![289.0, 281.0, 271.0];
        let mut stored = vec![288.5, 280.0, 271.5];
        let returning = Returning::Coldest(Box::new([271.0; LATITUDE_BANDS]));
        let before = (state.clone(), stored.clone());
        spin.jump(
            5,
            0.8,
            gain,
            (&mut state, &annual, &mut stored),
            returning.clone(),
        );
        for p in 0..3 {
            let shift = (annual[p] - before.1[p]) * gain;
            assert_same_bits(state[p], before.0[p] + shift);
            assert_same_bits(stored[p], annual[p] + shift);
        }
        let Review::Rejected(jumped) = spin.review(0.9) else {
            panic!("a growing move rejects the jump");
        };
        for (restored, kept) in jumped.state_k.iter().zip(&before.0) {
            assert_same_bits(*restored, *kept);
        }
        for (restored, kept) in jumped.annual_k.iter().zip(&annual) {
            assert_same_bits(*restored, *kept);
        }
        let (Returning::Coldest(restored), Returning::Coldest(kept)) =
            (&jumped.returning, &returning)
        else {
            panic!("the jump kept the coldest months' return temperatures");
        };
        for (restored, kept) in restored.iter().zip(kept.iter()) {
            assert_same_bits(*restored, *kept);
        }
        assert_same_bits(jumped.move_k, 0.8);
        assert!(matches!(spin.review(0.1), Review::None));
        // After the reject: one ratio is not enough, two that agree within 0.05 are, two that
        // disagree are not.
        assert!(spin.gain(7, 0.5, Some(0.6), bound).is_none());
        assert!(spin.gain(8, 0.42, Some(0.5), bound).is_some());
        let mut other = Extrapolation::new(Jumps::Allowed);
        other.rejected = true;
        assert!(other.gain(7, 0.5, Some(0.6), bound).is_none());
        assert!(other.gain(8, 0.3, Some(0.5), bound).is_none());
        // A shrinking first fresh move accepts it, and the run may jump again two orbits on.
        let mut spin = Extrapolation::new(Jumps::Allowed);
        let gain = spin.gain(5, 0.8, Some(1.0), bound).expect("r 0.8 is due");
        spin.jump(5, 0.8, gain, (&mut state, &annual, &mut stored), returning);
        assert!(spin.gain(6, 0.8, Some(1.0), bound).is_none());
        assert!(matches!(spin.review(0.4), Review::Accepted));
        assert_eq!(spin.jumps, 1);
        assert!(spin.gain(7, 0.3, Some(0.4), bound).is_some());
        // After a jump that stood, the stop's remainder reads r* = max(r, the jump's 0.8): a fresh
        // ratio of 0.42, the jump's own transient, does not stop it (follow-up 7 §2.3).
        assert_eq!(spin.jump_ratio, Some(0.8));
        assert!(settled(0.05, Some(0.119), bound));
        assert!(!settled_after(0.05, Some(0.119), bound, spin.jump_ratio));
        assert!(settled_after(0.01, Some(0.012), bound, spin.jump_ratio));
        // At most two rejects: after the second the run only marches (follow-up 7 §2.2).
        let mut capped = Extrapolation::new(Jumps::Allowed);
        for orbit in [5, 9] {
            capped.last_ratio = Some(0.8);
            let gain = capped.gain(orbit, 0.8, Some(1.0), bound).expect("due");
            capped.jump(
                orbit,
                0.8,
                gain,
                (&mut state, &annual, &mut stored),
                Returning::Own,
            );
            assert!(matches!(capped.review(1.0), Review::Rejected(_)));
        }
        assert_eq!(capped.rejects, MAX_REJECTS);
        capped.last_ratio = Some(0.8);
        assert!(capped.gain(13, 0.8, Some(1.0), bound).is_none());
    }

    /// The zonal search (decision-r09-t13i's §2.4 follow-up 4 §4) and the defect step (its
    /// follow-up 6 §2) on a hand-built linear pair: a zonal branch of slope 340 K per unit of u
    /// through 288 K at u 0.5, and a geographic one 1 K warmer there whose slope is 380. Z₀ at
    /// u 0.49, then the slope's step, closes on 288 K in two runs; G1 at its u lies at the defect
    /// d = 1 K; `S_Z` is the secant of the two runs nearest T\* that met the search's stop, 0.1 K
    /// or more apart, here 340; and G2 at `u_Z` − d ÷ `S_Z` misses T\* by |1 − `S_g` ÷ `S_Z`| d, the
    /// defect step's own error.
    #[test]
    fn the_search_closes_on_a_linear_branch_and_its_defect_step_misses_by_the_slopes_difference() {
        let zonal = |u: f64| 288.0 + 340.0 * (u - 0.5);
        let geographic = |u: f64| zonal(u) + 1.0 + 40.0 * (u - 0.5);
        let run = |u: f64, _: &SearchStep, _: &[(SearchRun, ())]| (record(u, zonal(u), true), ());
        let mut runs = vec![(
            SearchRun {
                run: record(0.49, zonal(0.49), true),
                target_k: 288.0,
                step: SearchStep::Fit,
            },
            (),
        )];
        let first = search(&mut runs, 288.0, 340.0, 13.8, SEARCH_RUNS - 1, &run).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[1].0.step, SearchStep::Slope { from: 0 });
        assert!((runs[first].0.run.mean_k - 288.0).abs() <= SEARCH_CONVERGENCE_K);
        let answer = runs[first].0.run;
        let defect = geographic(answer.u) - answer.mean_k;
        assert!((defect - 1.0).abs() < 1e-9, "{defect}");
        let records: Vec<CorrectionRun> = runs.iter().map(|(r, ())| r.run).collect();
        let slope = defect_slope(&records, 288.0, 999.0);
        assert_eq!(slope.pair, Some([1, 0]));
        assert!((slope.slope_k_per_u - 340.0).abs() < 1e-6, "{slope:?}");
        let miss = geographic(answer.u - defect / slope.slope_k_per_u) - 288.0;
        let expected = (1.0 - 380.0 / 340.0) * defect;
        eprintln!("the defect step's miss {miss:+.6} K against {expected:+.6} K");
        assert!((miss - expected).abs() < 1e-6, "{miss} against {expected}");
        // With no pair 0.1 K apart, the steady slope.
        let close = [record(0.5, 288.0, true), record(0.501, 288.05, true)];
        let fallback = defect_slope(&close, 288.0, 333.0);
        assert!(fallback.pair.is_none());
        assert_same_bits(fallback.slope_k_per_u, 333.0);
    }

    /// The zonal search never takes an unconverged run for a secant point (decision-r09-t13i's
    /// §2.4 follow-up 4 §4, as its follow-up 6 §3 amends it): on a hand-built branch that ends
    /// below u 0.497, a run there meets neither stop, and the next u is halfway from the nearest
    /// run that met the climate's to its u; the secant reads only runs that met the search's; the
    /// answer met the climate's.
    #[test]
    fn the_search_never_takes_an_unconverged_run_for_a_secant_point() {
        let zonal = |u: f64| 288.0 + 340.0 * (u - 0.5);
        let run =
            |u: f64, _: &SearchStep, _: &[(SearchRun, ())]| (record(u, zonal(u), u >= 0.497), ());
        let mut runs = vec![(
            SearchRun {
                run: record(0.505, zonal(0.505), true),
                target_k: 286.0,
                step: SearchStep::Fit,
            },
            (),
        )];
        let answer = search(&mut runs, 286.0, 340.0, 13.8, SEARCH_RUNS - 1, &run).unwrap();
        for (i, (record, ())) in runs.iter().enumerate() {
            eprintln!("run {i}: {record:?}");
            match record.step {
                SearchStep::Secant { points, from } => {
                    assert!(points.iter().all(|&j| runs[j].0.run.searched), "{i}");
                    assert!(runs[from].0.run.converged);
                }
                SearchStep::Halfway { from, toward } => {
                    assert!(runs[from].0.run.converged && !runs[toward].0.run.converged);
                    assert_same_bits(
                        record.run.u,
                        f64::midpoint(runs[from].0.run.u, runs[toward].0.run.u),
                    );
                }
                SearchStep::Fit | SearchStep::Slope { .. } => {}
            }
        }
        assert!(
            runs.iter()
                .any(|(record, ())| matches!(record.step, SearchStep::Halfway { .. }))
        );
        assert!(runs[answer].0.run.converged);
    }

    /// Plan 14's greenhouse (decision-r09-t13i §2.2): water's 4.47 per √bar at 0.77 of its
    /// saturation by mole, `M_w` ÷ μ of that by weight on the reference Earth's air.
    #[test]
    fn the_greenhouse_reads_the_batch_s_rules() {
        let inputs = reference::earth_like();
        let air = Air::of(&inputs);
        let water_term = |constants: &GreenhouseConstants| {
            let unit = PartialPressures::from_fn(|gas| match gas {
                Gas::Water => Pascals::new(BAR_PA),
                Gas::CarbonDioxide
                | Gas::Nitrogen
                | Gas::Oxygen
                | Gas::Argon
                | Gas::Methane
                | Gas::Ammonia
                | Gas::Hydrogen
                | Gas::Helium => Pascals::ZERO,
            });
            constants
                .terms(&unit, Kelvin::new(255.0))
                .iter()
                .find(|(gas, _)| *gas == Gas::Water)
                .map_or(0.0, |&(_, term)| term)
        };
        let (batch, batch_humidity) = greenhouse_in_force(&air);
        assert!((water_term(&batch) - 4.47).abs() < 1e-12);
        let expected =
            WATER_SURFACE_HUMIDITY * Gas::Water.molar_mass_g_per_mol() / air.molar_mass_g_per_mol;
        assert_same_bits(batch_humidity, expected);
        eprintln!("23's humidity by weight on the reference Earth's air {batch_humidity:.4}");
        assert!((0.47..0.50).contains(&batch_humidity), "{batch_humidity}");
    }

    #[test]
    fn the_water_term_follows_the_column_s_own_temperature() {
        let inputs = reference::earth_like();
        let model = EnergyBalance::for_inputs(&inputs);
        let Greenhouse::Vapour { dry_depth, water } = model.greenhouse else {
            panic!("the reference Earth's air holds water");
        };
        let share = water.depth(288.0) / (dry_depth + water.depth(288.0));
        eprintln!(
            "τ_d {dry_depth:.4}, τ_w(288 K) {:.4}, the water's share {share:.3}; k_w {} per √bar, \
             h_w {}",
            water.depth(288.0),
            water.constant,
            water.humidity
        );
        assert!((0.40..0.55).contains(&share), "{share}");
        assert_same_bits(water.constant, 4.47);
        let radiation = Radiation::at(model.greenhouse, 1.65, 288.0);
        let Radiation::Vapour { multiplier, .. } = radiation else {
            panic!("a water term")
        };
        // The emission factor at T* is the one asked for.
        let at_mean = 1.0 + EDDINGTON_COEFFICIENT * multiplier * (dry_depth + water.depth(288.0));
        assert!((at_mean - 1.65).abs() < 1e-12, "{at_mean}");
        for t in [275.0, 288.0, 300.0, 320.0] {
            let column = Column::vapour(t, multiplier, dry_depth, water.depth_and_slope(t));
            let tau = (STEFAN_BOLTZMANN * math::powi(t, 4) / column.emission.flux_w_m2 - 1.0)
                / EDDINGTON_COEFFICIENT;
            let expected = multiplier * (dry_depth + water.depth(t));
            assert!(
                (tau - expected).abs() <= 1e-12 * expected,
                "{t}: {tau} against {expected}"
            );
            let flux = |t: f64| {
                Column::vapour(t, multiplier, dry_depth, water.depth_and_slope(t))
                    .emission
                    .flux_w_m2
            };
            let numeric = (flux(t + 1e-3) - flux(t - 1e-3)) / 2e-3;
            let e = column.emission;
            assert!(
                (numeric - column.whole_slope_w_m2_k).abs() < 1e-5 * e.fixed_slope_w_m2_k,
                "{t}: {numeric} against {}",
                column.whole_slope_w_m2_k
            );
            assert!(e.slope_w_m2_k >= EMISSION_SLOPE_FLOOR * e.fixed_slope_w_m2_k);
            assert!(e.slope_w_m2_k < e.fixed_slope_w_m2_k);
            let cube = t * t * t;
            assert_same_bits(
                e.fixed_slope_w_m2_k,
                4.0 * STEFAN_BOLTZMANN * cube
                    / (1.0 + EDDINGTON_COEFFICIENT * multiplier * (dry_depth + water.depth(t))),
            );
        }
        // A world whose gases hold no water has one τ, independent of T.
        let dry = reference::builder_of(&inputs)
            .gases([crate::planetary::surface::inputs::GasShare {
                substance: crate::substance::SubstanceKey::new("N2").unwrap(),
                mole_fraction: 1.0,
            }])
            .build()
            .unwrap();
        assert_eq!(EnergyBalance::for_inputs(&dry).greenhouse, Greenhouse::Grey);
    }

    /// R1 on a world with no surface water (decision-r09-t13i's §2.4 follow-up 2 §4, by weight as
    /// its follow-up 5 §2 amends it): the reference Earth with no liquid keeps its air's water, so
    /// its water term's vapour is the lesser of `h_w` of the saturation and that water's column
    /// weight, `x_w` (`M_w` ÷ μ) `p_s`: above the crossover, where `h_w` `p_sat`(T) equals it, the
    /// inventory's term, and below it the saturation's, on the ice branch too, each to 10⁻¹²; and
    /// its zonal model converges, every record finite. The reference Earth itself, with its sea,
    /// takes the saturation at every temperature, to the bit as before the cap.
    #[test]
    fn a_world_with_no_surface_water_caps_its_vapour_at_its_air_s_water() {
        let inputs = reference::earth_like();
        let desert = reference::builder_of(&inputs)
            .liquids([])
            .condensates([])
            .build()
            .unwrap();
        let model = EnergyBalance::for_inputs(&desert);
        let Greenhouse::Vapour { water, .. } = model.greenhouse else {
            panic!("the desert's air holds water: {:?}", model.greenhouse);
        };
        let Reservoir::Air { inventory_pa } = water.reservoir else {
            panic!("no liquid water lies at the desert's surface");
        };
        let saturated = |water: &WaterVapour, t: f64| {
            water.humidity
                * saturation_pressure(water.water, Kelvin::new(t)).map_or(0.0, Pascals::value)
        };
        let term = |water: &WaterVapour, pressure: f64| water.constant * (pressure / BAR_PA).sqrt();
        let (mut low, mut high) = (200.0, 330.0);
        for _ in 0..60 {
            let middle = f64::midpoint(low, high);
            if saturated(&water, middle) < inventory_pa {
                low = middle;
            } else {
                high = middle;
            }
        }
        let dew = f64::midpoint(low, high);
        eprintln!(
            "the desert's water {inventory_pa:.1} Pa by weight, its crossover {dew:.3} K (h_w {}); \
             τ_w capped {:.5}, at 288 K {:.5}",
            water.humidity,
            term(&water, inventory_pa),
            water.depth(288.0)
        );
        let capped = term(&water, inventory_pa);
        for t in [dew + 0.01, dew + 1.0, dew + 10.0, 330.0, 400.0] {
            let depth = water.depth(t);
            assert!(
                (depth - capped).abs() <= 1e-12 * capped,
                "{t} K: {depth} against {capped}"
            );
        }
        for t in [dew - 0.01, dew - 1.0, dew - 10.0, 260.0, 230.0, 200.0] {
            let (depth, expected) = (water.depth(t), term(&water, saturated(&water, t)));
            assert!(
                (depth - expected).abs() <= 1e-12 * expected,
                "{t} K: {depth} against {expected}"
            );
        }
        let Greenhouse::Vapour { water: sea, .. } = EnergyBalance::for_inputs(&inputs).greenhouse
        else {
            panic!("the reference Earth's air holds water");
        };
        assert_eq!(sea.reservoir, Reservoir::Surface);
        for t in [200.0, 230.0, 260.0, dew, 300.0, 330.0, 400.0] {
            assert_same_bits(sea.depth(t), term(&sea, saturated(&sea, t)));
        }
        let grid = ModelGrid::zonal();
        let sun = Sunlight::of(&desert, &grid, super::super::sun::NOMINAL_STEP);
        let solution = model.run(
            &grid,
            &sun,
            desert.mean_surface_temperature().value(),
            &[0.0; LATITUDE_BANDS],
            Ground::SeaLevel,
            None,
        );
        eprintln!(
            "the desert: x {:.5} ({:?}, m {:?}), steady converged {}, orbits {}, converged {}, \
             surface mean {:.3} K, the mean's correction {:?}",
            solution.emission_factor,
            solution.optical_depth,
            solution.radiation.multiplier(),
            solution.steady_converged,
            solution.climate.orbits,
            solution.climate.converged,
            grid.mean(&solution.climate.annual_k),
            solution.correction
        );
        assert!(solution.climate.converged);
        assert!(solution.steady_converged);
        assert!(solution.emission_factor.is_finite());
        assert!(
            solution
                .climate
                .annual_k
                .iter()
                .chain(solution.climate.months_k.iter().flatten())
                .chain(solution.month_insolation_w_m2.iter().flatten())
                .all(|v| v.is_finite())
        );
    }

    /// R09.T13.i's R2 (decision-r09-t13i §2.5, test 4): at z = 0 a raised quantile's outgoing
    /// radiation under the water term is the sea-level column's to the bit; higher up, its whole
    /// slope is the derivative of its flux in T, the transmission's own slope included, and it is
    /// never below the floor; the vapour above raised ground follows the ground's temperature, so
    /// that a plateau's column is thinner than the dry gases' thinning alone gives.
    #[test]
    fn a_raised_quantile_s_water_term_follows_its_ground() {
        let inputs = reference::earth_like();
        let model = EnergyBalance::for_inputs(&inputs);
        let radiation = Radiation::at(model.greenhouse, 1.65, 288.0);
        let Radiation::Vapour {
            multiplier,
            dry_depth,
            water,
        } = radiation
        else {
            panic!("a water term")
        };
        let (gamma, h) = (lapse::lapse_rate_k_per_m(&inputs), scale_height_m(&inputs));
        let dry = |z: f64| {
            math::exp(
                -FLUX_OPTICAL_DEPTH_PER_GREY * multiplier * dry_depth * math::exp(-z / (2.0 * h)),
            )
        };
        for t in [260.0, 288.0, 305.0] {
            let column = Column::vapour(t, multiplier, dry_depth, water.depth_and_slope(t));
            let at_sea = vapour_quantile_emission(
                &column,
                t,
                1.0,
                dry(0.0),
                wet_of(multiplier, water.depth_and_slope(t)),
            );
            assert_same_bits(at_sea.flux_w_m2, column.emission.flux_w_m2);
            assert_same_bits(at_sea.slope_w_m2_k, column.emission.slope_w_m2_k);
            assert_same_bits(
                at_sea.fixed_slope_w_m2_k,
                column.emission.fixed_slope_w_m2_k,
            );
            for z in [500.0, 2_000.0, 4_000.0] {
                let quantile = |dt: f64| {
                    let column = Column::vapour(
                        t + dt,
                        multiplier,
                        dry_depth,
                        water.depth_and_slope(t + dt),
                    );
                    let ground = t + dt - gamma * z;
                    vapour_quantile_emission(
                        &column,
                        ground,
                        1.0,
                        dry(z),
                        wet_of(multiplier, water.depth_and_slope(ground)),
                    )
                };
                let e = quantile(0.0);
                let numeric = (quantile(1e-3).flux_w_m2 - quantile(-1e-3).flux_w_m2) / 2e-3;
                let whole = e.slope_w_m2_k.max(numeric);
                assert!(
                    e.flux_w_m2 > 0.0 && e.slope_w_m2_k > 0.0,
                    "{t} K, {z} m: {e:?}"
                );
                if e.slope_w_m2_k > EMISSION_SLOPE_FLOOR * e.fixed_slope_w_m2_k * (1.0 + 1e-9) {
                    assert!(
                        (numeric - e.slope_w_m2_k).abs() < 1e-5 * whole,
                        "{t} K, {z} m: {numeric} against {e:?}"
                    );
                }
                // The ground's vapour: thinner than at the sea level's temperature.
                let ground = t - gamma * z;
                assert!(water.depth(ground) < water.depth(t));
            }
        }
    }
}
