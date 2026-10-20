//! Radiative equilibrium with thermal inertia (R09.T13.c, its thin air R09.T13.h's; Design note
//! 8): the climate of airless and thin-air worlds, which
//! [`ClimateModelKind::RadiativeEquilibrium`](hyperion_surface::field::ClimateModelKind::RadiativeEquilibrium)
//! names, built to the lane's research for it (research-c, in the lane's scratch directory,
//! `research-c/notes.md`).
//!
//! # The column
//!
//! Every grid point, and each of a few end columns, is a column of regolith of constant thermal
//! inertia Γ, infrared emissivity ε and Bond albedo A over the body's internal heat flow Q: Hayne
//! et al.'s (2017, JGR Planets 122, 2371, Appendix A, read) heat equation ρc ∂T/∂t = ∂/∂z (k ∂T/∂z)
//! (their eq. A1), with the surface's balance −k ∂T/∂z = F − εσT⁴ (A7) and the bottom's
//! k ∂T/∂z = Q (A12), at constant k and ρc. F is the absorbed sunlight (1 − A) S max(0, cos z), the
//! cosmic background's εσ`T_CMB`⁴ and, under a thin air, the share ε of its slab's back radiation.
//! With constant properties only Γ = √(kρc) and the period P reach the surface: in depths of the
//! skin depth `z_s` = √(κP ÷ π) (their eq. A31), a node's heat capacity per area is Γ √(P ÷ π)
//! times its share of skin depths and a pair's conductance Γ √(π ÷ P) over their spacing, so the
//! column needs no density or heat capacity of its own (research-c §2.2).
//!
//! Γ is plan 14's for the body's surface material, ε plan 14's regolith's
//! ([`ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S`],
//! [`ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S`], [`REGOLITH_EMISSIVITY`]), so that the
//! climate's nights are plan 14's, and A plan 14's Bond albedo of the body's surface state on its
//! material ([`SurfaceState::albedo`]), the one its mean temperature is computed with. The nodes
//! lie on Hayne et al.'s geometric grid (eqs. A32–A33), the first spacing a fifth of the skin depth
//! of the shorter of the solar day and the year ([`FIRST_SPACING_SKIN_DEPTHS`]), each next 4/3 of
//! the last ([`SPACING_GROWTH`]), down to six skin depths of the solar day or two of the year,
//! whichever is deeper ([`BOTTOM_DIURNAL_SKIN_DEPTHS`], [`BOTTOM_ANNUAL_SKIN_DEPTHS`]).
//!
//! # The step
//!
//! Backward Euler in finite volumes, the surface node holding half the first spacing. The
//! column's tridiagonal system is eliminated from the bottom up, Tᵢ = pᵢ Tᵢ₋₁ + rᵢ, which leaves
//! the surface node's own balance, linear in T₀ but for its emission, solved by Newton's method
//! each step ([`surface_temperatures`]); the column below follows by substitution. The exact surface
//! is what lets a step be long: at 48 steps a solar day ([`STEPS_PER_SOLAR_DAY`]) the lunar
//! equator's noon, predawn and mean lie within 0.6 K of a step forty times shorter, where a surface
//! linearised about the step's start lies 4 K off (research-c §2.2).
//!
//! # The clock
//!
//! Design note 8's diurnal-mean sun does not serve this model: radiative balance under a day's mean
//! light puts the Moon's equator near 291 K, where its true mean over the day is about 215 K. So the
//! day is resolved, and the records are its means over local time (research-c §2.3). The year's
//! months are the field's, twelve equal spans of eccentric anomaly from periapsis
//! ([`month_edges`]), and every step is binned in its month; the sun's distance and declination
//! advance in true time through each month.
//!
//! - **A world that turns under its sun, every month at least a solar day long** (the Moon, Mars,
//!   Ceres): each month runs whole solar days, as many as it holds up to [`MAX_DAYS_PER_MONTH`], at
//!   [`STEPS_PER_SOLAR_DAY`], every day from midnight, the sun moving through the whole month in
//!   true time over them. Whole days sample every local time evenly, so one column per band holds
//!   the climatology at every longitude. The layers below [`COMPRESSED_SKIN_DEPTHS`] skin depths
//!   store heat at the days' share of the month, so that the deep column sees the month's true
//!   length (research-c §2.3: monthly means within 1.5 K of the full march at 85°).
//! - **A world whose solar day is longer than one of its months** (a 3:2 Mercury, a slow rotator):
//!   true time, each month in steps of at most a 48th of the solar day and at least
//!   [`MIN_STEPS_PER_MONTH`], with [`PHASE_COLUMNS`] columns per band at hour angles evenly spaced
//!   round the day, a month's record the mean over them all: a climatology over local time
//!   (research-c §2.3: within 0.9 K of 72 phases).
//! - **A world locked 1:1 to its host** (`Forcing::Locked`, or no solar day): no day, so each point
//!   of the tidally locked grid marches its own column over the seasonal orbit in
//!   [`MIN_STEPS_PER_MONTH`] steps a month, lit by the host's beam (T13.b's [`HostTrack`]); in a
//!   one-month year, or with no orbit at all, the column is in radiative balance with its constant
//!   light, the closed form εσT⁴ = F + Q, its night the heat flow's floor, under a thin air its
//!   balance with the slab ([`ThinAir::balance`]).
//!
//! The run starts every column at [`SPIN_UP_START`] of its closed form's temperature under its
//! annual-mean sunlight ([`ThinAir::balance`]: its radiative temperature on an airless world; all
//! of it on a locked world), runs one orbit, then jumps every layer so that its mean over the
//! orbit is the surface's plus the heat flow's gradient (Kieffer 2013, JGR Planets 118, 451,
//! §3.2.5 [108]–[109], read: "the condition for no net heat flow with temperature-independent
//! thermal conductivity"), the surface's mean being the column's long run's ([`long_run_means`]:
//! a phase column of a 3:2 Mercury takes the hour angles of the one half a day round in its next
//! orbit), and runs whole orbits until no point's annual mean moves by [`CONVERGENCE_K`] from the
//! orbit before, at most [`MAX_ORBITS`]. A run's columns advance in groups of [`LANES`], each
//! lane's arithmetic a lone column's.
//!
//! # The thin air
//!
//! A positive surface pressure (the reference Mars's) adds Kieffer's one-layer grey slab with no
//! solar heating (2013, §2.2.3 eq. 5 and [43]–[46], read), balanced and bounded, with its snowfall
//! (decision-r09-t13-thin-air §1.9, R09.T13.h). It absorbs a share β of the surface's emission and
//! emits βσ`T_a`⁴ from each face, so in its own balance σ`T_a`⁴ = F̄ ÷ 2, F̄ being its column's mean
//! emission ⟨εσ`T_s`⁴⟩ over the same month of the orbit before (the slab's time constant is "a few
//! sols", his [118], shorter than a month and longer than a day); the first orbit takes a column in
//! balance with its month's sunlight under the unbounded slab, T13.c's form. Where the bulk gas
//! condenses, the slab never falls below its frost point one scale height up
//! ([`Frost::aloft_point_k`], his [46]), and where it is held there its deficit β (2σ`T_a`⁴ − F̄)
//! falls as snow on the column, frost or none on the ground ([`Slab`]), which sublimes from bare
//! ground through the frost's balance. The surface takes ε of the back radiation βσ`T_a`⁴. A slab's column is a site on the
//! whole-days clock, a phase column in true time (whose emission is carried to the hour angles it
//! takes in the next orbit, [`carried`]) and a point on a locked world. β is solved once so that the
//! model's mean in the brightness kind is plan 14's ([`ThinAir`]). Where the air's bulk gas
//! condenses, the surface never falls below its frost point at its partial pressure: Kieffer's
//! [101], "the unbalanced energy W is applied to condensation or sublimation" ([`Frost`]).
//!
//! # The answer
//!
//! [`RadiativeSolution`]: each grid point's annual and monthly means, the arithmetic climatology
//! the field records, and what R09.T13.d's normalisation in the brightness kind needs
//! (decision-r09-t13d-normalisation §2.3–§2.4): each point's moments about its previous orbit's
//! mean, its coldest and warmest samples, end columns at the equator and both poles carrying no
//! weight in the field's means, and the model's lower bound. No lapse term (that ruling's §2.4 and
//! decision-r09-t13a-relief §2.2).

use core::f64::consts::{E, PI, TAU};

use hyperion_surface::field::month_edges;

use super::coordinates::Coordinates;
use super::grid::{GridClimate, LATITUDE_BANDS, ModelGrid, index};
use super::sun::{Daylight, HostTrack, orbit_at, reduced};
use crate::math;
use crate::orbit::Eccentricity;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::hooks::surface::{
    COSMIC_BACKGROUND_TEMPERATURE, ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
    REGOLITH_EMISSIVITY, ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S, night_floor,
};
use crate::planetary::surface::inputs::{CoarseInputs, Forcing};
use crate::substance::by_key;
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{Pascals, Radians};

/// The steps of a solar day: 48 (`Adopted`; research-c §2.2, its `runs/e2b_steps.py`). With the
/// surface solved exactly each step, the lunar equator's noon, predawn and mean at 48 steps lie
/// −0.26, −0.02 and +0.57 K from 1,920 steps, and its radiative mean 0.04 K.
pub(super) const STEPS_PER_SOLAR_DAY: u32 = 48;

/// The fewest steps a month takes: 8 (`Adopted`; research-c §2.3 and §9, its
/// `runs/e11_mercury.py`, a 3:2 Mercury within 0.9 K of 24 steps a month).
pub(super) const MIN_STEPS_PER_MONTH: u32 = 8;

/// The most solar days a month runs on a world whose months are at least a solar day long: 16
/// (`Adopted`; research-c §2.3, its `runs/e6_season.py` and `e7_season2.py`: an airless world of
/// Earth's rotation within 1.5 K of the full march in every month at 85°, against 4.9 K at 8).
pub(super) const MAX_DAYS_PER_MONTH: u32 = 16;

/// The depth, in skin depths of the solar day, below which a month of whole days stores heat at
/// the days' share of the month: 2 (`Adopted`; research-c §2.3: 6 K off at 8).
pub(super) const COMPRESSED_SKIN_DEPTHS: f64 = 2.0;

/// The columns per band of a world whose solar day outlasts a month, at hour angles evenly spaced
/// round the day: 24 (`Adopted`; research-c §2.3, a 3:2 Mercury within 0.9 K of 72).
pub(super) const PHASE_COLUMNS: usize = 24;

/// The first spacing of a column's nodes, in skin depths: ⅕, Hayne et al.'s Δz₀ = `z_s` ÷ m (2017,
/// eq. A33, read) at m = 5 (`Adopted`; research-c §2.2: with [`SPACING_GROWTH`] and six skin depths,
/// ten nodes, every figure of the lunar cycle within 0.3 K of eighteen; Hayne et al.'s optimum
/// "close to m = 10 and n = 5").
pub(super) const FIRST_SPACING_SKIN_DEPTHS: f64 = 0.2;

/// Each spacing over the one above it: 4/3, Hayne et al.'s 1 + 1 ÷ n (2017, eq. A33) at n = 3
/// (`Adopted`; research-c §2.2; Kieffer 2013's default RLAY is 1.15, research-c's reading of KRC's
/// `master.inp`).
pub(super) const SPACING_GROWTH: f64 = 4.0 / 3.0;

/// The depth a column reaches, in skin depths of the solar day: at least 6 (`Adopted`;
/// research-c §2.2: the diurnal cycle at four is as at ten; Hayne et al. 2017 take "typically
/// ~10").
pub(super) const BOTTOM_DIURNAL_SKIN_DEPTHS: f64 = 6.0;

/// The depth a column reaches, in skin depths of the year: at least 2 (`Adopted`; research-c
/// §2.3; Kieffer 2013, [123], read: for annual effects "a total thickness of about 25 diurnal
/// skin-depths (1 annual skin-depth)", Mars's annual skin depth being 25.85 diurnal ones).
pub(super) const BOTTOM_ANNUAL_SKIN_DEPTHS: f64 = 2.0;

/// The most nodes of a column: 64, a guard that no plausible year reaches (a year of 10¹² solar
/// days takes 58).
const MAX_NODES: usize = 64;

/// The most steps a month takes in true time: 4,096, a guard (an orbit of e near 1 whose shortest
/// month is under a solar day takes at most about 2,100 in its longest).
const MAX_STEPS_PER_MONTH: u32 = 4_096;

/// The share of a column's closed-form temperature under its annual-mean sunlight
/// ([`ThinAir::balance`]; on an airless world its radiative temperature, ((F + Q) ÷ εσ)^¼) that a
/// turning world's columns start at: 0.8 (`Adopted`; research-c §2.3, a day's arithmetic mean
/// lying below its radiative one). A locked world's start at all of it.
pub(super) const SPIN_UP_START: f64 = 0.8;

/// The most orbits a run takes: 16 (`Adopted`; research-c §2.3 measured 1–2 orbits at the lunar
/// equator and 4 at its poles and on a Mercury, with the jump).
pub(super) const MAX_ORBITS: u32 = 16;

/// The move of the points' annual means between orbits under which a run has converged, K: 0.1
/// (Design note 8).
pub(super) const CONVERGENCE_K: f64 = 0.1;

/// The most Newton iterations of a surface's balance: 50, a guard; from its upper bound it takes
/// about five.
const SURFACE_ITERATIONS: u32 = 50;

/// The Newton step under which a surface's balance is solved, K: 10⁻⁹.
const SURFACE_TOLERANCE_K: f64 = 1e-9;

/// The end columns a locked world's grid carries round its geographic equator: 144, every 2.5°
/// of longitude from the substellar point, the locked grid's own spacing.
pub(super) const EQUATOR_RING_COLUMNS: usize = 144;

/// Whether the body of `inputs` is locked 1:1 to its seasonal host, whose radiative model takes
/// the tidally locked grid: its forcing is [`Forcing::Locked`], or it has no solar day.
#[must_use]
pub(super) fn locked(inputs: &CoarseInputs) -> bool {
    inputs.climate().forcing == Forcing::Locked || inputs.spin().solar_day.is_none()
}

/// The grid and coordinates of a pointwise model, the radiative or the isothermal one, for the
/// body of `inputs`: the tidally locked grid on a world locked 1:1 to its host ([`locked`]), and
/// the zonal grid of 72 bands otherwise, whose forcing is the same at every longitude.
#[must_use]
pub(super) fn frame(inputs: &CoarseInputs) -> (ModelGrid, Coordinates) {
    if locked(inputs) {
        (ModelGrid::locked(), Coordinates::TidallyLocked)
    } else {
        (ModelGrid::zonal(), Coordinates::Geographic)
    }
}

/// The regolith of a body's columns and what heats them from outside the sunlight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Regolith {
    /// The infrared emissivity ε.
    pub(super) emissivity: f64,
    /// The thermal inertia Γ, J m⁻² K⁻¹ s^−½.
    pub(super) inertia_j_per_m2_k_sqrt_s: f64,
    /// The Bond albedo A.
    pub(super) albedo: f64,
    /// The internal heat flow Q, W m⁻², not negative.
    pub(super) heat_flow_w_m2: f64,
}

impl Regolith {
    /// The regolith of the body of `inputs`: plan 14's emissivity, its inertia for the body's
    /// surface material, its Bond albedo for the surface state on that material, and the heat
    /// flow, a heat flow that is not positive taken as none, as [`night_floor`] takes it.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs) -> Self {
        let material = inputs.surface_material();
        let inertia = match material {
            SurfaceMaterial::Rock => ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
            SurfaceMaterial::Ice => ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
        };
        let heat = inputs.heat_flow().value();
        Self {
            emissivity: REGOLITH_EMISSIVITY,
            inertia_j_per_m2_k_sqrt_s: inertia,
            albedo: albedo_of(inputs.surface_state(), material),
            heat_flow_w_m2: if heat > 0.0 { heat } else { 0.0 },
        }
    }

    /// εσ, W m⁻² K⁻⁴.
    #[must_use]
    fn emission(&self) -> f64 {
        self.emissivity * STEFAN_BOLTZMANN
    }

    /// The cosmic background's flux the surface absorbs, εσ`T_CMB`⁴, W m⁻².
    #[must_use]
    fn background_w_m2(&self) -> f64 {
        self.emission() * math::powi(COSMIC_BACKGROUND_TEMPERATURE.value(), 4)
    }

    /// The night floor's flux, εσ`T_CMB`⁴ + Q, W m⁻²: what a surface with no sunlight radiates.
    #[must_use]
    fn floor_flux_w_m2(&self) -> f64 {
        self.background_w_m2() + self.heat_flow_w_m2
    }

    /// What heats a column of absorbed sunlight `absorbed_w_m2` from outside the slab, W m⁻²: the
    /// sunlight, the background and the heat flow, `E_in` = S + εσ`T_CMB`⁴ + Q, summed as T13.c's
    /// radiative temperature summed them, so that an airless column's closed form is T13.c's to
    /// the bit.
    #[must_use]
    fn outside_w_m2(&self, absorbed_w_m2: f64) -> f64 {
        self.input(absorbed_w_m2, 0.0) + self.heat_flow_w_m2
    }

    /// A column's input under absorbed sunlight `absorbed_w_m2` and the slab's back radiation
    /// `down_w_m2`, W m⁻²: the sunlight, the background and ε of the back radiation.
    #[must_use]
    fn input(&self, absorbed_w_m2: f64, down_w_m2: f64) -> f64 {
        absorbed_w_m2 + self.background_w_m2() + self.emissivity * down_w_m2
    }
}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Plan 14's Bond albedo of `state` on `material` ([`SurfaceState::albedo`], P14.T13.c).
#[must_use]
fn albedo_of(state: SurfaceState, material: SurfaceMaterial) -> f64 {
    state.albedo(material).value()
}

/// The frost point where the air's bulk gas condenses on the ground, and the slab's bound aloft.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frost {
    /// The bulk gas's frost point at its partial pressure, K: the registry's saturation
    /// temperature (its phase diagram, P14.T49.b, which P14.T51.a's partition reads too).
    pub(super) point_k: f64,
    /// Its latent heat there, J kg⁻¹: the registry curve's own Clausius–Clapeyron enthalpy over
    /// the gas's molar mass.
    pub(super) latent_heat_j_per_kg: f64,
    /// The bulk gas's frost point one scale height up, at its partial pressure over e, K.
    ///
    /// The slab cannot fall below it: its air condenses until it is held there (Kieffer 2013, JGR
    /// Planets 118, 451, [46], read: "bounded at this value and the excess energy loss is converted
    /// to snow"; decision-r09-t13-thin-air §1.9). The reference Mars's
    /// carbon dioxide at 604.8 Pa ÷ e, 222.5 Pa, holds it at 141.13 K by James et al.'s curve as
    /// Kieffer gives it (the ruling's figure), and at 141.09 K by the registry's.
    pub(super) aloft_point_k: f64,
}

impl Frost {
    /// The frost of the air of `inputs`: its largest gas's, at its mole fraction of the surface
    /// pressure, where the registry holds the gas's phase diagram and molar mass and the gas
    /// condenses at that pressure, with its frost point at a scale height's lower pressure;
    /// `None` otherwise, or on an airless body.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs) -> Option<Self> {
        let pressure = inputs.surface_pressure().value();
        if !positive(pressure) {
            return None;
        }
        let bulk = inputs.gases().first()?;
        let row = by_key(bulk.substance.as_str())?.substance();
        let diagram = row.phase_diagram()?;
        let mass = row.molar_mass_g_per_mol()?;
        let partial = bulk.mole_fraction * pressure;
        let point = diagram.saturation_temperature(Pascals::new(partial))?;
        let enthalpy = diagram.clausius_clapeyron_enthalpy_j_per_mol(point)?;
        let latent = enthalpy / (mass * 1e-3);
        // A lower positive pressure than the surface's, at which the curve has a frost point
        // wherever it has one at the surface: below its coldest piece it is a line in 1 ÷ T.
        let aloft = diagram.saturation_temperature(Pascals::new(partial / E))?;
        (point.value() > 0.0 && latent > 0.0 && aloft.value() > 0.0).then_some(Self {
            point_k: point.value(),
            latent_heat_j_per_kg: latent,
            aloft_point_k: aloft.value(),
        })
    }

    /// The slab's bound σ`T_fa`⁴, W m⁻²: the blackbody flux at the frost point aloft, of which a
    /// slab there emits β from each face.
    #[must_use]
    fn aloft_blackbody_w_m2(&self) -> f64 {
        STEFAN_BOLTZMANN * math::powi(self.aloft_point_k, 4)
    }
}

/// A thin air's grey slab over the regolith (the [module documentation](self)): none on an
/// airless body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ThinAir {
    /// The slab's hemispheric infrared absorptivity β, 0 to 1: 0 with no air.
    pub(super) absorptivity: f64,
    /// The bulk gas's frost, or `None`.
    pub(super) frost: Option<Frost>,
}

impl ThinAir {
    /// No air.
    pub(super) const NONE: Self = Self {
        absorptivity: 0.0,
        frost: None,
    };

    /// The thin air of `inputs` over `regolith`, under a grid-mean annual absorbed sunlight of
    /// `absorbed_w_m2`: β solved so that the mean in the brightness kind of
    /// decision-r09-t13d-normalisation §2.2, (⟨F⟩ − `E_nf`) ÷ σ with F the emitted flux and `E_nf`
    /// the night floor's, is plan 14's mean T\*, by energy with no frost and no bound: a periodic
    /// column under the balanced slab emits a mean ⟨F⟩ with ⟨F⟩ (1 − εβ ÷ 2) = ⟨S⟩ + `E_nf`, so
    /// β = (2 ÷ ε) (1 − (⟨S⟩ + `E_nf`) ÷ (σT\*⁴ + `E_nf`)), held to [0, 1]
    /// (decision-r09-t13-thin-air §1.9 item 4).
    ///
    /// The reference Mars's (T\* 214 K, ⟨S⟩ 103.9 W m⁻², a `T_eq` of 206.9 K at plan 14's temperate
    /// albedo) is 0.265, above Kieffer's clear carbon dioxide's 0.163 at 636 Pa (2013, eq. 4 with
    /// his C₁ 0.11 at 700 Pa, as research-c §8 computed it), because the brightness kind lays the
    /// whole 7 K of plan 14's greenhouse on the slab (R09's Risks, "Deferred findings (2026-10-13
    /// rule) (R09.T13.c)"). The frost's and the snow's remainders are R09.T13.d's constant.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, regolith: &Regolith, absorbed_w_m2: f64) -> Self {
        if !positive(inputs.surface_pressure().value()) {
            return Self::NONE;
        }
        let floor = regolith.floor_flux_w_m2();
        let target = STEFAN_BOLTZMANN * math::powi(inputs.mean_surface_temperature().value(), 4);
        let beta = 2.0 / regolith.emissivity * (1.0 - (absorbed_w_m2 + floor) / (target + floor));
        // By comparison, so that a NaN takes no slab.
        let absorptivity = if beta > 1.0 {
            1.0
        } else if beta > 0.0 {
            beta
        } else {
            0.0
        };
        Self {
            absorptivity,
            frost: Frost::of(inputs),
        }
    }

    /// The balanced bounded slab over a column whose mean emission over a month is
    /// `mean_emission_w_m2`, F̄ = ⟨εσ`T_s`⁴⟩ (decision-r09-t13-thin-air §1.9 items 1–3).
    ///
    /// In its own balance with no solar heating the slab absorbs βF̄ and emits βσ`T_a`⁴ from each
    /// face (Kieffer 2013, [43] and eq. 5 with no net heating), so σ`T_a`⁴ = F̄ ÷ 2. Where the bulk
    /// gas condenses it never falls below its frost point aloft, σ`T_a`⁴ = max(F̄ ÷ 2, σ`T_fa`⁴)
    /// ([`Frost::aloft_point_k`], his [46]), and its deficit β (2σ`T_a`⁴ − F̄) falls as snow. Its
    /// back radiation is βσ`T_a`⁴.
    #[must_use]
    pub(super) fn slab(&self, mean_emission_w_m2: f64) -> Slab {
        let half = 0.5 * mean_emission_w_m2;
        let (blackbody, snow) = match self.frost {
            Some(frost) if half < frost.aloft_blackbody_w_m2() => {
                let bound = frost.aloft_blackbody_w_m2();
                (
                    bound,
                    self.absorptivity * (2.0 * bound - mean_emission_w_m2),
                )
            }
            Some(_) | None => (half, 0.0),
        };
        Slab {
            mean_emission_w_m2,
            blackbody_w_m2: blackbody,
            down_w_m2: self.absorptivity * blackbody,
            snow_w_m2: snow,
        }
    }

    /// The balanced column of emissivity `emissivity` under a constant input from outside the
    /// slab, `outside_w_m2` = S + εσ`T_CMB`⁴ + Q, `E_in`: the march's own steady state under
    /// constant light (decision-r09-t13-thin-air §1.9 item 5, as decision-r09-t13h-scope's "§1.9
    /// item 5 amended" replaces it).
    ///
    /// With F = εσT⁴, A = σ`T_fa`⁴ and B = εσ`T_f`⁴, F = max(B, G), G the bare column's balance:
    /// `E_in` ÷ (1 − εβ ÷ 2), T13.c's form with the background and the heat flow under the slab, where
    /// `E_in` ≥ (2 − εβ) A, the slab unbounded (everywhere, with no bound); and below that, the slab
    /// bounded and its snow β (2A − F) sublimed by the ground as the march's is (§1.9 item 3;
    /// decision-r09-t13h-scope §2), (`E_in` − (2 − ε) βA) ÷ (1 − β), for β < 1 only, the column at
    /// β = 1 held at B. Where F = B the column is held at its frost point and its frost grows, and
    /// its back radiation is β max(F ÷ 2, A): βσ`T_fa`⁴ wherever B < 2A, as on Mars.
    #[must_use]
    pub(super) fn balance(&self, emissivity: f64, outside_w_m2: f64) -> Balanced {
        let emission = emissivity * STEFAN_BOLTZMANN;
        let beta = self.absorptivity;
        let eb = emissivity * beta;
        let unbounded = 2.0 * outside_w_m2 / (2.0 - eb);
        let open = |emitted: f64| Balanced {
            surface_k: (emitted / emission).sqrt().sqrt(),
            slab: self.slab(emitted),
        };
        let Some(frost) = self.frost else {
            return open(unbounded);
        };
        let bound = frost.aloft_blackbody_w_m2();
        let frosted = emission * math::powi(frost.point_k, 4);
        let bare = if outside_w_m2 >= (2.0 - eb) * bound {
            Some(unbounded)
        } else if beta < 1.0 {
            Some((outside_w_m2 - (2.0 - emissivity) * beta * bound) / (1.0 - beta))
        } else {
            None
        };
        match bare {
            Some(emitted) if emitted > frosted => open(emitted),
            Some(_) | None => Balanced {
                surface_k: frost.point_k,
                slab: self.slab(frosted),
            },
        }
    }
}

/// A column in balance under constant light ([`ThinAir::balance`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Balanced {
    /// The surface temperature, K.
    pub(super) surface_k: f64,
    /// The slab over it.
    pub(super) slab: Slab,
}

/// A thin air's slab over one column in one month (decision-r09-t13-thin-air §1.9): the
/// [`ThinAir::slab`] of the column's mean emission.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "each is a flux in W m⁻², its unit in its name as the crate's rules ask"
)]
pub(super) struct Slab {
    /// The column's mean emission F̄ it was formed from, W m⁻².
    pub(super) mean_emission_w_m2: f64,
    /// σ`T_a`⁴, W m⁻²: the blackbody flux at the slab's temperature, of which the slab emits β
    /// from each face (Kieffer 2013, [43]).
    pub(super) blackbody_w_m2: f64,
    /// Its back radiation R⇓ = βσ`T_a`⁴, W m⁻², of which the surface absorbs ε.
    pub(super) down_w_m2: f64,
    /// Its deficit β (2σ`T_a`⁴ − F̄), W m⁻², the latent heat of the snow it condenses.
    ///
    /// It is zero but where the slab is bounded. Its snow, this over the frost's latent heat,
    /// falls on the column at every step of the month over the step's true duration, frost or none
    /// on the ground.
    pub(super) snow_w_m2: f64,
}

impl Slab {
    /// The slab's temperature `T_a`, K.
    #[cfg(test)]
    #[must_use]
    pub(super) fn temperature_k(&self) -> f64 {
        (self.blackbody_w_m2 / STEFAN_BOLTZMANN).sqrt().sqrt()
    }
}

/// A column's nodes: their depths, heat capacities, conductances and the heat flow's steady
/// offsets.
#[derive(Debug, Clone, PartialEq)]
struct Layers {
    /// Each node's depth, in skin depths of the column's period.
    depth: Vec<f64>,
    /// Each node's heat capacity per area, J m⁻² K⁻¹: Γ √(P ÷ π) times its share of skin depths,
    /// half the first spacing at the surface, half the last at the bottom, the mean of the two
    /// beside it elsewhere.
    capacity_j_per_m2_k: Vec<f64>,
    /// The conductance between each node and the next, W m⁻² K⁻¹: Γ √(π ÷ P) over their spacing.
    conductance_w_per_m2_k: Vec<f64>,
    /// Each node's temperature over the surface's in the heat flow's steady state, Q Σ 1 ÷ G, K.
    geothermal_k: Vec<f64>,
    /// The first node below [`COMPRESSED_SKIN_DEPTHS`] skin depths of the solar day.
    first_deep: usize,
}

impl Layers {
    /// The nodes of a column of thermal inertia `inertia_j_per_m2_k_sqrt_s` whose skin depth is the
    /// period `period_s`'s, s, down to `bottom` of its skin depths, compressed below `deep` of
    /// them, under a heat flow of `heat_flow_w_m2`.
    #[must_use]
    fn new(
        inertia_j_per_m2_k_sqrt_s: f64,
        period_s: f64,
        bottom: f64,
        deep: f64,
        heat_flow_w_m2: f64,
    ) -> Self {
        let mut depth = vec![0.0];
        let mut spacing = Vec::new();
        let mut gap = FIRST_SPACING_SKIN_DEPTHS;
        while depth.last().is_some_and(|&z| z < bottom) && depth.len() < MAX_NODES {
            let next = depth.last().map_or(0.0, |&z| z + gap);
            depth.push(next);
            spacing.push(gap);
            gap *= SPACING_GROWTH;
        }
        let nodes = depth.len();
        let (storing, conducting) = (
            inertia_j_per_m2_k_sqrt_s * (period_s / PI).sqrt(),
            inertia_j_per_m2_k_sqrt_s * (PI / period_s).sqrt(),
        );
        let capacity = (0..nodes)
            .map(|i| {
                let above = if i > 0 { spacing[i - 1] } else { 0.0 };
                let below = spacing.get(i).copied().unwrap_or(0.0);
                storing * 0.5 * (above + below)
            })
            .collect();
        let conductance: Vec<f64> = spacing.iter().map(|gap| conducting / gap).collect();
        let mut geothermal = Vec::with_capacity(nodes);
        let mut offset = 0.0;
        geothermal.push(offset);
        for g in &conductance {
            offset += heat_flow_w_m2 / g;
            geothermal.push(offset);
        }
        let first_deep = depth.iter().position(|&z| z > deep).unwrap_or(nodes);
        Self {
            depth,
            capacity_j_per_m2_k: capacity,
            conductance_w_per_m2_k: conductance,
            geothermal_k: geothermal,
            first_deep,
        }
    }

    /// The column's nodes.
    #[must_use]
    fn nodes(&self) -> usize {
        self.depth.len()
    }
}

/// A month's implicit system: each node's storage over the step, and the bottom-up elimination's
/// factors.
#[derive(Debug, Clone, PartialEq)]
struct Implicit {
    /// Each node's heat capacity over the step, W m⁻² K⁻¹, the deep ones' scaled by the month's
    /// compression.
    storage_w_per_m2_k: Vec<f64>,
    /// pᵢ of Tᵢ = pᵢ Tᵢ₋₁ + rᵢ, for each node but the surface.
    factor: Vec<f64>,
    /// The inverse of each node's eliminated diagonal, but the surface's.
    inverse: Vec<f64>,
    /// The surface node's eliminated diagonal, the linear part of its balance, W m⁻² K⁻¹.
    surface_w_per_m2_k: f64,
}

impl Implicit {
    /// The system of `layers` over a step of `step_s` seconds, its nodes below
    /// [`Layers::first_deep`] storing `compression` of their heat capacity.
    #[must_use]
    fn new(layers: &Layers, step_s: f64, compression: f64) -> Self {
        let nodes = layers.nodes();
        let g = &layers.conductance_w_per_m2_k;
        let storage: Vec<f64> = layers
            .capacity_j_per_m2_k
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let scale = if i >= layers.first_deep {
                    compression
                } else {
                    1.0
                };
                c * scale / step_s
            })
            .collect();
        let (mut factor, mut inverse) = (vec![0.0; nodes], vec![0.0; nodes]);
        let last = nodes - 1;
        inverse[last] = 1.0 / (storage[last] + g[last - 1]);
        factor[last] = g[last - 1] * inverse[last];
        for i in (1..last).rev() {
            let diagonal = storage[i] + g[i - 1] + g[i] - g[i] * factor[i + 1];
            inverse[i] = 1.0 / diagonal;
            factor[i] = g[i - 1] * inverse[i];
        }
        let surface = storage[0] + g[0] - g[0] * factor[1];
        Self {
            storage_w_per_m2_k: storage,
            factor,
            inverse,
            surface_w_per_m2_k: surface,
        }
    }
}

/// The columns a run advances together, in the same loops: 8 (`Adopted`). Each column's
/// elimination and its surface's Newton iteration are serial chains, so one column at a time waits
/// on its own arithmetic; the lanes' chains run side by side, every lane's arithmetic the lone
/// column's, to the bit, so that no result depends on its group (a test holds the two equal on
/// every clock: whole days, true time and a locked world's beam). On
/// the reference Mars the step took 340 ms one column at a time, 148 ms in fours and 130 ms in
/// eights (as built, at opt-level 2 on a shared machine).
pub(super) const LANES: usize = 8;

/// The surface temperatures, K, that solve each lane's balance `linear` T + `emission` T⁴ =
/// `rhs[l]` (W m⁻²), by Newton's method from `guess[l]` (the step's starting temperature), held
/// under the lesser of the root's two upper bounds, (rhs ÷ emission)^¼ and rhs ÷ linear: the
/// balance rises and is convex for positive T, so from above the iteration falls monotonically onto
/// the root and from below its first step lands above it. Each lane stops once its step is under
/// [`SURFACE_TOLERANCE_K`], at most [`SURFACE_ITERATIONS`] times; 0 for a right side that is not
/// positive.
#[must_use]
fn surface_temperatures<const L: usize>(
    linear: f64,
    emission: f64,
    rhs: [f64; L],
    guess: [f64; L],
) -> [f64; L] {
    let mut t = [0.0; L];
    let mut done = [false; L];
    for lane in 0..L {
        let r = rhs[lane];
        if !positive(r) {
            done[lane] = true;
            continue;
        }
        let radiative = (r / emission).sqrt().sqrt();
        let conductive = r / linear;
        let bound = if conductive < radiative {
            conductive
        } else {
            radiative
        };
        // By comparison, so that a guess that is not a positive number starts at the bound.
        let g = guess[lane];
        t[lane] = if g > 0.0 && g < bound { g } else { bound };
    }
    for _ in 0..SURFACE_ITERATIONS {
        let mut all = true;
        for lane in 0..L {
            if !done[lane] {
                let v = t[lane];
                let cube = v * v * v;
                let step = (linear * v + emission * cube * v - rhs[lane])
                    / (linear + 4.0 * emission * cube);
                t[lane] = v - step;
                done[lane] = step.abs() < SURFACE_TOLERANCE_K;
            }
            all &= done[lane];
        }
        if all {
            break;
        }
    }
    t
}

/// Which end of the field's equator–pole contrast an end column stands at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum End {
    /// On the geographic equator: one column of the zonal model, which is every longitude's, or
    /// one of a locked world's ring round it.
    Equator,
    /// At the geographic north pole, the body frame's +z.
    NorthPole,
    /// At the geographic south pole.
    SouthPole,
}

/// A column's record over the converged orbit, the statistics of R09.T13.d's normalisation
/// (decision-r09-t13d-normalisation §2.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Moments {
    /// R, the column's annual mean over the orbit before, K; in a closed form, its own
    /// temperature.
    pub(super) reference_k: f64,
    /// ⟨(T − R)ᵏ⟩ for k = 1 to 4, Kᵏ, over every sample of the orbit weighted by its true
    /// duration (and over its phase columns alike).
    pub(super) about_reference: [f64; 4],
    /// The coldest sample, K.
    pub(super) coldest_k: f64,
    /// The warmest sample, K.
    pub(super) warmest_k: f64,
}

impl Moments {
    /// ⟨(T + s)⁴⟩ for a shift `shift_k` of every sample, K⁴: Σₖ C(4, k) (R + s)⁴⁻ᵏ Mₖ, M₀ = 1.
    #[must_use]
    pub(super) fn fourth_power_mean(&self, shift_k: f64) -> f64 {
        let x = self.reference_k + shift_k;
        let [m1, m2, m3, m4] = self.about_reference;
        x * x * x * x + 4.0 * x * x * x * m1 + 6.0 * x * x * m2 + 4.0 * x * m3 + m4
    }

    /// ⟨(T + s)³⟩ for a shift `shift_k` of every sample, K³: Σₖ C(3, k) (R + s)³⁻ᵏ Mₖ, M₀ = 1, a
    /// quarter of [`fourth_power_mean`](Self::fourth_power_mean)'s slope in s (R09.T13.d's solve,
    /// decision-r09-t13d-normalisation §2.4).
    #[must_use]
    pub(super) fn third_power_mean(&self, shift_k: f64) -> f64 {
        let x = self.reference_k + shift_k;
        let [m1, m2, m3, _] = self.about_reference;
        x * x * x + 3.0 * x * x * m1 + 3.0 * x * m2 + m3
    }
}

/// An end column (decision-r09-t13d-normalisation §2.3): one at the field's equator or a pole,
/// with its own records, carrying no weight in the field's means.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EndColumn {
    /// Where it stands.
    pub(super) end: End,
    /// Its unit vector in the body frame.
    pub(super) direction: [f64; 3],
    /// Its annual mean, K.
    pub(super) annual_k: f64,
    /// Each month's mean, K: twelve, or one in a one-month year.
    pub(super) months_k: Vec<f64>,
    /// Its moments.
    pub(super) moments: Moments,
}

/// The radiative model's answer (the [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RadiativeSolution {
    /// Each grid point's annual and monthly means, with the orbits run.
    pub(super) climate: GridClimate,
    /// The model's grid: the zonal one, or the tidally locked one.
    pub(super) grid: ModelGrid,
    /// The grid's coordinates.
    pub(super) coordinates: Coordinates,
    /// Each grid point's moments.
    pub(super) moments: Vec<Moments>,
    /// The end columns: on the zonal grid the equator and the north and south poles; on the
    /// locked grid the two poles and [`EQUATOR_RING_COLUMNS`] round the equator.
    pub(super) ends: Vec<EndColumn>,
    /// The model's lower bound `T_lo`, K: the night floor ([`night_floor`]), or the frost point
    /// where that is warmer.
    pub(super) lower_bound_k: f64,
    /// The regolith's emissivity ε, whose εσ⟨T⁴⟩ is a column's emitted flux.
    pub(super) emissivity: f64,
    /// The night floor's flux `E_nf` = εσ`T_CMB`⁴ + Q, W m⁻².
    pub(super) floor_flux_w_m2: f64,
    /// The thin air's slab and frost.
    pub(super) air: ThinAir,
}

/// What the sunlight does over a run's orbit.
#[derive(Debug, Clone, PartialEq)]
enum Light {
    /// Whole solar days, the same every orbit: each step's absorbed sunlight at each site, W m⁻²,
    /// `step * sites + site`.
    Table(Vec<f64>),
    /// True time: each step's absorbed flux at normal incidence, (1 − A) F, W m⁻², and its hour
    /// angles from the orbit's start at phase 0, and each step's daylight at each site,
    /// `step * sites + site`. Each orbit turns the hour angles on by its solar days.
    Turning {
        normal_w_m2: Vec<f64>,
        start_rad: Vec<f64>,
        span_rad: Vec<f64>,
        daylight: Vec<Daylight>,
        days_per_orbit: f64,
    },
    /// The host's beam on a locked world: each step's absorbed flux at normal incidence, W m⁻²,
    /// and its direction in the model frame.
    Beam {
        normal_w_m2: Vec<f64>,
        direction: Vec<[f64; 3]>,
    },
}

/// One orbit of a run's steps.
#[derive(Debug, Clone, PartialEq)]
struct Schedule {
    /// The months, 1 or 12.
    months: usize,
    /// Each month's share of the orbit's time.
    month_share: Vec<f64>,
    /// Each month's implicit system.
    implicit: Vec<Implicit>,
    /// Each step's month.
    month_of: Vec<usize>,
    /// Each step's share of the orbit's time.
    share: Vec<f64>,
    /// Each step's true duration, s, over which a frost condenses or sublimes.
    duration_s: Vec<f64>,
    /// Each step's middle, its mean anomaly from periapsis, rad.
    mean_anomaly_rad: Vec<f64>,
    /// The columns of each site, at hour angles evenly spaced round the day.
    phases: usize,
    /// The sunlight.
    light: Light,
    /// Each month's mean absorbed sunlight at each site, W m⁻², `month * sites + site`.
    month_absorbed: Vec<f64>,
}

impl Schedule {
    /// Each month's steps.
    #[must_use]
    fn steps_in_months(&self) -> Vec<u32> {
        let mut steps = vec![0_u32; self.months];
        for &month in &self.month_of {
            steps[month] += 1;
        }
        steps
    }
}

/// How a run's orbit is stepped.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Clock {
    /// Whole solar days of `day_s` seconds a month.
    WholeDays { day_s: f64 },
    /// True time, under a solar day of `day_s` seconds.
    TrueTime { day_s: f64 },
    /// No day: a world locked 1:1 to its host.
    Locked,
}

/// One column's running statistics over an orbit, about its reference R.
#[derive(Debug, Clone, PartialEq)]
struct Tally {
    reference_k: f64,
    weight: f64,
    sums: [f64; 4],
    coldest_k: f64,
    warmest_k: f64,
    month_sums: [f64; 12],
    month_weights: [f64; 12],
}

impl Tally {
    /// An empty tally about `reference_k`.
    #[must_use]
    fn new(reference_k: f64) -> Self {
        Self {
            reference_k,
            weight: 0.0,
            sums: [0.0; 4],
            coldest_k: f64::INFINITY,
            warmest_k: f64::NEG_INFINITY,
            month_sums: [0.0; 12],
            month_weights: [0.0; 12],
        }
    }

    /// Adds a sample `t_k` of weight `weight` in month `month`.
    fn add(&mut self, month: usize, weight: f64, t_k: f64) {
        let d = t_k - self.reference_k;
        let d2 = d * d;
        self.weight += weight;
        self.sums[0] += weight * d;
        self.sums[1] += weight * d2;
        self.sums[2] += weight * d2 * d;
        self.sums[3] += weight * d2 * d2;
        if t_k < self.coldest_k {
            self.coldest_k = t_k;
        }
        if t_k > self.warmest_k {
            self.warmest_k = t_k;
        }
        self.month_sums[month] += weight * t_k;
        self.month_weights[month] += weight;
    }

    /// The annual mean, K: the months' sums over their weights.
    #[must_use]
    fn annual_k(&self, months: usize) -> f64 {
        let (sum, weight) = (0..months).fold((0.0, 0.0), |(s, w), k| {
            (s + self.month_sums[k], w + self.month_weights[k])
        });
        sum / weight
    }

    /// Month `month`'s mean, K.
    #[must_use]
    fn month_k(&self, month: usize) -> f64 {
        self.month_sums[month] / self.month_weights[month]
    }

    /// The tally's moments.
    #[must_use]
    fn moments(&self) -> Moments {
        Moments {
            reference_k: self.reference_k,
            about_reference: self.sums.map(|s| s / self.weight),
            coldest_k: self.coldest_k,
            warmest_k: self.warmest_k,
        }
    }
}

/// A run's marched state: the tallies of its last orbit, the orbits it took, whether it
/// converged, the mean emissions its last orbit's slabs were formed from (`month * columns +
/// column`, W m⁻²) and each column's frost at its end, kg m⁻².
#[derive(Debug, Clone, PartialEq)]
struct Marched {
    tallies: Vec<Tally>,
    orbits: u32,
    converged: bool,
    mean_emission_w_m2: Vec<f64>,
    frost_kg_m2: Vec<f64>,
}

/// What an orbit sums for the next: each column's mean surface temperature over the orbit, K;
/// its emission εσT⁴ summed over each month's steps, `month * columns + column`, W m⁻²; and on
/// the first orbit each node's mean, `column * nodes + node`, K, for the jump.
#[derive(Debug, Clone, PartialEq)]
struct OrbitSums {
    surface_means: Vec<f64>,
    emitted_w_m2: Vec<f64>,
    layer_means: Option<Vec<f64>>,
}

/// One column's state after one step of a run, for the run's [`Observer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sample {
    /// The orbit, from 0.
    pub(super) orbit: u32,
    /// The step of the orbit.
    pub(super) step: usize,
    /// The step's middle, its mean anomaly from periapsis, rad.
    pub(super) mean_anomaly_rad: f64,
    /// The step's month.
    pub(super) month: usize,
    /// The column, `site * phases + phase`.
    pub(super) column: usize,
    /// The column's site: the grid's points first, then the end columns.
    pub(super) site: usize,
    /// The sample's weight in its site's tally: its step's share of the orbit over the phases.
    pub(super) weight: f64,
    /// The step's true duration, s, over which the frost and the snow change.
    pub(super) duration_s: f64,
    /// The surface temperature after the step, K.
    pub(super) surface_k: f64,
    /// The sunlight the surface absorbed over the step, W m⁻².
    pub(super) absorbed_w_m2: f64,
    /// The column's input over the step, W m⁻²: the sunlight, the background and ε of the slab's
    /// back radiation.
    pub(super) input_w_m2: f64,
    /// The slab over the column in the step's month.
    pub(super) slab: Slab,
    /// The frost on the column after the step, kg m⁻².
    pub(super) frost_kg_m2: f64,
    /// The frost's change over the step, its snow included, kg m⁻².
    pub(super) frost_change_kg_m2: f64,
    /// The column's change of stored heat over the step, `Σᵢ (Cᵢ ÷ Δt) ΔTᵢ` over its nodes with
    /// each month's compression, W m⁻², where the observer asks for it
    /// ([`Observer::STORED_HEAT`]); 0 otherwise.
    pub(super) stored_w_m2: f64,
}

/// What watches a run's samples: every closure over a [`Sample`], or a test's own recorder.
pub(super) trait Observer {
    /// Whether the samples carry each step's change of the column's stored heat, which costs a
    /// sum over the nodes twice a step.
    const STORED_HEAT: bool = false;

    /// Takes one sample.
    fn sample(&mut self, sample: &Sample);
}

impl<F: FnMut(&Sample)> Observer for F {
    fn sample(&mut self, sample: &Sample) {
        self(sample);
    }
}

/// Each lane's stored heat Σᵢ (Cᵢ ÷ Δt) Tᵢ over the nodes `t` under the system `implicit`, W m⁻²:
/// its change over a step is what the step stored.
#[must_use]
fn heat_content<const L: usize>(implicit: &Implicit, t: &[[f64; L]]) -> [f64; L] {
    let mut heat = [0.0; L];
    for (storage, node) in implicit.storage_w_per_m2_k.iter().zip(t) {
        for (sum, value) in heat.iter_mut().zip(node) {
            *sum += storage * value;
        }
    }
    heat
}

/// The next orbit's mean emissions, `month * columns + column`, from this orbit's `means`, W m⁻².
///
/// On a world stepped in true time each column takes the emission at the hour angles it takes in
/// the next orbit, an orbit turning every phase column on by the fraction of a day it holds
/// (decision-r09-t13-thin-air §1.5, the slab following local time; decision-r09-t13h-scope §7):
/// the column that many phase columns on, interpolated linearly between the two either side of a
/// fractional turn, whose convex weights keep each band's ring sum (§7.2); elsewhere its own.
#[must_use]
fn carried(schedule: &Schedule, means: Vec<f64>, count: usize) -> Vec<f64> {
    let Some(turn) = Turn::of(schedule) else {
        return means;
    };
    let phases = schedule.phases;
    let (whole, part) = match turn {
        Turn::Whole(whole) => (whole, 0.0),
        Turn::Between(whole, part) => (whole, part),
    };
    means
        .chunks_exact(count)
        .flat_map(|month| {
            month.chunks_exact(phases).flat_map(move |site| {
                (0..phases).map(move |phase| {
                    let near = (phase + whole) % phases;
                    match turn {
                        Turn::Whole(_) => site[near],
                        Turn::Between(..) => {
                            (1.0 - part) * site[near] + part * site[(near + 1) % phases]
                        }
                    }
                })
            })
        })
        .collect()
}

/// How far an orbit turns the hour angles of a world stepped in true time, in phase columns:
/// N frac(D), N the phase columns and D the solar days an orbit holds, so that phase column p
/// takes in each orbit the hour angles column p + N frac(D) (mod N) took in the orbit before
/// (decision-r09-t13h-scope §7.1).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Turn {
    /// A whole number of columns, mod N: within [`COMMENSURATE_COLUMNS`] of one (a 3:2 Mercury's
    /// 11.999 86 is 12).
    Whole(usize),
    /// n + w columns, 0 < w < 1, n mod N.
    Between(usize, f64),
}

impl Turn {
    /// The turn of `schedule`'s orbit; `None` where it is not stepped in true time.
    #[must_use]
    fn of(schedule: &Schedule) -> Option<Self> {
        let Light::Turning { days_per_orbit, .. } = schedule.light else {
            return None;
        };
        let phases = schedule.phases;
        let turn = (days_per_orbit - days_per_orbit.floor()) * index(phases);
        let nearest = turn.round();
        let count = |x: f64| {
            usize::try_from(floor_count(x, u32::MAX)).expect("a column count fits a usize") % phases
        };
        Some(if (turn - nearest).abs() < COMMENSURATE_COLUMNS {
            Self::Whole(count(nearest))
        } else {
            let whole = turn.floor();
            Self::Between(count(whole), turn - whole)
        })
    }
}

/// The radiative model of one body.
#[derive(Debug, Clone, PartialEq)]
struct Model {
    regolith: Regolith,
    air: ThinAir,
    layers: Layers,
}

impl Model {
    /// One step of a group of `L` columns, node i of lane l at `t[i][l]`, their frosts
    /// `frost_kg_m2`, under the inputs `input` (W m⁻²: the absorbed sunlight, the background and
    /// the slab's share) and the slab's snowfall `snow_kg_m2_s`, over the system `implicit`, its true
    /// duration `duration_s` seconds for the frost and the snow; `scratch` holds the elimination's
    /// rᵢ. Each lane's arithmetic is a lone column's.
    ///
    /// The snow joins the frost before the frost's balance, so that snow on warmer ground sublimes
    /// through it (decision-r09-t13-thin-air §1.9 item 3). A frost that the step's energy more than
    /// sublimes is spent within the step: the surface is solved free with the frost's latent heat
    /// taken from its balance, so that the step conserves energy (R09.T13.c's deferred finding, "the
    /// step in which a frost runs out loses its remainder").
    #[expect(
        clippy::too_many_arguments,
        reason = "a step reads the system, the inputs, the snow and the duration, and writes the \
                  nodes, the frosts and the elimination's scratch; bundling them would only \
                  rename them"
    )]
    fn advance<const L: usize>(
        &self,
        implicit: &Implicit,
        t: &mut [[f64; L]],
        frost_kg_m2: &mut [f64; L],
        scratch: &mut [[f64; L]],
        input: [f64; L],
        snow_kg_m2_s: [f64; L],
        duration_s: f64,
    ) {
        let last = t.len() - 1;
        let (g, s) = (
            &self.layers.conductance_w_per_m2_k,
            &implicit.storage_w_per_m2_k,
        );
        let (inverse, factor) = (&implicit.inverse, &implicit.factor);
        let heat = self.regolith.heat_flow_w_m2;
        for lane in 0..L {
            scratch[last][lane] = (s[last] * t[last][lane] + heat) * inverse[last];
        }
        for i in (1..last).rev() {
            let (here, below) = scratch.split_at_mut(i + 1);
            for lane in 0..L {
                here[i][lane] = (s[i] * t[i][lane] + g[i] * below[0][lane]) * inverse[i];
            }
        }
        let mut rhs = [0.0; L];
        for lane in 0..L {
            rhs[lane] = s[0] * t[0][lane] + input[lane] + g[0] * scratch[1][lane];
        }
        let emission = self.regolith.emission();
        let linear = implicit.surface_w_per_m2_k;
        let free = surface_temperatures(linear, emission, rhs, t[0]);
        for lane in 0..L {
            let surface = match self.air.frost {
                Some(frost) => {
                    let available = frost_kg_m2[lane] + snow_kg_m2_s[lane] * duration_s;
                    if available > 0.0 || free[lane] < frost.point_k {
                        let held = frost.point_k;
                        let below = factor[1] * held + scratch[1][lane];
                        let unbalanced = input[lane] - emission * math::powi(held, 4)
                            + g[0] * (below - held)
                            - s[0] * (held - t[0][lane]);
                        let latent = frost.latent_heat_j_per_kg;
                        let mass = available - unbalanced * duration_s / latent;
                        if mass > 0.0 {
                            frost_kg_m2[lane] = mass;
                            held
                        } else {
                            frost_kg_m2[lane] = 0.0;
                            // The frost's latent heat over the step's true duration, W m⁻², is no
                            // more than the balance's excess at the frost point, so the balance
                            // less it rises through zero at or above that point
                            // (decision-r09-t13h-scope §2). With no frost at all, the balance is
                            // nil at the point itself.
                            let spent = available * latent / duration_s;
                            if spent > 0.0 {
                                let [warmer] = surface_temperatures(
                                    linear,
                                    emission,
                                    [rhs[lane] - spent],
                                    [t[0][lane]],
                                );
                                warmer
                            } else {
                                held
                            }
                        }
                    } else {
                        free[lane]
                    }
                }
                None => free[lane],
            };
            t[0][lane] = surface;
        }
        for i in 1..=last {
            let (above, here) = t.split_at_mut(i);
            for lane in 0..L {
                here[0][lane] = factor[i] * above[i - 1][lane] + scratch[i][lane];
            }
        }
    }

    /// The absorbed sunlight of column `column` (site `site`, its phase `phase` of `phases`) at
    /// step `step` of orbit `orbit` (from 0) of `schedule`, over `sites` sites at `directions`.
    #[must_use]
    fn absorbed(
        schedule: &Schedule,
        directions: &[[f64; 3]],
        site: usize,
        phase: usize,
        step: usize,
        orbit: u32,
        sites: usize,
    ) -> f64 {
        match &schedule.light {
            Light::Table(table) => table[step * sites + site],
            Light::Turning {
                normal_w_m2,
                start_rad,
                span_rad,
                daylight,
                days_per_orbit,
            } => {
                let turns = f64::from(orbit) * days_per_orbit;
                let start = reduced(
                    start_rad[step]
                        + TAU * (turns - turns.floor())
                        + TAU * index(phase) / index(schedule.phases),
                );
                let end = start + span_rad[step];
                normal_w_m2[step]
                    * daylight[step * sites + site].mean_over(
                        start,
                        math::sin(start),
                        end,
                        math::sin(end),
                    )
            }
            Light::Beam {
                normal_w_m2,
                direction,
            } => {
                let (n, s) = (directions[site], direction[step]);
                let cosine = n[0] * s[0] + n[1] * s[1] + n[2] * s[2];
                if cosine > 0.0 {
                    normal_w_m2[step] * cosine
                } else {
                    0.0
                }
            }
        }
    }

    /// One orbit of `schedule` over the run's columns, at the sites `directions`, in `columns`,
    /// under the slabs `slabs` (`month * columns + column`): each site's tally about its reference
    /// in `tallies`, and into `sums` each column's mean surface temperature, its emission summed
    /// over each month's steps and, where it asks for them, each node's mean; every sample also to
    /// `observe`.
    ///
    /// Step by step over every group of columns: a group's [`LANES`] columns advance together.
    #[expect(
        clippy::too_many_arguments,
        reason = "an orbit reads the schedule, the sites and the slabs and writes the columns, the \
                  tallies and the orbit's sums; bundling them would only rename them"
    )]
    fn orbit<const L: usize, O: Observer>(
        &self,
        schedule: &Schedule,
        directions: &[[f64; 3]],
        slabs: &[Slab],
        columns: &mut Columns<L>,
        tallies: &mut [Tally],
        sums: &mut OrbitSums,
        orbit: u32,
        observe: &mut O,
    ) {
        let nodes = self.layers.nodes();
        let sites = directions.len();
        let (phases, count) = (schedule.phases, columns.count);
        let mut scratch = vec![[0.0; L]; nodes];
        let phase_weight = 1.0 / index(phases);
        let emission = self.regolith.emission();
        let latent = self.air.frost.map(|frost| frost.latent_heat_j_per_kg);
        for step in 0..schedule.month_of.len() {
            let month = schedule.month_of[step];
            let (implicit, duration) = (&schedule.implicit[month], schedule.duration_s[step]);
            let share = schedule.share[step];
            let weight = share * phase_weight;
            let month_slabs = &slabs[month * count..(month + 1) * count];
            for (group, (t, frost)) in columns
                .states
                .chunks_exact_mut(nodes)
                .zip(columns.frost.iter_mut())
                .enumerate()
            {
                let first = group * L;
                let (mut absorbed, mut input, mut snow) = ([0.0; L], [0.0; L], [0.0; L]);
                for lane in 0..L {
                    // A group's lanes past the last column repeat its first.
                    let column = if first + lane < count {
                        first + lane
                    } else {
                        first
                    };
                    let (site, phase) = (column / phases, column % phases);
                    let slab = month_slabs[column];
                    absorbed[lane] =
                        Self::absorbed(schedule, directions, site, phase, step, orbit, sites);
                    input[lane] = self.regolith.input(absorbed[lane], slab.down_w_m2);
                    snow[lane] = latent.map_or(0.0, |latent| slab.snow_w_m2 / latent);
                }
                let frost_before = *frost;
                let heat_before = if O::STORED_HEAT {
                    heat_content(implicit, t)
                } else {
                    [0.0; L]
                };
                self.advance(implicit, t, frost, &mut scratch, input, snow, duration);
                let heat_after = if O::STORED_HEAT {
                    heat_content(implicit, t)
                } else {
                    [0.0; L]
                };
                for lane in 0..L.min(count - first) {
                    let column = first + lane;
                    let (site, surface) = (column / phases, t[0][lane]);
                    tallies[site].add(month, weight, surface);
                    sums.surface_means[column] += share * surface;
                    sums.emitted_w_m2[month * count + column] += emission * math::powi(surface, 4);
                    if let Some(means) = sums.layer_means.as_deref_mut() {
                        for (mean, node) in means[column * nodes..(column + 1) * nodes]
                            .iter_mut()
                            .zip(t.iter())
                        {
                            *mean += share * node[lane];
                        }
                    }
                    observe.sample(&Sample {
                        orbit,
                        step,
                        mean_anomaly_rad: schedule.mean_anomaly_rad[step],
                        month,
                        column,
                        site,
                        weight,
                        duration_s: duration,
                        surface_k: surface,
                        absorbed_w_m2: absorbed[lane],
                        input_w_m2: input[lane],
                        slab: month_slabs[column],
                        frost_kg_m2: frost[lane],
                        frost_change_kg_m2: frost[lane] - frost_before[lane],
                        stored_w_m2: heat_after[lane] - heat_before[lane],
                    });
                }
            }
        }
    }

    /// The run over `schedule` at the sites `directions`, each column started at `start` (one per
    /// site) plus the heat flow's gradient, in groups of `L` columns: the first orbit, the jump,
    /// then orbits until no site's annual mean moves by [`CONVERGENCE_K`], at most
    /// [`MAX_ORBITS`]; every sample of every orbit to `observe`.
    ///
    /// Each orbit's slabs are formed from each column's mean emission in each month of the orbit
    /// before ([`carried`]), the first orbit's from its sunlight ([`first_emission`](Self::first_emission));
    /// decision-r09-t13-thin-air §1.9.
    fn march<const L: usize, O: Observer>(
        &self,
        schedule: &Schedule,
        directions: &[[f64; 3]],
        start: &[f64],
        observe: &mut O,
    ) -> Marched {
        let nodes = self.layers.nodes();
        let phases = schedule.phases;
        let count = directions.len() * phases;
        let months = schedule.months;
        let mut columns = Columns::<L>::new(count, nodes, |column, node| {
            start[column / phases] + self.layers.geothermal_k[node]
        });
        let steps = schedule.steps_in_months();
        let mut mean_emission = self.first_emission(schedule, directions, &steps);
        let mut reference: Vec<f64> = start.to_vec();
        let mut orbits = 0;
        loop {
            let slabs: Vec<Slab> = mean_emission.iter().map(|&f| self.air.slab(f)).collect();
            let mut tallies: Vec<Tally> = reference.iter().map(|&r| Tally::new(r)).collect();
            let mut sums = OrbitSums {
                surface_means: vec![0.0; count],
                emitted_w_m2: vec![0.0; months * count],
                layer_means: (orbits == 0).then(|| vec![0.0; count * nodes]),
            };
            self.orbit(
                schedule,
                directions,
                &slabs,
                &mut columns,
                &mut tallies,
                &mut sums,
                orbits,
                observe,
            );
            orbits += 1;
            let annual: Vec<f64> = tallies
                .iter()
                .map(|tally| tally.annual_k(schedule.months))
                .collect();
            let moved = annual
                .iter()
                .zip(&reference)
                .fold(0.0, |most: f64, (a, r)| {
                    let d = (a - r).abs();
                    if d > most { d } else { most }
                });
            let converged = orbits > 1 && moved < CONVERGENCE_K;
            if converged || orbits >= MAX_ORBITS {
                return Marched {
                    tallies,
                    orbits,
                    converged,
                    mean_emission_w_m2: mean_emission,
                    frost_kg_m2: columns.frosts(),
                };
            }
            if let Some(means) = &sums.layer_means {
                let targets = long_run_means(schedule, &sums.surface_means);
                self.jump(&mut columns, means, &targets);
            }
            reference = annual;
            let mut means = sums.emitted_w_m2;
            for (i, sum) in means.iter_mut().enumerate() {
                *sum /= f64::from(steps[i / count]);
            }
            mean_emission = carried(schedule, means, count);
        }
    }

    /// The first orbit's mean emission of each column in each month, `month * columns + column`,
    /// W m⁻²: 2 (⟨S⟩ + `E_nf`) ÷ (2 − εβ).
    ///
    /// That is a column in balance with its month's mean absorbed sunlight ⟨S⟩ under the unbounded
    /// slab, T13.c's slab at balance (decision-r09-t13-thin-air §1.9; decision-r09-t13h-scope §7.3).
    /// ⟨S⟩ is the site's month mean, T13.c's input, on whole days and on a locked world; in true
    /// time it is each phase column's own mean over the month's steps at the first orbit's hour
    /// angles, the window §1.5 gives the slab there. `steps` holds each month's steps.
    #[must_use]
    fn first_emission(
        &self,
        schedule: &Schedule,
        directions: &[[f64; 3]],
        steps: &[u32],
    ) -> Vec<f64> {
        let sites = directions.len();
        let phases = schedule.phases;
        let count = sites * phases;
        let scale = 2.0 / (2.0 - self.regolith.emissivity * self.air.absorptivity);
        let floor = self.regolith.floor_flux_w_m2();
        match schedule.light {
            Light::Turning { .. } => {
                let mut sums = vec![0.0; schedule.months * count];
                for (step, &month) in schedule.month_of.iter().enumerate() {
                    let row = &mut sums[month * count..(month + 1) * count];
                    for (column, sum) in row.iter_mut().enumerate() {
                        *sum += Self::absorbed(
                            schedule,
                            directions,
                            column / phases,
                            column % phases,
                            step,
                            0,
                            sites,
                        );
                    }
                }
                for (i, sum) in sums.iter_mut().enumerate() {
                    *sum = scale * (*sum / f64::from(steps[i / count]) + floor);
                }
                sums
            }
            Light::Table(_) | Light::Beam { .. } => schedule
                .month_absorbed
                .iter()
                .map(|s| scale * (s + floor))
                .collect(),
        }
    }

    /// Kieffer's jump: every node of every column of `columns` offset so that its mean over the
    /// orbit, `layer_means` (`column * nodes + node`), becomes the column's long-run surface mean,
    /// `surface_means` ([`long_run_means`]), plus the heat flow's steady gradient (the
    /// [module documentation](self)).
    fn jump<const L: usize>(
        &self,
        columns: &mut Columns<L>,
        layer_means: &[f64],
        surface_means: &[f64],
    ) {
        let nodes = self.layers.nodes();
        for (column, (means, &surface)) in layer_means
            .chunks_exact(nodes)
            .zip(surface_means)
            .enumerate()
        {
            let (group, lane) = (column / L, column % L);
            let t = &mut columns.states[group * nodes..(group + 1) * nodes];
            for ((node, &mean), &offset) in t
                .iter_mut()
                .zip(means)
                .zip(&self.layers.geothermal_k)
                .skip(1)
            {
                node[lane] += surface + offset - mean;
            }
        }
    }
}

/// A run's columns in groups of `L`: each group's nodes, node i of lane l at `states[group *
/// nodes + i][l]`, and its frosts, kg m⁻²; a last group's lanes past the last column copy its first
/// and are never read.
#[derive(Debug, Clone, PartialEq)]
struct Columns<const L: usize> {
    count: usize,
    states: Vec<[f64; L]>,
    frost: Vec<[f64; L]>,
}

impl<const L: usize> Columns<L> {
    /// `count` columns of `nodes` nodes, node i of column c at `start(c, i)`, with no frost.
    #[must_use]
    fn new(count: usize, nodes: usize, start: impl Fn(usize, usize) -> f64) -> Self {
        let groups = count.div_ceil(L);
        let mut states = vec![[0.0; L]; groups * nodes];
        for (group, t) in states.chunks_exact_mut(nodes).enumerate() {
            for (node, values) in t.iter_mut().enumerate() {
                for (lane, value) in values.iter_mut().enumerate() {
                    let column = group * L + lane;
                    let column = if column < count { column } else { group * L };
                    *value = start(column, node);
                }
            }
        }
        Self {
            count,
            states,
            frost: vec![[0.0; L]; groups],
        }
    }

    /// Each column's frost, kg m⁻².
    #[must_use]
    fn frosts(&self) -> Vec<f64> {
        (0..self.count)
            .map(|column| self.frost[column / L][column % L])
            .collect()
    }
}

/// How far from a whole number of phase columns an orbit may turn the hour angles for the run to
/// take the turn as commensurate, in columns: 0.01 (`Adopted`). A 3:2 Mercury's orbit turns them
/// by 0.499 994 of a day, 11.999 86 columns, so that its pattern drifts by a column in 7,000 orbits.
const COMMENSURATE_COLUMNS: f64 = 0.01;

/// Each column's long-run mean surface temperature as its first orbit's means `surface_means`
/// give it: on a world stepped in true time, the mean over the columns whose hour angles the
/// column takes in its later orbits, those a whole turn of phase columns on where an orbit turns
/// the day by a whole number of them (a 3:2 Mercury's columns half a day apart), and its site's
/// every column otherwise, whose hour angles drift round the day; elsewhere its own.
#[must_use]
fn long_run_means(schedule: &Schedule, surface_means: &[f64]) -> Vec<f64> {
    let phases = schedule.phases;
    let Some(turn) = Turn::of(schedule) else {
        return surface_means.to_vec();
    };
    let step = match turn {
        Turn::Whole(step) => Some(step),
        Turn::Between(..) => None,
    };
    surface_means
        .chunks_exact(phases)
        .flat_map(|site| {
            (0..phases).map(move |phase| match step {
                Some(step) => {
                    let (mut sum, mut count, mut member) = (0.0, 0.0, phase);
                    loop {
                        sum += site[member];
                        count += 1.0;
                        member = (member + step) % phases;
                        if member == phase {
                            break sum / count;
                        }
                    }
                }
                None => site.iter().fold(0.0, |a, b| a + b) / index(phases),
            })
        })
        .collect()
}

/// The sites of a run on `grid` in `coordinates`: each grid point's, then each end column's, as
/// model-frame unit vectors (on the zonal grid (cos φ, 0, sin φ), its forcing being every
/// longitude's), and the end columns' kinds and body-frame directions.
#[must_use]
fn sites(grid: &ModelGrid, coordinates: Coordinates) -> (Vec<[f64; 3]>, Vec<EndSite>) {
    let mut ends = vec![
        (End::NorthPole, [0.0, 0.0, 1.0]),
        (End::SouthPole, [0.0, 0.0, -1.0]),
    ];
    let points: Vec<[f64; 3]> = match coordinates {
        Coordinates::Geographic => {
            ends.insert(0, (End::Equator, [1.0, 0.0, 0.0]));
            (0..LATITUDE_BANDS)
                .map(|k| [grid.centre_cosine(k), 0.0, grid.centre_sine(k)])
                .collect()
        }
        Coordinates::TidallyLocked => {
            ends.extend((0..EQUATOR_RING_COLUMNS).map(|m| {
                let (s, c) = math::sin_cos(TAU * index(m) / index(EQUATOR_RING_COLUMNS));
                (End::Equator, [c, s, 0.0])
            }));
            (0..grid.points())
                .map(|p| grid.centre_direction(p))
                .collect()
        }
    };
    let mut directions = points;
    directions.extend(ends.iter().map(|(_, d)| coordinates.to_model(*d)));
    (directions, ends)
}

/// An end column's kind and its body-frame direction.
type EndSite = (End, [f64; 3]);

/// ⌊x⌋ for x held to [0, `most`], as a `u32`; 0 for a NaN.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is held to [0, most] with most a u32, so its floor is a small non-negative \
              integer that the cast holds exactly"
)]
fn floor_count(x: f64, most: u32) -> u32 {
    let held = if x > 0.0 { x.min(f64::from(most)) } else { 0.0 };
    held.floor() as u32
}

/// ⌈x⌉ held to [`least`, `most`], as a `u32`; `least` for a NaN.
#[must_use]
fn ceil_count(x: f64, least: u32, most: u32) -> u32 {
    let up = if x > 0.0 { x.ceil() } else { 0.0 };
    floor_count(up, most).max(least)
}

/// The run's schedule for the body of `inputs` on the sites `directions` under the regolith
/// `regolith`, its clock `clock`, with the layers it builds; `None` where the light is constant (a
/// locked world in a one-month year, or a body with no seasonal orbit), whose columns take the
/// closed form.
///
/// # Panics
///
/// If the seasonal orbit's eccentricity is not at least 0 and below 1, which the inputs' builder
/// refuses.
#[must_use]
fn schedule(
    inputs: &CoarseInputs,
    directions: &[[f64; 3]],
    regolith: &Regolith,
    coordinates: Coordinates,
) -> Option<(Schedule, Layers)> {
    let orbit = inputs.seasonal_orbit()?;
    let eccentricity = Eccentricity::new(orbit.eccentricity)
        .expect("the inputs' builder holds a seasonal orbit's eccentricity in [0, 1)");
    let months = usize::from(inputs.months());
    let clock = if locked(inputs) {
        if months == 1 {
            return None;
        }
        Clock::Locked
    } else {
        let day_s = inputs.spin().solar_day?.value();
        clock_of(orbit.period.value(), eccentricity, months, day_s)
    };
    let period = orbit.period.value();
    let layers = layers_of(clock, period, regolith);
    Some((
        Plan {
            inputs,
            directions,
            regolith,
            coordinates,
            clock,
            months,
            period_s: period,
            eccentricity,
        }
        .build(&layers),
        layers,
    ))
}

/// The month edges, mean anomalies from periapsis, of a year of `months` months on an orbit of
/// eccentricity `eccentricity`: the field's twelve ([`month_edges`]), or the whole orbit.
#[must_use]
fn edges_of(months: usize, eccentricity: Eccentricity) -> Vec<f64> {
    if months == 12 {
        month_edges(eccentricity.value())
            .expect("an `Eccentricity` is in [0, 1)")
            .map(Radians::value)
            .to_vec()
    } else {
        vec![0.0, TAU]
    }
}

/// The clock of a world that turns under its sun with a solar day of `day_s` seconds, on an orbit
/// of `period_s` seconds and eccentricity `eccentricity` in a year of `months` months: whole days
/// where every month is at least a day long, true time otherwise.
#[must_use]
fn clock_of(period_s: f64, eccentricity: Eccentricity, months: usize, day_s: f64) -> Clock {
    let edges = edges_of(months, eccentricity);
    let shortest = edges
        .windows(2)
        .fold(f64::INFINITY, |least, w| least.min(w[1] - w[0]))
        / TAU
        * period_s;
    if shortest >= day_s {
        Clock::WholeDays { day_s }
    } else {
        Clock::TrueTime { day_s }
    }
}

/// The layers of a column under `clock` on an orbit of `period_s` seconds of `regolith`: the skin
/// depth the shorter of the solar day's and the year's, the bottom six of the day's or two of the
/// year's, whichever is deeper, compressed below two of the day's.
#[must_use]
fn layers_of(clock: Clock, period_s: f64, regolith: &Regolith) -> Layers {
    let day_s = match clock {
        Clock::WholeDays { day_s } | Clock::TrueTime { day_s } => day_s,
        Clock::Locked => period_s,
    };
    let short = if day_s < period_s { day_s } else { period_s };
    let (daily, yearly) = ((day_s / short).sqrt(), (period_s / short).sqrt());
    let bottom = (BOTTOM_DIURNAL_SKIN_DEPTHS * daily).max(BOTTOM_ANNUAL_SKIN_DEPTHS * yearly);
    Layers::new(
        regolith.inertia_j_per_m2_k_sqrt_s,
        short,
        bottom,
        COMPRESSED_SKIN_DEPTHS * daily,
        regolith.heat_flow_w_m2,
    )
}

/// What a schedule is built from.
struct Plan<'a> {
    inputs: &'a CoarseInputs,
    directions: &'a [[f64; 3]],
    regolith: &'a Regolith,
    coordinates: Coordinates,
    clock: Clock,
    months: usize,
    period_s: f64,
    eccentricity: Eccentricity,
}

/// One step's sun: its absorbed flux at normal incidence, (1 − A) F, W m⁻², and its direction: the
/// declination's sine and cosine on a turning world, the beam's model-frame direction on a locked
/// one.
struct Sun {
    normal_w_m2: f64,
    sine_declination: f64,
    cosine_declination: f64,
    beam: [f64; 3],
}

impl Plan<'_> {
    /// The sun at the mean anomaly `mean_anomaly_rad`: on a twelve-month orbit at its place, in a
    /// one-month year at equinox at the mean distance, its beam from +x.
    #[must_use]
    fn sun(&self, mean_anomaly_rad: f64, track: &HostTrack) -> Sun {
        let e = self.eccentricity.value();
        let flux = self.inputs.host_flux().value();
        let absorbing = 1.0 - self.regolith.albedo;
        if self.months == 1 {
            return Sun {
                normal_w_m2: absorbing * flux,
                sine_declination: 0.0,
                cosine_declination: 1.0,
                beam: self.coordinates.to_model([1.0, 0.0, 0.0]),
            };
        }
        let place = orbit_at(mean_anomaly_rad, self.eccentricity);
        let (true_anomaly, distance) = (place.true_anomaly_rad, place.distance_over_a);
        let spin = self.inputs.spin();
        let sine_declination = math::sin(spin.obliquity.value())
            * math::sin(true_anomaly - spin.equinox_true_anomaly.value());
        Sun {
            normal_w_m2: absorbing * flux * (1.0 - e * e).sqrt() / (distance * distance),
            sine_declination,
            cosine_declination: (1.0 - sine_declination * sine_declination).max(0.0).sqrt(),
            beam: self
                .coordinates
                .to_model(track.direction(true_anomaly, mean_anomaly_rad)),
        }
    }

    /// A month's steps, the length of each as stepped, s, and its deep layers' compression, from
    /// its true duration `duration_s`.
    #[must_use]
    fn month_steps(&self, duration_s: f64) -> (u32, f64, f64) {
        match self.clock {
            Clock::WholeDays { day_s } => {
                let days = floor_count(duration_s / day_s, MAX_DAYS_PER_MONTH).max(1);
                (
                    days * STEPS_PER_SOLAR_DAY,
                    day_s / f64::from(STEPS_PER_SOLAR_DAY),
                    f64::from(days) * day_s / duration_s,
                )
            }
            Clock::TrueTime { day_s } => {
                let steps = ceil_count(
                    duration_s * f64::from(STEPS_PER_SOLAR_DAY) / day_s,
                    MIN_STEPS_PER_MONTH,
                    MAX_STEPS_PER_MONTH,
                );
                (steps, duration_s / f64::from(steps), 1.0)
            }
            Clock::Locked => (
                MIN_STEPS_PER_MONTH,
                duration_s / f64::from(MIN_STEPS_PER_MONTH),
                1.0,
            ),
        }
    }

    /// The schedule over `layers`.
    #[must_use]
    fn build(&self, layers: &Layers) -> Schedule {
        let edges = edges_of(self.months, self.eccentricity);
        let track = HostTrack::of(self.inputs.spin());
        let sites = self.directions.len();
        let mut schedule = Schedule {
            months: self.months,
            month_share: Vec::with_capacity(self.months),
            implicit: Vec::with_capacity(self.months),
            month_of: Vec::new(),
            share: Vec::new(),
            duration_s: Vec::new(),
            mean_anomaly_rad: Vec::new(),
            phases: match self.clock {
                Clock::TrueTime { .. } => PHASE_COLUMNS,
                Clock::WholeDays { .. } | Clock::Locked => 1,
            },
            light: Light::Table(Vec::new()),
            month_absorbed: vec![0.0; self.months * sites],
        };
        let (mut normal, mut start, mut span, mut beam) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (mut table, mut daylight) = (Vec::new(), Vec::new());
        let edge_sines: Vec<f64> = (0..=STEPS_PER_SOLAR_DAY)
            .map(|s| math::sin(day_edge(s)))
            .collect();
        for (month, pair) in edges.windows(2).enumerate() {
            let (from, width) = (pair[0], pair[1] - pair[0]);
            let duration = width / TAU * self.period_s;
            let (steps, stepped_s, compression) = self.month_steps(duration);
            schedule.month_share.push(width / TAU);
            schedule
                .implicit
                .push(Implicit::new(layers, stepped_s, compression));
            let absorbed = &mut schedule.month_absorbed[month * sites..(month + 1) * sites];
            for step in 0..steps {
                let middle = from + (f64::from(step) + 0.5) / f64::from(steps) * width;
                let sun = self.sun(middle, &track);
                schedule.month_of.push(month);
                schedule.share.push(width / TAU / f64::from(steps));
                schedule.duration_s.push(duration / f64::from(steps));
                schedule.mean_anomaly_rad.push(middle);
                for (mean, n) in absorbed.iter_mut().zip(self.directions) {
                    let (value, day) = self.site_light(&sun, *n);
                    *mean += value / f64::from(steps);
                    match self.clock {
                        Clock::WholeDays { .. } => {
                            table.push(Self::whole_day_light(&sun, day, step, &edge_sines));
                        }
                        Clock::TrueTime { .. } => daylight.push(day),
                        Clock::Locked => {}
                    }
                }
                normal.push(sun.normal_w_m2);
                beam.push(sun.beam);
                if let Clock::TrueTime { day_s } = self.clock {
                    let seconds =
                        (from + f64::from(step) / f64::from(steps) * width) / TAU * self.period_s;
                    start.push(-PI + TAU * seconds / day_s);
                    span.push(TAU * stepped_s / day_s);
                }
            }
        }
        schedule.light = match self.clock {
            Clock::WholeDays { .. } => Light::Table(table),
            Clock::TrueTime { day_s } => Light::Turning {
                normal_w_m2: normal,
                start_rad: start,
                span_rad: span,
                daylight,
                days_per_orbit: self.period_s / day_s,
            },
            Clock::Locked => Light::Beam {
                normal_w_m2: normal,
                direction: beam,
            },
        };
        schedule
    }

    /// A site's daily-mean absorbed sunlight under `sun`, W m⁻², and its daylight (the beam's on a
    /// locked world, whose daylight is unused).
    #[must_use]
    fn site_light(&self, sun: &Sun, n: [f64; 3]) -> (f64, Daylight) {
        match self.clock {
            Clock::Locked => {
                let cosine = n[0] * sun.beam[0] + n[1] * sun.beam[1] + n[2] * sun.beam[2];
                let value = if cosine > 0.0 {
                    sun.normal_w_m2 * cosine
                } else {
                    0.0
                };
                (value, Daylight::new(0.0, 0.0, 0.0, 0.0))
            }
            Clock::WholeDays { .. } | Clock::TrueTime { .. } => {
                let day = Daylight::new(n[2], n[0], sun.sine_declination, sun.cosine_declination);
                (sun.normal_w_m2 * day.daily_mean(), day)
            }
        }
    }

    /// A whole-day step's absorbed sunlight at a site of daylight `day` under `sun`, W m⁻²: its
    /// mean over the step's fixed span of hour angles, the day's `step` modulo
    /// [`STEPS_PER_SOLAR_DAY`] from midnight.
    #[must_use]
    fn whole_day_light(sun: &Sun, day: Daylight, step: u32, edge_sines: &[f64]) -> f64 {
        let k = step % STEPS_PER_SOLAR_DAY;
        let i = usize::try_from(k).expect("a step of a day fits a usize");
        sun.normal_w_m2
            * day.mean_over(
                day_edge(k),
                edge_sines[i],
                day_edge(k + 1),
                edge_sines[i + 1],
            )
    }
}

/// The hour angle of the start of a whole day's step `k`, radians: −π, midnight, plus k
/// [`STEPS_PER_SOLAR_DAY`]ths of a turn.
#[must_use]
fn day_edge(k: u32) -> f64 {
    -PI + TAU * f64::from(k) / f64::from(STEPS_PER_SOLAR_DAY)
}

/// The radiative model's answer for the body of `inputs` (the [module documentation](self)).
///
/// # Panics
///
/// If a value of the answer is not finite: a fault of the model.
#[must_use]
pub(super) fn solve(inputs: &CoarseInputs) -> RadiativeSolution {
    solve_observed(inputs, &mut |_: &Sample| {})
}

/// [`solve`], every sample of every orbit also to `observe`.
///
/// The samples come the grid points' sites first, then the end columns'. A body whose light is
/// constant takes the closed form and has no samples.
///
/// # Panics
///
/// If a value of the answer is not finite: a fault of the model.
#[must_use]
pub(super) fn solve_observed<O: Observer>(
    inputs: &CoarseInputs,
    observe: &mut O,
) -> RadiativeSolution {
    let (grid, coordinates) = frame(inputs);
    let regolith = Regolith::of(inputs);
    let (directions, ends) = sites(&grid, coordinates);
    let points = grid.points();
    let floor_k = night_floor(inputs.heat_flow()).value();
    let (tallies, climate, air) = match schedule(inputs, &directions, &regolith, coordinates) {
        Some((schedule, layers)) => {
            let annual_absorbed: Vec<f64> = (0..directions.len())
                .map(|site| {
                    (0..schedule.months).fold(0.0, |sum, k| {
                        sum + schedule.month_share[k]
                            * schedule.month_absorbed[k * directions.len() + site]
                    })
                })
                .collect();
            let air = ThinAir::of(inputs, &regolith, grid.mean(&annual_absorbed[..points]));
            let model = Model {
                regolith,
                air,
                layers,
            };
            let start_share = match coordinates {
                Coordinates::TidallyLocked => 1.0,
                Coordinates::Geographic => SPIN_UP_START,
            };
            // Each column's closed form under its annual-mean sunlight (decision-r09-t13-thin-air
            // §1.9 item 6).
            let start: Vec<f64> = annual_absorbed
                .iter()
                .map(|&a| {
                    start_share
                        * air
                            .balance(regolith.emissivity, regolith.outside_w_m2(a))
                            .surface_k
                })
                .collect();
            let marched = model.march::<LANES, _>(&schedule, &directions, &start, observe);
            let climate = GridClimate {
                annual_k: marched.tallies[..points]
                    .iter()
                    .map(|t| t.annual_k(schedule.months))
                    .collect(),
                months_k: (0..schedule.months)
                    .map(|k| {
                        marched.tallies[..points]
                            .iter()
                            .map(|t| t.month_k(k))
                            .collect()
                    })
                    .collect(),
                orbits: marched.orbits,
                converged: marched.converged,
            };
            (marched.tallies, climate, air)
        }
        None => closed_form(inputs, &grid, &directions, &regolith),
    };
    let months = climate.months_k.len();
    let ends = ends
        .iter()
        .zip(&tallies[points..])
        .map(|(&(end, direction), tally)| EndColumn {
            end,
            direction,
            annual_k: tally.annual_k(months),
            months_k: (0..months).map(|k| tally.month_k(k)).collect(),
            moments: tally.moments(),
        })
        .collect();
    let lower_bound_k = air
        .frost
        .map_or(floor_k, |frost| frost.point_k.max(floor_k));
    let solution = RadiativeSolution {
        moments: tallies[..points].iter().map(Tally::moments).collect(),
        climate,
        grid,
        coordinates,
        ends,
        lower_bound_k,
        emissivity: regolith.emissivity,
        floor_flux_w_m2: regolith.floor_flux_w_m2(),
        air,
    };
    for v in solution
        .climate
        .annual_k
        .iter()
        .chain(solution.climate.months_k.iter().flatten())
    {
        let _finite = super::finite_k(*v);
    }
    solution
}

/// The closed form of a body whose light is constant (a world locked 1:1 to its host in a
/// one-month year, or one with no seasonal orbit): every column of `grid` and every end column at
/// `directions` in radiative balance with its sunlight, the background, the heat flow and its
/// balanced bounded slab ([`ThinAir::balance`]), and never below the frost point; its tallies, each
/// one sample, its one month and its thin air.
#[must_use]
fn closed_form(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    directions: &[[f64; 3]],
    regolith: &Regolith,
) -> (Vec<Tally>, GridClimate, ThinAir) {
    let normal = (1.0 - regolith.albedo) * inputs.host_flux().value();
    let substellar = if locked(inputs) {
        Some(frame(inputs).1.to_model([1.0, 0.0, 0.0]))
    } else {
        None
    };
    let absorbed: Vec<f64> = directions
        .iter()
        .map(|n| match substellar {
            Some(s) => normal * (n[0] * s[0] + n[1] * s[1] + n[2] * s[2]).max(0.0),
            None => normal * Daylight::new(n[2], n[0], 0.0, 1.0).daily_mean(),
        })
        .collect();
    let air = ThinAir::of(inputs, regolith, grid.mean(&absorbed[..grid.points()]));
    let tallies: Vec<Tally> = absorbed
        .iter()
        .map(|&a| {
            let t = air
                .balance(regolith.emissivity, regolith.outside_w_m2(a))
                .surface_k;
            let mut tally = Tally::new(t);
            tally.add(0, 1.0, t);
            tally
        })
        .collect();
    let field: Vec<f64> = tallies[..grid.points()]
        .iter()
        .map(|t| t.reference_k)
        .collect();
    let climate = GridClimate {
        annual_k: field.clone(),
        months_k: vec![field],
        orbits: 0,
        converged: true,
    };
    (tallies, climate, air)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::surface::reference;
    use crate::units::consts::SECONDS_PER_DAY;

    /// The Moon's synodic month, s: 29.5306 days (Hayne et al. 2017, Table A1, read).
    const LUNAR_DAY_S: f64 = 29.5306 * SECONDS_PER_DAY;

    /// The lunar regolith of research-c's prototype: plan 14's rock (Γ 55, ε 0.95) under the
    /// airless Bond albedo 0.11 and the reference Moon's heat flow, 0.018 W m⁻².
    fn lunar_regolith() -> Regolith {
        Regolith::of(&reference::moon_like())
    }

    /// One step from a profile that varies with depth conserves the column's energy: the change
    /// of its stored heat is the step's net input, sunlight and background in, the emission at
    /// the step's end out, the heat flow in at the bottom; and with the surface held at a frost
    /// point the rest goes into the frost, at its latent heat. So it does where snow falls on the
    /// frost or on bare ground, and where the step spends the last of a frost and warms past its
    /// point (decision-r09-t13-thin-air §1.9 item 3).
    #[test]
    fn a_column_step_conserves_energy() {
        let (moon, mars) = (reference::moon_like(), reference::mars_like());
        for (inputs, input, frosty, frost_kg_m2, snow_kg_m2_s) in [
            (&moon, 900.0, false, 0.0, 0.0),
            (&moon, 0.0, false, 0.0, 0.0),
            (&mars, 1.0, true, 5.0, 0.0),
            (&mars, 400.0, true, 5.0, 0.0),
            (&mars, 1.0, true, 5.0, 2e-5),
            (&mars, 400.0, true, 0.0, 2e-5),
            (&mars, 400.0, true, 0.01, 0.0),
            (&mars, 40.0, true, 0.0, 1e-6),
        ] {
            let regolith = Regolith::of(inputs);
            let day = inputs.spin().solar_day.unwrap().value();
            let year = inputs.seasonal_orbit().unwrap().period.value();
            let layers = layers_of(Clock::WholeDays { day_s: day }, year, &regolith);
            let air = if frosty {
                ThinAir::of(inputs, &regolith, 100.0)
            } else {
                ThinAir::NONE
            };
            let model = Model {
                regolith,
                air,
                layers,
            };
            let step = day / f64::from(STEPS_PER_SOLAR_DAY);
            let implicit = Implicit::new(&model.layers, step, 0.3);
            let nodes = model.layers.nodes();
            let mut t: Vec<[f64; 1]> = (0..nodes)
                .map(|i| [150.0 + 40.0 * math::sin(index(i))])
                .collect();
            let before = t.clone();
            let (mut frost, mut scratch) = ([frost_kg_m2], vec![[0.0]; nodes]);
            let frost_before = frost[0];
            // The step's true duration, longer than its stepped length as a whole day's month's is.
            let duration = 3.5 * step;
            model.advance(
                &implicit,
                &mut t,
                &mut frost,
                &mut scratch,
                [input],
                [snow_kg_m2_s],
                duration,
            );
            let stored = (0..nodes).fold(0.0, |sum, i| {
                sum + implicit.storage_w_per_m2_k[i] * (t[i][0] - before[i][0])
            });
            // The latent heat the surface gave the frost: its condensation, the snow aside.
            let into_frost = model.air.frost.map_or(0.0, |f| {
                (frost_before + snow_kg_m2_s * duration - frost[0]) * f.latent_heat_j_per_kg
                    / duration
            });
            let surface = t[0][0];
            let net = input + regolith.heat_flow_w_m2
                - regolith.emission() * math::powi(surface, 4)
                - into_frost;
            let scale = input.abs() + regolith.emission() * math::powi(surface, 4) + 1.0;
            assert!(
                (stored - net).abs() < 1e-10 * scale,
                "input {input}, frost {frost_kg_m2}, snow {snow_kg_m2_s}: stored {stored} against \
                 {net}"
            );
            if frosty {
                let point = model.air.frost.unwrap().point_k;
                assert!(surface >= point - 1e-9, "{surface} under {point}");
            }
        }
    }

    /// The surface's balance is solved to its root from far off on either side: a cold night's
    /// small input, a noon's large one, a conductive and a radiative regime, from any guess.
    #[test]
    fn the_surface_balance_is_solved_to_its_root() {
        let emission = REGOLITH_EMISSIVITY * STEFAN_BOLTZMANN;
        for linear in [1e-3, 0.4, 5.0, 300.0] {
            for rhs in [1e-4, 0.5, 40.0, 1_200.0, 5e5] {
                for guess in [f64::NAN, 0.0, 3.0, 95.0, 400.0, 1e5] {
                    let [t] = surface_temperatures(linear, emission, [rhs], [guess]);
                    let residual = linear * t + emission * math::powi(t, 4) - rhs;
                    assert!(
                        residual.abs() < 1e-11 * rhs + 1e-12,
                        "linear {linear}, rhs {rhs}, guess {guess}: T {t}, residual {residual}"
                    );
                }
            }
        }
        let [none] = surface_temperatures(1.0, emission, [0.0], [100.0]);
        assert!(none.abs() < 1e-300);
    }

    /// The column's nodes: a fifth of a skin depth first, each spacing 4/3 of the last, down to
    /// six skin depths of the solar day or two of the year, whichever is deeper (the reference
    /// Moon's, Mars's and Ceres's), the deep ones from two skin depths of the day.
    #[test]
    fn the_layers_follow_their_rules() {
        for (name, inputs, nodes) in [
            ("Moon", reference::moon_like(), 10),
            ("Mars", reference::mars_like(), 17),
            ("Ceres", reference::ceres_like(), 20),
        ] {
            let regolith = Regolith::of(&inputs);
            let day = inputs.spin().solar_day.unwrap().value();
            let year = inputs.seasonal_orbit().unwrap().period.value();
            let layers = layers_of(Clock::WholeDays { day_s: day }, year, &regolith);
            let bottom = BOTTOM_DIURNAL_SKIN_DEPTHS.max(2.0 * (year / day).sqrt());
            let depth = &layers.depth;
            eprintln!(
                "{name}: {} nodes to {:.1} skin depths",
                depth.len(),
                depth[depth.len() - 1]
            );
            assert_eq!(depth.len(), nodes, "{name}");
            assert!((depth[1] - 0.2).abs() < 1e-15);
            for w in depth.windows(3) {
                assert!(((w[2] - w[1]) / (w[1] - w[0]) - 4.0 / 3.0).abs() < 1e-12);
            }
            assert!(depth[depth.len() - 1] >= bottom && depth[depth.len() - 2] < bottom);
            assert!(depth[layers.first_deep] > 2.0 && depth[layers.first_deep - 1] <= 2.0);
            // The capacities sum to Γ √(P ÷ π) times the column's depth.
            let total: f64 = layers.capacity_j_per_m2_k.iter().sum();
            let expected =
                regolith.inertia_j_per_m2_k_sqrt_s * (day / PI).sqrt() * depth[depth.len() - 1];
            assert!((total / expected - 1.0).abs() < 1e-12, "{name}");
        }
    }

    /// The lunar equator's diurnal cycle under a constant sun converges to a periodic one, in
    /// energy balance, and reproduces research-c's prototype of the same column (its §2.2, at 96
    /// steps a day: noon 386.3 K, predawn 96.1 K, mean 220.5 K) to 1 K at 48.
    #[test]
    fn a_diurnal_cycle_converges_to_its_period() {
        let regolith = lunar_regolith();
        let model = Model {
            regolith,
            air: ThinAir::NONE,
            layers: Layers::new(
                regolith.inertia_j_per_m2_k_sqrt_s,
                LUNAR_DAY_S,
                6.0,
                f64::INFINITY,
                0.018,
            ),
        };
        let steps = STEPS_PER_SOLAR_DAY;
        let interval_s = LUNAR_DAY_S / f64::from(steps);
        let implicit = Implicit::new(&model.layers, interval_s, 1.0);
        let day = Daylight::new(0.0, 1.0, 0.0, 1.0);
        let normal = (1.0 - regolith.albedo) * 1_361.0;
        let inputs: Vec<f64> = (0..steps)
            .map(|k| {
                let (from, to) = (day_edge(k), day_edge(k + 1));
                regolith.input(
                    normal * day.mean_over(from, math::sin(from), to, math::sin(to)),
                    0.0,
                )
            })
            .collect();
        let nodes = model.layers.nodes();
        let mut column = Columns::<1>::new(1, nodes, |_, _| 215.0);
        let mut scratch = vec![[0.0]; nodes];
        let mut previous = f64::INFINITY;
        let mut cycle = Vec::new();
        for days in 0..80 {
            let mut layer_means = vec![0.0; nodes];
            cycle.clear();
            for &input in &inputs {
                let t = &mut column.states;
                model.advance(
                    &implicit,
                    t,
                    &mut column.frost[0],
                    &mut scratch,
                    [input],
                    [0.0],
                    interval_s,
                );
                cycle.push(t[0][0]);
                for (m, v) in layer_means.iter_mut().zip(t.iter()) {
                    *m += v[0] / f64::from(steps);
                }
            }
            let mean = cycle.iter().sum::<f64>() / f64::from(steps);
            if days == 0 {
                model.jump(&mut column, &layer_means, &[mean]);
            }
            if (mean - previous).abs() < 1e-6 {
                break;
            }
            previous = mean;
        }
        let mean = cycle.iter().sum::<f64>() / f64::from(steps);
        assert!(
            (mean - previous).abs() < 1e-6,
            "not periodic: {mean} after {previous}"
        );
        let noon = cycle[usize::try_from(steps / 2).unwrap() - 1];
        let predawn = cycle.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let emitted = cycle
            .iter()
            .map(|v| regolith.emission() * math::powi(*v, 4))
            .sum::<f64>()
            / f64::from(steps);
        let absorbed = inputs.iter().sum::<f64>() / f64::from(steps) + regolith.heat_flow_w_m2;
        eprintln!(
            "noon {noon:.2} K, predawn {predawn:.2} K, mean {mean:.2} K, emitted {emitted:.4} against {absorbed:.4} W m⁻²"
        );
        assert!((noon - 386.3).abs() < 1.0, "{noon}");
        assert!((predawn - 96.1).abs() < 1.0, "{predawn}");
        assert!((mean - 220.5).abs() < 1.0, "{mean}");
        assert!(
            (emitted / absorbed - 1.0).abs() < 1e-6,
            "{emitted} against {absorbed}"
        );
    }

    /// The groups' lanes are the lone column's to the bit, on every clock: the reference Moon's
    /// and Mars's runs (whole days, the slab, its snow and the frost), the climate tests' 3:2
    /// Mercury, bare and under the reference Mars's air (true time, phase columns, the slab's
    /// map), and a seasonal locked Moon (the beam, at e 0.05 and the Moon's 1.535° of obliquity,
    /// every 37th site of its grid, so that its last group is part-filled) give the same orbits,
    /// the same tallies, every field, the same slabs' mean emissions and the same frosts, in
    /// groups of [`LANES`] as one column at a time.
    #[test]
    fn the_lanes_are_the_lone_column_to_the_bit() {
        use hyperion_testkit::float::assert_same_bits;
        let moon = reference::moon_like();
        let locked = locked_like(&moon, 0.05, 1.535_f64.to_radians());
        let (mercury, mars) = (
            super::super::tests::other_regimes::mercury_like(),
            reference::mars_like(),
        );
        let thin_mercury = super::super::tests::with_thin_air(&mercury, &mars);
        for (name, inputs, every) in [
            ("Moon", moon, 1),
            ("Mars", mars, 1),
            ("Mercury", mercury, 1),
            ("Mercury under Mars's air", thin_mercury, 1),
            ("locked Moon", locked, 37),
        ] {
            let (grid, coordinates) = frame(&inputs);
            let regolith = Regolith::of(&inputs);
            let directions: Vec<[f64; 3]> = sites(&grid, coordinates)
                .0
                .into_iter()
                .step_by(every)
                .collect();
            let (schedule, layers) =
                schedule(&inputs, &directions, &regolith, coordinates).unwrap();
            let model = Model {
                regolith,
                air: ThinAir::of(&inputs, &regolith, 110.0),
                layers,
            };
            let start = vec![200.0; directions.len()];
            let lone = model.march::<1, _>(&schedule, &directions, &start, &mut |_: &Sample| {});
            let grouped =
                model.march::<LANES, _>(&schedule, &directions, &start, &mut |_: &Sample| {});
            eprintln!(
                "{name}: {} columns of {} phases, {} orbits",
                directions.len(),
                schedule.phases,
                lone.orbits
            );
            assert_eq!(lone.orbits, grouped.orbits, "{name}");
            assert_eq!(lone.converged, grouped.converged, "{name}");
            assert_eq!(lone.tallies.len(), grouped.tallies.len(), "{name}");
            for (a, b) in lone.tallies.iter().zip(&grouped.tallies) {
                assert_same_bits(a.reference_k, b.reference_k);
                assert_same_bits(a.weight, b.weight);
                for (x, y) in a.sums.iter().zip(&b.sums) {
                    assert_same_bits(*x, *y);
                }
                assert_same_bits(a.coldest_k, b.coldest_k);
                assert_same_bits(a.warmest_k, b.warmest_k);
                for (x, y) in a.month_sums.iter().zip(&b.month_sums) {
                    assert_same_bits(*x, *y);
                }
                for (x, y) in a.month_weights.iter().zip(&b.month_weights) {
                    assert_same_bits(*x, *y);
                }
            }
            for (pair, name_of) in [
                (
                    (&lone.mean_emission_w_m2, &grouped.mean_emission_w_m2),
                    "mean emission",
                ),
                ((&lone.frost_kg_m2, &grouped.frost_kg_m2), "frost"),
            ] {
                assert_eq!(pair.0.len(), pair.1.len(), "{name}: {name_of}");
                for (x, y) in pair.0.iter().zip(pair.1) {
                    assert_same_bits(*x, *y);
                }
            }
        }
    }

    /// The true-time map of the slab's mean emission (decision-r09-t13h-scope §7.2).
    ///
    /// On the 3:2 Mercury's schedule with its orbit made to turn the day by 12 and by 11.5 phase
    /// columns: each phase column p takes in orbit o the hour angles column p + N frac(D) took in
    /// orbit o − 1; `absorbed` lights it as column p + 12 one orbit back, and at 11.5 as column
    /// p + 23 two orbits back, at every step; the map reads that column, copied where the turn is
    /// whole and blended half and half at 11.5; a turn within [`COMMENSURATE_COLUMNS`] of a whole
    /// number (11.996) is taken as whole; and each band's sum of the mapped emission round its ring
    /// is the previous orbit's to 10⁻¹² relative.
    #[test]
    fn the_true_time_map_reads_the_column_at_the_same_hour_angles_one_orbit_back() {
        let inputs = super::super::tests::other_regimes::mercury_like();
        let (grid, coordinates) = frame(&inputs);
        let regolith = Regolith::of(&inputs);
        let (directions, _) = sites(&grid, coordinates);
        let (mut schedule, _) = schedule(&inputs, &directions, &regolith, coordinates).unwrap();
        let (phases, sites) = (schedule.phases, directions.len());
        assert_eq!(phases, PHASE_COLUMNS);
        let count = sites * phases;
        for (turned, expected) in [
            (12.0, Turn::Whole(12)),
            (11.5, Turn::Between(11, 0.5)),
            (11.996, Turn::Whole(12)),
        ] {
            let days = 7.0 + turned / index(phases);
            if let Light::Turning { days_per_orbit, .. } = &mut schedule.light {
                *days_per_orbit = days;
            }
            let turn = Turn::of(&schedule).unwrap();
            match (turn, expected) {
                (Turn::Whole(a), Turn::Whole(b)) => assert_eq!(a, b),
                (Turn::Between(a, w), Turn::Between(b, v)) => {
                    assert_eq!(a, b);
                    assert!((w - v).abs() < 1e-12, "{w}");
                }
                (Turn::Whole(_), Turn::Between(..)) | (Turn::Between(..), Turn::Whole(_)) => {
                    panic!("{turned} columns: {turn:?}")
                }
            }
            assert_lit_alike_orbits_back(&schedule, &directions, days, turned);
            assert_the_map_reads_its_turn(&schedule, count, turn, turned);
        }
    }

    /// For [`the_true_time_map_reads_the_column_at_the_same_hour_angles_one_orbit_back`]: on
    /// `schedule` at the sites `directions`, its orbit turning `days` solar days and so `turned`
    /// phase columns, each phase column p takes in orbit o the hour angles column p + `turned` took
    /// in orbit o − 1; and where one or two orbits turn it by a whole number of columns,
    /// `absorbed` lights it as that column then, at every seventh step and three sites.
    fn assert_lit_alike_orbits_back(
        schedule: &Schedule,
        directions: &[[f64; 3]],
        days: f64,
        turned: f64,
    ) {
        let (phases, sites) = (schedule.phases, directions.len());
        let offset = |orbit: u32, phase: f64| {
            let turns = f64::from(orbit) * days;
            TAU * (turns - turns.floor()) + TAU * phase / index(phases)
        };
        // The fewest orbits back, one or two, over which the turn is whole, and its columns.
        let back = (1..=2_u32).find_map(|k| {
            let whole = f64::from(k) * turned;
            ((whole - whole.round()).abs() < 1e-12).then(|| (k, whole.round()))
        });
        for orbit in 2..5 {
            for p in 0..phases {
                let (now, then) = (
                    offset(orbit, index(p)),
                    offset(orbit - 1, index(p) + turned),
                );
                assert!(
                    reduced(now - then).abs() < 1e-10,
                    "{turned}: {p}, orbit {orbit}"
                );
                let Some((k, whole)) = back else {
                    continue;
                };
                let q = (p + usize::try_from(floor_count(whole, u32::MAX)).unwrap()) % phases;
                for step in (0..schedule.month_of.len()).step_by(7) {
                    for site in [10, 36, 60] {
                        let a = Model::absorbed(schedule, directions, site, p, step, orbit, sites);
                        let b =
                            Model::absorbed(schedule, directions, site, q, step, orbit - k, sites);
                        assert!(
                            (a - b).abs() < 1e-9 * (a.abs() + 1.0),
                            "{turned}: column {p} against {q}, {k} orbits back: {a} against {b}"
                        );
                    }
                }
            }
        }
    }

    /// For [`the_true_time_map_reads_the_column_at_the_same_hour_angles_one_orbit_back`]: on
    /// `schedule`'s `count` columns, `carried` copies column p + n where the turn `turn` is whole
    /// and blends p + n and p + n + 1 by its fraction otherwise, and keeps each band's ring sum to
    /// 10⁻¹² relative, on a synthetic table of means.
    fn assert_the_map_reads_its_turn(schedule: &Schedule, count: usize, turn: Turn, turned: f64) {
        let phases = schedule.phases;
        let means: Vec<f64> = (0..schedule.months * count)
            .map(|i| 100.0 + 50.0 * math::sin(0.37 * index(i)))
            .collect();
        let next = carried(schedule, means.clone(), count);
        for (month, (before, after)) in means
            .chunks_exact(count)
            .zip(next.chunks_exact(count))
            .enumerate()
        {
            for (site, (ring, mapped)) in before
                .chunks_exact(phases)
                .zip(after.chunks_exact(phases))
                .enumerate()
            {
                for (p, &value) in mapped.iter().enumerate() {
                    let expected = match turn {
                        Turn::Whole(n) => ring[(p + n) % phases],
                        Turn::Between(n, w) => {
                            (1.0 - w) * ring[(p + n) % phases] + w * ring[(p + n + 1) % phases]
                        }
                    };
                    hyperion_testkit::float::assert_same_bits(value, expected);
                }
                let (a, b): (f64, f64) = (ring.iter().sum(), mapped.iter().sum());
                assert!(
                    (a / b - 1.0).abs() < 1e-12,
                    "{turned}: month {month}, site {site}: {a} against {b}"
                );
            }
        }
    }

    /// A recorder of every sample a run takes, by orbit.
    #[derive(Default)]
    struct Recorder {
        samples: Vec<(usize, usize, f64, f64)>,
    }

    /// The moments, the extremes, the months and the annual mean of every site are the direct
    /// sums over the last orbit's samples (the reference Moon), about each site's mean over the
    /// orbit before; and ⟨T⁴⟩ from the moments is the samples' own.
    #[test]
    fn the_moments_are_the_direct_sums() {
        let inputs = reference::moon_like();
        let mut recorder = Recorder::default();
        let solution = solve_observed(&inputs, &mut |s: &Sample| {
            recorder
                .samples
                .push((s.site, s.month, s.weight, s.surface_k));
        });
        let climate = &solution.climate;
        let points = solution.grid.points();
        let sites = points + solution.ends.len();
        let per_orbit = recorder.samples.len() / usize::try_from(climate.orbits).unwrap();
        assert!(climate.orbits >= 2 && climate.converged);
        let orbit = |k: usize| &recorder.samples[k * per_orbit..(k + 1) * per_orbit];
        let last = usize::try_from(climate.orbits).unwrap() - 1;
        let mean_of = |samples: &[(usize, usize, f64, f64)]| {
            let mut sums = vec![(0.0, 0.0); sites];
            for &(site, _, w, t) in samples {
                sums[site].0 += w * t;
                sums[site].1 += w;
            }
            sums.iter().map(|(s, w)| s / w).collect::<Vec<f64>>()
        };
        let reference_k = mean_of(orbit(last - 1));
        let annual = mean_of(orbit(last));
        for site in 0..sites {
            let (moments, months) = if site < points {
                (
                    solution.moments[site],
                    climate
                        .months_k
                        .iter()
                        .map(|m| m[site])
                        .collect::<Vec<f64>>(),
                )
            } else {
                let end = &solution.ends[site - points];
                (end.moments, end.months_k.clone())
            };
            assert!((moments.reference_k - reference_k[site]).abs() < 1e-9);
            let mine: Vec<&(usize, usize, f64, f64)> =
                orbit(last).iter().filter(|s| s.0 == site).collect();
            let weight: f64 = mine.iter().map(|s| s.2).sum();
            for k in 0..4 {
                let direct = mine.iter().fold(0.0, |sum, s| {
                    sum + s.2 * math::powi(s.3 - reference_k[site], i32::try_from(k + 1).unwrap())
                }) / weight;
                let scale = math::powi(100.0, i32::try_from(k + 1).unwrap());
                assert!(
                    (moments.about_reference[k] - direct).abs() < 1e-9 * scale,
                    "site {site}, k {}: {} against {direct}",
                    k + 1,
                    moments.about_reference[k]
                );
            }
            let fourth = mine
                .iter()
                .fold(0.0, |sum, s| sum + s.2 * math::powi(s.3, 4))
                / weight;
            assert!((moments.fourth_power_mean(0.0) / fourth - 1.0).abs() < 1e-11);
            let shifted = mine
                .iter()
                .fold(0.0, |sum, s| sum + s.2 * math::powi(s.3 - 3.0, 4))
                / weight;
            assert!((moments.fourth_power_mean(-3.0) / shifted - 1.0).abs() < 1e-11);
            let coldest = mine.iter().fold(f64::INFINITY, |a, s| a.min(s.3));
            let warmest = mine.iter().fold(f64::NEG_INFINITY, |a, s| a.max(s.3));
            hyperion_testkit::float::assert_same_bits(moments.coldest_k, coldest);
            hyperion_testkit::float::assert_same_bits(moments.warmest_k, warmest);
            for (month, &value) in months.iter().enumerate() {
                let (s, w) = mine
                    .iter()
                    .filter(|s| s.1 == month)
                    .fold((0.0, 0.0), |(s, w), x| (s + x.2 * x.3, w + x.2));
                assert!((value - s / w).abs() < 1e-9, "site {site}, month {month}");
            }
            let stored = if site < points {
                climate.annual_k[site]
            } else {
                solution.ends[site - points].annual_k
            };
            assert!((stored - annual[site]).abs() < 1e-9);
            assert!((moments.reference_k + moments.about_reference[0] - annual[site]).abs() < 1e-9);
        }
    }

    /// A world locked 1:1 to its host in a one-month year is its closed form at every point and
    /// end column, εσT⁴ = (1 − A) F cos γ₊ + εσ`T_CMB`⁴ + Q: the substellar point at its full
    /// light, the nightside at the night floor, with one month and no spread in time.
    #[test]
    fn a_locked_world_in_a_one_month_year_is_its_closed_form() {
        let moon = reference::moon_like();
        let inputs = locked_like(&moon, 0.0, 0.0);
        assert_eq!(inputs.months(), 1);
        let solution = solve(&inputs);
        assert_eq!(solution.coordinates, Coordinates::TidallyLocked);
        assert_eq!(solution.climate.months_k.len(), 1);
        let regolith = Regolith::of(&inputs);
        let floor = night_floor(inputs.heat_flow()).value();
        let normal = (1.0 - regolith.albedo) * inputs.host_flux().value();
        let expected = |cosine: f64| {
            ((normal * cosine.max(0.0) + regolith.floor_flux_w_m2()) / regolith.emission())
                .sqrt()
                .sqrt()
        };
        for p in 0..solution.grid.points() {
            let cosine = solution.grid.centre_direction(p)[2];
            let t = solution.climate.annual_k[p];
            assert!((t - expected(cosine)).abs() < 1e-9, "point {p}: {t}");
            assert!(
                solution.moments[p]
                    .about_reference
                    .iter()
                    .all(|m| m.abs() < 1e-300)
            );
        }
        for end in &solution.ends {
            assert!(
                (end.annual_k - expected(end.direction[0])).abs() < 1e-9,
                "{end:?}"
            );
        }
        assert!((solution.lower_bound_k - floor).abs() < 1e-12);
        let antistellar = solution.climate.annual_k[0];
        assert!(
            (antistellar - floor).abs() < 1e-9,
            "{antistellar} against {floor}"
        );
    }

    /// The balanced slab's limit (decision-r09-t13-thin-air §1.10, test 2): a frost-free column
    /// under constant sunlight S in the closed form has a slab of σ`T_a`⁴ = εσ`T_s`⁴ ÷ 2 and emits
    /// εσ`T_s`⁴ = 2 (S + `E_nf`) ÷ (2 − εβ), each to 10⁻⁹ relative, with no snow; and with the heat
    /// flow and the background set to nothing it is T13.c's closed form, εσT⁴ = S + εβ ÷ (2 − εβ) S,
    /// to 10⁻¹² relative, for every β from none to 1. Under the reference Mars's frost a dark
    /// column is held at its frost point under the slab's bound, its frost point aloft.
    #[test]
    fn the_balanced_slab_s_limit_is_t13c_s_closed_form() {
        let mars = reference::mars_like();
        let regolith = Regolith::of(&mars);
        let (epsilon, floor) = (regolith.emissivity, regolith.floor_flux_w_m2());
        for beta in [0.0, 0.163, 0.265, 0.6, 1.0] {
            let air = ThinAir {
                absorptivity: beta,
                frost: None,
            };
            for sunlight in [0.5, 50.0, 103.9, 400.0, 1_200.0] {
                let Balanced { surface_k, slab } = air.balance(epsilon, sunlight + floor);
                let emitted = regolith.emission() * math::powi(surface_k, 4);
                let expected = 2.0 * (sunlight + floor) / (2.0 - epsilon * beta);
                assert!(
                    (emitted / expected - 1.0).abs() < 1e-9,
                    "β {beta}, S {sunlight}"
                );
                assert!(
                    (slab.blackbody_w_m2 / (0.5 * emitted) - 1.0).abs() < 1e-9,
                    "β {beta}, S {sunlight}"
                );
                assert!(
                    (slab.down_w_m2 - beta * slab.blackbody_w_m2).abs() < 1e-12 * emitted,
                    "β {beta}, S {sunlight}"
                );
                hyperion_testkit::float::assert_same_bits(slab.snow_w_m2, 0.0);
                let bare = air.balance(epsilon, sunlight).surface_k;
                let present = ((sunlight + epsilon * beta / (2.0 - epsilon * beta) * sunlight)
                    / regolith.emission())
                .sqrt()
                .sqrt();
                assert!(
                    (bare / present - 1.0).abs() < 1e-12,
                    "β {beta}, S {sunlight}: {bare} against T13.c's {present}"
                );
            }
        }
        let air = ThinAir::of(&mars, &regolith, 103.9);
        let frost = air.frost.expect("Mars's carbon dioxide condenses");
        let dark = air.balance(epsilon, floor);
        hyperion_testkit::float::assert_same_bits(dark.surface_k, frost.point_k);
        hyperion_testkit::float::assert_same_bits(
            dark.slab.blackbody_w_m2,
            frost.aloft_blackbody_w_m2(),
        );
        assert!(dark.slab.snow_w_m2 > 0.0);
    }

    /// An airless column's closed form and spin-up start are T13.c's to the bit: with no slab,
    /// [`ThinAir::balance`] of [`Regolith::outside_w_m2`] is ((S + εσ`T_CMB`⁴ + 0) + Q) ÷ εσ, its
    /// fourth root, summed as T13.c summed it, for the reference Moon's and Ceres's regoliths at
    /// any sunlight.
    #[test]
    fn an_airless_closed_form_is_t13c_s_to_the_bit() {
        for inputs in [reference::moon_like(), reference::ceres_like()] {
            let regolith = Regolith::of(&inputs);
            for sunlight in [0.0, 1e-3, 0.5, 37.2, 341.0, 1_361.0, 9_126.6] {
                let t = ThinAir::NONE
                    .balance(regolith.emissivity, regolith.outside_w_m2(sunlight))
                    .surface_k;
                let t13c = ((regolith.input(sunlight, 0.0) + regolith.heat_flow_w_m2)
                    / regolith.emission())
                .sqrt()
                .sqrt();
                hyperion_testkit::float::assert_same_bits(t, t13c);
            }
        }
    }

    /// A world locked 1:1 on an orbit all but circular and untilted, which has twelve months,
    /// marches each point's column to its closed form: the seasonal path on the locked grid.
    #[test]
    fn a_locked_world_s_seasonal_march_tends_to_its_closed_form() {
        let moon = reference::moon_like();
        let marched = solve(&locked_like(&moon, 1e-7, 1e-7));
        let closed = solve(&locked_like(&moon, 0.0, 0.0));
        assert_eq!(marched.climate.months_k.len(), 12);
        assert!(
            marched.climate.converged,
            "{} orbits",
            marched.climate.orbits
        );
        let worst = marched
            .climate
            .annual_k
            .iter()
            .zip(&closed.climate.annual_k)
            .fold(0.0, |most: f64, (a, b)| most.max((a - b).abs()));
        eprintln!("{} orbits, worst {worst:e} K", marched.climate.orbits);
        assert!(worst < 0.1, "{worst}");
    }

    /// The closed form's branches (decision-r09-t13h-scope, "§1.9 item 5 amended"), on the
    /// reference Mars's frost at β 0.2653, 0.6 and 0.9: F = max(B, G) is continuous to 10⁻⁹
    /// relative where the bound meets the unbounded slab, `E_hi` = (2 − εβ) A, F = 2A there, and
    /// where the bare column meets the frost, `E_lo` = (1 − β) B + (2 − ε) β A, F = B there; every
    /// bare column's books close, `E_in` = (1 − β) F + (2 − ε) β max(F ÷ 2, A), to 10⁻¹² relative;
    /// below `E_lo` the column is held at its frost point under the bound; and at β = 1 the bare
    /// band is empty, F stepping from B to 2A at `E_in` = (2 − ε) A.
    #[test]
    fn the_closed_form_s_branches_join_and_close_their_books() {
        use hyperion_testkit::float::assert_same_bits;
        let mars = reference::mars_like();
        let regolith = Regolith::of(&mars);
        let epsilon = regolith.emissivity;
        let frost = Frost::of(&mars).expect("Mars's carbon dioxide condenses");
        let (a, b) = (
            frost.aloft_blackbody_w_m2(),
            regolith.emission() * math::powi(frost.point_k, 4),
        );
        assert!(b < 2.0 * a, "a frost-held night lies under the bound");
        let emitted =
            |air: &ThinAir, outside: f64| air.balance(epsilon, outside).slab.mean_emission_w_m2;
        for beta in [0.2653, 0.6, 0.9] {
            let air = ThinAir {
                absorptivity: beta,
                frost: Some(frost),
            };
            let high = (2.0 - epsilon * beta) * a;
            let low = (1.0 - beta) * b + (2.0 - epsilon) * beta * a;
            assert!(low < high, "β {beta}: {low} against {high}");
            for (edge, value) in [(high, 2.0 * a), (low, b)] {
                let (below, above) = (
                    emitted(&air, edge * (1.0 - 1e-12)),
                    emitted(&air, edge * (1.0 + 1e-12)),
                );
                assert!(
                    (above / below - 1.0).abs() < 1e-9,
                    "β {beta}, at {edge}: {below} against {above}"
                );
                assert!(
                    (emitted(&air, edge) / value - 1.0).abs() < 1e-9,
                    "β {beta}, at {edge}"
                );
            }
            for k in 0..=64 {
                let outside = low + (2.0 * high - low) * f64::from(k) / 64.0;
                let f = emitted(&air, outside);
                if f > b {
                    let books = (1.0 - beta) * f + (2.0 - epsilon) * beta * (0.5 * f).max(a);
                    assert!(
                        (books / outside - 1.0).abs() < 1e-12,
                        "β {beta}, E_in {outside}: {books}"
                    );
                }
            }
            let night = air.balance(epsilon, 0.5 * low);
            assert_same_bits(night.surface_k, frost.point_k);
            assert_same_bits(night.slab.blackbody_w_m2, a);
        }
        let air = ThinAir {
            absorptivity: 1.0,
            frost: Some(frost),
        };
        let step = (2.0 - epsilon) * a;
        assert_same_bits(
            air.balance(epsilon, step * (1.0 - 1e-12)).surface_k,
            frost.point_k,
        );
        assert!((emitted(&air, step) / (2.0 * a) - 1.0).abs() < 1e-12);
    }

    /// A thin-aired world locked 1:1 on an orbit all but circular and untilted marches each
    /// point's column to within 0.1 K of its closed form, as the airless one does
    /// (decision-r09-t13h-scope, "§1.9 item 5 amended", test 1): the reference Mars locked 1:1 at
    /// e and obliquity 10⁻⁷ against both at 0. The closed form's bounded slab over bare ground pays
    /// the snow it sublimes, as the march's ground does (decision-r09-t13-thin-air §1.9 items 3 and
    /// 5; decision-r09-t13h-scope §2), so the terminator's band, where the slab is bounded and the
    /// ground bare, `T_f` < T < (2 ÷ ε)^¼ `T_fa`, agrees with the march too.
    #[test]
    fn a_thin_aired_locked_world_s_seasonal_march_tends_to_its_closed_form() {
        let mars = reference::mars_like();
        let marched = solve(&locked_like(&mars, 1e-7, 1e-7));
        let closed = solve(&locked_like(&mars, 0.0, 0.0));
        assert!(
            marched.climate.converged,
            "{} orbits",
            marched.climate.orbits
        );
        assert!(marched.air.absorptivity > 0.0);
        let frost = closed.air.frost.expect("Mars's carbon dioxide condenses");
        let join = (2.0 / closed.emissivity).sqrt().sqrt() * frost.aloft_point_k;
        let (mut worst, mut at, mut band) = (0.0_f64, 0, 0);
        for (p, (a, b)) in marched
            .climate
            .annual_k
            .iter()
            .zip(&closed.climate.annual_k)
            .enumerate()
        {
            if *b > frost.point_k + 1e-9 && *b < join {
                band += 1;
            }
            if (a - b).abs() > worst {
                (worst, at) = ((a - b).abs(), p);
            }
        }
        eprintln!(
            "{} orbits, β {:.4}: worst {worst:.3e} K at point {at} ({:.3} K closed); {band} points \
             in the bounded bare band, {:.2}–{join:.2} K",
            marched.climate.orbits,
            marched.air.absorptivity,
            closed.climate.annual_k[at],
            frost.point_k
        );
        assert!(band > 0, "the test reaches the bounded bare band");
        assert!(worst < 0.1, "{worst} at {at}");
    }

    /// `inputs` locked 1:1 to its seasonal host on its own orbit's period at eccentricity `e` and
    /// obliquity `obliquity` rad, its regime's forcing locked.
    fn locked_like(inputs: &CoarseInputs, e: f64, obliquity: f64) -> CoarseInputs {
        let period = inputs.seasonal_orbit().unwrap().period;
        super::super::tests::builder_of(inputs)
            .spin(crate::planetary::surface::inputs::Spin {
                obliquity: Radians::new(obliquity),
                rotation_period: period,
                solar_day: None,
                equinox_true_anomaly: Radians::ZERO,
            })
            .seasonal_orbit(crate::planetary::surface::inputs::SeasonalOrbit {
                eccentricity: e,
                period,
            })
            .climate(crate::planetary::surface::inputs::ClimateRegime {
                forcing: Forcing::Locked,
                ..*inputs.climate()
            })
            .build()
            .unwrap()
    }
}
