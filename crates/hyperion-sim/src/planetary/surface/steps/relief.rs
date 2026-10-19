//! Step 2, the coarse elevation (R09.T12.a–c and e, Design note 7).
//!
//! The crust's populations about sea level with GDH1's age–depth law at the ridges, the
//! convergent landforms, a stagnant lid's volcanic provinces with their flexure, and the affine
//! scaling that makes the reconstructed field's `σ_h` plan 14's, with sea level at the
//! area-weighted quantile that leaves plan 14's ocean fraction below it.
//!
//! # As built
//!
//! R09.T12.a writes the crust's populations, about the datum, which stands for sea level until
//! R09.T12.e places it:
//!
//! - **On a mobile lid** (R09.T11 laid out plates), the continental population is a core of
//!   width [`CONTINENTAL_CORE_WIDTH`] about [`CONTINENTAL_CORE_MEAN`], skewed as a log-normal of
//!   shape [`CONTINENTAL_SKEW`] so that its mode is Earth2014's +125 m, which with R09.T12.b's
//!   convergent landforms on it gives the continental crust Earth2014's mean [`CONTINENTAL_MEAN`]
//!   and width above −2 km; and the
//!   oceanic crust is the sea floor of [`seafloor`]: GDH1's basement at the floor's steady-state
//!   age, lifted by its sediment's isostatic load, with the oceanic noise, whose RMS makes the
//!   oceanic s.d. plan 14's [`OCEANIC_WIDTH`] with GDH1's own spread. Each oceanic cell also gets
//!   its steady-state age and its sea floor's roughness inputs, the draped hill relief and the
//!   ponded sediment, which the quantiser packs into its `seafloor` byte (`decision-r09-t5.md`
//!   item 5).
//! - **On a stagnant lid** (every other regime, and a mobile lid R09.T11 could not lay out), the
//!   one lid is one population about the datum, of plan 14's width [`STAGNANT_LID_RELIEF`] (Design
//!   note 3's structural share on such a lid, Venus's), Gaussian, which no other step realises: a
//!   red field, its degree variance ∝ l^−β₁ from degree 2 with no degree-1 term, β₁ its crust
//!   class's ([`crust_shape`]), its area mean zero and its area RMS the constant (ruled:
//!   `decision-r09-t12-coarse.md`'s addendum of 2026-10-16).
//!
//! R09.T12.b adds a mobile lid's convergent landforms ([`convergent`]): collisional belts on
//! continental crust, trenches on a subduction's subducting side, and arcs with their volcanic
//! fronts behind it, by the overriding crust, their heights scaled by the height factor
//! h = g⊕ ÷ g ([`convergent::height_factor`]; `decision-r09-t12-coarse.md` item 2), their distances Earth's.
//!
//! R09.T12.c adds a stagnant lid's volcanic provinces ([`provinces`]): fields of edifices at plan
//! 14's edifice cap, gathered into provinces, covering the share of the surface whose cone field
//! has plan 14's constructional share, each standing at its height on the fill of its deflection,
//! with its moat filled to the surrounding level and its bulge planed to it, by the lid's elastic
//! thickness from its heat flow and its crust's substance (`decision-r09-t12-coarse.md` items 3
//! and 4, and its addenda 2 and 3).
//!
//! Each population's spread is the coarse landform noise ([`noise`]), which holds no power finer
//! than the coarse cell's Nyquist wavelength, the synthesis's structural share owning what is
//! finer (`decision-r09-t5.md` item 3).
//!
//! Until R09.T12.e, no sea is placed: every cell is dry, the sea level is the lowest elevation, and
//! the realised `σ_h` and relief are the cells' own (area-weighted s.d., and greatest less least),
//! interim figures that R09.T12.e replaces with the reconstructed field's.
//!
//! # The draws
//!
//! All on `surface.coarse.relief`, keyed `surface_cell(face, ℓ, i, j, instance)` of a lattice
//! cell, one standard normal (two words) each:
//!
//! - instance 0 ([`noise::LANDFORM_INSTANCE`]): the coarse landform noise, at levels 2 to the
//!   field's on a mobile lid and 1 to the field's on a stagnant one (whose red field adds the
//!   level-1 octave and weights the others by its β₁).
//! - instance 1 ([`convergent::STRIKE_INSTANCE`]), level 5 alone: the convergent landforms'
//!   along-strike draw.
//! - instances 2 and 3 ([`provinces::PROVINCE_INSTANCE`], [`provinces::EDIFICE_INSTANCE`]), level 0's
//!   first cell alone: the provinces' count and centres, and the edifices' offsets.

pub(crate) mod convergent;
mod noise;
pub(crate) mod provinces;
mod seafloor;

#[cfg(test)]
mod tests;

// The draws' instances on `surface.coarse.relief` are pairwise distinct, so that no two of the
// step's draws share a stream (the determinism audit's hardening).
const _: () = {
    let instances = [
        noise::LANDFORM_INSTANCE,
        convergent::STRIKE_INSTANCE,
        provinces::PROVINCE_INSTANCE,
        provinces::EDIFICE_INSTANCE,
    ];
    let mut i = 0;
    while i < instances.len() {
        let mut j = i + 1;
        while j < instances.len() {
            assert!(
                instances[i] != instances[j],
                "two of the relief step's draws share an instance"
            );
            j += 1;
        }
        i += 1;
    }
};

use hyperion_surface::field::Crust;
use hyperion_surface::num;
use hyperion_surface::synth::SpectrumShape;

use super::plates::{cell_of, reach, slot};
use super::{Pass, Working};
use crate::math;
use crate::planetary::derive::atmosphere::SurfaceMaterial;
use crate::planetary::hooks::figures::relief::{OCEANIC_WIDTH, STAGNANT_LID_RELIEF};
use crate::planetary::surface::inputs::{CoarseInputs, TectonicRegime};
use crate::units::{Metres, Radians};

/// The continental crust's mean elevation over sea level, +405 m.
///
/// Earth2014's cells above −2 km (Hirt and Rexer 2015, Int. J. Appl. Earth Obs. Geoinf. 39,
/// 103–112, its TBI layer at 5′, recomputed by `decision-r09-t12-coarse.md`, 2026-10-16: +404.9 m,
/// s.d. 1,084.1 m, which plan 14's
/// [`CONTINENTAL_WIDTH`](crate::planetary::hooks::figures::relief::CONTINENTAL_WIDTH) rounds to
/// 1.08 km); Design note 7's +406 m is superseded. The continental population and R09.T12.b's
/// convergent landforms realise it together.
pub const CONTINENTAL_MEAN: Metres = Metres::new(405.0);

/// The continental population's core mean, +222.6 m: [`CONTINENTAL_MEAN`] less the convergent
/// landforms' mean lift over the continental crust, so that the crust with them keeps Earth2014's
/// mean (ruled: `decision-r09-t12-coarse.md`'s addendum, item 3, "net the belts out"), calibrated
/// once on the reference Earth under its reference seed (`reference::SEED`), never per world. There
/// the belts' plateau profile averages 0.035 over the continental crust and the arcs lift it by
/// 14.7 m, so the mean μ solves μ + 0.035 09 (5,000 m − μ) + 14.7 m = 405 m
/// (`calibrate_the_continental_population_net_of_the_landforms`, by hand).
pub(crate) const CONTINENTAL_CORE_MEAN: Metres = Metres::new(222.6);

/// The continental population's core width, 628.5 m: √(1,084² − 883.2²) m, Earth2014's width
/// above −2 km less the variance of the convergent landforms' lift over the continental crust on
/// the reference Earth (the addendum's item 3, calibrated with [`CONTINENTAL_CORE_MEAN`]). Earth2014's
/// own continents are a narrow core with long tails (an interquartile range of 723 m and a
/// skewness of 1.20, the addendum's figures), which this core's skewness of 0.32 (its shape s
/// 0.105) does not reach.
pub(crate) const CONTINENTAL_CORE_WIDTH: Metres = Metres::new(628.5);

/// The shape s of the continental population's skew, 0.1047: the population is
/// μ + σ (e^(s n) − e^(s² ÷ 2)) ÷ √((e^(s²) − 1) e^(s²)) of a unit normal n, a log-normal shifted
/// and scaled to the core's mean [`CONTINENTAL_CORE_MEAN`] and width [`CONTINENTAL_CORE_WIDTH`],
/// whose mode μ + σ (e^(−s²) − e^(s² ÷ 2)) ÷ √((e^(s²) − 1) e^(s²)) is then Earth2014's land mode,
/// +125 m, the 250 m bin from 0 to +250 m (Design note 7, recomputed 2026-09-29; s solved to four
/// places, refitted with the core by the addendum's item 3). An illustration of Earth's skewed land
/// hypsometry, a long upper tail of uplands over lowlands near sea level, labelled: a Gaussian
/// puts its mode at its mean.
pub(crate) const CONTINENTAL_SKEW: f64 = 0.104_7;

/// Runs the step on `working`: the coarse elevation of R09.T12.a's populations, each cell's ground
/// and dry water surface, each oceanic cell's steady-state age and sea floor, and the interim sea
/// level and realised figures (see the module documentation).
///
/// # Panics
///
/// Never on valid inputs: every value it writes is finite and within its code.
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    if working.plates.is_empty() {
        lid_population(pass, working);
        let _ = provinces::run(pass, working);
    } else {
        let noise = noise::unit_field(
            pass,
            noise::LANDFORM_INSTANCE,
            noise::LANDFORM_FIRST_LEVEL,
            pass.level().get(),
        );
        crust_populations(pass, working, &noise);
        let _ = convergent::run(pass, working);
    }
    interim_figures(pass, working);
}

/// The structural spectrum's shape of the body of `inputs`' crust class, as R09.T12.e's anchored
/// spectrum will choose it from the surface material and the regime (`decision-r09-t5.md` item 2):
/// an ice shell or an ice surface takes [`SpectrumShape::ICE_RICH`], every rock surface in any
/// other regime [`SpectrumShape::SILICATE`].
#[must_use]
pub(crate) const fn crust_shape(inputs: &CoarseInputs) -> SpectrumShape {
    match (inputs.surface_material(), inputs.tectonics()) {
        (SurfaceMaterial::Ice, _) | (SurfaceMaterial::Rock, TectonicRegime::IceShell) => {
            SpectrumShape::ICE_RICH
        }
        (
            SurfaceMaterial::Rock,
            TectonicRegime::MobileLid
            | TectonicRegime::StagnantLid
            | TectonicRegime::HeatPipe
            | TectonicRegime::Episodic,
        ) => SpectrumShape::SILICATE,
    }
}

/// A stagnant lid's one population: each cell's elevation [`STAGNANT_LID_RELIEF`] times the red
/// unit field of the crust class's β₁ ([`noise::red_field`], [`crust_shape`]), about the datum,
/// its area mean zero and its area RMS the constant (`decision-r09-t12-coarse.md`'s addendum).
fn lid_population(pass: &Pass<'_>, working: &mut Working) {
    let noise = noise::red_field(
        pass,
        noise::LANDFORM_INSTANCE,
        crust_shape(pass.inputs()).exponent(),
    );
    for (cell, &n) in working.cells.iter_mut().zip(&noise) {
        let z = Metres::new(STAGNANT_LID_RELIEF.value() * n);
        cell.elevation = z;
        cell.water_surface = z;
    }
}

/// The skew of the continental population: (e^(s n) − e^(s² ÷ 2)) ÷ √((e^(s²) − 1) e^(s²)) of a
/// unit value `n`, s [`CONTINENTAL_SKEW`]: zero-mean and unit-variance for a unit normal n.
#[must_use]
fn skewed(n: f64) -> f64 {
    let s = CONTINENTAL_SKEW;
    let s2 = s * s;
    (math::exp(s * n) - math::exp(0.5 * s2)) / (math::exp_m1(s2) * math::exp(s2)).sqrt()
}

/// A mobile lid's two populations and its sea floor (see the module documentation): continental
/// cells from the unit `noise`, skewed; oceanic cells from [`seafloor`]'s laws, with the noise
/// sized so that the oceanic s.d. is [`OCEANIC_WIDTH`].
fn crust_populations(pass: &Pass<'_>, working: &mut Working, noise: &[f64]) {
    let inputs = pass.inputs();
    let grid = pass.grid();
    let omega = grid.solid_angles_sr();
    let figure = inputs.figure();
    let sphere: f64 = omega.iter().sum();
    let oceanic: Vec<bool> = working
        .cells
        .iter()
        .map(|c| c.crust == Crust::Oceanic)
        .collect();
    let oceanic_area: f64 = omega
        .iter()
        .zip(&oceanic)
        .filter(|&(_, &o)| o)
        .map(|(w, _)| w)
        .sum();
    let oldest = seafloor::oldest_floor_age(
        oceanic_area / sphere,
        inputs.heat_flow(),
        inputs.craters().system_age(),
    );
    let origins: Vec<_> = working.cells.iter().map(|c| c.seafloor_origin).collect();
    let ages = seafloor::steady_state_ages(&origins, omega, oldest);
    let margins = margin_distances(pass, working);
    // The sea floor before its noise: the basement under its load, and the roughness inputs.
    let mut floor = vec![0.0; working.cells.len()];
    for (n, cell) in working.cells.iter_mut().enumerate() {
        let (Some(origin), Some(age)) = (cell.seafloor_origin, ages[n]) else {
            continue;
        };
        let normal = figure.normal(grid.centre(cell_of(n)));
        let latitude = Radians::new(math::asin(normal[2].clamp(-1.0, 1.0)));
        let drape = seafloor::pelagic_drape(age, latitude);
        let ponded = seafloor::ponded_sediment(margins[n].map(Metres::new));
        let sediment = Metres::new(drape.value() + ponded.value());
        let basement = seafloor::basement_hill_relief(origin.half_rate);
        floor[n] = -seafloor::gdh1_depth(age).value() + seafloor::sediment_load(sediment).value();
        cell.seafloor_age = Some(age);
        cell.hill_relief = Some(seafloor::draped_hill_relief(basement, drape));
        cell.ponded_sediment = ponded;
    }
    let ocean_noise = subset_unit(noise, omega, &oceanic);
    let (_, floor_spread) = subset_mean_and_spread(&floor, omega, &oceanic);
    let width = OCEANIC_WIDTH.value();
    let noise_rms = num::max(width * width - floor_spread * floor_spread, 0.0).sqrt();
    let continental: Vec<bool> = working
        .cells
        .iter()
        .map(|c| c.crust == Crust::Continental)
        .collect();
    let mut land: Vec<f64> = subset_unit(noise, omega, &continental)
        .iter()
        .map(|&n| skewed(n))
        .collect();
    land = subset_unit(&land, omega, &continental);
    for (n, cell) in working.cells.iter_mut().enumerate() {
        let z = match cell.crust {
            Crust::Oceanic => floor[n] + noise_rms * ocean_noise[n],
            Crust::Continental => {
                CONTINENTAL_CORE_MEAN.value() + CONTINENTAL_CORE_WIDTH.value() * land[n]
            }
            Crust::Lid | Crust::Province => cell.elevation.value(),
        };
        let z = Metres::new(num::assert_finite(z));
        cell.elevation = z;
        cell.water_surface = z;
    }
}

/// Each oceanic cell's distance to its plate's nearest passive margin over the plate's own oceanic
/// cells, metres, or `None` (off oceanic crust, or where no margin of its plate reaches it).
///
/// A passive margin is an oceanic cell with an edge neighbour of continental crust on the same
/// plate (`decision-r09-t5.md` item 5). The paths are R09.T11's ([`reach`]), each plate's oceanic
/// cells a region, so they cross no plate boundary and no continent.
#[must_use]
fn margin_distances(pass: &Pass<'_>, working: &Working) -> Vec<Option<f64>> {
    let grid = pass.grid();
    let region: Vec<Option<u8>> = working
        .cells
        .iter()
        .map(|c| (c.crust == Crust::Oceanic).then_some(c.plate))
        .collect();
    let sources: Vec<u32> = (0..grid.len())
        .filter(|&cell| {
            let own = &working.cells[slot(cell)];
            own.crust == Crust::Oceanic
                && grid.edge_neighbours(cell).into_iter().any(|m| {
                    let other = &working.cells[slot(m)];
                    other.crust == Crust::Continental && other.plate == own.plate
                })
        })
        .collect();
    reach(grid, &region, &sources, pass.inputs().radius().value())
        .into_iter()
        .zip(&region)
        .map(|(r, own)| own.and(r.map(|r| r.distance_m)))
        .collect()
}

/// The area-weighted mean and s.d. of `values` over the cells `inside` marks, under `weights`;
/// (0, 0) where it marks none.
#[must_use]
fn subset_mean_and_spread(values: &[f64], weights: &[f64], inside: &[bool]) -> (f64, f64) {
    let (mut total, mut sum) = (0.0, 0.0);
    for ((v, w), &i) in values.iter().zip(weights).zip(inside) {
        if i {
            total += w;
            sum += v * w;
        }
    }
    if total <= 0.0 {
        return (0.0, 0.0);
    }
    let mean = sum / total;
    let mut square = 0.0;
    for ((v, w), &i) in values.iter().zip(weights).zip(inside) {
        if i {
            square += (v - mean) * (v - mean) * w;
        }
    }
    (mean, (square / total).sqrt())
}

/// `values` made zero-mean and unit-RMS over the cells `inside` marks, under `weights`; zero at
/// every other cell, and everywhere where the marked cells have no spread.
#[must_use]
fn subset_unit(values: &[f64], weights: &[f64], inside: &[bool]) -> Vec<f64> {
    let (mean, spread) = subset_mean_and_spread(values, weights, inside);
    values
        .iter()
        .zip(inside)
        .map(|(&v, &i)| {
            if i && spread > 0.0 {
                (v - mean) / spread
            } else {
                0.0
            }
        })
        .collect()
}

/// The interim sea level and realised figures, until R09.T12.e: the lowest elevation, the cells'
/// area-weighted s.d. and their greatest less least (see the module documentation).
pub(super) fn interim_figures(pass: &Pass<'_>, working: &mut Working) {
    let elevations: Vec<f64> = working
        .cells
        .iter()
        .map(|c| num::assert_finite(c.elevation.value()))
        .collect();
    let lowest = elevations.iter().copied().fold(f64::INFINITY, num::min);
    let highest = elevations.iter().copied().fold(f64::NEG_INFINITY, num::max);
    let (_, spread) = noise::mean_and_spread(&elevations, pass.grid().solid_angles_sr());
    working.sea_level = Metres::new(lowest);
    working.realised_sigma_h = Metres::new(spread);
    working.realised_relief = Metres::new(highest - lowest);
}
