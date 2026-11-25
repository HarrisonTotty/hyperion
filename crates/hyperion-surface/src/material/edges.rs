//! The class edges' warp (lane fix-jagged-biome-edges; the owner's report of 2026-11-10, "the
//! boundary line between biomes is drawn very jagged"): where the classifier reads a point's coarse
//! cell's record, it reads the record of the cell holding the point moved by a fractal warp of the
//! body's detail seed, so that a boundary the records decide is a natural line within a cell of the
//! cells' own, and no longer their staircase.
//!
//! # The cause
//!
//! The classifier takes every rule's input but the point's height, steepness, ice and channel bed
//! from its coarse cell ([`Site`](super::classify)): the water's surface, the climate, the crust,
//! the closed basin and the ice cover. Two classes so decided meet along the cells' edges, at
//! every level: the class map's texels and every patch's vertices hold a staircase of the field's
//! cells (level 8, 39 km on an Earth-sized body), which no level finer than the field's resolves.
//!
//! # The warp
//!
//! The record is the one of the cell holding the point moved along
//! the sphere by w, a sum over [`WARP_OCTAVES`] octaves of R05's gradient noise ([`noise`]) on the
//! unit sphere of each one's gradient in its lattice's units, octave m of spacing
//! [`BASE_SPACING_CELLS`] cells ÷ 2ᵐ and amplitude [`WARP_GAIN`]ᵐ, scaled to [`WARP_RMS_CELLS`] of a
//! cell a component over the whole sum, its tangential part held to [`MAX_WARP_CELLS`] of a cell:
//! one noise a point and octave gives the warp's three components, its gradient's (a warp three
//! octaves' values a component cost a bake half as much again, pod, 2026-11-10). A cell here is
//! the cube sphere's smallest side, [`SMALLEST_CELL_SIDE`] ÷ 2ᴸ, so a point reads its own cell or
//! one a king move away anywhere on the sphere, which a client's view holds wherever it holds the
//! point's (its survey's margin,
//! [`SYNTHESIS_MARGIN_CELLS`](crate::field::SYNTHESIS_MARGIN_CELLS)). The heights, the slopes, the
//! channels, the omitted closed forms and the ice cut's grid stay the point's own cell's: the warp
//! moves only where a record's decisions apply, and reads no grid past the point's margin.
//!
//! The warp is band-limited to the level asked: octave m is drawn at band level n where its
//! spacing is at least twice the level's vertex spacing, m ≤ n + 6 − L for a field of level L, so
//! that a patch's vertices sample it without aliasing and a finer level adds the finer octaves,
//! which the terrain's morph blends in; the class map's texels at band 0 read no octave, and its
//! refined cells' corners at band L − 6 the first.
//!
//! # The version
//!
//! On the record from generator version 34 (train 34), which retired its hold: at 31 to 33 every
//! point read its own cell and no word of the warp's stream was drawn.
//!
//! # Streams
//!
//! The warp's lattices are keyed on `surface.class_edges` ([`NoiseKey::ClassEdges`]): octave m's
//! rotation is [`noise::rotation`]'s and its offset words 2⁴⁰ + 4m + axis of item 0, as R09's
//! relief takes its own.

use hyperion_base::rng::DetailSeed;

use crate::cube::{PatchKey, unit_dir};
use crate::field::{FieldHeader, FieldView, SynthesisCell};
use crate::noise::{self, LatticeCache, NoiseKey, Octave, gradient_noise};
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};

/// The warp's octaves: from [`BASE_SPACING_CELLS`] cells to 2⁻⁵ of that, a sixteenth of a cell,
/// 2.4 km at a level-8 field on an Earth-sized body. Below it a boundary is the warp's smooth
/// line; the drawn edges' own noise shapes it within a patch's quad.
pub(crate) const WARP_OCTAVES: usize = 6;

/// The first octave's lattice spacing, in the field's cells: 2, so that a boundary's largest
/// wiggle spans two cells.
pub(crate) const BASE_SPACING_CELLS: f64 = 2.0;

/// Each octave's amplitude against the one before: 2^(−¾), a warp of Hurst exponent H = ¾
/// (amplitude ∝ λᴴ), the scaling whose local fractal dimension 2 − H (Mandelbrot 1975, PNAS 72,
/// 3825, doi:10.1073/pnas.72.10.3825; Mandelbrot 1985, Physica Scripta 32, 257,
/// doi:10.1088/0031-8949/32/4/001) is 1.25, the west coast of Britain's (Mandelbrot 1967, Science
/// 156, 636–638, doi:10.1126/science.156.3775.636). At [`WARP_RMS_CELLS`] a boundary's slopes stay
/// near or below 1 within the octaves' band, so it measures about 1.05 to 1.1 there; a true 1.25 at
/// a cell's scale would take displacements of several cells, past [`MAX_WARP_CELLS`].
pub(crate) const WARP_GAIN: f64 = 0.594_603_557_501_360_5;

/// The warp's root mean square a component over its whole sum, in cells: a quarter, so that its
/// tangential part's is about 0.35 of a cell and a boundary wanders across the cells' staircase.
pub(crate) const WARP_RMS_CELLS: f64 = 0.25;

/// The longest tangential warp, in cells: 0.9, so that a point reads its own cell or one a king
/// move away.
pub(crate) const MAX_WARP_CELLS: f64 = 0.9;

/// The smallest side of a cell of R05's cube sphere at level 0, rad: 0.94, rounded down from the
/// 0.944 the S2 quadratic warp gives near a face's corners (a scan of a face at 2⁻¹² in s and t,
/// pod, 2026-11-10), against the nominal π ÷ 2 at a face's centre and 1.70 at its widest.
pub(crate) const SMALLEST_CELL_SIDE: f64 = 0.94;

/// The first draw number of the octaves' offsets, above every corner's (as R09's relief).
const OFFSET_DRAW_BASE: u64 = 1 << 40;

/// The root mean square of one body-fixed component of an octave's gradient, per lattice unit,
/// over space and gradient choices: measured over 2¹⁶ points of octave 0's rotated lattice and
/// pinned (`the_noise_gradient_s_rms_is_pinned` re-measures it to 2%).
pub(crate) const GRADIENT_RMS: f64 = 0.715;

/// A body's class edges: the warp a point is moved by before the classifier reads its cell's
/// record (the module's remarks), boxed, its six octaves riding the synthesiser's parts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassEdges(Box<Warp>);

/// A body's class edges' warp: its field's level and cell, its octaves and their scale.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Warp {
    /// The field's level, L.
    level: u8,
    /// A cell's smallest side on the unit sphere, [`SMALLEST_CELL_SIDE`] ÷ 2ᴸ.
    cell: f64,
    /// Octave m at m.
    octaves: [Octave; WARP_OCTAVES],
    /// The scale from the octaves' sum to the warp: [`WARP_RMS_CELLS`] cells over its deviation.
    scale: f64,
}

impl ClassEdges {
    /// The class edges of a body of field header `header` and detail seed `seed`.
    #[must_use]
    pub(crate) fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        Self(Box::new(Warp::new(header, seed)))
    }

    /// Its warp.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn warp(&self) -> &Warp {
        &self.0
    }

    /// The unit direction `dir` moved by the warp at band level `level`, through `lattice`:
    /// `dir` itself where the level draws no octave.
    #[must_use]
    pub(crate) fn warped(
        &self,
        dir: [f64; 3],
        level: BandLevel,
        lattice: &mut LatticeCache,
    ) -> [f64; 3] {
        self.0.warped(dir, level, lattice)
    }

    /// Covers the warp's octaves drawn at band level `level` in `lattice` within `radius` of the
    /// unit direction `centre`, on the unit sphere: the points the warp is asked at, which it reads
    /// its noise at before it moves them. No value depends on it.
    pub(crate) fn cover(
        &self,
        centre: [f64; 3],
        radius: f64,
        level: BandLevel,
        lattice: &mut LatticeCache,
    ) {
        let warp = &self.0;
        for octave in &warp.octaves[..warp.drawn(level)] {
            lattice.cover(octave, centre, radius);
        }
    }
}

impl Warp {
    /// The warp of a body of field header `header` and detail seed `seed`.
    #[must_use]
    fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        let level = header.level().get();
        let cell = SMALLEST_CELL_SIDE / f64::from(1_u32 << level);
        let octaves = core::array::from_fn(|m| {
            octave(seed, u8::try_from(m).expect("WARP_OCTAVES fit a u8"), cell)
        });
        // Σ gain^2m over the octaves, by repeated products: exact to rounding on every target.
        let mut squares = 0.0;
        let mut amplitude = 1.0;
        for _ in 0..WARP_OCTAVES {
            squares += amplitude * amplitude;
            amplitude *= WARP_GAIN;
        }
        Self {
            level,
            cell,
            octaves,
            scale: WARP_RMS_CELLS * cell / (GRADIENT_RMS * squares.sqrt()),
        }
    }

    /// The octaves drawn at band level `level`: m ≤ n + 6 − L, at most [`WARP_OCTAVES`].
    #[must_use]
    fn drawn(&self, level: BandLevel) -> usize {
        let through = i32::from(level.get()) + 6 - i32::from(self.level);
        usize::try_from(through + 1).map_or(0, |n| n.min(WARP_OCTAVES))
    }

    /// The unit direction `dir` moved by the warp at band level `level`, through `lattice`; `dir`
    /// itself where the level draws no octave.
    #[must_use]
    fn warped(&self, dir: [f64; 3], level: BandLevel, lattice: &mut LatticeCache) -> [f64; 3] {
        let drawn = self.drawn(level);
        if drawn == 0 {
            return dir;
        }
        let mut w = [0.0_f64; 3];
        let mut amplitude = 1.0;
        for octave in &self.octaves[..drawn] {
            // The gradient in the lattice's units: the body's times the spacing.
            let (_, gradient) = gradient_noise(dir, octave, lattice);
            let k = amplitude * octave.spacing();
            for (component, g) in w.iter_mut().zip(gradient) {
                *component += k * g;
            }
            amplitude *= WARP_GAIN;
        }
        let w = w.map(|c| c * self.scale);
        // Its part along the sphere, held to MAX_WARP_CELLS of a cell.
        let along = w[0] * dir[0] + w[1] * dir[1] + w[2] * dir[2];
        let t = [0, 1, 2].map(|a| w[a] - along * dir[a]);
        let size = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        let most = MAX_WARP_CELLS * self.cell;
        let held = if size > most { most / size } else { 1.0 };
        unit_dir([0, 1, 2].map(|a| dir[a] + held * t[a]))
    }
}

/// Octave `m` of the warp of the body of detail seed `seed` on a field of cells of side `cell` on
/// the unit sphere: its spacing [`BASE_SPACING_CELLS`] cells ÷ 2ᵐ, its offset from its stream's
/// words.
#[must_use]
fn octave(seed: DetailSeed, m: u8, cell: f64) -> Octave {
    let key = NoiseKey::ClassEdges(seed);
    let offset = [0_u64, 1, 2].map(|axis| {
        let word = key.word(0, OFFSET_DRAW_BASE + 4 * u64::from(m) + axis);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in f64"
        )]
        let top = (word >> 11) as f64;
        top / 9_007_199_254_740_992.0
    });
    let spacing = BASE_SPACING_CELLS * cell / f64::from(1_u32 << m);
    Octave::new(m, spacing, noise::rotation(m), offset, key)
}

impl<'a, F: FieldView> Synthesiser<'a, F> {
    /// The cell whose record the classifier reads at the unit direction `dir`, and its record: the
    /// cell holding `dir` moved by the class edges' warp at the classifier's band, R05's finest
    /// level (the module's remarks), where [`cell_at`](Self::cell_at) gives the point's own. Its
    /// rules' inputs from the record (the water's surface, the climate, the crust, the closed
    /// basin and the ice's share and entry) are this cell's; the heights, the channels and the
    /// ice cut's grid stay the point's own cell's.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the cell the view does not hold.
    ///
    /// # Panics
    ///
    /// As [`cell_at`](Self::cell_at).
    pub fn site_cell_at(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
    ) -> Result<(PatchKey, &'a SynthesisCell), QueryHeightError> {
        let level = super::classify::finest_band(self.field().header());
        site_cell_at(self, cache, dir, level)
    }
}

/// The cell whose record the classifier reads at the unit direction `dir` and band level `level`
/// under `synth`'s class edges, and its record: the cell holding `dir` moved by the warp
/// ([`Warp`]).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the cell the view does not hold.
pub(crate) fn site_cell_at<'a, F: FieldView>(
    synth: &Synthesiser<'a, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    level: BandLevel,
) -> Result<(PatchKey, &'a SynthesisCell), QueryHeightError> {
    synth.cell_at(
        synth
            .class_edges()
            .warped(dir, level, cache.class_edges_mut()),
    )
}

#[cfg(test)]
mod tests;
