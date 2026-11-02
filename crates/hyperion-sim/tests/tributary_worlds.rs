//! The wet-now tributaries of the reference Earth and the locked Earth (plan R09, R09.T6.i): each
//! world's coarse pass ([`coarse_pass`]) and the look's census of its network, over the coarse cells
//! of the look's frames.
//!
//! - **The junction angles**, after Seybold, Rothman and Kirchner 2017 (GRL 44, 2272,
//!   doi:10.1002/2016GL072089, §2.1's measure, §2.2's means): between the orthogonal-regression
//!   lines of the two upstream branches, in the plane tangent at the junction, but each over the
//!   model's whole segment upstream of the junction, the parent's course from its head and the
//!   tributary's whole course, where their segments end at the next junction upstream. At the
//!   interior joins of every level from L + 1, the median is within 72° ± 5° (their humid mean,
//!   72 ± 1°; their medians follow nearly the same relation, Fig. 2c) and the interquartile range
//!   at least 15° (a floor: Earth's standard deviation is about 25° within a basin and 35° across
//!   the humid end-member's junctions); the standard deviation is recorded.
//! - **The join points** taking three or more inflows are at most 1.5% at every level from L + 2,
//!   pooled (research-r09-earth-look's own count of the NHD's, 0.5–1.1% pooled and 0.23–1.66% by
//!   basin, not literature); the share of tributaries joining their parent's head is recorded by
//!   level.
//! - **The heads**: over the tributaries of each cell's cut level whose join is cut deeper than
//!   10 m, the head's groove depth over the join's has a median of at most 0.6 (a design
//!   threshold, research-r09-earth-look's; T6.h's network has 0.8–1.0).
//! - **The step at the join**, the parent's groove depth there less the tributary's, is 0 at the
//!   median, their beds accordant; the share over 10 m (a design threshold), where a level's cap
//!   binds, is recorded.
//! - **The gradient**: the incision's analytic gradient is its central difference over 1 m at 97%
//!   of 640 steps below five cells carrying tributaries, through L + 4.
//! - **The cell edges**, recorded by frame: on the look's trunk-1, trunk-2 and first view at band
//!   level 12, the median ratio of the incision's mean slope in bands of 13 or 3 pixel steps (about
//!   1.3–2 km) either side of each coarse-cell edge between dry cells, the larger over the smaller,
//!   against the same between two adjacent bands inside a cell, and whether it is within 1.15
//!   (research-r09-earth-look's band metric; T6.h's network has 1.10–1.70 on the reference Earth).
//!   It holds on five of the six frames, the reference Earth's trunk-1 the exception, so it is not
//!   asserted.
//! - **The budget**: the coarse pass's `C_c` books what the synthesis draws, to 1% over every
//!   sixteenth cell, and the realised `σ_h` is plan 14's to 0.1% (R09.T6.g's (b), under the rules).
//!
//! [`measure_the_bent_network_s_constants`] is the by-hand measurement behind
//! [`MEAN_BENT_TRIBUTARY_LENGTH`], [`WET_TRUNK_SINUOSITY`], [`BENT_TRIBUTARY_VARIANCE_SHARE`] and
//! [`BENT_WHOLE_VARIANCE_SHARE`], on these two worlds and the synthetic Earth-like.
//!
//! Run with `cargo nextest run -p hyperion-sim --features testing --test tributary_worlds
//! --cargo-profile slow-test --profile slow --run-ignored only`.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the tributaries' tests use only the channels' shared helpers"
)]
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use common::channels::{point_in, quantile};

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;
use hyperion_sim::math;
use hyperion_sim::planetary::surface::{CoarseInputs, coarse_pass, reference};
use hyperion_surface::cube::{PATCH_QUADS, PatchKey, unit_dir};
use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
use hyperion_surface::synth::channels::{
    BENT_TRIBUTARY_VARIANCE_SHARE, BENT_WHOLE_VARIANCE_SHARE, ChannelBody, ChannelNetwork,
    MEAN_BENT_TRIBUTARY_LENGTH, Outlet, Segment, TRIBUTARY_SHARE, WET_TRUNK_SINUOSITY,
    deepest_level, drawn_channel_variance,
};
use hyperion_surface::synth::{SynthCache, Synthesiser};
use hyperion_surface::testing::{SyntheticWorld, synthetic_field};
use hyperion_testkit::float::bits;

/// The detail seed: `trunk_map`'s, the look's.
const DETAIL: DetailSeed = DetailSeed::new(0x7472_756e_6b6d_6170);

/// The junction angles' median's range, degrees: Seybold et al. 2017's humid mean, 72°, ± 5°.
const ANGLE_MEDIAN_DEG: (f64, f64) = (67.0, 77.0);

/// The junction angles' interquartile range's floor, degrees (the plan's, with the bend).
const ANGLE_IQR_DEG: f64 = 15.0;

/// The largest share of join points taking three or more inflows at a level from L + 2.
const MOST_MULTIPLE_JOINS: f64 = 0.015;

/// The largest median of a head's depth over its join's at the cut's level.
const MOST_HEAD_OVER_JOIN: f64 = 0.6;

/// The depth a join is cut below which its head's ratio is not counted, and the step at a join
/// counted as one, metres (a design threshold).
const CUT_DEEPER_THAN_M: f64 = 10.0;

/// A reference world, built once: `"earth"` or `"locked"`.
fn world(name: &str) -> &'static CoarseField {
    static EARTH: OnceLock<CoarseField> = OnceLock::new();
    static LOCKED: OnceLock<CoarseField> = OnceLock::new();
    let (cell, inputs): (_, fn() -> CoarseInputs) = match name {
        "earth" => (&EARTH, reference::earth_like),
        "locked" => (&LOCKED, reference::locked_earth_like),
        other => panic!("no reference world {other}"),
    };
    cell.get_or_init(|| coarse_pass(reference::SEED, &inputs()))
}

/// The distance between two points.
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [0, 1, 2].map(|k| b[k] - a[k]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The cross product.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The dot product.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The unit vector of `v`.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    v.map(|x| x / n)
}

/// A level's mean cell width on a body of mean radius `radius_m`, metres: the root of its mean
/// cell's area.
fn mean_cell_width(radius_m: f64, level: u8) -> f64 {
    radius_m * (4.0 * core::f64::consts::PI / 6.0).sqrt() / f64::from(1_u32 << level)
}

/// The mean and the standard deviation of `values`.
fn mean_and_sd(values: &[f64]) -> (f64, f64) {
    #[expect(clippy::cast_precision_loss, reason = "a count of joins, exact in f64")]
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0);
    (mean, variance.sqrt())
}

/// `values` sorted, with their count, p10, p25, median, p75 and p90 printed as `label`'s.
fn sorted_and_printed(label: &str, mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        println!("    {label}: none");
    } else {
        println!(
            "    {label} ({}): p10 {:.2}, p25 {:.2}, median {:.2}, p75 {:.2}, p90 {:.2}",
            values.len(),
            quantile(&values, 0.1),
            quantile(&values, 0.25),
            quantile(&values, 0.5),
            quantile(&values, 0.75),
            quantile(&values, 0.9)
        );
    }
    values
}

/// The look's frames' centres: of the dry cells heading a trunk, ranked by drainage and then by
/// index, the median's, the 90th percentile's and the largest's (its trunk-1, trunk-2 and trunk-3,
/// the first view about the last).
fn frame_centres(
    field: &CoarseField,
    network: &mut ChannelNetwork<'_, CoarseField>,
) -> [PatchKey; 3] {
    let level = field.header().level();
    let mut ranked = Vec::new();
    for (index, record) in (0_u32..).zip(field.synthesis()) {
        if record.is_under_water() {
            continue;
        }
        let cell = cell_at_index(level.get(), index).expect("a field's cell");
        if network
            .segment(cell)
            .expect("the field holds its cells")
            .is_some_and(|segment| segment.is_trunk())
        {
            ranked.push((record.drainage.area().value(), index, cell));
        }
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let count = ranked.len();
    [count / 2, count * 9 / 10, count - 1].map(|rank| ranked[rank].2)
}

/// The look's frames' coarse cells: about each of its three centres ([`frame_centres`]) the 5 × 5
/// cells of a 150 m frame on their face, and about the largest's the 9 × 9 of the 600 m first view;
/// in packed key order.
fn census_cells(field: &CoarseField, centres: [PatchKey; 3]) -> Vec<PatchKey> {
    let level = field.header().level();
    let side = 1_i64 << level.get();
    let mut cells = BTreeSet::new();
    for (centre, reach) in [
        (centres[0], 2),
        (centres[1], 2),
        (centres[2], 2),
        (centres[2], 4),
    ] {
        for di in -reach..=reach {
            for dj in -reach..=reach {
                let (i, j) = (i64::from(centre.i()) + di, i64::from(centre.j()) + dj);
                if (0..side).contains(&i) && (0..side).contains(&j) {
                    let key = PatchKey::new(
                        centre.face(),
                        level.get(),
                        u32::try_from(i).expect("on the face"),
                        u32::try_from(j).expect("on the face"),
                    )
                    .expect("a cell of the field");
                    cells.insert(key.to_u64());
                }
            }
        }
    }
    cells
        .into_iter()
        .map(|key| PatchKey::from_u64(key).expect("a packed key"))
        .collect()
}

/// The cells of `level` below `cell`, in (i, j) order.
fn below(cell: PatchKey, level: u8) -> Vec<PatchKey> {
    let mut out = vec![cell];
    while out[0].level() < level {
        out = out.into_iter().flat_map(PatchKey::children).collect();
    }
    out
}

/// The upstream direction at `junction` of `segment`'s course over [`from`, `to`], `to` at the
/// junction: the orthogonal regression line of 33 points along it, in the plane tangent at the
/// junction, pointed towards its upstream end; `None` over an empty stretch.
fn upstream_line(segment: &Segment, from: f64, to: f64, junction: [f64; 3]) -> Option<[f64; 3]> {
    if to <= from {
        return None;
    }
    let up = unit(junction);
    let east = unit(cross([0.0, 0.0, 1.0], up));
    let north = cross(up, east);
    let samples = 32_u32;
    let points: Vec<(f64, f64)> = (0..=samples)
        .map(|k| {
            let t = if k == samples {
                to
            } else {
                from + (to - from) * f64::from(k) / f64::from(samples)
            };
            let p = segment.point_m(t);
            let d = [0, 1, 2].map(|i| p[i] - junction[i]);
            (dot(d, east), dot(d, north))
        })
        .collect();
    let n = f64::from(samples + 1);
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for &(x, y) in &points {
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
        sxy += (x - mx) * (y - my);
    }
    let angle = 0.5 * math::atan2(2.0 * sxy, sxx - syy);
    let (sin, cos) = (math::sin(angle), math::cos(angle));
    let far = points[0];
    let toward = if far.0 * cos + far.1 * sin >= 0.0 {
        1.0
    } else {
        -1.0
    };
    Some([0, 1, 2].map(|i| toward * (cos * east[i] + sin * north[i])))
}

/// A groove's depth at `t` along `segment` as the census reads it: its depth below its top, at
/// least its channel's and at most its level's cap, metres.
fn depth_at(segment: &Segment, t: f64, radius_m: f64) -> f64 {
    let cap = 0.2 * mean_cell_width(radius_m, segment.level());
    let channel = segment.bankfull_depth().value().min(cap);
    segment.below_top(t).value().max(channel).min(cap)
}

/// One level's census.
#[derive(Debug, Default)]
struct Level {
    tributaries: u32,
    at_head: u32,
    joins: BTreeMap<(u64, u64), u32>,
    angles: Vec<f64>,
    ratios: Vec<f64>,
    steps: Vec<f64>,
}

/// The census of `field`'s network below `cells`, by level, each cell's levels to its cut.
fn census(field: &CoarseField, cells: &[PatchKey]) -> BTreeMap<u8, Level> {
    let header = field.header();
    let body = ChannelBody::of(header);
    let first = header.level().get();
    let radius_m = body.radius.value();
    let synth = Synthesiser::new(field, DETAIL);
    let mut network = synth.channels().network(field);
    let mut levels: BTreeMap<u8, Level> = BTreeMap::new();
    for &coarse in cells {
        let cut = deepest_level(field.cell(coarse).expect("held"), body);
        for level in first + 1..=cut.min(first + 6) {
            let tally = levels.entry(level).or_default();
            for cell in below(coarse, level) {
                let Some(segment) = network.segment(cell).expect("held") else {
                    continue;
                };
                let Outlet::Join { parent, at } = segment.outlet() else {
                    continue;
                };
                let parent_segment = network.segment(parent).expect("held").expect("a parent");
                tally.tributaries += 1;
                tally.at_head += u32::from(at <= 0.0);
                *tally.joins.entry((parent.to_u64(), bits(at))).or_default() += 1;
                if at > 0.0 && at < 1.0 {
                    let junction = parent_segment.point_m(at);
                    if let (Some(a), Some(b)) = (
                        upstream_line(&parent_segment, 0.0, at, junction),
                        upstream_line(&segment, 0.0, 1.0, junction),
                    ) {
                        tally
                            .angles
                            .push(math::acos(dot(a, b).clamp(-1.0, 1.0)).to_degrees());
                    }
                }
                let join = depth_at(&segment, 1.0, radius_m);
                if level == cut && join > CUT_DEEPER_THAN_M {
                    tally.ratios.push(depth_at(&segment, 0.0, radius_m) / join);
                }
                tally
                    .steps
                    .push(depth_at(&parent_segment, at, radius_m) - join);
            }
        }
    }
    levels
}

/// The census of `name`'s network over its look's frames, printed and asserted.
fn check_the_network(name: &str) {
    let field = world(name);
    let header = field.header();
    let first = header.level().get();
    let synth = Synthesiser::new(field, DETAIL);
    let centres = frame_centres(field, &mut synth.channels().network(field));
    let cells = census_cells(field, centres);
    let levels = census(field, &cells);
    println!("{name}: {} coarse cells", cells.len());
    let (mut angles, mut ratios, mut steps) = (Vec::new(), Vec::new(), Vec::new());
    for (&level, tally) in &levels {
        let points = tally.joins.len();
        let multiple = tally.joins.values().filter(|&&n| n >= 3).count();
        #[expect(clippy::cast_precision_loss, reason = "counts of joins, exact in f64")]
        let share = multiple as f64 / points.max(1) as f64;
        println!(
            "  level {level}: {} tributaries, {:.1}% joining their parent's head, {multiple} of \
             {points} join points with three or more inflows ({:.2}%)",
            tally.tributaries,
            100.0 * f64::from(tally.at_head) / f64::from(tally.tributaries.max(1)),
            100.0 * share
        );
        if level >= first + 2 {
            assert!(
                share <= MOST_MULTIPLE_JOINS,
                "{name}: {:.2}% of level {level}'s join points take three or more inflows",
                100.0 * share
            );
        }
        sorted_and_printed("junction angles, °", tally.angles.clone());
        sorted_and_printed("head over join at the cut", tally.ratios.clone());
        let level_steps = sorted_and_printed("step at the join, m", tally.steps.clone());
        #[expect(clippy::cast_precision_loss, reason = "a count of joins, exact in f64")]
        let over = level_steps
            .iter()
            .filter(|&&s| s > CUT_DEEPER_THAN_M)
            .count() as f64
            / level_steps.len().max(1) as f64;
        println!("    steps over 10 m: {:.1}%", 100.0 * over);
        angles.extend_from_slice(&tally.angles);
        ratios.extend_from_slice(&tally.ratios);
        steps.extend_from_slice(&tally.steps);
    }
    let (mean, sd) = mean_and_sd(&angles);
    let angles = sorted_and_printed("all junction angles from L + 1, °", angles);
    println!("    their mean {mean:.1}° and SD {sd:.1}° (Earth's about 25–35°)");
    let ratios = sorted_and_printed("all heads over joins at the cut", ratios);
    let steps = sorted_and_printed("all steps at the join, m", steps);
    let median = quantile(&angles, 0.5);
    let iqr = quantile(&angles, 0.75) - quantile(&angles, 0.25);
    assert!(
        (ANGLE_MEDIAN_DEG.0..=ANGLE_MEDIAN_DEG.1).contains(&median),
        "{name}: the junction angles' median is {median}°"
    );
    assert!(
        iqr >= ANGLE_IQR_DEG,
        "{name}: the junction angles' interquartile range is {iqr}°"
    );
    let head = quantile(&ratios, 0.5);
    assert!(
        head <= MOST_HEAD_OVER_JOIN,
        "{name}: the heads' median depth is {head} of their join's"
    );
    let step = quantile(&steps, 0.5);
    assert!(
        step.abs() <= 1e-6,
        "{name}: the median step at a join is {step} m"
    );
}

/// R09.T6.g's (b) under R09.T6.i's rules: the coarse pass's `C_c` books the synthesis's drawn
/// channel variance to 1% over every sixteenth cell, and the realised `σ_h` is the inputs' to 0.1%.
fn check_the_budget(name: &str, inputs: &CoarseInputs) {
    let field = world(name);
    let header = field.header();
    let body = ChannelBody::of(header);
    let synth = Synthesiser::new(field, DETAIL);
    let whole = Metres::new(1e7);
    let level = header.level();
    let (mut booked, mut drawn, mut cells) = (0.0, 0.0, 0_u32);
    for index in (0..level.cell_count()).step_by(16) {
        let cell = cell_at_index(level.get(), index).expect("a field's cell");
        booked += drawn_channel_variance(|k| field.cell(k), cell, header.sea_level(), body, whole)
            .expect("held")
            .value();
        drawn += synth
            .unresolved_variance(cell, whole)
            .expect("held")
            .channels()
            .value();
        cells += 1;
    }
    let n = f64::from(cells);
    println!(
        "{name}: the budget's channel share {:.1} m² against the synthesis's {:.1} m², {:.3}% apart, \
         over {cells} cells",
        booked / n,
        drawn / n,
        100.0 * (booked / drawn - 1.0)
    );
    assert!(
        (booked / drawn - 1.0).abs() <= 0.01,
        "{name}: the budget books {booked} m² against {drawn} m² drawn"
    );
    let target = inputs.sigma_h().value();
    let realised = header.realised_sigma_h().value();
    println!("{name}: realised σ_h {realised:.3} m against {target:.3} m");
    assert!(
        (realised / target - 1.0).abs() <= 1e-3,
        "{name}: the realised σ_h {realised} m misses {target} m"
    );
}

/// R09.T6.i's (b) and (d) on `name`: at 64 points off the edges of each of five cells carrying
/// tributaries, along two tangents each, the incision's analytic gradient through L + 4, its walls'
/// width changing along its graded and accordant courses, is its central difference over 1 m, but
/// for at most 3% of the steps, which cross a kink of the maximum over its grooves and sub-chords,
/// or where a depth meets its floor or its cap.
fn check_the_gradient(name: &str) {
    let field = world(name);
    let header = field.header();
    let first = header.level().get();
    let figure = header.figure();
    let synth = Synthesiser::new(field, DETAIL);
    let mut cache = SynthCache::new();
    let (mut good, mut total) = (0_u32, 0_u32);
    let offsets = [0.093, 0.218, 0.361, 0.477, 0.604, 0.729, 0.852, 0.967];
    for cell in with_tributaries(field, 5) {
        for (x, y) in offsets
            .iter()
            .flat_map(|&x| offsets.iter().map(move |&y| (x, y)))
        {
            let dir = point_in(cell, x, 1.0 - y);
            let sample = synth
                .channels()
                .incision_at(field, dir, first + 4, &mut cache)
                .expect("held");
            let normal = figure.normal(dir);
            let east = unit(cross([0.0, 0.0, 1.0], normal));
            let north = cross(normal, east);
            let p = figure.point(dir);
            for tangent in [east, north] {
                let mut height = |step: f64| {
                    let q = [0, 1, 2].map(|k| p[k] + step * tangent[k]);
                    let at = unit_dir([
                        q[0] / figure.equatorial_radius_m,
                        q[1] / figure.equatorial_radius_m,
                        q[2] / figure.polar_radius_m,
                    ]);
                    synth
                        .channels()
                        .incision_at(field, at, first + 4, &mut cache)
                        .expect("held")
                        .height_m
                };
                let numeric = f64::midpoint(height(1.0), -height(-1.0));
                let analytic = dot(sample.gradient, tangent);
                total += 1;
                good += u32::from((numeric - analytic).abs() < 1e-5 + 1e-4 * analytic.abs());
            }
        }
    }
    println!("{name}: {good} of {total} gradients are their central differences");
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{name}: {good} of {total}"
    );
}

/// The largest median ratio of the incision's mean slope across a coarse cell's edge to its ratio
/// between two bands inside a cell (the plan's threshold, research-r09-earth-look's metric).
const MOST_EDGE_CONTRAST: f64 = 1.15;

/// A frame of the look: `side` × `side` pixels of `pixel_m` metres about `centre`'s middle,
/// rows north to south and columns west to east on the plane tangent there, as unit directions.
fn frame(centre: PatchKey, side: u32, pixel_m: f64, radius_m: f64) -> Vec<[f64; 3]> {
    let half = u8::try_from(PATCH_QUADS / 2).expect("a patch's half fits a u8");
    let c = centre.vertex_dir(half, half);
    let east = unit([-c[1], c[0], 0.0]);
    let north = cross(c, east);
    let offset = |k: u32| (f64::from(k) + 0.5 - f64::from(side) / 2.0) * pixel_m;
    (0..side)
        .flat_map(|row| (0..side).map(move |column| (row, column)))
        .map(|(row, column)| {
            let (e, n) = (offset(column), -offset(row));
            unit_dir([0, 1, 2].map(|k| c[k] * radius_m + east[k] * e + north[k] * n))
        })
        .collect()
}

/// The pixels of `cells` (each pixel's coarse cell, row by row on a `side`-pixel frame) that lie
/// in `a`, by their distance in pixel steps from those of `a`'s touching `b`, to `reach`: a
/// breadth-first walk inside `a`.
fn distances_from(
    cells: &[u64],
    side: usize,
    a: u64,
    b: u64,
    reach: usize,
) -> BTreeMap<usize, usize> {
    let neighbours = |i: usize| {
        let (r, c) = (i / side, i % side);
        let mut out = Vec::with_capacity(4);
        if r > 0 {
            out.push(i - side);
        }
        if r + 1 < side {
            out.push(i + side);
        }
        if c > 0 {
            out.push(i - 1);
        }
        if c + 1 < side {
            out.push(i + 1);
        }
        out
    };
    let mut distance = BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();
    for i in 0..cells.len() {
        if cells[i] == a && neighbours(i).iter().any(|&j| cells[j] == b) {
            distance.insert(i, 0);
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let d = distance[&i];
        if d >= reach {
            continue;
        }
        for j in neighbours(i) {
            if cells[j] == a && !distance.contains_key(&j) {
                distance.insert(j, d + 1);
                queue.push_back(j);
            }
        }
    }
    distance
}

/// The edge contrasts of a frame `side` pixels of `pixel_m` metres a side, row by row, given each
/// pixel's coarse cell, whether that is dry and the incision's slope there: for each edge between
/// two dry cells, the ratio of the larger to the smaller of the mean slopes in the `band` pixels
/// either side of it; and for each side, the same between its second and third bands from it.
fn edge_contrasts(
    cells: &[u64],
    dry: &[bool],
    slope: &[f64],
    side: usize,
    band: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = side;
    let mut pairs = BTreeSet::new();
    for i in 0..n * n {
        for j in [i + 1, i + n] {
            let wraps = j == i + 1 && (i + 1) % n == 0;
            if j < n * n && !wraps && cells[i] != cells[j] && dry[i] && dry[j] {
                pairs.insert((cells[i].min(cells[j]), cells[i].max(cells[j])));
            }
        }
    }
    let ratio = |x: f64, y: f64| {
        let (low, high) = (x.min(y), x.max(y));
        if low > 1e-9 {
            high / low
        } else {
            f64::INFINITY
        }
    };
    let band_mean = |distance: &BTreeMap<usize, usize>, from: usize, to: usize| {
        let values: Vec<f64> = distance
            .iter()
            .filter(|&(_, &d)| d >= from && d < to)
            .map(|(&i, _)| slope[i])
            .collect();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of pixels, exact in f64"
        )]
        let count = values.len() as f64;
        (values.iter().sum::<f64>() / count.max(1.0), values.len())
    };
    let (mut across, mut within) = (Vec::new(), Vec::new());
    for &(a, b) in &pairs {
        let (from_a, from_b) = (
            distances_from(cells, n, a, b, 3 * band),
            distances_from(cells, n, b, a, 3 * band),
        );
        let ((mean_a, count_a), (mean_b, count_b)) =
            (band_mean(&from_a, 0, band), band_mean(&from_b, 0, band));
        if count_a < 4 * band || count_b < 4 * band {
            continue;
        }
        across.push(ratio(mean_a, mean_b));
        for distance in [&from_a, &from_b] {
            let ((first, count_1), (second, count_2)) = (
                band_mean(distance, band, 2 * band),
                band_mean(distance, 2 * band, 3 * band),
            );
            if count_1 >= 4 * band && count_2 >= 4 * band {
                within.push(ratio(first, second));
            }
        }
    }
    across.sort_by(f64::total_cmp);
    within.sort_by(f64::total_cmp);
    (across, within)
}

/// R09.T6.i's (e) on `name`, recorded by frame: over the look's trunk-1 and trunk-2 (720 pixels of
/// 150 m) and first view (540 of 600 m) at band level 12, the median ratio of the incision's mean
/// slope in bands of 13 or 3 pixel steps (about 1.3–2 km) either side of each coarse-cell edge
/// between two dry cells, the larger over the smaller, against the same between two adjacent bands
/// inside a cell, its second and third from the edge, and whether it is within
/// [`MOST_EDGE_CONTRAST`] (research-r09-earth-look's band metric, ported). It is recorded, not
/// asserted: the reference Earth's trunk-1 is beyond it (1.25), as the research's pilot was there.
fn check_the_cell_edges(name: &str) {
    let field = world(name);
    let header = field.header();
    let level = header.level().get();
    let radius_m = header.radius().value();
    let synth = Synthesiser::new(field, DETAIL);
    let centres = frame_centres(field, &mut synth.channels().network(field));
    let mut cache = SynthCache::new();
    for (label, centre, side, pixel_m, band) in [
        ("trunk-1", centres[0], 720_u32, 150.0, 13_usize),
        ("trunk-2", centres[1], 720, 150.0, 13),
        ("the first view", centres[2], 540, 600.0, 3),
    ] {
        let dirs = frame(centre, side, pixel_m, radius_m);
        let n = usize::try_from(side).expect("a frame's side");
        let cells: Vec<u64> = dirs
            .iter()
            .map(|&dir| PatchKey::containing(level, dir).expect("a cell").to_u64())
            .collect();
        let dry: Vec<bool> = cells
            .iter()
            .map(|&key| {
                let cell = PatchKey::from_u64(key).expect("a packed key");
                !field.cell(cell).expect("held").is_under_water()
            })
            .collect();
        let heights: Vec<f64> = dirs
            .iter()
            .map(|&dir| {
                synth
                    .channels()
                    .incision_at(field, dir, 12, &mut cache)
                    .expect("held")
                    .height_m
            })
            .collect();
        let mut slope = vec![0.0; n * n];
        for r in 1..n - 1 {
            for c in 1..n - 1 {
                let i = r * n + c;
                let gx = (heights[i + 1] - heights[i - 1]) / (2.0 * pixel_m);
                let gy = (heights[i + n] - heights[i - n]) / (2.0 * pixel_m);
                slope[i] = (gx * gx + gy * gy).sqrt();
            }
        }
        let (across, within) = edge_contrasts(&cells, &dry, &slope, n, band);
        let (edge, inside) = (quantile(&across, 0.5), quantile(&within, 0.5));
        println!(
            "{name}'s {label} about {centre:?}: {} edges; across an edge median {edge:.2}, p90 \
             {:.2}; between bands inside a cell median {inside:.2}, p90 {:.2}; their ratio {:.2}, \
             {} the plan's {MOST_EDGE_CONTRAST}",
            across.len(),
            quantile(&across, 0.9),
            quantile(&within, 0.9),
            edge / inside,
            if edge <= MOST_EDGE_CONTRAST * inside {
                "within"
            } else {
                "beyond"
            }
        );
        assert!(
            across.len() >= 10 && within.len() >= 10,
            "{name}'s {label}: {} edges and {} bands",
            across.len(),
            within.len()
        );
    }
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8 and the census of 20,000 tributaries"]
fn the_reference_earth_s_tributaries_join_as_a_humid_network_s_do() {
    check_the_network("earth");
    check_the_gradient("earth");
    check_the_cell_edges("earth");
    check_the_budget("earth", &reference::earth_like());
}

#[test]
#[ignore = "slow: the locked Earth's coarse pass at level 8 and the census of its tributaries"]
fn the_locked_earth_s_tributaries_join_as_a_humid_network_s_do() {
    check_the_network("locked");
    check_the_gradient("locked");
    check_the_cell_edges("locked");
    check_the_budget("locked", &reference::locked_earth_like());
}

/// The cells of `field` carrying tributaries, about `count` of them spread through index order.
fn with_tributaries(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let header = field.header();
    let body = ChannelBody::of(header);
    let level = header.level();
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).expect("a field's cell"))
        .filter(|&c| {
            let record = field.cell(c).expect("held");
            !record.is_under_water() && deepest_level(record, body) > level.get()
        })
        .collect();
    let step = (all.len() / count).max(1);
    all.into_iter().step_by(step).take(count).collect()
}

/// The by-hand measurement behind R09.T6.i's constants, on its three wet worlds: per level below
/// 12 cells carrying tributaries, the share of cells heading one and their bent courses' mean
/// length in the level's cells ([`TRIBUTARY_SHARE`], [`MEAN_BENT_TRIBUTARY_LENGTH`]); over every dry
/// trunk, the bent course's length over its straight chords' (`s_net`, [`WET_TRUNK_SINUOSITY`]);
/// and over 20 cells on 32 × 32 grids, each tributary level's realised variance over its closed
/// form's and the whole's over the levels' sum, scaled by the constants in force, whose medians are
/// [`BENT_TRIBUTARY_VARIANCE_SHARE`] and [`BENT_WHOLE_VARIANCE_SHARE`] re-measured.
#[test]
#[ignore = "a measurement for R09.T6.i's constants, run by hand"]
#[expect(
    clippy::too_many_lines,
    reason = "one by-hand measurement, printed in the order the records give it"
)]
fn measure_the_bent_network_s_constants() {
    let (mut all_levels, mut all_whole) = (Vec::new(), Vec::new());
    let synthetic = synthetic_field(SyntheticWorld::EarthLike);
    for (name, field) in [
        ("the synthetic Earth-like", &synthetic),
        ("the reference Earth", world("earth")),
        ("the locked Earth", world("locked")),
    ] {
        let header = field.header();
        let first = header.level().get();
        let radius_m = header.radius().value();
        let synth = Synthesiser::new(field, DETAIL);
        let mut network = synth.channels().network(field);
        println!("{name}:");
        let cells = with_tributaries(field, 12);
        let (mut lengths, mut shares) = (Vec::new(), Vec::new());
        for level in first + 1..=first + 4 {
            let (mut count, mut heads, mut length) = (0_u32, 0_u32, 0.0);
            for &c in &cells {
                for cell in below(c, level) {
                    count += 1;
                    if let Some(segment) = network.segment(cell).expect("held") {
                        heads += 1;
                        length += segment.length().value() / mean_cell_width(radius_m, level);
                    }
                }
            }
            let share = f64::from(heads) / f64::from(count);
            let mean = length / f64::from(heads.max(1));
            println!("  level {level}: share {share:.3}, length {mean:.3} cells");
            lengths.push(mean);
            shares.push(share);
        }
        println!(
            "  mean share {:.3} (TRIBUTARY_SHARE {TRIBUTARY_SHARE}), mean length {:.3} cells \
             (MEAN_BENT_TRIBUTARY_LENGTH {MEAN_BENT_TRIBUTARY_LENGTH})",
            mean_and_sd(&shares).0,
            mean_and_sd(&lengths).0
        );
        let (mut ratio, mut trunks) = (0.0, 0_u32);
        for (index, record) in (0_u32..).zip(field.synthesis()) {
            if record.is_under_water() {
                continue;
            }
            let cell = cell_at_index(first, index).expect("a field's cell");
            let Some(segment) = network.segment(cell).expect("held") else {
                continue;
            };
            let Some(crossing) = segment.crossing_m() else {
                continue;
            };
            let straight =
                distance(segment.from_m(), crossing) + distance(crossing, segment.to_m());
            ratio += segment.length().value() / straight;
            trunks += 1;
        }
        println!(
            "  s_net {:.4} over {trunks} trunks (WET_TRUNK_SINUOSITY {WET_TRUNK_SINUOSITY})",
            ratio / f64::from(trunks)
        );
        drop(network);
        let mut cache = SynthCache::new();
        let (mut by_level, mut whole) = (Vec::new(), Vec::new());
        for c in with_tributaries(field, 20) {
            let Some(check) = synth
                .channels()
                .variance_check(field, c, 32, &mut cache)
                .expect("held")
            else {
                continue;
            };
            let closed: BTreeMap<u8, f64> = check.closed_m2.iter().copied().collect();
            let mut levels = 0.0;
            for &(level, realised) in &check.realised_m2 {
                levels += realised;
                if level > first
                    && let Some(&closed) = closed.get(&level)
                    && closed > 1.0
                {
                    by_level.push(realised / closed * BENT_TRIBUTARY_VARIANCE_SHARE);
                }
            }
            if levels > 1.0 {
                whole.push(check.whole_m2 / levels);
            }
        }
        all_levels.extend_from_slice(&by_level);
        all_whole.extend_from_slice(&whole);
        let by_level = sorted_and_printed("tributary levels' realised over closed", by_level);
        let whole = sorted_and_printed("whole over the levels' sum", whole);
        println!(
            "  BENT_TRIBUTARY_VARIANCE_SHARE {:.3} (in force {BENT_TRIBUTARY_VARIANCE_SHARE}), \
             BENT_WHOLE_VARIANCE_SHARE {:.3} (in force {BENT_WHOLE_VARIANCE_SHARE})",
            quantile(&by_level, 0.5),
            quantile(&whole, 0.5)
        );
    }
    println!("the three worlds:");
    let all_levels = sorted_and_printed("tributary levels' realised over closed", all_levels);
    let all_whole = sorted_and_printed("whole over the levels' sum", all_whole);
    println!(
        "  BENT_TRIBUTARY_VARIANCE_SHARE {:.3}, BENT_WHOLE_VARIANCE_SHARE {:.3}",
        quantile(&all_levels, 0.5),
        quantile(&all_whole, 0.5)
    );
}
