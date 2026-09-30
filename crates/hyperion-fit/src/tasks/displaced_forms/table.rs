//! The displaced form table's task (plan 15, P15.T6.c–f): the orbits' histograms in, the fits,
//! `tables::displaced_forms` out.
//!
//! [`DisplacedFormsTask`] reads its manifest, `manifests/displaced_forms.toml`, whose one dataset
//! is an orbit run's histograms (`displaced_orbits.txt`, as `hyperion-fit orbits` writes it,
//! verified against the dataset's `PROVENANCE.toml`) and whose `orbit_manifest` names the orbit
//! manifest that made them: the histograms' header must carry that manifest's SHA-256. It then
//! fits the disc-born forms ([`fit_disc`](super::fit_disc)), the old sources' and the own-form
//! shares ([`fit_old`](super::fit_old)), reads the kinematics and in-cube shares
//! ([`kinematics`](super::kinematics)), fits the hypervelocity row
//! ([`hypervelocity`](super::hypervelocity)), and renders the table.
//!
//! Which histograms are fitted is the manifest's `histograms`: `smoke` (P15.T6.b's smoke run,
//! seconds), `one_percent` (the production counts over 100) or `production`. Only `production`
//! makes a table that is not provisional.
//!
//! # Rerunning on the production histograms
//!
//! When `hyperion-fit orbits --threads 3` (manifest `displaced_orbits.toml`) has written
//! `crates/hyperion-fit/data/cache/displaced/displaced_orbits.txt` (in the tree it ran in; the
//! production run is in `target/lanes/brainstorm`) and printed its SHA-256:
//!
//! 1. copy the file into this checkout's `crates/hyperion-fit/data/cache/displaced/` if it was
//!    made elsewhere (that tree still calls the manifest `displaced_forms.toml`, same bytes), and
//!    check that its second line carries `displaced_orbits.toml`'s SHA-256;
//! 2. add `crates/hyperion-fit/data/displaced/PROVENANCE.toml`, as `displaced_smoke/`'s but naming the
//!    production manifest, with the file's SHA-256;
//! 3. in `manifests/displaced_forms.toml` set `datasets = ["displaced"]`, `histograms =
//!    "production"` and `orbit_manifest = "displaced_orbits.toml"`, and drop the provisional
//!    paragraph of its header comment;
//! 4. run `just fit displaced_forms` (`--since` the current `GENERATOR_VERSION`), which rewrites
//!    the table, its `tables.lock` entry and its `tables::MANIFEST` row, no longer provisional;
//!    it is a few minutes on three threads at the committed settings;
//! 5. rerun the consumers' tests that read the table (plan 08's `galaxy_class_table`,
//!    `galaxy_displaced_forms`, `galaxy_displaced_bounds`), re-bless the class table's golden,
//!    and read the header's acceptance figures against P15.T6.c–f. The table is read by nothing
//!    generated until P08.T12's bump, so swapping it moves no other golden.

use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::path::Path;

use hyperion_sim::galaxy::displaced::class_table::{
    MILKY_WAY_IA_RATE_PER_YEAR, SURVIVOR_POPULATIONS,
};
use hyperion_sim::galaxy::displaced::forms::{
    COROTATION_RATIO_NODES, ClassKinematics, CoredPowerLawParams, ESCAPE_RATIO_NODES,
    FlaredLayerParams,
};
use hyperion_sim::galaxy::displaced::{AGE_BINS, AGE_EDGES, GalaxyScales, SPEED_BINS, SPEED_EDGES};
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::math;
use hyperion_sim::units::LightYears;

use super::births::ThinHistory;
use super::cells::{BarCells, RzCells};
use super::fit_disc::{DiscFits, DisciplineSettings, fit_disc};
use super::fit_old::{
    Elongation, OldFit, OldSettings, elongation, fit_mixtures, fit_spheroids, own_form,
};
use super::histogram::ClassHistogram;
use super::hypervelocity::{HypervelocityFit, HypervelocitySettings, fit_hypervelocity};
use super::kinematics::{in_cube, kinematics};
use super::{OrbitClass, Source, TASK, classes, sha256_hex};
use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision.
pub const REVISION: u32 = 0;

/// The histograms' file within their dataset.
pub const ORBITS_FILE: &str = "displaced_orbits.txt";

/// The radius at which the neutron stars' half-density height is read, ly: the Sun-like point's.
pub const SUNLIKE_RADIUS_LY: f64 = 26_000.0;

/// The items the table declares, in order.
pub const ITEMS: [&str; 12] = [
    "SPEED_EDGES",
    "AGE_EDGES",
    "ESCAPE_RATIO_NODES",
    "COROTATION_RATIO_NODES",
    "DISC_BORN",
    "THICK_DISC_BORN",
    "HALO_BORN",
    "BULGE_BORN",
    "BAR_BORN",
    "NUCLEAR_DISC_BORN",
    "HYPERVELOCITY",
    "HYPERVELOCITY_MEAN_EXIT_LY",
];

/// The table's task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DisplacedFormsTask;

/// The histograms could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadOrbitsError {
    /// The file is not an orbit run's histograms.
    #[error("the histograms are malformed: {0}")]
    Malformed(String),
    /// The histograms were made from another orbit manifest than the one named.
    #[error(
        "the histograms carry manifest-sha256 {found}, not the named orbit manifest's {expected}"
    )]
    OtherManifest {
        /// The hash the file carries.
        found: String,
        /// The named manifest's.
        expected: String,
    },
    /// The dataset has no histogram file.
    #[error("the dataset holds no `{ORBITS_FILE}`")]
    Missing,
}

/// Reads an orbit run's histograms as [`render`](super::render) wrote them: the manifest hash in
/// their header and one record per class of [`classes`], in order.
///
/// # Errors
///
/// [`ReadOrbitsError::Malformed`] if a line is missing or out of order.
pub fn read_orbits(text: &str) -> Result<(String, Vec<ClassHistogram>), ReadOrbitsError> {
    let bad = |why: &str| ReadOrbitsError::Malformed(why.to_owned());
    let mut lines = text.lines();
    if !lines
        .next()
        .is_some_and(|l| l.starts_with(&format!("# hyperion-fit {TASK} orbit histograms")))
    {
        return Err(bad("no title line"));
    }
    let hash = lines
        .next()
        .and_then(|l| l.strip_prefix("# manifest-sha256 "))
        .ok_or_else(|| bad("no manifest-sha256 line"))?
        .to_owned();
    let mut records = Vec::with_capacity(99);
    for class in classes() {
        let (name, record) = ClassHistogram::read(&mut lines, class.source.is_barred())
            .map_err(|e| ReadOrbitsError::Malformed(e.to_string()))?;
        if name != class.name() {
            return Err(bad(&format!("class {name} where {} belongs", class.name())));
        }
        records.push(record);
    }
    if lines.next().is_some() {
        return Err(bad("lines after the last class"));
    }
    Ok((hash, records))
}

/// Which histograms a manifest fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Histograms {
    /// P15.T6.b's smoke run: a few orbits a class.
    Smoke,
    /// The production counts over 100.
    OnePercent,
    /// The production run.
    Production,
}

impl Histograms {
    fn parse(text: &str) -> Result<Self, ManifestParamError> {
        match text {
            "smoke" => Ok(Self::Smoke),
            "one_percent" => Ok(Self::OnePercent),
            "production" => Ok(Self::Production),
            _ => Err(ManifestParamError::new(
                "histograms",
                "`smoke`, `one_percent` or `production`",
            )),
        }
    }

    /// The header's `@provisional by` for a table fitted to them, `None` for production.
    #[must_use]
    pub const fn provisional(self) -> Option<&'static str> {
        match self {
            Self::Smoke => Some("P15.T6.c–f on P15.T6.b's smoke histograms"),
            Self::OnePercent => Some("P15.T6.c–f on the 1% orbit run"),
            Self::Production => None,
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::Smoke => "P15.T6.b's smoke histograms",
            Self::OnePercent => "the 1% orbit run's histograms",
            Self::Production => "the production orbit run's histograms",
        }
    }
}

/// The fit's parameters, from its manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct FitParams {
    /// Which histograms.
    pub histograms: Histograms,
    /// The orbit manifest that made them, beside this manifest.
    pub orbit_manifest: String,
    /// The disc-born fits' settings.
    pub disc: DisciplineSettings,
    /// The old fits' settings.
    pub old: OldSettings,
    /// The hypervelocity row's.
    pub hypervelocity: HypervelocitySettings,
    /// Each speed bin's share of neutron stars at the fixture, for the weighted means.
    pub speed_weights: [f64; SPEED_BINS],
}

/// An array of `N` floats under `key`.
fn floats<const N: usize>(manifest: &Manifest, key: &str) -> Result<[f64; N], ManifestParamError> {
    let wrong = || ManifestParamError::new(key, &format!("an array of {N} numbers"));
    let values = manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(wrong)?;
    let mut out = [0.0; N];
    if values.len() != N {
        return Err(wrong());
    }
    for (o, v) in out.iter_mut().zip(values) {
        #[expect(clippy::cast_precision_loss, reason = "small integers in a manifest")]
        let x = v
            .as_float()
            .or_else(|| v.as_integer().map(|i| i as f64))
            .ok_or_else(wrong)?;
        *o = x;
    }
    Ok(out)
}

/// A list of floats under `key`.
fn float_list(manifest: &Manifest, key: &str) -> Result<Vec<f64>, ManifestParamError> {
    let wrong = || ManifestParamError::new(key, "an array of numbers");
    manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(wrong)?
        .iter()
        .map(|v| v.as_float().ok_or_else(wrong))
        .collect()
}

fn count(manifest: &Manifest, key: &str) -> Result<usize, ManifestParamError> {
    usize::try_from(manifest.u64(key)?).map_err(|_| ManifestParamError::new(key, "a count"))
}

/// A Gauss–Legendre order under `key`, one of `allowed`.
fn order(manifest: &Manifest, key: &str, allowed: &[usize]) -> Result<usize, ManifestParamError> {
    let n = count(manifest, key)?;
    if allowed.contains(&n) {
        Ok(n)
    } else {
        Err(ManifestParamError::new(
            key,
            &format!("a Gauss–Legendre order among {allowed:?}"),
        ))
    }
}

impl FitParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if one is missing or of the wrong type.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let tolerance = manifest.f64("tolerance")?;
        Ok(Self {
            histograms: Histograms::parse(manifest.str("histograms")?)?,
            orbit_manifest: manifest.str("orbit_manifest")?.to_owned(),
            disc: DisciplineSettings {
                order: order(manifest, "cell_nodes", &[2, 3, 4, 8])?,
                evaluations: count(manifest, "evaluations")?,
                refit_evaluations: count(manifest, "refit_evaluations")?,
                tolerance,
                drop_weight: manifest.f64("drop_weight")?,
                lambdas: float_list(manifest, "lambdas")?,
                rise_limit: manifest.f64("rise_limit")?,
                sweeps: count(manifest, "sweeps")?,
            },
            old: OldSettings {
                evaluations: count(manifest, "evaluations")?,
                tolerance,
            },
            hypervelocity: HypervelocitySettings {
                panel_nodes: order(manifest, "hypervelocity_panel_nodes", &[4, 8])?,
                directions: count(manifest, "hypervelocity_directions")?,
                evaluations: count(manifest, "evaluations")?,
                tolerance,
            },
            speed_weights: floats(manifest, "speed_weights")?,
        })
    }
}

/// Everything the fits produced, for the table and its header.
#[derive(Debug, Clone, PartialEq)]
pub struct FormFits {
    /// Which histograms.
    pub histograms: Histograms,
    /// The orbits they hold.
    pub orbits: u64,
    /// The disc-born fits.
    pub disc: DiscFits,
    /// The old sources' fits, in the order of [`Source::OLD`].
    pub old: [Vec<OldFit>; 5],
    /// Every class's record, in the order of [`classes`].
    pub records: Vec<ClassHistogram>,
    /// The bar's elongation by speed bin against its control.
    pub bar_elongation: Vec<Option<Elongation>>,
    /// The hypervelocity row.
    pub hypervelocity: HypervelocityFit,
    /// The galaxy's scales.
    pub scales: GalaxyScales,
    /// The neutron stars' half-density height at the Sun-like radius, ly.
    pub half_height_ly: f64,
}

/// The fixture's scales, fields and parameters.
fn fixture() -> (GalaxyParams, GalaxyScales, Fields) {
    let params = GalaxyParams::milky_way_like();
    let model = MassModel::new(&params);
    let scales = GalaxyScales::new(&params, &PotentialTables::in_plane(&model));
    let fields = Fields::new(&params, &model);
    (params, scales, fields)
}

/// The thin disc's share of deaths in each age bin: its history's weight on the bin's span.
fn age_weights(fields: &Fields, scales: &GalaxyScales) -> [f64; AGE_BINS] {
    let history = ThinHistory::new(fields);
    let unit = scales.tau_unit().value();
    let oldest = history.oldest(fields);
    let mut out = [0.0; AGE_BINS];
    for (a, w) in out.iter_mut().enumerate() {
        let lo = if a == 0 { 0.0 } else { AGE_EDGES[a - 1] * unit };
        let hi = AGE_EDGES.get(a).map_or(oldest, |e| e * unit);
        *w = history.weight(fields, lo, hi);
    }
    let total: f64 = out.iter().sum();
    if total > 0.0 {
        for w in &mut out {
            *w /= total;
        }
    }
    out
}

/// The flared layer's density at `(r, z)`, `R_d`, before normalisation.
fn layer_at(l: &FlaredLayerParams, r: f64, z: f64) -> f64 {
    let h = l.h_0 * math::exp(r / l.r_flare);
    math::exp(-r / l.h_r) * math::exp(-math::powf(z.abs() / h, l.beta)) / h
}

/// The cored power law's density at `(r, z)`, `R_d`, before normalisation.
fn spheroid_at(s: &CoredPowerLawParams, r: f64, z: f64) -> f64 {
    math::exp(-0.5 * s.gamma * math::ln_1p((r * r + z * z / (s.q * s.q)) / (s.a * s.a)))
}

/// The neutron stars' half-density height at `radius` `R_d`: where the class-weighted sum of the
/// disc-born forms, each normalised over the cube, falls to half its midplane value, `R_d`.
fn half_height(cells: &RzCells, disc: &DiscFits, radius: f64) -> f64 {
    let mut scratch = vec![0.0; super::histogram::R_CELLS * super::histogram::Z_CELLS];
    let mut terms = Vec::new();
    for (row, weights) in disc.fits.iter().zip(&disc.weights) {
        for (fit, &w) in row.iter().zip(weights) {
            if fit.empty || w <= 0.0 {
                continue;
            }
            cells.integrate(|r, z| layer_at(&fit.layer, r, z), &mut scratch);
            let layer_norm: f64 = scratch.iter().sum();
            let sph_norm = if fit.spheroid.weight > 0.0 {
                cells.integrate(|r, z| spheroid_at(&fit.spheroid, r, z), &mut scratch);
                scratch.iter().sum::<f64>()
            } else {
                1.0
            };
            terms.push((w, *fit, layer_norm, sph_norm));
        }
    }
    let density = |z: f64| -> f64 {
        terms
            .iter()
            .map(|(w, f, ln, sn)| {
                w * (f.layer.weight * layer_at(&f.layer, radius, z) / ln
                    + f.spheroid.weight * spheroid_at(&f.spheroid, radius, z) / sn)
            })
            .sum()
    };
    let target = 0.5 * density(0.0);
    let (mut lo, mut hi) = (0.0, 10.0);
    if density(hi) > target {
        return hi;
    }
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if density(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// Runs every fit on `records`, the histograms of [`classes`], on `threads` threads.
///
/// # Errors
///
/// [`RunTaskError::Threads`] if the thread pool cannot be built.
pub fn fit_all(
    params: &FitParams,
    records: Vec<ClassHistogram>,
    threads: NonZeroUsize,
) -> Result<FormFits, RunTaskError> {
    let (galaxy_params, scales, fields) = fixture();
    let r_d = scales.r_d().value();
    let cells = RzCells::new(r_d, params.disc.order);
    let bar_cells = BarCells::new(r_d, params.disc.order.min(3));
    let all = classes();
    let of = |pick: &dyn Fn(&OrbitClass) -> bool| -> Vec<ClassHistogram> {
        all.iter()
            .zip(&records)
            .filter(|(c, _)| pick(c))
            .map(|(_, r)| r.clone())
            .collect()
    };
    let thin = of(&|c| c.source == Source::Thin);
    let disc = fit_disc(
        &cells,
        &thin,
        &params.speed_weights,
        &age_weights(&fields, &scales),
        &params.disc,
        threads,
    )?;
    let mut old: [Vec<OldFit>; 5] = Default::default();
    let mut bar_elongation = Vec::new();
    for (slot, source) in old.iter_mut().zip(Source::OLD) {
        let kicked = of(&|c| c.source == source && c.speed.is_some());
        *slot = if source.is_barred() {
            let own = own_form(&fields, source.population(), &bar_cells, r_d);
            if source == Source::LongBar {
                let control = of(&|c| c.source == source && c.speed.is_none());
                let radius = 2.0 * galaxy_params.bar().half_length().value();
                bar_elongation = elongation(&kicked, &control[0], radius);
            }
            fit_mixtures(&bar_cells, &own, &kicked, params.old, threads)?
        } else {
            fit_spheroids(&cells, &kicked, params.old, threads)?
        };
    }
    let full = PotentialTables::full(&MassModel::new(&galaxy_params));
    let escape = |r: f64, z: f64| {
        full.escape_speed(LightYears::new(r), LightYears::new(z))
            .map_or(0.0, hyperion_sim::units::KilometresPerSecond::value)
    };
    let hypervelocity =
        fit_hypervelocity(&fields, r_d, &cells, params.hypervelocity, escape, threads)?;
    let half_height_ly = half_height(&cells, &disc, SUNLIKE_RADIUS_LY / r_d) * r_d;
    Ok(FormFits {
        histograms: params.histograms,
        orbits: records.iter().map(|r| r.orbits).sum(),
        disc,
        old,
        records,
        bar_elongation,
        hypervelocity,
        scales,
        half_height_ly,
    })
}

/// A literal, for the table's rows.
fn lit(v: f64) -> String {
    literal(v)
}

fn layer_text(l: &FlaredLayerParams) -> String {
    format!(
        "FlaredLayerParams {{ weight: {}, h_r: {}, h_0: {}, r_flare: {}, beta: {} }}",
        lit(l.weight),
        lit(l.h_r),
        lit(l.h_0),
        lit(l.r_flare),
        lit(l.beta)
    )
}

fn spheroid_text(s: &CoredPowerLawParams) -> String {
    format!(
        "CoredPowerLawParams {{ weight: {}, a: {}, q: {}, gamma: {} }}",
        lit(s.weight),
        lit(s.a),
        lit(s.q),
        lit(s.gamma)
    )
}

fn kinematics_text(k: &ClassKinematics) -> String {
    format!(
        "ClassKinematics {{ mean_phi: {}, sigma_r: {}, sigma_phi: {}, sigma_z: {}, outbound: {} }}",
        lit(k.mean_phi),
        lit(k.sigma_r),
        lit(k.sigma_phi),
        lit(k.sigma_z),
        lit(k.outbound)
    )
}

fn nodes_text(v: f64) -> String {
    format!("[{}, {}, {}]", lit(v), lit(v), lit(v))
}

/// The `DISC_BORN` item.
fn disc_born_item(fits: &FormFits) -> String {
    let mut out = String::new();
    out.push_str(
        "/// The disc-born classes, `[speed bin][age bin]`: the thin disc's remnants by kick speed and\n\
         /// time since death, their bound members' form, in-cube shares and kinematics.\n\
         #[rustfmt::skip]\n\
         pub const DISC_BORN: [[DiscBornRow; 7]; 8] = [\n",
    );
    for s in 0..SPEED_BINS {
        out.push_str("    [\n");
        for a in 0..AGE_BINS {
            let fit = &fits.disc.fits[s][a];
            let record = &fits.records[s * AGE_BINS + a];
            let (bound, unbound) = in_cube(record);
            writeln!(
                out,
                "        DiscBornRow {{ layer: {}, spheroid: {}, in_cube: {}, unbound_in_cube: {}, \
                 kinematics: {}, misplaced: {} }},",
                layer_text(&fit.layer),
                spheroid_text(&fit.spheroid),
                nodes_text(bound),
                nodes_text(unbound),
                kinematics_text(&kinematics(record)),
                lit(fit.misplaced)
            )
            .expect("writing to a String cannot fail");
        }
        out.push_str("    ],\n");
    }
    out.push_str("];\n");
    out
}

/// One old source's item.
fn old_item(fits: &FormFits, source: Source, index: usize, name: &str, doc: &str) -> String {
    let mut out = String::new();
    write!(
        out,
        "/// {doc}\n#[rustfmt::skip]\npub const {name}: [OldBornRow; 8] = [\n"
    )
    .expect("writing to a String cannot fail");
    let records: Vec<&ClassHistogram> = classes()
        .iter()
        .zip(&fits.records)
        .filter(|(c, _)| c.source == source && c.speed.is_some())
        .map(|(_, r)| r)
        .collect();
    for (fit, record) in fits.old[index].iter().zip(records) {
        let (bound, unbound) = in_cube(record);
        writeln!(
            out,
            "    OldBornRow {{ own_share: {}, spheroid: {}, in_cube: {}, unbound_in_cube: {}, \
             kinematics: {}, misplaced: {} }},",
            nodes_text(fit.own_share),
            spheroid_text(&fit.spheroid),
            nodes_text(bound),
            nodes_text(unbound),
            kinematics_text(&kinematics(record)),
            lit(fit.misplaced)
        )
        .expect("writing to a String cannot fail");
    }
    out.push_str("];\n");
    out
}

/// The table's items.
#[must_use]
pub fn render(fits: &FormFits) -> RustTable {
    let doc = |lines: &[&str]| lines.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
    let mut items = vec![
        TableItem::Source(
            "use crate::galaxy::displaced::forms::{\n    ClassKinematics, CoredPowerLawParams, \
             DiscBornRow, FlaredLayerParams, OldBornRow,\n};\n"
                .to_owned(),
        ),
        TableItem::Array {
            name: "SPEED_EDGES".to_owned(),
            doc: doc(&[
                "The speed bins' inner edges, in `v_c` (the nuclear disc's own for its classes).",
            ]),
            values: SPEED_EDGES.to_vec(),
        },
        TableItem::Array {
            name: "AGE_EDGES".to_owned(),
            doc: doc(&["The age bins' inner edges, time since death in `R_d ÷ v_c`."]),
            values: AGE_EDGES.to_vec(),
        },
        TableItem::Array {
            name: "ESCAPE_RATIO_NODES".to_owned(),
            doc: doc(&["The escape ratios `v_esc ÷ v_c` at 3 `R_d` of the in-cube shares' nodes."]),
            values: ESCAPE_RATIO_NODES.to_vec(),
        },
        TableItem::Array {
            name: "COROTATION_RATIO_NODES".to_owned(),
            doc: doc(&["The bar's corotation over half-length at the own-form shares' nodes."]),
            values: COROTATION_RATIO_NODES.to_vec(),
        },
        TableItem::Source(disc_born_item(fits)),
    ];
    for (index, (source, name, text)) in [
        (
            Source::Thick,
            "THICK_DISC_BORN",
            "The thick disc's remnants by speed bin; `own_share` zero.",
        ),
        (
            Source::Halo,
            "HALO_BORN",
            "The halo's remnants by speed bin; `own_share` zero.",
        ),
        (
            Source::Bulge,
            "BULGE_BORN",
            "The bulge's remnants by speed bin.",
        ),
        (
            Source::LongBar,
            "BAR_BORN",
            "The long bar's remnants by speed bin.",
        ),
        (
            Source::NuclearDisc,
            "NUCLEAR_DISC_BORN",
            "The nuclear disc's remnants by speed bin, speeds against its own circular speed.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        items.push(TableItem::Source(old_item(fits, source, index, name, text)));
    }
    items.push(TableItem::Source(format!(
        "/// The ancient Type Ia survivors on straight lines (P15.T6.f), weight 1.\n\
         #[rustfmt::skip]\n\
         pub const HYPERVELOCITY: CoredPowerLawParams = {};\n",
        spheroid_text(&fits.hypervelocity.form)
    )));
    items.push(TableItem::Scalar {
        name: "HYPERVELOCITY_MEAN_EXIT_LY".to_owned(),
        doc: doc(&[
            "The survivors' rate-weighted mean path from launch to the cube's face, ly, along",
            "isotropic directions: a population's residence is it over its `1 ÷ ⟨1 ÷ v⟩` (P15.T6.f).",
        ]),
        value: fits.hypervelocity.mean_exit_ly,
    });
    RustTable {
        summary: vec![
            "The displaced classes' dimensionless forms, in-cube shares and kinematics (plan 15,"
                .to_owned(),
            "P15.T6), which `galaxy::displaced` reads.".to_owned(),
        ],
        notes: notes(fits),
        items,
    }
}

/// The table's notes: units, what is provisional, and every class's misplaced share against its
/// noise floor.
fn notes(fits: &FormFits) -> Vec<String> {
    let mut out = vec![
        "Lengths are in the thin disc's scale length `R_d`, speeds in the circular speed `v_c` at 3"
            .to_owned(),
        "`R_d` (the nuclear disc's own at 1.5 of its scale lengths for its classes), times in `R_d ÷"
            .to_owned(),
        "v_c` (plan 08, Design note 12). Node arrays are read by linear interpolation, clamped."
            .to_owned(),
        String::new(),
        "One orbit run, at the fixture's halo mass and bar, fills each node array: the value".to_owned(),
        "measured there stands at all three nodes until the runs at other halo masses and".to_owned(),
        "corotation ratios exist (P15.T6.d–e).".to_owned(),
    ];
    if let Some(by) = fits.histograms.provisional() {
        out.push(String::new());
        out.push(format!("**Provisional** ({by}): the production histograms"));
        out.push("replace them, with plan 08's P08.T12 bump.".to_owned());
    }
    out.push(String::new());
    out.push("Misplaced share ÷ noise floor, disc-born, by speed row (age bins 0–6):".to_owned());
    for (s, row) in fits.disc.fits.iter().enumerate() {
        let cells: Vec<String> = row
            .iter()
            .map(|f| {
                if f.empty || f.floor <= 0.0 {
                    "-".to_owned()
                } else {
                    format!("{:.2}", f.misplaced / f.floor)
                }
            })
            .collect();
        out.push(format!("- row {s}: {}", cells.join(", ")));
    }
    out
}

/// The acceptance figures, for the header.
#[must_use]
pub fn acceptance(fits: &FormFits) -> String {
    let d = &fits.disc;
    let mut edges = 0;
    let mut heavy_edges = 0;
    for (row, weights) in d.fits.iter().zip(&d.weights) {
        for (f, &w) in row.iter().zip(weights) {
            if f.at_edge > 0 {
                edges += 1;
                if w > 1e-4 {
                    heavy_edges += 1;
                }
            }
        }
    }
    let worst = |fits: &[OldFit]| fits.iter().map(|f| f.misplaced).fold(0.0, f64::max);
    let shares = |fits: &[OldFit]| {
        fits.iter()
            .take(4)
            .map(|f| format!("{:.2}", f.own_share))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let elongations: Vec<String> = fits
        .bar_elongation
        .iter()
        .map(|e| e.map_or_else(|| "-".to_owned(), |e| format!("{:.2}", e.relative)))
        .collect();
    let lengths = fits
        .bar_elongation
        .iter()
        .flatten()
        .map(|e| (e.length - 1.0).abs())
        .fold(0.0, f64::max);
    let thin = |s: usize| {
        let record = &fits.records[s * AGE_BINS + AGE_BINS - 1];
        if record.in_cube_bound == 0 {
            "-".to_owned()
        } else {
            format!("{:.3}", kinematics(record).mean_phi)
        }
    };
    let pc = fits.half_height_ly / hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
    format!(
        "on {} ({} orbits): disc-born weighted mean misplaced share {:.4} (unregularised {:.4}, λ \
         {}); {edges} of 56 classes with a parameter on a box edge, {heavy_edges} of them over \
         10⁻⁴ of the weight; {} second-difference sign changes along the rows; worst misplaced \
         share thick disc {:.3}, halo {:.3}, bulge {:.3}, bar {:.3}, nuclear disc {:.3}; own-form \
         shares at the fixture's corotation ratio {:.2}, bar {}, bulge {}, nuclear disc {}; bar \
         elongation over the control {} and length within {:.2} of it; slowest thin class's \
         mean rotation {} `v_c`, fastest's {}; the neutron stars' half-density height at \
         26,000 ly {pc:.0} pc; hypervelocity row misplaced {:.3}, {:.0} survivors inside the cube ({:.0} slow, {:.0} \
         fast; mean path {:.0} ly), {:.3} of slow launches below 1.5 times the local escape speed. \
         Not yet run: the universality potentials, plan 08's births' baseline and the other halo \
         masses and corotation ratios (P15.T6.d–e), and the kick-law reweighting of the unbound, \
         in-cube and phase-mixing checks (the histograms hold no total unbound count and one \
         last age bin)",
        fits.histograms.describe(),
        fits.orbits,
        d.mean_misplaced,
        d.unregularised,
        literal(d.lambda),
        super::fit_disc::second_difference_sign_changes(d),
        worst(&fits.old[0]),
        worst(&fits.old[1]),
        worst(&fits.old[2]),
        worst(&fits.old[3]),
        worst(&fits.old[4]),
        fits.scales.corotation_ratio(),
        shares(&fits.old[3]),
        shares(&fits.old[2]),
        shares(&fits.old[4]),
        elongations.join(", "),
        lengths,
        thin(0),
        thin(SPEED_BINS - 1),
        fits.hypervelocity.misplaced,
        fits.hypervelocity.inside[0] + fits.hypervelocity.inside[1],
        fits.hypervelocity.inside[0],
        fits.hypervelocity.inside[1],
        fits.hypervelocity.mean_exit_ly,
        fits.hypervelocity.slow_near_escape,
    )
}

/// The citations the fit was made against.
pub const SOURCE: &str = "orbits integrated in the model's own potential with a rotating \
    Dehnen (2000) quadrupole bar (P15.T6.a–b); the brainstorm's form families and own-form \
    shares; Maoz and Graur (2017, ApJ 848, 25) for the Type Ia delays; Shen et al. (2018, ApJ \
    865, 15) for the survivors' D6 mechanism and El-Badry et al. (2023, Open Journal of Astrophysics 6, §8.2) for their \
    slow and fast populations (ruling 128.1); Li et al. (2011, MNRAS 412, 1473) \
    for the Galaxy's Type Ia rate";

impl FitTask for DisplacedFormsTask {
    fn name(&self) -> &'static str {
        TASK
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "displaced_forms.rs"
    }

    fn revision(&self) -> u32 {
        REVISION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    fn fingerprint(&self) -> SimFingerprint {
        let (_, scales, _) = fixture();
        SimFingerprint::new(
            vec![
                ("GalaxyScales::r_d (ly)".to_owned(), scales.r_d().value()),
                ("GalaxyScales::v_c (km/s)".to_owned(), scales.v_c().value()),
                (
                    "GalaxyScales::escape_ratio".to_owned(),
                    scales.escape_ratio(),
                ),
                (
                    "GalaxyScales::nuclear_v_c (km/s)".to_owned(),
                    scales.nuclear_v_c().value(),
                ),
                (
                    "GalaxyScales::corotation_ratio".to_owned(),
                    scales.corotation_ratio(),
                ),
                (
                    "class_table::MILKY_WAY_IA_RATE_PER_YEAR".to_owned(),
                    MILKY_WAY_IA_RATE_PER_YEAR,
                ),
            ]
            .into_iter()
            .chain(SURVIVOR_POPULATIONS.iter().enumerate().flat_map(|(i, p)| {
                [
                    (
                        format!("class_table::SURVIVOR_POPULATIONS[{i}].share"),
                        p.share,
                    ),
                    (
                        format!("class_table::SURVIVOR_POPULATIONS[{i}].min_km_s"),
                        p.min_km_s,
                    ),
                    (
                        format!("class_table::SURVIVOR_POPULATIONS[{i}].max_km_s"),
                        p.max_km_s,
                    ),
                ]
            }))
            .collect(),
        )
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let params = FitParams::from_manifest(manifest)?;
        let input = |e: ReadOrbitsError| RunTaskError::Input {
            task: TASK,
            source: Box::new(e),
        };
        let [dataset] = manifest.datasets() else {
            return Err(RunTaskError::Param(ManifestParamError::new(
                "datasets",
                "one dataset, the orbit run's histograms",
            )));
        };
        let data = manifest.load_dataset(dataset)?;
        let bytes = data
            .files
            .iter()
            .find(|(name, _)| name == ORBITS_FILE)
            .map(|(_, b)| b)
            .ok_or_else(|| input(ReadOrbitsError::Missing))?;
        let text = std::str::from_utf8(bytes)
            .map_err(|e| input(ReadOrbitsError::Malformed(e.to_string())))?;
        let (hash, records) = read_orbits(text).map_err(input)?;
        let orbit_path = manifest
            .path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&params.orbit_manifest);
        let orbit_manifest = Manifest::load(&orbit_path).map_err(|e| RunTaskError::Input {
            task: TASK,
            source: Box::new(e),
        })?;
        let expected = sha256_hex(orbit_manifest.bytes());
        if hash != expected {
            return Err(input(ReadOrbitsError::OtherManifest {
                found: hash,
                expected,
            }));
        }
        let fits = fit_all(&params, records, threads)?;
        Ok(TaskOutput {
            table: render(&fits),
            source: SOURCE.to_owned(),
            acceptance: acceptance(&fits),
            provisional: params.histograms.provisional(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_run_reads_back() {
        let mut records: Vec<ClassHistogram> = classes()
            .iter()
            .map(|c| ClassHistogram::new(c.source.is_barred()))
            .collect();
        records[3].orbits = 7;
        let mut text =
            String::from("# hyperion-fit displaced_forms orbit histograms (plan 15, P15.T6.b)\n");
        text.push_str("# manifest-sha256 abc\n");
        for (class, record) in classes().iter().zip(&records) {
            record.write(&class.name(), &mut text);
        }
        let (hash, back) = read_orbits(&text).unwrap();
        assert_eq!(hash, "abc");
        assert_eq!(back, records);
        assert!(matches!(
            read_orbits(&text[..text.len() / 2]),
            Err(ReadOrbitsError::Malformed(_))
        ));
    }
}
