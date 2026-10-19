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
//! - σT⁴ ÷ (1 + ¾τ) the grey outgoing radiation in Spiegel et al.'s Eddington form, with one τ
//!   for the body, solved once so that the surface mean of the annual-mean steady state is plan
//!   14's mean ([`CoarseInputs::mean_surface_temperature`]);
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
//!   3/2 ([`quantile_emission`]);
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
use super::sun::Sunlight;
use super::transport::{Diffusing, REFERENCE_PRESSURE_PA, rotation_ratio, transport_kg_m2_s};
use crate::math;
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

/// The optical depth's solve ([`EnergyBalance::fit`]): the emission factor, the annual-mean
/// steady state there, its surface mean and how τ was found.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Fitted {
    /// The emission factor 1 + ¾τ.
    pub(super) emission_factor: f64,
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
    /// Its steady state's surface mean, K.
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
    /// By bisection, so that the steady state's surface mean is the inputs'.
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
    /// The emission factor 1 + ¾τ.
    pub(super) emission_factor: f64,
    /// How τ was found.
    pub(super) optical_depth: OpticalDepth,
    /// Whether the steady state τ was solved on met [`STEADY_TOLERANCE_K`]: true for a body
    /// with neither light nor heat, which has none.
    pub(super) steady_converged: bool,
    /// That steady state's surface mean, K ([`Fitted::surface_mean_k`]): the inputs' mean for a
    /// body with neither light nor heat.
    pub(super) steady_surface_mean_k: f64,
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
    #[cfg(test)]
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

    /// Each point-quantile's transmission through the column above it at the emission factor
    /// `emission_factor`, e^(−(3/2) `τ_z`) with `τ_z` = τ e^(−z ÷ 2H) and τ = (x − 1) ÷ ¾ the model's
    /// own (plan 14's τ ∝ √p, `optical_depth`, and its diffusivity
    /// [`FLUX_OPTICAL_DEPTH_PER_GREY`]), each through [`math::exp`].
    #[must_use]
    fn transmissions(&self, emission_factor: f64) -> Vec<[f64; QUANTILES]> {
        let tau = (emission_factor - 1.0) / EDDINGTON_COEFFICIENT;
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
    /// §1.4, g2), which the caller gives; land's capacity reads no temperature.
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
    /// in `storage`, over the ground `relief` at that factor (every point at the sea level where
    /// `None`), split into the latitude solve and then the longitude one (the
    /// [module documentation](self)); the largest change, K.
    #[expect(
        clippy::too_many_arguments,
        reason = "a step reads the grid, the forcing, the storage, the table, the relief and the \
                  state, and writes the work arrays; bundling them would only rename them"
    )]
    fn advance(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        storage: Storage<'_>,
        table: Option<&Tabulated>,
        relief: Option<ReliefAt<'_>>,
        t_k: &mut [f64],
        work: &mut Work,
    ) -> f64 {
        let lambda = self.transport_kg_m2_s;
        let columns = grid.columns();
        let raised = |p: usize| relief.and_then(|at| Some((at.relief, at.raised(p)?)));
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
                let tk = t_k[p];
                // The sea's temperature for its capacity is its water surface's.
                let (
                    Emission {
                        flux_w_m2: emitted,
                        slope_w_m2_k: emission_slope,
                    },
                    sea_k,
                ) = match raised(p) {
                    None => (emission(tk, emission_factor), tk),
                    Some((relief, (heights, transmission))) => (
                        point_emission(tk, emission_factor, heights, transmission, relief),
                        relief.water_k(tk, heights),
                    ),
                };
                let stored = match storage {
                    Storage::Relaxation => RELAXATION * emission_slope,
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
            emission_factor,
            insolation_w_m2,
            start_k,
            None,
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
            emission_factor,
            insolation_w_m2,
            start_k,
            None,
            Some(relief),
            STEADY_ITERATIONS,
        )
    }

    /// [`steady_state`](Self::steady_state) for at most `iterations`, its functions read from
    /// `table` where it holds them, over the ground `relief` (every point at the sea level where
    /// `None`), its transmissions taken once at the emission factor.
    #[expect(
        clippy::too_many_arguments,
        reason = "the steady state's grid, forcing and start, and the step's table, relief and \
                  count; bundling them would only rename them"
    )]
    #[must_use]
    fn relax(
        &self,
        grid: &ModelGrid,
        emission_factor: f64,
        insolation_w_m2: &[f64],
        start_k: Vec<f64>,
        table: Option<&Tabulated>,
        relief: Option<&Relief>,
        iterations: u32,
    ) -> Steady {
        let transmission = relief.map(|relief| relief.transmissions(emission_factor));
        let at = relief
            .zip(transmission.as_deref())
            .map(|(relief, transmission)| ReliefAt {
                relief,
                transmission,
            });
        let mut t_k = start_k;
        let mut work = Work::new(grid);
        for _ in 0..iterations {
            let change = self.advance(
                grid,
                emission_factor,
                insolation_w_m2,
                Storage::Relaxation,
                table,
                at,
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
    /// steady state there, and how τ was found ([`fit`](Self::fit), at the sea level).
    #[cfg(test)]
    #[must_use]
    pub(super) fn emission_factor(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
    ) -> Fitted {
        self.fit(grid, insolation_w_m2, mean_k, None)
    }

    /// The emission factor 1 + ¾τ at which the annual-mean steady state under `insolation_w_m2`
    /// over the ground `relief` (every point at the sea level where `None`), each solve from an
    /// isothermal start at `mean_k`, has the surface mean `mean_k`, K ([`Relief::surface_mean_k`],
    /// decision-r09-t13a-relief §1.3): 48 bisections of ln(1 + ¾τ); the steady state there, and
    /// how τ was found.
    ///
    /// The fold guard (decision-r09-t13ge §1.4, g4): the bisection keeps, on each side of its
    /// bracket, the last probe whose steady state converged, the bracket's two ends among them. If
    /// the final midpoint's steady state has not converged within [`STEADY_ITERATIONS`], it
    /// answers whichever kept probe's surface mean lies nearer `mean_k`, ties to the warm side
    /// ([`OpticalDepth::Folded`]), or the final midpoint where it kept none; otherwise the
    /// midpoint, as before, to the bit. On a world of one month the steady state is the climate,
    /// and a steady state that had not converged would be a transient between two branches.
    #[must_use]
    fn fit(
        &self,
        grid: &ModelGrid,
        insolation_w_m2: &[f64],
        mean_k: f64,
        relief: Option<&Relief>,
    ) -> Fitted {
        let solve = |factor: f64| {
            let steady = self.relax(
                grid,
                factor,
                insolation_w_m2,
                vec![mean_k; grid.points()],
                None,
                relief,
                STEADY_ITERATIONS,
            );
            let surface = match relief {
                Some(relief) => relief.surface_mean_k(grid, &steady.t_k),
                None => grid.mean(&steady.t_k),
            };
            Probe {
                emission_factor: factor,
                surface_mean_k: surface,
                steady,
            }
        };
        let fitted = |probe: Probe, optical_depth: OpticalDepth| Fitted {
            emission_factor: probe.emission_factor,
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

    /// Whole seasonal orbits of `sunlight` at emission factor `emission_factor` from `start_k`, K,
    /// over points of sea share `sea_share` and the ground `relief` (every point at the sea level
    /// where `None`), until the annual means move under [`CONVERGENCE_K`] between orbits, at most
    /// [`MAX_ORBITS`]: the last orbit's annual and monthly means. The table reaches as far below
    /// and above the start as the highest and deepest quantiles' grounds need.
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
        relief: Option<&Relief>,
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
        let (below, above) = relief.map_or((0.0, 0.0), Relief::table_reach_k);
        let table = Tabulated::new(self, low - below, high + above);
        let start_k = if sunlight.varies_in_longitude() {
            self.relax(
                grid,
                emission_factor,
                sunlight.annual(),
                start_k,
                Some(&table),
                relief,
                SPIN_UP_ITERATIONS,
            )
            .t_k
        } else {
            start_k
        };
        let transmission = relief.map(|relief| relief.transmissions(emission_factor));
        let at = relief
            .zip(transmission.as_deref())
            .map(|(relief, transmission)| ReliefAt {
                relief,
                transmission,
            });
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
                    at,
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
    /// points of `grid` of sea share `sea_share` and the ground `ground` (the
    /// [module documentation](self)): τ solved on the zonal grid in the grid's coordinates, over
    /// the zonal relief, so that the steady state's surface mean is `mean_k`, then the orbits over
    /// the grid's own relief.
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
    ) -> Solution {
        let (on_grid, on_zonal) = match ground {
            Ground::SeaLevel => (None, None),
            Ground::Relief { grid, zonal } => (Some(grid), Some(zonal)),
        };
        let months = usize::from(sunlight.months());
        let zonal = ModelGrid::zonal();
        let annual = grid.band_means(sunlight.annual());
        let forcing = zonal.mean(&annual) + self.heat_flow_w_m2;
        let solution = if positive(forcing) {
            let Fitted {
                emission_factor: factor,
                steady,
                surface_mean_k,
                optical_depth: depth,
            } = self.fit(&zonal, &annual, mean_k, on_zonal);
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
                self.orbits(grid, sunlight, factor, steady, sea_share, on_grid)
            };
            Solution {
                climate,
                emission_factor: factor,
                optical_depth: depth,
                steady_converged: converged,
                steady_surface_mean_k: surface_mean_k,
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
                steady_converged: true,
                steady_surface_mean_k: mean_k,
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

/// The sea-level column of a point at one temperature, what each of its quantiles' outgoing
/// radiation reads: σT⁴ ÷ (1 + ¾τ) and its slope ([`emission`]), T³ and T⁴.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Column {
    emission: Emission,
    cube: f64,
    fourth: f64,
}

impl Column {
    /// The column at the sea-level temperature `t_k`, K, under emission factor `emission_factor`.
    #[must_use]
    fn at(t_k: f64, emission_factor: f64) -> Self {
        let cube = t_k * t_k * t_k;
        Self {
            emission: emission(t_k, emission_factor),
            cube,
            fourth: cube * t_k,
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
    if through >= ground {
        Emission {
            flux_w_m2: through,
            slope_w_m2_k: column.slope_w_m2_k
                - transmission * 4.0 * STEFAN_BOLTZMANN * (cube - ground_cube * ground_rate),
        }
    } else {
        Emission {
            flux_w_m2: ground,
            slope_w_m2_k: transmission * 4.0 * STEFAN_BOLTZMANN * ground_cube * ground_rate,
        }
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
    match heights.shape {
        Shape::SeaLevel | Shape::Level => at(0),
        Shape::Varied => {
            let (flux, slope) = (0..QUANTILES).fold((0.0, 0.0), |(flux, slope), q| {
                let quantile = at(q);
                (flux + quantile.flux_w_m2, slope + quantile.slope_w_m2_k)
            });
            Emission {
                flux_w_m2: flux * QUANTILE_WEIGHT,
                slope_w_m2_k: slope * QUANTILE_WEIGHT,
            }
        }
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
/// `grid` (the zonal or the geographic grid) of sea share `sea_share` and the ground `ground`,
/// under the diurnal-mean sunlight, at a nominal step of `step`
/// ([`NOMINAL_STEP`](super::sun::NOMINAL_STEP) in the pass).
#[must_use]
pub(super) fn solve(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::of(inputs, grid, step);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
    )
}

/// The model's answer for the body of `inputs`, locked 1:1 to its seasonal host, over the points
/// of `grid` (the locked grid) in tidally locked coordinates, of sea share `sea_share` and the
/// ground `ground` (its zonal relief the tidally locked bands'), under the host's beam
/// ([`Sunlight::locked`]), at a nominal step of `step`.
#[must_use]
pub(super) fn solve_locked(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    sea_share: &[f64],
    step: Seconds,
    ground: Ground<'_>,
) -> Solution {
    let model = EnergyBalance::for_inputs(inputs);
    let sunlight = Sunlight::locked(inputs, grid, Coordinates::TidallyLocked, step);
    model.run(
        grid,
        &sunlight,
        inputs.mean_surface_temperature().value(),
        sea_share,
        ground,
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
        let none = model.run(&grid, &dark, 40.0, &[0.0; LATITUDE_BANDS], Ground::SeaLevel);
        assert_eq!(none.optical_depth, OpticalDepth::Unforced);
        assert!(
            none.climate
                .annual_k
                .iter()
                .all(|t| (t - 40.0).abs() < 1e-12)
        );
        model.heat_flow_w_m2 = 0.0916;
        let warm = model.run(&grid, &dark, 60.0, &[0.0; LATITUDE_BANDS], Ground::SeaLevel);
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
        let table = Tabulated::new(&model, 200.0, 320.0);
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
            for scale_heights in [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0] {
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
                    if scale_heights == 0.0 {
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
