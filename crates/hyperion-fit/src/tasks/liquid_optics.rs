//! The liquids' optical constants by bin (galaxy plan 14, P14.T49.h; decision-r11-palette-liquids
//! §1.2 and §1.8), which `hyperion_sim::substance::liquid` carries to each record's temperature.
//!
//! Each of rendering plan R08's committed liquid files that the manifest names
//! ([`crate::materials`]) is reduced to the fifteen bake bins of R08's Design note 5, 25.33 nm wide
//! from 380 to 760 nm, centred where `hyperion_surface::field::LIQUID_BIN_CENTRES_NM` puts them:
//!
//! - **n:** the uniform mean over each bin of the file's real index, linear between its grid
//!   points (the integral of that interpolant, exact);
//! - **a = 4πk ÷ λ,** m⁻¹, Napierian: the same mean of the absorption coefficient, the file's k
//!   turned into a at each grid point and then interpolated linearly in a;
//! - **the temperature** the file states, K, its label to the kelvin.
//!
//! Nothing is fitted: every value is the files' own, reduced by stated arithmetic. R09.T2.e's
//! hand-entered records took their bins the same way (`binmeans.py` in its lane's scratch), so a
//! record the registry resolves at a file's own state reproduces them. Which of a file's members
//! the registry takes, and on what basis, is the sim's per liquid (`substance::liquid`): the task
//! reduces every file whole, and a k that a file holds at zero, unmeasured, becomes an a of zero
//! here that the sim may leave unread.

use std::num::NonZeroUsize;

use hyperion_sim::substance::LIQUID_BIN_EDGES_NM;
use hyperion_surface::field::LIQUID_BINS;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::materials::MaterialFile;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The files the manifest names, in its order (by name), each one of R08's liquid files.
pub const FILES: [&str; 7] = [
    "ammonia-liquid.json",
    "ethane-liquid.json",
    "methane-liquid.json",
    "nitrogen-liquid.json",
    "sulphuric-acid-75.json",
    "sulphuric-acid-84.json",
    "water.json",
];

/// The table's items: for each file in [`FILES`]'s order, its bins of n, its bins of a and its
/// temperature.
pub const ITEMS: [&str; 21] = [
    "AMMONIA_LIQUID_N",
    "AMMONIA_LIQUID_A",
    "AMMONIA_LIQUID_TEMPERATURE_K",
    "ETHANE_LIQUID_N",
    "ETHANE_LIQUID_A",
    "ETHANE_LIQUID_TEMPERATURE_K",
    "METHANE_LIQUID_N",
    "METHANE_LIQUID_A",
    "METHANE_LIQUID_TEMPERATURE_K",
    "NITROGEN_LIQUID_N",
    "NITROGEN_LIQUID_A",
    "NITROGEN_LIQUID_TEMPERATURE_K",
    "SULPHURIC_ACID_75_N",
    "SULPHURIC_ACID_75_A",
    "SULPHURIC_ACID_75_TEMPERATURE_K",
    "SULPHURIC_ACID_84_N",
    "SULPHURIC_ACID_84_A",
    "SULPHURIC_ACID_84_TEMPERATURE_K",
    "WATER_N",
    "WATER_A",
    "WATER_TEMPERATURE_K",
];

/// The bins' edges, nm: the sim's `substance::LIQUID_BIN_EDGES_NM`, 380 + 25.33 k for k = 0 to
/// 15, so that the table's bins and the sim's scattering bins are one set of edges, bit for bit.
#[must_use]
pub fn bin_edges() -> [f64; LIQUID_BINS + 1] {
    LIQUID_BIN_EDGES_NM
}

/// The function linear between the points (`xs`, `ys`) at `x`, inside them.
///
/// `xs` increases and has two or more points, which a [`MaterialFile`] guarantees, with `ys`
/// one value at each; `x` lies inside them, which [`reduce`] checks.
fn interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let i = xs.partition_point(|&g| g <= x).clamp(1, xs.len() - 1);
    let t = (x - xs[i - 1]) / (xs[i] - xs[i - 1]);
    ys[i - 1] + t * (ys[i] - ys[i - 1])
}

/// The uniform mean over [`lo`, `hi`] of the function linear between the points (`xs`, `ys`):
/// its integral by the trapezoid rule between `lo`, each grid point strictly inside and `hi`,
/// which is exact for the piecewise-linear interpolant, over `hi` − `lo`.
///
/// `xs` increases and covers [`lo`, `hi`], as [`MaterialFile::read`] and [`reduce`] check.
fn interval_mean(xs: &[f64], ys: &[f64], lo: f64, hi: f64) -> f64 {
    let at = |x: f64| interpolate(xs, ys, x);
    let mut total = 0.0;
    let mut left = lo;
    let mut left_value = at(lo);
    for (&x, &y) in xs.iter().zip(ys) {
        if x > lo && x < hi {
            total += f64::midpoint(left_value, y) * (x - left);
            left = x;
            left_value = y;
        }
    }
    total += f64::midpoint(left_value, at(hi)) * (hi - left);
    total / (hi - lo)
}

/// One file's reduction: its bins of n, its bins of a, m⁻¹, and its temperature, K.
#[derive(Debug, Clone, PartialEq)]
pub struct Reduced {
    /// The file's name.
    pub file: &'static str,
    /// The uniform mean of n over each bin.
    pub n: [f64; LIQUID_BINS],
    /// The uniform mean of a = 4πk ÷ λ over each bin, m⁻¹.
    pub a: [f64; LIQUID_BINS],
    /// The temperature the file states, K.
    pub temperature_k: f64,
}

/// The absorption coefficient a = 4πk ÷ λ at each of `file`'s grid points, m⁻¹.
#[must_use]
pub fn absorption_per_m(file: &MaterialFile) -> Vec<f64> {
    file.wavelengths_nm()
        .iter()
        .zip(file.k())
        .map(|(&nm, &k)| 4.0 * std::f64::consts::PI * k / (nm * 1e-9))
        .collect()
}

/// Reduces `file`, named `name`, to the bins.
///
/// # Errors
///
/// [`RunTaskError::Input`] if the file is not a liquid's, states no temperature, or its grid does
/// not cover 380–760 nm.
pub fn reduce(name: &'static str, file: &MaterialFile) -> Result<Reduced, RunTaskError> {
    let bad = |why: String| RunTaskError::Input {
        task: "liquid_optics",
        source: format!("{name}: {why}").into(),
    };
    if file.phase() != "liquid" {
        return Err(bad(format!("its phase is {}, not liquid", file.phase())));
    }
    let temperature_k = file
        .temperature_k()
        .ok_or_else(|| bad("it states no temperature".to_owned()))?;
    let edges = bin_edges();
    let grid = file.wavelengths_nm();
    if grid[0] > edges[0] || grid[grid.len() - 1] < edges[LIQUID_BINS] {
        return Err(bad("its grid does not cover 380–760 nm".to_owned()));
    }
    let a_grid = absorption_per_m(file);
    let mut n = [0.0; LIQUID_BINS];
    let mut a = [0.0; LIQUID_BINS];
    for (k, pair) in edges.windows(2).enumerate() {
        n[k] = interval_mean(grid, file.n(), pair[0], pair[1]);
        a[k] = interval_mean(grid, &a_grid, pair[0], pair[1]);
    }
    Ok(Reduced {
        file: name,
        n,
        a,
        temperature_k,
    })
}

/// The table item prefix of file `name`: its stem in capitals, a hyphen as an underscore.
#[must_use]
pub fn item_prefix(name: &str) -> String {
    name.trim_end_matches(".json")
        .replace('-', "_")
        .to_ascii_uppercase()
}

/// Reduces every file of [`FILES`] in `manifest`'s material directory.
///
/// # Errors
///
/// [`RunTaskError::Input`] if the manifest names other files than [`FILES`], or as [`reduce`];
/// [`RunTaskError::Dataset`] if a file cannot be read.
pub fn fit(manifest: &Manifest) -> Result<Vec<Reduced>, RunTaskError> {
    if manifest.materials() != FILES {
        return Err(RunTaskError::Input {
            task: "liquid_optics",
            source: format!(
                "the manifest names the materials {:?}, not the task's {FILES:?}",
                manifest.materials()
            )
            .into(),
        });
    }
    FILES
        .iter()
        .map(|&name| {
            let file = manifest.load_material(name).map_err(|e| {
                RunTaskError::Dataset(crate::data::LoadDatasetError::Material(Box::new(e)))
            })?;
            reduce(name, &file)
        })
        .collect()
}

/// The table's contents for `reduced`, in its order.
#[must_use]
pub fn render(reduced: &[Reduced]) -> RustTable {
    let mut items = Vec::with_capacity(3 * reduced.len());
    for file in reduced {
        let prefix = item_prefix(file.file);
        items.push(TableItem::Array {
            name: format!("{prefix}_N"),
            doc: vec![
                format!(
                    "`{}`'s real index n, the uniform mean over each bake bin,",
                    file.file
                ),
                "380–760 nm, dimensionless.".to_owned(),
            ],
            values: file.n.to_vec(),
        });
        items.push(TableItem::Array {
            name: format!("{prefix}_A"),
            doc: vec![
                format!(
                    "`{}`'s absorption coefficient a = 4πk ÷ λ, the uniform mean over each",
                    file.file
                ),
                "bake bin, 380–760 nm, m⁻¹ (Napierian).".to_owned(),
            ],
            values: file.a.to_vec(),
        });
        items.push(TableItem::Scalar {
            name: format!("{prefix}_TEMPERATURE_K"),
            doc: vec![format!(
                "The temperature `{}` states, K, its label to the kelvin.",
                file.file
            )],
            value: file.temperature_k,
        });
    }
    RustTable {
        summary: vec![
            "The liquids' optical constants by bin (plan 14, P14.T49.h), which".to_owned(),
            "`substance::liquid` carries to each record's temperature.".to_owned(),
        ],
        notes: vec![
            "Each of rendering plan R08's liquid files is reduced to the fifteen bake bins, 25.33 nm"
                .to_owned(),
            "wide from 380 to 760 nm: n and a = 4πk ÷ λ each as its uniform mean over a bin, the file's"
                .to_owned(),
            "grid interpolated linearly (a after k is turned into a at each grid point). Nothing is"
                .to_owned(),
            "fitted: the values are the files', reduced. Which members the registry reads, and on what"
                .to_owned(),
            "basis, is `substance::liquid`'s.".to_owned(),
        ],
        items,
    }
}

/// The task: the liquids' bins, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LiquidOpticsTask;

impl FitTask for LiquidOpticsTask {
    fn name(&self) -> &'static str {
        "liquid_optics"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "liquid_optics.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The bins' edges, the one sim value the reduction reads; the material files are the inputs
    /// hash's.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(
            bin_edges()
                .iter()
                .enumerate()
                .map(|(k, edge)| (format!("liquid bin edge {k} nm"), *edge))
                .collect(),
        )
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let reduced = fit(manifest)?;
        let water_file = manifest.load_material("water.json").map_err(|e| {
            RunTaskError::Dataset(crate::data::LoadDatasetError::Material(Box::new(e)))
        })?;
        let water_a = absorption_per_m(&water_file);
        let at = |nm: f64| -> String {
            let i = water_file
                .wavelengths_nm()
                .iter()
                .position(|&w| (w - nm).abs() < 1e-9);
            i.map_or_else(
                || "off the grid".to_owned(),
                |i| format!("{:.5}", water_a[i]),
            )
        };
        let acceptance = format!(
            "{} liquid files, each to {LIQUID_BINS} bins over 380–760 nm; water.json's a on its grid: \
             {} m⁻¹ at 440 nm (Mason, Cone and Fry 2016's 0.00522), {} at 550 nm and {} at 700 nm \
             (Pope and Fry 1997's 0.0565 and 0.624)",
            reduced.len(),
            at(440.0),
            at(550.0),
            at(700.0),
        );
        Ok(TaskOutput {
            table: render(&reduced),
            source: "rendering plan R08's committed liquid material files \
                     (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), each citing its \
                     own primaries: IAPWS R9-97, Mason, Cone and Fry 2016, Pope and Fry 1997 and \
                     Kou, Labrie and Chýlek 1993 (water); Martonchik and Orton 1994 (methane); \
                     Augustin 1915 (nitrogen); Hagermann et al. 2005 with Loria 1909 (ethane); \
                     Wong and Anderson 1972 with Cuthbertson 1914 (ammonia); Palmer and Williams \
                     1975 (sulphuric acid)"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::materials::MATERIALS_FROM_CRATE;

    /// The task's own manifest, read from the repository.
    fn manifest() -> Manifest {
        Manifest::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/liquid_optics.toml"))
            .unwrap()
    }

    /// Each bin's mean is the interpolant's, within 10⁻¹² relative of an independent sum: Simpson's
    /// rule on each piece between the bin's edges and the grid points inside it, exact on a linear
    /// piece; and the edges are R08's, 380 to 760 nm, centred on the surface crate's bin centres.
    #[test]
    fn liquid_optics_each_bin_is_its_uniform_mean() {
        let edges = bin_edges();
        assert!((edges[0] - 380.0).abs() < 1e-12 && (edges[LIQUID_BINS] - 760.0).abs() < 1e-12);
        for (k, pair) in edges.windows(2).enumerate() {
            let centre = f64::midpoint(pair[0], pair[1]);
            let surface = hyperion_surface::field::LIQUID_BIN_CENTRES_NM[k];
            assert!(
                (centre - surface).abs() < 1e-9,
                "bin {k}: {centre} against {surface}"
            );
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        for name in FILES {
            let file = MaterialFile::read(&dir, name).unwrap();
            let reduced = reduce(name, &file).unwrap();
            let a_grid = absorption_per_m(&file);
            let grid = file.wavelengths_nm();
            for (k, pair) in edges.windows(2).enumerate() {
                let mut knots = vec![pair[0]];
                knots.extend(grid.iter().copied().filter(|&x| x > pair[0] && x < pair[1]));
                knots.push(pair[1]);
                for (ys, mean) in [(file.n(), reduced.n[k]), (&a_grid[..], reduced.a[k])] {
                    let value = |x: f64| interpolate(grid, ys, x);
                    let simpson: f64 = knots
                        .windows(2)
                        .map(|w| {
                            (w[1] - w[0]) / 6.0
                                * (value(w[0])
                                    + 4.0 * value(f64::midpoint(w[0], w[1]))
                                    + value(w[1]))
                        })
                        .sum::<f64>()
                        / (pair[1] - pair[0]);
                    let scale = mean.abs().max(1e-30);
                    assert!(
                        (simpson - mean).abs() <= 1e-12 * scale,
                        "{name} bin {k}: {mean} against Simpson's {simpson}"
                    );
                }
            }
        }
    }

    /// Water's absorption on the file's grid is its sources': Mason, Cone and Fry 2016's 0.00522
    /// m⁻¹ at 440 nm (Table 2), and Pope and Fry 1997's 0.0565 at 550 nm and 0.624 at 700 nm, each
    /// within 1% (R08.T5.h moved 440 nm off Pope and Fry's 0.00635, decision-r08-t5g §2).
    #[test]
    fn liquid_optics_water_s_absorption_is_its_sources() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        let water = MaterialFile::read(&dir, "water.json").unwrap();
        let a = absorption_per_m(&water);
        for (nm, source) in [(440.0, 0.005_22), (550.0, 0.056_5), (700.0, 0.624)] {
            let i = water
                .wavelengths_nm()
                .iter()
                .position(|&w| (w - nm).abs() < 1e-9)
                .unwrap();
            assert!(
                (a[i] / source - 1.0).abs() < 0.01,
                "{nm} nm: {} m⁻¹ against {source}",
                a[i]
            );
        }
    }

    /// The files' provenances are the ones `substance::liquid`'s bases were set against
    /// (decision-r08-t5g §3: water, methane and the acids measured; nitrogen, ethane and ammonia
    /// derived). A file whose provenance changes fails here, so that its liquid's basis is
    /// revisited.
    #[test]
    fn liquid_optics_the_files_provenances_are_the_bases_assumed() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        let expected = [
            "derived", "derived", "measured", "derived", "measured", "measured", "measured",
        ];
        for (name, provenance) in FILES.iter().zip(expected) {
            let file = MaterialFile::read(&dir, name).unwrap();
            assert_eq!(file.provenance(), provenance, "{name}");
        }
    }

    /// A file that is not a liquid's, that states no temperature or whose grid does not cover
    /// 380–760 nm is refused, and so is a manifest naming other files.
    #[test]
    fn liquid_optics_refuses_what_it_cannot_reduce() {
        let dir = tempfile::tempdir().unwrap();
        let body = |phase: &str, temperature: &str, grid: &str| {
            format!(
                r#"{{"key":"X","phase":"{phase}","shape":"sphere","provenance":"measured",{temperature}"wavelengthsNm":{grid},"n":[1.3,1.3],"k":[0,0]}}"#
            )
        };
        let files = [
            (
                "solid.json",
                body("solid", r#""temperatureK":90,"#, "[380,760]"),
            ),
            ("untimed.json", body("liquid", "", "[380,760]")),
            (
                "short.json",
                body("liquid", r#""temperatureK":90,"#, "[400,760]"),
            ),
        ];
        for (name, text) in &files {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        for (name, _) in &files {
            let file = MaterialFile::read(dir.path(), name).unwrap();
            assert!(
                matches!(reduce("test.json", &file), Err(RunTaskError::Input { .. })),
                "{name}"
            );
        }
        let other = Manifest::from_bytes(
            Path::new("crates/hyperion-fit/manifests/liquid_optics.toml"),
            b"task = \"liquid_optics\"\nmaterials = [\"water.json\"]\n".to_vec(),
        )
        .unwrap();
        assert!(matches!(fit(&other), Err(RunTaskError::Input { .. })));
    }

    /// The manifest names the task's files, and the items follow them.
    #[test]
    fn liquid_optics_the_items_follow_the_files() {
        assert_eq!(manifest().materials(), FILES);
        let names: Vec<String> = FILES
            .iter()
            .flat_map(|f| {
                let p = item_prefix(f);
                [
                    format!("{p}_N"),
                    format!("{p}_A"),
                    format!("{p}_TEMPERATURE_K"),
                ]
            })
            .collect();
        assert_eq!(names, ITEMS);
    }
}
