//! The test planet's noise basis: 3D improved Perlin gradient noise with its analytic gradient
//! (plan R05, Design note 12).
//!
//! Perlin's improved noise (Perlin 2002, "Improving Noise", SIGGRAPH; the reference
//! implementation at mrl.cs.nyu.edu/~perlin/noise/) with its quintic fade 6t⁵ − 15t⁴ + 10t³ and its
//! 16-entry gradient table: the twelve cube-edge directions (±1, ±1, 0), (±1, 0, ±1), (0, ±1, ±1),
//! padded with (1, 1, 0), (0, −1, 1), (−1, 1, 0) and (0, −1, −1). The table is indexed by the top
//! four bits of one Threefry word per lattice corner, so a uniform index gives an exactly zero-mean
//! gradient: the twelve sum to zero and so do the four padding vectors. (A multiply-high of a word
//! by 12 would not be exactly uniform, since 2⁶⁴ is not a multiple of 12.) Perlin's permutation
//! table is not used: the corner's word comes from a counter-based stream, so the value at a point
//! never depends on which points were computed first.
//!
//! # Keys
//!
//! An octave places the point in its own lattice: q = R · p ÷ λ + o, with p the point in metres,
//! λ the octave's lattice spacing, R its fixed rotation (rational entries, so no transcendental
//! is needed) and o its offset (see [`Octave`]). Lattice corner (i, j, k) of octave n reads one
//! word of the stream `Stream::open(seed, TEST_PLANET, ObjectKey::galaxy_item(object))`:
//!
//! | Integer        | Where                               | Bits                                 |
//! | -------------- | ----------------------------------- | ------------------------------------ |
//! | i              | the object word, bits 32–63         | i's 32-bit two's complement          |
//! | j              | the object word, bits 0–31          | j's 32-bit two's complement          |
//! | n, the octave  | the draw number (word index), 32–36 | n, 0 to 31                           |
//! | k              | the draw number, bits 0–31          | k's 32-bit two's complement          |
//!
//! The gradient's index is the word's top four bits. The test planet's octave offsets
//! (`test_planet::octaves`) read words 2⁴⁰ + 4n + axis of object 0, far above every corner's draw
//! number, which is below 2³⁷. Every lattice index of a body up to Earth's
//! size at the finest octave is far inside ±2³¹ (about ±1.4 × 10⁶ at λ = 4.77 m), which
//! [`gradient_noise`] asserts.
//!
//! # The cache
//!
//! Hashing a corner costs one Threefry block. A [`LatticeCache`] belongs to its caller, one per
//! bake (the crate keeps no cache of its own): it holds, per octave, the gradient indices of a box
//! of lattice corners that [`LatticeCache::cover`] sets, each filled on first use. The value of a
//! filled entry is the pure function above, so a bake is independent of the order it fills the
//! cache in, which the tests check. Corners outside every box are hashed each time.
//!
//! # Bounds
//!
//! [`NOISE_BOUND`], B, is the certified maximum of |noise| and [`NOISE_RMS`], `σ_noise`, its
//! root-mean-square; both are pinned here and recomputed by tests, the first by
//! `noise_bound_certified` under `just test-slow` (Design note 15).

use hyperion_base::Seed;
use hyperion_base::rng::{ObjectKey, Stream};

use crate::tags::TEST_PLANET;

/// The certified maximum of |noise| over all points and gradient choices, B (Design note 15).
///
/// Within one lattice cell the noise at a point is `Σ_c w_c(x) g_c · (x − c)` over the eight
/// corners c, and each corner's gradient is chosen independently, so the largest value any choice
/// gives at x is `F(x) = Σ_c w_c(x) max_g g · (x − c)`. F has the cube's full symmetry about the
/// cell's centre (the twelve edge vectors are closed under the axes' permutations and signs, and
/// the padding repeats four of them), so its maximum is searched over the 1/48 of the cell with
/// x ≤ y ≤ z ≤ ½, on a grid of step 1/256, and raised by a Lipschitz margin: F's gradient is at
/// most √3 · 1.875 · √6 + √2 ≤ 9.37 (1.875 the fade's steepest slope, √6 the largest |g| |x − c|)
/// times the half-diagonal of a grid step, √3 ÷ 512. The noise's minimum is −B, the table being
/// closed under negation.
pub const NOISE_BOUND: f64 = 1.0681;

/// The root-mean-square of the noise over space and gradient choices, `σ_noise` (Design note 15),
/// measured over 10⁶ points and pinned.
pub const NOISE_RMS: f64 = 0.2701;

/// The gradient table, indexed by a corner word's top four bits (Perlin 2002's `grad`).
const GRADIENTS: [[f64; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

/// The largest |lattice index| a corner may have: its 32-bit two's complement is its key.
const MAX_LATTICE_INDEX: f64 = 2_147_483_000.0;

/// The largest number of corners a cache holds for one octave; a box above it is not cached.
const MAX_BOX_CORNERS: u64 = 1 << 22;

/// One octave of noise: its index, its lattice spacing, rotation and offset, and the planet's
/// seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Octave {
    index: u8,
    spacing_m: f64,
    rotation: [[f64; 3]; 3],
    offset: [f64; 3],
    seed: Seed,
}

impl Octave {
    /// Octave `index` of the planet of `seed`, with lattice spacing `spacing_m` metres, rows of
    /// `rotation` and lattice `offset`.
    ///
    /// `rotation` must be orthonormal, to 10⁻¹² in each row's products: the cache's covering box
    /// ([`LatticeCache::cover`]) relies on a ball staying a ball of the same radius.
    ///
    /// # Panics
    ///
    /// If `index` is above 31, `spacing_m` is not finite and positive, or `rotation` is not
    /// orthonormal.
    #[must_use]
    pub fn new(
        index: u8,
        spacing_m: f64,
        rotation: [[f64; 3]; 3],
        offset: [f64; 3],
        seed: Seed,
    ) -> Self {
        assert!(index < 32, "an octave index is 0 to 31, got {index}");
        assert!(
            spacing_m.is_finite() && spacing_m > 0.0,
            "an octave's lattice spacing must be finite and positive, got {spacing_m}"
        );
        for (a, row_a) in rotation.iter().enumerate() {
            for (b, row_b) in rotation.iter().enumerate() {
                let dot = row_a[0] * row_b[0] + row_a[1] * row_b[1] + row_a[2] * row_b[2];
                let expected = if a == b { 1.0 } else { 0.0 };
                assert!(
                    (dot - expected).abs() <= 1e-12,
                    "an octave's rotation must be orthonormal, got {rotation:?}"
                );
            }
        }
        Self {
            index,
            spacing_m,
            rotation,
            offset,
            seed,
        }
    }

    /// The octave's index, 0 to 31.
    #[must_use]
    pub const fn index(&self) -> u8 {
        self.index
    }

    /// The lattice spacing λ, metres.
    #[must_use]
    pub const fn spacing_m(&self) -> f64 {
        self.spacing_m
    }

    /// The point `p_m` (metres, body-fixed) in the octave's lattice coordinates: R · p ÷ λ + o,
    /// each row's products summed in axis order.
    #[must_use]
    fn lattice_point(&self, p_m: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = p_m;
        let row = |n: usize| {
            let r = self.rotation[n];
            (r[0] * x + r[1] * y + r[2] * z) / self.spacing_m + self.offset[n]
        };
        [row(0), row(1), row(2)]
    }

    /// A lattice gradient (∂/∂q) taken back to body-fixed metres: Rᵀ · g ÷ λ.
    #[must_use]
    fn gradient_to_body(&self, g: [f64; 3]) -> [f64; 3] {
        let r = &self.rotation;
        let column = |n: usize| (r[0][n] * g[0] + r[1][n] * g[1] + r[2][n] * g[2]) / self.spacing_m;
        [column(0), column(1), column(2)]
    }

    /// The gradient index of lattice corner `corner`: the top four bits of its word (see the
    /// module documentation).
    #[must_use]
    fn corner_gradient(&self, corner: [i64; 3]) -> u8 {
        let [i, j, k] = corner.map(twos_complement_32);
        let object = (u64::from(i) << 32) | u64::from(j);
        let draw = (u64::from(self.index) << 32) | u64::from(k);
        let word =
            Stream::open(self.seed, TEST_PLANET, ObjectKey::galaxy_item(object)).word_at(draw);
        u8::try_from(word >> 60).expect("four bits fit a u8")
    }
}

/// The low 32 bits of `n`'s two's complement.
#[must_use]
fn twos_complement_32(n: i64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the key is the index's 32-bit two's complement, by design; indices are asserted \
                  within ±2^31"
    )]
    let low = n as u32;
    low
}

/// One octave's box of cached corners: its lowest corner, its extent along each axis, and the
/// gradient index of each corner, [`UNFILLED`] until first use.
#[derive(Debug, Clone, PartialEq)]
struct CornerBox {
    /// The octave the box was covered for. A box serves only an octave of the same index and
    /// seed, so a cache passed from one planet or seed to another never returns the other's
    /// gradients; the seed alone is compared on each lookup, which is cheap, and debug builds
    /// check that the whole octave matches (one seed with two octave tables in one cache is the
    /// caller's error).
    octave: Octave,
    low: [i64; 3],
    extent: [u64; 3],
    indices: Vec<u8>,
}

/// An entry of a [`CornerBox`] not yet filled; gradient indices are 0 to 15.
const UNFILLED: u8 = u8::MAX;

impl CornerBox {
    /// The entry of `corner`, or `None` outside the box.
    #[must_use]
    fn slot(&self, corner: [i64; 3]) -> Option<usize> {
        let mut flat = 0_u64;
        for ((&c, &low), &extent) in corner.iter().zip(&self.low).zip(&self.extent) {
            // A negative offset, which the conversion refuses, is a corner below the box.
            let offset = u64::try_from(c - low).ok()?;
            if offset >= extent {
                return None;
            }
            flat = flat * extent + offset;
        }
        usize::try_from(flat).ok()
    }
}

/// The caller's cache of lattice-corner gradients, one per bake (see the module documentation).
///
/// It also keeps a planet's octave table, built once per seed rather than once per point
/// ([`take_octaves`](Self::take_octaves)): each octave's offset is two Threefry blocks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LatticeCache {
    /// Per octave index, the box [`cover`](Self::cover) set, if any.
    boxes: Vec<Option<CornerBox>>,
    /// The octave table last built, with the seed it was built for.
    octaves: Option<(Seed, Vec<Octave>)>,
}

impl LatticeCache {
    /// An empty cache, which caches nothing until [`cover`](Self::cover) is called.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Caches `octave`'s corners within `radius_m` metres of `centre_m` (body-fixed), replacing
    /// any box it held for the octave. A box of more than about four million corners is not
    /// cached; its corners are hashed each time.
    ///
    /// # Panics
    ///
    /// If `centre_m` or `radius_m` is not finite, `radius_m` is negative, or the ball reaches a
    /// lattice index beyond ±2³¹.
    pub fn cover(&mut self, octave: &Octave, centre_m: [f64; 3], radius_m: f64) {
        assert!(
            centre_m.iter().all(|c| c.is_finite()) && radius_m.is_finite() && radius_m >= 0.0,
            "a covered ball must be finite, got {centre_m:?} and {radius_m}"
        );
        let centre = octave.lattice_point(centre_m);
        // The rotation is orthonormal, so the ball stays a ball of radius r ÷ λ; one corner more
        // on each side covers the cells its edge touches.
        let reach = radius_m / octave.spacing_m + 1.0;
        let low = centre.map(|c| lattice_index(c - reach));
        let high = centre.map(|c| lattice_index(c + reach) + 1);
        let extent: [u64; 3] = [0, 1, 2].map(|a| {
            u64::try_from(high[a] - low[a] + 1).expect("a box's high corner is above its low")
        });
        let corners = extent[0]
            .checked_mul(extent[1])
            .and_then(|n| n.checked_mul(extent[2]));
        let slot = usize::from(octave.index);
        if self.boxes.len() <= slot {
            self.boxes.resize(slot + 1, None);
        }
        self.boxes[slot] = match corners {
            Some(n) if n <= MAX_BOX_CORNERS => Some(CornerBox {
                octave: *octave,
                low,
                extent,
                indices: vec![UNFILLED; usize::try_from(n).expect("a capped box fits memory")],
            }),
            _ => None,
        };
    }

    /// The octave table of the planet of `seed`, built by `build` unless this cache holds it, and
    /// taken out of the cache so that the caller can read it while it lends the cache to
    /// [`gradient_noise`]; give it back with [`restore_octaves`](Self::restore_octaves).
    ///
    /// `build` must be a pure function of `seed`, the same on every call: the table is reused for
    /// every later call with that seed, so the octaves are those `build` would give.
    pub fn take_octaves(&mut self, seed: Seed, build: impl FnOnce() -> Vec<Octave>) -> Vec<Octave> {
        match self.octaves.take() {
            Some((held, table)) if held == seed => table,
            _ => build(),
        }
    }

    /// Gives back a table [`take_octaves`](Self::take_octaves) took for `seed`.
    pub fn restore_octaves(&mut self, seed: Seed, table: Vec<Octave>) {
        self.octaves = Some((seed, table));
    }

    /// The gradient index of `corner` of `octave`, from the cache where it covers the corner.
    fn gradient(&mut self, octave: &Octave, corner: [i64; 3]) -> u8 {
        let cached = self
            .boxes
            .get_mut(usize::from(octave.index))
            .and_then(Option::as_mut)
            .filter(|b| {
                debug_assert!(
                    b.octave.seed != octave.seed || b.octave == *octave,
                    "a cache box covered for {:?} was read for {octave:?}",
                    b.octave
                );
                b.octave.seed == octave.seed
            })
            .and_then(|b| b.slot(corner).map(|s| &mut b.indices[s]));
        match cached {
            Some(entry) if *entry != UNFILLED => *entry,
            Some(entry) => {
                let g = octave.corner_gradient(corner);
                *entry = g;
                g
            }
            None => octave.corner_gradient(corner),
        }
    }
}

/// The lattice index of the cell containing lattice coordinate `c`, ⌊c⌋.
#[must_use]
fn lattice_index(c: f64) -> i64 {
    let floor = c.floor();
    assert!(
        floor.abs() <= MAX_LATTICE_INDEX,
        "lattice coordinate {c} is beyond the key's 32 bits"
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the floor is an integer asserted within ±2^31"
    )]
    let index = floor as i64;
    index
}

/// The quintic fade 6t⁵ − 15t⁴ + 10t³ and its derivative 30t⁴ − 60t³ + 30t², Horner's form.
#[must_use]
fn fade(t: f64) -> (f64, f64) {
    let value = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let slope = 30.0 * t * t * (t * (t - 2.0) + 1.0);
    (value, slope)
}

/// The noise of `octave` at `p_m` (body-fixed metres) and its gradient there, per metre.
///
/// The value lies within ±[`NOISE_BOUND`]. The eight corners' contributions are summed in the
/// fixed order of their index, (0, 0, 0), (1, 0, 0), (0, 1, 0), …, (1, 1, 1).
///
/// # Panics
///
/// If `p_m` is not finite, or lies so far out that a lattice index leaves ±2³¹.
#[must_use]
pub fn gradient_noise(p_m: [f64; 3], octave: &Octave, cache: &mut LatticeCache) -> (f64, [f64; 3]) {
    assert!(
        p_m.iter().all(|c| c.is_finite()),
        "a noise point must be finite, got {p_m:?}"
    );
    let q = octave.lattice_point(p_m);
    let cell = q.map(lattice_index);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a lattice index within ±2^31 is exact in f64"
    )]
    let frac: [f64; 3] = [0, 1, 2].map(|a| q[a] - cell[a] as f64);
    let fades = frac.map(fade);

    let mut value = 0.0;
    let mut grad = [0.0; 3];
    for corner in 0..8_u8 {
        let bit = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let at = [0, 1, 2].map(|a| cell[a] + i64::from(bit[a]));
        let g = GRADIENTS[usize::from(cache.gradient(octave, at))];
        let d = [0, 1, 2].map(|a| frac[a] - f64::from(bit[a]));
        let dot = g[0] * d[0] + g[1] * d[1] + g[2] * d[2];
        // The corner's weight along each axis, f(t) towards the far corner and 1 − f(t) towards
        // the near one, and its derivative.
        let w = [0, 1, 2].map(|a| {
            let (f, df) = fades[a];
            if bit[a] == 1 { (f, df) } else { (1.0 - f, -df) }
        });
        let weight = w[0].0 * w[1].0 * w[2].0;
        value += weight * dot;
        grad[0] += w[0].1 * w[1].0 * w[2].0 * dot + weight * g[0];
        grad[1] += w[0].0 * w[1].1 * w[2].0 * dot + weight * g[1];
        grad[2] += w[0].0 * w[1].0 * w[2].1 * dot + weight * g[2];
    }
    (value, octave.gradient_to_body(grad))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use std::cell::RefCell;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    /// A rational rotation: the Cayley transform of (1, 2, 2) ÷ 3 (entries over 9).
    const ROTATION: [[f64; 3]; 3] = [
        [-7.0 / 9.0, 4.0 / 9.0, 4.0 / 9.0],
        [4.0 / 9.0, -1.0 / 9.0, 8.0 / 9.0],
        [4.0 / 9.0, 8.0 / 9.0, -1.0 / 9.0],
    ];

    fn octave(index: u8, spacing_m: f64) -> Octave {
        Octave::new(
            index,
            spacing_m,
            ROTATION,
            [0.137, 0.512, 0.873],
            Seed::new(0x7465_7374),
        )
    }

    fn random_point(rng: &mut Lcg, half_width_m: f64) -> [f64; 3] {
        [0, 1, 2].map(|_| (rng.next_f64() * 2.0 - 1.0) * half_width_m)
    }

    #[test]
    fn the_gradient_table_has_zero_mean() {
        let sum = GRADIENTS
            .iter()
            .fold([0.0; 3], |s, g| [s[0] + g[0], s[1] + g[1], s[2] + g[2]]);
        assert!(sum.iter().all(|&c| c.abs() <= 0.0), "{sum:?}");
    }

    #[test]
    fn the_rotation_is_orthonormal() {
        for (a, row_a) in ROTATION.iter().enumerate() {
            for (b, row_b) in ROTATION.iter().enumerate() {
                let dot: f64 = row_a.iter().zip(row_b).map(|(x, y)| x * y).sum();
                let expected = if a == b { 1.0 } else { 0.0 };
                assert!((dot - expected).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn the_gradient_agrees_with_a_central_difference() {
        let o = octave(3, 1_000.0);
        let mut cache = LatticeCache::new();
        let mut rng = Lcg::new(0x6772_6164);
        let h = 1e-4;
        for _ in 0..10_000 {
            let p = random_point(&mut rng, 50_000.0);
            let (_, grad) = gradient_noise(p, &o, &mut cache);
            let scale = grad.iter().map(|g| g.abs()).fold(0.0, f64::max).max(1e-6);
            for axis in 0..3 {
                let mut lo = p;
                let mut hi = p;
                lo[axis] -= h;
                hi[axis] += h;
                let numeric = (gradient_noise(hi, &o, &mut cache).0
                    - gradient_noise(lo, &o, &mut cache).0)
                    / (2.0 * h);
                assert!(
                    (numeric - grad[axis]).abs() <= 1e-6 * scale,
                    "{p:?} axis {axis}: {numeric} vs {}",
                    grad[axis]
                );
            }
        }
    }

    #[test]
    fn values_stay_within_the_bound_with_zero_mean() {
        let o = octave(5, 37.0);
        let mut cache = LatticeCache::new();
        let mut rng = Lcg::new(0x6d65_616e);
        let n = 100_000_u32;
        let (mut sum, mut sum_sq) = (0.0, 0.0);
        for _ in 0..n {
            let (v, _) = gradient_noise(random_point(&mut rng, 1.0e6), &o, &mut cache);
            assert!(v.abs() <= NOISE_BOUND, "{v}");
            sum += v;
            sum_sq += v * v;
        }
        let count = f64::from(n);
        let mean = sum / count;
        let rms = (sum_sq / count).sqrt();
        let standard_error = rms / count.sqrt();
        assert!(
            mean.abs() < 3.0 * standard_error,
            "mean {mean}, s.e. {standard_error}"
        );
        assert!(
            (rms / NOISE_RMS - 1.0).abs() < 0.02,
            "rms {rms} vs {NOISE_RMS}"
        );
    }

    #[test]
    fn the_cache_is_independent_of_its_fill_order() {
        let o = octave(7, 10.0);
        let centre = [12_345.0, -6_789.0, 4_321.0];
        let mut rng = Lcg::new(0x6f72_6465);
        let points: Vec<[f64; 3]> = (0..2_000)
            .map(|_| {
                let d = random_point(&mut rng, 40.0);
                [centre[0] + d[0], centre[1] + d[1], centre[2] + d[2]]
            })
            .collect();
        let mut warm = LatticeCache::new();
        warm.cover(&o, centre, 70.0);
        let warm = RefCell::new(warm);
        assert_order_independent(&points, |p| gradient_noise(*p, &o, &mut warm.borrow_mut()));
        // And the warm cache gives what no cache gives.
        assert_order_independent(&points, |p| {
            gradient_noise(*p, &o, &mut LatticeCache::new())
        });
        for p in &points {
            assert_eq!(
                gradient_noise(*p, &o, &mut warm.borrow_mut()),
                gradient_noise(*p, &o, &mut LatticeCache::new())
            );
        }
    }

    #[test]
    fn a_cache_filled_for_one_octave_never_serves_another() {
        let a = octave(7, 10.0);
        let b = Octave::new(7, 10.0, ROTATION, [0.137, 0.512, 0.873], Seed::new(99));
        let centre = [1_000.0, 2_000.0, 3_000.0];
        let mut cache = LatticeCache::new();
        cache.cover(&a, centre, 50.0);
        let mut rng = Lcg::new(0x7365_6564);
        for _ in 0..500 {
            let d = random_point(&mut rng, 40.0);
            let p = [centre[0] + d[0], centre[1] + d[1], centre[2] + d[2]];
            let _ = gradient_noise(p, &a, &mut cache);
            assert_eq!(
                gradient_noise(p, &b, &mut cache),
                gradient_noise(p, &b, &mut LatticeCache::new())
            );
        }
    }

    #[test]
    fn octaves_and_seeds_draw_different_lattices() {
        let mut cache = LatticeCache::new();
        let p = [1_234.5, 2_345.6, 3_456.7];
        let a = gradient_noise(p, &octave(4, 100.0), &mut cache);
        let b = gradient_noise(p, &octave(5, 100.0), &mut cache);
        let other_seed = Octave::new(4, 100.0, ROTATION, [0.137, 0.512, 0.873], Seed::new(1));
        let c = gradient_noise(p, &other_seed, &mut cache);
        assert_ne!(a, b);
        assert_ne!(a, c);
        // The identity rotation is accepted too, and lattice points give zero.
        let flat = Octave::new(0, 1.0, IDENTITY, [0.0; 3], Seed::new(9));
        assert!(gradient_noise([3.0, -4.0, 5.0], &flat, &mut cache).0.abs() <= 0.0);
    }

    /// `F(x) = Σ_c w_c(x) max_g g · (x − c)`, the largest noise any gradient choice gives at x in
    /// the unit cell.
    fn envelope(x: [f64; 3]) -> f64 {
        let fades = x.map(|t| fade(t).0);
        let mut total = 0.0;
        for corner in 0..8_u8 {
            let bit = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
            let d = [0, 1, 2].map(|a| x[a] - f64::from(bit[a]));
            let best = GRADIENTS
                .iter()
                .map(|g| g[0] * d[0] + g[1] * d[1] + g[2] * d[2])
                .fold(f64::NEG_INFINITY, f64::max);
            let weight: f64 = (0..3)
                .map(|a| {
                    if bit[a] == 1 {
                        fades[a]
                    } else {
                        1.0 - fades[a]
                    }
                })
                .product();
            total += weight * best;
        }
        total
    }

    #[test]
    #[ignore = "slow: certifies NOISE_BOUND by a grid search over the cell"]
    fn noise_bound_certified() {
        let steps = 256_u32;
        let half = steps / 2;
        let mut best = 0.0_f64;
        for i in 0..=half {
            for j in i..=half {
                for k in j..=half {
                    let x = [i, j, k].map(|n| f64::from(n) / f64::from(steps));
                    best = best.max(envelope(x));
                }
            }
        }
        let lipschitz = 3.0_f64.sqrt() * 1.875 * 6.0_f64.sqrt() + 2.0_f64.sqrt();
        let margin = lipschitz * 3.0_f64.sqrt() / (2.0 * f64::from(steps));
        let certified = best + margin;
        println!("grid maximum {best}, margin {margin}, certified {certified}");
        assert!(certified <= NOISE_BOUND, "{certified} > {NOISE_BOUND}");
        assert!(
            NOISE_BOUND - certified < 1e-3,
            "NOISE_BOUND {NOISE_BOUND} is loose: {certified}"
        );
    }

    #[test]
    #[ignore = "slow: measures NOISE_RMS and the ensemble mean over 10^6 points"]
    fn noise_rms_measured() {
        let o = octave(2, 1_000.0);
        let mut cache = LatticeCache::new();
        let mut rng = Lcg::new(0x726d_7331);
        let n = 1_000_000_u32;
        let (mut sum, mut sum_sq) = (0.0, 0.0);
        for _ in 0..n {
            let (v, _) = gradient_noise(random_point(&mut rng, 1.0e8), &o, &mut cache);
            assert!(v.abs() <= NOISE_BOUND);
            sum += v;
            sum_sq += v * v;
        }
        let count = f64::from(n);
        let mean = sum / count;
        let rms = (sum_sq / count).sqrt();
        println!("mean {mean}, rms {rms}");
        assert!(mean.abs() < 3.0 * rms / count.sqrt(), "mean {mean}");
        assert!((rms - NOISE_RMS).abs() < 5e-4, "rms {rms} vs {NOISE_RMS}");
    }
}
