//! Millisecond pulsars against the encounter rate (plan 15, P15.T8.c; consumer plan 09's
//! `galaxy::features::cluster::ClusterModel::millisecond_pulsars`).
//!
//! Plan 09's law: a cluster holds `A (Γ ÷ Γ_47Tuc)^γ` millisecond pulsars, a core-collapsed one at
//! most the count at 47 Tucanae's Γ, `A`. The task fits `A` and `γ` by Poisson regression of the
//! census of pulsars per cluster (Freire's table) on Bahramian et al.'s (2013) Γ, normalised to 47
//! Tucanae's 1,000:
//!
//! `E[N_i] = min(A (Γ_i ÷ 1000)^γ, A if core-collapsed) × C(d_i)`, `C(d) = 1 ÷ (1 + (d ÷ d₀)²)`,
//!
//! the completeness of a flux-limited search for a luminosity function `N(> L) ∝ L^−1` (Hessels et
//! al. 2007, ApJ 670, 363, fit a cumulative slope near −1), saturating inside `d₀`, which is fitted
//! with `A` and `γ` by maximum likelihood (Nelder and Mead in `ln A`, `γ`, `ln d₀`). A cluster of
//! Bahramian's table absent from Freire's has none known. Its distance is Bahramian's, or Freire's
//! where Bahramian gives none; clusters with neither are left out, as is Freire's one host missing
//! from Bahramian's table (GLIMPSE-C01).
//!
//! Acceptance (the plan): `γ` within 0.5–0.9; about 40 (30–50) at 47 Tucanae's Γ; the implied
//! total over the catalogue, `Σ min(A (Γ_i ÷ 1000)^γ, cap)`, within 2,000–8,000.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use hyperion_sim::math;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::optimise::nelder_mead;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// Plan 09's scratch values (`A`, `γ`, the cap), which the table keeps while the fit misses its
/// acceptance.
pub const SCRATCH: [f64; 3] = [40.0, 0.7, 40.0];

/// One cluster of the regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Host {
    /// Γ ÷ 47 Tucanae's.
    pub gamma: f64,
    /// Heliocentric distance, kpc.
    pub distance: f64,
    /// Whether Harris flags it core-collapsed.
    pub core_collapsed: bool,
    /// Its known pulsars.
    pub pulsars: f64,
}

/// The regression's clusters from the two datasets' CSV texts, keyed and sorted by name, and the
/// number of clusters left out for want of a distance.
///
/// # Errors
///
/// A description of the first malformed line.
pub fn hosts(gamma_csv: &str, counts_csv: &str) -> Result<(Vec<Host>, usize), String> {
    let mut counts = BTreeMap::new();
    for (i, line) in counts_csv.lines().enumerate().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        let (Some(key), Some(d), Some(n)) = (c.first(), c.get(1), c.get(2)) else {
            return Err(format!("counts line {}: too few columns", i + 1));
        };
        let d: f64 = d
            .parse()
            .map_err(|_| format!("counts line {}: distance", i + 1))?;
        let n: f64 = n
            .parse()
            .map_err(|_| format!("counts line {}: count", i + 1))?;
        counts.insert((*key).to_owned(), (d, n));
    }
    let mut out = Vec::new();
    let mut dropped = 0;
    for (i, line) in gamma_csv.lines().enumerate().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        if c.len() != 7 {
            return Err(format!("gamma line {}: {} columns, not 7", i + 1, c.len()));
        }
        let gamma: f64 = c[2]
            .parse()
            .map_err(|_| format!("gamma line {}: Γ", i + 1))?;
        let known = counts.get(c[0]).copied();
        let distance = match (c[1].parse::<f64>(), known) {
            (Ok(d), _) => d,
            (Err(_), Some((d, _))) => d,
            (Err(_), None) => {
                dropped += 1;
                continue;
            }
        };
        out.push(Host {
            gamma: gamma / 1000.0,
            distance,
            core_collapsed: c[6] == "1",
            pulsars: known.map_or(0.0, |(_, n)| n),
        });
    }
    Ok((out, dropped))
}

/// The expected pulsars of `host` before completeness at `a`, `gamma`, with the cap `a`.
#[must_use]
pub fn law(host: &Host, a: f64, gamma: f64) -> f64 {
    let n = a * math::powf(host.gamma, gamma);
    if host.core_collapsed { n.min(a) } else { n }
}

/// The negative log-likelihood (less the constant `Σ ln N!`).
fn negative_log_likelihood(hosts: &[Host], a: f64, gamma: f64, d0: f64) -> f64 {
    hosts.iter().fold(0.0, |s, h| {
        let x = h.distance / d0;
        let lambda = (law(h, a, gamma) / (1.0 + x * x)).max(1e-300);
        s + lambda - h.pulsars * math::ln(lambda)
    })
}

/// The fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsarFit {
    /// `A`, pulsars at 47 Tucanae's Γ.
    pub a: f64,
    /// `γ`.
    pub gamma: f64,
    /// The completeness distance, kpc.
    pub d0: f64,
    /// The implied total over the catalogue.
    pub total: f64,
    /// The census's total over the regression's clusters.
    pub observed: f64,
    /// The clusters regressed and those left out.
    pub clusters: (usize, usize),
}

impl PulsarFit {
    /// Whether each check passes: `γ`, `A`, the total.
    #[must_use]
    pub fn passed(&self) -> [bool; 3] {
        [
            (0.5..=0.9).contains(&self.gamma),
            (30.0..=50.0).contains(&self.a),
            (2_000.0..=8_000.0).contains(&self.total),
        ]
    }
}

/// Fits `A`, `γ` and `d₀` to `hosts`.
#[must_use]
pub fn fit(hosts: &[Host], dropped: usize) -> PulsarFit {
    let (best, _) = nelder_mead(
        |v| negative_log_likelihood(hosts, math::exp(v[0]), v[1], math::exp(v[2])),
        &[math::ln(SCRATCH[0]), SCRATCH[1], math::ln(10.0)],
        &[0.3, 0.1, 0.3],
        2_000,
    );
    let (a, gamma, d0) = (math::exp(best[0]), best[1], math::exp(best[2]));
    PulsarFit {
        a,
        gamma,
        d0,
        total: hosts.iter().fold(0.0, |s, h| s + law(h, a, gamma)),
        observed: hosts.iter().fold(0.0, |s, h| s + h.pulsars),
        clusters: (hosts.len(), dropped),
    }
}

/// The table's block with `constants` (`A`, `γ`, the cap).
#[must_use]
pub fn render(constants: [f64; 3]) -> RustTable {
    let scalar = |name: &str, doc: &[&str], value: f64| TableItem::Scalar {
        name: name.to_owned(),
        doc: doc.iter().map(|&l| l.to_owned()).collect(),
        value,
    };
    RustTable {
        summary: vec![
            "The millisecond pulsars' encounter-rate law (plan 15, P15.T8.c), against the census of"
                .to_owned(),
            "pulsars in globular clusters and Bahramian et al.'s (2013) encounter rates.".to_owned(),
        ],
        notes: Vec::new(),
        items: vec![
            scalar(
                "PULSARS_AT_47_TUC_GAMMA",
                &["The millisecond pulsars a cluster of 47 Tucanae's encounter rate holds."],
                constants[0],
            ),
            scalar(
                "PULSAR_GAMMA_EXPONENT",
                &["The exponent of the pulsars' encounter-rate law."],
                constants[1],
            ),
            scalar(
                "PULSAR_CORE_COLLAPSE_CAP",
                &["The most millisecond pulsars a core-collapsed cluster holds: the count at 47 Tucanae's Γ."],
                constants[2],
            ),
        ],
    }
}

/// The task (P15.T8.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PulsarsTask;

impl FitTask for PulsarsTask {
    fn name(&self) -> &'static str {
        "pulsars"
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
            "PULSARS_AT_47_TUC_GAMMA",
            "PULSAR_GAMMA_EXPONENT",
            "PULSAR_CORE_COLLAPSE_CAP",
        ]
    }

    /// None: the fit reads only its datasets and `math`.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let input_error = |why: String| RunTaskError::Input {
            task: "pulsars",
            source: why.into(),
        };
        let text = |name: &str| -> Result<String, RunTaskError> {
            let dataset = manifest.load_dataset(name)?;
            String::from_utf8(dataset.files[0].1.clone()).map_err(|e| input_error(e.to_string()))
        };
        let (hosts, dropped) =
            hosts(&text("bahramian_2013")?, &text("gc_pulsars")?).map_err(input_error)?;
        let fit = fit(&hosts, dropped);
        let passed = fit.passed();
        let all = passed.iter().all(|&ok| ok);
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let acceptance = format!(
            "fitted over {} clusters ({} left out for want of a distance; {:.0} known pulsars): γ = \
             {:.3} (0.5–0.9; {}), A = {:.1} at 47 Tucanae's Γ (30–50; {}), completeness distance \
             {:.1} kpc; the implied total over the catalogue {:.0} (2,000–8,000; {}){}",
            fit.clusters.0,
            fit.clusters.1,
            fit.observed,
            fit.gamma,
            verdict(passed[0]),
            fit.a,
            verdict(passed[1]),
            fit.d0,
            fit.total,
            verdict(passed[2]),
            if all {
                String::new()
            } else {
                format!(
                    "; the table keeps plan 09's scratch A {}, γ {} and cap {} until a ruling",
                    SCRATCH[0], SCRATCH[1], SCRATCH[2]
                )
            },
        );
        let constants = if all {
            [fit.a, fit.gamma, fit.a]
        } else {
            SCRATCH
        };
        Ok(TaskOutput {
            table: render(constants),
            source: "P. C. C. Freire's table of pulsars in globular clusters (retrieved \
                     2026-09-30); Bahramian et al. (2013, ApJ 766, 136); Hessels et al. (2007, ApJ \
                     670, 363) for the completeness's form"
                .to_owned(),
            acceptance,
            provisional: (!all).then_some("P15.T8.c"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAMMA: &str = "key,distance_kpc,gamma,gamma_lower,gamma_upper,estimate,core_collapsed\n\
                         a,4.5,1000.0,900,1100,gamma,0\nb,,10.0,5,20,gamma3,1\nc,,5.0,1,9,gamma3,0\n";
    const COUNTS: &str = "key,distance_kpc,pulsars\na,4.5,42\nb,8.0,3\n";

    #[test]
    fn hosts_join_the_census_to_the_rates() {
        let (hosts, dropped) = hosts(GAMMA, COUNTS).unwrap();
        assert_eq!(dropped, 1);
        assert_eq!(hosts.len(), 2);
        assert!((hosts[0].gamma - 1.0).abs() < 1e-12 && (hosts[0].pulsars - 42.0).abs() < 1e-12);
        assert!(hosts[1].core_collapsed && (hosts[1].distance - 8.0).abs() < 1e-12);
    }

    /// A core-collapsed cluster is capped at `A`; others are not.
    #[test]
    fn the_law_caps_core_collapsed_clusters() {
        let host = Host {
            gamma: 10.0,
            distance: 1.0,
            core_collapsed: true,
            pulsars: 0.0,
        };
        assert!((law(&host, 40.0, 0.7) - 40.0).abs() < 1e-12);
        let open = Host {
            core_collapsed: false,
            ..host
        };
        assert!((law(&open, 40.0, 0.7) - 40.0 * math::powf(10.0, 0.7)).abs() < 1e-9);
    }

    /// The fit recovers its parameters from counts drawn at their expectations.
    #[test]
    fn the_fit_recovers_expected_counts() {
        let hosts: Vec<Host> = (0..60)
            .map(|i| {
                let gamma = math::powf(10.0, -2.0 + 0.06 * f64::from(i));
                let distance = 3.0 + 0.3 * f64::from(i % 17);
                let mut h = Host {
                    gamma,
                    distance,
                    core_collapsed: false,
                    pulsars: 0.0,
                };
                let x = distance / 12.0;
                h.pulsars = law(&h, 35.0, 0.65) / (1.0 + x * x);
                h
            })
            .collect();
        let f = fit(&hosts, 0);
        assert!((f.a / 35.0 - 1.0).abs() < 1e-3, "{f:?}");
        assert!((f.gamma - 0.65).abs() < 1e-3, "{f:?}");
        assert!((f.d0 / 12.0 - 1.0).abs() < 1e-3, "{f:?}");
    }
}
