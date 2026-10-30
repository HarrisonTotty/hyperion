//! The reference Mars's bent trunks (plan R09, R09.T6.h; main's rulings of 2026-10-30): its coarse
//! pass at level 7 and the synthesis's trunk of every cell whose trunk cuts its wet epoch's valley
//! ([`epoch_valley`] of depth above zero), the drawn courses R09.T6.c's look ranked.
//!
//! - **The bend:** every course runs from its key point through its crossing to its receiver's key
//!   point, the straight trunk's bits, and every point of its first half lies in its cell and of
//!   its second in its receiver, across face edges too;
//! - **`s_net`**, the courses' mean length over their straight chords', lies in 1.03–1.07 (the
//!   run's scale, a labelled extrapolation of a fractal dimension derived from Som, Montgomery and
//!   Greenberg 2009's Table 2), recorded by the cell's place on its face;
//! - **the whole courses**, from each drawn head down its drawn receivers, have their length over
//!   the great-circle span from the head's key point a median inside Som et al.'s 1.10–1.57 among
//!   the courses 160–885 km long, the valley lengths their ten valleys cover (JGR 114, E02005,
//!   doi:10.1029/2008JE003132, Table 2's "Valley Length"), the quantiles recorded; and, recorded,
//!   among those whose span is 160–885 km, the measure the network's own 1.263 was taken by before
//!   the bend.
//!
//! Run with `cargo nextest run -p hyperion-sim --features testing --test trunk_bend_worlds
//! --cargo-profile slow-test --profile slow --run-ignored only`.
#![cfg(feature = "testing")]

use std::collections::{BTreeMap, BTreeSet};

use hyperion_base::rng::DetailSeed;
use hyperion_sim::math;
use hyperion_sim::planetary::surface::{coarse_pass, reference};
use hyperion_surface::cube::{PatchKey, unit_dir};
use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
use hyperion_surface::synth::Synthesiser;
use hyperion_surface::synth::channels::{
    BEND_PARTS, ChannelBody, ChannelNetwork, EPOCH_TRUNK_SINUOSITY, Outlet, Segment, epoch_valley,
};
use hyperion_testkit::float::bits;

/// The detail seed: `trunk_map`'s, the look's.
const DETAIL: DetailSeed = DetailSeed::new(0x7472_756e_6b6d_6170);

/// `s_net`'s range, the run's scale (the brief's lean, 2026-10-30).
const S_NET: (f64, f64) = (1.03, 1.07);

/// Som, Montgomery and Greenberg 2009's whole-valley sinuosity, its ten valleys' range.
const SOM: (f64, f64) = (1.10, 1.57);

/// The lengths along the valleys Som et al.'s cover, metres: Table 2's "Valley Length", Tinto's
/// 160 km to Hrad's 885 km.
const SOM_LENGTHS_M: (f64, f64) = (160e3, 885e3);

/// The distance between two points.
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [0, 1, 2].map(|k| b[k] - a[k]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// `values`' count, p10, p25, median, mean, p75, p90 and maximum, as `label`'s, and its median.
fn print_quantiles(label: &str, values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a rank of a list of trunks, exact in f64 and back"
        )]
        let rank = ((values.len() - 1) as f64 * q).round() as usize;
        values[rank]
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "a count of trunks, exact in f64"
    )]
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    eprintln!(
        "{label} ({}): p10 {:.3}, p25 {:.3}, median {:.3}, mean {mean:.4}, p75 {:.3}, p90 {:.3}, \
         max {:.3}",
        values.len(),
        at(0.1),
        at(0.25),
        at(0.5),
        at(0.75),
        at(0.9),
        at(1.0)
    );
    at(0.5)
}

/// The mean of `values`.
fn mean(values: &[f64]) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a count of trunks, exact in f64"
    )]
    let n = values.len() as f64;
    values.iter().sum::<f64>() / n
}

/// Every trunk of `field` that cuts its wet epoch's valley, by packed cell key.
fn drawn_trunks(
    field: &CoarseField,
    network: &mut ChannelNetwork<'_, CoarseField>,
) -> BTreeMap<u64, Segment> {
    let header = field.header();
    let level = header.level().get();
    let body = ChannelBody::of(header);
    let mut drawn = BTreeMap::new();
    for (index, cell) in (0_u32..).zip(field.synthesis()) {
        if cell.is_under_water()
            || !epoch_valley(cell, body).is_some_and(|valley| valley.depth().value() > 0.0)
        {
            continue;
        }
        let c = cell_at_index(level, index).expect("a field's cell");
        let segment = network.segment(c).expect("the field holds its cells");
        drawn.insert(
            c.to_u64(),
            segment.expect("a cell that cuts a valley has a trunk"),
        );
    }
    drawn
}

/// Asserts each of `drawn`'s courses keeps its straight trunk's ends and each half in its cell,
/// and returns each one's bent over straight length, `s_net`, with the key point's place on its
/// face, max(|u|, |v|) in quarters, and how many cross a face edge.
fn check_the_bends(
    field: &CoarseField,
    network: &mut ChannelNetwork<'_, CoarseField>,
    drawn: &BTreeMap<u64, Segment>,
) -> (Vec<(f64, usize)>, u32) {
    let header = field.header();
    let level = header.level().get();
    let figure = header.figure();
    let dir_of = |p: [f64; 3]| {
        unit_dir([
            p[0] / figure.equatorial_radius_m,
            p[1] / figure.equatorial_radius_m,
            p[2] / figure.polar_radius_m,
        ])
    };
    let (mut out, mut across_faces) = (Vec::new(), 0_u32);
    for (&key, segment) in drawn {
        let c = PatchKey::from_u64(key).expect("a packed key");
        let Outlet::Cell { receiver, .. } = segment.outlet() else {
            panic!("a coarse cell's segment is a trunk");
        };
        let points = segment.bend_m().expect("a world dry now's trunk is bent");
        let crossing = segment.crossing_m().expect("a trunk has a crossing");
        assert_eq!(points[0].map(bits), segment.from_m().map(bits), "{c:?}");
        assert_eq!(points[BEND_PARTS].map(bits), crossing.map(bits), "{c:?}");
        assert_eq!(
            points[2 * BEND_PARTS].map(bits),
            segment.to_m().map(bits),
            "{c:?}"
        );
        let key_point = network.key_point(c);
        let onward = network.key_point(receiver);
        assert_eq!(
            segment.from_m().map(bits),
            figure.point(key_point.dir()).map(bits),
            "{c:?}"
        );
        assert_eq!(
            segment.to_m().map(bits),
            figure.point(onward.dir()).map(bits),
            "{c:?}"
        );
        across_faces += u32::from(receiver.face() != c.face());
        for (k, &p) in points.iter().enumerate() {
            let holder = PatchKey::containing(level, dir_of(p)).expect("a direction's cell");
            if (1..BEND_PARTS).contains(&k) {
                assert_eq!(holder, c, "{c:?}'s point {k} leaves its cell");
            } else if (BEND_PARTS + 1..2 * BEND_PARTS).contains(&k) {
                assert_eq!(holder, receiver, "{c:?}'s point {k} leaves its receiver");
            }
        }
        let straight = distance(segment.from_m(), crossing) + distance(crossing, segment.to_m());
        let (s, t) = key_point.st();
        let off_centre = (2.0 * s - 1.0).abs().max((2.0 * t - 1.0).abs());
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share of a face in [0, 1], in quarters"
        )]
        let quarter = ((off_centre * 4.0) as usize).min(3);
        out.push((segment.length().value() / straight, quarter));
    }
    (out, across_faces)
}

/// The whole courses of `drawn` on a body of mean radius `radius_m`: from each drawn cell no
/// drawn trunk flows into, down its drawn receivers, the length along them over the great-circle
/// span from the head's key point, where the span first reaches 40 km and 160 km, and at the
/// course's end where its length is one of Som et al.'s, and where its span is; and the same as
/// the first along its trunks' straight chords, unbent.
fn courses(drawn: &BTreeMap<u64, Segment>, radius_m: f64) -> [Vec<f64>; 5] {
    let fed: BTreeSet<u64> = drawn
        .values()
        .filter_map(|segment| match segment.outlet() {
            Outlet::Cell { receiver, .. } => Some(receiver.to_u64()),
            Outlet::Join { .. } => None,
        })
        .collect();
    let arc = |a: [f64; 3], b: [f64; 3]| {
        2.0 * radius_m * math::asin((distance(a, b) / (2.0 * radius_m)).min(1.0))
    };
    let (mut at_40, mut at_160) = (Vec::new(), Vec::new());
    let (mut by_length, mut by_span, mut unbent) = (Vec::new(), Vec::new(), Vec::new());
    for (key, first) in drawn {
        if fed.contains(key) {
            continue;
        }
        let start = first.from_m();
        let (mut length, mut straight, mut current) = (0.0, 0.0, first);
        let (mut got_40, mut got_160) = (false, false);
        for _ in 0..400 {
            length += current.length().value();
            let crossing = current.crossing_m().expect("a trunk has a crossing");
            straight += distance(current.from_m(), crossing) + distance(crossing, current.to_m());
            let span = arc(start, current.to_m());
            if !got_40 && span >= 40e3 {
                at_40.push(length / span);
                got_40 = true;
            }
            if !got_160 && span >= 160e3 {
                at_160.push(length / span);
                got_160 = true;
            }
            let Outlet::Cell { receiver, .. } = current.outlet() else {
                break;
            };
            let Some(onward) = drawn.get(&receiver.to_u64()) else {
                let som = SOM_LENGTHS_M.0..=SOM_LENGTHS_M.1;
                if som.contains(&length) {
                    by_length.push(length / span);
                }
                if som.contains(&span) {
                    by_span.push(length / span);
                }
                if som.contains(&straight) {
                    unbent.push(straight / span);
                }
                break;
            };
            current = onward;
        }
    }
    [at_40, at_160, by_length, by_span, unbent]
}

#[test]
#[ignore = "slow: the reference Mars's coarse pass at level 7 and its 27,000 drawn trunks"]
fn the_reference_mars_s_trunks_are_bent_inside_their_cells() {
    let field = coarse_pass(reference::SEED, &reference::mars_like());
    let synth = Synthesiser::new(&field, DETAIL);
    let mut network = synth.channels().network(&field);
    let drawn = drawn_trunks(&field, &mut network);
    assert!(drawn.len() > 20_000, "{} drawn trunks", drawn.len());
    let (ratios, across_faces) = check_the_bends(&field, &mut network, &drawn);
    eprintln!(
        "{} drawn trunks bent, {across_faces} across face edges",
        drawn.len()
    );
    let mut s_net: Vec<f64> = ratios.iter().map(|&(ratio, _)| ratio).collect();
    let net = mean(&s_net);
    print_quantiles("s_net, each trunk's bent over straight length", &mut s_net);
    eprintln!("s_net's mean {net:.4} (EPOCH_TRUNK_SINUOSITY {EPOCH_TRUNK_SINUOSITY})");
    for quarter in 0..4 {
        let values: Vec<f64> = ratios
            .iter()
            .filter(|&&(_, q)| q == quarter)
            .map(|&(ratio, _)| ratio)
            .collect();
        if !values.is_empty() {
            eprintln!(
                "s_net by the key point's place on its face, max(|u|, |v|) in quarter {quarter}: \
                 {:.4} over {} trunks",
                mean(&values),
                values.len()
            );
        }
    }
    let [
        mut at_40,
        mut at_160,
        mut by_length,
        mut by_span,
        mut unbent,
    ] = courses(&drawn, field.header().radius().value());
    print_quantiles(
        "whole courses 160–885 km long on their straight chords, unbent, sinuosity",
        &mut unbent,
    );
    print_quantiles("the courses' sinuosity at a 40 km span", &mut at_40);
    print_quantiles("the courses' sinuosity at a 160 km span", &mut at_160);
    print_quantiles("whole courses spanning 160–885 km, sinuosity", &mut by_span);
    let median = print_quantiles("whole courses 160–885 km long, sinuosity", &mut by_length);
    assert!(across_faces > 0, "no drawn trunk crosses a face edge");
    assert!(
        (S_NET.0..=S_NET.1).contains(&net),
        "s_net's mean is {net}, outside {S_NET:?}"
    );
    assert!(
        (SOM.0..=SOM.1).contains(&median),
        "the whole courses' median sinuosity is {median}, outside Som et al.'s {SOM:?}"
    );
}
