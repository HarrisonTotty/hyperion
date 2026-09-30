//! The stripping table (plan 11, P11.T1.d): the stripped share and the stage radii of massive
//! primaries over initial mass and metallicity, which
//! `hyperion_sim::galaxy::displaced::binarity::StrippingTable` interpolates.
//!
//! Each node is [`StrippingNode::of`]: the primary's full track at the median draws, unstripped,
//! for its largest radii to the ends of its main sequence, Hertzsprung gap and core helium burning,
//! and plan 11's quadrature of the companions the hierarchy draw gives over the stripping band
//! those radii make (ruling 123.2). The grid is the sim's ([`stripping_masses`],
//! [`stripping_metallicities`]). The run validates the interpolant at the centre of every cell
//! against the exact node there. Every node costs a track and the quadrature, some ten
//! milliseconds, so the task is slow.

use std::num::NonZeroUsize;

use hyperion_sim::galaxy::displaced::binarity::{
    STRIPPING_MASSES, STRIPPING_METALLICITIES, StrippingNode, StrippingTable, exact_stripped_share,
    stripping_masses, stripping_metallicities,
};
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The masses, M☉, and \[Fe/H\] of the fingerprint's probes.
pub const FINGERPRINT_POINTS: [(f64, f64); 3] = [(8.0, 0.0), (20.0, 0.0), (12.0, -1.0)];

/// The composition of \[Fe/H\] `fe_h` with no helium excess.
fn composition(fe_h: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The table's nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)`, mass fastest, computed on
/// `threads` threads a node per chunk, joined in node order.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never: the grid holds a few hundred nodes.
pub fn nodes(threads: NonZeroUsize) -> Result<Vec<(f64, f64, f64, f64)>, BuildThreadPoolError> {
    let masses = stripping_masses();
    let metallicities = stripping_metallicities();
    let cells = u64::try_from(masses.len() * metallicities.len()).expect("a few hundred nodes");
    let mut rows = Vec::with_capacity(masses.len() * metallicities.len());
    map_reduce_chunks(
        cells,
        1,
        threads,
        |items| {
            items
                .map(|k| {
                    let k = usize::try_from(k).expect("a few hundred nodes");
                    let (i, j) = (k % STRIPPING_MASSES, k / STRIPPING_MASSES);
                    StrippingNode::of(SolarMasses::new(masses[i]), &composition(metallicities[j]))
                        .to_row()
                })
                .collect::<Vec<_>>()
        },
        |part| rows.extend(part),
    )?;
    Ok(rows)
}

/// The validation of a table at the centres of its cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingCheck {
    /// The cells.
    pub cells: usize,
    /// The largest absolute error of the share.
    pub worst_share: f64,
    /// The median absolute error of the share.
    pub median_share: f64,
    /// The largest relative error of the core-helium-burning radius over the cells whose four
    /// nodes agree to 25%.
    pub worst_smooth_radius: f64,
    /// The cells whose four core-helium-burning radii spread by more than 25%.
    pub rough_cells: usize,
}

/// `table` against the exact nodes at the centres of every `every`-th of its cells (every cell for
/// `every` of 0 or 1), on `threads` threads.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `table` does not hold every node.
pub fn check(
    table: &StrippingTable<'_>,
    every: u64,
    threads: NonZeroUsize,
) -> Result<StrippingCheck, BuildThreadPoolError> {
    let masses = stripping_masses();
    let metallicities = stripping_metallicities();
    let (nm, nz) = (masses.len() - 1, metallicities.len() - 1);
    let cells = u64::try_from(nm * nz)
        .expect("a few hundred cells")
        .div_ceil(every.max(1));
    let mut errors: Vec<(f64, Option<f64>)> = Vec::with_capacity(nm * nz);
    map_reduce_chunks(
        cells,
        1,
        threads,
        |items| {
            items
                .map(|k| {
                    let k = usize::try_from(k * every.max(1)).expect("a few hundred cells");
                    let (i, j) = (k % nm, k / nm);
                    let m = math::exp(f64::midpoint(math::ln(masses[i]), math::ln(masses[i + 1])));
                    let comp = composition(f64::midpoint(metallicities[j], metallicities[j + 1]));
                    let m = SolarMasses::new(m);
                    let node = table.at(m, &comp).expect("the table holds every node");
                    let exact = StrippingNode::of(m, &comp);
                    let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)]
                        .map(|(a, b)| table.nodes()[b * STRIPPING_MASSES + a].3);
                    let spread = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                        - corners.iter().copied().fold(f64::INFINITY, f64::min);
                    let radius = (spread < math::log10(1.25)).then(|| {
                        (node.radii().core_helium_burning_rsun()
                            / exact.radii().core_helium_burning_rsun()
                            - 1.0)
                            .abs()
                    });
                    ((node.share() - exact.share()).abs(), radius)
                })
                .collect::<Vec<_>>()
        },
        |part| errors.extend(part),
    )?;
    let mut shares: Vec<f64> = errors.iter().map(|e| e.0).collect();
    shares.sort_by(f64::total_cmp);
    Ok(StrippingCheck {
        cells: errors.len(),
        worst_share: shares.last().copied().unwrap_or(0.0),
        median_share: shares.get(shares.len() / 2).copied().unwrap_or(0.0),
        worst_smooth_radius: errors.iter().filter_map(|e| e.1).fold(0.0, f64::max),
        rough_cells: errors.iter().filter(|e| e.1.is_none()).count(),
    })
}

/// The table's contents for `nodes`.
#[must_use]
pub fn render(nodes: &[(f64, f64, f64, f64)]) -> RustTable {
    RustTable {
        summary: vec![
            "The stripped share and the stage radii of massive primaries over initial mass and"
                .to_owned(),
            "metallicity (plan 11, P11.T1.d), which `galaxy::displaced::binarity::StrippingTable`"
                .to_owned(),
            "interpolates.".to_owned(),
        ],
        notes: vec![
            format!(
                "{STRIPPING_MASSES} masses even in ln m over 5.5–150 M☉ by \
                 {STRIPPING_METALLICITIES} values of [Fe/H] of `z_fit`, even over Z = 10⁻⁴–0.03,"
            ),
            "each node a primary's full track at the median draws, unstripped, and plan 11's"
                .to_owned(),
            "quadrature of the drawn companions over its stripping band (ruling 123.2).".to_owned(),
        ],
        items: vec![TableItem::Tuples {
            name: "NODES".to_owned(),
            doc: vec![
                "The nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)` with the radii in R☉, mass"
                    .to_owned(),
                "fastest: `galaxy::displaced::binarity::StrippingNode::of` at each mass of".to_owned(),
                "`stripping_masses` and \\[Fe/H\\] of `stripping_metallicities`.".to_owned(),
            ],
            rows: nodes.iter().map(|&(s, a, b, c)| vec![s, a, b, c]).collect(),
        }],
    }
}

/// The task: the stripping table of P11.T1.d; slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StrippingTask;

impl FitTask for StrippingTask {
    fn name(&self) -> &'static str {
        "stripping"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "stripping.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["NODES"]
    }

    /// The exact share at three primaries and the 15 M☉ solar star's three radii.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes: Vec<(String, f64)> = FINGERPRINT_POINTS
            .iter()
            .map(|&(m, fe_h)| {
                (
                    format!("exact_stripped_share({m} M☉, [Fe/H] {fe_h})"),
                    exact_stripped_share(SolarMasses::new(m), &composition(fe_h)),
                )
            })
            .collect();
        let (_, a, b, c) = StrippingNode::of(SolarMasses::new(15.0), &Composition::SOLAR).to_row();
        probes.push(("log r_MS(15 M☉)".to_owned(), a));
        probes.push(("log r_HG(15 M☉)".to_owned(), b));
        probes.push(("log r_CHeB(15 M☉)".to_owned(), c));
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let masses = manifest.u64("masses")?;
        let metallicities = manifest.u64("metallicities")?;
        if usize::try_from(masses).ok() != Some(STRIPPING_MASSES)
            || usize::try_from(metallicities).ok() != Some(STRIPPING_METALLICITIES)
        {
            return Err(
                ManifestParamError::new("masses and metallicities", "the sim's 34 and 11").into(),
            );
        }
        let every = manifest.u64("validate_every")?;
        if every == 0 {
            return Err(ManifestParamError::new("validate_every", "at least 1").into());
        }
        let nodes = nodes(threads)?;
        let table = StrippingTable::new(&nodes).expect("the task computes every node");
        let figures = check(&table, every, threads)?;
        Ok(TaskOutput {
            table: render(&nodes),
            source: "plan 06's tracks for the radii; plan 11's multiplicity model (Moe and Di \
                     Stefano 2017, ApJS 230, 15, for the direct companions) for the share; the \
                     stripping band of ruling 123 (Podsiadlowski et al. 2004; Eggleton 1983; \
                     Hurley, Tout and Pols 2002 for the onset ratios)"
                .to_owned(),
            acceptance: format!(
                "at the centres of {} cells, the share within {:.4} of the exact (median \
                 {:.1e}), the core-helium-burning radius within {:.3} relative where the cell's \
                 four radii agree to 25% ({} cells spread more)",
                figures.cells,
                figures.worst_share,
                figures.median_share,
                figures.worst_smooth_radius,
                figures.rough_cells,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rendered_table_declares_its_nodes() {
        let rows = vec![(0.3, 0.5, 1.0, 2.0); STRIPPING_MASSES * STRIPPING_METALLICITIES];
        let table = render(&rows);
        assert_eq!(table.item_names(), ["NODES"]);
        let body = table.body().unwrap();
        assert!(body.contains("#[rustfmt::skip]"));
        assert!(body.contains(&format!(
            "pub const NODES: [(f64, f64, f64, f64); {}] = [",
            rows.len()
        )));
        assert_eq!(body.matches("),\n").count(), rows.len());
    }
}
