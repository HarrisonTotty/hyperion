//! The incision's tests (plan R09, T6.b): each level's mean over every parent cell, the cache and
//! its order, the runoff factor and the widths from discharge, the cut, continuity and the
//! gradient, the cells read, and the closed forms against the realised incision.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_base::rng::DetailSeed;
use hyperion_testkit::float::bits;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::{ClimateCell, CoarseCrater, CoarseField, LogRunoff, cell_at_index};
use crate::synth::channels::{
    BANKFULL_RATIO, MINIMUM_SLOPE, Outlet, bankfull_depth, bankfull_width,
    channel_forming_discharge_m3_s, mean_discharge_m3_s, tributary_steepness_m0_9,
};
use crate::testing::{SyntheticWorld, mars_like_without_its_epoch, synthetic_field};

/// The detail seed the tests draw from.
const SEED: DetailSeed = DetailSeed::new(0x6368_616e_6e65_6c73);

/// Earth's standard gravity, at which the hydraulic geometry's figures are quoted.
const EARTH_G: MetresPerSecondSquared = MetresPerSecondSquared::new(9.806_65);

/// Each synthetic world, built once.
fn world(kind: SyntheticWorld) -> &'static CoarseField {
    static FIELDS: OnceLock<Vec<CoarseField>> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| {
        SyntheticWorld::ALL
            .iter()
            .map(|&k| synthetic_field(k))
            .collect()
    });
    let at = SyntheticWorld::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every world is listed");
    &fields[at]
}

fn earth() -> &'static CoarseField {
    world(SyntheticWorld::EarthLike)
}

/// The Mars-like world without its epoch's incision: R09.T6.b's steady state on a dry world of
/// uniform runoff, which these tests are of (a world dry now draws its epoch's trunks alone,
/// R09.T6.c).
fn mars() -> &'static CoarseField {
    static FIELD: OnceLock<CoarseField> = OnceLock::new();
    FIELD.get_or_init(mars_like_without_its_epoch)
}

/// The Mars-like world as it is, its valleys its epoch's (R09.T6.c).
fn mars_epoch() -> &'static CoarseField {
    world(SyntheticWorld::MarsLike)
}

/// About `count` of `field`'s cells carrying tributaries, spread through cell-index order.
fn with_tributaries(field: &impl FieldView, count: usize) -> Vec<PatchKey> {
    let level = field.header().level();
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .filter(|&c| tributary_steepness_of(field.cell(c).unwrap()).is_some())
        .collect();
    let step = (all.len() / count).max(1);
    all.into_iter().step_by(step).take(count).collect()
}

/// Up to `count` of `field`'s cells carrying tributaries that touch a face edge, the first one of
/// them touching a cube corner if any does.
fn at_face_edges(field: &impl FieldView, count: usize) -> Vec<PatchKey> {
    let level = field.header().level();
    let last = (1_u32 << level.get()) - 1;
    let on_edge = |c: PatchKey| c.i() == 0 || c.j() == 0 || c.i() == last || c.j() == last;
    let at_corner = |c: PatchKey| (c.i() == 0 || c.i() == last) && (c.j() == 0 || c.j() == last);
    let mut all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .filter(|&c| on_edge(c) && tributary_steepness_of(field.cell(c).unwrap()).is_some())
        .collect();
    all.sort_by_key(|&c| (!at_corner(c), c.to_u64()));
    let step = (all.len() / count).max(1);
    let mut out: Vec<PatchKey> = all.iter().copied().take(1).collect();
    out.extend(all.into_iter().skip(1).step_by(step).take(count - 1));
    out
}

/// A cell carrying tributaries whose own and catchment runoffs have the same code, so that its
/// tributaries' factor is 1, at least 41 codes above none.
fn uniform_runoff_cell(field: &impl FieldView) -> PatchKey {
    with_tributaries(field, 4000)
        .into_iter()
        .find(|&c| {
            let cell = field.cell(c).unwrap();
            cell.runoff == cell.catchment_runoff && cell.runoff.get() > 40
        })
        .expect("a cell with uniform runoff")
}

/// The unit direction at face coordinates (`x`, `y`) of `cell`, each in [0, 1].
fn at(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

/// The solid angle, to a constant, that a point at face coordinates (`x`, `y`) of `cell` stands
/// for on a grid even in face coordinates: the area element of S2's warp and the gnomonic face.
#[expect(
    clippy::many_single_char_names,
    reason = "the cube's (s, t) and (u, v) and the cell's (x, y), as the crate names them"
)]
fn area_weight(cell: PatchKey, x: f64, y: f64) -> f64 {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    let (u, v) = (st_to_uv(s), st_to_uv(t));
    let along = 1.0 + u * u + v * v;
    warp_rate(s) * warp_rate(t) / (along * along.sqrt())
}

/// The cells of `level` below `cell`, in (i, j) order.
fn below(cell: PatchKey, level: u8) -> Vec<PatchKey> {
    let up = level - cell.level();
    let side = 1_u32 << up;
    (0..side)
        .flat_map(|i| (0..side).map(move |j| (i, j)))
        .map(|(i, j)| {
            PatchKey::new(
                cell.face(),
                level,
                (cell.i() << up) + i,
                (cell.j() << up) + j,
            )
            .unwrap()
        })
        .collect()
}

/// A view of a field with some coarse cells' records replaced.
struct Patched<'a> {
    inner: &'a CoarseField,
    cells: BTreeMap<PatchKey, SynthesisCell>,
}

impl FieldView for Patched<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.cells.get(&cell).or_else(|| self.inner.cell(cell))
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.inner.craters_reaching(cell)
    }
}

/// A view of a field that records every coarse cell read.
struct Recording<'a> {
    inner: &'a CoarseField,
    read: RefCell<Vec<PatchKey>>,
}

impl FieldView for Recording<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.read.borrow_mut().push(cell);
        self.inner.cell(cell)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.inner.craters_reaching(cell)
    }
}

/// The bits of a sample's height and gradient.
fn sample_bits(s: HeightSample) -> [u64; 4] {
    [
        bits(s.height_m),
        bits(s.gradient[0]),
        bits(s.gradient[1]),
        bits(s.gradient[2]),
    ]
}

/// Points spread over `cell`: an `n` × `n` grid at its sub-cells' centres and the points just
/// inside its corners.
fn points_in(cell: PatchKey, n: u32) -> Vec<[f64; 3]> {
    let mut dirs: Vec<[f64; 3]> = (0..n)
        .flat_map(|b| (0..n).map(move |a| (a, b)))
        .map(|(a, b)| {
            at(
                cell,
                (f64::from(a) + 0.5) / f64::from(n),
                (f64::from(b) + 0.5) / f64::from(n),
            )
        })
        .collect();
    for (x, y) in [(1e-6, 1e-6), (1.0 - 1e-6, 1e-6), (1e-6, 1.0 - 1e-6)] {
        dirs.push(at(cell, x, y));
    }
    dirs
}

/// Level `level`'s corrected incision at `dir`, zero where the cut leaves the level out there.
fn level_value(
    channels: &Channels,
    field: &CoarseField,
    cache: &mut SynthCache,
    dir: [f64; 3],
    level: u8,
) -> f64 {
    let levels = channels.incision(field, cache).levels(dir, level).unwrap();
    levels
        .as_slice()
        .get(usize::from(level - channels.first_level()))
        .map_or(0.0, |l| l.value)
}

/// The parent cells of `level` below the coarse cell `coarse` the mean tests check: the coarse cell
/// for the first level, and up to four of its descendants at the level above for the others.
fn parents(coarse: PatchKey, level: u8) -> Vec<PatchKey> {
    if level == coarse.level() {
        return vec![coarse];
    }
    let all = below(coarse, level - 1);
    let step = (all.len() / 4).max(1);
    all.into_iter().step_by(step).take(4).collect()
}

#[test]
fn each_level_s_mean_over_every_parent_cell_is_zero() {
    // The epoch Mars draws its trunks alone, so its six cells are its first level's parents.
    for (field, least) in [(earth(), 61), (mars(), 61), (mars_epoch(), 6)] {
        let header = field.header();
        let first = header.level().get();
        let channels = Channels::new(header, SEED);
        let mut cache = SynthCache::new();
        let mut checked = 0;
        let cells = with_tributaries(field, 4)
            .into_iter()
            .chain(at_face_edges(field, 2));
        for c in cells {
            let (_, around) = channels.incision(field, &mut cache).deepest(c).unwrap();
            for level in first..=around.min(first + 3) {
                let parents = if level == first {
                    vec![c]
                } else {
                    below(c, level - 1)
                };
                for parent in parents {
                    let (mut sum, mut weights, mut largest) = (0.0, 0.0, 0.0_f64);
                    for sample in mean_grid(parent) {
                        let value = level_value(&channels, field, &mut cache, sample.dir, level);
                        sum += sample.weight * value;
                        weights += sample.weight;
                        largest = largest.max(value.abs());
                    }
                    let mean = sum / weights;
                    assert!(
                        mean.abs() < 1e-9,
                        "level {level}'s mean over {parent:?} is {mean} m (its largest {largest} m)"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= least, "only {checked} parent cells checked");
    }
}

/// Level `level`'s corrected incision's mean and RMS over `cell` on an `n` × `n` grid of its own,
/// offset from the mean grid's points.
fn mean_and_rms(
    channels: &Channels,
    field: &CoarseField,
    cache: &mut SynthCache,
    cell: PatchKey,
    level: u8,
    n: u32,
) -> (f64, f64) {
    let (mut sum, mut square, mut weights) = (0.0, 0.0, 0.0);
    for b in 0..n {
        for a in 0..n {
            let x = (f64::from(a) + 0.37) / f64::from(n);
            let y = (f64::from(b) + 0.61) / f64::from(n);
            let w = area_weight(cell, x, y);
            let value = level_value(channels, field, cache, at(cell, x, y), level);
            sum += w * value;
            square += w * value * value;
            weights += w;
        }
    }
    let mean = sum / weights;
    (mean, (square / weights - mean * mean).max(0.0).sqrt())
}

#[test]
fn the_mean_holds_on_a_finer_grid_to_its_quadrature_error() {
    // The mean grid's 8 × 8 against an independent 24 × 24 over the same parent cells, and over
    // the coarse cell, which the parents tile, for each level.
    for field in [earth(), mars()] {
        let header = field.header();
        let first = header.level().get();
        let channels = Channels::new(header, SEED);
        let mut cache = SynthCache::new();
        let (mut worst_parent, mut worst_coarse) = (0.0_f64, 0.0_f64);
        for c in with_tributaries(field, 3) {
            let (_, around) = channels.incision(field, &mut cache).deepest(c).unwrap();
            for level in first..=around.min(first + 3) {
                for parent in parents(c, level) {
                    let (mean, rms) = mean_and_rms(&channels, field, &mut cache, parent, level, 24);
                    if rms > 1.0 {
                        worst_parent = worst_parent.max(mean.abs() / rms);
                    }
                }
                let (mean, rms) = mean_and_rms(&channels, field, &mut cache, c, level, 48);
                if rms > 1.0 {
                    worst_coarse = worst_coarse.max(mean.abs() / rms);
                }
            }
        }
        assert!(
            worst_parent < 0.08 && worst_coarse < 0.04,
            "a level's mean on a finer grid is up to {worst_parent} of its RMS over a parent cell \
             and {worst_coarse} over a coarse cell"
        );
    }
}

#[test]
fn the_cached_build_equals_the_single_point_query_bit_for_bit() {
    for field in [earth(), mars(), mars_epoch()] {
        let header = field.header();
        let first = header.level().get();
        let channels = Channels::new(header, SEED);
        for c in with_tributaries(field, 2) {
            let patch = below(c, first + 3)[21];
            let dirs = points_in(patch, 9);
            let mut shared = SynthCache::new();
            for &d in &dirs {
                let warm = channels
                    .incision_at(field, d, first + 4, &mut shared)
                    .unwrap();
                let cold = channels
                    .incision_at(field, d, first + 4, &mut SynthCache::new())
                    .unwrap();
                assert_eq!(sample_bits(warm), sample_bits(cold), "at {d:?}");
            }
        }
    }
}

#[test]
fn cache_order_does_not_change_a_height() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut dirs = Vec::new();
    for c in with_tributaries(field, 3) {
        dirs.extend(points_in(below(c, first + 2)[5], 4));
        dirs.extend(points_in(c, 3));
    }
    let shared = RefCell::new(SynthCache::new());
    assert_order_independent(&dirs, |&d| {
        sample_bits(
            channels
                .incision_at(field, d, first + 4, &mut shared.borrow_mut())
                .unwrap(),
        )
    });
    assert_order_independent(&dirs, |&d| {
        sample_bits(
            channels
                .incision_at(field, d, first + 4, &mut SynthCache::new())
                .unwrap(),
        )
    });
}

#[test]
fn a_cache_starts_afresh_for_another_body_or_seed() {
    let (e, m) = (earth(), mars());
    let on_earth = Channels::new(e.header(), SEED);
    let on_mars = Channels::new(m.header(), SEED);
    let other_seed = Channels::new(e.header(), DetailSeed::new(SEED.get() ^ 1));
    let de = at(with_tributaries(e, 1)[0], 0.3, 0.6);
    let dm = at(with_tributaries(m, 1)[0], 0.3, 0.6);
    let alone = |ch: &Channels, f: &CoarseField, d| {
        sample_bits(ch.incision_at(f, d, 20, &mut SynthCache::new()).unwrap())
    };
    let mut cache = SynthCache::new();
    for _ in 0..2 {
        for (ch, f, d) in [(&on_earth, e, de), (&on_mars, m, dm), (&other_seed, e, de)] {
            assert_eq!(
                sample_bits(ch.incision_at(f, d, 20, &mut cache).unwrap()),
                alone(ch, f, d)
            );
        }
    }
}

/// The bits of every value a segment holds that the incision reads.
fn segment_bits(s: &Segment) -> Vec<u64> {
    let mut out = vec![s.cell().to_u64()];
    out.extend(s.from_m().map(bits));
    out.extend(s.to_m().map(bits));
    out.extend([
        bits(s.bed_from().value()),
        bits(s.bed_to().value()),
        bits(s.mean_discharge_m3_s()),
        bits(s.bankfull_width().value()),
        bits(s.bankfull_depth().value()),
        bits(s.below_top(0.5).value()),
    ]);
    out
}

#[test]
fn a_network_in_a_cache_is_the_network_of_its_own_memo() {
    let field = earth();
    let first = field.header().level().get();
    let channels = Channels::new(field.header(), SEED);
    let mut cache = SynthCache::new();
    for c in with_tributaries(field, 3) {
        for cell in std::iter::once(c).chain(below(c, first + 2)) {
            let own = channels.network(field).segment(cell).unwrap();
            let cached = channels
                .network_in(field, &mut cache)
                .segment(cell)
                .unwrap();
            assert_eq!(
                own.map(|s| segment_bits(&s)),
                cached.map(|s| segment_bits(&s))
            );
        }
    }
}

#[test]
fn segments_carry_their_discharge_width_depth_and_depth_below_the_top() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let g = header.craters().gravity();
    let channels = Channels::new(header, SEED);
    let mut network = channels.network(field);
    let mut tributaries = 0;
    for c in with_tributaries(field, 4) {
        let record = field.cell(c).unwrap();
        let trunk = network.segment(c).unwrap().unwrap();
        // A trunk's discharge is its catchment's runoff times its drainage.
        let area = record.drainage.area();
        let mean = mean_discharge_m3_s(record.catchment_runoff.metres_per_year(), area);
        let forming = channel_forming_discharge_m3_s(mean, area);
        assert_eq!(bits(trunk.mean_discharge_m3_s()), bits(mean));
        assert_eq!(trunk.bankfull_width(), bankfull_width(forming, g));
        assert_eq!(trunk.bankfull_depth(), bankfull_depth(forming, g));
        let valley = valley_depth(
            tributary_steepness_of(record).unwrap(),
            header.radius(),
            first,
        );
        assert_eq!(trunk.below_top(0.0), valley);
        assert_eq!(trunk.below_top(1.0), valley);
        // A tributary's is its own cell's runoff times its area where it joins, and its bed lies
        // below its parent's top by its parent's depth there.
        for cell in below(c, first + 1).into_iter().chain(below(c, first + 2)) {
            let Some(s) = network.segment(cell).unwrap() else {
                continue;
            };
            let area = s.area_at(1.0);
            let mean = mean_discharge_m3_s(record.runoff.metres_per_year(), area);
            let forming = channel_forming_discharge_m3_s(mean, area);
            assert_eq!(bits(s.mean_discharge_m3_s()), bits(mean));
            assert_eq!(s.bankfull_width(), bankfull_width(forming, g));
            assert_eq!(s.bankfull_depth(), bankfull_depth(forming, g));
            let Outlet::Join { parent, at } = s.outlet() else {
                panic!("a tributary joins");
            };
            let parent = network.segment(parent).unwrap().unwrap();
            let inherited = parent.below_top(at).value();
            let own = s.below_top(1.0).value();
            assert!(
                (own - inherited).abs() <= 1e-9 * inherited.abs().max(1.0),
                "{own} against {inherited}"
            );
            for t in [0.0, 0.3, 0.7] {
                // The top less the bed, plus the bed: the same to its rounding, a few ulps (the
                // assembly's builder change, R09.T8, found one an ulp apart where the bits were
                // compared).
                let top = s.below_top(t).value() + s.bed_at(t).value();
                let at_join = s.below_top(1.0).value() + s.bed_at(1.0).value();
                assert!(
                    (top - at_join).abs() <= 4.0 * f64::EPSILON * top.abs().max(1.0),
                    "the top is the same all along it: {top} against {at_join}"
                );
                assert!(s.below_top(t) <= s.below_top(1.0));
            }
            tributaries += 1;
        }
    }
    assert!(tributaries > 20, "{tributaries}");
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "k_s, the cell, the two segments, their slope and discharge ratios, as the plan names \
              them"
)]
fn a_dry_tributary_is_steeper_by_ten_to_the_0_45_and_narrower_by_its_discharge() {
    // The closed form, at an exact tenth.
    let k = 12.7;
    let wet = tributary_steepness_m0_9(k, 1.0, 1.0).unwrap();
    let dry = tributary_steepness_m0_9(k, 0.1, 1.0).unwrap();
    assert_eq!(bits(wet), bits(k), "equal runoffs leave k_s as it is");
    let factor = math::powf(10.0, 0.45);
    assert!((factor - 2.818_382_931_264_455).abs() < 1e-12);
    assert!((dry / wet / factor - 1.0).abs() < 1e-12, "{dry} ÷ {wet}");
    assert_eq!(tributary_steepness_m0_9(k, 0.0, 1.0), None);
    // Through the network: a cell whose own runoff is 40 codes, a factor 2^(40/12) = 10.08, below
    // its catchment's, against the same cell with the two equal.
    let field = earth();
    let first = field.header().level().get();
    let c = uniform_runoff_cell(field);
    let record = *field.cell(c).unwrap();
    let mut drier = record;
    drier.runoff = LogRunoff::new(record.runoff.get() - 40);
    let ratio = record.runoff.metres_per_year() / drier.runoff.metres_per_year();
    let wet_view = Patched {
        inner: field,
        cells: BTreeMap::from([(c, record)]),
    };
    let dry_view = Patched {
        inner: field,
        cells: BTreeMap::from([(c, drier)]),
    };
    let channels = Channels::new(field.header(), SEED);
    let mut wet_net = channels.network(&wet_view);
    let mut dry_net = channels.network(&dry_view);
    let mut compared = 0;
    for level in first + 1..=first + 2 {
        for cell in below(c, level) {
            let (Some(w), Some(d)) = (
                wet_net.segment(cell).unwrap(),
                dry_net.segment(cell).unwrap(),
            ) else {
                continue;
            };
            assert_eq!(w.to_m().map(bits), d.to_m().map(bits), "the same join");
            let steeper = d.steepness_m0_9().unwrap() / w.steepness_m0_9().unwrap();
            let expected = math::powf(ratio, 0.45);
            assert!(
                (steeper / expected - 1.0).abs() < 1e-12,
                "{steeper} against {expected}"
            );
            for t in [0.0, 0.5, 1.0] {
                if d.slope_at(t) > 10.0 * MINIMUM_SLOPE && w.slope_at(t) > 10.0 * MINIMUM_SLOPE {
                    let s = d.slope_at(t) / w.slope_at(t);
                    assert!(
                        (s / expected - 1.0).abs() < 1e-12,
                        "slope ratio {s} at t {t}"
                    );
                }
            }
            // Width ∝ Q^0.4 Q^0.0667 at the same drainage area.
            let q = d.mean_discharge_m3_s() / w.mean_discharge_m3_s();
            assert!((q * ratio - 1.0).abs() < 1e-12, "discharge ratio {q}");
            let narrower = d.bankfull_width().value() / w.bankfull_width().value();
            let by_discharge = math::powf(q, 0.4) * math::powf(q, 0.0667);
            assert!(
                (narrower / by_discharge - 1.0).abs() < 1e-12,
                "width ratio {narrower} against {by_discharge}"
            );
            assert!(narrower < 0.4);
            compared += 1;
        }
    }
    assert!(compared > 4, "only {compared} tributaries compared");
}

#[test]
fn a_cell_with_no_runoff_carries_no_tributaries() {
    let field = earth();
    let first = field.header().level().get();
    let c = with_tributaries(field, 3)[1];
    let mut dry = *field.cell(c).unwrap();
    dry.runoff = LogRunoff::NONE;
    assert_eq!(tributary_steepness_of(&dry), None);
    assert_eq!(deepest_level(&dry, ChannelBody::of(field.header())), first);
    let view = Patched {
        inner: field,
        cells: BTreeMap::from([(c, dry)]),
    };
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(&view);
    for level in first + 1..=first + 3 {
        for cell in below(c, level) {
            assert_eq!(network.segment(cell).unwrap(), None, "{cell:?}");
        }
    }
    // Its trunk still carries the catchment's water, in a channel with no valley about it.
    let trunk = network.segment(c).unwrap().expect("a trunk");
    assert_eq!(trunk.below_top(0.5), Metres::ZERO);
    assert!(trunk.bankfull_width().value() > 0.0);
    // And the wet cell has tributaries and a valley.
    let mut wet = channels.network(field);
    assert!(wet.segment(c).unwrap().unwrap().below_top(0.5).value() > 0.0);
    assert!(
        below(c, first + 1)
            .into_iter()
            .any(|cell| wet.segment(cell).unwrap().is_some())
    );
}

#[test]
fn widths_and_depths_follow_parker_and_the_drafted_bankfull_ratio() {
    // Parker et al. 2007, eqs. 10a and 12, at 0.073 m and Earth's standard gravity.
    let close = |a: f64, b: f64, tol: f64| assert!((a / b - 1.0).abs() < tol, "{a} against {b}");
    close(bankfull_width(3.0, EARTH_G).value(), 7.02, 2e-3);
    close(bankfull_width(1e3, EARTH_G).value(), 105.65, 2e-4);
    close(bankfull_width(1e5, EARTH_G).value(), 906.26, 2e-5);
    close(bankfull_depth(3.0, EARTH_G).value(), 0.3755, 2e-3);
    close(bankfull_depth(1e3, EARTH_G).value(), 3.835, 2e-4);
    close(bankfull_depth(1e5, EARTH_G).value(), 24.197, 2e-5);
    assert_eq!(bankfull_width(0.0, EARTH_G), Metres::ZERO);
    // At a Mars's gravity, g^−(1/5 + 0.0667/2) ≈ g^(−7/30) wider and g^(−1/5) deeper.
    let mars_g = MetresPerSecondSquared::new(3.71);
    let ratio_g: f64 = 9.806_65 / 3.71;
    close(
        bankfull_width(1e3, mars_g).value() / bankfull_width(1e3, EARTH_G).value(),
        math::powf(ratio_g, 0.2 + 0.5 * 0.0667),
        1e-12,
    );
    close(
        bankfull_depth(1e3, mars_g).value() / bankfull_depth(1e3, EARTH_G).value(),
        math::powf(ratio_g, 0.2),
        1e-12,
    );
    // The ratio: 55 at 5 km², held below 5.4 km² and above 2.964 × 10⁶ km².
    let ratio = |a: f64| channel_forming_discharge_m3_s(1.0, SquareMetres::new(a));
    close(
        ratio(5.4e6),
        BANKFULL_RATIO * math::powf(1.08, -0.25),
        1e-12,
    );
    assert_eq!(bits(ratio(1e4)), bits(ratio(5.4e6)));
    close(ratio(1e10), 8.22, 1e-3);
    close(ratio(1e12), 2.60, 2e-3);
    assert_eq!(bits(ratio(1e14)), bits(ratio(2.964e12)));
    // The mean discharge: Earth's mean land runoff from 10⁴ km² is 111 m³ s⁻¹.
    close(
        mean_discharge_m3_s(0.35, SquareMetres::new(1e10)),
        110.909,
        1e-4,
    );
}

#[test]
fn the_cut_ends_the_network_at_four_metre_channels_and_the_channel_heads() {
    for field in [earth(), mars()] {
        let header = field.header();
        let first = header.level().get();
        let body = ChannelBody::of(header);
        let r = body.radius.value();
        for c in with_tributaries(field, 30) {
            let cell = field.cell(c).unwrap();
            let deepest = deepest_level(cell, body);
            let runoff = cell.runoff.metres_per_year();
            let k = tributary_steepness_of(cell).unwrap();
            let heads = channel_head_area(k).value();
            let width = |level: u8| {
                channel_size(
                    runoff,
                    SquareMetres::new(mean_cell_area(r, level)),
                    body.gravity,
                )
                .1
            };
            for level in first + 1..=deepest {
                assert!(width(level) >= CUT_WIDTH.value());
                assert!(mean_cell_area(r, level) >= heads);
            }
            let next = deepest + 1;
            assert!(
                width(next) < CUT_WIDTH.value() || mean_cell_area(r, next) < heads,
                "level {next} below {c:?} is neither narrow nor past the heads"
            );
        }
    }
    // The heads: 10⁴ m² on gentle ground, and where the slope reaches 0.1 on steep.
    assert_eq!(channel_head_area(1.0), CHANNEL_HEAD_AREA);
    let at_median = channel_head_area(12.7).value();
    assert!((at_median / 4.7e4 - 1.0).abs() < 0.02, "{at_median}");
    let slope = 405.0 * math::powf(channel_head_area(405.0).value(), -0.45);
    assert!((slope / DEBRIS_FLOW_SLOPE - 1.0).abs() < 1e-12);
}

#[test]
fn a_trunk_s_valley_is_the_relief_from_its_channel_heads() {
    let radius = Metres::new(6.371e6);
    let k = 12.7;
    let heads = main_stream_length(channel_head_area(k)).value();
    let cell = head_length(radius, 8).value();
    assert_eq!(
        bits(valley_depth(k, radius, 8).value()),
        bits(rise(k, heads, cell))
    );
    // Linear in k_s while the heads stay at their least area.
    let gentle = valley_depth(2.0, radius, 8).value() / valley_depth(1.0, radius, 8).value();
    assert!((gentle - 2.0).abs() < 1e-12);
    // No valley on a body whose cells are smaller than a channel head's drainage.
    assert_eq!(valley_depth(k, Metres::new(1e3), 8), Metres::ZERO);
}

#[test]
fn the_incision_is_continuous_across_cell_edges() {
    for field in [earth(), mars(), mars_epoch()] {
        let header = field.header();
        let first = header.level().get();
        let channels = Channels::new(header, SEED);
        let mut cache = SynthCache::new();
        let mut worst = 0.0_f64;
        let mut pairs = 0;
        let cells = with_tributaries(field, 4)
            .into_iter()
            .chain(at_face_edges(field, 3));
        for c in cells {
            // Across the coarse cell's own edges, face edges among them, and the edges of its
            // children to L + 3, at a fifth of a millimetre either side.
            let eps = 1e-7 / f64::from(1_u32 << 8);
            for k in 0..=8_u32 {
                let x = f64::from(k) / 8.0;
                for y in [0.137, 0.52, 0.861] {
                    for (a, b) in [
                        (at(c, x - eps, y), at(c, x + eps, y)),
                        (at(c, y, x - eps), at(c, y, x + eps)),
                    ] {
                        let ha = channels.incision_at(field, a, first + 4, &mut cache);
                        let hb = channels.incision_at(field, b, first + 4, &mut cache);
                        let (Ok(ha), Ok(hb)) = (ha, hb) else {
                            continue;
                        };
                        worst = worst.max((ha.height_m - hb.height_m).abs());
                        pairs += 1;
                    }
                }
            }
        }
        assert!(pairs > 300);
        assert!(worst < 0.01, "a step of {worst} m across an edge");
    }
}

#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the cell, its two tangents, the point and the step, as a central difference names \
              them"
)]
fn the_gradient_is_the_incision_s_central_difference() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let figure = header.figure();
    let mut cache = SynthCache::new();
    let (mut good, mut total) = (0, 0);
    for c in with_tributaries(field, 5) {
        // Off every cell's edges to L + 4, where the corrections are C⁰.
        let offsets = [0.093, 0.218, 0.361, 0.477, 0.604, 0.729, 0.852, 0.967];
        let dirs = offsets
            .iter()
            .flat_map(|&x| offsets.iter().map(move |&y| (x, y)))
            .map(|(x, y)| at(c, x, 1.0 - y));
        for dir in dirs {
            let sample = channels
                .incision_at(field, dir, first + 4, &mut cache)
                .unwrap();
            // Two tangents at the point, and a step of 1 m along each.
            let normal = figure.normal(dir);
            let a = unit_dir(cross([0.0, 0.0, 1.0], normal));
            let b = cross(normal, a);
            let p = figure.point(dir);
            for e in [a, b] {
                let step = 1.0;
                let mut height = |s: f64| {
                    let q = [0, 1, 2].map(|k| p[k] + s * e[k]);
                    let d = unit_dir([
                        q[0] / figure.equatorial_radius_m,
                        q[1] / figure.equatorial_radius_m,
                        q[2] / figure.polar_radius_m,
                    ]);
                    channels
                        .incision_at(field, d, first + 4, &mut cache)
                        .unwrap()
                        .height_m
                };
                let numeric = (height(step) - height(-step)) / (2.0 * step);
                let analytic = dot(sample.gradient, e);
                total += 1;
                if (numeric - analytic).abs() < 1e-5 + 1e-4 * analytic.abs() {
                    good += 1;
                }
            }
        }
    }
    // A few steps cross a kink: where the deepest groove changes, or a groove's depth meets its
    // floor or its cap.
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{good} of {total}"
    );
}

/// a × b.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The king distance from `cell` of every cell within `radius` king moves, on the cube's graph.
fn king_distances(cell: PatchKey, radius: u8) -> BTreeMap<PatchKey, u8> {
    let mut out = BTreeMap::from([(cell, 0)]);
    let mut ring = vec![cell];
    for d in 1..=radius {
        let mut next = Vec::new();
        for c in ring {
            for n in neighbourhood(c).into_iter().flatten() {
                if let std::collections::btree_map::Entry::Vacant(v) = out.entry(n) {
                    v.insert(d);
                    next.push(n);
                }
            }
        }
        ring = next;
    }
    out
}

/// The farthest, in king moves from `coarse`, of the cells the incision reads for points of it
/// through L + 4.
fn farthest_read(field: &CoarseField, channels: &Channels, coarse: PatchKey) -> u8 {
    let first = channels.first_level();
    let view = Recording {
        inner: field,
        read: RefCell::new(Vec::new()),
    };
    let mut cache = SynthCache::new();
    for dir in points_in(coarse, 3) {
        channels
            .incision_at(&view, dir, first + 4, &mut cache)
            .unwrap();
    }
    let distances = king_distances(coarse, 8);
    view.read
        .take()
        .into_iter()
        .map(|cell| {
            *distances
                .get(&cell)
                .expect("a read within eight king moves")
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn the_incision_reads_four_king_moves_inside_the_margin() {
    for field in [earth(), mars(), mars_epoch()] {
        let channels = Channels::new(field.header(), SEED);
        let cells = with_tributaries(field, 3)
            .into_iter()
            .chain(at_face_edges(field, 3));
        let farthest = cells
            .map(|c| farthest_read(field, &channels, c))
            .max()
            .unwrap();
        assert_eq!(
            farthest, 4,
            "the incision reads {farthest} king moves from its cell"
        );
        assert!(farthest <= crate::field::SYNTHESIS_MARGIN_CELLS);
    }
}

#[test]
fn unresolved_variance_carries_the_channels_share() {
    let field = earth();
    let body = ChannelBody::of(field.header());
    for c in with_tributaries(field, 5) {
        let cell = field.cell(c).unwrap();
        for finer_than in [Metres::new(500.0), Metres::new(5e4)] {
            let v = crate::synth::unresolved_variance(field, c, finer_than).unwrap();
            assert_eq!(v.channels(), channel_variance(cell, body, finer_than));
            assert_eq!(
                v.total(),
                v.relief() + v.channels() + v.craters() + v.rims()
            );
        }
        // Wider bands hold more.
        let narrow = channel_variance(cell, body, Metres::new(1e3));
        let wide = channel_variance(cell, body, Metres::new(1e5));
        assert!(wide.value() > narrow.value());
    }
}

#[test]
fn the_structure_function_rises_as_its_slopes_and_saturates_at_twice_the_variance() {
    let field = earth();
    let body = ChannelBody::of(field.header());
    let wide = Metres::new(1e6);
    for c in with_tributaries(field, 5) {
        let cell = field.cell(c).unwrap();
        let d = |lag: f64| {
            channel_structure_function(field, c, wide, Metres::new(lag))
                .unwrap()
                .value()
        };
        assert_eq!(bits(d(0.0)), bits(0.0));
        let slopes: f64 = level_terms(cell, body).iter().map(|t| t.slope2).sum();
        assert!((d(0.1) / (WHOLE_VARIANCE_SHARE * slopes * 0.01) - 1.0).abs() < 1e-3);
        let variance = channel_variance(cell, body, wide).value();
        assert!((d(1e8) / (2.0 * variance) - 1.0).abs() < 1e-12);
    }
}

/// The variance of the channels' whole incision over `coarse` on an `n` × `n` grid weighted by
/// area, and the sum of its levels' own variances.
fn realised_variance(
    channels: &Channels,
    field: &CoarseField,
    cache: &mut SynthCache,
    coarse: PatchKey,
    n: u32,
) -> (f64, f64) {
    let (mut sum, mut square, mut weights) = (0.0, 0.0, 0.0);
    let mut by_level = [(0.0, 0.0); LEVELS];
    for b in 0..n {
        for a in 0..n {
            let x = (f64::from(a) + 0.5) / f64::from(n);
            let y = (f64::from(b) + 0.5) / f64::from(n);
            let w = area_weight(coarse, x, y);
            let levels = channels
                .incision(field, cache)
                .levels(at(coarse, x, y), MAX_LEVEL)
                .unwrap();
            let total: f64 = levels.as_slice().iter().map(|l| l.value).sum();
            sum += w * total;
            square += w * total * total;
            weights += w;
            for (s, l) in by_level.iter_mut().zip(levels.as_slice()) {
                s.0 += w * l.value;
                s.1 += w * l.value * l.value;
            }
        }
    }
    let mean = sum / weights;
    let levels: f64 = by_level
        .iter()
        .map(|(s, s2)| s2 / weights - (s / weights) * (s / weights))
        .sum();
    (square / weights - mean * mean, levels)
}

#[test]
fn the_closed_form_variance_is_within_a_factor_of_two_of_the_realised() {
    for field in [earth(), mars(), mars_epoch()] {
        let channels = Channels::new(field.header(), SEED);
        let body = ChannelBody::of(field.header());
        let mut cache = SynthCache::new();
        let mut ratios = Vec::new();
        for c in with_tributaries(field, 8) {
            let (realised, _) = realised_variance(&channels, field, &mut cache, c, 32);
            let closed = channel_variance(field.cell(c).unwrap(), body, Metres::new(1e7)).value();
            if closed > 1.0 {
                ratios.push(realised / closed);
            }
        }
        ratios.sort_by(f64::total_cmp);
        let median = ratios[ratios.len() / 2];
        assert!(
            (0.5..2.0).contains(&median),
            "the realised variance is {median} of the closed form's (all {ratios:?})"
        );
    }
}

#[test]
#[ignore = "a measurement for R09.T6.b's constants and the plan's Risks, run by hand"]
#[expect(
    clippy::too_many_lines,
    reason = "one by-hand measurement, printed in the order the plan's Risks record it"
)]
fn measure_the_network_and_the_incision() {
    for (name, field) in [("earth", earth()), ("mars", mars())] {
        let header = field.header();
        let body = ChannelBody::of(header);
        let first = header.level().get();
        let channels = Channels::new(header, SEED);
        let cells = with_tributaries(field, 40);
        let mut deepest = BTreeMap::new();
        for &c in &cells {
            *deepest
                .entry(deepest_level(field.cell(c).unwrap(), body))
                .or_insert(0) += 1;
        }
        println!("{name}: deepest levels {deepest:?}");
        // What the cut removes, by the closed form: the levels a 4 m and an 8 m cut leave out down
        // to the channel heads, as an RMS, and the 4 m cut's levels themselves.
        let (mut kept, mut beyond_4, mut between) = (Vec::new(), Vec::new(), Vec::new());
        for &c in &cells {
            let cell = field.cell(c).unwrap();
            let rms = |from: u8, to: u8| {
                level_terms_to(cell, body, to)
                    .iter()
                    .filter(|t| t.level > from)
                    .map(|t| t.variance)
                    .sum::<f64>()
                    .sqrt()
            };
            let at_4 = deepest_level_cut(cell, body, 4.0);
            let at_8 = deepest_level_cut(cell, body, 8.0);
            let heads = deepest_level_cut(cell, body, 0.0);
            kept.push(rms(first, at_4));
            beyond_4.push(rms(at_4, heads));
            between.push(rms(at_8, at_4));
        }
        for v in [&mut kept, &mut beyond_4, &mut between] {
            v.sort_by(f64::total_cmp);
        }
        let median = |v: &[f64]| v[v.len() / 2];
        println!(
            "  RMS kept by the 4 m cut: median {:.1} m; removed below 4 m to the heads: median \
             {:.2} m, largest {:.2} m; between the 8 m and 4 m cuts: median {:.2} m, largest {:.2} m",
            median(&kept),
            median(&beyond_4),
            beyond_4[beyond_4.len() - 1],
            median(&between),
            between[between.len() - 1]
        );
        // The realised variance against the closed form, of the whole and of the levels.
        let mut cache = SynthCache::new();
        for &c in cells.iter().take(10) {
            let (whole, levels) = realised_variance(&channels, field, &mut cache, c, 32);
            let closed = channel_variance(field.cell(c).unwrap(), body, Metres::new(1e7)).value();
            println!(
                "  {:?}: whole RMS {:.1} m, levels' {:.1} m, closed {:.1} m, whole ÷ closed {:.2}",
                (c.face().index(), c.i(), c.j()),
                whole.sqrt(),
                levels.sqrt(),
                closed.sqrt(),
                whole / closed
            );
        }
        // The mean on a finer grid, as a share of each level's RMS.
        for &c in cells.iter().take(3) {
            let (_, around) = channels.incision(field, &mut cache).deepest(c).unwrap();
            for level in first..=around.min(first + 3) {
                let mut parent_worst = 0.0_f64;
                for parent in parents(c, level) {
                    let (mean, rms) = mean_and_rms(&channels, field, &mut cache, parent, level, 24);
                    if rms > 1.0 {
                        parent_worst = parent_worst.max(mean.abs() / rms);
                    }
                }
                let (mean, rms) = mean_and_rms(&channels, field, &mut cache, c, level, 48);
                println!(
                    "  {:?} level {level}: over its parents up to {:.3} of the RMS; over the coarse \
                     cell {:.3} m of {:.1} m",
                    (c.face().index(), c.i(), c.j()),
                    parent_worst,
                    mean,
                    rms
                );
            }
        }
        let farthest = with_tributaries(field, 3)
            .into_iter()
            .chain(at_face_edges(field, 3))
            .map(|c| farthest_read(field, &channels, c))
            .max()
            .unwrap();
        println!("  farthest read: {farthest} king moves");
        let mut network = channels.network_in(field, &mut cache);
        for level in first..=first + 4 {
            let (mut heads, mut count, mut length, mut long, mut longer) = (0, 0, 0.0, 0, 0);
            let (mut width, mut depth) = (0.0, 0.0);
            let cell_width = mean_cell_width(body.radius.value(), level);
            for &c in cells.iter().take(12) {
                for cell in below(c, level) {
                    count += 1;
                    if let Some(s) = network.segment(cell).unwrap() {
                        heads += 1;
                        let far = s.length().value() / cell_width;
                        length += far;
                        long += usize::from(far > 0.75);
                        longer += usize::from(far > 1.75);
                        width += s.bankfull_width().value();
                        depth += s.bankfull_depth().value();
                    }
                }
            }
            let n = f64::from(heads.max(1));
            println!(
                "  level {level}: share {:.3}, length {:.3} cells, over 0.75 {:.3}, over 1.75 \
                 {:.4}, width {:.2} m, depth {:.3} m",
                f64::from(heads) / f64::from(count),
                length / n,
                f64::from(u32::try_from(long).unwrap()) / n,
                f64::from(u32::try_from(longer).unwrap()) / n,
                width / n,
                depth / n
            );
        }
    }
}

#[test]
fn the_bubble_s_weighted_mean_stays_above_its_floor() {
    // Every cell at the coarse levels 5 and 6, and at each level to the deepest the cells nearest a
    // face's corners, edges and centre, where the warp's rates and the gnomonic face's solid
    // angle are most uneven: the floor is what R09.T8's level bound divides by.
    let weighted_mean = |cell: PatchKey| {
        let (mut bump, mut weights) = (0.0, 0.0);
        for sample in mean_grid(cell) {
            bump += sample.weight * bubble(sample.x).0 * bubble(sample.y).0;
            weights += sample.weight;
        }
        bump / weights
    };
    let mut lowest = f64::INFINITY;
    for level in 5..=MAX_LEVEL {
        let last = (1_u32 << level) - 1;
        let cells: Vec<PatchKey> = if level <= 6 {
            (0..=last)
                .flat_map(|i| (0..=last).map(move |j| (i, j)))
                .map(|(i, j)| PatchKey::new(crate::cube::Face::PosZ, level, i, j).unwrap())
                .collect()
        } else {
            let along = [0, 1, last / 4, last / 2, last / 2 + 1, last - 1, last];
            along
                .iter()
                .flat_map(|&i| {
                    along.map(|j| PatchKey::new(crate::cube::Face::NegX, level, i, j).unwrap())
                })
                .collect()
        };
        for cell in cells {
            let mean = weighted_mean(cell);
            assert!(mean >= BUBBLE_MEAN_FLOOR, "{cell:?}: {mean}");
            lowest = lowest.min(mean);
        }
    }
    assert!(
        lowest < BUBBLE_MEAN_FLOOR + 1e-3,
        "the floor is within a per mille: {lowest}"
    );
}

#[test]
fn every_groove_is_within_its_level_s_cap() {
    for field in [earth(), mars(), mars_epoch()] {
        let channels = Channels::new(field.header(), SEED);
        let radius_m = channels.body().radius.value();
        let deepest = deepest_possible_level(radius_m, channels.first_level());
        let mut network = channels.network(field);
        for coarse in with_tributaries(field, 4) {
            assert!(deepest_level(field.cell(coarse).unwrap(), channels.body()) <= deepest);
            for level in channels.first_level()..=channels.first_level() + 3 {
                for cell in below(coarse, level) {
                    if let Some(segment) = network.segment(cell).unwrap()
                        && let Some(groove) = Groove::of(&segment, radius_m)
                    {
                        let deepest_groove = match groove.depth {
                            Depth::Constant(d) => d,
                            Depth::Tributary { floor_m, cap_m, .. } => floor_m.max(cap_m),
                        };
                        assert!(deepest_groove <= groove_cap_m(radius_m, level));
                    }
                }
            }
        }
    }
}

/// About `count` of the epoch Mars's cells whose trunk cuts a valley ([`epoch_valley`]), spread
/// through cell-index order.
fn with_epoch_valleys(field: &impl FieldView, count: usize) -> Vec<PatchKey> {
    let body = ChannelBody::of(field.header());
    let level = field.header().level();
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .filter(|&c| {
            epoch_valley(field.cell(c).unwrap(), body).is_some_and(|v| v.depth().value() > 0.0)
        })
        .collect();
    let step = (all.len() / count).max(1);
    all.into_iter().step_by(step).take(count).collect()
}

/// R09.T6.c (main's ruling, option A): on a world dry now, each coarse cell's trunk cuts its
/// epoch's valley, of depth G = min(T6.b's steady-state groove, the coded incision), its floor the
/// bankfull channel and each wall 5 G wide at most T6.b's; no segment heads a finer cell below it;
/// the cut ends at the coarse level; and the closed form's one term is that valley's.
#[test]
fn a_world_dry_now_draws_its_epoch_s_trunks_alone() {
    let field = mars_epoch();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let body = channels.body();
    let radius_m = body.radius.value();
    let mut network = channels.network(field);
    let mut capped = 0;
    let cells = with_epoch_valleys(field, 24);
    assert!(cells.len() >= 20, "only {} cells", cells.len());
    for &coarse in &cells {
        let record = field.cell(coarse).unwrap();
        let incision = record.epoch_incision.incision().unwrap().value();
        let valley = epoch_valley(record, body).unwrap();
        let g = valley.depth().value();
        // T6.b's groove on the same cell, which the epoch's incision caps.
        let segment = network.segment(coarse).unwrap().expect("a trunk");
        let k = epoch_valley_steepness(record).unwrap();
        let steady = num::min(
            num::max(
                valley_depth(k, body.radius, first).value(),
                segment.bankfull_depth().value(),
            ),
            groove_cap_m(radius_m, first),
        );
        assert_eq!(bits(g), bits(num::min(steady, incision)), "{coarse:?}");
        if incision < steady {
            capped += 1;
        }
        let wall_cap = VALLEY_WALL_SHARE * mean_cell_width(radius_m, first);
        assert_eq!(
            bits(valley.wall().value()),
            bits(num::min(WALL_RUN_PER_DEPTH * g, wall_cap))
        );
        assert_eq!(
            bits(valley.floor_width().value()),
            bits(segment.bankfull_width().value())
        );
        assert!(1.5 * g / valley.wall().value() <= 0.3 + 1e-12, "{coarse:?}");
        assert_eq!(bits(segment.below_top(0.5).value()), bits(g));
        let groove = Groove::of(&segment, radius_m).expect("a valley");
        assert_eq!(groove.depth, Depth::Constant(g));
        assert_eq!(bits(groove.wall_m), bits(valley.wall().value()));
        assert_eq!(
            bits(groove.floor_m),
            bits(0.5 * valley.floor_width().value())
        );
        assert_eq!(deepest_level(record, body), first);
        for level in first + 1..=first + 2 {
            for cell in below(coarse, level) {
                assert_eq!(network.segment(cell).unwrap(), None, "{cell:?}");
            }
        }
        let terms = level_terms(record, body);
        let expected = level_term(
            first,
            MEAN_TRUNK_LENGTH / mean_cell_width(radius_m, first),
            g,
            0.5 * valley.floor_width().value(),
            valley.wall().value(),
        );
        assert_eq!(terms, vec![expected], "{coarse:?}");
    }
    // The epoch, not the steady state, sets most of the Mars-like world's valleys.
    assert!(
        capped * 2 > cells.len(),
        "{capped} of {} capped",
        cells.len()
    );
}

/// R09.T6.c's cross-section (decision-r09-t14-mars-ceiling §5): across an epoch trunk's chord its
/// groove is G on its floor, half the bankfull width either side, falls as T6.b's smoothstep over
/// the wall W to zero at the floor's edge plus W, its analytic gradient across is the depth's
/// central difference, and no wall is steeper than 1.5 G ÷ W, at most 0.3.
#[test]
fn an_epoch_valley_is_ten_times_as_wide_as_deep() {
    let field = mars_epoch();
    let header = field.header();
    let channels = Channels::new(header, SEED);
    let radius_m = channels.body().radius.value();
    let mut network = channels.network(field);
    for coarse in with_epoch_valleys(field, 6) {
        let segment = network.segment(coarse).unwrap().unwrap();
        let groove = Groove::of(&segment, radius_m).unwrap();
        let Depth::Constant(g) = groove.depth else {
            panic!("a trunk's groove is constant along it");
        };
        let [chord, _] = groove.chords;
        // The chord's middle, and the unit direction across it along the surface.
        let middle = [0, 1, 2].map(|k| f64::midpoint(chord.from[k], chord.to[k]));
        let along = [0, 1, 2].map(|k| chord.to[k] - chord.from[k]);
        let across = {
            let c = cross(along, middle);
            let n = dot(c, c).sqrt();
            c.map(|x| x / n)
        };
        let point = |x: f64| [0, 1, 2].map(|k| middle[k] + x * across[k]);
        let (floor, wall) = (groove.floor_m, groove.wall_m);
        assert_eq!(bits(groove.at(point(0.0)).value), bits(g));
        assert_eq!(bits(groove.at(point(0.5 * floor)).value), bits(g));
        assert_eq!(groove.at(point(floor + wall * 1.001)).value, 0.0);
        let mut steepest = 0.0_f64;
        for n in 1..200 {
            let x = floor + wall * f64::from(n) / 200.0;
            let h = 1e-3 * wall;
            let slope = (groove.at(point(x + h)).value - groove.at(point(x - h)).value) / (2.0 * h);
            let analytic = dot(groove.at(point(x)).gradient, across);
            assert!(
                (slope - analytic).abs() <= 1e-6 * (1.0 + slope.abs()),
                "{coarse:?} at {x} m: {slope} against {analytic}"
            );
            steepest = steepest.max(slope.abs());
        }
        assert!(
            steepest <= 1.5 * g / wall * (1.0 + 1e-6),
            "{coarse:?}: {steepest}"
        );
        assert!(steepest <= 0.3 * (1.0 + 1e-6), "{coarse:?}: {steepest}");
    }
}

/// A cell the epoch did not cut ([`LogIncision::UNCUT`]) carries no valley, and a cell no water
/// reaches, its catchment shedding none, carries no trunk and no channels, on any world
/// (R09.T6.c; decision-r09-t14-choices, "T14.d follow-up" C).
#[test]
fn a_cell_the_epoch_did_not_cut_or_no_water_reaches_draws_nothing() {
    use crate::field::LogIncision;
    let field = mars_epoch();
    let channels = Channels::new(field.header(), SEED);
    let body = channels.body();
    let c = with_epoch_valleys(field, 3)[1];
    let mut uncut = *field.cell(c).unwrap();
    uncut.epoch_incision = LogIncision::UNCUT;
    assert_eq!(epoch_valley(&uncut, body).unwrap().depth, Metres::ZERO);
    assert_eq!(level_terms(&uncut, body), vec![]);
    let view = Patched {
        inner: field,
        cells: BTreeMap::from([(c, uncut)]),
    };
    let mut network = channels.network(&view);
    let trunk = network.segment(c).unwrap().expect("a trunk");
    assert_eq!(trunk.below_top(0.5), Metres::ZERO);
    assert_eq!(Groove::of(&trunk, body.radius.value()), None);
    for (world, cell) in [(earth(), with_tributaries(earth(), 3)[1]), (field, c)] {
        let mut dry = *world.cell(cell).unwrap();
        dry.runoff = LogRunoff::NONE;
        dry.catchment_runoff = LogRunoff::NONE;
        assert!(!super::super::has_channels(&dry));
        assert_eq!(epoch_valley(&dry, body), None);
        assert_eq!(level_terms(&dry, ChannelBody::of(world.header())), vec![]);
        let view = Patched {
            inner: world,
            cells: BTreeMap::from([(cell, dry)]),
        };
        let channels = Channels::new(world.header(), SEED);
        assert_eq!(channels.network(&view).segment(cell).unwrap(), None);
    }
}

/// R09.T6.c's closure is the drawn valleys' void: over the epoch Mars's cells with a valley, the
/// realised first level's mean incision over each coarse cell, the void the synthesis adds back to
/// the upland, summed by area, against the closed form's `MEAN_TRUNK_LENGTH` G (`w_bf` + W) ÷ w²
/// that the coarse pass's width closure takes from each cell, within a quarter; recorded: 1.19
/// as built (the Mars-like world's trunks run 1.12 coarse cells, against the closed form's
/// [`MEAN_TRUNK_LENGTH`] of 1.0, R09.T6.b's measurement, and each groove's ends add their caps), so
/// that the upland stands about a fifth of the cell's lowering above its construction.
#[test]
fn the_drawn_valleys_void_is_the_closure_s() {
    let field = mars_epoch();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let body = channels.body();
    let radius_m = body.radius.value();
    let mut cache = SynthCache::new();
    let (mut realised, mut closed) = (0.0, 0.0);
    for c in with_epoch_valleys(field, 120) {
        let valley = epoch_valley(field.cell(c).unwrap(), body).unwrap();
        let mean = channels.incision(field, &mut cache).mean(first, c).unwrap();
        let width = mean_cell_width(radius_m, first);
        realised += -mean;
        closed += MEAN_TRUNK_LENGTH * valley.void_m2() / width;
    }
    eprintln!(
        "the realised first level's void {realised:.4} m against the closure's {closed:.4} m"
    );
    assert!(
        (realised / closed - 1.0).abs() < 0.25,
        "the realised void is {} of the closure's",
        realised / closed
    );
}
