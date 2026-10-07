//! `run sky_binary_light_c`, `sky_binary_light_d` and `sky_binary_light_e`: what pair evolution
//! changes in the sky's luminosity tables (rendering plan R06, R06.T5.d; decided 2026-10-03,
//! `decision-r06-tables.md`), which the sim's `sky::binary_light` adds to `LuminosityTables`.
//!
//! For each cell of a layer, an \[Fe/H\] node of [`FE_H_NODES`] and a log-age bin of
//! [`age_edge`](hyperion_sim::sky::binary_light::age_edge), the task averages the sim's own [`system_difference`] over the cell's first
//! systems: per 1-mag M<sub>V</sub> bin, the pair-evolved less single-evolved V light, colour sums
//! and star count, per born system of the layer, with the standard error of the total light's
//! difference. The table is independent of any galaxy: its systems are drawn in the fit's own
//! ([`fit_galaxy`]), by the generator's laws.
//!
//! The ruling named one task, `sky_binary_light`; it is one a layer, as `stellar_fates` is one a
//! mass panel, so that each table stays under the repository's 500 kB (decided 2026-10-04).
//!
//! The systems of a cell are cut into chunks of [`CHUNK`], each summed in index order and the
//! chunks added in index order, so the table is the same for any thread count. Each value is
//! stored to four significant digits ([`stored`]), far below the sampling's noise. The task's
//! constants are the sim's; the manifest repeats them, and the task refuses one that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::id::Layer;
use hyperion_sim::sky::binary_light::{
    AGE_BINS, AGE_STEP_DEX, CELLS, CellSums, FE_H_NODES, FIRST_AGE_LOG10, GALAXY_SEED,
    LAST_AGE_YEARS, MAGNITUDE_BINS, MAX_CELL_SYSTEMS, VALUES, cell_sums, fit_galaxy,
    system_difference,
};
use hyperion_sim::sky::luminosity::{BRIGHTEST_MAGNITUDE, MAGNITUDE_STEP};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// Systems per chunk of the parallel sampling. The thread count cannot change the result, but the
/// chunk is part of each cell's summation order, so the manifests record it.
pub const CHUNK: u32 = 250;

/// The layer a task fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FitLayer {
    /// Layer C, 0.75–2.5 M☉.
    C,
    /// Layer D, 2.5–8 M☉.
    D,
    /// Layer E, 8–150 M☉.
    E,
}

impl FitLayer {
    /// The sim's layer.
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::C => Layer::C,
            Self::D => Layer::D,
            Self::E => Layer::E,
        }
    }

    /// The systems a cell of the committed table averages: the ruling's 2 × 10⁴, and 2 × 10⁵ in
    /// layer C, whose cap rests on rare bright phases of pair channels (decided 2026-10-04, "T5.d
    /// caps after the pair correction").
    #[must_use]
    pub const fn systems_per_cell(self) -> u32 {
        match self {
            Self::C => 200_000,
            Self::D | Self::E => 20_000,
        }
    }

    /// The fingerprint's probe cells, as (\[Fe/H\] node, age bin, systems): two where the
    /// layer's primaries are giants, at solar and at a tenth of solar metallicity, with enough
    /// systems that a few dozen of them interact (layer C's systems interact least and cost
    /// least); and one young cell at solar metallicity, so that fit-check sees a change to the
    /// young pairs (decided 2026-10-04). That cell was age bin 2, where the protostars were, until
    /// plan 11's protostar fix and P11.T4.i left it empty in every layer at generator version 21.
    /// It is now the youngest solar cell whose first systems include a handful that pair
    /// evolution changes, at a cost of a few seconds: age bin 12 (25–40 Myr) in C and D, on
    /// 20,000 and 10,000 systems, and age bin 3 (0.4–0.6 Myr) in E, on 2,000 (14, 8 and 9 changed
    /// at 21; amended 2026-10-07, R06's Risks, "Generator version 21").
    #[must_use]
    pub const fn probe_cells(self) -> [(usize, usize, u32); 3] {
        match self {
            Self::C => [(3, 21, 640), (1, 24, 640), (3, 12, 20_000)],
            Self::D => [(3, 16, 96), (1, 17, 96), (3, 12, 10_000)],
            Self::E => [(3, 11, 48), (1, 12, 48), (3, 3, 2_000)],
        }
    }
}

/// How many systems each cell averages: `systems_per_cell`, and more in the raised cells, which
/// carry a component bin's correction error (decided 2026-10-04, "T5.d gate reading": a bin that
/// fails the component guard takes more systems in those cells, never a smaller correction).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Sampling {
    /// The systems of every cell not raised.
    pub systems_per_cell: u32,
    /// The raised cells and their systems, by ascending cell.
    pub raised: Vec<(usize, u32)>,
}

impl Sampling {
    /// `systems_per_cell` systems in every cell.
    #[must_use]
    pub const fn uniform(systems_per_cell: u32) -> Self {
        Self {
            systems_per_cell,
            raised: Vec::new(),
        }
    }

    /// The systems `cell` averages.
    #[must_use]
    pub fn systems(&self, cell: usize) -> u32 {
        self.raised
            .iter()
            .find(|&&(c, _)| c == cell)
            .map_or(self.systems_per_cell, |&(_, n)| n)
    }

    /// The systems of every cell together.
    #[must_use]
    pub fn total(&self) -> u64 {
        (0..CELLS).map(|cell| u64::from(self.systems(cell))).sum()
    }

    /// The manifest's `raised_cells`, `[[cell, systems], …]`, checked: cells ascending and below
    /// [`CELLS`], each raised above `systems_per_cell`.
    fn of_manifest(manifest: &Manifest, systems_per_cell: u32) -> Result<Self, ManifestParamError> {
        let wrong = || {
            ManifestParamError::new(
                "raised_cells",
                "pairs [cell, systems], cells ascending below 130, systems above systems_per_cell \
                 and at most 2^24",
            )
        };
        let mut raised: Vec<(usize, u32)> = Vec::new();
        if let Some(value) = manifest.params().get("raised_cells") {
            for pair in value.as_array().ok_or_else(wrong)? {
                let pair = pair.as_array().ok_or_else(wrong)?;
                let [cell, systems] = pair.as_slice() else {
                    return Err(wrong());
                };
                let cell = cell
                    .as_integer()
                    .and_then(|c| usize::try_from(c).ok())
                    .filter(|&c| c < CELLS && raised.last().is_none_or(|&(last, _)| last < c))
                    .ok_or_else(wrong)?;
                let systems = systems
                    .as_integer()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|&n| n > systems_per_cell && n <= MAX_CELL_SYSTEMS)
                    .ok_or_else(wrong)?;
                raised.push((cell, systems));
            }
        }
        Ok(Self {
            systems_per_cell,
            raised,
        })
    }
}

/// One layer's fitted table, values as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerFit {
    /// The first 1-mag bin held.
    pub first_bin: usize,
    /// Per cell, per bin held, the mean differences.
    pub delta: Vec<Vec<[f64; VALUES]>>,
    /// Per cell, the standard error of the mean total light difference, L☉,V.
    pub light_se: Vec<f64>,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// The systems each cell averages.
    pub sampling: Sampling,
    /// The systems pair evolution changed, over all cells.
    pub changed: u64,
    /// The least and the greatest mean total light difference of a cell, L☉,V per system.
    pub light_range: (f64, f64),
    /// The greatest standard error of a cell's mean total light difference, L☉,V.
    pub largest_se: f64,
}

/// `v` rounded to four significant digits, as the table stores it.
///
/// # Panics
///
/// Never: Rust's own exponent format parses back.
#[must_use]
pub fn stored(v: f64) -> f64 {
    text(v).parse().expect("the exponent format parses")
}

/// The source text of a stored value: `0.0` for zero, otherwise four significant digits with an
/// exponent, which no Clippy lint on literals reads as a constant or as unreadable.
#[must_use]
fn text(v: f64) -> String {
    if v.abs() > 0.0 {
        format!("{v:.3e}")
    } else {
        "0.0".to_owned()
    }
}

/// The table of `layer` from `sampling`'s systems, on `threads` threads: the same for any thread
/// count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If a cell averages no system.
pub fn fit(
    layer: FitLayer,
    sampling: &Sampling,
    threads: NonZeroUsize,
) -> Result<LayerFit, BuildThreadPoolError> {
    // Each cell's chunks, cell by cell: a cell's sums are its chunks' in this order.
    let mut chunks: Vec<(usize, std::ops::Range<u32>)> = Vec::new();
    for cell in 0..CELLS {
        let systems = sampling.systems(cell);
        assert!(systems > 0, "a cell averages at least one system");
        chunks.extend(
            (0..systems.div_ceil(CHUNK))
                .map(|chunk| (cell, chunk * CHUNK..((chunk + 1) * CHUNK).min(systems))),
        );
    }
    let galaxy = fit_galaxy();
    let mut sums = vec![CellSums::default(); CELLS];
    map_reduce_chunks(
        u64::try_from(chunks.len()).expect("some 10⁴ chunks"),
        1,
        threads,
        |items| {
            items
                .map(|item| {
                    let (cell, range) = &chunks[usize::try_from(item).expect("a chunk's index")];
                    (
                        *cell,
                        cell_sums(&galaxy, layer.layer(), *cell, range.clone()),
                    )
                })
                .collect::<Vec<_>>()
        },
        |part| {
            for (cell, s) in part {
                sums[cell].merge(&s);
            }
        },
    )?;
    let means: Vec<[[f64; VALUES]; MAGNITUDE_BINS]> = sums
        .iter()
        .map(|s| s.mean().map(|b| b.map(stored)))
        .collect();
    let held: Vec<usize> = (0..MAGNITUDE_BINS)
        .filter(|&j| means.iter().any(|m| m[j].iter().any(|v| v.abs() > 0.0)))
        .collect();
    let (first_bin, end) = match (held.first(), held.last()) {
        (Some(&a), Some(&b)) => (a, b + 1),
        _ => (0, 0),
    };
    let light_se: Vec<f64> = sums
        .iter()
        .map(|s| stored(s.light_standard_error()))
        .collect();
    let totals: Vec<f64> = means
        .iter()
        .map(|m| m.iter().fold(0.0, |a, b| a + b[0]))
        .collect();
    let figures = Figures {
        sampling: sampling.clone(),
        changed: sums.iter().map(CellSums::changed).sum(),
        light_range: totals
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &t| {
                (lo.min(t), hi.max(t))
            }),
        largest_se: light_se.iter().copied().fold(0.0, f64::max),
    };
    Ok(LayerFit {
        first_bin,
        delta: means.iter().map(|m| m[first_bin..end].to_vec()).collect(),
        light_se,
        figures,
    })
}

/// The task's parameters: the sim's constants, the luminosity tables' bin edges the differences are
/// binned by, [`CHUNK`] and `sampling`'s counts (`raised_cells` only where some are), which its
/// manifest must repeat.
///
/// # Panics
///
/// Never: the counts are small, and the seed is below 2⁶³.
#[must_use]
pub fn expected_params(sampling: &Sampling) -> toml::Table {
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let mut table = toml::Table::new();
    table.insert(
        "systems_per_cell".to_owned(),
        toml::Value::Integer(i64::from(sampling.systems_per_cell)),
    );
    if !sampling.raised.is_empty() {
        table.insert(
            "raised_cells".to_owned(),
            toml::Value::Array(
                sampling
                    .raised
                    .iter()
                    .map(|&(cell, systems)| {
                        toml::Value::Array(vec![
                            count(cell),
                            toml::Value::Integer(i64::from(systems)),
                        ])
                    })
                    .collect(),
            ),
        );
    }
    table.insert(
        "fe_h_nodes".to_owned(),
        toml::Value::Array(FE_H_NODES.iter().map(|&v| toml::Value::Float(v)).collect()),
    );
    table.insert(
        "first_age_log10".to_owned(),
        toml::Value::Float(FIRST_AGE_LOG10),
    );
    table.insert("age_step_dex".to_owned(), toml::Value::Float(AGE_STEP_DEX));
    table.insert("age_bins".to_owned(), count(AGE_BINS));
    table.insert(
        "last_age_years".to_owned(),
        toml::Value::Float(LAST_AGE_YEARS),
    );
    table.insert("magnitude_bins".to_owned(), count(MAGNITUDE_BINS));
    table.insert(
        "brightest_magnitude".to_owned(),
        toml::Value::Float(BRIGHTEST_MAGNITUDE),
    );
    table.insert(
        "magnitude_step".to_owned(),
        toml::Value::Float(MAGNITUDE_STEP),
    );
    table.insert("chunk".to_owned(), toml::Value::Integer(i64::from(CHUNK)));
    table.insert(
        "galaxy_seed".to_owned(),
        toml::Value::Integer(i64::try_from(GALAXY_SEED).expect("the seed is below 2^63")),
    );
    table
}

/// The table's body: the first bin, the differences one cell a line, and the standard errors one
/// \[Fe/H\] node a line.
fn body(fit: &LayerFit) -> String {
    let width = fit.delta.first().map_or(0, Vec::len);
    let mut out = String::new();
    let _ = write!(
        out,
        "/// The first 1-mag M<sub>V</sub> bin the table holds, counted from M<sub>V</sub> {BRIGHTEST_MAGNITUDE}\n\
         /// (`sky::binary_light::MAGNITUDE_BINS`); the bins outside the held ones are zero in every cell.\n\
         pub const FIRST_BIN: usize = {};\n\n",
        fit.first_bin
    );
    out.push_str(
        "/// Per cell (each \\[Fe/H\\] node of `sky::binary_light::FE_H_NODES`, then each age bin of\n\
         /// `sky::binary_light::age_edge`), per 1-mag bin from [`FIRST_BIN`], the mean per born system\n\
         /// of the layer of the pair-evolved less the single-evolved V light (L☉,V), its four colour\n\
         /// sums (the light times `lux_per_v0`, times that and each chroma channel, and times that and\n\
         /// ρ) and its star count.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static DELTA: [[[f64; {VALUES}]; {width}]; {}] = [",
        fit.delta.len()
    );
    for cell in &fit.delta {
        let bins: Vec<String> = cell
            .iter()
            .map(|b| {
                let values: Vec<String> = b.iter().map(|&v| text(v)).collect();
                format!("[{}]", values.join(", "))
            })
            .collect();
        let _ = writeln!(out, "    [{}],", bins.join(", "));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// Per cell, as [`DELTA`], the standard error of its mean difference in total V light,\n\
         /// L☉,V per born system.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static LIGHT_SE: [f64; {}] = [",
        fit.light_se.len()
    );
    for node in fit.light_se.chunks(AGE_BINS) {
        let values: Vec<String> = node.iter().map(|&v| text(v)).collect();
        let _ = writeln!(out, "    {},", values.join(", "));
    }
    out.push_str("];\n");
    out
}

/// The raised cells, as the notes and the acceptance name them: empty for none.
fn raised_text(sampling: &Sampling) -> String {
    if sampling.raised.is_empty() {
        return String::new();
    }
    let cells: Vec<String> = sampling
        .raised
        .iter()
        .map(|&(cell, systems)| format!("cell {cell} {systems}"))
        .collect();
    format!(" (raised: {})", cells.join(", "))
}

/// The table's contents.
#[must_use]
pub fn render(layer: FitLayer, fit: &LayerFit) -> RustTable {
    let notes = format!(
        "\
Each cell averages `sky::binary_light::system_difference` over its first {n} systems{raised}: the
primary from the default mass function within layer {l:?}'s band and the age log-uniform within
the cell's 0.2-dex bin, on the R2 low-discrepancy sequence, and the companions and orbits from
plan 11's laws in the fit's own galaxy (seed {seed:#x}); each system's stars as
`SystemStars::state_at` gives them at the epoch, less each `StarModel` alone. Values are stored to
four significant digits.",
        n = fit.figures.sampling.systems_per_cell,
        raised = raised_text(&fit.figures.sampling),
        l = layer.layer(),
        seed = GALAXY_SEED,
    );
    RustTable {
        summary: vec![
            format!(
                "What pair evolution changes in layer {:?}'s luminosity tables (rendering plan R06,",
                layer.layer()
            ),
            "R06.T5.d): the mean pair-evolved less single-evolved light, colour and count per"
                .to_owned(),
            "born system, which `sky::binary_light` adds to `LuminosityTables`.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task of one layer: R06.T5.d, slow, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SkyBinaryLightTask {
    /// The layer it fits.
    pub layer: FitLayer,
}

/// Layer C's task.
pub static C_TASK: SkyBinaryLightTask = SkyBinaryLightTask { layer: FitLayer::C };

/// Layer D's task.
pub static D_TASK: SkyBinaryLightTask = SkyBinaryLightTask { layer: FitLayer::D };

/// Layer E's task.
pub static E_TASK: SkyBinaryLightTask = SkyBinaryLightTask { layer: FitLayer::E };

impl FitTask for SkyBinaryLightTask {
    fn name(&self) -> &'static str {
        match self.layer {
            FitLayer::C => "sky_binary_light_c",
            FitLayer::D => "sky_binary_light_d",
            FitLayer::E => "sky_binary_light_e",
        }
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        match self.layer {
            FitLayer::C => "sky_binary_light_c.rs",
            FitLayer::D => "sky_binary_light_d.rs",
            FitLayer::E => "sky_binary_light_e.rs",
        }
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["FIRST_BIN", "DELTA", "LIGHT_SE"]
    }

    /// At each of the layer's probe cells ([`FitLayer::probe_cells`]), over its first systems:
    /// the summed differences of each of a bin's
    /// [`VALUES`] (the light, the four colour sums and the count), the summed difference in light
    /// times its bin's index (where the light lands), the summed absolute difference in light, and
    /// the systems pair evolution changed.
    fn fingerprint(&self) -> SimFingerprint {
        const NAMES: [&str; VALUES] = ["light", "lux", "lux_r", "lux_g", "lux_rho", "count"];
        let galaxy = fit_galaxy();
        let layer = self.layer.layer();
        let mut probes = Vec::new();
        for (fe_h, age, systems) in self.layer.probe_cells() {
            let cell = fe_h * AGE_BINS + age;
            let mut sums = [0.0; VALUES];
            let (mut placed, mut absolute, mut changed) = (0.0, 0.0, 0.0);
            for index in 0..systems {
                let d = system_difference(&galaxy, layer, cell, index);
                for (j, bin) in (0_u32..).zip(&d.bins) {
                    for (sum, v) in sums.iter_mut().zip(bin) {
                        *sum += v;
                    }
                    placed += f64::from(j) * bin[0];
                }
                absolute += d.light.abs();
                if d.bins.iter().flatten().any(|v| v.abs() > 0.0) {
                    changed += 1.0;
                }
            }
            let at = format!("cell([Fe/H] {}, age bin {age})", FE_H_NODES[fe_h]);
            for (name, sum) in NAMES.iter().zip(sums) {
                probes.push((format!("{at}.{name}"), sum));
            }
            probes.push((format!("{at}.light_by_bin"), placed));
            probes.push((format!("{at}.absolute_light"), absolute));
            probes.push((format!("{at}.changed"), changed));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let systems = u32::try_from(manifest.u64("systems_per_cell")?)
            .ok()
            .filter(|&n| n > 0 && n <= MAX_CELL_SYSTEMS)
            .ok_or_else(|| ManifestParamError::new("systems_per_cell", "from 1 to 2^24"))?;
        let sampling = Sampling::of_manifest(manifest, systems)?;
        manifest.expect_params(&expected_params(&sampling))?;
        let fit = fit(self.layer, &sampling, threads)?;
        let f = &fit.figures;
        let width = fit.delta.first().map_or(0, Vec::len);
        #[expect(clippy::cast_precision_loss, reason = "bin indices below 32")]
        let edge = |j: usize| BRIGHTEST_MAGNITUDE + j as f64;
        let total = sampling.total();
        Ok(TaskOutput {
            table: render(self.layer, &fit),
            source: "the generator's own pair evolution (plan 11: Moe and Di Stefano 2017, ApJS \
                     230, 15, for the companions and orbits; Hurley, Tout and Pols 2002, MNRAS 329, \
                     897, BSE, for the interaction; on plan 06's tracks, Hurley, Pols and Tout \
                     2000, MNRAS 315, 543) through R06's photometry \
                     (`sky::photometry::absolute_v_of_state` and `colour_of_state`)"
                .to_owned(),
            acceptance: format!(
                "{CELLS} cells × {systems} systems{} ({total}), {} changed by pair evolution; the \
                 1-mag bins from absolute V {} to {} held; a cell's mean total light difference \
                 {:.4e} to {:.4e} L☉,V a system, its standard error at most {:.4e}",
                raised_text(&sampling),
                f.changed,
                edge(fit.first_bin),
                edge(fit.first_bin + width),
                f.light_range.0,
                f.light_range.1,
                f.largest_se,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_stored_to_four_significant_digits() {
        assert_eq!(text(0.0), "0.0");
        assert_eq!(text(-0.0), "0.0");
        assert_eq!(text(-1.234_567e-5), "-1.235e-5");
        assert_eq!(text(4.567_89), "4.568e0");
        assert!((stored(2.468_09) - 2.468).abs() < 1e-15);
    }

    /// The smoke run fits two systems a cell, the same on one thread and on three, and renders
    /// the items the sim reads.
    #[test]
    fn the_smoke_run_is_the_same_on_any_thread_count_and_renders_its_items() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("manifests/sky_binary_light_d.smoke.toml");
        let manifest = Manifest::load(&path).expect("the committed smoke manifest loads");
        let output = D_TASK
            .run(&manifest, NonZeroUsize::MIN)
            .expect("the smoke run runs");
        let again = D_TASK
            .run(&manifest, NonZeroUsize::new(3).unwrap())
            .expect("the smoke run runs");
        assert_eq!(output, again);
        let TableItem::Source(body) = &output.table.items[0] else {
            panic!("one verbatim item")
        };
        assert!(body.contains("pub const FIRST_BIN: usize = "));
        assert!(body.contains("pub static DELTA: [[[f64; 6]; "));
        assert!(body.contains("pub static LIGHT_SE: [f64; 130] = ["));
        assert!(output.acceptance.starts_with("130 cells × 2 systems (260)"));
    }

    /// A manifest's raised cells raise only themselves, and malformed ones are refused.
    #[test]
    fn raised_cells_raise_only_themselves() {
        let manifest = |raised: &str| {
            let text = format!(
                "task = \"sky_binary_light_c\"\n[params]\nsystems_per_cell = 10\n{raised}\n"
            );
            Manifest::from_bytes(std::path::Path::new("raised.toml"), text.into_bytes()).unwrap()
        };
        let sampling =
            Sampling::of_manifest(&manifest("raised_cells = [[3, 40], [71, 200]]"), 10).unwrap();
        assert_eq!(sampling.systems(3), 40);
        assert_eq!(sampling.systems(71), 200);
        assert_eq!(sampling.systems(4), 10);
        assert_eq!(sampling.total(), 128 * 10 + 40 + 200);
        assert_eq!(
            Sampling::of_manifest(&manifest(""), 10).unwrap(),
            Sampling::uniform(10)
        );
        for wrong in [
            "raised_cells = [[71, 200], [3, 40]]",
            "raised_cells = [[130, 40]]",
            "raised_cells = [[3, 10]]",
            "raised_cells = [[3]]",
            "raised_cells = [[3, 16777217]]",
            "raised_cells = 3",
        ] {
            assert!(
                Sampling::of_manifest(&manifest(wrong), 10).is_err(),
                "{wrong}"
            );
        }
        let params = expected_params(&sampling);
        assert_eq!(
            params
                .get("raised_cells")
                .and_then(toml::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert!(!expected_params(&Sampling::uniform(10)).contains_key("raised_cells"));
    }

    /// A raised cell's chunks are summed in their order whatever the thread count: a cell of
    /// several chunks fits the same on one thread and on three.
    #[test]
    fn a_cell_of_several_chunks_fits_the_same_on_any_thread_count() {
        let sampling = Sampling {
            systems_per_cell: 1,
            raised: vec![(21, 2 * CHUNK + 3)],
        };
        let one = fit(FitLayer::C, &sampling, NonZeroUsize::MIN).unwrap();
        let three = fit(FitLayer::C, &sampling, NonZeroUsize::new(3).unwrap()).unwrap();
        assert_eq!(one, three);
        assert_eq!(sampling.total(), 129 + u64::from(2 * CHUNK + 3));
    }

    /// Each layer's manifests repeat the sim's constants.
    #[test]
    fn the_manifests_repeat_the_sims_constants() {
        for task in [&C_TASK, &D_TASK, &E_TASK] {
            for (suffix, systems) in [("", task.layer.systems_per_cell()), (".smoke", 2)] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("manifests/{}{suffix}.toml", task.name()));
                let manifest = Manifest::load(&path).expect("the committed manifest loads");
                assert_eq!(manifest.task(), task.name());
                let sampling = Sampling::of_manifest(&manifest, systems)
                    .expect("the raised cells are well formed");
                manifest
                    .expect_params(&expected_params(&sampling))
                    .expect("the manifest's parameters are the code's");
            }
        }
    }
}
