//! The scattering step after a supernova (P14.T28.c, ruling 71).
//!
//! Mass lost at once gives each surviving body its own eccentricity, e = ΔM ÷ `M_after` for a
//! circular orbit, with its pericentre where the body was at the death, and with a natal kick its
//! own orientation too. So neighbours' orbits cross, or come closer than Gladman's (1993) 2√3
//! mutual Hill radii. Such pairs are unstable on orbital timescales, not over gigayears, and a
//! snapshot that showed them would be of a state that never exists. So right after each sudden
//! death of a host star, [`scatter`] resolves them:
//!
//! 1. The bodies the death left bound are walked in slot order, pair by pair, and the first pair
//!    found unsettled ([`unsettled`]) is resolved. The walk then starts again, until no pair is
//!    left unsettled. Each pass removes one body, so there are at most n − 1 passes.
//! 2. The outcome follows the Safronov number of the heavier body at its own orbit, Θ = ½ (`v_esc`
//!    ÷ `v_orb`)² = (m ÷ M) (a ÷ R) ([`safronov_number`]). It is Ford and Rasio's (2008, ApJ 686,
//!    621, eqs. 4–5) θ², with which a planet "is able to efficiently eject bodies" for θ ≫ 1
//!    while "when θ < 1, collisions will be much more frequent", and the ratio of ejections to
//!    collisions grows with a ÷ R (Petrovich et al. 2014, ApJ 786, 101, §1 and §4.2):
//!    - **Θ ≥ 1: ejection.** The lighter body is `Unbound`, and the heavier takes the pair's
//!      binding energy (Ford and Rasio 2008, eq. 2), so its orbit shrinks in its own plane, with
//!      an eccentricity drawn from Ford and Rasio's Table 1 at the pair's mass ratio
//!      ([`recoiled`], ruling 80). In
//!      unequal pairs it is the lighter that is ejected: for mass ratios of 0.3 or less, under 1%
//!      of ejections leave the lighter bound (Ford and Rasio 2008, §3.2.2), and in Petrovich et
//!      al.'s runs of 0.5 and 1.5 Jupiter masses, "if a planet is ejected, it is always the
//!      lighter one" (§4.3).
//!    - **Θ < 1: collision.** The lighter body is `Destroyed { Collided }`, and merges into the
//!      heavier, which takes both masses. The merger is completely inelastic and conserves
//!      momentum (Ford and Rasio 2008, §3.2.1): its specific angular momentum and energy are the
//!      pair's, mass-weighted ([`merged`]). The energy's mean is their eq. 1, 1 ÷ `a_f` =
//!      Σ (`m_i` ÷ m) ÷ `a_i`, "only slightly less" than their simulations' merged axes.
//! 3. Both happen at the death. The scattering takes a few orbits, which no snapshot resolves.
//!
//! Of two bodies of the same mass, the one in the later slot is taken as the lighter. The only
//! draws are an ejection survivor's, on its own `planet.scatter` stream ([`ScatterDraws`]): its
//! k-th survived ejection reads words 4k to 4k + 3, whatever else happens in its system, so the
//! slot order and the body's own stream make the step deterministic.

use core::f64::consts::PI;

use super::{BodyFate, BodyState, DestructionCause, ScatterDraws, Segment};
use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::orbit::{Eccentricity, InvertStateError, KeplerElements, Orbit, elements_from_state};
use crate::planetary::hosts::evolved::remnant_roche_limit;
use crate::planetary::params::HILL_STABLE_GAP;
use crate::planetary::placement::mutual_hill_radius;
use crate::rng::{ObjectKey, Stream, tags};
use crate::time::UniverseTime;
use crate::units::{EarthMasses, Kilograms, KilogramsPerCubicMetre, Metres, Radians, SolarMasses};

/// The Safronov number from which a heavier body ejects its neighbour rather than colliding with
/// it: Θ = 1, Ford and Rasio's (2008, eq. 4) θ = 1 (ruling 71).
pub(crate) const EJECTING_SAFRONOV_NUMBER: f64 = 1.0;

/// Resolves every unsettled pair among the bodies of `fates`, one host's in slot order, that the
/// sudden death at `at` left bound (the [module](self) documentation), and returns the passes it
/// took: one per body removed.
pub(super) fn scatter(fates: &mut [BodyFate<'_>], at: UniverseTime) -> usize {
    let mut passes = 0;
    while let Some((heavier, lighter)) = first_unsettled(fates, at) {
        resolve(fates, heavier, lighter, at);
        passes += 1;
    }
    passes
}

/// The first unsettled pair in slot order among the bodies the death at `at` left bound, as the
/// indices of its heavier and its lighter body, or `None` if there is none.
fn first_unsettled(fates: &[BodyFate<'_>], at: UniverseTime) -> Option<(usize, usize)> {
    let survivors: Vec<(usize, &Segment)> = fates
        .iter()
        .enumerate()
        .filter(|(_, fate)| fate.ending.is_none())
        .filter_map(|(k, fate)| {
            fate.segments
                .last()
                .filter(|segment| segment.start == at)
                .map(|segment| (k, segment))
        })
        .collect();
    survivors.iter().enumerate().find_map(|(n, &(i, first))| {
        survivors[n + 1..].iter().find_map(|&(j, second)| {
            unsettled(first, second).then(|| {
                if second.mass > first.mass {
                    (j, i)
                } else {
                    (i, j)
                }
            })
        })
    })
}

/// Whether the bodies on `first` and `second`, segments about the same mass that start at the
/// same death, are too close to be stable: the outer's pericentre is less than
/// [`HILL_STABLE_GAP`] (2√3) mutual Hill radii outside the inner's apocentre about the remnant's
/// mass (Gladman 1993), which crossing orbits are by any margin.
///
/// Inner and outer are by semi-major axis, as design note 7's spacing test takes them.
#[must_use]
fn unsettled(first: &Segment, second: &Segment) -> bool {
    let (inner, outer) = if second.orbit.semi_major_axis() < first.orbit.semi_major_axis() {
        (second, first)
    } else {
        (first, second)
    };
    let hill = mutual_hill_radius(
        inner.mass,
        outer.mass,
        inner.reference,
        inner.orbit.semi_major_axis(),
        outer.orbit.semi_major_axis(),
    );
    let gap = outer.orbit.periapsis().value() - inner.orbit.apoapsis().value();
    gap < HILL_STABLE_GAP * hill.value()
}

/// The Safronov number Θ = ½ (`v_esc` ÷ `v_orb`)² = (m ÷ M) (a ÷ R) of a body of `mass` and bulk
/// `density` on an orbit of semi-major axis `a` about `host`: half the square of its surface
/// escape speed over its circular orbital speed at `a`, or its escape speed's square over the
/// host's at `a` (Ford and Rasio 2008, eq. 4's θ², with r = a; Petrovich et al.'s 2014 eq. 1 θ²
/// is 2Θ).
///
/// The definition is Ford and Rasio's (2008) eq. 4, θ² = (Gm ÷ R)(r ÷ GM★), with r taken at the
/// semi-major axis rather than their apocentre, since it decides only the outcome (ruling 75.1–2).
/// Petrovich et al.'s (2014) eq. 1 defines θ² = (`v_esc` ÷ `v_orb`)² instead, which is 2Θ, with its
/// boundary at θ = 1: the same physics under another definition.
///
/// The radius is (3m ÷ 4πρ)^⅓. Jupiter at 5.2 au from the Sun has Θ ≈ 10.6, and the Earth at
/// 1 au Θ ≈ 0.07.
#[must_use]
pub(crate) fn safronov_number(
    mass: EarthMasses,
    density: KilogramsPerCubicMetre,
    a: Metres,
    host: SolarMasses,
) -> f64 {
    let m = Kilograms::from(mass).value();
    let radius = math::cbrt(3.0 * m / (4.0 * PI * density.value()));
    m / Kilograms::from(host).value() * (a.value() / radius)
}

/// Resolves the unsettled pair of `fates[heavier]` and `fates[lighter]` at the death at `at`: an
/// ejection or a collision by the heavier's Safronov number.
fn resolve(fates: &mut [BodyFate<'_>], heavier: usize, lighter: usize, at: UniverseTime) {
    let heavy = *fates[heavier]
        .segments
        .last()
        .expect("a survivor has a segment");
    let density = fates[heavier].body.density;
    let theta = safronov_number(
        heavy.mass,
        density,
        heavy.orbit.semi_major_axis(),
        heavy.reference,
    );
    let light_fate = &mut fates[lighter];
    // The lighter keeps its segment from the death, which its ending then cuts off at once, so
    // that it keeps the mass it had, with any neighbour merged into it before.
    let light = *light_fate
        .segments
        .last()
        .expect("a survivor has a segment");
    if theta >= EJECTING_SAFRONOV_NUMBER {
        light_fate.ending = Some(BodyState::Unbound { at });
        let heavy_fate = &mut fates[heavier];
        let segment = heavy_fate
            .segments
            .last_mut()
            .expect("a survivor has a segment");
        let ranks = heavy_fate.body.scatter.ranks(heavy_fate.recoils);
        heavy_fate.recoils += 1;
        match recoiled(&heavy, &light, density, ranks, at) {
            Recoil::Bound(orbit) => segment.orbit = orbit,
            Recoil::Disrupted => {
                heavy_fate.ending = Some(BodyState::Destroyed {
                    cause: DestructionCause::TidallyDisrupted,
                    at,
                });
            }
        }
        return;
    }
    light_fate.ending = Some(BodyState::Destroyed {
        cause: DestructionCause::Collided,
        at,
    });
    let heavy_fate = &mut fates[heavier];
    let segment = heavy_fate
        .segments
        .last_mut()
        .expect("a survivor has a segment");
    // The merged body takes both masses whatever becomes of it, so that a body gone has the mass
    // it had when it went.
    segment.mass = heavy.mass + light.mass;
    match merged(&heavy, &light, density, at) {
        Merger::Bound(orbit) => segment.orbit = orbit,
        Merger::Disrupted => {
            heavy_fate.ending = Some(BodyState::Destroyed {
                cause: DestructionCause::TidallyDisrupted,
                at,
            });
        }
        Merger::Unbound => heavy_fate.ending = Some(BodyState::Unbound { at }),
    }
}

/// Ford and Rasio's (2008, ApJ 686, 621) Table 1: the survivor of an ejection's mean
/// eccentricity and its dispersion, by the ejected body's share of the pair's mass β = m₂ ÷ (m₁ +
/// m₂), from their 6,525 integrations that ended in an ejection.
const EJECTION_ECCENTRICITY_TABLE: [(f64, f64, f64); 7] = [
    (0.20, 0.202, 0.056),
    (0.25, 0.263, 0.071),
    (0.30, 0.333, 0.100),
    (0.35, 0.421, 0.133),
    (0.40, 0.513, 0.145),
    (0.45, 0.602, 0.142),
    (0.50, 0.624, 0.135),
];

/// The dispersion of the survivor's eccentricity as a share of its mean below Table 1's β = 0.2:
/// 0.28, Table 1's own 0.056 ÷ 0.202 at β = 0.2 (ruling 80). It is this generator's
/// extrapolation, not Ford and Rasio's.
const EXTRAPOLATED_DISPERSION_SHARE: f64 = 0.28;

/// The mean and the dispersion of the eccentricity of the survivor of an ejection in which the
/// ejected body is `beta` = m₂ ÷ (m₁ + m₂) of the pair's mass (ruling 80).
///
/// From β = 0.2 to 0.5, linear in β through Ford and Rasio's (2008) Table 1. Below 0.2 the mean is
/// their §4.2 fit to the medians, ē = 1.44 β^1.23, and the dispersion 0.28 ē, which is this
/// generator's extrapolation (0.199 and 0.056 at β = 0.2, against the table's 0.202 and 0.056).
/// The survivor's eccentricity grows with the ejected body's share of the mass, as Raymond,
/// Armitage and Gorelick (2010, ApJ 711, 772, §3) find for unequal pairs.
#[must_use]
pub(crate) fn ejection_eccentricity_law(beta: f64) -> (f64, f64) {
    let [first, .., last] = EJECTION_ECCENTRICITY_TABLE;
    if beta < first.0 {
        let mean = 1.44 * math::powf(beta, 1.23);
        return (mean, EXTRAPOLATED_DISPERSION_SHARE * mean);
    }
    if beta >= last.0 {
        return (last.1, last.2);
    }
    EJECTION_ECCENTRICITY_TABLE
        .windows(2)
        .find(|pair| beta < pair[1].0)
        .map(|pair| {
            let ((b0, e0, s0), (b1, e1, s1)) = (pair[0], pair[1]);
            let t = (beta - b0) / (b1 - b0);
            (e0 + t * (e1 - e0), s0 + t * (s1 - s0))
        })
        .expect("a β inside the table lies in one of its intervals")
}

/// The standard normal's cumulative distribution, ½ erfc(−x ÷ √2).
fn normal_cdf(x: f64) -> f64 {
    0.5 * math::erfc(-x / core::f64::consts::SQRT_2)
}

/// The eccentricity of rank `rank`, in (0, 1), of the normal of `mean` and `dispersion`
/// truncated to [0, 1): its inverse distribution, one uniform for one draw (ruling 80).
#[must_use]
pub(crate) fn truncated_eccentricity(mean: f64, dispersion: f64, rank: f64) -> f64 {
    let lo = normal_cdf(-mean / dispersion);
    let hi = normal_cdf((1.0 - mean) / dispersion);
    let p = (lo + rank * (hi - lo)).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    (mean + dispersion * math::normal_quantile(p)).clamp(0.0, 1.0 - f64::EPSILON)
}

/// What an ejection leaves of its survivor.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Recoil {
    /// The survivor, on these elements.
    Bound(KeplerElements),
    /// The survivor's new pericentre is inside the remnant's Roche limit.
    Disrupted,
}

/// The orbit the survivor `heavy` of the ejection of `light` at `at` is left on, for its bulk
/// `density` and its two ranks `(eccentricity, phase)` (ruling 80).
///
/// - **Its energy** is Ford and Rasio's (2008) eq. 2: the ejected body leaves with a very small
///   positive energy (Moorehead and Adams 2005), so the survivor takes the pair's binding energy,
///   m₁ `ε_f` = m₁ ε₁ + m₂ ε₂, and 1 ÷ `a_f` = 1 ÷ a₁ + (m₂ ÷ m₁) ÷ a₂: its orbit shrinks.
/// - **Its eccentricity** is drawn from Ford and Rasio's Table 1 at β = m₂ ÷ (m₁ + m₂), a normal
///   truncated to [0, 1) ([`ejection_eccentricity_law`], [`truncated_eccentricity`]).
/// - **Its plane and its line of apsides** are kept, and its mean anomaly at `at` is drawn,
///   uniform over a turn.
/// - A pericentre inside the remnant's Roche limit is [`Recoil::Disrupted`]: Ford and Rasio find
///   about 3% of survivors grazing their star at β = 0.5 (§3.2.3).
///
/// The survivor then re-enters the walk, so its new orbit is tested against every other.
fn recoiled(
    heavy: &Segment,
    light: &Segment,
    density: KilogramsPerCubicMetre,
    (eccentricity_rank, phase_rank): (f64, f64),
    at: UniverseTime,
) -> Recoil {
    let orbit = &heavy.orbit;
    let mu = orbit.gravitational_parameter();
    let share = light.mass.value() / heavy.mass.value();
    let energy = specific_energy(orbit) + share * specific_energy(&light.orbit);
    let axis = -mu.value() / (2.0 * energy);
    let beta = light.mass.value() / (heavy.mass.value() + light.mass.value());
    let (mean, dispersion) = ejection_eccentricity_law(beta);
    let e = truncated_eccentricity(mean, dispersion, eccentricity_rank);
    if axis * (1.0 - e) < remnant_roche_limit(mu, density).value() {
        return Recoil::Disrupted;
    }
    let build = |at_epoch: f64| {
        KeplerElements::from_semi_major_axis(
            Metres::new(axis),
            mu,
            Eccentricity::new(e).expect("an eccentricity in [0, 1)"),
            *orbit.orientation(),
            Radians::new(at_epoch),
        )
        .expect("a bound orbit smaller than the survivor's is representable")
    };
    let gained = build(0.0).mean_anomaly_at(at).value();
    Recoil::Bound(build(core::f64::consts::TAU * phase_rank - gained))
}

impl ScatterDraws {
    /// The ranks of the `k`-th ejection a body survives, `(eccentricity, phase)`: words 4k and
    /// 4k + 1 of its `planet.scatter` stream, or ½ and 0 for [`ScatterDraws::Median`].
    #[must_use]
    pub(crate) fn ranks(&self, k: u64) -> (f64, f64) {
        match *self {
            Self::Median => (0.5, 0.0),
            Self::Stream { seed, body } => {
                let mut stream = Stream::open(seed, tags::PLANET_SCATTER, ObjectKey::from(body));
                stream.seek(k * super::SCATTER_WORDS_PER_EJECTION);
                (stream.uniform_open(), stream.uniform_open())
            }
        }
    }
}

/// What a collision leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Merger {
    /// The merged body, on these elements.
    Bound(KeplerElements),
    /// The merged orbit's pericentre is inside the remnant's Roche limit.
    Disrupted,
    /// The merged orbit is open, from e = 0.9999 as a supernova's is (ruling 62.6).
    Unbound,
}

/// The orbit of the body `heavy` and `light` merge into at `at`, for the merged body's bulk
/// `density`, the heavier's (Ford and Rasio 2008, §3.2.1; ruling 71).
///
/// - Its specific angular momentum h and energy ε are the pair's mass-weighted means, the vector
///   h with its direction, so the plane is the mean of theirs, and its gravitational parameter is
///   the heavier's. Then a = −μ ÷ 2ε and e = √(1 − h² ÷ μa).
/// - Where the mean energy is lower than any orbit of that angular momentum can have (h² > μa,
///   which near-circular neighbours of nearly equal axes can give, since a harmonic mean of axes
///   lies under the square of the mean of their roots), the merged orbit is circular at h² ÷ μ:
///   the angular momentum is kept, and the energy rises to the least it allows.
/// - Its pericentre points along the mass-weighted mean of the two eccentricity vectors, projected
///   on the new plane, or along the heavier's pericentre where that mean is too small to point
///   anywhere, and the body is at its pericentre at `at`: the phase of a collision a few orbits
///   after the death is not modelled.
fn merged(
    heavy: &Segment,
    light: &Segment,
    density: KilogramsPerCubicMetre,
    at: UniverseTime,
) -> Merger {
    let total = heavy.mass.value() + light.mass.value();
    let (wh, wl) = (heavy.mass.value() / total, light.mass.value() / total);
    let mean = |x: [f64; 3], y: [f64; 3]| {
        [
            wh * x[0] + wl * y[0],
            wh * x[1] + wl * y[1],
            wh * x[2] + wl * y[2],
        ]
    };
    let momentum = mean(
        angular_momentum(&heavy.orbit),
        angular_momentum(&light.orbit),
    );
    let energy = wh * specific_energy(&heavy.orbit) + wl * specific_energy(&light.orbit);
    let pointing = mean(
        eccentricity_vector(&heavy.orbit),
        eccentricity_vector(&light.orbit),
    );
    let mu = heavy.orbit.gravitational_parameter();
    let size = length(momentum);
    if size.is_nan() || size <= 0.0 {
        // Opposed angular momenta that cancel leave a radial plunge.
        return Merger::Disrupted;
    }
    let axis = -mu.value() / (2.0 * energy);
    let semi_latus = size * size / mu.value();
    let eccentricity = (1.0 - semi_latus / axis).max(0.0).sqrt();
    let pericentre = semi_latus / (1.0 + eccentricity);
    if pericentre < remnant_roche_limit(mu, density).value() {
        return Merger::Disrupted;
    }
    let normal = momentum.map(|c| c / size);
    let direction = in_plane(pointing, normal)
        .or_else(|| in_plane(heavy.orbit.orientation().periapsis_direction(), normal))
        .or_else(|| in_plane([1.0, 0.0, 0.0], normal))
        .or_else(|| in_plane([0.0, 1.0, 0.0], normal))
        .expect("of two orthogonal axes, one lies off any unit normal");
    let along = cross(normal, direction);
    let speed = size / pericentre;
    let r = SystemVector::new(direction.map(|c| c * pericentre));
    let v = SystemVelocity::new(along.map(|c| c * speed));
    merger_of(elements_from_state(r, v, mu, at))
}

/// The merger an inversion of its state gives.
fn merger_of(orbit: Result<Orbit, InvertStateError>) -> Merger {
    match orbit {
        Ok(Orbit::Bound(orbit)) => Merger::Bound(orbit),
        Err(InvertStateError::Radial) => Merger::Disrupted,
        Ok(Orbit::Open(_))
        | Err(
            InvertStateError::NotFinite
            | InvertStateError::GravitationalParameterNotPositive { .. }
            | InvertStateError::PericentreTimeOutOfRange
            | InvertStateError::Unrepresentable(_),
        ) => Merger::Unbound,
    }
}

/// The specific angular momentum of `orbit`, m² s⁻¹: √(μ a (1 − e²)) along its normal.
fn angular_momentum(orbit: &KeplerElements) -> [f64; 3] {
    let e = orbit.eccentricity().value();
    let size = (orbit.gravitational_parameter().value()
        * orbit.semi_major_axis().value()
        * ((1.0 - e) * (1.0 + e)))
        .sqrt();
    orbit.orientation().normal().map(|c| c * size)
}

/// The specific orbital energy of `orbit`, −μ ÷ 2a, J kg⁻¹.
fn specific_energy(orbit: &KeplerElements) -> f64 {
    -orbit.gravitational_parameter().value() / (2.0 * orbit.semi_major_axis().value())
}

/// The eccentricity vector of `orbit`: e towards its pericentre.
fn eccentricity_vector(orbit: &KeplerElements) -> [f64; 3] {
    let e = orbit.eccentricity().value();
    orbit.orientation().periapsis_direction().map(|c| c * e)
}

/// The unit vector along `x`'s projection on the plane normal to the unit `normal`, or `None` if
/// that projection is too small against `x` to have a direction.
fn in_plane(x: [f64; 3], normal: [f64; 3]) -> Option<[f64; 3]> {
    let along = dot(x, normal);
    let projected = [
        x[0] - along * normal[0],
        x[1] - along * normal[1],
        x[2] - along * normal[2],
    ];
    let size = length(projected);
    (size > 1e-9 * length(x) && size > 0.0).then(|| projected.map(|c| c / size))
}

/// a · b, summed in the fixed order x, y, z.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// a × b.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// |x|, summed in the fixed order x, y, z.
fn length(x: [f64; 3]) -> f64 {
    dot(x, x).sqrt()
}

#[cfg(test)]
mod tests;
