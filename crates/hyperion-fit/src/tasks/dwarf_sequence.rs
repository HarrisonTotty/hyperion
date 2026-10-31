//! `run dwarf_sequence`: the mean dwarf sequence on the sim's own grid, which its dwarf scale and
//! photometry read from version 26 (plan 06, P06.T23.a's licence fix; the sim's
//! `stellar::classify::pm13`).
//!
//! # The source
//!
//! E. Mamajek's "A Modern Mean Dwarf Stellar Color and Effective Temperature Sequence",
//! **version 2022.04.16** (<https://www.pas.rochester.edu/~emamajek/EEM_dwarf_UBVIJHK_colors_Teff.txt>),
//! the maintained extension of Pecaut and Mamajek's (2013, ApJS 208, 9) Table 5 to O3 and to the
//! L, T and Y dwarfs, whose header asks that Pecaut and Mamajek (2013) be cited for it. Of its 32
//! columns the sim reads three quantities against T<sub>eff</sub>: the spectral type, as a code ten
//! per class from O0 at 0 (G2 is 42); the V-band bolometric correction BC<sub>V</sub>, on the IAU
//! 2015 system (the Sun's −0.085); and B − V. The file gives BC<sub>V</sub> down to L5V (1,710 K)
//! and B − V down to M9V (2,380 K).
//!
//! **The file is not committed.** It states no licence, and a column subset of it, every row, is
//! still the raw table (decision-r08-licences): it is a *fetched* dataset (plan 15, Design note
//! 12), of which only `data/mamajek_dwarf_sequence/PROVENANCE.toml` is committed, with the file's
//! SHA-256, and the file is read from the git-ignored cache ([`DEFAULT_DATA_DIR`], or `--data`).
//!
//! # The table
//!
//! Each quantity is the source's own interpolation, linear in log₁₀ T<sub>eff</sub> between its
//! rows, as the sim read the rows before (P06.T23.a), sampled on an even grid in log₁₀
//! T<sub>eff</sub> from the hottest row (O3V, 44,900 K) to the coolest row that gives it: the
//! spectral code to Y4V (250 K), BC<sub>V</sub> to L5V and B − V to M9V. Each grid's step is the
//! nearest to [`TARGET_STEP_DEX`] that fits a whole number of steps between its ends, which lie
//! exactly on the end rows, so the ends keep the rows' values and the sim's continuation beyond
//! O3V and its cut below the cool ends are as before. Between the ends the nodes fall between the
//! source's rows, so the table holds the source's quantities at none of its own points but those
//! two. Read back by linear interpolation on the grid, it differs from the rows' interpolation only
//! near their corners, by at most the [`ACCEPTANCE`] bounds, which the run measures and reports.

use std::num::NonZeroUsize;

use hyperion_sim::math;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The dataset the task reads: Mamajek's file, fetched.
pub const DATASET: &str = "mamajek_dwarf_sequence";

/// The file in the dataset.
pub const FILE: &str = "EEM_dwarf_UBVIJHK_colors_Teff.txt";

/// The version the file's header must name.
pub const SOURCE_VERSION: &str = "Version 2022.04.16";

/// Where `run dwarf_sequence` reads the file unless `--data` says otherwise: the fetched dataset's
/// cache, which git ignores (plan 15, Design note 12).
pub const DEFAULT_DATA_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/cache/mamajek_dwarf_sequence"
);

/// The rows of the sequence, O3V to Y4V.
pub const ROWS: usize = 118;

/// The grids' step in log₁₀ T<sub>eff</sub>, dex, before each is fitted to its ends: our choice,
/// fine enough that the grid's corners stay within [`ACCEPTANCE`] of the rows' (2,000 to 4,500
/// nodes a quantity).
pub const TARGET_STEP_DEX: f64 = 0.0005;

/// The most the grid's interpolation may differ from the rows' anywhere in its range: 0.05 of a
/// subtype in the spectral code, 0.005 mag in BC<sub>V</sub> and in B − V. Our bounds: a tenth of
/// the half-subtype the type is written to, and a tenth of the 0.05 mag of the plan's M<sub>V</sub>
/// tolerance for the Sun.
pub const ACCEPTANCE: [f64; 3] = [0.05, 0.005, 0.005];

/// One of the quantities the table holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Quantity {
    /// The spectral code, ten per class from O0 at 0.
    Code,
    /// BC<sub>V</sub> = M<sub>bol</sub> − M<sub>V</sub>, mag.
    BcV,
    /// B − V, mag.
    ColourBV,
}

impl Quantity {
    /// Every quantity, in the table's order.
    pub const ALL: [Self; 3] = [Self::Code, Self::BcV, Self::ColourBV];

    /// The quantity's value at `row`, or `None` where the source gives none.
    #[must_use]
    pub fn of(self, row: &Row) -> Option<f64> {
        match self {
            Self::Code => Some(row.code),
            Self::BcV => row.bc_v,
            Self::ColourBV => row.colour_b_v,
        }
    }

    /// The prefix of the quantity's items in the table.
    #[must_use]
    pub const fn item(self) -> &'static str {
        match self {
            Self::Code => "CODE",
            Self::BcV => "BC_V",
            Self::ColourBV => "COLOUR_B_V",
        }
    }

    /// The position of the quantity in [`Quantity::ALL`] and [`ACCEPTANCE`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// One row of the source: a class V type and the three quantities the table holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    /// The spectral code, ten per class from O0 at 0.
    pub code: f64,
    /// Effective temperature, K.
    pub teff_k: f64,
    /// BC<sub>V</sub>, mag, where the source gives it.
    pub bc_v: Option<f64>,
    /// B − V, mag, where the source gives it.
    pub colour_b_v: Option<f64>,
}

/// The file could not be read as the sequence: the line, counted from 1 (0 for the file as a
/// whole), and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{FILE} line {line}: {why}")]
pub struct ParseSequenceError {
    /// The line, counted from 1, or 0 for the whole file.
    pub line: usize,
    /// What is wrong.
    pub why: String,
}

/// The spectral class letters in order, O0 at code 0.
const LETTERS: &str = "OBAFGKMLTY";

/// The code of a class V type as the file writes it (`O3V`, `B1.5V`, `Y4V`), or `None`.
#[must_use]
fn code_of(spt: &str) -> Option<f64> {
    let rest = spt.strip_suffix('V')?;
    let mut chars = rest.chars();
    let letter = chars.next()?;
    let class = LETTERS.find(letter)?;
    let subtype: f64 = chars.as_str().parse().ok()?;
    (0.0..10.0)
        .contains(&subtype)
        .then(|| 10.0 * f64::from(u8::try_from(class).expect("ten letters")) + subtype)
}

/// A column's value as the file prints it: `None` for a run of dots, its blank.
fn value_of(field: &str, line: usize) -> Result<Option<f64>, ParseSequenceError> {
    if !field.is_empty() && field.chars().all(|c| c == '.') {
        return Ok(None);
    }
    field.parse().map(Some).map_err(|_| ParseSequenceError {
        line,
        why: format!("`{field}` is neither a number nor a blank"),
    })
}

/// The rows of the file's `text`, hottest first: the lines from its first `#SpT` header to its
/// second, whose first, second, fourth and ninth columns are `SpT`, `Teff`, `BCv` and `B-V`.
///
/// # Errors
///
/// [`ParseSequenceError`] if the header does not name [`SOURCE_VERSION`], a row is malformed, or
/// the rows are not the [`ROWS`] the version holds, falling in temperature and rising in code.
pub fn parse(text: &str) -> Result<Vec<Row>, ParseSequenceError> {
    let whole = |why: String| ParseSequenceError { line: 0, why };
    if !text.lines().take(10).any(|l| l.contains(SOURCE_VERSION)) {
        return Err(whole(format!("the header names no {SOURCE_VERSION}")));
    }
    let mut rows = Vec::with_capacity(ROWS);
    let mut headers = 0;
    for (i, line) in text.lines().enumerate() {
        if line.starts_with("#SpT") {
            headers += 1;
            if headers == 2 {
                break;
            }
            continue;
        }
        if headers == 0 || line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let bad = |why: String| ParseSequenceError { line: i + 1, why };
        if fields.len() < 9 {
            return Err(bad(format!("{} columns, fewer than nine", fields.len())));
        }
        let code = code_of(fields[0]).ok_or_else(|| bad(format!("`{}` is no type", fields[0])))?;
        let teff_k = value_of(fields[1], i + 1)?.ok_or_else(|| bad("no Teff".to_owned()))?;
        rows.push(Row {
            code,
            teff_k,
            bc_v: value_of(fields[3], i + 1)?,
            colour_b_v: value_of(fields[8], i + 1)?,
        });
    }
    if rows.len() != ROWS {
        return Err(whole(format!("{} rows, not {ROWS}", rows.len())));
    }
    if !rows
        .windows(2)
        .all(|w| w[0].teff_k > w[1].teff_k && w[0].code < w[1].code)
    {
        return Err(whole(
            "the rows do not fall in temperature and rise in type".to_owned(),
        ));
    }
    for q in [Quantity::BcV, Quantity::ColourBV] {
        let last = rows.iter().rposition(|r| q.of(r).is_some());
        if last.is_none_or(|k| rows[..=k].iter().any(|r| q.of(r).is_none())) {
            return Err(whole(format!("{q:?} has a gap above its last value")));
        }
    }
    Ok(rows)
}

/// The rows' own interpolation of `quantity` at log₁₀ T<sub>eff</sub> `x`: a row's value at its
/// temperature, linear in log₁₀ T<sub>eff</sub> between two rows that both give it, as the sim's
/// `pm13` read them; `None` outside the rows or where either row lacks it.
#[must_use]
pub fn rows_at(rows: &[Row], quantity: Quantity, x: f64) -> Option<f64> {
    let log_t = |r: &Row| math::log10(r.teff_k);
    // Rows `..hotter` are strictly hotter than `x`.
    let hotter = rows.partition_point(|r| log_t(r) > x);
    let at = rows.get(hotter)?;
    if log_t(at) >= x {
        return quantity.of(at);
    }
    let hot = rows.get(hotter.checked_sub(1)?)?;
    let (a, b) = (quantity.of(hot)?, quantity.of(at)?);
    let fraction = (log_t(hot) - x) / (log_t(hot) - log_t(at));
    Some(a + fraction * (b - a))
}

/// One quantity's grid: its cool end and step in log₁₀ T<sub>eff</sub> and its values, hottest
/// first, the first at the hottest row and the last at the coolest that gives the quantity.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// log₁₀ T<sub>eff</sub> of the last node, the coolest row that gives the quantity.
    pub log_teff_cool: f64,
    /// The step between nodes, dex.
    pub step_dex: f64,
    /// The values at the nodes, from the hottest row down.
    pub values: Vec<f64>,
}

impl Grid {
    /// The grid's interpolation at log₁₀ T<sub>eff</sub> `x`, from `log_teff_hot`, its first node,
    /// as the sim's `pm13` reads it: linear between nodes, `None` outside its ends.
    #[must_use]
    pub fn at(&self, log_teff_hot: f64, x: f64) -> Option<f64> {
        if !(x >= self.log_teff_cool && x <= log_teff_hot) {
            return None;
        }
        let cells = self.values.len() - 1;
        let position = (log_teff_hot - x) / self.step_dex;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a non-negative number of steps, held below the grid's few thousand cells"
        )]
        let i = (position.floor().clamp(0.0, 1.0e5) as usize).min(cells - 1);
        let fraction = position - index(i);
        Some(self.values[i] + fraction * (self.values[i + 1] - self.values[i]))
    }
}

/// A grid index as a float.
#[must_use]
fn index(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a grid's index fits in u32"))
}

/// The fit: the hottest row's temperature, each quantity's grid, and the worst difference of each
/// from the rows' interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct DwarfSequence {
    /// The hottest row's temperature, K (O3V).
    pub teff_hot_k: f64,
    /// log₁₀ of it.
    pub log_teff_hot: f64,
    /// The grids, in [`Quantity::ALL`]'s order.
    pub grids: Vec<Grid>,
    /// Each grid's worst |grid − rows| over its range, and the log₁₀ T<sub>eff</sub> where it lies.
    pub worst: Vec<(f64, f64)>,
}

/// The samples per quantity at which [`fit`] measures the grid against the rows, besides the rows
/// themselves.
pub const SAMPLES: u32 = 200_000;

/// The grids of `rows`, measured against them.
///
/// # Panics
///
/// If `rows` is empty or its first row lacks a quantity: [`parse`]'s rows never do.
#[must_use]
pub fn fit(rows: &[Row]) -> DwarfSequence {
    let teff_hot_k = rows[0].teff_k;
    let log_teff_hot = math::log10(teff_hot_k);
    let mut grids = Vec::with_capacity(Quantity::ALL.len());
    let mut worst = Vec::with_capacity(Quantity::ALL.len());
    for q in Quantity::ALL {
        let coolest = rows
            .iter()
            .rfind(|r| q.of(r).is_some())
            .expect("every quantity is given at the hottest row");
        let log_teff_cool = math::log10(coolest.teff_k);
        let span = log_teff_hot - log_teff_cool;
        let cells = (span / TARGET_STEP_DEX).round();
        let step_dex = span / cells;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole, positive number of cells, a few thousand"
        )]
        let nodes = cells as u32 + 1;
        let values: Vec<f64> = (0..nodes)
            .map(|i| {
                let x = if i + 1 == nodes {
                    log_teff_cool
                } else {
                    log_teff_hot - step_dex * f64::from(i)
                };
                rows_at(rows, q, x).expect("every node lies within the rows that give it")
            })
            .collect();
        let grid = Grid {
            log_teff_cool,
            step_dex,
            values,
        };
        let row_points = rows.iter().map(|r| math::log10(r.teff_k));
        let sweep = (0..=SAMPLES).map(|k| log_teff_cool + span * f64::from(k) / f64::from(SAMPLES));
        let mut q_worst = (0.0_f64, log_teff_hot);
        for x in sweep.chain(row_points) {
            if let (Some(g), Some(r)) = (grid.at(log_teff_hot, x), rows_at(rows, q, x)) {
                let d = (g - r).abs();
                if d > q_worst.0 {
                    q_worst = (d, x);
                }
            }
        }
        grids.push(grid);
        worst.push(q_worst);
    }
    DwarfSequence {
        teff_hot_k,
        log_teff_hot,
        grids,
        worst,
    }
}

/// Whether every grid lies within its [`ACCEPTANCE`] bound of the rows.
#[must_use]
pub fn passes(fit: &DwarfSequence) -> bool {
    fit.worst
        .iter()
        .zip(ACCEPTANCE)
        .all(|(&(worst, _), bound)| worst <= bound)
}

/// What a quantity's items say of it: its name, and its unit.
const fn described(q: Quantity) -> (&'static str, &'static str) {
    match q {
        Quantity::Code => ("spectral code (ten per class from O0 at 0)", ""),
        Quantity::BcV => (
            "bolometric correction BC<sub>V</sub> (IAU 2015 scale)",
            ", mag",
        ),
        Quantity::ColourBV => ("intrinsic Johnson B − V colour", ", mag"),
    }
}

/// The table's contents.
#[must_use]
pub fn render(fit: &DwarfSequence) -> RustTable {
    let notes = format!(
        "\
Mamajek's file is not redistributed here. Its provenance, with its SHA-256, is
`crates/hyperion-fit/data/{DATASET}/PROVENANCE.toml`.

Each quantity is the rows' own interpolation, linear in log₁₀ T<sub>eff</sub> between them,
sampled on an even grid in log₁₀ T<sub>eff</sub> from the hottest row, O3V, down to the coolest
row that gives it (the code to Y4V, BC<sub>V</sub> to L5V, B − V to M9V). Node `i` lies at
`LOG_TEFF_HOT − i × <step>`, the last at the cool end. `stellar::classify::pm13` interpolates
each grid linearly in log₁₀ T<sub>eff</sub> from version 26."
    );
    let scalar =
        |name: String, doc: Vec<String>, value: f64| TableItem::Scalar { name, doc, value };
    let mut items = vec![
        scalar(
            "TEFF_HOT_K".to_owned(),
            vec![
                "The hottest row's effective temperature, K: O3V, every grid's first node."
                    .to_owned(),
            ],
            fit.teff_hot_k,
        ),
        scalar(
            "LOG_TEFF_HOT".to_owned(),
            vec!["log₁₀ of [`TEFF_HOT_K`].".to_owned()],
            fit.log_teff_hot,
        ),
    ];
    for (q, grid) in Quantity::ALL.into_iter().zip(&fit.grids) {
        let (what, unit) = described(q);
        let name = q.item();
        items.push(scalar(
            format!("{name}_LOG_TEFF_COOL"),
            vec![format!("log₁₀ T<sub>eff</sub> (K) of [`{name}`]'s last node, the coolest row that gives it.")],
            grid.log_teff_cool,
        ));
        items.push(scalar(
            format!("{name}_STEP_DEX"),
            vec![format!(
                "The step between [`{name}`]'s nodes in log₁₀ T<sub>eff</sub>, dex."
            )],
            grid.step_dex,
        ));
        // A static, not the emitter's `const` array: a few thousand values (clippy's
        // `large_const_arrays`).
        let n = grid.values.len();
        let mut text = format!(
            "/// The {what}{unit}, at each of the {n} nodes\n/// from [`LOG_TEFF_HOT`] down.\n\
             #[rustfmt::skip]\npub static {name}: [f64; {n}] = [\n"
        );
        for &v in &grid.values {
            text.push_str("    ");
            text.push_str(&literal(v));
            text.push_str(",\n");
        }
        text.push_str("];\n");
        items.push(TableItem::Source(text));
    }
    RustTable {
        summary: vec![
            "The mean dwarf sequence of Pecaut and Mamajek (2013, ApJS 208, 9) in Mamajek's version"
                .to_owned(),
            "2022.04.16 on an even grid in log₁₀ T<sub>eff</sub>: the spectral code, BC<sub>V</sub> and B − V"
                .to_owned(),
            "that `stellar::classify::pm13` reads from version 26 (plan 06, P06.T23.a).".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items,
    }
}

/// The fit's parameters as its manifest records them.
fn parameters() -> toml::Table {
    let text = format!(
        "target_step_dex = {TARGET_STEP_DEX:?}\nacceptance = [{:?}, {:?}, {:?}]\n",
        ACCEPTANCE[0], ACCEPTANCE[1], ACCEPTANCE[2],
    );
    toml::from_str(&text).expect("the parameters' own text is TOML")
}

/// The task: plan 06's P06.T23.a, fast, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DwarfSequenceTask;

impl FitTask for DwarfSequenceTask {
    fn name(&self) -> &'static str {
        "dwarf_sequence"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "dwarf_sequence.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "TEFF_HOT_K",
            "LOG_TEFF_HOT",
            "CODE_LOG_TEFF_COOL",
            "CODE_STEP_DEX",
            "CODE",
            "BC_V_LOG_TEFF_COOL",
            "BC_V_STEP_DEX",
            "BC_V",
            "COLOUR_B_V_LOG_TEFF_COOL",
            "COLOUR_B_V_STEP_DEX",
            "COLOUR_B_V",
        ]
    }

    /// None: the grids read only the file and `math`.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        manifest.expect_params(&parameters())?;
        let input = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
            task: "dwarf_sequence",
            source,
        };
        let dataset = manifest.load_dataset(DATASET)?;
        let bytes = dataset
            .files
            .iter()
            .find(|(name, _)| name == FILE)
            .map(|(_, bytes)| bytes.as_slice())
            .ok_or_else(|| input(format!("the dataset has no {FILE}").into()))?;
        let text = std::str::from_utf8(bytes).map_err(|e| input(Box::new(e)))?;
        let rows = parse(text).map_err(|e| input(Box::new(e)))?;
        let fit = fit(&rows);
        let measured: Vec<String> = Quantity::ALL
            .iter()
            .zip(&fit.grids)
            .zip(&fit.worst)
            .map(|((q, grid), &(worst, x))| {
                let (what, unit) = match q {
                    Quantity::Code => ("the code", " of a subtype"),
                    Quantity::BcV => ("the bolometric correction", " mag"),
                    Quantity::ColourBV => ("B − V", " mag"),
                };
                format!(
                    "{what}'s {} nodes (step {:.7} dex) within {worst:.4}{unit} (at {:.0} K)",
                    grid.values.len(),
                    grid.step_dex,
                    math::exp10(x),
                )
            })
            .collect();
        let acceptance = format!(
            "the grids against the rows' interpolation over {SAMPLES} points each and every row: \
             {}; bounds {} of a subtype and {} and {} mag",
            measured.join("; "),
            ACCEPTANCE[0],
            ACCEPTANCE[1],
            ACCEPTANCE[2],
        );
        if !passes(&fit) {
            return Err(input(
                format!("a grid misses the rows beyond its bound: {acceptance}").into(),
            ));
        }
        Ok(TaskOutput {
            source: "Pecaut and Mamajek (2013, ApJS 208, 9), Table 5, in E. Mamajek's maintained \
                     version 2022.04.16, <https://www.pas.rochester.edu/~emamajek/EEM_dwarf_UBVIJHK_colors_Teff.txt>, \
                     retrieved 2026-09-23"
                .to_owned(),
            acceptance,
            table: render(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;

    #[test]
    fn types_and_blanks_read_as_the_file_writes_them() {
        assert_eq!(code_of("O3V").map(bits), Some(bits(3.0)));
        assert_eq!(code_of("B1.5V").map(bits), Some(bits(11.5)));
        assert_eq!(code_of("G2V").map(bits), Some(bits(42.0)));
        assert_eq!(code_of("Y4V").map(bits), Some(bits(94.0)));
        assert_eq!(code_of("G2III"), None);
        assert_eq!(code_of("Q2V"), None);
        assert_eq!(value_of("...", 1).unwrap(), None);
        assert_eq!(value_of(".....", 1).unwrap(), None);
        assert_eq!(value_of("-0.085", 1).unwrap().map(bits), Some(bits(-0.085)));
        assert_eq!(value_of("19.25:", 7).unwrap_err().line, 7);
    }

    #[test]
    fn a_file_of_another_version_or_length_is_refused() {
        assert_eq!(parse("# Version 2019.03.22\n").unwrap_err().line, 0);
        let short = format!(
            "# {SOURCE_VERSION}\n#SpT Teff\nO3V 44900 4.652 -4.01 5.82 -9.81 13.43 -5.80 -0.330\n#SpT\n"
        );
        assert!(parse(&short).unwrap_err().why.contains("1 rows"));
    }

    /// Three rows linear in log₁₀ T<sub>eff</sub> in each quantity, B − V ending at the second.
    fn line_rows() -> Vec<Row> {
        let row = |t: f64, colour: bool| {
            let x = math::log10(t);
            Row {
                code: 100.0 - 20.0 * x,
                teff_k: t,
                bc_v: Some(3.0 * x - 12.0),
                colour_b_v: colour.then_some(2.0 - 0.5 * x),
            }
        };
        vec![row(40_000.0, true), row(5_000.0, true), row(300.0, false)]
    }

    /// On lines, the grids reproduce the rows' interpolation to rounding, end exactly on the end
    /// rows' values, and read nothing beyond their ends.
    #[test]
    fn the_grids_reproduce_lines_and_keep_their_ends() {
        let rows = line_rows();
        let fit = fit(&rows);
        assert!(passes(&fit), "{:?}", fit.worst);
        for (q, (grid, &(worst, _))) in Quantity::ALL
            .into_iter()
            .zip(fit.grids.iter().zip(&fit.worst))
        {
            assert!(worst < 1e-12, "{q:?}: {worst}");
            assert_eq!(bits(grid.values[0]), bits(q.of(&rows[0]).unwrap()), "{q:?}");
            let cool = rows.iter().rfind(|r| q.of(r).is_some()).unwrap();
            assert_eq!(
                bits(*grid.values.last().unwrap()),
                bits(q.of(cool).unwrap()),
                "{q:?}"
            );
            assert_eq!(grid.at(fit.log_teff_hot, fit.log_teff_hot + 1e-9), None);
            assert_eq!(grid.at(fit.log_teff_hot, grid.log_teff_cool - 1e-9), None);
            let span = fit.log_teff_hot - grid.log_teff_cool;
            let cells = f64::from(u32::try_from(grid.values.len() - 1).unwrap());
            assert!((grid.step_dex * cells - span).abs() < 1e-12);
            assert!((grid.step_dex - TARGET_STEP_DEX).abs() < 0.01 * TARGET_STEP_DEX);
        }
    }

    /// Between two rows the rows' interpolation is linear in log₁₀ T<sub>eff</sub>, at a row it
    /// is the row's value, and it reads nothing beyond the rows or past a quantity's last row.
    #[test]
    fn the_rows_interpolate_in_log_temperature() {
        let rows = line_rows();
        let x = math::log10(10_000.0);
        let code = rows_at(&rows, Quantity::Code, x).unwrap();
        assert!((code - (100.0 - 20.0 * x)).abs() < 1e-12, "{code}");
        let at_row = rows_at(&rows, Quantity::BcV, math::log10(5_000.0)).map(bits);
        assert_eq!(at_row, rows[1].bc_v.map(bits));
        assert_eq!(rows_at(&rows, Quantity::Code, math::log10(50_000.0)), None);
        assert_eq!(
            rows_at(&rows, Quantity::ColourBV, math::log10(1_000.0)),
            None
        );
    }
}
