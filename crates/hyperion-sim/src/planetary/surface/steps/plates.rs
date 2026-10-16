//! Step 1, the plates (R09.T11, Design note 6).
//!
//! Where the tectonic regime is a mobile lid, the step draws the plates, rasterises them onto the
//! cells, places the continental crust, classes every plate boundary by the plates' relative
//! motion, and runs two distance transforms over the cell graph: one gives each cell its signed
//! distance to the nearest boundary, that boundary's kind and its obliquity, and the other each
//! oceanic cell its distance to its own plate's nearest ridge, the half-rate there and its age
//! (`decision-r09-t5.md` item 5). A stagnant lid gets no boundaries, and so, until a morphology is
//! ruled for each, do the heat-pipe, episodic and ice-shell regimes: the step leaves their cells
//! on the initial state's one lid, for R09.T12.c's provinces.
//!
//! # The draws
//!
//! The plates are drawn on `surface.coarse.plates`, plate k on `surface_item(k)`, and the tail's
//! count from word `COUNT_WORD` of item 0. Seven large plates share whatever of 4π the tail
//! leaves, each at least 1 sr; the tail takes Bird 2003's form, N(>A) ≈ 7 (A ÷ 1 sr)^−1/3 (G³ 4,
//! 1027, ¶115 and Fig. 19), with Design note 6's exponent 0.31, at which the count from 0.002 sr
//! is PB2002's 41 (Table 1): N(>A) = 7 (A ÷ 1 sr)^−0.31 from `SMALLEST_PLATE_SR`, or the coarse
//! cell's mean solid angle where that is larger, up to 1 sr, its count a Poisson draw of the
//! law's mean, about 41, and each area a draw from the law. That gives about 48 plates, as
//! PB2002 has at or above 0.002 sr. The law's expected tail, about 3.1 sr, exceeds Earth's 2.39,
//! and in about one draw in fifty (11 of R09.T11's 500 seeds) the tail exceeds the 4π − 7 sr the
//! large plates leave it: the tail then ends at the plate that would overflow it.
//!
//! Each plate draws, in this order on its stream: its seed point, uniform on the sphere (two
//! words); its angular velocity, each component a standard normal (two Box–Muller pairs, the last
//! variate unused) times the speed scale (`angular_speed_scale_rad_per_s`); its area, a tail
//! plate's from the law and a large plate's share of the large plates' excess over 1 sr each as
//! an exponential (one word); and its continental nucleus, a centre uniform on the sphere and a
//! size (three words).
//!
//! # The raster
//!
//! Each cell takes the plate of largest score x′ · p + w, where x′ is the cell's centre under the
//! low-frequency warp of `surface.coarse.warp` (at `WARP_LEVEL`, by `WARP_AMPLITUDE`), p the
//! plate's seed and w its weight,
//! ties to the lower plate index. The weights are what make the drawn areas the realised ones: an
//! unweighted diagram of uniform seeds (w = 0, Design note 6's largest dot product) has cells of
//! near-equal area, not the tail law, so each plate's weight is fitted until its cells cover its
//! drawn area to 1%, or two mean cells for the smallest (a power diagram on the sphere: the
//! semi-discrete transport problem, solved by damped Newton steps on a Laplacian of the plates'
//! shared boundaries, at each level from 6 up to the field's). A part of a plate that the raster
//! cuts off from its largest part, mostly a single cell, joins the neighbouring plate it shares
//! most edges with, so that every plate is one region; the field's level is fitted with its parts
//! joined.
//!
//! # The crust
//!
//! The continental crust covers the inputs' continental fraction of the sphere to half a cell: the
//! cells nearest the plates' continental nuclei, each distance divided by its nucleus's size. The
//! nuclei are not confined to their plates, since continents are older than the plates that carry
//! them: a continent may straddle a boundary, where it collides, rifts or slides, and may lie
//! inside a plate, whose passive margins R09.T12.a's turbidites start from. The rest is oceanic. A
//! world with no tertiary crust in its palette has no continental crust, and a mobile lid with no
//! secondary crust, whose plates could form no oceanic floor, gets the stagnant lid's.
//!
//! # The boundaries
//!
//! A boundary cell is one with an edge neighbour on another plate; of its neighbours' plates it
//! takes the nearest boundary, by the score difference over its gradient. The boundary's normal
//! is that gradient's, from the four edge neighbours by least squares, so that the raster's
//! staircase does not reach it. The relative velocity (`ω_a` − `ω_b`) × r against the normal
//! classes it: within 60° of head-on it is convergent, within 60° of head-on opening divergent, and
//! otherwise a transform (the testing worlds' equal thirds, a convention of low confidence). A
//! convergent boundary between two continents is a collision; elsewhere one side subducts: the
//! oceanic side beneath a continent, and of two oceanic plates the one whose floor at their shared
//! convergent boundary is older on average (the higher index at a tie).
//!
//! Both distance transforms are multi-source shortest paths over each plate's own cells, edge and
//! vertex neighbours, with great-circle edge lengths, settled in (distance, cell index, source)
//! order, each cell inheriting its source's boundary. The nearest-boundary transform starts from
//! every boundary cell; the ridge transform from each plate's divergent boundary cells alone, so
//! that the age is never taken from a trench (Design note 7).
//!
//! # The motion
//!
//! The pass has no plate history: a floor's rate at formation is today's at its source ridge, and
//! its age that ridge's distance over that rate, capped at [`OLDEST_FLOOR`] (labelled,
//! `decision-r09-t5.md` item 5). Plates move faster on a world that loses more heat: the speed
//! scale goes as the square of the heat flow (`angular_speed_scale_rad_per_s`).

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{BoundaryKind, Crust, cell_index};
use hyperion_surface::num;
use hyperion_surface::synth::relief::H_REF;
use hyperion_surface::tags::{SURFACE_COARSE_PLATES, SURFACE_COARSE_WARP};

use super::{Pass, SeafloorOrigin, Working};
use crate::math;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::TectonicRegime;
use crate::rng::{ObjectKey, PowerLaw, Stream, SurfaceSeed};
use crate::units::consts::{EARTH_RADIUS_M, SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MEGAYEAR};
use crate::units::{Metres, MetresPerSecond, Radians, Seconds, WattsPerSquareMetre};

/// The whole sphere's solid angle, steradians.
const FOUR_PI: f64 = 4.0 * core::f64::consts::PI;

/// The large plates, seven: PB2002's plates of 1 sr and more (Design note 6, from Bird 2003, G³ 4,
/// 1027, Table 1).
pub(crate) const LARGE_PLATES: usize = 7;

/// A large plate's least area, 1 sr, the tail law's anchor (Design note 6).
pub(crate) const LARGE_PLATE_FLOOR_SR: f64 = 1.0;

/// The tail law's count at its anchor: N(> 1 sr) = 7 (Design note 6).
pub(crate) const TAIL_ANCHOR: f64 = 7.0;

/// The tail law's cumulative exponent, N(>A) ∝ A^−0.31 (Design note 6: Bird 2003 fits 0.002–1 sr
/// by eye with −1/3; a maximum-likelihood fit to PB2002's 41 plates in 0.002–1 sr gives 0.30 ±
/// 0.09; medium confidence in 0.31, high in the counts).
pub(crate) const TAIL_EXPONENT: f64 = 0.31;

/// The smallest plate the tail law reaches, 0.002 sr: PB2002 has 48 plates at or above it, and 52
/// in all (Design note 6, from Bird 2003's Table 1).
pub(crate) const SMALLEST_PLATE_SR: f64 = 0.002;

/// The large plates' count as a float, 7.
const LARGE_PLATES_F64: f64 = 7.0;

/// The most plates a field holds: its plate byte's 256.
pub(crate) const MAX_PLATES: usize = 256;

/// The word of item 0's stream on `surface.coarse.plates` from which the tail's count is drawn,
/// 2⁴⁰: above every word plate 0 draws, so that the count and plate 0 share an item and no word.
pub(crate) const COUNT_WORD: u64 = 1 << 40;

/// Earth's mean ridge half-rate, 23 mm a⁻¹, as metres a second: the rate of floor whose plate has
/// no divergent boundary of its own (`decision-r09-t5.md` item 5), and the speed scale's
/// calibration on an Earth. Bird 2003 (G³ 4, 1027, Table 3) gives oceanic spreading ridges a mean
/// relative speed of 46.6 mm a⁻¹, whose half is 23.3; their mean opening, area production over
/// length (0.095348 m² s⁻¹ over 67,338 km), is 44.7 mm a⁻¹, 22.3 halved, the like-for-like figure
/// of this opening rate (T11's science check; the ruling's 23 stands).
pub const MEAN_HALF_RATE: MetresPerSecond = MetresPerSecond::new(23e-3 / SECONDS_PER_JULIAN_YEAR);

/// The oldest sea floor, 200 Myr, as seconds: ages are capped there, where GDH1 is 9.5 m short
/// of its asymptote (Design note 7's "within 40 m", which holds from 150 Myr), and floor whose
/// plate has no divergent boundary of its own takes it (`decision-r09-t5.md` item 5).
pub const OLDEST_FLOOR: Seconds = Seconds::new(200.0 * SECONDS_PER_MEGAYEAR);

/// Earth's mean surface heat flow, 0.0916 W m⁻² (Davies and Davies 2010, Solid Earth 1, 5,
/// Table 7: 46.7 TW over 5.101 × 10¹⁴ m²), at which the speed scale is
/// [`EARTH_ANGULAR_SPEED_RAD_PER_S`].
pub(crate) const EARTH_HEAT_FLOW: WattsPerSquareMetre = WattsPerSquareMetre::new(0.0916);

/// The angular-speed scale Ω on a world of Earth's heat flow, radians a second: each component of
/// a plate's angular velocity is a standard normal times Ω, so that two plates' relative rotation
/// has a component's standard deviation √2 Ω.
///
/// It is calibrated, in closed form, so that a ridge's mean half-rate on an Earth is
/// [`MEAN_HALF_RATE`]. Two plates' relative surface velocity at a boundary is then a
/// two-dimensional normal of σ = √2 Ω R in the tangent plane; a divergent boundary takes the third
/// of its directions within 60° of opening head-on, where the mean opening is σ √(π ÷ 2) times
/// sin 60° ÷ (π ÷ 3), (3√3 ÷ 2√π) Ω R ≈ 1.466 Ω R. So the mean half-rate is about 0.733 Ω R, and
/// Ω = 4√π `u_h` ÷ (3√3 R⊕): 1.56 × 10⁻¹⁶ rad s⁻¹, 0.28° Myr⁻¹ a component and 0.45° Myr⁻¹ in the
/// mean magnitude, against NNR-MORVEL56's eight largest plates' 0.109–0.651° Myr⁻¹ (mean 0.34)
/// and 23 of its 25 MORVEL plates' 0.107–1.198° Myr⁻¹ (Argus et al. 2011, G³ 12, Q11001,
/// Table 1, read through a transcription; T11's science check). R09.T11's tests measure
/// the reference Earth's realised mean.
pub(crate) const EARTH_ANGULAR_SPEED_RAD_PER_S: f64 =
    4.0 * 1.772_453_850_905_516 * MEAN_HALF_RATE.value()
        / (3.0 * 1.732_050_807_568_877 * EARTH_RADIUS_M);

/// The warp lattice's level, 4: 1,536 cells 0.06–0.11 rad (380–680 km on an Earth) across, whose
/// kernel mean bends the boundaries over wavelengths of some two thousand kilometres there.
pub(crate) const WARP_LEVEL: u8 = 4;

/// The warp kernel's reach, a chord of 0.21 (about 0.21 rad): twice the lattice's widest cell, so
/// that some fifteen lattice vectors weigh in at every point.
const WARP_KERNEL_CHORD: f64 = 0.21;

/// The warp's amplitude for the plates: a cell's centre moves by this times the lattice's
/// interpolated vector of uniform components in [−1, 1) before it is scored, which bends the
/// boundaries off great and small circles by some 0.03 rad RMS (about 200 km on an Earth). An
/// illustration, of low confidence, chosen by eye on the reference Earth.
pub(crate) const WARP_AMPLITUDE: f64 = 0.12;

/// The warp's amplitude for the continental nuclei, larger than the plates' so that continents
/// are not caps: an illustration, of low confidence, chosen by eye on the reference Earth.
pub(crate) const CONTINENT_WARP_AMPLITUDE: f64 = 0.25;

/// The relative motion's angle from head-on convergence below which a boundary is convergent,
/// 60°, and above whose supplement it is divergent: the testing worlds' equal thirds, a
/// convention of low confidence. Under random motion it gives a third of boundaries to each
/// class, against Earth's 35.2% convergent, 36.4% divergent and 28.4% transform by length; Bird
/// 2003 calls a step strike-slip within ±20° of the boundary's azimuth (G³ 4, 1027, ¶127 and
/// Table 3; T11's science check), which would give some 22% transforms.
const CONVERGENT_BELOW: f64 = core::f64::consts::FRAC_PI_3;

/// The relative motion's angle from head-on convergence above which a boundary is divergent, 120°.
const DIVERGENT_ABOVE: f64 = 2.0 * core::f64::consts::FRAC_PI_3;

/// The exponent of a continental nucleus's size, a uniform in (0, 1] squared: most nuclei make
/// microcontinents and a few the continents. An illustration, of low confidence.
const NUCLEUS_SIZE_EXPONENT: f64 = 2.0;

/// The level of the weights' first fit, 6 (24,576 cells), which each finer level up to the
/// field's refines in turn.
const FIT_LEVEL: u8 = 6;

/// The Newton steps of a fit at most, at each level.
const FIT_STEPS: usize = 40;

/// The halvings of a Newton step at most, before the fit stops.
const FIT_HALVINGS: usize = 6;

/// The sweeps of single-plate steps at most, after the Newton steps.
const FIT_SWEEPS: usize = 3;

/// A plate's realised area is fitted to its drawn area to this share of it…
const FIT_RELATIVE: f64 = 0.01;

/// …or to this many of the level's mean cells, whichever is larger.
const FIT_CELLS: f64 = 2.0;

/// The passes of [`join_parts`] at most: one joins nearly every cut-off part.
const JOIN_PASSES: usize = 4;

/// The level whose cells group a grid's cells into the raster's blocks, 4: 256 cells a block at
/// level 8.
const BLOCK_LEVEL: u8 = 4;

/// The margin by which a plate's upper bound in a block must fall short of the best lower bound
/// before the block sets it aside, 10⁻⁹: far above the bounds' rounding, some 10⁻¹⁵ on scores of
/// order 1.
const PRUNE_MARGIN: f64 = 1e-9;

/// The share of a cell side that the boundary's length is per edge crossed: π ÷ 4, the reciprocal
/// of the mean of |cos θ| + |sin θ|, 4 ÷ π, over a boundary's directions on a square grid (a line
/// of length ℓ crosses (4 ÷ π) ℓ unit-grid edges on average, by Cauchy and Crofton).
const BOUNDARY_PER_EDGE: f64 = core::f64::consts::FRAC_PI_4;

/// One plate as the step realised it.
///
/// Plain data with public fields, as the working state's are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plate {
    /// The plate's seed, a unit vector in the body-fixed frame.
    pub seed: [f64; 3],
    /// The plate's angular velocity ω about the body's centre, radians a second in the body-fixed
    /// frame: a point r of the plate on the unit sphere moves at ω × r times the radius.
    pub angular_velocity_rad_per_s: [f64; 3],
    /// The area drawn for the plate, steradians.
    pub drawn_area_sr: f64,
    /// The area its cells cover, steradians: the drawn area to the fit's tolerance.
    pub area_sr: f64,
    /// The plate's weight in the score x′ · p + w its cells won by.
    pub weight: f64,
}

/// Runs the step on `working`: on a mobile lid, every cell's plate, crust, nearest boundary (its
/// kind, signed distance and obliquity) and, on oceanic crust, its sea floor's origin and the
/// reference hill relief, and the working state's plates; on every other regime nothing (see the
/// module documentation).
///
/// # Panics
///
/// Never on valid inputs: every value it writes is finite and within its code.
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    if let Some(layout) = Layout::of(pass) {
        layout.write(working);
    }
}

/// Whether the inputs' regime has plates: a mobile lid alone, until a morphology is ruled for the
/// others (R09's Risks, "Composition").
#[must_use]
const fn has_plates(regime: TectonicRegime) -> bool {
    match regime {
        TectonicRegime::MobileLid => true,
        TectonicRegime::StagnantLid
        | TectonicRegime::HeatPipe
        | TectonicRegime::Episodic
        | TectonicRegime::IceShell => false,
    }
}

/// The index into per-cell vectors of cell `cell`.
///
/// # Panics
///
/// Never on the targets the sim builds for: a `u32` fits a `usize` on 32- and 64-bit targets.
#[must_use]
pub(super) fn slot(cell: u32) -> usize {
    usize::try_from(cell).expect("a cell index fits a usize on every target the sim builds for")
}

/// The cell index of slot `n`.
///
/// # Panics
///
/// If `n` is not below 2³², which no coarse level reaches.
#[must_use]
pub(super) fn cell_of(n: usize) -> u32 {
    u32::try_from(n).expect("a coarse level has under 2^32 cells")
}

/// The plate byte of plate `k`.
///
/// # Panics
///
/// If `k` is 256 or more, which [`MAX_PLATES`] keeps out.
#[must_use]
fn plate_byte(k: usize) -> u8 {
    u8::try_from(k).expect("a field has at most 256 plates")
}

/// a · b, summed in index order.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// a × b.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// a − b.
#[must_use]
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// s a.
#[must_use]
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// |a|.
#[must_use]
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// a ÷ |a|.
#[must_use]
fn unit(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / norm(a))
}

/// The component of `a` tangent to the unit sphere at `at`.
#[must_use]
fn tangent(a: [f64; 3], at: [f64; 3]) -> [f64; 3] {
    sub(a, scale(at, dot(a, at)))
}

/// A point uniform on the unit sphere, from two words: z uniform in [−1, 1) and the longitude
/// uniform in [0, 2π).
fn uniform_direction(stream: &mut Stream) -> [f64; 3] {
    let z = 2.0 * stream.uniform() - 1.0;
    let longitude = core::f64::consts::TAU * stream.uniform();
    let (sin, cos) = math::sin_cos(longitude);
    let r = (1.0 - z * z).sqrt();
    [r * cos, r * sin, z]
}

/// The tail's least plate on a field of `cells` cells: `SMALLEST_PLATE_SR`, or the mean cell's
/// solid angle where that is larger (a level-5 field's 2.05 × 10⁻³ sr), since no plate is smaller
/// than a cell.
#[must_use]
fn smallest_plate_sr(cells: u32) -> f64 {
    num::max(SMALLEST_PLATE_SR, FOUR_PI / f64::from(cells))
}

/// The tail law's expected count of plates from `smallest_sr` (steradians) to 1 sr:
/// 7 ((`smallest_sr` ÷ 1 sr)^−0.31 − 1), 41.06 from 0.002 sr.
#[must_use]
fn expected_tail(smallest_sr: f64) -> f64 {
    TAIL_ANCHOR * (math::powf(smallest_sr / LARGE_PLATE_FLOOR_SR, -TAIL_EXPONENT) - 1.0)
}

/// The angular-speed scale Ω of a world of heat flow `heat_flow`, radians a second:
/// [`EARTH_ANGULAR_SPEED_RAD_PER_S`] × (F ÷ F⊕)².
///
/// In the half-space cooling model the floor's heat flow falls as C ÷ √t with its age (Davies and
/// Davies 2010, Solid Earth 1, 5, eq. 1), so a plate of length L moving at u loses
/// q̄ = 2kΔT √(u ÷ πκL) on average (Turcotte and Schubert 2014, Geodynamics, 3rd ed., ch. 4 and
/// 6; from memory), so at the same mantle and with plates a fixed share of the sphere, L ∝ R,
/// u ∝ q̄² R and the angular speed u ÷ R goes as q̄² whatever the radius. Medium-low confidence,
/// labelled: boundary-layer scaling is contested for hotter mantles (Korenaga 2006 and 2008, from
/// memory, argues for slower plates). Earth's heat flow gives Earth's speeds, and a world of none
/// has plates that do not move.
#[must_use]
pub(crate) fn angular_speed_scale_rad_per_s(heat_flow: WattsPerSquareMetre) -> f64 {
    let ratio = heat_flow.value() / EARTH_HEAT_FLOW.value();
    EARTH_ANGULAR_SPEED_RAD_PER_S * ratio * ratio
}

/// What the step draws for one plate, or what a test places.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlateDraw {
    /// The seed, a unit vector.
    seed: [f64; 3],
    /// The angular velocity, radians a second.
    angular_velocity_rad_per_s: [f64; 3],
    /// The drawn area, steradians.
    area_sr: f64,
    /// The continental nucleus's centre, a unit vector.
    nucleus: [f64; 3],
    /// The continental nucleus's size, in (0, 1].
    nucleus_size: f64,
}

/// The plates of the body of `seed` on a field of `cells` cells, at the speed scale
/// `omega_rad_per_s` (radians a second): see the module documentation, "The draws".
///
/// # Panics
///
/// Never: the law's limits are valid and every draw is finite.
#[must_use]
fn draw_plates(seed: SurfaceSeed, cells: u32, omega_rad_per_s: f64) -> Vec<PlateDraw> {
    let smallest = smallest_plate_sr(cells);
    let mut count = seed.stream(SURFACE_COARSE_PLATES, ObjectKey::surface_item(0));
    count.seek(COUNT_WORD);
    let cap = u64::try_from(MAX_PLATES - LARGE_PLATES).expect("249 fits a u64");
    let tail = usize::try_from(count.poisson(expected_tail(smallest)).min(cap))
        .expect("at most 249 tail plates fit a usize");
    let law = PowerLaw::new(1.0 + TAIL_EXPONENT, smallest, LARGE_PLATE_FLOOR_SR)
        .expect("the tail law's limits are positive and ordered");
    let budget = FOUR_PI - LARGE_PLATE_FLOOR_SR * LARGE_PLATES_F64;
    let mut tail_area = 0.0;
    let mut draws: Vec<PlateDraw> = Vec::with_capacity(LARGE_PLATES + tail);
    for k in 0..LARGE_PLATES + tail {
        let item = u64::try_from(k).expect("a plate index fits a u64");
        let mut stream = seed.stream(SURFACE_COARSE_PLATES, ObjectKey::surface_item(item));
        let point = uniform_direction(&mut stream);
        let (a, b) = stream.standard_normal_pair();
        let (c, _) = stream.standard_normal_pair();
        let size = if k < LARGE_PLATES {
            -math::ln(stream.uniform_open_low())
        } else {
            stream.power_law(&law)
        };
        let nucleus = uniform_direction(&mut stream);
        let nucleus_size = math::powf(stream.uniform_open_low(), NUCLEUS_SIZE_EXPONENT);
        if k >= LARGE_PLATES {
            if tail_area + size > budget {
                break;
            }
            tail_area += size;
        }
        draws.push(PlateDraw {
            seed: point,
            angular_velocity_rad_per_s: [
                a * omega_rad_per_s,
                b * omega_rad_per_s,
                c * omega_rad_per_s,
            ],
            area_sr: size,
            nucleus,
            nucleus_size,
        });
    }
    let excess = FOUR_PI - tail_area - LARGE_PLATE_FLOOR_SR * LARGE_PLATES_F64;
    let shares: f64 = draws[..LARGE_PLATES].iter().map(|d| d.area_sr).sum();
    for draw in &mut draws[..LARGE_PLATES] {
        draw.area_sr = LARGE_PLATE_FLOOR_SR + excess * (draw.area_sr / shares);
    }
    draws
}

/// The boundaries' low-frequency warp: a vector at each cell of the lattice at `WARP_LEVEL`,
/// each component uniform in [−1, 1), and between them their Shepard mean under a compact kernel,
/// φ = (1 − |x − c|² ÷ h²)³ within the chord h = [`WARP_KERNEL_CHORD`] of a lattice centre c: a
/// polynomial in x, C² wherever it is not zero, so that the mean is C² over the sphere, the face
/// edges included, since it reads directions and not face coordinates.
#[derive(Debug, Clone, PartialEq)]
struct Warp {
    /// The lattice's centres, in its cell-index order.
    centres: Vec<[f64; 3]>,
    /// Each lattice cell's vector, in the same order.
    vectors: Vec<[f64; 3]>,
    /// For each lattice cell, the lattice cells whose kernel reaches into it, ascending.
    reaching: Vec<Vec<u32>>,
}

/// The warp's vector at every cell of a grid, and the centres it gives.
#[derive(Debug, Clone, PartialEq)]
struct Warped {
    /// The grid's centres moved by `WARP_AMPLITUDE`: the plates'.
    plates: Vec<[f64; 3]>,
    /// The grid's centres moved by [`CONTINENT_WARP_AMPLITUDE`]: the continental nuclei's.
    continents: Vec<[f64; 3]>,
}

impl Warp {
    /// The warp of the body of `seed`: each lattice cell's stream on `surface.coarse.warp`, keyed
    /// by its `surface_cell`, draws the three components in order, one word each.
    ///
    /// # Panics
    ///
    /// Never: every lattice index names a cell.
    #[must_use]
    fn draw(seed: SurfaceSeed) -> Self {
        let lattice =
            CellGraph::new(WARP_LEVEL).expect("the warp's level, 4, is at most CoarseLevel::MAX");
        let vectors = lattice
            .cells()
            .iter()
            .map(|cell| {
                let mut stream = seed.stream(
                    SURFACE_COARSE_WARP,
                    ObjectKey::surface_cell(
                        cell.face().index(),
                        cell.level(),
                        cell.i(),
                        cell.j(),
                        0,
                    )
                    .expect("a lattice cell is a surface cell"),
                );
                [0, 1, 2].map(|_| stream.uniform_in(-1.0, 1.0))
            })
            .collect();
        let centres = lattice.centres().to_vec();
        // A point of cell A lies within its corners' chord ρ_A of A's centre, so a kernel reaching
        // it has its centre within ρ_A + h of A's.
        let reaching = lattice
            .cells()
            .iter()
            .zip(&centres)
            .map(|(cell, &centre)| {
                let corners = [(0, 0), (64, 0), (0, 64), (64, 64)]
                    .map(|(x, y)| norm(sub(cell.vertex_dir(x, y), centre)));
                let reach = corners.into_iter().fold(0.0, num::max) + WARP_KERNEL_CHORD;
                (0..lattice.len())
                    .filter(|&m| norm(sub(centres[slot(m)], centre)) < reach)
                    .collect()
            })
            .collect();
        Self {
            centres,
            vectors,
            reaching,
        }
    }

    /// The warp's vector at `x`, a direction in lattice cell `home`: the kernel-weighted mean of
    /// the lattice vectors that reach it, in lattice order.
    ///
    /// # Panics
    ///
    /// If no kernel reaches `x`, which the kernel's chord, wider than any cell, rules out.
    #[must_use]
    fn vector(&self, x: [f64; 3], home: u32) -> [f64; 3] {
        let h2 = WARP_KERNEL_CHORD * WARP_KERNEL_CHORD;
        let mut sum = [0.0; 3];
        let mut weight = 0.0;
        for &m in &self.reaching[slot(home)] {
            let offset = sub(x, self.centres[slot(m)]);
            let r2 = dot(offset, offset);
            if r2 >= h2 {
                continue;
            }
            let t = 1.0 - r2 / h2;
            let w = t * t * t;
            let v = self.vectors[slot(m)];
            sum = [sum[0] + w * v[0], sum[1] + w * v[1], sum[2] + w * v[2]];
            weight += w;
        }
        assert!(weight > 0.0, "the warp's kernels reach every point");
        scale(sum, 1.0 / weight)
    }

    /// The centres of `grid`'s cells under the warp, in cell order: each moved by an amplitude
    /// times the warp's vector there and put back on the unit sphere.
    ///
    /// # Panics
    ///
    /// If `grid` is coarser than the lattice, or a warped centre is not finite.
    #[must_use]
    fn apply(&self, grid: &CellGraph) -> Warped {
        let shift = 2 * u32::from(
            grid.level()
                .checked_sub(WARP_LEVEL)
                .expect("a coarse grid is finer than the warp's lattice"),
        );
        let (plates, continents) = grid
            .centres()
            .iter()
            .enumerate()
            .map(|(n, &c)| {
                let v = self.vector(c, cell_of(n) >> shift);
                (
                    displaced(c, v, WARP_AMPLITUDE),
                    displaced(c, v, CONTINENT_WARP_AMPLITUDE),
                )
            })
            .unzip();
        Warped { plates, continents }
    }
}

impl Warped {
    /// No warp: `grid`'s centres as they are, for hand-placed plates.
    #[cfg(test)]
    #[must_use]
    fn none(grid: &CellGraph) -> Self {
        Self {
            plates: grid.centres().to_vec(),
            continents: grid.centres().to_vec(),
        }
    }
}

/// The unit direction `c` moved by `amplitude` times `v` and put back on the unit sphere.
///
/// # Panics
///
/// If the result is not finite.
#[must_use]
fn displaced(c: [f64; 3], v: [f64; 3], amplitude: f64) -> [f64; 3] {
    unit([
        c[0] + amplitude * v[0],
        c[1] + amplitude * v[1],
        c[2] + amplitude * v[2],
    ])
    .map(num::assert_finite)
}

/// A grid's cells as the plates are scored on them: the grid, its warped centres, and the centres
/// in blocks, each the cells of one cell at [`BLOCK_LEVEL`] (contiguous in the field's Morton
/// order), with the block's mean direction and the chord within which its centres lie.
struct Raster<'a> {
    /// The grid.
    grid: &'a CellGraph,
    /// Its cells' warped centres, in cell order.
    centres: &'a [[f64; 3]],
    /// The cells a block.
    block_cells: usize,
    /// Each block's mean direction, a unit vector.
    block_centres: Vec<[f64; 3]>,
    /// Each block's reach: the largest chord from its mean direction to one of its centres.
    block_reach: Vec<f64>,
}

impl<'a> Raster<'a> {
    /// `grid`'s cells, whose warped centres are `centres`, in blocks.
    ///
    /// # Panics
    ///
    /// If `centres` is not one a cell.
    #[must_use]
    fn new(grid: &'a CellGraph, centres: &'a [[f64; 3]]) -> Self {
        assert_eq!(centres.len(), slot(grid.len()), "one centre a cell");
        let block_cells = 1_usize << (2 * (grid.level() - grid.level().min(BLOCK_LEVEL)));
        let (block_centres, block_reach) = centres
            .chunks(block_cells)
            .map(|members| {
                let sum = members
                    .iter()
                    .fold([0.0; 3], |s, c| [s[0] + c[0], s[1] + c[1], s[2] + c[2]]);
                let mean = unit(sum);
                let reach = members
                    .iter()
                    .map(|&c| norm(sub(c, mean)))
                    .fold(0.0, num::max);
                (mean, reach)
            })
            .unzip();
        Self {
            grid,
            centres,
            block_cells,
            block_centres,
            block_reach,
        }
    }
}

/// Each cell's plate, the one of largest score x′ · p + w over `seeds` and `weights`, ties to the
/// lower index, written into `plate` in cell order.
///
/// Each block first sets aside the plates that cannot win any of its cells: for a centre x within
/// the chord ρ of the block's direction c and a plate at chord d = |c − p| from it, x · p =
/// 1 − |x − p|² ÷ 2 lies between 1 − (d + ρ)² ÷ 2 and 1 − max(0, d − ρ)² ÷ 2, so a plate whose
/// upper bound, less [`PRUNE_MARGIN`] for rounding, falls below the best lower bound scores below
/// some other plate at every cell. The rest are scored in index order, so the result is the full
/// scan's, ties and all.
fn assign(raster: &Raster<'_>, seeds: &[[f64; 3]], weights: &[f64], plate: &mut [u8]) {
    let mut bounds: Vec<f64> = Vec::with_capacity(seeds.len());
    let mut candidates: Vec<usize> = Vec::with_capacity(seeds.len());
    let blocks = raster
        .centres
        .chunks(raster.block_cells)
        .zip(plate.chunks_mut(raster.block_cells))
        .zip(raster.block_centres.iter().zip(&raster.block_reach));
    for ((members, cells), (&c, &rho)) in blocks {
        bounds.clear();
        let mut floor = f64::NEG_INFINITY;
        for (p, &w) in seeds.iter().zip(weights) {
            let d = norm(sub(c, *p));
            let near = num::max(d - rho, 0.0);
            let far = d + rho;
            bounds.push(1.0 - 0.5 * near * near + w);
            floor = num::max(floor, 1.0 - 0.5 * far * far + w);
        }
        candidates.clear();
        candidates.extend(
            bounds
                .iter()
                .enumerate()
                .filter(|&(_, &upper)| upper + PRUNE_MARGIN >= floor)
                .map(|(k, _)| k),
        );
        for (x, cell) in members.iter().zip(cells.iter_mut()) {
            let mut best = 0;
            let mut best_score = f64::NEG_INFINITY;
            for &k in &candidates {
                let p = seeds[k];
                let score = x[0] * p[0] + x[1] * p[1] + x[2] * p[2] + weights[k];
                if score > best_score {
                    best = k;
                    best_score = score;
                }
            }
            *cell = plate_byte(best);
        }
    }
}

/// Each of `plates` plates' area, the solid angles of its cells summed in cell order.
#[must_use]
fn areas(plate: &[u8], solid_angles: &[f64], plates: usize) -> Vec<f64> {
    let mut areas = vec![0.0; plates];
    for (&k, &omega) in plate.iter().zip(solid_angles) {
        areas[usize::from(k)] += omega;
    }
    areas
}

/// Whether a fit's assignment keeps the raster's parts as scored or joins each plate's cut-off
/// parts to their neighbours ([`join_parts`]), as the field's own does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parts {
    /// As scored: a coarser level's fit, a warm start for the next.
    AsScored,
    /// Joined: the field's level, whose plates are each one region.
    Joined,
}

/// A fit's state at one set of weights: the assignment, the areas and the misfit.
struct Fitted {
    /// The weights.
    weights: Vec<f64>,
    /// Each cell's plate.
    plate: Vec<u8>,
    /// Each plate's area, steradians.
    areas: Vec<f64>,
    /// Σ ((drawn − realised) ÷ tolerance)² over the plates.
    merit: f64,
    /// The largest |drawn − realised| ÷ tolerance: 1 or less when the fit holds.
    worst: f64,
}

impl Fitted {
    /// The assignment at `weights` on `raster`, its `parts` joined or not, against `targets` to
    /// `tolerance`.
    #[must_use]
    fn at(
        raster: &Raster<'_>,
        seeds: &[[f64; 3]],
        targets: &[f64],
        tolerance: &[f64],
        weights: Vec<f64>,
        parts: Parts,
    ) -> Self {
        assert!(
            weights.iter().all(|w| w.is_finite()),
            "a fit's weights are finite"
        );
        let mut plate = vec![0; raster.centres.len()];
        assign(raster, seeds, &weights, &mut plate);
        match parts {
            Parts::AsScored => {}
            Parts::Joined => join_parts(raster.grid, &mut plate, seeds.len()),
        }
        let areas = areas(&plate, raster.grid.solid_angles_sr(), seeds.len());
        let mut merit = 0.0;
        let mut worst: f64 = 0.0;
        for ((&t, &a), &tol) in targets.iter().zip(&areas).zip(tolerance) {
            let ratio = (t - a) / tol;
            merit += ratio * ratio;
            worst = num::max(worst, ratio.abs());
        }
        Self {
            weights,
            plate,
            areas,
            merit,
            worst,
        }
    }
}

/// The weights that make each plate's cells on `grid` (warped `centres`), its `parts` joined or
/// not, cover its target area to [`FIT_RELATIVE`] or [`FIT_CELLS`] cells, from `weights`, and the
/// assignment they give.
///
/// Damped Newton steps on the semi-discrete transport problem: the area of plate k changes with
/// the weight of its neighbour j at −`ℓ_kj` ÷ |∇(`s_k` − `s_j`)|, over their shared boundary's
/// length, and with its own at the sum of those, so each step solves that Laplacian, regularised,
/// for the areas' errors ([`newton_step`]), halving the step until the misfit,
/// Σ (error ÷ tolerance)², falls. The steps stop when every plate is within its tolerance, after
/// [`FIT_STEPS`] steps, or when no halving helps; then, while some plate is outside its
/// tolerance, up to [`FIT_SWEEPS`] sweeps move one plate's weight at a time where that lowers the
/// misfit, since the raster's areas change in steps of a cell.
#[must_use]
fn fit(
    grid: &CellGraph,
    centres: &[[f64; 3]],
    seeds: &[[f64; 3]],
    targets: &[f64],
    weights: Vec<f64>,
    parts: Parts,
) -> Fitted {
    let mean_cell = FOUR_PI / f64::from(grid.len());
    let tolerance: Vec<f64> = targets
        .iter()
        .map(|&t| num::max(FIT_RELATIVE * t, FIT_CELLS * mean_cell))
        .collect();
    let raster = Raster::new(grid, centres);
    let mut fitted = Fitted::at(&raster, seeds, targets, &tolerance, weights, parts);
    let mut sensitivity = Vec::new();
    for _ in 0..FIT_STEPS {
        if fitted.worst <= 1.0 {
            return fitted;
        }
        let (step, own) = newton_step(grid, centres, &fitted, seeds, targets);
        sensitivity = own;
        let mut size = 1.0;
        let mut improved = None;
        for _ in 0..=FIT_HALVINGS {
            let trial: Vec<f64> = fitted
                .weights
                .iter()
                .zip(&step)
                .map(|(&w, &dw)| w + size * dw)
                .collect();
            let candidate = Fitted::at(&raster, seeds, targets, &tolerance, trial, parts);
            if candidate.merit < fitted.merit {
                improved = Some(candidate);
                break;
            }
            size *= 0.5;
        }
        match improved {
            Some(next) => fitted = next,
            None => break,
        }
    }
    // Where the raster's steps leave a plate outside its tolerance, its own weight alone is moved
    // by its error over its own sensitivity, halved until the misfit falls, a plate at a time in
    // index order.
    for _ in 0..FIT_SWEEPS {
        if fitted.worst <= 1.0 || sensitivity.is_empty() {
            break;
        }
        for k in 0..seeds.len() {
            let error = targets[k] - fitted.areas[k];
            if error.abs() <= tolerance[k] {
                continue;
            }
            let mut size = error / sensitivity[k];
            for _ in 0..=FIT_HALVINGS {
                let mut trial = fitted.weights.clone();
                trial[k] += size;
                let candidate = Fitted::at(&raster, seeds, targets, &tolerance, trial, parts);
                if candidate.merit < fitted.merit {
                    fitted = candidate;
                    break;
                }
                size *= 0.5;
            }
        }
    }
    fitted
}

/// The Newton step of `fitted`'s weights towards `targets` on `grid`, whose warped centres are
/// `centres` (see [`fit`]), and each plate's own sensitivity, its area's change with its own
/// weight.
#[must_use]
fn newton_step(
    grid: &CellGraph,
    centres: &[[f64; 3]],
    fitted: &Fitted,
    seeds: &[[f64; 3]],
    targets: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let k = seeds.len();
    let mut laplacian = vec![0.0; k * k];
    for (n, &a) in fitted.plate.iter().enumerate() {
        let cell = cell_of(n);
        let x = grid.centre(cell);
        for m in grid.edge_neighbours(cell) {
            let b = fitted.plate[slot(m)];
            if b == a {
                continue;
            }
            let (a, b) = (usize::from(a), usize::from(b));
            let gradient = norm(tangent(sub(seeds[a], seeds[b]), x));
            if gradient <= 0.0 {
                continue;
            }
            // Each shared edge is met from both sides; each side books half its boundary.
            let coupling = 0.5 * BOUNDARY_PER_EDGE * grid.arc(cell, m).value() / gradient;
            laplacian[a * k + b] -= coupling;
            laplacian[b * k + a] -= coupling;
            laplacian[a * k + a] += coupling;
            laplacian[b * k + b] += coupling;
        }
    }
    // A plate with no cells is coupled, at a cap's 2π, to the plate that holds its seed's cell, so
    // that the system's one null mode is the constant, which the errors, summing to zero, do not
    // excite; its own step is set below.
    let mut empty: Vec<(usize, usize, f64)> = Vec::new();
    for (i, seed) in seeds.iter().enumerate() {
        if laplacian[i * k + i] > 0.0 {
            continue;
        }
        let home = PatchKey::containing(grid.level(), *seed)
            .map(cell_index)
            .expect("a coarse level is a patch level");
        let host = usize::from(fitted.plate[slot(home)]);
        let c = centres[slot(home)];
        let deficit =
            (dot(c, seeds[host]) + fitted.weights[host]) - (dot(c, seeds[i]) + fitted.weights[i]);
        empty.push((i, host, num::max(deficit, 0.0)));
        let coupling = core::f64::consts::TAU;
        laplacian[i * k + i] += coupling;
        if host != i {
            laplacian[i * k + host] -= coupling;
            laplacian[host * k + i] -= coupling;
            laplacian[host * k + host] += coupling;
        }
    }
    let largest = (0..k).map(|i| laplacian[i * k + i]).fold(0.0, num::max);
    let ridge = 1e-9 * num::max(largest, 1.0);
    for i in 0..k {
        laplacian[i * k + i] += ridge;
    }
    let own: Vec<f64> = (0..k).map(|i| laplacian[i * k + i]).collect();
    let mut step: Vec<f64> = targets
        .iter()
        .zip(&fitted.areas)
        .map(|(&t, &a)| t - a)
        .collect();
    solve_positive_definite(&mut laplacian, &mut step, k);
    // An empty plate's step makes it win its seed's cell over its host's new weight by the margin
    // of a cap of its area: near its seed a cap of area A about it scores within A ÷ 2π of 1.
    for (i, host, deficit) in empty {
        let host_step = if host == i { 0.0 } else { step[host] };
        step[i] = deficit + host_step + targets[i] / core::f64::consts::TAU;
    }
    (step, own)
}

/// Solves `matrix` x = `rhs` in place for a symmetric positive definite `n` × `n` `matrix` (row
/// major), by Cholesky's factorisation in index order; `rhs` becomes x.
///
/// # Panics
///
/// If a pivot is not positive, which a regularised Laplacian never gives.
fn solve_positive_definite(matrix: &mut [f64], rhs: &mut [f64], n: usize) {
    for j in 0..n {
        let mut pivot = matrix[j * n + j];
        for p in 0..j {
            pivot -= matrix[j * n + p] * matrix[j * n + p];
        }
        assert!(pivot > 0.0, "the fit's Laplacian is positive definite");
        let root = pivot.sqrt();
        matrix[j * n + j] = root;
        for i in j + 1..n {
            let mut value = matrix[i * n + j];
            for p in 0..j {
                value -= matrix[i * n + p] * matrix[j * n + p];
            }
            matrix[i * n + j] = value / root;
        }
    }
    for i in 0..n {
        let mut value = rhs[i];
        for p in 0..i {
            value -= matrix[i * n + p] * rhs[p];
        }
        rhs[i] = value / matrix[i * n + i];
    }
    for i in (0..n).rev() {
        let mut value = rhs[i];
        for p in i + 1..n {
            value -= matrix[p * n + i] * rhs[p];
        }
        rhs[i] = value / matrix[i * n + i];
    }
}

/// The weights of a cap of each plate's area about its seed, w = A ÷ 2π − 1: a cap of area A has
/// x · p ≥ 1 − A ÷ 2π, and so a score of zero on its rim.
#[must_use]
fn cap_weights(draws: &[PlateDraw]) -> Vec<f64> {
    draws
        .iter()
        .map(|d| d.area_sr / core::f64::consts::TAU - 1.0)
        .collect()
}

/// The fitted weights and assignment of `draws` on `grid` under `warp`, whose warped centres
/// are `centres`: fits at each level from [`FIT_LEVEL`] below the
/// field's, then the field's, and the cut-off parts joined ([`join_parts`]).
#[must_use]
fn realise(grid: &CellGraph, centres: &[[f64; 3]], draws: &[PlateDraw], warp: &Warp) -> Fitted {
    let seeds: Vec<[f64; 3]> = draws.iter().map(|d| d.seed).collect();
    let targets: Vec<f64> = draws.iter().map(|d| d.area_sr).collect();
    let mut weights = cap_weights(draws);
    for level in FIT_LEVEL..grid.level() {
        let coarse = CellGraph::new(level).expect("a fit's level is a coarse grid's");
        let coarse_centres = warp.apply(&coarse).plates;
        weights = fit(
            &coarse,
            &coarse_centres,
            &seeds,
            &targets,
            weights,
            Parts::AsScored,
        )
        .weights;
    }
    fit(grid, centres, &seeds, &targets, weights, Parts::Joined)
}

/// Joins every part of a plate cut off from its largest part to the neighbouring plate it shares
/// most edges with, ties to the lower plate, until each plate is one edge-connected region of
/// `grid`'s cells; `plate` is each cell's plate, of `plates` plates.
///
/// The raster's discreteness and the warp leave a few such parts, mostly single cells, which would
/// otherwise ring themselves with boundaries of their own. A plate's largest part is the one of
/// largest area, ties to the part with the lowest cell; parts are found in cell order and joined
/// in that order, and a pass that joins any is followed by another, at most [`JOIN_PASSES`].
fn join_parts(grid: &CellGraph, plate: &mut [u8], plates: usize) {
    for _ in 0..JOIN_PASSES {
        let (part, parts) = parts_of(grid, plate);
        let mut area = vec![0.0; parts.len()];
        for (n, &p) in part.iter().enumerate() {
            area[slot(p)] += grid.solid_angle_sr(cell_of(n));
        }
        let mut largest: Vec<Option<u32>> = vec![None; plates];
        for (p, &first) in parts.iter().enumerate() {
            let k = usize::from(plate[slot(first)]);
            let p = cell_of(p);
            if largest[k].is_none_or(|q| area[slot(p)] > area[slot(q)]) {
                largest[k] = Some(p);
            }
        }
        let mut cut_off: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (n, &p) in part.iter().enumerate() {
            let own = usize::from(plate[n]);
            if largest[own] != Some(p) {
                cut_off.entry(p).or_default().push(cell_of(n));
            }
        }
        if cut_off.is_empty() {
            return;
        }
        for cells in cut_off.values() {
            let own = plate[slot(cells[0])];
            let mut shared = vec![0_u32; plates];
            for &c in cells {
                for m in grid.edge_neighbours(c) {
                    let other = plate[slot(m)];
                    if other != own {
                        shared[usize::from(other)] += 1;
                    }
                }
            }
            let mut best: Option<(usize, u32)> = None;
            for (k, &count) in shared.iter().enumerate() {
                if count > 0 && best.is_none_or(|(_, c)| count > c) {
                    best = Some((k, count));
                }
            }
            if let Some((target, _)) = best {
                for &c in cells {
                    plate[slot(c)] = plate_byte(target);
                }
            }
        }
    }
}

/// Each cell's part, the edge-connected region of its plate's cells it lies in, numbered in the
/// order of their lowest cells, and each part's lowest cell.
#[must_use]
fn parts_of(grid: &CellGraph, plate: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let mut part = vec![u32::MAX; plate.len()];
    let mut firsts = Vec::new();
    let mut stack = Vec::new();
    for n in 0..plate.len() {
        if part[n] != u32::MAX {
            continue;
        }
        let id = cell_of(firsts.len());
        firsts.push(cell_of(n));
        part[n] = id;
        stack.push(cell_of(n));
        while let Some(c) = stack.pop() {
            for m in grid.edge_neighbours(c) {
                if part[slot(m)] == u32::MAX && plate[slot(m)] == plate[n] {
                    part[slot(m)] = id;
                    stack.push(m);
                }
            }
        }
    }
    (part, firsts)
}

/// Whether `crust` is oceanic: of the crusts a mobile lid has, `Oceanic`; the stagnant lid's two
/// are never written on one.
#[must_use]
const fn is_oceanic(crust: Crust) -> bool {
    match crust {
        Crust::Oceanic => true,
        Crust::Continental | Crust::Lid | Crust::Province => false,
    }
}

/// Each cell's crust: continental over `fraction` of the sphere's solid angle to half a cell, the
/// cells of least distance to a plate's continental nucleus over its size, |x′ − c| ÷ ρ, in
/// (score, cell index) order; oceanic elsewhere (see the module documentation, "The crust").
///
/// # Panics
///
/// If a score is not finite.
#[must_use]
fn continents(
    centres: &[[f64; 3]],
    solid_angles: &[f64],
    draws: &[PlateDraw],
    fraction: f64,
) -> Vec<Crust> {
    let mut crust = vec![Crust::Oceanic; centres.len()];
    if fraction <= 0.0 || draws.is_empty() {
        return crust;
    }
    let mut order: Vec<(f64, u32)> = centres
        .iter()
        .enumerate()
        .map(|(n, &c)| {
            let score = draws
                .iter()
                .map(|d| norm(sub(c, d.nucleus)) / d.nucleus_size)
                .fold(f64::INFINITY, num::min);
            (num::assert_finite(score), cell_of(n))
        })
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let total: f64 = solid_angles.iter().sum();
    let target = fraction * total;
    let mut covered = 0.0;
    for &(_, cell) in &order {
        let omega = solid_angles[slot(cell)];
        // A cell is taken while it brings the area nearer the target.
        if covered + 0.5 * omega >= target {
            break;
        }
        covered += omega;
        crust[slot(cell)] = Crust::Continental;
    }
    crust
}

/// How two plates move across a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Motion {
    /// Within 60° of head-on approach.
    Convergent,
    /// Within 60° of head-on opening.
    Divergent,
    /// Otherwise, or with no relative motion.
    Transform,
}

/// A boundary cell's nearest boundary: the other plate, the neighbour across it, the motion and
/// its rates.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Contact {
    /// The plate across the boundary.
    other: u8,
    /// The edge neighbour across it, the first in [`Edge::ALL`](hyperion_surface::cube::Edge)'s
    /// order on that plate.
    neighbour: u32,
    /// How the plates move across it.
    motion: Motion,
    /// The obliquity of the relative motion, 0 (head-on) to π ÷ 2 (strike-slip).
    obliquity_rad: f64,
    /// The rate at which this cell's plate approaches the other normal to the boundary, metres a
    /// second: negative where they open.
    closing_m_per_s: f64,
}

impl Contact {
    /// The half-rate of a divergent boundary, half the opening rate normal to it.
    #[must_use]
    fn half_rate_m_per_s(&self) -> f64 {
        -0.5 * self.closing_m_per_s
    }
}

/// A transform's result at one cell: the distance to its nearest source and that source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Reach {
    /// The shortest path's length over the region's cells, metres.
    pub(super) distance_m: f64,
    /// The source cell.
    pub(super) source: u32,
}

/// A distance transform's heap entry, ordered by (distance, cell, source).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    /// The path's length, metres, finite.
    distance_m: f64,
    /// The cell it reaches.
    cell: u32,
    /// The source it comes from.
    source: u32,
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance_m
            .total_cmp(&other.distance_m)
            .then(self.cell.cmp(&other.cell))
            .then(self.source.cmp(&other.source))
    }
}

/// Whether the reach (`d`, `source`) is shorter than `best`, ties to the lower source.
#[must_use]
fn shorter(d: f64, source: u32, best: Option<Reach>) -> bool {
    best.is_none_or(|b| {
        d.total_cmp(&b.distance_m)
            .then(source.cmp(&b.source))
            .is_lt()
    })
}

/// The multi-source shortest paths from `sources` (ascending cells) over each region of `region`
/// on `grid`, a region being the cells of one label (each plate's own cells, as this step calls
/// it), through edge and vertex neighbours with great-circle edge lengths on a sphere of
/// `radius_m`, settled in (distance, cell, source) order: each cell's nearest source in its own
/// region and the distance to it, or `None` where its region has none it reaches.
///
/// R09.T12.a reads it too, over each plate's oceanic cells, for the distance to a passive margin.
#[must_use]
pub(super) fn reach<L: Copy + PartialEq>(
    grid: &CellGraph,
    region: &[L],
    sources: &[u32],
    radius_m: f64,
) -> Vec<Option<Reach>> {
    let mut best: Vec<Option<Reach>> = vec![None; region.len()];
    let mut heap = BinaryHeap::new();
    for &s in sources {
        best[slot(s)] = Some(Reach {
            distance_m: 0.0,
            source: s,
        });
        heap.push(Reverse(Entry {
            distance_m: 0.0,
            cell: s,
            source: s,
        }));
    }
    while let Some(Reverse(entry)) = heap.pop() {
        let current = best[slot(entry.cell)].is_some_and(|r| {
            r.source == entry.source && r.distance_m.total_cmp(&entry.distance_m).is_eq()
        });
        if !current {
            continue;
        }
        let own = region[slot(entry.cell)];
        let neighbours = grid
            .edge_neighbours(entry.cell)
            .into_iter()
            .chain(grid.vertex_neighbours(entry.cell));
        for m in neighbours {
            if region[slot(m)] != own {
                continue;
            }
            let d =
                num::assert_finite(entry.distance_m + grid.arc(entry.cell, m).value() * radius_m);
            if shorter(d, entry.source, best[slot(m)]) {
                best[slot(m)] = Some(Reach {
                    distance_m: d,
                    source: entry.source,
                });
                heap.push(Reverse(Entry {
                    distance_m: d,
                    cell: m,
                    source: entry.source,
                }));
            }
        }
    }
    best
}

/// A boundary cell's boundary as the field records it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Boundary {
    /// Its kind.
    kind: BoundaryKind,
    /// Whether this cell's plate subducts beneath it.
    subducts: bool,
    /// The relative motion's obliquity, radians.
    obliquity_rad: f64,
}

/// The plates of one pass laid out on its cells: what [`run`] writes.
#[derive(Debug, Clone, PartialEq)]
struct Layout {
    /// The plates as realised.
    plates: Vec<Plate>,
    /// Each cell's plate.
    plate: Vec<u8>,
    /// Each cell's crust.
    crust: Vec<Crust>,
    /// Each boundary cell's nearest boundary, `None` inside a plate.
    contact: Vec<Option<Contact>>,
    /// Each boundary cell's boundary as recorded.
    boundary: Vec<Option<Boundary>>,
    /// Each cell's nearest boundary cell on its plate.
    nearest: Vec<Option<Reach>>,
    /// Each cell's nearest ridge cell on its plate.
    ridge: Vec<Option<Reach>>,
}

impl Layout {
    /// The layout of `pass`'s body, or `None` where it has no plates (see the module
    /// documentation).
    #[must_use]
    fn of(pass: &Pass<'_>) -> Option<Self> {
        let inputs = pass.inputs();
        let [continental, oceanic, _, _] = inputs.crust_palette();
        if !has_plates(inputs.tectonics()) || oceanic.is_none() {
            return None;
        }
        let grid = pass.grid();
        let draws = draw_plates(
            pass.seed(),
            grid.len(),
            angular_speed_scale_rad_per_s(inputs.heat_flow()),
        );
        let warp = Warp::draw(pass.seed());
        let warped = warp.apply(grid);
        let fitted = realise(grid, &warped.plates, &draws, &warp);
        let fraction = if continental.is_some() {
            inputs.continental_fraction()
        } else {
            0.0
        };
        let crust = continents(&warped.continents, grid.solid_angles_sr(), &draws, fraction);
        Some(Self::new(
            grid,
            inputs.radius().value(),
            &warped.plates,
            &draws,
            fitted,
            crust,
        ))
    }

    /// The layout of `draws` realised as `fitted` on `grid` (warped `centres`) with `crust`, on a
    /// sphere of `radius_m`.
    #[must_use]
    fn new(
        grid: &CellGraph,
        radius_m: f64,
        centres: &[[f64; 3]],
        draws: &[PlateDraw],
        fitted: Fitted,
        crust: Vec<Crust>,
    ) -> Self {
        let Fitted { plate, weights, .. } = fitted;
        let contact: Vec<Option<Contact>> = (0..grid.len())
            .map(|cell| contact(grid, radius_m, centres, draws, &weights, &plate, cell))
            .collect();
        let ridges: Vec<u32> = (0..grid.len())
            .filter(|&c| contact[slot(c)].is_some_and(|k| k.motion == Motion::Divergent))
            .collect();
        let ridge = reach(grid, &plate, &ridges, radius_m);
        let ages: Vec<f64> = (0..plate.len())
            .map(|n| seafloor(&contact, &ridge, &crust, n).map_or(0.0, |o| o.age.value()))
            .collect();
        let older = older_sides(&plate, &crust, &contact, &ages);
        let boundary: Vec<Option<Boundary>> = (0..plate.len())
            .map(|n| {
                contact[n].map(|k| {
                    let (kind, subducts) = match k.motion {
                        Motion::Divergent => (BoundaryKind::Divergent, false),
                        Motion::Transform => (BoundaryKind::Transform, false),
                        Motion::Convergent => {
                            let here = crust[n];
                            let there = crust[slot(k.neighbour)];
                            match (is_oceanic(here), is_oceanic(there)) {
                                (false, false) => (BoundaryKind::Collision, false),
                                (true, false) => (BoundaryKind::Subduction, true),
                                (false, true) => (BoundaryKind::Subduction, false),
                                (true, true) => {
                                    let pair = (plate[n].min(k.other), plate[n].max(k.other));
                                    let lower_subducts = *older
                                        .get(&pair)
                                        .expect("older_sides holds every oceanic convergent pair");
                                    (
                                        BoundaryKind::Subduction,
                                        (plate[n] == pair.0) == lower_subducts,
                                    )
                                }
                            }
                        }
                    };
                    Boundary {
                        kind,
                        subducts,
                        obliquity_rad: k.obliquity_rad,
                    }
                })
            })
            .collect();
        let sources: Vec<u32> = (0..grid.len())
            .filter(|&c| boundary[slot(c)].is_some())
            .collect();
        let nearest = reach(grid, &plate, &sources, radius_m);
        let realised = areas(&plate, grid.solid_angles_sr(), draws.len());
        let plates = draws
            .iter()
            .zip(&weights)
            .zip(realised)
            .map(|((d, &weight), area_sr)| Plate {
                seed: d.seed,
                angular_velocity_rad_per_s: d.angular_velocity_rad_per_s,
                drawn_area_sr: d.area_sr,
                area_sr,
                weight,
            })
            .collect();
        Self {
            plates,
            plate,
            crust,
            contact,
            boundary,
            nearest,
            ridge,
        }
    }

    /// The sea floor's origin of cell `n`, on oceanic crust (see [`seafloor`]).
    #[must_use]
    fn seafloor(&self, n: usize) -> Option<SeafloorOrigin> {
        seafloor(&self.contact, &self.ridge, &self.crust, n)
    }

    /// Writes the layout into `working`'s cells and plates.
    fn write(self, working: &mut Working) {
        for (n, state) in working.cells.iter_mut().enumerate() {
            state.plate = self.plate[n];
            state.crust = self.crust[n];
            if let Some(reach) = self.nearest[n] {
                let boundary = self.boundary[slot(reach.source)]
                    .expect("a transform's source is a boundary cell");
                let d = reach.distance_m;
                state.boundary = boundary.kind;
                state.boundary_distance = Some(Metres::new(if boundary.subducts && d > 0.0 {
                    -d
                } else {
                    d
                }));
                state.boundary_obliquity = Radians::new(boundary.obliquity_rad);
            }
            state.seafloor_origin = self.seafloor(n);
            if is_oceanic(self.crust[n]) {
                state.hill_relief = Some(H_REF);
            }
        }
        working.plates = self.plates;
    }
}

/// The sea floor's origin of cell `n` from the ridge transform `ridge` and the boundary cells'
/// `contact`s, on oceanic crust: its source ridge's half-rate and its distance over it, capped at
/// [`OLDEST_FLOOR`]; the mean half-rate and the cap where its plate's ridges do not reach it.
#[must_use]
fn seafloor(
    contact: &[Option<Contact>],
    ridge: &[Option<Reach>],
    crust: &[Crust],
    n: usize,
) -> Option<SeafloorOrigin> {
    if !is_oceanic(crust[n]) {
        return None;
    }
    let Some(reach) = ridge[n] else {
        return Some(SeafloorOrigin {
            ridge_distance: None,
            half_rate: MEAN_HALF_RATE,
            age: OLDEST_FLOOR,
        });
    };
    let half_rate = contact[slot(reach.source)]
        .expect("a ridge transform's source is a boundary cell")
        .half_rate_m_per_s();
    let age = num::min(reach.distance_m / half_rate, OLDEST_FLOOR.value());
    Some(SeafloorOrigin {
        ridge_distance: Some(Metres::new(reach.distance_m)),
        half_rate: MetresPerSecond::new(half_rate),
        age: Seconds::new(age),
    })
}

/// For each pair of plates (lower, higher) that converge with oceanic floor on both sides of some
/// boundary cell, whether the lower-indexed plate subducts: the side whose floor is older on
/// average over those cells and their neighbours across, the higher index at a tie.
#[must_use]
fn older_sides(
    plate: &[u8],
    crust: &[Crust],
    contact: &[Option<Contact>],
    ages: &[f64],
) -> BTreeMap<(u8, u8), bool> {
    let mut sums: BTreeMap<(u8, u8), [(f64, u32); 2]> = BTreeMap::new();
    for (n, k) in contact.iter().enumerate() {
        let Some(k) = k else { continue };
        let m = slot(k.neighbour);
        if k.motion != Motion::Convergent || !is_oceanic(crust[n]) || !is_oceanic(crust[m]) {
            continue;
        }
        let (a, b) = (plate[n], k.other);
        let pair = (a.min(b), a.max(b));
        let entry = sums.entry(pair).or_insert([(0.0, 0); 2]);
        let (own, across) = if a == pair.0 { (0, 1) } else { (1, 0) };
        entry[own].0 += ages[n];
        entry[own].1 += 1;
        entry[across].0 += ages[m];
        entry[across].1 += 1;
    }
    sums.into_iter()
        .map(|(pair, [(lower, nl), (higher, nh)])| {
            let lower_mean = lower / f64::from(nl.max(1));
            let higher_mean = higher / f64::from(nh.max(1));
            (pair, lower_mean > higher_mean)
        })
        .collect()
}

/// Cell `cell`'s nearest boundary, or `None` inside its plate: of the plates across its edges, the
/// one whose boundary is nearest by the score difference f over its gradient, ties to the lower
/// plate; the normal from f's gradient by least squares over the four edge neighbours, and the
/// relative motion against it (see the module documentation, "The boundaries").
#[must_use]
fn contact(
    grid: &CellGraph,
    radius_m: f64,
    centres: &[[f64; 3]],
    draws: &[PlateDraw],
    weights: &[f64],
    plate: &[u8],
    cell: u32,
) -> Option<Contact> {
    let own = plate[slot(cell)];
    let edges = grid.edge_neighbours(cell);
    let centre = grid.centre(cell);
    let warped = centres[slot(cell)];
    let mut best: Option<(f64, u8, u32, [f64; 3])> = None;
    for &across in &edges {
        let other = plate[slot(across)];
        if other == own || best.is_some_and(|(_, seen, _, _)| seen == other) {
            continue;
        }
        let (here, there) = (usize::from(own), usize::from(other));
        let seeds_apart = sub(draws[here].seed, draws[there].seed);
        let difference = dot(warped, seeds_apart) + (weights[here] - weights[there]);
        let gradient = score_gradient(grid, centres, &edges, centre, warped, seeds_apart);
        let size = norm(gradient);
        let (estimate, normal) = if size > 0.0 {
            (
                num::max(difference, 0.0) / size,
                scale(gradient, -1.0 / size),
            )
        } else {
            (0.0, unit(tangent(grid.centre(across), centre)))
        };
        let estimate = num::assert_finite(estimate);
        let nearer = |(e, seen, _, _): (f64, u8, u32, [f64; 3])| {
            estimate.total_cmp(&e).then(other.cmp(&seen)).is_lt()
        };
        if best.is_none_or(nearer) {
            best = Some((estimate, other, across, normal));
        }
    }
    let (_, other, neighbour, normal) = best?;
    let relative = scale(
        cross(
            sub(
                draws[usize::from(own)].angular_velocity_rad_per_s,
                draws[usize::from(other)].angular_velocity_rad_per_s,
            ),
            centre,
        ),
        radius_m,
    );
    let closing = dot(relative, normal);
    let sliding = norm(sub(relative, scale(normal, closing)));
    // Exactly no relative motion, the one case atan2 cannot class (atan2(0, 0) is 0): plates
    // that do not move meet at a transform.
    let (motion, obliquity) = if sliding <= 0.0 && closing.abs() <= 0.0 {
        (Motion::Transform, core::f64::consts::FRAC_PI_2)
    } else {
        let direction = math::atan2(sliding, closing);
        // An opening whose half rounds to zero (a subnormal one, from a heat flow below some
        // 10⁻¹⁵⁰ W m⁻²) is no ridge: its floor would have no age.
        let motion = if direction < CONVERGENT_BELOW {
            Motion::Convergent
        } else if direction > DIVERGENT_ABOVE && -0.5 * closing > 0.0 {
            Motion::Divergent
        } else {
            Motion::Transform
        };
        (motion, math::atan2(sliding, closing.abs()))
    };
    Some(Contact {
        other,
        neighbour,
        motion,
        obliquity_rad: obliquity,
        closing_m_per_s: closing,
    })
}

/// The gradient at the cell centred at `x` (warped `warped`) of the score difference x′ · `d`, a
/// vector tangent to the unit sphere: the least-squares fit of the differences to its four
/// `edges` neighbours over their offsets in the tangent plane.
#[must_use]
fn score_gradient(
    grid: &CellGraph,
    centres: &[[f64; 3]],
    edges: &[u32; 4],
    x: [f64; 3],
    warped: [f64; 3],
    d: [f64; 3],
) -> [f64; 3] {
    let offsets = edges.map(|m| tangent(grid.centre(m), x));
    let e1 = unit(sub(offsets[1], offsets[0]));
    let e2 = cross(x, e1);
    let (mut saa, mut sab, mut sbb, mut saf, mut sbf) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (&m, offset) in edges.iter().zip(offsets) {
        let df = dot(sub(centres[slot(m)], warped), d);
        let (p, q) = (dot(offset, e1), dot(offset, e2));
        saa += p * p;
        sab += p * q;
        sbb += q * q;
        saf += p * df;
        sbf += q * df;
    }
    let det = saa * sbb - sab * sab;
    let g1 = (saf * sbb - sbf * sab) / det;
    let g2 = (sbf * saa - saf * sab) / det;
    [
        g1 * e1[0] + g2 * e2[0],
        g1 * e1[1] + g2 * e2[1],
        g1 * e1[2] + g2 * e2[2],
    ]
}

#[cfg(test)]
mod tests;
