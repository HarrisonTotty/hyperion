//! `run sky_phase_envelope`: the sky's phase envelope (rendering plan R06, R06.T8.m; decided
//! 2026-10-05, `decision-r06-census-cost.md`, split from R06.T8.g on 2026-10-07,
//! `decision-p11-t16-hierarchy-bound.md`): the brightest absolute V a single star can have in each
//! cell of mass, \[Fe/H\], Reimers η and age relative to its lifetime, which the sim's
//! `sky::phase::PhaseEnvelope::fitted` reads.
//!
//! The table depends on no galaxy, only on the generator's tracks, so it is fitted once and
//! checked in, as `sky_envelope` is. The run takes the lifetimes at every node
//! ([`node_lifetime`]), then each mass interval's cells ([`rows_of_interval`], one interval a
//! chunk, in parallel, each a pure function of the frame and the interval), and assembles them in
//! cell order with the sim's own [`PhaseEnvelope::assemble`]: the margin, the merged segments and
//! the rounding brighter. So the table is bit for bit what the sim builds from the same rows, for any
//! thread count.
//!
//! The task's parameters are the sim's constants, and the mass intervals it samples: all of them
//! for the table, two near 0.1 M☉ for the smoke run. The manifest records them so that the inputs
//! hash covers them, and the task refuses a manifest that disagrees.

use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::ops::Range;

use hyperion_sim::sky::envelope::{
    FE_H_NODES, MARGIN_MAG, MAX_AGE_YEARS, SAMPLES_PER_PHASE, mass_nodes,
};
use hyperion_sim::sky::phase::{
    BINS, COARSE_BINS, ETA_NODES, ETA_SAMPLES, FAINT_FROM_MAG, FAINT_TOLERANCE_SLOPE, FE_H_SAMPLES,
    FINE_BIN_WIDTH, FINE_BINS, FINE_START, FIRST_RELATIVE_AGE, FORMAT, LIFETIME_CAP_YEARS,
    LIFETIME_UNITS_PER_DEX, MASS_SAMPLES, MERGE_TOLERANCE_MAG, PhaseEnvelope, PhaseFrame, SPREADS,
    mass_sample, node_lifetime, node_lifetimes, quantize_lifetime, rows_of_interval, sample_parts,
};
use hyperion_sim::stellar::draws::StandardNormal;
use hyperion_sim::stellar::sse::MIN_INITIAL_MASS;
use hyperion_sim::units::{Dex, SolarMasses};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// [`FINE_BINS`] as a `u32`, for the notes.
#[expect(clippy::cast_possible_truncation, reason = "250 bins")]
const FINE_BINS_U32: u32 = FINE_BINS as u32;

/// The fingerprint's probe samples, (M☉, \[Fe/H\], η draw).
///
/// They are a brown dwarf on the cooling fits, the meeting of the fits and the tracks, a red
/// dwarf, a metal-poor and a solar turnoff star at the η = 0 extreme and the median, a B star, two
/// stars of the electron-capture window (whose samples carry both companion-stripped marks), a
/// massive star, and one of the 50–110 M☉ stars whose excursions set the widest spreads.
pub const FINGERPRINT_SAMPLES: [(f64, f64, f64); 11] = [
    (0.05, 0.0, 0.0),
    (0.1, -2.5, 0.0),
    (0.4, 0.0, 0.0),
    (0.9, -2.5, -0.5 / 0.07),
    (1.0, 0.0, 0.0),
    (1.0, 0.18, 3.5),
    (3.0, -1.0, 0.0),
    (6.5, 0.0, 0.0),
    (8.0, -1.0, 0.0),
    (25.0, 0.0, 0.0),
    (80.0, 0.18, -3.5),
];

/// The most bytes the committed table may take: the coordinator's ceiling for the packed format
/// (2026-10-07), with the 500 kB hook beyond it. A refit past it needs a new ruling.
pub const TABLE_BYTES_AT_MOST: u64 = 400_000;

/// The fitted phase envelope, in the table's units.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyPhaseEnvelopeFit {
    /// The lifetimes at every node of mass, \[Fe/H\] and η, in the table's integers.
    pub lifetimes: Vec<u16>,
    /// The envelope, as the sim reads it.
    pub envelope: PhaseEnvelope,
    /// The acceptance's figures.
    pub figures: Figures,
}

/// What the run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// Cells.
    pub cells: usize,
    /// Distinct samples: masses × \[Fe/H\] × η.
    pub samples: usize,
    /// Of them, tracks built (0.1 M☉ and above).
    pub tracks: usize,
    /// Node lifetimes at or above the cap.
    pub capped: usize,
    /// Distinct rows: cells with the same segments share one.
    pub rows: usize,
    /// The distinct rows' segments.
    pub segments: usize,
    /// Cells with any shining bin beyond the fine bins, where only remnants are expected.
    pub lit_beyond: usize,
    /// How many cells took each of `SPREADS`.
    pub spreads: [usize; SPREADS.len()],
    /// Cells whose samples passed the neighbour test at no spread, which took the largest.
    pub fell_back: usize,
    /// The largest amount, mag, by which a stored value brighter than V 10 is brighter than a bin
    /// it holds, less the margin: the merging's and the rounding's looseness, at most the merge
    /// tolerance plus a millimagnitude.
    pub largest_looseness: f64,
    /// The brightest stored magnitude.
    pub brightest: f64,
}

/// The mass intervals a manifest's `mass_cells` names, `[first, end)`: every interval for the
/// table, a few for the smoke run.
///
/// # Errors
///
/// [`ManifestParamError`] if the key is missing, not two integers, or not an increasing range
/// within the intervals.
pub fn mass_cells_of(manifest: &Manifest) -> Result<Range<usize>, ManifestParamError> {
    let intervals = mass_nodes().len() - 1;
    let wrong = || {
        ManifestParamError::new(
            "mass_cells",
            &format!("two increasing integers within 0..={intervals}"),
        )
    };
    let values = manifest
        .params()
        .get("mass_cells")
        .and_then(toml::Value::as_array)
        .ok_or_else(wrong)?;
    let ends: Vec<usize> = values
        .iter()
        .map(|v| {
            v.as_integer()
                .and_then(|k| usize::try_from(k).ok())
                .ok_or_else(wrong)
        })
        .collect::<Result<_, _>>()?;
    match ends.as_slice() {
        &[first, end] if first < end && end <= intervals => Ok(first..end),
        _ => Err(wrong()),
    }
}

/// The phase envelope built on `threads` threads from the samples of the mass intervals
/// `mass_cells` (the rest left dark), one mass interval a chunk: the same bits for any thread
/// count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `mass_cells` reaches past the intervals.
pub fn fit(
    threads: NonZeroUsize,
    mass_cells: Range<usize>,
) -> Result<SkyPhaseEnvelopeFit, BuildThreadPoolError> {
    let masses = mass_nodes();
    assert!(mass_cells.end < masses.len(), "{mass_cells:?}");
    let n = u64::try_from(masses.len()).expect("some two hundred nodes");
    let mut lifetimes = Vec::with_capacity(masses.len() * FE_H_NODES.len() * ETA_NODES.len());
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .flat_map(|j| {
                    let j = usize::try_from(j).expect("a node index fits in usize");
                    node_lifetimes(&masses[j..=j])
                })
                .collect::<Vec<_>>()
        },
        |chunk| lifetimes.extend(chunk),
    )?;
    let frame = PhaseFrame::new(masses.clone(), &lifetimes);
    let mut rows = vec![[f64::INFINITY; BINS]; frame.cells()];
    let mut spreads = [0_usize; SPREADS.len()];
    let mut fell_back = 0;
    let span = u64::try_from(mass_cells.len()).expect("some two hundred intervals");
    map_reduce_chunks(
        span,
        1,
        threads,
        |range| {
            range
                .flat_map(|k| {
                    let k = usize::try_from(k).expect("an interval index fits in usize");
                    rows_of_interval(&frame, mass_cells.start + k)
                })
                .collect::<Vec<_>>()
        },
        |chunk| {
            for built in chunk {
                rows[built.cell().index()] = *built.row();
                let rung = SPREADS
                    .iter()
                    .position(|s| s.total_cmp(&built.spread()).is_eq())
                    .expect("a cell's spread is one of the ladder's");
                spreads[rung] += 1;
                fell_back += usize::from(!built.passed());
            }
        },
    )?;
    let lit_beyond = rows.iter().filter(|row| row[BINS - 1].is_finite()).count();
    let envelope = PhaseEnvelope::assemble(frame, &rows);
    let samples_per_mass = (FE_H_NODES.len() - 1) * (FE_H_SAMPLES - 1) + 1;
    let samples_per_mass = samples_per_mass * ((ETA_NODES.len() - 1) * (ETA_SAMPLES - 1) + 1);
    let sampled: Vec<SolarMasses> = mass_cells
        .clone()
        .flat_map(|j| (0..MASS_SAMPLES).map(move |k| (j, k)))
        .map(|(j, k)| {
            mass_sample(
                SolarMasses::new(masses[j]),
                SolarMasses::new(masses[j + 1]),
                k,
            )
        })
        .collect();
    let figures = Figures {
        cells: envelope.frame().cells(),
        samples: sampled.len() * samples_per_mass,
        tracks: sampled.iter().filter(|&&m| m >= MIN_INITIAL_MASS).count() * samples_per_mass,
        capped: lifetimes.iter().filter(|&&k| k == 0).count(),
        rows: envelope.row_count(),
        segments: envelope.segment_count(),
        lit_beyond,
        spreads,
        fell_back,
        largest_looseness: largest_looseness(&envelope, &rows),
        brightest: brightest(&envelope),
    };
    Ok(SkyPhaseEnvelopeFit {
        lifetimes,
        envelope,
        figures,
    })
}

/// The largest amount by which a stored value, less the margin, is brighter than a bin it holds,
/// over the values brighter than [`FAINT_FROM_MAG`], where the merge tolerance is constant.
#[must_use]
fn largest_looseness(envelope: &PhaseEnvelope, rows: &[[f64; BINS]]) -> f64 {
    let mut worst = 0.0_f64;
    for (cell, row) in rows.iter().enumerate() {
        let mut first = 0;
        for (end, value) in envelope.segments_of(cell) {
            let end = usize::from(end);
            let bright = value.filter(|&k| f64::from(k) / 1000.0 + MARGIN_MAG <= FAINT_FROM_MAG);
            if let Some(k) = bright {
                let stored = f64::from(k) / 1000.0 + MARGIN_MAG;
                for &v in &row[first..=end] {
                    worst = worst.max(v - stored);
                }
            }
            first = end + 1;
        }
    }
    worst
}

/// The brightest stored magnitude.
#[must_use]
fn brightest(envelope: &PhaseEnvelope) -> f64 {
    (0..envelope.frame().cells())
        .flat_map(|cell| envelope.segments_of(cell))
        .filter_map(|(_, v)| v)
        .map(|k| f64::from(k) / 1000.0)
        .fold(f64::INFINITY, f64::min)
}

/// The task's parameters: the sim's constants, which its manifest must repeat, and the mass
/// intervals sampled.
///
/// # Panics
///
/// Never: the counts are a few hundred, which fit in an `i64`.
#[must_use]
pub fn expected_params(mass_cells: &Range<usize>) -> toml::Table {
    let floats = |values: &[f64]| {
        toml::Value::Array(values.iter().map(|&v| toml::Value::Float(v)).collect())
    };
    let count = |n: usize| toml::Value::Integer(i64::try_from(n).expect("a small count"));
    let mut table = toml::Table::new();
    table.insert(
        "mass_cells".to_owned(),
        toml::Value::Array(vec![count(mass_cells.start), count(mass_cells.end)]),
    );
    table.insert("mass_nodes".to_owned(), count(mass_nodes().len()));
    table.insert("fe_h_nodes".to_owned(), floats(&FE_H_NODES));
    table.insert("eta_nodes".to_owned(), floats(&ETA_NODES));
    table.insert("mass_samples".to_owned(), count(MASS_SAMPLES));
    table.insert("fe_h_samples".to_owned(), count(FE_H_SAMPLES));
    table.insert("eta_samples".to_owned(), count(ETA_SAMPLES));
    table.insert(
        "samples_per_phase".to_owned(),
        toml::Value::Integer(i64::from(SAMPLES_PER_PHASE)),
    );
    table.insert(
        "first_relative_age".to_owned(),
        toml::Value::Float(FIRST_RELATIVE_AGE),
    );
    table.insert("coarse_bins".to_owned(), count(COARSE_BINS));
    table.insert("fine_start".to_owned(), toml::Value::Float(FINE_START));
    table.insert(
        "fine_bin_width".to_owned(),
        toml::Value::Float(FINE_BIN_WIDTH),
    );
    table.insert("fine_bins".to_owned(), count(FINE_BINS));
    table.insert(
        "lifetime_cap_years".to_owned(),
        toml::Value::Float(LIFETIME_CAP_YEARS),
    );
    table.insert(
        "lifetime_units_per_dex".to_owned(),
        toml::Value::Float(LIFETIME_UNITS_PER_DEX),
    );
    table.insert("spreads".to_owned(), floats(&SPREADS));
    table.insert(
        "merge_tolerance_mag".to_owned(),
        toml::Value::Float(MERGE_TOLERANCE_MAG),
    );
    table.insert(
        "faint_from_mag".to_owned(),
        toml::Value::Float(FAINT_FROM_MAG),
    );
    table.insert(
        "faint_tolerance_slope".to_owned(),
        toml::Value::Float(FAINT_TOLERANCE_SLOPE),
    );
    table.insert("format".to_owned(), toml::Value::Integer(i64::from(FORMAT)));
    table.insert("margin_mag".to_owned(), toml::Value::Float(MARGIN_MAG));
    table.insert(
        "max_age_years".to_owned(),
        toml::Value::Float(MAX_AGE_YEARS),
    );
    table
}

/// Writes `values` as the lines of a `static` array, `per_line` a line.
fn write_lines<T>(out: &mut String, values: &[T], per_line: usize, text: impl Fn(&T) -> String) {
    for line in values.chunks(per_line) {
        let cells: Vec<String> = line.iter().map(&text).collect();
        writeln!(out, "    {},", cells.join(", ")).expect("writing to a String cannot fail");
    }
}

/// The table's body: its format and dimensions, the lifetimes and each cell's row in decimal, and
/// the distinct rows packed, one a line.
#[must_use]
fn body(fit: &SkyPhaseEnvelopeFit) -> String {
    let envelope = &fit.envelope;
    let cells = envelope.frame().cells();
    let per_node = FE_H_NODES.len() * ETA_NODES.len();
    let per_mass_cell = (FE_H_NODES.len() - 1) * (ETA_NODES.len() - 1);
    let rows = envelope.packed_rows();
    let mut out = String::new();
    writeln!(
        out,
        "/// The format's version (`sky::phase::FORMAT`), which `PhaseEnvelope::fitted` checks.\n\
         pub const FORMAT: u32 = {FORMAT};\n\n\
         /// The table's dimensions, which `PhaseEnvelope::fitted` checks: mass nodes, \\[Fe/H\\]\n\
         /// nodes, η nodes, bins of relative age, distinct rows and their segments.\n\
         pub const DIMENSIONS: [usize; 6] = [{}, {}, {}, {BINS}, {}, {}];\n\n\
         /// A packed segment's value where no single star of the cell shines.\n\
         pub const DARK: i16 = i16::MAX;\n",
        envelope.frame().masses().len(),
        FE_H_NODES.len(),
        ETA_NODES.len(),
        rows.len(),
        envelope.segment_count(),
    )
    .expect("writing to a String cannot fail");
    writeln!(
        out,
        "/// The lifetime at each node of mass (`sky::envelope::mass_nodes`), \\[Fe/H\\]\n\
         /// (`sky::envelope::FE_H_NODES`) and η (`sky::phase::ETA_NODES`), in that order of\n\
         /// nesting, a mass node a line: how far it lies below the {cap:e} years cap, in\n\
         /// 10⁻⁴ dex of log₁₀ years (`sky::phase::quantize_lifetime`), 0 at or above it.\n\
         #[rustfmt::skip]\n\
         pub static LIFETIMES: [u16; {}] = [",
        fit.lifetimes.len(),
        cap = LIFETIME_CAP_YEARS,
    )
    .expect("writing to a String cannot fail");
    write_lines(&mut out, &fit.lifetimes, per_node, u16::to_string);
    out.push_str("];\n\n");
    writeln!(
        out,
        "/// Per cell (mass interval, then \\[Fe/H\\] interval, then η interval), its row among\n\
         /// [`ROWS`], counted from 0, a mass interval a line.\n\
         #[rustfmt::skip]\n\
         pub static CELL_ROWS: [u16; {cells}] = ["
    )
    .expect("writing to a String cannot fail");
    write_lines(
        &mut out,
        envelope.cell_rows(),
        per_mass_cell,
        u16::to_string,
    );
    out.push_str("];\n\n");
    out.push_str(
        "/// The distinct rows, one a line, each its segments in order of relative age. A\n\
         /// segment is four characters of RFC 4648's base64 alphabet (no padding) holding\n\
         /// three bytes: the run of bins it covers (1–255), then its value as a\n\
         /// little-endian `i16`, the brightest absolute V of its bins, margin included, in\n\
         /// millimagnitudes rounded brighter, or [`DARK`]. A row ends where its runs reach\n\
         /// the last bin.\n\
         #[rustfmt::skip]\n\
         pub static ROWS: &str = \"\\\n",
    );
    for (k, row) in rows.iter().enumerate() {
        let tail = if k + 1 == rows.len() { "\";" } else { "\\" };
        writeln!(out, "    {row}{tail}").expect("writing to a String cannot fail");
    }
    out
}

/// The table's contents.
#[must_use]
pub fn render(fit: &SkyPhaseEnvelopeFit) -> RustTable {
    let notes = format!(
        "\
Each cell is a mass interval of `sky::envelope::mass_nodes`, an interval of its {z} \\[Fe/H\\]
nodes and of {e} η nodes (η = 0 to +7σ), and a bin of relative age: one below {first:e}, {coarse}
even in log to {fine}, {fine_bins} of {width} to {end} and one beyond. Each holds the brightest V of
{ms} × {zs} × {es} single stars sampled within it (`sky::phase::rows_of_interval`), each phase cut
into {s} parts, every part entering the bins its relative ages overlap; in the electron-capture
window, with and without the companion-stripped mark. A star's relative age is its age over the
lifetime at the nodes (its track's death age, capped at {cap:e} years), interpolated linearly in
ln L within its cell. Each cell is then widened in relative age by the least of the spreads
{spreads:?}
under which every sample with neighbours on both sides along an axis is bounded by those two
alone, margin included (the largest where none is). Then `PhaseEnvelope::assemble` brightens by the {margin} mag
margin, merges runs of bins within {tolerance} mag of their brightest (growing by {slope} mag a mag
fainter than M<sub>V</sub> {faint}) into one segment at their brightest, and rounds each value
toward −∞ to the millimagnitude.",
        z = FE_H_NODES.len(),
        e = ETA_NODES.len(),
        first = FIRST_RELATIVE_AGE,
        coarse = COARSE_BINS,
        fine = FINE_START,
        fine_bins = FINE_BINS,
        width = FINE_BIN_WIDTH,
        end = FINE_START + FINE_BIN_WIDTH * f64::from(FINE_BINS_U32),
        ms = MASS_SAMPLES,
        zs = FE_H_SAMPLES,
        es = ETA_SAMPLES,
        s = SAMPLES_PER_PHASE,
        spreads = SPREADS,
        slope = FAINT_TOLERANCE_SLOPE,
        faint = FAINT_FROM_MAG,
        cap = LIFETIME_CAP_YEARS,
        margin = MARGIN_MAG,
        tolerance = MERGE_TOLERANCE_MAG,
    );
    RustTable {
        summary: vec![
            "The sky's phase envelope (rendering plan R06, R06.T8.m): the brightest absolute V"
                .to_owned(),
            "a single star can have by its mass, [Fe/H], Reimers η and age relative to its"
                .to_owned(),
            "lifetime, which `sky::phase::PhaseEnvelope::fitted` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(body(fit))],
    }
}

/// The task (R06.T8.m, decided 2026-10-05 and 2026-10-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SkyPhaseEnvelopeTask;

impl FitTask for SkyPhaseEnvelopeTask {
    fn name(&self) -> &'static str {
        "sky_phase_envelope"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "sky_phase_envelope.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "FORMAT",
            "DIMENSIONS",
            "DARK",
            "LIFETIMES",
            "CELL_ROWS",
            "ROWS",
        ]
    }

    /// The mass nodes' count and ends, and at each of [`FINGERPRINT_SAMPLES`] the node lifetime
    /// and the sample's parts: their number, the sum of their magnitudes and their brightest.
    /// Some ten tracks, which `just ci`'s fit-check can afford.
    fn fingerprint(&self) -> SimFingerprint {
        let masses = mass_nodes();
        #[expect(clippy::cast_precision_loss, reason = "some two hundred nodes")]
        let nodes = masses.len() as f64;
        let mut probes = vec![
            ("mass_nodes.len()".to_owned(), nodes),
            ("mass_nodes[0]".to_owned(), masses[0]),
            ("mass_nodes[last]".to_owned(), masses[masses.len() - 1]),
            (
                "mass_nodes.sum()".to_owned(),
                masses.iter().fold(0.0, |a, &m| a + m),
            ),
        ];
        for (m, fe_h, eta) in FINGERPRINT_SAMPLES {
            let at = format!("({m} M☉, [Fe/H] {fe_h}, η draw {eta:.3})");
            let (m, fe_h) = (SolarMasses::new(m), Dex::new(fe_h));
            let eta = StandardNormal::new(eta).expect("a probe's draw is finite");
            probes.push((
                format!("node_lifetime{at}"),
                f64::from(quantize_lifetime(node_lifetime(m, fe_h, eta))),
            ));
            let parts = sample_parts(m, fe_h, eta);
            #[expect(clippy::cast_precision_loss, reason = "a few thousand parts")]
            let count = parts.len() as f64;
            let sum = parts.iter().fold(0.0, |a, p| a + p.brightest().value());
            let brightest = parts
                .iter()
                .fold(f64::INFINITY, |a, p| a.min(p.brightest().value()));
            probes.push((format!("sample_parts{at}.len()"), count));
            probes.push((format!("sample_parts{at}.sum"), sum));
            probes.push((format!("sample_parts{at}.brightest"), brightest));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let mass_cells = mass_cells_of(manifest)?;
        manifest.expect_params(&expected_params(&mass_cells))?;
        let fit = fit(threads, mass_cells.clone())?;
        let f = fit.figures;
        Ok(TaskOutput {
            table: render(&fit),
            source: "the generator's own tracks and cooling fits (plan 06: Hurley, Pols and Tout \
                     2000, MNRAS 315, 543; above 100 M☉ P06.T14's factors fitted to Yusof et al. \
                     2013, MNRAS 433, 1114; below 0.1 M☉ P06.T13's cooling fits, Burrows, \
                     Hubbard, Lunine and Liebert 2001, Rev. Mod. Phys. 73, 719, and Baraffe et al. \
                     2015, A&A 577, A42, to which the pre-main-sequence arrivals above 0.1 M☉ are \
                     fitted too; the protostars' dark 0.5 Myr after Dunham et al. 2014, \
                     Protostars and Planets VI, 195) through R06's photometry \
                     (`sky::photometry::absolute_v_of_state`; V from the bolometric corrections of \
                     Pecaut and Mamajek 2013, ApJS 208, 9, in Mamajek's dwarf sequence of \
                     2022.04.16)"
                .to_owned(),
            acceptance: format!(
                "{} cells × {} bins (mass intervals {}..{} sampled) from {} samples (each \
                 interval sampling its own ends), {} of them tracks; {} of {} node lifetimes at \
                 the cap; cells by spread {:?} (of {:?}), {} of them passing no spread; {} \
                 distinct rows of {} segments; {} cells lit beyond the fine bins; every stored \
                 value brighter than V 10 brighter than its bins, less the margin, by at most \
                 {:.4} mag; brightest {:.3} mag",
                f.cells,
                BINS,
                mass_cells.start,
                mass_cells.end,
                f.samples,
                f.tracks,
                f.capped,
                fit.lifetimes.len(),
                f.spreads,
                SPREADS,
                f.fell_back,
                f.rows,
                f.segments,
                f.lit_beyond,
                f.largest_looseness,
                f.brightest,
            ),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_sim::sky::phase::Cell;

    fn manifest(suffix: &str) -> Manifest {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("manifests/sky_phase_envelope{suffix}.toml"));
        Manifest::load(&path).expect("the committed manifest loads")
    }

    /// The table's manifest samples every mass interval, the smoke's a few, and both repeat the
    /// sim's constants.
    #[test]
    fn the_manifests_repeat_the_sims_constants() {
        let intervals = mass_nodes().len() - 1;
        for (suffix, every) in [("", true), (".smoke", false)] {
            let manifest = manifest(suffix);
            assert_eq!(manifest.task(), "sky_phase_envelope");
            let cells = mass_cells_of(&manifest).expect("well formed");
            assert_eq!(cells == (0..intervals), every, "{suffix}: {cells:?}");
            manifest
                .expect_params(&expected_params(&cells))
                .expect("the manifest's parameters are the code's");
        }
    }

    /// The smoke run is the same on one thread and on three, leaves the intervals it does not
    /// sample dark, and renders the items the sim reads.
    #[test]
    fn the_smoke_run_is_the_same_on_any_thread_count_and_renders_its_items() {
        let manifest = manifest(".smoke");
        let task = SkyPhaseEnvelopeTask;
        let output = task
            .run(&manifest, NonZeroUsize::MIN)
            .expect("the smoke run runs");
        let again = task
            .run(&manifest, NonZeroUsize::new(3).unwrap())
            .expect("the smoke run runs");
        assert_eq!(output, again);
        let TableItem::Source(body) = &output.table.items[0] else {
            panic!("one verbatim item")
        };
        for item in task.items() {
            let kind = if ["FORMAT", "DIMENSIONS", "DARK"].contains(item) {
                "const"
            } else {
                "static"
            };
            assert!(body.contains(&format!("pub {kind} {item}:")), "{item}");
        }
        let cells = mass_cells_of(&manifest).unwrap();
        let fit = fit(NonZeroUsize::new(2).unwrap(), cells.clone()).unwrap();
        let total = fit.envelope.frame().cells();
        for cell in 0..total {
            let segments = fit.envelope.segments_of(cell);
            assert_eq!(
                segments.last().map(|&(end, _)| usize::from(end)),
                Some(BINS - 1),
                "{cell}"
            );
            let mass = Cell::from_index(cell, total)
                .expect("a cell of the frame")
                .mass();
            if cells.contains(&mass) {
                assert!(segments.iter().any(|&(_, v)| v.is_some()), "{cell}");
            } else {
                assert!(segments.iter().all(|&(_, v)| v.is_none()), "{cell}");
            }
        }
    }

    /// A malformed `mass_cells` is refused.
    #[test]
    fn a_malformed_range_of_mass_cells_is_refused() {
        let manifest = |cells: &str| {
            let text = format!("task = \"sky_phase_envelope\"\n[params]\nmass_cells = {cells}\n");
            Manifest::from_bytes(std::path::Path::new("cells.toml"), text.into_bytes()).unwrap()
        };
        assert_eq!(mass_cells_of(&manifest("[3, 5]")).unwrap(), 3..5);
        let refused =
            ManifestParamError::new("mass_cells", "two increasing integers within 0..=197");
        for wrong in ["[5, 3]", "[3, 3]", "[0, 999]", "[3]", "3", "[-1, 2]"] {
            assert_eq!(
                mass_cells_of(&manifest(wrong)),
                Err(refused.clone()),
                "{wrong}"
            );
        }
    }

    /// The committed table stays under the coordinator's ceiling for the packed format, so a
    /// refit that grows it past 400 kB fails here before the 500 kB hook refuses it.
    #[test]
    fn the_committed_table_stays_under_its_ceiling() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../hyperion-sim/src/tables/sky_phase_envelope.rs");
        let bytes = std::fs::metadata(&path)
            .expect("the committed table is there")
            .len();
        assert!(
            bytes <= TABLE_BYTES_AT_MOST,
            "{bytes} bytes against {TABLE_BYTES_AT_MOST}"
        );
    }
}
