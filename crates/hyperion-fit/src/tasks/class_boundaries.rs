//! `run class_boundaries`: the boundaries between the luminosity classes that the sim's
//! `stellar::classify::luminosity` reads (plan 06, P06.T23.b), from the calibration of MK types in
//! temperature, bolometric magnitude and surface gravity of Straižys and Kuriliene (1981, Ap&SS 80,
//! 353).
//!
//! # The source
//!
//! Four of the paper's tables, O5 to M6, one row per spectral type, as P06.T23.b transcribed them
//! from the ADS scan (`1981Ap&SS..80..353S`):
//!
//! - Table III, log₁₀ T<sub>eff</sub> for classes V, III and I–II. From O5 to F5 the paper prints
//!   one value for V and III (its section 3: "the temperature scale to be the same for the ZAMS and
//!   luminosity V, IV, and III stars of spectral classes from O to F5"); one scale serves every
//!   supergiant class and II.
//! - Table IV, M<sub>bol</sub> for V, IV, III, II, Ib, Iab and Ia, on the paper's zero point
//!   M<sub>bol</sub>☉ = +4.72 (section 3).
//! - Table VI, log₁₀ of the mass in M☉ for the same seven classes, from evolutionary tracks.
//! - Table VII, log₁₀ g (cm s⁻²) for the same seven, which the paper computes as log g = log 𝔐 +
//!   4 log T<sub>eff</sub> + 0.4 M<sub>bol</sub> − 12.49 from Tables III, IV and VI.
//!
//! **The tables are not committed.** Springer holds the article and states no licence for its
//! data; a table made from the authors' values needs no permission, and only values derived from
//! them are committed, with the citation (decision-r08-licences). The transcription is a *fetched*
//! dataset (plan 15, Design note 12): only `data/straizys_kuriliene_1981/PROVENANCE.toml` is
//! committed, with the file's SHA-256, and the file is read from the git-ignored cache
//! ([`DEFAULT_DATA_DIR`], or `--data`).
//!
//! # The table
//!
//! Six boundaries, each a list of nodes (log₁₀ T<sub>eff</sub>, value) hottest first: at every type
//! that gives both classes, the point halfway between their two points. The gravity boundaries
//! V/IV and IV/III are drawn from Table VII; the luminosity boundaries III/II, II/Ib, Ib/Iab and
//! Iab/Ia from Table IV. A class's temperature is its column of Table III: V's, III's, the I–II
//! column for II and the supergiants, and for IV the midpoint in log T<sub>eff</sub> of V's and
//! III's, which Tables IV, VI and VII imply (the gravities of class IV follow from them so, to
//! within the tables' rounding). Only the boundaries are written, no column of the paper's; the
//! supergiant boundaries' abscissae are the I–II column at the types they span, since the classes
//! either side share it. Every midpoint is `f64::midpoint`'s, so the sim reads the nodes the
//! transcription itself gave it before P06.T23.b's licence fix, bit for bit.
//!
//! Table VI is read only to check the transcription: every gravity of Table VII must follow from
//! Tables III, IV and VI by the paper's formula to within [`GRAVITY_TOLERANCE_DEX`], which a slip
//! in any of the four would break.

use std::num::NonZeroUsize;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The dataset the task reads: the transcription, fetched.
pub const DATASET: &str = "straizys_kuriliene_1981";

/// The transcription's file in the dataset.
pub const FILE: &str = "sk81_tables.txt";

/// Where `run class_boundaries` reads the transcription unless `--data` says otherwise: the
/// fetched dataset's cache, which git ignores (plan 15, Design note 12).
pub const DEFAULT_DATA_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/cache/straizys_kuriliene_1981"
);

/// The spectral types the paper tabulates, O5 to M6.
pub const TYPES: usize = 42;

/// How far, dex, a gravity of Table VII may lie from the paper's formula applied to Tables III, IV
/// and VI, P06.T23.b's bound. The tables' rounding (two decimals in log g and log 𝔐, three in log
/// T<sub>eff</sub>, one in M<sub>bol</sub>) reaches 0.03 dex, and the worst entry lies 0.044 dex
/// off.
pub const GRAVITY_TOLERANCE_DEX: f64 = 0.045;

/// The constant of the paper's formula for Table VII: log g = log 𝔐 + 4 log T<sub>eff</sub> + 0.4
/// M<sub>bol</sub> − 12.49.
const GRAVITY_FORMULA_CONSTANT: f64 = 12.49;

/// A column of Tables IV, VI and VII: the luminosity classes V to Ia, faintest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Column {
    /// Class V.
    Dwarf,
    /// Class IV.
    Subgiant,
    /// Class III.
    Giant,
    /// Class II.
    BrightGiant,
    /// Class Ib.
    SupergiantIb,
    /// Class Iab.
    SupergiantIab,
    /// Class Ia.
    SupergiantIa,
}

impl Column {
    /// Every column, in the tables' order.
    pub const ALL: [Self; 7] = [
        Self::Dwarf,
        Self::Subgiant,
        Self::Giant,
        Self::BrightGiant,
        Self::SupergiantIb,
        Self::SupergiantIab,
        Self::SupergiantIa,
    ];

    /// The column's index in a row's arrays.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// One spectral type of the four tables, as transcribed; `None` where a table has no value.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The type as the paper writes it: `O5`, `K1`, `M6`.
    pub name: String,
    /// Table III: log₁₀ T<sub>eff</sub> (K) for V, III and I–II.
    pub log_teff: [Option<f64>; 3],
    /// Table IV: M<sub>bol</sub>, mag, for V to Ia.
    pub m_bol: [Option<f64>; 7],
    /// Table VI: log₁₀ 𝔐 (M☉) for V to Ia.
    pub log_mass: [Option<f64>; 7],
    /// Table VII: log₁₀ g (cm s⁻²) for V to Ia.
    pub log_g: [Option<f64>; 7],
}

impl Row {
    /// log₁₀ T<sub>eff</sub> of `column`'s stars at this type (see the [module](self)
    /// documentation), or `None` where Table III has none.
    #[must_use]
    pub fn column_log_teff(&self, column: Column) -> Option<f64> {
        let [v, iii, i] = self.log_teff;
        match column {
            Column::Dwarf => v,
            Column::Subgiant => Some(f64::midpoint(v?, iii?)),
            Column::Giant => iii,
            Column::BrightGiant
            | Column::SupergiantIb
            | Column::SupergiantIab
            | Column::SupergiantIa => i,
        }
    }
}

/// The transcription's text could not be read: the line, counted from 1, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{FILE} line {line}: {why}")]
pub struct ParseTranscriptionError {
    /// The line, counted from 1.
    pub line: usize,
    /// What is wrong with it.
    pub why: String,
}

/// The rows of the transcription's `text`, hottest first: every line that is not blank and does
/// not start with `#` is a type, then its 24 values (3 + 7 + 7 + 7), `-` for none.
///
/// # Errors
///
/// [`ParseTranscriptionError`] for a line of another length or a value that is not a number, and for a file
/// of other than [`TYPES`] rows.
pub fn parse(text: &str) -> Result<Vec<Row>, ParseTranscriptionError> {
    let mut rows = Vec::with_capacity(TYPES);
    let mut last = 0;
    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        last = line_no;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let bad = |why: String| ParseTranscriptionError { line: line_no, why };
        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        if fields.len() != 25 {
            return Err(bad(format!(
                "{} fields, not a type and 24 values",
                fields.len()
            )));
        }
        let mut values = [None; 24];
        for (slot, field) in values.iter_mut().zip(&fields[1..]) {
            if *field != "-" {
                *slot = Some(
                    field
                        .parse::<f64>()
                        .map_err(|_| bad(format!("`{field}` is not a number")))?,
                );
            }
        }
        let take = |from: usize| -> [Option<f64>; 7] { std::array::from_fn(|k| values[from + k]) };
        rows.push(Row {
            name: fields[0].to_owned(),
            log_teff: [values[0], values[1], values[2]],
            m_bol: take(3),
            log_mass: take(10),
            log_g: take(17),
        });
    }
    if rows.len() == TYPES {
        Ok(rows)
    } else {
        Err(ParseTranscriptionError {
            line: last,
            why: format!("{} types, not {TYPES}", rows.len()),
        })
    }
}

/// One of the six boundaries: the table it is drawn from, and the classes it separates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boundary {
    /// The item's name in the table.
    pub name: &'static str,
    /// Whether it is drawn from Table VII's gravities (else Table IV's magnitudes).
    pub gravity: bool,
    /// The fainter (or higher-gravity) class.
    pub a: Column,
    /// The brighter (or lower-gravity) class.
    pub b: Column,
}

/// The six boundaries, in the order the sim reads them: by gravity V/IV and IV/III, then by
/// luminosity III/II, II/Ib, Ib/Iab and Iab/Ia.
pub const BOUNDARIES: [Boundary; 6] = [
    Boundary {
        name: "V_IV_LOG_G",
        gravity: true,
        a: Column::Dwarf,
        b: Column::Subgiant,
    },
    Boundary {
        name: "IV_III_LOG_G",
        gravity: true,
        a: Column::Subgiant,
        b: Column::Giant,
    },
    Boundary {
        name: "III_II_M_BOL",
        gravity: false,
        a: Column::Giant,
        b: Column::BrightGiant,
    },
    Boundary {
        name: "II_IB_M_BOL",
        gravity: false,
        a: Column::BrightGiant,
        b: Column::SupergiantIb,
    },
    Boundary {
        name: "IB_IAB_M_BOL",
        gravity: false,
        a: Column::SupergiantIb,
        b: Column::SupergiantIab,
    },
    Boundary {
        name: "IAB_IA_M_BOL",
        gravity: false,
        a: Column::SupergiantIab,
        b: Column::SupergiantIa,
    },
];

impl Boundary {
    /// The boundary's nodes over `rows`, hottest first: at every type that gives both classes a
    /// value and a temperature, the midpoint of their two points (log₁₀ T<sub>eff</sub>, value).
    #[must_use]
    pub fn nodes(&self, rows: &[Row]) -> Vec<(f64, f64)> {
        rows.iter()
            .filter_map(|row| {
                let values = if self.gravity { &row.log_g } else { &row.m_bol };
                let (ya, yb) = (values[self.a.index()]?, values[self.b.index()]?);
                let (xa, xb) = (row.column_log_teff(self.a)?, row.column_log_teff(self.b)?);
                Some((f64::midpoint(xa, xb), f64::midpoint(ya, yb)))
            })
            .collect()
    }
}

/// The transcription's check: the worst distance, dex, of a Table VII gravity from the paper's
/// formula on Tables III, IV and VI, and the number of gravities checked.
#[must_use]
pub fn gravity_check(rows: &[Row]) -> (f64, usize) {
    let mut worst = 0.0_f64;
    let mut checked = 0;
    for row in rows {
        for column in Column::ALL {
            let k = column.index();
            let (Some(g), Some(m_bol), Some(log_m), Some(log_t)) = (
                row.log_g[k],
                row.m_bol[k],
                row.log_mass[k],
                row.column_log_teff(column),
            ) else {
                continue;
            };
            let implied = log_m + 4.0 * log_t + 0.4 * m_bol - GRAVITY_FORMULA_CONSTANT;
            worst = worst.max((g - implied).abs());
            checked += 1;
        }
    }
    (worst, checked)
}

/// The fit: the six boundaries' nodes, and the checks the acceptance line reports.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassBoundaries {
    /// Each boundary's nodes, in [`BOUNDARIES`]' order.
    pub nodes: Vec<Vec<(f64, f64)>>,
    /// The gravity check's worst distance, dex, and the gravities checked ([`gravity_check`]).
    pub gravity: (f64, usize),
}

/// What is wrong with a transcription whose boundaries the sim cannot read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitBoundariesError {
    /// A gravity of Table VII is further from the paper's formula than the tables' rounding
    /// allows: a slip in the transcription.
    #[error(
        "a gravity of Table VII lies {worst} dex from the paper's formula, beyond {GRAVITY_TOLERANCE_DEX}"
    )]
    Gravity {
        /// The worst distance, dex.
        worst: f64,
    },
    /// A boundary's nodes do not fall strictly in temperature, which the sim's interpolation
    /// assumes.
    #[error("the {name} boundary's nodes do not fall strictly in temperature")]
    NotFalling {
        /// The boundary.
        name: &'static str,
    },
}

/// The six boundaries of `rows`, checked.
///
/// # Errors
///
/// [`FitBoundariesError`] if the transcription fails its gravity check or a boundary does not fall strictly
/// in temperature.
pub fn fit(rows: &[Row]) -> Result<ClassBoundaries, FitBoundariesError> {
    let gravity = gravity_check(rows);
    if gravity.0 >= GRAVITY_TOLERANCE_DEX {
        return Err(FitBoundariesError::Gravity { worst: gravity.0 });
    }
    let nodes = BOUNDARIES
        .iter()
        .map(|b| {
            let nodes = b.nodes(rows);
            if nodes.windows(2).all(|w| w[0].0 > w[1].0) {
                Ok(nodes)
            } else {
                Err(FitBoundariesError::NotFalling { name: b.name })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ClassBoundaries { nodes, gravity })
}

/// What a boundary's item says of itself.
fn item_doc(b: &Boundary) -> Vec<String> {
    let class = |c: Column| match c {
        Column::Dwarf => "V",
        Column::Subgiant => "IV",
        Column::Giant => "III",
        Column::BrightGiant => "II",
        Column::SupergiantIb => "Ib",
        Column::SupergiantIab => "Iab",
        Column::SupergiantIa => "Ia",
    };
    let (quantity, table) = if b.gravity {
        ("log₁₀ g (cm s⁻²)", "Table VII")
    } else {
        (
            "M<sub>bol</sub> (mag, M<sub>bol</sub>☉ = +4.72)",
            "Table IV",
        )
    };
    vec![
        format!(
            "The {}/{} boundary: (log₁₀ T<sub>eff</sub>, {quantity}) halfway between the two classes",
            class(b.a),
            class(b.b),
        ),
        format!("at each type of {table} that gives both, hottest first."),
    ]
}

/// The table's contents.
#[must_use]
pub fn render(fit: &ClassBoundaries) -> RustTable {
    let notes = "\
The paper's tables are not redistributed here. Their provenance, with the SHA-256 of the
transcription, is `crates/hyperion-fit/data/straizys_kuriliene_1981/PROVENANCE.toml`.

Each boundary joins, type by type, the points halfway between its two classes in (log₁₀
T<sub>eff</sub>, value), where both are tabulated: a class's temperature is its column of Table III
(I–II for II and the supergiants; for IV the midpoint in log T<sub>eff</sub> of V's and III's).
`classify::luminosity` interpolates it linearly in log T<sub>eff</sub> and holds it beyond its
first and last nodes.";
    RustTable {
        summary: vec![
            "The boundaries between the luminosity classes in gravity (V/IV, IV/III) and in bolometric"
                .to_owned(),
            "magnitude (III/II to Iab/Ia), drawn from Straižys and Kuriliene's (1981, Ap&SS 80, 353)"
                .to_owned(),
            "calibration of MK types (plan 06, P06.T23.b), which `stellar::classify::luminosity` reads."
                .to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: BOUNDARIES
            .iter()
            .zip(&fit.nodes)
            .map(|(b, nodes)| TableItem::Tuples {
                name: b.name.to_owned(),
                doc: item_doc(b),
                rows: nodes.iter().map(|&(x, y)| vec![x, y]).collect(),
            })
            .collect(),
    }
}

/// The task: plan 06's P06.T23.b, fast, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ClassBoundariesTask;

impl FitTask for ClassBoundariesTask {
    fn name(&self) -> &'static str {
        "class_boundaries"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "class_boundaries.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "V_IV_LOG_G",
            "IV_III_LOG_G",
            "III_II_M_BOL",
            "II_IB_M_BOL",
            "IB_IAB_M_BOL",
            "IAB_IA_M_BOL",
        ]
    }

    /// None: the boundaries read only the transcription.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let input = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
            task: "class_boundaries",
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
        let fit = fit(&rows).map_err(|e| input(Box::new(e)))?;
        let counts: Vec<String> = fit.nodes.iter().map(|n| n.len().to_string()).collect();
        Ok(TaskOutput {
            source: "Straižys and Kuriliene (1981), Ap&SS 80, 353-368, DOI 10.1007/BF00652936: \
                     Tables III, IV, VI and VII, O5 to M6, as P06.T23.b transcribed them from the \
                     ADS scan (`1981Ap&SS..80..353S`)"
                .to_owned(),
            acceptance: format!(
                "Table VII's {} gravities follow from Tables III, IV and VI by the paper's formula \
                 within {:.3} dex (at most {GRAVITY_TOLERANCE_DEX}); the six boundaries' {} nodes \
                 each fall strictly in temperature",
                fit.gravity.1,
                fit.gravity.0,
                counts.join(", "),
            ),
            table: render(&fit),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits as f64_bits;

    use super::*;

    /// Two rows of the transcription's form: O5 (no Ia gravity) and M6 (class III alone).
    const SAMPLE: &str = "\
# a comment
O5 4.626 4.626 4.618 -9.8 -10.0 -10.2 -10.3 -10.4 -10.7 -11.0 1.81 1.85 1.89 1.9 1.92 1.99 - 3.9 3.86 3.82 3.76 3.74 3.69 -
M6 - 3.512 - - - -3.6 - - - - - - 0.4 - - - - - - 0.52 - - - -
";

    /// `SAMPLE`'s rows, without the check on their count.
    fn sample_rows() -> Vec<Row> {
        let mut text = String::from(SAMPLE);
        let filler = "\nX0 - - - - - - - - - - - - - - - - - - - - - - - -";
        for _ in 2..TYPES {
            text.push_str(filler);
        }
        parse(&text).unwrap()
    }

    /// A value's bits, for exact comparison.
    fn bits(value: Option<f64>) -> Option<u64> {
        value.map(f64_bits)
    }

    #[test]
    fn a_row_is_parsed_in_the_tables_order() {
        let rows = sample_rows();
        let o5 = &rows[0];
        assert_eq!(o5.name, "O5");
        assert_eq!(
            o5.log_teff.map(bits),
            [4.626, 4.626, 4.618].map(|v| bits(Some(v)))
        );
        assert_eq!(bits(o5.m_bol[6]), bits(Some(-11.0)));
        assert_eq!(bits(o5.log_mass[0]), bits(Some(1.81)));
        assert_eq!(bits(o5.log_g[6]), None);
        assert_eq!(bits(rows[1].log_g[Column::Giant.index()]), bits(Some(0.52)));
        assert_eq!(bits(rows[1].column_log_teff(Column::Subgiant)), None);
    }

    #[test]
    fn a_malformed_transcription_is_refused() {
        assert_eq!(parse("O5 1 2\n").unwrap_err().line, 1);
        let word = SAMPLE.replace("4.618", "x");
        assert!(parse(&word).unwrap_err().why.contains("`x`"));
        assert!(parse(SAMPLE).unwrap_err().why.contains("2 types"));
    }

    /// The nodes' bits, for exact comparison.
    fn node_bits(nodes: &[(f64, f64)]) -> Vec<(u64, u64)> {
        nodes
            .iter()
            .map(|&(x, y)| (f64_bits(x), f64_bits(y)))
            .collect()
    }

    /// A boundary's node is the midpoint of its classes' points; class IV's temperature is the
    /// midpoint of V's and III's; a type without either class gives none.
    #[test]
    fn a_node_lies_halfway_between_its_classes() {
        let rows = sample_rows();
        let v_iv = BOUNDARIES[0].nodes(&rows);
        assert_eq!(
            node_bits(&v_iv),
            node_bits(&[(4.626, f64::midpoint(3.9, 3.86))])
        );
        let iab_ia = BOUNDARIES[5].nodes(&rows);
        assert_eq!(
            node_bits(&iab_ia),
            node_bits(&[(4.618, f64::midpoint(-10.7, -11.0))])
        );
        let iii_ii = BOUNDARIES[2].nodes(&rows);
        assert_eq!(
            node_bits(&iii_ii),
            node_bits(&[(f64::midpoint(4.626, 4.618), f64::midpoint(-10.2, -10.3))])
        );
    }

    /// The paper's formula reproduces the sample's seven gravities, O5's six and M6 III's; a
    /// gravity moved by 0.1 dex fails the fit.
    #[test]
    fn the_gravities_check_the_transcription() {
        let rows = sample_rows();
        let (worst, checked) = gravity_check(&rows);
        assert_eq!(checked, 7);
        assert!(worst < GRAVITY_TOLERANCE_DEX, "{worst}");
        let mut slipped = rows;
        slipped[0].log_g[0] = Some(4.0);
        assert!(matches!(
            fit(&slipped),
            Err(FitBoundariesError::Gravity { .. })
        ));
    }
}
