//! Where the authoritative rocks lie: drawn by hash in cells per diameter octave (plan R11, Design
//! note 5).
//!
//! # The octaves
//!
//! Four diameter octaves, 0.4–0.8, 0.8–1.6, 1.6–3.2 and 3.2–6.4 m ([`ROCK_OCTAVES`]): at k ≤ 0.40 the
//! law's N(>6.4 m) is at most 1.0 × 10⁻⁸ per m², about 10⁻⁵ rocks a 32 m patch, so nothing
//! measurable lies beyond them. Each octave is drawn on one quadtree level of the cube sphere, the
//! deepest whose narrowest cell holds its largest rock's reach ([`rock_octave_levels`]), as R09's
//! crater octaves are, so that a rock drawn in a cell reaches no point outside the cell's 3 × 3
//! neighbourhood, across face edges by the cube's rule, and a query searches that neighbourhood.
//! The reach is [`MAX_REACH_SHARE`] of the octave's top diameter, taken [`REACH_MARGIN`] wider
//! for the ground's depth below the datum, which brings the datum's directions closer. The levels
//! are the body's, and deepen by at least one an octave.
//!
//! # The abundance
//!
//! A cell's rock abundance is its site's ([`rock_abundance`]), read at the
//! centre of its ancestor at the largest octave's level, its **site cell**, about 5 m across on an
//! Earth: every octave's cells within it share one k, and a 32 m patch reads some forty sites, not
//! the thousands of cells its finest octave holds (lean, 2026-11-02). k is so piecewise constant
//! at a few metres, finer than R10's material reaches from orbit and coarser than its finest grid.
//!
//! # The draw
//!
//! A cell's count is a Poisson draw whose mean is the law's number between the octave's diameters,
//! N(>`D_low`) − N(>`D_high`), times the cell's true area on the spheroid
//! ([`cell_area`]), on `surface.scatter` keyed by the cell
//! (instance 0). Its rocks follow in the same stream, each its place, uniform in area over the cell
//! by rejection as R09's craters' are, and a word for its diameter, the law inverted within the
//! octave ([`RockAbundance::diameter_at`]). Each rock's shape is drawn on `surface.scatter.shape`
//! keyed by the cell with the rock's index as the instance ([`RockShape::draw`]), and its axis
//! stands on R10's authoritative ground there, the finest level's collision surface. Every key is
//! built from the cell's integers, every value is `f64`, and the rocks come in draw order.

use hyperion_base::rng::{DetailSeed, ObjectKey};
use hyperion_base::units::Metres;

use super::law::RockAbundance;
use super::shape::{MAX_REACH_SHARE, Rock, RockShape};
use super::site::{rock_abundance, site_at};
use crate::cube::PatchKey;
use crate::field::FieldView;
use crate::patch::HeightSource;
use crate::patch::collision::finest_surface_height;
use crate::spheroid::Spheroid;
use crate::synth::craters::{
    area_density_bound, cell_area, place, search_level, smallest_radius_m,
};
use crate::synth::{QueryHeightError, SynthCache, Synthesiser};
use crate::tags::{SURFACE_SCATTER, SURFACE_SCATTER_SHAPE};

/// The number of rock octaves.
pub const ROCK_OCTAVE_COUNT: usize = 4;

/// The rock octaves' diameters, from each one's lower to its upper: 0.4–0.8, 0.8–1.6, 1.6–3.2 and
/// 3.2–6.4 m (Design note 5), from [`AUTHORITATIVE_ROCK_DIAMETER`](super::AUTHORITATIVE_ROCK_DIAMETER)
/// up.
pub const ROCK_OCTAVES: [(Metres, Metres); ROCK_OCTAVE_COUNT] = [
    (Metres::new(0.4), Metres::new(0.8)),
    (Metres::new(0.8), Metres::new(1.6)),
    (Metres::new(1.6), Metres::new(3.2)),
    (Metres::new(3.2), Metres::new(6.4)),
];

/// The factor on an octave's reach for its level: 1.1, so that a rock's reach, measured at the
/// ground, holds in the datum's directions down to a depth of an eleventh (9.1%) of the body's
/// smallest radius below it, deeper than any basin (Vesta's Rheasilvia, some 20 km on a 262 km
/// body, is 7.6%; from memory).
pub const REACH_MARGIN: f64 = 1.1;

/// The quadtree level of each rock octave on `figure`'s body (see the [module
/// documentation](self)), from the smallest rocks' to the largest's, strictly shallower an octave.
///
/// # Panics
///
/// If no level's cells hold an octave's reach, which no body of a few kilometres or more meets.
#[must_use]
pub fn rock_octave_levels(figure: &Spheroid) -> [u8; ROCK_OCTAVE_COUNT] {
    let radius_m = smallest_radius_m(figure);
    let levels = ROCK_OCTAVES
        .map(|(_, high)| search_level(REACH_MARGIN * MAX_REACH_SHARE * high.value(), radius_m));
    assert!(
        levels.windows(2).all(|pair| pair[1] < pair[0]),
        "the rock octaves' levels shallow an octave: {levels:?}"
    );
    levels
}

/// The octave whose level is `level` on a body of rock octave levels `levels`, if any.
#[must_use]
pub(crate) fn octave_of(levels: [u8; ROCK_OCTAVE_COUNT], level: u8) -> Option<usize> {
    levels.iter().position(|&l| l == level)
}

/// A rock placement cell: a cell of one rock octave's quadtree level (R09's `level` slot), whose
/// draws are keyed through [`ObjectKey::surface_cell`] (the body is in the detail seed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RockCellKey(PatchKey);

impl RockCellKey {
    /// The cell `cell`, at whatever level.
    ///
    /// [`rocks_in_cell`] checks the level against the body's octave levels, and panics on another.
    #[must_use]
    pub const fn new(cell: PatchKey) -> Self {
        Self(cell)
    }

    /// The cell of octave `octave`'s level on `figure`'s body that holds the unit direction `dir`.
    ///
    /// # Panics
    ///
    /// If `octave` is not below [`ROCK_OCTAVE_COUNT`], or `dir` is zero or not finite; or as
    /// [`rock_octave_levels`].
    #[must_use]
    pub fn containing(figure: &Spheroid, octave: usize, dir: [f64; 3]) -> Self {
        assert!(
            octave < ROCK_OCTAVE_COUNT,
            "there are {ROCK_OCTAVE_COUNT} rock octaves, not {octave}"
        );
        let level = rock_octave_levels(figure)[octave];
        Self(
            PatchKey::containing(level, dir).expect("a rock octave's level is at most the deepest"),
        )
    }

    /// The cell as a patch key.
    #[must_use]
    pub const fn cell(self) -> PatchKey {
        self.0
    }

    /// The cube face, 0 to 5.
    #[must_use]
    pub const fn face(self) -> u8 {
        self.0.face().index()
    }

    /// The quadtree level.
    #[must_use]
    pub const fn level(self) -> u8 {
        self.0.level()
    }

    /// The cell's coordinate along the face's s.
    #[must_use]
    pub const fn i(self) -> u32 {
        self.0.i()
    }

    /// The cell's coordinate along the face's t.
    #[must_use]
    pub const fn j(self) -> u32 {
        self.0.j()
    }

    /// The key of the cell's draw `instance`: [`ObjectKey::surface_cell`] of its face, level and
    /// coordinates, every part an integer.
    ///
    /// # Panics
    ///
    /// Never: a patch key's face, level and coordinates are always a surface cell's.
    #[must_use]
    pub fn object_key(self, instance: u16) -> ObjectKey {
        ObjectKey::surface_cell(self.face(), self.level(), self.i(), self.j(), instance)
            .expect("a patch key is a surface cell")
    }
}

/// Why the rocks could not be found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindRocksError {
    /// A rock's site or ground reads a cell the view does not hold, so this side does not know the
    /// rocks there: on a client, no rock is drawn there and nothing reaches it.
    Query(QueryHeightError),
}

impl std::fmt::Display for FindRocksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Query(e) => write!(f, "the rocks are not known here: {e}"),
        }
    }
}

impl std::error::Error for FindRocksError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Query(e) => Some(e),
        }
    }
}

impl From<QueryHeightError> for FindRocksError {
    fn from(e: QueryHeightError) -> Self {
        Self::Query(e)
    }
}

/// The site cell of `cell`: its ancestor at `site_level`, the largest octave's level, at or above
/// its own.
#[must_use]
pub(crate) fn site_cell(cell: PatchKey, site_level: u8) -> PatchKey {
    let up = cell
        .level()
        .checked_sub(site_level)
        .expect("a rock cell lies at or below its site's level");
    PatchKey::new(cell.face(), site_level, cell.i() >> up, cell.j() >> up)
        .expect("an ancestor of a patch key is a patch key")
}

/// The rock abundance of the site cell `site`: its site's at its centre (see the [module
/// documentation](self)).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell the site reads that the view does not
/// hold.
pub(crate) fn site_abundance<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    site: PatchKey,
) -> Result<RockAbundance, QueryHeightError> {
    let centre = site.vertex_dir(32, 32);
    Ok(rock_abundance(&site_at(synth, synth_cache, centre)?))
}

/// One rock's draw from its cell's stream: its axis and its diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RockDraw {
    pub(crate) dir: [f64; 3],
    pub(crate) diameter: Metres,
}

/// The draws of `cell` of octave `octave` at the abundance `k`, from the cell's own stream on
/// `surface.scatter` (see the [module documentation](self)), in draw order.
#[must_use]
pub(crate) fn draw_cell(
    seed: DetailSeed,
    cell: PatchKey,
    octave: usize,
    k: RockAbundance,
    figure: &Spheroid,
) -> Vec<RockDraw> {
    let (low, high) = ROCK_OCTAVES[octave];
    let mean = k.number_between_per_m2(low, high) * cell_area(cell, figure).value();
    let mut stream = seed.stream(
        SURFACE_SCATTER,
        ObjectKey::surface_cell(cell.face().index(), cell.level(), cell.i(), cell.j(), 0)
            .expect("a patch key is a surface cell"),
    );
    let count = stream.poisson(mean);
    if count == 0 {
        return Vec::new();
    }
    let bound = area_density_bound(cell, figure);
    // A capacity hint alone, which a count beyond usize's range (never reached: the mean is under
    // one a cell) leaves at none; the loop draws every rock whatever it is.
    let mut draws = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for _ in 0..count {
        let dir = place(cell, figure, bound, &mut stream);
        let u = stream.uniform();
        draws.push(RockDraw {
            dir,
            diameter: k.diameter_at(low, high, u),
        });
    }
    draws
}

/// The rocks of `cell` of octave `octave` at the abundance `k`, appended to `out` in draw order:
/// each draw's shape from `surface.scatter.shape` and its ground from R10's, the finest level's
/// collision surface over `ground`'s vertices, which are `synth`'s own, bit for bit (`synth`
/// itself, or the patch's heights a bake has made, R11.T4.a's).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell a rock's ground reads that the view does
/// not hold; `out` then holds the rocks made before it.
pub(crate) fn rocks_of<F, G>(
    synth: &Synthesiser<'_, F>,
    ground: &G,
    synth_cache: &mut SynthCache,
    cell: PatchKey,
    octave: usize,
    k: RockAbundance,
    out: &mut Vec<Rock>,
) -> Result<(), QueryHeightError>
where
    F: FieldView,
    G: HeightSource<Cache = SynthCache, Error = QueryHeightError>,
{
    let figure = synth.field().header().figure();
    let seed = synth.seed();
    for (index, draw) in (0_u32..).zip(draw_cell(seed, cell, octave, k, &figure)) {
        let instance = u16::try_from(index).expect("a cell holds far fewer than 2^16 rocks");
        let mut stream = seed.stream(
            SURFACE_SCATTER_SHAPE,
            ObjectKey::surface_cell(
                cell.face().index(),
                cell.level(),
                cell.i(),
                cell.j(),
                instance,
            )
            .expect("a patch key is a surface cell"),
        );
        let shape = RockShape::draw(&mut stream);
        let ground_m = finest_surface_height(ground, draw.dir, synth_cache)?;
        out.push(Rock::new(&figure, draw.dir, ground_m, draw.diameter, shape));
    }
    Ok(())
}

/// The authoritative rocks drawn in `cell`, appended to `out` in draw order.
///
/// See the [module documentation](self). `synth_cache` is the caller's, and the rocks do not depend
/// on what it holds.
///
/// # Errors
///
/// [`FindRocksError::Query`] where the cell's site or a rock's ground reads a cell the view does not
/// hold: never "no rocks there". `out` may then hold some of the cell's rocks.
///
/// # Panics
///
/// If `cell`'s level is not one of the body's rock octave levels ([`rock_octave_levels`]); or if a
/// height is not finite (a bug).
pub fn rocks_in_cell<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    cell: RockCellKey,
    out: &mut Vec<Rock>,
) -> Result<(), FindRocksError> {
    let levels = rock_octave_levels(&synth.field().header().figure());
    let octave = octave_of(levels, cell.level()).unwrap_or_else(|| {
        panic!(
            "level {} is not a rock octave's level, {levels:?}",
            cell.level()
        )
    });
    let site = site_cell(cell.cell(), levels[ROCK_OCTAVE_COUNT - 1]);
    let k = site_abundance(synth, synth_cache, site)?;
    Ok(rocks_of(
        synth,
        synth,
        synth_cache,
        cell.cell(),
        octave,
        k,
        out,
    )?)
}

/// The rocks of `cell` at the abundance `k`, whatever its site's, appended to `out` in draw order.
///
/// It is [`rocks_in_cell`]'s draw at a stated k, for the benches and other crates' tests (feature
/// `testing`), never the generator's.
///
/// # Errors
///
/// [`FindRocksError::Query`] where a rock's ground reads a cell the view does not hold.
///
/// # Panics
///
/// As [`rocks_in_cell`].
#[cfg(any(test, feature = "testing"))]
pub fn rocks_in_cell_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache,
    cell: RockCellKey,
    k: RockAbundance,
    out: &mut Vec<Rock>,
) -> Result<(), FindRocksError> {
    let levels = rock_octave_levels(&synth.field().header().figure());
    let octave = octave_of(levels, cell.level()).unwrap_or_else(|| {
        panic!(
            "level {} is not a rock octave's level, {levels:?}",
            cell.level()
        )
    });
    Ok(rocks_of(
        synth,
        synth,
        synth_cache,
        cell.cell(),
        octave,
        k,
        out,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::super::golden::cells_under;
    use super::super::shape::ROCK_ASPECT;
    use super::super::{AUTHORITATIVE_ROCK_DIAMETER, AUTHORITATIVE_ROCK_HEIGHT, ROCK_HEIGHT_RATIO};
    use crate::cube::Face;
    use crate::geometry::min_rate;
    use crate::synth::test_support::{SEED, world};
    use crate::testing::SyntheticWorld;
    use hyperion_base::math;

    /// The rocks under the site cell of `dir` on `w`, octave by octave.
    fn rocks_about(w: SyntheticWorld, dir: [f64; 3]) -> Vec<Rock> {
        rocks_about_with(w, dir, &mut SynthCache::new())
    }

    /// [`rocks_about`] through `cache`.
    fn rocks_about_with(w: SyntheticWorld, dir: [f64; 3], cache: &mut SynthCache) -> Vec<Rock> {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let levels = rock_octave_levels(&field.header().figure());
        let site = PatchKey::containing(levels[3], dir).unwrap();
        let mut rocks = Vec::new();
        for (_, cell) in cells_under(site, levels) {
            rocks_in_cell(&synth, cache, RockCellKey::new(cell), &mut rocks).unwrap();
        }
        rocks
    }

    /// The directions the tests read the worlds' rocks about.
    const DIRS: [[f64; 3]; 6] = [
        [1.0, 0.1, 0.2],
        [-0.3, 0.9, 0.1],
        [0.2, -0.4, 0.8],
        [0.5, 0.5, -0.7],
        [-0.8, -0.3, 0.4],
        [0.05, 0.6, -0.8],
    ];

    /// Every level holds its octave's reach in its narrowest cell, and the levels shallow an
    /// octave, on bodies from 50 km to twice the Earth.
    #[test]
    fn each_octaves_level_holds_its_reach() {
        for radius in [5e4, 4.7e5, 1.7374e6, 3.3895e6, 6.371e6, 1.2742e7] {
            let figure = Spheroid::sphere(radius);
            let levels = rock_octave_levels(&figure);
            let chord = |level: u8| {
                let width = min_rate() / f64::from(1_u32 << level);
                0.98 * 2.0 * radius * math::sin(0.5 * width)
            };
            for (octave, &level) in levels.iter().enumerate() {
                let reach = REACH_MARGIN * MAX_REACH_SHARE * ROCK_OCTAVES[octave].1.value();
                assert!(reach <= chord(level), "{radius} m, octave {octave}");
                // The next level down would not hold it, so the level is the deepest.
                if level < crate::cube::MAX_LEVEL {
                    assert!(reach > chord(level + 1), "{radius} m, octave {octave}");
                }
            }
        }
    }

    /// The octaves run from the authoritative diameter, each twice its lower, up to 6.4 m.
    #[test]
    fn the_octaves_cover_the_authoritative_rocks() {
        assert_eq!(ROCK_OCTAVES[0].0, AUTHORITATIVE_ROCK_DIAMETER);
        for pair in ROCK_OCTAVES.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        for (low, high) in ROCK_OCTAVES {
            assert_eq!(high.value(), 2.0 * low.value());
        }
        assert_eq!(ROCK_OCTAVES[3].1, Metres::new(6.4));
    }

    /// A cell's key is the surface cell of its integers, and its draws are keyed by it.
    #[test]
    fn every_key_is_built_from_integers() {
        let cell = PatchKey::new(Face::NegZ, 23, 1_234_567, 7_654_321).unwrap();
        let key = RockCellKey::new(cell);
        for instance in [0_u16, 1, 7, u16::MAX] {
            assert_eq!(
                key.object_key(instance),
                ObjectKey::surface_cell(5, 23, 1_234_567, 7_654_321, instance).unwrap()
            );
        }
        assert_eq!(
            (key.face(), key.level(), key.i(), key.j()),
            (5, 23, 1_234_567, 7_654_321)
        );
    }

    /// The same seed gives the same rocks twice, from a cold cache and a warm one.
    #[test]
    fn the_same_seed_gives_the_same_rocks_twice() {
        for w in [SyntheticWorld::MoonLike, SyntheticWorld::EarthLike] {
            let mut warm = SynthCache::new();
            for dir in DIRS {
                let first = rocks_about(w, dir);
                assert_eq!(first, rocks_about(w, dir));
                assert_eq!(first, rocks_about_with(w, dir, &mut warm));
            }
        }
    }

    /// The rocks of a cell do not depend on what the synthesis cache holds or on the order the
    /// cells are asked in, to the bit.
    #[test]
    fn the_rocks_of_a_cell_are_order_and_cache_independent() {
        let w = SyntheticWorld::MoonLike;
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let levels = rock_octave_levels(&field.header().figure());
        let mut cells = Vec::new();
        for dir in DIRS {
            let site = PatchKey::containing(levels[3], dir).unwrap();
            cells.extend(
                cells_under(site, levels)
                    .into_iter()
                    .map(|(_, c)| c)
                    .step_by(5),
            );
        }
        let print = |rocks: Vec<Rock>| format!("{rocks:?}");
        let shared = core::cell::RefCell::new(SynthCache::new());
        let warm = |cell: &PatchKey| {
            let mut rocks = Vec::new();
            let cache = &mut *shared.borrow_mut();
            rocks_in_cell(&synth, cache, RockCellKey::new(*cell), &mut rocks).unwrap();
            print(rocks)
        };
        hyperion_testkit::order::assert_order_independent(&cells, warm);
        for cell in &cells {
            let mut rocks = Vec::new();
            rocks_in_cell(
                &synth,
                &mut SynthCache::new(),
                RockCellKey::new(*cell),
                &mut rocks,
            )
            .unwrap();
            assert_eq!(print(rocks), warm(cell));
        }
    }

    /// At the site's own k, the draw at a stated k is the cell's own draw; and a cell containing a
    /// direction is the patch key's at the octave's level.
    #[test]
    fn the_draw_at_a_stated_k_and_the_containing_cell_agree() {
        let w = SyntheticWorld::MoonLike;
        let field = world(w);
        let figure = field.header().figure();
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let levels = rock_octave_levels(&figure);
        for dir in DIRS {
            let site = PatchKey::containing(levels[3], dir).unwrap();
            let k = site_abundance(&synth, &mut cache, site).unwrap();
            for (octave, cell) in cells_under(site, levels) {
                let key = RockCellKey::new(cell);
                let (mut own, mut stated) = (Vec::new(), Vec::new());
                rocks_in_cell(&synth, &mut cache, key, &mut own).unwrap();
                rocks_in_cell_at(&synth, &mut cache, key, k, &mut stated).unwrap();
                assert_eq!(own, stated);
                let at = cell.vertex_dir(17, 40);
                let containing = RockCellKey::containing(&figure, octave, at);
                assert_eq!(
                    containing.cell(),
                    PatchKey::containing(levels[octave], at).unwrap()
                );
            }
        }
    }

    /// No rock is narrower than 0.4 m or wider than 6.4 m, every one is in its octave's range, every
    /// rock 0.2 m tall or more is among them, and the drawn ratios lie in their ranges.
    #[test]
    fn every_rock_is_authoritative_and_shaped_within_its_ranges() {
        let mut seen = 0;
        for w in [
            SyntheticWorld::MoonLike,
            SyntheticWorld::MarsLike,
            SyntheticWorld::EarthLike,
        ] {
            for dir in DIRS {
                for rock in rocks_about(w, dir) {
                    seen += 1;
                    let d = rock.diameter().value();
                    assert!((0.4..=6.4).contains(&d), "{d}");
                    let shape = rock.shape();
                    let (lo, hi) = ROCK_HEIGHT_RATIO;
                    assert!((lo..=hi).contains(&shape.height_ratio()));
                    assert!((ROCK_ASPECT.0..=ROCK_ASPECT.1).contains(&shape.aspect()));
                    // A rock 0.2 m tall is at least the authoritative diameter across.
                    if rock.exposed_height() >= AUTHORITATIVE_ROCK_HEIGHT {
                        assert!(rock.diameter() >= AUTHORITATIVE_ROCK_DIAMETER);
                    }
                }
            }
        }
        assert!(seen > 0, "the worlds' sites hold some rocks");
    }

    /// A rock's axis lies in its cell, and its octave's diameters hold its diameter.
    #[test]
    fn each_rock_lies_in_its_cell_and_its_octave() {
        let field = world(SyntheticWorld::MarsLike);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let levels = rock_octave_levels(&field.header().figure());
        for dir in DIRS {
            let site = PatchKey::containing(levels[3], dir).unwrap();
            for (octave, cell) in cells_under(site, levels) {
                let mut rocks = Vec::new();
                rocks_in_cell(&synth, &mut cache, RockCellKey::new(cell), &mut rocks).unwrap();
                let (low, high) = ROCK_OCTAVES[octave];
                for rock in rocks {
                    assert_eq!(PatchKey::containing(cell.level(), rock.dir()), Ok(cell));
                    assert!(rock.diameter() >= low && rock.diameter() <= high);
                }
            }
        }
    }

    /// A site cell is the ancestor at its level of every octave's cell under it.
    #[test]
    fn a_site_cell_is_its_cells_ancestor() {
        let figure = Spheroid::sphere(3.3895e6);
        let levels = rock_octave_levels(&figure);
        let site = PatchKey::containing(levels[3], [0.3, 0.4, -0.5]).unwrap();
        for (_, cell) in cells_under(site, levels) {
            assert_eq!(site_cell(cell, levels[3]), site);
        }
    }

    /// The tests that take minutes (`just test-slow`).
    mod slow {
        use super::*;
        use hyperion_testkit::stats::{
            ALPHA, assert_p_value, assert_poisson_count, chi_square_gof,
        };
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        /// The cells the slow test draws at `level`: a block of 317 × 317, about 10⁵, by the middle
        /// of face 1.
        fn block(level: u8) -> impl Iterator<Item = PatchKey> {
            let side = 1_u32 << level;
            let start = side / 2 - 158;
            (0..317_u32).flat_map(move |di| {
                (0..317_u32).map(move |dj| {
                    PatchKey::new(Face::NegX, level, start + di, start + dj).unwrap()
                })
            })
        }

        /// The diameter bins of an octave: eight of equal share at `k`.
        const BINS: usize = 8;

        /// The counts over 10⁵ cells at three k match the law's expectation in each octave: each
        /// octave's total against its Poisson mean, all twelve, and its rocks over eight diameter
        /// bins of equal share by chi-square, at α = 10⁻³, wherever the octave expects 40 rocks or
        /// more, which nine of the twelve do.
        #[test]
        #[ignore = "slow: 1.2 x 10^6 cells' rocks at three abundances, four octaves"]
        fn the_counts_follow_the_law_in_each_octave() {
            let figure = Spheroid::sphere(6.371e6);
            let levels = rock_octave_levels(&figure);
            let seed = SEED;
            let mut tested = 0;
            for k in [0.05, 0.18, 0.40] {
                let law = RockAbundance::new(k);
                for (octave, &level) in levels.iter().enumerate() {
                    let (low, high) = ROCK_OCTAVES[octave];
                    let per_m2 = law.number_between_per_m2(low, high);
                    let edges: Vec<Metres> = (1..BINS)
                        .map(|b| {
                            let share = f64::from(u32::try_from(b).unwrap()) / 8.0;
                            law.diameter_at(low, high, share)
                        })
                        .collect();
                    let mut mean = 0.0;
                    let mut bins = [0_u64; BINS];
                    for cell in block(level) {
                        mean += per_m2 * cell_area(cell, &figure).value();
                        for draw in draw_cell(seed, cell, octave, law, &figure) {
                            let bin = edges.iter().filter(|&&e| draw.diameter >= e).count();
                            bins[bin] += 1;
                        }
                    }
                    let total: u64 = bins.iter().sum();
                    let name = format!("k = {k}, octave {octave}");
                    assert_poisson_count(&name, total, mean, ALPHA);
                    // The bins' chi-square only where each bin expects five rocks or more.
                    if mean < 40.0 {
                        continue;
                    }
                    tested += 1;
                    let expected = [count_as_f64(total) / 8.0; BINS];
                    let fit = chi_square_gof(&bins, &expected);
                    assert_p_value(&name, fit.p_value, ALPHA);
                }
            }
            assert!(tested >= 9, "{tested} octaves tested");
        }

        /// `n` as an `f64`, exact below 2⁵³.
        fn count_as_f64(n: u64) -> f64 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a count of rocks, far below 2^53"
            )]
            let x = n as f64;
            x
        }
    }
}
