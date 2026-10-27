//! A body's subsurface ocean beyond its flag (plan 14, P14.T51.g; science-p14-subsurface-ocean.md):
//! so far the floor of a regular moon's ocean at the liquidus of its base, P14.T51.g.a, a must-fix
//! of the 23 → 24 batch (decision-p14-t51g Q9).
//!
//! At version 23 a regular moon's published ocean is its flag's whole water layer less the flag's
//! shell, pure water on rock at 273 K at its base
//! ([`SubsurfaceOcean::of_regular_moon`](crate::planetary::hooks::SubsurfaceOcean::of_regular_moon)).
//! Water at 273 K is liquid only up to ice V's melting pressure there, 626.09 MPa (IAPWS R14-08,
//! as the registry's water row carries it), and a large icy moon's layer reaches 1.5–3.1 GPa, so
//! the wire carried oceans 0.9–1.4 Mm deep. From [`OCEAN_FLOOR_VERSION`] the flag's shell, pure
//! water and 273 K base stay, and the ocean ends at the shallower of the rock and that pressure
//! ([`ocean_end`]):
//!
//! - **The column** ([`WaterColumn`]): the body's water spread over its rock at
//!   [`WATER_DENSITY`], as the flag spreads it, with the hydrostatic pressure in it, P(r), and the
//!   radius at which it reaches a pressure, found by a fixed count of halvings.
//! - **The liquidus** ([`base_liquidus`]): water's melting pressure at the ocean's base
//!   temperature, read from the registry's diagram (`PhaseDiagram::melting_pressure`), not copied.
//! - **The end**: the rock where the pressure there is at most the liquidus; else ice V at the
//!   radius where the pressure reaches it; and none modelled where the shell's base itself lies at
//!   or beyond it, liquid under high-pressure ice being P14.T55.k's.
//!
//! Only the published member reads it: the flag, the passes' ocean bit and the tides read the
//! moon's tidal branch, as at 23, and the shell is the flag's. The tectonic regime reads the
//! published member's shell, so an absent member gives it none. That is the branch's regime
//! wherever the absent member's shell is thicker than the 60 km an ice shell's regime needs
//! ([`ICE_SHELL_LIMIT`](crate::planetary::hooks::figures::interior::ICE_SHELL_LIMIT)): a shell
//! whose base reaches 626.09 MPa is thinner only where the gravity in it exceeds 626.09 MPa ÷
//! (`ρ_w` × 60 km) = 10.4 m s⁻², an icy regular moon of some 4–6 M⊕. The tests' census asserts
//! it over the golden systems, the 400 and LOOK72 (recorded in plan 14). P14.T51.g.b reuses the
//! column for every body.
//!
//! # Every body's ocean (P14.T51.g.b)
//!
//! From [`OCEAN_VERSION`] every body with a solid surface takes its ocean from this module's
//! solve ([`Hydrosphere`]), in place of the flag's and its floor (decision-p14-t51g; science-p14-
//! subsurface-ocean §2):
//!
//! - **The structure.** The body's water, the larger of its bulk's and its condensed water, is
//!   spread over its rock at [`WATER_DENSITY`] ([`WaterColumn::of_water`]), with its pressure P(r).
//! - **The shell** conducts the body's heat flow Q = 4πR²F in steady state in a sphere, with
//!   k = C ÷ T ([`ICE_CONDUCTIVITY_COEFFICIENT`]) held at its value at
//!   [`ICE_CONDUCTIVITY_FLOOR`] below it: ∫k dT from the surface's `T_s` to T(r) is
//!   R²F (1/r − 1/R).
//! - **The liquidus** at the shell's base is ice Ih's melting point at its pressure less the
//!   solutes' depression ([`AqueousLiquidus`]), the solutes being the body's chlorine as sodium
//!   chloride and its ice's ammonia ([`OceanSolutes`]), all in the liquid, which ice Ih excludes.
//! - **The ocean** is isothermal at its base temperature, and rests on the shallower of the rock
//!   and the high-pressure ice whose liquidus meets it.
//! - **The solve** reads the shell's base by its pure-water melting point T* (ice Ih's at the
//!   base's pressure): at each T* the base's pressure is explicit, its radius and the floor's are
//!   the column's halvings, and the heat flow that puts the base there, F(T*), rises strictly with
//!   T*, so a fixed count of halvings of T* finds the base ([`SHELL_BASE_HALVINGS`]). The
//!   presence test, [`Hydrosphere::holds_an_ocean`], is the plan's closed form at the deepest
//!   base the column allows.
//! - **None** where the surface is at or above water's triple point or its liquidus, where there
//!   is no water or heat, and where the test fails above ice Ih's foot; **not modelled** where it
//!   fails with the column reaching the foot, its base lies in ice II's field (Hammond et al.
//!   2016's line), or its floor, at its own pressure and the ocean's temperature, lies outside
//!   the field of the ice it names (decision-p14-t51gb-ice-ii; P14.T55.k's).
//!
//! P14.T56.d's ocean branch reads the same solve at the branch's inputs
//! ([`MoonTides`](crate::planetary::moons::tides::MoonTides)), whose shell alone may make a moon's
//! regime an ice shell's; the every-body published ocean never feeds it (decision-p14-t51g Q1).

use crate::math;
use crate::planetary::derive::atmosphere::{Gas, VOLATILE_SCATTER_DEX};
use crate::planetary::hooks::figures::{OceanFloor, Subsurface, SubsurfaceOcean};
use crate::planetary::moons::regular::WATER_DENSITY;
use crate::stellar::draws::UnitUniform;
use crate::substance::Element;
use crate::substance::phase::{
    AqueousLiquidus, ForeignIce, HighPressureIce, foreign_ice, high_pressure_melting, ice_ih_foot,
    ice_ih_melting_pressure, ice_ih_melting_temperature, water_triple_temperature,
};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerCubicMetre, Metres, Pascals, WattsPerSquareMetre,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod tests;

/// The generator version from which a regular moon's published ocean ends at its liquidus
/// (P14.T51.g.a): 24, the 23 → 24 batch's bump, P14.T54.b, a must-fix member that boards until
/// P14.T54.b.l's blessing message (decision-p14-t51g Q9).
///
/// Below it [`OceanFloorRules::in_force`] is [`OceanFloorRules::Held`], so that every published
/// ocean is version 23's bit for bit. P14.T54.b deletes this hold, [`OceanFloorRules`] with its
/// `Held` arm and the tests' switch, and its tripwire
/// (`the_ocean_floor_is_held_only_until_the_24_bump`).
pub(crate) const OCEAN_FLOOR_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether a regular moon's published ocean ends at its liquidus (the hold,
/// [`OCEAN_FLOOR_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OceanFloorRules {
    /// Versions 23's and before: the flag's whole water layer less its shell, on rock.
    Held,
    /// The 23 → 24 batch's: the layer ends at the shallower of the rock and the liquidus of its
    /// base ([`ocean_end`]).
    Floored,
}

impl OceanFloorRules {
    /// The rules this generator version gives: the floor from [`OCEAN_FLOOR_VERSION`], none below
    /// it; in the tests, the rules a test's switch sets on its thread ([`with_ocean_floor`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = OCEAN_FLOOR_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= OCEAN_FLOOR_VERSION {
            Self::Floored
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The ocean floor's rules a test sets on its thread, `None` for the version's.
    static OCEAN_FLOOR_RULES: core::cell::Cell<Option<OceanFloorRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the ocean floor's rules `rules` on this thread, for the tests to reach 24's
/// oceans ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// P14.T54.b deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_ocean_floor<R>(rules: OceanFloorRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<OceanFloorRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OCEAN_FLOOR_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(OCEAN_FLOOR_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The halvings by which [`WaterColumn::radius_at`] finds a radius: 64, a fixed count, so that the
/// bracket of a column up to 10⁷ m deep closes past f64's resolution of its radii (10⁷ m ÷ 2⁶⁴ is
/// 5 × 10⁻¹³ m) and its pressure is the asked one to about 10⁻¹⁵ of itself.
pub(crate) const RADIUS_HALVINGS: u32 = 64;

/// The least radius of a column's base, as a share of the body's radius: 0.001 (P14.T51.g's
/// structure, "to `r_w` (at least 0.001 R)"), so that the column of a body whose water fills it,
/// where the flag's layer reaches the centre, keeps a finite pressure at its base.
pub(crate) const LEAST_BASE_SHARE: f64 = 0.001;

/// A body's water spread over its rock at [`WATER_DENSITY`], `ρ_w`, from the base radius `r_w` to
/// its surface R, as the flag spreads it (P14.T17.b's
/// [`ocean_layers`](crate::planetary::moons::regular::ocean_layers)), with the body's whole mass M
/// beneath and in it.
///
/// Its pressure at a radius r in the water is the hydrostatic column's (P14.T51.g's structure):
///
/// P(r) = `ρ_w` G [(M − (4π/3) `ρ_w` R³)(1/r − 1/R) + (2π/3) `ρ_w` (R² − r²)],
///
/// the integral of `ρ_w` G m(s) ÷ s² from r to R, m(s) = M − (4π/3) `ρ_w` (R³ − s³) being the mass
/// inside s. Within the water m(s) is at least the rock's (1 − x) M, so P falls monotonically from
/// the base to 0 at the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WaterColumn {
    /// The body's mean radius R.
    radius: Metres,
    /// The body's mass M.
    mass: Kilograms,
    /// The water's base radius `r_w`: R less the layer's depth, at least [`LEAST_BASE_SHARE`] R.
    base: Metres,
}

impl WaterColumn {
    /// The column of a body of mean radius `radius` and mean density `density`, whose water lies
    /// `depth` deep over its rock: M = (4π/3) R³ ρ̄, and its base R − `depth`, at least
    /// [`LEAST_BASE_SHARE`] R.
    #[must_use]
    pub(crate) fn new(radius: Metres, density: KilogramsPerCubicMetre, depth: Metres) -> Self {
        let r = radius.value();
        let mass = 4.0 / 3.0 * core::f64::consts::PI * r * r * r * density.value();
        let base = r - depth.value();
        let least = LEAST_BASE_SHARE * r;
        Self {
            radius,
            mass: Kilograms::new(mass),
            // Ordered by comparison, not `max`, which may return either zero at a tie of signs.
            base: Metres::new(if base > least { base } else { least }),
        }
    }

    /// The column of a body of mean radius `radius` and mass `mass` whose water is the mass
    /// fraction `water` of it (P14.T51.g.b's structure): x M spread over the rock at
    /// [`WATER_DENSITY`], its base R (1 − x M ÷ (`ρ_w` (4π/3) R³))^⅓, at least
    /// [`LEAST_BASE_SHARE`] R, which a body whose water would fill it keeps.
    #[must_use]
    pub(crate) fn of_water(radius: Metres, mass: Kilograms, water: f64) -> Self {
        let r = radius.value();
        let volume = 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
        let share = water * mass.value() / (WATER_DENSITY.value() * volume);
        let solid = if share < 1.0 { 1.0 - share } else { 0.0 };
        let base = r * math::cbrt(solid);
        let least = LEAST_BASE_SHARE * r;
        Self {
            radius,
            mass,
            // Ordered by comparison, not `max`, which may return either zero at a tie of signs.
            base: Metres::new(if base > least { base } else { least }),
        }
    }

    /// The body's mean radius, R.
    #[must_use]
    pub(crate) const fn radius(&self) -> Metres {
        self.radius
    }

    /// The body's mass, M.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn mass(&self) -> Kilograms {
        self.mass
    }

    /// The radius of its base, `r_w`.
    #[must_use]
    pub(crate) const fn base(&self) -> Metres {
        self.base
    }

    /// Its pressure at the radius `r` in its water, P(r), Pa: 0 at the surface, and rising to its
    /// base.
    #[must_use]
    pub(crate) fn pressure_at(&self, r: Metres) -> Pascals {
        let (rho, big_r, r) = (WATER_DENSITY.value(), self.radius.value(), r.value());
        let pi = core::f64::consts::PI;
        let beneath = self.mass.value() - 4.0 / 3.0 * pi * rho * big_r * big_r * big_r;
        Pascals::new(
            rho * GRAVITATIONAL_CONSTANT
                * (beneath * (1.0 / r - 1.0 / big_r)
                    + 2.0 * pi / 3.0 * rho * (big_r * big_r - r * r)),
        )
    }

    /// The radius between `below` and `above` at which its pressure is `pressure`, by
    /// [`RADIUS_HALVINGS`] halvings of the bracket, for a pressure at `below` above `pressure` and
    /// at `above` at most it: the bracket's upper end at the last, where the pressure is at most
    /// `pressure`.
    #[must_use]
    pub(crate) fn radius_at(&self, pressure: Pascals, below: Metres, above: Metres) -> Metres {
        let (mut lo, mut hi) = (below.value(), above.value());
        for _ in 0..RADIUS_HALVINGS {
            let mid = f64::midpoint(lo, hi);
            if self.pressure_at(Metres::new(mid)) > pressure {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Metres::new(hi)
    }
}

/// Where a body's ocean under a shell ends in its water column ([`ocean_end`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OceanEnd {
    /// On the rock at the column's base: the whole layer below the shell is liquid.
    Rock,
    /// On high-pressure ice at this radius, where the column's pressure reaches the liquidus.
    Ice(Metres),
    /// Not modelled: the shell's base itself lies at or beyond the liquidus, so any liquid lies
    /// under high-pressure ice (P14.T55.k's).
    Beyond,
}

/// Where the ocean under a shell whose base is at the radius `shell_base` ends in the column
/// `column`, its liquid freezing at and above the pressure `liquidus` (P14.T51.g.a): at the rock
/// where the pressure at the column's base is at most `liquidus`; else on the high-pressure ice at
/// the radius where it reaches `liquidus` ([`WaterColumn::radius_at`]), below the shell's base;
/// and [`OceanEnd::Beyond`] where the pressure at the shell's base is at or beyond `liquidus`, or
/// no water lies between it and that radius.
#[must_use]
pub(crate) fn ocean_end(column: &WaterColumn, shell_base: Metres, liquidus: Pascals) -> OceanEnd {
    // A shell's base at or past the liquidus, or at a pressure that is not a number, holds none
    // that is modelled.
    let at_shell_base = column.pressure_at(shell_base).value();
    if at_shell_base.is_nan() || at_shell_base >= liquidus.value() {
        return OceanEnd::Beyond;
    }
    if column.pressure_at(column.base()) <= liquidus {
        return OceanEnd::Rock;
    }
    let floor = column.radius_at(liquidus, column.base(), shell_base);
    if floor < shell_base {
        OceanEnd::Ice(floor)
    } else {
        OceanEnd::Beyond
    }
}

/// Water's melting pressure at `temperature` on its rising branch, from the registry's water row
/// (IAPWS R14-08's equations for ices III, V, VI and VII, `PhaseDiagram::melting_pressure`): at or
/// above it the liquid freezes. Ice V's 626.09 MPa at 273.0 K, the base of P14.T17.b's shell, and
/// 629.34 MPa at the triple point's 273.16 K (decision-p14-t51g Q6).
///
/// # Panics
///
/// For a temperature outside the curve's rising branch, from ice III's start at 251.165 K to ice
/// VII's end at 715 K, a broken invariant: the ocean's base is at 273 K, which the module's tests
/// read.
#[must_use]
pub(crate) fn base_liquidus(temperature: Kelvin) -> Pascals {
    Gas::Water
        .substance()
        .substance()
        .phase_diagram()
        .and_then(|water| water.melting_pressure(temperature))
        .expect(
            "water's row carries IAPWS R14-08's melting curve over the ocean's base temperature",
        )
}

/// The generator version from which every body with a solid surface publishes its subsurface
/// ocean by P14.T51.g.b's solve ([`Hydrosphere`]), and P14.T56.d's ocean branch reads the
/// spherical shell over it: 24, a rider of the 23 → 24 batch (P14.T54.b.j) that boards if its
/// commit lands before P14.T54.b.i's, and 25 otherwise (decision-p14-t51g).
///
/// Below it [`OceanRules::in_force`] is [`OceanRules::Held`], so that every published ocean and
/// every moon's tides are version 23's bit for bit, with P14.T51.g.a's floor where its own hold
/// is in force. P14.T54.b, or the next bump's wiring, deletes this hold with [`OceanRules`]'s
/// `Held` arms, the tests' switch and its tripwire (`the_ocean_is_held_only_until_its_bump`).
pub(crate) const OCEAN_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether every body's subsurface ocean is P14.T51.g.b's solve (the hold, [`OCEAN_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OceanRules {
    /// Version 23's: only a regular moon's ocean is computed, by its flag (with P14.T51.g.a's
    /// floor where [`OceanFloorRules`] is in force), and P14.T56.d's ocean branch reads the flag's
    /// planar shell.
    Held,
    /// P14.T51.g.b's: every body's ocean is the solve's, and the ocean branch reads its spherical
    /// shell, its solutes and its base temperature.
    Solved,
}

impl OceanRules {
    /// The rules this generator version gives: the solve from [`OCEAN_VERSION`], the flag below it;
    /// in the tests, the rules a test's switch sets on its thread ([`with_ocean_rules`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = OCEAN_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= OCEAN_VERSION {
            Self::Solved
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The ocean's rules a test sets on its thread, `None` for the version's.
    static OCEAN_RULES: core::cell::Cell<Option<OceanRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result with the ocean's rules `rules` on this thread, for the tests to reach the solve
/// ahead of its bump; the version's rules are set back after, a panic in `run` included. Its bump
/// deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_ocean_rules<R>(rules: OceanRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<OceanRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OCEAN_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(OCEAN_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The thermal conductivity of water ice times temperature: 651 W m⁻¹ (k = 651 ÷ T; Petrenko
/// and Whitworth 1999, as Hussmann et al. 2006, Icarus 185, 258 use it), so that a conductive ice
/// shell of thickness D between a surface at `T_s` and its melting point carries 651 ln(`T_m` ÷
/// `T_s`) ÷ D. P14.T17.b's, moved here by P14.T51.g.b, whose spherical shell reads it, and
/// re-exported from [`moons::regular`](crate::planetary::moons::regular).
///
/// Carnahan et al. 2021 find the law a lower bound near a cold surface, their own coefficient
/// 6% from it (science-p14-subsurface-ocean §2.2): taking theirs would thin Ganymede's shell from
/// 115 to 109 km, so the as-built value stays.
pub const ICE_CONDUCTIVITY_COEFFICIENT: f64 = 651.0;

/// The temperature below which ice's conductivity is held at its value there,
/// [`ICE_CONDUCTIVITY_COEFFICIENT`] ÷ 30 K: 30 K, the bottom of the C ÷ T law's range, below which
/// the plan does not extrapolate the fit. Carnahan et al. 2021 (EPSL 563, 116886; arXiv:2011.12502,
/// its supplement §1, read at build) find "the model k = 612/T fits the data well over
/// temperatures ranging from 30 to 273 K": the range is theirs, for their 612, while the as-built
/// 651 is Petrenko and Whitworth's, whose own range was not read (recorded). It reaches only
/// rogues, unlit bodies and the coldest TNOs.
pub(crate) const ICE_CONDUCTIVITY_FLOOR: Kelvin = Kelvin::new(30.0);

/// Comets' ammonia per unit mass of their water, the ice's ammonia a body takes where its nitrogen
/// took the icy boost: 0.0063, 67P's NH₃ at 0.67 ± 0.20 per 100 H₂O by number (Rubin et al. 2019,
/// MNRAS 489, 594, Table 2, read at build) times 17.031 ÷ 18.015; comets' "typical abundances ∼
/// 0.5 - 1%" (Biver et al., Comets III, arXiv:2207.04800, §4.3.2; Mumma and Charnley 2011, ARA&A
/// 49, 471, not read) (science-p14-subsurface-ocean §2.7).
pub(crate) const ACCRETED_AMMONIA: f64 = 0.0063;

/// The least mass fraction of the liquid at which an ocean's solute names its liquid: 10⁻³, the
/// fresh-water ceiling of 1,000 mg L⁻¹ (USGS; science-p14-subsurface-ocean §2.8); below it the
/// liquid is water.
pub(crate) const FRESH_WATER_SOLUTES: f64 = 1e-3;

/// The halvings of the shell's base's melting point T* by which [`Hydrosphere::base_under`] finds
/// it: 48, a fixed count, which close the bracket of ice Ih's 22 K curve to 8 × 10⁻¹⁴ K, f64's
/// resolution there, and the base's radius far inside the plan's 1 m.
pub(crate) const SHELL_BASE_HALVINGS: u32 = 48;

/// The least shell and ocean the solve publishes: 1 m. A thinner one is a surface's sea under a
/// skin of ice, or an ocean at its threshold, and publishes none (the wire refuses a depth of 0).
pub(crate) const LEAST_LAYER: Metres = Metres::new(1.0);

/// The multiple a body's nitrogen rank gives its nitrogen inventory, 10^(σ Φ⁻¹(rank)) with σ
/// [`VOLATILE_SCATTER_DEX`] (P14.T13.a, T50.a): the volatile inventory's own, which a test holds it
/// to bit for bit, and the factor its ice's ammonia takes ([`OceanSolutes::of_body`]).
#[must_use]
pub(crate) fn nitrogen_multiple(rank: UnitUniform) -> f64 {
    math::exp10(VOLATILE_SCATTER_DEX * math::normal_quantile(rank.value()))
}

/// What a body's ocean would dissolve (P14.T51.g.b's solutes; crate-private), built once from its
/// air's bulk and carried to its record: its chlorine as sodium chloride, and its ice's ammonia
/// as a share of its water.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct OceanSolutes {
    /// The sodium chloride its chlorine makes, all of it dissolved: the body's whole chlorine
    /// (P14.T50.a's) × M(NaCl) ÷ M(Cl), chondritic sodium exceeding chlorine.
    pub(crate) sodium_chloride: Kilograms,
    /// Its ice's ammonia per unit mass of its water: [`ACCRETED_AMMONIA`] × its nitrogen multiple
    /// where its nitrogen took the icy boost (formed beyond the snow line at or below ammonia
    /// hydrate's 123 K: P14.T50.c, T56.c), and 0 otherwise.
    pub(crate) ammonia_per_water: f64,
}

impl OceanSolutes {
    /// None: an ocean of fresh water.
    pub(crate) const FRESH: Self = Self {
        sodium_chloride: Kilograms::new(0.0),
        ammonia_per_water: 0.0,
    };

    /// The solutes of a body holding the chlorine `chlorine`, whose ice carries ammonia as
    /// `ammonia` says (carried only where its nitrogen took the icy boost), at its nitrogen rank
    /// `nitrogen_rank`.
    #[must_use]
    pub(crate) fn of_body(
        chlorine: Kilograms,
        ammonia: crate::planetary::hooks::crust::AmmoniaIce,
        nitrogen_rank: UnitUniform,
    ) -> Self {
        let cl = Element::Cl.standard_atomic_weight_g_per_mol();
        let salt = Element::Na.standard_atomic_weight_g_per_mol() + cl;
        let ammonia_per_water = match ammonia {
            crate::planetary::hooks::crust::AmmoniaIce::Carried => {
                ACCRETED_AMMONIA * nitrogen_multiple(nitrogen_rank)
            }
            crate::planetary::hooks::crust::AmmoniaIce::Absent => 0.0,
        };
        Self {
            sodium_chloride: Kilograms::new(chlorine.value() * salt / cl),
            ammonia_per_water,
        }
    }
}

/// What a body's subsurface ocean reads (P14.T51.g.b), as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OceanInputs {
    /// Its mean radius R.
    pub(crate) radius: Metres,
    /// Its mass M.
    pub(crate) mass: Kilograms,
    /// Its water's mass fraction, 0 to 1: its bulk's, or its condensed water's where larger.
    pub(crate) water_fraction: f64,
    /// The temperature at the top of its shell: its record's mean surface temperature, or a
    /// moon's equilibrium temperature on its tidal branch.
    pub(crate) surface_temperature: Kelvin,
    /// The heat flow crossing its shell's base, W m⁻²: its whole heat flow, all of it taken as
    /// crossing the base (heat dissipated in the shell would thicken it, recorded).
    pub(crate) heat_flow: WattsPerSquareMetre,
    /// What its ocean dissolves.
    pub(crate) solutes: OceanSolutes,
}

/// The subsurface ocean of the body `inputs` describe (P14.T51.g.b): its ocean, none, or not
/// modelled, by [`Hydrosphere::subsurface_under`]; none for a body with no water or a surface at
/// or above water's triple point.
#[must_use]
pub(crate) fn subsurface_ocean(inputs: &OceanInputs) -> Subsurface {
    Hydrosphere::of(inputs).map_or(Subsurface::NoOcean, |water| {
        water.subsurface_under(inputs.heat_flow)
    })
}

/// Whether the body `inputs` describe holds an ocean under its shell, the plan's closed form
/// ([`Hydrosphere::holds_an_ocean`], which the tidal branch reads): false with no water or a
/// surface at or above water's triple point.
#[cfg(test)]
#[must_use]
pub(crate) fn has_ocean(inputs: &OceanInputs) -> bool {
    Hydrosphere::of(inputs).is_some_and(|water| water.holds_an_ocean(inputs.heat_flow))
}

/// ∫ k dT from the shell's top at `surface_k` to `temperature_k`, K, in W m⁻¹, with k = C ÷ T held at
/// C ÷ 30 K below [`ICE_CONDUCTIVITY_FLOOR`]: C ln(T ÷ `T_s`) above it, C (T − `T_s`) ÷ 30 K below,
/// negative where `temperature` is under `surface`.
#[must_use]
pub(crate) fn conducted_w_per_m(surface_k: f64, temperature_k: f64) -> f64 {
    let (c, floor) = (ICE_CONDUCTIVITY_COEFFICIENT, ICE_CONDUCTIVITY_FLOOR.value());
    let (surface, temperature) = (surface_k, temperature_k);
    if surface >= floor && temperature >= floor {
        return c * math::ln(temperature / surface);
    }
    let antiderivative = |t: f64| {
        if t >= floor {
            c * (1.0 + math::ln(t / floor))
        } else {
            c * t / floor
        }
    };
    antiderivative(temperature) - antiderivative(surface)
}

/// The temperature, K, at which ∫ k dT from the shell's top at `surface_k` reaches
/// `integral_w_per_m`, not negative: [`conducted_w_per_m`]'s inverse, `T_s` exp(x ÷ C) for a top at or above
/// [`ICE_CONDUCTIVITY_FLOOR`], the shell's T(r) at x = R²F (1/r − 1/R).
#[must_use]
pub(crate) fn conducted_temperature_k(surface_k: f64, integral_w_per_m: f64) -> f64 {
    let (c, floor) = (ICE_CONDUCTIVITY_COEFFICIENT, ICE_CONDUCTIVITY_FLOOR.value());
    let (surface, integral) = (surface_k, integral_w_per_m);
    if surface >= floor {
        return surface * math::exp(integral / c);
    }
    let reached = c * surface / floor + integral;
    if reached <= c {
        floor * reached / c
    } else {
        floor * math::exp(reached / c - 1.0)
    }
}

/// The high-pressure ice a floor names, `None` for rock.
#[must_use]
const fn floor_ice(floor: OceanFloor) -> Option<HighPressureIce> {
    match floor {
        OceanFloor::Rock => None,
        OceanFloor::IceIii => Some(HighPressureIce::IceIii),
        OceanFloor::IceV => Some(HighPressureIce::IceV),
        OceanFloor::IceVi => Some(HighPressureIce::IceVi),
        OceanFloor::IceVii => Some(HighPressureIce::IceVii),
    }
}

/// Why an ocean under a found base is not modelled ([`absence`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Absence {
    /// Its base lies in ice II's field (Hammond et al. 2016's eq. 5).
    BaseInIceIi,
    /// Its floor lies in this ice's field, not the one it names.
    Floor(ForeignIce),
}

/// Why the ocean under the base `base` is not modelled, if it is not (decision-p14-t51gb-ice-ii
/// §4.1): its base lies in ice II's field (Hammond et al. 2016's eq. 5, the plan's rule), or its
/// floor, at its own pressure and the ocean's temperature, lies outside the field of the ice it
/// names (`foreign_ice`): in ice II's, which meets a cold solution's liquid whose liquidus is not
/// modelled, ice VI's, or for ice III ice V's. Nothing between them is tested: a column that only
/// crosses ice II's field, its floor beyond it, is right.
#[must_use]
pub(crate) fn absence(base: &ShellBase) -> Option<Absence> {
    let t_b = base.temperature;
    if foreign_ice(base.pressure, t_b, None).is_some() {
        return Some(Absence::BaseInIceIi);
    }
    foreign_ice(base.floor_pressure, t_b, floor_ice(base.floor)).map(Absence::Floor)
}

/// The floor a high-pressure ice makes.
#[must_use]
const fn floor_of(ice: HighPressureIce) -> OceanFloor {
    match ice {
        HighPressureIce::IceIii => OceanFloor::IceIii,
        HighPressureIce::IceV => OceanFloor::IceV,
        HighPressureIce::IceVi => OceanFloor::IceVi,
        HighPressureIce::IceVii => OceanFloor::IceVii,
    }
}

/// The base of a body's ice shell at one state of the solve ([`Hydrosphere::base_at`]): where it
/// lies, what its ocean rests on and holds, its temperature and the heat flow that puts it there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShellBase {
    /// T*, ice Ih's melting point at the base's pressure: the pure-water liquidus there.
    pub(crate) melting_point: Kelvin,
    /// The base's radius `r_b`.
    pub(crate) radius: Metres,
    /// The base's pressure `P_b`, ice Ih's melting pressure at T*.
    pub(crate) pressure: Pascals,
    /// The ocean's floor's radius `r_f`: the rock's or the high-pressure ice's, the shallower.
    pub(crate) floor_radius: Metres,
    /// What the ocean rests on.
    pub(crate) floor: OceanFloor,
    /// The column's pressure at the floor.
    pub(crate) floor_pressure: Pascals,
    /// Sodium chloride's mass fraction in its binary with the liquid, before its excess
    /// crystallises.
    pub(crate) sodium_chloride: f64,
    /// Ammonia's mass fraction in its binary with the liquid, before its excess crystallises.
    pub(crate) ammonia: f64,
    /// The base's temperature, T* less the solutes' depression: the ocean's, which is isothermal.
    pub(crate) temperature: Kelvin,
    /// The heat flow at the surface whose conduction puts the base here, F(T*), W m⁻²: negative
    /// where the base would be colder than the surface.
    pub(crate) heat_flow: WattsPerSquareMetre,
}

impl ShellBase {
    /// The ice shell over the base, R − `r_b`, for the body of radius `radius`.
    #[must_use]
    pub(crate) fn shell(&self, radius: Metres) -> Metres {
        Metres::new(radius.value() - self.radius.value())
    }

    /// The liquid's depth, from the base to the floor.
    #[must_use]
    pub(crate) fn thickness(&self) -> Metres {
        Metres::new(self.radius.value() - self.floor_radius.value())
    }
}

/// A body's shell could not be given a base under a heat flow ([`Hydrosphere::base_under`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum FindShellBaseError {
    /// No heat crosses it: a heat flow that is not positive.
    NoHeat,
    /// The surface lies at or above the liquidus of the whole column: the body's water is the
    /// surface's, a sea (P14.T51.a's), or its shell would be thinner than its top's triple
    /// pressure allows.
    SurfaceLiquid,
    /// The conduction keeps no liquid even at the deepest base the column allows.
    TooCold,
}

impl core::fmt::Display for FindShellBaseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NoHeat => "no heat crosses the shell: its heat flow is not positive",
            Self::SurfaceLiquid => "the surface lies at or above the liquidus of its whole water",
            Self::TooCold => "the shell keeps no liquid at the deepest base its water allows",
        })
    }
}

impl std::error::Error for FindShellBaseError {}

/// A body's water over its rock, under its surface, with its solutes: the solve of P14.T51.g.b's
/// ice shell and ocean (the [module](self) documentation; science-p14-subsurface-ocean §2).
///
/// Its states are read by T*, the shell's base's pure-water melting point
/// ([`base_at`](Self::base_at)), on [T*₀, 273.16 K]: T*₀ is ice Ih's melting point at the deepest
/// base the column allows, its rock's pressure or ice Ih's foot's 208.566 MPa, whichever is less.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hydrosphere {
    column: WaterColumn,
    /// The shell's top temperature, K.
    surface_k: f64,
    /// Its sodium chloride, kg.
    sodium_chloride_kg: f64,
    /// Its ammonia, kg.
    ammonia_kg: f64,
    liquidus: AqueousLiquidus,
    /// T*₀, K.
    deepest_k: f64,
    /// The deepest base's radius `r₀`, m.
    deepest_radius_m: f64,
    /// Whether the column reaches ice Ih's foot, where a test that fails leaves liquid only under
    /// high-pressure ice.
    reaches_foot: bool,
    /// Water's triple temperature, the top of T*'s range, K.
    top_k: f64,
}

impl Hydrosphere {
    /// The water of the body `inputs` describe, `None` where it has none (a water fraction that is
    /// not positive, a radius or mass that is not), or its surface is at or above water's triple
    /// point.
    #[must_use]
    pub(crate) fn of(inputs: &OceanInputs) -> Option<Self> {
        Self::new(
            inputs.radius,
            inputs.mass,
            inputs.water_fraction,
            inputs.surface_temperature,
            inputs.solutes,
        )
    }

    /// The water of a body of radius `radius`, mass `mass` and water fraction `water`, under a
    /// surface at `surface`, dissolving `solutes`: `None` as [`of`](Self::of).
    #[must_use]
    pub(crate) fn new(
        radius: Metres,
        mass: Kilograms,
        water: f64,
        surface: Kelvin,
        solutes: OceanSolutes,
    ) -> Option<Self> {
        let top = water_triple_temperature().value();
        let (r, m, t_s) = (radius.value(), mass.value(), surface.value());
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !(positive(r) && positive(m) && positive(water) && t_s.is_finite()) || t_s >= top {
            return None;
        }
        let column = WaterColumn::of_water(radius, mass, water);
        let base = column.base();
        let (foot, foot_pressure) = ice_ih_foot();
        let at_rock = column.pressure_at(base);
        let (deepest, deepest_radius, reaches_foot) = if at_rock >= foot_pressure {
            let r0 = column.radius_at(foot_pressure, base, radius);
            (foot.value(), r0.value(), true)
        } else {
            (
                ice_ih_melting_temperature(at_rock).value(),
                base.value(),
                false,
            )
        };
        let held = |x: f64| if x > 0.0 { x } else { 0.0 };
        Some(Self {
            column,
            surface_k: t_s,
            sodium_chloride_kg: held(solutes.sodium_chloride.value()),
            ammonia_kg: held(solutes.ammonia_per_water * water * m),
            liquidus: AqueousLiquidus::of_registry(),
            deepest_k: deepest,
            deepest_radius_m: deepest_radius,
            reaches_foot,
            top_k: top,
        })
    }

    /// Its water column.
    #[must_use]
    pub(crate) const fn column(&self) -> &WaterColumn {
        &self.column
    }

    /// T*₀, the pure-water melting point of the deepest base the column allows.
    #[must_use]
    pub(crate) const fn deepest(&self) -> Kelvin {
        Kelvin::new(self.deepest_k)
    }

    /// The top of T*'s range: water's triple temperature, 273.16 K.
    #[must_use]
    pub(crate) const fn top(&self) -> Kelvin {
        Kelvin::new(self.top_k)
    }

    /// The shell's base whose pure-water melting point is `melting_point_k`, K, on [T*₀, 273.16 K]:
    ///
    /// - its pressure is ice Ih's melting pressure there, and its radius the column's there;
    /// - the floor is the rock where the rock's pressure is under the high-pressure ice's melting
    ///   pressure at T*, else that ice at the radius where the column reaches it;
    /// - each solute's mass fraction is its mass over its own and the liquid's between the base
    ///   and the floor, at [`WATER_DENSITY`]: its fraction in its binary with water, which its
    ///   liquidus reads, so that at no liquid every solute present is eutectic;
    /// - the base's temperature is T* less their depression, and the heat flow that puts it there
    ///   is F(T*) = ∫k dT × `r_b` ÷ (R (R − `r_b`)), from ∫k dT = R²F (1/`r_b` − 1/R).
    ///
    /// # Panics
    ///
    /// For a melting point outside [251.165 K, 273.16 K], a broken invariant: the solve reads it
    /// only on [T*₀, 273.16 K].
    #[must_use]
    pub(crate) fn base_at(&self, melting_point_k: f64) -> ShellBase {
        let t_star = Kelvin::new(melting_point_k);
        let (big_r, base) = (self.column.radius, self.column.base());
        let pressure =
            ice_ih_melting_pressure(t_star).expect("the solve reads T* on ice Ih's melting curve");
        // The deepest base is r₀ itself, as the presence test reads it, so that its liquid is none
        // and its solutes eutectic, and the two agree.
        let radius = if melting_point_k <= self.deepest_k {
            Metres::new(self.deepest_radius_m)
        } else {
            self.column.radius_at(pressure, base, big_r)
        };
        let (ice_pressure, ice) = high_pressure_melting(t_star)
            .expect("ice III's or ice V's melting curve holds every T* on ice Ih's");
        let at_rock = self.column.pressure_at(base);
        let (floor_radius, floor) = if at_rock < ice_pressure {
            (base, OceanFloor::Rock)
        } else {
            (
                self.column.radius_at(ice_pressure, base, radius),
                floor_of(ice),
            )
        };
        let (r_b, r_f) = (radius.value(), floor_radius.value());
        let liquid = 4.0 / 3.0
            * core::f64::consts::PI
            * WATER_DENSITY.value()
            * (r_b * r_b * r_b - r_f * r_f * r_f);
        let liquid = if liquid > 0.0 { liquid } else { 0.0 };
        // Each solute's mass fraction in its own binary with the liquid, as its liquidus reads it:
        // at no liquid every solute present is eutectic, as the presence test takes it.
        let share = |mass: f64| {
            if mass > 0.0 {
                mass / (mass + liquid)
            } else {
                0.0
            }
        };
        let (sodium_chloride, ammonia) = (share(self.sodium_chloride_kg), share(self.ammonia_kg));
        let temperature = melting_point_k - self.liquidus.depression_k(sodium_chloride, ammonia);
        let r = big_r.value();
        let heat_flow = conducted_w_per_m(self.surface_k, temperature) * r_b / (r * (r - r_b));
        ShellBase {
            melting_point: t_star,
            radius,
            pressure,
            floor_radius,
            floor,
            floor_pressure: self.column.pressure_at(floor_radius),
            sodium_chloride,
            ammonia,
            temperature: Kelvin::new(temperature),
            heat_flow: WattsPerSquareMetre::new(heat_flow),
        }
    }

    /// The shell's base under the heat flow `heat_flow` (the plan's solve): the root of F(T*) =
    /// F on [T*₀, 273.16 K] by [`SHELL_BASE_HALVINGS`] halvings, F(T*) rising strictly with T*
    /// wherever the base is warmer than the surface, and not positive elsewhere.
    ///
    /// # Errors
    ///
    /// [`FindShellBaseError`]: a heat flow that is not positive; a surface at or above the liquidus of the
    /// whole column, or a heat flow no shell below its top's triple pressure conducts; and a
    /// conduction that keeps no liquid at T*₀.
    pub(crate) fn base_under(
        &self,
        heat_flow: WattsPerSquareMetre,
    ) -> Result<ShellBase, FindShellBaseError> {
        let f = heat_flow.value();
        if f.is_nan() || f <= 0.0 {
            return Err(FindShellBaseError::NoHeat);
        }
        let top = self.base_at(self.top_k);
        if top.temperature.value() <= self.surface_k || top.heat_flow.value() <= f {
            return Err(FindShellBaseError::SurfaceLiquid);
        }
        if self.base_at(self.deepest_k).heat_flow.value() > f {
            return Err(FindShellBaseError::TooCold);
        }
        let (mut lo, mut hi) = (self.deepest_k, self.top_k);
        for _ in 0..SHELL_BASE_HALVINGS {
            let mid = f64::midpoint(lo, hi);
            if self.base_at(mid).heat_flow.value() > f {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Ok(self.base_at(f64::midpoint(lo, hi)))
    }

    /// Whether it holds an ocean under the heat flow `heat_flow`, the plan's closed form: T(r₀) +
    /// `ΔT_e` ≥ `T_m`(P(r₀)), with `ΔT_e` the largest eutectic depression present (0 with none),
    /// for T(r₀) + `ΔT_e` ≥ 273.16 K, or at least the foot's 251.165 K with ice Ih's melting pressure
    /// there at most P(r₀); false where [`base_under`](Self::base_under) finds no heat or a liquid
    /// surface. It agrees with the solve's root (a test holds the two over 10⁴ bodies).
    #[must_use]
    pub(crate) fn holds_an_ocean(&self, heat_flow: WattsPerSquareMetre) -> bool {
        let f = heat_flow.value();
        if f.is_nan() || f <= 0.0 {
            return false;
        }
        let top = self.base_at(self.top_k);
        if top.temperature.value() <= self.surface_k || top.heat_flow.value() <= f {
            return false;
        }
        let (r, r0) = (self.column.radius.value(), self.deepest_radius_m);
        let integral = f * r * (r - r0) / r0;
        let eutectic = self
            .liquidus
            .eutectic_depression_k(self.sodium_chloride_kg, self.ammonia_kg);
        let warmest = conducted_temperature_k(self.surface_k, integral) + eutectic;
        let (foot, _) = ice_ih_foot();
        if warmest >= self.top_k {
            return true;
        }
        if warmest < foot.value() {
            return false;
        }
        let at_r0 = self.column.pressure_at(Metres::new(r0));
        ice_ih_melting_pressure(Kelvin::new(warmest)).is_some_and(|p| p <= at_r0)
    }

    /// Its subsurface ocean under the heat flow `heat_flow`: the ocean at the base the solve finds
    /// ([`publish`](Self::publish)); none without heat, under a liquid surface, or where no liquid
    /// is kept and the column ends above ice Ih's foot; not modelled where none is kept and the
    /// column reaches the foot, so that any liquid lies under high-pressure ice (P14.T55.k's).
    #[must_use]
    pub(crate) fn subsurface_under(&self, heat_flow: WattsPerSquareMetre) -> Subsurface {
        match self.base_under(heat_flow) {
            Ok(base) => self.publish(&base),
            Err(FindShellBaseError::TooCold) if self.reaches_foot => Subsurface::NotComputed,
            Err(
                FindShellBaseError::NoHeat
                | FindShellBaseError::SurfaceLiquid
                | FindShellBaseError::TooCold,
            ) => Subsurface::NoOcean,
        }
    }

    /// The ocean under the base `base`: not modelled where its base or its floor lies outside its
    /// ice's field ([`absence`]); none where its
    /// shell or its ocean is thinner than [`LEAST_LAYER`]; otherwise its ocean, its liquid the row
    /// of the solute whose own depression is larger (sodium chloride's on a tie) where that
    /// solute's held fraction is at least [`FRESH_WATER_SOLUTES`], and water otherwise. Its solute
    /// fraction is the solutes' mass over the liquid's, each held at its eutectic, Σr ÷ (1 + Σr)
    /// with r = w ÷ (1 − w) each solute's mass per unit of water: at most 0.437, both eutectic.
    #[must_use]
    pub(crate) fn publish(&self, base: &ShellBase) -> Subsurface {
        if absence(base).is_some() {
            return Subsurface::NotComputed;
        }
        let (shell, thickness) = (base.shell(self.column.radius), base.thickness());
        if shell < LEAST_LAYER || thickness < LEAST_LAYER {
            return Subsurface::NoOcean;
        }
        let liquidus = &self.liquidus;
        let held = |w: f64, eutectic: f64| if w < eutectic { w } else { eutectic };
        let sodium_chloride = held(
            base.sodium_chloride,
            liquidus.sodium_chloride_eutectic().solute_mass_fraction,
        );
        let ammonia = held(
            base.ammonia,
            liquidus.ammonia_eutectic().solute_mass_fraction,
        );
        let liquid = if liquidus.sodium_chloride_depression_k(base.sodium_chloride)
            >= liquidus.ammonia_depression_k(base.ammonia)
        {
            (sodium_chloride >= FRESH_WATER_SOLUTES).then(|| liquidus.sodium_chloride_row())
        } else {
            (ammonia >= FRESH_WATER_SOLUTES).then(|| liquidus.ammonia_row())
        };
        Subsurface::Ocean(SubsurfaceOcean::solved(
            shell,
            thickness,
            base.floor,
            liquid.unwrap_or(Gas::Water.substance()),
            solute_mass_fraction(sodium_chloride, ammonia),
            base.temperature,
        ))
    }
}

/// The solutes' mass fraction of a liquid holding sodium chloride and ammonia at their binary
/// fractions `sodium_chloride` and `ammonia` (each its mass over its own and the water's), in
/// [0, 1): Σr ÷ (1 + Σr), r = w ÷ (1 − w) each solute's mass per unit mass of water.
#[must_use]
fn solute_mass_fraction(sodium_chloride: f64, ammonia: f64) -> f64 {
    let per_water = |w: f64| w / (1.0 - w);
    let solutes = per_water(sodium_chloride) + per_water(ammonia);
    solutes / (1.0 + solutes)
}

/// What is implausible about the published ocean `ocean` of the body `inputs` describe, by the
/// plan's invariants for every published ocean (P14.T51.g.b's tests), `None` where nothing is: a
/// shell and an ocean of at least [`LEAST_LAYER`]; both within R − `r_w` (to 1 m); the ocean's
/// mass at [`WATER_DENSITY`] at most x M; its base at most ice Ih's foot's 208.566 MPa and outside
/// ice II's field, and its floor, at its own pressure and the ocean's temperature, inside the
/// field of its own ice (decision-p14-t51gb-ice-ii); an ice floor at its ice's melting pressure at
/// T* to 10⁻⁶ and of that ice, a
/// rock floor below that pressure, every floor at or under ice V's 629.34 MPa at 273.16 K; the
/// base's temperature between T* less the largest eutectic depression present (the eutectic at
/// its pressure) and 273.16 K, and above the surface's; the solutes' fraction in [0, 0.437], both
/// eutectic (the plan's 0.554 sums the two binaries' fractions); and
/// its liquid one of the three rows the solve names.
#[cfg(test)]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one list of the plan's invariants, each checked and named in turn"
)]
pub(crate) fn implausibility(inputs: &OceanInputs, ocean: &SubsurfaceOcean) -> Option<String> {
    let Some(water) = Hydrosphere::of(inputs) else {
        return Some(format!("an ocean with no water under it: {inputs:?}"));
    };
    let column = water.column;
    let (big_r, base) = (column.radius.value(), column.base().value());
    let (shell, thickness) = (ocean.shell_thickness().value(), ocean.thickness().value());
    let (r_b, r_f) = (big_r - shell, big_r - shell - thickness);
    let mut faults = Vec::new();
    if shell < LEAST_LAYER.value() || thickness < LEAST_LAYER.value() {
        faults.push(format!(
            "a layer under 1 m: shell {shell} m, ocean {thickness} m"
        ));
    }
    if shell + thickness > big_r - base + 1.0 {
        faults.push(format!(
            "deeper than its water: {} m against {} m",
            shell + thickness,
            big_r - base
        ));
    }
    let mass = 4.0 / 3.0
        * core::f64::consts::PI
        * WATER_DENSITY.value()
        * (r_b * r_b * r_b - r_f * r_f * r_f);
    let water_mass = inputs.water_fraction * inputs.mass.value();
    if mass > water_mass * (1.0 + 1e-9) {
        faults.push(format!("{mass} kg of ocean from {water_mass} kg of water"));
    }
    let pressure = column.pressure_at(Metres::new(r_b));
    let (_, foot) = ice_ih_foot();
    let t_b = ocean.base_temperature().value();
    if pressure.value() > foot.value() * (1.0 + 1e-9) {
        faults.push(format!("its base at {pressure:?}, past ice Ih's foot"));
    }
    let t_star = ice_ih_melting_temperature(pressure);
    let floor_pressure = column.pressure_at(Metres::new(r_f)).value();
    let temperature = ocean.base_temperature();
    if let Some(ice) = foreign_ice(pressure, temperature, None) {
        faults.push(format!(
            "its base in {ice:?}'s field: {pressure:?} at {t_b} K"
        ));
    }
    if let Some(ice) = foreign_ice(
        Pascals::new(floor_pressure),
        temperature,
        floor_ice(ocean.floor()),
    ) {
        faults.push(format!(
            "its floor in {ice:?}'s field: {floor_pressure} Pa at {t_b} K"
        ));
    }
    match high_pressure_melting(t_star) {
        None => faults.push(format!("no high-pressure ice at T* {t_star:?}")),
        Some((melting, ice)) => match ocean.floor() {
            OceanFloor::Rock => {
                if column.pressure_at(column.base()).value() >= melting.value() * (1.0 + 1e-6) {
                    faults.push(format!("a rock floor past {melting:?}"));
                }
            }
            OceanFloor::IceIii | OceanFloor::IceV | OceanFloor::IceVi | OceanFloor::IceVii => {
                if (floor_pressure / melting.value() - 1.0).abs() > 1e-6 {
                    faults.push(format!(
                        "an ice floor at {floor_pressure} Pa, not {melting:?}"
                    ));
                }
                if ocean.floor() != floor_of(ice) {
                    faults.push(format!("a floor of {:?}, not {ice:?}", ocean.floor()));
                }
            }
        },
    }
    if let Some((bound, _)) = high_pressure_melting(water.top())
        && floor_pressure > bound.value() * (1.0 + 1e-9)
    {
        faults.push(format!("a floor at {floor_pressure} Pa, past {bound:?}"));
    }
    let eutectic = water
        .liquidus
        .eutectic_depression_k(water.sodium_chloride_kg, water.ammonia_kg);
    if t_b < t_star.value() - eutectic - 1e-6 || t_b > water.top_k || t_b < water.surface_k {
        faults.push(format!(
            "a base at {t_b} K: T* {t_star:?}, eutectic depression {eutectic} K, surface {} K",
            water.surface_k
        ));
    }
    let solutes = ocean.solute_mass_fraction();
    let most = solute_mass_fraction(
        water
            .liquidus
            .sodium_chloride_eutectic()
            .solute_mass_fraction,
        water.liquidus.ammonia_eutectic().solute_mass_fraction,
    );
    if !(0.0..=most).contains(&solutes) {
        faults.push(format!("a solute fraction of {solutes}"));
    }
    let liquids = [
        Gas::Water.substance(),
        water.liquidus.sodium_chloride_row(),
        water.liquidus.ammonia_row(),
    ];
    if !liquids.contains(&ocean.liquid()) {
        faults.push(format!("a liquid of {:?}", ocean.liquid()));
    }
    (!faults.is_empty()).then(|| faults.join("; "))
}
