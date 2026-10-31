//! The stripping table's helium axis (plan 11, P11.T1.d, read from version 26): the stripped share
//! and the stage radii of massive primaries with a helium excess, which
//! `hyperion_sim::galaxy::displaced::binarity::HeliumStrippingTable` interpolates over the
//! stripping table at none.
//!
//! Each node is [`StrippingNode::of_fitted_helium`]: the `stripping` task's node, its track built
//! under the fitted helium table (`tables::helium_26`), at the helium excesses
//! [`stripping_heliums`] over the stripping table's grid ([`stripping_masses`],
//! [`stripping_metallicities`]). The run validates the interpolant at the centre of every cell of
//! the axis, those between no helium excess and the first slice included, against the exact node
//! there. Every node costs a track and the quadrature, some four milliseconds, so the task is slow.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::displaced::binarity::{
    HeliumStrippingTable, STRIPPING_HELIUM_MAX, STRIPPING_HELIUMS, STRIPPING_MASSES,
    STRIPPING_METALLICITIES, STRIPPING_NODES, StrippingNode, StrippingTable, stripping_heliums,
    stripping_masses, stripping_metallicities,
};
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The masses, M☉, \[Fe/H\] and helium excesses of the fingerprint's probes.
pub const FINGERPRINT_POINTS: [(f64, f64, f64); 3] =
    [(8.0, 0.0, 0.12), (20.0, -1.0, 0.18), (40.0, -1.0, 0.06)];

/// The composition of \[Fe/H\] `fe_h` with the helium excess `helium`.
fn composition(fe_h: f64, helium: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(helium))
}

/// The axis's nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)`, mass fastest, then
/// metallicity, then helium excess, computed on `threads` threads a node per chunk, joined in node
/// order.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never: the axis holds about a thousand nodes.
pub fn nodes(threads: NonZeroUsize) -> Result<Vec<(f64, f64, f64, f64)>, BuildThreadPoolError> {
    let masses = stripping_masses();
    let metallicities = stripping_metallicities();
    let heliums = stripping_heliums();
    let count = STRIPPING_NODES * STRIPPING_HELIUMS;
    let cells = u64::try_from(count).expect("about a thousand nodes");
    let mut rows = Vec::with_capacity(count);
    map_reduce_chunks(
        cells,
        1,
        threads,
        |items| {
            items
                .map(|k| {
                    let k = usize::try_from(k).expect("about a thousand nodes");
                    let (i, j, h) = (
                        k % STRIPPING_MASSES,
                        k / STRIPPING_MASSES % STRIPPING_METALLICITIES,
                        k / STRIPPING_NODES,
                    );
                    StrippingNode::of_fitted_helium(
                        SolarMasses::new(masses[i]),
                        &composition(metallicities[j], heliums[h]),
                    )
                    .to_row()
                })
                .collect::<Vec<_>>()
        },
        |part| rows.extend(part),
    )?;
    Ok(rows)
}

/// The validation of an axis at the centres of its cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingHeliumCheck {
    /// The cells.
    pub cells: usize,
    /// The largest absolute error of the share.
    pub worst_share: f64,
    /// The median absolute error of the share.
    pub median_share: f64,
    /// The largest relative error of the core-helium-burning radius over the cells whose eight
    /// nodes agree to 25%.
    pub worst_smooth_radius: f64,
    /// The cells whose eight core-helium-burning radii spread by more than 25%.
    pub rough_cells: usize,
}

/// `table` against the exact nodes at the centres of every `every`-th of its cells (every cell for
/// `every` of 0 or 1), on `threads` threads: in ln m, \[Fe/H\] and the helium excess, from none to
/// the first slice and between the slices.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `table` does not hold every node.
pub fn check(
    table: &HeliumStrippingTable<'_>,
    every: u64,
    threads: NonZeroUsize,
) -> Result<StrippingHeliumCheck, BuildThreadPoolError> {
    let masses = stripping_masses();
    let metallicities = stripping_metallicities();
    let mut heliums = vec![0.0];
    heliums.extend(stripping_heliums());
    let (nm, nz, nh) = (masses.len() - 1, metallicities.len() - 1, heliums.len() - 1);
    let cells = u64::try_from(nm * nz * nh)
        .expect("about a thousand cells")
        .div_ceil(every.max(1));
    // The core-helium-burning radius of node (mass, metallicity, slice), slice 0 the stripping
    // table's.
    let radius = |i: usize, j: usize, h: usize| {
        let k = j * STRIPPING_MASSES + i;
        if h == 0 {
            StrippingTable::generator().nodes()[k].3
        } else {
            table.slices()[(h - 1) * STRIPPING_NODES + k].3
        }
    };
    let mut errors: Vec<(f64, Option<f64>)> = Vec::with_capacity(nm * nz * nh);
    map_reduce_chunks(
        cells,
        1,
        threads,
        |items| {
            items
                .map(|k| {
                    let k = usize::try_from(k * every.max(1)).expect("about a thousand cells");
                    let (i, j, h) = (k % nm, k / nm % nz, k / (nm * nz));
                    let mass =
                        math::exp(f64::midpoint(math::ln(masses[i]), math::ln(masses[i + 1])));
                    let comp = composition(
                        f64::midpoint(metallicities[j], metallicities[j + 1]),
                        f64::midpoint(heliums[h], heliums[h + 1]),
                    );
                    let mass = SolarMasses::new(mass);
                    let node = table.at(mass, &comp).expect("the axis holds every node");
                    let exact = StrippingNode::of_fitted_helium(mass, &comp);
                    let corners: Vec<f64> = [i, i + 1]
                        .into_iter()
                        .flat_map(|a| [j, j + 1].map(|b| (a, b)))
                        .flat_map(|(a, b)| [h, h + 1].map(|c| radius(a, b, c)))
                        .collect();
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
    Ok(StrippingHeliumCheck {
        cells: errors.len(),
        worst_share: shares.last().copied().unwrap_or(0.0),
        median_share: shares.get(shares.len() / 2).copied().unwrap_or(0.0),
        worst_smooth_radius: errors.iter().filter_map(|e| e.1).fold(0.0, f64::max),
        rough_cells: errors.iter().filter(|e| e.1.is_none()).count(),
    })
}

/// The table's contents for `nodes`: a `pub static` array of tuples, one a line, since Clippy
/// refuses so large a `const` array (`large_const_arrays`).
#[must_use]
pub fn render(nodes: &[(f64, f64, f64, f64)]) -> RustTable {
    let mut out = String::new();
    for line in [
        "The nodes, `(share, log₁₀ r_MS, log₁₀ r_HG, log₁₀ r_CHeB)` with the radii in R☉, mass",
        "fastest, then metallicity, then helium excess:",
        "`galaxy::displaced::binarity::StrippingNode::of_fitted_helium` at each mass of",
        "`stripping_masses`, \\[Fe/H\\] of `stripping_metallicities` and helium excess of",
        "`stripping_heliums`.",
    ] {
        let _ = writeln!(out, "/// {line}");
    }
    let _ = writeln!(out, "#[rustfmt::skip]");
    let _ = writeln!(
        out,
        "pub static NODES: [(f64, f64, f64, f64); {}] = [",
        nodes.len()
    );
    for &(s, a, b, c) in nodes {
        let values = [s, a, b, c].map(literal);
        let _ = writeln!(out, "    ({}),", values.join(", "));
    }
    out.push_str("];\n");
    RustTable {
        summary: vec![
            "The stripped share and the stage radii of massive primaries with a helium excess"
                .to_owned(),
            "(plan 11, P11.T1.d's helium axis, read from version 26), which".to_owned(),
            "`galaxy::displaced::binarity::HeliumStrippingTable` interpolates over the stripping"
                .to_owned(),
            "table at none.".to_owned(),
        ],
        notes: vec![
            format!(
                "{STRIPPING_HELIUMS} slices of helium excess, evenly to {STRIPPING_HELIUM_MAX} \
                 above none, each the stripping table's"
            ),
            format!(
                "{STRIPPING_MASSES} masses by {STRIPPING_METALLICITIES} values of [Fe/H], each \
                 node a primary's full track under the"
            ),
            "fitted helium table at the median draws, unstripped, and plan 11's quadrature of the"
                .to_owned(),
            "drawn companions over its stripping band (ruling 123.2).".to_owned(),
        ],
        items: vec![TableItem::Source(out)],
    }
}

/// The task: the stripping table's helium axis of P11.T1.d; slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StrippingHeliumTask;

impl FitTask for StrippingHeliumTask {
    fn name(&self) -> &'static str {
        "stripping_helium"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "stripping_helium.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["NODES"]
    }

    /// The exact node's share and core-helium-burning radius at three helium-rich primaries.
    fn fingerprint(&self) -> SimFingerprint {
        let probes = FINGERPRINT_POINTS
            .iter()
            .flat_map(|&(m, fe_h, helium)| {
                let (share, _, _, r_cheb) = StrippingNode::of_fitted_helium(
                    SolarMasses::new(m),
                    &composition(fe_h, helium),
                )
                .to_row();
                [
                    (format!("share({m} M☉, [Fe/H] {fe_h}, ΔY {helium})"), share),
                    (
                        format!("log r_CHeB({m} M☉, [Fe/H] {fe_h}, ΔY {helium})"),
                        r_cheb,
                    ),
                ]
            })
            .collect();
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let masses = manifest.u64("masses")?;
        let metallicities = manifest.u64("metallicities")?;
        let heliums = manifest.u64("heliums")?;
        if usize::try_from(masses).ok() != Some(STRIPPING_MASSES)
            || usize::try_from(metallicities).ok() != Some(STRIPPING_METALLICITIES)
            || usize::try_from(heliums).ok() != Some(STRIPPING_HELIUMS)
        {
            return Err(ManifestParamError::new(
                "masses, metallicities and heliums",
                "the sim's 34, 11 and 3",
            )
            .into());
        }
        let every = manifest.u64("validate_every")?;
        if every == 0 {
            return Err(ManifestParamError::new("validate_every", "at least 1").into());
        }
        let nodes = nodes(threads)?;
        let table = HeliumStrippingTable::new(StrippingTable::generator(), &nodes)
            .expect("the task computes every node");
        let figures = check(&table, every, threads)?;
        Ok(TaskOutput {
            table: render(&nodes),
            source:
                "the `stripping` task's: plan 06's tracks for the radii, here under the fitted \
                     helium table (`tables::helium_26`, after Pietrinferni, Cassisi, Salaris and \
                     Castelli 2006, ApJ 642, 797); plan 11's multiplicity model (Moe and Di \
                     Stefano 2017, ApJS 230, 15, for the direct companions) for the share; the \
                     stripping band of ruling 123 (Podsiadlowski et al. 2004; Eggleton 1983; \
                     Hurley, Tout and Pols 2002 for the onset ratios)"
                    .to_owned(),
            acceptance: format!(
                "at the centres of {} cells in ln m, [Fe/H] and the helium excess, the share \
                 within {:.4} of the exact (median {:.1e}), the core-helium-burning radius within \
                 {:.3} relative where the cell's eight radii agree to 25% ({} cells spread more)",
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
        let rows = vec![(0.3, 0.5, 1.0, 2.0); STRIPPING_NODES * STRIPPING_HELIUMS];
        let body = render(&rows).body().unwrap();
        assert!(body.contains("#[rustfmt::skip]"));
        assert!(body.contains(&format!(
            "pub static NODES: [(f64, f64, f64, f64); {}] = [",
            rows.len()
        )));
        assert_eq!(body.matches("),\n").count(), rows.len());
    }
}
