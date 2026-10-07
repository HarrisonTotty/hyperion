//! Layer caps: how far out the census looks in each layer (rendering plan R06, Design note 9;
//! decided 2026-10-03, `decision-r06-t7-caps.md`).
//!
//! A layer's cap is the least radius beyond which its expected number of stars brighter than the
//! cut falls below one: the density field times the layer's luminosity function
//! ([`LuminosityTables`]), each star dimmed by its distance modulus and by the extinction of its
//! direction. The sky is cut into [`CapResolution::STANDARD`]'s 768 rays of a Fibonacci lattice,
//! each standing for its own solid angle and dimming its stars by its own extinction profile
//! ([`profile`], [`NoiseMode::Realised`], [`Quality::Full`], one march a ray, interpolated
//! linearly in distance between the profile's nodes): the census lists the realised field's
//! stars, and a mean field undercounts wherever the dust is patchy, since the count goes roughly
//! as 10^(−0.6 A), convex in A.
//!
//! A cap never passes the layer's **rule bound**, the distance at which the brightest star the
//! envelope allows the layer ([`BrightnessEnvelope::brightest`] at [`max_star_mass`] of the band's
//! top, twice it since R06.T16.b, at any age) falls to the cut with the **least** extinction of
//! any ray. Neither is a strict bound: a clear window narrower than the rays' spacing, about 7°,
//! could still show stars beyond them. Each cap therefore carries the expected number of the
//! layer's stars brighter than the cut beyond it, under one by construction, which the response
//! states; the slow test `caps_converge_in_rays` holds it under 1.5 against 3,072 rays and twice
//! the radial steps.
//!
//! The count is a quadrature over the rays and [`CapResolution::STANDARD`]'s 24 steps a decade in
//! radius from 1 ly, summed from the rule bound inward by the trapezoid rule.
//!
//! The plan first dimmed every direction by the least extinction of 48 rays, the polar rays' near
//! the Sun, and then each ray by the mean field's; R06's Risks record both and what they missed.

use crate::coords::{GalacticPosition, UnitVector};
use crate::galaxy::fields::MAX_COMPONENTS;
use crate::galaxy::gas::extinction::{NoiseMode, Quality, profile};
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::imf::MassBand;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::observe::Observer;
use crate::time::Span;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{LightYears, Magnitudes, SolarMasses, Years};

use super::envelope::{BrightnessEnvelope, max_star_mass};
use super::luminosity::LuminosityTables;

/// The rays of the standard resolution (decided 2026-10-03): 48 did not converge in the realised
/// field, 768 do near the Sun.
pub const CAP_RAYS: usize = 768;

/// The count's radial steps per decade of distance at the standard resolution.
pub const RADIAL_STEPS_PER_DECADE: u32 = 24;

/// The nodes per decade of each ray's extinction profile.
const PROFILE_NODES_PER_DECADE: u32 = 24;

/// The nearest distance the count reaches, ly: the stars nearer than it are the census's anyway.
const NEAREST_LY: f64 = 1.0;

/// The farthest any cap or rule bound reaches, ly: across the galaxy.
const FARTHEST_LY: f64 = 120_000.0;

/// The layers a cap is computed for: the five stellar layers and the brown dwarfs.
pub const CAPPED_LAYERS: [Layer; 6] = [
    Layer::A,
    Layer::B,
    Layer::C,
    Layer::D,
    Layer::E,
    Layer::BrownDwarf,
];

/// How finely the caps' count is taken: the rays over the sky and the radial steps a decade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapResolution {
    rays: usize,
    steps_per_decade: u32,
}

impl CapResolution {
    /// The caps' own: [`CAP_RAYS`] rays, [`RADIAL_STEPS_PER_DECADE`] steps a decade.
    pub const STANDARD: Self = Self {
        rays: CAP_RAYS,
        steps_per_decade: RADIAL_STEPS_PER_DECADE,
    };

    /// `rays` rays and `steps_per_decade` radial steps a decade, or `None` if either is zero.
    #[must_use]
    pub fn new(rays: usize, steps_per_decade: u32) -> Option<Self> {
        (rays > 0 && steps_per_decade > 0).then_some(Self {
            rays,
            steps_per_decade,
        })
    }
}

/// One layer's cap (Design note 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerCap {
    layer: Layer,
    radius: LightYears,
    rule_bound: LightYears,
    expected_beyond: f64,
}

impl LayerCap {
    /// The layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The cap: the census looks no farther in this layer.
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The rule's bound: where the layer's brightest possible star falls to the cut with the least
    /// extinction of any ray. The cap is never beyond it.
    #[must_use]
    pub const fn rule_bound(&self) -> LightYears {
        self.rule_bound
    }

    /// The expected number of the layer's stars brighter than the cut beyond the cap, under one.
    #[must_use]
    pub const fn expected_beyond(&self) -> f64 {
        self.expected_beyond
    }

    /// A cap of `radius` for `layer`, with nothing stated beyond it: the brute force's, which
    /// forces every layer to one radius.
    #[must_use]
    pub const fn forced(layer: Layer, radius: LightYears) -> Self {
        Self {
            layer,
            radius,
            rule_bound: radius,
            expected_beyond: 0.0,
        }
    }
}

/// The distances of every ray's profile, ly: 0, then [`PROFILE_NODES_PER_DECADE`] a decade from
/// [`NEAREST_LY`] to [`FARTHEST_LY`].
#[must_use]
fn profile_nodes() -> Vec<f64> {
    let decades = math::log10(FARTHEST_LY / NEAREST_LY);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few decades, a few hundred nodes"
    )]
    let steps = (decades * f64::from(PROFILE_NODES_PER_DECADE)).ceil() as u32;
    std::iter::once(0.0)
        .chain((0..=steps).map(|k| {
            if k == steps {
                FARTHEST_LY
            } else {
                NEAREST_LY * math::exp10(decades * f64::from(k) / f64::from(steps))
            }
        }))
        .collect()
}

/// The extinction in V along rays from a point, each from one [`profile`] march in the realised
/// field at full quality.
#[derive(Debug, Clone, PartialEq)]
pub struct RayExtinctions {
    nodes: Vec<f64>,
    directions: Vec<UnitVector>,
    /// Each ray's A<sub>V</sub> at each node.
    profiles: Vec<Vec<f64>>,
}

impl RayExtinctions {
    /// `rays` rays of a Fibonacci lattice from `origin`.
    #[must_use]
    pub fn measure(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        rays: usize,
        cache: &mut NoiseCache,
    ) -> Self {
        let nodes = profile_nodes();
        let distances: Vec<LightYears> = nodes.iter().map(|&d| LightYears::new(d)).collect();
        let directions = fibonacci_sphere(rays);
        let mut out = Vec::with_capacity(nodes.len());
        let profiles = directions
            .iter()
            .map(|&direction| {
                profile(
                    galaxy.gas(),
                    origin,
                    direction,
                    &distances,
                    NoiseMode::Realised,
                    Quality::Full,
                    &[],
                    cache,
                    &mut out,
                );
                out.iter().map(|a| a.value()).collect()
            })
            .collect();
        Self {
            nodes,
            directions,
            profiles,
        }
    }

    /// The rays' directions, in order.
    pub fn directions(&self) -> impl Iterator<Item = &UnitVector> {
        self.directions.iter()
    }

    /// Ray `ray`'s extinction at distance `d`, ly, interpolated linearly between its nodes.
    ///
    /// # Panics
    ///
    /// If `ray` is not one of the rays.
    #[must_use]
    pub fn along(&self, ray: usize, d: f64) -> f64 {
        let values = &self.profiles[ray];
        let k = self.nodes.partition_point(|&node| node <= d);
        if k == 0 {
            return values[0];
        }
        if k >= self.nodes.len() {
            return values[values.len() - 1];
        }
        let (d0, d1) = (self.nodes[k - 1], self.nodes[k]);
        let t = (d - d0) / (d1 - d0);
        values[k - 1] + (values[k] - values[k - 1]) * t
    }

    /// The least extinction of any ray at distance `d`, ly.
    #[must_use]
    pub fn least(&self, d: f64) -> f64 {
        (0..self.profiles.len())
            .map(|ray| self.along(ray, d))
            .fold(f64::INFINITY, f64::min)
    }
}

/// `n` directions spread evenly over the sphere (a Fibonacci lattice).
#[must_use]
pub(crate) fn fibonacci_sphere(n: usize) -> Vec<UnitVector> {
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    #[expect(clippy::cast_precision_loss, reason = "a few thousand directions")]
    let count = n as f64;
    (0..n)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "k < n")]
            let k = k as f64;
            let z = 1.0 - (2.0 * k + 1.0) / count;
            let rho = (1.0 - z * z).sqrt();
            let (sin, cos) = math::sin_cos(golden * k);
            UnitVector::from_components([rho * cos, rho * sin, z]).expect("a unit vector")
        })
        .collect()
}

/// The distance modulus at `d` ly: 5 log₁₀(d ÷ 10 pc).
#[must_use]
fn distance_modulus(d: f64) -> f64 {
    5.0 * math::log10(d / (10.0 * LY_PER_PARSEC))
}

/// Light-years in a parsec.
const LY_PER_PARSEC: f64 = 3.261_563_777_167_433_6;

/// The rule's bound of a layer whose brightest star is `brightest`: where it falls to `cut`.
#[must_use]
fn rule_bound(brightest: f64, cut: f64, rays: &RayExtinctions) -> f64 {
    let faint = |d: f64| brightest + distance_modulus(d) + rays.least(d) > cut;
    if faint(NEAREST_LY) {
        return NEAREST_LY;
    }
    if !faint(FARTHEST_LY) {
        return FARTHEST_LY;
    }
    let (mut lo, mut hi) = (NEAREST_LY, FARTHEST_LY);
    for _ in 0..60 {
        let mid = (lo * hi).sqrt();
        if faint(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// The count of stars brighter than the cut per unit ln r, per layer, at each radius.
#[derive(Debug, Clone, PartialEq)]
struct Counts {
    bounds: Vec<f64>,
    radii: Vec<f64>,
    per_ln_r: Vec<Vec<f64>>,
    step_ln: f64,
}

impl Counts {
    /// The expected count of `layer` (an index of [`CAPPED_LAYERS`]) between `r` and its bound,
    /// by the trapezoid rule, the interval holding `r` taken in part.
    #[must_use]
    fn beyond(&self, layer: usize, r: f64) -> f64 {
        let bound = self.bounds[layer];
        let values = &self.per_ln_r[layer];
        let mut sum = 0.0;
        for i in 0..self.radii.len() - 1 {
            let (r0, r1) = (self.radii[i], self.radii[i + 1]);
            if r1 <= r || r0 >= bound {
                continue;
            }
            let slice = f64::midpoint(values[i], values[i + 1]) * self.step_ln;
            let share = if r0 < r {
                math::ln(r1 / r) / math::ln(r1 / r0)
            } else {
                1.0
            };
            sum += slice * share;
        }
        sum
    }
}

/// The count for `observer` at `cut` and `resolution`.
fn count(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    resolution: CapResolution,
    cache: &mut NoiseCache,
) -> Counts {
    let origin = observer.position();
    let rays = RayExtinctions::measure(galaxy, origin, resolution.rays, cache);
    let any = galaxy
        .fields()
        .component_ids()
        .next()
        .expect("a galaxy has components");
    let bounds: Vec<f64> = CAPPED_LAYERS
        .iter()
        .map(|&layer| {
            let band = MassBand::from(layer);
            envelope
                .brightest(
                    layer,
                    any,
                    max_star_mass(SolarMasses::new(band.hi())),
                    (Years::ZERO, Years::new(super::envelope::MAX_AGE_YEARS)),
                )
                .map_or(NEAREST_LY, |m| rule_bound(m.value(), cut.value(), &rays))
        })
        .collect();
    let farthest = bounds.iter().copied().fold(NEAREST_LY, f64::max);
    // The radial nodes, from the nearest to the farthest bound, even in ln r.
    let decades = math::log10(farthest / NEAREST_LY).max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few decades of distance, a few hundred steps"
    )]
    let steps = ((decades * f64::from(resolution.steps_per_decade)).ceil() as u32).max(1);
    let radii: Vec<f64> = (0..=steps)
        .map(|k| NEAREST_LY * math::exp10(decades * f64::from(k) / f64::from(steps)))
        .collect();
    let p0 = origin.to_light_years_f64();
    let fields = galaxy.fields();
    let components: Vec<_> = fields.component_ids().collect();
    // Per layer, the count per unit ln r at each radius: r³ × each ray's solid angle × the density
    // of stars brighter than the cut there, summed over the rays.
    let mut per_ln_r = vec![vec![0.0; radii.len()]; CAPPED_LAYERS.len()];
    let mut densities = [0.0; MAX_COMPONENTS];
    #[expect(clippy::cast_precision_loss, reason = "a few thousand rays")]
    let weight = 4.0 * core::f64::consts::PI / resolution.rays as f64;
    for (i, &r) in radii.iter().enumerate() {
        let ago = tables.age_for(
            observer.time(),
            Span::from_seconds_f64(r * SECONDS_PER_JULIAN_YEAR).unwrap_or(Span::ZERO),
        );
        for (ray, direction) in rays.directions().enumerate() {
            let limit = cut.value() - distance_modulus(r) - rays.along(ray, r);
            let u = direction.components();
            let p = PointLy::new(p0[0] + r * u[0], p0[1] + r * u[1], p0[2] + r * u[2]);
            fields.densities(&p, &mut densities);
            for (l, &layer) in CAPPED_LAYERS.iter().enumerate() {
                if r > bounds[l] {
                    continue;
                }
                let band = MassBand::from(layer);
                let mut n = 0.0;
                for &id in &components {
                    let rho = densities[id.index()];
                    if rho <= 0.0 {
                        continue;
                    }
                    let share = galaxy.shares().component_share(band, fields.component(id));
                    n += rho
                        * share
                        * tables
                            .get_at(id, layer, &p)
                            .count_brighter_than(Magnitudes::new(limit), ago);
                }
                per_ln_r[l][i] += weight * r * r * r * n;
            }
        }
    }
    let step_ln = math::ln(10.0) * decades / f64::from(steps);
    Counts {
        bounds,
        radii,
        per_ln_r,
        step_ln,
    }
}

/// Each layer's cap for `observer` at `cut` (apparent V) at the standard resolution: see the
/// [module](self) documentation.
///
/// `tables` are the galaxy's, read at the observer's time through
/// [`LuminosityTables::age_for`]; `cache` is the caller's noise cache for the rays.
///
/// Measured at cut 7.95 (the eye's near the Sun), version 21, with R06.T5.d's pair-evolved
/// correction as refitted at 21. Its count excess raised C near the Sun from 6,764 ly and D from
/// 8,193; the refit moved only the nuclear disc's C, from 116 ly, and D, from 205. They stand
/// against the brainstorm's version-14 estimates (C 3,000, D 4,300 and E 10,000 ly near the Sun; a
/// few hundred to about 1,000 in the nuclear disc), which the tests do not pin (decided 2026-10-03
/// and 2026-10-04). Caps in ly (R06.T7, the slow test `caps_converge_in_rays`; T17 re-derives
/// them):
///
/// | Point (ly) | A | B | C | D | E |
/// | ---------- | - | - | - | - | - |
/// | Near the Sun (0, 26,000, 68) | 11 | 68 | 8,193 | 9,925 | 21,369 |
/// | Nuclear disc (0, 150, 0) | 13 | 17 | 127 | 225 | 362 |
/// | Solar circle (26,000, 0, 68) | 11 | 68 | 7,444 | 9,018 | 19,416 |
/// | Solar circle (−18,385, −18,385, 68) | 15 | 75 | 8,193 | 9,925 | 23,519 |
/// | Inner disc (0, 8,000, 0) | 26 | 100 | 8,193 | 12,023 | 17,641 |
/// | Above the Sun (0, 26,000, 2,000) | 1 | 62 | 14,563 | 14,563 | 46,010 |
///
/// C's cap rests on rare bright phases of pair channels whose fitted rate rests on a few of the
/// fit's systems (R06 Risks, "T5.d's pair-evolved light, as built").
///
/// # Panics
///
/// If the galaxy has no density component, which no galaxy is built without.
#[must_use]
pub fn layer_caps(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    layer_caps_at(
        galaxy,
        tables,
        envelope,
        observer,
        cut,
        CapResolution::STANDARD,
        cache,
    )
}

/// [`layer_caps`] at `resolution`.
///
/// # Panics
///
/// As [`layer_caps`].
#[must_use]
pub fn layer_caps_at(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    resolution: CapResolution,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    let counts = count(galaxy, tables, envelope, observer, cut, resolution, cache);
    let radii = &counts.radii;
    CAPPED_LAYERS
        .iter()
        .enumerate()
        .map(|(l, &layer)| {
            let bound = counts.bounds[l];
            // The count beyond each node, by the trapezoid rule from the bound inward.
            let mut beyond = 0.0;
            let mut radius = bound;
            let mut stated = 0.0;
            for i in (0..radii.len() - 1).rev() {
                if radii[i] >= bound {
                    continue;
                }
                let slice = f64::midpoint(counts.per_ln_r[l][i], counts.per_ln_r[l][i + 1])
                    * counts.step_ln;
                if beyond + slice >= 1.0 {
                    radius = radii[i + 1].min(bound);
                    stated = beyond;
                    break;
                }
                beyond += slice;
                radius = radii[i];
                stated = beyond;
            }
            LayerCap {
                layer,
                radius: LightYears::new(radius),
                rule_bound: LightYears::new(bound),
                expected_beyond: stated,
            }
        })
        .collect()
}

/// The expected count of each layer's stars brighter than `cut` beyond its cap in `caps`,
/// recounted at `resolution`: the check that a finer count agrees with the caps' own claim.
///
/// # Panics
///
/// As [`layer_caps`], and if `caps` does not hold a cap for every layer of [`CAPPED_LAYERS`].
#[expect(
    clippy::too_many_arguments,
    reason = "the caps' inputs, the resolution, the caps checked and the caller's cache"
)]
#[must_use]
pub fn expected_beyond_caps(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    resolution: CapResolution,
    caps: &[LayerCap],
    cache: &mut NoiseCache,
) -> Vec<f64> {
    let counts = count(galaxy, tables, envelope, observer, cut, resolution, cache);
    CAPPED_LAYERS
        .iter()
        .enumerate()
        .map(|(l, &layer)| {
            let cap = caps
                .iter()
                .find(|c| c.layer() == layer)
                .expect("a cap for every layer");
            counts.beyond(l, cap.radius().value())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::extinction::sightline;
    use crate::sky::testing::{milky_way_envelope, milky_way_tables};
    use crate::time::UniverseTime;

    fn observer_at(ly: [f64; 3]) -> Observer {
        Observer::new(
            GalacticPosition::from_light_years(ly).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("a valid observer")
    }

    fn caps_at(ly: [f64; 3]) -> Vec<LayerCap> {
        let (tables, envelope) = (milky_way_tables(), milky_way_envelope());
        let mut cache = NoiseCache::with_capacity(1 << 16);
        layer_caps(
            milky_way_galaxy(),
            tables,
            envelope,
            &observer_at(ly),
            Magnitudes::new(7.95),
            &mut cache,
        )
    }

    fn radius(caps: &[LayerCap], layer: Layer) -> f64 {
        caps.iter()
            .find(|c| c.layer() == layer)
            .expect("capped")
            .radius()
            .value()
    }

    fn print(name: &str, caps: &[LayerCap]) {
        for cap in caps {
            eprintln!(
                "{name} {:?}: cap {:.0} ly, rule {:.0} ly, beyond {:.3}",
                cap.layer(),
                cap.radius().value(),
                cap.rule_bound().value(),
                cap.expected_beyond()
            );
        }
    }

    /// The caps near the Sun and in the nuclear disc at the eye's cut, 7.4 + 0.45 + 0.1, against
    /// the gates of the 2026-10-03 decision: a factor of three of the brainstorm's version-14
    /// figures near the Sun is a sanity bracket, not a confirmation.
    #[test]
    fn caps_near_the_sun_and_in_the_nuclear_disc() {
        let sun = caps_at([0.0, 26_000.0, 68.0]);
        let nuclear = caps_at([0.0, 150.0, 0.0]);
        print("Sun", &sun);
        print("nuclear disc", &nuclear);
        for cap in sun.iter().chain(&nuclear) {
            assert!(cap.radius() <= cap.rule_bound(), "{cap:?}");
            assert!((0.0..1.0).contains(&cap.expected_beyond()), "{cap:?}");
        }
        assert!(radius(&sun, Layer::A) < 100.0);
        assert!(radius(&sun, Layer::B) < 100.0);
        // C's cap rests on rare bright phases of pair channels (R06.T5.d's count excess), which
        // plan 11's fixes move, so it keeps only its floor; the rule's bound, asserted above, is
        // its ceiling (decided 2026-10-04, "T5.d caps after the pair correction"). D and E keep
        // the brainstorm's brackets.
        let c = radius(&sun, Layer::C);
        assert!(c >= 1_000.0, "C: {c} ly");
        assert!(
            radius(&nuclear, Layer::C) < c,
            "C grows in the nuclear disc"
        );
        for (layer, brainstorm) in [(Layer::D, 4_300.0), (Layer::E, 10_000.0)] {
            let r = radius(&sun, layer);
            assert!(
                (brainstorm / 3.0..=brainstorm * 3.0).contains(&r),
                "{layer:?}: {r} ly against {brainstorm}"
            );
            assert!(
                radius(&nuclear, layer) < r,
                "{layer:?} grows in the nuclear disc"
            );
        }
        let e = radius(&nuclear, Layer::E);
        assert!(e < 1_500.0, "{e}");
    }

    #[test]
    fn a_rays_last_node_is_its_sightline() {
        let galaxy = milky_way_galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let mut cache = NoiseCache::with_capacity(1 << 14);
        let rays = RayExtinctions::measure(galaxy, &sun, 8, &mut cache);
        for (ray, &direction) in rays.directions().enumerate() {
            let end = sun
                .translated(crate::coords::GalacticDisplacement::new(
                    direction
                        .components()
                        .map(|c| c * FARTHEST_LY * crate::units::consts::METRES_PER_LIGHT_YEAR),
                ))
                .expect("in the cube");
            let line = sightline(
                galaxy.gas(),
                &sun,
                &end,
                NoiseMode::Realised,
                Quality::Full,
                &[],
                &mut cache,
            );
            let (a, b) = (rays.along(ray, FARTHEST_LY), line.a_v().value());
            assert!(((a - b) / b).abs() < 1e-12, "ray {ray}: {a} against {b}");
            assert!(rays.along(ray, 0.0).abs() < 1e-300);
        }
    }
}
