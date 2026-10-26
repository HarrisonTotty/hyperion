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

use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::moons::regular::WATER_DENSITY;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kelvin, Kilograms, KilogramsPerCubicMetre, Metres, Pascals};
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

    /// The body's mean radius, R.
    #[cfg(test)]
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
