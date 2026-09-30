//! The cluster retention table (plan 09, P09.T9.b; ruling 139.5): the neutron stars' and black
//! holes' kick shares by mode at the retention quadrature's 16 mass nodes and 11 \[Fe/H\] of
//! `z_fit`, the ordinary mode's kicked below 32 speeds, which
//! `hyperion_sim::galaxy::features::interior::retention::retention_tabulated` interpolates.
//!
//! Each node is the sim's own [`node_shares`]: one full track at the median draws and plan 08's
//! kick quadrature with its first bin edge at each speed, so a node's shares are exactly what the
//! reference [`retention`] reads there. The run's acceptance compares the tabulated retention with
//! the reference at off-node \[Fe/H\] and speeds for both mass functions.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::features::interior::retention::{
    FE_H_NODES, MASS_NODES, SPEED_NODES, fe_h_nodes, mass_nodes, node_shares, retention,
    retention_from_rows, speed_nodes, table_row,
};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::remnant::StandardKickLaw;
use hyperion_sim::units::{Dex, HeliumExcess, KilometresPerSecond};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The rows of the table: two kinds at every mass and \[Fe/H\] node.
pub const ROWS: usize = 2 * MASS_NODES * FE_H_NODES;

/// The acceptance's \[Fe/H\] of `z_fit`, between the nodes.
pub const ACCEPTANCE_FE_H: [f64; 4] = [-1.95, -1.1, -0.43, 0.05];

/// The acceptance's effective escape speeds, km/s, between the nodes.
pub const ACCEPTANCE_SPEEDS: [f64; 4] = [3.3, 17.0, 45.0, 120.0];

/// The fitted table: the rows in [`table_row`]'s order.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterRetentionFit {
    /// `(kind share, complete fallback, low mode, envelope loss)` per row.
    pub shares: Vec<[f64; 4]>,
    /// The ordinary mode's share kicked below each speed node, per row.
    pub ordinary_below: Vec<[f64; SPEED_NODES]>,
}

/// The table on `threads` threads, one node a chunk, joined in node order: the same bytes for
/// any thread count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never: 176 node indices fit in `u64` and `usize`.
pub fn fit(threads: NonZeroUsize) -> Result<ClusterRetentionFit, BuildThreadPoolError> {
    let (masses, metallicities) = (mass_nodes(), fe_h_nodes());
    let nodes = u64::try_from(MASS_NODES * FE_H_NODES).expect("176 nodes");
    let mut out = ClusterRetentionFit {
        shares: vec![[0.0; 4]; ROWS],
        ordinary_below: vec![[0.0; SPEED_NODES]; ROWS],
    };
    let mut done = Vec::with_capacity(MASS_NODES * FE_H_NODES);
    map_reduce_chunks(
        nodes,
        1,
        threads,
        |range| {
            range
                .map(|n| {
                    let n = usize::try_from(n).expect("a node index below 176");
                    let (f, k) = (n / MASS_NODES, n % MASS_NODES);
                    (k, f, node_shares(masses[k], metallicities[f]))
                })
                .collect::<Vec<_>>()
        },
        |chunk| done.extend(chunk),
    )?;
    for (k, f, kinds) in done {
        for (kind, s) in kinds.iter().enumerate() {
            let row = table_row(k, f, kind);
            out.shares[row] = [s.kind, s.fallback, s.low, s.envelope];
            let mut below = [0.0; SPEED_NODES];
            below.copy_from_slice(&s.ordinary_below);
            out.ordinary_below[row] = below;
        }
    }
    Ok(out)
}

/// The largest absolute and relative (where the reference exceeds 0.05) differences between the
/// tabulated and the reference retention of neutron stars and black holes at the acceptance points,
/// for both mass functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterRetentionAcceptance {
    /// The points compared.
    pub points: usize,
    /// The largest |tabulated − reference| over every point.
    pub largest_absolute: f64,
    /// The largest |tabulated − reference| where the reference is 0.05 or less.
    pub largest_absolute_small: f64,
    /// The largest |tabulated − reference| ÷ reference where the reference exceeds 0.05.
    pub largest_relative: f64,
}

/// The acceptance figures of `fit` (module documentation).
#[must_use]
pub fn acceptance(fit: &ClusterRetentionFit) -> ClusterRetentionAcceptance {
    let law = StandardKickLaw::default();
    let (mut points, mut largest_absolute, mut largest_relative) = (0, 0.0_f64, 0.0_f64);
    let mut largest_absolute_small = 0.0_f64;
    for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
        let imf = kind.to_mass_function();
        for fe_h in ACCEPTANCE_FE_H {
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for v in ACCEPTANCE_SPEEDS {
                let v = KilometresPerSecond::new(v);
                let exact = retention(&law, imf.as_ref(), &composition, v);
                let table = retention_from_rows(
                    &fit.shares,
                    &fit.ordinary_below,
                    imf.as_ref(),
                    &composition,
                    v,
                );
                for (a, b) in [
                    (table.neutron_stars, exact.neutron_stars),
                    (table.black_holes, exact.black_holes),
                ] {
                    points += 1;
                    largest_absolute = largest_absolute.max((a - b).abs());
                    if b > 0.05 {
                        largest_relative = largest_relative.max((a - b).abs() / b);
                    } else {
                        largest_absolute_small = largest_absolute_small.max((a - b).abs());
                    }
                }
            }
        }
    }
    ClusterRetentionAcceptance {
        points,
        largest_absolute,
        largest_absolute_small,
        largest_relative,
    }
}

/// The two arrays as `static` items: 352 rows of 32 are too large for a `const`.
fn body(fit: &ClusterRetentionFit) -> String {
    let mut out = String::new();
    out.push_str(
        "/// Per row, the probabilities that the remnant is of the row's kind, that it is a complete\n\
         /// fallback, a low-mode kick and an envelope loss.\n#[rustfmt::skip]\n",
    );
    let _ = writeln!(out, "pub static SHARES: [[f64; 4]; {ROWS}] = [");
    for row in &fit.shares {
        let cells: Vec<String> = row.iter().map(|&v| literal(v)).collect();
        let _ = writeln!(out, "    [{}],", cells.join(", "));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// Per row, the probability that the remnant is of the row's kind and kicked in the\n\
         /// ordinary mode below each speed node.\n#[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static ORDINARY_BELOW: [[f64; {SPEED_NODES}]; {ROWS}] = ["
    );
    for row in &fit.ordinary_below {
        let cells: Vec<String> = row.iter().map(|&v| literal(v)).collect();
        let _ = writeln!(out, "    [{}],", cells.join(", "));
    }
    out.push_str("];\n");
    out
}

/// The table's contents.
#[must_use]
pub fn render(fit: &ClusterRetentionFit) -> RustTable {
    let notes = format!(
        "\
{MASS_NODES} masses even in ln m over 8–100 M☉ (the retention quadrature's own nodes) by
{FE_H_NODES} values of [Fe/H] of `z_fit`, even over Z = 10⁻⁴–0.03, two rows a node, neutron
stars then black holes, mass fastest (`retention::table_row`). Each node is
`retention::node_shares`: one full track at the median draws and plan 08's kick quadrature with
its first speed bin's edge at each of {SPEED_NODES} speeds even in log v over 0.1–500 km/s."
    );
    RustTable {
        summary: vec![
            "The cluster retention table (plan 09, P09.T9.b; ruling 139.5): the neutron stars' and"
                .to_owned(),
            "black holes' kick shares by mode over progenitor mass and metallicity, which"
                .to_owned(),
            "`galaxy::features::interior::retention::retention_tabulated` interpolates.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task (ruling 139.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ClusterRetentionTask;

impl FitTask for ClusterRetentionTask {
    fn name(&self) -> &'static str {
        "cluster_retention"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "cluster_retention.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["SHARES", "ORDINARY_BELOW"]
    }

    /// The node axes' ends, and the shares at three grid nodes (a light, a middling and a heavy
    /// progenitor, at three metallicities) with their ordinary mode inside its rise.
    fn fingerprint(&self) -> SimFingerprint {
        let (masses, metallicities, speeds) = (mass_nodes(), fe_h_nodes(), speed_nodes());
        let mut probes = vec![
            ("mass_nodes[0]".to_owned(), masses[0]),
            ("mass_nodes[15]".to_owned(), masses[MASS_NODES - 1]),
            ("fe_h_nodes[0]".to_owned(), metallicities[0]),
            ("fe_h_nodes[10]".to_owned(), metallicities[FE_H_NODES - 1]),
            ("speed_nodes[0]".to_owned(), speeds[0]),
            ("speed_nodes[31]".to_owned(), speeds[SPEED_NODES - 1]),
        ];
        for (k, f) in [(1, 10), (5, 7), (10, 2)] {
            for (kind, s) in ["ns", "bh"]
                .iter()
                .zip(node_shares(masses[k], metallicities[f]))
            {
                let at = format!("mass node {k}, [Fe/H] node {f}");
                probes.push((format!("{kind}.share({at})"), s.kind));
                probes.push((format!("{kind}.fallback({at})"), s.fallback));
                probes.push((format!("{kind}.low({at})"), s.low));
                probes.push((format!("{kind}.envelope({at})"), s.envelope));
                for i in [20, 24, 28, 31] {
                    probes.push((
                        format!("{kind}.ordinary_below[{i}]({at})"),
                        s.ordinary_below[i],
                    ));
                }
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, _manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let fit = fit(threads)?;
        let figures = acceptance(&fit);
        Ok(TaskOutput {
            table: render(&fit),
            source: "plan 06's tracks and plan 08's kick quadrature (`kick_bins`) under the \
                     default kick law; the retention quadrature of P09.T9.b"
                .to_owned(),
            acceptance: format!(
                "at {} off-node points ([Fe/H] {:?}, v {:?} km/s, Chabrier and Kroupa), the \
                 tabulated neutron-star and black-hole retention within {:.5} absolute of the \
                 exact quadrature where it is 0.05 or less and {:.4} relative where it exceeds \
                 0.05 (ruling 139.5: 0.005 and 3%; {}); largest absolute {:.5}",
                figures.points,
                ACCEPTANCE_FE_H,
                ACCEPTANCE_SPEEDS,
                figures.largest_absolute_small,
                figures.largest_relative,
                if figures.largest_absolute_small <= 0.005 && figures.largest_relative <= 0.03 {
                    "met"
                } else {
                    "NOT met"
                },
                figures.largest_absolute,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node's rows are the sim's own shares, in `table_row`'s place, and the rendered body
    /// declares both arrays.
    #[test]
    fn rows_sit_where_the_sim_reads_them() {
        let (masses, metallicities) = (mass_nodes(), fe_h_nodes());
        let shares = node_shares(masses[5], metallicities[7]);
        let mut fit = ClusterRetentionFit {
            shares: vec![[0.0; 4]; ROWS],
            ordinary_below: vec![[0.0; SPEED_NODES]; ROWS],
        };
        for (kind, s) in shares.iter().enumerate() {
            let row = table_row(5, 7, kind);
            fit.shares[row] = [s.kind, s.fallback, s.low, s.envelope];
            fit.ordinary_below[row].copy_from_slice(&s.ordinary_below);
        }
        let TableItem::Source(body) = &render(&fit).items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains("pub static SHARES: [[f64; 4]; 352] = ["));
        assert!(body.contains("pub static ORDINARY_BELOW: [[f64; 32]; 352] = ["));
        assert!(shares[0].ordinary_below.windows(2).all(|w| w[0] <= w[1]));
        assert!(shares[0].kind + shares[1].kind <= 1.0 + 1e-12);
    }
}
