//! The bake on real terrain (plan R10, T6.a): R05's patch bake over R09's synthesis of a client's
//! field, with this plan's layers.
//!
//! [`bake_terrain`] is R05's [`bake_patch`](crate::patch::bake_patch), the heights, morph targets,
//! positions, normals, bounds and skirts of R05's Design notes 4 to 6, pointed through R10.T3's
//! [`HeightSource`] for R09's [`Synthesiser`] at R09's own bake of a patch's heights, parent-band
//! heights and gradients ([`Synthesiser::bake_patch`], the structural octaves' lattice covered once
//! for the patch first), and extended with [`TerrainLayers`]: the class palette and weights
//! (R10.T2's [`weights_at`](crate::material::weights_at), at each vertex, from the height the bake
//! has made there), the ground's maturity at the patch's level and its parent's (R10.T1.c's
//! [`patch_maturity`], at each vertex with weights, after them and through the same cache), the
//! survey mask, the patch's bound and σ, the horizon map (R10.T8.a's
//! [`horizon`] module, from the bake's own heights and the ground beyond the patch), and R11's
//! layers through [`BakeHook`]. Normals are at R05's
//! [`NormalScale`](crate::patch::NormalScale), which the worker packs as `rg16float` (R05's Design
//! note 5).
//!
//! # The survey
//!
//! The view is a client's field, which knows how each cell it holds was surveyed ([`SurveyView`],
//! R10.T5). A vertex is surveyed where its coarse cell is, and its mask is then [`SURVEYED`], else
//! [`UNSURVEYED`] (Design note 12). The image of surveyed ground is the ground at the patch's level
//! whatever resolution surveyed it, so a cell's resolution code changes no byte of a bake (Design
//! note 6), and a block of new cells changes no value at a vertex surveyed before it, since cells
//! arrive exact and whole with their margins (Design note 16).
//!
//! A vertex outside the survey is drawn by nothing, the shader discarding it, but the triangles
//! from it to surveyed vertices are drawn up to the mask's half, so it carries the ground where the
//! view can make it, a margin cell's ground being the server's to the bit: its height, gradient,
//! parent-band height and weights. Where the view cannot (a cell its reads reach that the view
//! does not hold), it takes the reference surface instead, the datum itself, height 0 with its
//! normal (Design note 12's "unsurveyed ground is the reference surface"), and no weights, and the
//! bake is not [complete](TerrainLayers::is_complete). Each query takes it on its own: a vertex's
//! morph target is its parent level's height there, the reference surface where the parent level
//! cannot be made, which is what the parent patch bakes at that vertex, so a child at morph one
//! lies on its parent's mesh across the survey's edge too, as R05's morph requires. A surveyed
//! vertex the view cannot make is an error: its view breaks the margin's contract.
//!
//! # Determinism
//!
//! Every value is a pure function of the view's cells, the body's detail seed and the patch: the
//! same bytes in any order of bakes and from any cache, on every target. The weights and the
//! maturity are presentation (the plan's Generator version section); nothing is drawn from a
//! stream, so the module opens no domain tag.

pub mod horizon;
mod layers;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use hyperion_base::units::Metres;

pub use horizon::{
    AZIMUTH_STEPS, AZIMUTHS, FAR_SIDE, FAR_STRIDE, HorizonLayer, HorizonMap, NO_HORIZON,
    WORDS_PER_TEXEL, ZENITH_TOLERANCE, decode_horizon, encode_horizon, far_field_point,
    grid_azimuth,
};
pub use layers::{SURVEYED, TerrainLayers, UNSURVEYED, decode_maturity, encode_maturity};

use crate::cube::{PATCH_QUADS, PatchKey};
use crate::field::ResolutionCode;
use crate::height::HeightSample;
use crate::material::{MAX_CLASSES, kept_omitted_rms, patch_maturity, weights_with_sample};
use crate::num;
use crate::patch::{BakeOptions, HeightSource, PATCH_VERTICES, PatchBake, bake_with_heights};
use crate::reading::SurveyView;
use crate::scatter::ExtraLayer;
use crate::spheroid::Spheroid;
use crate::synth::{BandLevel, PatchHeights, QueryHeightError, SynthCache, Synthesiser};

/// What R11 adds to a patch's bake: its layers, made in the same bake and kept in the same cache
/// (plan R10, Provides). R11's `RockBakeHook` owns its own [`SynthCache`], so a hook takes the
/// patch and its heights alone.
pub trait BakeHook {
    /// Appends `key`'s layers to `out`, from the patch's heights as R05's bake lays them out
    /// ([`PatchBake::heights`]: each vertex's own height and morph target, metres, row by row).
    fn bake(&self, key: PatchKey, heights: &[f32], out: &mut Vec<ExtraLayer>);
}

/// Whether a bake makes the class weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeightsLayer {
    /// The palette and each vertex's weights, for the lit style (R10.T10).
    Baked,
    /// None: the palette is empty and the weights are not made, for a style that draws no
    /// material, as the wireframe's depth-only terrain (R10.T11).
    Skipped,
}

/// What [`bake_terrain`] makes: R05's options, and whether the class weights and the horizon map
/// are baked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainBakeOptions {
    /// R05's: the vertex path, the normals' resolution and the skirts' margin.
    pub patch: BakeOptions,
    /// Whether the class weights are baked.
    pub weights: WeightsLayer,
    /// Whether the horizon map is baked (R10.T8.a), on either setting.
    pub horizon: HorizonLayer,
}

/// A patch baked on real terrain: R05's arrays and this plan's layers.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainBake {
    patch: PatchBake,
    layers: TerrainLayers,
}

impl TerrainBake {
    /// R05's arrays: heights, morph targets, offsets, normals, bounds and skirts.
    #[must_use]
    pub const fn patch(&self) -> &PatchBake {
        &self.patch
    }

    /// This plan's layers.
    #[must_use]
    pub const fn layers(&self) -> &TerrainLayers {
        &self.layers
    }

    /// The two parts, to hand on without a copy.
    #[must_use]
    pub fn into_parts(self) -> (PatchBake, TerrainLayers) {
        (self.patch, self.layers)
    }
}

/// The reference surface, where a vertex outside the survey has no ground the view can make: the
/// datum itself, level (Design note 12).
const REFERENCE_SURFACE: HeightSample = HeightSample {
    height_m: 0.0,
    gradient: [0.0; 3],
};

/// Bakes `key` over the client's field `synth` reads, with the layers `opts` asks for and each of
/// `hooks`' (see the [module documentation](self)).
///
/// `cache` is the caller's, a worker's for the body; no byte depends on what it holds, so long as
/// it serves views of one field whose held cells only grow, as [`SynthCache`]'s rule is and a
/// worker's view is: a cache filled under a view holding more cells than this one could answer a
/// query this view refuses, and so bake real ground where this view's bake takes the reference
/// surface. It keeps the horizon's far-field vertex heights too ([`horizon`]), which a patch's
/// neighbours and children read again.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell the view does not hold that a surveyed
/// vertex's height, normal, weights or σ reads: a view whose survey lacks its margin.
///
/// # Panics
///
/// If `opts.patch.skirt_m` is negative or not finite, as R05's bake; or if a height is not finite
/// (a bug).
pub fn bake_terrain<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    opts: &TerrainBakeOptions,
    hooks: &[&dyn BakeHook],
) -> Result<TerrainBake, QueryHeightError> {
    let level = band(key.level());
    let survey_mask = survey_mask(synth.field(), key);
    let ground = SurveyedGround {
        synth,
        mask: &survey_mask,
        reference: RefCell::new(vec![false; survey_mask.len()]),
        complete: Cell::new(true),
    };
    let (patch, heights) = bake_with_heights(&ground, key, &opts.patch, cache)?;
    let SurveyedGround {
        reference,
        complete,
        ..
    } = ground;
    let (reference, mut complete) = (reference.into_inner(), complete.get());
    // Whether every height was the view's own, before the weights' reads.
    let heights_complete = complete;

    let (palette, weights, maturity) = match opts.weights {
        WeightsLayer::Baked => {
            let mut vertices = Vec::with_capacity(survey_mask.len());
            // Each vertex's direction where it has weights, for the maturity.
            let mut taken = Vec::with_capacity(survey_mask.len());
            for (n, sample) in heights.samples().iter().enumerate() {
                let (weights, dir) = if reference[n] {
                    (None, None)
                } else {
                    let dir = vertex_dir(key, n);
                    match weights_with_sample(synth, cache, dir, level, sample) {
                        Ok(weights) => (Some(weights), Some(dir)),
                        Err(e) if survey_mask[n] == SURVEYED => return Err(e),
                        Err(_) => {
                            // Outside the survey, where the view lacks a cell the weights read:
                            // none, until a block brings it.
                            complete = false;
                            (None, None)
                        }
                    }
                };
                vertices.push(weights);
                taken.push(dir);
            }
            let (palette, weights) = layers::gather(&vertices);
            (palette, weights, maturity_layer(synth, cache, key, &taken)?)
        }
        WeightsLayer::Skipped => ([None; MAX_CLASSES], Vec::new(), Vec::new()),
    };
    let sigma = sigma(synth, cache, key, level, &survey_mask)?;
    let horizon = match opts.horizon {
        HorizonLayer::Baked => horizon::bake_horizon(synth, cache, key, &heights, heights_complete),
        HorizonLayer::Skipped => HorizonMap::default(),
    };
    let mut extra = Vec::new();
    for hook in hooks {
        hook.bake(key, &patch.heights, &mut extra);
    }
    let layers = TerrainLayers {
        palette,
        weights,
        maturity,
        survey_mask,
        horizon,
        bound: Metres::new(synth.level_bound_m(level)),
        sigma,
        complete,
        extra,
    };
    Ok(TerrainBake { patch, layers })
}

/// The patch's maturity layer ([`TerrainLayers::maturity`]): each vertex's pair of
/// [`patch_maturity`] at the directions `taken` gives the vertices with weights, encoded against
/// the header's `m_body`, or empty where the header carries none.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] where the view does not hold a taken vertex's coarse cell,
/// which its weights have read already.
fn maturity_layer<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    taken: &[Option<[f64; 3]>],
) -> Result<Vec<[u8; 2]>, QueryHeightError> {
    let Some(pairs) = patch_maturity(synth, cache, key, taken)? else {
        return Ok(Vec::new());
    };
    let background = synth
        .field()
        .header()
        .maturity()
        .expect("a patch's maturity is the header's");
    Ok(pairs
        .iter()
        .map(|pair| pair.map(|m| encode_maturity(m, background)))
        .collect())
}

/// R05's `u8` level of a patch as R09's band level.
#[must_use]
fn band(level: u8) -> BandLevel {
    BandLevel::new(level).expect("a patch's level is a band level")
}

/// The direction of the patch's `n`th vertex in row order.
#[must_use]
fn vertex_dir(key: PatchKey, n: usize) -> [f64; 3] {
    let index = |k: usize| u8::try_from(k).expect("a vertex index along a side is at most 64");
    key.vertex_dir(index(n % PATCH_VERTICES), index(n / PATCH_VERTICES))
}

/// Whether the field's level cell holding the unit direction `dir` is surveyed.
#[must_use]
fn surveyed_at<F: SurveyView>(field: &F, dir: [f64; 3]) -> bool {
    let level = field.header().level().get();
    let cell = PatchKey::containing(level, dir).expect("a coarse level is a quadtree level");
    field
        .resolution(cell)
        .is_some_and(ResolutionCode::is_surveyed)
}

/// Each vertex's survey mask, row by row ([`TerrainLayers::survey_mask`]).
#[must_use]
fn survey_mask<F: SurveyView>(field: &F, key: PatchKey) -> Vec<u8> {
    let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
    (0..=quads)
        .flat_map(|y| (0..=quads).map(move |x| (x, y)))
        .map(|(x, y)| {
            if surveyed_at(field, key.vertex_dir(x, y)) {
                SURVEYED
            } else {
                UNSURVEYED
            }
        })
        .collect()
}

/// The patch's σ ([`TerrainLayers::sigma`]): the largest omitted RMS at `level` over the cells of
/// its surveyed vertices, each cell once, from the closed forms its weights keep in `cache`.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] where the view does not hold a surveyed vertex's cell (a view
/// that breaks [`SurveyView`]'s contract).
fn sigma<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    level: BandLevel,
    mask: &[u8],
) -> Result<Metres, QueryHeightError> {
    let coarse = synth.field().header().level().get();
    let cells: BTreeSet<PatchKey> = mask
        .iter()
        .enumerate()
        .filter(|&(_, &m)| m == SURVEYED)
        .map(|(n, _)| {
            PatchKey::containing(coarse, vertex_dir(key, n))
                .expect("a coarse level is a quadtree level")
        })
        .collect();
    let mut largest = 0.0_f64;
    for cell in cells {
        largest = num::max(
            largest,
            kept_omitted_rms(synth, cache, cell, level)?.value(),
        );
    }
    Ok(Metres::new(largest))
}

/// R10.T3's height source for R09's synthesiser, seen through a client's survey: a vertex outside
/// it that the view cannot make takes the reference surface (the [module documentation](self)).
struct SurveyedGround<'s, 'a, F> {
    synth: &'s Synthesiser<'a, F>,
    /// The patch's survey mask, row by row.
    mask: &'s [u8],
    /// Each vertex whose own-level height took the reference surface, row by row.
    reference: RefCell<Vec<bool>>,
    /// Whether every query was answered by the view.
    complete: Cell<bool>,
}

impl<F: SurveyView> SurveyedGround<'_, '_, F> {
    /// `answer`, or the reference surface where it is the view's refusal of a point outside the
    /// survey (`surveyed` false).
    ///
    /// # Errors
    ///
    /// `answer`'s, at a surveyed point.
    fn or_reference(
        &self,
        answer: Result<HeightSample, QueryHeightError>,
        surveyed: bool,
    ) -> Result<HeightSample, QueryHeightError> {
        match answer {
            Ok(sample) => Ok(sample),
            Err(e @ QueryHeightError::NotSurveyed(_)) => {
                if surveyed {
                    Err(e)
                } else {
                    self.complete.set(false);
                    Ok(REFERENCE_SURFACE)
                }
            }
        }
    }
}

impl<F: SurveyView> HeightSource for SurveyedGround<'_, '_, F> {
    type Cache = SynthCache;
    type Error = QueryHeightError;

    fn figure(&self) -> Spheroid {
        HeightSource::figure(self.synth)
    }

    /// R09's height at `dir`, or the reference surface outside the survey where the view cannot
    /// make it: what the double-resolution normals read between the vertices.
    fn height(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: u8,
    ) -> Result<HeightSample, QueryHeightError> {
        let answer = self.synth.height_at(cache, dir, band(level));
        self.or_reference(answer, surveyed_at(self.synth.field(), dir))
    }

    fn level_bound_m(&self, level: u8) -> f64 {
        HeightSource::level_bound_m(self.synth, level)
    }

    fn height_range_m(&self, key: PatchKey) -> (f64, f64) {
        HeightSource::height_range_m(self.synth, key)
    }

    fn prepare_cache(&self, cache: &mut SynthCache, key: PatchKey) {
        HeightSource::prepare_cache(self.synth, cache, key);
    }

    /// R09's bake of the patch's heights, each query answered as [`height`](Self::height) answers,
    /// the survey read from the mask by vertex.
    fn patch_heights(
        &self,
        cache: &mut SynthCache,
        key: PatchKey,
    ) -> Result<PatchHeights, QueryHeightError> {
        PatchHeights::from_queries(key, |q| {
            let n = usize::from(q.y) * PATCH_VERTICES + usize::from(q.x);
            let answer = self.synth.height_at(cache, q.dir, band(q.level));
            let refused = answer.is_err();
            let sample = self.or_reference(answer, self.mask[n] == SURVEYED)?;
            if refused && q.level == key.level() {
                self.reference.borrow_mut()[n] = true;
            }
            Ok(sample)
        })
    }
}

#[cfg(test)]
mod drawn_ground;
#[cfg(test)]
mod golden;
#[cfg(test)]
mod tests;
