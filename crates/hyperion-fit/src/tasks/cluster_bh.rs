//! The black holes' loss from clusters against the CMC Cluster Catalog (plan 15, P15.T8.a;
//! consumer plan 09's `galaxy::features::cluster`).
//!
//! Plan 09's law is Breen and Heggie's (2013) as parametrised by Antonini and Gieles (2020): the
//! black holes' mass fraction `f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ k t ÷ t★) − 1] ÷ ψ₁`, floored at
//! zero, with `t★ = c √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ)`, the clock factor `k` (ruling 126.4) and `f₀`
//! 0.06 times the retention at the birth escape speed; the count is `f M ÷ 15 M☉`. The task fits
//! it to the 124 models of Kremer et al. (2020) that reach 14 Gyr, each placed in the sim's own
//! `ClusterModel` for its relaxation time and its black holes' retention at birth:
//!
//! - its mass, core radius and three-dimensional half-mass radius at 14 Gyr, the last `4 ÷ 3` of
//!   the paper's half-light radius (the projected-to-3D ratio of Spitzer 1987, §1.2; light is taken
//!   to follow mass);
//! - its birth mass `N ⟨m⟩₀`, `⟨m⟩₀` the mean of Kroupa's (2001) function over 0.08–150 M☉, which
//!   the models draw from, and its birth half-mass radius 0.78 of its virial radius (a Plummer
//!   sphere's ratio, Heggie and Hut 2003, §8.3);
//! - its [Fe/H] `log₁₀(Z ÷ Z☉)` and its galactocentric distance in the plane.
//!
//! `t★` enters only as `β ÷ c`: the prefactor and β cannot both be fitted. The task keeps `c` at
//! Spitzer's 0.138 (a relaxation time, not a free constant) and `k` at 2.5 (ruling 126.4), and fits
//! β and ψ₁ by Nelder and Mead on the squared error of `log₁₀(1 + N)`.
//!
//! Acceptance (the plan): rms error in `log₁₀` of the count under 0.3 dex over the models that
//! retain any; at least 85% of models predicted empty are empty; and on the Baumgardt–Hilker
//! catalogue at 12 Gyr (plan 09's `testing::catalogue_parameters`) none in dynamically old
//! clusters, tens to a few hundred in a typical massive one, thousands in ω Centauri and 15–25%
//! of the catalogue beyond the core-collapse line of 14 relaxation times with none (Trager et al.
//! 1995).

use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, LIGHT_YEARS_PER_PARSEC};
use hyperion_sim::galaxy::features::cluster::{
    BLACK_HOLE_BIRTH_FRACTION, CORE_COLLAPSE_RELAXATION_TIMES, ClusterKind, ClusterMarks,
    ClusterModel, ClusterParameters, MEAN_BLACK_HOLE_MASS,
};
use hyperion_sim::galaxy::features::testing::{
    OMEGA_CEN, catalogue_parameters, milky_way_globulars,
};
use hyperion_sim::galaxy::imf::{Kroupa, MassFunction};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::tables::cluster_dynamics::BH_RELAXATION_PREFACTOR;
use hyperion_sim::units::{Dex, LightYears, SolarMasses, Years};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::optimise::nelder_mead;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The dataset's name.
pub const DATASET: &str = "cmc";

/// Plan 09's scratch values, which the table keeps while the fit misses its acceptance.
pub const SCRATCH: [f64; 4] = [2.8e-3, 147.0, 2.5, 0.138];

/// The models' age at the end of their runs, years (Kremer et al. 2020, §2).
pub const CMC_AGE: f64 = 14e9;

/// The three-dimensional half-mass radius over the projected half-light radius.
pub const HALF_MASS_OVER_HALF_LIGHT: f64 = 4.0 / 3.0;

/// The half-mass radius over the virial radius at birth.
pub const HALF_MASS_OVER_VIRIAL: f64 = 0.78;

/// One CMC model at 14 Gyr, as the dataset gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct CmcModel {
    /// Its name, such as `n8-rv2-rg8-z0.1`.
    pub name: String,
    /// Initial N, in 10⁵.
    pub n: f64,
    /// Virial radius, pc.
    pub rv: f64,
    /// Galactocentric distance, kpc.
    pub rgc: f64,
    /// Metallicity, Z☉.
    pub z: f64,
    /// Mass at 14 Gyr, M☉.
    pub mass: f64,
    /// Core radius at 14 Gyr, pc.
    pub rc: f64,
    /// Half-light radius at 14 Gyr, pc.
    pub rh: f64,
    /// Black holes at 14 Gyr.
    pub black_holes: f64,
}

/// Parses the dataset's `models.csv`.
///
/// # Errors
///
/// A description of the first malformed line.
pub fn parse_models(text: &str) -> Result<Vec<CmcModel>, String> {
    let mut models = Vec::new();
    for (i, line) in text.lines().enumerate().skip(1) {
        let cells: Vec<&str> = line.split(',').collect();
        let num = |k: usize| -> Result<f64, String> {
            cells
                .get(k)
                .and_then(|c| c.trim().parse::<f64>().ok())
                .ok_or_else(|| format!("line {}: column {k} is not a number", i + 1))
        };
        if cells.len() != 10 {
            return Err(format!("line {}: {} columns, not 10", i + 1, cells.len()));
        }
        models.push(CmcModel {
            name: cells[0].to_owned(),
            n: num(1)?,
            rv: num(2)?,
            rgc: num(3)?,
            z: num(4)?,
            mass: num(5)? * 1e5,
            rc: num(6)?,
            rh: num(7)?,
            black_holes: num(9)?,
        });
    }
    Ok(models)
}

/// What the law needs of a cluster: its black holes' birth fraction, its age over `t★` at
/// Spitzer's prefactor, and its mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LawInput {
    /// `f₀`.
    pub f0: f64,
    /// `age ÷ t★` with `c` = 0.138.
    pub clock: f64,
    /// Mass, M☉.
    pub mass: f64,
}

impl LawInput {
    /// The input of `model`, read at its own parameters.
    #[must_use]
    pub fn of(model: &ClusterModel) -> Self {
        let t_star = model.relaxation_time().value() * 0.138 / BH_RELAXATION_PREFACTOR;
        Self {
            f0: BLACK_HOLE_BIRTH_FRACTION * model.retention().black_holes,
            clock: model.age().value() / t_star,
            mass: model.mass().value(),
        }
    }

    /// The black holes by the law with `beta`, `psi`, clock factor `k` and prefactor `c`.
    #[must_use]
    pub fn black_holes(&self, beta: f64, psi: f64, k: f64, c: f64) -> f64 {
        let x = k * self.clock * 0.138 / c;
        let f = (((1.0 + psi * self.f0) * math::exp(-beta * psi * x) - 1.0) / psi).max(0.0);
        f * self.mass / MEAN_BLACK_HOLE_MASS
    }

    /// Whether the cluster is past the core-collapse line with none left, under the law.
    #[must_use]
    pub fn core_collapsed(&self, beta: f64, psi: f64, k: f64, c: f64) -> bool {
        self.black_holes(beta, psi, k, c) <= 0.0
            && self.clock * 0.138 / c > CORE_COLLAPSE_RELAXATION_TIMES
    }
}

/// The mean mass, M☉, of Kroupa's (2001) function over 0.08–150 M☉.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the quadrature's usual symbols"
)]
pub fn kroupa_mean_mass() -> f64 {
    let imf = Kroupa;
    let edges = [0.08, 0.5, 1.0, 8.0, 150.0];
    let (mut n, mut m) = (0.0, 0.0);
    for w in edges.windows(2) {
        let (a, b) = (math::ln(w[0]), math::ln(w[1]));
        n += hyperion_sim::galaxy::quad::gl32(|t| imf.pdf(math::exp(t)) * math::exp(t), a, b);
        m += hyperion_sim::galaxy::quad::gl32(
            |t| {
                let x = math::exp(t);
                imf.pdf(x) * x * x
            },
            a,
            b,
        );
    }
    m / n
}

/// The sim's model of a CMC model at 14 Gyr.
#[must_use]
pub fn cmc_cluster(galaxy: &Galaxy, model: &CmcModel, mean_birth_mass: f64) -> ClusterModel {
    let r_h = model.rh * HALF_MASS_OVER_HALF_LIGHT * LIGHT_YEARS_PER_PARSEC;
    let initial = (model.n * 1e5 * mean_birth_mass).max(model.mass);
    let parameters = ClusterParameters {
        kind: ClusterKind::Globular,
        population: Population::Halo,
        component: None,
        position: PointLy::new(0.0, model.rgc * LIGHT_YEARS_PER_KILOPARSEC, 0.0),
        age: Years::new(CMC_AGE),
        fe_h: Dex::new(math::log10(model.z)),
        mass: SolarMasses::new(model.mass),
        initial_mass: SolarMasses::new(initial),
        half_mass_radius: LightYears::new(r_h),
        birth_half_mass_radius: LightYears::new(
            model.rv * HALF_MASS_OVER_VIRIAL * LIGHT_YEARS_PER_PARSEC,
        ),
        core_radius: Some(LightYears::new(model.rc * LIGHT_YEARS_PER_PARSEC)),
        concentration: 0.0,
        mass_loss_rate: 0.0,
        age_spread: Years::ZERO,
        marks: ClusterMarks::MEDIAN,
    };
    ClusterModel::new(galaxy, &parameters)
}

/// The fit's figures.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterBhFit {
    /// β, ψ₁, k, c.
    pub constants: [f64; 4],
    /// The rms error in `log₁₀` of the count over the models that retain any, dex.
    pub rms_dex: f64,
    /// The same at the scratch constants.
    pub rms_dex_scratch: f64,
    /// Models predicted empty (under one black hole), and how many of them are.
    pub predicted_empty: (usize, usize),
    /// Models that retain any.
    pub retaining: usize,
    /// On the Baumgardt–Hilker catalogue at 12 Gyr: the most in a cluster past 14 `t★`, the
    /// median in clusters over 3 × 10⁵ M☉ not past it, ω Centauri's count, and the share past
    /// the core-collapse line with none.
    pub catalogue: CatalogueFigures,
}

/// The fit's figures on the Baumgardt–Hilker catalogue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogueFigures {
    /// The most black holes in a cluster past 14 `t★`.
    pub most_in_old: f64,
    /// The median in clusters over 3 × 10⁵ M☉ that are not.
    pub median_massive: f64,
    /// ω Centauri's.
    pub omega_cen: f64,
    /// The share past the core-collapse line with none.
    pub collapsed_share: f64,
}

impl ClusterBhFit {
    /// Whether each check passes: the rms, the empty ones, none in old clusters, tens to a few
    /// hundred in massive ones, thousands in ω Centauri, a fifth collapsed.
    #[must_use]
    pub fn passed(&self) -> [bool; 6] {
        let (predicted, empty) = self.predicted_empty;
        let c = self.catalogue;
        [
            self.rms_dex < 0.3,
            predicted == 0 || usize_f64(empty) >= 0.85 * usize_f64(predicted),
            c.most_in_old < 1.0,
            (10.0..=500.0).contains(&c.median_massive),
            (1_000.0..=10_000.0).contains(&c.omega_cen),
            (0.15..=0.25).contains(&c.collapsed_share),
        ]
    }
}

/// The squared-error objective of `log₁₀(1 + N)`.
fn objective(inputs: &[(LawInput, f64)], beta: f64, psi: f64) -> f64 {
    inputs.iter().fold(0.0, |s, (input, observed)| {
        let predicted = input.black_holes(beta, psi, SCRATCH[2], SCRATCH[3]);
        let e = math::log10(1.0 + predicted) - math::log10(1.0 + observed);
        s + e * e
    })
}

/// The rms error in `log₁₀(1 + N)` over the models that retain any.
fn rms_dex(inputs: &[(LawInput, f64)], beta: f64, psi: f64) -> f64 {
    let (sum, n) = inputs.iter().filter(|(_, observed)| *observed > 0.0).fold(
        (0.0, 0_usize),
        |(s, n), (input, observed)| {
            let predicted = input.black_holes(beta, psi, SCRATCH[2], SCRATCH[3]);
            let e = math::log10(1.0 + predicted) - math::log10(1.0 + observed);
            (s + e * e, n + 1)
        },
    );
    (sum / usize_f64(n.max(1))).sqrt()
}

/// The catalogue figures at `beta` and `psi`.
fn catalogue(galaxy: &Galaxy, beta: f64, psi: f64) -> CatalogueFigures {
    let (k, c) = (SCRATCH[2], SCRATCH[3]);
    let mut massive = Vec::new();
    let (mut most_in_old, mut omega_cen, mut collapsed, mut n) = (0.0_f64, 0.0, 0_u32, 0_u32);
    for g in milky_way_globulars() {
        let model = ClusterModel::new(galaxy, &catalogue_parameters(g));
        let input = LawInput::of(&model);
        let count = input.black_holes(beta, psi, k, c);
        let old = input.clock * 0.138 / c > CORE_COLLAPSE_RELAXATION_TIMES;
        if old {
            most_in_old = most_in_old.max(count);
        } else if g.mass > 3e5 {
            massive.push(count);
        }
        if g.name == OMEGA_CEN {
            omega_cen = count;
        }
        collapsed += u32::from(input.core_collapsed(beta, psi, k, c));
        n += 1;
    }
    massive.sort_by(f64::total_cmp);
    CatalogueFigures {
        most_in_old,
        median_massive: massive.get(massive.len() / 2).copied().unwrap_or(0.0),
        omega_cen,
        collapsed_share: f64::from(collapsed) / f64::from(n.max(1)),
    }
}

/// Fits β and ψ₁ to `models` in `galaxy`.
#[must_use]
pub fn fit(galaxy: &Galaxy, models: &[CmcModel]) -> ClusterBhFit {
    let mean_birth_mass = kroupa_mean_mass();
    // A model whose mass the paper rounds to 0.00 × 10⁵ M☉ has all but dissolved: left out.
    let inputs: Vec<(LawInput, f64)> = models
        .iter()
        .filter(|m| m.mass > 0.0)
        .map(|m| {
            (
                LawInput::of(&cmc_cluster(galaxy, m, mean_birth_mass)),
                m.black_holes,
            )
        })
        .collect();
    let (best, _) = nelder_mead(
        |v| objective(&inputs, math::exp(v[0]), math::exp(v[1])),
        &[math::ln(SCRATCH[0]), math::ln(SCRATCH[1])],
        &[0.5, 0.5],
        400,
    );
    let (beta, psi) = (math::exp(best[0]), math::exp(best[1]));
    let predicted: Vec<(f64, f64)> = inputs
        .iter()
        .map(|(input, observed)| {
            (
                input.black_holes(beta, psi, SCRATCH[2], SCRATCH[3]),
                *observed,
            )
        })
        .collect();
    let empty = predicted.iter().filter(|(p, _)| *p < 1.0);
    ClusterBhFit {
        constants: [beta, psi, SCRATCH[2], SCRATCH[3]],
        rms_dex: rms_dex(&inputs, beta, psi),
        rms_dex_scratch: rms_dex(&inputs, SCRATCH[0], SCRATCH[1]),
        predicted_empty: (
            empty.clone().count(),
            empty.filter(|(_, o)| *o < 1.0).count(),
        ),
        retaining: predicted.iter().filter(|(_, o)| *o > 0.0).count(),
        catalogue: catalogue(galaxy, beta, psi),
    }
}

/// The table's block with `constants` (β, ψ₁, k, c).
#[must_use]
pub fn render(constants: [f64; 4]) -> RustTable {
    let scalar = |name: &str, doc: &[&str], value: f64| TableItem::Scalar {
        name: name.to_owned(),
        doc: doc.iter().map(|&l| l.to_owned()).collect(),
        value,
    };
    RustTable {
        summary: vec![
            "The black holes' loss from clusters (plan 15, P15.T8.a): Breen and Heggie's (2013) law"
                .to_owned(),
            "as parametrised by Antonini and Gieles (2020), against the CMC Cluster Catalog.".to_owned(),
        ],
        notes: Vec::new(),
        items: vec![
            scalar(
                "BH_LOSS_BETA",
                &["β: black-hole mass lost per relaxation time, in cluster masses (Antonini and Gieles 2020)."],
                constants[0],
            ),
            scalar(
                "BH_LOSS_PSI_SLOPE",
                &[
                    "ψ₁: how fast the relaxation time shortens with the black holes' mass fraction f,",
                    "`1 + ψ₁ f` (Antonini and Gieles 2020).",
                ],
                constants[1],
            ),
            scalar(
                "BH_CLOCK_FACTOR",
                &["The factor on age ÷ t★ in the black-hole law only (ruling 126.4)."],
                constants[2],
            ),
            scalar(
                "BH_RELAXATION_PREFACTOR",
                &["The prefactor of the half-mass relaxation time, 0.138 (Spitzer 1987, eq. 2-63)."],
                constants[3],
            ),
        ],
    }
}

/// The task (P15.T8.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ClusterBhTask;

impl FitTask for ClusterBhTask {
    fn name(&self) -> &'static str {
        "cluster_bh"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "cluster_dynamics.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "BH_LOSS_BETA",
            "BH_LOSS_PSI_SLOPE",
            "BH_CLOCK_FACTOR",
            "BH_RELAXATION_PREFACTOR",
        ]
    }

    /// Kroupa's mean mass, and one CMC-like cluster's law inputs in the Milky Way.
    fn fingerprint(&self) -> SimFingerprint {
        let probes =
            match Galaxy::from_params(Seed::new(0x0918_0001), GalaxyParams::milky_way_like()) {
                Ok(galaxy) => {
                    let model = CmcModel {
                        name: "probe".to_owned(),
                        n: 8.0,
                        rv: 2.0,
                        rgc: 8.0,
                        z: 0.1,
                        mass: 2.3e5,
                        rc: 1.0,
                        rh: 3.0,
                        black_holes: 0.0,
                    };
                    let input = LawInput::of(&cmc_cluster(&galaxy, &model, kroupa_mean_mass()));
                    vec![
                        ("kroupa_mean_mass".to_owned(), kroupa_mean_mass()),
                        ("probe.f0".to_owned(), input.f0),
                        ("probe.clock".to_owned(), input.clock),
                    ]
                }
                Err(_) => vec![("kroupa_mean_mass".to_owned(), kroupa_mean_mass())],
            };
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let input_error = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
            task: "cluster_bh",
            source,
        };
        let dataset = manifest.load_dataset(DATASET)?;
        let text =
            std::str::from_utf8(&dataset.files[0].1).map_err(|e| input_error(Box::new(e)))?;
        let models = parse_models(text).map_err(|e| input_error(e.into()))?;
        let seed = Seed::new(manifest.u64("seed")?);
        let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
            .map_err(|e| input_error(Box::new(e)))?;
        let fit = fit(&galaxy, &models);
        let passed = fit.passed();
        let all = passed.iter().all(|&ok| ok);
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let c = fit.catalogue;
        let [beta, psi, k, prefactor] = fit.constants;
        let acceptance = format!(
            "fitted β = {beta:.4e}, ψ₁ = {psi:.2} (β ψ₁ = {:.4}) at k = {k} and c = {prefactor} \
             (β and c enter only as β ÷ c) over {} CMC models at 14 Gyr ({} of them dissolved to \
             under 500 M☉ and left out), {} retaining any: rms \
             {:.3} dex in log₁₀(1 + N) over those (under 0.3; {}; {:.3} at the scratch \
             constants); {} of {} predicted empty are empty ({}); on the Baumgardt–Hilker \
             catalogue at 12 Gyr, the most in a cluster past 14 t★ {:.2} ({}), the median over \
             3 × 10⁵ M☉ {:.0} (tens to a few hundred; {}), ω Centauri {:.0} (thousands; {}), \
             {:.3} past the core-collapse line with none (0.15–0.25; {}){}",
            beta * psi,
            models.len(),
            models.iter().filter(|m| m.mass <= 0.0).count(),
            fit.retaining,
            fit.rms_dex,
            verdict(passed[0]),
            fit.rms_dex_scratch,
            fit.predicted_empty.1,
            fit.predicted_empty.0,
            verdict(passed[1]),
            c.most_in_old,
            verdict(passed[2]),
            c.median_massive,
            verdict(passed[3]),
            c.omega_cen,
            verdict(passed[4]),
            c.collapsed_share,
            verdict(passed[5]),
            if all {
                String::new()
            } else {
                format!(
                    "; the table keeps plan 09's scratch β {}, ψ₁ {} until a ruling",
                    SCRATCH[0], SCRATCH[1]
                )
            },
        );
        let constants = if all { fit.constants } else { SCRATCH };
        Ok(TaskOutput {
            table: render(constants),
            source: "Kremer et al. (2020, ApJS 247, 48), Table A1; Breen and Heggie (2013, MNRAS \
                     432, 2779); Antonini and Gieles (2020, MNRAS 492, 2936); Spitzer (1987); \
                     Kroupa (2001); the Baumgardt–Hilker catalogue as plan 09's tests hold it"
                .to_owned(),
            acceptance,
            provisional: (!all).then_some("P15.T8.a"),
        })
    }
}

/// A count as a float.
fn usize_f64(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a count under 2³²"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_models_parse() {
        let text = "name,n_1e5,rv_pc,rgc_kpc,z_solar,mass_1e5,rc_pc,rh_pc,neutron_stars,black_holes\n\
                    n8-rv2-rg8-z0.1,8,2,8,0.1,2.30,1.0,3.0,494,120\n";
        let models = parse_models(text).unwrap();
        assert_eq!(models.len(), 1);
        assert!((models[0].mass - 2.3e5).abs() < 1e-6);
        assert!((models[0].black_holes - 120.0).abs() < 1e-12);
        assert!(parse_models("h\n1,2\n").is_err());
    }

    /// The law at the scratch constants is plan 09's: gone after `ln(1 + ψ₁ f₀) ÷ (β ψ₁)` on its
    /// clock.
    #[test]
    fn the_law_is_plan_nines() {
        let input = LawInput {
            f0: 0.05,
            clock: 1.0,
            mass: 1e5,
        };
        let full = input.black_holes(SCRATCH[0], SCRATCH[1], 0.0, 0.138);
        assert!(
            (full - 0.05 * 1e5 / MEAN_BLACK_HOLE_MASS).abs() < 1e-9,
            "{full}"
        );
        let life = math::ln_1p(SCRATCH[1] * 0.05) / (SCRATCH[0] * SCRATCH[1]);
        let gone = LawInput {
            clock: life / 2.5 * 1.001,
            ..input
        };
        assert!(gone.black_holes(SCRATCH[0], SCRATCH[1], 2.5, 0.138) <= 0.0);
        let kept = LawInput {
            clock: life / 2.5 * 0.999,
            ..input
        };
        assert!(kept.black_holes(SCRATCH[0], SCRATCH[1], 2.5, 0.138) > 0.0);
    }

    #[test]
    fn kroupa_s_mean_mass_is_about_six_tenths() {
        let m = kroupa_mean_mass();
        assert!((0.55..0.7).contains(&m), "{m}");
    }
}
