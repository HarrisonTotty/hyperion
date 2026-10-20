//! A body's surface age and its crater contract (plan 14, P14.T24.b's surface age and crater
//! density, with P14.T48.c; science-p14-t24b-laws Part 2, §2.1 and §2.3).
//!
//! # The surface age
//!
//! S, the time since the last global resurfacing, as rendering plan R09 reads it, is
//! S = min(age − `t_MO`, τ(F), `t_GI`), held to [0, age] ([`surface_age`]):
//!
//! - age − `t_MO`, the time since the young magma ocean's end, P14.T28.a's `molten_until`, so that
//!   a primary lid is never younger than its melt (P14.T51.c's test); a body with no magma-ocean
//!   draw takes its age;
//! - τ(F) = τ₀ (F₀ ÷ F)^n, the resurfacing time at the present heat flow F, tides included
//!   ([`resurfacing_time`]): it reads F, not the regime, so it steps only where F steps, at a
//!   mobile lid's edge or an ocean's freezing;
//! - `t_GI`, the time since a giant impact: P14.T18 puts every one at the system's birth, so today
//!   it is the age, kept for a later impact clock.
//!
//! The crust ([`Crust::of`], the one mechanical predicate) picks τ₀ and F₀, since ice resurfaces at
//! a heat flow that would leave rock untouched: a silicate crust takes Earth's ocean floor,
//! [`SILICATE_RESURFACING_TIME`] at the generator's Earth's heat flow, and an ice-rich one
//! Europa's surface, [`ICE_RESURFACING_TIME`] at [`ICE_RESURFACING_HEAT_FLOW`] (P14.T56.d's
//! recalibration). The one exponent
//! [`RESURFACING_EXPONENT`] is empirical, fitted on Mars's Noachian 3.9 Gyr across three regimes:
//! a mobile lid's boundary-layer cooling alone would give n = 2, its heat flow falling as the
//! floor's age to the −½ (Turcotte and Schubert 2002, Geodynamics, 2nd ed., §4-16 and §4-23),
//! physical for Earth only.
//!
//! Out of sample, the generator's Venus is 0.39 Gyr old, against the 0.3–0.8 Gyr of the
//! published estimates as science-p14-t24b-laws §2.1 gives them (uncited there), and Io's
//! surface a few thousand years. The form's limits are recorded (science-p14-t24b-laws §2.1): S
//! can grow faster than time over gigayears, which no one universe's game time spans, and it steps
//! with F. Titan's young surface is its air's and liquids' work (R09.T14, P14.T51.d), and
//! Triton's its capture's tides, which are not modelled: both are recorded misses.
//!
//! # The crater contract
//!
//! N☾(>1 km) = [`lunar_chronology`](hyperion_surface::craters::lunar_chronology)(S, A) ×
//! `R_b`, the lunar-equivalent production at the surface age, before any screening or diameter
//! map, with A the system's age and `R_b` the impactors' rate
//! ([`Impactors::rate`](super::Impactors::rate)). It is published as the shared density's
//! [`CraterParams`](hyperion_surface::craters::CraterParams), with the air's column mass p ÷ g as
//! its screening, the gravity, the system's age and P14.T48.f's impactors, which the surface crate
//! derives the rest from (R09's Design note 12).

use super::Crust;
use super::interior::GENERATED_EARTH_HEAT_FLOW;
use crate::math;
use crate::units::{WattsPerSquareMetre, Years};

/// A silicate crust's resurfacing time at the generator's Earth's heat flow
/// ([`GENERATED_EARTH_HEAT_FLOW`]), yr: 64.2 Myr, the mean age of Earth's ocean floor (Seton et
/// al. 2020, Geochemistry, Geophysics, Geosystems 21, e2020GC009214, as R09's reference cites it;
/// science-p14-t24b-laws §2.1).
///
/// On a mobile lid the plates set the global resurfacing; continents lose their craters by
/// erosion, which is R09's T14 and P14.T51.d's, not the interior's.
pub const SILICATE_RESURFACING_TIME: Years = Years::new(64.2e6);

/// An ice-rich crust's resurfacing time at [`ICE_RESURFACING_HEAT_FLOW`], yr: 60 Myr, Europa's
/// surface age ("the corresponding nominal surface age is 60 Myr": Zahnle, Schenk, Levison and
/// Dones 2003, Icarus 163, 263, §5), inside Bierhaus et al.'s 40–90 Myr (2009, in Europa, Univ. of
/// Arizona Press, 161; as Pappalardo 2010 quotes it; science-p14-t24b-laws §2.1).
pub const ICE_RESURFACING_TIME: Years = Years::new(60e6);

/// The heat flow at which an ice-rich crust resurfaces in [`ICE_RESURFACING_TIME`], W m⁻²: 0.019 24,
/// the measured Europa's whole heat flow at its measured elements under P14.T56.d's own tides, as
/// built, so that its surface is Zahnle et al.'s 60 Myr (decision-p14-t56d-membrane §4.6 and §7.2).
/// `Fitted`: `europa_s_surface_at_its_own_tides_is_its_calibration` refits it.
///
/// It is calibrated on the moons' own tides, which it takes effect with. It replaces T48.c's
/// 0.0386, Europa's under decision-p14-moons F2's membrane, which the ruling found was not
/// Beuthe's (science-p14-t24b-laws §2.1's figure): at a given heat flow an ice-rich surface's
/// resurfacing time is (F₀ ÷ 0.0386)^n, about 0.13, of what that gave.
pub const ICE_RESURFACING_HEAT_FLOW: WattsPerSquareMetre = WattsPerSquareMetre::new(ICE_FITTED);

/// [`ICE_RESURFACING_HEAT_FLOW`]'s value, W m⁻², as its test refits it.
const ICE_FITTED: f64 = 0.019_239_4;

/// The exponent n of the resurfacing time τ = τ₀ (F₀ ÷ F)^n: about 2.91, empirical, fitted so that
/// the Solar System table's Mars gets 3.9 Gyr, the Noachian highlands' age R09's reference uses
/// (science-p14-t24b-laws §2.1). `Fitted`: a test refits it.
///
/// The ruling's 2.86 is the same fit on its replica's rounded heat flows (Earth 92.4 and Mars
/// 22.0 mW m⁻²); the generator's are 92.55 and 22.54. It must keep Mars's surface older than its
/// wet epoch's start (Part 3), for R09's craters to precede their erosion.
pub const RESURFACING_EXPONENT: f64 = 2.907_695_194_384_198;

/// The resurfacing time τ = τ₀ (F₀ ÷ F)^n of a crust `crust` at the present heat flow
/// `heat_flow`, yr ([`RESURFACING_EXPONENT`]): from [`SILICATE_RESURFACING_TIME`] at the
/// generator's Earth's heat flow for a silicate crust, and from [`ICE_RESURFACING_TIME`] at
/// [`ICE_RESURFACING_HEAT_FLOW`] for an ice-rich one. Infinite for a heat flow that is not
/// positive, which resurfaces nothing.
///
/// # Examples
///
/// The generator's Earth renews its ocean floor in 64.2 Myr, and a crust of a quarter of its heat
/// flow lasts some 4 Gyr; ice resurfaces at a heat flow under rock's.
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::Crust;
/// use hyperion_sim::planetary::hooks::figures::surface_age::resurfacing_time;
/// use hyperion_sim::planetary::hooks::figures::interior::GENERATED_EARTH_HEAT_FLOW;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let earth = resurfacing_time(Crust::Silicate, GENERATED_EARTH_HEAT_FLOW);
/// assert!((earth.value() / 64.2e6 - 1.0).abs() < 1e-12);
/// let quarter = WattsPerSquareMetre::new(GENERATED_EARTH_HEAT_FLOW.value() / 4.0);
/// let old = resurfacing_time(Crust::Silicate, quarter).value();
/// assert!((3.5e9..4.5e9).contains(&old));
/// assert!(resurfacing_time(Crust::IceRich, quarter).value() < old / 10.0);
/// ```
#[must_use]
pub fn resurfacing_time(crust: Crust, heat_flow: WattsPerSquareMetre) -> Years {
    let (time, flow) = match crust {
        Crust::Silicate => (SILICATE_RESURFACING_TIME, GENERATED_EARTH_HEAT_FLOW),
        Crust::IceRich => (ICE_RESURFACING_TIME, ICE_RESURFACING_HEAT_FLOW),
    };
    let f = heat_flow.value();
    if f > 0.0 {
        Years::new(time.value() * math::powf(flow.value() / f, RESURFACING_EXPONENT))
    } else {
        Years::new(f64::INFINITY)
    }
}

/// What a body's surface age reads ([`surface_age`]), as plain values.
///
/// Public fields carrying no invariant: the age is held to be finite and non-negative by
/// [`GlobalFigures::derive`](super::GlobalFigures::derive), which builds them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceAgeInputs {
    /// The body's age, its system's at the time, yr.
    pub age: Years,
    /// The system age at which its young magma ocean ended (P14.T28.a's `molten_until`), yr;
    /// `None` for a body with no magma-ocean draw, which takes its age.
    pub molten_until: Option<Years>,
    /// The system age of its last giant impact (P14.T18), yr; `None` for a body that had none.
    pub last_giant_impact: Option<Years>,
    /// Its present heat flow F, tides included, W m⁻².
    pub heat_flow: WattsPerSquareMetre,
    /// Its crust, by [`Crust::of`], which picks the resurfacing time's calibration.
    pub crust: Crust,
}

/// The surface age S of the body `inputs` describes, yr: min(age − `t_MO`, τ(F), `t_GI`), held to
/// [0, age] (the [module](self) documentation): zero while its magma ocean lasts.
///
/// # Examples
///
/// A Mars-like world of 22 mW m⁻² keeps a Noachian surface, but a body heated as Io is renews its
/// surface within a few thousand years; an old cold surface dates from its magma ocean's end.
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::Crust;
/// use hyperion_sim::planetary::hooks::figures::surface_age::{SurfaceAgeInputs, surface_age};
/// use hyperion_sim::units::{WattsPerSquareMetre, Years};
///
/// let at = |heat_flow: f64| {
///     surface_age(&SurfaceAgeInputs {
///         age: Years::new(4.57e9),
///         molten_until: Some(Years::new(3.2e7)),
///         last_giant_impact: None,
///         heat_flow: WattsPerSquareMetre::new(heat_flow),
///         crust: Crust::Silicate,
///     })
///     .value()
/// };
/// assert!((3.5e9..4.3e9).contains(&at(0.0225)));
/// assert!(at(2.5) < 1e4);
/// assert!((at(0.011) - (4.57e9 - 3.2e7)).abs() < 1.0);
/// ```
#[must_use]
pub fn surface_age(inputs: &SurfaceAgeInputs) -> Years {
    let age = inputs.age.value();
    let since = |event: Option<Years>| event.map_or(age, |at| age - at.value());
    let candidates = [
        since(inputs.molten_until),
        resurfacing_time(inputs.crust, inputs.heat_flow).value(),
        since(inputs.last_giant_impact),
    ];
    // The least by comparison, so that neither end's sign of zero nor a NaN, which validated
    // inputs never make, can pick another.
    let least = candidates
        .into_iter()
        .fold(age, |least, s| if s < least { s } else { least });
    Years::new(if least > 0.0 { least } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// The surface age is the least of its three terms, held to [0, age]: zero while the magma ocean
    /// lasts, never over the age, and an ice-rich crust younger than a silicate one at one heat flow.
    #[test]
    fn the_surface_age_is_the_least_of_its_terms() {
        let inputs = SurfaceAgeInputs {
            age: Years::new(4.0e9),
            molten_until: Some(Years::new(5e7)),
            last_giant_impact: None,
            heat_flow: WattsPerSquareMetre::new(0.01),
            crust: Crust::Silicate,
        };
        assert_same_bits(surface_age(&inputs).value(), 3.95e9);
        let molten = SurfaceAgeInputs {
            age: Years::new(1e7),
            ..inputs
        };
        assert_same_bits(surface_age(&molten).value(), 0.0);
        let impact = SurfaceAgeInputs {
            last_giant_impact: Some(Years::new(1e9)),
            ..inputs
        };
        assert_same_bits(surface_age(&impact).value(), 3e9);
        let none = SurfaceAgeInputs {
            molten_until: None,
            heat_flow: WattsPerSquareMetre::new(0.0),
            ..inputs
        };
        assert_same_bits(surface_age(&none).value(), 4e9);
        let warm = SurfaceAgeInputs {
            heat_flow: WattsPerSquareMetre::new(0.05),
            ..inputs
        };
        let ice = SurfaceAgeInputs {
            crust: Crust::IceRich,
            ..warm
        };
        assert!(surface_age(&ice) < surface_age(&warm));
        assert_same_bits(
            surface_age(&warm).value(),
            resurfacing_time(Crust::Silicate, warm.heat_flow).value(),
        );
    }
}
