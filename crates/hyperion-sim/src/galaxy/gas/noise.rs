//! The gas's lattice noise: a mean-preserving log-normal factor on the smooth field (plan 07,
//! Design notes 7–9 and 17).
//!
//! # The lattice
//!
//! Five octaves, with lattice spacings of 1,024, 512, 256, 128 and 64 ly
//! ([`OCTAVE_WAVELENGTHS_LY`]). Each lattice point of each octave holds one standard normal, drawn
//! on [`tags::GAS_NOISE`] with [`ObjectKey::galaxy_item`] of a word that packs the octave and the
//! point's three lattice coordinates, so that every point is a stream of its own and the two words
//! a normal costs never interleave between points. The spacings are powers of two, so the lattice
//! planes are faces of the light-year grid: a position's lattice cell and its place in it follow
//! from its integer light-year cell by an arithmetic shift, and its fraction `t` across the lattice
//! cell is the remainder plus the metre offset, divided by a power of two.
//!
//! # One octave
//!
//! Within a lattice cell, an octave's value is
//!
//! ```text
//! g_k(x) = Σ_i w_i G_i ÷ √(Σ_i w_i²)
//! ```
//!
//! over the cell's eight corners, with trilinear weights `w_i` built from the quintic fade `s(t) =
//! t³ (6t² − 15t + 10)`. A sum of independent standard normals with weights whose squares sum to 1
//! is itself standard normal, so dividing by the root of the summed squares makes `g_k` exactly
//! N(0, 1) at every point; plain interpolation would lose up to seven eighths of the variance at a
//! cell's centre and with it the mean of the factor below. `Σ w_i²` factorises per axis as
//! `(1 − s)² + s²`. The value is continuous across cell faces, since there the weights of the far
//! face vanish, and at a lattice point it is that point's normal exactly.
//!
//! # The factor
//!
//! The octaves combine as `g = Σ a_k g_k` with `a_k² ∝ λ_k^⅔` — more variance at large scales, as
//! turbulence has — normalised to `Σ a_k² = 1` ([`OCTAVE_VARIANCES`]), so `g` is N(0, 1) too, and
//!
//! ```text
//! F(x) = exp(σ_ln g(x) − σ_ln² ÷ 2)
//! ```
//!
//! has expectation exactly 1 at every point. Smoothing to a scale ℓ ([`SmoothingScale::AtLeast`])
//! keeps the octaves with `λ_k ≥ ℓ` and uses `σ_eff² = σ_ln² Σ_kept a_k²` in place of `σ_ln²`:
//! that is the exact conditional mean of the full factor given the coarse octaves, so it is still
//! mean-preserving, and it is what a supernova shell reads at its own scale (Design note 9).
//!
//! Every operation before the final [`math::exp`] is `+ − × ÷` or `sqrt`, which are IEEE-exact, on
//! a fraction `t` that is an offset divided by a power of two, so the factor is the same bits on
//! every machine. Outside the root cube, where the lattice has no points, the factor is its mean,
//! 1.
//!
//! # The phase draw
//!
//! Two more octaves, numbers 5 and 6 of the lattice word, with spacings of 32 and 8 ly
//! ([`PHASE_OCTAVE_WAVELENGTHS_LY`]), carry the normal [`phase_normal`] that picks which of a
//! parcel's four phases a point lies in (ruling 103 of 2026-09-22; [`super::phase`]). They are
//! built exactly as the factor's octaves are, variance-normalised and weighted `a_k² ∝ λ_k^⅔`
//! between the two, so the normal is exactly N(0, 1) at every point. They are on the same tag and
//! the same lattice words as the factor's, the octave's number keeping every word distinct, so one
//! [`NoiseCache`] serves both and never returns one lattice's normal for the other's. No
//! [`SmoothingScale`] reaches them: a scale keeps at most the factor's five octaves.
//!
//! # Caching
//!
//! The sim holds no caches, so [`NoiseCache`] is the caller's: a direct-mapped table of lattice
//! normals keyed by the packed word, which consecutive steps along a line of sight share most of.
//! It changes no result, only what they cost.

use crate::Seed;
use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The lattice spacings of the five octaves, light-years, coarsest first (Design note 8).
///
/// Nothing in the field varies faster than 64 ly, which is what bounds the extinction integral's
/// step. The octave's index in this array is its number in the lattice word, which reserves
/// numbers 5–15 for octaves a later generator version may add.
pub const OCTAVE_WAVELENGTHS_LY: [u32; 5] = [1_024, 512, 256, 128, 64];

/// The number of the factor's octaves.
const OCTAVES: usize = OCTAVE_WAVELENGTHS_LY.len();

/// Each octave's share `a_k²` of the variance of `g`: `λ_k^⅔ ÷ Σ λ^⅔`, so that they sum to 1 and
/// each is `2^(−⅔)` of the one before (Design note 8).
///
/// Written out, because `powf` is not `const`; a unit test holds each against [`math::powf`] and
/// their sum against 1.
pub const OCTAVE_VARIANCES: [f64; OCTAVES] = [
    0.410_795_556_178_959_8,
    0.258_784_984_216_571_7,
    0.163_024_324_505_585_6,
    0.102_698_889_044_739_95,
    0.064_696_246_054_142_93,
];

/// Each octave's amplitude `a_k`, the square root of its [`OCTAVE_VARIANCES`] entry as `f64::sqrt`
/// rounds it (unit-tested bit for bit).
const OCTAVE_AMPLITUDES: [f64; OCTAVES] = [
    0.640_933_347_688_322_2,
    0.508_709_135_180_971_3,
    0.403_762_708_166_053_4,
    0.320_466_673_844_161_1,
    0.254_354_567_590_485_64,
];

/// The lattice spacings of the phase draw's two octaves, light-years, coarser first (ruling 103 of
/// 2026-09-22): numbers 5 and 6 of the lattice word, which Design note 8 reserved.
pub const PHASE_OCTAVE_WAVELENGTHS_LY: [u32; 2] = [32, 8];

/// The phase octaves' numbers in the lattice word.
const PHASE_OCTAVES: [usize; 2] = [OCTAVES, OCTAVES + 1];

/// Each phase octave's share of the phase normal's variance: `λ^⅔ ÷ Σ λ^⅔` over the two, `2^(10⁄3)
/// ÷ (2^(10⁄3) + 4)` and `4 ÷ (2^(10⁄3) + 4)` (unit-tested against [`math::powf`]).
pub const PHASE_OCTAVE_VARIANCES: [f64; 2] = [0.715_896_346_583_349_9, 0.284_103_653_416_650_1];

/// Each phase octave's amplitude, the square root of its [`PHASE_OCTAVE_VARIANCES`] entry as
/// `f64::sqrt` rounds it (unit-tested bit for bit).
const PHASE_OCTAVE_AMPLITUDES: [f64; 2] = [0.846_106_581_101_547_7, 0.533_013_745_992_211_9];

/// Each octave's spacing as a power of two, the factor's five and the phase draw's two, by the
/// octave's number in the lattice word: `λ = 2^LATTICE_LOG2[k]` ly.
const LATTICE_LOG2: [u32; OCTAVES + 2] = [10, 9, 8, 7, 6, 5, 3];

/// The bits of the lattice word each lattice coordinate takes (Design note 8).
const COORDINATE_BITS: u32 = 20;

/// What a lattice coordinate is offset by to make it unsigned: 2¹⁹, so that −2¹⁹ ≤ i < 2¹⁹ fits.
const COORDINATE_OFFSET: i64 = 1 << (COORDINATE_BITS - 1);

/// Where the octave's number starts in the lattice word: above the three coordinates.
const OCTAVE_SHIFT: u32 = 3 * COORDINATE_BITS;

const _: () = assert!(
    OCTAVES + 2 <= 16,
    "the lattice word holds an octave's number in four bits"
);

/// The word no lattice point has, which marks an empty slot of a [`NoiseCache`]: it would be
/// octave 0 at the coordinate −2¹⁹ on every axis, some 5 × 10⁸ ly outside the root cube.
const EMPTY_WORD: u64 = 0;

/// How much of the lattice noise a caller reads: all of it, or only the octaves at least as coarse
/// as a length (Design note 9).
///
/// Dropping the finer octaves and their variance is the conditional mean of the full field given
/// the coarse ones, so a smoothed field is still mean-preserving. The extinction integral under a
/// budget reads `AtLeast(2 Δ)` for its step `Δ`, so that the octaves its steps cannot resolve are
/// replaced by their mean and not aliased; a supernova shell reads its own scale; the hazard API
/// reads the full field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmoothingScale {
    /// Every octave, down to 64 ly.
    Full,
    /// The octaves whose spacing is at least this length. Above 1,024 ly none is kept and the
    /// factor is 1.
    AtLeast(LightYears),
}

impl SmoothingScale {
    /// How many octaves, coarsest first, the scale keeps: all five for [`Full`](Self::Full),
    /// those with a spacing of at least `ℓ` for [`AtLeast`](Self::AtLeast).
    #[must_use]
    pub fn kept_octaves(self) -> usize {
        match self {
            Self::Full => OCTAVES,
            Self::AtLeast(length) => OCTAVE_WAVELENGTHS_LY
                .iter()
                .take_while(|&&wavelength| f64::from(wavelength) >= length.value())
                .count(),
        }
    }

    /// `Σ_kept a_k²`: the share of the log-normal's variance the kept octaves carry, 1 for
    /// [`Full`](Self::Full) and less for a coarser scale. `σ_eff² = σ_ln² ×` this.
    #[must_use]
    pub fn kept_variance(self) -> f64 {
        OCTAVE_VARIANCES[..self.kept_octaves()]
            .iter()
            .fold(0.0, |sum, variance| sum + variance)
    }
}

/// A caller-owned cache of lattice normals: a direct-mapped table of a fixed number of entries,
/// keyed by the lattice word (Design note 17).
///
/// Consecutive steps along a line of sight share most of their lattice corners, which is where the
/// saving is: a lattice normal costs a Threefry block, a logarithm and a sine and cosine, and a
/// factor reads forty of them. A cache changes no result, only its cost: every value is
/// bit-identical with a cache of any capacity, none included, and after [`clear`](Self::clear).
/// It remembers the seed its entries were drawn for and empties itself when asked for another, so
/// one cache may serve several galaxies in turn. The default is a cache of no slots.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
///
/// let seed = Seed::new(7);
/// let p = GalacticPosition::from_light_years([26_000.3, 417.9, 12.5]).expect("inside i32");
/// let mut small = NoiseCache::with_capacity(1);
/// let mut large = NoiseCache::with_capacity(4_096);
/// let a = log_normal_factor(seed, &p, 2.3, SmoothingScale::Full, &mut small);
/// let b = log_normal_factor(seed, &p, 2.3, SmoothingScale::Full, &mut large);
/// assert_eq!(a, b);
/// ```
#[derive(Debug, Clone, Default)]
pub struct NoiseCache {
    /// The seed the entries were drawn for; `None` while the cache is empty.
    seed: Option<Seed>,
    /// `(word, normal)` per slot, [`EMPTY_WORD`] marking an empty one.
    slots: Vec<(u64, f64)>,
}

impl NoiseCache {
    /// An empty cache of `entries` slots. Zero slots is a cache that holds nothing, whose every
    /// lookup draws the normal afresh.
    #[must_use]
    pub fn with_capacity(entries: usize) -> Self {
        Self {
            seed: None,
            slots: vec![(EMPTY_WORD, 0.0); entries],
        }
    }

    /// The number of slots.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Empties every slot, keeping the allocation.
    pub fn clear(&mut self) {
        self.slots.fill((EMPTY_WORD, 0.0));
        self.seed = None;
    }

    /// Makes the cache hold `seed`'s normals, emptying it if it held another seed's.
    fn bind(&mut self, seed: Seed) {
        if self.seed != Some(seed) {
            self.clear();
            self.seed = Some(seed);
        }
    }

    /// The lattice normal of `word` under the seed the cache is bound to, from its slot or drawn
    /// into it.
    fn normal(&mut self, seed: Seed, word: u64) -> f64 {
        let Some(slot) = self.slot(word) else {
            return lattice_normal(seed, word);
        };
        let entry = &mut self.slots[slot];
        if entry.0 != word {
            *entry = (word, lattice_normal(seed, word));
        }
        entry.1
    }

    /// The slot `word` maps to, `None` for a cache of no slots: a multiplicative hash of the word,
    /// scaled onto the slots by a 128-bit product, so that neighbouring lattice points spread over
    /// the table whatever its size.
    fn slot(&self, word: u64) -> Option<usize> {
        let slots = u64::try_from(self.slots.len()).expect("a cache's size fits in 64 bits");
        if slots == 0 {
            return None;
        }
        let hash = u128::from(word.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let scaled = (hash * u128::from(slots)) >> 64;
        Some(usize::try_from(scaled).expect("a slot index is below the cache's size"))
    }
}

/// The lattice word of octave `octave` at the lattice coordinates `index`: the octave's number in
/// the top four bits and each coordinate, offset by 2¹⁹, in twenty bits below it, x highest (Design
/// note 8). The four bits hold sixteen octaves, of which this version uses the first seven: the
/// factor's five and the phase draw's two.
///
/// `None` unless `octave` is one of this version's and every lattice plane the coordinates name
/// lies inside the root cube, `±ROOT_HALF_WIDTH_LY`: the planes on the cube's far faces are inside,
/// since a position just below a face interpolates towards them, and at the finest spacing, the
/// phase draw's 8 ly, they are the coordinates ±8,192, far inside the twenty bits.
fn lattice_word(octave: usize, index: [i32; 3]) -> Option<u64> {
    let log2 = *LATTICE_LOG2.get(octave)?;
    let limit = i64::from(ROOT_HALF_WIDTH_LY) >> log2;
    let mut word = u64::try_from(octave).expect("one of seven octaves") << OCTAVE_SHIFT;
    for (shift, coordinate) in [2 * COORDINATE_BITS, COORDINATE_BITS, 0]
        .into_iter()
        .zip(index)
    {
        let coordinate = i64::from(coordinate);
        if coordinate.abs() > limit {
            return None;
        }
        // At most 8,193 from 0, well inside ±2¹⁹, so the offset coordinate is not negative.
        let unsigned = u64::try_from(coordinate + COORDINATE_OFFSET)
            .expect("a coordinate inside the root cube is inside twenty bits");
        word |= unsigned << shift;
    }
    Some(word)
}

/// The standard normal at the lattice point `word`: the first of a Box–Muller pair on its own
/// stream, [`tags::GAS_NOISE`] keyed by [`ObjectKey::galaxy_item`] of the word.
fn lattice_normal(seed: Seed, word: u64) -> f64 {
    Stream::open(seed, tags::GAS_NOISE, ObjectKey::galaxy_item(word)).standard_normal()
}

/// The quintic fade `s(t) = t³ (6t² − 15t + 10)`, which rises from 0 to 1 over `[0, 1]` with its
/// first and second derivatives 0 at both ends.
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Where `p` lies in octave `octave`'s lattice: the lattice coordinates of its cell's low corner
/// and its fraction `t ∈ [0, 1]` across the cell on each axis.
///
/// The coordinate is the light-year cell shifted right by the spacing's power of two, which is
/// floor division for negative cells too, and `t` is the cell's remainder plus the metre offset in
/// light-years, divided by the spacing: an offset divided by a power of two.
fn lattice_cell(p: &GalacticPosition, octave: usize) -> ([i32; 3], [f64; 3]) {
    let log2 = LATTICE_LOG2[octave];
    let spacing = f64::from(1_u32 << log2);
    let cell = p.cell().to_array();
    let offset = p.offset_metres();
    let mut index = [0_i32; 3];
    let mut t = [0.0; 3];
    for axis in 0..3 {
        index[axis] = cell[axis] >> log2;
        let remainder = cell[axis] - (index[axis] << log2);
        t[axis] = (f64::from(remainder) + offset[axis] / METRES_PER_LIGHT_YEAR) / spacing;
    }
    (index, t)
}

/// Octave `octave`'s value `g_k` at `p`, standard normal: the variance-normalised trilinear
/// interpolation of the lattice normals at the eight corners of `p`'s lattice cell, or `None` if
/// a corner lies outside the root cube.
fn octave_value(
    seed: Seed,
    octave: usize,
    p: &GalacticPosition,
    cache: &mut NoiseCache,
) -> Option<f64> {
    cache.bind(seed);
    let (index, t) = lattice_cell(p, octave);
    // The low corner's word, checked with the far corner so that every corner is inside the root
    // cube; the other seven add one to a coordinate, which cannot carry out of its twenty bits.
    lattice_word(octave, index.map(|i| i + 1))?;
    let low = lattice_word(octave, index)?;
    let s = t.map(fade);
    let weights = s.map(|s| [1.0 - s, s]);
    let mut sum = 0.0;
    for (i, wx) in (0_u64..).zip(&weights[0]) {
        for (j, wy) in (0_u64..).zip(&weights[1]) {
            for (k, wz) in (0_u64..).zip(&weights[2]) {
                let word = low + (i << (2 * COORDINATE_BITS)) + (j << COORDINATE_BITS) + k;
                sum += wx * wy * wz * cache.normal(seed, word);
            }
        }
    }
    let squares: f64 = weights
        .iter()
        .map(|&[low, high]| low * low + high * high)
        .product();
    Some(sum / squares.sqrt())
}

/// The log-normal factor `F = exp(σ g − σ_eff² ÷ 2)` of seed `seed`'s gas at `p`, with `σ` the
/// galaxy's `σ_ln` ([`GasParams::sigma_ln`](super::params::GasParams::sigma_ln)) and `g` the sum
/// of the octaves `scale` keeps, `σ_eff² = σ² Σ_kept a_k²` (module documentation).
///
/// Its expectation over seeds is exactly 1 at every point, for every `σ` and every scale; its
/// logarithm is normal with variance `σ_eff²`. Outside the root cube it is 1. `cache` changes what
/// the call costs, never what it returns.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
/// use hyperion_sim::units::LightYears;
///
/// let p = GalacticPosition::from_light_years([26_000.0, 0.0, 30.0]).expect("inside i32");
/// let mut cache = NoiseCache::with_capacity(64);
/// // One seed's gas is clumpy: the factor is its own at every point, often far from 1.
/// let full = log_normal_factor(Seed::new(1), &p, 2.3, SmoothingScale::Full, &mut cache);
/// assert!(full > 0.0);
/// // Smoothed beyond the coarsest octave, nothing is left of the noise.
/// let beyond = SmoothingScale::AtLeast(LightYears::new(2_048.0));
/// let smooth = log_normal_factor(Seed::new(1), &p, 2.3, beyond, &mut cache);
/// assert_eq!(smooth, 1.0);
/// ```
#[must_use]
pub fn log_normal_factor(
    seed: Seed,
    p: &GalacticPosition,
    sigma_ln: f64,
    scale: SmoothingScale,
    cache: &mut NoiseCache,
) -> f64 {
    if !p.in_root_cube() {
        return 1.0;
    }
    let kept = scale.kept_octaves();
    let mut g = 0.0;
    for (octave, amplitude) in OCTAVE_AMPLITUDES.iter().enumerate().take(kept) {
        // Every corner of a lattice cell holding a point of the root cube is inside it.
        let Some(value) = octave_value(seed, octave, p, cache) else {
            return 1.0;
        };
        g += amplitude * value;
    }
    let variance = sigma_ln * sigma_ln * scale.kept_variance();
    math::exp(sigma_ln * g - 0.5 * variance)
}

/// The phase draw's normal of seed `seed`'s gas at `p`: `g_u = Σ a_k g_k` over the two phase
/// octaves of 32 and 8 ly, exactly N(0, 1) at every point, and independent of the log-normal
/// factor's octaves (module documentation; ruling 103 of 2026-09-22).
///
/// [`GasField::state`](super::field::GasField::state) turns it into the uniform `u = Φ(g_u)` that
/// picks a point's phase. It depends on no [`SmoothingScale`], so a point's draw is the same at
/// every scale. Outside the root cube it is 0, the median. `cache` changes what the call costs,
/// never what it returns, and may be the one the factor reads.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::gas::noise::{NoiseCache, phase_normal};
///
/// let p = GalacticPosition::from_light_years([26_000.0, 0.0, 30.0]).expect("inside i32");
/// let mut cache = NoiseCache::with_capacity(64);
/// let g = phase_normal(Seed::new(1), &p, &mut cache);
/// // The same point and seed give the same bits, whatever the cache.
/// assert_eq!(g, phase_normal(Seed::new(1), &p, &mut NoiseCache::with_capacity(0)));
/// ```
#[must_use]
pub fn phase_normal(seed: Seed, p: &GalacticPosition, cache: &mut NoiseCache) -> f64 {
    if !p.in_root_cube() {
        return 0.0;
    }
    let mut g = 0.0;
    for (octave, amplitude) in PHASE_OCTAVES.into_iter().zip(PHASE_OCTAVE_AMPLITUDES) {
        // Every corner of a lattice cell holding a point of the root cube is inside it.
        let Some(value) = octave_value(seed, octave, p, cache) else {
            return 0.0;
        };
        g += amplitude * value;
    }
    g
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::{assert_same_bits, bits};
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, normal_cdf};

    use super::*;
    use crate::coords::LyCell;

    fn position(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("inside i32")
    }

    /// The variances are `λ^⅔` normalised, sum to 1, and the amplitudes are their square roots.
    #[test]
    fn the_octave_variances_sum_to_one_and_follow_the_two_thirds_law() {
        let total: f64 = OCTAVE_VARIANCES.iter().sum();
        assert!((total - 1.0).abs() < 1e-15, "{total}");
        let weights = OCTAVE_WAVELENGTHS_LY.map(|l| math::powf(f64::from(l), 2.0 / 3.0));
        let sum: f64 = weights.iter().sum();
        for k in 0..OCTAVES {
            let expected = weights[k] / sum;
            assert!(
                (OCTAVE_VARIANCES[k] / expected - 1.0).abs() < 1e-15,
                "octave {k}: {} against {expected}",
                OCTAVE_VARIANCES[k]
            );
            assert_same_bits(OCTAVE_AMPLITUDES[k], OCTAVE_VARIANCES[k].sqrt());
            assert_eq!(1_u32 << LATTICE_LOG2[k], OCTAVE_WAVELENGTHS_LY[k]);
        }
        let squares: f64 = OCTAVE_AMPLITUDES.iter().map(|a| a * a).sum();
        assert!((squares - 1.0).abs() < 1e-15, "{squares}");
    }

    /// Ruling 103: the phase draw's two octaves are 32 and 8 ly, numbers 5 and 6 of the word, with
    /// variances `λ^⅔` normalised over the two and amplitudes their square roots.
    #[test]
    fn the_phase_octaves_follow_the_two_thirds_law() {
        let weights = PHASE_OCTAVE_WAVELENGTHS_LY.map(|l| math::powf(f64::from(l), 2.0 / 3.0));
        let sum: f64 = weights.iter().sum();
        for k in 0..2 {
            let expected = weights[k] / sum;
            assert!(
                (PHASE_OCTAVE_VARIANCES[k] / expected - 1.0).abs() < 1e-15,
                "phase octave {k}: {} against {expected}",
                PHASE_OCTAVE_VARIANCES[k]
            );
            assert_same_bits(PHASE_OCTAVE_AMPLITUDES[k], PHASE_OCTAVE_VARIANCES[k].sqrt());
            assert_eq!(
                1_u32 << LATTICE_LOG2[PHASE_OCTAVES[k]],
                PHASE_OCTAVE_WAVELENGTHS_LY[k]
            );
        }
        let total: f64 = PHASE_OCTAVE_VARIANCES.iter().sum();
        assert!((total - 1.0).abs() < 1e-15, "{total}");
        // No smoothing scale reaches them.
        assert!(SmoothingScale::AtLeast(LightYears::new(1.0)).kept_octaves() <= OCTAVES);
    }

    /// At a fixed point the phase normal over 10⁵ seeds is standard normal by a Kolmogorov–Smirnov
    /// test, at a point 0.2–0.8 of the way across both phase octaves' cells; it is 0 outside the
    /// root cube; and at a lattice point of both octaves it is their normals' weighted sum.
    #[test]
    fn the_phase_normal_is_standard_normal_over_seeds() {
        let p = position([26_004.0, -404.0, 300.9]);
        for octave in PHASE_OCTAVES {
            let (_, t) = lattice_cell(&p, octave);
            assert!(t.iter().all(|t| (0.2..0.8).contains(t)), "{t:?}");
        }
        let mut cache = NoiseCache::with_capacity(64);
        let mut sample: Vec<f64> = (0..100_000_u64)
            .map(|n| phase_normal(Seed::new(0x0703_0000_0000_0000 | n), &p, &mut cache))
            .collect();
        let ks = ks_one_sample(&mut sample, normal_cdf);
        assert_p_value("the phase normal against N(0, 1)", ks.p_value, ALPHA);
        let outside = GalacticPosition::new(LyCell::new([65_536, 0, 0]), [0.0; 3]).unwrap();
        assert_same_bits(phase_normal(Seed::new(9), &outside, &mut cache), 0.0);
        let seed = Seed::new(0x0703_1a77);
        let corner = GalacticPosition::new(LyCell::new([64, -32, 96]), [0.0; 3]).unwrap();
        let expected = PHASE_OCTAVE_AMPLITUDES[0]
            * lattice_normal(seed, lattice_word(5, [2, -1, 3]).unwrap())
            + PHASE_OCTAVE_AMPLITUDES[1]
                * lattice_normal(seed, lattice_word(6, [8, -4, 12]).unwrap());
        assert_same_bits(phase_normal(seed, &corner, &mut cache), expected);
    }

    /// One cache shared between the factor and the phase normal, of any size, one entry included,
    /// changes neither: the octave's number keeps the two lattices' words apart.
    #[test]
    fn a_cache_shared_with_the_phase_draw_changes_no_result() {
        let seed = Seed::new(0x0703_cace);
        let points: Vec<GalacticPosition> = (0..200)
            .map(|i| {
                let i = f64::from(i);
                position([26_000.0 + 3.7 * i, -300.0 + 1.1 * i, 40.0 - 0.3 * i])
            })
            .collect();
        let run = |cache: &mut NoiseCache| -> Vec<u64> {
            points
                .iter()
                .flat_map(|p| {
                    [
                        bits(log_normal_factor(seed, p, 1.2, SmoothingScale::Full, cache)),
                        bits(phase_normal(seed, p, cache)),
                    ]
                })
                .collect()
        };
        let reference = run(&mut NoiseCache::with_capacity(0));
        for capacity in [1, 2, 7, 64, 4_096] {
            let mut cache = NoiseCache::with_capacity(capacity);
            assert_eq!(run(&mut cache), reference, "capacity {capacity}");
            assert_eq!(run(&mut cache), reference, "warm, capacity {capacity}");
        }
    }

    /// A scale keeps the octaves at least as coarse as itself, and their share of the variance.
    #[test]
    fn a_smoothing_scale_keeps_the_coarse_octaves() {
        let at = |ly: f64| SmoothingScale::AtLeast(LightYears::new(ly));
        assert_eq!(SmoothingScale::Full.kept_octaves(), 5);
        assert_same_bits(SmoothingScale::Full.kept_variance(), 1.0);
        assert_eq!(at(64.0).kept_octaves(), 5);
        assert_eq!(at(65.0).kept_octaves(), 4);
        assert_eq!(at(250.0).kept_octaves(), 3);
        assert_eq!(at(256.0).kept_octaves(), 3);
        assert_eq!(at(1_024.0).kept_octaves(), 1);
        assert_eq!(at(1_025.0).kept_octaves(), 0);
        assert_same_bits(at(1_025.0).kept_variance(), 0.0);
        let three: f64 = OCTAVE_VARIANCES[..3].iter().sum();
        assert_same_bits(at(250.0).kept_variance(), three);
        assert!((0.83..0.84).contains(&three));
    }

    /// The word packs the octave above x, y and z, each offset to unsigned, and refuses a plane
    /// outside the root cube but not one on its faces.
    #[test]
    fn the_lattice_word_packs_the_octave_and_the_coordinates() {
        let word = lattice_word(3, [-1, 0, 5]).unwrap();
        assert_eq!(word >> OCTAVE_SHIFT, 3);
        let field = |shift: u32| i64::try_from((word >> shift) & ((1 << COORDINATE_BITS) - 1));
        assert_eq!(field(40), Ok(COORDINATE_OFFSET - 1));
        assert_eq!(field(20), Ok(COORDINATE_OFFSET));
        assert_eq!(field(0), Ok(COORDINATE_OFFSET + 5));
        // The cube's faces are ±64 lattice cells at 1,024 ly and ±1,024 at 64 ly.
        assert!(lattice_word(0, [64, -64, 0]).is_some());
        assert!(lattice_word(0, [65, 0, 0]).is_none());
        assert!(lattice_word(0, [0, 0, -65]).is_none());
        assert!(lattice_word(4, [1_024, -1_024, 1_024]).is_some());
        assert!(lattice_word(4, [1_025, 0, 0]).is_none());
        // The phase draw's octaves 5 and 6 reach ±2,048 and ±8,192 cells; octave 7 is not built.
        assert!(lattice_word(5, [2_048, -2_048, 0]).is_some());
        assert!(lattice_word(5, [2_049, 0, 0]).is_none());
        assert!(lattice_word(6, [8_192, -8_192, 8_192]).is_some());
        assert!(lattice_word(6, [0, -8_193, 0]).is_none());
        assert!(lattice_word(OCTAVES + 2, [0, 0, 0]).is_none());
        // Distinct points and octaves have distinct words, and none is the empty slot's.
        let mut words = Vec::new();
        for octave in 0..OCTAVES + 2 {
            for x in -2..=2 {
                for y in -2..=2 {
                    for z in -2..=2 {
                        words.push(lattice_word(octave, [x, y, z]).unwrap());
                    }
                }
            }
        }
        let count = words.len();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), count);
        assert!(!words.contains(&EMPTY_WORD));
    }

    /// A position's lattice cell is the floor of its light-year coordinate over the spacing, on
    /// either side of 0, and its fraction runs from 0 at the low face.
    #[test]
    fn a_positions_lattice_cell_is_the_floor_division() {
        let p = GalacticPosition::new(
            LyCell::new([-1, 1_023, 1_024]),
            [
                0.5 * METRES_PER_LIGHT_YEAR,
                0.0,
                0.25 * METRES_PER_LIGHT_YEAR,
            ],
        )
        .unwrap();
        let (index, t) = lattice_cell(&p, 0);
        assert_eq!(index, [-1, 0, 1]);
        assert_same_bits(t[0], (1_023.0 + 0.5) / 1_024.0);
        assert_same_bits(t[1], 1_023.0 / 1_024.0);
        assert_same_bits(t[2], 0.25 / 1_024.0);
        let (index, t) = lattice_cell(&p, 4);
        assert_eq!(index, [-1, 15, 16]);
        assert_same_bits(t[0], 63.5 / 64.0);
    }

    /// At a lattice point an octave's value is the point's normal, bit for bit: the fade and the
    /// normalisation leave it alone.
    #[test]
    fn at_a_lattice_point_the_value_is_the_lattice_normal() {
        let seed = Seed::new(0x0700_0015e);
        let mut cache = NoiseCache::with_capacity(16);
        for (octave, &wavelength) in OCTAVE_WAVELENGTHS_LY.iter().enumerate() {
            let spacing = i32::try_from(wavelength).unwrap();
            for corner in [[3, -2, 0], [-7, 11, -1], [0, 0, 0]] {
                let cell = corner.map(|c| c * spacing);
                let p = GalacticPosition::new(LyCell::new(cell), [0.0; 3]).unwrap();
                let value = octave_value(seed, octave, &p, &mut cache).unwrap();
                let word = lattice_word(octave, corner).unwrap();
                assert_same_bits(value, lattice_normal(seed, word));
            }
        }
    }

    /// Across a lattice cell's face the value is continuous: two points 10⁻⁹ ly either side of it
    /// differ by under 10⁻¹², on every axis and at every octave.
    #[test]
    fn the_value_is_continuous_across_cell_faces() {
        let seed = Seed::new(0x0700_0c0a7);
        let mut cache = NoiseCache::with_capacity(64);
        let nudge = 1e-9 * METRES_PER_LIGHT_YEAR;
        for (octave, &wavelength) in OCTAVE_WAVELENGTHS_LY.iter().enumerate() {
            let spacing = i32::try_from(wavelength).unwrap();
            for axis in 0..3 {
                let mut face = [26_013, -417, 88];
                face[axis] = 5 * spacing;
                let mut below = face;
                below[axis] -= 1;
                let mut low = [0.37 * METRES_PER_LIGHT_YEAR; 3];
                let mut high = low;
                low[axis] = METRES_PER_LIGHT_YEAR - nudge;
                high[axis] = nudge;
                let a = GalacticPosition::new(LyCell::new(below), low).unwrap();
                let b = GalacticPosition::new(LyCell::new(face), high).unwrap();
                let va = octave_value(seed, octave, &a, &mut cache).unwrap();
                let vb = octave_value(seed, octave, &b, &mut cache).unwrap();
                assert!(
                    (va - vb).abs() < 1e-12,
                    "octave {octave}, axis {axis}: {va} against {vb}"
                );
            }
        }
    }

    /// At a fixed point inside a lattice cell, away from every face, an octave's value over 10⁵
    /// seeds is standard normal by a Kolmogorov–Smirnov test: the normalisation keeps the variance
    /// that plain interpolation would lose. The coarsest and the finest octaves are tested, at a
    /// point whose fraction across the cell is 0.2–0.8 on every axis for both.
    #[test]
    fn an_octaves_value_is_standard_normal_over_seeds() {
        let p = position([26_013.7, -417.3, 300.9]);
        let mut cache = NoiseCache::with_capacity(16);
        for octave in [0, OCTAVES - 1] {
            let (_, t) = lattice_cell(&p, octave);
            assert!(t.iter().all(|t| (0.2..0.8).contains(t)), "{t:?}");
            let mut sample: Vec<f64> = (0..100_000_u64)
                .map(|n| {
                    let seed = Seed::new(0x0700_0000_0000_0000 | n);
                    octave_value(seed, octave, &p, &mut cache).unwrap()
                })
                .collect();
            let ks = ks_one_sample(&mut sample, normal_cdf);
            assert_p_value(
                &format!("octave {octave} against N(0, 1)"),
                ks.p_value,
                ALPHA,
            );
        }
    }

    /// A cache of any size, none included, and a cache after `clear` or after serving another
    /// seed, give the same bits as a fresh one.
    #[test]
    fn the_cache_changes_no_result() {
        let seeds = [Seed::new(3), Seed::new(0x5eed)];
        let points: Vec<GalacticPosition> = (0..200)
            .map(|i| {
                let i = f64::from(i);
                position([26_000.0 + 31.7 * i, -300.0 + 7.1 * i, 40.0 - 0.9 * i])
            })
            .collect();
        let run = |cache: &mut NoiseCache, seed: Seed| -> Vec<u64> {
            points
                .iter()
                .flat_map(|p| {
                    [
                        SmoothingScale::Full,
                        SmoothingScale::AtLeast(LightYears::new(250.0)),
                    ]
                    .map(|scale| bits(log_normal_factor(seed, p, 2.2, scale, cache)))
                })
                .collect()
        };
        for seed in seeds {
            let reference = run(&mut NoiseCache::with_capacity(0), seed);
            for capacity in [1, 2, 7, 64, 4_096] {
                let mut cache = NoiseCache::with_capacity(capacity);
                assert_eq!(run(&mut cache, seed), reference, "capacity {capacity}");
                assert_eq!(
                    run(&mut cache, seed),
                    reference,
                    "warm, capacity {capacity}"
                );
                cache.clear();
                assert_eq!(
                    run(&mut cache, seed),
                    reference,
                    "cleared, capacity {capacity}"
                );
            }
        }
        // One cache serving two seeds in turn forgets the first.
        let mut shared = NoiseCache::with_capacity(4_096);
        let first = run(&mut shared, seeds[0]);
        let second = run(&mut shared, seeds[1]);
        assert_ne!(first, second);
        assert_eq!(second, run(&mut NoiseCache::with_capacity(0), seeds[1]));
        assert_eq!(run(&mut shared, seeds[0]), first);
    }

    /// Outside the root cube the factor is its mean, and on the cube's far faces it is defined.
    #[test]
    fn outside_the_root_cube_the_factor_is_one() {
        let mut cache = NoiseCache::with_capacity(8);
        let seed = Seed::new(9);
        let outside = GalacticPosition::new(LyCell::new([65_536, 0, 0]), [0.0; 3]).unwrap();
        assert_same_bits(
            log_normal_factor(seed, &outside, 2.5, SmoothingScale::Full, &mut cache),
            1.0,
        );
        let far = position([-70_000.0, 3.0, 3.0]);
        assert_same_bits(
            log_normal_factor(seed, &far, 2.5, SmoothingScale::Full, &mut cache),
            1.0,
        );
        let edge = GalacticPosition::new(
            LyCell::new([65_535, -65_536, 65_535]),
            [
                0.999 * METRES_PER_LIGHT_YEAR,
                0.0,
                0.5 * METRES_PER_LIGHT_YEAR,
            ],
        )
        .unwrap();
        for octave in 0..OCTAVES {
            assert!(octave_value(seed, octave, &edge, &mut cache).is_some());
        }
        let inside = log_normal_factor(seed, &edge, 2.5, SmoothingScale::Full, &mut cache);
        assert!(
            (inside - 1.0).abs() > 1e-6,
            "the noise is off at the cube's far corner"
        );
    }

    /// The fade: 0 and 1 at the ends, a half in the middle, and flat at both ends.
    #[test]
    fn the_fade_is_the_quintic_smoothstep() {
        assert_same_bits(fade(0.0), 0.0);
        assert_same_bits(fade(1.0), 1.0);
        assert_same_bits(fade(0.5), 0.5);
        for t in [0.1, 0.3, 0.7, 0.9] {
            assert!((fade(t) + fade(1.0 - t) - 1.0).abs() < 1e-15);
        }
        assert!(fade(1e-5) < 1e-14 && 1.0 - fade(1.0 - 1e-5) < 1e-14);
    }
}
