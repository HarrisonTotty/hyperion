//! `run limb_darkening`: the power-2 limb darkening of stars' discs in Johnson B, V and R
//! (rendering plan R06, R06.T4.a, Design note 16), which the sim's `sky::disc` interpolates.
//!
//! # The law
//!
//! I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Hestroffer 1997, A&A 327, 199; Maxted 2018, A&A 616, A39), whose
//! coefficients the catalogues below give directly as g = c and h = α, so no conversion is needed.
//!
//! # The grids
//!
//! Two rectangular grids in `T_eff` and log g, read bilinearly in log `T_eff` and log g and clamped
//! at their edges, each node holding c and α in B, V and R:
//!
//! - **Stars that are not white dwarfs**, log g 0–6 by 0.5: Claret and Southworth (2023, A&A 674,
//!   A63; spherical PHOENIX-COND, Table 9, truncation method M1) from 2,300 to 3,900 K, and Claret
//!   and Southworth (2022, A&A 664, A128; ATLAS, Table 3, solar metallicity, 2 km/s) from 4,000
//!   to 50,000 K.
//! - **White dwarfs**, log g 6.5–9.5 by 0.5: Claret et al. (2020, A&A 634, A93; table gh), the
//!   LTE DA models from 3,750 to 35,000 K and the non-LTE DA models from 40,000 to 100,000 K.
//!
//! A node outside the gravities a catalogue holds at that temperature takes the nearest it holds
//! (the hot stars hold no low gravities, ATLAS none above 5); a node between two it holds is
//! interpolated linearly in log g (the non-LTE DA models skip log g 8.0). Beyond the grids'
//! temperatures the sim clamps: at 50,000 K for O stars, at 2,300 K below, at 100,000 K for white
//! dwarfs (Design note 16).
//!
//! The catalogues are fetched datasets (plan 15, Design note 12); only their `PROVENANCE.toml` and
//! `urls.txt` are committed.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The task's name.
const NAME: &str = "limb_darkening";

/// The gravities of the grid of stars that are not white dwarfs: log₁₀ g (cgs) 0–6 by 0.5.
pub const NORMAL_LOG_G: [f64; 13] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0,
];

/// The gravities of the white dwarfs' grid: log₁₀ g (cgs) 6.5–9.5 by 0.5.
pub const WHITE_DWARF_LOG_G: [f64; 7] = [6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5];

/// One node: c and α in B, V and R.
pub type Coefficients = [f64; 6];

/// The catalogue a node is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Catalogue {
    /// Claret and Southworth (2023), PHOENIX-COND.
    Phoenix,
    /// Claret and Southworth (2022), ATLAS.
    Atlas,
    /// Claret et al. (2020), LTE DA white dwarfs.
    WhiteDwarfLte,
    /// Claret et al. (2020), non-LTE DA white dwarfs.
    WhiteDwarfNlte,
}

impl Catalogue {
    /// A label for the notes.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Phoenix => "Claret and Southworth 2023 (PHOENIX-COND, M1)",
            Self::Atlas => "Claret and Southworth 2022 (ATLAS)",
            Self::WhiteDwarfLte => "Claret et al. 2020 (DA, LTE)",
            Self::WhiteDwarfNlte => "Claret et al. 2020 (DA, non-LTE)",
        }
    }
}

/// The temperature nodes of the grid of stars that are not white dwarfs, K, with their catalogue.
#[must_use]
pub fn normal_teff_nodes() -> Vec<(u32, Catalogue)> {
    let mut nodes: Vec<(u32, Catalogue)> = (2_300..=3_900)
        .step_by(100)
        .map(|t| (t, Catalogue::Phoenix))
        .collect();
    nodes.extend((4_000..=13_000).step_by(250).map(|t| (t, Catalogue::Atlas)));
    nodes.extend(
        (14_000..=50_000)
            .step_by(1_000)
            .map(|t| (t, Catalogue::Atlas)),
    );
    nodes
}

/// The temperature nodes of the white dwarfs' grid, K, with their catalogue.
#[must_use]
pub fn white_dwarf_teff_nodes() -> Vec<(u32, Catalogue)> {
    let mut nodes: Vec<(u32, Catalogue)> = [
        3_750, 4_000, 4_250, 4_500, 4_750, 5_000, 5_250, 5_500, 5_750, 6_000, 6_250, 6_500, 7_000,
        7_500, 8_000, 8_500, 9_000, 9_500, 10_000, 10_500, 11_000, 11_500, 12_000, 12_500, 13_000,
        13_500, 14_000, 14_500, 15_000, 15_500, 16_000, 16_500, 17_000, 20_000, 25_000, 30_000,
        35_000,
    ]
    .map(|t| (t, Catalogue::WhiteDwarfLte))
    .to_vec();
    nodes.extend(
        [
            40_000, 45_000, 50_000, 55_000, 60_000, 65_000, 70_000, 75_000, 80_000, 85_000, 90_000,
            100_000,
        ]
        .map(|t| (t, Catalogue::WhiteDwarfNlte)),
    );
    nodes
}

/// The coefficients each catalogue holds, by (`T_eff` K, log g in hundredths).
pub type Held = BTreeMap<(u32, i32), Coefficients>;

/// A field of fixed columns, 1-based inclusive bytes as the catalogues' `ReadMe`s give them.
fn field(line: &str, from: usize, to: usize) -> Option<f64> {
    line.get(from - 1..to)?.trim().parse().ok()
}

/// (`T_eff`, log g) as the map's key.
fn key(teff: f64, log_g: f64) -> Option<(u32, i32)> {
    let t = u32::try_from(format!("{teff:.0}").parse::<i64>().ok()?).ok()?;
    let g = i32::try_from(format!("{:.0}", log_g * 100.0).parse::<i64>().ok()?).ok()?;
    Some((t, g))
}

/// Claret and Southworth's (2022) Table 3 at solar metallicity and 2 km/s: B, V and R from bytes
/// 70–76, 79–85, 88–94 (g) and 177–183, 186–192, 195–201 (h).
///
/// # Errors
///
/// A message naming the malformed line.
pub fn parse_atlas(text: &str) -> Result<Held, String> {
    let mut held = Held::new();
    for (n, line) in text.lines().enumerate() {
        let bad = || format!("table3.dat line {} is malformed", n + 1);
        let (Some(log_g), Some(teff), Some(z), Some(vel)) = (
            field(line, 1, 5),
            field(line, 7, 12),
            field(line, 14, 17),
            field(line, 19, 22),
        ) else {
            return Err(bad());
        };
        if z.abs() > 1e-9 || (vel - 2.0).abs() > 1e-9 {
            continue;
        }
        let c = [
            (70, 76),
            (177, 183),
            (79, 85),
            (186, 192),
            (88, 94),
            (195, 201),
        ]
        .map(|(a, b)| field(line, a, b));
        let c = c
            .iter()
            .copied()
            .collect::<Option<Vec<f64>>>()
            .ok_or_else(bad)?;
        held.insert(
            key(teff, log_g).ok_or_else(bad)?,
            [c[0], c[1], c[2], c[3], c[4], c[5]],
        );
    }
    Ok(held)
}

/// Claret and Southworth's (2023) Table 9: B, V and R from bytes 89–100, 102–113, 115–126 (g) and
/// 245–256, 258–269, 271–282 (h).
///
/// # Errors
///
/// A message naming the malformed line.
pub fn parse_phoenix(text: &str) -> Result<Held, String> {
    let mut held = Held::new();
    for (n, line) in text.lines().enumerate() {
        let bad = || format!("table9.dat line {} is malformed", n + 1);
        let (Some(log_g), Some(teff)) = (field(line, 1, 5), field(line, 7, 12)) else {
            return Err(bad());
        };
        let c = [
            (89, 100),
            (245, 256),
            (102, 113),
            (258, 269),
            (115, 126),
            (271, 282),
        ]
        .map(|(a, b)| field(line, a, b));
        let c = c
            .iter()
            .copied()
            .collect::<Option<Vec<f64>>>()
            .ok_or_else(bad)?;
        held.insert(
            key(teff, log_g).ok_or_else(bad)?,
            [c[0], c[1], c[2], c[3], c[4], c[5]],
        );
    }
    Ok(held)
}

/// g and h in B, V and R, as each is read.
type BandPairs = [Option<(f64, f64)>; 3];

/// Claret et al.'s (2020) table gh, the `model` rows (`DA` or `DA-NLTE`) in B, V and R: model in
/// bytes 1–7, log g 9–13, `T_eff` 15–22, g 33–39, h 41–47, filter 78–80.
///
/// # Errors
///
/// A message naming the malformed line, or a model missing a band.
pub fn parse_white_dwarfs(text: &str, model: &str) -> Result<Held, String> {
    let mut bands: BTreeMap<(u32, i32), BandPairs> = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let bad = || format!("tablegh.dat line {} is malformed", n + 1);
        if line.get(..7).map(str::trim) != Some(model) {
            continue;
        }
        let slot = match line.get(77..80).map(str::trim) {
            Some("B") => 0,
            Some("V") => 1,
            Some("R") => 2,
            _ => continue,
        };
        let (Some(log_g), Some(teff), Some(g), Some(h)) = (
            field(line, 9, 13),
            field(line, 15, 22),
            field(line, 33, 39),
            field(line, 41, 47),
        ) else {
            return Err(bad());
        };
        bands.entry(key(teff, log_g).ok_or_else(bad)?).or_default()[slot] = Some((g, h));
    }
    bands
        .into_iter()
        .map(|(k, b)| match b {
            [Some(b), Some(v), Some(r)] => Ok((k, [b.0, b.1, v.0, v.1, r.0, r.1])),
            _ => Err(format!("{model} at {k:?} lacks one of B, V and R")),
        })
        .collect()
}

/// One grid: its nodes and a row per node, temperature-major.
#[derive(Debug, Clone, PartialEq)]
pub struct LimbGrid {
    /// The temperature nodes, K, with their catalogues.
    pub teff: Vec<(u32, Catalogue)>,
    /// The gravity nodes.
    pub log_g: Vec<f64>,
    /// c and α in B, V and R at each node.
    pub rows: Vec<Coefficients>,
    /// How many nodes took the nearest gravity held, and how many were interpolated.
    pub clamped: usize,
    /// How many nodes were interpolated between two gravities held.
    pub interpolated: usize,
}

/// Builds a grid from the catalogues read.
///
/// # Errors
///
/// A message if a catalogue holds nothing at a node's temperature.
pub fn build(
    teff: Vec<(u32, Catalogue)>,
    log_g: &[f64],
    held: &BTreeMap<Catalogue, Held>,
) -> Result<LimbGrid, String> {
    let mut rows = Vec::new();
    let (mut clamped, mut interpolated) = (0, 0);
    for &(t, catalogue) in &teff {
        let at: Vec<(i32, Coefficients)> = held
            .get(&catalogue)
            .into_iter()
            .flat_map(|h| h.range((t, i32::MIN)..=(t, i32::MAX)))
            .map(|(&(_, g), &c)| (g, c))
            .collect();
        let (Some(&(least, low)), Some(&(most, high))) = (at.first(), at.last()) else {
            return Err(format!("{} holds nothing at {t} K", catalogue.label()));
        };
        for &g in log_g {
            let c = centi(g);
            rows.push(if let Some(&(_, row)) = at.iter().find(|(h, _)| *h == c) {
                row
            } else if c < least {
                clamped += 1;
                low
            } else if c > most {
                clamped += 1;
                high
            } else {
                interpolated += 1;
                let above = at.partition_point(|(h, _)| *h < c);
                let (g0, r0) = at[above - 1];
                let (g1, r1) = at[above];
                let f = f64::from(c - g0) / f64::from(g1 - g0);
                std::array::from_fn(|i| r0[i] + (r1[i] - r0[i]) * f)
            });
        }
    }
    Ok(LimbGrid {
        teff,
        log_g: log_g.to_vec(),
        rows,
        clamped,
        interpolated,
    })
}

/// A gravity node in hundredths.
fn centi(log_g: f64) -> i32 {
    let mut c = 0;
    while f64::from(c) < log_g * 100.0 - 0.5 {
        c += 25;
    }
    c
}

/// The fitted table: the two grids.
#[derive(Debug, Clone, PartialEq)]
pub struct LimbTable {
    /// Stars that are not white dwarfs.
    pub normal: LimbGrid,
    /// White dwarfs.
    pub white_dwarf: LimbGrid,
}

/// Reads the catalogues and builds the grids.
///
/// # Errors
///
/// [`RunTaskError`] if a catalogue cannot be read or is malformed.
pub fn fit(manifest: &Manifest) -> Result<LimbTable, RunTaskError> {
    let input = |why: String| RunTaskError::Input {
        task: NAME,
        source: why.into(),
    };
    let text = |dataset: &str, file: &str| -> Result<String, RunTaskError> {
        let set = manifest.load_dataset(dataset)?;
        set.files
            .iter()
            .find(|(n, _)| n == file)
            .and_then(|(_, b)| String::from_utf8(b.clone()).ok())
            .ok_or_else(|| input(format!("`{file}` of `{dataset}` is missing or not text")))
    };
    let mut held = BTreeMap::new();
    held.insert(
        Catalogue::Atlas,
        parse_atlas(&text("claret_southworth_2022", "table3.dat")?).map_err(input)?,
    );
    held.insert(
        Catalogue::Phoenix,
        parse_phoenix(&text("claret_southworth_2023", "table9.dat")?).map_err(input)?,
    );
    let wd = text("claret_2020_white_dwarfs", "tablegh.dat")?;
    held.insert(
        Catalogue::WhiteDwarfLte,
        parse_white_dwarfs(&wd, "DA").map_err(input)?,
    );
    held.insert(
        Catalogue::WhiteDwarfNlte,
        parse_white_dwarfs(&wd, "DA-NLTE").map_err(input)?,
    );
    Ok(LimbTable {
        normal: build(normal_teff_nodes(), &NORMAL_LOG_G, &held).map_err(input)?,
        white_dwarf: build(white_dwarf_teff_nodes(), &WHITE_DWARF_LOG_G, &held).map_err(input)?,
    })
}

/// The disc average of the power-2 law, ∫ I(μ) 2μ dμ ÷ I(1) = 1 − c α ÷ (α + 2).
#[must_use]
pub fn disc_average(c: f64, alpha: f64) -> f64 {
    1.0 - c * alpha / (alpha + 2.0)
}

/// The solar row in V, interpolated bilinearly in log `T_eff` and log g at 5,772 K and log g
/// 4.438, as the sim reads it.
#[must_use]
pub fn solar_v(grid: &LimbGrid) -> (f64, f64) {
    let log_t: Vec<f64> = grid
        .teff
        .iter()
        .map(|&(t, _)| math::log10(f64::from(t)))
        .collect();
    let (ti, tf) = bracket(&log_t, math::log10(5_772.0));
    let (gi, gf) = bracket(&grid.log_g, 4.438);
    let n = grid.log_g.len();
    let mix = |k: usize| {
        let low = grid.rows[ti * n + gi][k] * (1.0 - gf) + grid.rows[ti * n + gi + 1][k] * gf;
        let high =
            grid.rows[(ti + 1) * n + gi][k] * (1.0 - gf) + grid.rows[(ti + 1) * n + gi + 1][k] * gf;
        low * (1.0 - tf) + high * tf
    };
    (mix(2), mix(3))
}

/// The interval of rising `nodes` holding `x`, and the clamped fraction along it.
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 2;
    let i = nodes
        .partition_point(|&v| v <= x)
        .saturating_sub(1)
        .min(last);
    (
        i,
        ((x - nodes[i]) / (nodes[i + 1] - nodes[i])).clamp(0.0, 1.0),
    )
}

/// A grid's rows as a `static` array of `LimbRow`, one a line.
fn rows_source(prefix: &str, what: &str, grid: &LimbGrid) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "/// The {what} grid's rows, temperature-major: row `i × {} + j` is temperature node `i`,\n\
         /// gravity node `j`.\n#[rustfmt::skip]\n#[allow(clippy::approx_constant, reason = \"fitted values that may land near a constant are data\")]\npub static {prefix}: [LimbRow; {}] = [",
        grid.log_g.len(),
        grid.rows.len()
    );
    for r in &grid.rows {
        let _ = writeln!(
            out,
            "    LimbRow {{ c_b: {}, alpha_b: {}, c_v: {}, alpha_v: {}, c_r: {}, alpha_r: {} }},",
            literal(r[0]),
            literal(r[1]),
            literal(r[2]),
            literal(r[3]),
            literal(r[4]),
            literal(r[5]),
        );
    }
    out.push_str("];\n");
    out
}

/// The items of one grid.
fn grid_items(prefix: &str, what: &str, grid: &LimbGrid) -> Vec<TableItem> {
    vec![
        TableItem::Array {
            name: format!("{prefix}_LOG_TEFF"),
            doc: vec![format!(
                "The temperature nodes of the {what} grid: log₁₀ of the effective temperature (K), \
                 rising."
            )],
            values: grid
                .teff
                .iter()
                .map(|&(t, _)| {
                    format!("{:.6e}", math::log10(f64::from(t)))
                        .parse()
                        .expect("a formatted float parses")
                })
                .collect(),
        },
        TableItem::Array {
            name: format!("{prefix}_LOG_G"),
            doc: vec![format!(
                "The gravity nodes of the {what} grid: log₁₀ g (cgs), rising."
            )],
            values: grid.log_g.clone(),
        },
        TableItem::Source(rows_source(prefix, what, grid)),
    ]
}

/// The table's contents.
#[must_use]
pub fn render(table: &LimbTable) -> RustTable {
    let notes = format!(
        "\
Each row holds the power-2 law's c and α, I(μ) ÷ I(1) = 1 − c (1 − μ^α), in Johnson B, V and R
(`c_b`, `alpha_b` and so on), as the catalogues give them (g = c, h = α). Not white dwarfs:
Claret and Southworth 2023 (PHOENIX-COND, Table 9, truncation M1) from 2,300 to 3,900 K, Claret and
Southworth 2022 (ATLAS, Table 3, [M/H] = 0, 2 km/s) from 4,000 to 50,000 K, log g 0–6. White
dwarfs: Claret et al. 2020 (table gh), DA in LTE from 3,750 to 35,000 K and DA in non-LTE from
40,000 to 100,000 K, log g 6.5–9.5. Nodes outside the gravities a catalogue holds at a
temperature take the nearest it holds ({} and {} such nodes); nodes between two it holds are
interpolated linearly in log g ({} and {}). The catalogues are not redistributed; their
provenance is `crates/hyperion-fit/data/<dataset>/PROVENANCE.toml`.",
        table.normal.clamped,
        table.white_dwarf.clamped,
        table.normal.interpolated,
        table.white_dwarf.interpolated,
    );
    let mut items = vec![TableItem::Source(
        "use crate::sky::disc::LimbRow;\n".to_owned(),
    )];
    items.extend(grid_items("NORMAL", "not-white-dwarf", &table.normal));
    items.extend(grid_items("WHITE_DWARF", "white-dwarf", &table.white_dwarf));
    RustTable {
        summary: vec![
            "The power-2 limb darkening of stars in B, V and R by effective temperature and gravity"
                .to_owned(),
            "(rendering plan R06, Design note 16), which `sky::disc` interpolates.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items,
    }
}

/// The task: rendering plan R06's R06.T4.a, fast, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LimbDarkeningTask;

impl FitTask for LimbDarkeningTask {
    fn name(&self) -> &'static str {
        NAME
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "limb_darkening.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "NORMAL_LOG_TEFF",
            "NORMAL_LOG_G",
            "NORMAL",
            "WHITE_DWARF_LOG_TEFF",
            "WHITE_DWARF_LOG_G",
            "WHITE_DWARF",
        ]
    }

    /// The table reads nothing of the sim but `math`.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let table = fit(manifest)?;
        let (c, alpha) = solar_v(&table.normal);
        Ok(TaskOutput {
            source: "Claret and Southworth (2022, A&A 664, A128, Table 3), Claret and Southworth \
                     (2023, A&A 674, A63, Table 9) and Claret et al. (2020, A&A 634, A93, table \
                     gh), as CDS distributes them; retrieved 2026-10-02"
                .to_owned(),
            acceptance: format!(
                "the Sun (5,772 K, log g 4.438) in V: c {c:.4}, α {alpha:.4}, disc average {:.4} \
                 (Design note 16: 0.7837, 0.6893, 0.799)",
                disc_average(c, alpha)
            ),
            table: render(&table),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_columns_are_read() {
        let mut line = " 4.50  5750.  0.0  2.0".to_owned();
        line.push_str(&" ".repeat(400));
        let put = |line: &mut String, from: usize, text: &str| {
            line.replace_range(from - 1..from - 1 + text.len(), text);
        };
        for (at, v) in [
            (70, " 0.8000"),
            (79, " 0.7842"),
            (88, " 0.7000"),
            (177, " 0.6000"),
            (186, " 0.6932"),
            (195, " 0.5000"),
        ] {
            put(&mut line, at, v);
        }
        let held = parse_atlas(&line).unwrap();
        let row = held[&(5_750, 450)];
        for (got, want) in row.iter().zip([0.8, 0.6, 0.7842, 0.6932, 0.7, 0.5]) {
            assert!((got - want).abs() < 1e-12, "{row:?}");
        }
    }

    #[test]
    fn a_gap_is_interpolated_and_an_edge_clamped() {
        let mut held = BTreeMap::new();
        let mut h = Held::new();
        h.insert((40_000, 750), [0.0; 6]);
        h.insert((40_000, 850), [1.0; 6]);
        held.insert(Catalogue::WhiteDwarfNlte, h);
        let grid = build(
            vec![(40_000, Catalogue::WhiteDwarfNlte)],
            &[7.0, 8.0, 9.0],
            &held,
        )
        .unwrap();
        assert_eq!(grid.rows, [[0.0; 6], [0.5; 6], [1.0; 6]]);
        assert_eq!((grid.clamped, grid.interpolated), (2, 1));
    }

    #[test]
    fn the_disc_average_is_the_laws_integral() {
        // ∫₀¹ (1 − c + c μ^α) 2μ dμ by the midpoint rule.
        let (c, alpha) = (0.7837, 0.6893);
        let n = 100_000;
        let sum: f64 = (0..n)
            .map(|i| {
                let mu = (f64::from(i) + 0.5) / f64::from(n);
                (1.0 - c + c * math::powf(mu, alpha)) * 2.0 * mu / f64::from(n)
            })
            .sum();
        assert!((sum - disc_average(c, alpha)).abs() < 1e-6);
        assert!((disc_average(c, alpha) - 0.799).abs() < 0.0005);
    }
}
