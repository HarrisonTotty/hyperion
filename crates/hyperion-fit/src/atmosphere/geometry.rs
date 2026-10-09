//! Vectors, local frames and the spherical shells the tracer walks.
//!
//! The body's centre is the origin and its axis z; positions are in metres. A ray crosses the
//! shells one at a time: from a point in shell k it leaves through the sphere below, if it reaches
//! it, or else through the one above, and each crossing is solved in the form of the quadratic's
//! roots that loses no digits to cancellation (Press et al. 2007, _Numerical Recipes_, 3rd ed.,
//! §5.6).

use core::f64::consts::PI;
use core::ops::{Add, Mul, Neg, Sub};

use hyperion_sim::math;

use crate::tasks::displaced_forms::births::Draws;

/// A vector in the body's frame: a position in metres, or a dimensionless direction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Vec3 {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl Vec3 {
    #[must_use]
    pub(crate) const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub(crate) fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[must_use]
    pub(crate) fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[must_use]
    pub(crate) fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// This vector scaled to unit length; a zero vector stays zero.
    #[must_use]
    pub(crate) fn normalised(self) -> Self {
        let length = self.length();
        if length > 0.0 {
            self * (1.0 / length)
        } else {
            self
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

/// The local frame at a latitude on the meridian of longitude 0: up, north and east, a
/// right-handed triple (east × north = up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LocalFrame {
    pub(crate) up: Vec3,
    pub(crate) north: Vec3,
    pub(crate) east: Vec3,
}

impl LocalFrame {
    #[must_use]
    pub(crate) fn at_latitude(latitude_rad: f64) -> Self {
        let (s, c) = math::sin_cos(latitude_rad);
        Self {
            up: Vec3::new(c, 0.0, s),
            north: Vec3::new(-s, 0.0, c),
            east: Vec3::new(0.0, 1.0, 0.0),
        }
    }

    /// The horizontal unit vector at azimuth `azimuth_rad` from north towards east.
    fn horizontal(&self, azimuth_rad: f64) -> Vec3 {
        let (s, c) = math::sin_cos(azimuth_rad);
        self.north * c + self.east * s
    }

    /// The unit vector at zenith angle `zenith_rad` from up and azimuth `azimuth_rad`.
    #[must_use]
    pub(crate) fn direction(&self, zenith_rad: f64, azimuth_rad: f64) -> Vec3 {
        let (s, c) = math::sin_cos(zenith_rad);
        self.up * c + self.horizontal(azimuth_rad) * s
    }

    /// The derivative of [`direction`](Self::direction) by the zenith angle: the unit vector in
    /// the direction's vertical plane, perpendicular to it, towards the larger zenith angle. It is
    /// defined at the zenith and the nadir too, where the azimuth chooses the plane.
    #[must_use]
    pub(crate) fn zenith_tangent(&self, zenith_rad: f64, azimuth_rad: f64) -> Vec3 {
        let (s, c) = math::sin_cos(zenith_rad);
        self.up * -s + self.horizontal(azimuth_rad) * c
    }
}

/// Two unit vectors that make a right-handed orthonormal basis with the unit vector `n`
/// (Duff et al. 2017, JCGT 6(1), 1, listing 3).
#[must_use]
pub(crate) fn basis(n: Vec3) -> (Vec3, Vec3) {
    let sign = 1.0_f64.copysign(n.z);
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    (
        Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x),
        Vec3::new(b, sign + n.y * n.y * a, -n.y),
    )
}

/// The unit vector at angle acos(`cos_theta`) from the unit vector `axis`, at azimuth
/// `azimuth_rad` about it in [`basis`]'s frame.
#[must_use]
pub(crate) fn turned(axis: Vec3, cos_theta: f64, azimuth_rad: f64) -> Vec3 {
    let (u, v) = basis(axis);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let (s, c) = math::sin_cos(azimuth_rad);
    (axis * cos_theta + (u * c + v * s) * sin_theta).normalised()
}

/// A direction drawn uniformly in solid angle within the cone of half-angle `half_angle_rad`
/// about `axis`; the axis itself, with no draws, for a cone of zero width.
pub(crate) fn in_cone(axis: Vec3, half_angle_rad: f64, draws: &mut Draws) -> Vec3 {
    if half_angle_rad <= 0.0 {
        return axis;
    }
    // 1 − cos α, written without cancellation.
    let s = math::sin(0.5 * half_angle_rad);
    let cos_theta = 1.0 - draws.uniform() * 2.0 * s * s;
    turned(axis, cos_theta, 2.0 * PI * draws.uniform())
}

/// A direction drawn from the cosine-weighted hemisphere about the unit vector `normal`.
pub(crate) fn cosine_weighted(normal: Vec3, draws: &mut Draws) -> Vec3 {
    let cos_theta = draws.uniform().sqrt();
    turned(normal, cos_theta, 2.0 * PI * draws.uniform())
}

/// Where a ray goes when it leaves a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    /// Into the shell of this index.
    Shell(usize),
    /// Onto the ground.
    Ground,
    /// Out through the top, to space.
    Space,
}

/// How far a ray goes before it leaves its shell, and where it goes then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Exit {
    /// The distance to the boundary, m.
    pub(crate) distance_m: f64,
    pub(crate) next: Next,
}

/// The atmosphere's concentric shells: `radii_m[0]` the ground, the last the top, ascending.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShellGrid {
    radii_m: Vec<f64>,
}

impl ShellGrid {
    /// Shells about a ground of radius `ground_radius_m`, from 0 to `top_height_m`, split at
    /// every height of `heights_m` strictly between the two.
    #[must_use]
    pub(crate) fn new(
        ground_radius_m: f64,
        top_height_m: f64,
        heights_m: impl IntoIterator<Item = f64>,
    ) -> Self {
        let mut inner: Vec<f64> = heights_m
            .into_iter()
            .filter(|&h| h > 0.0 && h < top_height_m)
            .collect();
        inner.sort_by(f64::total_cmp);
        inner.dedup_by(|a, b| (*a - *b).abs() <= 1e-9 * top_height_m);
        let mut radii_m = Vec::with_capacity(inner.len() + 2);
        radii_m.push(ground_radius_m);
        radii_m.extend(inner.iter().map(|h| ground_radius_m + h));
        radii_m.push(ground_radius_m + top_height_m);
        Self { radii_m }
    }

    /// How many shells there are.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.radii_m.len() - 1
    }

    /// The ground's radius, m.
    #[must_use]
    pub(crate) fn ground_radius_m(&self) -> f64 {
        self.radii_m[0]
    }

    /// The top's radius, m.
    #[must_use]
    pub(crate) fn top_radius_m(&self) -> f64 {
        self.radii_m[self.radii_m.len() - 1]
    }

    /// The heights above the ground of shell `shell`'s floor and ceiling, m.
    #[must_use]
    pub(crate) fn heights_m(&self, shell: usize) -> (f64, f64) {
        let ground = self.radii_m[0];
        (
            self.radii_m[shell] - ground,
            self.radii_m[shell + 1] - ground,
        )
    }

    /// The shell a ray at `point` moving along `dir` is in, if `point` is in the atmosphere and
    /// the ray is not leaving it there. A point on a boundary belongs to the shell the ray moves
    /// into; a point just below the ground, by rounding, to the lowest.
    #[must_use]
    pub(crate) fn locate(&self, point: Vec3, dir: Vec3) -> Option<usize> {
        let radius = point.length();
        let top = self.top_radius_m();
        let outward = point.dot(dir) >= 0.0;
        if radius > top || (radius >= top && outward) {
            return None;
        }
        let above = self.radii_m.partition_point(|&r| r <= radius);
        let mut shell = above.saturating_sub(1).min(self.len() - 1);
        if shell > 0 && radius <= self.radii_m[shell] && !outward {
            shell -= 1;
        }
        Some(shell)
    }

    /// The distance along the unit vector `dir` at which a ray from `point`, outside the
    /// atmosphere, enters it through the top, m; `None` if it misses.
    #[must_use]
    pub(crate) fn entry_distance_m(&self, point: Vec3, dir: Vec3) -> Option<f64> {
        let along = point.dot(dir);
        if along >= 0.0 {
            return None;
        }
        let radius = point.length();
        let top = self.top_radius_m();
        let offset = (radius - top) * (radius + top);
        let disc = along * along - offset;
        if disc < 0.0 {
            return None;
        }
        Some((offset / (-along + disc.sqrt())).max(0.0))
    }

    /// Where a ray from `point` in shell `shell` along the unit vector `dir` leaves it: through
    /// the floor if it reaches it, otherwise through the ceiling.
    #[must_use]
    pub(crate) fn exit(&self, point: Vec3, dir: Vec3, shell: usize) -> Exit {
        let radius = point.length();
        let along = point.dot(dir);
        let (floor, ceiling) = (self.radii_m[shell], self.radii_m[shell + 1]);
        if along < 0.0 {
            let offset = (radius - floor) * (radius + floor);
            let disc = along * along - offset;
            if disc >= 0.0 {
                return Exit {
                    distance_m: (offset / (-along + disc.sqrt())).max(0.0),
                    next: if shell == 0 {
                        Next::Ground
                    } else {
                        Next::Shell(shell - 1)
                    },
                };
            }
        }
        let offset = (radius - ceiling) * (radius + ceiling);
        let disc = (along * along - offset).max(0.0);
        let distance_m = if along > 0.0 {
            -offset / (along + disc.sqrt())
        } else {
            -along + disc.sqrt()
        };
        Exit {
            distance_m: distance_m.max(0.0),
            next: if shell + 1 == self.len() {
                Next::Space
            } else {
                Next::Shell(shell + 1)
            },
        }
    }

    /// The point `distance_m` along `dir` from `point`, set onto the sphere of the boundary that
    /// `next` says it crossed from shell `shell`.
    #[must_use]
    pub(crate) fn cross(
        &self,
        point: Vec3,
        dir: Vec3,
        distance_m: f64,
        shell: usize,
        next: Next,
    ) -> Vec3 {
        let moved = point + dir * distance_m;
        let radius = match next {
            Next::Ground => self.radii_m[0],
            Next::Space => self.top_radius_m(),
            Next::Shell(j) if j < shell => self.radii_m[shell],
            Next::Shell(_) => self.radii_m[shell + 1],
        };
        moved * (radius / moved.length())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distance from `point` along `dir` to the sphere of radius `radius`, the far root.
    fn far_root(point: Vec3, dir: Vec3, radius: f64) -> f64 {
        let along = point.dot(dir);
        -along + (along * along - point.dot(point) + radius * radius).sqrt()
    }

    /// Walks a ray from `point` in `shell` to the ground or space, returning the distance, how it
    /// ended and the crossings it took.
    fn walk(grid: &ShellGrid, mut point: Vec3, dir: Vec3, mut shell: usize) -> (f64, Next, usize) {
        let mut total = 0.0;
        let mut crossings = 0;
        loop {
            let exit = grid.exit(point, dir, shell);
            total += exit.distance_m;
            crossings += 1;
            assert!(crossings < 10_000, "the walk does not end");
            match exit.next {
                Next::Shell(j) => {
                    point = grid.cross(point, dir, exit.distance_m, shell, exit.next);
                    shell = j;
                }
                Next::Ground | Next::Space => return (total, exit.next, crossings),
            }
        }
    }

    #[test]
    fn atmosphere_shell_walks_cover_the_chord() {
        let ground = 6_371_000.0;
        let grid = ShellGrid::new(ground, 100_000.0, (1..20).map(|i| f64::from(i) * 5_000.0));
        assert_eq!(grid.len(), 20);
        let frame = LocalFrame::at_latitude(0.3);
        let start = frame.up * (ground + 1_234.0);
        // Up at 60° from the zenith: the chord to the top.
        let up = frame.direction(60f64.to_radians(), 1.0);
        let shell = grid.locate(start, up).unwrap();
        assert_eq!(shell, 0);
        let (distance, end, _) = walk(&grid, start, up, shell);
        assert_eq!(end, Next::Space);
        let expected = far_root(start, up, ground + 100_000.0);
        assert!((distance - expected).abs() < 1e-6, "{distance} {expected}");
        // Down to the ground.
        let down = frame.direction(120f64.to_radians(), 0.0);
        let (distance, end, _) = walk(&grid, start, down, grid.locate(start, down).unwrap());
        assert_eq!(end, Next::Ground);
        let along = start.dot(down);
        let expected = -along - (along * along - start.dot(start) + ground * ground).sqrt();
        assert!((distance - expected).abs() < 1e-6, "{distance} {expected}");
    }

    #[test]
    fn atmosphere_rays_grazing_a_boundary_pass_it() {
        let ground = 6_371_000.0;
        let grid = ShellGrid::new(ground, 100_000.0, [30_000.0, 60_000.0]);
        let frame = LocalFrame::at_latitude(0.0);
        // From a point on the 30 km sphere, exactly horizontal, and a hair below and above it.
        let start = frame.up * (ground + 30_000.0);
        for dip in [0.0, 1e-12, -1e-12, 1e-9] {
            let dir = (frame.north + frame.up * dip).normalised();
            let shell = grid.locate(start, dir).unwrap();
            let (distance, end, crossings) = walk(&grid, start, dir, shell);
            assert_eq!(end, Next::Space, "{dip}");
            let expected = far_root(start, dir, ground + 100_000.0);
            assert!(
                (distance - expected).abs() < 1e-3,
                "{dip}: {distance} {expected}"
            );
            assert!(crossings < 10, "{dip}: {crossings}");
        }
    }

    #[test]
    fn atmosphere_rays_enter_from_orbit_and_locate_on_boundaries() {
        let ground = 6_371_000.0;
        let grid = ShellGrid::new(ground, 100_000.0, [50_000.0]);
        let frame = LocalFrame::at_latitude(0.0);
        let orbit = frame.up * (ground + 400_000.0);
        let down = -frame.up;
        assert!((grid.entry_distance_m(orbit, down).unwrap() - 300_000.0).abs() < 1e-6);
        assert_eq!(grid.entry_distance_m(orbit, frame.up), None);
        assert_eq!(grid.entry_distance_m(orbit, frame.north), None);
        let on_top = frame.up * (ground + 100_000.0);
        assert_eq!(grid.locate(on_top, down), Some(1));
        assert_eq!(grid.locate(on_top, frame.up), None);
        let on_middle = frame.up * (ground + 50_000.0);
        assert_eq!(grid.locate(on_middle, down), Some(0));
        assert_eq!(grid.locate(on_middle, frame.up), Some(1));
        assert_eq!(grid.locate(frame.up * ground, frame.up), Some(0));
        assert_eq!(grid.locate(frame.up * ground, down), Some(0));
        let exit = grid.exit(frame.up * ground, down, 0);
        assert_eq!(exit.next, Next::Ground);
        assert!(exit.distance_m.abs() < 1e-12, "{exit:?}");
    }

    #[test]
    fn atmosphere_frames_and_cones_are_orthonormal_and_uniform() {
        let frame = LocalFrame::at_latitude(0.7);
        assert!((frame.east.cross(frame.north) - frame.up).length() < 1e-15);
        for (zenith, azimuth) in [(0.0, 1.0), (0.4, 2.0), (PI, 0.3)] {
            let dir = frame.direction(zenith, azimuth);
            let tangent = frame.zenith_tangent(zenith, azimuth);
            assert!((dir.length() - 1.0).abs() < 1e-15 && (tangent.length() - 1.0).abs() < 1e-15);
            assert!(dir.dot(tangent).abs() < 1e-15);
        }
        let axis = frame.direction(1.0, 2.0);
        let (u, v) = basis(axis);
        assert!(u.dot(v).abs() < 1e-15 && u.dot(axis).abs() < 1e-15);
        assert!((u.cross(v) - axis).length() < 1e-15);
        let half_angle = 1f64.to_radians();
        let mut draws = Draws::new(5, 0, 0);
        let n = 50_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let cos_theta = in_cone(axis, half_angle, &mut draws).dot(axis);
            assert!(cos_theta >= math::cos(half_angle) - 1e-15);
            sum += cos_theta;
        }
        // Uniform in solid angle: the mean cosine is (1 + cos α) ÷ 2.
        let mean = sum / f64::from(n);
        let expected = f64::midpoint(1.0, math::cos(half_angle));
        // The cosines spread over 1.5 × 10⁻⁴, so the mean's standard error is 2 × 10⁻⁷.
        assert!((mean - expected).abs() < 1e-6, "{mean} {expected}");
    }
}
