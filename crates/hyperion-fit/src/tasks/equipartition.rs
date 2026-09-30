//! The mass-segregation exponent η of the cluster classes' profiles against multimass King models
//! (plan 15, P15.T8.b; consumer plan 09's `galaxy::features::interior::profile`).
//!
//! A class of mass ratio `q = m ÷ m_TO` below 1 has the cored profile `(1 + r² ÷ r_c²)^(−3q^η ÷
//! 2)`, times the outer factor of ruling 139.1 and the common taper `(1 − r² ÷ r_t²)²`
//! (`ProfileShape::Core`, `ClassProfile`). The task fits η to multimass lowered-isothermal (King)
//! models, which it integrates itself:
//!
//! - **Models.** One King (`g = 1`) component per mass class, in the manner of Da Costa and
//!   Freeman (1976, ApJ 206, 128) and Gunn and Griffin (1979, AJ 84, 752): the density of class
//!   `j` is `A_j ρ̂(μ_j^(2δ) W)`, `ρ̂(w) = e^w erf √w − √(4w ÷ π) (1 + 2w ÷ 3)`, with
//!   `μ_j = m_j ÷ m_TO` and δ the velocity-scale exponent `s_j ∝ μ_j^(−δ)` of Gieles and Zocchi
//!   (2015, MNRAS 454, 576, eqs. 24–29). δ = ½ is the standard multimass King law (ruling 139.2).
//!   Poisson's equation in King's units, `∇²W = −9 ρ ÷ ρ₀`, is integrated outward in `ln r` by RK4
//!   from the centre's series to the tidal radius where `W = 0`, and the `A_j` are iterated until
//!   the classes hold their mass fractions.
//! - **Classes.** An old globular's (12 Gyr, [Fe/H] −1.3, the turn-off at plan 06's mass): living
//!   stars on the galaxy's mass function over 0.08 M☉ to the turn-off in ten bins even in `ln m`,
//!   depleted at the cluster's slope as plan 09's classes are (`counts::depletion`); white dwarfs
//!   from the turn-off to 8 M☉ in six bins at Kalirai et al.'s (2008) masses; neutron stars at
//!   1.35 M☉, a tenth of those born kept. A tracer at the turn-off mass sets the core radius `r_c`
//!   the profiles are read with: the `q = 1` profile's half-mass radius is the tracer's. The
//!   profiles' `r_h` is the model's half-mass radius and `r_t` its tidal radius.
//! - **Grid.** The central potentials of the manifest, which give concentrations `log₁₀(r_t ÷
//!   r_c)` over 0.7–2.3, by the depleted slopes over the catalogue's range.
//! - **Fit.** η minimises the squared log errors of the living classes' half-mass radii below the
//!   turn-off over the grid, by golden-section search.
//!
//! Acceptance: each class's half-mass radius below the turn-off to 10%; η within 0.8–1.0 (ruling
//! 139.2: Peuten et al. 2017 find δ ≃ 0.5 in N-body models, Hénault-Brunet et al. 2019 fit 0.44 at
//! 47 Tucanae).

use std::num::NonZeroUsize;

use hyperion_sim::galaxy::features::interior::counts::{depletion, white_dwarf_mass};
use hyperion_sim::galaxy::features::interior::profile::{ClassProfile, ProfileShape};
use hyperion_sim::galaxy::imf::{Chabrier, MassFunction};
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::sse::turn_off_mass;
use hyperion_sim::units::{Dex, HeliumExcess, Years};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::optimise::golden_section;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The living stars' bins, even in `ln m` from 0.08 M☉ to the turn-off.
const LIVING_BINS: usize = 10;

/// The white dwarfs' bins, even in `ln m` of their progenitors from the turn-off to 8 M☉.
const DWARF_BINS: usize = 6;

/// Plan 09's scratch η, 1 (δ = ½), which the table keeps while the fit misses its acceptance.
pub const SCRATCH_ETA: f64 = 1.0;

/// The neutron stars' mass, M☉, as plan 09's classes weigh them.
const NEUTRON_STAR_MASS: f64 = 1.35;

/// The share of neutron stars born that the model keeps.
const NEUTRON_STARS_KEPT: f64 = 0.1;

/// The tracer's mass fraction: small enough not to move the potential.
const TRACER_FRACTION: f64 = 1e-12;

/// The step in `ln r` of the Poisson integration, and its start in King radii.
const LN_STEP: f64 = 0.008;
const R_START: f64 = 1e-4;

/// Iterations of the classes' normalisation.
const NORMALISATION_ITERATIONS: usize = 25;

/// The age and metallicity of the model cluster.
const AGE_YEARS: f64 = 12e9;
const FE_H: f64 = -1.3;

/// One mass class of a model: its mean mass, M☉, and its share of the model's mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassClass {
    /// Mean mass, M☉.
    pub mass: f64,
    /// Share of the cluster's mass.
    pub fraction: f64,
    /// Whether it is a living star below the turn-off, which the fit reads.
    pub living: bool,
}

/// The classes of an old globular at depleted slope `alpha`, with the tracer last.
#[must_use]
pub fn mass_classes(alpha: f64) -> (Vec<MassClass>, f64) {
    let composition = Composition::from_fe_h(Dex::new(FE_H), HeliumExcess::ZERO);
    let m_to = turn_off_mass(Years::new(AGE_YEARS), &composition).value();
    let imf = Chabrier::provisional();
    let mut classes = Vec::new();
    let moments = |lo: f64, hi: f64, w: &dyn Fn(f64) -> f64| {
        let f = |t: f64| {
            let m = math::exp(t);
            imf.pdf(m) * w(m) * m
        };
        let (a, b) = (math::ln(lo), math::ln(hi));
        let n = hyperion_sim::galaxy::quad::gl32(f, a, b);
        let mass = hyperion_sim::galaxy::quad::gl32(|t| f(t) * math::exp(t), a, b);
        (n, mass)
    };
    let edges = |lo: f64, hi: f64, n: usize, k: usize| {
        math::exp(math::ln(lo) + (math::ln(hi) - math::ln(lo)) * index_f64(k) / index_f64(n))
    };
    for k in 0..LIVING_BINS {
        let (lo, hi) = (
            edges(0.08, m_to, LIVING_BINS, k),
            edges(0.08, m_to, LIVING_BINS, k + 1),
        );
        let (n, mass) = moments(lo, hi, &|m| depletion(m, alpha));
        classes.push(MassClass {
            mass: mass / n,
            fraction: mass,
            living: true,
        });
    }
    for k in 0..DWARF_BINS {
        let (lo, hi) = (
            edges(m_to, 8.0, DWARF_BINS, k),
            edges(m_to, 8.0, DWARF_BINS, k + 1),
        );
        let (n, _) = moments(lo, hi, &|_| 1.0);
        let (_, weighted) = moments(lo, hi, &|m| white_dwarf_mass(m) / m);
        let mean = weighted / n;
        classes.push(MassClass {
            mass: mean,
            fraction: n * mean,
            living: false,
        });
    }
    let (n, _) = moments(8.0, 25.0, &|_| 1.0);
    classes.push(MassClass {
        mass: NEUTRON_STAR_MASS,
        fraction: n * NEUTRON_STARS_KEPT * NEUTRON_STAR_MASS,
        living: false,
    });
    let total = classes.iter().fold(0.0, |s, c| s + c.fraction);
    for c in &mut classes {
        c.fraction /= total;
    }
    classes.push(MassClass {
        mass: m_to,
        fraction: TRACER_FRACTION,
        living: false,
    });
    (classes, m_to)
}

/// King's (`g = 1`) dimensionless density at `w`: `e^w erf √w − √(4w ÷ π) (1 + 2w ÷ 3)`, by its
/// series below 0.05, where the difference cancels.
#[must_use]
pub fn king_density(w: f64) -> f64 {
    if w <= 0.0 {
        return 0.0;
    }
    if w < 0.05 {
        // e^w erf √w − √(4w/π)(1 + 2w/3) = (8 ÷ 15√π) w^(5/2) (1 + 2w/7 + 4w²/63 + ...).
        let lead = 8.0 / (15.0 * core::f64::consts::PI.sqrt()) * w * w * w.sqrt();
        return lead * (1.0 + 2.0 * w / 7.0 + 4.0 * w * w / 63.0);
    }
    math::exp(w) * math::erf(w.sqrt())
        - (4.0 * w / core::f64::consts::PI).sqrt() * (1.0 + 2.0 * w / 3.0)
}

/// A solved multimass King model: radii in King radii.
#[derive(Debug, Clone, PartialEq)]
pub struct KingModel {
    /// The tidal radius.
    pub tidal: f64,
    /// The half-mass radius of all the classes but the tracer.
    pub half_mass: f64,
    /// Each class's half-mass radius, in the classes' order.
    pub class_half_mass: Vec<f64>,
    /// The mass fractions reached, in the classes' order.
    pub fractions: Vec<f64>,
}

/// Integrates the model with central potential `w0` and normalisations `a` once: the tidal
/// radius, and each class's cumulative mass along the radii.
fn integrate(w0: f64, exponents: &[f64], a: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let rho = |w: f64| -> f64 {
        exponents
            .iter()
            .zip(a)
            .fold(0.0, |s, (&e, &aj)| s + aj * king_density(e * w))
    };
    let rho0 = rho(w0);
    let n = exponents.len();
    // State: W, u = r² W', and each class's mass inside r (∫ r² ρ_j dr, unnormalised).
    let r = R_START;
    let mut state = vec![0.0; 2 + n];
    state[0] = w0 - 1.5 * r * r;
    state[1] = -3.0 * r * r * r;
    for j in 0..n {
        state[2 + j] = a[j] * king_density(exponents[j] * w0) / rho0 * r * r * r / 3.0;
    }
    let deriv = |x: f64, s: &[f64], out: &mut [f64]| {
        let r = math::exp(x);
        let w = s[0].max(0.0);
        out[0] = s[1] / r;
        let mut total = 0.0;
        for j in 0..n {
            let d = a[j] * king_density(exponents[j] * w) / rho0;
            total += d;
            out[2 + j] = r * r * r * d;
        }
        out[1] = -9.0 * r * r * r * total;
    };
    let mut radii = vec![r];
    let mut masses: Vec<Vec<f64>> = (0..n).map(|j| vec![state[2 + j]]).collect();
    let mut x = math::ln(r);
    let (mut k1, mut k2, mut k3, mut k4) = (
        vec![0.0; 2 + n],
        vec![0.0; 2 + n],
        vec![0.0; 2 + n],
        vec![0.0; 2 + n],
    );
    let mut tmp = vec![0.0; 2 + n];
    let h = LN_STEP;
    loop {
        deriv(x, &state, &mut k1);
        for i in 0..state.len() {
            tmp[i] = state[i] + 0.5 * h * k1[i];
        }
        deriv(x + 0.5 * h, &tmp, &mut k2);
        for i in 0..state.len() {
            tmp[i] = state[i] + 0.5 * h * k2[i];
        }
        deriv(x + 0.5 * h, &tmp, &mut k3);
        for i in 0..state.len() {
            tmp[i] = state[i] + h * k3[i];
        }
        deriv(x + h, &tmp, &mut k4);
        let mut next = state.clone();
        for i in 0..state.len() {
            next[i] = state[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        let x_next = x + h;
        if next[0] <= 0.0 {
            // The tidal radius, where W crosses zero, by linear interpolation in ln r.
            let f = state[0] / (state[0] - next[0]);
            let x_t = x + f * h;
            radii.push(math::exp(x_t));
            for j in 0..n {
                masses[j].push(state[2 + j] + f * (next[2 + j] - state[2 + j]));
            }
            break;
        }
        state = next;
        x = x_next;
        radii.push(math::exp(x));
        for j in 0..n {
            masses[j].push(state[2 + j]);
        }
        if x > 20.0 {
            break;
        }
    }
    (radii, masses)
}

/// The radius holding half of `cumulative`'s last value, by linear interpolation.
fn half_radius(radii: &[f64], cumulative: &[f64]) -> f64 {
    let half = 0.5 * cumulative[cumulative.len() - 1];
    let i = cumulative
        .partition_point(|&m| m < half)
        .clamp(1, cumulative.len() - 1);
    let (m0, m1) = (cumulative[i - 1], cumulative[i]);
    let t = if m1 > m0 {
        (half - m0) / (m1 - m0)
    } else {
        0.0
    };
    radii[i - 1] + t * (radii[i] - radii[i - 1])
}

/// Solves the model of central potential `w0` (the turn-off's) with `classes` and velocity-scale
/// exponent `delta`.
#[must_use]
pub fn solve(w0: f64, classes: &[MassClass], m_to: f64, delta: f64) -> KingModel {
    let exponents: Vec<f64> = classes
        .iter()
        .map(|c| math::powf(c.mass / m_to, 2.0 * delta))
        .collect();
    let target: Vec<f64> = classes.iter().map(|c| c.fraction).collect();
    let mut a: Vec<f64> = classes
        .iter()
        .zip(&exponents)
        .map(|(c, &e)| c.fraction / king_density(e * w0).max(1e-300))
        .collect();
    let mut result = integrate(w0, &exponents, &a);
    for _ in 0..NORMALISATION_ITERATIONS {
        let totals: Vec<f64> = result.1.iter().map(|m| m[m.len() - 1]).collect();
        let sum = totals.iter().fold(0.0, |s, t| s + t);
        for j in 0..a.len() {
            let now = totals[j] / sum;
            if now > 0.0 {
                a[j] *= target[j] / now;
            }
        }
        result = integrate(w0, &exponents, &a);
    }
    let (radii, masses) = result;
    let n = classes.len();
    let totals: Vec<f64> = masses.iter().map(|m| m[m.len() - 1]).collect();
    let sum = totals.iter().fold(0.0, |s, t| s + t);
    let all: Vec<f64> = (0..radii.len())
        .map(|i| (0..n - 1).fold(0.0, |s, j| s + masses[j][i]))
        .collect();
    KingModel {
        tidal: radii[radii.len() - 1],
        half_mass: half_radius(&radii, &all),
        class_half_mass: masses.iter().map(|m| half_radius(&radii, m)).collect(),
        fractions: totals.iter().map(|t| t / sum).collect(),
    }
}

/// The half-mass radius of the sim's class profile for a class of mass ratio `q` at exponent `eta`,
/// in a cluster of core `core`, half-mass radius `half_mass` and tidal radius `tidal`.
#[must_use]
pub fn profile_half_mass(q: f64, eta: f64, core: f64, half_mass: f64, tidal: f64) -> f64 {
    let q_prime = if q < 1.0 { math::powf(q, eta) } else { q };
    let exponent = 1.5 * q_prime;
    let shape = ProfileShape::Core {
        core,
        exponent,
        halo: half_mass,
        outer: (1.0 - exponent).max(0.0),
    };
    ClassProfile::new(shape, tidal, 1.0).radius_quantile(0.5)
}

/// The core radius whose `q = 1` profile has half-mass radius `target`, by bisection in `ln r_c`.
fn matched_core(target: f64, half_mass: f64, tidal: f64) -> f64 {
    let (mut lo, mut hi) = (math::ln(target * 1e-4), math::ln(target));
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if profile_half_mass(1.0, 1.0, math::exp(mid), half_mass, tidal) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    math::exp(0.5 * (lo + hi))
}

/// One model of the grid, reduced to what the fit reads.
#[derive(Debug, Clone, PartialEq)]
pub struct GridModel {
    /// Its central potential.
    pub w0: f64,
    /// Its depleted slope.
    pub alpha: f64,
    /// Its concentration `log₁₀(r_t ÷ r_c)`.
    pub concentration: f64,
    /// Its matched core radius, half-mass and tidal radii, King radii.
    pub core: f64,
    /// Its half-mass radius.
    pub half_mass: f64,
    /// Its tidal radius.
    pub tidal: f64,
    /// `(q, half-mass radius)` of each living class below the turn-off.
    pub living: Vec<(f64, f64)>,
    /// The largest share by which a class's mass fraction missed its target.
    pub normalisation_miss: f64,
}

/// The model at central potential `w0` and slope `alpha`.
#[must_use]
pub fn grid_model(w0: f64, alpha: f64, delta: f64) -> GridModel {
    let (classes, m_to) = mass_classes(alpha);
    let model = solve(w0, &classes, m_to, delta);
    let tracer = model.class_half_mass[classes.len() - 1];
    let core = matched_core(tracer, model.half_mass, model.tidal);
    let living = classes
        .iter()
        .zip(&model.class_half_mass)
        .filter(|(c, _)| c.living && c.mass < m_to)
        .map(|(c, &r)| (c.mass / m_to, r))
        .collect();
    let normalisation_miss = classes
        .iter()
        .zip(&model.fractions)
        .take(classes.len() - 1)
        .fold(0.0_f64, |m, (c, &f)| m.max((f / c.fraction - 1.0).abs()));
    GridModel {
        w0,
        alpha,
        concentration: math::log10(model.tidal / core),
        core,
        half_mass: model.half_mass,
        tidal: model.tidal,
        living,
        normalisation_miss,
    }
}

/// The sum of squared log errors of the living classes' half-mass radii at `eta`, and the largest
/// relative error.
#[must_use]
pub fn misfit(models: &[GridModel], eta: f64) -> (f64, f64) {
    let mut sum = 0.0;
    let mut worst = 0.0_f64;
    for m in models {
        for &(q, r) in &m.living {
            let fit = profile_half_mass(q, eta, m.core, m.half_mass, m.tidal);
            let e = math::ln(fit / r);
            sum += e * e;
            worst = worst.max((fit / r - 1.0).abs());
        }
    }
    (sum, worst)
}

/// η minimising [`misfit`] on `[lo, hi]` by golden-section search.
#[must_use]
pub fn fit_eta(models: &[GridModel], lo: f64, hi: f64) -> f64 {
    golden_section(|eta| misfit(models, eta).0, lo, hi, 60)
}

/// The fit: the grid's models and η.
#[derive(Debug, Clone, PartialEq)]
pub struct EquipartitionFit {
    /// The grid's models.
    pub models: Vec<GridModel>,
    /// The fitted η.
    pub eta: f64,
    /// The largest relative error of a living class's half-mass radius at η.
    pub worst: f64,
    /// The same at plan 09's η.
    pub worst_committed: f64,
    /// The velocity-scale exponent δ of the models.
    pub delta: f64,
}

/// Fits η over the grid of central potentials `w0s` by slopes `alphas` at `delta`.
#[must_use]
pub fn fit(w0s: &[f64], alphas: &[f64], delta: f64) -> EquipartitionFit {
    let models: Vec<GridModel> = alphas
        .iter()
        .flat_map(|&alpha| w0s.iter().map(move |&w0| grid_model(w0, alpha, delta)))
        .collect();
    let eta = fit_eta(&models, 0.0, 2.0);
    EquipartitionFit {
        worst: misfit(&models, eta).1,
        worst_committed: misfit(&models, SCRATCH_ETA).1,
        models,
        eta,
        delta,
    }
}

/// The table's block.
#[must_use]
pub fn render(eta: f64) -> RustTable {
    RustTable {
        summary: vec![
            "The mass-segregation exponent η of the cluster classes' profiles (plan 15, P15.T8.b)."
                .to_owned(),
        ],
        notes: Vec::new(),
        items: vec![TableItem::Scalar {
            name: "EQUIPARTITION_EXPONENT".to_owned(),
            doc: vec![
                "η of the class profiles' `q′ = q^η` below the turn-off: 1 is δ = ½ mass segregation"
                    .to_owned(),
                "(η = 2δ; ruling 139.2; Gieles and Zocchi 2015, MNRAS 454, 576), which P15.T8.b fits"
                    .to_owned(),
                "to multimass King models (the block's acceptance).".to_owned(),
            ],
            value: eta,
        }],
    }
}

/// A float list parameter of the manifest.
fn floats(manifest: &Manifest, key: &str) -> Result<Vec<f64>, ManifestParamError> {
    manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(|v| {
                    v.as_float().or_else(|| {
                        v.as_integer()
                            .and_then(|i| i32::try_from(i).ok())
                            .map(f64::from)
                    })
                })
                .collect::<Option<Vec<f64>>>()
        })
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ManifestParamError::new(key, "a non-empty list of numbers"))
}

/// The task (P15.T8.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct EquipartitionTask;

impl FitTask for EquipartitionTask {
    fn name(&self) -> &'static str {
        "equipartition"
    }

    /// About half a minute in a release build, most of it the models' normalisation: slow, so
    /// CI holds it to its inputs hash and runs its smoke manifest.
    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "cluster_dynamics.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["EQUIPARTITION_EXPONENT"]
    }

    /// The model cluster's turn-off, a class profile's half-mass radius, and the mass function
    /// and depletion the classes follow.
    fn fingerprint(&self) -> SimFingerprint {
        let composition = Composition::from_fe_h(Dex::new(FE_H), HeliumExcess::ZERO);
        let imf = Chabrier::provisional();
        SimFingerprint::new(vec![
            (
                "turn_off_mass(12 Gyr, [Fe/H] -1.3)".to_owned(),
                turn_off_mass(Years::new(AGE_YEARS), &composition).value(),
            ),
            (
                "profile_half_mass(0.5, 1, 1, 10, 100)".to_owned(),
                profile_half_mass(0.5, 1.0, 1.0, 10.0, 100.0),
            ),
            (
                "chabrier.pdf(0.3) / pdf(0.1)".to_owned(),
                imf.pdf(0.3) / imf.pdf(0.1),
            ),
            ("depletion(0.3, -0.5)".to_owned(), depletion(0.3, -0.5)),
            ("white_dwarf_mass(2)".to_owned(), white_dwarf_mass(2.0)),
        ])
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let w0s = floats(manifest, "central_potentials")?;
        let alphas = floats(manifest, "slopes")?;
        let delta = manifest.f64("delta")?;
        let fit = fit(&w0s, &alphas, delta);
        let (c_lo, c_hi) = fit
            .models
            .iter()
            .fold((f64::INFINITY, 0.0_f64), |(lo, hi), m| {
                (lo.min(m.concentration), hi.max(m.concentration))
            });
        let miss = fit
            .models
            .iter()
            .fold(0.0_f64, |m, g| m.max(g.normalisation_miss));
        let in_window = (0.8..=1.0).contains(&fit.eta);
        let radii = fit.worst <= 0.1;
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let acceptance = format!(
            "η = {:.4} at δ = {} over {} multimass King models (W₀ {:?}, slopes {:?}; \
             concentrations {:.2}–{:.2}; mass fractions within {:.1e}): in 0.8–1.0 (ruling 139.2; \
             {}); the living classes' half-mass radii below the turn-off within {:.3} ({}; 10%); \
             at plan 09's η = {} within {:.3}{}",
            fit.eta,
            fit.delta,
            fit.models.len(),
            w0s,
            alphas,
            c_lo,
            c_hi,
            miss,
            verdict(in_window),
            fit.worst,
            verdict(radii),
            SCRATCH_ETA,
            fit.worst_committed,
            if in_window && radii {
                ""
            } else {
                "; the table keeps plan 09's η until a ruling"
            },
        );
        Ok(TaskOutput {
            table: render(if in_window && radii {
                fit.eta
            } else {
                SCRATCH_ETA
            }),
            source: "multimass lowered-isothermal models (Da Costa and Freeman 1976, ApJ 206, \
                     128; Gunn and Griffin 1979, AJ 84, 752) in the parametrisation of Gieles and \
                     Zocchi (2015, MNRAS 454, 576), integrated by the task; plan 09's class \
                     profiles and counts"
                .to_owned(),
            acceptance,
            provisional: (!(in_window && radii)).then_some("P15.T8.b"),
        })
    }
}

/// A small index as a float.
fn index_f64(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a small index fits in u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// King's density: its series joins the closed form, and it rises with `w`.
    #[test]
    fn king_density_is_continuous_and_rising() {
        let below = king_density(0.05 - 1e-9);
        let above = king_density(0.05 + 1e-9);
        assert!((below / above - 1.0).abs() < 1e-5, "{below} {above}");
        let mut last = 0.0;
        for k in 1..100 {
            let v = king_density(0.1 * f64::from(k));
            assert!(v > last);
            last = v;
        }
    }

    /// A one-class model is King's: at W₀ = 6 the concentration log₁₀(r_t ÷ r₀) is about 1.25
    /// (King 1966's table), and the class holds all the mass.
    #[test]
    fn a_single_class_model_is_kings() {
        let classes = [MassClass {
            mass: 1.0,
            fraction: 1.0,
            living: true,
        }];
        let model = solve(6.0, &classes, 1.0, 0.5);
        let c = math::log10(model.tidal);
        assert!((c - 1.25).abs() < 0.03, "{c}");
        assert!((model.fractions[0] - 1.0).abs() < 1e-12);
    }

    /// Heavier classes sit deeper: the half-mass radii fall with mass, and the fractions converge.
    #[test]
    fn heavier_classes_are_more_concentrated() {
        let (classes, m_to) = mass_classes(-0.5);
        let model = solve(7.0, &classes, m_to, 0.5);
        let living: Vec<f64> = classes
            .iter()
            .zip(&model.class_half_mass)
            .filter(|(c, _)| c.living)
            .map(|(_, &r)| r)
            .collect();
        assert!(living.windows(2).all(|w| w[1] < w[0]), "{living:?}");
        for (c, f) in classes.iter().zip(&model.fractions).take(classes.len() - 1) {
            assert!((f / c.fraction - 1.0).abs() < 1e-6, "{c:?}: {f}");
        }
    }
}
