//! The provisional test planet: an Earth-sized, dry world of summed gradient-noise octaves, by
//! hand, for the descent spike (plan R05, Design note 12). R09's surface generator replaces it.
//!
//! It belongs to no universe and realises none of plan 14's figures: its goldens carry
//! [`crate::TEST_PLANET_VERSION`], which any change to its heights bumps. It is a single-peaked
//! Gaussian field, not Earth's continents and ocean floor, and has no ocean.
//!
//! # The figure and the height
//!
//! The datum is WGS 84's ellipsoid ([`Spheroid::WGS84`]). In the body-fixed frame with z along the
//! pole, M = diag(a, a, c) maps a unit direction d to the spheroid point M·d, and height is
//! measured along the spheroid's normal above it (Design note 5). The height at d is the sum of
//! the octaves of [`octaves`] evaluated at the point M·d in metres, each scaled to its RMS `σ_k`, in
//! the fixed order of their index; [`HeightSample::gradient`] is its gradient in body-fixed space
//! at M·d, from the noise's analytic gradient.
//!
//! The large octaves cover the sphere in a few lattice cells, so their realised variance differs
//! from the nominal one. Octaves 0 to 8 are therefore rescaled together, by
//! [`COARSE_RESCALE`], so that the realised standard deviation over a fixed point set (a Fibonacci
//! sphere of [`FIXED_POINTS`] points) is [`SIGMA_H_M`], as R09's coarse pass rescales to plan
//! 14's figures.
//!
//! # Levels
//!
//! Level n evaluates the octaves whose lattice spacing is at least four times the level's largest
//! vertex spacing on this figure ([`TestPlanet::octaves_at`]), fixed per level, never by the local
//! spacing, and never one finer than the 2 m band limit. The newest octave of a level is faded in
//! across the morph zone by the morph itself: a patch's morph target is its parent level's height,
//! which lacks that octave (Design notes 5 and 6), so the vertex shader's blend from own-level
//! height to morph target is the fade, and the height function needs no fade of its own (T3.b as
//! built).
//!
//! # Measured (T3.b, 2026-10-02)
//!
//! Over the fixed point set the realised `σ_h` is 2,509.3 m (2,506.6 m with ridges), against
//! [`SIGMA_H_M`]; the octaves finer than 35 km hold 0.179% of the variance, inside the
//! brainstorm's 0.1–0.2%. Perlin noise is only roughly band-limited (Lagae et al. 2010, "A survey
//! of procedural noise functions", Computer Graphics Forum 29, 2579), so the leakage above the
//! 2 m band limit was measured, not assumed: along 1,000 profiles of 128 m sampled at 0.125 m at
//! the finest level, detrended and Hann-windowed, the power at wavelengths under 2 m is
//! **0.149 mm RMS**, far below the 1 cm that would be a finding
//! (`the_leakage_above_the_band_limit_is_measured`, under `just test-slow`).
//!
//! # Cost (T3.c)
//!
//! `just bench -- test_planet` times the height and gradient at the 4,225 vertices of a level-19
//! patch (`benches/test_planet.rs`). **Provisional** (Design note 27), taken 2026-10-02 on the
//! development machine (Ryzen 7 3700X) with other lanes' builds running, load average 19 to 28:
//! **4.1 µs a point with the lattice cache, 7.6 µs without**, against the research estimate of
//! about 2 and 6 µs and the budget of 10 µs. The quiet-machine run is pending for the owner.
//!
//! # Ridges
//!
//! [`Ridges::On`] replaces octaves 8 to 12 with ridged terms, r = 1 − √(n² + ε²), centred on their
//! pinned mean [`RIDGE_MEAN`], scaled by their pinned RMS [`RIDGE_RMS`] and gated by a smooth mask
//! from octaves 2 and 3, because sharp crests are the worst case for pops; the spike runs with and
//! without them.

pub mod bound;
pub mod octaves;

use hyperion_base::Seed;

use crate::cube::PatchKey;
use crate::geometry::vertex_spacing;
use crate::noise::{LatticeCache, NOISE_BOUND, NOISE_RMS, Octave, gradient_noise};
use crate::num::assert_finite;

pub use crate::spheroid::Spheroid;

/// The height at a point and its gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightSample {
    /// The height above the spheroid along its normal, metres.
    pub height_m: f64,
    /// The height field's gradient in body-fixed space at the spheroid point, metres per metre.
    pub gradient: [f64; 3],
}

/// Whether octaves 8 to 12 are ridged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ridges {
    /// Plain gradient noise in every octave.
    Off,
    /// Ridged terms in octaves 8 to 12, gated by a mask from octaves 2 and 3.
    On,
}

/// The test planet: its figure, its seed and whether it is ridged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestPlanet {
    figure: Spheroid,
    seed: Seed,
    ridges: Ridges,
}

/// The test planet the spike draws: WGS 84's figure, a fixed seed, no ridges.
pub const TEST_PLANET: TestPlanet = TestPlanet {
    figure: Spheroid::WGS84,
    seed: Seed::new(0x7465_7374_706c_616e),
    ridges: Ridges::Off,
};

/// Earth's hypsometric standard deviation, metres, the test planet's realised `σ_h` (Design note
/// 12).
///
/// From Earth2014's TBI layer (topography over land, bathymetry under the oceans, the ice
/// surface over ice sheets; Hirt and Rexer 2015, "Earth2014: 1 arc-min shape, topography, bedrock
/// and ice-sheet models – Available as gridded data and degree-10,800 spherical harmonics",
/// International Journal of Applied Earth Observation and Geoinformation 39, 103, whose Table 2
/// gives a standard deviation of 2,508.3 m about a mean of −2,385.0 m): 2,508.2 m about its area-weighted mean of −2,381.6 m, the square root of the degree
/// variances summed over degrees 1 to 2160, computed once (2026-10-02) with pyshtools 4.14.1 from
/// `Earth2014.TBI2014.degree2160.bshc` (`ddfe.curtin.edu.au/models/Earth2014/data_5min/`
/// `shcs_to2160/`), by this script:
///
/// ```text
/// import numpy as np, pyshtools as sh
/// from pyshtools.datasets.Earth import Earth2014 as E
/// c = E.tbi(lmax=2160)
/// p = c.spectrum(unit='per_l', convention='power')   # mean square per degree
/// print(np.sqrt(p[1:].sum()))                        # 2508.2
/// ```
///
/// The bedrock layer (BED) gives 2,420.0 m, and the design's estimate was 2.45 km. The same
/// spectrum's slope is −1.80 between degrees 10 and 1000, "near ℓ⁻²" as Balmino 1993 has it, and
/// 0.18% of its variance lies between 35 and 18.5 km (degree 1144 to the model's 2160), 105 m RMS,
/// against the octave table's 0.19% below 35 km.
pub const SIGMA_H_M: f64 = 2_508.2;

/// The last octave rescaled with the coarse ones.
const LAST_COARSE_OCTAVE: u8 = 8;

/// The factor octaves 0 to 8 are scaled by, so that the realised `σ_h` over the fixed point set is
/// [`SIGMA_H_M`]; pinned, and recomputed by a test.
pub const COARSE_RESCALE: f64 = 1.055_655_072_702_052_2;

/// The number of points in the fixed point set.
pub const FIXED_POINTS: u32 = 10_000;

/// The ridged octaves.
const RIDGED: std::ops::RangeInclusive<u8> = 8..=12;

/// The ridges' rounding of the crest, ε, in units of the noise.
const RIDGE_EPSILON: f64 = 0.05;

/// The mean of r = 1 − √(n² + ε²) over space, pinned and recomputed by a test.
pub const RIDGE_MEAN: f64 = 0.7692;

/// The RMS of r − [`RIDGE_MEAN`] over space, pinned and recomputed by a test.
pub const RIDGE_RMS: f64 = 0.1488;

/// The largest |r − [`RIDGE_MEAN`]| of a ridge term r = 1 − √(n² + ε²), with |n| at most
/// [`NOISE_BOUND`].
fn ridge_reach() -> f64 {
    let crest = 1.0 - RIDGE_EPSILON;
    let trough = 1.0 - (NOISE_BOUND * NOISE_BOUND + RIDGE_EPSILON * RIDGE_EPSILON).sqrt();
    crate::num::max((crest - RIDGE_MEAN).abs(), (trough - RIDGE_MEAN).abs())
}

/// The set of octaves a level evaluates: octaves 0 to `count − 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OctaveSet {
    count: u8,
}

impl OctaveSet {
    /// The number of octaves, which are 0 to `count − 1`.
    #[must_use]
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Whether octave `k` is in the set.
    #[must_use]
    pub const fn contains(self, k: u8) -> bool {
        k < self.count
    }
}

impl TestPlanet {
    /// This planet with ridges on or off.
    #[must_use]
    pub const fn with_ridges(self, ridges: Ridges) -> Self {
        Self { ridges, ..self }
    }

    /// The datum.
    #[must_use]
    pub const fn figure(&self) -> Spheroid {
        self.figure
    }

    /// Whether octaves 8 to 12 are ridged.
    #[must_use]
    pub const fn ridges(&self) -> Ridges {
        self.ridges
    }

    /// The octaves level `level` evaluates: those whose lattice spacing is at least four times
    /// the level's largest vertex spacing on the equatorial radius.
    ///
    /// # Panics
    ///
    /// If `level` is above [`crate::cube::MAX_LEVEL`].
    #[must_use]
    pub fn octaves_at(&self, level: u8) -> OctaveSet {
        let largest = vertex_spacing(self.figure.equatorial_radius_m, level).max_m;
        let count = (0..=octaves::FINEST_OCTAVE)
            .take_while(|&k| octaves::spacing_m(k) >= 4.0 * largest)
            .count();
        OctaveSet {
            count: u8::try_from(count).expect("at most 22 octaves"),
        }
    }

    /// The RMS height octave `k` contributes, metres: its nominal `σ_k`, times [`COARSE_RESCALE`]
    /// for octaves 0 to 8.
    #[must_use]
    pub fn sigma_m(&self, k: u8) -> f64 {
        let nominal = octaves::nominal_sigma_m(k);
        if k <= LAST_COARSE_OCTAVE {
            nominal * COARSE_RESCALE
        } else {
            nominal
        }
    }

    /// The largest |height| octave `k` can contribute, metres.
    fn octave_bound_m(&self, k: u8) -> f64 {
        if self.ridges == Ridges::On && RIDGED.contains(&k) {
            self.sigma_m(k) * ridge_reach() / RIDGE_RMS
        } else {
            self.sigma_m(k) * NOISE_BOUND / NOISE_RMS
        }
    }

    /// The lowest and highest height level `level` can reach anywhere, metres: the sum of its
    /// octaves' certified bounds, for culling (Design note 8).
    #[must_use]
    pub fn height_range_m(&self, level: u8) -> (f64, f64) {
        let set = self.octaves_at(level);
        let mut reach = 0.0;
        for k in 0..set.count() {
            reach += self.octave_bound_m(k);
        }
        (-reach, reach)
    }

    /// Sets `cache` to cover the lattice corners every query of a bake of `key` reads, at the
    /// patch's own level and its parent's (the morph targets): each octave's box about the patch's
    /// centre, out to its farthest corner (the patch's corners bound it, its edges being great
    /// circles of the cube; a margin of a tenth covers the double-resolution normals' and the
    /// spheroid's slight excess). The values a bake reads never depend on it.
    ///
    /// # Panics
    ///
    /// Never: a patch has 64 quads a side.
    pub fn cover_patch(&self, cache: &mut LatticeCache, key: PatchKey) {
        let quads = u8::try_from(crate::cube::PATCH_QUADS).expect("64 fits a u8");
        let centre = self.figure.point(key.vertex_dir(quads / 2, quads / 2));
        let mut reach = 0.0_f64;
        for (x, y) in [(0, 0), (quads, 0), (0, quads), (quads, quads)] {
            let p = self.figure.point(key.vertex_dir(x, y));
            let d = [0, 1, 2].map(|a| p[a] - centre[a]);
            reach = crate::num::max(reach, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        }
        let radius = reach * 1.1;
        for k in 0..self.octaves_at(key.level()).count() {
            cache.cover(&self.octave(k), centre, radius);
        }
    }

    /// Octave `k` of this planet.
    fn octave(&self, k: u8) -> Octave {
        octaves::octave(self.seed, k)
    }

    /// The height at the unit direction `dir` (body-fixed) at level `level`, and its gradient.
    ///
    /// `cache` is the caller's, one per bake; the result is independent of what it holds.
    ///
    /// # Panics
    ///
    /// If `dir` is not finite, or a height is not finite (a bug).
    #[must_use]
    pub fn height(&self, dir: [f64; 3], level: u8, cache: &mut LatticeCache) -> HeightSample {
        self.sum_octaves(dir, self.octaves_at(level).count(), cache)
    }

    /// The height of octaves 0 to `count − 1` alone at `dir`, metres: for the tests' per-octave
    /// statistics.
    #[cfg(test)]
    fn partial_height(&self, dir: [f64; 3], count: u8, cache: &mut LatticeCache) -> f64 {
        self.sum_octaves(dir, count, cache).height_m
    }

    /// The sum of octaves 0 to `count − 1` at `dir`, in index order, and its gradient.
    fn sum_octaves(&self, dir: [f64; 3], count: u8, cache: &mut LatticeCache) -> HeightSample {
        let p = self.figure.point(dir);
        let mask = if self.ridges == Ridges::On && count > *RIDGED.start() {
            Some(self.ridge_mask(p, cache))
        } else {
            None
        };
        let mut height = 0.0;
        let mut gradient = [0.0; 3];
        for k in 0..count {
            let octave = self.octave(k);
            let (n, grad_n) = gradient_noise(p, &octave, cache);
            let sigma = self.sigma_m(k);
            let (value, grad) = match mask {
                Some((w, grad_w)) if RIDGED.contains(&k) => {
                    let root = (n * n + RIDGE_EPSILON * RIDGE_EPSILON).sqrt();
                    let r = (1.0 - root - RIDGE_MEAN) / RIDGE_RMS;
                    let dr = -n / root / RIDGE_RMS;
                    let scale = sigma;
                    (
                        scale * w * r,
                        [0, 1, 2].map(|a| scale * (grad_w[a] * r + w * dr * grad_n[a])),
                    )
                }
                _ => {
                    let scale = sigma / NOISE_RMS;
                    (scale * n, grad_n.map(|g| scale * g))
                }
            };
            height += value;
            for a in 0..3 {
                gradient[a] += grad[a];
            }
        }
        HeightSample {
            height_m: assert_finite(height),
            gradient: gradient.map(assert_finite),
        }
    }

    /// The ridges' mask at `p` (metres) and its gradient: a smoothstep of
    /// s = ½ + (n₂ + n₃) ÷ (4 `σ_noise`), clamped to [0, 1].
    fn ridge_mask(&self, p: [f64; 3], cache: &mut LatticeCache) -> (f64, [f64; 3]) {
        let (n2, g2) = gradient_noise(p, &self.octave(2), cache);
        let (n3, g3) = gradient_noise(p, &self.octave(3), cache);
        let s = 0.5 + (n2 + n3) / (4.0 * NOISE_RMS);
        if s <= 0.0 {
            (0.0, [0.0; 3])
        } else if s >= 1.0 {
            (1.0, [0.0; 3])
        } else {
            let w = s * s * (3.0 - 2.0 * s);
            let dw = 6.0 * s * (1.0 - s) / (4.0 * NOISE_RMS);
            (w, [0, 1, 2].map(|a| dw * (g2[a] + g3[a])))
        }
    }
}

/// The test planet as the bake's height source: infallible, its cache a [`LatticeCache`] that
/// [`TestPlanet::cover_patch`] prepares, and a patch's height range its level's.
impl crate::patch::HeightSource for TestPlanet {
    type Cache = LatticeCache;
    type Error = std::convert::Infallible;

    fn figure(&self) -> Spheroid {
        self.figure
    }

    fn height(
        &self,
        cache: &mut LatticeCache,
        dir: [f64; 3],
        level: u8,
    ) -> Result<HeightSample, Self::Error> {
        Ok(TestPlanet::height(self, dir, level, cache))
    }

    fn level_bound_m(&self, level: u8) -> f64 {
        TestPlanet::level_bound_m(self, level)
    }

    fn height_range_m(&self, key: PatchKey) -> (f64, f64) {
        TestPlanet::height_range_m(self, key.level())
    }

    fn prepare_cache(&self, cache: &mut LatticeCache, key: PatchKey) {
        self.cover_patch(cache, key);
    }
}

/// The fixed point set the coarse octaves' variance is measured over: a Fibonacci sphere of
/// [`FIXED_POINTS`] unit directions, point i at z = 1 − (2i + 1) ÷ N and longitude i times the
/// golden angle π (3 − √5), which samples the sphere evenly by area.
#[must_use]
pub fn fixed_point_set() -> Vec<[f64; 3]> {
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let n = f64::from(FIXED_POINTS);
    (0..FIXED_POINTS)
        .map(|i| {
            let i = f64::from(i);
            let z = 1.0 - (2.0 * i + 1.0) / n;
            let r = (1.0 - z * z).sqrt();
            let (sin, cos) = hyperion_base::math::sin_cos(golden * i);
            [r * cos, r * sin, z]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::{Face, MAX_LEVEL, PatchKey, unit_dir};
    use crate::geometry::BAND_LIMIT_M;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn random_dir(rng: &mut Lcg) -> [f64; 3] {
        loop {
            let p = [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0);
            let r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            if r2 > 1e-6 && r2 <= 1.0 {
                return unit_dir(p);
            }
        }
    }

    #[test]
    fn wgs84s_polar_radius_is_nimas() {
        assert!((Spheroid::WGS84.polar_radius_m - 6_356_752.314_245).abs() < 1e-6);
    }

    #[test]
    fn each_level_evaluates_the_octaves_four_spacings_above_it() {
        let mut previous = 0;
        for level in 0..=MAX_LEVEL {
            let set = TEST_PLANET.octaves_at(level);
            let largest = vertex_spacing(Spheroid::WGS84.equatorial_radius_m, level).max_m;
            for k in 0..=octaves::FINEST_OCTAVE {
                assert_eq!(
                    set.contains(k),
                    octaves::spacing_m(k) >= 4.0 * largest,
                    "level {level} octave {k}"
                );
                if set.contains(k) {
                    assert!(octaves::spacing_m(k) > BAND_LIMIT_M);
                }
            }
            assert!(set.count() >= previous);
            previous = set.count();
        }
        // An Earth's finest level, 19, evaluates every octave.
        assert_eq!(TEST_PLANET.octaves_at(19).count(), 22);
    }

    #[test]
    fn the_gradient_agrees_with_a_central_difference() {
        let mut rng = Lcg::new(0x6772_6164);
        let mut cache = LatticeCache::new();
        for planet in [TEST_PLANET, TEST_PLANET.with_ridges(Ridges::On)] {
            for _ in 0..200 {
                let d = random_dir(&mut rng);
                let sample = planet.height(d, 19, &mut cache);
                // A step of about 1 cm along a random direction.
                let t = random_dir(&mut rng);
                let step = 1.5e-9;
                let plus = unit_dir([0, 1, 2].map(|a| d[a] + step * t[a]));
                let minus = unit_dir([0, 1, 2].map(|a| d[a] - step * t[a]));
                let figure = planet.figure();
                let (pp, pm) = (figure.point(plus), figure.point(minus));
                let predicted: f64 = (0..3).map(|a| sample.gradient[a] * (pp[a] - pm[a])).sum();
                let measured = planet.height(plus, 19, &mut cache).height_m
                    - planet.height(minus, 19, &mut cache).height_m;
                let scale = predicted.abs().max(1e-3);
                assert!(
                    (measured - predicted).abs() <= 1e-3 * scale,
                    "{d:?}: {measured} vs {predicted}"
                );
            }
        }
    }

    #[test]
    fn heights_lie_in_their_levels_range() {
        let mut rng = Lcg::new(0x7261_6e67);
        let mut cache = LatticeCache::new();
        for planet in [TEST_PLANET, TEST_PLANET.with_ridges(Ridges::On)] {
            for level in [0, 4, 10, 19] {
                let (low, high) = planet.height_range_m(level);
                for _ in 0..300 {
                    let h = planet
                        .height(random_dir(&mut rng), level, &mut cache)
                        .height_m;
                    assert!(
                        low <= h && h <= high,
                        "level {level}: {h} outside {low}..{high}"
                    );
                }
            }
        }
    }

    #[test]
    fn heights_are_independent_of_the_cache_and_repeatable() {
        let key = PatchKey::new(Face::NegY, 12, 1_234, 3_210).unwrap();
        let dirs: Vec<[f64; 3]> = (0..=64)
            .step_by(8)
            .flat_map(|y| (0..=64).step_by(8).map(move |x| key.vertex_dir(x, y)))
            .collect();
        let mut warm = LatticeCache::new();
        let centre = TEST_PLANET.figure().point(key.vertex_dir(32, 32));
        for k in 0..=octaves::FINEST_OCTAVE {
            warm.cover(&octaves::octave(TEST_PLANET.seed, k), centre, 2_000.0);
        }
        let warm = std::cell::RefCell::new(warm);
        hyperion_testkit::order::assert_order_independent(&dirs, |d| {
            TEST_PLANET.height(*d, 12, &mut warm.borrow_mut())
        });
        for d in &dirs {
            assert_eq!(
                TEST_PLANET.height(*d, 12, &mut warm.borrow_mut()),
                TEST_PLANET.height(*d, 12, &mut LatticeCache::new())
            );
        }
    }

    #[test]
    fn ridges_change_only_octaves_8_to_12() {
        let ridged = TEST_PLANET.with_ridges(Ridges::On);
        let mut rng = Lcg::new(0x7269_6467);
        let mut cache = LatticeCache::new();
        let coarse_level = (0..=MAX_LEVEL)
            .take_while(|&l| TEST_PLANET.octaves_at(l).count() <= 8)
            .last()
            .unwrap();
        let mut differed = false;
        for _ in 0..100 {
            let d = random_dir(&mut rng);
            assert_eq!(
                TEST_PLANET.height(d, coarse_level, &mut cache),
                ridged.height(d, coarse_level, &mut cache)
            );
            differed |= TEST_PLANET.height(d, 19, &mut cache) != ridged.height(d, 19, &mut cache);
        }
        assert!(differed);
    }

    /// The coarse octaves' rescale, recomputed: the factor that brings the realised standard
    /// deviation over the fixed point set to `σ_h`, with the finer octaves at their nominal `σ_k`.
    fn recomputed_coarse_rescale() -> f64 {
        let points = fixed_point_set();
        let mut cache = LatticeCache::new();
        let coarse: Vec<Octave> = (0..=LAST_COARSE_OCTAVE)
            .map(|k| octaves::octave(TEST_PLANET.seed, k))
            .collect();
        let (mut sum, mut sum_sq) = (0.0, 0.0);
        for d in &points {
            let p = TEST_PLANET.figure().point(*d);
            let mut h = 0.0;
            for o in &coarse {
                h += octaves::nominal_sigma_m(o.index()) / NOISE_RMS
                    * gradient_noise(p, o, &mut cache).0;
            }
            sum += h;
            sum_sq += h * h;
        }
        let n = f64::from(FIXED_POINTS);
        let variance = sum_sq / n - (sum / n) * (sum / n);
        let mut fine = 0.0;
        for k in LAST_COARSE_OCTAVE + 1..=octaves::FINEST_OCTAVE {
            let s = octaves::nominal_sigma_m(k);
            fine += s * s;
        }
        ((SIGMA_H_M * SIGMA_H_M - fine) / variance).sqrt()
    }

    #[test]
    #[ignore = "slow: recomputes the coarse octaves' rescale over the fixed point set"]
    fn the_coarse_rescale_is_pinned() {
        let c = recomputed_coarse_rescale();
        println!("COARSE_RESCALE recomputed: {c:?}");
        assert!(
            (c / COARSE_RESCALE - 1.0).abs() < 1e-12,
            "{c} vs {COARSE_RESCALE}"
        );
    }

    #[test]
    #[ignore = "slow: measures the ridges' mean and RMS over 10^6 points"]
    #[expect(
        clippy::many_single_char_names,
        reason = "an octave, a count and a ridge term"
    )]
    fn the_ridge_moments_are_pinned() {
        let o = octaves::octave(TEST_PLANET.seed, 10);
        let mut cache = LatticeCache::new();
        let mut rng = Lcg::new(0x7269_6467);
        let n = 1_000_000_u32;
        let (mut sum, mut sum_sq) = (0.0, 0.0);
        for _ in 0..n {
            let p = TEST_PLANET.figure().point(random_dir(&mut rng));
            let v = gradient_noise(p, &o, &mut cache).0;
            let r = 1.0 - (v * v + RIDGE_EPSILON * RIDGE_EPSILON).sqrt();
            sum += r;
            sum_sq += r * r;
        }
        let count = f64::from(n);
        let mean = sum / count;
        let rms = (sum_sq / count - mean * mean).sqrt();
        println!("ridge mean {mean}, rms {rms}");
        assert!((mean - RIDGE_MEAN).abs() < 5e-4, "{mean} vs {RIDGE_MEAN}");
        assert!((rms - RIDGE_RMS).abs() < 5e-4, "{rms} vs {RIDGE_RMS}");
    }

    #[test]
    #[ignore = "slow: heights and gradients at 10^5 points, both ridge settings"]
    fn heights_and_gradients_are_finite() {
        let mut rng = Lcg::new(0x6669_6e69);
        let mut cache = LatticeCache::new();
        for planet in [TEST_PLANET, TEST_PLANET.with_ridges(Ridges::On)] {
            for _ in 0..50_000 {
                // `height` asserts every value finite; the sample is its check.
                let s = planet.height(random_dir(&mut rng), 19, &mut cache);
                assert!(s.height_m.is_finite() && s.gradient.iter().all(|g| g.is_finite()));
            }
        }
    }

    /// An in-place radix-2 FFT of (`re`, `im`), whose length is a power of two.
    #[expect(
        clippy::many_single_char_names,
        reason = "the textbook names of an FFT's indices and twiddle"
    )]
    fn fft(re: &mut [f64], im: &mut [f64]) {
        let n = re.len();
        let mut j = 0;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            #[expect(clippy::cast_precision_loss, reason = "a length below 2^53")]
            let angle = -2.0 * core::f64::consts::PI / len as f64;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    #[expect(clippy::cast_precision_loss, reason = "an index below 2^53")]
                    let (s, c) = hyperion_base::math::sin_cos(angle * k as f64);
                    let (a, b) = (start + k, start + k + len / 2);
                    let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }

    #[test]
    #[ignore = "slow: the spectral leakage above the band limit, along 1,000 profiles"]
    fn the_leakage_above_the_band_limit_is_measured() {
        // 1,000 profiles of 1,024 samples at 0.125 m (128 m each) along random directions, at
        // the finest level; each is detrended, Hann-windowed and transformed, and the power at
        // wavelengths under 2 m (bins above 64) is summed, corrected for the window's mean
        // square of 3/8.
        const N: usize = 1024;
        const STEP_M: f64 = 0.125;
        let mut rng = Lcg::new(0x6c65_616b);
        let mut cache = LatticeCache::new();
        let radius = Spheroid::WGS84.equatorial_radius_m;
        let mut total = 0.0;
        let profiles = 1_000;
        for _ in 0..profiles {
            let d0 = random_dir(&mut rng);
            let t = random_dir(&mut rng);
            // The tangent part of t, as a unit vector.
            let along = t[0] * d0[0] + t[1] * d0[1] + t[2] * d0[2];
            let tangent = unit_dir([0, 1, 2].map(|a| t[a] - along * d0[a]));
            let mut re = vec![0.0; N];
            let mut im = vec![0.0; N];
            for (i, h) in re.iter_mut().enumerate() {
                #[expect(clippy::cast_precision_loss, reason = "an index below 2^53")]
                let s = i as f64 * STEP_M / radius;
                let dir = unit_dir([0, 1, 2].map(|a| d0[a] + s * tangent[a]));
                *h = TEST_PLANET.height(dir, 19, &mut cache).height_m;
            }
            // Detrend by the line through the ends, then window.
            let (first, last) = (re[0], re[N - 1]);
            for (i, h) in re.iter_mut().enumerate() {
                #[expect(clippy::cast_precision_loss, reason = "an index below 2^53")]
                let x = i as f64 / (N - 1) as f64;
                let w = 0.5 - 0.5 * hyperion_base::math::cos(2.0 * core::f64::consts::PI * x);
                *h = (*h - first - (last - first) * x) * w;
            }
            fft(&mut re, &mut im);
            let mut power = 0.0;
            for k in 65..N / 2 {
                power += 2.0 * (re[k] * re[k] + im[k] * im[k]);
            }
            #[expect(clippy::cast_precision_loss, reason = "a length below 2^53")]
            let n = N as f64;
            total += power / (n * n) / (3.0 / 8.0);
        }
        let rms_mm = (total / f64::from(profiles)).sqrt() * 1000.0;
        println!("leakage above the 2 m band limit: {rms_mm} mm RMS");
        assert!(rms_mm.is_finite());
    }

    /// The sample mean and variance of `values`.
    fn moments(values: &[f64]) -> (f64, f64) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a sample count is far below 2^53"
        )]
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
        (mean, var)
    }

    #[test]
    #[ignore = "slow: the realised σ_h, the fine octaves' share and each octave's mean"]
    fn the_spectrum_is_realised() {
        for planet in [TEST_PLANET, TEST_PLANET.with_ridges(Ridges::On)] {
            let points = fixed_point_set();
            let mut cache = LatticeCache::new();
            let all = planet.octaves_at(19).count();
            let mut totals = Vec::with_capacity(points.len());
            let mut per_octave: Vec<Vec<f64>> = vec![Vec::new(); usize::from(all)];
            for d in &points {
                // Each octave alone, as the difference of two levels' sets would be costly: the
                // height with octaves 0..=k less that with 0..k is octave k's contribution.
                let mut previous = 0.0;
                for k in 0..all {
                    let h = planet.partial_height(*d, k + 1, &mut cache);
                    per_octave[usize::from(k)].push(h - previous);
                    previous = h;
                }
                totals.push(previous);
            }
            let (_, total_var) = moments(&totals);
            let sigma = total_var.sqrt();
            println!(
                "{:?}: realised σ_h {sigma} m against {SIGMA_H_M}",
                planet.ridges()
            );
            if planet.ridges() == Ridges::Off {
                assert!((sigma / SIGMA_H_M - 1.0).abs() < 0.01);
                // The octaves finer than 35 km hold 0.1–0.2% of the variance.
                let fine: f64 = (0..all)
                    .filter(|&k| octaves::spacing_m(k) < 35_000.0)
                    .map(|k| moments(&per_octave[usize::from(k)]).1)
                    .sum();
                println!("fine share {}", fine / total_var);
                assert!(
                    (0.001..=0.002).contains(&(fine / total_var)),
                    "{}",
                    fine / total_var
                );
            }
            // Each octave fine enough to have many independent cells on the sphere has a zero
            // sample mean within three standard errors, ridged octaves included; octaves 0–5 span
            // the sphere in a few cells, so their samples are not independent.
            for k in 6..all {
                let (mean, var) = moments(&per_octave[usize::from(k)]);
                let se = (var / f64::from(FIXED_POINTS)).sqrt();
                assert!(mean.abs() < 3.0 * se, "octave {k}: mean {mean}, s.e. {se}");
            }
        }
    }
}
