//! The contact query: the rocks near a point, and the authoritative ground with its rocks there
//! (plan R11, T3.c; Design note 5).
//!
//! # The search
//!
//! A rock drawn in a cell of its octave's level reaches no point outside the cell's 3 × 3
//! neighbourhood ([`rock_octave_levels`]), so a point's own rocks are those of the 3 × 3 about its
//! cell at each octave's level, across face edges by the cube's rule, where a cube corner has
//! seven cells: a rock whose cell lies across a face edge, or at a cube corner, is found from the
//! neighbouring face. A wider search ([`rocks_near`] at a radius) walks outwards from the point's
//! cell through each cell's neighbours, keeping every cell whose spheroid points could lie within
//! the radius and the octave's reach of it, so no cell that could hold a rock reaching that far is
//! passed over. The rocks come octave by octave, each octave's cells in key order and each cell's
//! rocks in draw order.
//!
//! # The ground with its rocks
//!
//! [`surface_height_with_rocks`] is the larger of R10's [`ground_at`] and the top of every rock
//! the datum's normal through the point's foot meets ([`Rock::top_above`]), metres above the
//! datum: the ground's height, to the bit, where no rock stands, and never below it anywhere. A
//! rock's top is taken in its flat frame, within its [`Rock::contact_bound`] of the exact one.
//!
//! # The cache
//!
//! [`RockCache`] keeps each site cell's abundance and each cell's rocks, as R09's [`SynthCache`]
//! keeps its own: the caller's, bounded, and order-independent, since every value it holds is a
//! pure function of the body's field and detail seed, made only where the view holds what it
//! reads. Like a `SynthCache`, it serves views of one field whose held cells only grow.

use std::collections::{BTreeMap, BTreeSet};

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;

use super::hold::RockRules;
use super::law::RockAbundance;
use super::place::{
    REACH_MARGIN, ROCK_OCTAVE_COUNT, ROCK_OCTAVES, RockError, rock_octave_levels, rocks_of,
    site_abundance, site_cell,
};
use super::shape::{MAX_REACH_SHARE, Rock};
use crate::cube::PatchKey;
use crate::field::{BodyRef, FieldView};
use crate::ground::{datum_foot, ground_at};
use crate::num;
use crate::spheroid::Spheroid;
use crate::synth::craters::neighbourhood;
use crate::synth::{QueryHeightError, SynthCache, Synthesiser};

/// The most cells a [`RockCache`] keeps of each kind, sites and rocks, before it starts afresh:
/// 16,384, about the cells of a few dozen 32 m patches' finest octave.
const CACHE_CELLS: usize = 16_384;

/// The factor on a cell's corner distance that bounds the distance from its centre to any of its
/// points: 1.1, the slack for a spheroid's flattening of up to a tenth, which bends the cube's
/// great-circle edges' images.
const CELL_SPAN_MARGIN: f64 = 1.1;

/// The caller's cache of site cells' abundances and cells' rocks (see the [module
/// documentation](self)): empty or full, it changes no answer.
#[derive(Debug, Clone, Default)]
pub struct RockCache {
    owner: Option<(DetailSeed, BodyRef)>,
    sites: BTreeMap<u64, RockAbundance>,
    cells: BTreeMap<u64, Vec<Rock>>,
}

impl RockCache {
    /// An empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            owner: None,
            sites: BTreeMap::new(),
            cells: BTreeMap::new(),
        }
    }

    /// The number of cells whose rocks it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether it holds no cell's rocks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Takes the cache for the body of `seed` and `body`, emptying it if it held another's.
    fn own(&mut self, seed: DetailSeed, body: BodyRef) {
        if self.owner != Some((seed, body)) {
            self.sites.clear();
            self.cells.clear();
            self.owner = Some((seed, body));
        }
    }

    /// The rocks of `cell`, of octave `octave` on a body of rock octave levels `levels`, kept or
    /// made.
    fn rocks<F: FieldView>(
        &mut self,
        synth: &Synthesiser<'_, F>,
        synth_cache: &mut SynthCache,
        levels: [u8; ROCK_OCTAVE_COUNT],
        octave: usize,
        cell: PatchKey,
    ) -> Result<&[Rock], QueryHeightError> {
        let key = cell.to_u64();
        if !self.cells.contains_key(&key) {
            let site = site_cell(cell, levels[ROCK_OCTAVE_COUNT - 1]);
            let k = if let Some(&k) = self.sites.get(&site.to_u64()) {
                k
            } else {
                let k = site_abundance(synth, synth_cache, site)?;
                if self.sites.len() >= CACHE_CELLS {
                    self.sites.clear();
                }
                self.sites.insert(site.to_u64(), k);
                k
            };
            let mut rocks = Vec::new();
            rocks_of(synth, synth_cache, cell, octave, k, &mut rocks)?;
            if self.cells.len() >= CACHE_CELLS {
                self.cells.clear();
            }
            self.cells.insert(key, rocks);
        }
        Ok(&self.cells[&key])
    }
}

/// The rocks within `radius` of the body-fixed point `at`, appended to `out`: every rock whose
/// outline may come within `radius` of the point's foot on the datum, its reach taken
/// [`REACH_MARGIN`] wider, octave by octave (see the [module documentation](self)); none below
/// [`ROCKS_VERSION`](super::ROCKS_VERSION). Both caches are the caller's, and the rocks do not
/// depend on what they hold.
///
/// # Errors
///
/// [`RockError::Query`] where a cell searched, its site or a rock's ground reads a cell the view
/// does not hold: never "no rocks there". `out` may then hold some of the rocks.
///
/// # Panics
///
/// If `radius` is negative or not finite, or `at` is not finite or is the body's centre; or if a
/// height is not finite (a bug).
pub fn rocks_near<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    cache: &mut RockCache,
    at: &[f64; 3],
    radius: Metres,
    out: &mut Vec<Rock>,
) -> Result<(), RockError> {
    let radius_m = radius.value();
    assert!(
        radius_m.is_finite() && radius_m >= 0.0,
        "a search radius must be finite and non-negative, got {radius_m}"
    );
    let figure = synth.field().header().figure();
    let (foot, _) = datum_foot(&figure, at);
    match RockRules::in_force() {
        RockRules::Held => Ok(()),
        RockRules::Placed => Ok(each_near(
            synth,
            synth_cache,
            cache,
            foot,
            radius_m,
            |rock| out.push(rock.clone()),
        )?),
    }
}

/// The authoritative ground with its rocks at the body-fixed point `at`: the larger of R10's
/// [`ground_at`] there and every rock's top along the datum's normal through the point's foot,
/// metres above the datum (see the [module documentation](self)); the ground's height below
/// [`ROCKS_VERSION`](super::ROCKS_VERSION). Both caches are the caller's, and the height does not
/// depend on what they hold.
///
/// # Errors
///
/// [`RockError::Query`] where the ground, a cell searched, its site or a rock's ground reads a cell
/// the view does not hold: never a height without its rocks.
///
/// # Panics
///
/// As [`ground_at`]; or if a height is not finite (a bug).
pub fn surface_height_with_rocks<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    cache: &mut RockCache,
    at: &[f64; 3],
) -> Result<Metres, RockError> {
    let ground = ground_at(synth, synth_cache, at)?;
    match RockRules::in_force() {
        RockRules::Held => Ok(ground.height()),
        RockRules::Placed => {
            let figure = synth.field().header().figure();
            let foot = ground.dir();
            let mut height = ground.height().value();
            each_near(synth, synth_cache, cache, foot, 0.0, |rock| {
                if let Some(top) = rock.top_at(&figure, foot) {
                    height = num::max(height, top);
                }
            })?;
            Ok(Metres::new(num::assert_finite(height)))
        }
    }
}

/// Calls `found` with each rock that may come within `radius_m` metres of the datum point of the
/// unit direction `foot`, octave by octave (see the [module documentation](self)).
fn each_near<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    cache: &mut RockCache,
    foot: [f64; 3],
    radius_m: f64,
    mut found: impl FnMut(&Rock),
) -> Result<(), QueryHeightError> {
    let header = synth.field().header();
    cache.own(synth.seed(), header.body());
    let figure = header.figure();
    let levels = rock_octave_levels(&figure);
    let p = figure.point(foot);
    for (octave, &level) in levels.iter().enumerate() {
        let home = PatchKey::containing(level, foot).expect("a rock octave's level is a level");
        let cells = if radius_m > 0.0 {
            let reach = REACH_MARGIN * MAX_REACH_SHARE * ROCK_OCTAVES[octave].1.value();
            cells_within(home, p, radius_m + reach, &figure)
        } else {
            let (cells, count) = neighbourhood(home);
            cells[..count].to_vec()
        };
        for cell in cells {
            for rock in cache.rocks(synth, synth_cache, levels, octave, cell)? {
                let q = figure.point(rock.dir());
                let d = [0, 1, 2].map(|k| p[k] - q[k]);
                let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if distance <= radius_m + REACH_MARGIN * rock.reach().value() {
                    found(rock);
                }
            }
        }
    }
    Ok(())
}

/// The cells about `home`, of its level, whose spheroid points may lie within `reach_m` metres of
/// the spheroid point `p`, `home`'s own, in key order: a walk outwards through each kept cell's
/// 3 × 3 neighbourhood, a cell kept where `p` is within `reach_m` of its centre's point and its
/// span about it ([`cell_span_m`]).
#[must_use]
fn cells_within(home: PatchKey, p: [f64; 3], reach_m: f64, figure: &Spheroid) -> Vec<PatchKey> {
    let mut seen = BTreeSet::from([home]);
    let mut queue = vec![home];
    let mut kept = Vec::new();
    while let Some(cell) = queue.pop() {
        let centre = figure.point(cell.vertex_dir(32, 32));
        let d = [0, 1, 2].map(|k| p[k] - centre[k]);
        let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if cell != home && distance > reach_m + cell_span_m(cell, figure) {
            continue;
        }
        kept.push(cell);
        let (around, count) = neighbourhood(cell);
        for &next in &around[..count] {
            if seen.insert(next) {
                queue.push(next);
            }
        }
    }
    kept.sort_unstable();
    kept
}

/// A bound on the distance from `cell`'s centre's spheroid point to any of its points, metres: its
/// farthest corner's, [`CELL_SPAN_MARGIN`] wider.
#[must_use]
fn cell_span_m(cell: PatchKey, figure: &Spheroid) -> f64 {
    let centre = figure.point(cell.vertex_dir(32, 32));
    let mut farthest = 0.0;
    for (x, y) in [(0, 0), (64, 0), (0, 64), (64, 64)] {
        let corner = figure.point(cell.vertex_dir(x, y));
        let d = [0, 1, 2].map(|k| corner[k] - centre[k]);
        farthest = num::max(farthest, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
    }
    CELL_SPAN_MARGIN * farthest
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::super::golden::cells_under;
    use super::super::hold::with_rocks_placed;
    use super::super::place::{RockCellKey, rocks_in_cell};
    use crate::cube::{Face, unit_dir};
    use crate::field::PartialField;
    use crate::synth::test_support::{SEED, world};
    use crate::testing::SyntheticWorld;
    use hyperion_testkit::float::bits;
    use hyperion_testkit::order::assert_order_independent;

    /// The rocks of every octave under the site cell holding `dir` on `w`.
    fn rocks_about(w: SyntheticWorld, dir: [f64; 3]) -> Vec<Rock> {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let levels = rock_octave_levels(&field.header().figure());
        let site = PatchKey::containing(levels[3], unit_dir(dir)).unwrap();
        let mut rocks = Vec::new();
        for (_, cell) in cells_under(site, levels) {
            rocks_in_cell(&synth, &mut cache, RockCellKey::new(cell), &mut rocks).unwrap();
        }
        rocks
    }

    /// Points on a grid of `n` × `n` over a square of `side` metres about the rock's axis, in its
    /// frame's plane at its ground's height.
    fn grid_about(rock: &Rock, figure: &Spheroid, side: f64, n: u32) -> Vec<[f64; 3]> {
        let up = figure.normal(rock.dir());
        let reference = if up[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let east = unit_dir(cross(reference, up));
        let north = cross(up, east);
        let o = rock.base_centre();
        let mut out = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let x = side * (f64::from(i) / f64::from(n - 1) - 0.5);
                let y = side * (f64::from(j) / f64::from(n - 1) - 0.5);
                out.push([0, 1, 2].map(|k| o[k] + x * east[k] + y * north[k]));
            }
        }
        out
    }

    const DIRS: [[f64; 3]; 4] = [
        [1.0, 0.1, 0.2],
        [-0.3, 0.9, 0.1],
        [0.2, -0.4, 0.8],
        [0.5, 0.5, -0.7],
    ];

    /// A rock's exposed height above R10's `ground_at` at its axis is the drawn one, and its axis
    /// stands on that ground.
    #[test]
    fn the_exposed_height_above_the_ground_is_the_drawn_one() {
        with_rocks_placed(|| {
            let mut seen = 0;
            for w in [SyntheticWorld::MoonLike, SyntheticWorld::EarthLike] {
                let field = world(w);
                let figure = field.header().figure();
                let synth = Synthesiser::new(field, SEED);
                let mut cache = SynthCache::new();
                for dir in DIRS {
                    for rock in rocks_about(w, dir) {
                        seen += 1;
                        let ground = ground_at(&synth, &mut cache, &rock.base_centre()).unwrap();
                        let g = ground.height().value();
                        assert!((g - rock.ground_height().value()).abs() < 1e-8, "{g}");
                        let top = rock
                            .top_above(&figure, &rock.base_centre())
                            .unwrap()
                            .value();
                        let exposed = top - g;
                        assert!(
                            (exposed - rock.exposed_height().value()).abs() < 1e-8,
                            "{exposed} against {:?}",
                            rock.exposed_height()
                        );
                    }
                }
            }
            assert!(seen > 0);
        });
    }

    /// The height with rocks is the ground's, to the bit, where no rock's top is met, and never
    /// below the ground anywhere: over a grid about each rock, out past its outline.
    #[test]
    fn the_rocks_raise_the_ground_and_only_where_they_stand() {
        with_rocks_placed(|| {
            let w = SyntheticWorld::MoonLike;
            let field = world(w);
            let figure = field.header().figure();
            let synth = Synthesiser::new(field, SEED);
            let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
            let (mut raised, mut bare) = (0, 0);
            for dir in DIRS {
                for rock in rocks_about(w, dir) {
                    let side = 3.0 * rock.reach().value();
                    for p in grid_about(&rock, &figure, side, 13) {
                        let ground = ground_at(&synth, &mut synth_cache, &p).unwrap();
                        let with =
                            surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p)
                                .unwrap();
                        assert!(
                            with >= ground.height(),
                            "{with:?} below {:?}",
                            ground.height()
                        );
                        let mut near = Vec::new();
                        rocks_near(
                            &synth,
                            &mut synth_cache,
                            &mut cache,
                            &p,
                            Metres::ZERO,
                            &mut near,
                        )
                        .unwrap();
                        let met = near
                            .iter()
                            .filter_map(|r| r.top_above(&figure, &p))
                            .any(|top| top > ground.height());
                        if met {
                            raised += 1;
                            assert!(with > ground.height());
                        } else {
                            bare += 1;
                            assert_eq!(bits(with.value()), bits(ground.height().value()));
                        }
                    }
                }
            }
            assert!(raised > 0 && bare > 0, "{raised} raised, {bare} bare");
        });
    }

    /// A search at a radius finds every rock the cells within it hold that reaches it, the point's
    /// own rocks among them, and agrees with the 3 × 3 search at no radius.
    #[test]
    fn a_wide_search_finds_every_rock_within_its_radius() {
        with_rocks_placed(|| {
            let w = SyntheticWorld::MoonLike;
            let field = world(w);
            let figure = field.header().figure();
            let synth = Synthesiser::new(field, SEED);
            let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
            for dir in DIRS {
                let all = rocks_about(w, dir);
                let Some(first) = all.first() else { continue };
                let at = first.base_centre();
                let mut wide = Vec::new();
                rocks_near(
                    &synth,
                    &mut synth_cache,
                    &mut cache,
                    &at,
                    Metres::new(8.0),
                    &mut wide,
                )
                .unwrap();
                let p = figure.point(first.dir());
                for rock in &all {
                    let q = figure.point(rock.dir());
                    let d = (0..3)
                        .map(|k| (p[k] - q[k]) * (p[k] - q[k]))
                        .sum::<f64>()
                        .sqrt();
                    if d + rock.reach().value() <= 8.0 {
                        assert!(wide.contains(rock), "{rock:?} at {d} m is missed");
                    }
                }
                let mut own = Vec::new();
                rocks_near(
                    &synth,
                    &mut synth_cache,
                    &mut cache,
                    &at,
                    Metres::ZERO,
                    &mut own,
                )
                .unwrap();
                assert!(own.contains(first));
                for rock in &own {
                    assert!(wide.contains(rock));
                }
            }
        });
    }

    /// A rock whose cell lies by a face edge or at a cube corner, its outline crossing onto another
    /// face, is found from the point on that face, and raises the ground there.
    #[test]
    fn a_rock_across_a_face_edge_or_corner_is_found_from_the_other_face() {
        with_rocks_placed(|| {
            let w = SyntheticWorld::MoonLike;
            let field = world(w);
            let figure = field.header().figure();
            let synth = Synthesiser::new(field, SEED);
            let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
            let levels = rock_octave_levels(&figure);
            let site_level = levels[3];
            let side = 1_u32 << site_level;
            // The site cells along face 0's four edges near its middle, and those at its corners.
            let mut sites = Vec::new();
            for along in 0..48 {
                let m = side / 2 - 24 + along;
                for (i, j) in [(0, m), (side - 1, m), (m, 0), (m, side - 1)] {
                    sites.push(PatchKey::new(Face::PosX, site_level, i, j).unwrap());
                }
            }
            let mut at_corners = 0;
            for face in Face::ALL {
                for (i, j) in [(0, 0), (0, side - 1), (side - 1, 0), (side - 1, side - 1)] {
                    let corner = PatchKey::new(face, site_level, i, j).unwrap();
                    let (around, count) = neighbourhood(corner);
                    sites.extend_from_slice(&around[..count]);
                    at_corners += 1;
                }
            }
            assert_eq!(at_corners, 24);
            let mut crossings = 0;
            for site in sites {
                for (_, cell) in cells_under(site, levels) {
                    let mut rocks = Vec::new();
                    rocks_in_cell(&synth, &mut synth_cache, RockCellKey::new(cell), &mut rocks)
                        .unwrap();
                    for rock in rocks {
                        let own_face = cell.face();
                        for p in grid_about(&rock, &figure, 2.0 * rock.reach().value(), 9) {
                            let (foot, _) = datum_foot(&figure, &p);
                            let Some(top) = rock.top_at(&figure, foot) else {
                                continue;
                            };
                            if PatchKey::containing(cell.level(), foot).unwrap().face() == own_face
                            {
                                continue;
                            }
                            crossings += 1;
                            let mut near = Vec::new();
                            rocks_near(
                                &synth,
                                &mut synth_cache,
                                &mut cache,
                                &p,
                                Metres::ZERO,
                                &mut near,
                            )
                            .unwrap();
                            assert!(near.contains(&rock), "{rock:?} is missed from {p:?}");
                            let with =
                                surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p)
                                    .unwrap();
                            assert!(with.value() >= top);
                        }
                    }
                }
            }
            assert!(crossings > 0, "no rock's outline crosses a face edge");
        });
    }

    /// On a client's field that holds no cell, a query returns `NotSurveyed`, never a height or a
    /// search without its rocks.
    #[test]
    fn an_unsurveyed_query_is_not_surveyed() {
        let field = world(SyntheticWorld::MoonLike);
        let empty = PartialField::new(field.header().clone());
        let synth = Synthesiser::new(&empty, SEED);
        let figure = field.header().figure();
        let p = figure.surface_point(unit_dir([0.3, 0.2, 0.9]), 10.0);
        with_rocks_placed(|| {
            let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
            let height = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p);
            assert!(
                matches!(
                    height,
                    Err(RockError::Query(QueryHeightError::NotSurveyed(_)))
                ),
                "{height:?}"
            );
            let mut out = Vec::new();
            let near = rocks_near(
                &synth,
                &mut synth_cache,
                &mut cache,
                &p,
                Metres::ZERO,
                &mut out,
            );
            assert!(
                matches!(
                    near,
                    Err(RockError::Query(QueryHeightError::NotSurveyed(_)))
                ),
                "{near:?}"
            );
        });
    }

    /// Under the hold the height is the ground's to the bit and no rock is near.
    #[test]
    fn the_hold_leaves_the_ground_alone() {
        let w = SyntheticWorld::MoonLike;
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
        let placed = with_rocks_placed(|| rocks_about(w, DIRS[0]));
        for rock in &placed {
            let p = rock.base_centre();
            let ground = ground_at(&synth, &mut synth_cache, &p).unwrap();
            let with = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p).unwrap();
            assert_eq!(bits(with.value()), bits(ground.height().value()));
            let mut near = Vec::new();
            rocks_near(
                &synth,
                &mut synth_cache,
                &mut cache,
                &p,
                Metres::new(5.0),
                &mut near,
            )
            .unwrap();
            assert_eq!(near.len(), 0);
        }
        assert!(cache.is_empty());
    }

    /// The heights with rocks do not depend on what either cache holds or on the order they are
    /// asked in.
    #[test]
    fn the_height_with_rocks_is_order_and_cache_independent() {
        with_rocks_placed(|| {
            let w = SyntheticWorld::MoonLike;
            let field = world(w);
            let figure = field.header().figure();
            let synth = Synthesiser::new(field, SEED);
            let mut points = Vec::new();
            for dir in DIRS {
                for rock in rocks_about(w, dir).iter().take(3) {
                    points.extend(grid_about(rock, &figure, 2.0 * rock.reach().value(), 4));
                }
            }
            assert_ne!(points.len(), 0);
            let caches = RefCell::new((SynthCache::new(), RockCache::new()));
            let warm = |p: &[f64; 3]| {
                let (synth_cache, cache) = &mut *caches.borrow_mut();
                bits(
                    surface_height_with_rocks(&synth, synth_cache, cache, p)
                        .unwrap()
                        .value(),
                )
            };
            assert_order_independent(&points, warm);
            let cold = |p: &[f64; 3]| {
                let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
                bits(
                    surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, p)
                        .unwrap()
                        .value(),
                )
            };
            assert_order_independent(&points, cold);
            for p in &points {
                assert_eq!(warm(p), cold(p));
            }
        });
    }
}
