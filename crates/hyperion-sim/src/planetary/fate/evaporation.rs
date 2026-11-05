//! A small hot rocky body's evaporation (P14.T55.b.a): the vapour of its rock streams off its lit
//! face, and the fate transform has it `Destroyed { Evaporated }` once it has lost its whole
//! radius.
//!
//! # The rate
//!
//! Perez-Becker and Chiang (2013, MNRAS 433, 2294; arXiv:1302.2147v2, here PBC13) follow the
//! thermal wind of rock vapour off a strongly lit low-mass rocky planet. As the planet's mass
//! falls, all their solutions meet the free-streaming limit (their Figure 2), "where gravity
//! becomes irrelevant and Ṁ is set entirely by conditions at the surface" (Figure 8's caption):
//! Ṁ ≈ ρ `c_s` R² (eq. 28), the wind launched over Ω = 1 sr about the substellar point (eq. 27),
//! with
//!
//! - the vapour pressure of olivine, P = e^b exp(−m `L_sub` ÷ (k T)) (eq. 7), at
//!   m = 169 `m_H`, `L_sub` = 3.21 × 10¹⁰ erg g⁻¹ and e^b = 6.72 × 10¹⁴ dyn cm⁻², Kimura et al.'s
//!   (2002) from Nagahara et al.'s (1994) forsterite, the surface they take for every later
//!   figure (§3.1);
//! - the vapour's density ρ = μ P ÷ (k T) (eq. 9) at μ = 30 `m_H` (eq. 8), and its sound speed
//!   `c_s` = √(γ k T ÷ μ) (eq. 14) at γ = 1.3 (§2.1);
//! - T the dayside's peak, the substellar zero-albedo temperature T∗ √(R∗ ÷ a) e^(−τ ÷ 4)
//!   (eq. 6), at τ = `τ_s` = 0.01 (eq. 17), the optical depth their free-streaming solutions leave
//!   the surface, whose wind carries no dust that matters (§3.2, Figure 5);
//! - and the duty cycle `f_duty` = 0.5 by which their histories time-average the rate (§3.3).
//!
//! So a body of bulk density `ρ_b` loses mass at `f_duty` ρ `c_s` R² and its radius at
//! `f_duty` ρ `c_s` ÷ (4π `ρ_b`) whatever its size, and it is gone once the radius lost is its
//! own. For a starlit body the light is the hosts' bolometric light at the orbit, Σ L ÷ (4π a²),
//! averaged over the orbit as 1 ÷ (a² √(1 − e²)), with which T∗ √(R∗ ÷ a) is (L ÷ (4π σ a²))^¼;
//! the orbit average of the light, not of the rate, which the light's steep rise near pericentre
//! would raise (lean, 2026-11-03). PBC13's rocky planets lose no orbit to their wind (§4.4), and
//! the transform moves none.
//!
//! # Where it applies
//!
//! Below 10⁻³ M⊕ ([`FREE_STREAMING_MASS`]), PBC13's smallest mass, as their solutions converge on
//! the free-streaming limit (Figure 2): its vapour's Jeans parameter G M μ ÷ (k T R) is then about
//! 1 at their 2,145 K and 5.4 g cm⁻³, and falls as the body's radius squared. Above it gravity
//! throttles the wind by orders of magnitude (their full model, Figure 8, read at 2,145 K alone),
//! which the transform does not model: a heavier body keeps its mass (lean, 2026-11-03).
//!
//! # The history
//!
//! A body is followed from its formation along its history, its hosts' light L(t) and its orbit
//! a(t), e(t) as the fate transform holds them, never at its system's age: it is hot only while
//! its hosts are bright, as once a host has left its main sequence. [`first_loss`] integrates the
//! radius lost, interval by interval between the history's break points, by an adaptive
//! Gauss–Legendre rule, and bisects the interval it is lost in to the second.
//!
//! # What is left
//!
//! While it evaporates a body is present with the mass its radius has left,
//! R(t) = R₀ − ∫ rate dt ([`mass_left`]), read from the parts [`first_loss`] integrated
//! ([`lost_by`]): a planet or a member through the fate's mass, which its derivation and record
//! read, and a moon in its record's mass, its bulk read at the mass it formed with (lean,
//! 2026-11-03). A belt loses the share of its named members' mass they have lost, those gone and
//! the rest's losses, from its mass and its dust's shine, since its cascade's smaller bodies about
//! the same light go before them (lean, 2026-11-03); with none left it ends, `Destroyed
//! { Evaporated }`, when the last of them went (`PlanetarySystem::belt_evaporation`).

use crate::math;
use crate::planetary::hosts::evolved::bisect;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::time::{Span, UniverseTime};
use crate::units::consts::{
    BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG, SOLAR_LUMINOSITY_W, STEFAN_BOLTZMANN,
};
use crate::units::{EarthMasses, KilogramsPerCubicMetre, Metres, SolarLuminosities};

/// The mass below which a rocky body's vapour streams freely off it, M⊕: 10⁻³, PBC13's smallest
/// (Perez-Becker and Chiang 2013, Figure 2, whose mass axis begins there), toward which all their
/// solutions at 2,145 K converge on the free-streaming limit as the mass falls; there the vapour's
/// Jeans parameter G M μ ÷ (k T R) is about 1, at their 5.4 g cm⁻³. Their full model's rates above
/// it (Figure 8, from 0.01 M⊕) are not modelled.
pub(crate) const FREE_STREAMING_MASS: EarthMasses = EarthMasses::new(1e-3);

/// The bulk density a moon's evaporating rock is read at, kg m⁻³: 3,300, a small moon's
/// uncompressed rock (ruling 83.7's), since its ice, far more volatile, would go before it
/// (lean, 2026-11-03).
pub(crate) const MOON_EVAPORATING_ROCK_DENSITY: KilogramsPerCubicMetre =
    crate::planetary::moons::regular::MOON_ROCK_DENSITY;

/// m `L_sub` ÷ k of olivine's sublimation, K: the molecule's mass, 169 `m_H` (Mg₁.₁Fe₀.₉SiO₄),
/// times its latent heat, 3.21 × 10¹⁰ erg g⁻¹ = 3.21 × 10⁶ J kg⁻¹, over Boltzmann's constant,
/// about 65,800 K (PBC13, eq. 7 and §2.1, after Kimura et al. 2002 and Nagahara et al. 1994).
const OLIVINE_SUBLIMATION_TEMPERATURE_K: f64 =
    169.0 * HYDROGEN_MASS_KG * 3.21e6 / BOLTZMANN_CONSTANT;

/// The constant e^b of olivine's vapour pressure, Pa: 6.72 × 10¹⁴ dyn cm⁻² (PBC13, eq. 7 and
/// §2.1).
const OLIVINE_VAPOUR_PRESSURE_SCALE_PA: f64 = 6.72e13;

/// The rock vapour's mean molecular mass, kg: 30 `m_H`, that of Mg and SiO, which with O and O₂
/// make up such a vapour (PBC13, eq. 8, after Schaefer and Fegley 2009).
const VAPOUR_MOLECULAR_MASS_KG: f64 = 30.0 * HYDROGEN_MASS_KG;

/// The rock vapour's adiabatic index (PBC13, §2.1): 1.3, a diatomic gas's at high temperature.
const VAPOUR_ADIABATIC_INDEX: f64 = 1.3;

/// The share of the time the wind blows at its rate, by which PBC13's histories time-average it:
/// 0.5, from KIC 12557548b's transit depths (PBC13, §3.3, after Brogi et al. 2012).
const DUTY_CYCLE: f64 = 0.5;

/// The optical depth from the star to the surface: 0.01, PBC13's `τ_s` (eq. 17), all of it for
/// a wind with no dust that matters, as their free-streaming solutions are (§3.2).
const SURFACE_OPTICAL_DEPTH: f64 = 0.01;

/// The share of the radius left that a stretch of a history whose bound on the rate cannot reach
/// it in its length is taken to take: none, at 10⁻⁶ of it, so that a body never near hot is not
/// integrated at all ([`first_loss`]).
const NEGLIGIBLE_SHARE: f64 = 1e-6;

/// The adaptive rule's tolerance on the radius lost over a stretch, as a share of the radius
/// left, spread over the stretch by length ([`first_loss`]).
const RELATIVE_TOLERANCE: f64 = 1e-6;

/// The most halvings of an interval between break points the adaptive rule makes.
const MAX_DEPTH: u32 = 40;

/// The shortest part the adaptive rule halves, s: 10⁶, some twelve days, against histories of
/// gigayears, inside which the time is bisected to the second.
const SHORTEST_PART_S: f64 = 1e6;

/// The most parts the adaptive rule cuts one interval between break points into, a budget that
/// bounds its cost on a rate that is rough at every scale it reaches.
const MAX_PARTS: usize = 4_096;

/// The substellar zero-albedo temperature of a body on an orbit of semi-major axis
/// `semi_major_axis` and eccentricity `eccentricity` about hosts of light `luminosity`, K:
/// (e^(−`τ_s`) L ÷ (4π σ a² √(1 − e²)))^¼, PBC13's eq. 6 on the orbit-averaged light (the
/// [module documentation](self)). Zero for no light.
#[must_use]
pub(crate) fn substellar_temperature(
    luminosity: SolarLuminosities,
    semi_major_axis: Metres,
    eccentricity: f64,
) -> f64 {
    let a = semi_major_axis.value();
    let flux = luminosity.value() * SOLAR_LUMINOSITY_W
        / (4.0 * core::f64::consts::PI * a * a * (1.0 - eccentricity * eccentricity).sqrt());
    (flux * math::exp(-SURFACE_OPTICAL_DEPTH) / STEFAN_BOLTZMANN)
        .max(0.0)
        .sqrt()
        .sqrt()
}

/// The rate at which a body of bulk density `density` below [`FREE_STREAMING_MASS`] loses its
/// radius at the substellar temperature `temperature_k`, m s⁻¹: `f_duty` ρ `c_s` ÷ (4π `ρ_b`),
/// with ρ `c_s` = P √(γ μ ÷ (k T)) (PBC13, eqs. 7, 9, 14 and 28; the [module
/// documentation](self)). Zero at no temperature.
#[must_use]
pub(crate) fn radius_rate(temperature_k: f64, density: KilogramsPerCubicMetre) -> f64 {
    if temperature_k <= 0.0 {
        return 0.0;
    }
    let pressure = OLIVINE_VAPOUR_PRESSURE_SCALE_PA
        * math::exp(-OLIVINE_SUBLIMATION_TEMPERATURE_K / temperature_k);
    let flux = pressure
        * (VAPOUR_ADIABATIC_INDEX * VAPOUR_MOLECULAR_MASS_KG
            / (BOLTZMANN_CONSTANT * temperature_k))
            .sqrt();
    DUTY_CYCLE * flux / (4.0 * core::f64::consts::PI * density.value())
}

/// The radius of a sphere of `mass` at `density`, m.
#[must_use]
pub(crate) fn radius_of(mass: EarthMasses, density: KilogramsPerCubicMetre) -> f64 {
    let volume = crate::units::Kilograms::from(mass).value() / density.value();
    math::cbrt(3.0 * volume / (4.0 * core::f64::consts::PI))
}

/// A part of a stretch of a body's history that [`first_loss`] integrated, by which the radius
/// lost by any time in it is found again ([`lost_by`]): from `start` to `end`, after `before`, m,
/// was lost, taking `taken`, m.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LossPart {
    pub(crate) start: UniverseTime,
    pub(crate) end: UniverseTime,
    pub(crate) before: f64,
    pub(crate) taken: f64,
}

/// The radius lost by `t`, m, of a history whose integrated parts are `parts`, in time order, at
/// `rate` (m s⁻¹, at a time) over the part that holds `t`: what the parts before it took, and the
/// 16-point rule from its start to `t`, as [`first_loss`] bisects it; none before the first part,
/// whose stretch took none that counts.
#[must_use]
pub(crate) fn lost_by(
    parts: &[LossPart],
    t: UniverseTime,
    rate: impl Fn(UniverseTime) -> f64,
) -> f64 {
    let after = parts.partition_point(|part| part.start <= t);
    let Some(part) = after.checked_sub(1).map(|k| &parts[k]) else {
        return 0.0;
    };
    if t < part.end {
        part.before + rule(part.start, t, &rate)
    } else {
        part.before + part.taken
    }
}

/// The mass, M⊕, of a body that formed with `mass` at `density` and has lost `lost`, m, of its
/// radius: `mass` (1 − `lost` ÷ R₀)³, its rock's sphere shrunk (PBC13's eq. 28 at a radius that
/// falls; the [module documentation](self)). Never quite none, at a radius left of 2⁻⁵² of its own,
/// since a body present has some.
#[must_use]
pub(crate) fn mass_left(
    mass: EarthMasses,
    density: KilogramsPerCubicMetre,
    lost: f64,
) -> EarthMasses {
    let share = (1.0 - lost / radius_of(mass, density)).max(f64::EPSILON);
    EarthMasses::new(mass.value() * share * share * share)
}

/// How much of its radius a body loses in a stretch of its history ([`first_loss`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Loss {
    /// It loses all it has left, by this time.
    Gone(UniverseTime),
    /// It loses this much, m, and is left with the rest.
    Part(f64),
}

/// The first time in `from`–`to` by which a body losing its radius at `rate` (m s⁻¹, at a time)
/// has lost `left`, m, or, if it never has, the radius it loses in all of it, m.
///
/// `breaks` gives the times strictly between `from` and `to`, ascending, at which the rate may
/// turn sharply: the history's break points, found only for a stretch the bound does not rule out.
/// `bound`, at a time t, is a bound on the rate over `from`–t that never falls as t grows. Every
/// interval between break points (and the ends) whose bound at its end cannot take
/// [`NEGLIGIBLE_SHARE`] of `left` in the whole stretch takes none; since the bound never falls,
/// those are the intervals before the first that can, and a stretch whose bound at `to` cannot
/// takes nothing. From that one on, each interval is integrated by the 16-point Gauss–Legendre
/// rule: taken whole where that is within its share of [`RELATIVE_TOLERANCE`] of `left`, and
/// otherwise halved where the rule on its halves differs from it on the whole by more than that
/// share, up to [`MAX_DEPTH`] times, into parts no shorter than [`SHORTEST_PART_S`] and at most
/// [`MAX_PARTS`] of them, in time order; in the part where the radius lost reaches `left`, the
/// time is bisected to the second, its later end returned. Each part integrated is added to
/// `parts`, in time order, its `before` counted from `from`.
///
/// # Panics
///
/// If `to` is before `from`, or a break point lies outside them.
#[must_use]
pub(crate) fn first_loss(
    from: UniverseTime,
    to: UniverseTime,
    breaks: impl FnOnce() -> Vec<UniverseTime>,
    left: f64,
    (bound, rate): (impl Fn(UniverseTime) -> f64, impl Fn(UniverseTime) -> f64),
    parts: &mut Vec<LossPart>,
) -> Loss {
    if left <= 0.0 {
        return Loss::Gone(from);
    }
    let span = seconds_between(from, to);
    if span <= 0.0 {
        return Loss::Part(0.0);
    }
    let floor = NEGLIGIBLE_SHARE * left / span;
    if bound(to) <= floor {
        return Loss::Part(0.0);
    }
    let points: Vec<UniverseTime> = breaks().into_iter().chain(core::iter::once(to)).collect();
    // The first interval whose bound can take a share that counts, and every one after it.
    let first = points.partition_point(|&end| bound(end) <= floor);
    if first == points.len() {
        return Loss::Part(0.0);
    }
    let tolerance = RELATIVE_TOLERANCE * left / span;
    let mut lost = 0.0;
    let mut a = first.checked_sub(1).map_or(from, |k| points[k]);
    for &b in &points[first..] {
        if let Some(at) = integrate((a, b), left, tolerance, &rate, (&mut lost, parts)) {
            return Loss::Gone(at);
        }
        a = b;
    }
    Loss::Part(lost)
}

/// Adds to `lost` the radius lost at `rate` over `a`–`b`, by [`first_loss`]'s adaptive rule at
/// `tolerance` m per second of the interval, and each part it integrates to `parts`, and returns
/// the time by which it reaches `left`, if it does inside it.
fn integrate(
    (a, b): (UniverseTime, UniverseTime),
    left: f64,
    tolerance: f64,
    rate: &impl Fn(UniverseTime) -> f64,
    (lost, parts): (&mut f64, &mut Vec<LossPart>),
) -> Option<UniverseTime> {
    if b <= a {
        return None;
    }
    // Intervals still to integrate, the latest at the bottom, with their rule's value and depth.
    let mut stack = vec![(a, b, rule(a, b, rate), 0_u32)];
    let mut cut = 1;
    while let Some((lo, hi, whole, depth)) = stack.pop() {
        let allowed = tolerance * seconds_between(lo, hi);
        // A part that takes less than its share of the tolerance, and not the radius left, is
        // taken whole: most of a hot history's are.
        if whole.abs() <= allowed && *lost + whole < left {
            parts.push(LossPart {
                start: lo,
                end: hi,
                before: *lost,
                taken: whole,
            });
            *lost += whole;
            continue;
        }
        let mid = midpoint(lo, hi);
        let (first, second) = (rule(lo, mid, rate), rule(mid, hi, rate));
        let halvable = depth < MAX_DEPTH
            && cut < MAX_PARTS
            && seconds_between(lo, hi) >= 2.0 * SHORTEST_PART_S;
        if (first + second - whole).abs() > allowed && halvable {
            cut += 1;
            stack.push((mid, hi, second, depth + 1));
            stack.push((lo, mid, first, depth + 1));
            continue;
        }
        for (start, end, taken) in [(lo, mid, first), (mid, hi, second)] {
            parts.push(LossPart {
                start,
                end,
                before: *lost,
                taken,
            });
            if *lost + taken >= left {
                let before = *lost;
                let mut clearance = |t: UniverseTime| left - before - rule(start, t, rate);
                return Some(bisect(start, end, &mut clearance));
            }
            *lost += taken;
        }
    }
    None
}

/// The 16-point Gauss–Legendre rule for the integral of `rate` over `a`–`b`.
fn rule(a: UniverseTime, b: UniverseTime, rate: &impl Fn(UniverseTime) -> f64) -> f64 {
    let half = 0.5 * seconds_between(a, b);
    if half <= 0.0 {
        return 0.0;
    }
    let sum = GL16_NODES
        .iter()
        .zip(&GL16_WEIGHTS)
        .fold(0.0, |sum, (&node, &weight)| {
            let at = offset(a, half * (1.0 + node));
            sum + weight * rate(at)
        });
    half * sum
}

/// The seconds from `a` to `b`.
fn seconds_between(a: UniverseTime, b: UniverseTime) -> f64 {
    b.checked_since(a)
        .expect("two times of the clock are a span apart")
        .as_seconds_f64()
}

/// The time `seconds` after `t`, a time between two of the clock's.
fn offset(t: UniverseTime, seconds: f64) -> UniverseTime {
    Span::from_seconds_f64(seconds)
        .and_then(|span| t.checked_add(span))
        .expect("a time between two of the clock's is on the clock")
}

/// The time halfway from `a` to `b`, to the nanosecond.
fn midpoint(a: UniverseTime, b: UniverseTime) -> UniverseTime {
    let span = b
        .checked_since(a)
        .expect("two times of the clock are a span apart");
    let nanos = i128::from(span.seconds()) * 1_000_000_000 + i128::from(span.subsec_nanos());
    let half = nanos / 2;
    let seconds = i64::try_from(half.div_euclid(1_000_000_000)).expect("half a span is a span");
    let rest = u32::try_from(half.rem_euclid(1_000_000_000)).expect("a remainder below 10⁹");
    a.checked_add(Span::new(seconds, rest).expect("a normalised span"))
        .expect("halfway between two times of the clock is on it")
}

#[cfg(test)]
mod tests;
