//! The class weights a patch bakes (plan R10, T2; Design note 3): at the finest level the class,
//! blended across the slope thresholds for the image only, and at a coarser level the expected
//! fraction of each finest-level class over the texel's footprint.
//!
//! A coarse patch that took the class of its own band-limited ground would lose every cliff, since
//! smoothing flattens them, and a cliffed range would change colour as the camera closed. So
//! [`weights_at`] at a coarser level n asks what the classifier
//! ([`material_at`](super::material_at)) decides of the finest level's ground where the level
//! resolves only part of it: the finest ground is the level's plus what the level leaves out, and
//! the part left out is taken as a Gaussian field of R09's level-keyed closed forms
//! ([`Synthesiser::omitted_variance`] and [`Synthesiser::omitted_structure_function`], added by
//! R10.T5; decision-r10-t5-closed-forms.md) for the height and the slope ([`exceedance`]), and a
//! partial ice share's place from the cell's own grid. [`weights_at`] gives the model.
//!
//! The weights are presentation: they decide nothing generated, and the shader only mixes the
//! photometric parameters it is handed ("the shader blends the class's textures across a transition
//! band but never re-decides the class"). Nothing is drawn from a stream, so the module opens no
//! domain tag.

use core::f64::consts::{SQRT_2, TAU};

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::classify::{
    IcePlacement, Point, Site, Steepness, cos_slope, finest_band, ice_share_at, kept_forms,
    on_channel_floor,
};
use super::{CLIFF_SLOPE, GroundMaterial, SNOW_SHED_SLOPE, TRANSITION_HALF_WIDTH};
use crate::craters::density::GAUSS_LEGENDRE_8;
use crate::cube::{FaceUv, PatchKey, face_uv_to_xyz, unit_dir};
use crate::field::{FieldHeader, FieldView};
use crate::height::HeightSample;
use crate::num;
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};

/// The most materials a point's weights hold: eight, the palette a patch keeps (plan R10,
/// Provides).
pub const MAX_CLASSES: usize = 8;

/// The sum of a point's weights: 255, a byte's whole, so that a texel's weights fill its byte
/// channels and leave nothing to R07's `elsewhere` (R07's "Deviations in T8.b").
pub const WEIGHT_TOTAL: u8 = u8::MAX;

/// The lag at which the omitted ground's structure function gives its slopes' mean square along a
/// direction, D(b) ÷ b²: 5 cm, a fortieth of the 2 m band limit.
///
/// A structure function is ⟨(e · ∇h)²⟩ b² to second order at a lag b short against every
/// wavelength it holds, so the ratio is the slopes' mean square as b falls. At 5 cm every
/// contribution is in that regime: the structural octaves' tables are a power law of exponent 2
/// below a twentieth of an octave's spacing, and every spacing is at least the band limit (R09's
/// "Deviations in T5's second follow-up"); the small craters' J₀ form is within 0.2% of its
/// quadratic limit at the smallest octave, whose ring is at least 2 m; the channels' and the rims'
/// forms are 2V (1 − exp(−s b² ÷ 2V)), whose limit is s b². A test holds the ratio at 5 cm to its
/// value at 2.5 cm within 1%.
pub const SLOPE_LAG: Metres = Metres::new(0.05);

/// The standard deviations beyond which a normal density is taken as zero: 8, where the two tails
/// hold 1.2 × 10⁻¹⁵ of it.
const GAUSSIAN_REACH: f64 = 8.0;

/// The deviations of the normal density each panel of [`exceedance`]'s eight-point Gauss–Legendre
/// spans, at most: 2, where the rule integrates a Gaussian to about 10⁻¹⁰ of it.
const PANEL_DEVIATIONS: f64 = 2.0;

/// The most panels [`exceedance`] integrates over: 16, a bound on its cost. The reach's 16
/// deviations need 8 at [`PANEL_DEVIATIONS`]; the bound binds where the reach meets the disc's
/// edge and the threshold is long against the deviation, since x = t sin θ stretches θ there, and
/// there the error stays below 3 × 10⁻⁷ at σ = 10⁻³ and 3 × 10⁻⁶ at σ = 3 × 10⁻⁵ (the science
/// check's measurement), far below a byte's 1 ÷ 255.
const MAX_PANELS: u32 = 16;

/// A point's class weights: up to [`MAX_CLASSES`] materials, each a form of one of the body's
/// palette entries ([`GroundMaterial`]), with a byte each, the bytes summing to [`WEIGHT_TOTAL`]
/// (plan R10, Design note 3).
///
/// The slots are in falling weight, ties in rising class code and then entry, so the first is the
/// largest weight with ties broken by discriminant; empty slots, `None` with no weight, follow. At
/// the finest level the first is always the authoritative class,
/// [`material_at`](super::material_at)'s. A material whose weight rounds to no byte is not held.
///
/// A slot holds a [`GroundMaterial`], the form always and the entry where the palette names one,
/// as the classifier answers, so that a body whose palette names no substance still has its forms
/// weighted (signoff-9 §17.5's `MATERIAL REGOLITH · —`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassWeights {
    palette: [Option<GroundMaterial>; MAX_CLASSES],
    weights: [u8; MAX_CLASSES],
}

impl ClassWeights {
    /// The weights of these slots, for the tests of what reads weights (R10.T6.a's palette).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn from_slots(
        palette: [Option<GroundMaterial>; MAX_CLASSES],
        weights: [u8; MAX_CLASSES],
    ) -> Self {
        Self { palette, weights }
    }

    /// The materials, slot by slot: `None` where a slot is empty.
    #[must_use]
    pub const fn palette(&self) -> [Option<GroundMaterial>; MAX_CLASSES] {
        self.palette
    }

    /// The weights, slot by slot, summing to [`WEIGHT_TOTAL`]: 0 where a slot is empty.
    #[must_use]
    pub const fn weights(&self) -> [u8; MAX_CLASSES] {
        self.weights
    }

    /// Each material held, with its weight, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (GroundMaterial, u8)> + '_ {
        self.palette
            .iter()
            .zip(self.weights)
            .filter_map(|(m, w)| m.map(|m| (m, w)))
    }

    /// The weight of `material`: 0 where it is not held.
    #[must_use]
    pub fn weight(&self, material: GroundMaterial) -> u8 {
        self.iter()
            .find_map(|(m, w)| (m == material).then_some(w))
            .unwrap_or(0)
    }

    /// The material of the largest weight, ties broken by discriminant: the first slot's. At the
    /// finest level it is the authoritative class.
    ///
    /// # Panics
    ///
    /// Never: weights summing to [`WEIGHT_TOTAL`] hold a material in their first slot, which only
    /// [`weights_at`] builds.
    #[must_use]
    pub fn largest(&self) -> GroundMaterial {
        self.palette[0].expect("a point's weights hold at least one material")
    }

    /// How many materials the weights hold, 1 to [`MAX_CLASSES`].
    #[must_use]
    pub fn len(&self) -> usize {
        self.palette.iter().flatten().count()
    }

    /// Whether the weights hold no material: never, since they sum to [`WEIGHT_TOTAL`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The class weights at `at` for a patch of band level `level`: what share of its footprint each
/// finest-level material is expected to cover (Design note 3).
///
/// - **At the finest level** ([`finest_level`](crate::geometry::finest_level), or deeper): the
///   authoritative class ([`material_at`](super::material_at)), blended linearly across
///   [`TRANSITION_HALF_WIDTH`] either side of [`CLIFF_SLOPE`] and of [`SNOW_SHED_SLOPE`] for the
///   image only: at a slope θ within the band of a threshold θ₀, the steeper ground's material
///   takes (θ − θ₀ + w) ÷ 2w of the weight. The authoritative class always takes the largest
///   weight, ties included; the rules on the height, which have no band of their own, are not
///   blended.
/// - **At a coarser level n**: the expected fraction of each class over the texel's footprint, one
///   vertex spacing of the level's lattice about `at`, from the level's height and gradient at
///   `at`. The finest ground is the level's plus what the level leaves out, which the height and
///   the slope take as a Gaussian field of R09's level-keyed closed forms
///   ([`Synthesiser::omitted_variance`] and [`Synthesiser::omitted_structure_function`], kept in
///   `cache` by cell and level) and the ice from the cell's own grid:
///   - the height is the level's plus a normal deviate of the omitted variance, for the rules on
///     the height: the water surface, and where the temperature by the lapse rate crosses the
///     crust's melt, the snow line and Köppen–Geiger's arid bound;
///   - the tangential gradient is the level's plus an isotropic normal deviate whose variance
///     along each tangent axis is the omitted structure function's small-lag slope, D(b) ÷ b² at
///     [`SLOPE_LAG`], so the probability that the finest slope exceeds [`CLIFF_SLOPE`] or
///     [`SNOW_SHED_SLOPE`] is the Rice distribution's tail;
///   - a partial ice share's place is the point's shadowed-ground index at the level against the
///     cell's cut at the finest level, plus the index the level leaves out, taken from the cell's
///     own 256 grid points (the finest index less the level's at each), since the index is
///     heavy-tailed and a normal deviate puts a small share's ice in the wrong place.
///
///   The three are taken as independent: the height and the gradient of a stationary field at one
///   point are uncorrelated, and the ice index, which reads both, is an approximation there. Each
///   combination of a height band, a steepness and the ice is a case of the classifier's rules,
///   weighted by its probability, and the weights are the cases' materials summed. Where the level
///   leaves nothing out, from the band limit's level on, they are the class at `at` with all of the
///   weight.
///
/// The cell's regime, crust, basin and climate are `at`'s cell's, and a channel's bed is one the
/// level itself evaluates, at `at`: the beds of finer channel levels, narrower than the level's
/// texels, are not counted (the plan's Risks). At most [`MAX_CLASSES`] materials are kept, the
/// likeliest, the rest of the weight spread over them in proportion. `cache` is the caller's, and
/// no answer depends on what it holds.
///
/// A texel of band level n is a cell of quadtree level n + 6, so R07's class map at quadtree level
/// L takes its texels' weights at band level L − 6 (the plan's Risks, "Deviations in T2").
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold.
///
/// # Panics
///
/// If `at` names no direction (a (u, v) that is not finite), or a height is not finite (a bug).
pub fn weights_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: FaceUv,
    level: BandLevel,
) -> Result<ClassWeights, QueryHeightError> {
    let dir = unit_dir(face_uv_to_xyz(at));
    if level > finest_band(synth.field().header()) {
        // Deeper than the finest level, at which the classifier reads the ground: no height at
        // the level itself is read.
        return Ok(finest_weights(&Point::at(synth, cache, dir)?));
    }
    let sample = synth.height_at(cache, dir, level)?;
    weights_with_sample(synth, cache, dir, level, &sample)
}

/// [`weights_at`] at the unit direction `dir`, whose height and gradient at `level`, `sample`,
/// the caller has in hand: [`Synthesiser::height_at`] at `dir` and `level`, bit for bit. R10.T6.a's
/// bake, which has made each vertex's, asks so, and the answer is [`weights_at`]'s at that
/// direction.
///
/// # Errors
///
/// As [`weights_at`], the height's apart.
pub(crate) fn weights_with_sample<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    level: BandLevel,
    sample: &HeightSample,
) -> Result<ClassWeights, QueryHeightError> {
    let finest = finest_band(synth.field().header());
    let point = match level.cmp(&finest) {
        core::cmp::Ordering::Less => return coarse_weights(synth, cache, dir, level, sample),
        core::cmp::Ordering::Equal => Point::with_sample(synth, cache, dir, sample)?,
        // Deeper than the finest level, at which the classifier reads the ground: its height there.
        core::cmp::Ordering::Greater => Point::at(synth, cache, dir)?,
    };
    Ok(finest_weights(&point))
}

/// The weights at the finest level at `point`: the class, blended across the slope thresholds
/// ([`weights_at`]).
#[must_use]
fn finest_weights(point: &Point<'_>) -> ClassWeights {
    let (_, authoritative) = point.facts().decide();
    let slope = math::acos(num::max(-1.0, num::min(point.cos_slope(), 1.0)));
    let mut shares = Shares::default();
    for (steepness, share) in blend(slope) {
        shares.add(point.facts_at(steepness).decide().1, share);
    }
    shares.into_weights(First::Authoritative(authoritative))
}

/// Each steepness's share of the image at a slope of `slope` radians: linear across
/// [`TRANSITION_HALF_WIDTH`] either side of each threshold. The two bands, 35–45° and 55–65°, do
/// not meet, so at most two steepnesses share a slope.
#[must_use]
pub(crate) fn blend(slope: f64) -> [(Steepness, f64); 3] {
    let w = TRANSITION_HALF_WIDTH.value();
    let past = |threshold: f64| num::max(0.0, num::min((slope - threshold + w) / (2.0 * w), 1.0));
    let cliff = past(CLIFF_SLOPE.value());
    let shed = past(SNOW_SHED_SLOPE.value());
    [
        (Steepness::Loose, 1.0 - cliff),
        (Steepness::Cliff, cliff - shed),
        (Steepness::Shed, shed),
    ]
}

/// The weights at a band level `level` coarser than the finest at the unit direction `dir`, whose
/// height and gradient at the level are `sample` ([`weights_at`]).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold: the
/// cell's site's, then the ice cut's grid and the channels'.
fn coarse_weights<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    level: BandLevel,
    sample: &HeightSample,
) -> Result<ClassWeights, QueryHeightError> {
    let field = synth.field();
    let header = field.header();
    let sample = *sample;
    let (key, cell) = synth.cell_at(dir)?;
    let site = Site::of(field, key, cell)?;
    let omitted = Omitted::of(synth, cache, key, level)?;
    let heights = height_cases(&site.height_breaks(), sample.height_m, omitted.height_m2);
    let steepnesses = steepness_cases(header, dir, &sample, omitted.slope2);
    let channel_floor = on_channel_floor(synth, cache, dir, cell, level)?;
    let mut by_cut = None;
    let mut shares = Shares::default();
    for &(h, p_height) in &heights {
        let in_ice = match site.ice_placement(site.under_water(h)) {
            IcePlacement::Settled(lies) => f64::from(u8::from(lies)),
            IcePlacement::ByCut => {
                if let Some(p) = by_cut {
                    p
                } else {
                    let p = ice_share_at(synth, cache, (key, cell), dir, level, &sample)?;
                    by_cut = Some(p);
                    p
                }
            }
        };
        for (ice, p_ice) in [(true, in_ice), (false, 1.0 - in_ice)] {
            for &(steepness, p_steep) in &steepnesses {
                let p = p_height * p_ice * p_steep;
                if p > 0.0 {
                    let facts = site.facts(h, steepness, ice, channel_floor);
                    shares.add(facts.decide().1, p);
                }
            }
        }
    }
    Ok(shares.into_weights(First::ByDiscriminant))
}

/// What a band level leaves out of the finest ground in a cell, as [`weights_at`] reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Omitted {
    /// The height's variance, square metres.
    height_m2: f64,
    /// The slopes' mean square along a direction, square metres a square metre.
    slope2: f64,
}

impl Omitted {
    /// What band level `level` leaves out in the coarse cell `cell`: R09's level-keyed closed forms
    /// (R10.T5, with R09.T7.h's sharpening of the craters a level keeps), the total of
    /// `omitted_variance`, and `omitted_structure_function` at [`SLOPE_LAG`] over the lag squared,
    /// kept in `cache` by cell and level, since the two cost 50 to 150 µs a call.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming `cell` where the view does not hold it, or the
    /// receiver of its trunk one king move away (R09.T6.f), the closed forms' `None`.
    fn of<F: FieldView>(
        synth: &Synthesiser<'_, F>,
        cache: &mut SynthCache,
        cell: PatchKey,
        level: BandLevel,
    ) -> Result<Self, QueryHeightError> {
        let [height_m2, slope2] = kept_forms(synth, cache, cell, level, || {
            let not_held = QueryHeightError::NotSurveyed(cell);
            let variance = synth.omitted_variance(cell, level).ok_or(not_held)?;
            let lag = SLOPE_LAG.value();
            let structure = synth
                .omitted_structure_function(cell, level, SLOPE_LAG)
                .ok_or(not_held)?;
            Ok::<_, QueryHeightError>([
                num::assert_finite(variance.total().value()),
                num::assert_finite(structure.value() / (lag * lag)),
            ])
        })?;
        Ok(Self { height_m2, slope2 })
    }
}

/// R09's [`Synthesiser::omitted_rms`] in the coarse cell `cell` at band level `level`, bit for
/// bit, from the closed forms [`weights_at`] keeps in `cache` by cell and level: what R10.T6.a's
/// bake states as a patch's σ, which then costs nothing more where its weights are baked.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming `cell` where the view does not hold it.
pub(crate) fn kept_omitted_rms<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    cell: PatchKey,
    level: BandLevel,
) -> Result<Metres, QueryHeightError> {
    Ok(Metres::new(
        Omitted::of(synth, cache, cell, level)?.height_m2.sqrt(),
    ))
}

/// The bands of the finest height a point of mean height `mean_m` and omitted variance
/// `variance_m2` may fall in, between the heights `breaks` (ascending) at which a rule may change,
/// each as a height within it at which to ask the rules and its probability: the mean itself where
/// it lies in the band, else a height inside it. With no variance, the mean alone.
#[must_use]
fn height_cases(breaks: &[f64], mean_m: f64, variance_m2: f64) -> Vec<(f64, f64)> {
    if variance_m2 <= 0.0 || breaks.is_empty() {
        return vec![(mean_m, 1.0)];
    }
    let sd = variance_m2.sqrt();
    let mut cases = Vec::with_capacity(breaks.len() + 1);
    let mut low = f64::NEG_INFINITY;
    let mut below = 0.0;
    for high in breaks.iter().copied().chain([f64::INFINITY]) {
        let up_to = if high.is_finite() {
            normal_cdf((high - mean_m) / sd)
        } else {
            1.0
        };
        let p = num::max(0.0, up_to - below);
        let at = if low < mean_m && mean_m < high {
            mean_m
        } else if !low.is_finite() {
            high - sd
        } else if !high.is_finite() {
            low + sd
        } else {
            f64::midpoint(low, high)
        };
        if p > 0.0 {
            cases.push((at, p));
        }
        low = high;
        below = up_to;
    }
    cases
}

/// The steepnesses the finest ground at the unit direction `dir` may take, each with its
/// probability, from the level's ground `sample` and the omitted slopes' mean square along a
/// direction `slope2`: the level's own steepness where nothing is left out.
///
/// The finest ground's tangential gradient is the level's plus an isotropic normal deviate of
/// variance `slope2` along each tangent axis, and its slope's tangent that gradient's magnitude
/// scaled by ρ ÷ (ρ + h), as R05's [`surface_normal`](crate::patch::normals::surface_normal) takes
/// it, with ρ the section radius along the level's gradient (the mean curvature's where it is
/// level).
#[must_use]
fn steepness_cases(
    header: &FieldHeader,
    dir: [f64; 3],
    sample: &HeightSample,
    slope2: f64,
) -> Vec<(Steepness, f64)> {
    if slope2 <= 0.0 {
        return vec![(Steepness::of(cos_slope(header, dir, sample)), 1.0)];
    }
    let figure = header.figure();
    let nu = figure.normal(dir);
    let g = sample.gradient;
    let along = g[0] * nu[0] + g[1] * nu[1] + g[2] * nu[2];
    let tangent = [0, 1, 2].map(|k| g[k] - along * nu[k]);
    let size = (tangent[0] * tangent[0] + tangent[1] * tangent[1] + tangent[2] * tangent[2]).sqrt();
    let rho = if size > 0.0 {
        figure.section_radius_m(dir, tangent.map(|t| t / size))
    } else {
        let (m, n) = figure.curvature_radii_m(dir);
        2.0 / (1.0 / m + 1.0 / n)
    };
    let scale = num::assert_finite(rho / (rho + sample.height_m));
    let tail = |slope: f64| exceedance(size, slope2, math::tan(slope) / scale);
    let cliff = tail(CLIFF_SLOPE.value());
    let shed = num::min(tail(SNOW_SHED_SLOPE.value()), cliff);
    vec![
        (Steepness::Loose, 1.0 - cliff),
        (Steepness::Cliff, cliff - shed),
        (Steepness::Shed, shed),
    ]
}

/// The probability that a two-dimensional normal deviate of mean (`mean`, 0) and variance
/// `variance` = σ² along each axis lies farther than `threshold` = t from the origin: the tail of
/// the Rice distribution, the envelope of a sine wave plus Gaussian noise (Rice 1945, Bell Syst.
/// Tech. J. 24, 46, §3.10, eq. 3.10-11, p. 100), which is Marcum's Q₁(`mean` ÷ σ, t ÷ σ) (Marcum
/// 1960, IRE Trans. Inf. Theory IT-6, 59; not read here).
///
/// It is 1 less the probability of the disc, integrated in closed form along one axis and by
/// Gauss–Legendre along the other, ∫ φ(x − mean) erf(√(t² − x²) ÷ σ√2) dx over |x| ≤ t with φ the
/// normal density of variance σ², by x = t sin θ, which takes the square root's end points out of
/// the integrand, over the part of the disc within [`GAUSSIAN_REACH`] deviations of the mean, in
/// panels of at most [`PANEL_DEVIATIONS`] deviations each, up to [`MAX_PANELS`]. `mean` and
/// `threshold` are non-negative and `variance` positive; a test holds it to the Rayleigh tail
/// exp(−t² ÷ 2σ²) at a zero mean and to an independent polar sum elsewhere, both within 10⁻⁶ over
/// variances of 10⁻⁴ to 1.
#[must_use]
pub(crate) fn exceedance(mean: f64, variance: f64, threshold: f64) -> f64 {
    let sd = variance.sqrt();
    let low = num::max(-threshold, mean - GAUSSIAN_REACH * sd);
    let high = num::min(threshold, mean + GAUSSIAN_REACH * sd);
    if low >= high {
        // The disc lies beyond the deviate's reach: all of it is outside.
        return 1.0;
    }
    let (from, to) = (math::asin(low / threshold), math::asin(high / threshold));
    // About PANEL_DEVIATIONS of the deviate a panel where x = t sin θ stretches it least, at θ = 0.
    let wanted = ((to - from) * threshold / (PANEL_DEVIATIONS * sd)).ceil();
    let panels = num::max(1.0, num::min(wanted, f64::from(MAX_PANELS)));
    let half = 0.5 * (to - from) / panels;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "panels is a whole number from 1 to MAX_PANELS"
    )]
    let panels = panels as u32;
    let mut inside = 0.0;
    for panel in 0..panels {
        let centre = from + f64::from(2 * panel + 1) * half;
        for (node, weight) in GAUSS_LEGENDRE_8 {
            let (sin, cos) = math::sin_cos(centre + half * node);
            let z = (threshold * sin - mean) / sd;
            let chord = threshold * cos;
            inside += weight * math::exp(-0.5 * z * z) * math::erf(chord / (sd * SQRT_2)) * chord;
        }
    }
    let inside = inside * half / (sd * TAU.sqrt());
    num::max(0.0, num::min(1.0 - inside, 1.0))
}

/// The standard normal distribution's cumulative probability at `z`.
#[must_use]
fn normal_cdf(z: f64) -> f64 {
    0.5 * math::erfc(-z / SQRT_2)
}

/// Which material a point's weights put first among equal shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum First {
    /// The authoritative class, at the finest level: it is kept whatever its share, and takes the
    /// largest byte, ties included.
    Authoritative(GroundMaterial),
    /// The lower class code, then the lower entry, at a coarser level.
    ByDiscriminant,
}

/// The probabilities of the materials a point's cases decide, before they are made bytes.
#[derive(Debug, Clone, Default)]
struct Shares {
    shares: Vec<(GroundMaterial, f64)>,
}

impl Shares {
    /// Adds `share` to `material`'s probability.
    fn add(&mut self, material: GroundMaterial, share: f64) {
        if share <= 0.0 {
            return;
        }
        match self.shares.iter_mut().find(|(m, _)| *m == material) {
            Some((_, p)) => *p += share,
            None => self.shares.push((material, share)),
        }
    }

    /// The weights: the [`MAX_CLASSES`] likeliest materials, their shares made bytes summing to
    /// [`WEIGHT_TOTAL`] by the largest remainder, and, where `first` names an authoritative class,
    /// that class given the largest byte, ties included.
    #[must_use]
    fn into_weights(self, first: First) -> ClassWeights {
        let authoritative = match first {
            First::Authoritative(m) => Some(m),
            First::ByDiscriminant => None,
        };
        let mut shares = self.shares;
        let is_other = |m: GroundMaterial| Some(m) != authoritative;
        shares.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        // The authoritative class first, whatever its share, then the likeliest others.
        let mut kept: Vec<(GroundMaterial, f64)> = Vec::with_capacity(MAX_CLASSES);
        if let Some(a) = authoritative {
            let share = shares
                .iter()
                .find(|(m, _)| *m == a)
                .map_or(0.0, |&(_, p)| p);
            kept.push((a, share));
        }
        for &(m, p) in &shares {
            if kept.len() == MAX_CLASSES {
                break;
            }
            if is_other(m) {
                kept.push((m, p));
            }
        }
        let total: f64 = kept.iter().map(|&(_, p)| p).sum();
        assert!(total > 0.0, "a point's cases have a probability");
        let mut bytes = [0_u8; MAX_CLASSES];
        let mut remainders = [0.0; MAX_CLASSES];
        let mut used = 0_u32;
        for (k, &(_, p)) in kept.iter().enumerate() {
            let raw = p / total * f64::from(WEIGHT_TOTAL);
            let whole = num::max(0.0, num::min(raw.floor(), f64::from(WEIGHT_TOTAL)));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "whole is a whole number from 0 to 255"
            )]
            let byte = whole as u8;
            bytes[k] = byte;
            used += u32::from(byte);
            remainders[k] = raw - whole;
        }
        let mut order: Vec<usize> = (0..kept.len()).collect();
        order.sort_by(|&a, &b| remainders[b].total_cmp(&remainders[a]).then(a.cmp(&b)));
        let left = u32::from(WEIGHT_TOTAL).saturating_sub(used);
        for &k in order
            .iter()
            .take(usize::try_from(left).expect("a count fits a usize"))
        {
            bytes[k] += 1;
        }
        if let Some(a) = authoritative {
            let at = kept
                .iter()
                .position(|&(m, _)| m == a)
                .expect("the authoritative class is kept");
            loop {
                let rival = (0..kept.len())
                    .filter(|&k| k != at)
                    .max_by(|&x, &y| bytes[x].cmp(&bytes[y]).then(y.cmp(&x)));
                match rival {
                    Some(r) if bytes[r] >= bytes[at] => {
                        bytes[r] -= 1;
                        bytes[at] += 1;
                    }
                    _ => break,
                }
            }
        }
        let mut slots: Vec<(GroundMaterial, u8)> = kept
            .iter()
            .zip(bytes)
            .filter_map(|(&(m, _), b)| (b > 0).then_some((m, b)))
            .collect();
        slots.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(is_other(a.0).cmp(&is_other(b.0)))
                .then(a.0.cmp(&b.0))
        });
        let mut palette = [None; MAX_CLASSES];
        let mut weights = [0; MAX_CLASSES];
        for (k, (m, b)) in slots.into_iter().enumerate() {
            palette[k] = Some(m);
            weights[k] = b;
        }
        ClassWeights { palette, weights }
    }
}

#[cfg(test)]
mod golden;
#[cfg(test)]
mod tests;
