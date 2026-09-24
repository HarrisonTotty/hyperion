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
//! # Giants (P14.T11.d)
//!
//! From 0.414 Jupiter masses, Chen and Kipping's Jovian class, [`composition`] returns
//! [`SolveCompositionError::GiantPlanet`], never a fallback: that is the seam at which a giant's
//! radius is [`radius_giant`]'s and its composition [`giant_composition`]'s.
//!
//! A giant is a hydrogen and helium envelope over its heavy elements, whose mass is Thorngren et
//! al.'s (2016, ApJ 831, 64, §5.1; arXiv:1511.07854v2) fit to 47 transiting giants cool enough not
//! to be inflated, `M_z` = 57.9 M⊕ × (M ÷ `M_J`)^0.61 ([`giant_heavy_elements`]). It gives 57.9 M⊕
//! for Jupiter and 27.7 M⊕ at Saturn's mass, against the 37 and 27 M⊕ their own models give the two
//! (their Table 1). The same relation is Thorngren and Fortney's (2018) prior on the composition of
//! the hot Jupiters whose radii [`radius_giant`] reads, so a giant's radius and composition come
//! from one model. It is the mean relation: the planets scatter about it by a factor of 1.82 (1σ),
//! which is not drawn, because plan 13's cooling fit has no heavy elements for a draw to move the
//! radius by, and design note 8 keeps radius and composition from being drawn apart.
//!
//! The heavy elements are the core's composition of the solve on the same side of the snow line:
//! beyond it, [`OUTER_WATER_CAP`] of water on Earth-like rock and iron, close to the half ice, half
//! rock of Thorngren et al.'s models; inside it, Earth-like rock and iron. They count here as the
//! core, though those models hold up to 10 M⊕ of them in a core and mix the rest into the envelope
//! (their §3.1), and Thorngren and Fortney's (2018, §3), whose radii [`radius_giant`] reads, have
//! no core at all. From 0.3 to 0.414 Jupiter masses a
//! body's fractions are the solve's blended into the giant's by [`giant_share`], as its radius is
//! ([`GiantComposition::blended`]).

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::derive::envelope::radius_with_envelope;
#[cfg(doc)]
use crate::planetary::derive::radius::radius_giant;
use crate::planetary::derive::radius::{
    CoreComposition, DRY_CURVES, DeriveGiantError, EARTH_CORE_MASS_FRACTION,
    NEPTUNIAN_JOVIAN_TRANSITION, TablePosition, dry_radius, giant_mass, giant_share, radius_zeng,
    water_fraction_of_blend,
};
use crate::planetary::params::{
    COMPOSITION_REFERENCE_AGE, ENVELOPE_CORE_FLOOR, INNER_WATER_CAP, OUTER_WATER_CAP,
};
use crate::units::{EarthFluxes, EarthMasses, EarthRadii, JupiterMasses};

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
    /// P14.T11.d's: [`radius_giant`] and [`giant_composition`].
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

/// A dry body of mass `mass` and core mass fraction `core_mass_fraction` (0–1), at the radius of
/// [`radius_zeng`] for that composition: the composition of a rocky outcome, which the derivation
/// takes from the observed spread rather than from a radius (ruling 53).
///
/// Beyond the snow line such a body can lie between the Earth-like and rock curves, where
/// [`composition`] would read its radius as water on an Earth-like core; the rocky outcome is the
/// other reading of the same radius.
#[must_use]
pub(crate) fn dry_composition(mass: EarthMasses, core_mass_fraction: f64) -> SolvedComposition {
    let cmf = core_mass_fraction.clamp(0.0, 1.0);
    let radius = radius_zeng(mass, CoreComposition::from_fractions(cmf, 0.0));
    dry_body(cmf, radius.value(), RadiusAdjustment::Unchanged)
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

/// The radii a body keeps unchanged through [`composition`]: from the iron curve up to the
/// largest radius the rules of the [module](self) documentation allow its mass and side of the
/// snow line, at the flux given.
///
/// Every radius inside the window is kept as given (to the rounding of the envelope solve); one
/// below it is raised to iron and one above it clamped
/// ([`RadiusAdjustment`]), with one exception: for a core under [`ENVELOPE_CORE_FLOOR`] inside the
/// snow line the window ends at the rock curve, below the trace of water up to
/// [`INNER_WATER_CAP`] that the solve would still keep, so that the window is continuous in mass
/// at the floor, where that sliver ends (ruling 53). The derivation assembly confines a body's
/// drawn radius to the window (ruling 47 of 2026-09-22; plan 14, P14.T16.a).
///
/// The window also says where its rocky outcomes end ([`dry_top`](Self::dry_top)): the radii from
/// the iron curve up to it are the ones whose composition the derivation takes from the observed
/// spread of rocky planets' (ruling 53), the rock curve inside the snow line and the Earth-like
/// curve beyond it, above which a body formed there keeps its water (ruling 58).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusWindow {
    mass: EarthMasses,
    least: EarthRadii,
    rock: EarthRadii,
    dry_top: EarthRadii,
    dry_top_core_mass_fraction: f64,
    greatest: EarthRadii,
}

impl RadiusWindow {
    /// The mass the window is of, positive and finite.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The smallest radius kept: the pure-iron curve at the body's mass.
    #[must_use]
    pub const fn least(&self) -> EarthRadii {
        self.least
    }

    /// The pure-rock curve at the body's mass: between it and [`least`](Self::least) the solve
    /// reads a radius as dry rock and iron inside the snow line, and above the Earth-like curve as
    /// water on an Earth-like core beyond it.
    #[must_use]
    pub const fn rock(&self) -> EarthRadii {
        self.rock
    }

    /// The top of the rocky outcomes (rulings 53 and 58): the pure-rock curve inside the snow
    /// line, and beyond it the Earth-like curve, above which a body formed there keeps Chen and
    /// Kipping's radius and the water the solve reads in it.
    #[must_use]
    pub const fn dry_top(&self) -> EarthRadii {
        self.dry_top
    }

    /// The core mass fraction of the composition at [`dry_top`](Self::dry_top): 0, pure rock,
    /// inside the snow line, and Earth's [`EARTH_CORE_MASS_FRACTION`], 0.325, beyond it.
    #[must_use]
    pub const fn dry_top_core_mass_fraction(&self) -> f64 {
        self.dry_top_core_mass_fraction
    }

    /// The largest radius kept: the largest envelope the mass allows on the side's core
    /// ([`ENVELOPE_CORE_FLOOR`] of core left) where a body may take one and that is larger than the
    /// dry curve beneath it, and otherwise that curve (rock inside the snow line, [`OUTER_WATER_CAP`]
    /// of water on an Earth-like core beyond it).
    #[must_use]
    pub const fn greatest(&self) -> EarthRadii {
        self.greatest
    }
}

/// The [`RadiusWindow`] of a body of mass `mass`, formed on `side` of its snow line and receiving
/// `flux`: the radii [`composition`] keeps unchanged.
///
/// It is computed with the solve's own expressions, so that a radius inside it is never adjusted
/// and one outside it always is (tested).
///
/// # Errors
///
/// As [`composition`]'s, for the mass and flux: [`SolveCompositionError::GiantPlanet`] from 0.414
/// Jupiter masses, and the other variants for a mass that is not positive and finite or a flux
/// that is negative or not finite.
///
/// # Examples
///
/// Earth's mass inside the snow line keeps radii from pure iron to pure rock, and
/// Earth itself lies inside:
///
/// ```
/// use hyperion_sim::planetary::derive::composition::{SnowLineSide, radius_window};
/// use hyperion_sim::units::{EarthFluxes, EarthMasses};
///
/// let window = radius_window(EarthMasses::new(1.0), SnowLineSide::Inside, EarthFluxes::new(1.0))?;
/// assert!((window.least().value() - 0.823).abs() < 1e-3);
/// assert!((window.greatest().value() - 1.067).abs() < 1e-3);
/// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
/// ```
pub fn radius_window(
    mass: EarthMasses,
    side: SnowLineSide,
    flux: EarthFluxes,
) -> Result<RadiusWindow, SolveCompositionError> {
    let m = mass.value();
    if !(m.is_finite() && m > 0.0) {
        return Err(SolveCompositionError::MassNotPositive);
    }
    if !(flux.value().is_finite() && flux.value() >= 0.0) {
        return Err(SolveCompositionError::FluxNotValid);
    }
    if m >= NEPTUNIAN_JOVIAN_TRANSITION.value() {
        return Err(SolveCompositionError::GiantPlanet { mass });
    }
    let dry = TablePosition::of(mass).dry_radii();
    let (rock, iron) = (dry[0], dry[DRY_CURVES.len() - 1]);
    let with_envelope = |curve: f64, core: CoreComposition| {
        if m > ENVELOPE_CORE_FLOOR.value() {
            curve.max(largest_envelope(mass, core, flux).1.value())
        } else {
            curve
        }
    };
    let greatest = match side {
        SnowLineSide::Inside if m > ENVELOPE_CORE_FLOOR.value() => {
            with_envelope(rock, CoreComposition::EARTH_LIKE)
        }
        SnowLineSide::Inside => rock,
        SnowLineSide::Beyond => with_envelope(radius_zeng(mass, ICY_CORE).value(), ICY_CORE),
    };
    let (dry_top, dry_top_core_mass_fraction) = match side {
        SnowLineSide::Inside => (rock, 0.0),
        SnowLineSide::Beyond => (
            dry_radius(&dry, EARTH_CORE_MASS_FRACTION),
            EARTH_CORE_MASS_FRACTION,
        ),
    };
    Ok(RadiusWindow {
        mass,
        least: EarthRadii::new(iron),
        rock: EarthRadii::new(rock),
        dry_top: EarthRadii::new(dry_top),
        dry_top_core_mass_fraction,
        greatest: EarthRadii::new(greatest),
    })
}

/// The largest envelope fraction a body of mass `mass` may take on a core of `core`, leaving
/// [`ENVELOPE_CORE_FLOOR`] of core, and its radius at [`COMPOSITION_REFERENCE_AGE`] and `flux`.
#[must_use]
fn largest_envelope(
    mass: EarthMasses,
    core: CoreComposition,
    flux: EarthFluxes,
) -> (f64, EarthRadii) {
    let most = 1.0 - ENVELOPE_CORE_FLOOR.value() / mass.value();
    let largest = radius_with_envelope(mass, core, most, flux, COMPOSITION_REFERENCE_AGE);
    (most, largest)
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
    let (most, largest) = largest_envelope(mass, core, flux);
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

/// The heavy elements of a giant of 1 Jupiter mass, 57.9 M⊕ (Thorngren et al. 2016, §5.1: 57.9 ±
/// 7.03 M⊕).
pub const GIANT_HEAVY_ELEMENTS_AT_ONE_JUPITER_MASS: EarthMasses = EarthMasses::new(57.9);

/// How a giant's heavy elements grow with its mass, `M_z` ∝ M^0.61 (Thorngren et al. 2016, §5.1:
/// 0.61 ± 0.08).
pub const GIANT_HEAVY_ELEMENT_INDEX: f64 = 0.61;

/// The mass of heavy elements in a giant of mass `mass`: Thorngren et al.'s (2016) mean relation,
/// 57.9 M⊕ × (M ÷ `M_J`)^0.61 (see the [module](self) documentation).
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
#[must_use]
pub fn giant_heavy_elements(mass: EarthMasses) -> EarthMasses {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a mass is positive and finite, got {mass:?}"
    );
    let jupiters = JupiterMasses::from(mass).value();
    GIANT_HEAVY_ELEMENTS_AT_ONE_JUPITER_MASS * math::powf(jupiters, GIANT_HEAVY_ELEMENT_INDEX)
}

/// A giant's bulk composition (P14.T11.d), from [`giant_composition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiantComposition {
    fractions: MassFractions,
    core: CoreComposition,
    heavy_elements: EarthMasses,
    share: f64,
}

impl GiantComposition {
    /// The giant's mass fractions: its heavy elements as iron, rock and water, and the rest
    /// hydrogen and helium envelope. They are the body's from 0.414 Jupiter masses; below that,
    /// [`blended`](Self::blended) are.
    #[must_use]
    pub const fn fractions(&self) -> MassFractions {
        self.fractions
    }

    /// The composition of the heavy elements.
    #[must_use]
    pub const fn core(&self) -> CoreComposition {
        self.core
    }

    /// The mass of the heavy elements ([`giant_heavy_elements`]).
    #[must_use]
    pub const fn heavy_elements(&self) -> EarthMasses {
        self.heavy_elements
    }

    /// The fractions of a body whose composition solve gave `below`: the solve's and the giant's
    /// blended linearly by [`giant_share`], so `below` at 0.3 Jupiter masses and
    /// [`fractions`](Self::fractions) from 0.414. They still sum to 1.
    #[must_use]
    pub fn blended(&self, below: MassFractions) -> MassFractions {
        let w = self.share;
        if w >= 1.0 {
            return self.fractions;
        }
        if w <= 0.0 {
            return below;
        }
        let mix = |a: f64, b: f64| (1.0 - w) * a + w * b;
        let giant = self.fractions;
        MassFractions {
            iron: mix(below.iron, giant.iron),
            rock: mix(below.rock, giant.rock),
            water: mix(below.water, giant.water),
            envelope: mix(below.envelope, giant.envelope),
        }
    }
}

/// The bulk composition of a giant planet of mass `mass` that formed on `side` of its snow line
/// (P14.T11.d): Thorngren et al.'s (2016) heavy elements, as the core of the composition solve on
/// that side, under a hydrogen and helium envelope (see the [module](self) documentation).
///
/// It is what fills the [`SolveCompositionError::GiantPlanet`] seam of [`composition`].
///
/// # Errors
///
/// [`DeriveGiantError::MassOutsideGiants`] outside 0.3–13 Jupiter masses, plan 13's cooling fit,
/// which [`radius_giant`] reads.
///
/// # Examples
///
/// A Jupiter is 82% hydrogen and helium; one formed beyond the snow line holds its heavy elements
/// half as water:
///
/// ```
/// use hyperion_sim::planetary::derive::composition::{SnowLineSide, giant_composition};
/// use hyperion_sim::planetary::derive::radius::DeriveGiantError;
/// use hyperion_sim::units::{EarthMasses, JupiterMasses};
///
/// let jupiter = EarthMasses::from(JupiterMasses::new(1.0));
/// let giant = giant_composition(jupiter, SnowLineSide::Beyond)?;
/// assert!((giant.heavy_elements().value() - 57.9).abs() < 1e-9);
/// let f = giant.fractions();
/// assert!((f.envelope() - 0.818).abs() < 0.001);
/// assert!((f.water() / (1.0 - f.envelope()) - 0.539).abs() < 0.001);
/// # Ok::<(), DeriveGiantError>(())
/// ```
pub fn giant_composition(
    mass: EarthMasses,
    side: SnowLineSide,
) -> Result<GiantComposition, DeriveGiantError> {
    giant_mass(mass)?;
    let heavy_elements = giant_heavy_elements(mass);
    let core = match side {
        SnowLineSide::Inside => CoreComposition::EARTH_LIKE,
        SnowLineSide::Beyond => ICY_CORE,
    };
    Ok(GiantComposition {
        fractions: MassFractions::of(core, 1.0 - heavy_elements.value() / mass.value()),
        core,
        heavy_elements,
        share: giant_share(mass),
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::radius::radius_chen_kipping;
    use crate::stellar::draws::UnitUniform;
    use crate::stellar::substellar::{GIANT_MAX_MASS, GIANT_MIN_MASS};
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

    #[test]
    fn the_window_is_what_the_solve_keeps() {
        // Just inside either edge the radius is kept; just outside it is moved. The masses include
        // both sides of the envelope floor and cores whose largest envelope is below their dry
        // curve.
        for m in [
            1e-3, 0.1, 1.0, 1.5, 1.500_01, 1.51, 1.6, 3.0, 10.0, 40.0, 131.0,
        ] {
            let mass = EarthMasses::new(m);
            for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                for s in [1e-3, 1.0, 300.0] {
                    let window = radius_window(mass, side, EarthFluxes::new(s)).unwrap();
                    let (least, greatest) = (window.least().value(), window.greatest().value());
                    let at = |r: f64| solve(m, r, side, s).adjustment();
                    let label = format!("{m} M⊕ {side:?} at {s} F⊕");
                    assert_eq!(
                        at(least * (1.0 + 1e-9)),
                        RadiusAdjustment::Unchanged,
                        "{label}"
                    );
                    assert_eq!(
                        at(greatest * (1.0 - 1e-9)),
                        RadiusAdjustment::Unchanged,
                        "{label}"
                    );
                    assert_eq!(
                        at(least * (1.0 - 1e-9)),
                        RadiusAdjustment::RaisedToIron,
                        "{label}"
                    );
                    let above = solve(m, greatest * (1.0 + 1e-9), side, s);
                    if side == SnowLineSide::Inside && m <= ENVELOPE_CORE_FLOOR.value() {
                        // The window stops at the rock curve, under the trace of water the solve
                        // still keeps.
                        assert_eq!(above.adjustment(), RadiusAdjustment::Unchanged, "{label}");
                        assert!(above.fractions().water() > 0.0, "{label}");
                        assert_same_bits(greatest, window.rock().value());
                    } else {
                        assert_ne!(above.adjustment(), RadiusAdjustment::Unchanged, "{label}");
                    }
                }
            }
        }
        assert_eq!(
            radius_window(EarthMasses::new(200.0), SnowLineSide::Inside, ONE),
            Err(SolveCompositionError::GiantPlanet {
                mass: EarthMasses::new(200.0)
            })
        );
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

    fn jupiters(m_j: f64) -> EarthMasses {
        EarthMasses::from(JupiterMasses::new(m_j))
    }

    #[test]
    fn giants_hold_thorngren_et_al_s_heavy_elements() {
        // Their fit: 57.9 M⊕ at 1 Jupiter mass, growing as M^0.61; at Saturn's mass 27.7 M⊕,
        // against the 27 M⊕ of their own Saturn model, and Jupiter's 57.9 within their scatter,
        // a factor of 1.82, of the 37 M⊕ of theirs (Thorngren et al. 2016, Table 1).
        assert!((giant_heavy_elements(jupiters(1.0)).value() - 57.9).abs() < 1e-12);
        let saturn = giant_heavy_elements(EarthMasses::new(568.32e24 / EARTH_MASS_KG)).value();
        assert!((saturn - 27.7).abs() < 0.05, "{saturn} M⊕");
        let ratio = 57.9 / 37.0;
        assert!(ratio < 1.82);
        let doubled = giant_heavy_elements(jupiters(2.0)) / giant_heavy_elements(jupiters(1.0));
        assert!((doubled - math::powf(2.0, 0.61)).abs() < 1e-12);
        // Fractions sum to 1 from 0.3 to 13 Jupiter masses; the heavy elements are the solve's
        // core on each side of the snow line, and the envelope rises from 71% to 93%.
        for i in 0..=100_u32 {
            let m_j = 0.3 * math::powf(13.0 / 0.3, f64::from(i) / 100.0);
            for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                let giant = giant_composition(jupiters(m_j), side).unwrap();
                let f = giant.fractions();
                assert!((sum(f) - 1.0).abs() < 1e-14, "{m_j} MJ: {f:?}");
                let heavy = 1.0 - f.envelope();
                let z = giant.heavy_elements().value() / jupiters(m_j).value();
                assert!((heavy - z).abs() < 1e-14);
                assert!((0.06..0.32).contains(&heavy), "{m_j} MJ: {heavy}");
                match side {
                    SnowLineSide::Inside => {
                        assert!(f.water().abs() < f64::MIN_POSITIVE);
                        assert!((f.iron() / heavy - EARTH_CORE_MASS_FRACTION).abs() < 1e-12);
                        assert_eq!(giant.core(), CoreComposition::EARTH_LIKE);
                    }
                    SnowLineSide::Beyond => {
                        assert!((f.water() / heavy - OUTER_WATER_CAP).abs() < 1e-12);
                        assert_eq!(giant.core(), ICY_CORE);
                    }
                }
            }
        }
    }

    #[test]
    fn the_giant_seam_is_filled() {
        // Where the solve refuses a giant, giant_composition answers, and it answers from 0.3
        // Jupiter masses, where the blend begins.
        let jupiter = jupiters(1.0);
        let refused = composition(jupiter, EarthRadii::new(11.2), SnowLineSide::Beyond, ONE);
        assert_eq!(
            refused,
            Err(SolveCompositionError::GiantPlanet { mass: jupiter })
        );
        assert!(giant_composition(jupiter, SnowLineSide::Beyond).is_ok());
        let edge = EarthMasses::from(GIANT_MIN_MASS);
        assert!(giant_composition(edge, SnowLineSide::Beyond).is_ok());
        assert!(giant_composition(EarthMasses::from(GIANT_MAX_MASS), SnowLineSide::Inside).is_ok());
        for m in [
            EarthMasses::new(95.0),
            jupiters(13.1),
            EarthMasses::new(f64::NAN),
        ] {
            assert!(matches!(
                giant_composition(m, SnowLineSide::Beyond),
                Err(DeriveGiantError::MassOutsideGiants(_))
            ));
        }
    }

    #[test]
    fn a_giant_s_fractions_are_continuous_where_the_giants_begin() {
        // Chen and Kipping's radius through the solve below, the giant's fractions above, and the
        // two blended between: continuous at 0.3 and 0.414 Jupiter masses, summing to 1.
        let lower = |m: f64, q: f64, s: f64| {
            let mass = EarthMasses::new(m);
            let r = radius_chen_kipping(mass, UnitUniform::new(q).unwrap());
            solve(m, r.value(), SnowLineSide::Beyond, s).fractions()
        };
        let body = |m: f64, q: f64, s: f64| {
            let giant = giant_composition(EarthMasses::new(m), SnowLineSide::Beyond);
            if m < EarthMasses::from(GIANT_MIN_MASS).value() {
                lower(m, q, s)
            } else if m >= NEPTUNIAN_JOVIAN_TRANSITION.value() {
                giant.unwrap().fractions()
            } else {
                giant.unwrap().blended(lower(m, q, s))
            }
        };
        let edges = [
            EarthMasses::from(GIANT_MIN_MASS).value(),
            NEPTUNIAN_JOVIAN_TRANSITION.value(),
        ];
        for q in [0.1, 0.5, 0.9] {
            for s in [0.01, 1.0, 1_000.0] {
                for edge in edges {
                    let (a, b) = (
                        body(edge * (1.0 - 1e-9), q, s),
                        body(edge * (1.0 + 1e-9), q, s),
                    );
                    for (x, y) in [
                        (a.iron(), b.iron()),
                        (a.rock(), b.rock()),
                        (a.water(), b.water()),
                        (a.envelope(), b.envelope()),
                    ] {
                        assert!((x - y).abs() < 1e-6, "{edge} M⊕, {q}, {s} F⊕: {a:?} {b:?}");
                    }
                    assert!((sum(b) - 1.0).abs() < 1e-14);
                }
            }
        }
        let giant = giant_composition(jupiters(1.0), SnowLineSide::Beyond).unwrap();
        let other = lower(100.0, 0.5, 1.0);
        assert_eq!(giant.blended(other), giant.fractions());
    }
}
