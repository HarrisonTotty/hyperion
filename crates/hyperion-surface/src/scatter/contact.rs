//! The contact query: the rocks near a point, and the authoritative ground with its rocks there
//! (plan R11, T3.c; Design note 5).
//!
//! # The search
//!
//! A rock drawn in a cell of its octave's level reaches no point outside the cell's 3 × 3
//! neighbourhood ([`rock_octave_levels`]), so a point's own rocks are those of the 3 × 3 about its
//! cell at each octave's level, across face edges by the cube's rule, where a cube corner has
//! seven neighbours: a rock whose cell lies across a face edge, or at a cube corner, is found from the
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
use hyperion_base::units::{Gigayears, Metres, Pascals};

use super::law::RockAbundance;
use super::place::{
    FindRocksError, REACH_MARGIN, ROCK_OCTAVE_COUNT, ROCK_OCTAVES, rock_octave_levels, rocks_of,
    site_abundance, site_cell,
};
use super::shape::{MAX_REACH_SHARE, Rock};
use crate::cube::PatchKey;
use crate::field::{BodyRef, CoarseLevel, FieldHeader, FieldView};
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
    owner: Option<Owner>,
    sites: BTreeMap<u64, RockAbundance>,
    cells: BTreeMap<u64, Vec<Rock>>,
}

/// What a [`RockCache`]'s values are made from besides the cells: the body's detail seed and the
/// header's parts the rocks read, so that a cache asked for another body's, or another field's, starts
/// afresh, as R09's crater cache does.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Owner {
    seed: DetailSeed,
    body: BodyRef,
    figure: Spheroid,
    level: CoarseLevel,
    surface_age: Gigayears,
    surface_pressure: Pascals,
}

impl Owner {
    /// The owner of the rocks of the body of `header` drawn from `seed`.
    #[must_use]
    fn of(seed: DetailSeed, header: &FieldHeader) -> Self {
        Self {
            seed,
            body: header.body(),
            figure: header.figure(),
            level: header.level(),
            surface_age: header.surface_age(),
            surface_pressure: header.surface_pressure(),
        }
    }
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

    /// Takes the cache for `owner`'s rocks, emptying it if it held another's.
    fn own(&mut self, owner: Owner) {
        if self.owner != Some(owner) {
            self.sites.clear();
            self.cells.clear();
            self.owner = Some(owner);
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
        if self.cells.len() >= CACHE_CELLS && !self.cells.contains_key(&key) {
            self.cells.clear();
        }
        match self.cells.entry(key) {
            std::collections::btree_map::Entry::Occupied(kept) => Ok(kept.into_mut()),
            std::collections::btree_map::Entry::Vacant(slot) => {
                let site = site_cell(cell, levels[ROCK_OCTAVE_COUNT - 1]).to_u64();
                if self.sites.len() >= CACHE_CELLS && !self.sites.contains_key(&site) {
                    self.sites.clear();
                }
                let k = match self.sites.entry(site) {
                    std::collections::btree_map::Entry::Occupied(kept) => *kept.get(),
                    std::collections::btree_map::Entry::Vacant(empty) => {
                        *empty.insert(site_abundance(
                            synth,
                            synth_cache,
                            PatchKey::from_u64(site).expect("a site cell's own key"),
                        )?)
                    }
                };
                let mut rocks = Vec::new();
                rocks_of(synth, synth, synth_cache, cell, octave, k, &mut rocks)?;
                Ok(slot.insert(rocks))
            }
        }
    }
}

/// The rocks within `radius` of the body-fixed point `at`, appended to `out`.
///
/// They are every rock whose outline may come within `radius` of the point's foot on the datum,
/// its reach taken [`REACH_MARGIN`] wider, octave by octave (see the [module
/// documentation](self)). Both caches are the caller's, and the rocks do not depend on what they
/// hold.
///
/// # Errors
///
/// [`FindRocksError::Query`] where a cell searched, its site or a rock's ground reads a cell the view
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
) -> Result<(), FindRocksError> {
    let radius_m = radius.value();
    assert!(
        radius_m.is_finite() && radius_m >= 0.0,
        "a search radius must be finite and non-negative, got {radius_m}"
    );
    let figure = synth.field().header().figure();
    let (foot, _) = datum_foot(&figure, at);
    Ok(each_near(
        synth,
        synth_cache,
        cache,
        foot,
        radius_m,
        |rock| out.push(rock.clone()),
    )?)
}

/// The authoritative ground with its rocks at the body-fixed point `at`, metres above the datum.
///
/// It is the larger of R10's [`ground_at`] there and every rock's top along the datum's normal
/// through the point's foot (see the [module documentation](self)). Both caches are the caller's,
/// and the height does not depend on what they hold.
///
/// # Errors
///
/// [`FindRocksError::Query`] where the ground, a cell searched, its site or a rock's ground reads a cell
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
) -> Result<Metres, FindRocksError> {
    let ground = ground_at(synth, synth_cache, at)?;
    let figure = synth.field().header().figure();
    let foot = ground.dir();
    let mut height = ground.height().value();
    each_near(synth, synth_cache, cache, foot, 0.0, |rock| {
        if let Some(top) = rock.top_at_m(&figure, foot) {
            height = num::max(height, top);
        }
    })?;
    Ok(Metres::new(num::assert_finite(height)))
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
    cache.own(Owner::of(synth.seed(), header));
    let figure = header.figure();
    let levels = rock_octave_levels(&figure);
    let p = figure.point(foot);
    for (octave, &level) in levels.iter().enumerate() {
        let home = PatchKey::containing(level, foot).expect("a rock octave's level is a level");
        let (around, count) = neighbourhood(home);
        let wide;
        let cells: &[PatchKey] = if radius_m > 0.0 {
            let reach = REACH_MARGIN * MAX_REACH_SHARE * ROCK_OCTAVES[octave].1.value();
            wide = cells_within(home, p, radius_m + reach, &figure);
            &wide
        } else {
            &around[..count]
        };
        for &cell in cells {
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
    use std::collections::BTreeSet;

    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::super::golden::cells_under;
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
    }

    /// The height with rocks is the ground's, to the bit, where no rock's top is met, and never
    /// below the ground anywhere: over a grid about each rock, out past its outline.
    #[test]
    fn the_rocks_raise_the_ground_and_only_where_they_stand() {
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
                    let with = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p)
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
    }

    /// A search at a radius finds every rock the cells within it hold that reaches it, the point's
    /// own rocks among them, and agrees with the 3 × 3 search at no radius.
    #[test]
    fn a_wide_search_finds_every_rock_within_its_radius() {
        let w = SyntheticWorld::MoonLike;
        let field = world(w);
        let figure = field.header().figure();
        let synth = Synthesiser::new(field, SEED);
        let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
        let mut searched = 0;
        for dir in DIRS {
            let all = rocks_about(w, dir);
            let Some(first) = all.first() else { continue };
            searched += 1;
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
        assert!(searched > 0, "no site held a rock to search about");
    }

    /// A rock whose cell lies by a face edge or at a cube corner, its outline crossing onto another
    /// face, is found from the point on that face, and raises the ground there.
    #[test]
    fn a_rock_across_a_face_edge_or_corner_is_found_from_the_other_face() {
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
        let mut corners = 0;
        for face in Face::ALL {
            for (i, j) in [(0, 0), (0, side - 1), (side - 1, 0), (side - 1, side - 1)] {
                let corner = PatchKey::new(face, site_level, i, j).unwrap();
                let (around, count) = neighbourhood(corner);
                assert_eq!(count, 8, "a cube corner's cell has seven neighbours");
                sites.extend_from_slice(&around[..count]);
                corners += 1;
            }
        }
        assert_eq!(corners, 24);
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
                        let Some(top) = rock.top_at_m(&figure, foot) else {
                            continue;
                        };
                        if PatchKey::containing(cell.level(), foot).unwrap().face() == own_face {
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
    }

    /// A rock in the cube corner's cell of one face, its outline over the corner, is found from
    /// points on the corner's two other faces, through the corner's seven neighbours: a rock
    /// planted in the cache, since the drawn ones seldom stand there.
    #[test]
    fn a_rock_at_a_cube_corner_is_found_from_the_other_faces() {
        use super::super::shape::RockShape;
        use crate::patch::collision::finest_surface_height;
        use hyperion_base::units::Radians;
        let field = world(SyntheticWorld::MoonLike);
        let header = field.header();
        let figure = header.figure();
        let synth = Synthesiser::new(field, SEED);
        let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
        let level = rock_octave_levels(&figure)[0];
        let dir = unit_dir([1.0, 1.0 - 2e-7, 1.0 - 2e-7]);
        let home = PatchKey::containing(level, dir).unwrap();
        assert_eq!(home.face(), Face::PosX);
        let ground = finest_surface_height(&synth, dir, &mut synth_cache).unwrap();
        let shape = RockShape::new(0.4, 0.9, Radians::ZERO, 2.0);
        let rock = Rock::new(&figure, dir, ground, Metres::new(0.8), shape);
        cache.own(Owner::of(SEED, header));
        cache.cells.insert(home.to_u64(), vec![rock.clone()]);
        for other in [[1.0 - 2e-7, 1.0, 1.0 - 2e-7], [1.0 - 2e-7, 1.0 - 2e-7, 1.0]] {
            let foot = unit_dir(other);
            let on = PatchKey::containing(level, foot).unwrap();
            assert_ne!(on.face(), home.face());
            let top = rock
                .top_at_m(&figure, foot)
                .expect("the outline covers the point");
            let p = figure.surface_point(foot, 1.0);
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
            assert!(
                near.contains(&rock),
                "the corner's rock is missed from {:?}",
                on.face()
            );
            let with = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p).unwrap();
            assert!(with.value() >= top);
        }
    }

    /// The list a search returns, contents and order, does not depend on what the caches hold or on
    /// the order the points are asked in.
    #[test]
    fn the_rocks_near_a_point_are_order_and_cache_independent() {
        let w = SyntheticWorld::MoonLike;
        let field = world(w);
        let figure = field.header().figure();
        let synth = Synthesiser::new(field, SEED);
        let mut points = Vec::new();
        for dir in DIRS {
            for rock in rocks_about(w, dir).iter().take(2) {
                points.extend(grid_about(rock, &figure, 3.0 * rock.reach().value(), 3));
            }
        }
        assert_ne!(points.len(), 0);
        let near = |p: &[f64; 3], radius: f64, caches: &mut (SynthCache, RockCache)| {
            let mut out = Vec::new();
            let (synth_cache, cache) = caches;
            rocks_near(&synth, synth_cache, cache, p, Metres::new(radius), &mut out).unwrap();
            format!("{out:?}")
        };
        for radius in [0.0, 6.0] {
            let shared = RefCell::new((SynthCache::new(), RockCache::new()));
            let warm = |p: &[f64; 3]| near(p, radius, &mut shared.borrow_mut());
            assert_order_independent(&points, warm);
            for p in &points {
                let cold = near(p, radius, &mut (SynthCache::new(), RockCache::new()));
                assert_eq!(cold, warm(p));
            }
        }
    }

    /// The payload's blocks of `cells` of `field`, at 50 m, with their margin, into a client's field.
    fn client_of(field: &crate::field::CoarseField, cells: &BTreeSet<PatchKey>) -> PartialField {
        use crate::field::{Cover, CoverRange, ResolutionCode, cell_index};
        use crate::wire::{decode_payload, encode_payload};
        let code = ResolutionCode::from_resolution(Metres::new(50.0)).unwrap();
        let mut indices: Vec<u32> = cells.iter().map(|&c| cell_index(c)).collect();
        indices.sort_unstable();
        let cover = Cover::from_ranges(
            indices
                .iter()
                .map(|&i| CoverRange::new(i, i + 1, code).unwrap()),
        )
        .unwrap();
        let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
        let mut client = PartialField::new(blocks[0].header().unwrap().clone());
        for block in &blocks {
            client.insert(block).unwrap();
        }
        client
    }

    /// On a client's field of one surveyed cell and its margin, along a line out of the survey,
    /// every query either answers with the rocks, never below the ground, or is `NotSurveyed`; and
    /// at the survey's edge, found by bisection, there are points whose ground the client knows but
    /// whose rocks' sites it does not, where the query is `NotSurveyed` too, never a height without
    /// the rocks.
    #[test]
    fn a_client_without_a_rocks_cells_is_not_surveyed_there() {
        let field = world(SyntheticWorld::MoonLike);
        let figure = field.header().figure();
        let level = field.header().level().get();
        let home = PatchKey::containing(level, unit_dir([0.3, 0.2, 0.9])).unwrap();
        let client = client_of(field, &BTreeSet::from([home]));
        let synth = Synthesiser::new(&client, SEED);
        let start = home.vertex_dir(32, 32);
        let out = unit_dir([start[1], -start[0], 0.0]);
        let at = |angle: f64| {
            let dir = unit_dir([0, 1, 2].map(|k| start[k] + angle * out[k]));
            figure.surface_point(dir, 0.0)
        };
        let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
        let mut query = |p: &[f64; 3]| {
            let ground = ground_at(&synth, &mut synth_cache, p).map(|g| g.height());
            let with = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, p);
            match (ground, with) {
                (Ok(g), Ok(h)) => {
                    assert!(h >= g);
                    (true, true)
                }
                (ground, Err(FindRocksError::Query(QueryHeightError::NotSurveyed(_)))) => {
                    (ground.is_ok(), false)
                }
                (ground, with) => panic!("{ground:?} and {with:?}"),
            }
        };
        // The first step out of the survey, then the edge of the ground's knowledge between it
        // and the last step in, to a millimetre.
        let step = 1.5e-3;
        let first_out = (0..400_u32)
            .map(|n| f64::from(n) * step)
            .find(|&angle| !query(&at(angle)).0)
            .expect("the line leaves the survey");
        assert!(first_out > 0.0);
        let (mut inside, mut outside) = (first_out - step, first_out);
        while (outside - inside) * figure.equatorial_radius_m > 1e-3 {
            let mid = f64::midpoint(inside, outside);
            if query(&at(mid)).0 {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        // Back into the survey from its edge, half a metre a step over 40 m.
        let mut ground_alone = 0;
        for n in 0..80_u32 {
            let back = f64::from(n) * 0.5 / figure.equatorial_radius_m;
            let (ground, rocks) = query(&at(inside - back));
            assert!(ground, "the ground is known inside its edge");
            if !rocks {
                ground_alone += 1;
            }
        }
        assert!(
            ground_alone > 0,
            "no point knows its ground but not its rocks"
        );
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
        let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
        let height = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p);
        assert!(
            matches!(
                height,
                Err(FindRocksError::Query(QueryHeightError::NotSurveyed(_)))
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
                Err(FindRocksError::Query(QueryHeightError::NotSurveyed(_)))
            ),
            "{near:?}"
        );
    }

    /// The heights with rocks do not depend on what either cache holds or on the order they are
    /// asked in.
    #[test]
    fn the_height_with_rocks_is_order_and_cache_independent() {
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
    }
}
