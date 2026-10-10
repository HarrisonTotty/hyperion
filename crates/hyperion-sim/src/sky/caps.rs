//! Layer caps: how far out the census looks in each layer and each direction (rendering plan R06,
//! Design note 9; decided 2026-10-03, `decision-r06-t7-caps.md`, and one radius a ray since
//! R06.T7.b, decided 2026-10-05, `decision-r06-census-cost.md`).
//!
//! **The count** ([`CapCount`]). The sky is cut into [`CapResolution::STANDARD`]'s 1,536 rays of a
//! Fibonacci lattice ([`CapLattice`]), each standing for its own solid angle and dimming its stars
//! by its own extinction ([`profile`], [`NoiseMode::Realised`], [`Quality::Full`], interpolated
//! linearly in distance between the profile's nodes): the census lists the realised field's stars,
//! and a mean field undercounts wherever the dust is patchy, since the count goes roughly as
//! 10^(−0.6 A), convex in A. A ray's extinction is the least, node by node, of its
//! [`SUB_RAYS`] sub-rays, itself and two more a quarter of the lattice's spacing to either side
//! ([`RayExtinctions::measure_clearest`]), so that the count holds the clear windows between the
//! rays that one profile a ray misses. Along each ray, at [`RADIAL_STEPS_PER_DECADE`] steps a
//! decade in radius from 1 ly, the count takes the density field times each layer's luminosity
//! function ([`LuminosityTables`]): the layer's systems, and their stars brighter than the cut at
//! M<sub>V</sub> = cut − DM − v☉(A<sub>V</sub>) A<sub>V</sub>, the band's own boundary (R06.T8.k),
//! with v☉ the solar point's V secant
//! ([`Reddened::v_extinction`](super::colour::Reddened::v_extinction) of [`solar_colour`]'s
//! curves; decided 2026-10-07, `decision-r06-t8k-cone.md`, item 3). An interval between two radii
//! holds the trapezoid rule's count.
//!
//! **The criterion** is unchanged: under one expected star brighter than the cut beyond the caps,
//! per layer, summed over the sky. Since R06.T7.b each layer's radius is one per ray
//! ([`RayRadii`]):
//!
//! - each ray's radius is the outer edge of its outermost radial interval whose expected stars
//!   brighter than the cut per system are at least λ, the largest value that keeps the layer's
//!   expected count beyond under one. So the census opens systems in the order of their expected
//!   yield, the least work for a given expected miss;
//! - each ray's radius is then raised to the largest of its neighbours' within
//!   [`WIDENING_SPACINGS`] times the lattice's spacing;
//! - the census's radius towards a direction is the largest radius of the rays whose cones, of
//!   half-angle the lattice's spacing, hold it ([`RayRadii::toward`]). The census opens every cell
//!   whose padded ball meets one of those cones within its ray's radius, and the band reads the
//!   radius towards each of its texels' centres (R06.T9.b). Until a layer's census is final it
//!   lists each star by the radius towards the star's texel (R06.T8.i), so the listing and the band
//!   share that boundary for every star of an opened cell; a final census lists every star of the
//!   cells it opens. Where a cone's edge crosses a texel, a star within the radius towards the
//!   texel's centre but beyond the one towards itself may lie in a cell left closed, and its light
//!   is in neither: a gap the count beyond bounds, under one star a layer, deferred by the owner
//!   on 2026-10-08 (`decision-r06-t8i-listing.md`).
//!
//! The lattice's spacing is its covering radius, the farthest any direction lies from its nearest
//! ray, bounded from above by measurement ([`CapLattice::spacing`]): 4.2° at 1,536 rays. So every
//! direction lies in some ray's cone, and none is left without cells. A ray's own share of the sky,
//! a cap of 2.9° at 1,536 rays, would leave gaps of about 1° between three rays.
//!
//! An eye-only request may count each ray to the eye's own limit in its cone, rather than the
//! uniform cut ([`layer_caps_by_visibility`]): the eye sees less in the bright band, where the far
//! records lie.
//!
//! A radius never passes the layer's **rule bound**, the distance at which the brightest star the
//! envelope allows the layer ([`BrightnessEnvelope::brightest`] at [`max_star_mass`] of the band's
//! top, twice it since R06.T16.b, at any age) falls to the cut with the **least** extinction of
//! any ray, dimmed by that extinction's v☉. Neither is a strict bound: a clear window narrower than
//! the sub-rays' spacing could still show stars beyond them. Each cap therefore carries the
//! expected number of the layer's stars brighter than the cut beyond its radii, under one by
//! construction, which the response states; the slow test `caps_converge_in_rays` holds it under
//! 1.5 against 3,072 rays, each through its own profile, and twice the radial steps.
//!
//! The plan first dimmed every direction by the least extinction of 48 rays, the polar rays' near
//! the Sun, then each ray by the mean field's, then gave each layer one radius, a sphere; R06's
//! Risks record each and what it missed. [`CapCount::spheres`] keeps the spheres, for comparison.
//!
//! **The real boundary** ([`real_boundary`]; rendering plan R13, Design note 3). A query that
//! states a synthetic ceiling V<sub>P</sub> censuses layers C to E only to their real boundary
//! R(u): ray by ray the least of the cap at V<sub>P</sub>, the cut's own cap and
//! [`REAL_LIMIT_LY`]. Beyond it every star brighter than the cut is the synthetic tier's, or the
//! band's until that tier lands, and each layer states the count brighter than V<sub>P</sub> left
//! there ([`LayerCap::bright_beyond`]). Its census opens cells by the rays' cones widened by the
//! band texel's largest radius ρ, and lists by the radius towards each star's texel at every
//! reply, so that no star within R(u) of its texel lies in a cell left closed: T7.b's gap, closed
//! by fix (i) of `decision-r06-t8i-listing.md` (ruled 2026-10-09, `decision-r13-guard-trip.md`
//! §1).

use std::ops::Range;
use std::sync::Arc;

use crate::coords::{GalacticPosition, UnitVector};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::gas::extinction::{NoiseMode, Quality, profile};
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::imf::MassBand;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::observe::Observer;
use crate::time::Span;
use crate::units::consts::{RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR};
use crate::units::{LightYears, Magnitudes, Radians, SolarMasses, Years};

use super::band::{BandSpec, CubeFace};
use super::colour::{Reddening, solar_colour};
use super::envelope::{BrightnessEnvelope, max_star_mass};
use super::limits::EyeVisibility;
use super::luminosity::LuminosityTables;

/// The rays of the standard resolution, 1,536 since R06.T7.b.
///
/// 48 did not converge in the realised field, and 768 did near the Sun for one radius a layer
/// (decided 2026-10-03). With one radius a ray, 768 left E's expected count beyond, recounted at
/// 3,072 rays, at 1.53 above the Sun and 3.9 for the eye's caps there, through edge-on windows
/// towards the inner disc; a wider widening that mended it opened more than the census-cost
/// ruling's 75% of the spheres' systems near the Sun. T7's own remedy, more rays, passed at 7.95,
/// and the sub-rays ([`SUB_RAYS`]) close the deeper cuts (R06's Risks, "Deviations in T7.b, as
/// built").
pub const CAP_RAYS: usize = 1_536;

/// The count's radial steps per decade of distance at the standard resolution.
pub const RADIAL_STEPS_PER_DECADE: u32 = 24;

/// How far a ray's radius is widened, in the lattice's spacings (R06.T7.b): each ray takes the
/// largest radius of the rays within twice the spacing of it.
pub const WIDENING_SPACINGS: f64 = 2.0;

/// The sub-rays of each of the standard resolution's rays, its own among them (R06.T7.b).
///
/// Each ray counts its stars through the clearest of them at each distance. With one profile a
/// ray, a window narrower than the rays, seen edge-on in the disc from 2,000 ly above it, left E's
/// expected count beyond at 2.5–4.2 for cuts of 8.2–9.1 against a finer count. Three close it at
/// every point and cut of `caps_converge_in_rays`, the census then opening 74% of R06.T7's spheres'
/// systems near the Sun at V 7.95, against 66% with one profile a ray (R06's Risks, "Deviations in
/// T7.b, as built").
pub const SUB_RAYS: usize = 3;

/// How far a ray's sub-rays lie from it, in the lattice's spacings: a quarter, 1.05° at 1,536 rays,
/// to either side across the ray from galactic north. Wider, or more of them, the clearest of
/// several patchy profiles overstates the ray's count, and its caps open more than the census-cost
/// ruling's 75% of the spheres' systems near the Sun.
const SUB_RAY_RING_SPACINGS: f64 = 0.25;

/// The nodes per decade of each ray's extinction profile.
const PROFILE_NODES_PER_DECADE: u32 = 24;

/// The nearest distance the count reaches, ly: the stars nearer than it are the census's anyway.
const NEAREST_LY: f64 = 1.0;

/// The farthest any cap or rule bound reaches, ly: across the galaxy.
const FARTHEST_LY: f64 = 120_000.0;

/// The faces of the cube map whose texel centres sample the sky for a lattice's spacing: 256² a
/// face, texels of at most 0.32° from their centres.
const SPACING_TEXELS: u16 = 256;

/// The faces of a lattice's index of its rays by direction: 32² a face, texels of at most 2.53°.
const INDEX_TEXELS: u16 = 32;

/// The largest angular radius of a cell's ball, radians, that a lattice's index serves (6°): a
/// nearer cell, whose ball is wider, is tested against every ray.
const INDEX_BALL_RAD: f64 = 6.0 * RADIANS_PER_DEGREE;

/// The margin on every angle a test widens, radians, against rounding (10⁻⁹).
const ANGLE_MARGIN_RAD: f64 = 1e-9;

/// The real limit, ly: the farthest the real tier of a query at a synthetic ceiling reaches in
/// layers C to E, on any ray (rendering plan R13, Design note 3; [`real_boundary`]).
///
/// 2,000 ly, twice the first jump drive's range of 1,000 ly and one of the census's fixed shell
/// edges ([`SHELL_EDGES_LY`](super::census::SHELL_EDGES_LY), which a test holds), so the real
/// tier's last shell ends on it. Decided by the owner on 2026-10-09
/// (`decision-r13-guard-trip.md`, option A), after R13.T1 measured the real tier to the caps at
/// V<sub>P</sub> alone at 8.8–22.6 times its estimate, and kept by the owner the same day
/// (`decision-r13-t1b-guard.md`, option A) after R13.T1.b measured it held at this limit: near
/// the Sun on the test fixture 1.31 × 10⁴ CPU-s at V<sub>P</sub> 5.0, with 74.8 stars brighter than
/// V 4.5 beyond R(u), C to E, and the brightest expected about V 2.1. It is one constant, lifted to
/// the caps at V<sub>P</sub> once the census's cost levers make that affordable (R13's Risks, "The
/// limit is lifted").
pub const REAL_LIMIT_LY: f64 = 2_000.0;

/// The synthetic ceiling V<sub>P</sub> a server serves the sky at, apparent V (rendering plan R13,
/// Design notes 3, 4 and 15): V 5.0, RM3's interim, until R13.T7 sets the owner's 4.5 (decided
/// 2026-10-08, `feasibility-hybrid-sky.md` §11). R13.T5.a's `sky::synthetic` takes it over.
///
/// A server's constant, not a client setting: a bridge shares one sky, and every ship takes the
/// same rule. The sim reads only the ceiling a query states
/// ([`SkyQuery::synthetic_ceiling`](super::census::SkyQuery::synthetic_ceiling)); a server states
/// this one, or the request's cut where the cut is brighter, or none where a ceiling would hold no
/// ray ([`served_ceiling`]; decided 2026-10-09, `decision-r13-t2b-ceiling.md`). Layers C, D and E
/// are then censused only to their real boundary ([`real_boundary`]), at most [`REAL_LIMIT_LY`],
/// so that no served sky reaches past the limit at any cut. Near the Sun at the eye's cut that
/// leaves some 172 stars brighter than V 5.0 beyond their real boundary, C to E, on the test
/// fixture, and costs about 1.3 × 10⁴ CPU-s there and 2.8 × 10⁴ on the server's galaxy, against
/// 1.4 × 10⁵ at the caps at V 5.0 alone (R13.T1, T1.b).
pub const SYNTHETIC_CEILING_V: f64 = 5.0;

/// The layers whose census a synthetic ceiling holds within the real boundary: C, D and E
/// (rendering plan R13, Design note 2).
///
/// A, B and the brown dwarfs, whose caps near the Sun are 11–260 ly at the eye's and the camera's
/// cuts, are wholly real.
pub(crate) const REAL_BOUNDARY_LAYERS: [Layer; 3] = [Layer::C, Layer::D, Layer::E];

/// The layers a cap is computed for: the five stellar layers and the brown dwarfs.
pub const CAPPED_LAYERS: [Layer; 6] = [
    Layer::A,
    Layer::B,
    Layer::C,
    Layer::D,
    Layer::E,
    Layer::BrownDwarf,
];

/// How finely the caps' count is taken: the rays over the sky, the radial steps a decade, and the
/// sub-rays each ray counts through the clearest of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapResolution {
    rays: usize,
    steps_per_decade: u32,
    sub_rays: usize,
}

impl CapResolution {
    /// The caps' own: [`CAP_RAYS`] rays, [`RADIAL_STEPS_PER_DECADE`] steps a decade, each ray
    /// through the clearest of its [`SUB_RAYS`] sub-rays.
    pub const STANDARD: Self = Self {
        rays: CAP_RAYS,
        steps_per_decade: RADIAL_STEPS_PER_DECADE,
        sub_rays: SUB_RAYS,
    };

    /// `rays` rays and `steps_per_decade` radial steps a decade, each ray through its own profile
    /// alone, or `None` if either is zero: a recount's.
    #[must_use]
    pub fn new(rays: usize, steps_per_decade: u32) -> Option<Self> {
        (rays > 0 && steps_per_decade > 0).then_some(Self {
            rays,
            steps_per_decade,
            sub_rays: 1,
        })
    }

    /// The sub-rays a ray counts through the clearest of.
    #[must_use]
    pub const fn sub_rays(&self) -> usize {
        self.sub_rays
    }

    /// The rays.
    #[must_use]
    pub const fn rays(&self) -> usize {
        self.rays
    }

    /// The radial steps a decade.
    #[must_use]
    pub const fn steps_per_decade(&self) -> u32 {
        self.steps_per_decade
    }
}

/// The rays of a caps' count (R06.T7.b): `n` directions of a Fibonacci lattice, how far apart they
/// lie, and an index of the rays near each direction.
///
/// Its **spacing** is the lattice's covering radius, the farthest any direction lies from its
/// nearest ray, bounded from above: the largest angle from the centre of any texel of a cube map of
/// 256² a face to its nearest ray, plus the texels' largest radius ([`BandSpec::largest_texel_radius`],
/// 0.32°), since the angle to the nearest ray moves by no more than the direction does. Every
/// direction therefore lies within the spacing of some ray: 4.20° at 1,536 rays, where the farthest
/// a direction was found from its rays is 3.88°, the rays' own caps of equal area are 2.92° and the
/// rays' nearest neighbours lie 4.5°–5.2° apart (5.81° at 768 rays, against 5.49° found).
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::caps::{CAP_RAYS, CapLattice};
/// use hyperion_sim::units::Degrees;
///
/// let lattice = CapLattice::new(CAP_RAYS);
/// assert_eq!(lattice.directions().len(), 1_536);
/// let spacing = Degrees::from(lattice.spacing()).value();
/// assert!((4.0..4.5).contains(&spacing), "{spacing}°");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CapLattice {
    directions: Vec<UnitVector>,
    /// The covering radius's bound, radians.
    spacing: f64,
    /// The cosine of the spacing: a direction lies in a ray's cone where its cosine with the ray is
    /// at least this.
    cos_spacing: f64,
    /// The index's cube map.
    index: BandSpec,
    /// Each index texel's first ray in `near`, in the texels' order (face, row, column), and the
    /// end.
    starts: Vec<usize>,
    /// Per index texel, the rays within the spacing, [`INDEX_BALL_RAD`] and the texel's radius of
    /// its centre, ascending.
    near: Vec<usize>,
}

impl CapLattice {
    /// The lattice of `rays` rays.
    ///
    /// # Panics
    ///
    /// If `rays` is zero.
    #[must_use]
    pub fn new(rays: usize) -> Self {
        assert!(rays > 0, "a lattice of no rays");
        let directions = fibonacci_sphere(rays);
        let spacing = covering_bound(&directions);
        let index = BandSpec::new(INDEX_TEXELS, 1).expect("a face of 32 texels is a band");
        let side = index.face_texels();
        let reach =
            spacing + INDEX_BALL_RAD + index.largest_texel_radius().value() + ANGLE_MARGIN_RAD;
        let mut starts = Vec::with_capacity(CubeFace::ALL.len() * usize::from(side).pow(2) + 1);
        let mut near = Vec::new();
        for face in CubeFace::ALL {
            for row in 0..side {
                for column in 0..side {
                    starts.push(near.len());
                    let centre = index.texel_direction(face, row, column);
                    near.extend(rays_within(&directions, centre, reach));
                }
            }
        }
        starts.push(near.len());
        Self {
            directions,
            spacing,
            cos_spacing: math::cos(spacing),
            index,
            starts,
            near,
        }
    }

    /// The rays' directions, in the lattice's order.
    #[must_use]
    pub fn directions(&self) -> &[UnitVector] {
        &self.directions
    }

    /// The lattice's spacing: an upper bound on the farthest any direction lies from its nearest
    /// ray, and the half-angle of each ray's cone.
    #[must_use]
    pub const fn spacing(&self) -> Radians {
        Radians::new(self.spacing)
    }

    /// The index texel `direction` falls in.
    #[must_use]
    fn texel(&self, direction: [f64; 3]) -> usize {
        let (face, row, column) = self
            .index
            .texel_of(direction)
            .expect("a unit direction falls in a texel");
        let side = usize::from(self.index.face_texels());
        (usize::from(face.layer()) * side + usize::from(row)) * side + usize::from(column)
    }

    /// The rays near index texel `texel`: every ray within the spacing and [`INDEX_BALL_RAD`] of
    /// any direction in it.
    #[must_use]
    fn near(&self, texel: usize) -> &[usize] {
        &self.near[self.starts[texel]..self.starts[texel + 1]]
    }
}

/// The cones about a lattice's rays that a census opens its cells by (R06.T7.b): each of
/// half-angle the lattice's spacing, widened by an angle that R06's census takes as none and a
/// census at a synthetic ceiling as the band texel's largest radius ρ (rendering plan R13, Design
/// note 3; fix (i) of `decision-r06-t8i-listing.md`, ruled 2026-10-09).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RayCones {
    /// The cosine and sine of the cones' half-angle and [`ANGLE_MARGIN_RAD`], which a cell's test
    /// widens by.
    widened: [f64; 2],
    /// The sine of the largest angular radius of a cell's ball that the lattice's index serves for
    /// these cones, [`INDEX_BALL_RAD`] less the widening, or −1 where the widening reaches it: the
    /// index holds every ray within the spacing and [`INDEX_BALL_RAD`] of its texels. A larger ball
    /// is tested against every ray.
    index_ball_sin: f64,
}

impl RayCones {
    /// The cones of half-angle `spacing` widened by `by`, both radians.
    ///
    /// Widened by none, they are R06's, bit for bit: `spacing + 0.0` is `spacing`. Widened by
    /// [`INDEX_BALL_RAD`] or more, no ball is tested through the index, which holds no ray that
    /// far.
    #[must_use]
    fn of(spacing: f64, by: f64) -> Self {
        let angle = spacing + by + ANGLE_MARGIN_RAD;
        Self {
            widened: [math::cos(angle), math::sin(angle)],
            index_ball_sin: if by < INDEX_BALL_RAD {
                math::sin(INDEX_BALL_RAD - by)
            } else {
                -1.0
            },
        }
    }
}

/// The rays of `directions` whose angle from `u` is at most `angle`, radians, in their order.
fn rays_within(
    directions: &[UnitVector],
    u: UnitVector,
    angle: f64,
) -> impl Iterator<Item = usize> + '_ {
    let cos = if angle >= core::f64::consts::PI {
        -1.0
    } else {
        math::cos(angle)
    };
    let z = u.components()[2];
    z_band(directions.len(), z - angle, z + angle).filter(move |&k| directions[k].dot(&u) >= cos)
}

/// The indices of a Fibonacci lattice of `n` rays ([`fibonacci_sphere`]) whose z, 1 − (2k + 1) ÷
/// n, may lie within `[lo, hi]`, with a ray to spare at each end against rounding. A ray within an
/// angle θ of a direction has its z within θ of the direction's, since z is the cosine of the
/// polar angle and no cosine moves faster than its angle.
fn z_band(n: usize, lo: f64, hi: f64) -> Range<usize> {
    #[expect(clippy::cast_precision_loss, reason = "a few thousand rays")]
    let count = n as f64;
    // z_k ≤ hi from k ≥ ((1 − hi) n − 1) ÷ 2, and z_k ≥ lo up to k ≤ ((1 − lo) n − 1) ÷ 2.
    let first = (((1.0 - hi) * count - 1.0) / 2.0 - 1.0).floor().max(0.0);
    let last = (((1.0 - lo) * count - 1.0) / 2.0 + 1.0).ceil().max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "both at least zero, and held at n below"
    )]
    let (first, last) = (first as usize, last as usize);
    first.min(n)..last.saturating_add(1).min(n)
}

/// The greatest cosine of `u` with any ray of `directions`, its nearest's: found among the rays
/// whose z lies within `band` radians of its own when the nearest lies within `band`, whose cosine
/// is `cos_band`, and among all of them otherwise.
fn nearest_cosine(directions: &[UnitVector], u: UnitVector, band: f64, cos_band: f64) -> f64 {
    let z = u.components()[2];
    let best = |range: Range<usize>| {
        directions[range]
            .iter()
            .map(|d| d.dot(&u))
            .fold(-1.0, f64::max)
    };
    let near = best(z_band(directions.len(), z - band, z + band));
    // A ray outside the band lies farther than `band`, so a nearer ray found inside it is the
    // nearest.
    if near > cos_band {
        near
    } else {
        best(0..directions.len())
    }
}

/// An upper bound on the farthest any direction lies from its nearest ray of `directions`,
/// radians: the farthest the centre of any texel of a cube map of [`SPACING_TEXELS`]² a face lies
/// from its nearest ray, plus the texels' largest radius and [`ANGLE_MARGIN_RAD`]. The angle to
/// the nearest ray moves by no more than the direction moves, and every direction lies within the
/// texels' largest radius of a texel's centre.
fn covering_bound(directions: &[UnitVector]) -> f64 {
    let grid = BandSpec::new(SPACING_TEXELS, 1).expect("a face of 256 texels is a band");
    let side = grid.face_texels();
    #[expect(clippy::cast_precision_loss, reason = "a few thousand rays")]
    let band = (1.5 * (4.0 * core::f64::consts::PI / directions.len() as f64).sqrt())
        .min(core::f64::consts::PI);
    let cos_band = math::cos(band);
    let mut least = 1.0_f64;
    for face in CubeFace::ALL {
        for row in 0..side {
            for column in 0..side {
                let u = grid.texel_direction(face, row, column);
                least = least.min(nearest_cosine(directions, u, band, cos_band));
            }
        }
    }
    math::acos(least.clamp(-1.0, 1.0)) + grid.largest_texel_radius().value() + ANGLE_MARGIN_RAD
}

/// One layer's radius per ray of a [`CapLattice`] (R06.T7.b): towards each direction, the census
/// is complete to the largest radius of the rays whose cones, of half-angle the lattice's spacing,
/// hold it ([`toward`](Self::toward)).
///
/// Cloning shares the lattice and the radii.
#[derive(Debug, Clone)]
pub struct RayRadii {
    lattice: Arc<CapLattice>,
    radii: Arc<[f64]>,
    /// Per index texel of the lattice, the largest radius of its near rays.
    texel_bound: Arc<[f64]>,
    largest: f64,
}

impl PartialEq for RayRadii {
    /// The same lattice (a lattice is a function of its ray count) and the same radii, bit for bit.
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.lattice, &other.lattice)
            || self.lattice.directions.len() == other.lattice.directions.len())
            && (Arc::ptr_eq(&self.radii, &other.radii)
                || (self.radii.len() == other.radii.len()
                    && self
                        .radii
                        .iter()
                        .zip(other.radii.iter())
                        .all(|(a, b)| a.total_cmp(b).is_eq())))
    }
}

impl RayRadii {
    /// The radii `radii_ly`, ly, one per ray of `lattice` in its order.
    ///
    /// # Panics
    ///
    /// Unless there is one radius a ray, each finite and positive.
    #[must_use]
    pub(crate) fn new(lattice: Arc<CapLattice>, radii_ly: Vec<f64>) -> Self {
        assert_eq!(radii_ly.len(), lattice.directions.len(), "one radius a ray");
        assert!(
            radii_ly.iter().all(|r| r.is_finite() && *r > 0.0),
            "every ray's radius is finite and positive"
        );
        let texels = lattice.starts.len() - 1;
        let texel_bound: Arc<[f64]> = (0..texels)
            .map(|t| {
                lattice
                    .near(t)
                    .iter()
                    .map(|&k| radii_ly[k])
                    .fold(0.0, f64::max)
            })
            .collect();
        let largest = radii_ly.iter().copied().fold(0.0, f64::max);
        Self {
            lattice,
            radii: radii_ly.into(),
            texel_bound,
            largest,
        }
    }

    /// The radii held within `edge_ly`, ly: each ray's the lesser of its own and the edge.
    ///
    /// They are on the same lattice (R06.T8.i, a shell's edge). Towards each direction they give
    /// the lesser of the edge and [`toward`](Self::toward), bit for bit, since both take a ray's
    /// radius as it is. The hybrid sky holds its real boundary so within its real limit (rendering
    /// plan R13, Design note 3), and R13.T1.b's bench censuses to the boundary held so, before
    /// R13.T2.a's rule is built.
    ///
    /// # Panics
    ///
    /// Unless `edge_ly` is finite and positive.
    #[must_use]
    pub fn within(&self, edge_ly: f64) -> Self {
        assert!(
            edge_ly.is_finite() && edge_ly > 0.0,
            "an edge of {edge_ly} ly holds no ray within it"
        );
        Self::new(
            Arc::clone(&self.lattice),
            self.radii.iter().map(|&r| r.min(edge_ly)).collect(),
        )
    }

    /// Each ray's radius the lesser of its own and `other`'s, on the same lattice.
    ///
    /// It is the hybrid sky's rule for its real boundary (rendering plan R13, Design note 3): each
    /// ray of a layer's cap at the synthetic ceiling, held within the same ray of its cap at the
    /// request's cut, so that the boundary is never beyond the cut's own cap on any ray. Towards
    /// each direction the result is complete to no farther than either: every ray within the cone
    /// is at most both.
    ///
    /// # Panics
    ///
    /// Unless `other`'s radii are on a lattice of as many rays, a lattice being a function of its
    /// count.
    ///
    /// # Examples
    ///
    /// The caps near the Sun at a ceiling of V 4.5, held within the caps at the eye's cut
    /// (`no_run`: the tables take a minute or more to build):
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::caps::layer_caps;
    /// use hyperion_sim::sky::envelope::BrightnessEnvelope;
    /// use hyperion_sim::sky::luminosity::LuminosityTables;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let galaxy = Galaxy::new(Seed::new(7));
    /// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
    /// let mut cache = NoiseCache::with_capacity(1 << 16);
    /// let mut caps = |cut| layer_caps(&galaxy, &tables, &envelope, &observer, Magnitudes::new(cut), &mut cache);
    /// let (ceiling, eye) = (caps(4.5), caps(7.95));
    /// let (high, cut) = (ceiling[2].rays().ok_or("by ray")?, eye[2].rays().ok_or("by ray")?);
    /// let real = high.lesser(cut);
    /// assert!(real.radii_ly().iter().zip(cut.radii_ly()).all(|(r, c)| r <= c));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn lesser(&self, other: &Self) -> Self {
        assert_eq!(
            self.radii.len(),
            other.radii.len(),
            "the lesser of two radii a ray is taken on one lattice"
        );
        Self::new(
            Arc::clone(&self.lattice),
            self.radii
                .iter()
                .zip(other.radii.iter())
                .map(|(&a, &b)| a.min(b))
                .collect(),
        )
    }

    /// Whether every ray's radius reaches `edge_ly` or beyond.
    ///
    /// The radii held within it are then the edge in every direction.
    #[must_use]
    pub(crate) fn all_reach(&self, edge_ly: f64) -> bool {
        self.radii.iter().all(|&r| r >= edge_ly)
    }

    /// The lattice whose rays the radii are of.
    #[must_use]
    pub fn lattice(&self) -> &CapLattice {
        &self.lattice
    }

    /// Each ray's radius, ly, in the lattice's order.
    #[must_use]
    pub fn radii_ly(&self) -> &[f64] {
        &self.radii
    }

    /// The largest radius of any ray: the census looks no farther in any direction.
    #[must_use]
    pub const fn largest(&self) -> LightYears {
        LightYears::new(self.largest)
    }

    /// The radius towards `direction`: the largest radius of the rays whose cones hold it, those
    /// within the lattice's spacing of it. Every direction lies in some ray's cone.
    #[must_use]
    pub fn toward(&self, direction: UnitVector) -> LightYears {
        let lattice = &*self.lattice;
        let radius = lattice
            .near(lattice.texel(direction.components()))
            .iter()
            .filter(|&&k| lattice.directions[k].dot(&direction) >= lattice.cos_spacing)
            .map(|&k| self.radii[k])
            .fold(0.0, f64::max);
        debug_assert!(radius > 0.0, "every direction lies in some ray's cone");
        LightYears::new(radius)
    }

    /// The cones of the rays' lattice widened by `by`, radians.
    ///
    /// A census at a synthetic ceiling opens its cells by them, with `by` the band texel's largest
    /// radius ρ (rendering plan R13, Design note 3; fix (i)), and R06's census by those widened by
    /// none. Towards a direction within ρ of a texel's centre, some ray whose radius is the radius
    /// towards that centre ([`toward`](Self::toward)) holds it in its widened cone, so the cell of
    /// every star nearer than its texel's radius is opened.
    ///
    /// # Panics
    ///
    /// Unless `by` is finite and not negative.
    #[must_use]
    pub(crate) fn cones_widened_by(&self, by: Radians) -> RayCones {
        let by = by.value();
        assert!(
            by.is_finite() && by >= 0.0,
            "a cone is widened by a finite angle, not negative: {by} rad"
        );
        RayCones::of(self.lattice.spacing, by)
    }

    /// Whether a ball of radius `radius_ly` whose centre lies `offset_ly` from the apex (both in
    /// light-years, on the galactic axes) meets the census's region: the cone of `cones` about
    /// some ray ([`cones_widened_by`](Self::cones_widened_by)), widened by [`ANGLE_MARGIN_RAD`],
    /// nearer than the ray's radius. A cell's padded ball that meets no cone within its radius
    /// holds no point the census is complete to ([`toward`](Self::toward)), so the census need not
    /// open it.
    ///
    /// The test takes the ball's angular radius as its own, asin(radius ÷ distance), and its
    /// nearest distance as distance less radius: a ball that meets a cone within its ray's radius
    /// passes, and a few that do not. Unwidened cones give R06.T7.b's test, bit for bit.
    #[must_use]
    pub(crate) fn meets_ball(&self, cones: &RayCones, offset_ly: [f64; 3], radius_ly: f64) -> bool {
        let distance = math::hypot(math::hypot(offset_ly[0], offset_ly[1]), offset_ly[2]);
        if distance <= radius_ly {
            return true;
        }
        let reach = distance - radius_ly;
        if reach >= self.largest {
            return false;
        }
        let sin_ball = radius_ly / distance;
        // cos(the cones' half-angle + margin + the ball's angular radius).
        let [cos_spacing, sin_spacing] = cones.widened;
        let threshold = cos_spacing * (1.0 - sin_ball * sin_ball).sqrt() - sin_spacing * sin_ball;
        let u = offset_ly.map(|c| c / distance);
        let meets = |&k: &usize| {
            self.radii[k] > reach && dot(self.lattice.directions[k].components(), u) >= threshold
        };
        if sin_ball <= cones.index_ball_sin {
            let texel = self.lattice.texel(u);
            reach < self.texel_bound[texel] && self.lattice.near(texel).iter().any(meets)
        } else {
            (0..self.radii.len()).any(|k| meets(&k))
        }
    }
}

/// The dot product of two vectors.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// One layer's cap (Design note 9): one radius in every direction, or since R06.T7.b one a ray.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerCap {
    layer: Layer,
    radius: LightYears,
    rule_bound: LightYears,
    expected_beyond: f64,
    /// The expected count brighter than the synthetic ceiling beyond the radii, for a real
    /// boundary ([`real_boundary`]).
    bright_beyond: Option<f64>,
    rays: Option<RayRadii>,
}

impl LayerCap {
    /// The layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The farthest the census looks in this layer, in any direction: its one radius, or the
    /// largest of its rays' ([`RayRadii::largest`]).
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The radius towards `direction`: its one radius, or its rays' radius towards it
    /// ([`RayRadii::toward`]).
    #[must_use]
    pub fn radius_toward(&self, direction: UnitVector) -> LightYears {
        self.rays
            .as_ref()
            .map_or(self.radius, |rays| rays.toward(direction))
    }

    /// The radius of each ray, if the cap has one a ray (R06.T7.b); `None` for one radius in every
    /// direction.
    #[must_use]
    pub const fn rays(&self) -> Option<&RayRadii> {
        self.rays.as_ref()
    }

    /// The rule's bound: where the layer's brightest possible star falls to the cut with the least
    /// extinction of any ray. No radius is beyond it.
    #[must_use]
    pub const fn rule_bound(&self) -> LightYears {
        self.rule_bound
    }

    /// The expected number of the layer's stars brighter than the cut beyond the cap's radii,
    /// under one for the caps' own; for a real boundary ([`real_boundary`]) C's, D's and E's count
    /// beyond R(u), which the synthetic tier and the band take, some thousands near the Sun.
    #[must_use]
    pub const fn expected_beyond(&self) -> f64 {
        self.expected_beyond
    }

    /// The expected number of the layer's stars brighter than the synthetic ceiling V<sub>P</sub>
    /// beyond the cap's radii, for a real boundary ([`real_boundary`]; rendering plan R13, Design
    /// note 3), and `None` for every other cap.
    ///
    /// Where the cap at V<sub>P</sub> lies within [`REAL_LIMIT_LY`] it is under one; beyond the
    /// limit it is the stars brighter than V<sub>P</sub> that the synthetic tier draws, which a
    /// reply states. Near the Sun on the test fixture at V<sub>P</sub> 5.0 they are C 0.854, D 8.60
    /// and E 162 (R13.T1.b, at the caps' own count).
    #[must_use]
    pub const fn bright_beyond(&self) -> Option<f64> {
        self.bright_beyond
    }

    /// A cap of `radius` in every direction for `layer`, with nothing stated beyond it: the brute
    /// force's, which forces every layer to one radius.
    #[must_use]
    pub const fn forced(layer: Layer, radius: LightYears) -> Self {
        Self {
            layer,
            radius,
            rule_bound: radius,
            expected_beyond: 0.0,
            bright_beyond: None,
            rays: None,
        }
    }

    /// A cap of `rays`' radii for `layer`, with nothing stated beyond them: a test's, or a bench's
    /// census to a boundary the caps' count does not give (rendering plan R13, R13.T1;
    /// [`RayRadii::lesser`]).
    ///
    /// A census to the hybrid sky's real boundary takes [`real_boundary`]'s caps, which state their
    /// counts beyond.
    #[must_use]
    pub fn forced_by_ray(layer: Layer, rays: RayRadii) -> Self {
        let radius = rays.largest();
        Self {
            layer,
            radius,
            rule_bound: radius,
            expected_beyond: 0.0,
            bright_beyond: None,
            rays: Some(rays),
        }
    }
}

/// The distances of every ray's profile, ly: 0, then [`PROFILE_NODES_PER_DECADE`] a decade from
/// [`NEAREST_LY`] to [`FARTHEST_LY`].
#[must_use]
fn profile_nodes() -> Vec<f64> {
    let decades = math::log10(FARTHEST_LY / NEAREST_LY);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few decades, a few hundred nodes"
    )]
    let steps = (decades * f64::from(PROFILE_NODES_PER_DECADE)).ceil() as u32;
    std::iter::once(0.0)
        .chain((0..=steps).map(|k| {
            if k == steps {
                FARTHEST_LY
            } else {
                NEAREST_LY * math::exp10(decades * f64::from(k) / f64::from(steps))
            }
        }))
        .collect()
}

/// The extinction in V along rays from a point, each from [`profile`] marches in the realised
/// field at full quality: a whole Fibonacci lattice of rays, or one consecutive share of one, each
/// through its own profile or the clearest of its sub-rays' ([`measure_clearest`](Self::measure_clearest)).
#[derive(Debug, Clone, PartialEq)]
pub struct RayExtinctions {
    /// The point the rays start from.
    origin: GalacticPosition,
    /// The rays of the whole lattice the rays are of.
    lattice: usize,
    /// The lattice's index of the first ray held.
    first: usize,
    /// The sub-rays each ray is the clearest of: 1 for its own profile alone.
    sub_rays: usize,
    /// The lattice, where its spacing set the sub-rays.
    grid: Option<Arc<CapLattice>>,
    nodes: Vec<f64>,
    directions: Vec<UnitVector>,
    /// Each ray's A<sub>V</sub> at each node.
    profiles: Vec<Vec<f64>>,
}

impl RayExtinctions {
    /// `rays` rays of a Fibonacci lattice from `origin`, each through its own profile.
    #[must_use]
    pub fn measure(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        rays: usize,
        cache: &mut NoiseCache,
    ) -> Self {
        Self::measure_rays(galaxy, origin, rays, 0..rays, cache)
    }

    /// The rays `which` of a Fibonacci lattice of `rays` rays from `origin`, each through its own
    /// profile: one share of [`measure`](Self::measure)'s rays, which a server measures as several
    /// jobs, each with its own `cache`, and [`join`](Self::join)s in order (decided 2026-10-03,
    /// `decision-r06-tables.md`, item B.4; R06.T11.c). Each ray is a function of its own direction
    /// alone, and the cache changes no value, so the shares joined are `measure`'s bit for bit.
    ///
    /// # Panics
    ///
    /// If `which` runs backwards or reaches past the lattice's `rays`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::sky::caps::RayExtinctions;
    ///
    /// let galaxy = Galaxy::new(Seed::new(7));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let measure = |which| {
    ///     RayExtinctions::measure_rays(&galaxy, &sun, 8, which, &mut NoiseCache::with_capacity(64))
    /// };
    /// let joined = RayExtinctions::join([measure(0..3), measure(3..8)]);
    /// let whole = RayExtinctions::measure(&galaxy, &sun, 8, &mut NoiseCache::with_capacity(64));
    /// assert_eq!(joined, whole);
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn measure_rays(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        rays: usize,
        which: Range<usize>,
        cache: &mut NoiseCache,
    ) -> Self {
        let one = (Radians::new(0.0), 1);
        Self::measure_share(galaxy, origin, &fibonacci_sphere(rays), one, which, cache)
    }

    /// The rays of `lattice` from `origin`, each through the clearest of its `sub_rays` sub-rays
    /// at each of the profiles' nodes (R06.T7.b).
    ///
    /// The sub-rays are the ray itself and `sub_rays` − 1 more, spread evenly round it at
    /// [`SUB_RAY_RING_SPACINGS`] of the lattice's spacing, within its cone. The least extinction at
    /// each node, interpolated linearly, is at most each sub-ray's own between the nodes, so a ray
    /// counts at least the stars of its clearest sub-ray: a window between the rays narrower than
    /// their spacing, which one profile a ray misses, is counted where a sub-ray meets it.
    ///
    /// # Panics
    ///
    /// If `sub_rays` is zero.
    #[must_use]
    pub fn measure_clearest(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        lattice: Arc<CapLattice>,
        sub_rays: usize,
        cache: &mut NoiseCache,
    ) -> Self {
        let rays = lattice.directions.len();
        Self::measure_clearest_rays(galaxy, origin, lattice, sub_rays, 0..rays, cache)
    }

    /// The rays `which` of [`measure_clearest`](Self::measure_clearest)'s: one share of them,
    /// which a server measures as several jobs and [`join`](Self::join)s in order, as
    /// [`measure_rays`](Self::measure_rays)' (R06.T11.c).
    ///
    /// # Panics
    ///
    /// If `sub_rays` is zero, or `which` runs backwards or reaches past the lattice's rays.
    #[must_use]
    pub fn measure_clearest_rays(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        lattice: Arc<CapLattice>,
        sub_rays: usize,
        which: Range<usize>,
        cache: &mut NoiseCache,
    ) -> Self {
        assert!(sub_rays > 0, "a ray is one sub-ray at least");
        let ring = Radians::new(SUB_RAY_RING_SPACINGS * lattice.spacing);
        let mut share = Self::measure_share(
            galaxy,
            origin,
            &lattice.directions,
            (ring, sub_rays),
            which,
            cache,
        );
        share.grid = Some(lattice);
        share
    }

    /// The rays `which` of `lattice` (every ray of a lattice, in order) from `origin`, each
    /// through the clearest of the sub-rays `(ring, sub_rays)` gives: their ring's angle and their
    /// count, one for a ray's own profile alone.
    fn measure_share(
        galaxy: &Galaxy,
        origin: &GalacticPosition,
        lattice: &[UnitVector],
        (ring, sub_rays): (Radians, usize),
        which: Range<usize>,
        cache: &mut NoiseCache,
    ) -> Self {
        let rays = lattice.len();
        assert!(
            which.start <= which.end && which.end <= rays,
            "rays {which:?} of a lattice of {rays}"
        );
        let nodes = profile_nodes();
        let distances: Vec<LightYears> = nodes.iter().map(|&d| LightYears::new(d)).collect();
        let directions = lattice[which.clone()].to_vec();
        let mut out = Vec::with_capacity(nodes.len());
        let profiles = directions
            .iter()
            .map(|&direction| {
                let mut clearest = vec![f64::INFINITY; nodes.len()];
                for ray in sub_ray_directions(direction, ring.value(), sub_rays) {
                    profile(
                        galaxy.gas(),
                        origin,
                        ray,
                        &distances,
                        NoiseMode::Realised,
                        Quality::Full,
                        &[],
                        cache,
                        &mut out,
                    );
                    for (least, a) in clearest.iter_mut().zip(&out) {
                        *least = least.min(a.value());
                    }
                }
                clearest
            })
            .collect();
        Self {
            origin: *origin,
            lattice: rays,
            first: which.start,
            sub_rays,
            grid: None,
            nodes,
            directions,
            profiles,
        }
    }

    /// Consecutive shares of one lattice's rays from one point ([`measure_rays`](Self::measure_rays)
    /// or [`measure_clearest_rays`](Self::measure_clearest_rays)), in the lattice's order, as one:
    /// the rays of every share in turn.
    ///
    /// # Panics
    ///
    /// If `shares` is empty, or its shares are not of one lattice from one point with the same
    /// sub-rays, each starting where the one before it ends, from the lattice's first ray.
    #[must_use]
    pub fn join(shares: impl IntoIterator<Item = Self>) -> Self {
        let mut shares = shares.into_iter();
        let mut joined = shares.next().expect("a share of rays to join");
        assert_eq!(
            joined.first, 0,
            "the shares start at the lattice's first ray"
        );
        for share in shares {
            assert!(
                share.origin == joined.origin
                    && share.lattice == joined.lattice
                    && share.sub_rays == joined.sub_rays
                    && same_grid(share.grid.as_ref(), joined.grid.as_ref())
                    && share.first == joined.first + joined.directions.len(),
                "the shares of one lattice from one point, in order: rays from {} of {} after {} of \
                 {}",
                share.first,
                share.lattice,
                joined.directions.len(),
                joined.lattice
            );
            joined.directions.extend(share.directions);
            joined.profiles.extend(share.profiles);
        }
        joined
    }

    /// Whether the rays are the whole lattice's, from `origin`.
    #[must_use]
    fn is_lattice_from(&self, origin: &GalacticPosition) -> bool {
        self.first == 0 && self.directions.len() == self.lattice && self.origin == *origin
    }

    /// The rays' directions, in order.
    pub fn directions(&self) -> impl Iterator<Item = &UnitVector> {
        self.directions.iter()
    }

    /// Ray `ray`'s extinction at distance `d`, ly, interpolated linearly between its nodes.
    ///
    /// # Panics
    ///
    /// If `ray` is not one of the rays.
    #[must_use]
    pub fn along(&self, ray: usize, d: f64) -> f64 {
        let values = &self.profiles[ray];
        let k = self.nodes.partition_point(|&node| node <= d);
        if k == 0 {
            return values[0];
        }
        if k >= self.nodes.len() {
            return values[values.len() - 1];
        }
        let (d0, d1) = (self.nodes[k - 1], self.nodes[k]);
        let t = (d - d0) / (d1 - d0);
        values[k - 1] + (values[k] - values[k - 1]) * t
    }

    /// The least extinction of any ray at distance `d`, ly.
    #[must_use]
    pub fn least(&self, d: f64) -> f64 {
        (0..self.profiles.len())
            .map(|ray| self.along(ray, d))
            .fold(f64::INFINITY, f64::min)
    }
}

/// Whether two shares' lattices are one: both none, or of one ray count, a lattice being a function
/// of its count.
fn same_grid(a: Option<&Arc<CapLattice>>, b: Option<&Arc<CapLattice>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a.directions.len() == b.directions.len(),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

/// `count` sub-rays of the ray along `ray`: `ray` itself, then `count` − 1 at `ring` radians from
/// it, evenly round it from a fixed side (across `ray` from galactic north, or from +x near the
/// poles).
fn sub_ray_directions(
    ray: UnitVector,
    ring: f64,
    count: usize,
) -> impl Iterator<Item = UnitVector> {
    let along = ray.components();
    let side = if along[2].abs() < 0.9 {
        UnitVector::NORTH
    } else {
        UnitVector::X
    };
    let first = UnitVector::from_components(ray.cross(&side)).expect("a side off the ray");
    let second = UnitVector::from_components(ray.cross(&first)).expect("perpendicular to the ray");
    let (sin_ring, cos_ring) = math::sin_cos(ring);
    let around = count.saturating_sub(1);
    std::iter::once(ray).chain((0..around).map(move |step| {
        #[expect(clippy::cast_precision_loss, reason = "a few sub-rays")]
        let phi = 2.0 * core::f64::consts::PI * step as f64 / around as f64;
        let (sin_phi, cos_phi) = math::sin_cos(phi);
        let (across, down) = (first.components(), second.components());
        UnitVector::from_components(std::array::from_fn(|axis| {
            cos_ring * along[axis] + sin_ring * (cos_phi * across[axis] + sin_phi * down[axis])
        }))
        .expect("a sub-ray is a direction")
    }))
}

/// `n` directions spread evenly over the sphere (a Fibonacci lattice): ray k at z = 1 − (2k + 1)
/// ÷ n, each turned by the golden angle from the last.
#[must_use]
pub(crate) fn fibonacci_sphere(n: usize) -> Vec<UnitVector> {
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    #[expect(clippy::cast_precision_loss, reason = "a few thousand directions")]
    let count = n as f64;
    (0..n)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "k < n")]
            let k = k as f64;
            let z = 1.0 - (2.0 * k + 1.0) / count;
            let rho = (1.0 - z * z).sqrt();
            let (sin, cos) = math::sin_cos(golden * k);
            UnitVector::from_components([rho * cos, rho * sin, z]).expect("a unit vector")
        })
        .collect()
}

/// The distance modulus at `d` ly: 5 log₁₀(d ÷ 10 pc).
#[must_use]
fn distance_modulus(d: f64) -> f64 {
    5.0 * math::log10(d / (10.0 * LY_PER_PARSEC))
}

/// Light-years in a parsec.
const LY_PER_PARSEC: f64 = 3.261_563_777_167_433_6;

/// The V extinction of the solar point's light through an A<sub>V</sub> of `a_v`, mag:
/// v☉(A<sub>V</sub>) A<sub>V</sub>, the band's own (R06.T8.k; `band`'s `march_slots` reads it so).
#[must_use]
fn v_extinction(dust: &Reddening, a_v: f64) -> f64 {
    dust.through(Magnitudes::new(a_v)).v_extinction().value()
}

/// The faintest absolute magnitude counted at `d` ly through an A<sub>V</sub> of `a_v` for the
/// apparent `cut`: cut − DM(d) − v☉(A<sub>V</sub>) A<sub>V</sub>, the band's own boundary.
#[must_use]
fn faintest_counted(cut: f64, d: f64, a_v: f64, dust: &Reddening) -> f64 {
    cut - distance_modulus(d) - v_extinction(dust, a_v)
}

/// The rule's bound of a layer whose brightest star is `brightest`: where it falls to `cut`
/// through the least extinction of any ray, dimmed by its v☉.
#[must_use]
fn rule_bound(brightest: f64, cut: f64, rays: &RayExtinctions, dust: &Reddening) -> f64 {
    let faint = |d: f64| brightest + distance_modulus(d) + v_extinction(dust, rays.least(d)) > cut;
    if faint(NEAREST_LY) {
        return NEAREST_LY;
    }
    if !faint(FARTHEST_LY) {
        return FARTHEST_LY;
    }
    let (mut lo, mut hi) = (NEAREST_LY, FARTHEST_LY);
    for _ in 0..60 {
        let mid = (lo * hi).sqrt();
        if faint(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Each layer's rule bound of [`CAPPED_LAYERS`] at the apparent `cut` along `rays`: where the
/// brightest star the envelope allows the layer falls to it through the rays' least extinction.
///
/// # Panics
///
/// If the galaxy has no density component, which no galaxy is built without.
#[must_use]
fn rule_bounds(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    cut: f64,
    rays: &RayExtinctions,
    dust: &Reddening,
) -> Vec<f64> {
    let any = galaxy
        .fields()
        .component_ids()
        .next()
        .expect("a galaxy has components");
    CAPPED_LAYERS
        .iter()
        .map(|&layer| {
            let band = MassBand::from(layer);
            envelope
                .brightest(
                    layer,
                    any,
                    max_star_mass(SolarMasses::new(band.hi())),
                    (Years::ZERO, Years::new(super::envelope::MAX_AGE_YEARS)),
                )
                .map_or(NEAREST_LY, |m| rule_bound(m.value(), cut, rays, dust))
        })
        .collect()
}

/// A boundary's radius towards a ray of a [`CapCount`], ly.
///
/// # Panics
///
/// If it is negative or not finite: a NaN would drop every interval of the ray from the count
/// unseen, and a negative radius means no boundary.
#[must_use]
fn boundary_ly(radius: LightYears) -> f64 {
    let r = radius.value();
    assert!(
        r.is_finite() && r >= 0.0,
        "a boundary's radius is finite and not negative, not {r} ly"
    );
    r
}

/// The share of the interval `[r0, r1]`, even in ln r, that lies beyond `r`.
#[must_use]
fn share_beyond(r0: f64, r1: f64, r: f64) -> f64 {
    if r1 <= r {
        0.0
    } else if r0 >= r {
        1.0
    } else {
        math::ln(r1 / r) / math::ln(r1 / r0)
    }
}

/// A count of each layer's systems and of their stars brighter than a cut, ray by ray and radius by
/// radius, from an observer (Design note 9; R06.T7.b).
///
/// It is what the caps are drawn from, and what a finer count checks them against.
/// Its rays are a [`CapLattice`]'s, each standing for 4π ÷ n of the sky, and its radii run from 1
/// ly, even in ln r, to the farthest layer's rule bound. Each layer's count stops at its own rule
/// bound.
#[derive(Debug, Clone, PartialEq)]
pub struct CapCount {
    lattice: Arc<CapLattice>,
    /// Per layer of [`CAPPED_LAYERS`], the rule's bound, ly.
    bounds: Vec<f64>,
    /// The radii, ly, ascending.
    radii: Vec<f64>,
    /// The width of each interval in ln r.
    step_ln: f64,
    /// Per layer, ray and radius, in that order: the stars brighter than the cut per unit ln r in
    /// the ray's share of the sky.
    stars: Vec<f64>,
    /// Per layer, ray and radius: the systems per unit ln r in the ray's share of the sky.
    systems: Vec<f64>,
}

impl CapCount {
    /// The count for `observer` at `cut` (apparent V) and `resolution`.
    ///
    /// `tables` are the galaxy's, read at the observer's time through
    /// [`LuminosityTables::age_for`]; `cache` is the caller's noise cache for the rays.
    ///
    /// # Panics
    ///
    /// If the galaxy has no density component, which no galaxy is built without.
    #[must_use]
    pub fn measure(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        cut: Magnitudes,
        resolution: CapResolution,
        cache: &mut NoiseCache,
    ) -> Self {
        let lattice = Arc::new(CapLattice::new(resolution.rays));
        let cuts = vec![cut.value(); resolution.rays];
        Self::measure_on(
            galaxy,
            tables,
            envelope,
            observer,
            (lattice, resolution.sub_rays),
            cuts,
            resolution.steps_per_decade,
            cache,
        )
    }

    /// The count on `lattice`'s rays, each through the clearest of its `sub_rays` sub-rays at each
    /// distance and counting the stars brighter than its own cut of `cuts_v` (one a ray, apparent
    /// V), at `steps_per_decade` radial steps a decade. The rule's bound takes the deepest cut.
    #[expect(
        clippy::too_many_arguments,
        reason = "the count's inputs: the galaxy, its tables, the envelope, the observer, the rays, \
                  their cuts, the radial steps and the caller's cache"
    )]
    #[must_use]
    pub(crate) fn measure_on(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        (lattice, sub_rays): (Arc<CapLattice>, usize),
        cuts_v: Vec<f64>,
        steps_per_decade: u32,
        cache: &mut NoiseCache,
    ) -> Self {
        let rays =
            RayExtinctions::measure_clearest(galaxy, observer.position(), lattice, sub_rays, cache);
        Self::over(
            galaxy,
            tables,
            envelope,
            observer,
            &rays,
            cuts_v,
            steps_per_decade,
        )
    }

    /// The count over `rays`, a whole lattice measured from the observer, each counting the stars
    /// brighter than its own cut of `cuts_v` (one a ray, apparent V), at `steps_per_decade` radial
    /// steps a decade: [`measure_on`](Self::measure_on)'s count, for rays measured in shares
    /// ([`RayExtinctions::join`]; R06.T11.c), counted here in one part of every ray
    /// ([`CapCountPlan`]). The rule's bound takes the deepest cut.
    ///
    /// # Panics
    ///
    /// If `rays` are not a whole lattice measured from `observer`'s position on a
    /// [`CapLattice`] ([`RayExtinctions::measure_clearest_rays`]), or `cuts_v` does not hold one cut
    /// a ray.
    #[must_use]
    pub(crate) fn over(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        rays: &RayExtinctions,
        cuts_v: Vec<f64>,
        steps_per_decade: u32,
    ) -> Self {
        let plan = CapCountPlan::new(
            galaxy,
            tables,
            envelope,
            observer,
            rays,
            cuts_v,
            steps_per_decade,
        );
        let all = plan.count_rays(galaxy, tables, rays, 0..plan.rays());
        plan.join([all])
    }

    /// [`layer_caps_over`]'s count over `rays` at `cut`, planned for counting in parts of rays,
    /// each a job of a server's (R06.T11.g; see [`CapCountPlan`]).
    ///
    /// # Panics
    ///
    /// As [`layer_caps_over`].
    #[must_use]
    pub fn plan_over(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        cut: Magnitudes,
        rays: &RayExtinctions,
    ) -> CapCountPlan {
        assert_standard(rays);
        CapCountPlan::new(
            galaxy,
            tables,
            envelope,
            observer,
            rays,
            vec![cut.value(); rays.lattice],
            CapResolution::STANDARD.steps_per_decade,
        )
    }

    /// [`layer_caps_by_visibility_over`]'s count over `rays` by the eye's `visibility`, planned for
    /// counting in parts of rays, as [`plan_over`](Self::plan_over) plans [`layer_caps_over`]'s
    /// (R06.T11.g).
    ///
    /// # Panics
    ///
    /// As [`layer_caps_over`].
    #[must_use]
    pub fn plan_by_visibility_over(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        visibility: &EyeVisibility,
        rays: &RayExtinctions,
    ) -> CapCountPlan {
        assert_standard(rays);
        let lattice = rays.grid.as_ref().expect("the standard lattice's rays");
        CapCountPlan::new(
            galaxy,
            tables,
            envelope,
            observer,
            rays,
            visible_cuts_v(lattice, visibility),
            CapResolution::STANDARD.steps_per_decade,
        )
    }

    /// The lattice whose rays the count is taken along.
    #[must_use]
    pub fn lattice(&self) -> &CapLattice {
        &self.lattice
    }

    /// The index of `layer` in [`CAPPED_LAYERS`].
    ///
    /// # Panics
    ///
    /// If `layer` is not capped.
    #[must_use]
    fn layer_index(layer: Layer) -> usize {
        CAPPED_LAYERS
            .iter()
            .position(|&l| l == layer)
            .expect("a capped layer")
    }

    /// Ray `ray`'s values of layer `l` in `values` (the count's `stars` or `systems`), per unit
    /// ln r at each radius.
    #[must_use]
    fn ray_values<'a>(&self, values: &'a [f64], l: usize, ray: usize) -> &'a [f64] {
        let n = self.radii.len();
        let start = (l * self.lattice.directions.len() + ray) * n;
        &values[start..start + n]
    }

    /// The count of interval `i` of `per_ln_r` (one ray's values of layer `l`), by the trapezoid
    /// rule, and none for an interval at or beyond the layer's rule bound.
    #[must_use]
    fn slice(&self, per_ln_r: &[f64], l: usize, i: usize) -> f64 {
        if self.radii[i] >= self.bounds[l] {
            return 0.0;
        }
        f64::midpoint(per_ln_r[i], per_ln_r[i + 1]) * self.step_ln
    }

    /// The sum over the count's rays and intervals of `values` (its `stars` or `systems`) in
    /// `layer`, each interval weighted by `weight(r0, r1, radius)`, with `radius` the radius
    /// `radius_toward` gives towards the ray, ly.
    #[must_use]
    fn weighted(
        &self,
        values: &[f64],
        layer: Layer,
        radius_toward: impl Fn(UnitVector) -> LightYears,
        weight: impl Fn(f64, f64, f64) -> f64,
    ) -> f64 {
        let l = Self::layer_index(layer);
        let mut sum = 0.0;
        for (ray, &direction) in self.lattice.directions.iter().enumerate() {
            let radius = boundary_ly(radius_toward(direction));
            let per_ln_r = self.ray_values(values, l, ray);
            for i in 0..self.radii.len() - 1 {
                let share = weight(self.radii[i], self.radii[i + 1], radius);
                if share > 0.0 {
                    sum += self.slice(per_ln_r, l, i) * share;
                }
            }
        }
        sum
    }

    /// The expected count of `cap`'s layer's stars brighter than the cut beyond its radius towards
    /// each of the count's rays ([`LayerCap::radius_toward`]).
    ///
    /// # Panics
    ///
    /// If the cap's layer is not one of [`CAPPED_LAYERS`], or its radius is negative or not
    /// finite, which only a cap forced so could be.
    #[must_use]
    pub fn stars_beyond(&self, cap: &LayerCap) -> f64 {
        self.stars_beyond_toward(cap.layer(), |u| cap.radius_toward(u))
    }

    /// The expected count of `layer`'s stars brighter than the cut beyond the radius
    /// `radius_toward` gives towards each of the count's rays.
    ///
    /// It is [`stars_beyond`](Self::stars_beyond) for a boundary that no [`LayerCap`] holds, such
    /// as the radius towards each ray's band texel or a cone wider than the lattice's (rendering
    /// plan R13, R13.T1). `radius_toward` is called once a ray, in the lattice's order, with the
    /// ray's direction from the observer on the galactic axes, and gives the boundary's radius
    /// there, at least zero. A radius at or below the count's nearest distance, 1 ly, counts every
    /// star of the ray, out to the layer's rule bound.
    ///
    /// # Panics
    ///
    /// If `layer` is not one of [`CAPPED_LAYERS`], or a radius is negative or not finite.
    ///
    /// # Examples
    ///
    /// The stars near the Sun brighter than V 6.5 beyond the caps at a ceiling of V 4.5, and those
    /// of the whole sky (`no_run`: the tables take a minute or more to build):
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::caps::{CapCount, CapResolution};
    /// use hyperion_sim::sky::envelope::BrightnessEnvelope;
    /// use hyperion_sim::sky::luminosity::LuminosityTables;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::{LightYears, Magnitudes};
    ///
    /// let galaxy = Galaxy::new(Seed::new(7));
    /// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
    /// let mut cache = NoiseCache::with_capacity(1 << 16);
    /// let mut count = |v| {
    ///     let cut = Magnitudes::new(v);
    ///     CapCount::measure(&galaxy, &tables, &envelope, &observer, cut, CapResolution::STANDARD, &mut cache)
    /// };
    /// let (ceiling, faint) = (count(4.5).caps(), count(6.5));
    /// let e = &ceiling[4];
    /// let beyond = faint.stars_beyond_toward(Layer::E, |u| e.radius_toward(u));
    /// let whole = faint.stars_beyond_toward(Layer::E, |_| LightYears::ZERO);
    /// assert!(beyond < whole);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn stars_beyond_toward(
        &self,
        layer: Layer,
        radius_toward: impl Fn(UnitVector) -> LightYears,
    ) -> f64 {
        self.weighted(&self.stars, layer, radius_toward, share_beyond)
    }

    /// The expected count of `cap`'s layer's systems within its radius towards each ray: the
    /// systems its census opens, the cells' edges aside.
    ///
    /// # Panics
    ///
    /// If the cap's layer is not one of [`CAPPED_LAYERS`], or its radius is negative or not
    /// finite, which only a cap forced so could be.
    #[must_use]
    pub fn systems_within(&self, cap: &LayerCap) -> f64 {
        self.systems_within_toward(cap.layer(), |u| cap.radius_toward(u))
    }

    /// The expected count of `layer`'s systems within the radius `radius_toward` gives towards each
    /// of the count's rays.
    ///
    /// It is [`systems_within`](Self::systems_within) for a boundary that no [`LayerCap`] holds
    /// (R13.T1), whose radius `radius_toward` gives as for
    /// [`stars_beyond_toward`](Self::stars_beyond_toward).
    ///
    /// # Panics
    ///
    /// If `layer` is not one of [`CAPPED_LAYERS`], or a radius is negative or not finite.
    #[must_use]
    pub fn systems_within_toward(
        &self,
        layer: Layer,
        radius_toward: impl Fn(UnitVector) -> LightYears,
    ) -> f64 {
        self.weighted(&self.systems, layer, radius_toward, |r0, r1, r| {
            1.0 - share_beyond(r0, r1, r)
        })
    }

    /// The expected count of the stars brighter than the cut that lie within `inside`'s radius and
    /// beyond `outside`'s, towards each ray: those the first census lists and the second does not,
    /// the cells' edges aside.
    ///
    /// # Panics
    ///
    /// If the two caps are of different layers, their layer is not one of [`CAPPED_LAYERS`], or a
    /// radius is negative or not finite, which only a cap forced so could be.
    #[must_use]
    pub fn stars_within_and_beyond(&self, inside: &LayerCap, outside: &LayerCap) -> f64 {
        assert_eq!(inside.layer(), outside.layer(), "two caps of one layer");
        self.stars_within_and_beyond_toward(
            inside.layer(),
            |u| inside.radius_toward(u),
            |u| outside.radius_toward(u),
        )
    }

    /// The expected count of `layer`'s stars brighter than the cut that lie within the radius
    /// `inside` gives towards each of the count's rays and beyond the one `outside` gives.
    ///
    /// It is [`stars_within_and_beyond`](Self::stars_within_and_beyond) for boundaries that no
    /// [`LayerCap`] holds, whose radii `inside` and `outside` give as for
    /// [`stars_beyond_toward`](Self::stars_beyond_toward). R13.T1 counts T7.b's gap with it: the
    /// stars within the radius towards each ray's band texel and beyond the one towards the ray
    /// itself (`decision-r06-t8i-listing.md` §2, "cheap measurement").
    ///
    /// # Panics
    ///
    /// If `layer` is not one of [`CAPPED_LAYERS`], or a radius is negative or not finite.
    #[must_use]
    pub fn stars_within_and_beyond_toward(
        &self,
        layer: Layer,
        inside: impl Fn(UnitVector) -> LightYears,
        outside: impl Fn(UnitVector) -> LightYears,
    ) -> f64 {
        let l = Self::layer_index(layer);
        let mut sum = 0.0;
        for (ray, &direction) in self.lattice.directions.iter().enumerate() {
            let (a, b) = (
                boundary_ly(inside(direction)),
                boundary_ly(outside(direction)),
            );
            let per_ln_r = self.ray_values(&self.stars, l, ray);
            for i in 0..self.radii.len() - 1 {
                let (r0, r1) = (self.radii[i], self.radii[i + 1]);
                let share = share_beyond(r0, r1, b) - share_beyond(r0, r1, a);
                if share > 0.0 {
                    sum += self.slice(per_ln_r, l, i) * share;
                }
            }
        }
        sum
    }

    /// Each layer's cap by ray (R06.T7.b; see the [module](self) documentation).
    ///
    /// Each ray's radius is the outer edge of its outermost interval yielding at least λ stars a
    /// system, λ the largest that keeps the layer's expected count beyond under one, then widened
    /// to the largest of its neighbours' within [`WIDENING_SPACINGS`] spacings. Each cap states its
    /// expected count beyond the census's radius towards each of the count's rays.
    #[must_use]
    pub fn caps(&self) -> Vec<LayerCap> {
        self.caps_widened(WIDENING_SPACINGS)
    }

    /// [`caps`](Self::caps), each ray widened to the largest radius within `spacings` of the
    /// lattice's spacing.
    #[must_use]
    fn caps_widened(&self, spacings: f64) -> Vec<LayerCap> {
        CAPPED_LAYERS
            .iter()
            .enumerate()
            .map(|(l, &layer)| self.cap_widened(l, layer, spacings))
            .collect()
    }

    /// `layer`'s cap of [`caps`](Self::caps), bit for bit: each layer's cap is drawn from its own
    /// count alone, so a server draws them a job a layer (R06.T11.g).
    ///
    /// # Panics
    ///
    /// If `layer` is not one of [`CAPPED_LAYERS`].
    #[must_use]
    pub fn cap(&self, layer: Layer) -> LayerCap {
        self.cap_widened(Self::layer_index(layer), layer, WIDENING_SPACINGS)
    }

    /// `layer`'s cap by [`cap`](Self::cap)'s rule with an expected count beyond of under `budget`
    /// stars instead of one: λ the largest yield that keeps the stars left out under `budget`, then
    /// the same widening. A budget of one is [`cap`](Self::cap), bit for bit, and a larger one
    /// holds every ray's radius at or within it.
    ///
    /// It is for the record only (rendering plan R13, R13.T1.b): a yield-ordered budget for D and
    /// E, weighed for a later ruling (R13's Risks), which no census takes.
    ///
    /// # Panics
    ///
    /// If `layer` is not one of [`CAPPED_LAYERS`], or `budget` is not finite and positive.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn cap_with_budget(&self, layer: Layer, budget: f64) -> LayerCap {
        assert!(
            budget.is_finite() && budget > 0.0,
            "a budget of stars beyond is finite and positive, not {budget}"
        );
        self.cap_budgeted(Self::layer_index(layer), layer, WIDENING_SPACINGS, budget)
    }

    /// The cap of `layer`, the `l`-th of [`CAPPED_LAYERS`], each ray widened to the largest radius
    /// within `spacings` of the lattice's spacing ([`caps_widened`](Self::caps_widened)).
    #[must_use]
    fn cap_widened(&self, l: usize, layer: Layer, spacings: f64) -> LayerCap {
        self.cap_budgeted(l, layer, spacings, 1.0)
    }

    /// [`cap_widened`](Self::cap_widened) with λ the largest yield that keeps the stars left out
    /// under `budget` ([`ray_extents`]).
    #[must_use]
    fn cap_budgeted(&self, l: usize, layer: Layer, spacings: f64, budget: f64) -> LayerCap {
        let n_rays = self.lattice.directions.len();
        let intervals = self.radii.len() - 1;
        let bound = self.bounds[l];
        let mut stars = Vec::with_capacity(n_rays * intervals);
        let mut systems = Vec::with_capacity(n_rays * intervals);
        for ray in 0..n_rays {
            let (s, n) = (
                self.ray_values(&self.stars, l, ray),
                self.ray_values(&self.systems, l, ray),
            );
            for i in 0..intervals {
                stars.push(self.slice(s, l, i));
                systems.push(self.slice(n, l, i));
            }
        }
        let kept = ray_extents(&stars, &systems, intervals, budget);
        let radii: Vec<f64> = kept.iter().map(|&k| self.radii[k].min(bound)).collect();
        let rays = RayRadii::new(
            Arc::clone(&self.lattice),
            widen(&self.lattice, &radii, spacings),
        );
        let mut cap = LayerCap {
            layer,
            radius: rays.largest(),
            rule_bound: LightYears::new(bound),
            expected_beyond: 0.0,
            bright_beyond: None,
            rays: Some(rays),
        };
        cap.expected_beyond = self.stars_beyond(&cap);
        cap
    }

    /// Each layer's cap as one sphere, as Design note 9 took it before R06.T7.b, for comparison:
    /// the least radius beyond which the layer's expected count, summed over the rays, falls below
    /// one, by the trapezoid rule from the rule bound inward.
    #[must_use]
    pub fn spheres(&self) -> Vec<LayerCap> {
        let n_rays = self.lattice.directions.len();
        let radii = &self.radii;
        CAPPED_LAYERS
            .iter()
            .enumerate()
            .map(|(l, &layer)| {
                let bound = self.bounds[l];
                // The count over the sky at each radius, summed ray by ray in order.
                let per_ln_r: Vec<f64> = (0..radii.len())
                    .map(|i| {
                        (0..n_rays)
                            .map(|ray| self.ray_values(&self.stars, l, ray)[i])
                            .fold(0.0, |sum, v| sum + v)
                    })
                    .collect();
                let mut beyond = 0.0;
                let mut radius = bound;
                let mut stated = 0.0;
                for i in (0..radii.len() - 1).rev() {
                    if radii[i] >= bound {
                        continue;
                    }
                    let slice = f64::midpoint(per_ln_r[i], per_ln_r[i + 1]) * self.step_ln;
                    if beyond + slice >= 1.0 {
                        radius = radii[i + 1].min(bound);
                        stated = beyond;
                        break;
                    }
                    beyond += slice;
                    radius = radii[i];
                    stated = beyond;
                }
                LayerCap {
                    layer,
                    radius: LightYears::new(radius),
                    rule_bound: LightYears::new(bound),
                    expected_beyond: stated,
                    bright_beyond: None,
                    rays: None,
                }
            })
            .collect()
    }
}

/// A [`CapCount`] over measured rays, planned before any ray is counted, for a server that counts
/// its rays in parts as jobs of its own and joins them (R06.T11.g;
/// `decision-r06-t11d-first-sky.md` §1.4): its rays' cuts, each layer's rule bound, the radial
/// nodes to the farthest bound, the light ages there and each layer's share of each component.
///
/// Made by [`CapCount::plan_over`] or [`CapCount::plan_by_visibility_over`]. Each ray's count is a
/// function of its own direction, cut and extinction profile alone, so the rays counted in any
/// parts ([`count_rays`](Self::count_rays)), joined ([`join`](Self::join)), are the count in one
/// job bit for bit, and so are its caps ([`CapCount::cap`], [`CapCount::caps`]). As one job the
/// count near the Sun holds a worker for a second or more, its 1,536 rays each through some 120
/// radial nodes of every layer.
///
/// # Examples
///
/// The caps near the Sun with the count's rays in four parts, as four jobs of a server's would
/// (`no_run`: the tables take a minute or more to build):
///
/// ```no_run
/// use std::sync::Arc;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::caps::{
///     CAP_RAYS, CAPPED_LAYERS, CapCount, CapLattice, RayExtinctions, SUB_RAYS, layer_caps,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let mut noise = NoiseCache::with_capacity(1 << 16);
/// let lattice = Arc::new(CapLattice::new(CAP_RAYS));
/// let rays = RayExtinctions::measure_clearest(&galaxy, &sun, lattice, SUB_RAYS, &mut noise);
/// let cut = Magnitudes::new(7.95);
/// let plan = CapCount::plan_over(&galaxy, &tables, &envelope, &observer, cut, &rays);
/// let quarter = CAP_RAYS / 4;
/// let parts: Vec<_> = (0..4)
///     .map(|k| plan.count_rays(&galaxy, &tables, &rays, k * quarter..(k + 1) * quarter))
///     .collect();
/// let count = plan.join(parts);
/// let caps: Vec<_> = CAPPED_LAYERS.iter().map(|&layer| count.cap(layer)).collect();
/// assert_eq!(caps, layer_caps(&galaxy, &tables, &envelope, &observer, cut, &mut noise));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CapCountPlan {
    lattice: Arc<CapLattice>,
    /// The rays' origin, ly.
    origin_ly: [f64; 3],
    /// Each ray's cut, apparent V, in the lattice's order. Shared with the plan's parts, by which
    /// [`join`](Self::join) knows a part as its own: two plans over one lattice share its rays but
    /// not their cuts.
    cuts_v: Arc<[f64]>,
    /// The reddening curves each ray's extinction is taken through: the solar point's.
    dust: Reddening,
    /// Per layer of [`CAPPED_LAYERS`], the rule's bound, ly.
    bounds: Vec<f64>,
    /// The radii, ly, ascending.
    radii: Vec<f64>,
    /// The light age at each radius.
    ages: Vec<Span>,
    /// The width of each interval in ln r.
    step_ln: f64,
    /// The galaxy's density components, in order.
    components: Vec<ComponentId>,
    /// Per layer, each component's share of the layer's systems.
    shares: [[f64; MAX_COMPONENTS]; CAPPED_LAYERS.len()],
    /// Each ray's solid angle, sr.
    weight: f64,
}

/// The count of some consecutive rays of a [`CapCountPlan`] ([`CapCountPlan::count_rays`]), for
/// [`CapCountPlan::join`].
#[derive(Debug, Clone, PartialEq)]
pub struct CapCountPart {
    rays: Range<usize>,
    /// The cuts of the plan counted, by which a plan joins only its own parts.
    plan: Arc<[f64]>,
    /// Per layer, ray of `rays` and radius, in that order: the stars per unit ln r.
    stars: Vec<f64>,
    /// Per layer, ray and radius: the systems per unit ln r.
    systems: Vec<f64>,
}

impl CapCountPart {
    /// The rays counted, by their place in the lattice.
    #[must_use]
    pub fn rays(&self) -> Range<usize> {
        self.rays.clone()
    }
}

impl CapCountPlan {
    /// The count's plan over `rays` from `observer`, each counting the stars brighter than its own
    /// cut of `cuts_v`, at `steps_per_decade` radial steps a decade.
    ///
    /// # Panics
    ///
    /// As [`CapCount::over`].
    #[must_use]
    fn new(
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        envelope: &BrightnessEnvelope,
        observer: &Observer,
        rays: &RayExtinctions,
        cuts_v: Vec<f64>,
        steps_per_decade: u32,
    ) -> Self {
        let origin = observer.position();
        assert!(
            rays.is_lattice_from(origin),
            "the caps count a whole lattice of rays from the observer"
        );
        let lattice = Arc::clone(
            rays.grid
                .as_ref()
                .expect("the caps count the rays of a lattice"),
        );
        assert_eq!(cuts_v.len(), lattice.directions().len(), "one cut a ray");
        let dust = solar_colour().reddening();
        let deepest = cuts_v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let fields = galaxy.fields();
        let components: Vec<ComponentId> = fields.component_ids().collect();
        let bounds = rule_bounds(galaxy, envelope, deepest, rays, &dust);
        let farthest = bounds.iter().copied().fold(NEAREST_LY, f64::max);
        // The radial nodes, from the nearest to the farthest bound, even in ln r.
        let decades = math::log10(farthest / NEAREST_LY).max(0.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few decades of distance, a few hundred steps"
        )]
        let steps = ((decades * f64::from(steps_per_decade)).ceil() as u32).max(1);
        let radii: Vec<f64> = (0..=steps)
            .map(|k| NEAREST_LY * math::exp10(decades * f64::from(k) / f64::from(steps)))
            .collect();
        let ages: Vec<Span> = radii
            .iter()
            .map(|&r| {
                tables.age_for(
                    observer.time(),
                    Span::from_seconds_f64(r * SECONDS_PER_JULIAN_YEAR).unwrap_or(Span::ZERO),
                )
            })
            .collect();
        let mut shares = [[0.0; MAX_COMPONENTS]; CAPPED_LAYERS.len()];
        for (layer_shares, &layer) in shares.iter_mut().zip(&CAPPED_LAYERS) {
            let band = MassBand::from(layer);
            for &id in &components {
                layer_shares[id.index()] =
                    galaxy.shares().component_share(band, fields.component(id));
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "a few thousand rays")]
        let weight = 4.0 * core::f64::consts::PI / lattice.directions().len() as f64;
        Self {
            origin_ly: origin.to_light_years_f64(),
            lattice,
            cuts_v: cuts_v.into(),
            dust,
            bounds,
            radii,
            ages,
            step_ln: math::ln(10.0) * decades / f64::from(steps),
            components,
            shares,
            weight,
        }
    }

    /// The rays the count is taken along, the lattice's.
    #[must_use]
    pub fn rays(&self) -> usize {
        self.lattice.directions().len()
    }

    /// The count of the rays `which` of the plan, by their place in its lattice, over `rays`, the
    /// rays it was planned over: per layer, ray and radius, r³ × the ray's solid angle × the
    /// density of the layer's systems there, and of their stars brighter than the ray's cut.
    ///
    /// # Panics
    ///
    /// If `which` reaches past the lattice's last ray, or `rays` are not of the plan's lattice.
    #[must_use]
    pub fn count_rays(
        &self,
        galaxy: &Galaxy,
        tables: &LuminosityTables,
        rays: &RayExtinctions,
        which: Range<usize>,
    ) -> CapCountPart {
        assert!(
            same_grid(rays.grid.as_ref(), Some(&self.lattice)),
            "a count's rays are the rays it was planned over"
        );
        let directions = self.lattice.directions();
        assert!(
            which.end <= directions.len(),
            "rays {which:?} of a lattice of {}",
            directions.len()
        );
        let fields = galaxy.fields();
        let p0 = self.origin_ly;
        let (n_rays, n_radii) = (which.len(), self.radii.len());
        let len = CAPPED_LAYERS.len() * n_rays * n_radii;
        let (mut stars, mut systems) = (vec![0.0; len], vec![0.0; len]);
        let mut densities = [0.0; MAX_COMPONENTS];
        for (k, ray) in which.clone().enumerate() {
            let u = directions[ray].components();
            for (i, (&r, &ago)) in self.radii.iter().zip(&self.ages).enumerate() {
                let limit = Magnitudes::new(faintest_counted(
                    self.cuts_v[ray],
                    r,
                    rays.along(ray, r),
                    &self.dust,
                ));
                let p = PointLy::new(p0[0] + r * u[0], p0[1] + r * u[1], p0[2] + r * u[2]);
                fields.densities(&p, &mut densities);
                for (l, &layer) in CAPPED_LAYERS.iter().enumerate() {
                    if r > self.bounds[l] {
                        continue;
                    }
                    let (mut n, mut s) = (0.0, 0.0);
                    for &id in &self.components {
                        let rho = densities[id.index()];
                        if rho <= 0.0 {
                            continue;
                        }
                        let share = self.shares[l][id.index()];
                        n += rho
                            * share
                            * tables.get_at(id, layer, &p).count_brighter_than(limit, ago);
                        s += rho * share;
                    }
                    let at = (l * n_rays + k) * n_radii + i;
                    stars[at] = self.weight * r * r * r * n;
                    systems[at] = self.weight * r * r * r * s;
                }
            }
        }
        CapCountPart {
            rays: which,
            plan: Arc::clone(&self.cuts_v),
            stars,
            systems,
        }
    }

    /// The count from its rays counted in `parts` ([`count_rays`](Self::count_rays)), in any order
    /// and any split.
    ///
    /// # Panics
    ///
    /// Unless `parts` count every ray of the plan's lattice once, each part of the plan's.
    #[must_use]
    pub fn join(&self, parts: impl IntoIterator<Item = CapCountPart>) -> CapCount {
        let (n_rays, n_radii) = (self.lattice.directions().len(), self.radii.len());
        let len = CAPPED_LAYERS.len() * n_rays * n_radii;
        let (mut stars, mut systems) = (vec![0.0; len], vec![0.0; len]);
        let mut counted = vec![false; n_rays];
        for part in parts {
            assert!(
                Arc::ptr_eq(&part.plan, &self.cuts_v),
                "a count joins the parts of its own plan"
            );
            let width = part.rays.len();
            let span = width * n_radii;
            assert_eq!(
                (part.stars.len(), part.systems.len()),
                (CAPPED_LAYERS.len() * span, CAPPED_LAYERS.len() * span),
                "a part of {width} rays counts every layer at each of the plan's {n_radii} radii"
            );
            for ray in part.rays.clone() {
                assert!(!counted[ray], "ray {ray} of the count is counted twice");
                counted[ray] = true;
            }
            for l in 0..CAPPED_LAYERS.len() {
                let from = l * width * n_radii;
                let to = (l * n_rays + part.rays.start) * n_radii;
                stars[to..to + span].copy_from_slice(&part.stars[from..from + span]);
                systems[to..to + span].copy_from_slice(&part.systems[from..from + span]);
            }
        }
        if let Some(ray) = counted.iter().position(|&done| !done) {
            panic!("ray {ray} of the count is not counted");
        }
        CapCount {
            lattice: Arc::clone(&self.lattice),
            bounds: self.bounds.clone(),
            radii: self.radii.clone(),
            step_ln: self.step_ln,
            stars,
            systems,
        }
    }
}

/// Each ray's intervals to keep, by expected yield (R06.T7.b): with `stars` and `systems` each
/// ray's count of each of its `intervals` intervals in turn, the number of a ray's intervals, from
/// the nearest, up to and including its outermost whose yield (stars ÷ systems) is at least λ, the
/// largest value that keeps the stars of the intervals left out, over every ray, under `budget`.
/// A ray with no such interval keeps none.
///
/// λ is one of the yields: the expected count left out only falls as λ does, so the largest λ is
/// found by bisection over the yields, descending.
#[must_use]
fn ray_extents(stars: &[f64], systems: &[f64], intervals: usize, budget: f64) -> Vec<usize> {
    let rays = stars.len() / intervals.max(1);
    // An interval of stars and no systems, which no count holds, would be the richest of all.
    let yield_of = |at: usize| {
        if systems[at] > 0.0 {
            stars[at] / systems[at]
        } else if stars[at] > 0.0 {
            f64::INFINITY
        } else {
            0.0
        }
    };
    // The intervals kept at λ, and the stars of the rest.
    let kept_at = |lambda: f64| -> (Vec<usize>, f64) {
        let mut left_out = 0.0;
        let kept = (0..rays)
            .map(|ray| {
                let from = ray * intervals;
                let kept = (0..intervals)
                    .rev()
                    .find(|&i| yield_of(from + i) >= lambda)
                    .map_or(0, |i| i + 1);
                for i in kept..intervals {
                    left_out += stars[from + i];
                }
                kept
            })
            .collect();
        (kept, left_out)
    };
    let (none, all) = kept_at(f64::INFINITY);
    if all < budget {
        return none;
    }
    let mut yields: Vec<f64> = (0..stars.len())
        .map(yield_of)
        .filter(|&y| y > 0.0)
        .collect();
    yields.sort_by(|a, b| b.total_cmp(a));
    yields.dedup_by(|a, b| a.total_cmp(b).is_eq());
    if yields.is_empty() {
        return none;
    }
    // The first yield, descending, whose λ leaves out under the budget: at the least it leaves out
    // only intervals of no stars.
    let (mut lo, mut hi) = (0, yields.len() - 1);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if kept_at(yields[mid]).1 < budget {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    kept_at(yields[lo]).0
}

/// Each ray's radius of `radii` raised to the largest of the rays' within `spacings` of the
/// lattice's spacing of it.
#[must_use]
fn widen(lattice: &CapLattice, radii: &[f64], spacings: f64) -> Vec<f64> {
    let reach = spacings * lattice.spacing + ANGLE_MARGIN_RAD;
    lattice
        .directions
        .iter()
        .map(|&direction| {
            rays_within(&lattice.directions, direction, reach)
                .map(|k| radii[k])
                .fold(0.0, f64::max)
        })
        .collect()
}

/// Each layer's cap for `observer` at `cut` (apparent V) at the standard resolution, one radius a
/// ray: see the [module](self) documentation.
///
/// `tables` are the galaxy's, read at the observer's time through
/// [`LuminosityTables::age_for`]; `cache` is the caller's noise cache for the rays.
///
/// Measured at cut 7.95 (the eye's near the Sun), version 21, with v☉, 1,536 rays and three
/// sub-rays a ray (R06.T7.b): each layer's rays' median radius, with the 10th and 90th
/// percentiles and the largest, against R06.T7's sphere at v☉ (one radius a layer, 768 rays,
/// one profile a ray). They stand against the brainstorm's version-14 estimates (C 3,000, D 4,300
/// and E 10,000 ly near the Sun; a few hundred to about 1,000 in the nuclear disc), which the tests
/// do not pin (decided 2026-10-03 and 2026-10-04; the slow test `caps_converge_in_rays`, whose six
/// points R06's Risks record, "Deviations in T7.b, as built"; T17 re-derives them). Radii in ly:
///
/// | Point (ly) | Layer | Rays: median (10th–90th), largest | Sphere |
/// | ---------- | ----- | --------------------------------- | ------ |
/// | Near the Sun (0, 26,000, 68) | A | 10 (10–10), 11 | 11 |
/// | | B | 68 (62–68), 68 | 68 |
/// | | C | 8,193 (6,764–9,018), 14,563 | 8,193 |
/// | | D | 6,764 (6,146–9,925), 13,232 | 9,925 |
/// | | E | 4,610 (4,188–14,563), 61,341 | 21,369 |
/// | Nuclear disc (0, 150, 0) | A | 12 (12–13), 13 | 13 |
/// | | B | 18 (16–18), 18 | 18 |
/// | | C | 74 (61–120), 213 | 131 |
/// | | D | 99 (82–213), 344 | 232 |
/// | | E | 235 (145–417), 555 | 374 |
///
/// The brown dwarfs' are 1 ly. Near the Sun the rays open 74% of the spheres' systems, and E's
/// 33% of its sphere's. C's cap rests on rare bright phases of pair channels whose fitted rate rests
/// on a few of the fit's systems (R06 Risks, "T5.d's pair-evolved light, as built").
///
/// # Panics
///
/// If the galaxy has no density component, which no galaxy is built without.
#[must_use]
pub fn layer_caps(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    layer_caps_at(
        galaxy,
        tables,
        envelope,
        observer,
        cut,
        CapResolution::STANDARD,
        cache,
    )
}

/// [`layer_caps`] at `resolution`.
///
/// # Panics
///
/// As [`layer_caps`].
#[must_use]
pub fn layer_caps_at(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    resolution: CapResolution,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    CapCount::measure(galaxy, tables, envelope, observer, cut, resolution, cache).caps()
}

/// [`layer_caps`] over rays already measured: `rays`, the whole lattice of
/// [`CapResolution::STANDARD`]'s [`CAP_RAYS`] rays from the observer's position, each the clearest
/// of its [`SUB_RAYS`] sub-rays, which a server measures in shares on its pool
/// ([`RayExtinctions::measure_clearest_rays`], joined in order), each with a noise cache of its own
/// (decided 2026-10-03, `decision-r06-tables.md`, item B.4; R06.T11.c).
///
/// The count is serial in ray order, as [`layer_caps`]' is, so the caps are its own bit for bit:
/// the rays are, since each is a function of its own direction alone and a cache changes no value.
///
/// # Panics
///
/// If `rays` are not the whole standard lattice measured from `observer`'s position with its
/// sub-rays, or as [`layer_caps`] panics.
///
/// # Examples
///
/// The caps near the Sun with their rays measured in four shares, as four jobs of a server's
/// would, each with a noise cache of its own (`no_run`: the tables take a minute or more to build).
///
/// ```no_run
/// use std::sync::Arc;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::caps::{
///     CAP_RAYS, CapLattice, RayExtinctions, SUB_RAYS, layer_caps, layer_caps_over,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let lattice = Arc::new(CapLattice::new(CAP_RAYS));
/// let quarter = CAP_RAYS / 4;
/// let shares = (0..4).map(|k| {
///     let mut own = NoiseCache::with_capacity(1 << 16);
///     let which = k * quarter..(k + 1) * quarter;
///     let lattice = Arc::clone(&lattice);
///     RayExtinctions::measure_clearest_rays(&galaxy, &sun, lattice, SUB_RAYS, which, &mut own)
/// });
/// let rays = RayExtinctions::join(shares);
/// let cut = Magnitudes::new(7.95);
/// let caps = layer_caps_over(&galaxy, &tables, &envelope, &observer, cut, &rays);
/// let mut cache = NoiseCache::with_capacity(1 << 16);
/// assert_eq!(caps, layer_caps(&galaxy, &tables, &envelope, &observer, cut, &mut cache));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn layer_caps_over(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    rays: &RayExtinctions,
) -> Vec<LayerCap> {
    assert_standard(rays);
    CapCount::over(
        galaxy,
        tables,
        envelope,
        observer,
        rays,
        vec![cut.value(); rays.lattice],
        CapResolution::STANDARD.steps_per_decade,
    )
    .caps()
}

/// [`layer_caps_by_visibility`] over rays already measured, as [`layer_caps_over`] is
/// [`layer_caps`]' (R06.T11.c).
///
/// # Panics
///
/// As [`layer_caps_over`].
#[must_use]
pub fn layer_caps_by_visibility_over(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    visibility: &EyeVisibility,
    rays: &RayExtinctions,
) -> Vec<LayerCap> {
    assert_standard(rays);
    let lattice = rays.grid.as_ref().expect("the standard lattice's rays");
    CapCount::over(
        galaxy,
        tables,
        envelope,
        observer,
        rays,
        visible_cuts_v(lattice, visibility),
        CapResolution::STANDARD.steps_per_decade,
    )
    .caps()
}

/// Panics unless `rays` are the standard resolution's: [`CAP_RAYS`] rays of the standard lattice,
/// each the clearest of its [`SUB_RAYS`] sub-rays.
fn assert_standard(rays: &RayExtinctions) {
    assert!(
        rays.lattice == CAP_RAYS
            && rays.sub_rays == SUB_RAYS
            && rays
                .grid
                .as_ref()
                .is_some_and(|g| g.directions.len() == CAP_RAYS),
        "the standard caps count the standard lattice's rays, each the clearest of its sub-rays"
    );
}

/// The expected count of each layer's stars brighter than `cut` beyond its cap in `caps` (its
/// radius towards each ray, [`LayerCap::radius_toward`]), recounted at `resolution`: the check
/// that a finer count agrees with the caps' own claim.
///
/// # Panics
///
/// As [`layer_caps`], and if `caps` does not hold a cap for every layer of [`CAPPED_LAYERS`].
#[expect(
    clippy::too_many_arguments,
    reason = "the caps' inputs, the resolution, the caps checked and the caller's cache"
)]
#[must_use]
pub fn expected_beyond_caps(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    cut: Magnitudes,
    resolution: CapResolution,
    caps: &[LayerCap],
    cache: &mut NoiseCache,
) -> Vec<f64> {
    let count = CapCount::measure(galaxy, tables, envelope, observer, cut, resolution, cache);
    CAPPED_LAYERS
        .iter()
        .map(|&layer| {
            let cap = caps
                .iter()
                .find(|c| c.layer() == layer)
                .expect("a cap for every layer");
            count.stars_beyond(cap)
        })
        .collect()
}

/// Each ray's cut by the eye's visibility, V, one a ray of `lattice` (R06.T7.b).
///
/// It is the deepest limit of `visibility`'s 16² texels whose centres lie within the lattice's
/// spacing and twice the texels' largest radius of the ray, held at the visibility's cut. That
/// reach holds every texel the ray's cone meets and more about it: the final map's finer texels may
/// be darker than the 16² texel they lie in by more than the cut's pad, and with the cone's
/// texels alone the safety test failed by 0.042 mag at 1,536 rays.
#[must_use]
fn visible_cuts_v(lattice: &CapLattice, visibility: &EyeVisibility) -> Vec<f64> {
    let reach =
        lattice.spacing + 2.0 * visibility.spec().largest_texel_radius().value() + ANGLE_MARGIN_RAD;
    let cos = math::cos(reach);
    let texels: Vec<(UnitVector, f64)> = visibility
        .texels()
        .map(|(centre, limit)| (centre, limit.value()))
        .collect();
    let cut = visibility.cut().value();
    lattice
        .directions
        .iter()
        .map(|ray| {
            texels
                .iter()
                .filter(|(centre, _)| centre.dot(ray) >= cos)
                .map(|&(_, limit)| limit)
                .fold(f64::NEG_INFINITY, f64::max)
                .min(cut)
        })
        .collect()
}

/// Each layer's cap by ray for an eye-only request, each ray counting the stars the eye can see in
/// its cone rather than all those brighter than the uniform cut.
///
/// These are the visibility-based caps (R06.T7.b; adopted under
/// `decision-r06-census-cost-signoff.md`, alternative 1): [`layer_caps`]' count and criterion, at
/// each ray's cut of `visibility`, the eye cut's pre-pass at the eye's cut
/// ([`eye_visibility`](super::limits::eye_visibility)), its deepest limit about the ray's cone, held
/// at the cut. The rule's bound takes the cut.
///
/// The eye's cut is set by the darkest sky, the poles'; in the plane, where the far records lie,
/// the eye sees less: near the Sun the rays' cuts run from 7.21 to the eye's 8.28. There these caps
/// open some three fifths of the systems of [`layer_caps`]' at the cut, and every star the eye can
/// see is counted: no texel of the final limit map, with no glare, is deeper than its rays' cuts
/// less the colour offset (R06's Risks, "Deviations in T7.b, as built").
///
/// # Panics
///
/// As [`layer_caps`].
///
/// # Examples
///
/// An eye-only request's census, capped by what the eye sees (`no_run`: the tables take a minute
/// or more to build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::EyeObserver;
/// use hyperion_sim::sky::caps::{layer_caps, layer_caps_by_visibility};
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext, SkyQuery, census_plan};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::limits::{eye_cut, eye_visibility};
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let eye = EyeObserver::default();
/// // The eye's cut, then the eye's limit across the sky at it, both from the coarse pre-pass.
/// let cut = eye_cut(&galaxy, &mut ctx, &observer, &eye, None);
/// let visibility = eye_visibility(&galaxy, &mut ctx, &observer, &eye, cut, None);
/// let seen = layer_caps_by_visibility(&galaxy, &tables, &envelope, &observer, &visibility, &mut ctx.noise);
/// let uniform = layer_caps(&galaxy, &tables, &envelope, &observer, cut, &mut ctx.noise);
/// assert!(seen.iter().zip(&uniform).all(|(s, u)| s.radius() <= u.radius()));
/// // A request that asks it, whose census plan takes these caps.
/// let query = SkyQuery::builder(observer, cut).eye(eye).eye_visibility(visibility).build()?;
/// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut ctx.noise);
/// assert_eq!(plan.caps(), seen.as_slice());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn layer_caps_by_visibility(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    visibility: &EyeVisibility,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    let lattice = Arc::new(CapLattice::new(CAP_RAYS));
    let cuts = visible_cuts_v(&lattice, visibility);
    CapCount::measure_on(
        galaxy,
        tables,
        envelope,
        observer,
        (lattice, CapResolution::STANDARD.sub_rays),
        cuts,
        RADIAL_STEPS_PER_DECADE,
        cache,
    )
    .caps()
}

/// [`expected_beyond_caps`] for caps by visibility ([`layer_caps_by_visibility`]): each of the
/// recount's rays counts the stars brighter than the deepest cut of the caps' rays whose cones hold
/// it, as the census lists towards it.
///
/// # Panics
///
/// As [`expected_beyond_caps`], and if the caps are not one radius a ray.
#[expect(
    clippy::too_many_arguments,
    reason = "the caps' inputs, the eye's visibility, the resolution, the caps checked and the \
              caller's cache"
)]
#[must_use]
pub fn expected_beyond_caps_by_visibility(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    observer: &Observer,
    visibility: &EyeVisibility,
    resolution: CapResolution,
    caps: &[LayerCap],
    cache: &mut NoiseCache,
) -> Vec<f64> {
    let lattice = caps
        .first()
        .and_then(LayerCap::rays)
        .expect("caps of one radius a ray")
        .lattice();
    let ray_cuts = visible_cuts_v(lattice, visibility);
    let fine = Arc::new(CapLattice::new(resolution.rays));
    let cuts: Vec<f64> = fine
        .directions
        .iter()
        .map(|u| {
            lattice
                .near(lattice.texel(u.components()))
                .iter()
                .filter(|&&k| lattice.directions[k].dot(u) >= lattice.cos_spacing)
                .map(|&k| ray_cuts[k])
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect();
    let count = CapCount::measure_on(
        galaxy,
        tables,
        envelope,
        observer,
        (fine, resolution.sub_rays),
        cuts,
        resolution.steps_per_decade,
        cache,
    );
    CAPPED_LAYERS
        .iter()
        .map(|&layer| {
            let cap = caps
                .iter()
                .find(|c| c.layer() == layer)
                .expect("a cap for every layer");
            count.stars_beyond(cap)
        })
        .collect()
}

/// Each layer's real boundary at a synthetic ceiling (rendering plan R13, Design note 3, in
/// R13.T1's ruled form): for C, D and E each ray the least of the layer's cap at the ceiling, its
/// cap at the request's cut and [`REAL_LIMIT_LY`]; for A, B and the brown dwarfs their caps at the
/// cut, which are wholly real.
///
/// `at_ceiling` is the count at the ceiling V<sub>P</sub>, one uniform cut over the rays of
/// `at_cut`, the request's own count (at its cut, or by the eye's visibility), and `caps_at_cut`
/// are `at_cut`'s caps ([`CapCount::cap`]), one for each layer of [`CAPPED_LAYERS`], as a server
/// draws them a job a layer (R06.T11.g), so that they are not drawn twice. The caps come back in
/// `caps_at_cut`'s order. Each of C's, D's and E's rays is its cap at V<sub>P</sub>'s
/// (`at_ceiling`'s [`CapCount::cap`], widened as R06.T7.b widens), held within the same ray of its
/// cap at the cut ([`RayRadii::lesser`]) and then within the limit ([`RayRadii::within`]), as
/// R13.T1's and R13.T1.b's benches built it; the two holds commute, bit for bit. Each such cap
/// keeps its cut's rule bound and states:
///
/// - its [`expected_beyond`](LayerCap::expected_beyond) at the cut beyond R(u) (`at_cut`'s
///   [`CapCount::stars_beyond`]): the stars the synthetic tier and the band take, near the Sun on
///   the test fixture at V<sub>P</sub> 5.0 and the eye's cut V 8.18 C 1,660, D 2,970 and E 3,940
///   (R13.T1.b);
/// - its [`bright_beyond`](LayerCap::bright_beyond), the count brighter than V<sub>P</sub> beyond
///   R(u) (`at_ceiling`'s [`CapCount::stars_beyond`]): under one where the cap at V<sub>P</sub>
///   lies within the limit, and near the Sun some 172 C to E at V<sub>P</sub> 5.0, beyond 2,000 ly.
///
/// A, B and the brown dwarfs keep their caps at the cut, each stating its count brighter than
/// V<sub>P</sub> beyond them too.
///
/// A census to these caps lists C to E by the radius towards each star's band texel at every
/// reply and opens their cells by the rays' cones widened by the texel's largest radius, where its
/// query states the ceiling ([`SkyQueryBuilder::synthetic_ceiling`](super::census::SkyQueryBuilder::synthetic_ceiling);
/// [`census_plan_of`](super::census::census_plan_of)).
///
/// # Panics
///
/// If `caps_at_cut` lacks a layer of [`CAPPED_LAYERS`] or holds a cap of another layer, if C's,
/// D's or E's cap at the cut is not one radius a ray, or if the counts and the caps are not of
/// lattices of one ray count. Debug builds also check that C's, D's and E's caps are `at_cut`'s.
///
/// # Examples
///
/// The real tier near the Sun at RM3's ceiling of V 5.0 (`no_run`: the tables take a minute or
/// more to build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::caps::{CapCount, CapResolution, REAL_LIMIT_LY, real_boundary};
/// use hyperion_sim::sky::census::{SkyQuery, census_plan_of};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let mut cache = NoiseCache::with_capacity(1 << 16);
/// let (cut, ceiling) = (Magnitudes::new(7.95), Magnitudes::new(5.0));
/// let mut count = |v| {
///     CapCount::measure(&galaxy, &tables, &envelope, &observer, v, CapResolution::STANDARD, &mut cache)
/// };
/// let (at_cut, at_ceiling) = (count(cut), count(ceiling));
/// let caps = real_boundary(&at_ceiling, &at_cut, &at_cut.caps());
/// assert!(caps.iter().all(|cap| cap.radius().value() <= REAL_LIMIT_LY));
/// // E's stars brighter than V 5.0 beyond 2,000 ly, which the synthetic tier draws.
/// assert!(caps[4].bright_beyond() > Some(1.0));
/// // The census of the real tier: C to E listed by each star's texel, their cones widened.
/// let query = SkyQuery::builder(observer, cut).synthetic_ceiling(ceiling).build()?;
/// let plan = census_plan_of(&query, caps.clone());
/// assert_eq!(plan.caps(), caps.as_slice());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn real_boundary(
    at_ceiling: &CapCount,
    at_cut: &CapCount,
    caps_at_cut: &[LayerCap],
) -> Vec<LayerCap> {
    let rays = at_cut.lattice.directions.len();
    assert_eq!(
        at_ceiling.lattice.directions.len(),
        rays,
        "the counts at the ceiling and at the cut are over one lattice"
    );
    for layer in CAPPED_LAYERS {
        assert!(
            caps_at_cut.iter().any(|cap| cap.layer() == layer),
            "a cap at the cut for {layer:?}"
        );
    }
    for cap in caps_at_cut {
        assert!(
            CAPPED_LAYERS.contains(&cap.layer()),
            "a cap at the cut of {:?}, which no count holds",
            cap.layer()
        );
    }
    caps_at_cut
        .iter()
        .map(|cut| {
            let layer = cut.layer();
            if !REAL_BOUNDARY_LAYERS.contains(&layer) {
                return LayerCap {
                    bright_beyond: Some(at_ceiling.stars_beyond(cut)),
                    ..cut.clone()
                };
            }
            let low = cut
                .rays()
                .unwrap_or_else(|| panic!("{layer:?}'s cap at the cut is one radius a ray"));
            assert_eq!(
                low.radii_ly().len(),
                rays,
                "{layer:?}'s cap at the cut is of the counts' lattice"
            );
            debug_assert_eq!(
                cut,
                &at_cut.cap(layer),
                "{layer:?}'s cap is the cut's count's"
            );
            let high = at_ceiling.cap(layer);
            let held = high
                .rays()
                .expect("the caps' own are one radius a ray")
                .lesser(low)
                .within(REAL_LIMIT_LY);
            let mut real = LayerCap {
                layer,
                radius: held.largest(),
                rule_bound: cut.rule_bound(),
                expected_beyond: 0.0,
                bright_beyond: None,
                rays: Some(held),
            };
            real.expected_beyond = at_cut.stars_beyond(&real);
            real.bright_beyond = Some(at_ceiling.stars_beyond(&real));
            real
        })
        .collect()
}

/// The synthetic ceiling a server states for a request at `cut`, its constant being `ceiling`
/// (rendering plan R13, R13.T2.b; decided 2026-10-09, `decision-r13-t2b-ceiling.md`): the lesser
/// of the two, or none where a ceiling would hold nothing.
///
/// That is `ceiling` where the cut is deeper, and otherwise the cut itself, so that no served
/// sky's census of C, D and E reaches past [`REAL_LIMIT_LY`] at any cut: at a ceiling at the cut
/// a uniform cut's two counts are one, and the real boundary is the cut's caps held within the
/// limit, each layer's [`bright_beyond`](LayerCap::bright_beyond) its
/// [`expected_beyond`](LayerCap::expected_beyond). Where the cut is at or brighter than `ceiling`
/// and C's, D's and E's caps at the cut, `caps_at_cut` (one a layer of [`CAPPED_LAYERS`], as
/// [`real_boundary`] takes them), already lie within the limit on every ray, a ceiling would hold
/// no ray, and the request is served without one: its plan and its reply are R06's, exact, with
/// under one star a layer expected beyond its caps. A forced cap counts as its one radius.
///
/// # Panics
///
/// If the cut is at or brighter than `ceiling` and `caps_at_cut` lacks a cap of C, D or E.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::caps::{CAPPED_LAYERS, LayerCap, served_ceiling};
/// use hyperion_sim::units::{LightYears, Magnitudes};
///
/// let caps = |ly| CAPPED_LAYERS.map(|layer| LayerCap::forced(layer, LightYears::new(ly)));
/// let v = Magnitudes::new;
/// // The eye near the Sun, at V 7.95: RM3's ceiling of V 5.0.
/// assert_eq!(served_ceiling(v(7.95), v(5.0), &caps(9_000.0)), Some(v(5.0)));
/// // A shallow cut whose caps reach past 2,000 ly: the cut itself, so that the limit holds.
/// assert_eq!(served_ceiling(v(4.6), v(5.0), &caps(3_000.0)), Some(v(4.6)));
/// // One whose caps lie within the limit: no ceiling, R06's sky.
/// assert_eq!(served_ceiling(v(4.6), v(5.0), &caps(150.0)), None);
/// ```
#[must_use]
pub fn served_ceiling(
    cut: Magnitudes,
    ceiling: Magnitudes,
    caps_at_cut: &[LayerCap],
) -> Option<Magnitudes> {
    if cut.value() > ceiling.value() {
        return Some(ceiling);
    }
    let within = REAL_BOUNDARY_LAYERS.iter().all(|&layer| {
        let cap = caps_at_cut
            .iter()
            .find(|cap| cap.layer() == layer)
            .unwrap_or_else(|| panic!("a cap at the cut for {layer:?}"));
        cap.radius().value() <= REAL_LIMIT_LY
    });
    (!within).then_some(cut)
}

/// The caps of `query`'s census at the synthetic ceiling `ceiling`, as
/// [`census_plan`](super::census::census_plan) derives them (rendering plan R13, R13.T2.a): the
/// standard lattice's rays measured once, counted at the request's cut (by the eye's visibility
/// where the query asks it) and at the ceiling, and [`real_boundary`] of the two. Each count is
/// [`layer_caps`]' (or [`layer_caps_by_visibility`]'s) over the same rays, bit for bit, since a ray
/// is a function of its own direction and the cache changes no value.
///
/// # Panics
///
/// As [`layer_caps`].
#[must_use]
pub(crate) fn real_boundary_of(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    query: &super::census::SkyQuery,
    ceiling: Magnitudes,
    cache: &mut NoiseCache,
) -> Vec<LayerCap> {
    let observer = query.observer();
    let lattice = Arc::new(CapLattice::new(CAP_RAYS));
    let cuts = match query.eye_visibility() {
        Some(visibility) => visible_cuts_v(&lattice, visibility),
        None => vec![query.cut().value(); CAP_RAYS],
    };
    let rays = RayExtinctions::measure_clearest(
        galaxy,
        observer.position(),
        lattice,
        CapResolution::STANDARD.sub_rays,
        cache,
    );
    let count = |cuts_v| {
        CapCount::over(
            galaxy,
            tables,
            envelope,
            observer,
            &rays,
            cuts_v,
            RADIAL_STEPS_PER_DECADE,
        )
    };
    let at_cut = count(cuts);
    let at_ceiling = count(vec![ceiling.value(); CAP_RAYS]);
    real_boundary(&at_ceiling, &at_cut, &at_cut.caps())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::extinction::sightline;
    use crate::sky::testing::{milky_way_envelope, milky_way_tables, uniforms};
    use crate::time::UniverseTime;
    use crate::units::Degrees;

    fn observer_at(ly: [f64; 3]) -> Observer {
        Observer::new(
            GalacticPosition::from_light_years(ly).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("a valid observer")
    }

    /// The count at `ly` at cut 7.95 (the eye's near the Sun, 7.4 + 0.45 + 0.1) and `resolution`.
    fn count_at(ly: [f64; 3], resolution: CapResolution) -> CapCount {
        let (tables, envelope) = (milky_way_tables(), milky_way_envelope());
        let mut cache = NoiseCache::with_capacity(1 << 16);
        CapCount::measure(
            milky_way_galaxy(),
            tables,
            envelope,
            &observer_at(ly),
            Magnitudes::new(7.95),
            resolution,
            &mut cache,
        )
    }

    fn cap(caps: &[LayerCap], layer: Layer) -> &LayerCap {
        caps.iter().find(|c| c.layer() == layer).expect("capped")
    }

    /// The `q` quantile of a cap's rays' radii, ly.
    fn quantile(cap: &LayerCap, q: f64) -> f64 {
        let mut radii = cap.rays().expect("one radius a ray").radii_ly().to_vec();
        radii.sort_by(f64::total_cmp);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "an index into a few hundred radii"
        )]
        let at = ((radii.len() - 1) as f64 * q).round() as usize;
        radii[at]
    }

    fn print(name: &str, caps: &[LayerCap], spheres: &[LayerCap]) {
        for (cap, sphere) in caps.iter().zip(spheres) {
            eprintln!(
                "{name} {:?}: rays median {:.0}, p10 {:.0}, p90 {:.0}, largest {:.0} ly, rule {:.0} \
                 ly, beyond {:.3}; the sphere {:.0} ly, beyond {:.3}",
                cap.layer(),
                quantile(cap, 0.5),
                quantile(cap, 0.1),
                quantile(cap, 0.9),
                cap.radius().value(),
                cap.rule_bound().value(),
                cap.expected_beyond(),
                sphere.radius().value(),
                sphere.expected_beyond(),
            );
        }
    }

    /// The caps near the Sun and in the nuclear disc at the eye's cut, 7.4 + 0.45 + 0.1, against
    /// the gates of the 2026-10-03 decision, on each ray's radius (R06.T7.b): every ray within its
    /// rule bound and each layer's expected count beyond under one; near the Sun A and B under
    /// 100 ly and C at least 1,000 ly on every ray, and D's and E's median rays within a factor of
    /// three of the brainstorm's version-14 figures (a sanity bracket, not a confirmation); in the
    /// nuclear disc E under 1,500 ly on every ray, and each of C, D and E's median rays nearer than
    /// near the Sun.
    #[test]
    fn caps_near_the_sun_and_in_the_nuclear_disc() {
        let sun_count = count_at([0.0, 26_000.0, 68.0], CapResolution::STANDARD);
        let nuclear_count = count_at([0.0, 150.0, 0.0], CapResolution::STANDARD);
        let (sun, nuclear) = (sun_count.caps(), nuclear_count.caps());
        print("Sun", &sun, &sun_count.spheres());
        print("nuclear disc", &nuclear, &nuclear_count.spheres());
        for cap in sun.iter().chain(&nuclear) {
            let rays = cap.rays().expect("one radius a ray");
            assert!(
                rays.radii_ly()
                    .iter()
                    .all(|&r| r > 0.0 && r <= cap.rule_bound().value()),
                "{:?}",
                cap.layer()
            );
            assert!(cap.radius() <= cap.rule_bound(), "{:?}", cap.layer());
            assert!(
                (0.0..1.0).contains(&cap.expected_beyond()),
                "{:?}: {}",
                cap.layer(),
                cap.expected_beyond()
            );
        }
        assert!(cap(&sun, Layer::A).radius().value() < 100.0);
        assert!(cap(&sun, Layer::B).radius().value() < 100.0);
        // C's cap rests on rare bright phases of pair channels (R06.T5.d's count excess), which
        // plan 11's fixes move, so it keeps only its floor; the rule's bound, asserted above, is
        // its ceiling (decided 2026-10-04, "T5.d caps after the pair correction"), on every ray. D
        // and E keep the brainstorm's brackets on their median ray: their largest rays reach far
        // past them through the clear windows (E's to 61,341 ly towards the poles).
        let c = quantile(cap(&sun, Layer::C), 0.0);
        assert!(c >= 1_000.0, "C's least ray: {c} ly");
        for (layer, brainstorm) in [(Layer::D, 4_300.0), (Layer::E, 10_000.0)] {
            let r = quantile(cap(&sun, layer), 0.5);
            assert!(
                (brainstorm / 3.0..=brainstorm * 3.0).contains(&r),
                "{layer:?}: {r} ly against {brainstorm}"
            );
        }
        for layer in [Layer::C, Layer::D, Layer::E] {
            let (here, there) = (
                quantile(cap(&nuclear, layer), 0.5),
                quantile(cap(&sun, layer), 0.5),
            );
            assert!(
                here < there,
                "{layer:?} grows in the nuclear disc: {here} against {there}"
            );
        }
        // The nuclear disc's E, on every ray.
        let e = cap(&nuclear, Layer::E).radius().value();
        assert!(e < 1_500.0, "E's largest ray in the nuclear disc: {e} ly");
    }

    /// Near the Sun the caps by ray open at most 75% of the spheres' systems over every layer,
    /// and E's at most 35% of its sphere's (`decision-r06-census-cost.md`, the interim gate), each
    /// counted at 3,072 rays and twice the radial steps, the convergence test's count, towards
    /// whose rays the caps' radii are read. The spheres are R06.T7's: one radius a layer from
    /// 768 rays, each through its own profile. Prints, for the record, each layer's share, its
    /// expected count beyond both shapes, and the expected stars the spheres list that the rays
    /// drop, and those the rays gain.
    #[test]
    fn near_the_sun_the_rays_open_at_most_three_quarters_of_the_spheres_systems() {
        let sun = [0.0, 26_000.0, 68.0];
        let caps = count_at(sun, CapResolution::STANDARD).caps();
        let spheres = count_at(
            sun,
            CapResolution::new(768, RADIAL_STEPS_PER_DECADE).expect("non-zero"),
        )
        .spheres();
        let fine = count_at(
            sun,
            CapResolution::new(3_072, 2 * RADIAL_STEPS_PER_DECADE).expect("non-zero"),
        );
        let (mut opened, mut whole) = (0.0, 0.0);
        let mut e_share = 0.0;
        for (cap, sphere) in caps.iter().zip(&spheres) {
            let (rays, round) = (fine.systems_within(cap), fine.systems_within(sphere));
            opened += rays;
            whole += round;
            let share = if round > 0.0 { rays / round } else { 0.0 };
            if cap.layer() == Layer::E {
                e_share = share;
            }
            eprintln!(
                "Sun {:?}: systems by ray {rays:.4e} against the sphere's {round:.4e} ({:.1}%); \
                 beyond, by ray {:.3} and the sphere {:.3}; the sphere's stars the rays drop \
                 {:.3}, the rays' gain {:.3}",
                cap.layer(),
                100.0 * share,
                fine.stars_beyond(cap),
                fine.stars_beyond(sphere),
                fine.stars_within_and_beyond(sphere, cap),
                fine.stars_within_and_beyond(cap, sphere),
            );
        }
        let share = opened / whole;
        eprintln!(
            "Sun, every layer: {:.1}% of the spheres' systems",
            100.0 * share
        );
        assert!(share <= 0.75, "{share}");
        assert!(e_share <= 0.35, "E: {e_share}");
    }

    /// The counts towards a boundary no cap holds (R13.T1) are those of a cap that holds it, near
    /// the Sun at 7.95 on a coarse count (192 rays, 12 steps a decade): each layer's sphere held
    /// within its rays, as a closure, counts what the cap of its rays held within the sphere
    /// counts, bit for bit, stars and systems; and the stars beyond the rays less those beyond the
    /// sphere are those within the sphere and beyond the rays less the reverse, to rounding.
    #[test]
    fn counts_towards_a_boundary_are_those_of_the_cap_that_holds_it() {
        let count = count_at(
            [0.0, 26_000.0, 68.0],
            CapResolution::new(192, 12).expect("non-zero"),
        );
        let (caps, spheres) = (count.caps(), count.spheres());
        for (cap, sphere) in caps.iter().zip(&spheres) {
            let layer = cap.layer();
            let held = LayerCap::forced_by_ray(
                layer,
                cap.rays()
                    .expect("one radius a ray")
                    .within(sphere.radius().value()),
            );
            let toward_held =
                |u| LightYears::new(sphere.radius().value().min(cap.radius_toward(u).value()));
            assert_eq!(
                bits(count.stars_beyond_toward(layer, toward_held)),
                bits(count.stars_beyond(&held)),
                "{layer:?}"
            );
            assert_eq!(
                bits(count.systems_within_toward(layer, toward_held)),
                bits(count.systems_within(&held)),
                "{layer:?}"
            );
            let (drop, gain) = (
                count.stars_within_and_beyond(sphere, cap),
                count.stars_within_and_beyond(cap, sphere),
            );
            let (rays, round) = (count.stars_beyond(cap), count.stars_beyond(sphere));
            assert!(
                ((rays - round) - (drop - gain)).abs() <= 1e-9 * (rays + round + drop + gain),
                "{layer:?}: {rays} − {round} against {drop} − {gain}"
            );
        }
    }

    /// A boundary's radius may be zero, the whole ray.
    #[test]
    fn a_zero_boundary_is_taken() {
        assert_eq!(bits(boundary_ly(LightYears::ZERO)), bits(0.0));
    }

    /// A boundary's radius may not be negative or a NaN.
    #[test]
    #[should_panic(expected = "a boundary's radius is finite and not negative, not -1 ly")]
    fn a_negative_boundary_is_refused() {
        let _ = boundary_ly(LightYears::new(-1.0));
    }

    #[test]
    #[should_panic(expected = "a boundary's radius is finite and not negative, not NaN ly")]
    fn a_nan_boundary_is_refused() {
        let _ = boundary_ly(LightYears::new(f64::NAN));
    }

    /// A cap with a budget of stars beyond (R13.T1.b), near the Sun at 7.95 on a coarse count (192
    /// rays, 12 steps a decade): a budget of one is [`CapCount::cap`], bit for bit, in every layer;
    /// each larger budget of 3, 10 and 30 holds every ray at or within the smaller's, states under
    /// its budget beyond, and opens fewer of E's systems than a budget of one.
    #[test]
    fn a_larger_budget_holds_every_ray_within_the_caps() {
        let count = count_at(
            [0.0, 26_000.0, 68.0],
            CapResolution::new(192, 12).expect("non-zero"),
        );
        let radii = |cap: &LayerCap| cap.rays().expect("one radius a ray").radii_ly().to_vec();
        let radii_bits = |cap: &LayerCap| radii(cap).into_iter().map(bits).collect::<Vec<_>>();
        for layer in CAPPED_LAYERS {
            let (own, one) = (count.cap(layer), count.cap_with_budget(layer, 1.0));
            assert_eq!(radii_bits(&one), radii_bits(&own), "{layer:?}");
            assert_eq!(bits(one.expected_beyond()), bits(own.expected_beyond()));
            assert_eq!(
                bits(one.rule_bound().value()),
                bits(own.rule_bound().value())
            );
            let mut nearer = own;
            for budget in [3.0, 10.0, 30.0] {
                let cap = count.cap_with_budget(layer, budget);
                let held = radii(&cap).iter().zip(radii(&nearer)).all(|(r, n)| r <= &n);
                assert!(held, "{layer:?} at a budget of {budget}");
                assert!(
                    (0.0..budget).contains(&cap.expected_beyond()),
                    "{layer:?}: {} beyond at a budget of {budget}",
                    cap.expected_beyond()
                );
                assert_eq!(bits(cap.expected_beyond()), bits(count.stars_beyond(&cap)));
                nearer = cap;
            }
        }
        let e = |budget| count.systems_within(&count.cap_with_budget(Layer::E, budget));
        assert!(e(30.0) < e(1.0), "E: {} against {}", e(30.0), e(1.0));
    }

    /// A budget of stars beyond must be finite and positive.
    #[test]
    #[should_panic(expected = "a budget of stars beyond is finite and positive, not 0")]
    fn a_budget_of_none_is_refused() {
        let (plan, rays) = small_count_plan();
        let tables = crate::sky::testing::milky_way_dark_tables();
        let count = plan.join([plan.count_rays(milky_way_galaxy(), tables, &rays, 0..8)]);
        let _ = count.cap_with_budget(Layer::E, 0.0);
    }

    #[test]
    #[should_panic(expected = "a budget of stars beyond is finite and positive, not NaN")]
    fn a_nan_budget_is_refused() {
        let (plan, rays) = small_count_plan();
        let tables = crate::sky::testing::milky_way_dark_tables();
        let count = plan.join([plan.count_rays(milky_way_galaxy(), tables, &rays, 0..8)]);
        let _ = count.cap_with_budget(Layer::E, f64::NAN);
    }

    /// A context near the Sun for the eye's pre-pass and the band: the fixture's tables.
    fn sky_context() -> crate::sky::census::SkyContext<'static> {
        crate::sky::census::SkyContext {
            tables: milky_way_tables(),
            envelope: milky_way_envelope(),
            offsets: crate::sky::testing::milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &crate::sky::census::NoSkyCellCache,
            sources: &[],
            modifiers: &crate::galaxy::gas::modifiers::NoModifiers,
        }
    }

    /// The caps by the eye's visibility of a uniform limit at the cut are [`layer_caps`]' at the
    /// cut, bit for bit: the count is the same count.
    #[test]
    fn caps_by_a_uniform_visibility_are_the_caps_at_its_cut() {
        let (tables, envelope) = (milky_way_tables(), milky_way_envelope());
        let observer = observer_at([0.0, 150.0, 0.0]);
        let cut = Magnitudes::new(7.95);
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let caps = layer_caps(
            milky_way_galaxy(),
            tables,
            envelope,
            &observer,
            cut,
            &mut cache,
        );
        let seen = layer_caps_by_visibility(
            milky_way_galaxy(),
            tables,
            envelope,
            &observer,
            &EyeVisibility::uniform(observer, crate::sky::EyeObserver::default(), cut),
            &mut cache,
        );
        assert_eq!(caps, seen);
    }

    /// The safety test's least margin (R06.T7.b): over every texel of the final 64² limit map near
    /// `observer` at `cut`, with no glare, and every ray of `lattice` whose cone meets it, the ray's
    /// cut of `cuts` less the texel's limit and the largest colour offset; with the pairs taken.
    /// The final map's query states the request's `illumination`, as the server's does (R06.T7.c;
    /// R06.T11.d), so its band holds the diffuse galactic light beside the starlight.
    fn least_margin(
        observer: &Observer,
        cut: Magnitudes,
        lattice: &CapLattice,
        cuts: &[f64],
        illumination: Arc<crate::sky::dgl::Illumination>,
        ctx: &mut crate::sky::census::SkyContext<'_>,
    ) -> (u64, f64) {
        use crate::sky::band::{CompleteTo, band_rows};
        use crate::sky::census::{SkyCensus, SkyQuery};
        use crate::sky::eye::EyeObserver;
        use crate::sky::limits::{Glare, largest_colour_offset, limit_map};
        let spec = BandSpec::STANDARD;
        let query = SkyQuery::builder(*observer, cut)
            .illumination(illumination)
            .build()
            .expect("a query");
        let mut band = Vec::new();
        for face in CubeFace::ALL {
            band_rows(
                milky_way_galaxy(),
                ctx,
                &query,
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                &spec,
                face,
                0..spec.face_texels(),
                &mut band,
            );
        }
        limit_map(&EyeObserver::default(), &spec, &Glare::default(), &mut band);
        let offset = largest_colour_offset().value();
        let reach = lattice.spacing().value() + spec.largest_texel_radius().value();
        let (mut pairs, mut least) = (0_u64, f64::INFINITY);
        let side = spec.face_texels();
        let mut texels = band.iter();
        for face in CubeFace::ALL {
            for row in 0..side {
                for column in 0..side {
                    let centre = spec.texel_direction(face, row, column);
                    let texel = texels.next().expect("a texel of the face");
                    let limit = texel.eye_limit().expect("the map sets every limit").value();
                    for (ray, &ray_cut) in lattice.directions().iter().zip(cuts) {
                        if math::acos(ray.dot(&centre).clamp(-1.0, 1.0)) <= reach {
                            pairs += 1;
                            // A NaN margin is kept, so that it fails the test.
                            let margin = ray_cut - (limit + offset);
                            if margin.is_nan() || margin < least {
                                least = margin;
                            }
                        }
                    }
                }
            }
        }
        (pairs, least)
    }

    /// The caps by the eye's visibility near the Sun (R06.T7.b; adopted under
    /// `decision-r06-census-cost-signoff.md`, alternative 1), at the eye's cut as R06.T9.d computes
    /// it: each ray's cut at most the eye's; every ray within its rule bound and each layer's
    /// expected count beyond under one; the safety test, that no texel of the final 64² limit map
    /// with no glare, plus the largest colour offset, is deeper than the cut of any ray whose cone
    /// meets it; and they open at least a fifth fewer systems than [`layer_caps`]' at the eye's
    /// cut, counted at 3,072 rays and twice the radial steps. The eye's cut, its visibility and the
    /// final map's query all state the request's illumination ([`Illumination::march`] at the
    /// observer), as the server's do (R06.T7.c; `decision-r06-t11d-first-sky.md` §4). Prints the
    /// records.
    ///
    /// [`Illumination::march`]: crate::sky::dgl::Illumination::march
    #[test]
    fn the_eyes_visibility_caps_count_what_it_sees_and_open_a_fifth_fewer_systems() {
        use crate::sky::eye::EyeObserver;
        use crate::sky::limits::{eye_cut, eye_visibility};
        let galaxy = milky_way_galaxy();
        let (tables, envelope) = (milky_way_tables(), milky_way_envelope());
        let observer = observer_at([0.0, 26_000.0, 68.0]);
        let eye = EyeObserver::default();
        let mut ctx = sky_context();
        // The fixture's `Illumination::march` at this observer, built once a process.
        let light = crate::sky::testing::sun_illumination();
        let cut = eye_cut(galaxy, &mut ctx, &observer, &eye, Some(light));
        let visibility = eye_visibility(galaxy, &mut ctx, &observer, &eye, cut, Some(light));
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let seen =
            layer_caps_by_visibility(galaxy, tables, envelope, &observer, &visibility, &mut cache);
        let uniform = layer_caps(galaxy, tables, envelope, &observer, cut, &mut cache);
        let lattice = seen[0].rays().expect("one radius a ray").lattice();
        let cuts = visible_cuts_v(lattice, &visibility);
        assert!(cuts.iter().all(|&c| c <= cut.value()));
        for cap in &seen {
            let rays = cap.rays().expect("one radius a ray");
            assert!(
                rays.radii_ly()
                    .iter()
                    .all(|&r| r <= cap.rule_bound().value())
            );
            assert!(
                (0.0..1.0).contains(&cap.expected_beyond()),
                "{:?}",
                cap.layer()
            );
        }
        let (pairs, least) =
            least_margin(&observer, cut, lattice, &cuts, Arc::clone(light), &mut ctx);
        let fine_resolution =
            CapResolution::new(3_072, 2 * RADIAL_STEPS_PER_DECADE).expect("non-zero");
        let fine = CapCount::measure(
            galaxy,
            tables,
            envelope,
            &observer,
            cut,
            fine_resolution,
            &mut cache,
        );
        let beyond = expected_beyond_caps_by_visibility(
            galaxy,
            tables,
            envelope,
            &observer,
            &visibility,
            fine_resolution,
            &seen,
            &mut cache,
        );
        let (mut ours, mut theirs) = (0.0, 0.0);
        for ((s, u), b) in seen.iter().zip(&uniform).zip(&beyond) {
            let (a, c) = (fine.systems_within(s), fine.systems_within(u));
            ours += a;
            theirs += c;
            eprintln!(
                "Sun at the eye's cut {:.3} {:?}: by visibility rays median {:.0} ly and {a:.4e} \
                 systems, by the uniform cut median {:.0} ly and {c:.4e}; beyond by visibility \
                 {:.3}, recounted at 3,072 rays {b:.3}",
                cut.value(),
                s.layer(),
                quantile(s, 0.5),
                quantile(u, 0.5),
                s.expected_beyond(),
            );
        }
        let mut sorted = cuts.clone();
        sorted.sort_by(f64::total_cmp);
        eprintln!(
            "rays' cuts {:.3}–{:.3} (median {:.3}); {pairs} texel–ray pairs, least margin {least:.4} \
             mag; by visibility {:.1}% of the uniform cut's systems",
            sorted[0],
            sorted[sorted.len() - 1],
            sorted[sorted.len() / 2],
            100.0 * ours / theirs
        );
        assert!(
            least >= 0.0,
            "a texel deeper than its rays' cut by {}",
            -least
        );
        assert!(beyond.iter().all(|&b| b < 1.5), "{beyond:?}");
        assert!(ours <= 0.8 * theirs, "{ours} against {theirs}");
    }

    /// `caps_converge_in_rays`' six points, ly: near the Sun, the nuclear disc, the solar circle a
    /// quarter turn round and on the far side, the inner disc, and 2,000 ly above the Sun. A copy
    /// of `tests/common/sky.rs`' `CAPS_POINTS`, which the integration tests share: change both
    /// together.
    const CONVERGENCE_POINTS: [[f64; 3]; 6] = [
        [0.0, 26_000.0, 68.0],
        [0.0, 150.0, 0.0],
        [26_000.0, 0.0, 68.0],
        [-18_385.0, -18_385.0, 68.0],
        [0.0, 8_000.0, 0.0],
        [0.0, 26_000.0, 2_000.0],
    ];

    /// The caps by the eye's visibility are safe with the request's illumination at
    /// `caps_converge_in_rays`' six points (R06.T7.c; `decision-r06-t11d-first-sky.md` §4).
    ///
    /// At each point the illumination, [`Illumination::march`] at the observer, is stated on the
    /// eye's cut, its visibility and the final 64² map's query, as the server states it
    /// (R06.T11.d). At every point each ray's cut is at most the eye's, and no texel of the final
    /// map with no glare, plus the largest colour offset, is deeper than the cut of any ray whose
    /// cone meets it. The rays are the standard lattice's, which [`layer_caps_by_visibility`]
    /// counts. Prints each point's least margin, beside T7.b's +0.094 mag near the Sun without the
    /// illumination. A negative margin widens [`visible_cuts_v`]'s reach, never this test. Slow:
    /// six illuminations, eye cuts and final maps.
    ///
    /// [`Illumination::march`]: crate::sky::dgl::Illumination::march
    #[test]
    #[ignore = "slow: marches the illumination, the eye's cut and the final 64² map at six points"]
    fn the_eyes_visibility_caps_are_safe_with_the_illumination_at_six_points() {
        use crate::sky::dgl::Illumination;
        use crate::sky::eye::EyeObserver;
        use crate::sky::limits::{eye_cut, eye_visibility};
        let galaxy = milky_way_galaxy();
        let eye = EyeObserver::default();
        let lattice = CapLattice::new(CAP_RAYS);
        let mut failures = Vec::new();
        for point in CONVERGENCE_POINTS {
            let observer = observer_at(point);
            let mut ctx = sky_context();
            let light = Arc::new(Illumination::march(galaxy, &mut ctx, &observer));
            let cut = eye_cut(galaxy, &mut ctx, &observer, &eye, Some(&light));
            let visibility = eye_visibility(galaxy, &mut ctx, &observer, &eye, cut, Some(&light));
            let cuts = visible_cuts_v(&lattice, &visibility);
            let (pairs, least) = least_margin(&observer, cut, &lattice, &cuts, light, &mut ctx);
            let mut sorted = cuts;
            sorted.sort_by(f64::total_cmp);
            eprintln!(
                "{point:?} with the illumination: the eye's cut {:.3}, rays' cuts {:.3}–{:.3} \
                 (median {:.3}); {pairs} texel–ray pairs, least margin {least:+.4} mag",
                cut.value(),
                sorted[0],
                sorted[sorted.len() - 1],
                sorted[sorted.len() / 2],
            );
            if pairs == 0 {
                failures.push(format!("{point:?}: no texel–ray pair compared"));
            }
            if sorted[sorted.len() - 1] > cut.value() {
                failures.push(format!("{point:?}: a ray's cut deeper than the eye's"));
            }
            if least.is_nan() || least < 0.0 {
                failures.push(format!(
                    "{point:?}: a texel deeper than its rays' cut by {}",
                    -least
                ));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// Rays measured in shares, each with a cache of its own, and joined in order are one
    /// measure's, and the caps over them are `layer_caps`' bit for bit: the server's ray-chunk
    /// jobs (R06.T11.c; `decision-r06-tables.md`, item B.4), each ray the clearest of its sub-rays
    /// (R06.T7.b).
    #[test]
    fn caps_over_rays_measured_in_shares_are_layer_caps() {
        let galaxy = milky_way_galaxy();
        let observer = observer_at([0.0, 26_000.0, 68.0]);
        let origin = observer.position();
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let shares = [0..1, 1..100, 100..100, 100..101, 101..CAP_RAYS];
        let joined = RayExtinctions::join(shares.into_iter().map(|which| {
            let mut own = NoiseCache::with_capacity(1 << 10);
            RayExtinctions::measure_clearest_rays(
                galaxy,
                origin,
                Arc::clone(&lattice),
                SUB_RAYS,
                which,
                &mut own,
            )
        }));
        let mut cache = NoiseCache::with_capacity(1 << 16);
        assert_eq!(
            joined,
            RayExtinctions::measure_clearest(
                galaxy,
                origin,
                Arc::clone(&lattice),
                SUB_RAYS,
                &mut cache
            )
        );
        let cut = Magnitudes::new(7.95);
        let (tables, envelope) = (milky_way_tables(), milky_way_envelope());
        assert_eq!(
            layer_caps_over(galaxy, tables, envelope, &observer, cut, &joined),
            layer_caps(galaxy, tables, envelope, &observer, cut, &mut cache)
        );
        let visibility = EyeVisibility::uniform(observer, crate::sky::EyeObserver::default(), cut);
        let seen =
            layer_caps_by_visibility(galaxy, tables, envelope, &observer, &visibility, &mut cache);
        assert_eq!(
            layer_caps_by_visibility_over(
                galaxy,
                tables,
                envelope,
                &observer,
                &visibility,
                &joined
            ),
            seen
        );
        // The count in parts of rays, out of order and of uneven sizes, joined, with its caps
        // drawn a layer at a time, is the count of one job bit for bit, at a uniform cut and by
        // the eye's visibility: the server's count jobs and cap jobs (R06.T11.g).
        let whole = layer_caps(galaxy, tables, envelope, &observer, cut, &mut cache);
        let in_parts = |plan: CapCountPlan| -> Vec<LayerCap> {
            assert_eq!(plan.rays(), CAP_RAYS);
            let parts = [1_000..CAP_RAYS, 0..1, 1..1, 1..1_000]
                .map(|which| plan.count_rays(galaxy, tables, &joined, which));
            assert_eq!(parts[0].rays(), 1_000..CAP_RAYS);
            let count = plan.join(parts);
            let by_layer: Vec<LayerCap> = CAPPED_LAYERS
                .iter()
                .map(|&layer| count.cap(layer))
                .collect();
            assert_eq!(by_layer, count.caps());
            by_layer
        };
        let planned = CapCount::plan_over(galaxy, tables, envelope, &observer, cut, &joined);
        assert_eq!(in_parts(planned), whole);
        let planned = CapCount::plan_by_visibility_over(
            galaxy,
            tables,
            envelope,
            &observer,
            &visibility,
            &joined,
        );
        assert_eq!(in_parts(planned), seen);
        // One profile a ray, in shares, is `measure`'s too.
        let plain = RayExtinctions::join([0..700, 700..CAP_RAYS].map(|which| {
            let mut own = NoiseCache::with_capacity(1 << 10);
            RayExtinctions::measure_rays(galaxy, origin, CAP_RAYS, which, &mut own)
        }));
        assert_eq!(
            plain,
            RayExtinctions::measure(galaxy, origin, CAP_RAYS, &mut cache)
        );
    }

    #[test]
    #[should_panic(expected = "the shares of one lattice from one point, in order")]
    fn shares_out_of_order_are_refused() {
        let galaxy = milky_way_galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let mut cache = NoiseCache::with_capacity(1 << 10);
        let mut share = |which| RayExtinctions::measure_rays(galaxy, &sun, 8, which, &mut cache);
        let (first, third) = (share(0..2), share(4..8));
        let _ = RayExtinctions::join([first, third]);
    }

    /// The plan of a count over a lattice of eight rays near the Sun, over tables of no star, and
    /// the rays it was planned over.
    fn small_count_plan() -> (CapCountPlan, RayExtinctions) {
        let mut cache = NoiseCache::with_capacity(1 << 10);
        let lattice = Arc::new(CapLattice::new(8));
        let rays = RayExtinctions::measure_clearest(
            milky_way_galaxy(),
            observer_at([0.0, 26_000.0, 68.0]).position(),
            lattice,
            SUB_RAYS,
            &mut cache,
        );
        (small_count_plan_over(&rays, 7.95), rays)
    }

    /// The plan of a count over `rays`, [`small_count_plan`]'s, each ray at the cut `cut_v`.
    fn small_count_plan_over(rays: &RayExtinctions, cut_v: f64) -> CapCountPlan {
        CapCountPlan::new(
            milky_way_galaxy(),
            crate::sky::testing::milky_way_dark_tables(),
            milky_way_envelope(),
            &observer_at([0.0, 26_000.0, 68.0]),
            rays,
            vec![cut_v; 8],
            RADIAL_STEPS_PER_DECADE,
        )
    }

    #[test]
    #[should_panic(expected = "ray 3 of the count is counted twice")]
    fn a_count_whose_parts_overlap_is_refused() {
        let (plan, rays) = small_count_plan();
        let tables = crate::sky::testing::milky_way_dark_tables();
        let parts =
            [0..4, 3..8].map(|which| plan.count_rays(milky_way_galaxy(), tables, &rays, which));
        let _ = plan.join(parts);
    }

    #[test]
    #[should_panic(expected = "ray 4 of the count is not counted")]
    fn a_count_missing_a_ray_is_refused() {
        let (plan, rays) = small_count_plan();
        let tables = crate::sky::testing::milky_way_dark_tables();
        let parts =
            [0..4, 5..8].map(|which| plan.count_rays(milky_way_galaxy(), tables, &rays, which));
        let _ = plan.join(parts);
    }

    #[test]
    #[should_panic(expected = "a count joins the parts of its own plan")]
    fn a_count_joins_only_its_own_plans_parts() {
        let (plan, rays) = small_count_plan();
        let (other, _) = small_count_plan();
        let tables = crate::sky::testing::milky_way_dark_tables();
        let part = other.count_rays(milky_way_galaxy(), tables, &rays, 0..8);
        let _ = plan.join([part]);
    }

    /// Two counts planned over one set of rays share its lattice, as a uniform cut's and the eye's
    /// visibility's do, but neither joins the other's parts.
    #[test]
    #[should_panic(expected = "a count joins the parts of its own plan")]
    fn a_count_refuses_the_parts_of_another_plan_over_its_rays() {
        let (plan, rays) = small_count_plan();
        let other = small_count_plan_over(&rays, 6.0);
        let tables = crate::sky::testing::milky_way_dark_tables();
        let part = other.count_rays(milky_way_galaxy(), tables, &rays, 0..8);
        let _ = plan.join([part]);
    }

    #[test]
    #[should_panic(expected = "the standard caps count the standard lattice's rays")]
    fn caps_over_rays_of_one_profile_are_refused() {
        let galaxy = milky_way_galaxy();
        let observer = observer_at([0.0, 26_000.0, 68.0]);
        let mut cache = NoiseCache::with_capacity(1 << 10);
        let rays = RayExtinctions::measure(galaxy, observer.position(), CAP_RAYS, &mut cache);
        let _ = layer_caps_over(
            galaxy,
            milky_way_tables(),
            milky_way_envelope(),
            &observer,
            Magnitudes::new(7.95),
            &rays,
        );
    }

    #[test]
    fn a_rays_last_node_is_its_sightline() {
        let galaxy = milky_way_galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let mut cache = NoiseCache::with_capacity(1 << 14);
        let rays = RayExtinctions::measure(galaxy, &sun, 8, &mut cache);
        for (ray, &direction) in rays.directions().enumerate() {
            let end = sun
                .translated(crate::coords::GalacticDisplacement::new(
                    direction
                        .components()
                        .map(|c| c * FARTHEST_LY * crate::units::consts::METRES_PER_LIGHT_YEAR),
                ))
                .expect("in the cube");
            let line = sightline(
                galaxy.gas(),
                &sun,
                &end,
                NoiseMode::Realised,
                Quality::Full,
                &[],
                &mut cache,
            );
            let (a, b) = (rays.along(ray, FARTHEST_LY), line.a_v().value());
            assert!(((a - b) / b).abs() < 1e-12, "ray {ray}: {a} against {b}");
            assert!(rays.along(ray, 0.0).abs() < 1e-300);
        }
    }

    /// The count's faintest magnitude is the band's boundary, cut − DM − v☉(A<sub>V</sub>)
    /// A<sub>V</sub> (R06.T8.k; decided 2026-10-07, `decision-r06-t8k-cone.md`, item 3): the solar
    /// row's V secant, 1.0043 as A<sub>V</sub> → 0 and 0.977 at 10, not the bare A<sub>V</sub>.
    #[test]
    fn the_count_cuts_at_the_bands_boundary() {
        let dust = solar_colour().reddening();
        for a in [0.0, 0.5, 2.0, 5.0, 10.0, 25.0] {
            let ours = faintest_counted(7.95, 1_000.0, a, &dust);
            let bands = 7.95
                - distance_modulus(1_000.0)
                - dust.through(Magnitudes::new(a)).v_extinction().value();
            assert_eq!(bits(ours), bits(bands), "A_V {a}");
            if a > 0.0 {
                // The solar row's V secant: 1.0043 as A_V → 0, 0.977 at 10, the reddened tail's
                // flatter curve beyond (0.933 at 25).
                let secant = (7.95 - distance_modulus(1_000.0) - ours) / a;
                assert!((0.9..1.01).contains(&secant), "A_V {a}: {secant}");
            }
        }
        let at_ten =
            (7.95 - distance_modulus(1_000.0) - faintest_counted(7.95, 1_000.0, 10.0, &dust))
                / 10.0;
        assert!((at_ten - 0.977).abs() < 0.002, "{at_ten}");
    }

    /// Every direction lies within the lattice's spacing of a ray, at 768, 1,536 and 3,072 rays, on
    /// 10⁵ random directions, each nearest ray found among them all; the bound is no more than
    /// the grid's slack and the sample's above the farthest found, and wider than a ray's own cap
    /// of equal area, which leaves gaps.
    #[test]
    fn every_direction_lies_within_the_spacing_of_a_ray() {
        for rays in [768, CAP_RAYS, 3_072] {
            let lattice = CapLattice::new(rays);
            let mut uniforms = uniforms(0x7b_0001 + u64::try_from(rays).expect("few rays"));
            let mut farthest = 0.0_f64;
            for _ in 0..100_000 {
                let z = 2.0 * uniforms.next().expect("endless") - 1.0;
                let phi = 2.0 * core::f64::consts::PI * uniforms.next().expect("endless");
                let rho = (1.0 - z * z).sqrt();
                let u =
                    UnitVector::from_components([rho * math::cos(phi), rho * math::sin(phi), z])
                        .expect("a unit vector");
                let best = lattice
                    .directions()
                    .iter()
                    .map(|d| d.dot(&u))
                    .fold(-1.0, f64::max);
                farthest = farthest.max(math::acos(best.min(1.0)));
            }
            let spacing = lattice.spacing().value();
            #[expect(clippy::cast_precision_loss, reason = "a few thousand rays")]
            let own_cap = math::acos(1.0 - 2.0 / rays as f64);
            eprintln!(
                "{rays} rays: spacing {:.3}°, the farthest of 10⁵ directions from its nearest ray \
                 {:.3}°, a ray's own cap {:.3}°",
                Degrees::from(Radians::new(spacing)).value(),
                Degrees::from(Radians::new(farthest)).value(),
                Degrees::from(Radians::new(own_cap)).value(),
            );
            assert!(farthest <= spacing, "{farthest} against {spacing}");
            assert!(
                spacing - farthest < 0.6 * RADIANS_PER_DEGREE,
                "{spacing} against {farthest}"
            );
            assert!(farthest > own_cap, "a ray's own cap leaves gaps");
        }
    }

    /// The standard lattice's spacing, bit for bit: it sets every ray's cone, the widening and the
    /// sub-rays, so every cap, and no golden pins the caps before R06.T17. It is measured on the
    /// band's cube-map geometry ([`BandSpec::texel_direction`], [`BandSpec::largest_texel_radius`]),
    /// whose change would move it.
    #[test]
    fn the_standard_lattices_spacing_is_pinned() {
        let spacing = CapLattice::new(CAP_RAYS).spacing().value();
        assert_eq!(bits(spacing), STANDARD_SPACING_BITS, "{spacing:e} rad");
    }

    /// [`the_standard_lattices_spacing_is_pinned`]'s bits: 4.1999° (0.07330 rad).
    const STANDARD_SPACING_BITS: u64 = 0x3fb2_c3e4_1492_081b;

    /// Each ray of the clearest of its sub-rays holds, at every node, the least of its sub-rays'
    /// own profiles there (`Realised`, `Full`), each marched alone, bit for bit; between the nodes
    /// it is at most each of them.
    #[test]
    fn a_clearest_ray_is_the_least_of_its_sub_rays_profiles() {
        let galaxy = milky_way_galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let lattice = Arc::new(CapLattice::new(12));
        let mut cache = NoiseCache::with_capacity(1 << 14);
        let rays = RayExtinctions::measure_clearest(
            galaxy,
            &sun,
            Arc::clone(&lattice),
            SUB_RAYS,
            &mut cache,
        );
        let ring = SUB_RAY_RING_SPACINGS * lattice.spacing().value();
        let nodes = profile_nodes();
        let distances: Vec<LightYears> = nodes.iter().map(|&d| LightYears::new(d)).collect();
        let mut out = Vec::new();
        for (ray, &direction) in lattice.directions().iter().enumerate() {
            let profiles: Vec<Vec<f64>> = sub_ray_directions(direction, ring, SUB_RAYS)
                .map(|sub| {
                    profile(
                        galaxy.gas(),
                        &sun,
                        sub,
                        &distances,
                        NoiseMode::Realised,
                        Quality::Full,
                        &[],
                        &mut cache,
                        &mut out,
                    );
                    out.iter().map(|a| a.value()).collect()
                })
                .collect();
            assert_eq!(profiles.len(), SUB_RAYS);
            for (k, &d) in nodes.iter().enumerate() {
                let least = profiles.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
                assert_eq!(bits(rays.along(ray, d)), bits(least), "ray {ray} at {d} ly");
            }
            for pair in nodes.windows(2) {
                let d = f64::midpoint(pair[0], pair[1]);
                let t = 0.5;
                for p in &profiles {
                    let k = nodes.partition_point(|&node| node <= d) - 1;
                    let own = p[k] + (p[k + 1] - p[k]) * t;
                    assert!(rays.along(ray, d) <= own + 1e-15, "ray {ray} at {d} ly");
                }
            }
        }
    }

    /// Random radii a ray of the standard lattice, from 10 to 10,000 ly, even in ln r.
    fn random_radii(seed: u64) -> RayRadii {
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let radii = uniforms(seed)
            .take(CAP_RAYS)
            .map(|u| 10.0 * math::exp10(3.0 * u))
            .collect();
        RayRadii::new(lattice, radii)
    }

    /// The radius towards a direction is the largest of the rays' whose cones hold it, as a scan of
    /// every ray finds it, bit for bit, at 10⁴ random directions.
    #[test]
    fn the_radius_toward_a_direction_is_the_largest_of_the_cones_holding_it() {
        let rays = random_radii(0x7b_0002);
        let lattice = rays.lattice();
        let cos = math::cos(lattice.spacing().value());
        let mut uniforms = uniforms(0x7b_0003);
        for _ in 0..10_000 {
            let u = UnitVector::from_components(std::array::from_fn(|_| {
                2.0 * uniforms.next().expect("endless") - 1.0
            }))
            .expect("a direction");
            let scanned = lattice
                .directions()
                .iter()
                .zip(rays.radii_ly())
                .filter(|(d, _)| d.dot(&u) >= cos)
                .map(|(_, &r)| r)
                .fold(0.0, f64::max);
            assert!(scanned > 0.0, "{u:?} lies in no cone");
            assert_eq!(bits(rays.toward(u).value()), bits(scanned), "{u:?}");
        }
    }

    /// The lesser of two radii a ray (R13.T1) is each ray's least, bit for bit, on the same
    /// lattice, and towards 10⁴ random directions its radius is the largest of the cones' least
    /// radii, as a scan of every ray finds it, never beyond either's.
    #[test]
    fn the_lesser_of_two_radii_a_ray_is_each_rays_least() {
        let a = random_radii(0x7b_0007);
        let b = RayRadii::new(
            Arc::clone(&a.lattice),
            uniforms(0x7b_0008)
                .take(CAP_RAYS)
                .map(|u| 10.0 * math::exp10(3.0 * u))
                .collect(),
        );
        let least = a.lesser(&b);
        for ((&l, &x), &y) in least.radii_ly().iter().zip(a.radii_ly()).zip(b.radii_ly()) {
            assert_eq!(bits(l), bits(x.min(y)));
        }
        assert_eq!(least, b.lesser(&a));
        let lattice = a.lattice();
        let cos = math::cos(lattice.spacing().value());
        let mut uniforms = uniforms(0x7b_0009);
        for _ in 0..10_000 {
            let u = UnitVector::from_components(std::array::from_fn(|_| {
                2.0 * uniforms.next().expect("endless") - 1.0
            }))
            .expect("a direction");
            let scanned = lattice
                .directions()
                .iter()
                .zip(a.radii_ly().iter().zip(b.radii_ly()))
                .filter(|(d, _)| d.dot(&u) >= cos)
                .map(|(_, (&x, &y))| x.min(y))
                .fold(0.0, f64::max);
            let toward = least.toward(u).value();
            assert_eq!(bits(toward), bits(scanned), "{u:?}");
            assert!(toward <= a.toward(u).value().min(b.toward(u).value()));
        }
    }

    #[test]
    #[should_panic(expected = "the lesser of two radii a ray is taken on one lattice")]
    fn the_lesser_of_radii_on_two_lattices_is_refused() {
        let few = RayRadii::new(Arc::new(CapLattice::new(12)), vec![100.0; 12]);
        let _ = random_radii(0x7b_000a).lesser(&few);
    }

    /// A ball meets the census's region exactly when, by a scan of every ray in angles, it meets a
    /// ray's cone within the ray's radius, at 10⁵ random balls near and far, wide and narrow: the
    /// test never misses one, and passes only those within 10⁻⁷ rad of one. So it is for the cones
    /// widened by the 64² band texel's largest radius ρ (rendering plan R13, fix (i)), and by 5°
    /// and 7°, which leave the lattice's index for wider balls and for every ball.
    #[test]
    fn a_ball_meets_the_region_where_it_meets_a_cone_within_its_rays_radius() {
        let rays = random_radii(0x7b_0004);
        let lattice = rays.lattice();
        let rho = BandSpec::STANDARD.largest_texel_radius().value();
        for by in [0.0, rho, 5.0 * RADIANS_PER_DEGREE, 7.0 * RADIANS_PER_DEGREE] {
            let cones = rays.cones_widened_by(Radians::new(by));
            let spacing = lattice.spacing().value() + by;
            let scan = |offset: [f64; 3], radius: f64, margin: f64| {
                let d = math::hypot(math::hypot(offset[0], offset[1]), offset[2]);
                if d <= radius {
                    return true;
                }
                let u = offset.map(|c| c / d);
                let opening = spacing + math::asin(radius / d) + margin;
                lattice
                    .directions()
                    .iter()
                    .zip(rays.radii_ly())
                    .any(|(k, &r)| {
                        r > d - radius
                            && math::acos(dot(k.components(), u).clamp(-1.0, 1.0)) <= opening
                    })
            };
            let mut uniforms = uniforms(0x7b_0005);
            let (mut met, mut missed) = (0_u32, 0_u32);
            for _ in 0..100_000 {
                let mut next = || uniforms.next().expect("endless");
                let distance = 10.0 * math::exp10(3.3 * next());
                let u = UnitVector::from_components(std::array::from_fn(|_| 2.0 * next() - 1.0))
                    .expect("a direction");
                let offset = u.components().map(|c| c * distance);
                let radius = distance * 0.3 * next() * next();
                let ours = rays.meets_ball(&cones, offset, radius);
                if scan(offset, radius, -1e-7) {
                    assert!(
                        ours,
                        "widened by {by} rad, {offset:?}, {radius}: a ball the region meets is \
                         missed"
                    );
                }
                if ours {
                    assert!(
                        scan(offset, radius, 1e-7),
                        "widened by {by} rad, {offset:?}, {radius}: passed, far from any cone"
                    );
                    met += 1;
                } else {
                    missed += 1;
                }
            }
            assert!(
                met > 1_000 && missed > 1_000,
                "widened by {by} rad: {met} met, {missed} not"
            );
        }
    }

    /// Cones widened by nothing are R06.T7.b's, whose half-angle is the lattice's spacing and
    /// whose index serves balls to [`INDEX_BALL_RAD`], bit for bit.
    #[test]
    fn cones_widened_by_nothing_are_the_lattices_own() {
        let rays = random_radii(0x7b_0006);
        let spacing = rays.lattice().spacing().value();
        let cones = rays.cones_widened_by(Radians::new(0.0));
        let angle = spacing + ANGLE_MARGIN_RAD;
        assert_eq!(
            cones.widened.map(bits),
            [math::cos(angle), math::sin(angle)].map(bits)
        );
        assert_eq!(bits(cones.index_ball_sin), bits(math::sin(INDEX_BALL_RAD)));
    }

    #[test]
    #[should_panic(
        expected = "a cone is widened by a finite angle, not negative: -0.000000001 rad"
    )]
    fn a_negative_widening_is_refused() {
        let _ = random_radii(0x7b_0006).cones_widened_by(Radians::new(-1e-9));
    }

    #[test]
    #[should_panic(expected = "a cone is widened by a finite angle, not negative: NaN rad")]
    fn a_nan_widening_is_refused() {
        let _ = random_radii(0x7b_0006).cones_widened_by(Radians::new(f64::NAN));
    }

    /// Each ray's widened radius is the largest of the rays' within twice the spacing, its own
    /// among them, by a scan of every ray.
    #[test]
    fn the_widening_takes_the_largest_radius_within_twice_the_spacing() {
        let rays = random_radii(0x7b_0006);
        let lattice = rays.lattice();
        let widened = widen(lattice, rays.radii_ly(), WIDENING_SPACINGS);
        let reach = WIDENING_SPACINGS * lattice.spacing().value();
        for (k, (direction, &w)) in lattice.directions().iter().zip(&widened).enumerate() {
            let scanned = lattice
                .directions()
                .iter()
                .zip(rays.radii_ly())
                .filter(|(d, _)| math::acos(d.dot(direction).clamp(-1.0, 1.0)) <= reach)
                .map(|(_, &r)| r)
                .fold(0.0, f64::max);
            assert_eq!(bits(w), bits(scanned), "ray {k}");
            assert!(w >= rays.radii_ly()[k]);
        }
    }

    /// λ is the largest yield that keeps the stars left out under the budget: with it they are,
    /// and with the next larger yield they are not; each ray keeps up to its outermost interval of
    /// that yield, so a richer interval beyond a poorer one carries the poorer one in.
    #[test]
    fn lambda_is_the_largest_yield_that_keeps_the_count_beyond_under_one() {
        // Two rays of four intervals: stars and systems.
        let stars = [5.0, 0.6, 0.3, 0.2, 4.0, 0.1, 0.4, 0.05];
        let systems = [1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 2.0, 5.0];
        // Yields: 5, 0.3, 0.1, 0.05; 4, 0.05, 0.2, 0.01.
        let kept = ray_extents(&stars, &systems, 4, 1.0);
        // At λ 0.2 the first ray keeps two intervals and the second three: 0.3 + 0.2 + 0.05 =
        // 0.55 left out. At λ 0.3 the second keeps one, leaving 0.55 + 0.5 = 1.05.
        assert_eq!(kept, vec![2, 3]);
        assert_eq!(ray_extents(&stars, &systems, 4, 0.5), vec![3, 3]);
        assert_eq!(ray_extents(&stars, &systems, 4, 100.0), vec![0, 0]);
        assert_eq!(ray_extents(&[0.0; 8], &systems, 4, 1.0), vec![0, 0]);
        assert_eq!(ray_extents(&stars, &systems, 4, 1e-9), vec![4, 4]);
    }

    /// The real limit is one of the census's shell edges, so the real tier's last shell ends on it
    /// (rendering plan R13, Design note 3).
    #[test]
    fn the_real_limit_is_a_shell_edge() {
        use crate::sky::census::SHELL_EDGES_LY;
        let at = SHELL_EDGES_LY
            .iter()
            .position(|&edge| bits(f64::from(edge)) == bits(REAL_LIMIT_LY));
        assert_eq!(at, Some(4), "{REAL_LIMIT_LY} ly among {SHELL_EDGES_LY:?}");
    }

    /// Holding a ceiling's radii within the cut's ray by ray and within the real limit commute,
    /// bit for bit, in either order and whichever radii are held within the other, at an edge
    /// some rays reach exactly (R13.T1.b's determinism audit): `real_boundary` takes `lesser`,
    /// then `within`.
    #[test]
    fn holding_within_the_cut_and_within_the_limit_commute() {
        let (high, low) = (random_radii(0x7b_0010), random_radii(0x7b_0011));
        let radii_bits =
            |rays: &RayRadii| rays.radii_ly().iter().map(|&r| bits(r)).collect::<Vec<_>>();
        let exact = high.radii_ly()[17];
        for edge in [REAL_LIMIT_LY, exact, 50.0, 1e6] {
            let first = high.lesser(&low).within(edge);
            for other in [
                high.within(edge).lesser(&low),
                low.lesser(&high).within(edge),
                low.within(edge).lesser(&high),
                high.within(edge).lesser(&low.within(edge)),
            ] {
                assert_eq!(radii_bits(&other), radii_bits(&first), "at {edge} ly");
                assert_eq!(other, first);
            }
            let (held, at) = (
                first.radii_ly().iter().filter(|&&r| r < edge).count(),
                first
                    .radii_ly()
                    .iter()
                    .filter(|&&r| bits(r) == bits(edge))
                    .count(),
            );
            if edge < 1e5 {
                assert!(held > 0 && at > 0, "at {edge} ly: {held} within, {at} at");
            }
        }
    }

    /// The counts at the cut and at the ceiling over the same rays from `observer`: the standard
    /// lattice's, measured once, as `census_plan` counts them at a ceiling, each count
    /// `layer_caps`' at its cut, bit for bit.
    fn counts_over_one_lattice(observer: &Observer, cuts_v: &[f64]) -> Vec<CapCount> {
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let rays = RayExtinctions::measure_clearest(
            milky_way_galaxy(),
            observer.position(),
            lattice,
            SUB_RAYS,
            &mut cache,
        );
        cuts_v
            .iter()
            .map(|&v| {
                CapCount::over(
                    milky_way_galaxy(),
                    milky_way_tables(),
                    milky_way_envelope(),
                    observer,
                    &rays,
                    vec![v; CAP_RAYS],
                    RADIAL_STEPS_PER_DECADE,
                )
            })
            .collect()
    }

    /// The real boundary near the Sun and in the nuclear disc at the cut 7.95, at the ceilings
    /// V 4.5 and 5.0 (rendering plan R13, R13.T2.a; Design note 3):
    ///
    /// - every ray of C's, D's and E's R(u) lies within its cap at the cut and the real limit, and
    ///   is its cap at the ceiling where neither holds it; near the Sun the limit holds some of
    ///   D's and E's rays, and in the nuclear disc none, where each layer's expected count
    ///   brighter than the ceiling beyond R(u) is under one;
    /// - each layer's `bright_beyond` is the count at the ceiling's beyond R(u), and its
    ///   `expected_beyond` the count at the cut's, bit for bit, each also by the closure towards
    ///   R(u)'s rays; its rule bound is its cut's;
    /// - A, B and the brown dwarfs keep their caps at the cut, stating their count brighter than
    ///   the ceiling beyond them;
    /// - `census_plan` at the ceiling takes these caps, bit for bit.
    #[test]
    fn the_real_boundary_is_held_within_the_cut_and_the_limit_and_states_its_counts() {
        let points = [
            ("near the Sun", [0.0, 26_000.0, 68.0], true),
            ("in the nuclear disc", [0.0, 150.0, 0.0], false),
        ];
        for (name, at, limit_holds) in points {
            let observer = observer_at(at);
            let counts = counts_over_one_lattice(&observer, &[7.95, 4.5, 5.0]);
            let (at_cut, caps) = (&counts[0], counts[0].caps());
            for (ceiling, at_ceiling) in [(4.5, &counts[1]), (5.0, &counts[2])] {
                let real = real_boundary(at_ceiling, at_cut, &caps);
                let own = at_ceiling.caps();
                for ((r, cut), high) in real.iter().zip(&caps).zip(&own) {
                    let layer = r.layer();
                    assert_eq!(layer, cut.layer());
                    let bright = r.bright_beyond().expect("a real boundary states it");
                    if !REAL_BOUNDARY_LAYERS.contains(&layer) {
                        assert_eq!(
                            &LayerCap {
                                bright_beyond: None,
                                ..r.clone()
                            },
                            cut,
                            "{name}: {layer:?} at the cut's cap"
                        );
                        assert_eq!(bits(bright), bits(at_ceiling.stars_beyond(cut)));
                        continue;
                    }
                    let rays = r.rays().expect("by ray");
                    let (low, high) = (
                        cut.rays().expect("by ray").radii_ly(),
                        high.rays().expect("by ray").radii_ly(),
                    );
                    let mut held = 0_usize;
                    for ((&r, &c), &h) in rays.radii_ly().iter().zip(low).zip(high) {
                        assert!(r <= c && r <= REAL_LIMIT_LY, "{name}: {layer:?} ray {r} ly");
                        if bits(r) == bits(REAL_LIMIT_LY) && h > REAL_LIMIT_LY {
                            held += 1;
                        } else {
                            assert_eq!(bits(r), bits(h.min(c)), "{name}: {layer:?}");
                        }
                    }
                    assert_eq!(bits(r.expected_beyond()), bits(at_cut.stars_beyond(r)));
                    assert_eq!(
                        bits(r.expected_beyond()),
                        bits(at_cut.stars_beyond_toward(layer, |u| rays.toward(u)))
                    );
                    assert_eq!(bits(bright), bits(at_ceiling.stars_beyond(r)));
                    assert_eq!(
                        bits(bright),
                        bits(at_ceiling.stars_beyond_toward(layer, |u| rays.toward(u)))
                    );
                    assert_eq!(r.rule_bound(), cut.rule_bound());
                    assert_eq!(bits(r.radius().value()), bits(rays.largest().value()));
                    eprintln!(
                        "{name}, V_P {ceiling:.1}, {layer:?}: {held} rays held at the limit; beyond \
                         R(u) {bright:.3} brighter than V_P, {:.1} brighter than the cut",
                        r.expected_beyond()
                    );
                    if limit_holds {
                        assert!(layer == Layer::C || held > 0, "{name}: {layer:?}");
                    } else {
                        assert_eq!(held, 0, "{name}: {layer:?}");
                        assert!(bright < 1.0, "{name}: {layer:?} {bright}");
                    }
                }
            }
            if limit_holds {
                let query = crate::sky::census::SkyQuery::builder(observer, Magnitudes::new(7.95))
                    .synthetic_ceiling(Magnitudes::new(5.0))
                    .build()
                    .expect("a query at the ceiling");
                let plan = crate::sky::census::census_plan(
                    milky_way_galaxy(),
                    milky_way_tables(),
                    milky_way_envelope(),
                    &query,
                    &mut NoiseCache::with_capacity(1 << 16),
                );
                assert_eq!(
                    plan.caps(),
                    real_boundary(&counts[2], at_cut, &caps).as_slice()
                );
                let own = layer_caps(
                    milky_way_galaxy(),
                    milky_way_tables(),
                    milky_way_envelope(),
                    &observer,
                    Magnitudes::new(7.95),
                    &mut NoiseCache::with_capacity(1 << 16),
                );
                assert_eq!(caps, own, "the counts over one lattice are layer_caps'");
            }
        }
    }

    /// The ceiling a server states, RM3's V 5.0 or the cut where the cut is brighter, holds every
    /// served sky's C, D and E within the real limit at every cut, and none is stated where it
    /// would hold nothing (rendering plan R13, R13.T2.b; `decision-r13-t2b-ceiling.md` §8), near
    /// the Sun and in the nuclear disc at uniform cuts, each point's rays measured once:
    ///
    /// - `served_ceiling` is 4.6, 5.0, 5.0 and 5.0 near the Sun at the cuts 4.6, 5.0, 5.4 and 7.95,
    ///   the cut's D and E caps reaching past 2,000 ly, and none, none, 5.0 and 5.0 in the nuclear
    ///   disc, whose cut's caps lie within it at 4.6 and 5.0; never fainter than the cut;
    /// - where stated, every C, D and E ray of the real boundary lies within the limit; at a
    ///   ceiling at the cut, each of their `bright_beyond` is its `expected_beyond`, bit for bit;
    ///   in the nuclear disc at 5.4 the limit holds no ray, the ceiling holds some, and under one
    ///   star brighter than it is expected beyond R(u) a layer;
    /// - where none is stated, the cut's C, D and E caps lie within the limit, a real boundary at
    ///   the cut would be those caps on every ray, and `census_plan` without a ceiling takes them,
    ///   bit for bit.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one table of points and cuts, each row's assertions as the ruling lists them"
    )]
    fn the_served_ceiling_holds_the_limit_at_every_cut() {
        use crate::sky::census::{SkyQuery, census_plan};
        const CEILING: f64 = 5.0;
        let cuts = [4.6, CEILING, 5.4, 7.95];
        let points = [
            (
                "near the Sun",
                [0.0, 26_000.0, 68.0],
                [Some(4.6), Some(CEILING), Some(CEILING), Some(CEILING)],
            ),
            (
                "in the nuclear disc",
                [0.0, 150.0, 0.0],
                [None, None, Some(CEILING), Some(CEILING)],
            ),
        ];
        let ray_bits = |cap: &LayerCap| -> Vec<u64> {
            cap.rays()
                .expect("by ray")
                .radii_ly()
                .iter()
                .map(|&r| bits(r))
                .collect()
        };
        for (name, at, expected) in points {
            let observer = observer_at(at);
            let counts = counts_over_one_lattice(&observer, &cuts);
            let at_ceiling = &counts[1];
            for ((&cut, at_cut), expected) in cuts.iter().zip(&counts).zip(expected) {
                let caps = at_cut.caps();
                let stated = served_ceiling(Magnitudes::new(cut), Magnitudes::new(CEILING), &caps)
                    .map(Magnitudes::value);
                assert_eq!(stated, expected, "{name} at the cut {cut}");
                let Some(v) = stated else {
                    // R06's sky, which reaches no farther than a real boundary would.
                    let real = real_boundary(at_cut, at_cut, &caps);
                    for (r, c) in real.iter().zip(&caps) {
                        if !REAL_BOUNDARY_LAYERS.contains(&c.layer()) {
                            continue;
                        }
                        assert!(
                            c.radius().value() <= REAL_LIMIT_LY,
                            "{name} at {cut}: {:?}",
                            c.layer()
                        );
                        assert_eq!(ray_bits(r), ray_bits(c), "{name} at {cut}: {:?}", c.layer());
                    }
                    let query = SkyQuery::builder(observer, Magnitudes::new(cut))
                        .build()
                        .expect("a query");
                    let plan = census_plan(
                        milky_way_galaxy(),
                        milky_way_tables(),
                        milky_way_envelope(),
                        &query,
                        &mut NoiseCache::with_capacity(1 << 16),
                    );
                    assert_eq!(plan.caps(), caps.as_slice(), "{name} at {cut}");
                    eprintln!("{name} at the cut {cut}: no ceiling; R06's caps within the limit");
                    continue;
                };
                assert!(v <= cut, "{name}: a ceiling of {v} at the cut {cut}");
                let at_v = if bits(v) == bits(cut) {
                    at_cut
                } else {
                    at_ceiling
                };
                let real = real_boundary(at_v, at_cut, &caps);
                for r in &real {
                    let layer = r.layer();
                    if !REAL_BOUNDARY_LAYERS.contains(&layer) {
                        continue;
                    }
                    let radii = r.rays().expect("by ray").radii_ly();
                    assert!(
                        radii.iter().all(|&ly| ly <= REAL_LIMIT_LY),
                        "{name} at {cut}: {layer:?} within the limit"
                    );
                    let held = radii
                        .iter()
                        .filter(|&&ly| bits(ly) == bits(REAL_LIMIT_LY))
                        .count();
                    let bright = r.bright_beyond().expect("a real boundary states it");
                    eprintln!(
                        "{name} at the cut {cut}, ceiling {v}, {layer:?}: {held} rays at the \
                         limit; beyond R(u) {bright:.3} brighter than the ceiling, {:.3} than \
                         the cut",
                        r.expected_beyond()
                    );
                    if bits(v) == bits(cut) {
                        assert_eq!(bits(bright), bits(r.expected_beyond()), "{name} at {cut}");
                    }
                    if name == "in the nuclear disc" && bits(cut) == bits(5.4) {
                        assert_eq!(held, 0, "{name}: {layer:?}");
                        assert!(bright < 1.0, "{name}: {layer:?} {bright}");
                    }
                }
                if name == "in the nuclear disc" && bits(cut) == bits(5.4) {
                    assert!(
                        real.iter().zip(&caps).any(|(r, c)| {
                            REAL_BOUNDARY_LAYERS.contains(&r.layer()) && ray_bits(r) != ray_bits(c)
                        }),
                        "{name}: the ceiling holds some C–E ray within the cut's cap"
                    );
                }
            }
        }
    }

    /// `census_plan` at a ceiling for an eye-only request that caps by the eye's visibility counts
    /// its cut by that visibility, as [`layer_caps_by_visibility`] does (rendering plan R13,
    /// R13.T2.a): in the nuclear disc, with a visibility of V 6.5 over half the sky and the cut
    /// 7.95 over the rest, its caps are [`real_boundary`] of the counts at the ceiling V 5.0 and
    /// by the visibility, bit for bit, and its A, B and brown dwarfs' caps are
    /// [`layer_caps_by_visibility`]'s, beside the count each states beyond them; they are not the
    /// uniform cut's.
    #[test]
    fn a_ceiling_by_the_eyes_visibility_counts_its_cut_by_it() {
        use crate::sky::EyeObserver;
        use crate::sky::census::{SkyQuery, census_plan};
        let (galaxy, tables, envelope) =
            (milky_way_galaxy(), milky_way_tables(), milky_way_envelope());
        let observer = observer_at([0.0, 150.0, 0.0]);
        let (eye, cut) = (EyeObserver::default(), Magnitudes::new(7.95));
        let visibility = EyeVisibility::of_limits(observer, eye, cut, |u| {
            if u.components()[1] > 0.0 { 6.5 } else { 7.95 }
        });
        let query = SkyQuery::builder(observer, cut)
            .eye(eye)
            .eye_visibility(visibility.clone())
            .synthetic_ceiling(Magnitudes::new(5.0))
            .build()
            .expect("an eye-only request at a ceiling");
        let plan = census_plan(
            galaxy,
            tables,
            envelope,
            &query,
            &mut NoiseCache::with_capacity(1 << 16),
        );
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let rays = RayExtinctions::measure_clearest(
            galaxy,
            observer.position(),
            Arc::clone(&lattice),
            SUB_RAYS,
            &mut NoiseCache::with_capacity(1 << 16),
        );
        let count = |cuts_v| {
            CapCount::over(
                galaxy,
                tables,
                envelope,
                &observer,
                &rays,
                cuts_v,
                RADIAL_STEPS_PER_DECADE,
            )
        };
        let (seen, at_ceiling) = (
            count(visible_cuts_v(&lattice, &visibility)),
            count(vec![5.0; CAP_RAYS]),
        );
        let expected = real_boundary(&at_ceiling, &seen, &seen.caps());
        assert_eq!(plan.caps(), expected.as_slice());
        let by_visibility = layer_caps_by_visibility(
            galaxy,
            tables,
            envelope,
            &observer,
            &visibility,
            &mut NoiseCache::with_capacity(1 << 16),
        );
        for (real, own) in plan.caps().iter().zip(&by_visibility) {
            if !REAL_BOUNDARY_LAYERS.contains(&real.layer()) {
                let without = LayerCap {
                    bright_beyond: None,
                    ..real.clone()
                };
                assert_eq!(&without, own, "{:?}", real.layer());
            }
        }
        let uniform = count(vec![cut.value(); CAP_RAYS]);
        assert_ne!(
            plan.caps(),
            real_boundary(&at_ceiling, &uniform, &uniform.caps()).as_slice(),
            "the visibility's count, not the uniform cut's"
        );
    }

    /// R(u)'s rays near the Sun at a coarse count (192 rays, 12 steps a decade) at the cut 7.95 and
    /// the ceilings V 4.5 and 5.0, pinned with each layer's counts beyond (R13.T1.b's determinism
    /// audit; rendering plan R13, R13.T2.a): every C, D and E ray, the real limit holding some of
    /// E's, and each layer's radius, rule bound, `expected_beyond` and `bright_beyond`, whose bits
    /// are the counts' own beyond R(u).
    #[test]
    fn the_real_boundary_is_pinned() {
        use hyperion_testkit::golden::GoldenWriter;
        let resolution = CapResolution::new(192, 12).expect("non-zero");
        let sun = [0.0, 26_000.0, 68.0];
        let at_cut = count_at(sun, resolution);
        let caps = at_cut.caps();
        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        w.line("near the Sun (0, 26,000, 68) ly, cut V 7.95, 192 rays at 12 steps a decade");
        for ceiling in [4.5, 5.0] {
            let at_ceiling = CapCount::measure(
                milky_way_galaxy(),
                milky_way_tables(),
                milky_way_envelope(),
                &observer_at(sun),
                Magnitudes::new(ceiling),
                resolution,
                &mut NoiseCache::with_capacity(1 << 16),
            );
            let real = real_boundary(&at_ceiling, &at_cut, &caps);
            let mut held_e = 0_usize;
            for cap in &real {
                let layer = cap.layer();
                let label = format!("V_P {ceiling:.1} {layer:?}");
                let bright = cap.bright_beyond().expect("stated");
                w.f64(&format!("{label} radius"), cap.radius().value());
                w.f64(&format!("{label} rule bound"), cap.rule_bound().value());
                w.f64(&format!("{label} expected beyond"), cap.expected_beyond());
                w.f64(&format!("{label} bright beyond"), bright);
                if !REAL_BOUNDARY_LAYERS.contains(&layer) {
                    continue;
                }
                let rays = cap.rays().expect("by ray");
                assert_eq!(
                    bits(bright),
                    bits(at_ceiling.stars_beyond_toward(layer, |u| rays.toward(u)))
                );
                assert_eq!(
                    bits(cap.expected_beyond()),
                    bits(at_cut.stars_beyond_toward(layer, |u| rays.toward(u)))
                );
                for (k, &r) in rays.radii_ly().iter().enumerate() {
                    w.f64(&format!("{label} ray {k:03}"), r);
                    if layer == Layer::E && bits(r) == bits(REAL_LIMIT_LY) {
                        held_e += 1;
                    }
                }
            }
            assert!(
                held_e > 0,
                "V_P {ceiling}: the limit holds some of E's rays"
            );
        }
        hyperion_testkit::golden!("sky/real_boundary", w.as_str());
    }
}
