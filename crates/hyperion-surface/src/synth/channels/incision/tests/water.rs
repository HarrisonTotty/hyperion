//! The incision by water (plan R09, T6.f; decision-r09-channel-term §4, tests P1–P3) on the
//! synthetic Earth-like world: a cell under water carries none at any level, the incision stays
//! continuous onto the water, keeps each level's mean over every dry parent cell and its analytic
//! gradient, every trunk's valley is held to its cell's height above its base level, its groove
//! and its closed form read one depth, and the synthesis's floor cuts no dry point below the sea,
//! keeping the level bound. The reference worlds' census is the sim's `channel_water_worlds` test.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::synth::{BandLevel, Synthesiser};

/// About `count` of `list`, spread through its order.
fn spread(list: Vec<PatchKey>, count: usize) -> Vec<PatchKey> {
    let step = (list.len() / count).max(1);
    list.into_iter().step_by(step).take(count).collect()
}

/// Every coarse cell of `field` that `keep` keeps, in cell-index order.
fn cells_where(
    field: &CoarseField,
    mut keep: impl FnMut(PatchKey, &SynthesisCell) -> bool,
) -> Vec<PatchKey> {
    let level = field.header().level();
    (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .filter(|&c| keep(c, field.cell(c).unwrap()))
        .collect()
}

/// Whether `cell` is under water in `field`.
fn wet(field: &CoarseField, cell: PatchKey) -> bool {
    field.cell(cell).unwrap().is_under_water()
}

/// About `count` of `field`'s cells under water that receive a dry cell's trunk: river mouths.
fn mouths(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let mut all: Vec<PatchKey> = cells_where(field, |_, cell| {
        !cell.is_under_water() && cell.catchment_runoff != LogRunoff::NONE
    })
    .into_iter()
    .filter_map(|c| {
        let receiver = c.edge_neighbour(field.cell(c).unwrap().flow.edge()?);
        wet(field, receiver).then_some(receiver)
    })
    .collect();
    all.sort_unstable_by_key(|c| c.to_u64());
    all.dedup();
    spread(all, count)
}

/// About `count` of `field`'s cells under water.
fn under_water(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    spread(cells_where(field, |_, cell| cell.is_under_water()), count)
}

/// Whether the dry `cell` shares an edge or a corner with a cell under water.
fn is_by_water(field: &CoarseField, cell: PatchKey) -> bool {
    Edge::ALL
        .into_iter()
        .map(|e| cell.edge_neighbour(e))
        .chain(cell.corner_neighbours().into_iter().flatten())
        .any(|n| wet(field, n))
}

/// About `count` of `field`'s dry cells with a trunk that share an edge or a corner with a cell
/// under water, the first of them at a face edge if any is.
fn by_water(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let last = (1_u32 << field.header().level().get()) - 1;
    let on_face_edge = |c: PatchKey| c.i() == 0 || c.j() == 0 || c.i() == last || c.j() == last;
    let mut all = cells_where(field, |c, cell| {
        !cell.is_under_water()
            && cell.flow.edge().is_some()
            && cell.catchment_runoff != LogRunoff::NONE
            && is_by_water(field, c)
    });
    all.sort_by_key(|&c| (!on_face_edge(c), c.to_u64()));
    let mut out: Vec<PatchKey> = all.iter().copied().take(1).collect();
    let mut rest: Vec<PatchKey> = all.into_iter().skip(1).collect();
    rest.sort_unstable_by_key(|c| c.to_u64());
    out.extend(spread(rest, count - 1));
    out
}

/// P1: a cell under water carries no incision, height and gradient exactly zero, at any level, so
/// no wet point is turned to land by the channels; and the synthesis's closed forms give it none.
#[test]
fn a_cell_under_water_carries_no_incision_at_any_level() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut cache = SynthCache::new();
    let mouths = mouths(field, 16);
    assert!(mouths.len() >= 12, "only {} river mouths", mouths.len());
    let mut points = 0;
    for c in mouths.iter().copied().chain(under_water(field, 16)) {
        for dir in points_in(c, 5) {
            for through in [first, first + 1, first + 2, first + 4, MAX_LEVEL] {
                let sample = channels
                    .incision_at(field, dir, through, &mut cache)
                    .unwrap();
                assert_eq!(
                    sample_bits(sample),
                    [0; 4],
                    "{c:?} at {dir:?} through {through}"
                );
            }
            points += 1;
        }
    }
    assert!(points >= 800, "{points}");
    for c in mouths {
        assert_eq!(synthesised_terms(field, c).unwrap().1, vec![]);
        let wide = Metres::new(1e7);
        let v = unresolved_channel_variance(field, c, wide).unwrap();
        assert_eq!(bits(v.value()), bits(0.0));
        let d = channel_structure_function(field, c, wide, Metres::new(1e5)).unwrap();
        assert_eq!(bits(d.value()), bits(0.0));
    }
}

/// Part (b): every dry trunk's valley is the cell's held to its height above its base level, its
/// receiver's water or the sea, and the groove and the synthesis's closed form read the one depth
/// G; where the hold does not bind, the closed form is the coarse pass's budget's to the bit.
#[test]
fn no_trunk_s_valley_is_cut_below_its_base_level() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let body = channels.body();
    let r = body.radius.value();
    let sea = header.sea_level();
    let mut network = channels.network(field);
    let trunks = spread(
        cells_where(field, |_, cell| {
            !cell.is_under_water() && cell.flow.edge().is_some()
        }),
        1_500,
    );
    let (mut checked, mut held, mut into_water, mut unheld_equal) = (0, 0, 0, 0);
    for c in trunks {
        let record = field.cell(c).unwrap();
        let Some(trunk) = network.segment(c).unwrap() else {
            continue;
        };
        let Outlet::Cell { receiver, .. } = trunk.outlet() else {
            panic!("a trunk drains to a cell");
        };
        let receiver = field.cell(receiver).unwrap();
        let room = record.water_surface().value() - base_level_m(receiver, sea);
        let from_the_cell = tributary_steepness_of(record)
            .map_or(0.0, |k| valley_depth(k, body.radius, first).value());
        let valley = trunk.below_top(0.5).value();
        assert_eq!(
            bits(valley),
            bits(num::min(from_the_cell, num::max(room, 0.0))),
            "{c:?}"
        );
        assert!(
            valley <= num::max(room, 0.0),
            "{c:?}: {valley} m in {room} m"
        );
        let g = trunk_depth_m(
            valley,
            trunk.bankfull_depth().value(),
            groove_cap_m(r, first),
        );
        let (_, terms) = synthesised_terms(field, c).unwrap();
        match Groove::of(&trunk, r) {
            Some(groove) => {
                assert_eq!(groove.depth, Depth::Constant(g), "{c:?}");
                let wall = VALLEY_WALL_SHARE * mean_cell_width(r, first);
                let expected = level_term(
                    first,
                    MEAN_TRUNK_LENGTH / mean_cell_width(r, first),
                    g,
                    0.5 * trunk.bankfull_width().value(),
                    wall,
                );
                assert_eq!(terms.first(), Some(&expected), "{c:?}");
            }
            None => assert!(g <= 0.0, "{c:?}"),
        }
        if valley < from_the_cell {
            held += 1;
        } else {
            let wide = Metres::new(1e7);
            assert_eq!(
                unresolved_channel_variance(field, c, wide),
                Some(channel_variance(record, body, wide)),
                "{c:?}"
            );
            unheld_equal += 1;
        }
        into_water += usize::from(receiver.is_under_water());
        checked += 1;
    }
    assert!(checked > 1_000, "{checked}");
    assert!(
        held > 50 && into_water > 20 && unheld_equal > 100,
        "{held}, {into_water}, {unheld_equal}"
    );
}

/// P3: across every edge of dry cells by water, the shores among them, and of their children to
/// L + 3, the incision steps by less than a centimetre, and it is zero on the water's side.
#[test]
fn the_incision_is_continuous_onto_the_water() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut cache = SynthCache::new();
    let (mut worst, mut pairs, mut onto_water) = (0.0_f64, 0, 0);
    for c in by_water(field, 12) {
        let eps = 1e-7 / f64::from(1_u32 << 8);
        for k in 0..=8_u32 {
            let x = f64::from(k) / 8.0;
            for y in [0.137, 0.52, 0.861] {
                for (a, b) in [
                    (at(c, x - eps, y), at(c, x + eps, y)),
                    (at(c, y, x - eps), at(c, y, x + eps)),
                ] {
                    let ha = channels
                        .incision_at(field, a, first + 4, &mut cache)
                        .unwrap();
                    let hb = channels
                        .incision_at(field, b, first + 4, &mut cache)
                        .unwrap();
                    worst = worst.max((ha.height_m - hb.height_m).abs());
                    pairs += 1;
                    let wet_side = [a, b].into_iter().any(|d| {
                        let cell = PatchKey::containing(first, d).unwrap();
                        wet(field, cell)
                    });
                    onto_water += usize::from(wet_side);
                }
            }
        }
    }
    assert!(
        pairs > 500 && onto_water > 30,
        "{pairs} pairs, {onto_water} onto water"
    );
    assert!(worst < 0.01, "a step of {worst} m across an edge");
}

/// P3: each level's mean over every dry parent cell by water is zero on its grid, the fade and
/// the zeroed corners included.
#[test]
fn each_level_s_mean_is_zero_over_dry_parent_cells_by_water() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut cache = SynthCache::new();
    let mut checked = 0;
    for c in by_water(field, 6) {
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
    assert!(checked >= 30, "only {checked} parent cells checked");
}

/// P3: by water too, the cached build is the single-point query bit for bit, in any order.
#[test]
fn by_water_the_cache_and_its_order_change_no_height() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut dirs = Vec::new();
    for c in by_water(field, 3) {
        dirs.extend(points_in(c, 4));
        dirs.extend(points_in(below(c, first + 2)[5], 3));
    }
    // One cache warmed by every point before it, against a cold one per point.
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

/// The faded incision's analytic gradient is its central difference by water too.
#[test]
fn by_water_the_gradient_is_the_incision_s_central_difference() {
    let field = earth();
    let (good, total) = central_differences(field, by_water(field, 5));
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{good} of {total}"
    );
}

/// What the channels did to a group of points at one band level (P1 and P2).
#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    points: u32,
    wet: u32,
    /// Wet points the channels took above their water from at or below it.
    turned_to_land: u32,
    /// Dry points the channels took more than 10 m below the sea from at or above it.
    cut_below_sea: u32,
    /// Points on the axis of a trunk into water the channels took from at or above its receiver's
    /// surface to below it less its bankfull depth.
    axis_below_base: u32,
    /// Dry points lower than the lower of the height without the channels and the sea: the
    /// synthesis's floor (R09.T6.f).
    below_the_floor: u32,
    /// Dry points the floor holds at the sea level.
    on_the_floor: u32,
}

/// A census's point: its unit direction and, where it lies on the axis of a trunk into water, that
/// trunk's receiver's water surface and its bankfull depth, metres.
type Point = ([f64; 3], Option<(f64, f64)>);

/// The census of `dirs` at band level `band` on `field`, each with its trunk axis's base level and
/// bankfull depth where it is on one. The height without the channels, h₀, is the synthesis's
/// other contributions summed in their order.
fn census(field: &CoarseField, dirs: &[Point], band: u8) -> Census {
    let synth = Synthesiser::new(field, SEED);
    let sea = field.header().sea_level().value();
    let level = BandLevel::new(band).unwrap();
    let mut cache = SynthCache::new();
    let mut out = Census::default();
    for &(dir, axis) in dirs {
        let (_, record) = synth.cell_at(dir).unwrap();
        let h = synth.height_at(&mut cache, dir, level).unwrap().height_m;
        let parts = synth.contributions(&mut cache, dir, level).unwrap();
        let h0 = parts.base.height_m
            + parts.relief.height_m
            + parts.craters.height_m
            + parts.rims.height_m;
        out.points += 1;
        if record.is_under_water() {
            out.wet += 1;
            let surface = record.water_surface().value();
            out.turned_to_land += u32::from(h > surface && h0 <= surface);
            continue;
        }
        let floor = sea - 10.0;
        out.cut_below_sea += u32::from(h < floor && h0 >= floor);
        out.below_the_floor += u32::from(h < h0.min(sea));
        out.on_the_floor += u32::from(bits(h) == bits(sea));
        if let Some((base, bankfull)) = axis {
            out.axis_below_base += u32::from(h < base - bankfull && h0 >= base);
        }
    }
    out
}

/// The points of `cells`, `n` × `n` each, none on a trunk's axis.
fn grid_points(cells: &[PatchKey], n: u32) -> Vec<Point> {
    cells
        .iter()
        .flat_map(|&c| {
            (0..n).flat_map(move |a| {
                (0..n).map(move |b| {
                    let x = (f64::from(a) + 0.5) / f64::from(n);
                    let y = (f64::from(b) + 0.5) / f64::from(n);
                    (at(c, x, y), None)
                })
            })
        })
        .collect()
}

/// `n` points along the first chord of each trunk of `cells` that runs into water, from its key
/// point towards its crossing, with its receiver's water surface and its bankfull depth.
fn axis_points(field: &CoarseField, cells: &[PatchKey], n: u32) -> Vec<Point> {
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let mut out = Vec::new();
    for &c in cells {
        let Some(trunk) = network.segment(c).unwrap() else {
            continue;
        };
        let Outlet::Cell { receiver, .. } = trunk.outlet() else {
            panic!("a trunk drains to a cell");
        };
        if !wet(field, receiver) {
            continue;
        }
        let base = field.cell(receiver).unwrap().water_surface().value();
        let (from, to) = (trunk.from_m(), trunk.crossing_m().unwrap());
        for k in 0..n {
            let f = (f64::from(k) + 0.5) / f64::from(n);
            let p = [0, 1, 2].map(|a| from[a] + f * (to[a] - from[a]));
            out.push((unit_dir(p), Some((base, trunk.bankfull_depth().value()))));
        }
    }
    out
}

/// P1 and P2 by the synthesis at band levels 5, 7 and 19: no wet point turned to land, no dry
/// point by water cut more than 10 m below the sea from at or above it, and no point on the axis
/// of a trunk into water cut below its receiver's surface less its bankfull depth. A sample; the
/// 3,200 points a group are `measure_the_channels_by_water`'s and the sim's reference worlds'.
#[test]
fn the_channels_cut_no_dry_point_below_the_sea() {
    let field = earth();
    let coast = by_water(field, 24);
    let mut points = grid_points(&mouths(field, 8), 3);
    points.extend(grid_points(&coast, 3));
    points.extend(axis_points(field, &coast, 8));
    assert!(points.len() > 400, "{}", points.len());
    for band in [5, 7, 19] {
        let census = census(field, &points, band);
        assert!(census.wet > 50, "{census:?}");
        assert_eq!(
            (
                census.turned_to_land,
                census.cut_below_sea,
                census.axis_below_base,
                census.below_the_floor
            ),
            (0, 0, 0, 0),
            "band level {band}: {census:?}"
        );
    }
}

/// The floor keeps the level bound (decision-r09-channel-term §4(b), main's condition): by water,
/// where it binds, every band level's height lies within its bound of the finest level's, and the
/// floored height is continuous across the coastal cells' edges.
#[test]
fn by_water_the_floor_keeps_the_level_bound_and_continuity() {
    let field = earth();
    let header = field.header();
    let synth = Synthesiser::new(field, SEED);
    let finest = BandLevel::new(19).unwrap();
    let coast = by_water(field, 24);
    let points = grid_points(&coast, 4);
    let mut cache = SynthCache::new();
    let (mut checked, mut floored) = (0, 0);
    for &(dir, _) in &points {
        if wet(field, synth.cell_at(dir).unwrap().0) {
            continue;
        }
        let fine = synth.height_at(&mut cache, dir, finest).unwrap().height_m;
        floored += usize::from(bits(fine) == bits(header.sea_level().value()));
        for band in [4, 5, 6, 7, 9, 12, 15, 18] {
            let level = BandLevel::new(band).unwrap();
            let h = synth.height_at(&mut cache, dir, level).unwrap().height_m;
            let bound = synth.level_bound_m(level);
            assert!(
                (h - fine).abs() <= bound,
                "band level {band} at {dir:?}: {h} m against {fine} m, bound {bound} m"
            );
            checked += 1;
        }
    }
    assert!(
        checked > 1_000 && floored > 0,
        "{checked} checked, {floored} on the floor"
    );
    // Across the coastal cells' edges, at a fifth of a millimetre either side.
    let eps = 1e-7 / f64::from(1_u32 << 8);
    let mut worst = 0.0_f64;
    for &c in &coast {
        for k in 0..=8_u32 {
            let x = f64::from(k) / 8.0;
            for y in [0.137, 0.52, 0.861] {
                for (a, b) in [
                    (at(c, x - eps, y), at(c, x + eps, y)),
                    (at(c, y, x - eps), at(c, y, x + eps)),
                ] {
                    let ha = synth.height_at(&mut cache, a, finest).unwrap().height_m;
                    let hb = synth.height_at(&mut cache, b, finest).unwrap().height_m;
                    worst = worst.max((ha - hb).abs());
                }
            }
        }
    }
    assert!(worst < 0.01, "a step of {worst} m across an edge");
}

/// The census of the synthetic Earth by water with 3,200 points a group, and the share of the
/// coastal cells' valley variance the fade removes, for the plan's record.
#[test]
#[ignore = "a measurement for R09.T6.f's record in the plan's Risks, run by hand"]
fn measure_the_channels_by_water() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let coast = by_water(field, 200);
    let groups = [
        ("mouths", grid_points(&mouths(field, 200), 4)),
        ("wet", grid_points(&under_water(field, 200), 4)),
        ("dry by water", grid_points(&coast, 4)),
        (
            "trunk axes into water",
            axis_points(field, &by_water(field, 400), 16),
        ),
    ];
    for (label, points) in &groups {
        for band in [5, 7, 19] {
            println!("{label} band {band}: {:?}", census(field, points, band));
        }
    }
    // The fade's share: over the coastal cells' 16 × 16 grids, the variance of the raw incision
    // summed over its levels with and without the fade.
    let channels = Channels::new(header, SEED);
    let mut cache = SynthCache::new();
    let (mut faded, mut whole) = ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
    for &c in coast.iter().take(60) {
        let mut incision = channels.incision(field, &mut cache);
        let shore = incision.shore(c).unwrap();
        let (_, around) = incision.deepest(c).unwrap();
        for b in 0..16 {
            for a in 0..16 {
                let (x, y) = ((f64::from(a) + 0.5) / 16.0, (f64::from(b) + 0.5) / 16.0);
                let dir = at(c, x, y);
                let weight = area_weight(c, x, y);
                let raw = incision.raw(dir, around).unwrap();
                let (mut f_sum, mut sum) = (0.0, 0.0);
                for (level, level_raw) in (first..).zip(raw.as_slice()) {
                    let fade = shore.fade(x, y, fade_width(first, level)).0;
                    f_sum += fade * level_raw.value;
                    sum += level_raw.value;
                }
                faded = (
                    faded.0 + weight,
                    faded.1 + weight * f_sum,
                    faded.2 + weight * f_sum * f_sum,
                );
                whole = (
                    whole.0 + weight,
                    whole.1 + weight * sum,
                    whole.2 + weight * sum * sum,
                );
            }
        }
    }
    let variance = |(w, s, s2): (f64, f64, f64)| s2 / w - (s / w) * (s / w);
    println!(
        "the fade keeps {:.3} of the coastal cells' raw valley variance ({:.0} m² of {:.0} m²)",
        variance(faded) / variance(whole),
        variance(faded),
        variance(whole)
    );
    // The floor's lift of a coastal cell's mean ground at the finest level, over the same grids:
    // the delivered height less the sum with the channels.
    let synth = Synthesiser::new(field, SEED);
    let finest = BandLevel::new(19).unwrap();
    let (mut lifted, mut largest, mut sum) = (0, 0.0_f64, 0.0);
    for &c in coast.iter().take(60) {
        let (mut shift, mut weights) = (0.0, 0.0);
        for b in 0..16 {
            for a in 0..16 {
                let (x, y) = ((f64::from(a) + 0.5) / 16.0, (f64::from(b) + 0.5) / 16.0);
                let dir = at(c, x, y);
                let weight = area_weight(c, x, y);
                let height = synth.height_at(&mut cache, dir, finest).unwrap().height_m;
                let parts = synth.contributions(&mut cache, dir, finest).unwrap();
                let summed = parts.base.height_m
                    + parts.relief.height_m
                    + parts.channels.height_m
                    + parts.craters.height_m
                    + parts.rims.height_m;
                shift += weight * (height - summed);
                weights += weight;
            }
        }
        let mean = shift / weights;
        lifted += usize::from(mean > 0.0);
        largest = largest.max(mean);
        sum += mean;
    }
    println!(
        "the floor lifts {lifted} of 60 coastal cells' mean ground, by {:.2} m on average and \
         {largest:.2} m at most",
        sum / 60.0
    );
}
