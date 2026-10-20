//! Step 3, the coarse craters (R09.T12.d, Design notes 5, 10, 12 and 13).
//!
//! Craters of the boundary diameter `D_b` and wider, drawn on `surface.coarse.crater` from the
//! body's crater contract, smoothed into the elevation and listed with their reach, as main's
//! rulings on R09.T7.c and R09.T8 have them (R09's Risks, "For T12.d (its contract)"):
//!
//! - **The count.** Each coarse octave [`D_b` 2^k, `D_b` 2^(k+1)] on its own draws a Poisson count
//!   of mean min(production, saturation) over the body's area: the production between its
//!   diameters by [`cumulative_density`] (through the diameter map and the screening), capped at
//!   Trask's saturation over the same octave, 0.079 (D⁻² − (2D)⁻²) km⁻² ([`saturation`]; Design note
//!   10), so that the rims' term of the level bound holds (a cap on N(>D), or on another grid,
//!   could exceed it by up to 1.24 in the octave that straddles the crossover). The body's area is
//!   its spheroid's, in closed form (`spheroid_area_m2`). The octaves run
//!   while their lower diameter is below the body's radius (labelled: no wider crater is drawn).
//! - **Each crater.** Its centre uniform on the unit sphere of directions (z, then the longitude),
//!   its diameter within the octave by the body's size–frequency distribution
//!   ([`diameter_in_octave`]), its age uniform in the cumulative production over its octave's
//!   window τ(D) (the surface age where production is under saturation, the youngest share
//!   `N_sat` ÷ `N_prod` of it where capped), through [`chronology_age`] at the contract's system age
//!   (Design note 12), as the synthesis's small craters draw theirs; its morphology its fresh
//!   [`crater_shape`]'s.
//! - **Its wear.** Its degradation byte first, the nearest integer to 255 × (1 − g), at most 255,
//!   g the small craters' relief factor of its rim radius and its wear in its octave
//!   ([`relief_factor`] of [`CraterWear::of_age`]; `decision-r09-t12-coarse.md` item 5b, R09.T7.j),
//!   one law with the small craters: Minton et al. 2019's eq. 8 with K ÷ R² the larger of Fassett
//!   and Thomson 2014's diffusion, κt ÷ R² (5.5 m² Myr⁻¹), and the equilibrium closure, `K_v` q,
//!   q the octave's production since the crater formed over its saturation ([`OctaveWear`]).
//!   Diffusion alone would keep at least 99.95% of every coarse crater's relief on the Moon; the
//!   closure wears a saturated octave's craters uniformly in q from fresh to the visibility limit,
//!   (15 ÷ 32)² of the fresh relief (bytes 0 to 199, E\[g²\] 0.2638), and an unsaturated octave's
//!   to its production's share of its saturation (on the reference Mars, P ÷ S about 0.27 at its
//!   first coarse octave, bytes to about 105). Infill, relaxation and erosion are not modelled.
//! - **Its relief.** Smoothed in as its kept relief ([`kept_relief`], the byte's) times its coarse
//!   profile ([`CoarseRims::coarse_profile`]: its fresh shape filleted over the coarse band), at
//!   the chord from its centre's spheroid point in rim radii, sampled at each cell's centre, so
//!   that the coarse elevation and the client's sharpening are one profile to the bit (ruling B).
//! - **Its reach.** Every cell that has a point within the chord (1 + a) R of the centre's
//!   spheroid point: the cells whose centre lies within that chord's angle on the smallest radius,
//!   2 asin(reach ÷ (2 min(a, c))), of the centre's direction plus the cell's circumradius, and the
//!   centre's own cell, as the testing builder takes them; a cover of `ResolutionCode::NONE`.
//! - **The list.** No two craters share a centre cell and diameter, the field's key: a later draw
//!   of a key already taken is dropped (never met in practice, the diameters being continuous).
//!   The quantiser sorts the list.
//!
//! **The wet-epoch split** (Design note 5): on a world whose wet epoch ended before its surface age
//! ran out (the reference Mars: surface 3.9 Ga, epoch 3.9–3.6 Ga), [`run`] smooths in only the
//! craters older than the epoch's end, and [`run_after_erosion`], which the pass calls after the
//! erosion step, smooths in the younger, so that "the craters of the rest of the surface age placed
//! on top of it" holds. Every crater is in [`Working::craters`] from the first.
//!
//! # The draws
//!
//! `surface.coarse.crater`, octave k on `surface_item(k)`: its count, a Poisson draw from word 0,
//! and crater j's four words from word 2³² + 8j: the centre's z and longitude, its diameter's
//! uniform and its age's uniform.

use hyperion_surface::craters::{
    CraterParams, DiameterOctave, UnitUniform, chronology_age, crater_shape, cumulative_density,
    diameter_in_octave, saturation,
};
use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{CoarseCrater, Cover, boundary_diameter, cell_index};
use hyperion_surface::num;
use hyperion_surface::synth::craters::rims::{CoarseRims, kept_relief};
use hyperion_surface::synth::craters::{CraterWear, OctaveWear, relief_factor};
use hyperion_surface::tags::SURFACE_COARSE_CRATER;

use super::plates::slot;
use super::{Pass, Working};
use crate::math;
use crate::rng::ObjectKey;
use crate::units::{Gigayears, Metres, PerSquareKilometre};

/// The word of an octave's stream from which its craters' words begin, 2³²: above every word its
/// Poisson count draws.
pub(crate) const CRATER_WORDS: u64 = 1 << 32;

/// The words a crater's stream reserves, 8, of which it reads four.
pub(crate) const WORDS_PER_CRATER: u64 = 8;

/// The coarse octaves at most, 24: no body's octaves reach its radius past it from its `D_b`.
const MAX_OCTAVES: u32 = 24;

/// One coarse octave as the step draws it: its diameters, its expected count over the body and
/// its craters' window of ages and wear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CoarseOctave {
    /// Its index k, from 0 at `D_b`.
    pub(crate) index: u32,
    /// Its diameters, `D_b` 2^k to twice that.
    pub(crate) diameters: DiameterOctave,
    /// Its production between its diameters, per km².
    pub(crate) production: PerSquareKilometre,
    /// Trask's saturation between them, per km².
    pub(crate) saturation: PerSquareKilometre,
    /// Its expected count over the body, min(production, saturation) × the body's area.
    pub(crate) expected: f64,
    /// Its window of ages and its craters' wear ([`OctaveWear`]): the lunar-equivalent density at
    /// the start of its age window (the surface's, or its youngest share `N_sat` ÷ `N_prod` of it
    /// where saturation caps it), and each crater's share of its saturation (R09.T7.j).
    pub(crate) wear: OctaveWear,
}

/// The coarse octaves of the body of `pass` (see the module documentation).
#[must_use]
pub(crate) fn octaves(pass: &Pass<'_>) -> Vec<CoarseOctave> {
    let inputs = pass.inputs();
    let params = inputs.craters();
    let area_km2 = spheroid_area_m2(&inputs.figure()) * 1e-6;
    let d_b = boundary_diameter(pass.level(), inputs.radius()).value();
    let radius = inputs.radius().value();
    let mut list = Vec::new();
    for k in 0..MAX_OCTAVES {
        let lower = d_b * math::powi(2.0, i32::try_from(k).expect("an octave index fits an i32"));
        if lower >= radius {
            break;
        }
        let diameters =
            DiameterOctave::new(Metres::new(lower)).expect("a coarse octave is a valid octave");
        let (low, high) = (diameters.lower(), diameters.upper());
        let production =
            cumulative_density(params, low).value() - cumulative_density(params, high).value();
        let capped = saturation(low).value() - saturation(high).value();
        let (production, capped) = (
            PerSquareKilometre::new(production),
            PerSquareKilometre::new(capped),
        );
        list.push(CoarseOctave {
            index: k,
            diameters,
            production,
            saturation: capped,
            expected: num::min(production.value(), capped.value()) * area_km2,
            wear: OctaveWear::new(
                inputs.surface_age(),
                params.system_age(),
                production,
                capped,
            ),
        });
    }
    list
}

/// The surface area of `figure`, square metres: 4πa² on a sphere, and on an oblate spheroid of
/// eccentricity e = √(1 − c² ÷ a²), 2πa² (1 + ((1 − e²) ÷ e) atanh e), its closed form (an
/// eccentricity under 10⁻⁶ is taken as a sphere's).
#[must_use]
pub(crate) fn spheroid_area_m2(figure: &hyperion_surface::spheroid::Spheroid) -> f64 {
    let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let ratio = num::min(c / a, 1.0);
    let e = (1.0 - ratio * ratio).sqrt();
    let full = 4.0 * core::f64::consts::PI * a * a;
    if e < 1e-6 {
        return full;
    }
    0.5 * full * (1.0 + (1.0 - e * e) / e * math::atanh(e))
}

/// The degradation byte of a coarse crater of `diameter` `age` old in an octave of wear `octave`:
/// the nearest integer to 255 × (1 − g), at most 255, g the relief factor of its rim radius and
/// its wear ([`relief_factor`] of [`CraterWear::of_age`]; `decision-r09-t12-coarse.md` item 5b,
/// R09.T7.j), the small craters' law.
#[must_use]
pub(crate) fn degradation(diameter: Metres, age: Gigayears, octave: &OctaveWear) -> u8 {
    let g = relief_factor(diameter * 0.5, CraterWear::of_age(age, octave));
    let lost = (255.0 * (1.0 - g)).round().clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rounded value clamped to 0–255"
    )]
    {
        lost as u8
    }
}

/// Whether a crater `age` old waits for the erosion step on the body of `pass`: where its wet epoch
/// ended before its surface age ran out, and the crater is no older than the epoch's end (Design
/// note 5).
#[must_use]
pub(crate) fn waits(pass: &Pass<'_>, age: Gigayears) -> bool {
    let inputs = pass.inputs();
    inputs.wet_epoch().is_some_and(|epoch| {
        let end = epoch.end.value();
        end > 0.0 && inputs.surface_age().value() > end && age.value() <= end
    })
}

/// Runs the step on `working`: draws every coarse crater into [`Working::craters`] in draw order,
/// and smooths into the elevation those that do not wait for the erosion (see the module
/// documentation), keeping the interim sea level and realised figures true.
///
/// # Panics
///
/// Never on valid inputs: every crater is a valid octave's, and every value finite.
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    let craters = draw(pass);
    working.craters = craters;
    apply(pass, working, Phase::BeforeErosion);
}

/// Smooths into the elevation the coarse craters that waited for the erosion step (Design note 5),
/// in draw order, at the degradation bytes R09.T12.e set: none on a world with no wet epoch that
/// ended before its surface age ran out. Where it smooths any in, it refloods the ocean against
/// the sea level as placed, to which the climate is reduced, and recomputes the realised figures
/// with every crater in (R09.T12.e; main's rulings R4 and R8, 2026-10-19), with no rescale: that
/// is R09.T14.d's.
///
/// # Panics
///
/// As [`run`].
pub fn run_after_erosion(pass: &Pass<'_>, working: &mut Working) {
    apply(pass, working, Phase::AfterErosion);
}

/// When a crater is smoothed in (Design note 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Before the erosion step: the craters that do not wait.
    BeforeErosion,
    /// After it: the craters younger than a wet epoch's end that ended before the surface age ran
    /// out.
    AfterErosion,
}

/// Whether a crater `age` old is in the elevation of the body of `pass` once `phase`'s craters are
/// smoothed in: before the erosion, every crater that does not wait for it; after it, every one.
#[must_use]
pub(crate) fn smoothed_in_by(pass: &Pass<'_>, age: Gigayears, phase: Phase) -> bool {
    match phase {
        Phase::BeforeErosion => !waits(pass, age),
        Phase::AfterErosion => true,
    }
}

/// Each cell of `grid` that `crater` reaches, in its reach's ascending order, handed to `emit` with
/// the crater's lift there: its [`kept_relief`] times `rims`' coarse profile of its diameter, at
/// the chord in rim radii from the crater centre's spheroid point on `figure` to the cell
/// centre's, sampled at the cell's centre (R09.T12.d's ruling B, the one profile). The one
/// expression for a coarse crater's lift, which the step's smoothing and R09.T12.e's budget share.
pub(crate) fn each_lift(
    grid: &crate::planetary::surface::grid::CellGraph,
    figure: &hyperion_surface::spheroid::Spheroid,
    rims: &CoarseRims,
    crater: &CoarseCrater,
    mut emit: impl FnMut(u32, f64),
) {
    let profile = rims.coarse_profile(crater.diameter);
    let kept = kept_relief(crater);
    let centre = figure.point(crater.centre);
    let rim = 0.5 * crater.diameter.value();
    for cell in crater.reach.cells() {
        let p = figure.point(grid.centre(cell));
        let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
        let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
        emit(cell, kept * profile.at(chord / rim).height_m);
    }
}

/// Smooths into `working`'s elevation its craters of `phase`, in list order.
fn apply(pass: &Pass<'_>, working: &mut Working, phase: Phase) {
    let inputs = pass.inputs();
    let figure = inputs.figure();
    let rims = CoarseRims::for_body(*inputs.craters(), figure, pass.level());
    let grid = pass.grid();
    let mut touched = false;
    let Working { cells, craters, .. } = working;
    for crater in craters.iter() {
        let waiting = waits(pass, crater.age);
        let now = match phase {
            Phase::BeforeErosion => !waiting,
            Phase::AfterErosion => waiting,
        };
        if !now {
            continue;
        }
        touched = true;
        each_lift(grid, &figure, &rims, crater, |cell, lift| {
            let state = &mut cells[slot(cell)];
            let z = Metres::new(num::assert_finite(state.elevation.value() + lift));
            state.elevation = z;
            state.water_surface = z;
        });
    }
    if !touched {
        return;
    }
    match phase {
        // R09.T12.d's staged figures, which R09.T12.e's second entry of the relief step replaces.
        Phase::BeforeErosion => super::relief::interim_figures(pass, working),
        Phase::AfterErosion => super::relief::after_late_craters(pass, working),
    }
}

/// The coarse craters of the body of `pass`, in draw order (see the module documentation).
#[must_use]
fn draw(pass: &Pass<'_>) -> Vec<CoarseCrater> {
    let inputs = pass.inputs();
    let params: &CraterParams = inputs.craters();
    let figure = inputs.figure();
    let grid = pass.grid();
    let level = pass.level().get();
    let smallest = num::min(figure.equatorial_radius_m, figure.polar_radius_m);
    let mut craters: Vec<CoarseCrater> = Vec::new();
    let mut keys: Vec<(u32, f64)> = Vec::new();
    for octave in octaves(pass) {
        let mut stream = pass.seed().stream(
            SURFACE_COARSE_CRATER,
            ObjectKey::surface_item(u64::from(octave.index)),
        );
        let count = stream.poisson(octave.expected);
        for j in 0..count {
            stream.seek(CRATER_WORDS + WORDS_PER_CRATER * j);
            let z = 2.0 * stream.uniform() - 1.0;
            let longitude = core::f64::consts::TAU * stream.uniform();
            let (sin, cos) = math::sin_cos(longitude);
            let r = (1.0 - z * z).sqrt();
            let centre = [r * cos, r * sin, z];
            let size = UnitUniform::new(stream.uniform()).expect("a uniform is in [0, 1)");
            let age_share = stream.uniform();
            let diameter = diameter_in_octave(params, octave.diameters, size);
            let age = chronology_age(
                octave.wear.window_density() * age_share,
                params.system_age(),
            );
            let shape = crater_shape(params, diameter);
            let home = cell_index(
                PatchKey::containing(level, centre).expect("a coarse level is a patch level"),
            );
            let key = (home, num::assert_finite(diameter.value()));
            if keys
                .iter()
                .any(|&(c, d)| c == key.0 && d.total_cmp(&key.1).is_eq())
            {
                continue;
            }
            keys.push(key);
            let reach = shape.reach().value();
            let reach_angle = 2.0 * math::asin(num::min(0.5 * reach / smallest, 1.0));
            let cells = reach_cells(grid, centre, home, reach_angle);
            craters.push(CoarseCrater {
                centre,
                diameter,
                morphology: shape.morphology(),
                age,
                degradation: degradation(diameter, age, &octave.wear),
                reach: Cover::from_cells(cells),
            });
        }
    }
    craters
}

/// The circumradius on the unit sphere of `grid`'s cell `cell`, radians: the largest angle from
/// its centre to its corners, which a cell's great-circle edges make its farthest points.
#[must_use]
fn circumradius(grid: &crate::planetary::surface::grid::CellGraph, cell: u32) -> f64 {
    let key = grid.cell(cell);
    let centre = grid.centre(cell);
    [(0, 0), (64, 0), (0, 64), (64, 64)]
        .map(|(x, y)| angle_between(key.vertex_dir(x, y), centre))
        .into_iter()
        .fold(0.0, num::max)
}

/// The angle between the unit vectors `a` and `b`, radians, from their chord.
#[must_use]
fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let gap = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
    2.0 * math::asin(num::min(0.5 * chord, 1.0))
}

/// The cells of `grid` that have a point within `reach_angle` of the unit direction `centre`
/// (the module documentation): flooded through edge neighbours from `home`, the centre's cell, each
/// taken where its centre's angle less its circumradius is within the reach; ascending.
#[must_use]
fn reach_cells(
    grid: &crate::planetary::surface::grid::CellGraph,
    centre: [f64; 3],
    home: u32,
    reach_angle: f64,
) -> Vec<u32> {
    let mut taken = vec![home];
    let mut seen = std::collections::BTreeSet::from([home]);
    let mut frontier = vec![home];
    while let Some(c) = frontier.pop() {
        for m in grid.edge_neighbours(c) {
            if !seen.insert(m) {
                continue;
            }
            if angle_between(grid.centre(m), centre) - circumradius(grid, m) <= reach_angle {
                taken.push(m);
                frontier.push(m);
            }
        }
    }
    taken.sort_unstable();
    taken
}

#[cfg(test)]
pub(crate) mod tests;
