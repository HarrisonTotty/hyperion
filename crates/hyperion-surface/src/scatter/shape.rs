//! A rock's shape: a superquadric standing on the authoritative ground, half buried, and its top
//! along the datum's normal through a point (plan R11, Design note 5).
//!
//! # The shape
//!
//! A rock is the superquadric |x ÷ a|ⁿ + |y ÷ b|ⁿ + |z ÷ c|ⁿ ≤ 1 in its own frame: x and y
//! horizontal, along its yaw from east, and z along the datum's normal at its axis, n = 2 an
//! ellipsoid. Each is drawn per rock on `surface.scatter.shape` ([`RockShape::draw`]):
//!
//! - its exposed height over its diameter, h ÷ D, uniform on [`ROCK_HEIGHT_RATIO`];
//! - its horizontal axial ratio b ÷ a, uniform on [`ROCK_ASPECT`], about the fragment ratio
//!   b ÷ a = √2 ÷ 2 of a : b : c ≈ 2 : √2 : 1;
//! - its yaw, uniform on [0, 2π);
//! - its exponent n, uniform on [`ROCK_EXPONENT`], from an ellipsoid to a rounded block.
//!
//! Its diameter D is its visible one, as Golombek's are: the outline at the ground, of the area
//! π D² ÷ 4 the size–frequency law gives each rock ([`law`](super::law)). The superquadric's
//! outline has the area 4 a b Γ(1 + 1 ÷ n)² ÷ Γ(1 + 2 ÷ n), π a b at n = 2, so
//! a b = π D² ÷ (16 Γ(1 + 1 ÷ n)² ÷ Γ(1 + 2 ÷ n)), D² ÷ 4 for an ellipsoid and 0.889 of it at
//! n = 3 ([`outline_area_factor`]), with b ÷ a the aspect. The vertical axis follows from the exposed height: c = h, the rock's
//! equator at the ground at its axis and the half below it buried, so the exposed height above
//! R10's ground there is the drawn one, and its base, its lowest point, lies c below the ground
//! ([`Rock::burial`]). Rocks measured at the MSL sites stand 0.29–0.41 of their diameters high
//! (Design note 4), a little over half of a fragment's height of 0.5–0.7 D, so a half-buried rock
//! is near what they show.
//!
//! # The top along a normal
//!
//! [`Rock::top_above`] is the height, above the datum along its normal through a point's foot, of
//! the rock's top there: the line meets the rock's frame at the rock's equator's height, at (x, y,
//! z), and the top is c (1 − |x ÷ a|ⁿ − |y ÷ b|ⁿ)^(1 ÷ n) above it less z, the frame taken flat over
//! the rock. The datum's normals turn by at most α = reach ÷ ρ over the rock, ρ the datum's
//! smallest radius of curvature, so where the top's slope is at most 1 the flat frame's height is
//! within 2 c α of the line's exact meeting with the rock ([`Rock::contact_bound`]): at most
//! 6.3 × 10⁻⁵ m for a 6.4 m rock on a body of 500 km radius, and nothing a hull resolves. The line through the rock's axis
//! is the frame's own, so the top there is the ground's height plus c exactly.

use hyperion_base::math;
use hyperion_base::rng::Stream;
use hyperion_base::units::{Metres, Radians};

use super::ROCK_HEIGHT_RATIO;
use crate::ground::datum_foot;
use crate::num;
use crate::spheroid::Spheroid;

/// The range of a rock's horizontal axial ratio b ÷ a, drawn per rock: 0.55 to 0.9, about the
/// fragment ratio √2 ÷ 2 (Design note 5).
pub const ROCK_ASPECT: (f64, f64) = (0.55, 0.9);

/// The range of a rock's superquadric exponent, drawn per rock: 2, an ellipsoid, to 3, a rounded
/// block, a low-order superquadric (the rendering brainstorm's "an ellipsoid or a low-order
/// superquadric"; lean, 2026-11-02).
pub const ROCK_EXPONENT: (f64, f64) = (2.0, 3.0);

/// The most a rock reaches from its axis over its diameter: √(a² + b²) ÷ D, the corner of its box,
/// at the narrowest aspect and the ellipsoid's a b = D² ÷ 4, the largest
/// ([`outline_area_factor`]), (√(1 ÷ 0.55 + 0.55)) ÷ 2 = 0.76945 ([`Rock::reach`]), rounded up.
pub const MAX_REACH_SHARE: f64 = 0.7695;

/// Γ(1 + 1 ÷ n)² ÷ Γ(1 + 2 ÷ n), the share of its bounding box's area 4 a b that a superquadric of
/// exponent `n` covers: π ÷ 4 at n = 2, rising with n to 0.8833 at n = 3 and 1 as n grows.
#[must_use]
pub fn outline_area_factor(n: f64) -> f64 {
    let g = math::gamma(1.0 + 1.0 / n);
    g * g / math::gamma(1.0 + 2.0 / n)
}

/// A rock's drawn shape: its height ratio, axial ratio, yaw and exponent (see the [module
/// documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RockShape {
    height_ratio: f64,
    aspect: f64,
    yaw: Radians,
    exponent: f64,
}

impl RockShape {
    /// The shape of a height ratio h ÷ D of `height_ratio`, an axial ratio b ÷ a of `aspect`, a yaw
    /// of `yaw` from east towards north and an exponent of `exponent`.
    ///
    /// # Panics
    ///
    /// If the height ratio, the aspect or the exponent lies outside its drawn range, or the yaw is
    /// not finite.
    #[must_use]
    pub fn new(height_ratio: f64, aspect: f64, yaw: Radians, exponent: f64) -> Self {
        let within = |x: f64, (lo, hi): (f64, f64)| x >= lo && x <= hi;
        assert!(
            within(height_ratio, ROCK_HEIGHT_RATIO),
            "a rock's height ratio must lie in {ROCK_HEIGHT_RATIO:?}, got {height_ratio}"
        );
        assert!(
            within(aspect, ROCK_ASPECT),
            "a rock's aspect must lie in {ROCK_ASPECT:?}, got {aspect}"
        );
        assert!(
            within(exponent, ROCK_EXPONENT),
            "a rock's exponent must lie in {ROCK_EXPONENT:?}, got {exponent}"
        );
        assert!(
            yaw.value().is_finite(),
            "a rock's yaw must be finite, got {yaw:?}"
        );
        Self {
            height_ratio,
            aspect,
            yaw,
            exponent,
        }
    }

    /// The shape `stream` draws, four words in order: the height ratio, the aspect, the yaw and the
    /// exponent, each uniform on its range.
    #[must_use]
    pub fn draw(stream: &mut Stream) -> Self {
        let height_ratio = stream.uniform_in(ROCK_HEIGHT_RATIO.0, ROCK_HEIGHT_RATIO.1);
        let aspect = stream.uniform_in(ROCK_ASPECT.0, ROCK_ASPECT.1);
        let yaw = Radians::new(core::f64::consts::TAU * stream.uniform());
        let exponent = stream.uniform_in(ROCK_EXPONENT.0, ROCK_EXPONENT.1);
        Self::new(height_ratio, aspect, yaw, exponent)
    }

    /// h ÷ D, the exposed height over the diameter.
    #[must_use]
    pub const fn height_ratio(&self) -> f64 {
        self.height_ratio
    }

    /// b ÷ a, the horizontal axial ratio.
    #[must_use]
    pub const fn aspect(&self) -> f64 {
        self.aspect
    }

    /// The yaw of the a axis, from east towards north.
    #[must_use]
    pub const fn yaw(&self) -> Radians {
        self.yaw
    }

    /// The superquadric exponent n, 2 an ellipsoid.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }
}

/// An authoritative rock: a superquadric standing half buried on the ground at its axis (see the
/// [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub struct Rock {
    dir: [f64; 3],
    ground_m: f64,
    base_centre: [f64; 3],
    diameter: Metres,
    shape: RockShape,
    semi_axes: [f64; 3],
    up: [f64; 3],
    axes: [[f64; 3]; 2],
}

impl Rock {
    /// The rock of `diameter` and `shape` on `figure`'s body, its axis along the unit direction
    /// `dir`, where the ground is `ground_m` metres above the datum.
    ///
    /// # Panics
    ///
    /// If `dir` is not finite or is zero, the diameter not positive and finite, or the ground's
    /// height not finite.
    #[must_use]
    pub fn new(
        figure: &Spheroid,
        dir: [f64; 3],
        ground_m: f64,
        diameter: Metres,
        shape: RockShape,
    ) -> Self {
        let d = diameter.value();
        assert!(
            d.is_finite() && d > 0.0,
            "a rock's diameter must be positive and finite, got {d}"
        );
        assert!(
            dir.iter().all(|x| x.is_finite()) && dir.iter().any(|x| x.abs() > 0.0),
            "a rock's axis must be a finite direction, got {dir:?}"
        );
        let ground_m = num::assert_finite(ground_m);
        let ab = core::f64::consts::PI * d * d / (16.0 * outline_area_factor(shape.exponent));
        let semi_axes = [
            (ab / shape.aspect).sqrt(),
            (ab * shape.aspect).sqrt(),
            shape.height_ratio * d,
        ];
        let up = figure.normal(dir);
        // East, ẑ × ν, except near a pole, where x̂ × ν stands in: the yaw is drawn, so either is
        // a frame, and the switch is a pure function of the axis.
        let reference = if up[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let east = unit(cross(reference, up));
        let north = cross(up, east);
        let (sin, cos) = math::sin_cos(shape.yaw.value());
        let along = [0, 1, 2].map(|k| cos * east[k] + sin * north[k]);
        let across = [0, 1, 2].map(|k| cos * north[k] - sin * east[k]);
        Self {
            dir,
            ground_m,
            base_centre: figure.surface_point(dir, ground_m),
            diameter,
            shape,
            semi_axes,
            up,
            axes: [along, across],
        }
    }

    /// The unit direction of its axis, body-fixed.
    #[must_use]
    pub const fn dir(&self) -> [f64; 3] {
        self.dir
    }

    /// The centre of its base outline, on the ground at its axis: body-fixed, metres.
    #[must_use]
    pub const fn base_centre(&self) -> [f64; 3] {
        self.base_centre
    }

    /// The ground's height at its axis, metres above the datum: R10's ground there.
    #[must_use]
    pub const fn ground_height(&self) -> Metres {
        Metres::new(self.ground_m)
    }

    /// Its visible diameter, the outline at the ground's.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.diameter
    }

    /// Its drawn shape.
    #[must_use]
    pub const fn shape(&self) -> &RockShape {
        &self.shape
    }

    /// Its semi-axes a, b and c, metres: a and b horizontal, c vertical.
    #[must_use]
    pub fn semi_axes(&self) -> [Metres; 3] {
        self.semi_axes.map(Metres::new)
    }

    /// Its exposed height above the ground at its axis: the drawn h = (h ÷ D) D, its vertical
    /// semi-axis c.
    #[must_use]
    pub const fn exposed_height(&self) -> Metres {
        Metres::new(self.semi_axes[2])
    }

    /// The depth of its base, its lowest point, below the ground at its axis: c, the half below its
    /// equator.
    #[must_use]
    pub const fn burial(&self) -> Metres {
        Metres::new(self.semi_axes[2])
    }

    /// The most it reaches from its axis, horizontally: √(a² + b²), its box's corner, at most
    /// [`MAX_REACH_SHARE`] of its diameter.
    #[must_use]
    pub fn reach(&self) -> Metres {
        let [a, b, _] = self.semi_axes;
        Metres::new((a * a + b * b).sqrt())
    }

    /// The height, metres above the datum along its normal through the foot of the body-fixed
    /// point `at`, of the rock's top there, or `None` where that line misses the rock's outline
    /// (see the [module documentation](self)).
    ///
    /// # Panics
    ///
    /// If `at` is not finite or is the body's centre.
    #[must_use]
    pub fn top_above(&self, figure: &Spheroid, at: &[f64; 3]) -> Option<Metres> {
        let (foot, _) = datum_foot(figure, at);
        self.top_at_m(figure, foot).map(Metres::new)
    }

    /// [`top_above`](Self::top_above) along the datum's normal at the unit direction `foot`,
    /// metres.
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "the frame's x, y and z, the semi-axes a, b and c and the exponent n"
    )]
    pub(crate) fn top_at_m(&self, figure: &Spheroid, foot: [f64; 3]) -> Option<f64> {
        let q = figure.surface_point(foot, self.ground_m);
        let delta = [0, 1, 2].map(|k| q[k] - self.base_centre[k]);
        let x = dot(delta, self.axes[0]);
        let y = dot(delta, self.axes[1]);
        let z = dot(delta, self.up);
        let [a, b, c] = self.semi_axes;
        let n = self.shape.exponent;
        let s = math::powf(x.abs() / a, n) + math::powf(y.abs() / b, n);
        if s >= 1.0 {
            return None;
        }
        let top = c * math::powf(1.0 - s, 1.0 / n);
        Some(num::assert_finite(self.ground_m + top - z))
    }

    /// The flat frame's bound on [`top_above`](Self::top_above)'s error, metres, where the rock's
    /// top is no steeper than 1: 2 c α, α = [`reach`](Self::reach) ÷ ρ the most the datum's normals
    /// turn over the rock, ρ = min(a, c)² ÷ max(a, c) the datum's smallest radius of curvature (see
    /// the [module documentation](self)).
    #[must_use]
    pub fn contact_bound(&self, figure: &Spheroid) -> Metres {
        let (equatorial, polar) = (figure.equatorial_radius_m, figure.polar_radius_m);
        let small = num::min(equatorial, polar);
        let rho = small * small / num::max(equatorial, polar);
        let alpha = self.reach().value() / rho;
        Metres::new(2.0 * self.semi_axes[2] * alpha)
    }
}

/// The dot product of `a` and `b`.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product a × b.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `v` over its length.
#[must_use]
fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    v.map(|x| x / length)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// A small, flattened body, so that the datum's normals turn measurably over a rock.
    const SMALL: Spheroid = Spheroid {
        equatorial_radius_m: 52_000.0,
        polar_radius_m: 48_000.0,
    };

    fn rock(figure: &Spheroid, dir: [f64; 3], exponent: f64) -> Rock {
        let shape = RockShape::new(0.4, 0.7, Radians::new(0.9), exponent);
        Rock::new(figure, unit(dir), 120.0, Metres::new(3.0), shape)
    }

    /// The body-fixed point at `x` and `y` metres along the rock's horizontal axes from its base
    /// centre.
    fn beside(r: &Rock, x: f64, y: f64) -> [f64; 3] {
        [0, 1, 2].map(|k| r.base_centre[k] + x * r.axes[0][k] + y * r.axes[1][k])
    }

    /// Where the datum's normal through `at`'s foot meets the rock's top, by bisection on the
    /// superquadric's own inequality along the exact line: the closed form the flat frame stands
    /// in for.
    #[expect(
        clippy::many_single_char_names,
        reason = "the frame's x, y and z, the semi-axes a, b and c and the exponent n"
    )]
    fn exact_top(r: &Rock, figure: &Spheroid, at: &[f64; 3]) -> f64 {
        let (foot, _) = datum_foot(figure, at);
        let [a, b, c] = r.semi_axes;
        let n = r.shape.exponent;
        let inside = |h: f64| {
            let p = figure.surface_point(foot, h);
            let d = [0, 1, 2].map(|k| p[k] - r.base_centre[k]);
            let (x, y, z) = (dot(d, r.axes[0]), dot(d, r.axes[1]), dot(d, r.up));
            math::powf(x.abs() / a, n) + math::powf(y.abs() / b, n) + math::powf(z.abs() / c, n)
                <= 1.0
        };
        let (mut lo, mut hi) = (r.ground_m, r.ground_m + 2.0 * c);
        assert!(inside(lo) && !inside(hi));
        for _ in 0..200 {
            let mid = f64::midpoint(lo, hi);
            if inside(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The axes are the drawn shape's: the outline's area π D² ÷ 4, b ÷ a the aspect, c the
    /// exposed height, and the reach within its share of the diameter.
    #[test]
    fn a_rocks_axes_follow_its_diameter_and_shape() {
        let r = rock(&SMALL, [0.3, -0.4, 0.8], 2.5);
        let [a, b, c] = r.semi_axes().map(Metres::value);
        let area = 4.0 * a * b * outline_area_factor(2.5);
        assert!(
            (area - core::f64::consts::PI * 2.25).abs() < 1e-12,
            "{area}"
        );
        assert!((b / a - 0.7).abs() < 1e-12);
        assert!((c - 1.2).abs() < 1e-12);
        assert_eq!(r.exposed_height(), r.burial());
        for aspect in [ROCK_ASPECT.0, 0.7, ROCK_ASPECT.1] {
            for n in [ROCK_EXPONENT.0, 2.5, ROCK_EXPONENT.1] {
                let shape = RockShape::new(0.3, aspect, Radians::ZERO, n);
                let r = Rock::new(&SMALL, [1.0, 0.0, 0.0], 0.0, Metres::new(1.0), shape);
                assert!(r.reach().value() <= MAX_REACH_SHARE, "{aspect}, {n}");
            }
        }
    }

    /// At its axis a rock's top is the ground plus its exposed height, to the bit's rounding.
    #[test]
    fn the_top_at_the_axis_is_the_exposed_height_above_the_ground() {
        for dir in [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.2, 0.3, -0.9],
            [-0.5, 0.5, 0.1],
        ] {
            let r = rock(&SMALL, dir, 3.0);
            let top = r
                .top_above(&SMALL, &r.base_centre)
                .expect("the axis is inside");
            let exposed = top.value() - r.ground_m;
            assert!(
                (exposed - r.exposed_height().value()).abs() < 1e-9,
                "{exposed}"
            );
        }
    }

    /// An ellipsoid's top is c √(1 − x² ÷ a² − y² ÷ b²) above its equator in its frame, and a
    /// superquadric's c (1 − |x ÷ a|ⁿ − |y ÷ b|ⁿ)^(1 ÷ n); on an Earth-sized sphere, where the frame
    /// is all but flat, both meet their formulae within the contact bound and the coordinates'
    /// rounding, and the line misses the rock beyond its outline.
    #[test]
    #[expect(
        clippy::many_single_char_names,
        reason = "the frame's x, y and z, the semi-axes a, b and c and the exponent n"
    )]
    fn the_tops_meet_their_closed_forms() {
        let figure = Spheroid::sphere(6.371e6);
        for n in [2.0, 2.5, 3.0] {
            let r = rock(&figure, [0.6, 0.0, 0.8], n);
            let [a, b, c] = r.semi_axes;
            for (fx, fy) in [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.4, 0.4), (-0.6, 0.3)] {
                let (x, y) = (fx * a, fy * b);
                let closed = if (n - 2.0).abs() < f64::EPSILON {
                    c * (1.0 - x * x / (a * a) - y * y / (b * b)).sqrt()
                } else {
                    c * math::powf(
                        1.0 - math::powf(fx.abs(), n) - math::powf(fy.abs(), n),
                        1.0 / n,
                    )
                };
                let top = r.top_above(&figure, &beside(&r, x, y)).expect("inside");
                assert!(
                    (top.value() - r.ground_m - closed).abs()
                        <= r.contact_bound(&figure).value() + 1e-8,
                    "n {n} at ({fx}, {fy}): {} against {closed}",
                    top.value() - r.ground_m
                );
            }
            assert_eq!(r.top_above(&figure, &beside(&r, 1.01 * a, 0.0)), None);
            assert_eq!(r.top_above(&figure, &beside(&r, 0.0, -1.01 * b)), None);
        }
    }

    /// On a small, flattened body the flat frame's top is within its contact bound of the exact
    /// line's meeting with the rock, wherever the top is no steeper than 1.
    #[test]
    fn the_flat_frame_is_within_its_contact_bound() {
        for n in [2.0, 3.0] {
            for dir in [[1.0, 0.0, 0.0], [0.6, 0.0, 0.8], [0.1, 0.2, 0.97]] {
                let r = rock(&SMALL, dir, n);
                let [a, b, _] = r.semi_axes;
                let bound = r.contact_bound(&SMALL).value();
                assert!(bound > 0.0 && bound < 1e-3, "{bound}");
                for (fx, fy) in [(0.0, 0.0), (0.3, 0.1), (-0.4, 0.2), (0.1, -0.5)] {
                    let at = beside(&r, fx * a, fy * b);
                    let top = r.top_above(&SMALL, &at).expect("inside").value();
                    let exact = exact_top(&r, &SMALL, &at);
                    assert!(
                        (top - exact).abs() <= bound,
                        "n {n}, {dir:?}, ({fx}, {fy}): {top} against {exact}, bound {bound}"
                    );
                }
            }
        }
    }

    /// The outline's area factor is π ÷ 4 for an ellipse and matches a midpoint quadrature of the
    /// superquadric's quarter, ∫₀¹ (1 − xⁿ)^(1 ÷ n) dx, at n = 3.
    #[test]
    fn the_outline_area_factor_is_the_superquadrics() {
        assert!((outline_area_factor(2.0) - core::f64::consts::PI / 4.0).abs() < 1e-15);
        let n = 3.0;
        let steps = 200_000_u32;
        let mut sum = 0.0;
        for i in 0..steps {
            let x = (f64::from(i) + 0.5) / f64::from(steps);
            sum += math::powf(1.0 - math::powf(x, n), 1.0 / n);
        }
        let quadrature = sum / f64::from(steps);
        assert!(
            (outline_area_factor(n) - quadrature).abs() < 1e-6,
            "{quadrature}"
        );
    }

    /// The four words draw each part within its range, in order.
    #[test]
    fn a_drawn_shape_lies_in_its_ranges() {
        use hyperion_base::Seed;
        use hyperion_base::rng::{ObjectKey, tags};
        let mut stream = Stream::open(Seed::new(11), tags::SELFTEST_STREAM, ObjectKey::cell(3));
        for _ in 0..1_000 {
            let start = stream.position();
            let s = RockShape::draw(&mut stream);
            assert_eq!(stream.position() - start, 4);
            assert!((ROCK_HEIGHT_RATIO.0..=ROCK_HEIGHT_RATIO.1).contains(&s.height_ratio()));
            assert!((ROCK_ASPECT.0..=ROCK_ASPECT.1).contains(&s.aspect()));
            assert!((0.0..core::f64::consts::TAU).contains(&s.yaw().value()));
            assert!((ROCK_EXPONENT.0..=ROCK_EXPONENT.1).contains(&s.exponent()));
        }
    }
}
