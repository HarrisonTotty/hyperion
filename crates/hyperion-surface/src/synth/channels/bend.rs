//! A trunk's bent course: its two straight chords split into [`BEND_PARTS`] parts each, the new
//! points on a spline through its neighbours' ends and displaced across the chord, in two passes
//! (plan R09, R09.T6.h).
//!
//! It is Dendry's own step (Gaillard et al. 2019's reference code, `SubdivideSegments` and
//! `DisplaceSegments` in `NoiseLib/include/noise.h` of github.com/mgaillard/Noise, built from their
//! description as R09.T6.a was; GPL-3.0, no code taken), which T6.a's chords left out, taken twice
//! as Dendry's levels take it (the [`half`] of a course). Each chord A → B is split into parts and
//! its interior points placed on a Catmull–Rom spline through the course's ends before and after
//! it: the first chord, from the key point K to the crossing C, through (the largest inflow's
//! crossing, K, C, K′), and the second, from C to the receiver's key point K′, through (K, C, K′,
//! C′), C′ the receiver's own crossing. Where there is no inflow or no onward trunk, Dendry's
//! reflected point, 2A − B before or 2B − A after, keeps the tangent along the chord. Each interior
//! point is then moved across its chord by u [`BEND_FACTOR`] times the chord turned 90°, u uniform
//! in [−1, 1) on `surface.channel.bend`, and each part split again on the spline through the first
//! pass's points, displaced by a quarter of the factor. The spline rounds the corners the
//! four-neighbour routing turns at the key points, and the displacement adds the wander below one
//! run; the course's length over the whole valley is the network's own (the reference Mars's drawn
//! courses already have Mars's sinuosity at the scale of whole valleys).
//!
//! Each half is built in its own cell's face plane, the gnomonic plane of the cube face its cell
//! lies on, where the chord between two directions is a straight line and the cell a rectangle (its
//! edges are great circles), and every new point is held inside the cell's rectangle shrunk by
//! [`CELL_MARGIN`]. So each sub-chord lies in its half's cell by convexity: the first half in the
//! trunk's cell and the second in its receiver's, even across a face edge, the crossing lying on
//! the edge in both faces' planes. The ends stay the straight trunk's, bit for bit: the key point,
//! the crossing and the receiver's key point.
//!
//! At a confluence the course's own inflow is the largest, by drainage and then by the higher
//! packed cell key, where Dendry's spline needs exactly one segment ending at the point and leaves
//! a confluence's corner sharp; so the main course stays smooth through every key point
//! (a deviation, the plan's records).
//!
//! The arithmetic is basic operations and square roots alone, the minimum and maximum through
//! [`num`], every value asserted finite, and the draws by counter, so that a course is the same
//! bits natively and as WebAssembly, whatever was computed first.

use hyperion_base::rng::Stream;

use crate::cube::{Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir, uv_on_face};
use crate::num;
use crate::spheroid::Spheroid;
use crate::synth::interp::axes;

use super::{Chord, distance, lerp};

/// D, the parts each of a bent trunk's two chords is split into: 16, four by Dendry's first pass,
/// each split again into four by a second, as Dendry's levels repeat the step, so that a part is
/// about 1.2 km on the reference Mars's 39 km runs (8 pixels at the look's 150 m). A design choice,
/// the look's (R09.T6.h): one pass of 8 left a sawtooth of kinks of about 30° every 2.5 km, and two
/// of 4 and 2 elbows of about 15°; this one costs about a quarter more in the groove than D = 8.
pub const BEND_PARTS: usize = FIRST_PARTS * SECOND_PARTS;

/// The parts of a chord's first pass, 4: Dendry's examples' (`SubdivideSegments`).
const FIRST_PARTS: usize = 4;

/// The parts each first-pass part is split into by the second pass, 4.
const SECOND_PARTS: usize = 4;

/// The points of a bent course, 2D + 1: the key point, the first chord's D − 1, the crossing, the
/// second chord's D − 1 and the receiver's key point.
pub(crate) const COURSE_POINTS: usize = 2 * BEND_PARTS + 1;

/// f, the largest displacement of a first-pass point of a bent course across its chord, as a share
/// of the chord: 0.1, and a quarter of it for a second-pass point across its part.
///
/// Calibrated so that the bent courses' mean length over their straight chords', `s_net`, is 1.03 to
/// 1.07 on the reference Mars with the spline in: 1.0535 there and 1.0505 on the Mars-like world
/// (R09.T6.h's measurement), the run's scale of Mars's valleys: a labelled extrapolation from Som,
/// Montgomery and Greenberg 2009 (JGR 114, E02005, doi:10.1029/2008JE003132, Table 2: ten large
/// Martian valleys' sinuosity 1.10–1.57, median 1.14, valley lengths 160–885 km, traced on 460 m
/// MOLA). Through the fractal length s = (R ÷ ε)^(d − 1) (Mandelbrot 1967, Science 156, 636, not
/// read here), d − 1 = ln s ÷ ln(R ÷ ε) derived here from Table 2 with R the length ÷ the
/// sinuosity and ε = 460 m is 0.014–0.063, median 0.019, which gives about 1.07 over 40 km at
/// 1 km. Dendry's examples pass factors of 0.025 to 0.1, its 0.075 at D = 4 adding about 2% of
/// length. The reference Mars's whole courses 160–885 km long then have a median sinuosity inside
/// Som et al.'s range (`trunk_bend_worlds`).
pub const BEND_FACTOR: f64 = 0.1;

/// The share of its cell's side, in the face coordinates (s, t), by which a bent course's new point
/// keeps inside its cell: 1/32, so that every sub-chord lies inside its half's cell.
const CELL_MARGIN: f64 = 1.0 / 32.0;

/// 2⁻⁵³, exactly: a uniform's step.
const TWO_POW_MINUS_53: f64 = 1.0 / 9_007_199_254_740_992.0;

/// The ends a bent trunk's course runs through, as unit directions in the body-fixed frame, with the
/// cells its halves lie in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CourseEnds {
    /// The trunk's coarse cell, which holds its first half.
    pub(crate) cell: PatchKey,
    /// Its receiver, which holds its second half.
    pub(crate) receiver: PatchKey,
    /// The largest inflow's crossing into the cell, `None` where none flows in.
    pub(crate) before: Option<[f64; 3]>,
    /// The key point K.
    pub(crate) key: [f64; 3],
    /// The crossing C.
    pub(crate) crossing: [f64; 3],
    /// The receiver's key point K′.
    pub(crate) onward: [f64; 3],
    /// The receiver's own crossing C′, `None` where it heads no trunk onward.
    pub(crate) after: Option<[f64; 3]>,
}

/// A bent trunk's course: its points, spheroid points in the body-fixed frame (metres), and each
/// point's parameter along it, 0 at the key point and 1 at the receiver's key point in proportion
/// to the length along the course.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bend {
    points: [[f64; 3]; COURSE_POINTS],
    at: [f64; COURSE_POINTS],
    length_m: f64,
}

impl Bend {
    /// The course through `ends` on `figure`, its ends the spheroid points `key_m`, `crossing_m`
    /// and `onward_m` (the straight trunk's, bit for bit), displaced by the words of `stream` and
    /// `factor` ([`BEND_FACTOR`]; see the module documentation).
    ///
    /// # Panics
    ///
    /// If a point is not finite or lies behind a half's face plane (a bug: every point is within
    /// two cells of the half's own).
    #[must_use]
    pub(crate) fn new(
        ends: &CourseEnds,
        [key_m, crossing_m, onward_m]: [[f64; 3]; 3],
        figure: Spheroid,
        stream: &Stream,
        factor: f64,
    ) -> Self {
        let first = half(
            ends.cell,
            [
                ends.before,
                Some(ends.key),
                Some(ends.crossing),
                Some(ends.onward),
            ],
            stream,
            0,
            factor,
        );
        let second = half(
            ends.receiver,
            [
                Some(ends.key),
                Some(ends.crossing),
                Some(ends.onward),
                ends.after,
            ],
            stream,
            1,
            factor,
        );
        let mut points = [[0.0; 3]; COURSE_POINTS];
        points[0] = key_m;
        points[BEND_PARTS] = crossing_m;
        points[COURSE_POINTS - 1] = onward_m;
        for k in 1..BEND_PARTS {
            points[k] = figure.point(first[k - 1]);
            points[BEND_PARTS + k] = figure.point(second[k - 1]);
        }
        let mut along = [0.0; COURSE_POINTS];
        for k in 1..COURSE_POINTS {
            along[k] = along[k - 1] + distance(points[k - 1], points[k]);
        }
        let length_m = num::assert_finite(along[COURSE_POINTS - 1]);
        assert!(length_m > 0.0, "a trunk's course has a length");
        // 0 and 1 exactly at the ends.
        let at = along.map(|l| l / length_m);
        Self {
            points,
            at,
            length_m,
        }
    }

    /// Its points, the key point first and the receiver's key point last.
    #[must_use]
    pub(crate) const fn points(&self) -> &[[f64; 3]; COURSE_POINTS] {
        &self.points
    }

    /// Each point's parameter along it, 0 to 1.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn params(&self) -> &[f64; COURSE_POINTS] {
        &self.at
    }

    /// Its length along its sub-chords, metres.
    #[must_use]
    pub(crate) const fn length_m(&self) -> f64 {
        self.length_m
    }

    /// The parameter at its crossing, where it leaves its cell.
    #[must_use]
    pub(crate) const fn exit(&self) -> f64 {
        self.at[BEND_PARTS]
    }

    /// The point at parameter `t` in [0, 1], on the sub-chord that spans it.
    #[must_use]
    pub(crate) fn point_m(&self, t: f64) -> [f64; 3] {
        let k = self.part_of(t);
        let (a, b) = (self.at[k], self.at[k + 1]);
        if b > a {
            lerp(self.points[k], self.points[k + 1], (t - a) / (b - a))
        } else {
            self.points[k]
        }
    }

    /// The sub-chord whose parameters span `t`, the first of two that share it.
    #[must_use]
    fn part_of(&self, t: f64) -> usize {
        (0..COURSE_POINTS - 2)
            .find(|&k| t <= self.at[k + 1])
            .unwrap_or(COURSE_POINTS - 2)
    }

    /// Its sub-chords of half `half` (0 to the crossing, 1 beyond), in order, each with the
    /// parameters it spans.
    pub(crate) fn chords(&self, half: usize) -> impl Iterator<Item = Chord> + '_ {
        (half * BEND_PARTS..(half + 1) * BEND_PARTS).map(|k| Chord {
            from: self.points[k],
            to: self.points[k + 1],
            span: (self.at[k], self.at[k + 1]),
        })
    }
}

/// The D − 1 interior points of one half of a bent course, unit directions: half `index` (0 the
/// first chord, 1 the second) of the course through `control`, its chord from `control[1]` to
/// `control[2]` and the points before and after, each `None` for Dendry's reflected point, in the
/// face plane of `cell`, displaced by `stream`'s words for the half and `factor`, and held inside
/// `cell`.
///
/// It is Dendry's step twice, as its levels repeat it: the chord is split into [`FIRST_PARTS`] on
/// the spline through its neighbours' ends, each new point displaced by `factor`; then each part
/// into [`SECOND_PARTS`] on the spline through the first pass's points, its tangents at the chord's
/// ends the first spline's, so that the course stays smooth through its ends, each new point
/// displaced by a quarter of `factor` across its part (Dendry's factor quartered at each finer
/// level). Vertex k of the half's D parts is the first pass's where k is a multiple of
/// [`SECOND_PARTS`] and the second's otherwise, and takes word (half)(D − 1) + k − 1.
#[must_use]
fn half(
    cell: PatchKey,
    [before, from, to, after]: [Option<[f64; 3]>; 4],
    stream: &Stream,
    index: u64,
    factor: f64,
) -> [[f64; 3]; BEND_PARTS - 1] {
    let face = cell.face();
    let on_plane = |dir: [f64; 3]| plane(face, dir);
    let a = on_plane(from.expect("a chord has its start"));
    let b = on_plane(to.expect("a chord has its end"));
    // Dendry's reflected points, 2A − B and 2B − A.
    let p0 = before.map_or([2.0 * a[0] - b[0], 2.0 * a[1] - b[1]], on_plane);
    let p3 = after.map_or([2.0 * b[0] - a[0], 2.0 * b[1] - a[1]], on_plane);
    let held = Held::of(cell);
    let parts = u32::try_from(BEND_PARTS).expect("a few parts");
    let shift =
        |k: u32| signed_uniform(stream.word_at(index * u64::from(parts - 1) + u64::from(k - 1)));
    // The first pass: FIRST_PARTS on the chord's spline, each point displaced across the chord.
    let spline = Spline::new(p0, a, b, p3);
    let first = u32::try_from(FIRST_PARTS).expect("a few parts");
    let second = u32::try_from(SECOND_PARTS).expect("a few parts");
    let mut coarse = [a; FIRST_PARTS + 1];
    coarse[FIRST_PARTS] = b;
    for (j, point) in (1_u32..).zip(coarse[1..FIRST_PARTS].iter_mut()) {
        let on = spline.at(f64::from(j) / f64::from(first));
        *point = held.displaced(on, a, b, factor * shift(second * j));
    }
    // The second: each part into SECOND_PARTS on the spline through the first pass's points, its
    // tangents there scaled to the part as the first spline's are, and at the chord's ends the
    // first spline's own.
    let share = 1.0 / f64::from(first);
    let length = |p: [f64; 2], q: [f64; 2]| {
        let (du, dv) = (q[0] - p[0], q[1] - p[1]);
        (du * du + dv * dv).sqrt()
    };
    let to_dir = |p: [f64; 2]| {
        unit_dir(face_uv_to_xyz(FaceUv {
            face,
            u: p[0],
            v: p[1],
        }))
    };
    let mut out = [[0.0; 3]; BEND_PARTS - 1];
    for j in 0..FIRST_PARTS {
        let (from, to) = (coarse[j], coarse[j + 1]);
        let part = length(from, to);
        let start = if j == 0 {
            spline.start.map(|x| share * x)
        } else {
            let scale = part / (length(coarse[j - 1], from) + part);
            [0, 1].map(|k| (to[k] - coarse[j - 1][k]) * scale)
        };
        let end = if j + 1 == FIRST_PARTS {
            spline.end.map(|x| share * x)
        } else {
            let scale = part / (part + length(to, coarse[j + 2]));
            [0, 1].map(|k| (coarse[j + 2][k] - from[k]) * scale)
        };
        let hermite = Spline {
            from,
            to,
            start,
            end,
        };
        let first_vertex = u32::try_from(j * SECOND_PARTS).expect("a few parts");
        for m in 1..second {
            let on = hermite.at(f64::from(m) / f64::from(second));
            let k = first_vertex + m;
            let at = usize::try_from(k - 1).expect("a few parts");
            out[at] = to_dir(held.displaced(on, from, to, 0.25 * factor * shift(k)));
        }
        if j + 1 < FIRST_PARTS {
            out[(j + 1) * SECOND_PARTS - 1] = to_dir(to);
        }
    }
    out
}

/// The rectangle a bent course's new points are held to in a cell's face plane: the cell's own
/// shrunk by [`CELL_MARGIN`] of its side in (s, t).
#[derive(Debug, Clone, Copy)]
struct Held {
    u: (f64, f64),
    v: (f64, f64),
}

impl Held {
    #[must_use]
    fn of(cell: PatchKey) -> Self {
        let [u_low, u_high, v_low, v_high] = held_rectangle(cell);
        Self {
            u: (u_low, u_high),
            v: (v_low, v_high),
        }
    }

    /// The point `on` moved by `shift` times the chord from `a` to `b` turned 90°, (−Δv, Δu),
    /// held inside the rectangle.
    #[must_use]
    fn displaced(self, on: [f64; 2], a: [f64; 2], b: [f64; 2], shift: f64) -> [f64; 2] {
        let across = [-(b[1] - a[1]), b[0] - a[0]];
        let u = num::assert_finite(on[0] + shift * across[0]);
        let v = num::assert_finite(on[1] + shift * across[1]);
        [
            num::max(self.u.0, num::min(u, self.u.1)),
            num::max(self.v.0, num::min(v, self.v.1)),
        ]
    }
}

/// `2 ((w >> 11) × 2⁻⁵³) − 1`, uniform in [−1, 1) and exact.
#[must_use]
fn signed_uniform(word: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a 53-bit integer, exact in f64")]
    let mark = (word >> 11) as f64;
    2.0 * (mark * TWO_POW_MINUS_53) - 1.0
}

/// The (u, v) of the unit direction `dir` on `face`'s plane, its gnomonic projection.
///
/// # Panics
///
/// If `dir` lies behind the plane or a coordinate is not finite.
#[must_use]
fn plane(face: Face, dir: [f64; 3]) -> [f64; 2] {
    let (normal, _, _) = axes(face);
    let along = normal[0] * dir[0] + normal[1] * dir[1] + normal[2] * dir[2];
    assert!(
        along > 0.0,
        "a bent course's point lies in front of its face"
    );
    let (u, v) = uv_on_face(face, dir);
    [num::assert_finite(u), num::assert_finite(v)]
}

/// The rectangle a bent course's new points are held to in `cell`'s face plane, (u low, u high,
/// v low, v high): the cell's own shrunk by [`CELL_MARGIN`] of its side in (s, t).
#[must_use]
fn held_rectangle(cell: PatchKey) -> [f64; 4] {
    let side = f64::from(1_u32 << cell.level());
    let (i, j) = (f64::from(cell.i()), f64::from(cell.j()));
    [
        st_to_uv((i + CELL_MARGIN) / side),
        st_to_uv((i + 1.0 - CELL_MARGIN) / side),
        st_to_uv((j + CELL_MARGIN) / side),
        st_to_uv((j + 1.0 - CELL_MARGIN) / side),
    ]
}

/// The Catmull–Rom spline from P1 to P2 through P0 before and P3 after, in a face plane, its
/// tangents scaled to the chord: m₁ = (P2 − P0) ℓ₁₂ ÷ (ℓ₀₁ + ℓ₁₂) and m₂ = (P3 − P1) ℓ₁₂ ÷ (ℓ₁₂ +
/// ℓ₂₃), the uniform spline's (P2 − P0) ÷ 2 and (P3 − P1) ÷ 2 where the chords are equal, so that a
/// short chord between long ones does not overshoot.
#[derive(Debug, Clone, Copy)]
struct Spline {
    from: [f64; 2],
    to: [f64; 2],
    start: [f64; 2],
    end: [f64; 2],
}

impl Spline {
    #[must_use]
    fn new(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], p3: [f64; 2]) -> Self {
        let length = |a: [f64; 2], b: [f64; 2]| {
            let (du, dv) = (b[0] - a[0], b[1] - a[1]);
            (du * du + dv * dv).sqrt()
        };
        let (l01, l12, l23) = (length(p0, p1), length(p1, p2), length(p2, p3));
        let before = l12 / (l01 + l12);
        let after = l12 / (l12 + l23);
        Self {
            from: p1,
            to: p2,
            start: [0, 1].map(|k| num::assert_finite((p2[k] - p0[k]) * before)),
            end: [0, 1].map(|k| num::assert_finite((p3[k] - p1[k]) * after)),
        }
    }

    /// Its point at `s` in [0, 1], by the cubic Hermite basis.
    #[must_use]
    fn at(&self, s: f64) -> [f64; 2] {
        let r = 1.0 - s;
        let h00 = (1.0 + 2.0 * s) * r * r;
        let h10 = s * r * r;
        let h01 = s * s * (3.0 - 2.0 * s);
        let h11 = s * s * (s - 1.0);
        [0, 1].map(|k| {
            h00 * self.from[k] + h10 * self.start[k] + h01 * self.to[k] + h11 * self.end[k]
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn the_signed_uniform_is_exact_and_in_its_range() {
        assert_eq!(bits(signed_uniform(0)), bits(-1.0));
        assert_eq!(bits(signed_uniform(1 << 63)), bits(0.0));
        let top = signed_uniform(u64::MAX);
        assert_eq!(bits(top), bits(1.0 - TWO_POW_MINUS_53 * 2.0));
    }

    #[test]
    fn the_spline_runs_from_its_start_to_its_end_and_is_straight_on_a_line() {
        let spline = Spline::new([-1.0, 0.0], [0.0, 0.0], [1.0, 0.0], [2.0, 0.0]);
        assert_eq!(spline.at(0.0).map(bits), [0.0, 0.0].map(bits));
        assert_eq!(spline.at(1.0).map(bits), [1.0, 0.0].map(bits));
        for k in 1..8 {
            let s = f64::from(k) / 8.0;
            let p = spline.at(s);
            assert!(
                (p[0] - s).abs() < 1e-15 && p[1].abs() < 1e-300,
                "{p:?} at {s}"
            );
        }
    }
}
