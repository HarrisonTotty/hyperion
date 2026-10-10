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
//! - **Directions drawn towards the suns** (R08.T12.c) where the medium is forward-peaked: the
//!   detector directional importance sampling (DDIS) of Buras and Mayer 2011 ("Efficient unbiased
//!   variance reduction techniques for Monte Carlo simulations of radiative transfer in cloudy
//!   atmospheres: The solution", JQSRT 112, 434; doi:10.1016/j.jqsrt.2010.10.005), with
//!   the suns in the detector's place, since this tracer runs backwards. A path's next-event
//!   estimate at its next vertex is the phase function between its direction and a sun's, so in an
//!   aerosol or a cloud the rare path that happens to point at a sun scores up to a thousand times
//!   the others, and the estimate converges slowly and from below (measured on IPRT's spheroids,
//!   R08's Risks). At a vertex whose phase mixture's forward value passes
//!   [`FORWARD_PEAK_PER_SR`], a fraction [`SUNWARD_FRACTION`] of the new directions is drawn from
//!   the mixture about one of the case's suns instead of the old direction, and the path is
//!   weighted by the phase function over the two branches' density, at most
//!   1 ÷ (1 − [`SUNWARD_FRACTION`]) for an exact draw: unbiased, and the peak is sampled as often as
//!   it matters.
//!   Elsewhere, Rayleigh's sky included, directions are drawn as before. The roulette's threshold
//!   scales with every such importance weight (a Legendre series' too), so that the roulette
//!   answers to losses alone.
//!
//! The quadrature is 8-point Gauss–Legendre on each shell's piece of a ray, where the integrand is
//! smooth: the profiles' kinks are shell boundaries, and an exponential profile changes by at most
//! e across a shell.
//!
//! **Over a level spheroid** ([`spheroid`](super::spheroid), R08.T12.d) nothing here changes but
//! the grid's: every profile is read at the gravity-scaled height h\* = s h, the shells are quadrics
//! through h\* = H at the equator and the poles, each shell's majorant bounds its profiles over
//! every h\* the shell can hold, and the ground's normal is the datum's. A kink lies on a surface
//! of constant h\*, which the quadrics leave between the equator and the poles (by up to 1.7% of
//! its height on Saturn), so a piece can hold one near its end. Measured on Saturn's grazing
//! chords: an exponential profile agrees with an `f64` quadrature to 4 × 10⁻¹⁰, R08.T3.a's column
//! (1,024 levels, each a slight kink) to 3 × 10⁻⁹, and a tent, whose kinks are sharp, to
//! 1.4 × 10⁻⁶.
//!
//! **The Stokes mode** carries, beside the scalar weight, the 4 × 4 matrix that takes the Stokes
//! vector (I, Q, U, V) of the light arriving along the current flight, in that flight's frame, to
//! the detector's. The light's frame for a propagation direction k is (e₁, e₂ = k × e₁); a
//! rotation of the frame by ψ, e₁′ = cos ψ e₁ + sin ψ e₂, takes (Q, U) to
//! (cos 2ψ Q + sin 2ψ U, −sin 2ψ Q + cos 2ψ U) and leaves I and V, and the scattering matrix, of
//! the block-diagonal form of the [`optics`](super::optics) module, acts in the scattering plane's
//! frame, e₂ its normal (Hovenier, van der Mee and Domke 2004, §1.4 and §2.3; Chandrasekhar 1950,
//! §15). Every term scatters by its own matrix, or by a₁ alone where the case marks it
//! `depolarising`. Directions are drawn from the phase function, the matrix divided by it, so that
//! the scalar and the Stokes modes follow the same paths from the same draws, and the Stokes mode
//! scores the scalar estimate of those paths beside the vector one: their difference is the scalar
//! approximation's error alone, and for depolarising terms they give the same I. Light is
//! unpolarised at the suns and after the ground; V arises only through b₂, from U.

use core::f64::consts::PI;

use hyperion_sim::galaxy::quad::{Gl16Panel, bisect};
use hyperion_sim::math;
use hyperion_sim::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};

use super::Polarisation;
use super::case::{Aggregate, AtmosphereCase, DensityProfile, Geometry, LocalDirection};
use super::geometry::{LocalFrame, Next, ShellGrid, Vec3, basis, cosine_weighted, in_cone, turned};
use super::optics::{Phase, ScatteringMatrix};
use crate::tasks::displaced_forms::births::Draws;

/// The components of a sample: the radiance (I, Q, U, V) and, in the Stokes mode, the scalar
/// radiance of the same paths. The scalar mode fills [`I`] alone.
pub(crate) const COMPONENTS: usize = 5;

/// A sample's radiance, the Stokes vector's I.
pub(crate) const I: usize = 0;

/// A sample's scalar radiance, in the Stokes mode.
pub(crate) const SCALAR_I: usize = 4;

/// Below this fraction of its branch's starting weight a path plays Russian roulette, surviving
/// with probability weight ÷ (this × the starting weight) and carrying that threshold if it does.
pub(crate) const ROULETTE_FRACTION: f64 = 0.1;

/// A vertex's phase mixture is forward-peaked, and its new directions are drawn partly towards the
/// suns, where its forward value passes this, sr⁻¹: some 12 times the isotropic 1 ÷ 4π, and
/// eight times Rayleigh's largest, which a Henyey–Greenstein phase function reaches at g ≈ 0.65.
pub(crate) const FORWARD_PEAK_PER_SR: f64 = 1.0;

/// The fraction of a forward-peaked vertex's new directions drawn about a sun (module
/// documentation): enough to sample the peak, at a cost of at most 1 ÷ (1 − this) in the weight
/// of the others, for an exact draw.
pub(crate) const SUNWARD_FRACTION: f64 = 0.2;

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
    /// Per term, its phase function forward (cos Θ = 1), sr⁻¹: what decides whether a vertex is
    /// forward-peaked.
    forward_per_sr: Vec<f64>,
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
    /// The detector of `geometry` in `case`, over `grid`.
    #[must_use]
    pub(crate) fn detector(case: &AtmosphereCase, grid: &ShellGrid, geometry: &Geometry) -> Self {
        let frame = LocalFrame::at_latitude(geometry.observer.latitude_deg.to_radians());
        let (zenith, azimuth) = (
            geometry.view.zenith_deg.to_radians(),
            geometry.view.azimuth_deg.to_radians(),
        );
        Self {
            origin_m: grid.point(&frame, geometry.observer.height_m),
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
    /// top above it from below, over `grid`. Horizontal is normal to the local vertical at the
    /// ground, which on a spheroid the top's quadric is not quite.
    #[must_use]
    pub(crate) fn flux(grid: &ShellGrid, aggregate: &Aggregate, level: Level) -> Self {
        let frame = LocalFrame::at_latitude(aggregate.latitude_deg.to_radians());
        let (origin_m, normal) = match level {
            Level::Ground => (grid.point(&frame, 0.0), frame.up),
            Level::Top => (grid.top_point(&frame), -frame.up),
        };
        Self {
            origin_m,
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

/// `n` as an `f64`, for the small counts of suns.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a case has fewer than 2³² suns"))
}

/// An index drawn uniformly from `0..n`, `n` positive; no draw for `n` = 1.
fn uniform_index(n: usize, draws: &mut Draws) -> usize {
    if n == 1 {
        return 0;
    }
    let target = draws.uniform() * count(n);
    (1..n).take_while(|&k| count(k) <= target).count()
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
    /// Takes (I, Q, U, V) arriving along the current flight, in its frame, to the detector's
    /// frame.
    m: [[f64; 4]; 4],
    /// The current flight's first Stokes axis, perpendicular to it.
    e1: Vec3,
}

impl Stokes {
    fn new(e1: Vec3) -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
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

    /// (I, Q, U, V) at the detector of unit unpolarised light from the unit vector `sun`
    /// scattered towards −`dir` by `z`: singly scattered, it carries no V.
    fn next_event(&self, z: &ScatteringMatrix, dir: Vec3, sun: Vec3) -> [f64; 4] {
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
        // R(ψ) Z: rows of the rotation [[1, 0, 0, 0], [0, c2, s2, 0], [0, −s2, c2, 0],
        // [0, 0, 0, 1]] times [[a1, b1, 0, 0], [b1, a2, 0, 0], [0, 0, a3, b2], [0, 0, −b2, a4]].
        let a = [
            [z.a1, z.b1, 0.0, 0.0],
            [c2 * z.b1, c2 * z.a2, s2 * z.a3, s2 * z.b2],
            [-s2 * z.b1, -s2 * z.a2, c2 * z.a3, c2 * z.b2],
            [0.0, 0.0, -z.b2, z.a4],
        ];
        self.m = self.m.map(|row| {
            [0, 1, 2, 3]
                .map(|j| row[0] * a[0][j] + row[1] * a[1][j] + row[2] * a[2][j] + row[3] * a[3][j])
        });
        self.e1 = new.cross(n);
    }

    /// Depolarises the path, as a Lambertian reflection does, into the flight along `dir`.
    fn depolarise(&mut self, dir: Vec3) {
        for row in &mut self.m {
            row[1] = 0.0;
            row[2] = 0.0;
            row[3] = 0.0;
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
                let (low_m, high_m) = grid.scaled_height_bounds_m(shell);
                profiles
                    .iter()
                    .zip(&scattering_per_m)
                    .map(|(profile, &s)| s * profile.max_over(low_m, high_m))
                    .sum::<f64>()
                    * MAJORANT_MARGIN
            })
            .collect();
        let phases: Vec<Phase> = terms
            .iter()
            .map(|t| Phase::of(&t.phase, wavelength))
            .collect();
        Self {
            grid,
            forward_per_sr: phases.iter().map(|p| p.value(1.0)).collect(),
            phases,
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

    /// The height the profiles are read at, at `x`: above the ground on a sphere, gravity-scaled
    /// on a spheroid, m.
    fn height_m(&self, x: Vec3) -> f64 {
        self.grid.scaled_height_m(x)
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

    /// One sample of the radiance (I, Q, U, V) and the scalar radiance of the same path at
    /// `source` ([`COMPONENTS`]), or of the fluxes for a hemisphere; all but [`I`] are zero in the
    /// scalar mode.
    pub(crate) fn sample(
        &self,
        source: &Source,
        polarisation: Polarisation,
        draws: &mut Draws,
        scratch: &mut Scratch,
    ) -> [f64; COMPONENTS] {
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
                Some(_) | None => return [0.0; COMPONENTS],
            },
        };
        let stokes = match polarisation {
            Polarisation::Scalar => None,
            Polarisation::Stokes => Some(Stokes::new(e1)),
        };
        let mut out = [0.0; COMPONENTS];
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

    /// The density the terms' draws have at cos Θ, mixed as [`phase_value`](Self::phase_value)
    /// mixes, sr⁻¹: the phase function but for a Legendre series, drawn from its proposal.
    fn sampling_density(&self, terms_per_m: &[f64], total_per_m: f64, cos_theta: f64) -> f64 {
        self.phases
            .iter()
            .zip(terms_per_m)
            .map(|(phase, &s)| s * phase.sampling_density(cos_theta))
            .sum::<f64>()
            / total_per_m
    }

    /// The mixture of the terms' scattering matrices at cos Θ, as
    /// [`phase_value`](Self::phase_value) mixes: a term without a matrix, which the Stokes mode
    /// takes only where the case marks it depolarising, by a₁ alone.
    fn phase_matrix(
        &self,
        terms_per_m: &[f64],
        total_per_m: f64,
        cos_theta: f64,
    ) -> ScatteringMatrix {
        let mut z = ScatteringMatrix::default();
        for (phase, &s) in self.phases.iter().zip(terms_per_m) {
            if s > 0.0 {
                let matrix = phase
                    .matrix(cos_theta)
                    .unwrap_or_else(|| ScatteringMatrix::depolarising(phase.value(cos_theta)));
                z.add_scaled(s / total_per_m, &matrix);
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
        out: &mut [f64; COMPONENTS],
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
    /// into a direction drawn from the terms' phase functions there, its weight times the draw's.
    /// `false` if nothing scatters at the vertex, or the draw carries no weight, which ends the
    /// path.
    fn scatter(
        &self,
        walker: &mut Walker,
        suns: &[Vec3],
        draws: &mut Draws,
        scratch: &mut Scratch,
        out: &mut [f64; COMPONENTS],
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
            let scalar = scale * self.phase_value(&scratch.terms_per_m, total_per_m, cos_theta);
            match &walker.stokes {
                None => out[I] += scalar,
                Some(stokes) => {
                    let z = self.phase_matrix(&scratch.terms_per_m, total_per_m, cos_theta);
                    for (o, x) in out.iter_mut().zip(stokes.next_event(&z, walker.dir, sun)) {
                        *o += scale * x;
                    }
                    out[SCALAR_I] += scalar;
                }
            }
        }
        // The scattering term, in proportion to its coefficient here: the last that scatters if
        // rounding leaves the pick past the total.
        let pick = draws.uniform() * total_per_m;
        let mut running = 0.0;
        let mut phase = None;
        for (candidate, &s) in self.phases.iter().zip(&scratch.terms_per_m) {
            if s > 0.0 {
                running += s;
                phase = Some(candidate);
                if pick < running {
                    break;
                }
            }
        }
        let Some(phase) = phase else { return false };
        let terms = &scratch.terms_per_m;
        let forward = self
            .forward_per_sr
            .iter()
            .zip(terms)
            .map(|(&f, &s)| s * f)
            .sum::<f64>()
            / total_per_m;
        let (new, cos_theta, weight) = if forward > FORWARD_PEAK_PER_SR {
            // DDIS (module documentation): the direction from the mixture, about the old one or
            // about a sun's, weighted by the phase function over both branches' density. Every sun
            // of the case is a target, lit or not at this wavelength, so that the draws do not
            // depend on the irradiance and the radiance stays linear in it path by path.
            let towards_sun = draws.uniform() < SUNWARD_FRACTION;
            let draw = phase.sample(draws);
            let axis = if towards_sun {
                suns[uniform_index(suns.len(), draws)]
            } else {
                walker.dir
            };
            let new = turned(axis, draw.cos_theta, 2.0 * PI * draws.uniform());
            let cos_theta = walker.dir.dot(new).clamp(-1.0, 1.0);
            let sunward = suns
                .iter()
                .map(|&sun| {
                    self.sampling_density(terms, total_per_m, sun.dot(new).clamp(-1.0, 1.0))
                })
                .sum::<f64>()
                / count(suns.len());
            let density = (1.0 - SUNWARD_FRACTION)
                * self.sampling_density(terms, total_per_m, cos_theta)
                + SUNWARD_FRACTION * sunward;
            (
                new,
                cos_theta,
                self.phase_value(terms, total_per_m, cos_theta) / density,
            )
        } else {
            let draw = phase.sample(draws);
            let new = turned(walker.dir, draw.cos_theta, 2.0 * PI * draws.uniform());
            (new, draw.cos_theta, draw.weight)
        };
        // A Legendre series' draw is weighted by the series over its proposal; where the series
        // is not positive between the proposal's entries, or a table is zero, the path ends, as
        // it carries nothing.
        if weight <= 0.0 {
            return false;
        }
        if let Some(stokes) = &mut walker.stokes {
            let z = self.phase_matrix(terms, total_per_m, cos_theta);
            stokes.scatter(&z.per_phase(), walker.dir, new);
        }
        walker.weight *= weight;
        // An importance weight is not a loss: the roulette's threshold scales with it, so that a
        // path drawn towards a sun with a small weight reaches the vertex whose peak it was drawn
        // for, rather than being rouletted before it and scoring a hundredfold if it survives.
        walker.roulette *= weight;
        walker.dir = new;
        true
    }

    /// At the ground: adds each sun's next-event estimate to `out` and reflects the walker into a
    /// cosine-weighted direction, its weight times the albedo.
    fn reflect(
        &self,
        walker: &mut Walker,
        suns: &[Vec3],
        draws: &mut Draws,
        out: &mut [f64; COMPONENTS],
    ) {
        let normal = self.grid.vertical(walker.position_m);
        for (&sun, &irradiance) in suns.iter().zip(&self.irradiance) {
            let cos_sun = normal.dot(sun);
            if irradiance <= 0.0 || cos_sun <= 0.0 || self.ground_albedo <= 0.0 {
                continue;
            }
            let transmittance = self.transmittance_to_space(walker.position_m, sun, 0);
            let scale =
                walker.weight * self.ground_albedo / PI * cos_sun * irradiance * transmittance;
            match &walker.stokes {
                None => out[I] += scale,
                Some(stokes) => {
                    for (o, row) in out.iter_mut().zip(&stokes.m) {
                        *o += scale * row[0];
                    }
                    out[SCALAR_I] += scale;
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
pub(crate) mod tests {
    use hyperion_sim::galaxy::quad::gl_panels;
    use serde_json::json;

    use super::super::spheroid;
    use super::*;

    #[test]
    fn atmosphere_sun_indices_are_drawn_uniformly() {
        // 30,000 draws among three suns: each count within 4σ of 10,000 (σ = √(n p (1 − p)) =
        // 81.6), and one sun takes no draw at all.
        let mut draws = Draws::new(5, 0, 0);
        let mut counts = [0_u32; 3];
        for _ in 0..30_000 {
            counts[uniform_index(3, &mut draws)] += 1;
        }
        for count in counts {
            assert!(
                (f64::from(count) - 10_000.0).abs() < 4.0 * 81.65,
                "{counts:?}"
            );
        }
        let (mut once, mut twice) = (Draws::new(5, 1, 0), Draws::new(5, 1, 0));
        assert_eq!(uniform_index(1, &mut once), 0);
        assert!((once.uniform() - twice.uniform()).abs() < f64::MIN_POSITIVE);
    }

    #[test]
    fn atmosphere_stokes_state_couples_u_and_v_through_b2() {
        // Two scatterings in planes at right angles, by a matrix with every element: the path's
        // matrix is R(ψ₂)Z₂ R(ψ₁)Z₁ written out, and its V row takes U through −b₂.
        let z = ScatteringMatrix {
            a1: 1.0,
            a2: 0.9,
            a3: 0.6,
            a4: 0.5,
            b1: -0.3,
            b2: 0.4,
        };
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        // The detector's flight along +z with e₁ = x; it scatters from the flight along x, in the
        // plane of x and z, whose normal is the current frame's e₂ = −y: no rotation.
        let mut stokes = Stokes::new(x);
        stokes.scatter(&z, up, x);
        let zm = [
            [z.a1, z.b1, 0.0, 0.0],
            [z.b1, z.a2, 0.0, 0.0],
            [0.0, 0.0, z.a3, z.b2],
            [0.0, 0.0, -z.b2, z.a4],
        ];
        for (i, row) in stokes.m.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                assert!(
                    (value - zm[i][j]).abs() < 1e-15,
                    "{i}{j}: {value} {}",
                    zm[i][j]
                );
            }
        }
        // Then from the flight along y: the plane of x and y, normal z, turned 90° from the
        // first, so the rotation is cos 2ψ = −1: Q and U change sign, I and V do not.
        stokes.scatter(&z, x, y);
        let rotated = [
            [z.a1, z.b1, 0.0, 0.0],
            [-z.b1, -z.a2, 0.0, 0.0],
            [0.0, 0.0, -z.a3, -z.b2],
            [0.0, 0.0, -z.b2, z.a4],
        ];
        let expected: Vec<[f64; 4]> = zm
            .iter()
            .map(|row| [0, 1, 2, 3].map(|j| (0..4).map(|k| row[k] * rotated[k][j]).sum::<f64>()))
            .collect();
        for (i, row) in stokes.m.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                assert!((value - expected[i][j]).abs() < 1e-15, "{i}{j}: {value}");
            }
        }
        // V's row takes U through b₂ at each scattering: −b₂(−a₃) − a₄b₂ after the two.
        assert!((stokes.m[3][2] - (z.b2 * z.a3 - z.a4 * z.b2)).abs() < 1e-15);
        // A Lambertian reflection keeps I alone.
        stokes.depolarise(up);
        assert!(
            stokes
                .m
                .iter()
                .all(|row| row[1..].iter().all(|v| v.abs() < f64::MIN_POSITIVE))
        );
    }

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

    /// Saturn's γₑ and γₚ, m s⁻², as the client's `normalGravity` prints them (the spheroid
    /// module's test of the two), so that the quadratures below share no gravity code with the
    /// tracer.
    pub(crate) const SATURN_GAMMA: (f64, f64) = (9.076_628_614_761_937, 12.036_945_078_629_161);

    /// Saturn's scale height for these tests, m: about RT ÷ μg at 1 bar, 134 K, μ = 2.3 and
    /// `g_ref` (46.3 km); NASA's Saturn fact sheet's μ = 2.07 would give 51 km. Illustrative.
    const SATURN_SCALE_HEIGHT_M: f64 = 47_000.0;

    /// The gravity-scaled height of `x` over Saturn's figure, m, with no code of the tracer's:
    /// the geodetic latitude by bisection on the condition that the datum's normal there passes
    /// through `x`, ρ sin φ − z cos φ − e² N sin φ cos φ = 0 (one root on the half meridian
    /// outside the evolute), the height along that normal, ρ cos φ + z sin φ − a √(1 − e² sin²φ),
    /// and Somigliana's gravity in NIMA TR8350.2's form, γₑ (1 + k sin²φ) ÷ √(1 − e² sin²φ) with
    /// k = c γₚ ÷ (a γₑ) − 1 (eq. 4-1).
    #[expect(
        clippy::many_single_char_names,
        reason = "the geodesy's own a, c, ρ's z, k and w, so that the formulas read as cited"
    )]
    pub(crate) fn saturn_scaled_height_m(x: Vec3) -> f64 {
        let (a, c) = (spheroid::tests::SATURN.0, spheroid::tests::SATURN.1);
        let e2 = 1.0 - (c / a) * (c / a);
        let (rho, z) = ((x.x * x.x + x.y * x.y).sqrt(), x.z);
        let condition = |phi: f64| {
            let (sin, cos) = math::sin_cos(phi);
            rho * sin - z * cos - e2 * a / (1.0 - e2 * sin * sin).sqrt() * sin * cos
        };
        let (mut low, mut high) = (-0.5 * PI, 0.5 * PI);
        for _ in 0..64 {
            let mid = f64::midpoint(low, high);
            if condition(mid) < 0.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        let (sin, cos) = math::sin_cos(f64::midpoint(low, high));
        let w = (1.0 - e2 * sin * sin).sqrt();
        let height = rho * cos + z * sin - a * w;
        let (gamma_e, gamma_p) = SATURN_GAMMA;
        let k = c * gamma_p / (a * gamma_e) - 1.0;
        height * gamma_e * (1.0 + k * sin * sin) / w / (gamma_e * gamma_p).sqrt()
    }

    /// ∫ σ(h*) dt along the ray from `origin` along `dir` inside Saturn's medium, whose top is
    /// the quadric (a + top ÷ sₑ, c + top ÷ sₚ) through h\* = `top_height_m` at the equator and the
    /// poles (the spheroid module's definition, solved here), in 2,000 panels of 32 points.
    pub(crate) fn saturn_chord_depth(
        origin: Vec3,
        dir: Vec3,
        top_height_m: f64,
        extinction: impl Fn(f64) -> f64,
    ) -> f64 {
        let (a, c) = (spheroid::tests::SATURN.0, spheroid::tests::SATURN.1);
        let (gamma_e, gamma_p) = SATURN_GAMMA;
        let g_ref = (gamma_e * gamma_p).sqrt();
        let (big_a, big_c) = (
            a + top_height_m * g_ref / gamma_e,
            c + top_height_m * g_ref / gamma_p,
        );
        let form = |u: Vec3, v: Vec3| {
            (u.x * v.x + u.y * v.y) / (big_a * big_a) + u.z * v.z / (big_c * big_c)
        };
        let (quadratic, linear, constant) = (
            form(dir, dir),
            form(origin, dir),
            form(origin, origin) - 1.0,
        );
        let disc = (linear * linear - quadratic * constant).sqrt();
        let enter = ((-linear - disc) / quadratic).max(0.0);
        let leave = (-linear + disc) / quadratic;
        let edges: Vec<f64> = (0..=2000)
            .map(|i| enter + (leave - enter) * f64::from(i) / 2000.0)
            .collect();
        gl_panels(
            |t| extinction(saturn_scaled_height_m(origin + dir * t)),
            &edges,
        )
    }

    /// An absorbing-only Saturn-class case of `terms`, with its top at `top_height_m`.
    fn saturn_absorbing(terms: &serde_json::Value, top_height_m: f64) -> AtmosphereCase {
        let (a, c, gm, omega) = spheroid::tests::SATURN;
        AtmosphereCase::from_json(
            &json!({
                "format": 1, "name": "saturn-absorbing",
                "wavelengthsNm": [550],
                "shells": {
                    "kind": "spheroid", "equatorialRadiusM": a, "polarRadiusM": c,
                    "gmM3S2": gm, "angularVelocityRadS": omega
                },
                "topHeightM": top_height_m,
                "groundAlbedo": [0.0],
                "terms": terms,
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
        .unwrap()
    }

    /// The grazing rays of R08.T12.d's test: horizontal from 100 m at the equator north and east
    /// and at the pole, and tangent to the 1 H and 3 H levels over the same, from far outside.
    fn saturn_grazing_rays(grid: &ShellGrid) -> Vec<(String, Vec3, Vec3)> {
        let mut rays = Vec::new();
        for (place, latitude, azimuth) in [
            ("equator north", 0.0, 0.0),
            ("equator east", 0.0, 0.5 * PI),
            ("pole", 0.5 * PI, 0.0),
        ] {
            let frame = LocalFrame::at_latitude(latitude);
            let level = frame.direction(0.5 * PI, azimuth);
            rays.push((
                format!("{place} from 100 m"),
                grid.point(&frame, 100.0),
                level,
            ));
            for scale_heights in [1.0, 3.0] {
                // The geodetic height whose h* is 1 or 3 H there.
                let s = if latitude > 0.0 {
                    SATURN_GAMMA.1
                } else {
                    SATURN_GAMMA.0
                } / (SATURN_GAMMA.0 * SATURN_GAMMA.1).sqrt();
                let tangent = grid.point(&frame, scale_heights * SATURN_SCALE_HEIGHT_M / s);
                rays.push((
                    format!("{place} tangent at {scale_heights} H"),
                    tangent - level * 4e7,
                    level,
                ));
            }
        }
        rays
    }

    /// A table of e^(−h ÷ H) at R08.T3.a's column levels: 1,024 intervals even in ln p from the
    /// datum to 10⁻⁷ of its pressure (`column.ts`'s `COLUMN_INTERVALS` and
    /// `COLUMN_TOP_PRESSURE_RATIO`), linear between them, as an isothermal column is.
    fn column_like(scale_height_m: f64) -> (Vec<f64>, Vec<f64>) {
        let span = math::ln(1e7);
        let altitudes: Vec<f64> = (0..=1024)
            .map(|i| scale_height_m * span * f64::from(i) / 1024.0)
            .collect();
        let relative = altitudes
            .iter()
            .map(|&h| math::exp(-h / scale_height_m))
            .collect();
        (altitudes, relative)
    }

    /// A profile's density as the case writes it, its top, its relative density at h\*, and the
    /// tolerance its chords are held to.
    type ProfileCase<'a> = (serde_json::Value, f64, &'a dyn Fn(f64) -> f64, f64);

    #[test]
    fn atmosphere_spheroid_absorbing_medium_gives_beer_lambert_along_grazing_chords() {
        let (sigma, h) = (3e-6, SATURN_SCALE_HEIGHT_M);
        let (bottom, peak, top) = (2.0 * h, 4.0 * h, 7.0 * h);
        let tent = move |x: f64| {
            if x <= bottom || x >= top {
                0.0
            } else if x <= peak {
                (x - bottom) / (peak - bottom)
            } else {
                (top - x) / (top - peak)
            }
        };
        let (altitudes, relative) = column_like(h);
        let column_top = altitudes[altitudes.len() - 1];
        let column = |x: f64| {
            let x = x.max(0.0);
            let above = altitudes.partition_point(|&a| a <= x);
            if above == 0 {
                relative[0]
            } else if above == altitudes.len() {
                relative[above - 1]
            } else {
                let (a0, a1) = (altitudes[above - 1], altitudes[above]);
                relative[above - 1] + (relative[above] - relative[above - 1]) * (x - a0) / (a1 - a0)
            }
        };
        // Measured: the exponential agrees to 4 × 10⁻¹⁰ (the 8-point rule on the piece about the
        // tangent, as on a sphere), and T3.a's column, every level a kink, to 3 × 10⁻⁹: both to the
        // task's 10⁻⁶. A kink lies on a surface h* = const, which the quadrics leave between the
        // equator and the poles, so a piece's rule can straddle one; the column's are slight. A
        // tent's are not, and cost up to 1.4 × 10⁻⁶ (held to 10⁻⁵, R08's Risks): the chords north
        // and over the pole cross latitudes where the quadrics leave them, the chord east never
        // leaves the equator.
        let cases: [ProfileCase<'_>; 3] = [
            (
                json!({ "kind": "exponential", "scaleHeightM": h }),
                40.0 * h,
                &|x: f64| math::exp(-x.max(0.0) / h),
                1e-6,
            ),
            (
                json!({ "kind": "tabulated", "altitudesM": altitudes, "relative": relative }),
                column_top,
                &column,
                1e-6,
            ),
            (
                json!({ "kind": "tent", "bottomM": bottom, "peakM": peak, "topM": top }),
                40.0 * h,
                &tent,
                1e-5,
            ),
        ];
        for (density, top_height_m, profile, tolerance) in cases {
            let case = saturn_absorbing(
                &json!([{
                    "name": "gas", "density": density,
                    "scattering": [0.0], "absorption": [sigma], "phase": { "kind": "none" }
                }]),
                top_height_m,
            );
            let grid = super::super::shell_grid(&case);
            let medium = Medium::new(&case, &grid, 0);
            for (name, origin, dir) in saturn_grazing_rays(&grid) {
                let depth = medium.optical_depth_to_space(origin, dir).unwrap();
                let expected =
                    saturn_chord_depth(origin, dir, top_height_m, |x| sigma * profile(x));
                assert!(expected > 0.5, "{name}: {expected}");
                assert!(
                    (depth / expected - 1.0).abs() < tolerance,
                    "{name}, top {top_height_m}: {depth} against {expected}"
                );
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_majorants_bound_the_scattering_coefficient() {
        // An exponential gas, a tent haze, and a table whose peak and kinks fall inside shells,
        // over Saturn: every point of the meridian half-plane, on a grid, below its shell's bound.
        let h = SATURN_SCALE_HEIGHT_M;
        let case = saturn_absorbing(
            &json!([
                {
                    "name": "gas", "density": { "kind": "exponential", "scaleHeightM": h },
                    "scattering": [6e-6], "absorption": [0.0], "phase": { "kind": "isotropic" }
                },
                {
                    "name": "haze",
                    "density": { "kind": "tent", "bottomM": 1.3 * h, "peakM": 2.9 * h, "topM": 5.2 * h },
                    "scattering": [2e-6], "absorption": [1e-7], "phase": { "kind": "isotropic" }
                },
                {
                    "name": "cloud",
                    "density": {
                        "kind": "tabulated",
                        "altitudesM": [0.0, 0.4 * h, 0.75 * h, 2.1 * h],
                        "relative": [0.2, 1.0, 0.3, 0.0]
                    },
                    "scattering": [5e-6], "absorption": [0.0], "phase": { "kind": "isotropic" }
                }
            ]),
            40.0 * h,
        );
        let grid = super::super::shell_grid(&case);
        let medium = Medium::new(&case, &grid, 0);
        let mut tightest = f64::INFINITY;
        for i in -90..=90 {
            let frame = LocalFrame::at_latitude(f64::from(i).to_radians());
            for j in 0..=2_000 {
                let x = grid.point(&frame, f64::from(j) * 1_000.0);
                let Some(shell) = grid.locate(x, frame.up) else {
                    continue;
                };
                let coefficient = medium.scattering_per_m_at(x);
                let majorant = medium.majorant_per_m[shell];
                assert!(
                    coefficient <= majorant,
                    "{i}°, {j} km, shell {shell}: {coefficient} above {majorant}"
                );
                if coefficient > 1e-9 {
                    tightest = tightest.min(majorant / coefficient);
                }
            }
        }
        assert!(tightest < 1.01, "{tightest}");
    }
}
