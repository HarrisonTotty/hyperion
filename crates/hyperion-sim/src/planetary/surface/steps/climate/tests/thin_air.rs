//! R09.T13.h's tests: the thin air's balanced bounded slab with its snowfall
//! (decision-r09-t13-thin-air §1.9–§1.10), on the radiative model's runs.
//!
//! Each run is watched by a [`Book`], which keeps the sums of the run's last orbit: each site's
//! energy terms, each column's frost at the orbit's ends and where it cleared, each column's slab
//! and its sums in each month, and the seasonal caps' mass at each step.

use super::*;
use crate::orbit::Eccentricity;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::surface::inputs::GasShare;
use crate::planetary::surface::steps::climate::airless::{
    self, End, Frost, Observer, RadiativeSolution, Regolith, Sample, Slab,
};
use crate::planetary::surface::steps::climate::sun::orbit_at;
use crate::substance::SubstanceKey;
use crate::units::consts::{SOLAR_CONSTANT_W_PER_M2, STEFAN_BOLTZMANN};
use crate::units::{Kelvin, Pascals, WattsPerSquareMetre};

/// The swing of the reference Mars's seasonal caps, the orbit's maximum less its minimum of
/// Σ frost × area over the air's mass `p_s` A ÷ g, at the inertia and the albedo as built:
/// 0.18–0.40 (decision-r09-t13-thin-air §1.10 test 4; measured about 0.25 by Kelly et al. 2006
/// and Hourdin et al. 1993, the ruling's prototype 0.334). The ruling tightens it to 0.20–0.32
/// once the frost's albedo and the high latitudes' inertia land.
const CAP_SWING: core::ops::RangeInclusive<f64> = 0.18..=0.40;

/// The tight window of the caps' swing, the calibration's acceptance, printed beside the swing:
/// 0.20–0.32 (decision-r09-t13-thin-air §1.10 test 4; decision-r09-t13h-scope §1).
const CALIBRATED_CAP_SWING: core::ops::RangeInclusive<f64> = 0.20..=0.32;

/// Kelly et al. 2006's share of Mars's air that its seasonal caps exchange: about 6.0 × 10¹⁵ kg,
/// 0.244 of the reference air (JGR 111, E03S07, abstract read; decision-r09-t13-thin-air §1.4).
const KELLY_CAP_SWING: f64 = 0.244;

/// The northern polar cap's spring clearing, Ls, degrees: 70–100 (decision-r09-t13-thin-air
/// §1.10 test 5; Kieffer and Titus 2001 and Piqueux et al. 2015, from memory, to be read by the
/// calibration): printed, not asserted before the frost's albedo lands.
const NORTH_CLEARING_LS_DEG: core::ops::RangeInclusive<f64> = 70.0..=100.0;

/// The most the stored power and the slab's lag may each be of ⟨S⟩ over a converged orbit:
/// 10⁻³ (decision-r09-t13h-scope §3).
const BOOKED_TERM_SHARE: f64 = 1e-3;

/// How far a column's frost may differ between the ends of the converged orbit, kg m⁻²: 1
/// (decision-r09-t13-thin-air §1.10 test 4: no seasonal frost grows from orbit to orbit).
const PERIODIC_FROST_KG_M2: f64 = 1.0;

/// Where Kieffer's polar-night slab lies on Mars: 135–150 K, the carbon dioxide frost point at
/// the slab's level (decision-r09-t13-thin-air §1.10 test 1; Kieffer 2013, [46]; 141.1 K by James et
/// al.'s curve as Kieffer gives it).
const POLAR_NIGHT_SLAB_K: core::ops::RangeInclusive<f64> = 135.0..=150.0;

/// The reference Mars's carbon dioxide frost point at its partial pressure over e, 222.5 Pa, by
/// James et al.'s curve as Kieffer 2013 gives it ([59]): 141.1 K (decision-r09-t13-thin-air §1.3,
/// §1.10 test 1), which the registry's curve meets to 0.5 K.
const KIEFFER_POLAR_NIGHT_SLAB_K: f64 = 141.1;

/// A site's sums over an orbit, each sample weighted by its share of the orbit over the phases.
#[derive(Debug, Clone, Copy, Default)]
struct SiteSums {
    /// Σ w, which an orbit makes 1.
    weight: f64,
    /// Σ w S, the absorbed sunlight, W m⁻².
    sunlight: f64,
    /// Σ w (S + εσ`T_CMB`⁴ + ε R⇓), the column's input, W m⁻².
    input: f64,
    /// Σ w R⇓, the back radiation, W m⁻².
    down: f64,
    /// Σ w σT⁴ of the surface, W m⁻².
    surface_sigma_t4: f64,
    /// Σ w F̄, the mean emission each sample's slab was formed from, W m⁻².
    mean_emission: f64,
    /// Σ w of the step's stored heat, W m⁻².
    stored: f64,
    /// Σ w Δfrost ÷ Δt, kg m⁻² s⁻¹.
    frost_rate: f64,
}

/// A column's sums over one month of an orbit.
#[derive(Debug, Clone, Copy)]
struct MonthSums {
    /// The month's slab.
    slab: Slab,
    /// Σ w T and Σ w over the month.
    surface: (f64, f64),
    /// Σ w σT⁴ over the month.
    surface_sigma_t4: f64,
    /// Σ S over the month's steps, W m⁻².
    sunlight: f64,
    /// The frost's change over the month, kg m⁻², and the month's duration, s.
    frost: (f64, f64),
}

impl Default for MonthSums {
    fn default() -> Self {
        Self {
            slab: Slab {
                mean_emission_w_m2: f64::NAN,
                blackbody_w_m2: f64::NAN,
                down_w_m2: f64::NAN,
                snow_w_m2: f64::NAN,
            },
            surface: (0.0, 0.0),
            surface_sigma_t4: 0.0,
            sunlight: 0.0,
            frost: (0.0, 0.0),
        }
    }
}

/// The last orbit of a run, as the run's samples give it.
#[derive(Debug, Clone)]
struct Book {
    /// Each site's share of the grid's area, Ω ÷ 4π (the end columns' zero), and whether it lies
    /// north of the equator.
    sites: Vec<(f64, bool)>,
    /// The orbit the sums are of.
    orbit: u32,
    /// Each site's energy sums.
    energy: Vec<SiteSums>,
    /// Each column's frost at the orbit's start and at its end, kg m⁻².
    frost_ends: Vec<(f64, f64)>,
    /// The mean anomaly of each column's last step that spent its frost, rad.
    cleared_at: Vec<Option<f64>>,
    /// Each column's sums in each month, `column * 12 + month`.
    months: Vec<MonthSums>,
    /// Each step's frost Σ ΔΩ ÷ 4π × frost over the grid's northern and southern sites, kg m⁻².
    caps: Vec<[f64; 2]>,
    /// Each orbit's Σ w T and Σ w at each site, kept over every orbit.
    annual: Vec<Vec<(f64, f64)>>,
}

impl Book {
    /// An empty book for the radiative model of `inputs`.
    fn of(inputs: &CoarseInputs) -> Self {
        let (grid, _) = airless::frame(inputs);
        let points = grid.points();
        let columns = points / LATITUDE_BANDS;
        let per_band = f64::from(u32::try_from(columns).unwrap());
        let mut sites: Vec<(f64, bool)> = (0..points)
            .map(|p| {
                let band = p / columns;
                (
                    grid.weight(band) / (2.0 * per_band),
                    grid.centre_sine(band) > 0.0,
                )
            })
            .collect();
        // The end columns carry no weight: at most the two poles and the equator's ring.
        sites.extend(core::iter::repeat_n(
            (0.0, false),
            3 + airless::EQUATOR_RING_COLUMNS,
        ));
        Self {
            sites,
            orbit: u32::MAX,
            energy: Vec::new(),
            frost_ends: Vec::new(),
            cleared_at: Vec::new(),
            months: Vec::new(),
            caps: Vec::new(),
            annual: Vec::new(),
        }
    }

    /// Starts orbit `orbit`'s sums afresh.
    fn restart(&mut self, orbit: u32) {
        self.orbit = orbit;
        self.energy.clear();
        self.frost_ends.clear();
        self.cleared_at.clear();
        self.months.clear();
        self.caps.clear();
        self.annual.push(Vec::new());
    }

    /// Each orbit's move from the one before, the most any site's annual mean moved, K, with the
    /// site that moved most.
    fn moves(&self) -> Vec<(f64, usize)> {
        let means: Vec<Vec<f64>> = self
            .annual
            .iter()
            .map(|orbit| orbit.iter().map(|(s, w)| s / w).collect())
            .collect();
        means
            .windows(2)
            .map(|pair| {
                pair[1].iter().zip(&pair[0]).enumerate().fold(
                    (0.0, 0),
                    |(most, at), (site, (a, b))| {
                        let d = (a - b).abs();
                        if d > most { (d, site) } else { (most, at) }
                    },
                )
            })
            .collect()
    }

    /// The grid's mean of a site quantity `f` over the points, each site's sums over its weight.
    fn grid_mean(&self, points: usize, f: impl Fn(&SiteSums) -> f64) -> f64 {
        self.energy[..points]
            .iter()
            .zip(&self.sites)
            .fold(0.0, |sum, (sums, &(area, _))| {
                sum + area * f(sums) / sums.weight
            })
    }

    /// The columns of a site: one, or the phase columns of a world stepped in true time.
    fn phases(&self) -> usize {
        self.frost_ends.len() / self.energy.len()
    }

    /// The first column of the site of `solution`'s end column `end`.
    fn end_column(&self, solution: &RadiativeSolution, end: End) -> usize {
        let site =
            solution.grid.points() + solution.ends.iter().position(|c| c.end == end).unwrap();
        site * self.phases()
    }

    /// Column `column`'s sums in month `month`.
    fn month(&self, column: usize, month: usize) -> &MonthSums {
        &self.months[column * 12 + month]
    }

    /// The orbit's swing of the caps' mass, and the peaks of the northern and southern ones, as
    /// shares of the air's mass `p_s` A ÷ g.
    fn caps(&self, inputs: &CoarseInputs) -> Caps {
        let phases = f64::from(u32::try_from(self.phases()).unwrap());
        let scale = inputs.gravity().value() / inputs.surface_pressure().value() / phases;
        let (mut high, mut low, mut north, mut south) = (
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for [n, s] in &self.caps {
            let total = scale * (n + s);
            high = high.max(total);
            low = low.min(total);
            north = north.max(scale * n);
            south = south.max(scale * s);
        }
        Caps {
            swing: high - low,
            peak: high,
            north,
            south,
        }
    }
}

impl Observer for Book {
    const STORED_HEAT: bool = true;

    fn sample(&mut self, s: &Sample) {
        if s.orbit != self.orbit {
            self.restart(s.orbit);
        }
        let site = s.site;
        if self.energy.len() <= site {
            self.energy.resize(site + 1, SiteSums::default());
        }
        if let Some(annual) = self.annual.last_mut() {
            if annual.len() <= site {
                annual.resize(site + 1, (0.0, 0.0));
            }
            annual[site].0 += s.weight * s.surface_k;
            annual[site].1 += s.weight;
        }
        if self.frost_ends.len() <= s.column {
            self.frost_ends.resize(s.column + 1, (f64::NAN, f64::NAN));
            self.cleared_at.resize(s.column + 1, None);
            self.months
                .resize((s.column + 1) * 12, MonthSums::default());
        }
        let sigma_t4 = STEFAN_BOLTZMANN * math::powi(s.surface_k, 4);
        let e = &mut self.energy[site];
        e.weight += s.weight;
        e.sunlight += s.weight * s.absorbed_w_m2;
        e.input += s.weight * s.input_w_m2;
        e.down += s.weight * s.slab.down_w_m2;
        e.surface_sigma_t4 += s.weight * sigma_t4;
        e.mean_emission += s.weight * s.slab.mean_emission_w_m2;
        e.stored += s.weight * s.stored_w_m2;
        e.frost_rate += s.weight * s.frost_change_kg_m2 / s.duration_s;
        let before = s.frost_kg_m2 - s.frost_change_kg_m2;
        let ends = &mut self.frost_ends[s.column];
        if ends.0.is_nan() {
            ends.0 = before;
        }
        ends.1 = s.frost_kg_m2;
        if before > 0.0 && s.frost_kg_m2 <= 0.0 {
            self.cleared_at[s.column] = Some(s.mean_anomaly_rad);
        }
        let m = &mut self.months[s.column * 12 + s.month];
        m.slab = s.slab;
        m.surface.0 += s.weight * s.surface_k;
        m.surface.1 += s.weight;
        m.surface_sigma_t4 += s.weight * sigma_t4;
        m.sunlight += s.absorbed_w_m2;
        m.frost.0 += s.frost_change_kg_m2;
        m.frost.1 += s.duration_s;
        if self.caps.len() <= s.step {
            self.caps.resize(s.step + 1, [0.0; 2]);
        }
        // Each phase column holds its share of its site's area, which `caps` divides.
        let (area, north) = self.sites[site];
        self.caps[s.step][usize::from(!north)] += area * s.frost_kg_m2;
    }
}

/// The seasonal caps over an orbit, as shares of the air's mass `p_s` A ÷ g: the swing of their
/// total, its peak, and each hemisphere's peak.
#[derive(Debug, Clone, Copy)]
struct Caps {
    swing: f64,
    peak: f64,
    north: f64,
    south: f64,
}

/// The radiative model's run of `inputs`, with its book.
fn run_booked(inputs: &CoarseInputs) -> (RadiativeSolution, Book) {
    let mut book = Book::of(inputs);
    let solution = airless::solve_observed(inputs, &mut book);
    (solution, book)
}

/// The month of column `column` of `book` whose mean surface temperature is the least, the first
/// of a tie.
fn coldest_month(book: &Book, column: usize) -> usize {
    (0..12)
        .min_by(|&a, &b| {
            let mean = |k: usize| {
                let m = book.month(column, k);
                m.surface.0 / m.surface.1
            };
            mean(a).total_cmp(&mean(b))
        })
        .unwrap()
}

/// The areocentric longitude of the sun at the mean anomaly `mean_anomaly_rad` on the orbit of
/// `inputs`, degrees: the true anomaly less the northern spring equinox's.
fn solar_longitude_deg(inputs: &CoarseInputs, mean_anomaly_rad: f64) -> f64 {
    let e = Eccentricity::new(inputs.seasonal_orbit().unwrap().eccentricity).unwrap();
    let place = orbit_at(mean_anomaly_rad, e);
    (place.true_anomaly_rad - inputs.spin().equinox_true_anomaly.value())
        .to_degrees()
        .rem_euclid(360.0)
}

/// The polar-night slab (decision-r09-t13-thin-air §1.10, test 1). On the reference Mars, in each
/// pole's coldest month the slab lies at the carbon dioxide frost point at its partial pressure
/// over e, the registry's figure, to 0.5 K, and within Kieffer's 135–150 K; and no column's slab
/// in any month lies below that bound.
#[test]
fn the_polar_night_slab_lies_at_the_frost_point_aloft() {
    let inputs = reference::mars_like();
    let (solution, book) = run_booked(&inputs);
    assert!(
        solution.climate.converged,
        "{} orbits",
        solution.climate.orbits
    );
    let frost = solution.air.frost.expect("Mars's carbon dioxide condenses");
    let bulk = inputs.gases()[0];
    let diagram = by_key(bulk.substance.as_str())
        .unwrap()
        .substance()
        .phase_diagram()
        .unwrap();
    let pressure = bulk.mole_fraction * inputs.surface_pressure().value() / core::f64::consts::E;
    let aloft = diagram
        .saturation_temperature(Pascals::new(pressure))
        .unwrap()
        .value();
    eprintln!(
        "{} orbits; β {:.4}; frost point {:.3} K at the surface, {aloft:.3} K at {pressure:.1} Pa \
         (Kieffer's 141.1 K)",
        solution.climate.orbits, solution.air.absorptivity, frost.point_k
    );
    assert!(
        (aloft - KIEFFER_POLAR_NIGHT_SLAB_K).abs() < 0.5,
        "the registry's {aloft} K against James et al.'s {KIEFFER_POLAR_NIGHT_SLAB_K} K"
    );
    for end in [End::NorthPole, End::SouthPole] {
        let column = book.end_column(&solution, end);
        let month = coldest_month(&book, column);
        let slab = book.month(column, month).slab;
        let t = slab.temperature_k();
        eprintln!(
            "{end:?}: coldest month {month}, slab {t:.3} K, F̄ {:.2} W m⁻², snow {:.3} W m⁻²",
            slab.mean_emission_w_m2, slab.snow_w_m2
        );
        assert!((t - aloft).abs() < 0.5, "{end:?}: {t} against {aloft}");
        assert!(POLAR_NIGHT_SLAB_K.contains(&t), "{end:?}: {t}");
        assert!(slab.snow_w_m2 > 0.0, "{end:?}: the bounded slab snows");
    }
    let lowest = book
        .months
        .iter()
        .map(|m| m.slab.temperature_k())
        .fold(f64::INFINITY, f64::min);
    eprintln!("the coldest slab of any column in any month: {lowest:.4} K");
    assert!(lowest >= frost.aloft_point_k - 1e-9, "{lowest}");
}

/// Energy (decision-r09-t13-thin-air §1.10, test 3). Over the reference Mars's converged orbit
/// the grid's mean of the absorbed sunlight, the background and the heat flow equals its mean
/// outgoing flux, (1 − β)εσ`T_s`⁴ + βσ`T_a`⁴ + (1 − ε)βσ`T_a`⁴ (the transmitted emission, the
/// slab's own emission upward and the reflected back radiation), less the latent heat the
/// orbit's net gain of frost and snow released, to 10⁻⁶ of ⟨S⟩, in the model's own terms: with
/// the column's stored heat over the orbit, which the whole days' compression of a month's upper
/// layers and the stop at 0.1 K leave, and the slab's lag β (⟨εσ`T_s`⁴⟩ − F̄), its balance being
/// the orbit before's, each of them printed and held below 10⁻³ of ⟨S⟩ (decision-r09-t13h-scope
/// §3). The ruling's terms alone are printed beside them. It runs on the reference Mars, on the 3:2
/// Mercury under the reference Mars's air (true time, the map) and on a seasonal locked world under
/// it (the beam).
#[test]
fn the_thin_air_conserves_energy_over_the_converged_orbit() {
    let mars = reference::mars_like();
    for (name, inputs) in [
        ("reference Mars", mars.clone()),
        (
            "3:2 Mercury with Mars's air",
            with_thin_air(&other_regimes::mercury_like(), &mars),
        ),
        (
            "seasonal locked Moon with Mars's air",
            with_thin_air(&other_regimes::locked_moon(), &mars),
        ),
    ] {
        assert_energy_closes(name, &inputs);
    }
}

/// Test 3's identity on the radiative model's run of `inputs`, named `name`, over its converged
/// orbit (decision-r09-t13h-scope §3).
fn assert_energy_closes(name: &str, inputs: &CoarseInputs) {
    let (solution, book) = run_booked(inputs);
    assert!(
        solution.climate.converged,
        "{name}: {} orbits",
        solution.climate.orbits
    );
    let points = solution.grid.points();
    let (beta, epsilon) = (solution.air.absorptivity, solution.emissivity);
    let latent = solution.air.frost.unwrap().latent_heat_j_per_kg;
    let heat_flow = Regolith::of(inputs).heat_flow_w_m2;
    let sunlight = book.grid_mean(points, |s| s.sunlight);
    // S + εσT_CMB⁴ + Q: the input less ε of the back radiation, and the heat flow.
    let inward = book.grid_mean(points, |s| s.input - epsilon * s.down) + heat_flow;
    let outgoing = book.grid_mean(points, |s| {
        (1.0 - beta) * epsilon * s.surface_sigma_t4 + (2.0 - epsilon) * s.down
    });
    // The latent heat released by the orbit's net gain of frost, snow included.
    let released = latent * book.grid_mean(points, |s| s.frost_rate);
    let stored = book.grid_mean(points, |s| s.stored);
    let lag = beta * book.grid_mean(points, |s| epsilon * s.surface_sigma_t4 - s.mean_emission);
    let ruled = inward + released - outgoing;
    let residual = ruled - stored - lag;
    eprintln!(
        "{name}: {} orbits, β {beta:.4}, {} phase columns; ⟨S⟩ {sunlight:.4} W m⁻²; in \
         {inward:.6}, out {outgoing:.6}, released by frost and snow {released:.3e}, stored \
         {stored:.3e} ({:.2e} of ⟨S⟩), lag {lag:.3e} ({:.2e}) W m⁻²; the ruling's terms leave \
         {ruled:.3e} ({:.2e} of ⟨S⟩), all of them {residual:.3e} ({:.2e})",
        solution.climate.orbits,
        book.phases(),
        stored / sunlight,
        lag / sunlight,
        ruled / sunlight,
        residual / sunlight
    );
    assert!(beta > 0.0, "{name}: a slab");
    assert!(residual.abs() < 1e-6 * sunlight, "{name}: {residual}");
    assert!(
        stored.abs() < BOOKED_TERM_SHARE * sunlight,
        "{name}: {stored}"
    );
    assert!(lag.abs() < BOOKED_TERM_SHARE * sunlight, "{name}: {lag}");
}

/// The seasonal caps (decision-r09-t13-thin-air §1.10, test 4). On the reference Mars, at the
/// inertia and the albedo as built, the orbit's swing in the frost's mass over the air's lies in
/// [`CAP_SWING`]; the southern cap's peak exceeds the northern's (the long aphelion winter); and
/// every column's frost is periodic over the converged orbit to [`PERIODIC_FROST_KG_M2`]. The
/// poles' spring clearing is printed: the ruling's calibration target, Ls 70–100° in the north
/// (Kieffer and Titus 2001; Piqueux et al. 2015), is not a test before the frost's albedo lands.
#[test]
fn the_reference_mars_s_seasonal_caps_swing_within_their_window_the_south_s_the_larger() {
    let inputs = reference::mars_like();
    let (solution, book) = run_booked(&inputs);
    assert!(
        solution.climate.converged,
        "{} orbits",
        solution.climate.orbits
    );
    let Caps {
        swing,
        peak,
        north,
        south,
    } = book.caps(&inputs);
    let cleared = |end: End| {
        let column = book.end_column(&solution, end);
        book.cleared_at[column].map(|m| solar_longitude_deg(&inputs, m))
    };
    eprintln!(
        "{} orbits: swing {swing:.3} of the air (asserted in {CAP_SWING:?}; the calibration's \
         {CALIBRATED_CAP_SWING:?}, Kelly et al.'s {KELLY_CAP_SWING}, which may be a peak total), \
         the total's peak {peak:.3}, north's peak {north:.3}, south's {south:.3}; the north pole \
         clears at Ls {:.1?} (observed {NORTH_CLEARING_LS_DEG:?}, printed), the south pole at Ls \
         {:.1?}",
        solution.climate.orbits,
        cleared(End::NorthPole),
        cleared(End::SouthPole)
    );
    assert!(CAP_SWING.contains(&swing), "{swing}");
    assert!(south > north, "{south} against {north}");
    let worst = book
        .frost_ends
        .iter()
        .fold(0.0_f64, |most, (a, b)| most.max((a - b).abs()));
    eprintln!("the frost's largest change over the converged orbit: {worst:.3e} kg m⁻²");
    assert!(worst <= PERIODIC_FROST_KG_M2, "{worst}");
}

/// The reference Mars with its mean temperature `mean_k`, so that its slab's β is solved anew.
fn mars_at(mean_k: f64) -> CoarseInputs {
    builder_of(&reference::mars_like())
        .mean_surface_temperature(Kelvin::new(mean_k))
        .build()
        .unwrap()
}

/// The latent heat of a polar night's condensation, surface and snow together, W m⁻²: the frost
/// that column `column` of `book` gains over its months with no sunlight, over their duration.
fn polar_night_condensation_w_m2(book: &Book, column: usize, latent: f64) -> f64 {
    let (gained, duration) = (0..12)
        .map(|k| book.month(column, k))
        .filter(|m| m.sunlight <= 0.0)
        .fold((0.0, 0.0), |(g, d), m| (g + m.frost.0, d + m.frost.1));
    assert!(duration > 0.0, "column {column} has a polar night");
    latent * gained / duration
}

/// A saturated slab (decision-r09-t13-thin-air §1.10, test 6): the reference Mars with plan 14's
/// mean raised to 250 K, where β is held at 1, converges within the orbits' cap, and each pole's
/// polar-night condensation, surface and snow together, lies within 10% of the slab-free column's
/// (the ruling's prototype: −7.8%). The slab-free column is the same Mars at 200 K, where β is 0.
#[test]
fn a_saturated_slab_converges_and_condenses_a_polar_night_as_the_slab_free_column() {
    let (saturated, saturated_book) = run_booked(&mars_at(250.0));
    let (free, free_book) = run_booked(&mars_at(200.0));
    let (reference_run, reference_book) = run_booked(&reference::mars_like());
    // Each orbit's move and the site that moved most (decision-r09-t13h-scope §4).
    for (orbit, (moved, site)) in saturated_book.moves().iter().enumerate() {
        let points = saturated.grid.points();
        let at = if *site < points {
            format!(
                "{:.2}°",
                math::asin(saturated.grid.centre_sine(*site)).to_degrees()
            )
        } else {
            format!("{:?}", saturated.ends[site - points].end)
        };
        eprintln!("β 1, orbit {}: moved {moved:.4} K, most at {at}", orbit + 1);
    }
    assert_same_bits(saturated.air.absorptivity, 1.0);
    assert_same_bits(free.air.absorptivity, 0.0);
    assert!(
        saturated.climate.converged,
        "{} orbits",
        saturated.climate.orbits
    );
    let latent = free.air.frost.unwrap().latent_heat_j_per_kg;
    for end in [End::NorthPole, End::SouthPole] {
        let column = free_book.end_column(&free, end);
        let bare = polar_night_condensation_w_m2(&free_book, column, latent);
        let with = polar_night_condensation_w_m2(&saturated_book, column, latent);
        let reference_value = polar_night_condensation_w_m2(&reference_book, column, latent);
        eprintln!(
            "{end:?}: the polar night condenses {with:.3} W m⁻² at β 1 ({} orbits), {:.3} at the \
             reference's β {:.3}, {bare:.3} with no slab: {:+.1}% and {:+.1}%",
            saturated.climate.orbits,
            reference_value,
            reference_run.air.absorptivity,
            100.0 * (with / bare - 1.0),
            100.0 * (reference_value / bare - 1.0)
        );
        assert!(
            (with / bare - 1.0).abs() < 0.10,
            "{end:?}: {with} against {bare}"
        );
    }
}

/// A Pluto-like world: Pluto's 1.3 Pa of 99% nitrogen, 0.5% methane and 0.05% carbon monoxide,
/// on its orbit (39.48 au, e 0.2488, 90,560 days), turning in 6.387 days at 60.49° to it (Pluto's
/// 119.51°, retrograde, gives the same sunlight); all from NASA's Pluto fact sheet (NSSDCA, read
/// 2026-10-20 by the lane's science check). A frozen ice surface at plan 14's snowball albedo, 0.50
/// (the fact sheet's Bond albedo is 0.72), 3 mW m⁻² of heat flow (an assumed figure of a rock
/// core's radiogenic order, from memory, no source read), and its mean set just above its
/// equilibrium temperature, so that its slab's β is about the ruling's 0.03 (§1.6). Its figure,
/// gravity and craters stay the reference Ceres's, which the radiative model does not read.
///
/// Built by hand: from the record, an air under plan 14's `AIRLESS_PRESSURE` (100 Pa) reaches the
/// pass at no pressure (R09's Risks, "Deferred findings (2026-10-13 rule) (R09.T13.h)").
fn pluto_like() -> CoarseInputs {
    let key = |k: &str| SubstanceKey::new(k).unwrap();
    let (a_au, e): (f64, f64) = (39.48, 0.2488);
    let year = 90_560.0 * SECONDS_PER_DAY;
    let rotation = 6.387 * SECONDS_PER_DAY;
    let flux = SOLAR_CONSTANT_W_PER_M2 / (a_au * a_au * (1.0 - e * e).sqrt());
    let albedo = SurfaceState::Snowball.albedo(SurfaceMaterial::Ice).value();
    let equilibrium = ((1.0 - albedo) * flux / (4.0 * STEFAN_BOLTZMANN))
        .sqrt()
        .sqrt();
    builder_of(&reference::ceres_like())
        .mean_surface_temperature(Kelvin::new(1.004 * equilibrium))
        .spin(Spin {
            obliquity: Radians::new(60.49_f64.to_radians()),
            rotation_period: Seconds::new(rotation),
            solar_day: Some(Seconds::new(1.0 / (1.0 / rotation - 1.0 / year))),
            equinox_true_anomaly: Radians::ZERO,
        })
        .host_flux(WattsPerSquareMetre::new(flux))
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: e,
            period: Seconds::new(year),
        })
        .surface_pressure(Pascals::new(1.3))
        .gases([
            GasShare {
                substance: key("N2"),
                mole_fraction: 0.99,
            },
            GasShare {
                substance: key("CH4"),
                mole_fraction: 0.005,
            },
            GasShare {
                substance: key("CO"),
                mole_fraction: 0.000_5,
            },
        ])
        .surface(SurfaceState::Snowball, SurfaceMaterial::Ice)
        .heat_flow(WattsPerSquareMetre::new(0.003))
        .build()
        .expect("a Pluto-like world's inputs are valid")
}

/// Any thin air (decision-r09-t13-thin-air §1.10, test 6): a Pluto-like nitrogen air converges,
/// and in each pole's coldest month its slab lies at nitrogen's bound, its frost point at its
/// partial pressure over e.
#[test]
fn a_pluto_like_nitrogen_air_converges_with_its_night_slab_at_its_bound() {
    let inputs = pluto_like();
    let (solution, book) = run_booked(&inputs);
    let frost = solution.air.frost.expect("Pluto's nitrogen condenses");
    eprintln!(
        "{} orbits, converged {}: β {:.4}; nitrogen's frost point {:.3} K, aloft {:.3} K",
        solution.climate.orbits,
        solution.climate.converged,
        solution.air.absorptivity,
        frost.point_k,
        frost.aloft_point_k
    );
    assert!(
        solution.climate.converged,
        "{} orbits",
        solution.climate.orbits
    );
    assert!(solution.air.absorptivity > 0.0);
    for end in [End::NorthPole, End::SouthPole] {
        let column = book.end_column(&solution, end);
        let month = coldest_month(&book, column);
        let t = book.month(column, month).slab.temperature_k();
        eprintln!("{end:?}: coldest month {month}, slab {t:.4} K");
        assert!((t - frost.aloft_point_k).abs() < 1e-9, "{end:?}: {t}");
    }
}

/// Any thin air (decision-r09-t13-thin-air §1.10, test 6): an air whose bulk gas has no phase
/// diagram (neon, the registry's row with none, in the reference Mars's carbon dioxide's place)
/// takes no bound and no snow and grows no frost, and its slab in each month is the balance of
/// its column's emission, σ`T_a`⁴ = F̄ ÷ 2, F̄ within 0.5 K of the converged orbit's own
/// (ε⟨`T_s`⁴⟩ ÷ 2)^¼ at the poles' polar night.
#[test]
fn an_air_whose_bulk_gas_has_no_diagram_takes_no_bound_and_no_snow() {
    let mars = reference::mars_like();
    let mut gases: Vec<GasShare> = mars.gases().to_vec();
    gases[0].substance = SubstanceKey::new("Ne").unwrap();
    assert!(by_key("Ne").unwrap().substance().phase_diagram().is_none());
    let inputs = builder_of(&mars).gases(gases).build().unwrap();
    assert!(Frost::of(&inputs).is_none());
    let (solution, book) = run_booked(&inputs);
    assert!(
        solution.climate.converged,
        "{} orbits",
        solution.climate.orbits
    );
    assert!(solution.air.frost.is_none());
    for (i, m) in book.months.iter().enumerate() {
        let (column, month) = (i / 12, i % 12);
        assert_same_bits(m.slab.blackbody_w_m2, 0.5 * m.slab.mean_emission_w_m2);
        assert_same_bits(m.slab.snow_w_m2, 0.0);
        assert_same_bits(m.frost.0, 0.0);
        assert!(m.slab.down_w_m2 > 0.0, "column {column}, month {month}");
    }
    let epsilon = solution.emissivity;
    for end in [End::NorthPole, End::SouthPole] {
        let column = book.end_column(&solution, end);
        let month = coldest_month(&book, column);
        let m = book.month(column, month);
        let own = (epsilon * m.surface_sigma_t4 / m.surface.1 / 2.0 / STEFAN_BOLTZMANN)
            .sqrt()
            .sqrt();
        let t = m.slab.temperature_k();
        eprintln!("{end:?}: coldest month {month}, slab {t:.3} K, the orbit's own {own:.3} K");
        assert!((t - own).abs() < 0.5, "{end:?}: {t} against {own}");
    }
}
