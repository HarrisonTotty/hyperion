//! The structural octaves: a body's relief between its coarse cells and its band limit, gradient
//! noise conditioned on the crust and the nearest plate boundary (plan R09, T5; Design notes 7 and
//! 13).
//!
//! # The octaves
//!
//! One octave belongs to each quadtree level m finer than the field's coarse level L, from
//! [`first_octave`], L + 1, to [`finest_octave`], the deepest whose lattice spacing is at least the
//! 2 m band limit (R05's rule: never an octave finer than the band limit). Octave m is R05's noise
//! basis ([`crate::noise`]: improved Perlin noise with its 16-entry gradient table, its certified
//! bound B and its cache) at the spheroid point P = M·d of the direction d, measured in units of
//! the body's mean radius R, so that a body's octave table is a function of its detail seed alone:
//!
//! - its lattice spacing is 1.5 mean cells of level m, `λ_m` = 1.5 √(2π ÷ 3) ÷ 2^m = √(3π ÷ 2) ÷
//!   2^m radii ([`octave_spacing`]; 27 km at level 9 and 3.3 m at level 22 on an Earth), which puts
//!   the noise's median wavelength in a planar section, 1.9 λ (measured on 4 × 256² samples,
//!   R09.T5), at the geometric centre of the octave's band below, 2√2 mean cells;
//! - its rotation is R05's rational one of its index (the noise basis's `rotation`);
//! - its offset is words 2⁴⁰ + 4m + axis of object 0 of the body's `surface.relief` stream, its
//!   corners those of [`NoiseKey::Relief`].
//!
//! # The spectrum
//!
//! The body's [`BandSpectrum`] is a degree variance that breaks once, continuous at the break
//! degree `l_b` (`decision-r09-t5.md` item 2): V(l) = V₁ l^−β₁ for l < `l_b`, and
//! V₁ `l_b`^(β₂−β₁) l^−β₂ from it. Level m resolves the degrees below its Nyquist degree,
//! [`nyquist_degree`], `l_N(m)` = ⌈π R ÷ `Δ̄_m`⌉ = ⌈√(3π ÷ 2) 2^m⌉ with `Δ̄_m` the level's mean
//! cell width √(4πR² ÷ (6 · 4^m)) (Design note 4's measure), the degree whose wavelength 2πR ÷ l
//! is two mean cells. Octave m carries the band between level m − 1's Nyquist degree and its own,
//! [`BandSpectrum::level_variance`], so its RMS height is `σ_m` = √(Σ V(l)) over
//! `l_N(m − 1)` ≤ l < `l_N(m)` ([`octave_rms`]). The octaves of a body together carry every degree
//! from the coarse cell's Nyquist degree to the finest octave's, and [`local_variance`] is the
//! closed form of that variance to every degree, the expected variance below the coarse cell at
//! the reference style. Every sum over degrees is a sum of the two segments' power-law tails,
//! T(β, x) = Σ l^−β over l ≥ x: with L\* = ⌈`l_b`⌉, the second segment's first degree, the
//! variance from degree d is V₁ [T(β₁, d) − T(β₁, L\*) + `l_b`^(β₂−β₁) T(β₂, L\*)] for d < L\*,
//! and V₁ `l_b`^(β₂−β₁) T(β₂, d) from L\* ([`BandSpectrum::variance_from_degree`]).
//!
//! # The styles
//!
//! Each cell has a [`ReliefStyle`] from its crust, its nearest plate boundary and its sea floor
//! (the brainstorm's "ridged multifractal for a mountain belt, low-amplitude for an abyssal plain,
//! domain-warped where a boundary is oblique"): an amplitude factor a, a belt weight b and a shear
//! weight s, each interpolated over the sphere by R09.T4's spline ([`interp::interpolate`]), so
//! that a style changes smoothly across cells and the relief stays C¹ wherever the noise is. The
//! amplitudes are absolute ratios to the body class's reference ground, whose law the spectrum
//! is, so a world with more mountain belts is rougher and one with more abyssal plain smoother
//! (`decision-r09-t5.md` item 1). Octave m's contribution is a `σ_m` `t_m`, with
//!
//! > `t_m` = cos θ · `n̂_m` + sin θ · `r̃_m`,  θ = (π ÷ 2) b,
//!
//! n̂ = n ÷ `σ_noise` the octave's noise at unit RMS and r̃ its ridged multifractal term. That is
//! the ridged transform r̂ = (r − `μ_r`) ÷ `σ_r` of the raw ridge r = 1 − √(n² + ε²), its pinned
//! mean removed and its pinned RMS divided out (R05's [`RIDGE_MEAN`] and [`RIDGE_RMS`]), weighted
//! as Musgrave's ridged multifractal is by the octave above: `r̃_m` = w(`r_{m−1}`) `r̂_m` ÷ W, with
//! w 1 on the coarser octave's crests and falling towards its troughs (its median is 0.71 and its
//! first percentile 0.15) and W = [`RIDGE_WEIGHT_RMS`] its pinned RMS, so that fine ridges grow on
//! coarse crests and valleys stay smoother. The first octave is unweighted. Octave m's lattice is
//! independent of the coarser ones, so the weighted term keeps zero mean, unit variance and no
//! correlation with the octaves above; and the noise's distribution is symmetric about zero, since
//! reflecting a point through its lattice cell's centre negates the noise with the corners'
//! gradients exchanged, which are drawn alike, so n̂ and the even r̂ are uncorrelated. The rotation
//! therefore keeps `t_m` at zero mean and unit variance in every style: the styles move the
//! relief's variance about the body only through a.
//!
//! # The warp
//!
//! Where s is not zero the octaves are domain-warped, each from coarser octaves alone (Design note
//! 13: "domain warp taking offsets only from octaves no finer than the level warped"): octave m,
//! from the fourth of the body's octaves on, is evaluated at p + s · c `λ_m` (`n̂_{m−1}`,
//! `n̂_{m−2}`, `n̂_{m−3}`), the three octaves above it along the body-fixed x, y and z axes, with
//! c = [`WARP_SCALE`]. The first three octaves are not warped. A warped octave's noise has the
//! same distribution as an unwarped one's, since its own lattice is independent of the octaves
//! that move it, so the variance is unchanged.
//!
//! # Bounds and gradient
//!
//! |`t_m`| is at most `T_MAX` = √((B ÷ `σ_noise`)² + (r̂(B) ÷ W)²) = 8.770, reached at n = −B with
//! the weight 1, where both terms are negative, and the interpolated amplitude never leaves the
//! cells' range, so octave m never contributes more than [`octave_bound`], [`MAX_AMPLITUDE`] ×
//! `σ_m` × `T_MAX`. The gradient is analytic, through the warp's Jacobian and the styles'
//! gradients, and is returned as R09.T4's base elevation returns its own: the gradient of
//! H(P) = h(M⁻¹P ÷ |M⁻¹P|), the relief as a function of direction, at the spheroid point, so that
//! the two add.
//!
//! [`RIDGE_MEAN`]: crate::noise::RIDGE_MEAN
//! [`RIDGE_RMS`]: crate::noise::RIDGE_RMS

use core::f64::consts::{FRAC_PI_2, PI};

use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Metres, SquareMetres};

use super::interp::{self, CellValues, ReadCellError};
use super::{BandSpectrum, SynthCache};
use crate::cube::PatchKey;
use crate::field::{
    BoundaryKind, CoarseLevel, Crust, FieldHeader, FieldView, SynthesisCell, hill_relief_of,
    ponded_sediment_of,
};
use crate::geometry::BAND_LIMIT_M;
use crate::height::HeightSample;
use crate::noise::{
    self, LatticeCache, NOISE_BOUND, NOISE_RMS, NoiseKey, Octave, RIDGE_EPSILON, RIDGE_MEAN,
    RIDGE_RMS, gradient_noise,
};
use crate::num;

/// The number of octave indices, 0 to 31: the relief's octaves are those of levels L + 1 to the
/// finest, and an octave index has five bits in a corner's draw number ([`crate::noise`]).
const OCTAVES: u8 = 32;

/// The first word of the offsets' draws, far above every lattice corner's draw number (which are
/// below 2³⁷; see [`crate::noise`]), as R05's test planet places its own.
const OFFSET_DRAW_BASE: u64 = 1 << 40;

/// The warp's displacement, in lattice spacings of the octave warped, a unit of the coarser
/// octaves' noise moves it: a procedural choice (the plan's own). At full shear the displacement's
/// Jacobian has an RMS of about 0.6 on its diagonal, so features shear visibly, and the warp folds
/// on about 5% of the ground (R09.T5's science check, measured on the noise).
pub const WARP_SCALE: f64 = 0.5;

/// The octave of the body's octaves from which the warp starts, counted from the first: the first
/// three are not warped, so that every warped octave takes its three components from three
/// coarser octaves.
const WARP_START: u8 = 3;

/// √(3π ÷ 2): level m's Nyquist degree is this times 2^m, and octave m's lattice spacing is this
/// divided by 2^m, in radii (see the module documentation).
#[must_use]
fn root_three_pi_over_two() -> f64 {
    (1.5 * PI).sqrt()
}

/// Level `level`'s Nyquist degree, ⌈√(3π ÷ 2) 2^level⌉: the first degree whose wavelength 2πR ÷ l
/// is shorter than two of the level's mean cells, so the first the level does not resolve (see
/// the module documentation). 556 at level 8, 70 at level 5; the same on every body, since the
/// cells scale with the radius.
///
/// # Panics
///
/// If `level` is above 40.
#[must_use]
pub fn nyquist_degree(level: u8) -> u64 {
    assert!(
        level <= 40,
        "a Nyquist degree is for levels 0 to 40, not {level}"
    );
    #[expect(
        clippy::cast_precision_loss,
        reason = "a power of two up to 2^40 is exact in f64"
    )]
    let cells = (1_u64 << level) as f64;
    let degree = (root_three_pi_over_two() * cells).ceil();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive integer below 2^42, exact in f64 and in u64"
    )]
    let degree = degree as u64;
    degree
}

/// The relief's first octave on a field of coarse level `level`: L + 1, the band just finer than
/// the coarse cells.
#[must_use]
pub const fn first_octave(level: CoarseLevel) -> u8 {
    level.get() + 1
}

/// The lattice spacing of octave `m` on a body of mean radius `radius`: √(3π ÷ 2) R ÷ 2^m, 1.5 of
/// the level's mean cell widths (see the module documentation).
///
/// # Panics
///
/// If `m` is 32 or more.
#[must_use]
pub fn octave_spacing(radius: Metres, m: u8) -> Metres {
    Metres::new(spacing_in_radii(m) * radius.value())
}

/// Octave `m`'s lattice spacing in radii, √(3π ÷ 2) ÷ 2^m.
#[must_use]
fn spacing_in_radii(m: u8) -> f64 {
    assert!(m < OCTAVES, "an octave index is 0 to 31, got {m}");
    root_three_pi_over_two() / f64::from(1_u32 << m)
}

/// The deepest octave on a body of mean radius `radius` whose lattice spacing is at least the 2 m
/// band limit ([`BAND_LIMIT_M`]): 22 for an Earth, 18 for a Ceres. No octave finer is evaluated.
///
/// # Panics
///
/// If `radius` is not finite and positive.
#[must_use]
pub fn finest_octave(radius: Metres) -> u8 {
    let r = radius.value();
    assert!(
        r.is_finite() && r > 0.0,
        "a body's radius must be finite and positive, got {r}"
    );
    (0..OCTAVES)
        .take_while(|&m| spacing_in_radii(m) * r >= BAND_LIMIT_M)
        .last()
        .unwrap_or(0)
}

/// The RMS height of octave `m` at the reference style (amplitude 1): the square root of its
/// band's variance, [`BandSpectrum::level_variance`].
///
/// # Panics
///
/// If `m` is 0 or above 40.
#[must_use]
pub fn octave_rms(spectrum: &BandSpectrum, m: u8) -> Metres {
    Metres::new(spectrum.level_variance(m).value().sqrt())
}

/// The largest |t| of an octave's term t = cos θ n̂ + sin θ r̃ over every noise value, weight and
/// style: √((B ÷ `σ_noise`)² + (r̂(B) ÷ W)²), at n = −B with the weight 1 (see the module
/// documentation).
#[must_use]
fn t_max() -> f64 {
    let plain = NOISE_BOUND / NOISE_RMS;
    let ridged = ridged(NOISE_BOUND) / RIDGE_WEIGHT_RMS;
    (plain * plain + ridged * ridged).sqrt()
}

/// The largest height octave `m` can contribute anywhere on a body of this spectrum, metres:
/// [`MAX_AMPLITUDE`] × `σ_m` × `T_MAX` (see the module documentation), its stated bound.
///
/// # Panics
///
/// If `m` is 0 or above 40.
#[must_use]
pub fn octave_bound(spectrum: &BandSpectrum, m: u8) -> Metres {
    Metres::new(MAX_AMPLITUDE * octave_rms(spectrum, m).value() * t_max())
}

/// The expected variance of the structural relief finer than a field's coarse cells, at the
/// reference style (amplitude 1): Σ V(l) over every degree from the coarse level's Nyquist degree
/// up, in closed form ([`BandSpectrum::variance_from_degree`]); the sum of every octave's band
/// (Design note 7).
///
/// It is the reference style's. The styles' amplitudes are absolute ratios to the body class's
/// reference ground, so the realised structural share is ⟨a²⟩ times this, ⟨a²⟩ the solid-angle
/// mean of the interpolated squared amplitude ([`ReliefStyle::amplitude`]): 1 on a body whose
/// cells all have the reference style, less on one with more plain than belt. Nothing normalises
/// it away, so a world with more belts is rougher: the coarse pass (R09.T12.e) books ⟨a²⟩ times
/// this in `σ_h`'s budget, with ⟨a²⟩ from its own quadrature of the interpolated amplitude
/// (`decision-r09-t5.md` item 1). The octaves stop at the band limit, far below the break, which
/// leaves out V₁ `l_b`^(β₂−β₁) T(β₂, `l_N(finest)`) of it: under 10⁻⁶ of it on the synthetic
/// worlds.
#[must_use]
pub fn local_variance(spectrum: &BandSpectrum, level: CoarseLevel) -> SquareMetres {
    spectrum.variance_from_degree(nyquist_degree(level.get()))
}

/// The relief's variance in wavelengths shorter than `finer_than` at `cell`, from the cell's own
/// style: a² Σ V(l) over the octaves' degrees whose wavelength 2πR ÷ l is shorter, from the
/// coarse level's Nyquist degree to the finest octave's; `None` where `field` does not hold the
/// cell (Design note 18).
///
/// The synthesis blends the styles of neighbouring cells, so within a cell or so of a change of
/// style the realised variance lies between the cells'.
///
/// # Panics
///
/// If `finer_than` is not finite and positive, or `cell` is not of the field's level.
#[must_use]
pub(crate) fn unresolved_relief_variance(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<SquareMetres> {
    let w = finer_than.value();
    assert!(
        w.is_finite() && w > 0.0,
        "a wavelength must be finite and positive, got {w}"
    );
    let header = field.header();
    assert_eq!(
        cell.level(),
        header.level().get(),
        "the cell is not of the field's level"
    );
    let style = ReliefStyle::of(field.cell(cell)?);
    let radius = header.radius().value();
    let start = nyquist_degree(header.level().get());
    let end = nyquist_degree(finest_octave(header.radius()));
    // The first degree whose wavelength 2πR ÷ l is shorter than w.
    let shorter = (2.0 * PI * radius / w).floor() + 1.0;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a Nyquist degree is below 2^42, exact in f64"
    )]
    let finest = end as f64;
    if shorter >= finest {
        return Some(SquareMetres::ZERO);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive integer below the finest Nyquist degree, under 2^42"
    )]
    let shorter = shorter as u64;
    // A body too small for any octave (its finest coarser than its coarse cells) has none.
    let from = start.max(shorter);
    if from >= end {
        return Some(SquareMetres::ZERO);
    }
    let band = header.spectrum().band_variance(from, end);
    let a = style.amplitude;
    Some(SquareMetres::new(a * a * band.value()))
}

/// The RMS of the ridged multifractal's weight w over space, `√E[w²]`: 0.719 88 measured over 10⁷
/// points (`the_ridge_weight_rms_is_measured`, under `just test-slow`) and pinned. A weighted
/// ridged term is divided by it to keep unit variance.
pub const RIDGE_WEIGHT_RMS: f64 = 0.7199;

/// The ridged multifractal's weight w(r) = (max(r, 0) ÷ (1 − ε))² of the coarser octave's raw ridge
/// r = 1 − √(n² + ε²), and its derivative dw ÷ dr: 1 on the coarser octave's crest (n = 0) and
/// falling towards its troughs (to 0 only where |n| ≥ 0.9987, which almost never occurs), C¹
/// where r crosses 0. It follows Musgrave's ridged multifractal, whose weight is the octave
/// before's squared ridge signal times a gain, clamped to [0, 1] (Musgrave, in Ebert et al. 2003,
/// "Texturing and Modeling: A Procedural Approach", 3rd ed., Morgan Kaufmann; libnoise's
/// `RidgedMulti` uses a gain of 2): here the gain is in effect 1 once the crest is normalised to 1,
/// and the weight reads only the octave immediately above, not the whole cascade, so that each
/// octave's mean is zero and its variance pinned: octave m's lattice is independent of octave
/// m − 1's, so E[w r̂] = E[w] E[r̂] = 0 and E[w² r̂²] = E[w²].
#[must_use]
fn ridge_weight(r: f64) -> (f64, f64) {
    let top = 1.0 - RIDGE_EPSILON;
    let x = num::max(r, 0.0) / top;
    (x * x, 2.0 * x / top)
}

/// The raw ridge r = 1 − √(n² + ε²) of a noise value `n`, and its derivative dr ÷ dn.
#[must_use]
fn ridge(n: f64) -> (f64, f64) {
    let root = (n * n + RIDGE_EPSILON * RIDGE_EPSILON).sqrt();
    (1.0 - root, -n / root)
}

/// The ridged transform r̂(n) = (r − `μ_r`) ÷ `σ_r` of a noise value `n`, the raw ridge with its
/// pinned mean removed and its pinned RMS divided out, unweighted.
#[must_use]
fn ridged(n: f64) -> f64 {
    (ridge(n).0 - RIDGE_MEAN) / RIDGE_RMS
}

/// Octave `m` of the body of detail seed `seed`.
#[must_use]
fn octave(seed: DetailSeed, m: u8) -> Octave {
    let key = NoiseKey::Relief(seed);
    let offset = [0_u64, 1, 2].map(|axis| {
        let word = key.word(0, OFFSET_DRAW_BASE + 4 * u64::from(m) + axis);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in f64"
        )]
        let top = (word >> 11) as f64;
        top / 9_007_199_254_740_992.0
    });
    Octave::new(m, spacing_in_radii(m), noise::rotation(m), offset, key)
}

/// The body's whole octave table, octaves 0 to 31, of which the relief reads L + 1 to the finest.
#[must_use]
fn octave_table(seed: DetailSeed) -> Vec<Octave> {
    (0..OCTAVES).map(|m| octave(seed, m)).collect()
}

/// The relief's amplitude factor in a mountain belt: 2.5 times the reference.
///
/// The reference style, amplitude 1, is a body's typical ground: Earth's global mean relief in
/// 35 km windows, 160 m RMS once a plane is removed from each, measured by R09.T5's science check
/// (2026-10-09) on Earth2014's BED layer at 1′ (the dataset of Hirt and Rexer 2015, Int. J. Appl.
/// Earth Obs. Geoinf. 39, 103, doi:10.1016/j.jag.2015.03.001), and a stagnant lid's own crust.
/// Active belts measure 2.1–3.4 times that mean on the same grid (the Andes above 1 km 2.1, the
/// Alps 2.15, the Himalaya 2.5, the Karakoram 3.4; the same check), as Montgomery and Brandon
/// 2002 (EPSL 201, 481, doi:10.1016/S0012-821X(02)00725-2, pp. 485–487, Figs. 6–7) find 1–2 km of
/// local relief within 10 km in active ranges, where at most 5% of the land exceeds 1 km.
/// Medium-high confidence.
pub const BELT_AMPLITUDE: f64 = 2.5;

/// The amplitude factor of a stable continental interior, far from every plate boundary: 0.12
/// (the Canadian Shield, the East European Platform, the Amazon, the Great Plains, Australia and
/// the Congo measure 0.06–0.15 of Earth's global mean in 35 km windows; measured by R09.T5's
/// science check on Earth2014's BED layer). High confidence in the ratio; Earth's old orogens far
/// from today's boundaries, which the coarse field cannot tell from cratons, are rougher.
pub const INTERIOR_AMPLITUDE: f64 = 0.12;

/// The amplitude factor of continental crust in the diffuse deformation zone about a boundary of
/// any kind (rifts, transform systems, the margins of belts): 1, the reference. Diffuse boundaries
/// cover about 15% of Earth (Gordon 1998, Annu. Rev. Earth Planet. Sci. 26, 615); the factor is
/// the plan's own, between the interiors' and the belts'.
pub const ACTIVE_AMPLITUDE: f64 = 1.0;

/// The draped abyssal-hill relief whose amplitude factor is the reference style's 1: 95 m
/// (`decision-r09-t5.md` item 5).
///
/// Oceanic crust's amplitude is `a_h` = min((H ÷ `H_REF`)^0.6, [`BELT_AMPLITUDE`]) of its hill
/// relief H ([`SynthesisCell::hill_relief`]): the exponent ([`HILL_AMPLITUDE_EXPONENT`]) and the
/// reference reconcile two of GMRT's measures of the floor against Earth's global mean (the grid
/// of Ryan et al. 2009, G³ 10, Q03014, doi:10.1029/2008GC002332, at 61 m; R09.T5's science check
/// and the ruling's). On the slow-spreading Mid-Atlantic Ridge's flank, H 232 m, the floor
/// measures 2.3 of the reference in 10 km windows and 1.35 in its 122 m slopes, and this gives
/// 1.71; on the fast East Pacific Rise's, H 55–62 m, 0.8 and 0.78, and this gives 0.72–0.77; on the
/// old fast Pacific floor, draped to 28 m, 0.6 in its 122 m slopes, and this gives 0.48. Goff 1991
/// (JGR 96, 21713, doi:10.1029/91JB02275) finds the slowest spreading the roughest. A style per
/// band is the lever if R10 finds the slow floor's 10 km relief short. Medium confidence.
pub const H_REF: Metres = Metres::new(95.0);

/// The exponent of oceanic crust's amplitude in its hill relief, 0.6 ([`H_REF`]).
pub const HILL_AMPLITUDE_EXPONENT: f64 = 0.6;

/// The amplitude factor of an abyssal plain, sea floor whose hills its ponded turbidites bury:
/// 0.1. The interiors of the Hatteras, Madeira, Argentine and Bengal plains measure 6–14 m
/// (median) and 17–23 m (mean) in 35 km windows of Earth2014, 0.04–0.14 of the global mean, and
/// the Hatteras plain's median gradient over 10 km of GMRT is 1:1,436 (both measured by R09.T5's
/// science check), within the usual "gradient under 1:1,000" (Heezen, Tharp and Ewing 1959, GSA
/// Spec. Pap. 65). A plain needs a turbidite supply, the ponded sediment of the cell's sea floor
/// ([`SynthesisCell::ponded_sediment`]), and the old floor far from a margin keeps its draped
/// hills. Medium confidence.
pub const PLAIN_AMPLITUDE: f64 = 0.1;

/// Ponded sediment buries hills from half their relief H to three times it: oceanic crust's
/// amplitude is `a_h`^(1 − t) [`PLAIN_AMPLITUDE`]^t with t = clamp(ln(`S_t` ÷ (½ H)) ÷ ln 6, 0, 1)
/// (`decision-r09-t5.md` item 5), harmless below H ÷ 2 and a plain complete at 3H, the wedge's
/// surface above the hills' highest crests: the ruling's own thresholds, not calibrated; compare
/// Goff 2010 (JGR 115, B12104, doi:10.1029/2010JB007867, ¶36 and ¶43), where hills stay visible
/// under pelagic drape many times their relief. Low confidence.
const PONDING: (f64, f64) = (0.5, 3.0);

/// The amplitude factor of a stagnant lid's volcanic plains, over its own crust's 1: 0.25. The
/// lunar maria's relief is 0.17–0.27 of the highlands' in 9–34 km windows of LOLA's 16 ppd grid
/// (Smith et al. 2010, GRL 37, L18204, doi:10.1029/2010GL043751) and their median slope at 17 m
/// 0.27 of it (2.0° against 7.5°; Rosenburg et al. 2011, JGR 116, E02001,
/// doi:10.1029/2010JE003716, Table 1); the Martian volcanic plains' is 0.24–0.56 of the southern
/// highlands' in 35 km windows of MOLA's 32 ppd grid (Smith et al. 2001, JGR 106, 23689,
/// doi:10.1029/2000JE001364). The windows measured by R09.T5's science check. Medium confidence.
pub const PROVINCE_AMPLITUDE: f64 = 0.25;

/// The largest amplitude factor any style has: the octaves' bounds use it. The sea floor's hills
/// are capped at it.
pub const MAX_AMPLITUDE: f64 = BELT_AMPLITUDE;

/// A collision belt's weight, full within 100 km of the boundary on either side and zero beyond
/// 250 km: Design note 7's belts 200–500 km wide, against rough belts measured at 185–410 km
/// across (the Himalaya 360–410, the Alps 270, the Zagros 185; R09.T5's science check on
/// Earth2014).
const COLLISION_KM: (f64, f64) = (100.0, 250.0);

/// A subduction margin's island arc on an oceanic overriding plate, km behind the trench: zero to
/// 90, rising to full weight at 140, full to 250 and gone by 330 (`decision-r09-t5.md` item 6).
///
/// Syracuse and Abers 2006 (G³ 7, Q05017, doi:10.1029/2005GC001045, Table S2a's 527 front
/// volcanoes, classified by arc segment in the ruling) put the volcanic front 190 km behind the
/// trench under an oceanic overriding plate, interquartile 170–220 km and 10–90% 140–250 km; the
/// arc behind it is 30–100 km wide (60 km typical). The ramp's foot at 90 km lies below the
/// fronts' 10%, and its fall from 250 to 330 km covers the arc's width behind the latest fronts.
/// The forearc between the trench and the ramp keeps its crust's own style, the sea floor here.
const ARC_OCEANIC_KM: (f64, f64, f64, f64) = (90.0, 140.0, 250.0, 330.0);

/// A subduction margin's arc and cordillera on a continental overriding plate, km behind the
/// trench: zero to 130, rising to full weight at 180, full to 450 and gone by 600
/// (`decision-r09-t5.md` item 6).
///
/// Syracuse and Abers 2006 (G³ 7, Q05017, Table S2a, as [`ARC_OCEANIC_KM`]) put the volcanic front
/// 260 km behind the trench under a continental overriding plate, interquartile 205–295 km and
/// 10–90% 180–330 km, and the arc 100–250 km wide from its front-most to its rear-most volcano
/// (175 km typical); the band reaches past it over the cordillera behind, as the Andes' rough
/// belt, 225–255 km across (R09.T5's science check on Earth2014). Bird 2003's "200–250 km is more
/// typical" (G³ 4, 1027, ¶105, p. 39) brackets the median of all fronts, 220 km. The forearc
/// between the trench and the ramp keeps its crust's own style, the deformation zone here.
const ARC_CONTINENTAL_KM: (f64, f64, f64, f64) = (130.0, 180.0, 450.0, 600.0);

/// The diffuse deformation zone of continental crust about a boundary, full to 150 km and gone by
/// 500 km: Bird 2003's thirteen orogens of PB2002 (0.937 sr, ¶116) are 500–1,400 km in mean width
/// 2A ÷ P, as R09.T5's science check derived from their outlines, which suggests "strong within
/// 150–250 km, tapering out to about 500 km".
const ZONE_KM: (f64, f64) = (150.0, 500.0);

/// A boundary's zone of distributed shear on continental crust, full to 50 km and gone by 100 km,
/// half weight across a band 150 km wide (a continental transform system about 150 km across,
/// R09.T5's science check), and on oceanic crust, full to 20 km and gone by 50 km (an oceanic
/// transform's valley 10–20 km wide): the plan's own, low confidence. At level 8 the oceanic zone
/// covers the boundary's cells and, at weights of 0.1 to 1, their neighbours.
const SHEAR_KM: [(f64, f64); 2] = [(50.0, 100.0), (20.0, 50.0)];

/// One minus the quintic smoothstep of x between `full` (weight 1 at and below) and `zero`
/// (weight 0 at and above).
#[must_use]
fn taper(x: f64, (full, zero): (f64, f64)) -> f64 {
    if x <= full {
        1.0
    } else if x >= zero {
        0.0
    } else {
        let t = (x - full) / (zero - full);
        1.0 - t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }
}

/// A trapezoid of quintic ramps in x: 0 at and below `rise`, rising to 1 at `from`, 1 to `to`,
/// and falling to 0 at and beyond `fall`.
#[must_use]
fn band(x: f64, (rise, from, to, fall): (f64, f64, f64, f64)) -> f64 {
    if x <= to {
        1.0 - taper(x, (rise, from))
    } else {
        taper(x, (to, fall))
    }
}

/// The amplitude factor of oceanic crust whose sea floor byte is `seafloor`
/// ([`SynthesisCell::seafloor`]): `a_h`^(1 − t) [`PLAIN_AMPLITUDE`]^t, with `a_h` =
/// min((H ÷ [`H_REF`])^0.6, [`BELT_AMPLITUDE`]) of the hill relief H and
/// t = clamp(ln(2 `S_t` ÷ H) ÷ ln 6, 0, 1) of the ponded sediment `S_t` (`decision-r09-t5.md`
/// item 5): the hills' own `a_h` to `S_t` = H ÷ 2, and a plain's from 3H. A byte that names no
/// hills, which no oceanic cell of a field has (its record would be refused), is taken for a
/// plain.
#[must_use]
fn seafloor_amplitude(seafloor: u8) -> f64 {
    let Some(h) = hill_relief_of(seafloor) else {
        return PLAIN_AMPLITUDE;
    };
    let h = h.value();
    let hills = num::min(
        math::powf(h / H_REF.value(), HILL_AMPLITUDE_EXPONENT),
        BELT_AMPLITUDE,
    );
    let (onset, buried) = PONDING;
    let ratio = ponded_sediment_of(seafloor).value() / h;
    if ratio <= onset {
        hills
    } else if ratio >= buried {
        PLAIN_AMPLITUDE
    } else {
        let t = math::ln(ratio / onset) / math::ln(buried / onset);
        math::powf(hills, 1.0 - t) * math::powf(PLAIN_AMPLITUDE, t)
    }
}

/// How a cell's relief is drawn: its amplitude factor, its mountain-belt weight and its shear
/// weight, from its crust, its nearest plate boundary and its sea floor (see the module
/// documentation).
///
/// The amplitude is the crust's, then lifted towards [`BELT_AMPLITUDE`] by the belt weight: a
/// continental cell's runs from [`INTERIOR_AMPLITUDE`] far from every boundary to
/// [`ACTIVE_AMPLITUDE`] within its deformation zone; an oceanic cell's is its sea floor's, from
/// its hills' relief (about 1.7 on a slow ridge's floor, 0.7 on a fast one's) down to
/// [`PLAIN_AMPLITUDE`] where ponded sediment buries them ([`H_REF`]); a stagnant lid's is 1 and its
/// provinces' [`PROVINCE_AMPLITUDE`]. The belt weight is 1 within a collision belt on either side,
/// and in a subduction margin's arc on the overriding plate (the positive side) in a band behind
/// the trench set by the overriding crust ([`ARC_OCEANIC_KM`], [`ARC_CONTINENTAL_KM`]), the
/// forearc before it keeping its crust's own style; the shear weight is the boundary's obliquity
/// within its shear zone. These are the plan's procedural reading of the brainstorm's three
/// rules, with the amplitudes Earth's, the Moon's and Mars's measured relief gives (each
/// constant's documentation). They are absolute ratios to the body class's reference ground,
/// which the spectrum anchors, and nothing normalises their mean (see [`local_variance`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliefStyle {
    amplitude: f64,
    belt: f64,
    shear: f64,
}

impl ReliefStyle {
    /// The reference style: amplitude 1, plain noise, no warp.
    pub const REFERENCE: Self = Self {
        amplitude: 1.0,
        belt: 0.0,
        shear: 0.0,
    };

    /// The style of `cell`: [`from_codes`](Self::from_codes) of its crust, boundary, boundary
    /// distance and obliquity, and sea floor.
    #[must_use]
    pub fn of(cell: &SynthesisCell) -> Self {
        Self::from_codes(
            cell.crust,
            cell.boundary,
            cell.boundary_distance_km,
            cell.boundary_obliquity,
            cell.seafloor,
        )
    }

    /// The style of a cell of crust `crust` whose nearest boundary is of kind `boundary`, at the
    /// coded distance `boundary_distance_km` ([`SynthesisCell::boundary_distance_km`],
    /// [`SynthesisCell::NO_BOUNDARY_KM`] for none) and obliquity `boundary_obliquity`
    /// ([`SynthesisCell::boundary_obliquity`]), with the sea floor byte `seafloor`
    /// ([`SynthesisCell::seafloor`]): the codes a cell stores, and exactly those
    /// [`of`](Self::of) reads, so that the coarse pass (R09.T12.e), which holds no
    /// [`SynthesisCell`] in the middle of the pass, takes the same style from its quantised codes.
    #[must_use]
    pub fn from_codes(
        crust: Crust,
        boundary: BoundaryKind,
        boundary_distance_km: i16,
        boundary_obliquity: u8,
        seafloor: u8,
    ) -> Self {
        let distance_km = (boundary_distance_km != SynthesisCell::NO_BOUNDARY_KM)
            .then(|| f64::from(boundary_distance_km));
        // The arc's band is set by the overriding crust; a stagnant lid's crusts have no
        // boundary, so only the first two arms are reached.
        let arc = match crust {
            Crust::Oceanic => ARC_OCEANIC_KM,
            Crust::Continental | Crust::Lid | Crust::Province => ARC_CONTINENTAL_KM,
        };
        let belt = match (boundary, distance_km) {
            (BoundaryKind::Collision, Some(d)) => taper(d.abs(), COLLISION_KM),
            // The overriding plate is on the positive side of a subduction boundary.
            (BoundaryKind::Subduction, Some(d)) if d >= 0.0 => band(d, arc),
            (
                BoundaryKind::Subduction
                | BoundaryKind::Collision
                | BoundaryKind::Divergent
                | BoundaryKind::Transform
                | BoundaryKind::Absent,
                _,
            ) => 0.0,
        };
        let obliquity = f64::from(boundary_obliquity) / 255.0;
        let shear_zone = match crust {
            Crust::Oceanic => SHEAR_KM[1],
            Crust::Continental | Crust::Lid | Crust::Province => SHEAR_KM[0],
        };
        let shear = match (boundary, distance_km) {
            (
                BoundaryKind::Subduction
                | BoundaryKind::Collision
                | BoundaryKind::Divergent
                | BoundaryKind::Transform,
                Some(d),
            ) => obliquity * taper(d.abs(), shear_zone),
            (BoundaryKind::Absent, _)
            | (
                BoundaryKind::Subduction
                | BoundaryKind::Collision
                | BoundaryKind::Divergent
                | BoundaryKind::Transform,
                None,
            ) => 0.0,
        };
        let near = |zone| distance_km.map_or(0.0, |d| taper(d.abs(), zone));
        let crust = match crust {
            Crust::Continental => {
                INTERIOR_AMPLITUDE + (ACTIVE_AMPLITUDE - INTERIOR_AMPLITUDE) * near(ZONE_KM)
            }
            Crust::Oceanic => seafloor_amplitude(seafloor),
            Crust::Lid => 1.0,
            Crust::Province => PROVINCE_AMPLITUDE,
        };
        Self {
            amplitude: crust + (BELT_AMPLITUDE - crust) * belt,
            belt,
            shear,
        }
    }

    /// The amplitude factor a, over the reference style's 1: the relief's RMS height is a times
    /// the spectrum's.
    #[must_use]
    pub const fn amplitude(&self) -> f64 {
        self.amplitude
    }

    /// The mountain-belt weight b, 0 to 1: the share of the octaves' rotation from plain noise to
    /// ridged noise.
    #[must_use]
    pub const fn belt(&self) -> f64 {
        self.belt
    }

    /// The shear weight s, 0 to 1: the strength of the domain warp.
    #[must_use]
    pub const fn shear(&self) -> f64 {
        self.shear
    }
}

/// Which component of a cell's style a [`StyleValues`] reads.
#[derive(Debug, Clone, Copy)]
enum Component {
    Amplitude,
    Belt,
    Shear,
}

/// One component of a field's cells' styles, as the interpolant's values.
struct StyleValues<'a, F> {
    field: &'a F,
    component: Component,
}

impl<F: FieldView> CellValues for StyleValues<'_, F> {
    fn level(&self) -> u8 {
        self.field.header().level().get()
    }

    fn value(&self, cell: PatchKey) -> Option<f64> {
        let style = ReliefStyle::of(self.field.cell(cell)?);
        Some(match self.component {
            Component::Amplitude => style.amplitude,
            Component::Belt => style.belt,
            Component::Shear => style.shear,
        })
    }
}

/// A style component's value at a point and its gradient in body-fixed space at the spheroid
/// point, per metre.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Smooth {
    value: f64,
    gradient: [f64; 3],
}

impl Smooth {
    /// A constant.
    #[must_use]
    const fn constant(value: f64) -> Self {
        Self {
            value,
            gradient: [0.0; 3],
        }
    }
}

/// The interpolated style at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
struct StyleSample {
    amplitude: Smooth,
    belt: Smooth,
    shear: Smooth,
}

impl StyleSample {
    /// A style the same everywhere.
    #[cfg(test)]
    const fn uniform(style: ReliefStyle) -> Self {
        Self {
            amplitude: Smooth::constant(style.amplitude),
            belt: Smooth::constant(style.belt),
            shear: Smooth::constant(style.shear),
        }
    }
}

/// The interpolated component `component` of `field`'s styles at `dir`, its gradient taken from
/// the unit sphere to body-fixed space at the spheroid point (`g` ÷ (a, a, c), as the base
/// elevation's is).
fn smooth<F: FieldView>(
    field: &F,
    dir: [f64; 3],
    component: Component,
) -> Result<Smooth, ReadCellError> {
    let figure = field.header().figure();
    let at = interp::interpolate(&StyleValues { field, component }, dir)?;
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let mut gradient = [0.0; 3];
    for ((g, per_radian), radius) in gradient.iter_mut().zip(at.gradient).zip(radii) {
        *g = per_radian / radius;
    }
    Ok(Smooth {
        value: at.value,
        gradient,
    })
}

/// Where the octaves' noise is read: the caller's lattice cache, or a test's recording of it.
trait Lattice {
    /// The noise of `octave` at `x` (radii) and its gradient per radius.
    fn noise(&mut self, octave: &Octave, x: [f64; 3]) -> (f64, [f64; 3]);
}

impl Lattice for LatticeCache {
    fn noise(&mut self, octave: &Octave, x: [f64; 3]) -> (f64, [f64; 3]) {
        gradient_noise(x, octave, self)
    }
}

/// One octave's term at a point: its noise at unit RMS n̂, its ridged transform r̂, the style's
/// mix t of the two, and its height a σ t, metres, with each one's gradient in body-fixed space at
/// the spheroid point, per metre.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Term {
    octave: u8,
    plain: Smooth,
    ridged: Smooth,
    mixed: Smooth,
    height: Smooth,
}

/// The gradient `g` of a function of the point P, at the spheroid point of the unit direction
/// `dir`, as the gradient of the same values as a function of direction alone, H(P) =
/// h(M⁻¹P ÷ |M⁻¹P|): M⁻¹ (I − d dᵀ) M g, the convention of R09.T4's base elevation.
#[must_use]
fn as_function_of_direction(header: &FieldHeader, dir: [f64; 3], g: [f64; 3]) -> [f64; 3] {
    let figure = header.figure();
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let u = [0, 1, 2].map(|k| radii[k] * g[k]);
    let along = u[0] * dir[0] + u[1] * dir[1] + u[2] * dir[2];
    [0, 1, 2].map(|k| (u[k] - along * dir[k]) / radii[k])
}

/// A body's structural octaves, prepared once for its field and detail seed: each octave's RMS
/// height from the spectrum, and its range of octaves (see the module documentation).
///
/// It is plain data, a function of the header and the seed alone, and [`at`](Self::at) reads the
/// cells' styles from the field it is given, which must be the one whose header made it. At 280
/// bytes it is `Clone` but not `Copy`.
#[derive(Debug, Clone, PartialEq)]
pub struct Relief {
    seed: DetailSeed,
    first: u8,
    finest: u8,
    radius_m: f64,
    /// Each octave's RMS height at the reference style, metres, by index; 0 outside the body's
    /// octaves.
    rms_m: [f64; OCTAVES as usize],
}

impl Relief {
    /// The structural octaves of the body of `header`, drawn from its detail seed `seed`.
    #[must_use]
    pub fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        let first = first_octave(header.level());
        let finest = finest_octave(header.radius());
        let mut rms_m = [0.0; OCTAVES as usize];
        for m in first..=finest {
            rms_m[usize::from(m)] = octave_rms(header.spectrum(), m).value();
        }
        Self {
            seed,
            first,
            finest,
            radius_m: header.radius().value(),
            rms_m,
        }
    }

    /// The body's detail seed.
    #[must_use]
    pub const fn seed(&self) -> DetailSeed {
        self.seed
    }

    /// The first octave, [`first_octave`] of the field's level.
    #[must_use]
    pub const fn first_octave(&self) -> u8 {
        self.first
    }

    /// The finest octave, [`finest_octave`] of the body's radius.
    #[must_use]
    pub const fn finest_octave(&self) -> u8 {
        self.finest
    }

    /// Octave `m`'s RMS height at the reference style, metres ([`octave_rms`]); zero for an octave
    /// outside the body's.
    ///
    /// # Panics
    ///
    /// If `m` is 32 or more.
    #[must_use]
    pub fn octave_rms(&self, m: u8) -> Metres {
        Metres::new(self.rms_m[usize::from(m)])
    }

    /// The structural octaves at the unit direction `dir`, octaves [`first_octave`] to `through`
    /// (inclusive, and never past [`finest_octave`]): their height above the base elevation,
    /// metres, and its gradient in body-fixed space at the spheroid point, metres per metre, as
    /// [`interp::base_elevation`]'s is (see the module documentation).
    ///
    /// `cache` is the caller's; the result is independent of what it holds. With `through` below
    /// the first octave the relief is zero and reads nothing.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell the styles' interpolant reads that `field` does not
    /// hold: the base elevation's cells, in its order.
    ///
    /// # Panics
    ///
    /// When an octave is evaluated: if `dir` is zero or has a component that is not finite, or a
    /// height is not finite (a bug).
    pub fn at<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        cache: &mut SynthCache,
    ) -> Result<HeightSample, ReadCellError> {
        self.at_with(field, dir, through, cache.lattice_mut())
    }

    /// [`at`](Self::at), reading the noise through `lattice`.
    fn at_with<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        lattice: &mut LatticeCache,
    ) -> Result<HeightSample, ReadCellError> {
        if through.min(self.finest) < self.first {
            return Ok(HeightSample {
                height_m: 0.0,
                gradient: [0.0; 3],
            });
        }
        let key = NoiseKey::Relief(self.seed);
        let seed = self.seed;
        let table = lattice.take_octaves(key, || octave_table(seed));
        let sample = self.sum(field, dir, through, &table, lattice);
        lattice.restore_octaves(key, table);
        sample
    }

    /// The sum of the octaves' terms ([`each_term`](Self::each_term)) in octave order, its
    /// gradient as a function of direction, each asserted finite.
    fn sum<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        table: &[Octave],
        lattice: &mut impl Lattice,
    ) -> Result<HeightSample, ReadCellError> {
        let mut height = 0.0;
        let mut gradient = [0.0; 3];
        self.each_term(field, dir, through, table, lattice, |term| {
            height += term.height.value;
            for (g, d) in gradient.iter_mut().zip(term.height.gradient) {
                *g += d;
            }
        })?;
        let gradient = as_function_of_direction(field.header(), dir, gradient);
        Ok(HeightSample {
            height_m: num::assert_finite(height),
            gradient: gradient.map(num::assert_finite),
        })
    }

    /// Each octave's term at `dir`, octaves [`first_octave`] to `through` (never past the finest)
    /// of `table`, handed to `emit` in octave order, the styles interpolated from `field` first;
    /// nothing is read when no octave is asked for.
    fn each_term<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        table: &[Octave],
        lattice: &mut impl Lattice,
        emit: impl FnMut(Term),
    ) -> Result<(), ReadCellError> {
        let last = through.min(self.finest);
        if last < self.first {
            return Ok(());
        }
        debug_assert_eq!(
            first_octave(field.header().level()),
            self.first,
            "a relief read with a field other than the one whose header made it"
        );
        let style = StyleSample {
            amplitude: smooth(field, dir, Component::Amplitude)?,
            belt: smooth(field, dir, Component::Belt)?,
            shear: smooth(field, dir, Component::Shear)?,
        };
        self.octave_terms(
            table,
            field.header().figure().point(dir),
            &style,
            last,
            lattice,
            emit,
        );
        Ok(())
    }

    /// Octaves [`first_octave`](Self::first_octave) to `last` of `table` at the spheroid point
    /// `point` (metres), in the style `style`: each octave's term handed to `emit` in octave order
    /// (see the module documentation).
    fn octave_terms(
        &self,
        table: &[Octave],
        point: [f64; 3],
        style: &StyleSample,
        last: u8,
        lattice: &mut impl Lattice,
        mut emit: impl FnMut(Term),
    ) {
        let radius = self.radius_m;
        let unit = point.map(|c| c / radius);
        let theta = FRAC_PI_2 * style.belt.value;
        let theta_gradient = style.belt.gradient.map(|g| FRAC_PI_2 * g);
        let (sin, cos) = math::sin_cos(theta);
        let shear = style.shear;
        // The three octaves above the one evaluated, nearest first: their n̂ and its gradient.
        let mut above = [Smooth::constant(0.0); 3];
        // The raw ridge r of the octave above, which weights this one's ridged term.
        let mut coarser_ridge: Option<Smooth> = None;
        for m in self.first..=last {
            let lambda = spacing_in_radii(m);
            let warped = m - self.first >= WARP_START;
            let scale = WARP_SCALE * lambda;
            let at = if warped {
                [0, 1, 2].map(|k| unit[k] + shear.value * scale * above[k].value)
            } else {
                unit
            };
            let (n, lattice_gradient) = lattice.noise(&table[usize::from(m)], at);
            // ∂n/∂P = g ÷ R + J_Wᵀ g, with g the lattice gradient and J_W the warp's Jacobian in
            // metres: ∂W_k/∂P = c λ_m (s ∇n̂_k + n̂_k ∇s).
            let mut noise_gradient = lattice_gradient.map(|d| d / radius);
            if warped {
                for (k, along) in lattice_gradient.iter().enumerate() {
                    for (axis, out) in noise_gradient.iter_mut().enumerate() {
                        *out += along
                            * scale
                            * (shear.value * above[k].gradient[axis]
                                + above[k].value * shear.gradient[axis]);
                    }
                }
            }
            let plain = Smooth {
                value: n / NOISE_RMS,
                gradient: noise_gradient.map(|d| d / NOISE_RMS),
            };
            let (raw, raw_slope) = ridge(n);
            let centred = Smooth {
                value: (raw - RIDGE_MEAN) / RIDGE_RMS,
                gradient: noise_gradient.map(|d| raw_slope / RIDGE_RMS * d),
            };
            let ridged = match coarser_ridge {
                None => centred,
                Some(coarser) => {
                    let (w, w_slope) = ridge_weight(coarser.value);
                    Smooth {
                        value: w * centred.value / RIDGE_WEIGHT_RMS,
                        gradient: [0, 1, 2].map(|k| {
                            (w * centred.gradient[k]
                                + centred.value * w_slope * coarser.gradient[k])
                                / RIDGE_WEIGHT_RMS
                        }),
                    }
                }
            };
            let swing = cos * ridged.value - sin * plain.value;
            let mixed = Smooth {
                value: cos * plain.value + sin * ridged.value,
                gradient: [0, 1, 2].map(|k| {
                    cos * plain.gradient[k] + sin * ridged.gradient[k] + swing * theta_gradient[k]
                }),
            };
            let sigma = self.rms_m[usize::from(m)];
            let amplitude = style.amplitude;
            let height = Smooth {
                value: amplitude.value * sigma * mixed.value,
                gradient: [0, 1, 2].map(|k| {
                    sigma
                        * (mixed.value * amplitude.gradient[k]
                            + amplitude.value * mixed.gradient[k])
                }),
            };
            emit(Term {
                octave: m,
                plain,
                ridged,
                mixed,
                height,
            });
            above = [plain, above[0], above[1]];
            coarser_ridge = Some(Smooth {
                value: raw,
                gradient: noise_gradient.map(|d| raw_slope * d),
            });
        }
    }

    /// Sets `cache` to hold the lattice corners of octaves [`first_octave`] to `through` within
    /// `radius` of the spheroid point of the unit direction `centre` of `field`'s body, so that
    /// queries there hash each corner once (R05's `LatticeCache::cover`), as a patch bake covers
    /// its patch. No height depends on it.
    ///
    /// # Panics
    ///
    /// If `centre` is not finite, or `radius` is not finite and non-negative.
    pub fn cover<F: FieldView>(
        &self,
        field: &F,
        centre: [f64; 3],
        radius: Metres,
        through: u8,
        cache: &mut SynthCache,
    ) {
        assert!(
            centre.iter().all(|c| c.is_finite())
                && radius.value().is_finite()
                && radius.value() >= 0.0,
            "a covered ball must be finite, got {centre:?} and {} m",
            radius.value()
        );
        let point = field
            .header()
            .figure()
            .point(centre)
            .map(|c| c / self.radius_m);
        let reach = radius.value() / self.radius_m;
        let lattice = cache.lattice_mut();
        let key = NoiseKey::Relief(self.seed);
        let seed = self.seed;
        let table = lattice.take_octaves(key, || octave_table(seed));
        for m in self.first..=through.min(self.finest) {
            let octave = &table[usize::from(m)];
            // A warped octave is read up to its warp's reach beyond the point: each axis's
            // displacement is at most c λ B ÷ σ_noise, so the whole at most √3 times it.
            let warp = WARP_SCALE * octave.spacing() * 3.0_f64.sqrt() * NOISE_BOUND / NOISE_RMS;
            lattice.cover(octave, point, reach + warp);
        }
        lattice.restore_octaves(key, table);
    }
}

/// Σ l^−β over every integer degree l from `from` up, for β above 1 and `from` at least 1: the
/// terms below 32 summed directly in increasing order, then the Euler–Maclaurin tail from the next
/// degree x, x^(1−β) ÷ (β − 1) + x^−β ÷ 2 + β x^(−β−1) ÷ 12 − β(β + 1)(β + 2) x^(−β−3) ÷ 720 +
/// β(β + 1)(β + 2)(β + 3)(β + 4) x^(−β−5) ÷ 30,240 (the Bernoulli numbers B₂, B₄ and B₆), whose
/// next term, β(β + 1)⋯(β + 6) x^(−β−7) ÷ 1,209,600, is under 2 × 10⁻¹² of the tail at x = 32
/// for β up to 4, and far less at the relief's degrees, 70 and above.
#[must_use]
fn degree_tail(beta: f64, from: u64) -> f64 {
    assert!(from >= 1, "degrees start at 1");
    tail_from(beta, degree_value(from))
}

/// The degree `degree` as an `f64`: exact below 2⁵³, which every degree a field reads is (its
/// finest Nyquist degree is below 2⁴²).
#[must_use]
fn degree_value(degree: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the relief's degrees are below 2^42, exact in f64; a degree beyond 2^53 rounds to \
                  the nearest f64, an error under 2^-53 in x, which the tail, smooth in x, \
                  carries as a relative error of the same order"
    )]
    let x = degree as f64;
    x
}

/// [`degree_tail`] from the degree `from`, an integer at least 1 held as an `f64` (as every `f64`
/// from 2⁵³ up is), so that a break degree beyond `u64`'s range is not saturated.
#[must_use]
fn tail_from(beta: f64, from: f64) -> f64 {
    /// Degrees below this are summed term by term.
    const DIRECT: f64 = 32.0;
    debug_assert!(from >= 1.0, "degrees start at 1, not {from}");
    let mut sum = 0.0;
    let mut x = from;
    while x < DIRECT {
        sum += math::powf(x, -beta);
        x += 1.0;
    }
    let b = beta;
    let p = math::powf(x, -beta);
    let x3 = x * x * x;
    let tail = x * p / (b - 1.0) + p / 2.0 + b * p / x / 12.0
        - b * (b + 1.0) * (b + 2.0) * p / x3 / 720.0
        + b * (b + 1.0) * (b + 2.0) * (b + 3.0) * (b + 4.0) * p / (x3 * x * x) / 30_240.0;
    sum + tail
}

/// The relief's arithmetic of the spectrum: its variance over a range of degrees, and each level's
/// band, each a sum of the two segments' power-law tails (see the module documentation).
impl BandSpectrum {
    /// L\* = ⌈`l_b`⌉, the first integer degree of the second segment, an integer as an `f64`.
    #[must_use]
    fn second_segment_start(&self) -> f64 {
        self.break_degree().ceil()
    }

    /// The variance of every degree from `degree` up, Σ V(l) for l ≥ `degree`, square metres, in
    /// closed form: V₁ [T(β₁, d) − T(β₁, L\*) + `l_b`^(β₂−β₁) T(β₂, L\*)] below L\* = ⌈`l_b`⌉ and
    /// V₁ `l_b`^(β₂−β₁) T(β₂, d) from it, each T an Euler–Maclaurin tail exact to about 10⁻¹²
    /// relative.
    ///
    /// # Panics
    ///
    /// If `degree` is 0.
    #[must_use]
    pub fn variance_from_degree(&self, degree: u64) -> SquareMetres {
        assert!(degree >= 1, "degrees start at 1");
        let start = self.second_segment_start();
        let small = self.small_scale_exponent();
        let tail = if degree_value(degree) < start {
            let large = self.exponent();
            (degree_tail(large, degree) - tail_from(large, start))
                + self.break_factor() * tail_from(small, start)
        } else {
            self.break_factor() * degree_tail(small, degree)
        };
        SquareMetres::new(self.unit_degree_variance().value() * tail)
    }

    /// The variance of the degrees from `from` up to but not including `to`, Σ V(l) for
    /// `from` ≤ l < `to`, square metres: the difference of the two tails
    /// ([`variance_from_degree`](Self::variance_from_degree)), taken segment by segment so that
    /// the terms the two share cancel exactly rather than in rounding.
    ///
    /// # Panics
    ///
    /// If `from` is 0 or above `to`.
    #[must_use]
    pub fn band_variance(&self, from: u64, to: u64) -> SquareMetres {
        assert!(
            from <= to,
            "a band of degrees {from} to {to} runs backwards"
        );
        let start = self.second_segment_start();
        let large = self.exponent();
        let small = self.small_scale_exponent();
        let band = if degree_value(to) <= start {
            degree_tail(large, from) - degree_tail(large, to)
        } else if degree_value(from) >= start {
            self.break_factor() * (degree_tail(small, from) - degree_tail(small, to))
        } else {
            (degree_tail(large, from) - tail_from(large, start))
                + self.break_factor() * (tail_from(small, start) - degree_tail(small, to))
        };
        SquareMetres::new(self.unit_degree_variance().value() * num::max(band, 0.0))
    }

    /// The variance of level `level`'s band, from level `level` − 1's Nyquist degree up to but not
    /// including its own ([`nyquist_degree`]): the variance the relief's octave of that level
    /// carries at the reference style.
    ///
    /// # Panics
    ///
    /// If `level` is 0 or above 40.
    #[must_use]
    pub fn level_variance(&self, level: u8) -> SquareMetres {
        assert!(level >= 1, "level 0 has no band below a coarser level");
        self.band_variance(nyquist_degree(level - 1), nyquist_degree(level))
    }
}

#[cfg(test)]
mod tests;
