//! A world dry now's bent and graded trunks (plan R09, R09.T6.h) on the synthetic Mars-like world,
//! whose every trunk is bent: each course keeps its straight trunk's ends and each half in its
//! cell, its depth stays between its own cell's valley and its receiver's, no step crosses a bend,
//! and the groove's gradient is its central difference. The reference Mars's courses are the sim's
//! `trunk_bend_worlds` test.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::synth::channels::{BEND_PARTS, distance};

/// The unit direction of the spheroid point `p` (metres) on `field`'s body: M⁻¹ p, normalised.
fn dir_of(field: &CoarseField, p: [f64; 3]) -> [f64; 3] {
    let figure = field.header().figure();
    unit_dir([
        p[0] / figure.equatorial_radius_m,
        p[1] / figure.equatorial_radius_m,
        p[2] / figure.polar_radius_m,
    ])
}

/// Every trunk of `field` that cuts a valley, with its cell, in cell-index order.
fn bent_trunks(field: &CoarseField) -> Vec<(PatchKey, Segment)> {
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    with_epoch_valleys(field, usize::MAX)
        .into_iter()
        .map(|c| (c, network.segment(c).unwrap().expect("a trunk")))
        .collect()
}

/// Each of the epoch Mars's bent courses runs from its key point through its crossing to its
/// receiver's key point, the straight trunk's bits; every point of its first half lies in its
/// cell and of its second in its receiver, across face edges too; its parameters rise from 0 to 1
/// in proportion to the length, its crossing's its exit; and its mean length over its straight
/// chords', `s_net`, is recorded (R09.T6.h).
#[test]
fn a_bent_course_keeps_its_ends_and_each_half_in_its_cell() {
    let field = mars_epoch();
    let level = field.header().level().get();
    let (mut ratio, mut count, mut across_faces) = (0.0, 0_u32, 0_u32);
    for (c, segment) in bent_trunks(field) {
        let points = segment.bend_m().expect("a world dry now's trunk is bent");
        assert_eq!(points.len(), 2 * BEND_PARTS + 1);
        let crossing = segment.crossing_m().unwrap();
        assert_eq!(points[0].map(bits), segment.from_m().map(bits), "{c:?}");
        assert_eq!(points[BEND_PARTS].map(bits), crossing.map(bits), "{c:?}");
        assert_eq!(points[2 * BEND_PARTS].map(bits), segment.to_m().map(bits));
        let Outlet::Cell { receiver, exit } = segment.outlet() else {
            panic!("a trunk");
        };
        across_faces += u32::from(receiver.face() != c.face());
        for (k, &p) in points.iter().enumerate() {
            let holder = PatchKey::containing(level, dir_of(field, p)).unwrap();
            if k > 0 && k < BEND_PARTS {
                assert_eq!(holder, c, "{c:?}'s point {k}");
            } else if k > BEND_PARTS && k < 2 * BEND_PARTS {
                assert_eq!(holder, receiver, "{c:?}'s point {k}");
            }
        }
        let bend = segment.bend().unwrap();
        assert_eq!(bits(bend.exit()), bits(exit));
        let mut along = 0.0;
        for k in 0..2 * BEND_PARTS {
            along += distance(points[k], points[k + 1]);
            let t = along / bend.length_m();
            assert!((t - bend.params()[k + 1]).abs() <= 1e-15, "{c:?}");
            // The same point, a zero's sign aside.
            assert_eq!(
                segment.point_m(bend.params()[k + 1]).map(|x| bits(x + 0.0)),
                points[k + 1].map(|x| bits(x + 0.0))
            );
        }
        assert_eq!(bits(bend.params()[0]), bits(0.0));
        assert_eq!(bits(bend.params()[2 * BEND_PARTS]), bits(1.0));
        let straight = distance(segment.from_m(), crossing) + distance(crossing, segment.to_m());
        ratio += segment.length().value() / straight;
        count += 1;
    }
    let s_net = ratio / f64::from(count);
    eprintln!(
        "the epoch Mars-like world's {count} bent trunks ({across_faces} across face edges): \
         s_net {s_net:.4} (EPOCH_TRUNK_SINUOSITY {EPOCH_TRUNK_SINUOSITY})"
    );
    assert!(across_faces > 0, "no trunk crosses a face edge");
    assert!((1.03..=1.07).contains(&s_net), "s_net {s_net}");
}

/// A graded trunk's depth below its top is its own cell's valley G over its first chord, grades to
/// its receiver's over its second with no gradient at either end, and lies between the two; and
/// its groove is nowhere deeper than the larger (R09.T6.h; main's ruling 1 of 2026-10-30).
#[test]
fn a_graded_trunk_is_never_deeper_than_its_two_cells_valleys() {
    let field = mars_epoch();
    let body = ChannelBody::of(field.header());
    let radius_m = body.radius.value();
    let mut graded = 0;
    for (c, segment) in bent_trunks(field) {
        let Outlet::Cell { receiver, exit } = segment.outlet() else {
            panic!("a trunk");
        };
        let own = epoch_valley(field.cell(c).unwrap(), body)
            .unwrap()
            .depth()
            .value();
        let onward =
            epoch_valley(field.cell(receiver).unwrap(), body).map_or(0.0, |v| v.depth().value());
        let (low, high) = (own.min(onward), own.max(onward));
        for n in 0..=64 {
            let t = f64::from(n) / 64.0;
            let g = segment.below_top(t).value();
            assert!(
                g >= low && g <= high,
                "{c:?} at {t}: {g} outside [{low}, {high}]"
            );
            if t <= exit {
                assert_eq!(bits(g), bits(own), "{c:?} at {t}");
            }
        }
        let end = segment.below_top(1.0).value();
        assert!(
            (end - onward).abs() <= 1e-12 * high,
            "{c:?}: {end} against {onward}"
        );
        let (_, start_rate) = graded_depth_m(own, onward, exit, exit + 1e-9 * (1.0 - exit));
        let (_, end_rate) = graded_depth_m(own, onward, exit, 1.0);
        assert!(start_rate.abs() <= 1e-6 * high && end_rate.abs() <= 1e-12 * high);
        graded += u32::from(bits(onward) != bits(own));
        let groove = Groove::of(&segment, radius_m).unwrap();
        for n in 0..=128 {
            let t = f64::from(n) / 128.0;
            let value = groove.at(segment.point_m(t)).value;
            assert!(value <= high, "{c:?} at {t}: {value} deeper than {high}");
        }
    }
    assert!(graded > 100, "{graded} graded trunks");
}

/// The unit vector of `v`.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    v.map(|x| x / n)
}

/// No step crosses a bend: about each of a bent course's interior points and its crossing, on
/// circles at two distances inside its walls, its groove changes between neighbouring samples by no
/// more than twice the steeper of their analytic slopes allows over their spacing, so that the
/// deepest sub-chord's groove changing across a bend's bisector leaves no jump
/// (research-r09-earth-look's finding: steps of up to 164 m where each point took its nearest
/// sub-chord's depth; R09.T6.h).
#[test]
fn no_step_crosses_a_bend() {
    let field = mars_epoch();
    let radius_m = ChannelBody::of(field.header()).radius.value();
    let samples = 720_u32;
    let mut circles = 0;
    for (c, segment) in bent_trunks(field).into_iter().step_by(211) {
        let Some(groove) = Groove::of(&segment, radius_m) else {
            continue;
        };
        let points = segment.bend_m().unwrap();
        for &centre in &points[1..2 * BEND_PARTS] {
            // Two tangents at the point.
            let up = unit(centre);
            let east = unit(cross([0.0, 0.0, 1.0], up));
            let north = cross(up, east);
            for share in [0.3, 0.7] {
                let r = groove.floor_m + share * groove.wall_m;
                let fine = 64_u32;
                let at = |n: u32| {
                    let a = 2.0 * core::f64::consts::PI * f64::from(n)
                        / (f64::from(samples) * f64::from(fine));
                    let (sin, cos) = math::sin_cos(a);
                    let q = [0, 1, 2].map(|k| centre[k] + r * (cos * east[k] + sin * north[k]));
                    groove.at(q)
                };
                let step = 2.0 * core::f64::consts::PI * r / f64::from(samples);
                let steepest = |v: Value| dot(v.gradient, v.gradient).sqrt();
                // A change the two ends' slopes do not allow is looked at over 64 finer steps, each
                // held to its own ends' slopes: a smooth change passes, a step does not.
                let mut before = at(0);
                for n in 1..=samples {
                    let now = at(n * fine);
                    let allowed = 2.0 * step * steepest(before).max(steepest(now)) + 1e-9;
                    if (now.value - before.value).abs() > allowed {
                        let mut last = before;
                        for m in 1..=fine {
                            let next = at((n - 1) * fine + m);
                            let allowed = 2.0 * step / f64::from(fine)
                                * steepest(last).max(steepest(next))
                                + 1e-9;
                            assert!(
                                (next.value - last.value).abs() <= allowed,
                                "{c:?}: a step of {} m about a bend over {} m",
                                next.value - last.value,
                                step / f64::from(fine)
                            );
                            last = next;
                        }
                    }
                    before = now;
                }
                circles += 1;
            }
        }
    }
    assert!(circles > 500, "{circles} circles");
}

/// A bent groove's analytic gradient, its depth and wall changing along its graded second chord,
/// is its central difference over a thousandth of its wall at points across its sub-chords inside
/// its walls, but where a step crosses a kink of the maximum over its sub-chords (R09.T6.h).
#[test]
fn a_bent_groove_s_gradient_is_its_central_difference() {
    let field = mars_epoch();
    let radius_m = ChannelBody::of(field.header()).radius.value();
    let (mut good, mut total) = (0_u32, 0_u32);
    for (_, segment) in bent_trunks(field).into_iter().step_by(53) {
        let Some(groove) = Groove::of(&segment, radius_m) else {
            continue;
        };
        let bend = segment.bend().unwrap();
        for half in 0..2 {
            for chord in bend.chords(half) {
                let middle = [0, 1, 2].map(|k| f64::midpoint(chord.from[k], chord.to[k]));
                let along = [0, 1, 2].map(|k| chord.to[k] - chord.from[k]);
                let across = unit(cross(along, middle));
                for share in [0.1, 0.35, 0.6, 0.85] {
                    let x = groove.floor_m + share * groove.wall_m;
                    let q = [0, 1, 2].map(|k| middle[k] + x * across[k]);
                    let analytic = groove.at(q).gradient;
                    for axis in 0..3 {
                        let h = 1e-3 * groove.wall_m;
                        let mut plus = q;
                        let mut minus = q;
                        plus[axis] += h;
                        minus[axis] -= h;
                        let numeric = (groove.at(plus).value - groove.at(minus).value) / (2.0 * h);
                        total += 1;
                        if (numeric - analytic[axis]).abs() <= 1e-6 + 1e-4 * numeric.abs() {
                            good += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(total > 1000, "{total} differences");
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{good} of {total}"
    );
}

/// The incision's analytic gradient on the epoch Mars-like world, its trunks bent and graded, is
/// its central difference, as on the Earth-like world
/// ([`the_gradient_is_the_incision_s_central_difference`]).
#[test]
fn the_epoch_world_s_gradient_is_the_incision_s_central_difference() {
    let field = mars_epoch();
    let (good, total) = central_differences(field, with_epoch_valleys(field, 5));
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{good} of {total}"
    );
}

/// The mean over a chord graded from `from` to `to` of `integrand(G, W)`, W = min(5 G, `cap`), by
/// a fine midpoint sum: what the closed forms stand for.
fn fine_mean(from: f64, to: f64, cap: f64, integrand: impl Fn(f64, f64) -> f64) -> f64 {
    let n = 200_000_u32;
    let mut sum = 0.0;
    for k in 0..n {
        let s = (f64::from(k) + 0.5) / f64::from(n);
        let g = from + (to - from) * (s * s * (3.0 - 2.0 * s));
        sum += integrand(g, (WALL_RUN_PER_DEPTH * g).min(cap));
    }
    sum / f64::from(n)
}

/// The graded second chord's closed forms are the integrals they stand for: its void a unit length
/// ([`EpochValley::graded_void_m2`]) and the trunk level's mean, mean square and slope's mean
/// square ([`epoch_trunk_term`]), from the smoothstep's ∫S = ½, ∫S² = 13 ÷ 35 and ∫S³ = 43 ÷ 140
/// below the wall's cap, to 10⁻⁹, and over [`GRADED_SAMPLES`] points where a wall reaches it, to
/// 10⁻³ (R09.T6.h).
#[test]
fn the_graded_closed_forms_are_their_integrals() {
    let body = ChannelBody::of(mars_epoch().header());
    let (r, level) = (body.radius.value(), body.level);
    let cap = VALLEY_WALL_SHARE * mean_cell_width(r, level);
    let floor = 120.0;
    let deep = Metres::new(1e4);
    // Valleys below the cap, and one whose receiver's wall reaches it.
    for (own, onward, tolerance) in [
        (150.0, 420.0, 1e-9),
        (380.0, 60.0, 1e-9),
        (90.0, 90.0, 1e-9),
        (200.0, 0.0, 1e-9),
        (3000.0, 6000.0, 1e-3),
    ] {
        let valley = EpochValley::of(
            deep,
            Metres::new(floor),
            Metres::ZERO,
            Metres::new(own),
            body,
        );
        assert_eq!(bits(valley.depth().value()), bits(own));
        let h = 0.5 * floor;
        let close = |closed: f64, fine: f64| (closed / fine - 1.0).abs() <= tolerance;
        let void = valley.graded_void_m2(Metres::new(onward));
        let fine_void = fine_mean(own, onward, cap, |g, w| g * (floor + w));
        assert!(
            close(void, fine_void),
            "{own} → {onward}: {void} against {fine_void}"
        );
        let density = 1e-5;
        let term = epoch_trunk_term(level, density, &valley, onward);
        let wall = valley.wall().value();
        let half = 0.5 * density;
        let mean = half * 2.0 * own * (h + 0.5 * wall)
            + half * fine_mean(own, onward, cap, |g, w| 2.0 * g * (h + 0.5 * w));
        let square = half * 2.0 * own * own * (h + 13.0 * wall / 35.0)
            + half * fine_mean(own, onward, cap, |g, w| 2.0 * g * g * (h + 13.0 * w / 35.0));
        let slope2 = half * own * own * 1.2 / wall
            + half
                * fine_mean(own, onward, cap, |g, w| {
                    if g > 0.0 { g * g * 1.2 / w } else { 0.0 }
                });
        let variance = square - mean * mean;
        assert!(
            close(term.variance, variance) && close(term.slope2, slope2),
            "{own} → {onward}: {term:?} against {variance} and {slope2}"
        );
    }
}

/// The bits of the graded forms on both sides of the wall's cap, which no golden world's valleys
/// reach (the reference Mars's walls are all 5 G): the graded and trunk voids and the trunk term at
/// four pairs of depths, and a bent groove with its wall at its cap, its depths set to 5 and 6 km on
/// one of the epoch Mars's trunks, at points across its course (R09.T6.h).
#[test]
fn the_graded_forms_match_their_golden() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let field = mars_epoch();
    let body = ChannelBody::of(field.header());
    let (radius_m, level) = (body.radius.value(), body.level);
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for (own, onward) in [
        (150.0, 420.0),
        (3000.0, 6000.0),
        (6000.0, 3000.0),
        (200.0, 0.0),
    ] {
        let valley = EpochValley::of(
            Metres::new(1e4),
            Metres::new(120.0),
            Metres::ZERO,
            Metres::new(own),
            body,
        );
        out.line(&format!("valley {own} m into {onward} m"));
        out.f64("graded void", valley.graded_void_m2(Metres::new(onward)));
        out.f64("trunk void", valley.trunk_void_m2(Metres::new(onward)));
        let term = epoch_trunk_term(level, 1e-5, &valley, onward);
        out.f64("variance", term.variance);
        out.f64("slope2", term.slope2);
    }
    let (cell, trunk) = bent_trunks(field).swap_remove(0);
    let mut deep = trunk.clone();
    let Kind::Trunk {
        valley_m,
        receiver_valley_m,
        ..
    } = &mut deep.kind
    else {
        panic!("a trunk");
    };
    *valley_m = 5000.0;
    *receiver_valley_m = Some(6000.0);
    let groove = Groove::of(&deep, radius_m).expect("a deep valley");
    let cap = VALLEY_WALL_SHARE * mean_cell_width(radius_m, level);
    assert_eq!(bits(groove.wall_m), bits(cap), "the wall at its cap");
    out.line(&format!("a deep groove on the trunk of {cell:?}"));
    let points = deep.bend_m().expect("a bent trunk");
    for k in [3, 2 * BEND_PARTS - 3] {
        let p = points[k];
        let up = unit(p);
        let east = unit(cross([0.0, 0.0, 1.0], up));
        for x in [0.0, 0.3, 0.6] {
            let q = [0, 1, 2].map(|i| p[i] + x * cap * east[i]);
            let value = groove.at(q);
            out.f64(&format!("point {k} at {x} of the wall"), value.value);
            for (i, g) in value.gradient.iter().enumerate() {
                out.f64(&format!("  gradient {i}"), *g);
            }
        }
    }
    golden!("height/graded", &out.finish());
}
