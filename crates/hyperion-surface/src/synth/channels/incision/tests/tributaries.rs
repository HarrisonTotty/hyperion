//! R09.T6.i's wet-now tributaries on the synthetic Earth-like world: accordant joins with each head
//! at its own relief, walls no steeper than the design cap, joins at the junction angle, bent and
//! graded courses, a cut continuous across coarse cells, and the gradient, the cache and the order
//! as before.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::synth::channels::{BEND_PARTS, JUNCTION_ANGLE_COT, KeyPoint, distance};

/// The unit vector of `v`.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    v.map(|x| x / n)
}

/// Every tributary below about `count` of `field`'s cells carrying tributaries, levels L + 1 to
/// L + `depth`, with its cell, in order.
fn tributaries_below(field: &CoarseField, count: usize, depth: u8) -> Vec<(PatchKey, Segment)> {
    let channels = Channels::new(field.header(), SEED);
    let first = channels.first_level();
    let mut network = channels.network(field);
    let mut out = Vec::new();
    for coarse in with_tributaries(field, count) {
        for level in first + 1..=first + depth {
            for cell in below(coarse, level) {
                if let Some(segment) = network.segment(cell).unwrap() {
                    out.push((cell, segment));
                }
            }
        }
    }
    out
}

/// R09.T6.i's (a): each tributary's bed at its join is its parent's there, bit for bit, and its
/// depth below its inherited top falls from its parent's at the join to its own relief at its head,
/// the rise from its cell's channel heads to its key point, wherever φ is above 1 (and to more than
/// it where its own rise is short of it, φ held at 1).
#[test]
fn tributaries_are_accordant_and_reach_their_own_relief_at_their_heads() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let (mut scaled, mut total) = (0_u32, 0_u32);
    for (cell, segment) in tributaries_below(field, 6, 3) {
        let Outlet::Join { parent, at } = segment.outlet() else {
            panic!("a tributary");
        };
        let parent_segment = network.segment(parent).unwrap().expect("a parent");
        assert_eq!(
            bits(segment.bed_to().value()),
            bits(parent_segment.bed_at(at).value()),
            "{cell:?}"
        );
        let Kind::Tributary {
            steepness,
            head_m,
            top_m,
            scale,
            ..
        } = segment.kind
        else {
            panic!("a tributary");
        };
        let heads = main_stream_length(channel_head_area(steepness)).value();
        let relief = if head_m > heads {
            rise(steepness, heads, head_m)
        } else {
            0.0
        };
        let head_depth = top_m - segment.bed_from().value();
        total += 1;
        if scale > 1.0 {
            scaled += 1;
            assert!(
                (head_depth - relief).abs() <= 1e-9 * (1.0 + relief.abs()) + 1e-9,
                "{cell:?}: head {head_depth} m against its relief {relief} m"
            );
        } else {
            // Its own rise is more than its join's depth leaves above its relief.
            assert!(
                head_depth <= relief + 1e-9 * (1.0 + relief.abs()),
                "{cell:?}"
            );
        }
    }
    eprintln!("{scaled} of {total} tributaries scaled");
    assert!(scaled > 0, "{scaled} of {total} scaled");
}

/// R09.T6.i's (c): where the join is not held at its parent's ends, it lies d cot 72° downstream
/// along its parent's course from the key point's projection on the chosen chord's line, d the
/// distance to the line: the junction angle on a straight parent.
#[test]
fn a_join_lies_downstream_of_its_foot_by_the_junction_angle() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let mut held = 0;
    let mut moved = 0;
    for (cell, segment) in tributaries_below(field, 4, 2) {
        let Outlet::Join { parent, at } = segment.outlet() else {
            panic!("a tributary");
        };
        if at <= 0.0 || at >= 1.0 {
            held += 1;
            continue;
        }
        let parent_segment = network.segment(parent).unwrap().expect("a parent");
        let q = segment.from_m();
        // The nearest chord, and the shift from its foot.
        let (chord, _) = (0..parent_segment.chord_count())
            .map(|k| {
                let chord = parent_segment.chord(k);
                let (d2, ..) = nearest_of(q, chord);
                (chord, d2)
            })
            .fold(None, |best: Option<(Chord, f64)>, (c, d2)| match best {
                Some((_, b)) if b <= d2 => best,
                _ => Some((c, d2)),
            })
            .unwrap();
        let ab = [0, 1, 2].map(|k| chord.to[k] - chord.from[k]);
        let aq = [0, 1, 2].map(|k| q[k] - chord.from[k]);
        let along = dot(aq, ab) / dot(ab, ab);
        let foot = [0, 1, 2].map(|k| chord.from[k] + along * ab[k]);
        let d = distance(q, foot);
        let t_foot = chord.span.0 + (chord.span.1 - chord.span.0) * along;
        let shift_m = (at - t_foot) * parent_segment.length().value();
        assert!(
            (shift_m - d * JUNCTION_ANGLE_COT).abs() <= 1e-6 * (1.0 + d),
            "{cell:?}: shifted {shift_m} m for {d} m"
        );
        moved += 1;
    }
    eprintln!("{moved} joins moved, {held} held at their parent's ends");
    assert!(moved > held, "{moved} moved, {held} held");
}

/// R09.T6.i's (d): every trunk and tributary of a world wet now is bent, its ends the straight
/// segment's bit for bit; a trunk into a dry receiver grades its valley to the receiver's.
#[test]
fn a_wet_world_s_courses_are_bent_with_their_ends_kept() {
    let field = earth();
    let mut graded = 0;
    for (cell, segment) in tributaries_below(field, 4, 2) {
        let points = segment.bend_m().expect("a bent tributary");
        assert_eq!(points.len(), BEND_PARTS + 1);
        assert_eq!(points[0].map(bits), segment.from_m().map(bits), "{cell:?}");
        assert_eq!(points[BEND_PARTS].map(bits), segment.to_m().map(bits));
    }
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    for coarse in with_tributaries(field, 40) {
        let segment = network.segment(coarse).unwrap().expect("a trunk");
        let points = segment.bend_m().expect("a bent trunk");
        assert_eq!(points[0].map(bits), segment.from_m().map(bits));
        assert_eq!(
            points[BEND_PARTS].map(bits),
            segment.crossing_m().unwrap().map(bits)
        );
        assert_eq!(points[2 * BEND_PARTS].map(bits), segment.to_m().map(bits));
        if let Kind::Trunk {
            receiver_valley_m: Some(_),
            ..
        } = segment.kind
        {
            graded += 1;
        }
    }
    assert!(graded > 20, "{graded} graded trunks");
}

/// The steepest of `groove`'s walls across the middle of each of its sub-chords of half `half`: its
/// central difference over a thousandth of its widest wall at 39 points across it.
fn steepest_wall(groove: &Groove, half: usize) -> f64 {
    let bent = groove.bent.as_ref().expect("bent");
    let mut steepest = 0.0_f64;
    for chord in bent.bend.chords(half) {
        let middle = [0, 1, 2].map(|k| f64::midpoint(chord.from[k], chord.to[k]));
        let along = [0, 1, 2].map(|k| chord.to[k] - chord.from[k]);
        let across = unit(cross(along, middle));
        for n in 1..40 {
            let x = groove.floor_m + groove.wall_m * f64::from(n) / 40.0;
            let h = 1e-3 * groove.wall_m;
            let at = |s: f64| {
                groove
                    .at([0, 1, 2].map(|k| middle[k] + s * across[k]))
                    .value
            };
            steepest = steepest.max(((at(x + h) - at(x - h)) / (2.0 * h)).abs());
        }
    }
    steepest
}

/// R09.T6.i's (b): no wall is steeper than [`VALLEY_WALL_GRADIENT`], trunks' and tributaries'
/// alike: across the middle of each of a groove's sub-chords, the groove's central difference over
/// a thousandth of its wall stays at or under 0.6, the trunks' along both their chords, the second
/// graded to their receivers' valleys.
#[test]
fn no_wall_is_steeper_than_the_design_cap() {
    let field = earth();
    let radius_m = field.header().radius().value();
    let mut tributaries = 0.0_f64;
    for (_, segment) in tributaries_below(field, 4, 2).into_iter().step_by(7) {
        if let Some(groove) = Groove::of(&segment, radius_m) {
            tributaries = tributaries.max(steepest_wall(&groove, 0));
        }
    }
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let mut trunks = 0.0_f64;
    for coarse in with_tributaries(field, 12) {
        let segment = network.segment(coarse).unwrap().expect("a trunk");
        if let Some(groove) = Groove::of(&segment, radius_m) {
            trunks = trunks.max(steepest_wall(&groove, 0).max(steepest_wall(&groove, 1)));
        }
    }
    eprintln!("the steepest wall: {tributaries:.4} of a tributary, {trunks:.4} of a trunk");
    for steepest in [tributaries, trunks] {
        assert!(
            steepest <= VALLEY_WALL_GRADIENT * (1.0 + 1e-3),
            "{steepest}"
        );
        assert!(steepest > 0.3, "{steepest}");
    }
}

/// The incision's analytic gradient under R09.T6.i is its central difference, as below it.
#[test]
fn the_tributaries_gradient_is_the_incision_s_central_difference() {
    let field = earth();
    let (good, total) = central_differences(field, with_tributaries(field, 5));
    assert!(
        f64::from(good) >= 0.97 * f64::from(total),
        "{good} of {total}"
    );
}

/// R09.T6.i's (e): a coarse cell's continuity is the same along each of its edges from either side,
/// each corner the mean of the cells that share it: at 9 points along each edge shared with an
/// edge neighbour on the same face, the steepness, the runoff and the cut interpolated in each cell
/// agree to 10⁻¹² of their size.
#[test]
fn the_continuity_agrees_across_a_coarse_cell_s_edges() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    // A coarse cell's side in a key point's integer face coordinates, 2⁵⁰ a face.
    let unit = 1_u64 << (50 - field.header().level().get());
    let mut checked = 0;
    for c in with_tributaries(field, 12) {
        // The integer face coordinates of the point `along` the cell's (i + 1) or (j + 1) edge.
        let on_edge = |edge: Edge, along: u64| match edge {
            Edge::UMax => (u64::from(c.i() + 1) * unit, u64::from(c.j()) * unit + along),
            Edge::VMax => (u64::from(c.i()) * unit + along, u64::from(c.j() + 1) * unit),
            Edge::UMin | Edge::VMin => unreachable!("the cell's far edges alone"),
        };
        for edge in [Edge::UMax, Edge::VMax] {
            let across = c.edge_neighbour(edge);
            if across.face() != c.face() {
                continue;
            }
            let (ours, theirs) = (
                network.continuity(c).unwrap(),
                network.continuity(across).unwrap(),
            );
            for k in 0..=8_u64 {
                let (s, t) = on_edge(edge, (unit / 8) * k);
                let key = KeyPoint {
                    face: c.face(),
                    s,
                    t,
                };
                let (a, b) = (ours.at(key, c), theirs.at(key, across));
                for (x, y) in [
                    (a.steepness, b.steepness),
                    (a.runoff_m_per_year, b.runoff_m_per_year),
                    (a.level, b.level),
                ] {
                    assert!(
                        (x - y).abs() <= 1e-12 * (1.0 + x.abs()),
                        "{c:?} and {across:?} at {k} eighths: {x} against {y}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "{checked} points");
}

/// R09.T6.i's (e): the cut's area in closed form inverts the channel's size: a channel draining
/// [`cut_area_m2`](crate::synth::channels::cut_area_m2) of a runoff and a width is that wide, to
/// 10⁻⁹, below, inside and beyond the forming discharge's fitted range, and a cell shedding none
/// cuts no area.
#[test]
fn the_cut_s_area_is_the_channel_s_size_inverted() {
    use crate::synth::channels::{channel_size, cut_area_m2};

    let gravity = ChannelBody::of(earth().header()).gravity;
    let (mut low, mut high) = (f64::INFINITY, 0.0_f64);
    for runoff in [0.01, 0.2, 0.9, 3.0] {
        for width in [0.3, 1.0, 4.0, 30.0, 400.0] {
            let area = cut_area_m2(runoff, Metres::new(width), gravity);
            let (_, drawn, _) = channel_size(runoff, SquareMetres::new(area), gravity);
            assert!(
                (drawn / width - 1.0).abs() <= 1e-9,
                "{runoff} m a⁻¹, {width} m: {area} m² draws {drawn} m"
            );
            low = low.min(area);
            high = high.max(area);
        }
    }
    // The cases reach both of the ratio's held ends (5.4 km² and 2.964 × 10⁶ km²).
    assert!(low < 5.4e6 && high > 2.964e12, "{low} to {high} m²");
    assert_eq!(cut_area_m2(0.0, CUT_WIDTH, gravity), f64::INFINITY);
}

/// R09.T6.i's (e): the continuous cut's whole part is the cut's level, [`deepest_level`], on every
/// dry cell of the Earth-like world carrying tributaries, but within 10⁻⁹ of a whole level.
#[test]
fn the_continuous_cut_s_whole_part_is_the_cut() {
    let field = earth();
    let body = ChannelBody::of(field.header());
    let level = field.header().level();
    let mut checked = 0;
    for index in 0..level.cell_count() {
        let record = field
            .cell(cell_at_index(level.get(), index).unwrap())
            .unwrap();
        if record.is_under_water() || tributary_steepness_of(record).is_none() {
            continue;
        }
        let cut = cut_level(record, body);
        let whole = deepest_level(record, body);
        let near_whole = (cut - cut.round()).abs() <= 1e-9;
        assert!(
            (f64::from(whole) - cut.floor()).abs() < 0.5 || near_whole,
            "cut {cut} against level {whole}"
        );
        checked += 1;
    }
    assert!(checked > 1000, "{checked} cells");
}

/// The closed form under R09.T6.i is within a factor of two of the realised incision's variance
/// below a coarse cell at the median, as R09.T6.b's is below it.
#[test]
fn the_bent_closed_form_is_within_a_factor_of_two_of_the_realised() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut cache = SynthCache::new();
    let mut ratios = Vec::new();
    for c in with_tributaries(field, 8) {
        let (realised, _) = realised_variance(&channels, field, &mut cache, c, 32);
        let closed = unresolved_channel_variance(field, c, Metres::new(1e7))
            .unwrap()
            .value();
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

/// The mean over a chord whose depth runs from `from` to `to` along `profile` (0 to 1 → 0 to 1) of
/// `integrand(G, W)`, W = 2.5 G, by a fine midpoint sum.
fn fine_wet_mean(
    from: f64,
    to: f64,
    profile: impl Fn(f64) -> f64,
    integrand: impl Fn(f64, f64) -> f64,
) -> f64 {
    let n = 200_000_u32;
    let mut sum = 0.0;
    for k in 0..n {
        let s = (f64::from(k) + 0.5) / f64::from(n);
        let g = from + (to - from) * profile(s);
        sum += integrand(g, WET_WALL_PER_DEPTH * g);
    }
    sum / f64::from(n)
}

/// R09.T6.i's closed forms are the integrals they stand for, to 10⁻⁹: a tributary level's groove
/// along its linear profile ([`Moments::linear`], [`wet_across`]) and the trunk level's halves,
/// graded by the smoothstep ([`wet_trunk_term`]), with walls 2.5 G.
#[test]
fn the_wet_closed_forms_are_their_integrals() {
    let close = |closed: f64, fine: f64| (closed / fine - 1.0).abs() <= 1e-9;
    let h = 2.5;
    for (from, to) in [(12.0, 180.0), (300.0, 40.0), (90.0, 90.0), (0.4, 450.0)] {
        let across = wet_across(h, Moments::linear(from, to));
        let line = |s: f64| s;
        let mean = fine_wet_mean(from, to, line, |g, w| 2.0 * g * (h + 0.5 * w));
        let square = fine_wet_mean(from, to, line, |g, w| 2.0 * g * g * (h + 13.0 * w / 35.0));
        let slope2 = fine_wet_mean(from, to, line, |g, w| g * g * 1.2 / w);
        assert!(
            close(across.mean, mean)
                && close(across.square, square)
                && close(across.slope2, slope2),
            "{from} → {to}: {across:?} against {mean}, {square} and {slope2}"
        );
    }
    let cell_m = 36_000.0;
    let halves = [
        TrunkHalf {
            depth_m: 210.0,
            end_m: 210.0,
            floor_m: 14.0,
            fade: INTO_WATER_FADE,
        },
        TrunkHalf {
            depth_m: 380.0,
            end_m: 210.0,
            floor_m: 9.0,
            fade: UNFADED,
        },
    ];
    let term = wet_trunk_term(8, &halves, cell_m);
    let density = 0.5 * MEAN_TRUNK_LENGTH * WET_TRUNK_SINUOSITY / cell_m;
    let smooth = |s: f64| s * s * (3.0 - 2.0 * s);
    let (mut mean, mut square, mut slope2) = (0.0, 0.0, 0.0);
    for half in &halves {
        let (g0, g1, h) = (half.depth_m, half.end_m, half.floor_m);
        mean += half.fade.mean
            * density
            * fine_wet_mean(g0, g1, smooth, |g, w| 2.0 * g * (h + 0.5 * w));
        square += half.fade.mean_square
            * density
            * fine_wet_mean(g0, g1, smooth, |g, w| 2.0 * g * g * (h + 13.0 * w / 35.0));
        slope2 +=
            half.fade.mean_square * density * fine_wet_mean(g0, g1, smooth, |g, w| g * g * 1.2 / w);
    }
    let variance = square - mean * mean;
    assert!(
        close(term.variance, variance) && close(term.slope2, slope2),
        "{term:?} against {variance} and {slope2}"
    );
}

/// No step crosses a tributary's bend: about each interior point of a bent tributary's course, on
/// circles at two distances inside its walls, its groove changes between neighbouring samples by
/// no more than the steeper of their analytic slopes allows over their spacing (R09.T6.h's test,
/// on R09.T6.i's tributaries, their depth falling fast along them: each sub-chord reads its depth
/// on its own line, [`Ends::OnLine`]).
#[test]
fn no_step_crosses_a_tributary_s_bend() {
    let field = earth();
    let radius_m = field.header().radius().value();
    let samples = 360_u32;
    let mut circles = 0;
    for (cell, segment) in tributaries_below(field, 4, 2).into_iter().step_by(7) {
        let Some(groove) = Groove::of(&segment, radius_m) else {
            continue;
        };
        let points = segment.bend_m().expect("a bent tributary");
        for &centre in &points[1..BEND_PARTS] {
            let up = unit(centre);
            let east = unit(cross([0.0, 0.0, 1.0], up));
            let north = cross(up, east);
            let wall = groove.at(centre).value;
            if wall <= 0.0 {
                continue;
            }
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
                                "{cell:?}: a step of {} m about a bend over {} m",
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
    assert!(circles > 200, "{circles} circles");
}

/// The synthesis's heights and gradients at band levels 12 and 16 at four points across a course's
/// sub-chord from `a` to `b` (spheroid points, metres), from its start to 400 m off it, as bits.
fn heights_across(
    out: &mut hyperion_testkit::golden::GoldenWriter,
    field: &CoarseField,
    a: [f64; 3],
    b: [f64; 3],
) {
    let figure = field.header().figure();
    let synth = crate::synth::Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    let across = unit(cross([0, 1, 2].map(|k| b[k] - a[k]), a));
    for offset in [0.0, 60.0, 180.0, 400.0] {
        let p = [0, 1, 2].map(|k| a[k] + offset * across[k]);
        let dir = crate::cube::unit_dir([
            p[0] / figure.equatorial_radius_m,
            p[1] / figure.equatorial_radius_m,
            p[2] / figure.polar_radius_m,
        ]);
        for band in [12, 16] {
            let sample = synth
                .height_at(&mut cache, dir, crate::synth::BandLevel::new(band).unwrap())
                .unwrap();
            out.f64(&format!("  {offset} m off, band {band}"), sample.height_m);
            for (axis, g) in sample.gradient.iter().enumerate() {
                out.f64(&format!("    gradient {axis}"), *g);
            }
        }
    }
}

/// Five tributaries of the Earth-like world under R09.T6.i's rules, as bits: each one's cell, φ,
/// weight, join, beds at its join and head, length, its bent course's vertices and the heights
/// across its middle; a graded wet trunk's heights; and five cells' closed forms (an extension
/// golden: the height goldens hold the network at R09.T6.h's below 26).
#[test]
fn five_tributaries_match_their_golden() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let field = earth();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line("the synthetic Earth-like world under R09.T6.i's rules");
    // Two whose rise φ scales, one whose continuous cut weights it, and two held at φ = 1 that
    // join inside their parents, the first of each in order.
    let all = tributaries_below(field, 6, 3);
    let pick = |test: &dyn Fn(&Segment) -> bool, count: usize| {
        all.iter()
            .filter(|(_, segment)| test(segment))
            .take(count)
            .cloned()
            .collect::<Vec<_>>()
    };
    let shares = |segment: &Segment| match segment.kind {
        Kind::Tributary { scale, weight, .. } => (scale, weight),
        Kind::Trunk { .. } => panic!("a tributary"),
    };
    let inside = |segment: &Segment| match segment.outlet() {
        Outlet::Join { at, .. } => at > 0.0 && at < 1.0,
        Outlet::Cell { .. } => false,
    };
    let chosen = [
        pick(&|segment| shares(segment).0 > 1.0, 2),
        pick(&|segment| shares(segment).1 < 1.0, 1),
        pick(&|segment| shares(segment).0 <= 1.0 && inside(segment), 2),
    ];
    assert!(
        chosen.iter().all(|list| !list.is_empty()),
        "every case drawn"
    );
    for (cell, segment) in chosen.into_iter().flatten() {
        let Outlet::Join { parent, at } = segment.outlet() else {
            panic!("a tributary");
        };
        let Kind::Tributary { scale, weight, .. } = segment.kind else {
            panic!("a tributary");
        };
        out.line(&format!("tributary of {cell:?} into {parent:?}"));
        out.f64("phi", scale);
        out.f64("weight", weight);
        out.f64("join at", at);
        out.f64("bed at the join", segment.bed_to().value());
        out.f64("bed at the head", segment.bed_from().value());
        out.f64("length", segment.length().value());
        let points = segment.bend_m().expect("bent");
        for (k, p) in points.iter().enumerate() {
            for (axis, x) in p.iter().enumerate() {
                out.f64(&format!("  vertex {k} axis {axis}"), *x);
            }
        }
        let middle = BEND_PARTS / 2;
        heights_across(&mut out, field, points[middle], points[middle + 1]);
    }
    // A graded wet trunk's heights across its second chord's middle.
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let (cell, trunk) = with_tributaries(field, 40)
        .into_iter()
        .find_map(|c| {
            let segment = network.segment(c).unwrap()?;
            matches!(
                segment.kind,
                Kind::Trunk {
                    receiver_valley_m: Some(_),
                    ..
                }
            )
            .then_some((c, segment))
        })
        .expect("a graded trunk");
    out.line(&format!("the graded trunk of {cell:?}"));
    let points = trunk.bend_m().expect("bent");
    let middle = BEND_PARTS + BEND_PARTS / 2;
    heights_across(&mut out, field, points[middle], points[middle + 1]);
    // And the closed forms of five cells carrying tributaries: the variance the synthesis books
    // and its structure function at a kilometre.
    for c in with_tributaries(field, 5) {
        out.line(&format!("closed forms of {c:?}"));
        out.f64(
            "variance",
            unresolved_channel_variance(field, c, Metres::new(1e7))
                .unwrap()
                .value(),
        );
        out.f64(
            "structure function at 1 km",
            channel_structure_function(field, c, Metres::new(1e7), Metres::new(1e3))
                .unwrap()
                .value(),
        );
    }
    golden!("height/tributaries", &out.finish());
}
