//! The lifetime bracket of layers D and E (plan 08, P08.T9.d, baked by its follow-up under main's
//! ruling on P08.T13's cost, 2026-10-15), which
//! `galaxy::displaced::marks::LifetimeBracket::shared` reads.
//!
//! Per mass node of bands D and E, 33 log-spaced masses each (2.5–8 and 8–150 M☉), the least and
//! greatest of plan 06's single-star lifetime (`stellar::lifetime`, the age at death) over 13
//! log-spaced metallicities on Z = 10⁻⁴–0.03, at the median star's draws: 858 lifetimes, kept as
//! 132 numbers, unwidened, in years. The bracket's tenth of widening is applied where it is read
//! (`LifetimeBracket::bracket`), so it is no input of the table.
//!
//! Nothing is fitted, and neither `lifetime` nor the bracket's loop is copied here: the table's
//! bounds are the sim's own `LifetimeBracket::new`'s, read through
//! `LifetimeBracket::bounds_years`, and its node masses `marks::MassNodes`', whose masses are the
//! band's alone (`band_masses`; the mass function sets only their weights). Only the fingerprint
//! composes metallicities of its own, the range's two ends, as `new` composes them. So the
//! committed table is `new()`'s bits, which the sim's `the_shared_bracket_is_new_bit_for_bit`
//! holds against a fresh build in every process, natively and under wasip1. The task exists to
//! spare each process those 858 lifetimes (0.43–0.70 s in a release build, plan 08's record "The
//! lifetime bracket as a fitted table"), and to make the bracket a locked generator-version input:
//! its fingerprint probes a few of the lifetimes it is made of, and the bracket's own constants.
//! A run in which a band's least or greatest lifetime rises with mass, which
//! `LifetimeBracket::bracket`'s reading between nodes rules out, writes the table as provisional.

use std::num::NonZeroUsize;

use hyperion_sim::galaxy::displaced::marks::{
    BRACKET_Z_RANGE, BRACKET_Z_SAMPLES, LifetimeBracket, MARK_MASS_NODES, MassNodes,
};
use hyperion_sim::galaxy::imf::{Kroupa, MASS_BAND_EDGES, MassBand};
use hyperion_sim::math;
use hyperion_sim::stellar::composition::Z_SOLAR;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::sse::MAX_INITIAL_MASS;
use hyperion_sim::stellar::{Composition, lifetime};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The table's items: each band's node masses, M☉, then its (least, greatest) lifetimes, years.
pub const ITEMS: [&str; 4] = [
    "BAND_D_MASSES_MSUN",
    "BAND_D_LIFETIMES_YEARS",
    "BAND_E_MASSES_MSUN",
    "BAND_E_LIFETIMES_YEARS",
];

/// The bracket's bands, in `LifetimeBracket::bounds_years`' order.
const BANDS: [MassBand; 2] = [MassBand::D, MassBand::E];

/// The masses, M☉, whose lifetimes the fingerprint probes at both ends of
/// [`BRACKET_Z_RANGE`]: band D's lightest node, the edge between D and E, a band-E star and the
/// tracks' heaviest.
const PROBE_MASSES: [f64; 4] = [2.5, 8.0, 20.0, 150.0];

/// The bracket as the table holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct LifetimeBracketFit {
    /// `[band][node]`, bands D and E: the node masses, M☉.
    pub masses: [[f64; MARK_MASS_NODES]; 2],
    /// `[band][node]`: the least and greatest lifetime over the metallicities, years, unwidened.
    pub bounds: [[(f64, f64); MARK_MASS_NODES]; 2],
}

/// The bracket: `LifetimeBracket::new`'s bounds and the bands' node masses.
#[must_use]
pub fn fit() -> LifetimeBracketFit {
    let bracket = LifetimeBracket::new();
    LifetimeBracketFit {
        masses: BANDS.map(|band| *MassNodes::of(band, &Kroupa).masses()),
        bounds: *bracket.bounds_years(),
    }
}

/// The table's contents for `fit`.
#[must_use]
pub fn render(fit: &LifetimeBracketFit) -> RustTable {
    let (z_lo, z_hi) = BRACKET_Z_RANGE;
    let mut items = Vec::with_capacity(ITEMS.len());
    for (b, band) in BANDS.into_iter().enumerate() {
        let (lo, hi) = (
            MASS_BAND_EDGES[band.index()],
            MASS_BAND_EDGES[band.index() + 1],
        );
        let (masses, lifetimes) = (ITEMS[2 * b], ITEMS[2 * b + 1]);
        items.push(TableItem::Array {
            name: masses.to_owned(),
            doc: vec![format!(
                "Band {band:?}'s {MARK_MASS_NODES} mass nodes, M☉, log-spaced from {lo} to {hi} M☉ \
                 (`marks::MassNodes`)."
            )],
            values: fit.masses[b].to_vec(),
        });
        items.push(TableItem::Tuples {
            name: lifetimes.to_owned(),
            doc: vec![
                format!(
                    "Per node of `{masses}`, the least and greatest `stellar::lifetime`, years, over"
                ),
                format!(
                    "the {BRACKET_Z_SAMPLES} log-spaced Z of {z_lo}–{z_hi} at the median draws, \
                     unwidened."
                ),
            ],
            rows: fit.bounds[b]
                .iter()
                .map(|&(least, greatest)| vec![least, greatest])
                .collect(),
        });
    }
    let lifetimes = BANDS.len() * MARK_MASS_NODES * BRACKET_Z_SAMPLES;
    RustTable {
        summary: vec![
            "The lifetime bracket of layers D and E (plan 08, P08.T9.d), which".to_owned(),
            "`galaxy::displaced::marks::LifetimeBracket::shared` reads.".to_owned(),
        ],
        notes: vec![
            format!(
                "Each band's {MARK_MASS_NODES} nodes take the least and greatest lifetime of \
                 {BRACKET_Z_SAMPLES} metallicities,"
            ),
            format!(
                "log-spaced over Z = {z_lo}–{z_hi}, at `StarDraws::median()`: `LifetimeBracket::new`'s"
            ),
            format!(
                "{lifetimes} lifetimes, read through `LifetimeBracket::bounds_years`. Nothing is \
                 fitted. The tenth"
            ),
            "of widening is applied where the bracket is read, `LifetimeBracket::bracket`."
                .to_owned(),
        ],
        items,
    }
}

/// The metallicity `z` as `LifetimeBracket::new` composes it: \[Fe/H\] = log₁₀(Z ÷ Z☉) with no
/// helium excess.
fn composition(z: f64) -> Composition {
    Composition::from_fe_h(
        Dex::new(math::log10(z / Z_SOLAR.value())),
        HeliumExcess::ZERO,
    )
}

/// Whether both columns of every band fall with mass from node to node, which
/// `LifetimeBracket::bracket`'s reading between nodes assumes.
fn falls_with_mass(fit: &LifetimeBracketFit) -> bool {
    fit.bounds.iter().all(|band| {
        band.windows(2)
            .all(|w| w[1].0 <= w[0].0 && w[1].1 <= w[0].1)
    })
}

/// The task: P08.T9.d's bracket, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LifetimeBracketTask;

impl FitTask for LifetimeBracketTask {
    fn name(&self) -> &'static str {
        "lifetime_bracket"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "lifetime_bracket.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The lifetimes at [`PROBE_MASSES`] at both ends of [`BRACKET_Z_RANGE`], median draws, so that
    /// a change to plan 06's tracks is caught; and the bracket's constants: its metallicities, its
    /// nodes, the bands' edges, the tracks' heaviest mass and Z☉.
    fn fingerprint(&self) -> SimFingerprint {
        let draws = StarDraws::median();
        let (z_lo, z_hi) = BRACKET_Z_RANGE;
        let mut probes = Vec::with_capacity(2 * PROBE_MASSES.len() + 9);
        for z in [z_lo, z_hi] {
            let comp = composition(z);
            for m in PROBE_MASSES {
                probes.push((
                    format!("lifetime({m} M☉, Z {z})"),
                    lifetime(SolarMasses::new(m), &comp, &draws).value(),
                ));
            }
        }
        probes.extend([
            ("BRACKET_Z_RANGE.0".to_owned(), z_lo),
            ("BRACKET_Z_RANGE.1".to_owned(), z_hi),
            ("BRACKET_Z_SAMPLES".to_owned(), count(BRACKET_Z_SAMPLES)),
            ("MARK_MASS_NODES".to_owned(), count(MARK_MASS_NODES)),
            ("MASS_BAND_EDGES[3]".to_owned(), MASS_BAND_EDGES[3]),
            ("MASS_BAND_EDGES[4]".to_owned(), MASS_BAND_EDGES[4]),
            ("MASS_BAND_EDGES[5]".to_owned(), MASS_BAND_EDGES[5]),
            ("MAX_INITIAL_MASS".to_owned(), MAX_INITIAL_MASS.value()),
            ("Z_SOLAR".to_owned(), Z_SOLAR.value()),
        ]);
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let fit = fit();
        let span = |b: usize| {
            fit.bounds[b].iter().fold(
                (f64::INFINITY, 0.0_f64, 1.0_f64),
                |(lo, hi, widest), &(least, greatest)| {
                    (
                        lo.min(least),
                        hi.max(greatest),
                        widest.max(greatest / least),
                    )
                },
            )
        };
        let ((d_lo, d_hi, d_ratio), (e_lo, e_hi, e_ratio)) = (span(0), span(1));
        let falls = falls_with_mass(&fit);
        let acceptance = format!(
            "the table is `LifetimeBracket::new()` bit for bit (read from it, and held to a fresh \
             build by the sim's `the_shared_bracket_is_new_bit_for_bit`); band D's lifetimes \
             {d_lo:.4e}–{d_hi:.4e} years, its widest node's greatest {d_ratio:.3} times its least; \
             band E's {e_lo:.4e}–{e_hi:.4e} years and {e_ratio:.3}; least and greatest fall with \
             mass at every node ({})",
            if falls { "passes" } else { "FAILS" },
        );
        Ok(TaskOutput {
            table: render(&fit),
            source: "plan 06's single-star lifetimes (`stellar::lifetime`: Hurley, Pols and Tout \
                     2000, MNRAS 315, 543, with ruling 92's giant radii; above 100 M☉ P06.T14's \
                     factors fitted to Yusof et al. 2013, MNRAS 433, 1114) at the median draws, \
                     over plan 08's P08.T9.d bracket's metallicities and mass nodes \
                     (`LifetimeBracket::new`)"
                .to_owned(),
            acceptance,
            provisional: (!falls).then_some("P08.T9.d"),
        })
    }
}

/// A small count as a probe value.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a count of the bracket fits in u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered items are [`ITEMS`] in order, each band's 33 masses and 33 pairs.
    #[test]
    fn the_table_has_each_bands_masses_and_bounds() {
        let fit = LifetimeBracketFit {
            masses: [[1.0; MARK_MASS_NODES]; 2],
            bounds: [[(1.0, 2.0); MARK_MASS_NODES]; 2],
        };
        let table = render(&fit);
        let names: Vec<&str> = table.items.iter().filter_map(TableItem::name).collect();
        assert_eq!(names, ITEMS);
        for item in &table.items {
            match item {
                TableItem::Array { values, .. } => assert_eq!(values.len(), MARK_MASS_NODES),
                TableItem::Tuples { rows, .. } => {
                    assert_eq!(rows.len(), MARK_MASS_NODES);
                    assert!(rows.iter().all(|r| r.len() == 2));
                }
                other => panic!("an unexpected item {other:?}"),
            }
        }
    }

    /// The node masses the task reads are the bands' edges at their ends, log-spaced between.
    #[test]
    fn the_node_masses_span_bands_d_and_e() {
        for (b, band) in BANDS.into_iter().enumerate() {
            let masses = *MassNodes::of(band, &Kroupa).masses();
            assert!(masses[0].total_cmp(&MASS_BAND_EDGES[3 + b]).is_eq());
            assert!(
                masses[MARK_MASS_NODES - 1]
                    .total_cmp(&MASS_BAND_EDGES[4 + b])
                    .is_eq()
            );
            assert!(masses.windows(2).all(|w| w[0] < w[1]));
        }
    }

    /// The fall-with-mass check refuses a node whose least or greatest lifetime rises.
    #[test]
    fn a_lifetime_that_rises_with_mass_fails_the_check() {
        let mut fit = LifetimeBracketFit {
            masses: [[1.0; MARK_MASS_NODES]; 2],
            bounds: [[(1.0, 2.0); MARK_MASS_NODES]; 2],
        };
        assert!(falls_with_mass(&fit));
        fit.bounds[1][7].1 = 3.0;
        assert!(!falls_with_mass(&fit));
    }
}
