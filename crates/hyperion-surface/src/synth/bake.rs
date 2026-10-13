//! A patch's heights from the synthesis, in R05's bake layout, and the range they can reach (plan
//! R09, T8).
//!
//! [`PatchHeights`] holds what R05's bake reads of a height source for one patch, made by
//! [`Synthesiser::bake_patch`]: each of the 65 × 65 vertices' height and gradient at the patch's
//! level, and the parent level's height at each even vertex, which is where R05's morph targets
//! take the parent's own heights (its other vertices take the parent mesh's chords between them).
//! Every value is the point query's, [`Synthesiser::height_at`] at the vertex's direction
//! ([`PatchKey::vertex_dir`]), bit for bit: the bake only orders the queries and fills the caller's
//! cache for them, first covering the structural octaves' lattice about the patch once.
//!
//! [`Synthesiser::height_range_m`] bounds a patch's heights at its level and its parent's for
//! culling: the base elevation never leaves the range of the cells its points read, and each
//! level's contributions stay within its [`LevelBounds::contribution_range_m`].
//!
//! [`LevelBounds::contribution_range_m`]: super::LevelBounds::contribution_range_m

use std::collections::BTreeSet;

use hyperion_base::units::Metres;

use super::{BandLevel, QueryHeightError, SynthCache, Synthesiser};
use crate::cube::{Edge, PATCH_QUADS, PatchKey};
use crate::field::{FieldView, SYNTHESIS_MARGIN_CELLS};
use crate::height::HeightSample;
use crate::num;
use crate::patch::PATCH_VERTICES;

/// The parent level's vertices along a patch's side that a patch carries: its even vertices, 33.
pub const PARENT_VERTICES: usize = PATCH_VERTICES.div_ceil(2);

/// A patch's heights, parent-band heights and gradients from the synthesis, in R05's bake layout:
/// what R05's bake reads of its height source for the patch (the
/// [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub struct PatchHeights {
    key: PatchKey,
    samples: Vec<HeightSample>,
    parent_heights_m: Vec<f64>,
}

impl PatchHeights {
    /// The patch.
    #[must_use]
    pub const fn key(&self) -> PatchKey {
        self.key
    }

    /// Each vertex's height and gradient at the patch's level, row by row (y outer, x inner),
    /// 65 × 65: [`Synthesiser::height_at`] at [`PatchKey::vertex_dir`].
    #[must_use]
    pub fn samples(&self) -> &[HeightSample] {
        &self.samples
    }

    /// Vertex (`x`, `y`)'s height and gradient at the patch's level.
    ///
    /// # Panics
    ///
    /// If `x` or `y` is above 64.
    #[must_use]
    pub fn sample(&self, x: u8, y: u8) -> HeightSample {
        self.samples[vertex_index(x, y, PATCH_VERTICES)]
    }

    /// The parent level's height at each even vertex (2a, 2b), row by row (b outer, a inner),
    /// 33 × 33, metres; empty at level 0, which has no parent.
    #[must_use]
    pub fn parent_heights_m(&self) -> &[f64] {
        &self.parent_heights_m
    }

    /// The parent level's height at vertex (`x`, `y`), metres, both even; `None` at level 0.
    ///
    /// # Panics
    ///
    /// If `x` or `y` is odd or above 64.
    #[must_use]
    pub fn parent_height_m(&self, x: u8, y: u8) -> Option<f64> {
        assert!(
            x.is_multiple_of(2) && y.is_multiple_of(2),
            "the parent level's heights are at even vertices, not ({x}, {y})"
        );
        let at = vertex_index(x / 2, y / 2, PARENT_VERTICES);
        self.parent_heights_m.get(at).copied()
    }
}

/// The index of vertex (`x`, `y`) of a grid `side` vertices a side, row by row.
///
/// # Panics
///
/// If `x` or `y` is outside the grid.
#[must_use]
fn vertex_index(x: u8, y: u8, side: usize) -> usize {
    let (x, y) = (usize::from(x), usize::from(y));
    assert!(
        x < side && y < side,
        "vertex ({x}, {y}) is outside a grid of {side} a side"
    );
    y * side + x
}

/// [`Synthesiser::bake_patch`].
pub(super) fn bake<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
) -> Result<PatchHeights, QueryHeightError> {
    let level = BandLevel::new(key.level()).expect("a patch's level is a band level");
    cover(synth, cache, key, level);
    let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
    let mut samples = Vec::with_capacity(PATCH_VERTICES * PATCH_VERTICES);
    for y in 0..=quads {
        for x in 0..=quads {
            samples.push(synth.height_at(cache, key.vertex_dir(x, y), level)?);
        }
    }
    let mut parent_heights_m = Vec::new();
    if let Some(parent) = key.level().checked_sub(1) {
        let parent = BandLevel::new(parent).expect("a level below a band level is one");
        parent_heights_m.reserve(PARENT_VERTICES * PARENT_VERTICES);
        for y in (0..=quads).step_by(2) {
            for x in (0..=quads).step_by(2) {
                let sample = synth.height_at(cache, key.vertex_dir(x, y), parent)?;
                parent_heights_m.push(sample.height_m);
            }
        }
    }
    Ok(PatchHeights {
        key,
        samples,
        parent_heights_m,
    })
}

/// Covers the structural octaves' lattice in `cache` for every query of a bake of `key` at
/// `level`, whose set is the parent's and more: each octave's corners within a ball about the
/// patch's centre out to its farthest corner (the patch's edges being great circles of the cube,
/// its corners bound it), with a tenth's margin for the spheroid. No value depends on it.
fn cover<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    level: BandLevel,
) {
    let Some(through) = synth.level_set(level).relief_through() else {
        return;
    };
    let figure = synth.field().header().figure();
    let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
    let centre = key.vertex_dir(quads / 2, quads / 2);
    let c = figure.point(centre);
    let mut reach = 0.0_f64;
    for (x, y) in [(0, 0), (quads, 0), (0, quads), (quads, quads)] {
        let p = figure.point(key.vertex_dir(x, y));
        let d = [0, 1, 2].map(|a| p[a] - c[a]);
        reach = num::max(reach, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
    }
    synth.relief().cover(
        synth.field(),
        centre,
        Metres::new(reach * 1.1),
        through,
        cache,
    );
}

/// The millimetres a metre of the cells' elevations.
const MM_PER_M: f64 = 1_000.0;

/// [`Synthesiser::height_range_m`].
#[must_use]
pub(super) fn height_range<F: FieldView>(synth: &Synthesiser<'_, F>, key: PatchKey) -> (f64, f64) {
    let (base_low, base_high) = base_range(synth.field(), key);
    let level = BandLevel::new(key.level()).expect("a patch's level is a band level");
    let (mut low, mut high) = synth.bounds().contribution_range_m(level);
    if let Some(parent) = key.level().checked_sub(1) {
        let parent = BandLevel::new(parent).expect("a level below a band level is one");
        let (l, h) = synth.bounds().contribution_range_m(parent);
        low = num::min(low, l);
        high = num::max(high, h);
    }
    (base_low + low, base_high + high)
}

/// The range of the base elevation over `key`, metres: the lowest and highest elevation of the
/// held cells within [`SYNTHESIS_MARGIN_CELLS`] king moves of the coarse cells it covers, which
/// hold every cell its points' interpolant reads, widened by a micrometre and a part in 10¹² for
/// rounding; or any elevation a cell can encode where the view holds none of them.
///
/// The margin's rings grow from the covered block's border alone, since an inner cell's
/// neighbours are the block's own.
#[must_use]
fn base_range(field: &impl FieldView, key: PatchKey) -> (f64, f64) {
    let level = field.header().level().get();
    let start: Vec<PatchKey> = if key.level() >= level {
        let up = key.level() - level;
        vec![
            PatchKey::new(key.face(), level, key.i() >> up, key.j() >> up)
                .expect("a patch's ancestor is a patch"),
        ]
    } else {
        let down = level - key.level();
        let side = 1_u32 << down;
        (0..side)
            .flat_map(|j| (0..side).map(move |i| (i, j)))
            .map(|(i, j)| {
                PatchKey::new(
                    key.face(),
                    level,
                    (key.i() << down) + i,
                    (key.j() << down) + j,
                )
                .expect("a patch's descendant is a patch")
            })
            .collect()
    };
    let last = (1_u32 << (level - key.level().min(level))) - 1;
    let (i0, j0) = (start[0].i(), start[0].j());
    let mut ring: Vec<PatchKey> = start
        .iter()
        .copied()
        .filter(|c| {
            let (di, dj) = (c.i() - i0, c.j() - j0);
            di == 0 || dj == 0 || di == last || dj == last
        })
        .collect();
    let mut seen: BTreeSet<PatchKey> = start.into_iter().collect();
    let mut next = Vec::new();
    for _ in 0..SYNTHESIS_MARGIN_CELLS {
        next.clear();
        for cell in &ring {
            let around = Edge::ALL
                .into_iter()
                .map(|edge| cell.edge_neighbour(edge))
                .chain(cell.corner_neighbours().into_iter().flatten());
            for neighbour in around {
                if seen.insert(neighbour) {
                    next.push(neighbour);
                }
            }
        }
        core::mem::swap(&mut ring, &mut next);
    }
    let mut extremes: Option<(i32, i32)> = None;
    for cell in &seen {
        if let Some(record) = field.cell(*cell) {
            let e = record.elevation_mm;
            extremes = Some(extremes.map_or((e, e), |(lo, hi)| (lo.min(e), hi.max(e))));
        }
    }
    let (lo, hi) = extremes.unwrap_or((i32::MIN, i32::MAX));
    let (lo, hi) = (f64::from(lo) / MM_PER_M, f64::from(hi) / MM_PER_M);
    let slack = |x: f64| 1e-6 + 1e-12 * x.abs();
    (lo - slack(lo), hi + slack(hi))
}
