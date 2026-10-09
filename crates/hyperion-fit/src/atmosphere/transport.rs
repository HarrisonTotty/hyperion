//! The estimator: backward Monte Carlo of the radiance arriving at a detector, at one wavelength,
//! in `f64`, with no tables.
//!
//! A sample starts at the detector, in a direction drawn within its cone (or, for a flux, from the
//! cosine-weighted hemisphere), and follows the light back:
//!
//! - **Absorption is a weight.** Free paths are drawn against scattering alone, and the path's
//!   weight carries the absorption transmittance e^(−τₐ) of every flight, by quadrature (the
//!   "absorption weighting" of MYSTIC, Mayer 2009, EPJ Web of Conferences 1, 75). An
//!   absorbing-only medium therefore gives Beer–Lambert exactly.
//! - **Free paths by delta tracking** (Woodcock et al. 1965) against each shell's majorant, the
//!   largest scattering coefficient the shell's profiles reach, so that the sampled distribution is
//!   exact whatever the profiles. Shell boundaries fall at every kink of a profile and every scale
//!   height of an exponential one, so an exponential term's majorant is within a factor e of its
//!   coefficient; a tent's or a table's can stand further above it near a zero, which costs time,
//!   not accuracy.
//! - **The first collision is forced** (Cashwell and Everett 1959; Marchuk et al. 1980, _The Monte
//!   Carlo Methods in Atmospheric Optics_, §2.4): the view ray is split into the part that
//!   scatters, weighted 1 − e^(−τₛ) and drawn from the scattering optical depth along the ray by
//!   quadrature and bisection, and the part that does not, weighted e^(−τₛ), which goes on to the
//!   ground, the black target of an aerial-perspective geometry, or space. A thin atmosphere's
//!   radiance is then no longer carried by the few rays that happen to scatter.
//! - **Next-event estimation to each sun** at every scattering and every reflection (the local
//!   estimate, Marchuk et al. 1980 §2.6): the sun's transmittance from the vertex is computed, not
//!   sampled, by Gauss–Legendre quadrature of the extinction along the ray, shell by shell. The
//!   suns are points: a path that escapes adds nothing, so nothing is counted twice.
//! - **The ground** is Lambertian: next-event estimation with A ÷ π, then a cosine-weighted
//!   direction with the weight times A.
//! - **Russian roulette** (Arvo and Kirk 1990) once a path's weight falls below
//!   [`ROULETTE_FRACTION`] of the weight its branch started with. The threshold is relative so
//!   that a forced collision's small weight, 1 − e^(−τₛ) on a thin limb, does not send its
//!   multiple scattering straight to the roulette.
//!
//! The quadrature is 8-point Gauss–Legendre on each shell's piece of a ray, where the integrand is
//! smooth: the profiles' kinks are shell boundaries, and an exponential profile changes by at most
//! e across a shell.
//!
//! **The Stokes mode** (Rayleigh, isotropic and absorbing terms) carries, beside the scalar
//! weight, the 3 × 3 matrix that takes the Stokes vector (I, Q, U) of the light arriving along the
//! current flight, in that flight's frame, to the detector's. The light's frame for a propagation
//! direction k is (e₁, e₂ = k × e₁); a rotation of the frame by ψ, e₁′ = cos ψ e₁ + sin ψ e₂,
//! takes (Q, U) to (cos 2ψ Q + sin 2ψ U, −sin 2ψ Q + cos 2ψ U), and the scattering matrix acts in
//! the scattering plane's frame, e₂ its normal (Hovenier, van der Mee and Domke 2004, §1.4 and
//! §2.3; Chandrasekhar 1950, §15). Directions are drawn from the phase function, the matrix
//! divided by it, so that the scalar and the Stokes modes follow the same paths from the same
//! draws: for an isotropic scatterer they give the same I, and for Rayleigh their difference is the
//! scalar approximation's error alone. Light is unpolarised at the suns and after the ground,
//! and V never arises, since no source or scatterer here makes it.

use core::f64::consts::PI;

use hyperion_sim::galaxy::quad::{Gl16Panel, bisect};
use hyperion_sim::math;
use hyperion_sim::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};

use super::Polarisation;
use super::case::{Aggregate, AtmosphereCase, DensityProfile, Geometry, LocalDirection};
use super::geometry::{LocalFrame, Next, ShellGrid, Vec3, basis, cosine_weighted, in_cone, turned};
use super::optics::{Phase, ScatteringMatrix};
use crate::tasks::displaced_forms::births::Draws;

/// Below this fraction of its branch's starting weight a path plays Russian roulette, surviving
/// with probability weight ÷ (this × the starting weight) and carrying that threshold if it does.
pub(crate) const ROULETTE_FRACTION: f64 = 0.1;

/// A sun's transmittance is taken as zero once its optical depth passes this: e^(−50) is
/// 2 × 10⁻²², below any radiance a case compares.
const NEGLIGIBLE_OPTICAL_DEPTH: f64 = 50.0;

/// Majorants are widened by this factor, so that rounding in a height cannot lift a coefficient
/// past its shell's bound: one ulp of a position on a 10¹² m sphere, a plane-parallel benchmark's,
/// is 1.2 × 10⁻⁴ m, which moves e^(−h ÷ 8 km) by 1.5 × 10⁻⁸. The extra null collisions cost
/// nothing measurable.
const MAJORANT_MARGIN: f64 = 1.0 + 1e-6;

/// The medium at one wavelength, on the case's shells.
#[derive(Debug)]
pub(crate) struct Medium<'a> {
    grid: &'a ShellGrid,
    profiles: Vec<&'a DensityProfile>,
    /// Per term, at relative density 1, m⁻¹.
    scattering_per_m: Vec<f64>,
    /// Per term, at relative density 1, m⁻¹.
    absorption_per_m: Vec<f64>,
    phases: Vec<Phase>,
    /// Per shell, a bound of the scattering coefficient in the shell, m⁻¹.
    majorant_per_m: Vec<f64>,
    absorbs: bool,
    ground_albedo: f64,
    /// Per sun, in the case's unit.
    irradiance: Vec<f64>,
}

/// How a sample is aimed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Aim {
    /// Within a detector cone about `axis`, `e1` the axis's first Stokes axis (in its vertical
    /// plane).
    Cone {
        axis: Vec3,
        e1: Vec3,
        half_angle_rad: f64,
    },
    /// Over the cosine-weighted hemisphere about `normal`: the sample is the flux through a
    /// surface facing `normal`, π times the radiance.
    Hemisphere { normal: Vec3 },
}

/// Where a sample starts, where it looks and where the suns are.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Source {
    /// Where the detector is, m.
    pub(crate) origin_m: Vec3,
    pub(crate) aim: Aim,
    /// The view ray ends here at a black target, m: infinite but for aerial perspective.
    pub(crate) cut_m: f64,
    /// The unit vectors towards each sun.
    pub(crate) suns: Vec<Vec3>,
}

impl Source {
    /// The detector of `geometry` in `case`.
    #[must_use]
    pub(crate) fn detector(case: &AtmosphereCase, geometry: &Geometry) -> Self {
        let frame = LocalFrame::at_latitude(geometry.observer.latitude_deg.to_radians());
        let (zenith, azimuth) = (
            geometry.view.zenith_deg.to_radians(),
            geometry.view.azimuth_deg.to_radians(),
        );
        Self {
            origin_m: frame.up * (case.ground_radius_m() + geometry.observer.height_m),
            aim: Aim::Cone {
                axis: frame.direction(zenith, azimuth),
                e1: frame.zenith_tangent(zenith, azimuth),
                half_angle_rad: case.detector_half_angle_deg(geometry).to_radians(),
            },
            cut_m: geometry.max_distance_m.unwrap_or(f64::INFINITY),
            suns: sun_vectors(&frame, &geometry.sun_directions),
        }
    }

    /// The flux through a horizontal surface at the ground below `aggregate` from above, or at the
    /// top above it from below.
    #[must_use]
    pub(crate) fn flux(case: &AtmosphereCase, aggregate: &Aggregate, level: Level) -> Self {
        let frame = LocalFrame::at_latitude(aggregate.latitude_deg.to_radians());
        let (height_m, normal) = match level {
            Level::Ground => (0.0, frame.up),
            Level::Top => (case.top_height_m(), -frame.up),
        };
        Self {
            origin_m: frame.up * (case.ground_radius_m() + height_m),
            aim: Aim::Hemisphere { normal },
            cut_m: f64::INFINITY,
            suns: sun_vectors(&frame, &aggregate.sun_directions),
        }
    }
}

/// Where a flux aggregate is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// At the ground, downwelling.
    Ground,
    /// At the top, upwelling.
    Top,
}

/// The unit vectors towards the suns at `directions` in `frame`.
#[must_use]
pub(crate) fn sun_vectors(frame: &LocalFrame, directions: &[LocalDirection]) -> Vec<Vec3> {
    directions
        .iter()
        .map(|s| frame.direction(s.zenith_deg.to_radians(), s.azimuth_deg.to_radians()))
        .collect()
}

/// Buffers a sample reuses.
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    pieces: Vec<Piece>,
    terms_per_m: Vec<f64>,
}

/// One shell's stretch of the first flight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    start_m: Vec3,
    length_m: f64,
    shell: usize,
    scattering_depth: f64,
    absorption_depth: f64,
}

/// How the first flight ends.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FirstEnd {
    /// On the ground, here, m.
    Ground(Vec3),
    /// In space, or at the black target.
    Lost,
}

/// Where a flight ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Scatter,
    Ground,
    Escape,
}

/// What a vertex is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vertex {
    Scatter,
    Ground,
}

/// The polarisation state a path carries in the Stokes mode.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stokes {
    /// Takes (I, Q, U) arriving along the current flight, in its frame, to the detector's frame.
    m: [[f64; 3]; 3],
    /// The current flight's first Stokes axis, perpendicular to it.
    e1: Vec3,
}

impl Stokes {
    fn new(e1: Vec3) -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            e1,
        }
    }

    /// The unit normal of the plane of `a` and `b`; for parallel vectors, the current frame's
    /// second axis for light leaving along −`dir`, so that the rotation into the plane's frame is
    /// none.
    fn plane_normal(&self, a: Vec3, b: Vec3, dir: Vec3) -> Vec3 {
        let n = a.cross(b);
        let length = n.length();
        if length > 1e-12 {
            n * (1.0 / length)
        } else {
            (-dir).cross(self.e1)
        }
    }

    /// (cos 2ψ, sin 2ψ) of the rotation from the frame of the light leaving along −`dir` in the
    /// scattering plane of normal `n`, (dir × n, n), to the current frame.
    fn rotation(&self, dir: Vec3, n: Vec3) -> (f64, f64) {
        let c = self.e1.dot(dir.cross(n));
        let s = self.e1.dot(n);
        let norm = c * c + s * s;
        ((c * c - s * s) / norm, 2.0 * c * s / norm)
    }

    /// (I, Q, U) at the detector of unit unpolarised light from the unit vector `sun` scattered
    /// towards −`dir` by `z`.
    fn next_event(&self, z: &ScatteringMatrix, dir: Vec3, sun: Vec3) -> [f64; 3] {
        let n = self.plane_normal(sun, dir, dir);
        let (c2, s2) = self.rotation(dir, n);
        let v = [z.a1, c2 * z.b1, -s2 * z.b1];
        self.m
            .map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2])
    }

    /// Scatters the path from the flight along `old` into the flight along `new`, by `z`
    /// divided by the phase function the direction was drawn from.
    fn scatter(&mut self, z: &ScatteringMatrix, old: Vec3, new: Vec3) {
        let n = self.plane_normal(new, old, old);
        let (c2, s2) = self.rotation(old, n);
        // R(ψ) Z: rows of the rotation [[1, 0, 0], [0, c2, s2], [0, −s2, c2]] times
        // [[a1, b1, 0], [b1, a2, 0], [0, 0, a3]].
        let a = [
            [z.a1, z.b1, 0.0],
            [c2 * z.b1, c2 * z.a2, s2 * z.a3],
            [-s2 * z.b1, -s2 * z.a2, c2 * z.a3],
        ];
        self.m = self
            .m
            .map(|row| [0, 1, 2].map(|j| row[0] * a[0][j] + row[1] * a[1][j] + row[2] * a[2][j]));
        self.e1 = new.cross(n);
    }

    /// Depolarises the path, as a Lambertian reflection does, into the flight along `dir`.
    fn depolarise(&mut self, dir: Vec3) {
        for row in &mut self.m {
            row[1] = 0.0;
            row[2] = 0.0;
        }
        self.e1 = basis(dir).0;
    }
}

/// A path in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Walker {
    /// Where the path is, m.
    position_m: Vec3,
    /// Where it is going, the reverse of the light's direction.
    dir: Vec3,
    shell: usize,
    weight: f64,
    /// The weight below which the path plays Russian roulette.
    roulette: f64,
    stokes: Option<Stokes>,
}

impl Walker {
    /// A branch starting at `position_m` along `dir` in `shell` with `weight`.
    fn new(position_m: Vec3, dir: Vec3, shell: usize, weight: f64, stokes: Option<Stokes>) -> Self {
        Self {
            position_m,
            dir,
            shell,
            weight,
            roulette: ROULETTE_FRACTION * weight,
            stokes,
        }
    }
}

impl<'a> Medium<'a> {
    /// The medium of `case` at wavelength index `wavelength`, on `grid`.
    #[must_use]
    pub(crate) fn new(case: &'a AtmosphereCase, grid: &'a ShellGrid, wavelength: usize) -> Self {
        let terms = case.terms();
        let profiles: Vec<&DensityProfile> = terms.iter().map(|t| &t.density).collect();
        let scattering_per_m: Vec<f64> = terms
            .iter()
            .map(|t| t.scattering_per_m[wavelength])
            .collect();
        let absorption_per_m: Vec<f64> = terms
            .iter()
            .map(|t| t.absorption_per_m[wavelength])
            .collect();
        let majorant_per_m = (0..grid.len())
            .map(|shell| {
                let (low_m, high_m) = grid.heights_m(shell);
                profiles
                    .iter()
                    .zip(&scattering_per_m)
                    .map(|(profile, &s)| s * profile.max_over(low_m, high_m))
                    .sum::<f64>()
                    * MAJORANT_MARGIN
            })
            .collect();
        Self {
            grid,
            phases: terms
                .iter()
                .map(|t| Phase::of(&t.phase, wavelength))
                .collect(),
            absorbs: absorption_per_m.iter().any(|&a| a > 0.0),
            profiles,
            scattering_per_m,
            absorption_per_m,
            majorant_per_m,
            ground_albedo: case.ground_albedo()[wavelength],
            irradiance: case
                .suns()
                .iter()
                .map(|s| s.irradiance[wavelength])
                .collect(),
        }
    }

    /// The height of `x` above the ground, m.
    fn height_m(&self, x: Vec3) -> f64 {
        x.length() - self.grid.ground_radius_m()
    }

    /// The scattering and absorption coefficients at `x`, m⁻¹.
    fn coefficients_per_m(&self, x: Vec3) -> (f64, f64) {
        let h = self.height_m(x);
        let mut scattering = 0.0;
        let mut absorption = 0.0;
        for ((profile, &s), &a) in self
            .profiles
            .iter()
            .zip(&self.scattering_per_m)
            .zip(&self.absorption_per_m)
        {
            let rho = profile.at(h);
            scattering += s * rho;
            absorption += a * rho;
        }
        (scattering, absorption)
    }

    /// The scattering coefficient at `x`, m⁻¹.
    fn scattering_per_m_at(&self, x: Vec3) -> f64 {
        let h = self.height_m(x);
        self.profiles
            .iter()
            .zip(&self.scattering_per_m)
            .map(|(profile, &s)| s * profile.at(h))
            .sum()
    }

    /// The absorption coefficient at `x`, m⁻¹.
    fn absorption_per_m_at(&self, x: Vec3) -> f64 {
        let h = self.height_m(x);
        self.profiles
            .iter()
            .zip(&self.absorption_per_m)
            .map(|(profile, &a)| a * profile.at(h))
            .sum()
    }

    /// The absorption optical depth over `[0, length_m]` along `dir` from `start_m`, by the
    /// 8-point Gauss–Legendre rule.
    fn absorption_depth(&self, start_m: Vec3, dir: Vec3, length_m: f64) -> f64 {
        if !self.absorbs {
            return 0.0;
        }
        let half = 0.5 * length_m;
        let mut sum = 0.0;
        for (&x, &w) in GL8_NODES.iter().zip(&GL8_WEIGHTS) {
            sum += w * self.absorption_per_m_at(start_m + dir * (half * (1.0 + x)));
        }
        sum * half
    }

    /// The scattering and absorption optical depths over `[0, length_m]` along `dir` from
    /// `start_m`, by the 8-point Gauss–Legendre rule.
    fn depths(&self, start_m: Vec3, dir: Vec3, length_m: f64) -> (f64, f64) {
        let half = 0.5 * length_m;
        let (mut scattering, mut absorption) = (0.0, 0.0);
        for (&x, &w) in GL8_NODES.iter().zip(&GL8_WEIGHTS) {
            let (s, a) = self.coefficients_per_m(start_m + dir * (half * (1.0 + x)));
            scattering += w * s;
            absorption += w * a;
        }
        (scattering * half, absorption * half)
    }

    /// The extinction optical depth from `start_m` in shell `shell` along `dir` to space, and
    /// `None` if the ground is in the way; it stops counting once past `limit`.
    fn depth_to_space_from(
        &self,
        start_m: Vec3,
        dir: Vec3,
        mut shell: usize,
        limit: f64,
    ) -> Option<f64> {
        let mut point = start_m;
        let mut depth = 0.0;
        loop {
            let exit = self.grid.exit(point, dir, shell);
            let (scattering, absorption) = self.depths(point, dir, exit.distance_m);
            depth += scattering + absorption;
            if depth > limit {
                return Some(depth);
            }
            match exit.next {
                Next::Shell(j) => {
                    point = self
                        .grid
                        .cross(point, dir, exit.distance_m, shell, exit.next);
                    shell = j;
                }
                Next::Ground => return None,
                Next::Space => return Some(depth),
            }
        }
    }

    /// The transmittance from `start_m` in shell `shell` along `dir` to space: zero if the ground
    /// is in the way, or past [`NEGLIGIBLE_OPTICAL_DEPTH`].
    fn transmittance_to_space(&self, start_m: Vec3, dir: Vec3, shell: usize) -> f64 {
        match self.depth_to_space_from(start_m, dir, shell, NEGLIGIBLE_OPTICAL_DEPTH) {
            Some(depth) if depth <= NEGLIGIBLE_OPTICAL_DEPTH => math::exp(-depth),
            Some(_) | None => 0.0,
        }
    }

    /// The extinction optical depth from `origin_m`, anywhere, along the unit vector `dir` to
    /// space; `None` if the ground is in the way. This is the direct beam's, to compare with
    /// Beer–Lambert.
    #[must_use]
    pub(crate) fn optical_depth_to_space(&self, origin_m: Vec3, dir: Vec3) -> Option<f64> {
        match self.grid.locate(origin_m, dir) {
            Some(shell) => self.depth_to_space_from(origin_m, dir, shell, f64::INFINITY),
            None => match self.grid.entry_distance_m(origin_m, dir) {
                None => Some(0.0),
                Some(distance_m) => {
                    let top = self.grid.len() - 1;
                    let entry = self.grid.cross(origin_m, dir, distance_m, top, Next::Space);
                    self.depth_to_space_from(entry, dir, top, f64::INFINITY)
                }
            },
        }
    }

    /// One sample of the radiance (I, Q, U) at `source`, or of the flux for a hemisphere; Q and U
    /// are zero in the scalar mode.
    pub(crate) fn sample(
        &self,
        source: &Source,
        polarisation: Polarisation,
        draws: &mut Draws,
        scratch: &mut Scratch,
    ) -> [f64; 3] {
        let (look, e1, scale) = match source.aim {
            Aim::Cone {
                axis,
                e1,
                half_angle_rad,
            } => {
                let look = in_cone(axis, half_angle_rad, draws);
                (look, (e1 - look * look.dot(e1)).normalised(), 1.0)
            }
            Aim::Hemisphere { normal } => {
                let look = cosine_weighted(normal, draws);
                (look, basis(look).0, PI)
            }
        };
        let (start_m, shell, cut_m) = match self.grid.locate(source.origin_m, look) {
            Some(shell) => (source.origin_m, shell, source.cut_m),
            None => match self.grid.entry_distance_m(source.origin_m, look) {
                Some(distance_m) if distance_m < source.cut_m => {
                    let top = self.grid.len() - 1;
                    let entry =
                        self.grid
                            .cross(source.origin_m, look, distance_m, top, Next::Space);
                    (entry, top, source.cut_m - distance_m)
                }
                Some(_) | None => return [0.0; 3],
            },
        };
        let stokes = match polarisation {
            Polarisation::Scalar => None,
            Polarisation::Stokes => Some(Stokes::new(e1)),
        };
        let mut out = [0.0; 3];
        let end = self.first_flight(start_m, look, shell, cut_m, &mut scratch.pieces);
        let scattering_depth: f64 = scratch.pieces.iter().map(|p| p.scattering_depth).sum();
        let absorption_depth: f64 = scratch.pieces.iter().map(|p| p.absorption_depth).sum();
        if scattering_depth > 0.0 {
            let collided = -math::exp_m1(-scattering_depth);
            let target = -math::ln_1p(-draws.uniform() * collided);
            let (at_m, shell, absorbed) = self.forced_collision(&scratch.pieces, look, target);
            let weight = collided * math::exp(-absorbed);
            let walker = Walker::new(at_m, look, shell, weight, stokes);
            self.follow(
                Vertex::Scatter,
                walker,
                &source.suns,
                draws,
                scratch,
                &mut out,
            );
        }
        if let FirstEnd::Ground(at_m) = end {
            let weight = math::exp(-scattering_depth - absorption_depth);
            let walker = Walker::new(at_m, look, 0, weight, stokes);
            self.follow(
                Vertex::Ground,
                walker,
                &source.suns,
                draws,
                scratch,
                &mut out,
            );
        }
        out.map(|x| x * scale)
    }

    /// Cuts the ray from `start_m` in shell `shell` along `dir` into its shells' pieces, as far
    /// as the ground, space or `cut_m`, with each piece's scattering and absorption optical
    /// depths.
    fn first_flight(
        &self,
        start_m: Vec3,
        dir: Vec3,
        mut shell: usize,
        cut_m: f64,
        pieces: &mut Vec<Piece>,
    ) -> FirstEnd {
        pieces.clear();
        let mut point = start_m;
        let mut travelled_m = 0.0;
        loop {
            let exit = self.grid.exit(point, dir, shell);
            let length_m = exit.distance_m.min(cut_m - travelled_m);
            let (scattering_depth, absorption_depth) = self.depths(point, dir, length_m);
            pieces.push(Piece {
                start_m: point,
                length_m,
                shell,
                scattering_depth,
                absorption_depth,
            });
            if exit.distance_m >= cut_m - travelled_m {
                return FirstEnd::Lost;
            }
            travelled_m += exit.distance_m;
            let crossed = self
                .grid
                .cross(point, dir, exit.distance_m, shell, exit.next);
            match exit.next {
                Next::Shell(j) => {
                    point = crossed;
                    shell = j;
                }
                Next::Ground => return FirstEnd::Ground(crossed),
                Next::Space => return FirstEnd::Lost,
            }
        }
    }

    /// The point at scattering optical depth `target` along the first flight's `pieces`, m, its
    /// shell, and the absorption optical depth up to it.
    fn forced_collision(&self, pieces: &[Piece], dir: Vec3, target: f64) -> (Vec3, usize, f64) {
        let mut before = 0.0;
        let mut absorbed = 0.0;
        let last = pieces.len() - 1;
        for (i, piece) in pieces.iter().enumerate() {
            if i < last && before + piece.scattering_depth <= target {
                before += piece.scattering_depth;
                absorbed += piece.absorption_depth;
                continue;
            }
            // The scattering coefficient's degree-15 interpolant on the piece, scaled to the
            // piece's 8-point depth, inverted by bisection.
            let nodes = Gl16Panel::nodes(0.0, piece.length_m);
            let values = nodes.map(|t| self.scattering_per_m_at(piece.start_m + dir * t));
            let panel = Gl16Panel::new(0.0, piece.length_m, &values);
            let total = panel.integral();
            let fraction = if piece.scattering_depth > 0.0 {
                ((target - before) / piece.scattering_depth).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance_m = bisect(
                |t| panel.integral_to(t) - fraction * total,
                0.0,
                piece.length_m,
                64,
            );
            return (
                piece.start_m + dir * distance_m,
                piece.shell,
                absorbed + self.absorption_depth(piece.start_m, dir, distance_m),
            );
        }
        unreachable!("the first flight has at least one piece")
    }

    /// A free flight from the walker by delta tracking, its weight taking the flight's absorption.
    fn fly(&self, walker: &mut Walker, draws: &mut Draws) -> Event {
        let mut absorbed = 0.0;
        loop {
            let exit = self.grid.exit(walker.position_m, walker.dir, walker.shell);
            let majorant = self.majorant_per_m[walker.shell];
            if majorant > 0.0 {
                let mut distance_m = 0.0;
                loop {
                    distance_m -= math::ln(draws.uniform()) / majorant;
                    if distance_m >= exit.distance_m {
                        break;
                    }
                    let x = walker.position_m + walker.dir * distance_m;
                    if draws.uniform() * majorant < self.scattering_per_m_at(x) {
                        absorbed +=
                            self.absorption_depth(walker.position_m, walker.dir, distance_m);
                        walker.position_m = x;
                        walker.weight *= math::exp(-absorbed);
                        return Event::Scatter;
                    }
                }
            }
            absorbed += self.absorption_depth(walker.position_m, walker.dir, exit.distance_m);
            match exit.next {
                Next::Shell(j) => {
                    walker.position_m = self.grid.cross(
                        walker.position_m,
                        walker.dir,
                        exit.distance_m,
                        walker.shell,
                        exit.next,
                    );
                    walker.shell = j;
                }
                Next::Ground => {
                    walker.position_m = self.grid.cross(
                        walker.position_m,
                        walker.dir,
                        exit.distance_m,
                        walker.shell,
                        exit.next,
                    );
                    walker.shell = 0;
                    walker.weight *= math::exp(-absorbed);
                    return Event::Ground;
                }
                Next::Space => return Event::Escape,
            }
        }
    }

    /// The mixture of the terms' phase functions at cos Θ, weighted by `terms_per_m`'s
    /// scattering coefficients totalling `total_per_m`, sr⁻¹.
    fn phase_value(&self, terms_per_m: &[f64], total_per_m: f64, cos_theta: f64) -> f64 {
        self.phases
            .iter()
            .zip(terms_per_m)
            .map(|(phase, &s)| s * phase.value(cos_theta))
            .sum::<f64>()
            / total_per_m
    }

    /// The mixture of the terms' scattering matrices at cos Θ, as
    /// [`phase_value`](Self::phase_value) mixes.
    fn phase_matrix(
        &self,
        terms_per_m: &[f64],
        total_per_m: f64,
        cos_theta: f64,
    ) -> ScatteringMatrix {
        let mut z = ScatteringMatrix::default();
        for (phase, &s) in self.phases.iter().zip(terms_per_m) {
            if s > 0.0 {
                z.add_scaled(s / total_per_m, &phase.matrix(cos_theta));
            }
        }
        z
    }

    /// Follows a path from a vertex until it escapes or is lost to roulette, adding its
    /// next-event estimates to `out`.
    fn follow(
        &self,
        mut vertex: Vertex,
        mut walker: Walker,
        suns: &[Vec3],
        draws: &mut Draws,
        scratch: &mut Scratch,
        out: &mut [f64; 3],
    ) {
        loop {
            match vertex {
                Vertex::Scatter => {
                    if !self.scatter(&mut walker, suns, draws, scratch, out) {
                        return;
                    }
                }
                Vertex::Ground => self.reflect(&mut walker, suns, draws, out),
            }
            if walker.weight <= 0.0 {
                return;
            }
            if walker.weight < walker.roulette {
                if draws.uniform() * walker.roulette < walker.weight {
                    walker.weight = walker.roulette;
                } else {
                    return;
                }
            }
            vertex = match self.fly(&mut walker, draws) {
                Event::Scatter => Vertex::Scatter,
                Event::Ground => Vertex::Ground,
                Event::Escape => return,
            };
        }
    }

    /// At a scattering vertex: adds each sun's next-event estimate to `out` and turns the walker
    /// into a direction drawn from the terms' phase functions there. `false` if nothing scatters
    /// at the vertex, which ends the path.
    fn scatter(
        &self,
        walker: &mut Walker,
        suns: &[Vec3],
        draws: &mut Draws,
        scratch: &mut Scratch,
        out: &mut [f64; 3],
    ) -> bool {
        let h = self.height_m(walker.position_m);
        scratch.terms_per_m.clear();
        scratch.terms_per_m.extend(
            self.profiles
                .iter()
                .zip(&self.scattering_per_m)
                .map(|(profile, &s)| s * profile.at(h)),
        );
        let total_per_m: f64 = scratch.terms_per_m.iter().sum();
        if total_per_m <= 0.0 {
            return false;
        }
        for (&sun, &irradiance) in suns.iter().zip(&self.irradiance) {
            if irradiance <= 0.0 {
                continue;
            }
            let transmittance = self.transmittance_to_space(walker.position_m, sun, walker.shell);
            if transmittance <= 0.0 {
                continue;
            }
            let scale = walker.weight * irradiance * transmittance;
            let cos_theta = sun.dot(walker.dir);
            match &walker.stokes {
                None => {
                    out[0] +=
                        scale * self.phase_value(&scratch.terms_per_m, total_per_m, cos_theta);
                }
                Some(stokes) => {
                    let z = self.phase_matrix(&scratch.terms_per_m, total_per_m, cos_theta);
                    for (o, x) in out.iter_mut().zip(stokes.next_event(&z, walker.dir, sun)) {
                        *o += scale * x;
                    }
                }
            }
        }
        // The scattering term, in proportion to its coefficient here: the last that scatters if
        // rounding leaves the pick past the total.
        let pick = draws.uniform() * total_per_m;
        let mut running = 0.0;
        let mut phase = None;
        for (&candidate, &s) in self.phases.iter().zip(&scratch.terms_per_m) {
            if s > 0.0 {
                running += s;
                phase = Some(candidate);
                if pick < running {
                    break;
                }
            }
        }
        let Some(phase) = phase else { return false };
        let cos_theta = phase.sample(draws);
        let new = turned(walker.dir, cos_theta, 2.0 * PI * draws.uniform());
        if let Some(stokes) = &mut walker.stokes {
            let z = self.phase_matrix(&scratch.terms_per_m, total_per_m, cos_theta);
            let per_phase = ScatteringMatrix {
                a1: 1.0,
                a2: z.a2 / z.a1,
                a3: z.a3 / z.a1,
                b1: z.b1 / z.a1,
            };
            stokes.scatter(&per_phase, walker.dir, new);
        }
        walker.dir = new;
        true
    }

    /// At the ground: adds each sun's next-event estimate to `out` and reflects the walker into a
    /// cosine-weighted direction, its weight times the albedo.
    fn reflect(&self, walker: &mut Walker, suns: &[Vec3], draws: &mut Draws, out: &mut [f64; 3]) {
        let normal = walker.position_m.normalised();
        for (&sun, &irradiance) in suns.iter().zip(&self.irradiance) {
            let cos_sun = normal.dot(sun);
            if irradiance <= 0.0 || cos_sun <= 0.0 || self.ground_albedo <= 0.0 {
                continue;
            }
            let transmittance = self.transmittance_to_space(walker.position_m, sun, 0);
            let scale =
                walker.weight * self.ground_albedo / PI * cos_sun * irradiance * transmittance;
            match &walker.stokes {
                None => out[0] += scale,
                Some(stokes) => {
                    for (o, row) in out.iter_mut().zip(&stokes.m) {
                        *o += scale * row[0];
                    }
                }
            }
        }
        walker.weight *= self.ground_albedo;
        walker.dir = cosine_weighted(normal, draws);
        walker.shell = 0;
        if let Some(stokes) = &mut walker.stokes {
            stokes.depolarise(walker.dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::galaxy::quad::gl_panels;
    use serde_json::json;

    use super::*;

    #[test]
    fn atmosphere_grazing_optical_depths_match_a_fine_quadrature() {
        let (radius, top) = (6_371_000.0, 100_000.0);
        let case = AtmosphereCase::from_json(
            &json!({
                "format": 1, "name": "grazing",
                "wavelengthsNm": [550],
                "shells": { "kind": "sphere", "radiusM": radius },
                "topHeightM": top,
                "groundAlbedo": [0.0],
                "terms": [
                    {
                        "name": "gas", "density": { "kind": "exponential", "scaleHeightM": 8000.0 },
                        "scattering": [1.15e-5], "absorption": [0.0],
                        "phase": { "kind": "rayleigh", "depolarisation": [0.0] }
                    },
                    {
                        "name": "haze", "density": { "kind": "exponential", "scaleHeightM": 1200.0 },
                        "scattering": [4e-6], "absorption": [4e-7],
                        "phase": { "kind": "isotropic" }
                    },
                    {
                        "name": "ozone",
                        "density": { "kind": "tent", "bottomM": 10e3, "peakM": 25e3, "topM": 40e3 },
                        "scattering": [0.0], "absorption": [1.9e-6], "phase": { "kind": "none" }
                    }
                ],
                "suns": [{ "name": "sun", "irradiance": [1.0] }],
                "detectorHalfAngleDeg": 0.0,
                "geometries": [{
                    "name": "unused", "observer": { "heightM": 0.0 },
                    "view": { "zenithDeg": 0.0, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": 0.0, "azimuthDeg": 0.0 }]
                }]
            })
            .to_string(),
        )
        .unwrap();
        let grid = super::super::shell_grid(&case);
        let medium = Medium::new(&case, &grid, 0);
        let frame = LocalFrame::at_latitude(0.2);
        // The extinction, written out again.
        let extinction = |x: Vec3| {
            let h = (x.length() - radius).max(0.0);
            let ozone = if (10e3..25e3).contains(&h) {
                (h - 10e3) / 15e3
            } else if (25e3..40e3).contains(&h) {
                (40e3 - h) / 15e3
            } else {
                0.0
            };
            1.15e-5 * math::exp(-h / 8000.0) + 4.4e-6 * math::exp(-h / 1200.0) + 1.9e-6 * ozone
        };
        // From orbit through tangent heights of 0.5, 5 and 30 km, and from 2 km horizontally.
        let orbit = frame.up * (radius + 400_000.0);
        let mut rays: Vec<(Vec3, Vec3)> = [500.0, 5_000.0, 30_000.0]
            .iter()
            .map(|&tangent| {
                let zenith = PI - math::asin((radius + tangent) / (radius + 400_000.0));
                (orbit, frame.direction(zenith, 0.7))
            })
            .collect();
        rays.push((
            frame.up * (radius + 2_000.0),
            frame.direction(0.5 * PI, 2.0),
        ));
        for (origin, dir) in rays {
            let depth = medium.optical_depth_to_space(origin, dir).unwrap();
            // The chord inside the top sphere, in 4,000 panels of 32 points and an edge where it
            // crosses each of the ozone's kinks.
            let (along, r) = (origin.dot(dir), origin.length());
            let crossings = |height: f64| {
                let reach = along * along - (r * r - (radius + height) * (radius + height));
                let reach = reach.max(0.0).sqrt();
                [-along - reach, -along + reach]
            };
            let [enter, leave] = crossings(top);
            let enter = enter.max(0.0);
            let mut edges: Vec<f64> = (0..=4000)
                .map(|i| enter + (leave - enter) * f64::from(i) / 4000.0)
                .chain([10e3, 25e3, 40e3].into_iter().flat_map(crossings))
                .filter(|&t| (enter..=leave).contains(&t))
                .collect();
            edges.sort_by(f64::total_cmp);
            let expected = gl_panels(|t| extinction(origin + dir * t), &edges);
            assert!(
                (depth / expected - 1.0).abs() < 1e-10,
                "{depth} against {expected}"
            );
        }
    }
}
