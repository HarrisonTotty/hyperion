//! The height goldens (plan R09, T9): the whole height function and each of its parts on every
//! synthetic world, pinned as `f64` bits in `tests/golden/height/`, blessed natively and compared on
//! `wasm32-wasip1` and on the browser's `wasm32-unknown-unknown`, where the testkit's
//! [`golden!`] embeds each file by its literal name (plan R04, Design note 14).
//!
//! The server collides with the heights [`Synthesiser::height_at`] answers natively, and the client
//! draws those its WebAssembly workers answer, so a split between the targets is ground the ship
//! does not touch (the brainstorm's "Determinism hazards specific to terrain"). The files carry
//! `GENERATOR_VERSION`, as R09's other goldens do, in a subdirectory that R05's check of
//! `TEST_PLANET_VERSION` over the files directly in `tests/golden/` does not read. A change to any
//! synthesis constant, draw or order of summation moves them, and is a generator-version change.
//! They are a module of the crate rather than a test of its own (`tests/`), since the synthetic
//! worlds are the `testing` feature's, which the browser target's test build does not enable.
//!
//! # The files
//!
//! Two of each world, whose stems are `earth_like`, `mars_like`, `moon_like`, `ceres_like`, `flat`
//! and `one_crater`:
//!
//! - `<world>_points`: [`Synthesiser::height_at`] at 64 sites, each at eight levels: 0, L − 1 to
//!   L + 4 and the complete level, the shallowest whose shortest wavelength is the band limit; 512
//!   samples. The sites are the cube's eight corners; eight vertices on face edges; 16 about four
//!   coarse craters, at 0.97, 1.0 and 1.04 rim radii and just inside the reach; 12 in coarse cells
//!   whose channels reach L + 4 or deeper (each cell's key point, its trunk's midpoint and two
//!   points at random), the first cell on a face edge; eight in cells whose belt weight is between 0
//!   and 1; and 12 at random, a cube corner's cell and two face edges' first. A world without
//!   craters, channels or belts takes random points in their place. Every site's height at the
//!   complete level is asserted equal, to the bit, to its height one level finer, at the finest
//!   level ([`finest_level`]) and at the deepest.
//! - `<world>_parts`: the contributions apart from the whole, as R09.T4 to T7.c's determinism
//!   reviews asked: the body's levels; the base elevation at every site
//!   ([`interp::base_elevation`]); the structural octaves' table and [`Relief::at`] through L + 3,
//!   the last octave before the warp, and through the finest; the channels' key points to L + 3,
//!   their segments and the first cell of each case R09.T6.a's review named (a trunk into another
//!   face, into the sea, held at its crossing; a tributary joining on another face, clamped at its
//!   parent's end, on the slope floor), their incision through L to L + 4 and their closed forms;
//!   the small craters' octave table, the craters drawn in a few cells of the first, middle and last
//!   octaves and the first cell from the corner's that holds one, and [`SmallCraters::at`] at the
//!   corner and edge sites and at such a crater's centre and rim; and the coarse craters' band, kept
//!   reliefs and [`CoarseRims::at`] at L − 4, L + 2 and the finest level, about four craters and at
//!   the face-edge vertices their reaches cross where the rims are more than a metre high.
//!
//! And four of every world:
//!
//! - `common`: R05's band limit and finest spacing, the Nyquist degrees, the 32 octave offsets of
//!   the tests' detail seed, and [`interp::interpolate`] over a closed form at level 2.
//! - `levels`: each world's [`LevelSet`], [`LevelBounds::bound_m`] and
//!   [`LevelBounds::contribution_range_m`] at every band level, which R10 reads.
//! - `closed_forms`: [`Synthesiser::unresolved_variance`] by contribution,
//!   [`Synthesiser::structure_function`] and [`Synthesiser::unresolved_rms`] at a cube corner's
//!   cell, a face edge's, and a cell under a coarse rim, a channelled cell and a belt's where the
//!   world has them; and at the same cells the level-keyed forms at every band level,
//!   [`Synthesiser::omitted_variance`] by contribution and
//!   [`Synthesiser::omitted_structure_function`] at lags of 2 m, 5 m and the level's shortest
//!   wavelength (R10.T5; R09.T7.h).
//! - `bakes`: one [`Synthesiser::bake_patch`] a world, at L + 3 in a channelled cell, or a rim's,
//!   or a random one: the digests of its samples and parent heights, and a few of each.
//!
//! # Format
//!
//! After the header, a line without `=` opens a section (a world, a site and its direction, a part
//! or a cell), and `label = values` lines follow, every float as `0x` and the 16 hexadecimal digits
//! of its IEEE 754 bits; a height sample is its height and its gradient's three components, with the
//! height in decimal metres after a `#` for the reader. A digest is
//! [`f64_digest`](hyperion_testkit::golden::f64_digest) over the values in the order named.
//!
//! [`LevelSet`]: super::LevelSet
//! [`Relief::at`]: super::Relief::at
//! [`SmallCraters::at`]: super::SmallCraters::at
//! [`CoarseRims::at`]: super::craters::rims::CoarseRims::at

use hyperion_base::math;
use hyperion_base::units::{Metres, SquareMetres};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::channels::{
    ChannelBody, ChannelNetwork, MINIMUM_SLOPE, Outlet, Segment, bankfull_depth, bankfull_width,
    channel_head_area, channel_structure_function, channel_variance, deepest_level, hack_area,
    head_length, main_stream_length, tributary_rise, valley_depth,
};
use super::craters::rims::{coarse_band_half_width, kept_relief, rims_structure_function};
use super::craters::{CraterCache, CraterOctave, SmallCrater, SmallCraters};
use super::interp::{self, CellValues};
use super::relief::{
    ReliefStyle, nyquist_degree, octave_bound, octave_offsets, octave_spacing,
    relief_structure_function,
};
use super::test_support::{SEED, random_cells, random_points_in, rim_cells, sample_bits, world};
use super::{BandLevel, LevelBounds, SynthCache, Synthesiser};
use crate::craters::crater_shape;
use crate::cube::{Face, MAX_LEVEL, PatchKey, unit_dir};
use crate::field::{CoarseCrater, CoarseField, FieldView, cell_at_index, cell_index};
use crate::geometry::{BAND_LIMIT_M, FINEST_SPACING_M, finest_level};
use crate::height::HeightSample;
use crate::testing::SyntheticWorld;

/// The sites a world's points and parts are pinned at.
const SITES: usize = 64;

/// The sites of the channelled cells, among a world's [`SITES`]: after the eight corners, the
/// eight face edges and the 16 rim sites.
const CHANNEL_SITES: core::ops::Range<usize> = 32..44;

/// The rim radii from a coarse crater's centre at which the rim sites sit, beside one just inside
/// its reach.
const RIM_RADII: [f64; 3] = [0.97, 1.0, 1.04];

/// The share of a coarse crater's reach, as a chord from its centre, at which a site sits "just
/// inside" it.
const INSIDE_REACH: f64 = 0.98;

/// The file stem of `kind`'s goldens.
const fn stem(kind: SyntheticWorld) -> &'static str {
    match kind {
        SyntheticWorld::EarthLike => "earth_like",
        SyntheticWorld::MarsLike => "mars_like",
        SyntheticWorld::MoonLike => "moon_like",
        SyntheticWorld::CeresLike => "ceres_like",
        SyntheticWorld::Flat => "flat",
        SyntheticWorld::OneCrater => "one_crater",
    }
}

/// `kind`'s place among [`SyntheticWorld::ALL`], which seeds its tests' generators apart.
fn ordinal(kind: SyntheticWorld) -> u64 {
    let at = SyntheticWorld::ALL
        .iter()
        .position(|&w| w == kind)
        .expect("every world is listed");
    u64::try_from(at).expect("six worlds")
}

/// Band level `level`.
fn band(level: u8) -> BandLevel {
    BandLevel::new(level).expect("a pinned level is at most MAX_LEVEL")
}

/// `value`'s IEEE 754 bits, `0x` and 16 hexadecimal digits.
fn hex(value: f64) -> String {
    assert!(
        !value.is_nan(),
        "a pinned value is a NaN, whose bits are unspecified"
    );
    format!("0x{:016x}", bits(value))
}

/// The bits of a vector's three components.
fn hex3(v: [f64; 3]) -> String {
    format!("{} {} {}", hex(v[0]), hex(v[1]), hex(v[2]))
}

/// A level's deepest contribution, or `-` for none.
fn deepest(level: Option<u8>) -> String {
    level.map_or_else(|| "-".to_owned(), |n| n.to_string())
}

/// A cell as its face, level, i and j.
fn key(cell: PatchKey) -> String {
    format!(
        "{}:{}:{}:{}",
        cell.face().index(),
        cell.level(),
        cell.i(),
        cell.j()
    )
}

/// Writes `label = <height> <gradient>  # <height> m`.
fn sample(w: &mut GoldenWriter, label: &str, s: HeightSample) {
    w.line(&format!(
        "{label} = {} {}  # {:?} m",
        hex(s.height_m),
        hex3(s.gradient),
        s.height_m
    ));
}

/// A golden text, its header written.
fn writer() -> GoldenWriter {
    let mut w = GoldenWriter::new();
    w.header(crate::generator_version());
    w
}

/// `a` × `b`.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The unit direction `psi` radians from the unit direction `d` along the great circle towards
/// `azimuth`, measured from a tangent fixed by `d` alone.
fn geodesic(d: [f64; 3], psi: f64, azimuth: f64) -> [f64; 3] {
    let helper = if d[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = unit_dir(cross(helper, d));
    let e2 = cross(d, e1);
    let (sin_a, cos_a) = math::sin_cos(azimuth);
    let (sin_psi, cos_psi) = math::sin_cos(psi);
    unit_dir([0, 1, 2].map(|k| cos_psi * d[k] + sin_psi * (cos_a * e1[k] + sin_a * e2[k])))
}

/// The direction `x` rim radii from `crater`'s centre towards `azimuth`, on a body of volumetric
/// radius `radius_m`: an arc of the chord's length, near enough for a site.
fn near(crater: &CoarseCrater, x: f64, azimuth: f64, radius_m: f64) -> [f64; 3] {
    geodesic(
        crater.centre,
        x * 0.5 * crater.diameter.value() / radius_m,
        azimuth,
    )
}

/// The rim radii at [`INSIDE_REACH`] of `crater`'s reach on `field`'s body.
fn inside_reach(field: &CoarseField, crater: &CoarseCrater) -> f64 {
    let shape = crater_shape(field.header().craters(), crater.diameter);
    INSIDE_REACH * shape.reach().value() / (0.5 * crater.diameter.value())
}

/// An azimuth for the `n`th site about a crater, radians: steps of 1.3 rad from 0.4, so that the
/// sites spread about the centre.
fn azimuth(n: usize) -> f64 {
    0.4 + 1.3 * f64::from(u32::try_from(n).expect("a few sites"))
}

/// The unit direction of the spheroid point `p` (metres) of `field`'s body.
fn dir_of(field: &CoarseField, p: [f64; 3]) -> [f64; 3] {
    let figure = field.header().figure();
    unit_dir([
        p[0] / figure.equatorial_radius_m,
        p[1] / figure.equatorial_radius_m,
        p[2] / figure.polar_radius_m,
    ])
}

/// Every cell of `field`'s level, in cell-index order.
fn coarse_cells(field: &CoarseField) -> impl Iterator<Item = PatchKey> {
    let level = field.header().level();
    (0..level.cell_count()).map(move |n| cell_at_index(level.get(), n).expect("a cell's index"))
}

/// Whether `cell` touches a face edge.
fn on_face_edge(cell: PatchKey) -> bool {
    let last = (1_u32 << cell.level()) - 1;
    cell.i() == 0 || cell.j() == 0 || cell.i() == last || cell.j() == last
}

/// The shallowest level whose shortest wavelength is the band limit: from it on every level
/// evaluates the same contributions with the same band.
fn complete_level(synth: &Synthesiser<'_, CoarseField>) -> u8 {
    (0..=MAX_LEVEL)
        .find(|&n| synth.level_set(band(n)).shortest_wavelength().value() <= BAND_LIMIT_M)
        .expect("the deepest level resolves the band limit")
}

/// Up to `count` of `field`'s listed craters by index, spread through its list, a crater repeated
/// where the list is shorter.
fn spread_craters(field: &CoarseField, count: usize) -> Vec<usize> {
    let n = field.craters().len();
    if n == 0 {
        return Vec::new();
    }
    (0..count).map(|k| k * n / count).collect()
}

/// Up to `count` of `field`'s coarse cells whose channels reach L + 4 or deeper (failing `count`
/// of those, any finer level), the first on a face edge if one is, the rest spread through
/// cell-index order.
fn channelled_cells(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let header = field.header();
    let level = header.level().get();
    let body = ChannelBody::of(header);
    let all: Vec<(PatchKey, u8)> = coarse_cells(field)
        .map(|c| {
            (
                c,
                deepest_level(field.cell(c).expect("a whole field"), body),
            )
        })
        .filter(|&(_, deepest)| deepest > level)
        .collect();
    let deep: Vec<PatchKey> = all
        .iter()
        .filter(|&&(_, deepest)| deepest >= level + 4)
        .map(|&(c, _)| c)
        .collect();
    let pool = if deep.len() >= count {
        deep
    } else {
        all.into_iter().map(|(c, _)| c).collect()
    };
    let mut cells: Vec<PatchKey> = pool
        .iter()
        .copied()
        .find(|&c| on_face_edge(c))
        .into_iter()
        .collect();
    let step = (pool.len() / count.max(1)).max(1);
    for c in pool.iter().copied().step_by(step) {
        if cells.len() == count {
            break;
        }
        if !cells.contains(&c) {
            cells.push(c);
        }
    }
    cells
}

/// Up to `count` of `field`'s coarse cells whose belt weight is between 0 and 1, spread through
/// cell-index order.
fn belt_cells(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let all: Vec<PatchKey> = coarse_cells(field)
        .filter(|&c| {
            let b = ReliefStyle::of(field.cell(c).expect("a whole field")).belt();
            b > 0.0 && b < 1.0
        })
        .collect();
    let step = (all.len() / count.max(1)).max(1);
    all.into_iter().step_by(step).take(count).collect()
}

/// A patch of `level`, at or below `cell`'s, inside `cell`, off its corners.
fn patch_in(cell: PatchKey, level: u8) -> PatchKey {
    let down = level - cell.level();
    let offset = (1_u32 << down) / 3;
    PatchKey::new(
        cell.face(),
        level,
        (cell.i() << down) + offset,
        (cell.j() << down) + offset,
    )
    .expect("a patch inside a cell")
}

/// A point the goldens query: what it is, for the reader, and its unit direction.
struct Site {
    what: String,
    dir: [f64; 3],
}

/// The [`SITES`] of `kind`'s world, in their groups' order (see the module documentation).
fn sites(kind: SyntheticWorld) -> Vec<Site> {
    let field = world(kind);
    let mut rng = Lcg::new(0x6865_6967_6874_0000 + ordinal(kind));
    let mut sites = Vec::with_capacity(SITES);
    corner_sites(&mut sites);
    edge_sites(field, &mut rng, &mut sites);
    rim_sites(field, &mut rng, &mut sites);
    assert_eq!(sites.len(), CHANNEL_SITES.start);
    channel_sites(field, &mut rng, &mut sites);
    assert_eq!(sites.len(), CHANNEL_SITES.end);
    belt_sites(field, &mut rng, &mut sites);
    let rest = SITES - sites.len();
    random_sites(field, &mut rng, &mut sites, rest);
    assert_eq!(sites.len(), SITES);
    sites
}

/// The cube's eight corners.
fn corner_sites(sites: &mut Vec<Site>) {
    for k in 0..8_u8 {
        let sign = |bit: u8| if k & bit == 0 { 1.0 } else { -1.0 };
        sites.push(Site {
            what: format!("cube corner {k}"),
            dir: unit_dir([sign(1), sign(2), sign(4)]),
        });
    }
}

/// Eight vertices of coarse cells on face edges, an edge of each side in turn, at random along it.
fn edge_sites(field: &CoarseField, rng: &mut Lcg, sites: &mut Vec<Site>) {
    let level = field.header().level().get();
    let last = (1_u32 << level) - 1;
    for k in 0..8_u8 {
        let face = Face::ALL[usize::from(k % 6)];
        let along = u32::try_from(rng.next_below(u64::from(last) + 1)).expect("a cell index");
        let at = u8::try_from(rng.next_below(65)).expect("a vertex index");
        let (i, j, x, y) = match k % 4 {
            0 => (last, along, 64, at),
            1 => (0, along, 0, at),
            2 => (along, last, at, 64),
            _ => (along, 0, at, 0),
        };
        let cell = PatchKey::new(face, level, i, j).expect("a cell of the coarse level");
        sites.push(Site {
            what: format!("face edge, vertex ({x}, {y}) of cell {}", key(cell)),
            dir: cell.vertex_dir(x, y),
        });
    }
}

/// Four sites about each of four coarse craters: at [`RIM_RADII`] and just inside the reach.
fn rim_sites(field: &CoarseField, rng: &mut Lcg, sites: &mut Vec<Site>) {
    let picked = spread_craters(field, 4);
    if picked.is_empty() {
        random_sites(field, rng, sites, 16);
        return;
    }
    let radius_m = field.header().radius().value();
    for (n, &index) in picked.iter().enumerate() {
        let crater = &field.craters()[index];
        let inside = inside_reach(field, crater);
        for (k, x) in RIM_RADII.into_iter().chain([inside]).enumerate() {
            let what = if k < RIM_RADII.len() {
                format!("crater {index} at {x} rim radii")
            } else {
                format!("crater {index} just inside its reach, {x:.4} rim radii")
            };
            sites.push(Site {
                what,
                dir: near(crater, x, azimuth(4 * n + k), radius_m),
            });
        }
    }
}

/// Four sites in each of three channelled cells: its key point, its trunk's midpoint and two at
/// random; random sites for the cells the world lacks.
fn channel_sites(field: &CoarseField, rng: &mut Lcg, sites: &mut Vec<Site>) {
    let cells = channelled_cells(field, 3);
    let synth = Synthesiser::new(field, SEED);
    let mut network = synth.channels().network(field);
    for &cell in &cells {
        let point = network.key_point(cell);
        sites.push(Site {
            what: format!("cell {} with channels, its key point", key(cell)),
            dir: point.dir(),
        });
        let trunk = network.segment(cell).expect("a whole field");
        let middle = trunk.map_or_else(
            || random_points_in(cell, 1, rng)[0],
            |t| dir_of(field, t.point_m(0.5)),
        );
        sites.push(Site {
            what: format!("cell {} with channels, its trunk's midpoint", key(cell)),
            dir: middle,
        });
        for dir in random_points_in(cell, 2, rng) {
            sites.push(Site {
                what: format!("cell {} with channels, at random", key(cell)),
                dir,
            });
        }
    }
    random_sites(field, rng, sites, 4 * (3 - cells.len()));
}

/// Two sites at random in each of four cells of a belt weight between 0 and 1; random sites for
/// the cells the world lacks.
fn belt_sites(field: &CoarseField, rng: &mut Lcg, sites: &mut Vec<Site>) {
    let cells = belt_cells(field, 4);
    for &cell in &cells {
        let b = ReliefStyle::of(field.cell(cell).expect("a whole field")).belt();
        for dir in random_points_in(cell, 2, rng) {
            sites.push(Site {
                what: format!("cell {} of belt weight {b:.4}, at random", key(cell)),
                dir,
            });
        }
    }
    random_sites(field, rng, sites, 2 * (4 - cells.len()));
}

/// `count` sites at random, three to a random cell, a cube corner's cell and two face edges' first.
fn random_sites(field: &CoarseField, rng: &mut Lcg, sites: &mut Vec<Site>, count: usize) {
    if count == 0 {
        return;
    }
    let level = field.header().level().get();
    let cells = random_cells(level, count.div_ceil(3).max(3), rng);
    let dirs = cells
        .iter()
        .flat_map(|&cell| {
            random_points_in(cell, 3, rng)
                .into_iter()
                .map(move |d| (cell, d))
        })
        .take(count);
    for (cell, dir) in dirs {
        sites.push(Site {
            what: format!("at random in cell {}", key(cell)),
            dir,
        });
    }
}

/// The eight levels a world's points are pinned at: 0, L − 1 to L + 4 and the complete level.
fn pinned_levels(field: &CoarseField, complete: u8) -> [u8; 8] {
    let l = field.header().level().get();
    assert!(
        l + 4 < complete,
        "the complete level {complete} is past L + 4"
    );
    [0, l - 1, l, l + 1, l + 2, l + 3, l + 4, complete]
}

/// The text of `kind`'s points golden.
fn points(kind: SyntheticWorld) -> String {
    let field = world(kind);
    let synth = Synthesiser::new(field, SEED);
    let complete = complete_level(&synth);
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    let levels = pinned_levels(field, complete);
    let mut w = writer();
    w.line(&format!("world {}", stem(kind)));
    w.line(&format!("coarse level = {}", field.header().level().get()));
    w.line(&format!("complete level = {complete}"));
    w.line(&format!("finest level = {finest}"));
    for (n, site) in sites(kind).iter().enumerate() {
        w.line(&format!("site {n}, {}: dir {}", site.what, hex3(site.dir)));
        let mut cache = SynthCache::new();
        let mut query = |level: u8| {
            synth
                .height_at(&mut cache, site.dir, band(level))
                .expect("the whole field holds every cell")
        };
        for level in levels {
            sample(&mut w, &format!("L{level}"), query(level));
        }
        // The band limit's level is every finer level's, to the bit.
        let at_complete = sample_bits(query(complete));
        for finer in [complete + 1, finest, MAX_LEVEL] {
            if finer > complete {
                assert_eq!(
                    sample_bits(query(finer)),
                    at_complete,
                    "{kind:?} site {n} at level {finer} against the complete level {complete}"
                );
            }
        }
    }
    w.finish()
}

/// The text of `kind`'s parts golden.
fn parts(kind: SyntheticWorld) -> String {
    let field = world(kind);
    let synth = Synthesiser::new(field, SEED);
    let sites = sites(kind);
    let mut w = writer();
    w.line(&format!("world {}", stem(kind)));
    write_body(&mut w, &synth);
    write_base(&mut w, field, &sites);
    write_relief(&mut w, &synth, &sites);
    write_channels(&mut w, &synth, &sites);
    write_small_craters(&mut w, &synth, &sites);
    write_rims(&mut w, &synth);
    w.finish()
}

/// The body's figures and levels.
fn write_body(w: &mut GoldenWriter, synth: &Synthesiser<'_, CoarseField>) {
    let header = synth.field().header();
    let figure = header.figure();
    w.line("body");
    w.f64("radius", header.radius().value());
    w.f64("equatorial radius", figure.equatorial_radius_m);
    w.f64("polar radius", figure.polar_radius_m);
    w.f64("boundary diameter", header.boundary_diameter().value());
    w.line(&format!("coarse level = {}", header.level().get()));
    w.line(&format!("complete level = {}", complete_level(synth)));
    w.line(&format!(
        "finest level = {}",
        finest_level(figure.equatorial_radius_m)
    ));
}

/// The base elevation at every site.
fn write_base(w: &mut GoldenWriter, field: &CoarseField, sites: &[Site]) {
    w.line("base elevation");
    for (n, site) in sites.iter().enumerate() {
        let s = interp::base_elevation(field, site.dir).expect("a whole field");
        sample(w, &format!("site {n}"), s);
    }
}

/// The structural octaves' table, and their height at every fourth site through L + 3, the last
/// octave before the warp, and through the finest.
fn write_relief(w: &mut GoldenWriter, synth: &Synthesiser<'_, CoarseField>, sites: &[Site]) {
    let field = synth.field();
    let header = field.header();
    let relief = synth.relief();
    w.line("structural octaves");
    w.line(&format!("first octave = {}", relief.first_octave()));
    w.line(&format!("finest octave = {}", relief.finest_octave()));
    for m in relief.first_octave()..=relief.finest_octave() {
        w.line(&format!(
            "octave {m} = spacing {} rms {} ridged share {} belt factor {} bound {}",
            hex(octave_spacing(header.radius(), m).value()),
            hex(relief.octave_rms(m).value()),
            hex(relief.ridged_share(m)),
            hex(relief.belt_factor(m)),
            hex(octave_bound(header.spectrum(), m).value()),
        ));
    }
    for (a, b) in [(1.0, 0.0), (0.1, 0.0), (2.5, 1.0), (1.0, 0.5)] {
        w.f64(
            &format!("point variance at amplitude {a} and belt {b}"),
            relief.point_variance(a, b).value(),
        );
    }
    let unwarped = relief.first_octave() + 2;
    let mut cache = SynthCache::new();
    for (n, site) in sites.iter().enumerate().step_by(4) {
        for through in [unwarped, relief.finest_octave()] {
            let s = relief
                .at(field, site.dir, through, &mut cache)
                .expect("a whole field");
            sample(w, &format!("site {n} through {through}"), s);
        }
    }
}

/// The channels: below six channelled cells, each cell's deepest level and closed-form variance and
/// the key points and segments of 13 cells to L + 3; the incision at the corner, edge and channel
/// sites through L to L + 4; and the closed forms of the body.
fn write_channels(w: &mut GoldenWriter, synth: &Synthesiser<'_, CoarseField>, sites: &[Site]) {
    let field = synth.field();
    let channels = synth.channels();
    let body = channels.body();
    w.line("channels");
    let cells = channelled_cells(field, 6);
    w.line(&format!("channelled cells = {}", cells.len()));
    let mut network = channels.network(field);
    for &coarse in &cells {
        let record = field.cell(coarse).expect("a whole field");
        w.line(&format!("below coarse cell {}", key(coarse)));
        w.line(&format!("deepest level = {}", deepest_level(record, body)));
        for finer_than in [1e5, 1e4, 1e3, 100.0] {
            w.f64(
                &format!("channel variance finer than {finer_than} m"),
                channel_variance(record, body, Metres::new(finer_than)).value(),
            );
        }
        let child = coarse.children()[1];
        let grandchild = child.children()[2];
        let below = std::iter::once(coarse)
            .chain(coarse.children())
            .chain(child.children())
            .chain(grandchild.children());
        for cell in below {
            write_network_cell(w, &mut network, cell);
        }
    }
    w.line("channel cases");
    let mut written = Vec::new();
    for (case, cell) in channel_cases(field, &mut network) {
        if let Some(cell) = cell {
            w.line(&format!("case {case} = cell {}", key(cell)));
            if !written.contains(&cell) {
                write_network_cell(w, &mut network, cell);
                written.push(cell);
            }
        } else {
            w.line(&format!("case {case} = none"));
        }
    }
    w.line("incision");
    let level = body.level;
    let mut cache = SynthCache::new();
    for n in (0..16).chain(CHANNEL_SITES) {
        for through in level..=level + 4 {
            let s = channels
                .incision_at(field, sites[n].dir, through, &mut cache)
                .expect("a whole field");
            sample(w, &format!("site {n} through {through}"), s);
        }
    }
    write_channel_closed_forms(w, body);
}

/// The coarse cells with channels whose cells at L + 1 and L + 2 the tributary cases search, at
/// most.
const CASE_SEARCH_CELLS: usize = 64;

/// The trunks the search for one held at its crossing computes, at most: on the Earth-like world 77
/// of 121,310 are held, near the cube's corners, where cell-index order starts.
const HELD_SEARCH_TRUNKS: u32 = 4096;

/// Whether `trunk`'s crossing is held to its edge's central half
/// ([`CROSSING_MARGIN`](super::channels::CROSSING_MARGIN)): then it leaves the great circle through
/// the trunk's key points, as the channels' own test tells it.
fn is_held(field: &CoarseField, trunk: &Segment) -> bool {
    let crossing = trunk.crossing_m().expect("a trunk crosses");
    let through = unit_dir(cross(
        dir_of(field, trunk.from_m()),
        dir_of(field, trunk.to_m()),
    ));
    let d = dir_of(field, crossing);
    (through[0] * d[0] + through[1] * d[1] + through[2] * d[2]).abs() > 1e-12
}

/// The first cell, in cell-index order, of each case R09.T6.a's determinism review named (Risks,
/// "Deviations in T6.a, as built"), or `None` where the world has none: a trunk into a cell of
/// another face, a trunk into the sea, a trunk held at its crossing (within
/// [`HELD_SEARCH_TRUNKS`]), and, among the cells at L + 1 and L + 2 below the first
/// [`CASE_SEARCH_CELLS`] coarse cells with tributaries (on a face edge for the first), a tributary
/// whose join lies on another face, one clamped at its parent's end and one whose bed's slope at its
/// outlet is the floor ([`MINIMUM_SLOPE`]).
fn channel_cases(
    field: &CoarseField,
    network: &mut ChannelNetwork<'_, CoarseField>,
) -> [(&'static str, Option<PatchKey>); 6] {
    let header = field.header();
    let level = header.level().get();
    let body = ChannelBody::of(header);
    let mut across = None;
    let mut to_sea = None;
    let mut held = None;
    let mut trunks = 0;
    for c in coarse_cells(field) {
        let seeking_held = held.is_none() && trunks < HELD_SEARCH_TRUNKS;
        if across.is_some() && to_sea.is_some() && !seeking_held {
            break;
        }
        let record = field.cell(c).expect("a whole field");
        let Some(receiver) = record.flow.edge().map(|e| c.edge_neighbour(e)) else {
            continue;
        };
        let is_across = across.is_none() && receiver.face() != c.face();
        let is_to_sea = to_sea.is_none()
            && field
                .cell(receiver)
                .expect("a whole field")
                .is_under_water();
        if !(is_across || is_to_sea || seeking_held) {
            continue;
        }
        let Some(trunk) = network.segment(c).expect("a whole field") else {
            continue;
        };
        trunks += 1;
        if is_across {
            across = Some(c);
        }
        if is_to_sea {
            to_sea = Some(c);
        }
        if seeking_held && is_held(field, &trunk) {
            held = Some(c);
        }
    }
    let channelled: Vec<PatchKey> = coarse_cells(field)
        .filter(|&c| deepest_level(field.cell(c).expect("a whole field"), body) > level)
        .collect();
    let mut tributary = |on_edge: bool, case: &dyn Fn(PatchKey, &Segment) -> bool| {
        let pool = channelled
            .iter()
            .copied()
            .filter(|&c| !on_edge || on_face_edge(c))
            .take(CASE_SEARCH_CELLS);
        for coarse in pool {
            let below = coarse.children().into_iter().flat_map(PatchKey::children);
            for cell in coarse.children().into_iter().chain(below) {
                let segment = network.segment(cell).expect("a whole field");
                if segment.is_some_and(|s| !s.is_trunk() && case(cell, &s)) {
                    return Some(cell);
                }
            }
        }
        None
    };
    let joins_across = tributary(true, &|cell, s| {
        PatchKey::containing(cell.level(), dir_of(field, s.to_m()))
            .expect("a level")
            .face()
            != cell.face()
    });
    let clamped = tributary(false, &|_, s| match s.outlet() {
        Outlet::Join { at, .. } => bits(at) == bits(0.0) || bits(at) == bits(1.0),
        Outlet::Cell { .. } => false,
    });
    let on_floor = tributary(false, &|_, s| bits(s.slope_at(1.0)) == bits(MINIMUM_SLOPE));
    [
        ("a trunk into another face", across),
        ("a trunk into the sea", to_sea),
        ("a trunk held at its crossing", held),
        ("a tributary joining on another face", joins_across),
        ("a tributary clamped at its parent's end", clamped),
        ("a tributary on the slope floor", on_floor),
    ]
}

/// `cell`'s key point and the segment it heads.
fn write_network_cell(
    w: &mut GoldenWriter,
    network: &mut ChannelNetwork<'_, CoarseField>,
    cell: PatchKey,
) {
    let point = network.key_point(cell);
    let (s, t) = point.st();
    w.line(&format!(
        "key point {} = face {} st {} {} inherited {}",
        key(cell),
        point.face().index(),
        hex(s),
        hex(t),
        network.is_inherited(cell)
    ));
    let Some(segment) = network.segment(cell).expect("a whole field") else {
        w.line(&format!("segment {} = none", key(cell)));
        return;
    };
    let outlet = match segment.outlet() {
        Outlet::Cell { receiver, exit } => format!("cell {} exit {}", key(receiver), hex(exit)),
        Outlet::Join { parent, at } => format!("join {} at {}", key(parent), hex(at)),
    };
    w.line(&format!(
        "segment {} = {} to {outlet} from {} to {} crossing {}",
        key(cell),
        if segment.is_trunk() {
            "trunk"
        } else {
            "tributary"
        },
        hex3(segment.from_m()),
        hex3(segment.to_m()),
        segment.crossing_m().map_or_else(|| "-".to_owned(), hex3),
    ));
    w.line(&format!(
        "bed {} = from {} to {} midway {} length {} slope {} below the top {}",
        key(cell),
        hex(segment.bed_from().value()),
        hex(segment.bed_to().value()),
        hex(segment.bed_at(0.5).value()),
        hex(segment.length().value()),
        hex(segment.slope_at(0.5)),
        hex(segment.below_top(0.5).value()),
    ));
    w.line(&format!(
        "channel {} = steepness {} main stream {} area {} discharge {} width {} depth {}",
        key(cell),
        segment.steepness_m0_9().map_or_else(|| "-".to_owned(), hex),
        hex(segment.main_stream_at(0.5).value()),
        hex(segment.area_at(0.5).value()),
        hex(segment.mean_discharge_m3_s()),
        hex(segment.bankfull_width().value()),
        hex(segment.bankfull_depth().value()),
    ));
}

/// The channels' closed forms on `body`: Hack's law both ways, the heads' lengths, tributaries'
/// rises about the slope floor's crossover, the hydraulic geometry, valley depths and channel
/// heads.
fn write_channel_closed_forms(w: &mut GoldenWriter, body: ChannelBody) {
    w.line("channel closed forms");
    for area in [1e4, 1e6, 1e8, 1e10, 1e12] {
        w.f64(
            &format!("main stream length of {area} m2"),
            main_stream_length(SquareMetres::new(area)).value(),
        );
    }
    for length in [1e2, 1e3, 1e4, 1e5, 1e6] {
        w.f64(
            &format!("hack area of {length} m"),
            hack_area(Metres::new(length)).value(),
        );
    }
    for level in body.level..=MAX_LEVEL {
        w.f64(
            &format!("head length at level {level}"),
            head_length(body.radius, level).value(),
        );
    }
    // The slope k_s C^¾ L^−¾ meets the floor at L* = (k_s C^¾ ÷ 10⁻⁵)^(4/3): 1.0 km at
    // 0.0043 m^0.9 and 1,500 km at 1, which the heads and lengths straddle, and 2.7 × 10⁵ km at
    // 50, beyond them all, where the floor never binds.
    for steepness in [0.0043, 1.0, 50.0] {
        for head in [10.0, 1e3, 1e5, 1e7] {
            for length in [1e2, 1e4, 1e6] {
                w.f64(
                    &format!("tributary rise at {steepness} from {head} m over {length} m"),
                    tributary_rise(steepness, Metres::new(head), Metres::new(length)).value(),
                );
            }
        }
        w.f64(
            &format!("channel head area at {steepness}"),
            channel_head_area(steepness).value(),
        );
        for level in [body.level, body.level + 4] {
            w.f64(
                &format!("valley depth at {steepness} and level {level}"),
                valley_depth(steepness, body.radius, level).value(),
            );
        }
    }
    for discharge in [0.01, 1.0, 100.0, 1e4] {
        w.f64(
            &format!("bankfull width at {discharge} m3/s"),
            bankfull_width(discharge, body.gravity).value(),
        );
        w.f64(
            &format!("bankfull depth at {discharge} m3/s"),
            bankfull_depth(discharge, body.gravity).value(),
        );
    }
}

/// The small craters: their octave table, the craters drawn in a cube corner's, a face edge's and
/// a random site's cells of the first and the last octave, and their height at the corner and edge
/// sites at three levels that evaluate them.
fn write_small_craters(w: &mut GoldenWriter, synth: &Synthesiser<'_, CoarseField>, sites: &[Site]) {
    let craters = synth.craters();
    let octaves = craters.octaves().octaves();
    w.line("small craters");
    w.line(&format!("octaves = {}", octaves.len()));
    for o in octaves {
        w.line(&format!(
            "octave {} = level {} lower {} smallest {} production {} saturation {} drawn {} kept {} window {} variance {}",
            o.index(),
            o.level(),
            hex(o.diameters().lower().value()),
            hex(o.smallest().value()),
            hex(o.production().value()),
            hex(o.saturation().value()),
            hex(o.drawn_density().value()),
            hex(o.density().value()),
            hex(o.window().value()),
            hex(o.variance().value()),
        ));
    }
    let mut ends: Vec<&CraterOctave> = Vec::new();
    for o in [
        octaves.first(),
        octaves.get(octaves.len() / 2),
        octaves.last(),
    ]
    .into_iter()
    .flatten()
    {
        if !ends.iter().any(|e| e.index() == o.index()) {
            ends.push(o);
        }
    }
    let mut cache = CraterCache::new();
    for o in ends {
        let octave = o.index();
        let fixed = [&sites[0], &sites[8], &sites[SITES - 1]]
            .map(|site| PatchKey::containing(o.level(), site.dir).expect("an octave's level"));
        let holding = first_holding(craters, o, fixed[0], &mut cache);
        w.line(&format!(
            "octave {octave} first cell holding craters from the corner's = {}",
            holding.map_or_else(|| "none".to_owned(), key)
        ));
        let mut cells: Vec<PatchKey> = Vec::new();
        for cell in fixed.into_iter().chain(holding) {
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
        for cell in cells {
            write_drawn(w, craters, octave, cell, &mut cache);
        }
        if let Some(cell) = holding {
            let crater = craters.craters_in(octave, cell, &mut cache)[0];
            write_crater_heights(w, synth, &crater, &mut cache);
        }
    }
    let evaluating: Vec<u8> = (0..=MAX_LEVEL)
        .filter(|&n| synth.level_set(band(n)).craters_through().is_some())
        .collect();
    let picked = [
        evaluating[0],
        evaluating[evaluating.len() / 2],
        evaluating[evaluating.len() - 1],
    ];
    for (n, site) in sites.iter().enumerate().take(16) {
        for level in picked {
            let through = synth
                .level_set(band(level))
                .craters_through()
                .expect("a level that evaluates craters");
            let s = craters.at(site.dir, band(level), through, &mut cache);
            sample(
                w,
                &format!("site {n} at level {level} through {through}"),
                s,
            );
        }
    }
}

/// The craters `craters` draws in `cell` of octave `octave`: their count and digest, and the first
/// three's centres, diameters and ages.
fn write_drawn(
    w: &mut GoldenWriter,
    craters: &SmallCraters,
    octave: u8,
    cell: PatchKey,
    cache: &mut CraterCache,
) {
    let drawn = craters.craters_in(octave, cell, cache);
    let values: Vec<f64> = drawn
        .iter()
        .flat_map(|c| {
            let [x, y, z] = c.centre();
            [
                x,
                y,
                z,
                c.diameter().value(),
                c.age().value(),
                c.diffusion().value(),
            ]
        })
        .collect();
    let label = format!("octave {octave} cell {}", key(cell));
    w.line(&format!(
        "{label} = count {} digest 0x{:016x}",
        drawn.len(),
        f64_digest(&values)
    ));
    for (i, c) in drawn.iter().take(3).enumerate() {
        w.line(&format!(
            "{label} crater {i} = centre {} diameter {} age {}",
            hex3(c.centre()),
            hex(c.diameter().value()),
            hex(c.age().value()),
        ));
    }
}

/// The small craters' height at `crater`'s centre and on its rim, at the shallowest level that
/// evaluates its octave and at the complete level, once where they are the same.
fn write_crater_heights(
    w: &mut GoldenWriter,
    synth: &Synthesiser<'_, CoarseField>,
    crater: &SmallCrater,
    cache: &mut CraterCache,
) {
    let octave = crater.octave();
    let radius_m = synth.field().header().radius().value();
    let shallowest = (0..=MAX_LEVEL)
        .find(|&n| {
            synth
                .level_set(band(n))
                .craters_through()
                .is_some_and(|t| t >= octave)
        })
        .expect("the complete level evaluates every octave");
    let rim = geodesic(
        crater.centre(),
        0.5 * crater.diameter().value() / radius_m,
        azimuth(octave.into()),
    );
    let complete = complete_level(synth);
    let levels = if shallowest == complete {
        vec![complete]
    } else {
        vec![shallowest, complete]
    };
    for (what, dir) in [("centre", crater.centre()), ("rim", rim)] {
        for &level in &levels {
            let through = synth
                .level_set(band(level))
                .craters_through()
                .expect("a level that evaluates the octave");
            let s = synth.craters().at(dir, band(level), through, cache);
            sample(
                w,
                &format!("octave {octave} crater's {what} at level {level} through {through}"),
                s,
            );
        }
    }
}

/// The cells the search for one holding craters walks, at most.
const CRATER_SEARCH_CELLS: u32 = 4096;

/// The first of `octave`'s cells, from `start` along its rows of its face (i rising, then j, each
/// wrapping), that holds a crater, within [`CRATER_SEARCH_CELLS`].
fn first_holding(
    craters: &SmallCraters,
    octave: &CraterOctave,
    start: PatchKey,
    cache: &mut CraterCache,
) -> Option<PatchKey> {
    let side = 1_u64 << start.level();
    let cells = (side * side).min(u64::from(CRATER_SEARCH_CELLS));
    (0..cells)
        .map(|k| {
            let along = u64::from(start.i()) + k;
            let i = u32::try_from(along % side).expect("a cell index");
            let j =
                u32::try_from((u64::from(start.j()) + along / side) % side).expect("a cell index");
            PatchKey::new(start.face(), start.level(), i, j).expect("a cell of the level")
        })
        .find(|&cell| !craters.craters_in(octave.index(), cell, cache).is_empty())
}

/// The coarse craters' rims: the coarse band, and about four craters their kept relief and the
/// rims' height at L − 4, L + 2 and the finest level, at 0, 0.97, 1.0 and 1.04 rim radii and just
/// inside the reach; then the same at face-edge vertices ([`write_rims_on_face_edges`]).
fn write_rims(w: &mut GoldenWriter, synth: &Synthesiser<'_, CoarseField>) {
    let field = synth.field();
    let header = field.header();
    let figure = header.figure();
    let rims = synth.rims();
    let level_l = header.level().get();
    let levels = [
        level_l - 4,
        level_l + 2,
        finest_level(figure.equatorial_radius_m),
    ];
    let radius_m = header.radius().value();
    w.line("coarse rims");
    w.f64(
        "coarse band half width",
        coarse_band_half_width(&figure, header.level()).value(),
    );
    let query = |w: &mut GoldenWriter, label: &str, dir: [f64; 3]| {
        for level in levels {
            let s = rims.at(field, dir, band(level)).expect("a whole field");
            sample(w, &format!("{label} at level {level}"), s);
        }
    };
    for (n, &index) in spread_craters(field, 4).iter().enumerate() {
        let crater = &field.craters()[index];
        w.line(&format!("crater {index}, {} of four", n + 1));
        w.line(&format!("centre = {}", hex3(crater.centre)));
        w.f64("diameter", crater.diameter.value());
        w.line(&format!("degradation = {}", crater.degradation));
        w.f64("kept relief", kept_relief(crater));
        let inside = inside_reach(field, crater);
        for (k, x) in [0.0, RIM_RADII[0], RIM_RADII[1], RIM_RADII[2], inside]
            .into_iter()
            .enumerate()
        {
            let dir = near(crater, x, azimuth(5 * n + k), radius_m);
            w.line(&format!("point {k} = dir {}", hex3(dir)));
            query(w, &format!("point {k}"), dir);
        }
    }
    write_rims_on_face_edges(w, synth, &query);
}

/// The face-edge vertices the rims are pinned at, at most, a crater.
const EDGE_VERTICES: usize = 3;

/// The craters whose rims are pinned at face-edge vertices, at most.
const EDGE_CRATERS: usize = 4;

/// The rims at the vertices of the face edges the listed craters' reaches cross, as the continuity
/// test samples them (every eighth vertex of each reached cell's face-edge side): for each of the
/// first [`EDGE_CRATERS`] craters, in list order, with any, the first [`EDGE_VERTICES`] whose rim
/// height at the finest level exceeds a metre.
fn write_rims_on_face_edges(
    w: &mut GoldenWriter,
    synth: &Synthesiser<'_, CoarseField>,
    query: &dyn Fn(&mut GoldenWriter, &str, [f64; 3]),
) {
    let field = synth.field();
    let level_l = field.header().level().get();
    let last = (1_u32 << level_l) - 1;
    let finest = band(finest_level(field.header().figure().equatorial_radius_m));
    w.line("coarse rims on face edges");
    let mut craters = 0;
    for (index, crater) in field.craters().iter().enumerate() {
        if craters == EDGE_CRATERS {
            break;
        }
        let mut vertices = Vec::new();
        let edge_cells = crater
            .reach
            .cells()
            .map(|i| cell_at_index(level_l, i).expect("a reach holds cells of the level"))
            .filter(|&c| on_face_edge(c));
        'cells: for cell in edge_cells {
            for k in (0..=64_u8).step_by(8) {
                let (x, y) = if cell.i() == 0 {
                    (0, k)
                } else if cell.i() == last {
                    (64, k)
                } else if cell.j() == 0 {
                    (k, 0)
                } else {
                    (k, 64)
                };
                let dir = cell.vertex_dir(x, y);
                let h = synth
                    .rims()
                    .at(field, dir, finest)
                    .expect("a whole field")
                    .height_m;
                if h.abs() > 1.0 {
                    vertices.push((cell, x, y, dir));
                    if vertices.len() == EDGE_VERTICES {
                        break 'cells;
                    }
                }
            }
        }
        if vertices.is_empty() {
            continue;
        }
        craters += 1;
        for (cell, x, y, dir) in vertices {
            let label = format!("crater {index} vertex ({x}, {y}) of cell {}", key(cell));
            query(w, &label, dir);
        }
    }
    w.line(&format!("craters with rims on face edges = {craters}"));
}

/// A closed form at level 2 that [`interp::interpolate`] reads: each cell's index scattered over
/// whole multiples of 12.5.
struct Scattered;

impl CellValues for Scattered {
    fn level(&self) -> u8 {
        2
    }

    fn value(&self, cell: PatchKey) -> Option<f64> {
        Some(f64::from(cell_index(cell) * 37 % 101) * 12.5 - 600.0)
    }
}

/// The text of the common golden.
fn common() -> String {
    let mut w = writer();
    w.line("R05's figures");
    w.f64("band limit", BAND_LIMIT_M);
    w.f64("finest spacing", FINEST_SPACING_M);
    w.line("nyquist degrees");
    for n in 0..=MAX_LEVEL {
        w.line(&format!("L{n} = {}", nyquist_degree(n)));
    }
    w.line(&format!(
        "relief octave offsets of the detail seed 0x{:016x}",
        SEED.get()
    ));
    for (m, offset) in octave_offsets(SEED).into_iter().enumerate() {
        w.line(&format!("octave {m} = {}", hex3(offset)));
    }
    w.line("interpolate at level 2 over a closed form");
    let mut dirs: Vec<[f64; 3]> = Vec::new();
    let mut corners = Vec::new();
    corner_sites(&mut corners);
    dirs.extend(corners.iter().map(|s| s.dir));
    for (k, face) in Face::ALL.into_iter().enumerate().take(4) {
        let cell = PatchKey::new(face, 2, 3, u32::try_from(k).expect("four")).expect("a cell");
        dirs.push(cell.vertex_dir(64, 21));
    }
    let mut rng = Lcg::new(0x696e_7465_7270);
    dirs.extend(random_points_in(
        PatchKey::new(Face::NegY, 2, 1, 2).expect("a cell"),
        4,
        &mut rng,
    ));
    for (k, dir) in dirs.into_iter().enumerate() {
        let v = interp::interpolate(&Scattered, dir).expect("every cell is held");
        w.line(&format!(
            "point {k} = dir {} value {} gradient {}",
            hex3(dir),
            hex(v.value),
            hex3(v.gradient)
        ));
    }
    w.finish()
}

/// The text of the levels golden.
fn levels() -> String {
    let mut w = writer();
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let bounds = LevelBounds::new(field.header());
        assert_eq!(&bounds, Synthesiser::new(field, SEED).bounds(), "{kind:?}");
        w.line(&format!("world {}", stem(kind)));
        for n in 0..=MAX_LEVEL {
            let set = bounds.set(band(n));
            let (low, high) = bounds.contribution_range_m(band(n));
            w.line(&format!(
                "L{n} = shortest {} relief {} channels {} craters {} bound {} range {} {}",
                hex(set.shortest_wavelength().value()),
                deepest(set.relief_through()),
                deepest(set.channels_through()),
                deepest(set.craters_through()),
                hex(bounds.bound_m(band(n))),
                hex(low),
                hex(high),
            ));
        }
    }
    w.finish()
}

/// The cells each world's closed forms are pinned at, with what each is: a cube corner's cell
/// and a face edge's, and a cell under a coarse rim, one with channels and one on a belt where the
/// world has them.
fn closed_form_cells(kind: SyntheticWorld) -> Vec<(&'static str, PatchKey)> {
    let field = world(kind);
    let level = field.header().level().get();
    let last = (1_u32 << level) - 1;
    let mut cells = vec![
        (
            "a cube corner's cell",
            PatchKey::new(Face::PosX, level, 0, 0).expect("a cube corner's cell"),
        ),
        (
            "a face edge's cell",
            PatchKey::new(Face::NegY, level, last, last / 3).expect("a face edge's cell"),
        ),
    ];
    cells.extend(
        rim_cells(field, 1)
            .into_iter()
            .map(|c| ("under a coarse rim", c)),
    );
    cells.extend(
        channelled_cells(field, 1)
            .into_iter()
            .map(|c| ("with channels", c)),
    );
    cells.extend(belt_cells(field, 1).into_iter().map(|c| ("on a belt", c)));
    cells
}

/// The text of the closed forms' golden.
fn closed_forms() -> String {
    let mut w = writer();
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        for (what, cell) in closed_form_cells(kind) {
            w.line(&format!("world {} cell {} {what}", stem(kind), key(cell)));
            for finer_than in [1e4, 1e3, 100.0, 10.0, 2.0] {
                let v = synth
                    .unresolved_variance(cell, Metres::new(finer_than))
                    .expect("a whole field");
                w.line(&format!(
                    "variance finer than {finer_than} m = relief {} channels {} craters {} rims {} total {}",
                    hex(v.relief().value()),
                    hex(v.channels().value()),
                    hex(v.craters().value()),
                    hex(v.rims().value()),
                    hex(v.total().value()),
                ));
            }
            for finer_than in [1e3, 10.0] {
                for lag in [1.0, 2.0, 5.0, 10.0, 100.0, 1e3] {
                    write_structure_function(&mut w, &synth, cell, finer_than, lag);
                }
            }
            for resolution in [100.0, 10.0] {
                let rms = synth
                    .unresolved_rms(cell, Metres::new(resolution))
                    .expect("a whole field");
                w.f64(&format!("unresolved rms at {resolution} m"), rms.value());
            }
            for n in 0..=MAX_LEVEL {
                write_omitted(&mut w, &synth, cell, band(n));
            }
        }
    }
    w.finish()
}

/// What band level `level` leaves out of `cell`'s ground, the level-keyed forms (R10.T5): the
/// variance by contribution and whole ([`Synthesiser::omitted_variance`]), and the structure
/// function ([`Synthesiser::omitted_structure_function`]) at lags of 2 m and 5 m, R10.T5's slope
/// baselines, and of the level's shortest wavelength, which its grazing test reads.
fn write_omitted(
    w: &mut GoldenWriter,
    synth: &Synthesiser<'_, CoarseField>,
    cell: PatchKey,
    level: BandLevel,
) {
    let n = level.get();
    let v = synth.omitted_variance(cell, level).expect("a whole field");
    w.line(&format!(
        "omitted at L{n} = relief {} channels {} craters {} rims {} total {}",
        hex(v.relief().value()),
        hex(v.channels().value()),
        hex(v.craters().value()),
        hex(v.rims().value()),
        hex(v.total().value()),
    ));
    let at = |lag: Metres| {
        hex(synth
            .omitted_structure_function(cell, level, lag)
            .expect("a whole field")
            .value())
    };
    let shortest = synth.level_set(level).shortest_wavelength();
    w.line(&format!(
        "omitted structure function at L{n} at lags of 2 m, 5 m and the shortest wavelength = {} {} {}",
        at(Metres::new(2.0)),
        at(Metres::new(5.0)),
        at(shortest),
    ));
}

/// The structure function of `cell`'s ground finer than `finer_than` metres at a lag of `lag`
/// metres, by contribution and whole.
fn write_structure_function(
    w: &mut GoldenWriter,
    synth: &Synthesiser<'_, CoarseField>,
    cell: PatchKey,
    finer_than: f64,
    lag: f64,
) {
    let field = synth.field();
    let (finer, at) = (Metres::new(finer_than), Metres::new(lag));
    let whole = synth
        .structure_function(cell, finer, at)
        .expect("a whole field");
    w.line(&format!(
        "structure function finer than {finer_than} m at a lag of {lag} m = relief {} channels {} craters {} rims {} whole {}",
        hex(relief_structure_function(field, cell, finer, at).expect("a whole field").value()),
        hex(channel_structure_function(field, cell, finer, at).expect("a whole field").value()),
        hex(synth.craters().octaves().structure_function(finer, at).value()),
        hex(rims_structure_function(field, cell, finer, at).expect("a whole field").value()),
        hex(whole.value()),
    ));
}

/// The text of the bakes' golden.
fn bakes() -> String {
    let mut w = writer();
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let level = field.header().level().get();
        let mut rng = Lcg::new(0x6261_6b65_0000 + ordinal(kind));
        let cell = channelled_cells(field, 1)
            .into_iter()
            .chain(rim_cells(field, 1))
            .next()
            .unwrap_or_else(|| random_cells(level, 4, &mut rng)[3]);
        let patch = patch_in(cell, level + 3);
        let bake = synth
            .bake_patch(&mut SynthCache::new(), patch)
            .expect("a whole field");
        w.line(&format!("world {} patch {}", stem(kind), key(patch)));
        let samples: Vec<f64> = bake
            .samples()
            .iter()
            .flat_map(|s| [s.height_m, s.gradient[0], s.gradient[1], s.gradient[2]])
            .collect();
        w.u64_hex("samples digest", f64_digest(&samples));
        w.u64_hex("parents digest", f64_digest(bake.parent_heights_m()));
        for (x, y) in [(0, 0), (64, 0), (0, 64), (64, 64), (32, 32), (17, 45)] {
            sample(&mut w, &format!("vertex ({x}, {y})"), bake.sample(x, y));
        }
        for (x, y) in [(0, 0), (32, 32), (64, 64), (18, 46)] {
            let h = bake.parent_height_m(x, y).expect("an even vertex");
            w.f64(&format!("parent ({x}, {y})"), h);
        }
    }
    w.finish()
}

/// A world's two goldens, each a test of its own, by the literal names the browser target embeds.
macro_rules! world_goldens {
    ($($world:ident: $points:ident $points_name:literal, $parts:ident $parts_name:literal;)*) => {$(
        #[test]
        fn $points() {
            golden!($points_name, &points(SyntheticWorld::$world));
        }

        #[test]
        fn $parts() {
            golden!($parts_name, &parts(SyntheticWorld::$world));
        }
    )*};
}

world_goldens! {
    EarthLike:
        earth_like_heights_match_their_golden "height/earth_like_points",
        earth_like_parts_match_their_golden "height/earth_like_parts";
    MarsLike:
        mars_like_heights_match_their_golden "height/mars_like_points",
        mars_like_parts_match_their_golden "height/mars_like_parts";
    MoonLike:
        moon_like_heights_match_their_golden "height/moon_like_points",
        moon_like_parts_match_their_golden "height/moon_like_parts";
    CeresLike:
        ceres_like_heights_match_their_golden "height/ceres_like_points",
        ceres_like_parts_match_their_golden "height/ceres_like_parts";
    Flat:
        flat_heights_match_their_golden "height/flat_points",
        flat_parts_match_their_golden "height/flat_parts";
    OneCrater:
        one_crater_heights_match_their_golden "height/one_crater_points",
        one_crater_parts_match_their_golden "height/one_crater_parts";
}

#[test]
fn the_common_figures_match_their_golden() {
    golden!("height/common", &common());
}

#[test]
fn every_world_s_level_sets_and_bounds_match_their_golden() {
    golden!("height/levels", &levels());
}

#[test]
fn every_world_s_closed_forms_match_their_golden() {
    golden!("height/closed_forms", &closed_forms());
}

#[test]
fn every_world_s_baked_patch_matches_its_golden() {
    golden!("height/bakes", &bakes());
}

/// Tests that read files, which the browser target cannot.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    /// The height goldens: two of each of the six worlds and four of all of them.
    const HEIGHT_GOLDENS: usize = 16;

    /// Every golden file under `tests/golden/height/` carries `GENERATOR_VERSION`, not R05's
    /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`.
    #[test]
    fn every_height_golden_carries_the_generator_version() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("height");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the height golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert_eq!(
            seen,
            HEIGHT_GOLDENS,
            "the height goldens in {}",
            dir.display()
        );
    }
}
