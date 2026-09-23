//! The composition solve: what a body of a given mass and radius is made of (plan 14, P14.T11.c;
//! design note 8).
//!
//! A body's radius comes from Chen and Kipping's relation at its drawn quantile
//! ([`radius_chen_kipping`](super::radius_chen_kipping)); [`composition`] turns that (mass,
//! radius) point into iron, rock, water and a hydrogen and helium envelope, using Zeng et al.'s
//! curves ([`radius_zeng`]) and the envelope model ([`radius_with_envelope`]), constrained by
//! which side of the snow line the body formed on. Radius and composition are never drawn apart:
//! the radius a body keeps is always the radius of the composition solved for it.
//!
//! # The rules
//!
//! Along the dry curves the radius falls as iron rises, from pure rock through Earth-like to pure
//! iron. Every body is placed as follows.
//!
//! - **Below the iron curve** the radius is raised to it: nothing is denser than iron.
//! - **Between iron and Earth-like** the body is dry rock and iron, the core mass fraction solved
//!   between the curves, on either side of the snow line.
//! - **Inside the snow line, above Earth-like**: up to the rock curve the body is dry and poor in
//!   iron. Above it a body heavier than [`ENVELOPE_CORE_FLOOR`] takes an envelope on an Earth-like
//!   core, the core of Lopez and Fortney's models, and a lighter one is clamped to rock with water
//!   up to [`INNER_WATER_CAP`]; so nothing formed inside the snow line is a water world.
//! - **Beyond the snow line, above Earth-like**: the body is water on an Earth-like core, as Zeng
//!   et al.'s (2019) icy cores are, up to [`OUTER_WATER_CAP`], the ice share of the solids it formed
//!   from. Above that a body heavier than the floor takes an envelope on that icy core, and a
//!   lighter one is clamped to it.
//! - **An envelope** never leaves less core than the floor. A radius above the largest such
//!   envelope's is lowered to it: the models have no planet that large, as with the iron curve
//!   below.
//!
//! The envelope is solved at [`COMPOSITION_REFERENCE_AGE`] and the flux the caller passes. Its
//! radius at other ages, and its loss to escape (P14.T13), follow from the solved fraction.
//!
//! Above 0.414 Jupiter masses, Chen and Kipping's Jovian class, radius and composition come from
//! plan 13's cooling of giant planets (P14.T11.d), which is not built; [`composition`] returns
//! [`SolveCompositionError::GiantPlanet`] there, never a fallback.

use std::error::Error;
use std::fmt;

use crate::planetary::derive::envelope::radius_with_envelope;
use crate::planetary::derive::radius::{
    CoreComposition, DRY_CURVES, EARTH_CORE_MASS_FRACTION, NEPTUNIAN_JOVIAN_TRANSITION,
    TablePosition, dry_radius, radius_zeng, water_fraction_of_blend,
};
use crate::planetary::params::{
    COMPOSITION_REFERENCE_AGE, ENVELOPE_CORE_FLOOR, INNER_WATER_CAP, OUTER_WATER_CAP,
};
use crate::units::{EarthFluxes, EarthMasses, EarthRadii};

/// The core of an icy body with the most water it can hold: [`OUTER_WATER_CAP`] of water on an
/// Earth-like core.
const ICY_CORE: CoreComposition =
    CoreComposition::from_fractions(EARTH_CORE_MASS_FRACTION, OUTER_WATER_CAP);

/// Which side of its host's snow line a body formed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SnowLineSide {
    /// Inside the snow line, where only rock and iron condense.
    Inside,
    /// Beyond the snow line, where water ice condenses too.
    Beyond,
}

/// A body's bulk composition as mass fractions of iron, rock, water and hydrogen and helium
/// envelope, summing to 1.
///
/// Plan 14's `hooks::BulkComposition` (P14.T23) holds these with the atmosphere's inventory (T13)
/// and the host's abundances.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MassFractions {
    iron: f64,
    rock: f64,
    water: f64,
    envelope: f64,
}

impl MassFractions {
    /// The fractions of a body whose envelope is `envelope` of its mass on a core of `core`.
    #[must_use]
    fn of(core: CoreComposition, envelope: f64) -> Self {
        let solid = 1.0 - envelope;
        Self {
            iron: core.iron_fraction() * solid,
            rock: core.rock_fraction() * solid,
            water: core.water_fraction() * solid,
            envelope,
        }
    }

    /// The mass fraction of iron.
    #[must_use]
    pub const fn iron(&self) -> f64 {
        self.iron
    }

    /// The mass fraction of rock.
    #[must_use]
    pub const fn rock(&self) -> f64 {
        self.rock
    }

    /// The mass fraction of water.
    #[must_use]
    pub const fn water(&self) -> f64 {
        self.water
    }

    /// The mass fraction of hydrogen and helium envelope.
    #[must_use]
    pub const fn envelope(&self) -> f64 {
        self.envelope
    }
}

/// How the solve changed the radius it was given, if it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RadiusAdjustment {
    /// The radius is the one given.
    Unchanged,
    /// The radius lay below the iron curve and was raised to it.
    RaisedToIron,
    /// A body formed inside the snow line and too light for an envelope lay above the rock curve,
    /// and was clamped to rock with [`INNER_WATER_CAP`] of water.
    ClampedToRock,
    /// A body formed beyond the snow line and too light for an envelope lay above the curve of
    /// [`OUTER_WATER_CAP`] of water, and was clamped to it.
    ClampedToIce,
    /// The radius lay above that of the largest envelope the body's mass allows, and was lowered
    /// to it.
    ClampedToEnvelopeLimit,
}

/// What [`composition`] found: the body's mass fractions, the composition of its core, its
/// envelope fraction and the radius it keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolvedComposition {
    fractions: MassFractions,
    core: CoreComposition,
    radius: EarthRadii,
    adjustment: RadiusAdjustment,
}

impl SolvedComposition {
    /// The body's mass fractions of iron, rock, water and envelope.
    #[must_use]
    pub const fn fractions(&self) -> MassFractions {
        self.fractions
    }

    /// The composition of everything but the envelope, whose [`radius_zeng`] a body stripped of
    /// its envelope takes (design note 8).
    #[must_use]
    pub const fn core(&self) -> CoreComposition {
        self.core
    }

    /// The envelope's mass fraction, the body's initial one (P14.T13.a).
    #[must_use]
    pub const fn envelope_fraction(&self) -> f64 {
        self.fractions.envelope
    }

    /// The radius the body keeps at [`COMPOSITION_REFERENCE_AGE`].
    #[must_use]
    pub const fn radius(&self) -> EarthRadii {
        self.radius
    }

    /// Whether, and how, the radius differs from the one given.
    #[must_use]
    pub const fn adjustment(&self) -> RadiusAdjustment {
        self.adjustment
    }
}

/// [`composition`] could not solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolveCompositionError {
    /// The mass was not positive and finite.
    MassNotPositive,
    /// The radius was not positive and finite.
    RadiusNotPositive,
    /// The flux was negative or not finite.
    FluxNotValid,
    /// The body is a giant, of 0.414 Jupiter masses or more, whose radius and composition are
    /// P14.T11.d's, from plan 13's cooling of giant planets.
    GiantPlanet {
        /// The body's mass.
        mass: EarthMasses,
    },
}

impl fmt::Display for SolveCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassNotPositive => f.write_str("a body's mass must be positive and finite"),
            Self::RadiusNotPositive => f.write_str("a body's radius must be positive and finite"),
            Self::FluxNotValid => f.write_str("a body's flux must be finite and not negative"),
            Self::GiantPlanet { mass } => write!(
                f,
                "a body of {} earth masses is a giant, whose composition is not solved here",
                mass.value()
            ),
        }
    }
}

impl Error for SolveCompositionError {}

/// The bulk composition of a body of mass `mass` and radius `radius`, formed on `side` of its snow
/// line and receiving `flux`, by the rules of the [module](self) documentation (design note 8).
///
/// `flux` matters only to an envelope, whose radius at [`COMPOSITION_REFERENCE_AGE`] it sets.
///
/// # Errors
///
/// [`SolveCompositionError::GiantPlanet`] from 0.414 Jupiter masses (P14.T11.d's range), and the
/// other variants for a mass or radius that is not positive and finite or a flux that is negative
/// or not finite.
///
/// # Examples
///
/// Earth's radius at Earth's mass is Earth's composition, and the same mass with a quarter more
/// radius is a water world if it formed beyond the snow line, and clamped to rock if inside:
///
/// ```
/// use hyperion_sim::planetary::derive::composition::{RadiusAdjustment, SnowLineSide, composition};
/// use hyperion_sim::units::{EarthFluxes, EarthMasses, EarthRadii};
///
/// let (mass, flux) = (EarthMasses::new(1.0), EarthFluxes::new(1.0));
/// let earth = composition(mass, EarthRadii::new(0.9995), SnowLineSide::Inside, flux)?;
/// assert!((earth.fractions().iron() - 0.325).abs() < 0.01);
/// let puffy = EarthRadii::new(1.25);
/// let icy = composition(mass, puffy, SnowLineSide::Beyond, flux)?;
/// assert!(icy.fractions().water() > 0.5);
/// let rocky = composition(mass, puffy, SnowLineSide::Inside, flux)?;
/// assert!(rocky.fractions().water() <= 0.001);
/// assert_eq!(rocky.adjustment(), RadiusAdjustment::ClampedToRock);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
pub fn composition(
    mass: EarthMasses,
    radius: EarthRadii,
    side: SnowLineSide,
    flux: EarthFluxes,
) -> Result<SolvedComposition, SolveCompositionError> {
    let (m, r) = (mass.value(), radius.value());
    if !(m.is_finite() && m > 0.0) {
        return Err(SolveCompositionError::MassNotPositive);
    }
    if !(r.is_finite() && r > 0.0) {
        return Err(SolveCompositionError::RadiusNotPositive);
    }
    if !(flux.value().is_finite() && flux.value() >= 0.0) {
        return Err(SolveCompositionError::FluxNotValid);
    }
    if m >= NEPTUNIAN_JOVIAN_TRANSITION.value() {
        return Err(SolveCompositionError::GiantPlanet { mass });
    }
    let dry = TablePosition::of(mass).dry_radii();
    let (rock, iron) = (dry[0], dry[DRY_CURVES.len() - 1]);
    if r < iron {
        return Ok(dry_body(1.0, iron, RadiusAdjustment::RaisedToIron));
    }
    let earth = dry_radius(&dry, EARTH_CORE_MASS_FRACTION);
    if r <= earth || (side == SnowLineSide::Inside && r <= rock) {
        return Ok(dry_body(
            dry_core_mass_fraction(&dry, r),
            r,
            RadiusAdjustment::Unchanged,
        ));
    }
    Ok(match side {
        SnowLineSide::Inside if m > ENVELOPE_CORE_FLOOR.value() => {
            enveloped(mass, radius, CoreComposition::EARTH_LIKE, flux)
        }
        SnowLineSide::Inside => watery(
            mass,
            r,
            0.0,
            INNER_WATER_CAP,
            RadiusAdjustment::ClampedToRock,
        ),
        SnowLineSide::Beyond => {
            if r <= radius_zeng(mass, ICY_CORE).value() || m <= ENVELOPE_CORE_FLOOR.value() {
                watery(
                    mass,
                    r,
                    EARTH_CORE_MASS_FRACTION,
                    OUTER_WATER_CAP,
                    RadiusAdjustment::ClampedToIce,
                )
            } else {
                enveloped(mass, radius, ICY_CORE, flux)
            }
        }
    })
}

/// A dry body of core mass fraction `cmf` and radius `radius` (R⊕).
#[must_use]
fn dry_body(cmf: f64, radius: f64, adjustment: RadiusAdjustment) -> SolvedComposition {
    let core = CoreComposition::from_fractions(cmf, 0.0);
    SolvedComposition {
        fractions: MassFractions::of(core, 0.0),
        core,
        radius: EarthRadii::new(radius),
        adjustment,
    }
}

/// The core mass fraction at which the dry curves, of radii `dry` at the body's mass, pass through
/// `radius`: the inverse of [`dry_radius`], linear between the two curves that bracket it.
#[must_use]
fn dry_core_mass_fraction(dry: &[f64; 6], radius: f64) -> f64 {
    let mut upper = 1;
    while upper < DRY_CURVES.len() - 1 && radius < dry[upper] {
        upper += 1;
    }
    let (low, high) = (DRY_CURVES[upper - 1].0, DRY_CURVES[upper].0);
    let share = (dry[upper - 1] - radius) / (dry[upper - 1] - dry[upper]);
    (low + share * (high - low)).clamp(low, high)
}

/// Water on a core of core mass fraction `cmf`, as much as puts a body of mass `mass` at radius
/// `radius` (R⊕), up to `cap`; at the cap the radius is clamped, with `clamp` saying so.
#[must_use]
fn watery(
    mass: EarthMasses,
    radius: f64,
    cmf: f64,
    cap: f64,
    clamp: RadiusAdjustment,
) -> SolvedComposition {
    let position = TablePosition::of(mass);
    let dry = dry_radius(&position.dry_radii(), cmf);
    let blend = (radius - dry) / (position.water_radius() - dry);
    let solved = water_fraction_of_blend(blend.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let (core, radius, adjustment) = if solved > cap {
        let core = CoreComposition::from_fractions(cmf, cap);
        (core, radius_zeng(mass, core), clamp)
    } else {
        let core = CoreComposition::from_fractions(cmf, solved);
        (core, EarthRadii::new(radius), RadiusAdjustment::Unchanged)
    };
    SolvedComposition {
        fractions: MassFractions::of(core, 0.0),
        core,
        radius,
        adjustment,
    }
}

/// Bisection steps in the envelope solve: enough to close the interval to adjacent floats.
const ENVELOPE_BISECTIONS: u32 = 64;

/// An envelope on a core of `core`, as much as puts a body of mass `mass` at radius `radius` at
/// [`COMPOSITION_REFERENCE_AGE`] and `flux`, leaving at least [`ENVELOPE_CORE_FLOOR`] of core.
///
/// The radius rises with the envelope fraction at that age ([`radius_with_envelope`]), so the
/// fraction is found by bisection, in a fixed number of steps; the radius kept is the model's at
/// the fraction found, which differs from `radius` only in rounding unless it was clamped.
#[must_use]
fn enveloped(
    mass: EarthMasses,
    radius: EarthRadii,
    core: CoreComposition,
    flux: EarthFluxes,
) -> SolvedComposition {
    let model = |f: f64| radius_with_envelope(mass, core, f, flux, COMPOSITION_REFERENCE_AGE);
    let most = 1.0 - ENVELOPE_CORE_FLOOR.value() / mass.value();
    let largest = model(most);
    let (envelope, radius, adjustment) = if largest.value() < radius.value() {
        (most, largest, RadiusAdjustment::ClampedToEnvelopeLimit)
    } else {
        let (mut low, mut high) = (0.0, most);
        for _ in 0..ENVELOPE_BISECTIONS {
            let middle = f64::midpoint(low, high);
            if model(middle).value() < radius.value() {
                low = middle;
            } else {
                high = middle;
            }
        }
        (high, model(high), RadiusAdjustment::Unchanged)
    };
    SolvedComposition {
        fractions: MassFractions::of(core, envelope),
        core,
        radius,
        adjustment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::derive::radius::radius_chen_kipping;
    use crate::stellar::draws::UnitUniform;
    use crate::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};

    const ONE: EarthFluxes = EarthFluxes::new(1.0);

    fn solve(m: f64, r: f64, side: SnowLineSide, s: f64) -> SolvedComposition {
        composition(
            EarthMasses::new(m),
            EarthRadii::new(r),
            side,
            EarthFluxes::new(s),
        )
        .unwrap()
    }

    fn sum(f: MassFractions) -> f64 {
        f.iron() + f.rock() + f.water() + f.envelope()
    }

    /// Masses spanning the solve's range, M⊕.
    const MASSES: [f64; 10] = [0.01, 0.1, 0.5, 1.0, 1.4, 1.6, 3.0, 10.0, 40.0, 131.0];

    #[test]
    fn fractions_sum_to_one_everywhere() {
        for m in MASSES {
            for i in 1..50_u32 {
                let q = UnitUniform::new(f64::from(i) / 50.0).unwrap();
                let r = radius_chen_kipping(EarthMasses::new(m), q).value();
                for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                    for s in [0.01, 1.0, 300.0] {
                        let solved = solve(m, r, side, s);
                        let f = solved.fractions();
                        assert!((sum(f) - 1.0).abs() < 1e-14, "{m} M⊕, {r} R⊕: {f:?}");
                        for x in [f.iron(), f.rock(), f.water(), f.envelope()] {
                            assert!((0.0..=1.0).contains(&x), "{f:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_solve_inverts_zeng_s_curves() {
        let dry = [1.0, 0.7, 0.5, EARTH_CORE_MASS_FRACTION, 0.3, 0.2, 0.1, 0.0];
        for m in MASSES {
            let mass = EarthMasses::new(m);
            for cmf in dry {
                let core = CoreComposition::new(cmf, 0.0).unwrap();
                let r = radius_zeng(mass, core).value();
                let sides: &[SnowLineSide] = if cmf >= EARTH_CORE_MASS_FRACTION {
                    &[SnowLineSide::Inside, SnowLineSide::Beyond]
                } else {
                    &[SnowLineSide::Inside]
                };
                for &side in sides {
                    let solved = solve(m, r, side, 1.0);
                    let found = solved.core().core_mass_fraction();
                    assert!((found - cmf).abs() < 1e-12, "{m} M⊕, {cmf}: {found}");
                    assert!((solved.radius().value() - r).abs() < 1e-15);
                    assert_eq!(solved.adjustment(), RadiusAdjustment::Unchanged);
                }
            }
            for x in [0.01, 0.1, 0.3, 0.5, OUTER_WATER_CAP] {
                let core = CoreComposition::new(EARTH_CORE_MASS_FRACTION, x).unwrap();
                let r = radius_zeng(mass, core).value();
                let solved = solve(m, r, SnowLineSide::Beyond, 1.0);
                assert!(
                    (solved.fractions().water() - x).abs() < 1e-12,
                    "{m} M⊕, {x}"
                );
                assert!(solved.fractions().envelope() < 1e-12);
            }
        }
    }

    #[test]
    fn nothing_formed_inside_the_snow_line_is_a_water_world() {
        for m in MASSES {
            for i in 1..200_u32 {
                let r = 0.1 * f64::from(i);
                let solved = solve(m, r, SnowLineSide::Inside, 10.0);
                assert!(
                    solved.fractions().water() <= INNER_WATER_CAP,
                    "{m} M⊕, {r} R⊕"
                );
            }
        }
    }

    #[test]
    fn the_rocky_planets_come_out_with_their_core_mass_fractions() {
        // NASA fact-sheet masses and mean radii; core mass fractions as in `radius`'s tests.
        for (name, kg, km, cmf) in [
            ("Mercury", 0.330_10e24, 2_439.7, 0.70),
            ("Venus", 4.867_3e24, 6_051.8, 0.31),
            ("Earth", 5.972_2e24, 6_371.0, EARTH_CORE_MASS_FRACTION),
            ("Mars", 0.641_69e24, 3_389.5, 0.2),
        ] {
            let solved = solve(
                kg / EARTH_MASS_KG,
                km * 1e3 / EARTH_RADIUS_M,
                SnowLineSide::Inside,
                1.0,
            );
            let found = solved.core().core_mass_fraction();
            assert!((found - cmf).abs() < 0.05, "{name}: {found}");
            assert!(solved.fractions().envelope() < 1e-15 && solved.fractions().water() < 1e-15);
        }
    }

    #[test]
    fn the_ice_giants_and_saturn_come_out_with_plausible_envelopes() {
        // (name, mass kg, mean radius km, semi-major axis au) from NASA's fact sheets, at their
        // flux 1 ÷ a² F⊕; formed beyond the snow line. Uranus and Neptune hold 10–20% hydrogen and
        // helium by mass in interior models, and Saturn about 25 M⊕ of heavy elements (Saumon and
        // Guillot 2004, as Fortney et al. 2007, §6.1, quote), an envelope of about 74%.
        for (name, kg, km, a, range) in [
            ("Uranus", 86.811e24, 25_362.0, 19.191, 0.05..0.25),
            ("Neptune", 102.409e24, 24_622.0, 30.069, 0.05..0.25),
            ("Saturn", 568.32e24, 58_232.0, 9.537, 0.65..0.9),
        ] {
            let solved = solve(
                kg / EARTH_MASS_KG,
                km * 1e3 / EARTH_RADIUS_M,
                SnowLineSide::Beyond,
                1.0 / (a * a),
            );
            let envelope = solved.envelope_fraction();
            assert!(range.contains(&envelope), "{name}: {envelope}");
            assert_eq!(solved.adjustment(), RadiusAdjustment::Unchanged, "{name}");
        }
    }

    #[test]
    fn radii_outside_the_models_are_moved_onto_them() {
        let tiny = solve(1.0, 0.5, SnowLineSide::Beyond, 1.0);
        assert_eq!(tiny.adjustment(), RadiusAdjustment::RaisedToIron);
        let iron = radius_zeng(EarthMasses::new(1.0), CoreComposition::IRON).value();
        assert!((tiny.radius().value() - iron).abs() < 1e-15);
        assert!((tiny.fractions().iron() - 1.0).abs() < 1e-15);
        let rock = solve(1.0, 1.2, SnowLineSide::Inside, 1.0);
        assert_eq!(rock.adjustment(), RadiusAdjustment::ClampedToRock);
        assert!((rock.fractions().water() - INNER_WATER_CAP).abs() < 1e-15);
        assert!(rock.radius().value() < 1.07);
        let ice = solve(1.0, 1.4, SnowLineSide::Beyond, 1.0);
        assert_eq!(ice.adjustment(), RadiusAdjustment::ClampedToIce);
        assert!((ice.fractions().water() - OUTER_WATER_CAP).abs() < 1e-15);
        let puffed = solve(100.0, 20.0, SnowLineSide::Inside, 1.0);
        assert_eq!(
            puffed.adjustment(),
            RadiusAdjustment::ClampedToEnvelopeLimit
        );
        assert!((puffed.fractions().envelope() - 0.985).abs() < 1e-12);
        assert!(puffed.radius().value() < 20.0);
    }

    #[test]
    fn the_envelope_grows_with_the_radius_and_vanishes_on_the_icy_curve() {
        let mass = EarthMasses::new(5.0);
        let icy = CoreComposition::new(EARTH_CORE_MASS_FRACTION, OUTER_WATER_CAP).unwrap();
        let edge = radius_zeng(mass, icy).value();
        let mut last = 0.0;
        for i in 0..200_u32 {
            let r = edge * (1.0 + 1e-6) + 0.02 * f64::from(i);
            let solved = solve(5.0, r, SnowLineSide::Beyond, 10.0);
            let f = solved.envelope_fraction();
            assert!(f > last || (i == 0 && f > 0.0), "{r}: {f} after {last}");
            assert!((solved.radius().value() / r - 1.0).abs() < 1e-12);
            last = f;
        }
        let just_above = solve(5.0, edge * (1.0 + 1e-9), SnowLineSide::Beyond, 10.0);
        assert!(just_above.envelope_fraction() < 1e-9);
    }

    #[test]
    fn giants_and_bad_inputs_are_refused() {
        let giant = EarthMasses::new(200.0);
        assert_eq!(
            composition(giant, EarthRadii::new(11.0), SnowLineSide::Beyond, ONE),
            Err(SolveCompositionError::GiantPlanet { mass: giant })
        );
        let one = EarthMasses::new(1.0);
        assert_eq!(
            composition(
                EarthMasses::new(f64::NAN),
                EarthRadii::new(1.0),
                SnowLineSide::Inside,
                ONE
            ),
            Err(SolveCompositionError::MassNotPositive)
        );
        assert_eq!(
            composition(one, EarthRadii::ZERO, SnowLineSide::Inside, ONE),
            Err(SolveCompositionError::RadiusNotPositive)
        );
        assert_eq!(
            composition(
                one,
                EarthRadii::new(1.0),
                SnowLineSide::Inside,
                EarthFluxes::new(-1.0)
            ),
            Err(SolveCompositionError::FluxNotValid)
        );
        let text = SolveCompositionError::GiantPlanet { mass: giant }.to_string();
        assert!(text.starts_with('a') && !text.ends_with('.'), "{text}");
    }

    #[test]
    fn the_solve_is_the_same_twice() {
        let a = solve(7.3, 2.61, SnowLineSide::Inside, 55.0);
        let b = solve(7.3, 2.61, SnowLineSide::Inside, 55.0);
        assert_eq!(a, b);
    }
}
