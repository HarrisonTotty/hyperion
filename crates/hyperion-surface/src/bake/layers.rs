//! What this plan adds to R05's patch bake (plan R10, Provides' `TerrainLayers`): the class
//! palette and weights, the survey mask, the patch's bound and σ, the horizon map's slot and R11's
//! layers.

use std::collections::BTreeMap;

use hyperion_base::units::Metres;

use crate::material::{ClassWeights, GroundMaterial, MAX_CLASSES, WEIGHT_TOTAL};
use crate::scatter::ExtraLayer;

/// A vertex's survey mask where its coarse cell is surveyed: a byte's whole, 255.
pub const SURVEYED: u8 = u8::MAX;

/// A vertex's survey mask where its coarse cell is not surveyed: a survey's margin, or a cell the
/// view does not hold (Design note 12).
pub const UNSURVEYED: u8 = 0;

/// A patch's horizon map: its slot in [`TerrainLayers`], empty until R10.T8.a bakes it (Design
/// note 10), when it holds each texel's horizon elevation in eight azimuths, linear signed 16-bit
/// two to a word, near (65 × 65) and far (9 × 9).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct HorizonMap {
    near: Vec<u32>,
    far: Vec<u32>,
}

impl HorizonMap {
    /// The near field's words, texel by texel, row by row: none until R10.T8.a.
    #[must_use]
    pub fn near_words(&self) -> &[u32] {
        &self.near
    }

    /// The far field's words: none until R10.T8.a.
    #[must_use]
    pub fn far_words(&self) -> &[u32] {
        &self.far
    }

    /// Whether the map holds nothing: always, until R10.T8.a.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.near.is_empty() && self.far.is_empty()
    }
}

/// What this plan adds to R05's [`PatchBake`](crate::patch::PatchBake) of a patch (Provides): the
/// patch's class palette and each vertex's weights over it, its survey mask, its bound and σ, its
/// horizon map and R11's layers. Each per-vertex array is 65 × 65, row by row (y outer, x inner),
/// as R05's heights are.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainLayers {
    pub(super) palette: [Option<GroundMaterial>; MAX_CLASSES],
    pub(super) weights: Vec<[u8; MAX_CLASSES]>,
    pub(super) survey_mask: Vec<u8>,
    pub(super) horizon: HorizonMap,
    pub(super) bound: Metres,
    pub(super) sigma: Metres,
    pub(super) complete: bool,
    pub(super) extra: Vec<ExtraLayer>,
}

impl TerrainLayers {
    /// The patch's materials, slot by slot: the union of its vertices' class weights' materials
    /// (R10.T2's [`ClassWeights`]), in rising class code and then entry, `None` in the slots past
    /// them. Where the union holds more than [`MAX_CLASSES`], the eight of largest summed weight
    /// over the patch, ties to the lower class code and entry. All `None` where the weights are not
    /// baked ([`WeightsLayer::Skipped`](super::WeightsLayer::Skipped)).
    #[must_use]
    pub const fn palette(&self) -> [Option<GroundMaterial>; MAX_CLASSES] {
        self.palette
    }

    /// Each vertex's weights over [`palette`](Self::palette), slot by slot: its
    /// [`ClassWeights`]' bytes, summing to [`WEIGHT_TOTAL`]. A vertex with a material the palette
    /// does not hold (a patch whose union passes eight) has the bytes of those it holds made to sum
    /// to it again, by the largest remainder, or, holding none of them, all of it on the patch's
    /// material of largest summed weight. Only a vertex with no weights (one the view cannot answer,
    /// outside the survey) has none, all zero. Empty where the weights are not baked.
    #[must_use]
    pub fn weights(&self) -> &[[u8; MAX_CLASSES]] {
        &self.weights
    }

    /// Each vertex's survey mask: [`SURVEYED`] where the field's level cell holding it
    /// ([`PatchKey::containing`](crate::cube::PatchKey::containing)) is surveyed, [`UNSURVEYED`]
    /// where it is a survey's margin or not held (Design note 12). The terrain's shader discards
    /// fragments whose interpolated mask is under a half.
    #[must_use]
    pub fn survey_mask(&self) -> &[u8] {
        &self.survey_mask
    }

    /// The patch's horizon map: empty until R10.T8.a.
    #[must_use]
    pub const fn horizon(&self) -> &HorizonMap {
        &self.horizon
    }

    /// The patch's stated bound at its level, metres: R09's
    /// [`level_bound_m`](crate::synth::level_bound_m), hard in its structural and channel terms and
    /// a 1 − 10⁻⁹ per-point quantile in its craters' (Design note 5).
    #[must_use]
    pub const fn bound(&self) -> Metres {
        self.bound
    }

    /// The patch's σ at its level, metres: the largest of R09's
    /// [`omitted_rms`](crate::synth::Synthesiser::omitted_rms) at its level over the coarse cells
    /// of its surveyed vertices, what the level leaves out there, one standard deviation (Design
    /// note 5's candidate selection by min(bound, 4σ)); 0 where no vertex is surveyed.
    #[must_use]
    pub const fn sigma(&self) -> Metres {
        self.sigma
    }

    /// Whether the view answered every query of the bake. It is false where a vertex outside the
    /// survey took the reference surface, for want of a cell its reads reach (Design note 12):
    /// such a patch can bake otherwise once a block brings that cell, though its survey mask stays
    /// as it was, so a client rebakes it on any new block, and every other patch only where a
    /// block changes its mask (Design note 16).
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    /// R11's layers, each [`BakeHook`](super::BakeHook)'s in turn: none until R11 lands.
    #[must_use]
    pub fn extra(&self) -> &[ExtraLayer] {
        &self.extra
    }
}

/// The patch's palette and each vertex's bytes over it, from each vertex's class weights, `None`
/// at a vertex that has none ([`TerrainLayers::palette`] and [`TerrainLayers::weights`]).
#[must_use]
pub(super) fn gather(
    vertices: &[Option<ClassWeights>],
) -> (
    [Option<GroundMaterial>; MAX_CLASSES],
    Vec<[u8; MAX_CLASSES]>,
) {
    let mut totals: BTreeMap<GroundMaterial, u64> = BTreeMap::new();
    for weights in vertices.iter().flatten() {
        for (material, byte) in weights.iter() {
            *totals.entry(material).or_insert(0) += u64::from(byte);
        }
    }
    let mut by_weight: Vec<(GroundMaterial, u64)> = totals.into_iter().collect();
    by_weight.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let heaviest = by_weight.first().map(|&(m, _)| m);
    let mut kept: Vec<GroundMaterial> = by_weight
        .into_iter()
        .take(MAX_CLASSES)
        .map(|(m, _)| m)
        .collect();
    kept.sort_unstable();
    let mut palette = [None; MAX_CLASSES];
    for (slot, &material) in palette.iter_mut().zip(&kept) {
        *slot = Some(material);
    }
    let heaviest = heaviest.and_then(|m| kept.binary_search(&m).ok());
    let bytes = vertices
        .iter()
        .map(|weights| {
            weights.map_or([0; MAX_CLASSES], |w| {
                over_palette(
                    &w,
                    &kept,
                    heaviest.expect("a vertex's weights are in the totals"),
                )
            })
        })
        .collect();
    (palette, bytes)
}

/// `weights`' bytes over the palette `kept`, slot by slot, summing to [`WEIGHT_TOTAL`] (see
/// [`TerrainLayers::weights`]): where none of its materials is kept, all of it on the slot
/// `heaviest`, the patch's material of largest summed weight.
#[must_use]
fn over_palette(
    weights: &ClassWeights,
    kept: &[GroundMaterial],
    heaviest: usize,
) -> [u8; MAX_CLASSES] {
    let mut bytes = [0_u32; MAX_CLASSES];
    for (material, byte) in weights.iter() {
        if let Ok(slot) = kept.binary_search(&material) {
            bytes[slot] = u32::from(byte);
        }
    }
    let held: u32 = bytes.iter().sum();
    let total = u32::from(WEIGHT_TOTAL);
    if held == 0 {
        bytes[heaviest] = total;
    } else if held != total {
        // A material outside the palette: the rest made to sum to the whole again, each slot's
        // share of it floored, and the bytes left given by the largest remainder, ties to the
        // lower slot, in integers alone.
        let mut remainders = [0_u32; MAX_CLASSES];
        let mut used = 0;
        for (byte, remainder) in bytes.iter_mut().zip(&mut remainders) {
            let scaled = *byte * total;
            *byte = scaled / held;
            *remainder = scaled % held;
            used += *byte;
        }
        let mut order: Vec<usize> = (0..MAX_CLASSES).collect();
        order.sort_by(|&a, &b| remainders[b].cmp(&remainders[a]).then(a.cmp(&b)));
        for &slot in order
            .iter()
            .take(usize::try_from(total - used).expect("a byte count fits"))
        {
            bytes[slot] += 1;
        }
    }
    bytes.map(|b| u8::try_from(b).expect("a slot's weight is at most a byte's whole"))
}
