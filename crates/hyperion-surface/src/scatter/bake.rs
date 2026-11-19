//! The rocks a finest patch carries in its bake (plan R11, T4.a): R10's [`BakeHook`] for the
//! authoritative rocks.
//!
//! # The patch's rocks
//!
//! A finest-level patch's rocks are those whose axis it holds: every rock drawn in a cell of an
//! octave's level under the patch (every octave's level is finer than the finest patch's on every
//! body of a few kilometres or more), or, where an octave's cells are larger, those of the one cell
//! above it whose axis the patch holds. So each rock belongs to one finest patch, and is drawn once
//! wherever the finest level is drawn. They come octave by octave, each octave's cells in key order
//! and each cell's rocks in draw order, which is [`rocks_near`](super::rocks_near)'s order: the
//! patch's list is `rocks_near` about the patch with the rocks whose axis lies elsewhere taken
//! out, rock for rock and bit for bit.
//!
//! # The ground from the patch's own heights
//!
//! A rock's axis stands on the finest level's collision surface, the mesh of the finest level's
//! vertex heights ([`finest_surface_height`](crate::patch::collision::finest_surface_height)).
//! Under a finest patch those vertices are the patch's own, which its bake has just made in `f64`
//! ([`BakedHeights::own_sample`]), [`Synthesiser::height_at`]'s at each vertex bit for bit, so the
//! hook reads them there and synthesises only a vertex the patch lacks (one on another face, where
//! a rock's axis lies on the cube's edge) or one that took the reference surface, which then
//! refuses as the synthesis would. That is the bench's lever on the rocks' cost (R11.T3.d's
//! record): the ground's queries were most of it.
//!
//! # Where the view lacks a cell
//!
//! A cell whose site or a rock's ground reads a cell the view does not hold has unknown rocks,
//! never none: the cell's rocks are left out of the list and the hook answers that its layer is
//! not complete, so that the bake is not [complete](crate::bake::TerrainLayers::is_complete) and a
//! later block bakes it again (R10's rebake rule). On surveyed ground the view holds every cell
//! the rocks read (its margin), so this happens only about the survey's edge.

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::ExtraLayer;
use super::law::RockAbundance;
use super::place::{ROCK_OCTAVE_COUNT, rock_octave_levels, rocks_of, site_abundance, site_cell};
use super::shape::Rock;
use crate::bake::{BakeHook, BakedHeights};
use crate::cube::{PATCH_QUADS, PatchKey};
use crate::field::FieldView;
use crate::geometry::finest_level;
use crate::height::HeightSample;
use crate::patch::HeightSource;
use crate::spheroid::Spheroid;
use crate::synth::{QueryHeightError, SynthCache, Synthesiser};

/// R10's [`BakeHook`] for the authoritative rocks (see the [module documentation](self)): a finest
/// patch's bake gains [`ExtraLayer::Rocks`], and a bake at any other level nothing.
///
/// It borrows the worker's [`Synthesiser`] and owns a [`SynthCache`] of its own, since the bake
/// holds the body's while the hooks run; the worker keeps it between bakes
/// ([`into_cache`](Self::into_cache)). No rock depends on what it holds.
#[derive(Debug)]
pub struct RockBakeHook<'s, 'a, F> {
    synth: &'s Synthesiser<'a, F>,
    cache: RefCell<SynthCache>,
}

impl<'s, 'a, F: FieldView> RockBakeHook<'s, 'a, F> {
    /// The hook over `synth`'s field and detail seed, with `cache` its own.
    #[must_use]
    pub const fn new(synth: &'s Synthesiser<'a, F>, cache: SynthCache) -> Self {
        Self {
            synth,
            cache: RefCell::new(cache),
        }
    }

    /// Its cache, to keep for the next bake.
    #[must_use]
    pub fn into_cache(self) -> SynthCache {
        self.cache.into_inner()
    }
}

impl<F: FieldView> BakeHook for RockBakeHook<'_, '_, F> {
    /// Appends a finest patch's rocks ([`ExtraLayer::Rocks`]), every one whose axis it holds that
    /// the view can make, and answers whether the view could make them all; at another level,
    /// nothing, and `true`.
    fn bake(&self, patch: &BakedHeights<'_>, out: &mut Vec<ExtraLayer>) -> bool {
        let figure = self.synth.field().header().figure();
        if patch.key().level() != finest_level(figure.equatorial_radius_m) {
            return true;
        }
        let rocks = patch_rocks(
            self.synth,
            &mut self.cache.borrow_mut(),
            patch,
            CellAbundance::Site,
        );
        let complete = rocks.unknown.is_none();
        out.push(ExtraLayer::Rocks(rocks.rocks));
        complete
    }
}

/// The words a rock is handed to the client in ([`rock_words`]): 20 `f64`.
pub const ROCK_WORDS: usize = 20;

/// `rock` as the client's rock layer holds it, [`ROCK_WORDS`] `f64` (`src/wasm.rs`'s
/// `BakedTerrain.rocks()`): its base centre, body-fixed metres (words 0–2); its frame's a, b and c
/// axes, body-fixed unit vectors ([`Rock::frame`], words 3–5, 6–8 and 9–11); its semi-axes a, b
/// and c, metres (12–14); its superquadric exponent (15); the ground's height at its axis, metres
/// above the datum (16); and its axis's unit direction, body-fixed ([`Rock::dir`], 17–19), which
/// names the cell, and so the patch, it lies in.
#[must_use]
pub fn rock_words(rock: &Rock) -> [f64; ROCK_WORDS] {
    let mut words = [0.0; ROCK_WORDS];
    words[..3].copy_from_slice(&rock.base_centre());
    for (k, axis) in rock.frame().iter().enumerate() {
        words[3 + 3 * k..6 + 3 * k].copy_from_slice(axis);
    }
    for (word, axis) in words[12..15].iter_mut().zip(rock.semi_axes()) {
        *word = axis.value();
    }
    words[15] = rock.shape().exponent();
    words[16] = rock.ground_height().value();
    words[17..].copy_from_slice(&rock.dir());
    words
}

/// The rocks whose axis the finest patch `patch` holds, each cell's at the abundance `k` whatever
/// its site's, their ground from the patch's own vertex heights: the hook's draw at a stated k,
/// for the benches (feature `testing`), never the generator's, as
/// [`rocks_in_cell_at`](super::rocks_in_cell_at) is the cell's.
///
/// # Errors
///
/// [`FindRocksError::Query`](super::FindRocksError::Query) with the first query the view could not
/// answer, in the cells' order: a cell a rock's ground reads that the view does not hold.
///
/// # Panics
///
/// As [`rock_octave_levels`]; or if a height is not finite (a bug).
#[cfg(any(test, feature = "testing"))]
pub fn patch_rocks_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    patch: &BakedHeights<'_>,
    k: RockAbundance,
) -> Result<Vec<Rock>, super::FindRocksError> {
    let rocks = patch_rocks(synth, cache, patch, CellAbundance::Stated(k));
    match rocks.unknown {
        None => Ok(rocks.rocks),
        Some(error) => Err(super::FindRocksError::Query(error)),
    }
}

/// The abundance each cell's rocks are drawn at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CellAbundance {
    /// Its site's, the generator's.
    Site,
    /// The one stated, whatever the site's: the benches' (feature `testing`).
    #[cfg(any(test, feature = "testing"))]
    Stated(RockAbundance),
}

/// A patch's rocks, and the first query the view could not answer, where it could not make every
/// cell's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PatchRocks {
    pub(crate) rocks: Vec<Rock>,
    pub(crate) unknown: Option<QueryHeightError>,
}

/// The rocks whose axis `patch` holds, their ground from its own vertex heights (see the [module
/// documentation](self)), each cell's at `abundance`.
///
/// # Panics
///
/// As [`rock_octave_levels`]; or if a height is not finite (a bug).
pub(crate) fn patch_rocks<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    patch: &BakedHeights<'_>,
    abundance: CellAbundance,
) -> PatchRocks {
    let figure = synth.field().header().figure();
    let levels = rock_octave_levels(&figure);
    let key = patch.key();
    let ground = PatchGround { synth, patch };
    // Each site's k once in the bake: a patch's cells share a few dozen sites.
    let mut sites: BTreeMap<PatchKey, Result<RockAbundance, QueryHeightError>> = BTreeMap::new();
    let mut rocks = Vec::new();
    let mut unknown = None;
    for (octave, &level) in levels.iter().enumerate() {
        for cell in cells_over(key, level) {
            let site = site_cell(cell, levels[ROCK_OCTAVE_COUNT - 1]);
            let k = *sites.entry(site).or_insert_with(|| match abundance {
                #[cfg(any(test, feature = "testing"))]
                CellAbundance::Stated(k) => Ok(k),
                CellAbundance::Site => site_abundance(synth, cache, site),
            });
            let k = match k {
                Ok(k) => k,
                Err(error) => {
                    unknown.get_or_insert(error);
                    continue;
                }
            };
            let start = rocks.len();
            if let Err(error) = rocks_of(synth, &ground, cache, cell, octave, k, &mut rocks) {
                // The cell's rocks are not known here: none of them, rather than some.
                rocks.truncate(start);
                unknown.get_or_insert(error);
                continue;
            }
            if level < key.level() {
                let above = rocks.split_off(start);
                rocks.extend(above.into_iter().filter(|rock| holds(key, rock)));
            }
        }
    }
    PatchRocks { rocks, unknown }
}

/// Whether `key` holds `rock`'s axis.
#[must_use]
fn holds(key: PatchKey, rock: &Rock) -> bool {
    PatchKey::containing(key.level(), rock.dir()) == Ok(key)
}

/// The cells of `level` over `key`: its descendants there in key order, or, where `level` is
/// shallower than the patch's, the one cell above it.
#[must_use]
fn cells_over(key: PatchKey, level: u8) -> Vec<PatchKey> {
    if level < key.level() {
        let up = key.level() - level;
        return vec![
            PatchKey::new(key.face(), level, key.i() >> up, key.j() >> up)
                .expect("an ancestor of a patch key is a patch key"),
        ];
    }
    let down = level - key.level();
    let span = 1_u32 << down;
    let mut cells = Vec::with_capacity(usize::try_from(span * span).unwrap_or(0));
    for di in 0..span {
        for dj in 0..span {
            cells.push(
                PatchKey::new(
                    key.face(),
                    level,
                    (key.i() << down) + di,
                    (key.j() << down) + dj,
                )
                .expect("a descendant of a patch key is a patch key"),
            );
        }
    }
    cells.sort_unstable();
    cells
}

/// The synthesis with a patch's own vertex heights in place of its queries at those vertices (see
/// the [module documentation](self)): every answer [`Synthesiser`]'s own, bit for bit.
struct PatchGround<'p, 's, 'a, F> {
    synth: &'s Synthesiser<'a, F>,
    patch: &'p BakedHeights<'p>,
}

impl<F: FieldView> PatchGround<'_, '_, '_, F> {
    /// Vertex (`x`, `y`) of `key` as the patch made it, where it is one of the patch's own vertices
    /// on its face and level and the view made it.
    #[must_use]
    fn own(&self, key: PatchKey, x: u8, y: u8) -> Option<HeightSample> {
        let patch = self.patch.key();
        if key.face() != patch.face() || key.level() != patch.level() {
            return None;
        }
        let quads = u64::from(PATCH_QUADS);
        let along = |cell: u32, at: u8, own: u32| {
            let lattice = u64::from(cell) * quads + u64::from(at);
            let first = u64::from(own) * quads;
            (first..=first + quads)
                .contains(&lattice)
                .then(|| u8::try_from(lattice - first).expect("a vertex index is at most 64"))
        };
        let (px, py) = (along(key.i(), x, patch.i())?, along(key.j(), y, patch.j())?);
        self.patch.own_sample(px, py)
    }
}

impl<F: FieldView> HeightSource for PatchGround<'_, '_, '_, F> {
    type Cache = SynthCache;
    type Error = QueryHeightError;

    fn figure(&self) -> Spheroid {
        HeightSource::figure(self.synth)
    }

    fn height(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: u8,
    ) -> Result<HeightSample, QueryHeightError> {
        HeightSource::height(self.synth, cache, dir, level)
    }

    /// The patch's own height at the vertex where it holds it, else the synthesis's.
    fn vertex_height(
        &self,
        cache: &mut SynthCache,
        key: PatchKey,
        x: u8,
        y: u8,
        dir: [f64; 3],
    ) -> Result<HeightSample, QueryHeightError> {
        match self.own(key, x, y) {
            Some(sample) => Ok(sample),
            None => HeightSource::vertex_height(self.synth, cache, key, x, y, dir),
        }
    }

    fn level_bound_m(&self, level: u8) -> f64 {
        HeightSource::level_bound_m(self.synth, level)
    }

    fn height_range_m(&self, key: PatchKey) -> (f64, f64) {
        HeightSource::height_range_m(self.synth, key)
    }
}

#[cfg(test)]
mod tests;
