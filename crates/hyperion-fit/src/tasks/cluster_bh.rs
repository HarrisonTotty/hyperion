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
//! Revision 1 (P15.T8.d; decision-p15-t8-blocks) emits the adopted constants, [`SCRATCH`], as
//! production, whatever the fit finds: the fit's optimum is degenerate (ψ₁ unbounded, a pure
//! exponential), and no (β, ψ₁) follows the CMC models to 0.3 dex, because the closed form holds
//! mass and radius constant.
//!
//! Gates, at the adopted constants (the block is `@generated` when all pass, else `@provisional`
//! by P15.T8.a): on the Baumgardt–Hilker catalogue at 12 Gyr (plan 09's
//! `testing::catalogue_parameters`), read through the sim's own `ClusterModel` as plan 09's checks
//! read it, none in a cluster past the core-collapse line of 14 relaxation times (the most under
//! one), 20–400 in 47 Tucanae, 3,000–20,000 in ω Centauri (ruling 126.4's reading of
//! "thousands"), 10⁴–10⁵ over the catalogue, and 15–25% of the catalogue past the line with none
//! (Trager et al. 1995).
//!
//! Reported, not gated: the CMC fit's rms error in `log₁₀(1 + N)` over the models that retain any
//! (the plan's 0.3 dex) and the share of the models predicted empty that are empty (the plan's
//! 85%), each at the adopted constants and at the best fit; the best fit's ψ₁ and β ψ₁; and the
//! median over 3 × 10⁵ M☉ at the adopted constants, against the plan's "tens to a few hundred in a
//! typical massive one", for which the 47 Tucanae gate stands.
//!
//! The fit's records (`CmcModel`, `ClusterBhFit`, `CatalogueFigures`) are plain data with public
//! fields: the task's acceptance line and tests read them field by field.

use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, LIGHT_YEARS_PER_PARSEC};
use hyperion_sim::galaxy::features::cluster::{
    BLACK_HOLE_BIRTH_FRACTION, CORE_COLLAPSE_RELAXATION_TIMES, ClusterKind, ClusterMarks,
    ClusterModel, ClusterParameters, MEAN_BLACK_HOLE_MASS,
};
use hyperion_sim::galaxy::features::testing::{
    OMEGA_CEN, TUC_47, catalogue_parameters, milky_way_globulars,
};
use hyperion_sim::galaxy::imf::{Kroupa, MassFunction};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::tables::cluster_dynamics::{
    BH_CLOCK_FACTOR, BH_LOSS_BETA, BH_LOSS_PSI_SLOPE, BH_RELAXATION_PREFACTOR,
};
use hyperion_sim::units::{Dex, LightYears, SolarMasses, Years};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::optimise::nelder_mead;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 1 adopts [`SCRATCH`] as production (P15.T8.d).
pub const VERSION: u32 = 1;

/// The dataset's name.
pub const DATASET: &str = "cmc";

/// The adopted constants (β, ψ₁, k, c), production by decision-p15-t8-blocks: Antonini and
/// Gieles's (2020, MNRAS 492, 2936) β = 2.80 × 10⁻³ (their eq. 14, `Ṁ_BH = −β M_cl ÷ t_rh`) and
/// a₁ = 1.47 (eq. 12, `ψ = 1 + a₁ f_BH ÷ 0.01`), so ψ₁ = a₁ ÷ 0.01 = 147, their §2.4 fit to eight
/// isolated N-body clusters of 10⁵ stars at Z = 0.0006, extrapolated here to the Milky Way's
/// globulars; Spitzer's 0.138 of their eq. 10 (§2.3.2, ln Λ = 10); and ruling 126.4's k 2.5.
pub const SCRATCH: [f64; 4] = [2.8e-3, 147.0, 2.5, 0.138];

/// The compiled table's constants (β, ψ₁, k, c), which `ClusterModel` reads.
const COMPILED: [f64; 4] = [
    BH_LOSS_BETA,
    BH_LOSS_PSI_SLOPE,
    BH_CLOCK_FACTOR,
    BH_RELAXATION_PREFACTOR,
];

/// Whether `table` holds the adopted constants, [`SCRATCH`], bit for bit.
#[must_use]
fn holds_adopted(table: &[f64; 4]) -> bool {
    table
        .iter()
        .zip(&SCRATCH)
        .all(|(value, adopted)| value.total_cmp(adopted).is_eq())
}

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

/// The CMC fit's figures, reported in the acceptance line and gated on none of them (revision 1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterBhFit {
    /// The best fit's β, ψ₁, k, c.
    pub constants: [f64; 4],
    /// The rms error in `log₁₀(1 + N)` over the models that retain any at the best fit, dex.
    pub rms_dex: f64,
    /// The same at the adopted constants, [`SCRATCH`].
    pub rms_dex_adopted: f64,
    /// At the best fit: the models predicted empty (under one black hole), and how many of them
    /// are.
    pub predicted_empty: (usize, usize),
    /// The same at the adopted constants.
    pub predicted_empty_adopted: (usize, usize),
    /// Models that retain any.
    pub retaining: usize,
}

/// The Baumgardt–Hilker catalogue's black holes at 12 Gyr ([`catalogue_parameters`]), read
/// through the sim's own [`ClusterModel`], as plan 09's checks read them.
///
/// `ClusterModel` reads the compiled table, which holds [`SCRATCH`] bit for bit (the tests pin
/// both, and the task's `run` checks it), so these are the adopted constants' figures, the
/// revision's gates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CatalogueFigures {
    /// The most black holes in a cluster past the core-collapse line, older than
    /// [`CORE_COLLAPSE_RELAXATION_TIMES`] relaxation times.
    pub most_in_old: f64,
    /// 47 Tucanae's black holes.
    pub tuc_47: f64,
    /// ω Centauri's.
    pub omega_cen: f64,
    /// The black holes over the catalogue.
    pub total: f64,
    /// The share of the catalogue core-collapsed: past the line with none
    /// ([`ClusterModel::is_core_collapsed`]).
    pub collapsed_share: f64,
    /// The median over the clusters above 3 × 10⁵ M☉ that are not past the line: reported, not
    /// gated.
    pub median_massive: f64,
}

impl CatalogueFigures {
    /// The catalogue's figures in `galaxy`.
    #[must_use]
    pub fn of(galaxy: &Galaxy) -> Self {
        let mut massive = Vec::new();
        let mut figures = Self::default();
        let (mut collapsed, mut n) = (0_u32, 0_u32);
        for g in milky_way_globulars() {
            let model = ClusterModel::new(galaxy, &catalogue_parameters(g));
            let count = model.black_hole_count();
            // The line as `ClusterModel::is_core_collapsed` draws it.
            let old = model.age().value()
                > CORE_COLLAPSE_RELAXATION_TIMES * model.relaxation_time().value();
            if old {
                figures.most_in_old = figures.most_in_old.max(count);
            } else if g.mass > 3e5 {
                massive.push(count);
            }
            if g.name == TUC_47 {
                figures.tuc_47 = count;
            }
            if g.name == OMEGA_CEN {
                figures.omega_cen = count;
            }
            figures.total += count;
            collapsed += u32::from(model.is_core_collapsed());
            n += 1;
        }
        massive.sort_by(f64::total_cmp);
        figures.median_massive = massive.get(massive.len() / 2).copied().unwrap_or(0.0);
        figures.collapsed_share = f64::from(collapsed) / f64::from(n.max(1));
        figures
    }

    /// Whether each gate passes (ruling 126.4's windows, P09.T9.c–d): none in a cluster past the
    /// line (the most under one), 47 Tucanae 20–400, ω Centauri 3,000–20,000, 10⁴–10⁵ over the
    /// catalogue, and 15–25% core-collapsed (Trager et al. 1995).
    #[must_use]
    pub fn passed(&self) -> [bool; 5] {
        [
            self.most_in_old < 1.0,
            (20.0..=400.0).contains(&self.tuc_47),
            (3_000.0..=20_000.0).contains(&self.omega_cen),
            (1e4..=1e5).contains(&self.total),
            (0.15..=0.25).contains(&self.collapsed_share),
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

/// The models predicted empty (under one black hole) at `beta` and `psi`, and how many of them
/// are.
#[must_use]
fn predicted_empty(inputs: &[(LawInput, f64)], beta: f64, psi: f64) -> (usize, usize) {
    inputs
        .iter()
        .filter(|(input, _)| input.black_holes(beta, psi, SCRATCH[2], SCRATCH[3]) < 1.0)
        .fold((0, 0), |(predicted, empty), (_, observed)| {
            (predicted + 1, empty + usize::from(*observed < 1.0))
        })
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
    ClusterBhFit {
        constants: [beta, psi, SCRATCH[2], SCRATCH[3]],
        rms_dex: rms_dex(&inputs, beta, psi),
        rms_dex_adopted: rms_dex(&inputs, SCRATCH[0], SCRATCH[1]),
        predicted_empty: predicted_empty(&inputs, beta, psi),
        predicted_empty_adopted: predicted_empty(&inputs, SCRATCH[0], SCRATCH[1]),
        retaining: inputs.iter().filter(|(_, o)| *o > 0.0).count(),
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
        let catalogue = CatalogueFigures::of(&galaxy);
        let passed = catalogue.passed();
        // The gates read the compiled table through `ClusterModel`: they are the emitted
        // constants' only while the table holds them, so a run after `SCRATCH` moves is
        // provisional until the next run reads the table it wrote.
        let read_at_adopted = holds_adopted(&COMPILED);
        let all = passed.iter().all(|&ok| ok) && read_at_adopted;
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let [beta, psi, k, prefactor] = SCRATCH;
        let [best_beta, best_psi, _, _] = fit.constants;
        let acceptance = format!(
            "adopted β = {beta}, ψ₁ = {psi}, k = {k} and c = {prefactor} (Antonini and Gieles \
             2020; ruling 126.4; decision-p15-t8-blocks); on the Baumgardt–Hilker catalogue at \
             12 Gyr through the sim's ClusterModel, the most in a cluster past 14 t★ {:.2} (under \
             1; {}), 47 Tucanae {:.0} (20–400; {}), ω Centauri {:.0} (3,000–20,000; {}), {:.0} \
             over the catalogue (10⁴–10⁵; {}), {:.3} past the core-collapse line with none \
             (0.15–0.25; {}). Reported: the median over 3 × 10⁵ M☉ {:.0} (tens to a few hundred); the CMC \
             fit over {} models at 14 Gyr ({} of them dissolved to under 500 M☉ and left out), \
             {} retaining any, rms {:.3} dex in log₁₀(1 + N) over those at the adopted constants \
             and {:.3} at the best fit (under 0.3), {} of {} and {} of {} predicted empty are \
             empty (at least 85%), the best fit's ψ₁ = {best_psi:.2e} and β ψ₁ = {:.4} (β and c \
             enter only as β ÷ c){}",
            catalogue.most_in_old,
            verdict(passed[0]),
            catalogue.tuc_47,
            verdict(passed[1]),
            catalogue.omega_cen,
            verdict(passed[2]),
            catalogue.total,
            verdict(passed[3]),
            catalogue.collapsed_share,
            verdict(passed[4]),
            catalogue.median_massive,
            models.len(),
            models.iter().filter(|m| m.mass <= 0.0).count(),
            fit.retaining,
            fit.rms_dex_adopted,
            fit.rms_dex,
            fit.predicted_empty_adopted.1,
            fit.predicted_empty_adopted.0,
            fit.predicted_empty.1,
            fit.predicted_empty.0,
            best_beta * best_psi,
            if all {
                ""
            } else if read_at_adopted {
                "; provisional until a ruling on the failed gates"
            } else {
                "; provisional: the gates read the compiled table, not the constants emitted, so \
                 the task runs again"
            },
        );
        Ok(TaskOutput {
            table: render(SCRATCH),
            source: "Antonini and Gieles (2020, MNRAS 492, 2936), §2.4's fit and §2.3.2's eqs. \
                     10, 12 and 14, for the constants; Kremer et al. (2020, ApJS 247, 48), Table \
                     A1; Breen and Heggie (2013, MNRAS 432, 2779); Spitzer (1987); Kroupa (2001); \
                     the Baumgardt–Hilker catalogue as plan 09's tests hold it"
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

    /// The gates count only while the compiled table holds the adopted constants: it does today,
    /// and a moved constant would not.
    #[test]
    fn the_gates_are_the_adopted_constants_only_while_the_table_holds_them() {
        assert!(holds_adopted(&COMPILED));
        for k in 0..4 {
            let mut moved = SCRATCH;
            moved[k] *= 1.0 + f64::EPSILON;
            assert!(!holds_adopted(&moved), "{moved:?}");
        }
    }

    /// Each catalogue gate fails just outside its window and passes just inside it.
    #[test]
    fn each_catalogue_gate_fails_outside_its_window() {
        let inside = CatalogueFigures {
            most_in_old: 0.99,
            tuc_47: 20.0,
            omega_cen: 20_000.0,
            total: 1e4,
            collapsed_share: 0.25,
            median_massive: 0.0,
        };
        assert_eq!(inside.passed(), [true; 5]);
        let outside = [
            CatalogueFigures {
                most_in_old: 1.0,
                ..inside
            },
            CatalogueFigures {
                tuc_47: 401.0,
                ..inside
            },
            CatalogueFigures {
                omega_cen: 2_999.0,
                ..inside
            },
            CatalogueFigures {
                total: 1.01e5,
                ..inside
            },
            CatalogueFigures {
                collapsed_share: 0.149,
                ..inside
            },
        ];
        for (k, figures) in outside.iter().enumerate() {
            let mut expected = [true; 5];
            expected[k] = false;
            assert_eq!(figures.passed(), expected, "{figures:?}");
        }
    }

    #[test]
    fn kroupa_s_mean_mass_is_about_six_tenths() {
        let m = kroupa_mean_mass();
        assert!((0.55..0.7).contains(&m), "{m}");
    }
}
