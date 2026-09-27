//! `run stellar_fates_low`, `stellar_fates_mid` and `stellar_fates_high`: the fate table of plan
//! 06's P06.T38.c (rulings 89, 90 and 99), which the sim's `stellar::fates::FittedFates` reads for
//! the range briefs of dead stars.
//!
//! # The table
//!
//! Each node is one full track, `hyperion_sim::stellar::fates::FateNode::of`: how a star of one
//! initial mass, metallicity and Reimers η dies (its route), log₁₀ of its death age, and two
//! value columns (a white dwarf's mass and cooling origin, or an iron core's carbon–oxygen and
//! helium cores). The nodes lie on three panels of mass, one task and one file each so that each
//! file stays under the repository's 500 kB, sharing their \[Fe/H\] nodes ([`fe_h_nodes`]), which
//! span the metallicities the formulae see, `z_fit` = 10⁻⁴ to 0.03:
//!
//! - **low** (`tables/stellar_fates_low.rs`), up to a little above [`LOW_SPLIT_MASS`], and **mid**
//!   (`tables/stellar_fates_mid.rs`), from a little below it to a little above [`SPLIT_MASS`],
//!   evenly in log₁₀ m₀ at four η nodes: their white dwarfs' masses move by up to 13% over ±3σ of
//!   η (P06.T38.a's re-survey);
//! - **high** (`tables/stellar_fates_high.rs`), from a little below [`SPLIT_MASS`] to 100 M☉, at one
//!   η node: its fates do not depend on η (the survey's cubic in η reproduces them to 10⁻¹⁵), which
//!   its validation, at η of ±2.4, checks.
//!
//! The grid was chosen on the briefs of real dead rows (P06.T38.c's record in plan 06): the
//! \[Fe/H\] spacing decides the white dwarfs' mass errors, and so how often the brief's guard
//! sends a white dwarf to the exact track; the mass spacing and the η nodes matter less. Values
//! are stored to eight significant digits ([`round`]), far below the table's errors.
//!
//! # The validation
//!
//! Every cell of mass and metallicity is checked at points it was not built from: four, a quarter
//! and three quarters of the way across the cell in mass and in \[Fe/H\], paired four ways, at the
//! middle of each η cell (low and mid) or at η of ±2.4, 0 and 1.2 (high). Each point is one more
//! full track, compared with the reader's interpolant. A column's bound is the manifest's `safety`
//! times the worst relative error of the cell's points (a white dwarf's or a core's mass relative
//! to at least 10⁻³ M☉, an origin to at least 1 Myr), at least `BOUND_FLOOR`, rounded up to two
//! digits. A cell is marked unusable (negative bounds) where a point dies by another route, where
//! no stencil avoids a change of route, or where a bound exceeds `max_bound` (`max_bound_iron` for
//! a cell of iron cores, whose guard reads only the remnant's kind). The sim's range brief then
//! takes the exact track.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StandardNormal;
use hyperion_sim::stellar::fates::{
    BOUND_FLOOR, FateNode, FatePanel, FateRoute, FittedFates, SINGLE_NODE_ETA_LIMIT, VALUE_FLOORS,
};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The mass, M☉, at which the high panel takes over from the low one.
pub const SPLIT_MASS: f64 = 8.0;

/// The mass, M☉, at which the mid panel takes over from the low one.
pub const LOW_SPLIT_MASS: f64 = 2.5;

/// HPT's calibration metallicities (Hurley, Pols and Tout 2000, section 2): the \[Fe/H\] nodes
/// include every one, since ruling 92's giant radii are interpolated between them and their slopes
/// change there.
pub const CALIBRATION_Z: [f64; 7] = [1e-4, 3e-4, 1e-3, 0.004, 0.01, 0.02, 0.03];

/// Which panel a task makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Panel {
    /// Below [`LOW_SPLIT_MASS`], with η nodes.
    Low,
    /// From [`LOW_SPLIT_MASS`] to [`SPLIT_MASS`], with η nodes.
    Mid,
    /// From [`SPLIT_MASS`] up, at one η node.
    High,
}

/// The η of the high panel's validation points, alternately below and above its one node: the
/// sim's `SINGLE_NODE_ETA_LIMIT`, as far as the reader lets that panel answer.
pub const HIGH_VALIDATION_ETA: f64 = SINGLE_NODE_ETA_LIMIT;

/// Nodes per chunk of the parallel build. The chunking cannot change the result.
const CHUNK: u64 = 32;

/// One panel's grid, as the manifest gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelGrid {
    /// log₁₀ of the first node's mass, M☉.
    pub log_mass_start: f64,
    /// The nodes' spacing in log₁₀ m₀.
    pub log_mass_step: f64,
    /// The number of mass nodes.
    pub masses: usize,
    /// The η nodes.
    pub etas: Vec<f64>,
}

/// One panel's grid and its validation's parameters, as its manifest gives them.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSpec {
    /// Which panel.
    pub panel: Panel,
    /// The \[Fe/H\] nodes, evenly spaced over the fitted metallicities.
    pub fe_h: Vec<f64>,
    /// The panel's nodes.
    pub grid: PanelGrid,
    /// The factor on a cell's worst validated error that makes its bound.
    pub safety: f64,
    /// The largest bound a usable cell may have.
    pub max_bound: f64,
    /// The same for a cell of iron cores.
    pub max_bound_iron: f64,
}

/// A panel's fitted contents.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelFit {
    /// Its grid.
    pub grid: PanelGrid,
    /// Its nodes, rounded.
    pub nodes: Vec<Row>,
    /// Its cells' bounds, negative where unusable.
    pub bounds: Vec<Bounds>,
}

/// What the validation found, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Validation {
    /// Cells checked.
    pub cells: usize,
    /// Cells left to the exact track.
    pub unusable: usize,
    /// The median and the 99th percentile of the usable cells' bounds, per column.
    pub bounds: [(f64, f64); 3],
    /// Full tracks built, nodes and validation points together.
    pub tracks: usize,
}

/// `x` to eight significant digits: the table's precision.
///
/// # Panics
///
/// Never: a float's own formatting parses.
#[must_use]
pub fn round(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    format!("{x:.7e}")
        .parse()
        .expect("a formatted float parses")
}

/// A bound to two significant digits, never below it.
///
/// # Panics
///
/// Never: a float's own formatting parses.
#[must_use]
pub fn round_up(x: f64) -> f64 {
    let mut up: f64 = format!("{:.1e}", x * 1.05)
        .parse()
        .expect("a formatted float parses");
    while up < x {
        up *= 1.01;
    }
    up
}

/// The \[Fe/H\] nodes: every calibration metallicity of [`CALIBRATION_Z`] as log₁₀(Z ÷ 0.02),
/// from 10⁻⁴ to 0.03 (the metallicities the formulae see, `Composition::z_fit`), and between each
/// pair the fewest evenly spaced nodes no more than `max_step` apart.
///
/// # Panics
///
/// If `max_step` is not positive.
#[must_use]
pub fn fe_h_nodes(max_step: f64) -> Vec<f64> {
    assert!(max_step > 0.0, "a positive spacing");
    let cal: Vec<f64> = CALIBRATION_Z
        .iter()
        .map(|&z| math::log10(z / 0.02))
        .collect();
    let mut nodes = vec![cal[0]];
    for pair in cal.windows(2) {
        let mut parts = 1_u32;
        while (pair[1] - pair[0]) / f64::from(parts) > max_step {
            parts += 1;
        }
        for k in 1..=parts {
            nodes.push(if k == parts {
                pair[1]
            } else {
                pair[0] + (pair[1] - pair[0]) * f64::from(k) / f64::from(parts)
            });
        }
    }
    nodes
}

/// The panel view of a grid with `nodes` and `bounds`.
fn view<'a>(grid: &'a PanelGrid, nodes: &'a [Row], bounds: &'a [Bounds]) -> FatePanel<'a> {
    FatePanel {
        log_mass_start: grid.log_mass_start,
        log_mass_step: grid.log_mass_step,
        etas: &grid.etas,
        nodes,
        bounds,
    }
}

/// The composition of \[Fe/H\] `fe_h` with no helium excess.
fn composition(fe_h: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The star of mass node `k` of `grid`.
fn mass_of(grid: &PanelGrid, k: f64) -> SolarMasses {
    SolarMasses::new(math::exp10(grid.log_mass_start + grid.log_mass_step * k))
}

fn eta(value: f64) -> StandardNormal {
    StandardNormal::new(value).expect("a finite η")
}

fn index(k: usize) -> f64 {
    f64::from(u32::try_from(k).expect("a node index fits in 32 bits"))
}

/// A node as the table stores it: route code, log₁₀ death age, and the two value columns.
type Row = (f64, f64, f64, f64);

/// A cell's bounds on the death age and the two value columns.
type Bounds = (f64, f64, f64);

/// The nodes of one panel, mass fastest, then η, then \[Fe/H\], built on `threads` threads.
fn build_nodes(
    grid: &PanelGrid,
    fe_h: &[f64],
    threads: NonZeroUsize,
) -> Result<Vec<Row>, RunTaskError> {
    let per_z = grid.masses * grid.etas.len();
    let n = u64::try_from(fe_h.len() * per_z).expect("a table fits in 64 bits");
    let mut nodes = Vec::with_capacity(fe_h.len() * per_z);
    map_reduce_chunks(
        n,
        CHUNK,
        threads,
        |range| {
            range
                .map(|i| {
                    let i = usize::try_from(i).expect("a node index fits in usize");
                    let (z_node, rest) = (i / per_z, i % per_z);
                    let (eta_node, mass_node) = (rest / grid.masses, rest % grid.masses);
                    let node = FateNode::of(
                        mass_of(grid, index(mass_node)),
                        &composition(fe_h[z_node]),
                        eta(grid.etas[eta_node]),
                    )
                    .to_row();
                    (node.0, round(node.1), round(node.2), round(node.3))
                })
                .collect::<Vec<_>>()
        },
        |chunk| nodes.extend(chunk),
    )?;
    Ok(nodes)
}

/// A validation point of one cell: its mass, \[Fe/H\] and η.
#[derive(Debug, Clone, Copy)]
struct Point {
    cell: usize,
    mass: SolarMasses,
    fe_h: f64,
    eta: f64,
}

/// The validation points of every cell of a panel whose η axis is its own (`vary_eta` false: the
/// high panel's, whose points take η of ±[`HIGH_VALIDATION_ETA`]).
fn points(grid: &PanelGrid, fe_h: &[f64], vary_eta: bool) -> Vec<Point> {
    // A quarter and three quarters across the cell in mass and in [Fe/H], paired four ways, so
    // that neither axis is checked only at its middle.
    const SPOTS: [(f64, f64); 4] = [(0.25, 0.25), (0.75, 0.75), (0.25, 0.75), (0.75, 0.25)];
    // The high panel's η at the four spots: both validation limits, its node, and between.
    const HIGH_ETAS: [f64; 4] = [
        HIGH_VALIDATION_ETA,
        -HIGH_VALIDATION_ETA,
        0.0,
        0.5 * HIGH_VALIDATION_ETA,
    ];
    let mut out = Vec::new();
    for z in 0..fe_h.len() - 1 {
        for m in 0..grid.masses - 1 {
            let cell = z * (grid.masses - 1) + m;
            let mut push = |(fm, fz): (f64, f64), e: f64| {
                out.push(Point {
                    cell,
                    mass: mass_of(grid, index(m) + fm),
                    fe_h: fe_h[z] + (fe_h[z + 1] - fe_h[z]) * fz,
                    eta: e,
                });
            };
            if vary_eta {
                for w in grid.etas.windows(2) {
                    for spot in SPOTS {
                        push(spot, w[0].midpoint(w[1]));
                    }
                }
            } else {
                for (spot, e) in SPOTS.into_iter().zip(HIGH_ETAS) {
                    push(spot, e);
                }
            }
        }
    }
    out
}

/// A column's relative error, as the sim's bounds measure it.
fn error(column: usize, exact: f64, fitted: f64) -> f64 {
    match column {
        0 => (fitted - exact).abs() / exact.abs(),
        c => (fitted - exact).abs() / exact.abs().max(VALUE_FLOORS[c - 1]),
    }
}

/// The bounds of a panel's cells from its validation points.
fn validate(
    grid: &PanelGrid,
    fe_h: &[f64],
    nodes: &[Row],
    vary_eta: bool,
    (safety, max_bound, max_bound_iron): (f64, f64, f64),
    threads: NonZeroUsize,
) -> Result<(Vec<Bounds>, usize), RunTaskError> {
    let cells = (fe_h.len() - 1) * (grid.masses - 1);
    let points = points(grid, fe_h, vary_eta);
    let dummy = vec![(0.0, 0.0, 0.0); cells];
    let panel = view(grid, nodes, &dummy);
    // The panel alone, answering for every mass.
    let fates = FittedFates {
        fe_h,
        panels: [panel; 3],
        splits: [f64::INFINITY; 2],
    };
    // Per cell: its worst errors, or `None` where the table must not answer, and whether every
    // point is an iron core.
    let mut worst: Vec<Option<[f64; 3]>> = vec![Some([0.0; 3]); cells];
    let mut iron = vec![true; cells];
    let n = u64::try_from(points.len()).expect("a few points");
    map_reduce_chunks(
        n,
        CHUNK,
        threads,
        |range| {
            range
                .map(|i| {
                    let p = points[usize::try_from(i).expect("an index fits")];
                    let comp = composition(p.fe_h);
                    let exact = FateNode::of(p.mass, &comp, eta(p.eta));
                    let fitted = fates.fate_unbounded(p.mass, &comp, eta(p.eta));
                    let errors = fitted.filter(|f| f.route == exact.route).map(|f| {
                        [
                            error(0, math::exp10(exact.log_death_age), f.death_age.value()),
                            error(1, exact.a, f.a),
                            error(2, exact.b, f.b),
                        ]
                    });
                    (p.cell, errors, exact.route == FateRoute::IronCore)
                })
                .collect::<Vec<_>>()
        },
        |chunk| {
            for (cell, errors, is_iron) in chunk {
                iron[cell] &= is_iron;
                worst[cell] = match (worst[cell], errors) {
                    (Some(w), Some(e)) => Some([w[0].max(e[0]), w[1].max(e[1]), w[2].max(e[2])]),
                    _ => None,
                };
            }
        },
    )?;
    let bounds = worst
        .into_iter()
        .zip(iron)
        .map(|(w, is_iron)| match w {
            Some(w) => {
                let limit = if is_iron { max_bound_iron } else { max_bound };
                let b = w.map(|e| (safety * e).max(BOUND_FLOOR));
                if b.iter().any(|&b| b > limit) {
                    (-1.0, -1.0, -1.0)
                } else {
                    (round_up(b[0]), round_up(b[1]), round_up(b[2]))
                }
            }
            None => (-1.0, -1.0, -1.0),
        })
        .collect();
    Ok((bounds, points.len()))
}

/// The panel `panel` of `manifest`'s `[params]`.
///
/// # Errors
///
/// [`ManifestParamError`] if a parameter is missing or out of range.
pub fn spec(manifest: &Manifest, panel: Panel) -> Result<PanelSpec, ManifestParamError> {
    let count = |key: &str, least: u64| -> Result<usize, ManifestParamError> {
        let v = manifest.u64(key)?;
        if v < least {
            return Err(ManifestParamError::new(key, "a larger count"));
        }
        usize::try_from(v).map_err(|_| ManifestParamError::new(key, "a count that fits"))
    };
    let etas = manifest
        .params()
        .get("etas")
        .and_then(toml::Value::as_array)
        .and_then(|a| {
            a.iter()
                .map(toml::Value::as_float)
                .collect::<Option<Vec<f64>>>()
        })
        .filter(|v| !v.is_empty() && v.windows(2).all(|w| w[0] < w[1]))
        .ok_or_else(|| ManifestParamError::new("etas", "an increasing array of numbers"))?;
    Ok(PanelSpec {
        panel,
        fe_h: fe_h_nodes(manifest.f64("fe_h_max_step")?),
        grid: PanelGrid {
            log_mass_start: manifest.f64("log_mass_start")?,
            log_mass_step: manifest.f64("log_mass_step")?,
            masses: count("masses", 4)?,
            etas,
        },
        safety: manifest.f64("safety")?,
        max_bound: manifest.f64("max_bound")?,
        max_bound_iron: manifest.f64("max_bound_iron")?,
    })
}

/// One panel and its validation, on `threads` threads.
///
/// # Errors
///
/// [`RunTaskError::Threads`] if the thread pool cannot be built.
pub fn fit(
    spec: &PanelSpec,
    threads: NonZeroUsize,
) -> Result<(PanelFit, Validation), RunTaskError> {
    let nodes = build_nodes(&spec.grid, &spec.fe_h, threads)?;
    let (bounds, points) = validate(
        &spec.grid,
        &spec.fe_h,
        &nodes,
        spec.panel != Panel::High,
        (spec.safety, spec.max_bound, spec.max_bound_iron),
        threads,
    )?;
    let mut validation = Validation {
        tracks: nodes.len() + points,
        cells: bounds.len(),
        unusable: bounds.iter().filter(|b| b.0 < 0.0).count(),
        ..Validation::default()
    };
    for (c, slot) in validation.bounds.iter_mut().enumerate() {
        let mut v: Vec<f64> = bounds
            .iter()
            .filter(|b| b.0 >= 0.0)
            .map(|b| [b.0, b.1, b.2][c])
            .collect();
        v.sort_by(f64::total_cmp);
        if !v.is_empty() {
            let at = |per_mille: usize| v[(v.len() - 1) * per_mille / 1000];
            *slot = (at(500), at(990));
        }
    }
    Ok((
        PanelFit {
            grid: spec.grid.clone(),
            nodes,
            bounds,
        },
        validation,
    ))
}

fn scalar(name: &str, doc: &[&str], value: f64) -> TableItem {
    TableItem::Scalar {
        name: name.to_owned(),
        doc: doc.iter().map(|&l| l.to_owned()).collect(),
        value,
    }
}

fn array(name: &str, doc: &[&str], values: Vec<f64>) -> TableItem {
    TableItem::Array {
        name: name.to_owned(),
        doc: doc.iter().map(|&l| l.to_owned()).collect(),
        values,
    }
}

/// A `pub static` array of tuples, one a line, each value in its shortest form: a `static`
/// rather than the emitter's `const`, since Clippy refuses so large a `const` array
/// (`large_const_arrays`), and without digit separators, which would take the file past 500 kB
/// (with `expect_unreadable`, the reason Clippy's `unreadable_literal` is expected).
fn tuples(name: &str, doc: &[&str], rows: &[Vec<f64>], expect_unreadable: bool) -> TableItem {
    let mut out = String::new();
    for line in doc {
        let _ = writeln!(out, "/// {line}");
    }
    let width = rows.first().map_or(0, Vec::len);
    let ty = vec!["f64"; width].join(", ");
    let _ = writeln!(out, "#[rustfmt::skip]");
    if expect_unreadable {
        let _ = writeln!(
            out,
            "#[expect(\n    clippy::unreadable_literal,\n    reason = \"digit separators would take the \
             file past the repository's 500 kB\"\n)]"
        );
    }
    let _ = writeln!(out, "pub static {name}: [({ty}); {}] = [", rows.len());
    for row in rows {
        let values: Vec<String> = row.iter().map(|&v| format!("{v:?}")).collect();
        let _ = writeln!(out, "    ({}),", values.join(", "));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The \[Fe/H\] nodes' item: an array in the emitter's style, which expects Clippy's
/// `approx_constant`, since HPT's calibration metallicity 0.01 is log₁₀(0.01 ÷ 0.02) = −log₁₀ 2.
fn fe_h_item(fe_h: &[f64]) -> TableItem {
    let mut out = String::from(
        "/// The \\[Fe/H\\] nodes, log₁₀(`z_fit` ÷ 0.02), increasing: HPT's calibration \
         metallicities\n/// and evenly spaced nodes between them.\n#[rustfmt::skip]\n#[expect(\n    \
         clippy::approx_constant,\n    reason = \"the calibration metallicity 0.01 is log₁₀(0.01 ÷ \
         0.02) = −log₁₀ 2\"\n)]\n",
    );
    let _ = writeln!(out, "pub const FE_H_NODES: [f64; {}] = [", fe_h.len());
    for &x in fe_h {
        let _ = writeln!(out, "    {},", crate::emit::literal(x));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The items of one panel's file.
fn items(spec: &PanelSpec, fit: &PanelFit) -> Vec<TableItem> {
    let mut items = vec![
        fe_h_item(&spec.fe_h),
        scalar(
            "LOG_MASS_START",
            &["log₁₀ of the panel's first node's initial mass, M☉."],
            fit.grid.log_mass_start,
        ),
        scalar(
            "LOG_MASS_STEP",
            &["The spacing of the panel's nodes in log₁₀ m₀."],
            fit.grid.log_mass_step,
        ),
        array(
            "ETA_NODES",
            &["The panel's Reimers η nodes, standard normal draws, increasing."],
            fit.grid.etas.clone(),
        ),
        tuples(
            "NODES",
            &[
                "The panel's nodes, `(route code, log₁₀ death age ÷ yr, a, b)`, mass fastest, then",
                "η, then \\[Fe/H\\] (`stellar::fates::FateNode`).",
            ],
            &fit.nodes
                .iter()
                .map(|&(r, t, a, b)| vec![r, t, a, b])
                .collect::<Vec<_>>(),
            true,
        ),
        tuples(
            "BOUNDS",
            &[
                "Each cell's validated relative bounds on the death age, `a` and `b`, mass cell",
                "fastest, then \\[Fe/H\\] cell; negative where the table does not answer.",
            ],
            &fit.bounds
                .iter()
                .map(|&(t, a, b)| vec![t, a, b])
                .collect::<Vec<_>>(),
            false,
        ),
    ];
    match spec.panel {
        Panel::Low => items.push(scalar(
            "SPLIT_MASS",
            &["The initial mass, M☉, from which the mid panel answers instead."],
            LOW_SPLIT_MASS,
        )),
        Panel::Mid => items.push(scalar(
            "SPLIT_MASS",
            &["The initial mass, M☉, from which the high panel answers instead."],
            SPLIT_MASS,
        )),
        Panel::High => {}
    }
    items
}

/// The panel's file contents.
#[must_use]
pub fn render(spec: &PanelSpec, fit: &PanelFit) -> RustTable {
    let (which, eta) = match spec.panel {
        Panel::Low => ("low", "below the first split mass, at its η nodes"),
        Panel::Mid => ("mid", "between the split masses, at its η nodes"),
        Panel::High => ("high", "from the second split mass up, at one η node"),
    };
    let notes = format!(
        "\
The {which} panel: stars {eta}. Each node is one full track at the median draws but η
(`stellar::fates::FateNode::of`), to eight significant digits: {m} masses from log₁₀ m₀ =
{s} by {d}, {e} η nodes and {z} \\[Fe/H\\] nodes. A cell's bound is {safety} times its worst
error at its validation points, which a cell over {mb} ({mbi} for iron cores) fails.",
        m = fit.grid.masses,
        s = fit.grid.log_mass_start,
        d = fit.grid.log_mass_step,
        e = fit.grid.etas.len(),
        z = spec.fe_h.len(),
        safety = spec.safety,
        mb = spec.max_bound,
        mbi = spec.max_bound_iron,
    );
    RustTable {
        summary: vec![
            format!(
                "The fates of stars over initial mass, metallicity and Reimers η, {which} panel"
            ),
            "(plan 06, P06.T38.c): how and when each dies and what it leaves, which".to_owned(),
            "`stellar::fates::FittedFates` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: items(spec, fit),
    }
}

/// The initial masses, M☉, \[Fe/H\] and η of the fingerprint's probes: a white dwarf of each
/// kind and an electron capture's neighbourhood (low), and iron cores (high).
pub const FINGERPRINT_STARS: [(f64, f64, f64); 6] = [
    (0.9, -1.0, 0.0),
    (1.5, 0.0, 1.0),
    (4.0, -0.5, -1.0),
    (7.0, 0.0, 0.0),
    (20.0, 0.0, 0.0),
    (60.0, -1.5, 0.0),
];

/// A task of one panel: plan 06's P06.T38.c, fast, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StellarFatesTask {
    /// Which panel it makes.
    pub panel: Panel,
}

/// The low panel's task.
pub static LOW_TASK: StellarFatesTask = StellarFatesTask { panel: Panel::Low };

/// The mid panel's task.
pub static MID_TASK: StellarFatesTask = StellarFatesTask { panel: Panel::Mid };

/// The high panel's task.
pub static HIGH_TASK: StellarFatesTask = StellarFatesTask { panel: Panel::High };

impl FitTask for StellarFatesTask {
    fn name(&self) -> &'static str {
        match self.panel {
            Panel::Low => "stellar_fates_low",
            Panel::Mid => "stellar_fates_mid",
            Panel::High => "stellar_fates_high",
        }
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        match self.panel {
            Panel::Low => "stellar_fates_low.rs",
            Panel::Mid => "stellar_fates_mid.rs",
            Panel::High => "stellar_fates_high.rs",
        }
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        match self.panel {
            Panel::Low | Panel::Mid => &[
                "FE_H_NODES",
                "LOG_MASS_START",
                "LOG_MASS_STEP",
                "ETA_NODES",
                "NODES",
                "BOUNDS",
                "SPLIT_MASS",
            ],
            Panel::High => &[
                "FE_H_NODES",
                "LOG_MASS_START",
                "LOG_MASS_STEP",
                "ETA_NODES",
                "NODES",
                "BOUNDS",
            ],
        }
    }

    /// The death age and first column of the panel's two probe stars (Design note 7).
    fn fingerprint(&self) -> SimFingerprint {
        let stars = match self.panel {
            Panel::Low => &FINGERPRINT_STARS[..2],
            Panel::Mid => &FINGERPRINT_STARS[2..4],
            Panel::High => &FINGERPRINT_STARS[4..],
        };
        let mut probes = Vec::with_capacity(2 * stars.len());
        for &(m, fe_h, e) in stars {
            let node = FateNode::of(SolarMasses::new(m), &composition(fe_h), eta(e));
            probes.push((
                format!("log_death_age({m} M☉, [Fe/H] {fe_h}, η {e})"),
                node.log_death_age,
            ));
            probes.push((format!("a({m} M☉, [Fe/H] {fe_h}, η {e})"), node.a));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let spec = spec(manifest, self.panel)?;
        let (fit, v) = fit(&spec, threads)?;
        let share = |n: usize| 100.0 * index(n) / index(v.cells.max(1));
        Ok(TaskOutput {
            table: render(&spec, &fit),
            source: "the generator's own tracks (plan 06: Hurley, Pols and Tout 2000, MNRAS 315, \
                     543, with ruling 92's giant radii; the remnants of P06.T11, T18 and T20)"
                .to_owned(),
            acceptance: format!(
                "{} full tracks; {} of {} cells ({:.1}%) left to the exact track; usable cells' \
                 bounds, median and 99th percentile: death age {:.1e} and {:.1e}, a {:.1e} and \
                 {:.1e}, b {:.1e} and {:.1e}",
                v.tracks,
                v.unusable,
                v.cells,
                share(v.unusable),
                v.bounds[0].0,
                v.bounds[0].1,
                v.bounds[1].0,
                v.bounds[1].1,
                v.bounds[2].0,
                v.bounds[2].1,
            ),
            provisional: None,
        })
    }
}
