//! P15.T7.b's fit of the helium-excess table to `BaSTI`'s helium-enhanced grid
//! ([`grid`](super::grid)), run from `manifests/helium.production.toml` (`method =
//! "basti_least_squares"`), and its figures on plan 06's tracks under the fitted table.
//!
//! - **`LIFETIME_SLOPE`.** At every point of the grid, a helium-enhanced set's track of mass m
//!   against the canonical sequence carried to the set's Z, the target is ln r, r = t(Y) ÷ t(Y₀)
//!   the ratio of the ages at the end of the main sequence, with ΔY = Y − Y₀(Z). The model is
//!   ln r = s(m, Z) ΔY in the table's shape: s a quadratic in x = log₁₀(m ÷ M☉) at each of
//!   `LOG_Z_NODES`, interpolated linearly in log₁₀ of the star's `z_fit` and held beyond the end
//!   nodes, as the hook reads it. It is linear in the twelve coefficients, solved by
//!   [`least_squares`] on the residuals in ln r: that is the brief's fit on ln r ÷ ΔY weighted by
//!   ΔY, so that each point counts by its own relative error in the ratio, which the acceptance
//!   holds to 3%.
//! - **`HB_TEMPERATURE_SHIFT`.** At every point of a helium-enhanced set's zero-age horizontal
//!   branch whose envelope mass lies in the manifest's range, the target is its log₁₀ `T_eff` less
//!   the canonical sequence's at the same envelope mass and Z; the model is (a + b `M_env`) ΔY,
//!   solved by [`least_squares`] unweighted in log₁₀ `T_eff`. The grid's branches at a higher Y
//!   are also fainter or brighter at the same envelope mass; the hook holds the luminosity, so
//!   only the temperature is fitted.
//! - **`LOG_Z_NODES`** are plan 06's, not fitted: the fit reads the sim's table for them.
//!
//! Every sum runs in the grid's file order, and every transcendental goes through
//! [`hyperion_sim::math`], so that the fitted bits are the same on every platform.

use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::sse::CandidateHeliumTable;
use hyperion_sim::tables::helium;
use hyperion_sim::units::{Dex, HeliumExcess};

use super::cluster::ClusterParams;
use super::grid::{Family, Grid, Set};
use super::{BASTI_LEAST_SQUARES, IDENTITY_BODY, METHOD_KEY, SIGNS, SUMMARY};
use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError};
use crate::optimise::least_squares;
use crate::task::{RunTaskError, TaskOutput};

/// The dataset the fit reads: T7.a's grid as P15.T7.b reduced it.
pub const DATASET: &str = "basti_helium";

/// Every key the fit's manifest holds; any other is refused.
pub const KEYS: [&str; 25] = [
    METHOD_KEY,
    "lifetime_terms",
    "mass_min",
    "mass_max",
    "fit_fe_h_min",
    "fit_fe_h_max",
    "hb_envelope_min",
    "hb_envelope_max",
    "acceptance_fe_h_min",
    "acceptance_fe_h_max",
    "lifetime_tolerance",
    "shortening_delta_y",
    "shortening_mass",
    "shortening_min",
    "shortening_max",
    "extrapolation_delta_y",
    "cluster_mass",
    "cluster_age",
    "cluster_z",
    "cluster_delta_y_max",
    "cluster_first_population",
    "cluster_window_margin",
    "cluster_window_bisections",
    "blue_end_quantile",
    "hb_tolerance",
];

/// The fit's parameters, every one its manifest's.
#[derive(Debug, Clone, PartialEq)]
pub struct FitParams {
    /// The terms of each node's slope fitted, `lifetime_terms`: 2 for s = c₀ + c₁ x with c₂ held
    /// at zero (ruled 2026-10-26, decision-p15-t7b-leans L2), 3 for the free quadratic.
    pub lifetime_terms: usize,
    /// The tracks' initial masses fitted and checked, M☉: `mass_min`–`mass_max`.
    pub masses_msun: (f64, f64),
    /// The sets fitted, by the sim's \[Fe/H\] of their Z: `fit_fe_h_min`–`fit_fe_h_max`.
    pub fit_fe_h: (f64, f64),
    /// The envelope masses of the branches fitted, M☉: `hb_envelope_min`–`hb_envelope_max`.
    pub hb_envelope_msun: (f64, f64),
    /// The \[Fe/H\] range of the acceptance's lifetime ratios (plan 15: −2.2 to −0.5). The ratios
    /// are held at every fitted point, whose rows bracket it, and stated inside it too.
    pub acceptance_fe_h: (f64, f64),
    /// The largest relative error of a lifetime ratio the acceptance allows (plan 15: 3%).
    pub lifetime_tolerance: f64,
    /// The ΔY of the shortening figure (plan 15: 0.1).
    pub shortening_delta_y: f64,
    /// The initial mass of the shortening figure, M☉ (plan 15: 0.8).
    pub shortening_mass_msun: f64,
    /// The shortening's window (plan 15: 0.25–0.4).
    pub shortening: (f64, f64),
    /// The ΔY the header's acceptance line extrapolates to (`HELIUM_EXCESS_MAX`, 0.18).
    pub extrapolation_delta_y: f64,
    /// The synthetic cluster's.
    pub cluster: ClusterParams,
    /// The share of the horizontal-branch blue end's heating the cluster must match (plan 15:
    /// 10%).
    pub hb_tolerance: f64,
}

/// A number of `manifest` in the open-closed range `lo < x ≤ hi`, or a [`ManifestParamError`].
fn bounded(manifest: &Manifest, key: &str, lo: f64, hi: f64) -> Result<f64, ManifestParamError> {
    let value = manifest.f64(key)?;
    if value > lo && value <= hi {
        Ok(value)
    } else {
        Err(ManifestParamError::new(
            key,
            &format!("a number above {lo} and at most {hi}"),
        ))
    }
}

/// The pair `(lo, hi)` of `manifest`'s keys `lo_key` and `hi_key`, finite with `lo < hi`.
fn range(
    manifest: &Manifest,
    lo_key: &str,
    hi_key: &str,
) -> Result<(f64, f64), ManifestParamError> {
    let (lo, hi) = (manifest.f64(lo_key)?, manifest.f64(hi_key)?);
    if lo.is_finite() && hi.is_finite() && lo < hi {
        Ok((lo, hi))
    } else {
        Err(ManifestParamError::new(
            hi_key,
            &format!("finite and above `{lo_key}`"),
        ))
    }
}

impl FitParams {
    /// Reads the fit's parameters from `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if `method` is not [`BASTI_LEAST_SQUARES`], a key of [`KEYS`] is
    /// missing or out of its range, or another key is present.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        if manifest.str(METHOD_KEY)? != BASTI_LEAST_SQUARES {
            return Err(ManifestParamError::new(
                METHOD_KEY,
                &format!("\"{BASTI_LEAST_SQUARES}\""),
            ));
        }
        if let Some(extra) = manifest
            .params()
            .keys()
            .find(|k| !KEYS.contains(&k.as_str()))
        {
            return Err(ManifestParamError::new(
                extra,
                "absent: the fit reads no such key",
            ));
        }
        let masses_msun = range(manifest, "mass_min", "mass_max")?;
        if masses_msun.0 <= 0.0 {
            return Err(ManifestParamError::new("mass_min", "positive"));
        }
        let shortening = range(manifest, "shortening_min", "shortening_max")?;
        let lifetime_terms = match manifest.u64("lifetime_terms")? {
            2 => 2,
            3 => 3,
            _ => {
                return Err(ManifestParamError::new(
                    "lifetime_terms",
                    "2 (c₂ held at zero) or 3 (the free quadratic)",
                ));
            }
        };
        Ok(Self {
            lifetime_terms,
            masses_msun,
            fit_fe_h: range(manifest, "fit_fe_h_min", "fit_fe_h_max")?,
            hb_envelope_msun: range(manifest, "hb_envelope_min", "hb_envelope_max")?,
            acceptance_fe_h: range(manifest, "acceptance_fe_h_min", "acceptance_fe_h_max")?,
            lifetime_tolerance: bounded(manifest, "lifetime_tolerance", 0.0, 1.0)?,
            shortening_delta_y: bounded(manifest, "shortening_delta_y", 0.0, 1.0)?,
            shortening_mass_msun: bounded(manifest, "shortening_mass", 0.0, 150.0)?,
            shortening,
            extrapolation_delta_y: bounded(manifest, "extrapolation_delta_y", 0.0, 1.0)?,
            cluster: ClusterParams::from_manifest(manifest)?,
            hb_tolerance: bounded(manifest, "hb_tolerance", 0.0, 1.0)?,
        })
    }
}

/// log₁₀ of the `z_fit` that the sim's track reads for a star of the sim's \[Fe/H\] `fe_h`.
#[must_use]
pub fn log_z_fit(fe_h: f64) -> f64 {
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    math::log10(composition.z_fit().value())
}

/// The weights of the table's four nodes at log₁₀ Z `log_z`, as the hook takes them
/// (`HeliumHook::of`): linear between the bracketing nodes, all on the end node beyond it.
#[must_use]
pub fn node_weights(log_z: f64, nodes: &[f64; 4]) -> [f64; 4] {
    let last = nodes.len() - 1;
    let mut weights = [0.0; 4];
    if log_z <= nodes[0] {
        weights[0] = 1.0;
    } else if log_z >= nodes[last] {
        weights[last] = 1.0;
    } else {
        let index = nodes[1..last]
            .iter()
            .take_while(|&&node| log_z >= node)
            .count();
        let share = (log_z - nodes[index]) / (nodes[index + 1] - nodes[index]);
        weights[index] = 1.0 - share;
        weights[index + 1] = share;
    }
    weights
}

/// s(m, Z) of `slope` at log₁₀ Z `log_z` and initial mass `mass_msun` (M☉), in the hook's
/// arithmetic: the coefficients interpolated first, then the quadratic evaluated.
#[must_use]
pub fn slope_at(slope: &[[f64; 3]; 4], nodes: &[f64; 4], log_z: f64, mass_msun: f64) -> f64 {
    let w = node_weights(log_z, nodes);
    let last = nodes.len() - 1;
    let c: [f64; 3] = if log_z <= nodes[0] {
        slope[0]
    } else if log_z >= nodes[last] {
        slope[last]
    } else {
        let index = w.iter().position(|&x| x > 0.0).unwrap_or(0);
        let share = w[index + 1];
        std::array::from_fn(|k| (1.0 - share) * slope[index][k] + share * slope[index + 1][k])
    };
    let x = math::log10(mass_msun);
    c[0] + x * (c[1] + x * c[2])
}

/// One point of the lifetime fit: a helium-enhanced set's track and the canonical sequence's at
/// the same mass and Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LifetimePoint {
    /// The helium-enhanced set.
    pub set: Set,
    /// The initial mass, M☉.
    pub mass_msun: f64,
    /// ΔY = Y − Y₀(Z).
    pub delta_y: f64,
    /// ln of the grid's ratio of the ages at the end of the main sequence, t(Y) ÷ t(Y₀).
    pub ln_ratio: f64,
    /// log₁₀ of the `z_fit` the sim reads for a star of the set's Z.
    pub log_z: f64,
}

/// The lifetime fit's points: every helium-enhanced set within `params.fit_fe_h`, at each of its
/// masses within `params.masses_msun` that both canonical sets carrying it have.
#[must_use]
pub fn lifetime_points(grid: &Grid, params: &FitParams) -> Vec<LifetimePoint> {
    let mut points = Vec::new();
    for set in grid.sets() {
        let fe_h = set.fe_h();
        if set.is_canonical() || fe_h < params.fit_fe_h.0 || fe_h > params.fit_fe_h.1 {
            continue;
        }
        let Some(y0) = grid.canonical_y(set.z) else {
            continue;
        };
        // The rows of the set itself, read from the same file as `set`: exact matches.
        let own = grid.turn_offs().iter().filter(|t| {
            t.z_row.total_cmp(&set.z_row).is_eq()
                && t.z.total_cmp(&set.z).is_eq()
                && t.y.total_cmp(&set.y).is_eq()
        });
        for t in own {
            let (lo, hi) = params.masses_msun;
            if t.mass_msun < lo || t.mass_msun > hi {
                continue;
            }
            let Some(ln_age0) = grid.canonical_ln_age_yr(set.z, t.mass_msun) else {
                continue;
            };
            points.push(LifetimePoint {
                set,
                mass_msun: t.mass_msun,
                delta_y: set.y - y0,
                ln_ratio: std::f64::consts::LN_10 * t.log_age_yr - ln_age0,
                log_z: log_z_fit(fe_h),
            });
        }
    }
    points
}

/// The least-squares `LIFETIME_SLOPE` on `points` at `nodes`, each node's slope of `terms`
/// terms (1–3: c₀, then c₁ x, then c₂ x²) and the rest exactly zero, or `None` if `terms` is
/// outside 1–3 or a node's coefficients are not determined by the points.
#[must_use]
pub fn fit_lifetime(
    points: &[LifetimePoint],
    nodes: &[f64; 4],
    terms: usize,
) -> Option<[[f64; 3]; 4]> {
    if !(1..=3).contains(&terms) || points.len() < 4 * terms {
        return None;
    }
    let rows: Vec<Vec<f64>> = points
        .iter()
        .map(|p| {
            let w = node_weights(p.log_z, nodes);
            let x = math::log10(p.mass_msun);
            let powers = [1.0, x, x * x];
            (0..4 * terms)
                .map(|j| p.delta_y * w[j / terms] * powers[j % terms])
                .collect()
        })
        .collect();
    let rhs: Vec<f64> = points.iter().map(|p| p.ln_ratio).collect();
    let c = least_squares(&rows, &rhs)?;
    Some(std::array::from_fn(|node| {
        std::array::from_fn(|power| {
            if power < terms {
                c[terms * node + power]
            } else {
                0.0
            }
        })
    }))
}

/// One point of the horizontal-branch fit: a helium-enhanced set's zero-age branch at one
/// envelope mass, against the canonical sequence's at the same envelope mass and Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchPoint {
    /// The helium-enhanced set.
    pub set: Set,
    /// The envelope mass, M☉.
    pub envelope_msun: f64,
    /// ΔY = Y − Y₀(Z).
    pub delta_y: f64,
    /// log₁₀ `T_eff` less the canonical sequence's.
    pub shift: f64,
}

/// The horizontal-branch fit's points: every point of every helium-enhanced set's branch within
/// `params.fit_fe_h` and `params.hb_envelope_msun` that the canonical branches carrying it
/// reach.
#[must_use]
pub fn branch_points(grid: &Grid, params: &FitParams) -> Vec<BranchPoint> {
    let mut points = Vec::new();
    for set in grid.sets() {
        let fe_h = set.fe_h();
        if set.is_canonical() || fe_h < params.fit_fe_h.0 || fe_h > params.fit_fe_h.1 {
            continue;
        }
        let (Some(y0), Some(canonical)) = (
            grid.canonical_y(set.z),
            grid.carried_branch(Family::Canonical, set.z),
        ) else {
            continue;
        };
        for p in grid.branch(&set) {
            let (lo, hi) = params.hb_envelope_msun;
            if p.envelope_msun < lo || p.envelope_msun > hi {
                continue;
            }
            let Some(reference) = canonical.log_teff(p.envelope_msun) else {
                continue;
            };
            points.push(BranchPoint {
                set,
                envelope_msun: p.envelope_msun,
                delta_y: set.y - y0,
                shift: p.log_teff - reference,
            });
        }
    }
    points
}

/// The least-squares `HB_TEMPERATURE_SHIFT` (a, b) on `points`, or `None` if it is not
/// determined.
#[must_use]
pub fn fit_branch(points: &[BranchPoint]) -> Option<[f64; 2]> {
    if points.len() < 2 {
        return None;
    }
    let rows: Vec<[f64; 2]> = points
        .iter()
        .map(|p| [p.delta_y, p.delta_y * p.envelope_msun])
        .collect();
    let rhs: Vec<f64> = points.iter().map(|p| p.shift).collect();
    let c = least_squares(&rows, &rhs)?;
    Some([c[0], c[1]])
}

/// The fitted table and the points it was fitted on.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// The candidate table: plan 06's nodes, the fitted slope and shift.
    pub table: CandidateHeliumTable,
    /// The lifetime fit's points.
    pub lifetime: Vec<LifetimePoint>,
    /// The horizontal-branch fit's points.
    pub branch: Vec<BranchPoint>,
}

/// The fit of the helium-excess table to `grid` under `params`, or `None` if either part is not
/// determined.
#[must_use]
pub fn fit(grid: &Grid, params: &FitParams) -> Option<Fit> {
    let lifetime = lifetime_points(grid, params);
    let branch = branch_points(grid, params);
    let slope = fit_lifetime(&lifetime, &helium::LOG_Z_NODES, params.lifetime_terms)?;
    let shift = fit_branch(&branch)?;
    Some(Fit {
        table: CandidateHeliumTable::new(slope, shift),
        lifetime,
        branch,
    })
}

/// The committed text of the `LOG_Z_NODES` item, which the fitted body keeps: plan 06's nodes are
/// not fitted.
#[must_use]
pub fn nodes_item() -> &'static str {
    IDENTITY_BODY
        .find("\n\n")
        .map_or(IDENTITY_BODY, |end| &IDENTITY_BODY[..=end])
}

/// The fitted table: P06.T17's summary, `LOG_Z_NODES` as committed, and the fitted slope and
/// shift with the fit's own doc comments.
#[must_use]
pub fn render(table: &CandidateHeliumTable, params: &FitParams) -> RustTable {
    let notes: Vec<String> = [
        "Fitted (plan 15, P15.T7.b) by least squares to the α-enhanced models of Pietrinferni, Cassisi,",
        "Salaris and Castelli (2006, ApJ 642, 797) and their database's helium-enhanced extension at",
        "Y = 0.30, 0.35 and 0.40 (`data/basti_helium/`), relative to the canonical sequence at the same",
        "Z, each set placed at its own total Z (Salaris, Chieffi and Straniero 1993, ApJ 414, 580; the",
        "equivalence holds to Z ≈ 0.002, Pietrinferni et al. 2006, §1). The grid's own shift of the",
        "horizontal branch peaks near an envelope of 0.06 M☉ and vanishes for the thinnest and the",
        "thickest envelopes, which the linear form only approximates.",
        "The track applies the correction only where ΔY > 0, so every star the grid places (ΔY = 0) is",
        "bit-identical whatever the table holds; members of globular clusters' second populations",
        "(ΔY > 0, plan 09) move with it.",
        "",
    ]
    .iter()
    .chain(&SIGNS)
    .map(|&l| l.to_owned())
    .collect();
    let mut slope_doc = vec![
        "The slope s(m, Z) of the log of the main-sequence and giant-branch timescales in ΔY, at each of"
            .to_owned(),
        "[`LOG_Z_NODES`]: the coefficients (c₀, c₁, c₂) of s = c₀ + c₁ x + c₂ x², x = log₁₀(m ÷ M☉). Between"
            .to_owned(),
        "the nodes s is interpolated linearly in log₁₀ Z, and held at the end nodes outside them. The"
            .to_owned(),
        "timescales are multiplied by exp(s × ΔY). Fitted to the ratios of the ages at central".to_owned(),
        format!(
            "hydrogen exhaustion of the grid's {}–{} M☉ tracks at Y = 0.30–0.40 to the canonical ones",
            literal(params.masses_msun.0),
            literal(params.masses_msun.1)
        ),
        "at the same mass and Z (P15.T7.b).".to_owned(),
    ];
    if params.lifetime_terms < 3 {
        slope_doc.extend([
            "c₂ is held at zero, so that s is linear in x and stays negative from 0.1 to 150 M☉ (ruled"
                .to_owned(),
            "2026-10-26, decision-p15-t7b-leans L2); outside the fitted masses it is the linear form's"
                .to_owned(),
            "extrapolation.".to_owned(),
        ]);
    }
    let shift_doc = vec![
        "The shift of log₁₀ `T_eff` on the horizontal branch per unit ΔY, linear in the envelope mass"
            .to_owned(),
        "`M_env` (M☉): (a, b) of a + b `M_env`. The radius changes to keep the luminosity. Fitted to"
            .to_owned(),
        "the grid's zero-age horizontal branches at Y = 0.30–0.40 against the canonical ones at the same"
            .to_owned(),
        format!(
            "Z and envelope mass, over envelopes of {}–{} M☉ (P15.T7.b).",
            literal(params.hb_envelope_msun.0),
            literal(params.hb_envelope_msun.1)
        ),
    ];
    RustTable {
        summary: SUMMARY.iter().map(|&l| l.to_owned()).collect(),
        notes,
        items: vec![
            TableItem::Source(nodes_item().to_owned()),
            TableItem::Nested {
                name: "LIFETIME_SLOPE".to_owned(),
                doc: slope_doc,
                rows: table.lifetime_slope().iter().map(|r| r.to_vec()).collect(),
            },
            TableItem::Array {
                name: "HB_TEMPERATURE_SHIFT".to_owned(),
                doc: shift_doc,
                values: table.hb_temperature_shift().to_vec(),
            },
        ],
    }
}

/// The fit's run on `manifest`: the fitted table, its figures in the acceptance line, `@generated`
/// when every figure passes and provisional by P15.T7.b otherwise.
///
/// # Errors
///
/// [`RunTaskError`] if a parameter is missing or wrong, the dataset cannot be read, or the fit
/// is not determined.
pub fn run(manifest: &Manifest) -> Result<TaskOutput, RunTaskError> {
    let params = FitParams::from_manifest(manifest)?;
    let input_error = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
        task: "helium",
        source,
    };
    let dataset = manifest.load_dataset(DATASET)?;
    let text = |name: &str| -> Result<&str, RunTaskError> {
        let bytes = dataset
            .files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_slice())
            .ok_or_else(|| input_error(format!("the dataset has no {name}").into()))?;
        std::str::from_utf8(bytes).map_err(|e| input_error(Box::new(e)))
    };
    let grid = Grid::parse(text("turnoff.csv")?, text("zahb.csv")?)
        .map_err(|e| input_error(Box::new(e)))?;
    let fitted = fit(&grid, &params)
        .ok_or_else(|| input_error("the grid does not determine the fit".into()))?;
    let figures = Figures::of(&fitted, &grid, &params);
    let passed = figures.passed(&params);
    let mut acceptance = String::new();
    figures.describe(&fitted, &params, &mut acceptance);
    Ok(TaskOutput {
        table: render(&fitted.table, &params),
        source: "Pietrinferni, Cassisi, Salaris and Castelli (2006, ApJ 642, 797): their \
                 α-enhanced canonical models with the database's helium-enhanced extension, Y = \
                 0.30, 0.35 and 0.40 (`data/basti_helium/`), placed at their total Z after \
                 Salaris, Chieffi and Straniero (1993, ApJ 414, 580); plan 06's tracks"
            .to_owned(),
        acceptance,
        provisional: if passed { None } else { Some("P15.T7.b") },
    })
}

pub use super::figures::Figures;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_node_weights_follow_the_hook() {
        let nodes = helium::LOG_Z_NODES;
        assert_eq!(node_weights(-5.0, &nodes), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(node_weights(-1.0, &nodes), [0.0, 0.0, 0.0, 1.0]);
        let mid = f64::midpoint(nodes[1], nodes[2]);
        let w = node_weights(mid, &nodes);
        assert!(
            (w[1] - 0.5).abs() < 1e-12 && (w[2] - 0.5).abs() < 1e-12,
            "{w:?}"
        );
        assert_eq!(node_weights(nodes[1], &nodes), [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn the_slope_is_interpolated_as_the_hook_does() {
        let nodes = helium::LOG_Z_NODES;
        let slope = [
            [-4.0, 0.0, 0.0],
            [-6.0, 1.0, 0.0],
            [-8.0, 0.0, 2.0],
            [-2.0, 0.0, 0.0],
        ];
        assert!((slope_at(&slope, &nodes, -6.0, 0.8) + 4.0).abs() < 1e-15);
        assert!((slope_at(&slope, &nodes, 0.0, 0.8) + 2.0).abs() < 1e-15);
        let mid = f64::midpoint(nodes[1], nodes[2]);
        let x = math::log10(0.8);
        let expected = -7.0 + 0.5 * x + x * x;
        assert!((slope_at(&slope, &nodes, mid, 0.8) - expected).abs() < 1e-12);
    }

    #[test]
    fn the_nodes_item_is_the_committed_text() {
        assert!(nodes_item().starts_with("/// The metallicities"));
        assert!(nodes_item().ends_with("];\n"));
        assert_eq!(nodes_item().matches("pub const").count(), 1);
    }

    /// Synthetic points of a known slope are fitted back.
    #[test]
    fn the_lifetime_fit_recovers_a_known_slope() {
        let nodes = helium::LOG_Z_NODES;
        let truth = [
            [-6.0, 1.0, 0.5],
            [-6.5, 0.5, 0.0],
            [-7.0, 0.0, -0.5],
            [-7.5, -1.0, 0.0],
        ];
        let set = Set {
            z_row: 0.001,
            z: 0.0008,
            y: 0.4,
        };
        let mut points = Vec::new();
        for &log_z in &[-4.0, -3.6, -3.1, -2.8, -2.45, -2.1] {
            for &mass_msun in &[0.6, 0.7, 0.8, 0.9] {
                for &delta_y in &[0.05, 0.1, 0.15] {
                    points.push(LifetimePoint {
                        set,
                        mass_msun,
                        delta_y,
                        ln_ratio: slope_at(&truth, &nodes, log_z, mass_msun) * delta_y,
                        log_z,
                    });
                }
            }
        }
        let fitted = fit_lifetime(&points, &nodes, 3).unwrap();
        for (row, want) in fitted.iter().zip(&truth) {
            for (c, w) in row.iter().zip(want) {
                assert!((c - w).abs() < 1e-8, "{fitted:?}");
            }
        }
    }

    /// With two terms the fit recovers a linear slope, and every c₂ is exactly zero; another number
    /// of terms is refused.
    #[test]
    fn the_two_term_fit_holds_c2_at_zero() {
        let nodes = helium::LOG_Z_NODES;
        let truth = [
            [-6.0, 1.0, 0.0],
            [-6.5, 0.5, 0.0],
            [-7.0, 0.0, 0.0],
            [-7.5, -1.0, 0.0],
        ];
        let set = Set {
            z_row: 0.001,
            z: 0.0008,
            y: 0.4,
        };
        let points: Vec<LifetimePoint> = [-4.0, -3.6, -3.1, -2.8, -2.45, -2.1]
            .iter()
            .flat_map(|&log_z| {
                [0.6, 0.7, 0.8, 0.9].map(move |mass_msun| LifetimePoint {
                    set,
                    mass_msun,
                    delta_y: 0.1,
                    ln_ratio: slope_at(&truth, &nodes, log_z, mass_msun) * 0.1,
                    log_z,
                })
            })
            .collect();
        let fitted = fit_lifetime(&points, &nodes, 2).unwrap();
        for (row, want) in fitted.iter().zip(&truth) {
            assert!(
                (row[0] - want[0]).abs() < 1e-9 && (row[1] - want[1]).abs() < 1e-9,
                "{fitted:?}"
            );
            assert!(row[2].total_cmp(&0.0).is_eq(), "{fitted:?}");
        }
        assert_eq!(fit_lifetime(&points, &nodes, 0), None);
        assert_eq!(fit_lifetime(&points, &nodes, 4), None);
    }

    #[test]
    fn the_branch_fit_recovers_a_known_shift() {
        let set = Set {
            z_row: 0.001,
            z: 0.0008,
            y: 0.4,
        };
        let points: Vec<BranchPoint> = [0.01, 0.05, 0.1, 0.15]
            .iter()
            .flat_map(|&envelope_msun| {
                [0.05, 0.15].map(|delta_y| BranchPoint {
                    set,
                    envelope_msun,
                    delta_y,
                    shift: (0.5 - 2.0 * envelope_msun) * delta_y,
                })
            })
            .collect();
        let [a, b] = fit_branch(&points).unwrap();
        assert!(
            (a - 0.5).abs() < 1e-12 && (b + 2.0).abs() < 1e-12,
            "{a} {b}"
        );
    }

    #[test]
    fn a_manifest_with_an_unknown_key_or_another_method_is_refused() {
        let text = "task = \"helium\"\n[params]\nmethod = \"basti_least_squares\"\nmass_min = 0.6\n\
                    stray = 1\n";
        let manifest = Manifest::from_bytes(
            std::path::Path::new("helium.toml"),
            text.as_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(
            FitParams::from_manifest(&manifest).unwrap_err(),
            ManifestParamError::new("stray", "absent: the fit reads no such key")
        );
        let text = "task = \"helium\"\n[params]\nmethod = \"identity\"\n";
        let manifest = Manifest::from_bytes(
            std::path::Path::new("helium.toml"),
            text.as_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(
            FitParams::from_manifest(&manifest).unwrap_err(),
            ManifestParamError::new(METHOD_KEY, "\"basti_least_squares\"")
        );
    }

    #[test]
    fn the_rendered_fit_keeps_the_committed_nodes_and_declares_the_three_items() {
        let table = CandidateHeliumTable::new([[-6.5, 0.25, 0.0]; 4], [0.5, -2.5]);
        let params_text = include_str!("../../../manifests/helium.production.toml");
        let manifest = Manifest::from_bytes(
            std::path::Path::new("helium.production.toml"),
            params_text.as_bytes().to_vec(),
        )
        .unwrap();
        let params = FitParams::from_manifest(&manifest).unwrap();
        let body = render(&table, &params).body().unwrap();
        assert!(body.starts_with(&format!("\n{}", nodes_item())), "{body}");
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|l| l.strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, super::super::ITEMS);
        assert!(body.contains(&literal(-2.5)), "{body}");
    }
}
