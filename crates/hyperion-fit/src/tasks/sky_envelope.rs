//! `run sky_envelope`: the sky's brightness envelope (rendering plan R06, R06.T6.b; Design note
//! 8), the brightest absolute V magnitude any star of at most a given mass reaches in each age
//! bin, which the sim's `sky::envelope::BrightnessEnvelope::build` reads.
//!
//! The envelope depends on no galaxy, only on the generator's tracks, so it is fitted once and
//! checked in (decided 2026-10-03, `decision-r06-tables.md`). Each mass node of
//! [`mass_nodes`] is the sim's own [`raw_node`] at its \[Fe/H\] nodes, η draws and samples per
//! phase, built in parallel; [`BrightnessEnvelope::assemble`] then spreads, takes the running
//! minimum over mass and adds the margin in mass order, so the envelope is bit for bit
//! [`BrightnessEnvelope::build_with`]'s. Each value is stored in integer millimagnitudes rounded
//! brighter by the sim's [`to_millimag`], so the table bounds whatever the build bounds; a bin
//! where none shines is `DARK`.
//!
//! The task's parameters are the sim's constants. The manifest records them so that the inputs
//! hash covers them, and the task refuses a manifest that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::sky::envelope::{
    AGE_BINS, AGE_SPREAD_FACTOR, BrightnessEnvelope, ETA_DRAWS, FE_H_NODES, MARGIN_MAG, MASS_NODES,
    MAX_AGE_YEARS, SAMPLES_PER_PHASE, from_millimag, mass_nodes, raw_node, to_millimag,
};
use hyperion_sim::stellar::sse::MIN_INITIAL_MASS;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The masses, M☉, of the fingerprint's probe nodes: a brown dwarf on the cooling fits, the
/// meeting of the fits and the tracks, a red dwarf, the Sun, a B star and two massive stars.
pub const FINGERPRINT_MASSES: [f64; 7] = [0.05, 0.1, 0.4, 1.0, 3.0, 25.0, 100.0];

/// The \[Fe/H\] of the fingerprint's probe nodes: the envelope's metal-poor end, which the
/// tracks read as their clamp, and solar.
pub const FINGERPRINT_FE_H: [f64; 2] = [-2.5, 0.0];

/// The probes' samples per phase: fewer than the table's, since only their values matter.
const FINGERPRINT_SAMPLES: u32 = 4;

/// The fitted envelope, in the table's units.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyEnvelopeFit {
    /// The nodes' masses, M☉, ascending.
    pub masses: Vec<f64>,
    /// Per node, per age bin, the brightest M<sub>V</sub> in millimagnitudes rounded brighter,
    /// `None` where none shines.
    pub rows: Vec<Vec<Option<i32>>>,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// Mass nodes.
    pub nodes: usize,
    /// Tracks built: one per node from 0.1 M☉ up, \[Fe/H\] node and η draw.
    pub tracks: usize,
    /// Bins, all nodes together, where none shines.
    pub dark: usize,
    /// The largest amount, mag, by which a stored value is brighter than the build's.
    pub largest_rounding: f64,
    /// The least amount, mag, by which a stored value is brighter than the build's: never
    /// negative, or the table would not bound the build.
    pub least_rounding: f64,
    /// The brightest stored magnitude.
    pub brightest: f64,
}

/// The envelope built on `threads` threads, one node a chunk, assembled in mass order: the same
/// bits for any thread count, and those of `BrightnessEnvelope::build_with(&FE_H_NODES,
/// SAMPLES_PER_PHASE)`.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If a rounded value is fainter than the build's, which [`to_millimag`] rules out.
pub fn fit(threads: NonZeroUsize) -> Result<SkyEnvelopeFit, BuildThreadPoolError> {
    let masses = mass_nodes();
    let n = u64::try_from(masses.len()).expect("some two hundred nodes");
    let mut raw = Vec::with_capacity(masses.len());
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|j| {
                    let j = usize::try_from(j).expect("a node index fits in usize");
                    raw_node(masses[j], &FE_H_NODES, SAMPLES_PER_PHASE)
                })
                .collect::<Vec<_>>()
        },
        |chunk| raw.extend(chunk),
    )?;
    let envelope = BrightnessEnvelope::assemble(masses.clone(), raw);
    let (_, brightest) = envelope.rows();
    let mut figures = Figures {
        nodes: masses.len(),
        tracks: masses
            .iter()
            .filter(|&&m| m >= MIN_INITIAL_MASS.value())
            .count()
            * FE_H_NODES.len()
            * ETA_DRAWS.len(),
        dark: 0,
        largest_rounding: 0.0,
        least_rounding: f64::INFINITY,
        brightest: f64::INFINITY,
    };
    let rows = brightest
        .iter()
        .map(|bins| {
            bins.iter()
                .map(|&v| {
                    let k = to_millimag(v);
                    match k {
                        None => figures.dark += 1,
                        Some(k) => {
                            let stored = from_millimag(k);
                            assert!(stored <= v, "{stored} is fainter than {v}");
                            figures.largest_rounding = figures.largest_rounding.max(v - stored);
                            figures.least_rounding = figures.least_rounding.min(v - stored);
                            figures.brightest = figures.brightest.min(stored);
                        }
                    }
                    k
                })
                .collect()
        })
        .collect();
    Ok(SkyEnvelopeFit {
        masses,
        rows,
        figures,
    })
}

/// The task's parameters: the sim's constants, which its manifest must repeat.
///
/// # Panics
///
/// Never: the counts are a few hundred, which fit in an `i64`.
#[must_use]
pub fn expected_params() -> toml::Table {
    let floats = |values: &[f64]| {
        toml::Value::Array(values.iter().map(|&v| toml::Value::Float(v)).collect())
    };
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let mut table = toml::Table::new();
    table.insert("mass_nodes".to_owned(), count(MASS_NODES));
    table.insert("fe_h_nodes".to_owned(), floats(&FE_H_NODES));
    table.insert("eta_draws".to_owned(), floats(&ETA_DRAWS));
    table.insert(
        "samples_per_phase".to_owned(),
        toml::Value::Integer(i64::from(SAMPLES_PER_PHASE)),
    );
    table.insert("age_bins".to_owned(), count(AGE_BINS));
    table.insert(
        "max_age_years".to_owned(),
        toml::Value::Float(MAX_AGE_YEARS),
    );
    table.insert(
        "age_spread_factor".to_owned(),
        toml::Value::Float(AGE_SPREAD_FACTOR),
    );
    table.insert("margin_mag".to_owned(), toml::Value::Float(MARGIN_MAG));
    table
}

/// The table's body: `DARK`, then the rows as a `static`, one node a line, too large for a
/// `const`.
fn body(fit: &SkyEnvelopeFit) -> String {
    let mut out = String::new();
    out.push_str(
        "/// Marks a bin where no star of the node shines: an envelope of +∞.\n\
         pub const DARK: i32 = i32::MAX;\n\n",
    );
    out.push_str(
        "/// The mass nodes' initial masses, M☉, ascending (`sky::envelope::mass_nodes`): even in\n\
         /// ln m over 0.0124–150 M☉, with every band edge and 0.1 M☉ among them.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(out, "pub static MASSES: [f64; {}] = [", fit.masses.len());
    for &m in &fit.masses {
        let _ = writeln!(out, "    {},", literal(m));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// Per mass node, per age bin (one from 0 to 10⁴ years, then 192 even in log age to\n\
         /// 1.5 × 10¹⁰ years), the brightest absolute V magnitude of a star of at most the node's\n\
         /// mass, margin included, in millimagnitudes rounded brighter; [`DARK`] where none shines.\n\
         #[rustfmt::skip]\n",
    );
    let width = fit.rows.first().map_or(0, Vec::len);
    let _ = writeln!(
        out,
        "pub static BRIGHTEST_MMAG: [[i32; {width}]; {}] = [",
        fit.rows.len()
    );
    for row in &fit.rows {
        let cells: Vec<String> = row
            .iter()
            .map(|k| k.map_or_else(|| "DARK".to_owned(), |k| k.to_string()))
            .collect();
        let _ = writeln!(out, "    [{}],", cells.join(", "));
    }
    out.push_str("];\n");
    out
}

/// The table's contents.
#[must_use]
pub fn render(fit: &SkyEnvelopeFit) -> RustTable {
    let notes = format!(
        "\
Each node is `sky::envelope::raw_node`: the brightest V of the tracks of its mass at
{z} \\[Fe/H\\] nodes (−2.5 to +0.18) and {e} Reimers η draws (from η = 0, no Reimers wind, to
+7σ), each phase cut into {s} parts, and of the cooling fits at and below 0.1 M☉. Then
`BrightnessEnvelope::assemble` spreads each node over ages within a factor of {spread}, takes the
running minimum over mass and brightens by the {margin} mag margin. Each value is rounded toward
−∞ to the millimagnitude (`sky::envelope::to_millimag`), so the table bounds the build.",
        z = FE_H_NODES.len(),
        e = ETA_DRAWS.len(),
        s = SAMPLES_PER_PHASE,
        spread = AGE_SPREAD_FACTOR,
        margin = MARGIN_MAG,
    );
    RustTable {
        summary: vec![
            "The sky's brightness envelope (rendering plan R06, R06.T6.b; Design note 8): the"
                .to_owned(),
            "brightest absolute V magnitude stars of at most each mass reach in each age bin,"
                .to_owned(),
            "which `sky::envelope::BrightnessEnvelope::build` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task (R06.T6.b, decided 2026-10-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SkyEnvelopeTask;

impl FitTask for SkyEnvelopeTask {
    fn name(&self) -> &'static str {
        "sky_envelope"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "sky_envelope.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["DARK", "MASSES", "BRIGHTEST_MMAG"]
    }

    /// The mass nodes' count and ends, and, at [`FINGERPRINT_MASSES`] and each of
    /// [`FINGERPRINT_FE_H`] with [`FINGERPRINT_SAMPLES`] parts a phase, each raw node's brightest
    /// bin, the sum of its shining bins and their number: some sixty tracks, which `just ci`'s
    /// fit-check can afford.
    fn fingerprint(&self) -> SimFingerprint {
        let masses = mass_nodes();
        #[expect(clippy::cast_precision_loss, reason = "some two hundred nodes")]
        let nodes = masses.len() as f64;
        let mut probes = vec![
            ("mass_nodes.len()".to_owned(), nodes),
            ("mass_nodes[0]".to_owned(), masses[0]),
            ("mass_nodes[last]".to_owned(), masses[masses.len() - 1]),
        ];
        for (m, fe_h) in FINGERPRINT_FE_H
            .iter()
            .flat_map(|&z| FINGERPRINT_MASSES.map(|m| (m, z)))
        {
            let bins = raw_node(m, &[fe_h], FINGERPRINT_SAMPLES);
            let shining: Vec<f64> = bins.iter().copied().filter(|v| v.is_finite()).collect();
            let brightest = shining.iter().copied().fold(f64::INFINITY, f64::min);
            // A node that never shines has no brightest bin; its probe is 0, a finite stand-in.
            let brightest = if brightest.is_finite() {
                brightest
            } else {
                0.0
            };
            #[expect(clippy::cast_precision_loss, reason = "at most 193 bins")]
            let count = shining.len() as f64;
            let sum = shining.iter().fold(0.0, |a, &v| a + v);
            let at = format!("raw_node({m} M☉, [Fe/H] {fe_h})");
            probes.push((format!("{at}.brightest"), brightest));
            probes.push((format!("{at}.sum"), sum));
            probes.push((format!("{at}.shining"), count));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        manifest.expect_params(&expected_params())?;
        let fit = fit(threads)?;
        let f = fit.figures;
        Ok(TaskOutput {
            table: render(&fit),
            source: "the generator's own tracks and cooling fits (plan 06: Hurley, Pols and Tout \
                     2000, MNRAS 315, 543; above 100 M☉ P06.T14's factors fitted to Yusof et al. \
                     2013, MNRAS 433, 1114; below 0.1 M☉ P06.T13's cooling fits, Burrows, \
                     Hubbard, Lunine and Liebert 2001, Rev. Mod. Phys. 73, 719, and Baraffe et al. \
                     2015, A&A 577, A42) through R06's photometry \
                     (`sky::photometry::absolute_v_of_state`; V from the bolometric corrections of \
                     Pecaut and Mamajek 2013, ApJS 208, 9)"
                .to_owned(),
            acceptance: format!(
                "{} mass nodes × {} age bins from {} tracks; {} bins dark; every stored value \
                 brighter than the build's by {:.6} to {:.6} mag (never fainter, so the table \
                 bounds the build); brightest {:.3} mag",
                f.nodes,
                AGE_BINS + 1,
                f.tracks,
                f.dark,
                f.least_rounding,
                f.largest_rounding,
                f.brightest,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered body declares its items with the sim's shapes, and writes dark bins as
    /// `DARK`.
    #[test]
    fn the_body_declares_the_items_the_sim_reads() {
        let fit = SkyEnvelopeFit {
            masses: vec![0.5, 1.0],
            rows: vec![vec![Some(-1234), None], vec![Some(-2000), Some(30_000)]],
            figures: Figures {
                nodes: 2,
                tracks: 0,
                dark: 1,
                largest_rounding: 0.0,
                least_rounding: 0.0,
                brightest: -2.0,
            },
        };
        let TableItem::Source(body) = &render(&fit).items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains("pub const DARK: i32 = i32::MAX;"));
        assert!(body.contains("pub static MASSES: [f64; 2] = ["));
        assert!(body.contains("pub static BRIGHTEST_MMAG: [[i32; 2]; 2] = ["));
        assert!(body.contains("    [-1234, DARK],\n"));
    }

    /// The manifest's parameters are the sim's constants.
    #[test]
    fn the_manifest_repeats_the_sims_constants() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/sky_envelope.toml");
        let manifest = Manifest::load(&path).expect("the committed manifest loads");
        assert_eq!(manifest.task(), "sky_envelope");
        manifest
            .expect_params(&expected_params())
            .expect("the manifest's parameters are the code's");
    }
}
