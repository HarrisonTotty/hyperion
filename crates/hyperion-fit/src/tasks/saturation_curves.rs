//! The anchors of the condensables' solid branches (plan 14, P14.T49.b), which
//! `hyperion_sim::substance::diagram` adds to each published sublimation equation.
//!
//! Each condensable's saturation curve is its reference equations evaluated verbatim: the
//! ancillary vapour-pressure equation of its equation of state over the liquid, and a published
//! sublimation equation for each solid phase below the triple point (the sim's
//! `substance::diagram`). Those were measured or fitted apart, so the first sublimation equation
//! misses the liquid branch at the triple temperature (by 1.6% for methane, 2.8% for nitrogen,
//! 3.8% for sulphur dioxide and 5.7% for nitrous oxide), and a colder phase's misses the warmer's at
//! the transition between them. The anchor of a piece is the amount added to its ln p (pressure in
//! pascals) so that it meets the branch above it there, which science-p14-t24f §6.3 asks for: the
//! first meets the liquid branch, and each colder piece the anchored piece above it. So the curve
//! is continuous from the critical point down, and each piece keeps its own shape.
//!
//! Nothing is fitted by least squares: the anchors are differences of the sim's own evaluations,
//! which no `const` can compute, so the table holds them and its fingerprint holds the evaluations
//! (every equation at both ends of its piece), which a change to a coefficient moves.

use std::num::NonZeroUsize;

use hyperion_sim::substance::diagram::condensables;
use hyperion_sim::substance::{PhaseDiagram, SublimationPiece, SubstanceId};
use hyperion_sim::units::Kelvin;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The table's items, one per condensable in registry order, named by its key.
pub const ITEMS: [&str; 14] = [
    "H2O_ANCHORS",
    "CH4_ANCHORS",
    "NH3_ANCHORS",
    "N2_ANCHORS",
    "O2_ANCHORS",
    "CO2_ANCHORS",
    "AR_ANCHORS",
    "KR_ANCHORS",
    "XE_ANCHORS",
    "N2O_ANCHORS",
    "CO_ANCHORS",
    "SO2_ANCHORS",
    "H2S_ANCHORS",
    "C2H6_ANCHORS",
];

/// NIST's Chemistry Web Book's methane (SRD 69, Setzmann and Wagner 1991's equation of state),
/// fetched 2026-10-14: (K, Pa) at 100, 150 and 180 K, the cross-check plan 14's test asks for
/// (science-p14-t24f §6.3).
pub const METHANE_WEBBOOK: [(f64, f64); 3] = [
    (100.0, 34_375.892),
    (150.0, 1_039_961.3),
    (180.0, 3_285_180.7),
];

/// The table item's name for row `id`: its key in capitals, then `_ANCHORS`.
#[must_use]
pub fn item_name(id: SubstanceId) -> String {
    format!(
        "{}_ANCHORS",
        id.substance().key().as_str().to_ascii_uppercase()
    )
}

/// The temperature at the top of each piece of `diagram`'s solid branch: the triple temperature,
/// then each warmer piece's lower bound.
fn tops(diagram: &PhaseDiagram) -> impl Iterator<Item = Kelvin> + '_ {
    std::iter::once(diagram.triple_temperature())
        .chain(diagram.sublimation().iter().map(SublimationPiece::lower))
}

/// Each piece's anchor: the liquid branch's ln p at the triple temperature less the first piece's
/// there, then each anchored piece's ln p at its lower bound less the next piece's there.
#[must_use]
pub fn anchors(diagram: &PhaseDiagram) -> Vec<f64> {
    let mut above = diagram
        .vapour_pressure()
        .ln_pressure(diagram.triple_temperature());
    let mut anchors = Vec::with_capacity(diagram.sublimation().len());
    for (piece, top) in diagram.sublimation().iter().zip(tops(diagram)) {
        let anchor = above - piece.equation().ln_pressure(top);
        anchors.push(anchor);
        above = piece.equation().ln_pressure(piece.lower()) + anchor;
    }
    anchors
}

/// The worst miss of the anchored branch at its joins, in ln p: each piece with `anchors` at its
/// top against the branch above it there.
#[must_use]
pub fn worst_join(diagram: &PhaseDiagram, anchors: &[f64]) -> f64 {
    let mut above = diagram
        .vapour_pressure()
        .ln_pressure(diagram.triple_temperature());
    let mut worst = 0.0_f64;
    for ((piece, top), anchor) in diagram.sublimation().iter().zip(tops(diagram)).zip(anchors) {
        worst = worst.max((piece.equation().ln_pressure(top) + anchor - above).abs());
        above = piece.equation().ln_pressure(piece.lower()) + anchor;
    }
    worst
}

/// The table's contents for each condensable's `anchors`, in registry order.
///
/// # Panics
///
/// If a row of `anchors` has no phase diagram: [`fit`]'s rows all have one.
#[must_use]
pub fn render(anchors: Vec<(SubstanceId, Vec<f64>)>) -> RustTable {
    let items = anchors
        .into_iter()
        .map(|(id, values)| {
            let row = id.substance();
            let diagram = row
                .phase_diagram()
                .expect("a condensable has a phase diagram");
            let phases: Vec<&str> = diagram
                .sublimation()
                .iter()
                .map(SublimationPiece::phase)
                .collect();
            TableItem::Array {
                name: item_name(id),
                doc: vec![
                    format!(
                        "`{}`'s anchors, added to ln(p ÷ 1 Pa) of its sublimation equations for {},",
                        row.key(),
                        phases.join(", ")
                    ),
                    "from the triple point down.".to_owned(),
                ],
                values,
            }
        })
        .collect();
    RustTable {
        summary: vec![
            "The anchors of the condensables' solid branches (plan 14, P14.T49.b), which".to_owned(),
            "`substance::diagram` adds to each published sublimation equation.".to_owned(),
        ],
        notes: vec![
            "Each anchor is the amount added to a sublimation equation's ln p (pressure in pascals)"
                .to_owned(),
            "so that it meets the branch above it: the first piece the liquid branch at the triple"
                .to_owned(),
            "temperature, each colder one the anchored piece above at its lower bound (science-p14-t24f"
                .to_owned(),
            "§6.3). Nothing is fitted by least squares: the equations are evaluated as published."
                .to_owned(),
        ],
        items,
    }
}

/// The condensables and their anchors, in registry order.
///
/// # Panics
///
/// Never: [`condensables`] yields only rows with a phase diagram.
#[must_use]
pub fn fit() -> Vec<(SubstanceId, Vec<f64>)> {
    condensables()
        .map(|id| {
            let diagram = id
                .substance()
                .phase_diagram()
                .expect("a condensable has a phase diagram");
            (id, anchors(diagram))
        })
        .collect()
}

/// The task: P14.T49.b's anchors, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SaturationCurvesTask;

impl FitTask for SaturationCurvesTask {
    fn name(&self) -> &'static str {
        "saturation_curves"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "saturation.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// Every evaluation the anchors are made of: each condensable's liquid branch at its triple
    /// temperature, and each sublimation equation at the top and the foot of its piece; and the
    /// anchors themselves, which are small beside those evaluations (water's 1.1 × 10⁻⁷ against
    /// ln p near 6), so that the probes' relative tolerance holds each to its own size.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::new();
        for id in condensables() {
            let row = id.substance();
            let diagram = row
                .phase_diagram()
                .expect("a condensable has a phase diagram");
            let key = row.key();
            let triple = diagram.triple_temperature();
            probes.push((
                format!("{key} liquid ln p at {} K", triple.value()),
                diagram.vapour_pressure().ln_pressure(triple),
            ));
            for (k, (piece, top)) in diagram.sublimation().iter().zip(tops(diagram)).enumerate() {
                for at in [top, piece.lower()] {
                    probes.push((
                        format!("{key} piece {k} ln p at {} K", at.value()),
                        piece.equation().ln_pressure(at),
                    ));
                }
            }
            for (k, anchor) in anchors(diagram).into_iter().enumerate() {
                probes.push((format!("{key} piece {k} anchor"), anchor));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let fitted = fit();
        let names: Vec<String> = fitted.iter().map(|(id, _)| item_name(*id)).collect();
        if names != ITEMS {
            return Err(RunTaskError::Input {
                task: "saturation_curves",
                source: format!(
                    "the registry's condensables are {names:?}, not the task's {ITEMS:?}"
                )
                .into(),
            });
        }
        let mut lines = Vec::new();
        let mut worst = 0.0_f64;
        let mut pieces = 0;
        for (id, values) in &fitted {
            let diagram = id
                .substance()
                .phase_diagram()
                .expect("a condensable has a phase diagram");
            worst = worst.max(worst_join(diagram, values));
            pieces += values.len();
            let percent: Vec<String> = values
                .iter()
                .map(|a| format!("{:+.4}%", 100.0 * hyperion_sim::math::exp_m1(*a)))
                .collect();
            lines.push(format!("{} {}", id.substance().key(), percent.join(" ")));
        }
        let methane = hyperion_sim::substance::by_key("CH4")
            .and_then(|id| id.substance().phase_diagram())
            .ok_or_else(|| RunTaskError::Input {
                task: "saturation_curves",
                source: "the registry has no methane diagram".into(),
            })?;
        let webbook: Vec<String> = METHANE_WEBBOOK
            .iter()
            .map(|&(t, p)| {
                let ours = methane.saturation_pressure(Kelvin::new(t)).value();
                format!("{:+.4}% at {t} K", 100.0 * (ours / p - 1.0))
            })
            .collect();
        let acceptance = format!(
            "{} condensables, {pieces} sublimation pieces; each anchor as the factor it puts on its \
             piece's pressure: {}; the anchored branch meets the one above it within {worst:.1e} in \
             ln p at every join; methane's liquid branch against NIST's WebBook {}",
            fitted.len(),
            lines.join(", "),
            webbook.join(", "),
        );
        Ok(TaskOutput {
            table: render(fitted),
            source: "each condensable's reference equation of state's ancillary vapour-pressure \
                     equation and its published sublimation equations (IAPWS SR1-86 and R14-08; \
                     Setzmann and Wagner 1991; Span et al. 2000; Span and Wagner 1996; Tegeler et \
                     al. 1999; Stewart et al. 1991; Gao et al. 2023; CoolProp's fits to Lemmon and \
                     Span 2006, Gao et al. 2016 and Bücker and Wagner 2006; Fray and Schmitt 2009 \
                     through Lisse et al. 2021), as `substance::diagram` holds them"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed table is this fit, bit for bit, item by item.
    #[test]
    fn saturation_the_committed_anchors_are_the_fit() {
        for (id, values) in fit() {
            let committed = id
                .substance()
                .phase_diagram()
                .expect("a condensable has a phase diagram")
                .anchors();
            assert_eq!(committed.len(), values.len(), "{:?}", id.substance().key());
            for (a, b) in committed.iter().zip(&values) {
                assert!(
                    a.total_cmp(b).is_eq(),
                    "{:?}: {a} against {b}",
                    id.substance().key()
                );
            }
        }
    }

    /// The anchored branch meets the liquid branch and each piece the one above it to rounding,
    /// and the items are the condensables in registry order.
    #[test]
    fn saturation_the_anchors_join_the_branches() {
        let fitted = fit();
        let names: Vec<String> = fitted.iter().map(|(id, _)| item_name(*id)).collect();
        assert_eq!(names, ITEMS);
        for (id, values) in &fitted {
            let diagram = id.substance().phase_diagram().unwrap();
            assert!(
                worst_join(diagram, values) < 1e-13,
                "{:?}",
                id.substance().key()
            );
            assert_eq!(values.len(), diagram.sublimation().len());
        }
        let table = render(fitted);
        assert_eq!(table.item_names(), ITEMS);
    }

    /// A piece's anchor is the miss it closes: each first piece's against the liquid branch at the
    /// triple point, as a second, independent transcription of the same equations evaluates it
    /// (the lane's research for this step, `research/saturation/data.json`): methane's Fray and
    /// Schmitt polynomial runs 1.6% above Setzmann and Wagner's liquid branch, nitrous oxide's
    /// 5.7%, and Span and Wagner's own sublimation equation meets their vapour pressure to 2 × 10⁻⁵.
    #[test]
    fn saturation_anchors_close_the_published_misses() {
        for (key, expected) in [
            ("H2O", 1.140_190e-7),
            ("CH4", -1.627_644e-2),
            ("NH3", 1.502_111e-4),
            ("N2", 2.801_975e-2),
            ("O2", -5.575_764e-4),
            ("CO2", 2.285_312e-5),
            ("Ar", 2.820_998e-5),
            ("Kr", 2.768_038e-3),
            ("Xe", -1.639_422e-3),
            ("N2O", -5.511_076e-2),
            ("CO", -1.086_997e-3),
            ("SO2", -3.716_071e-2),
            ("H2S", -5.594_057e-3),
            ("C2H6", -1.601_885e-3),
        ] {
            let id = hyperion_sim::substance::by_key(key).unwrap();
            let first = anchors(id.substance().phase_diagram().unwrap())[0];
            assert!(
                (first - expected).abs() < 5e-9 + 1e-6 * expected.abs(),
                "{key}: {first} against {expected}"
            );
        }
    }
}
