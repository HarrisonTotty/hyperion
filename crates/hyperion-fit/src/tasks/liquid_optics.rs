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
//! - **the temperature** the file states, K, as it writes it: a label to the kelvin for the first
//!   seven liquids, whose reductions state the measured temperature, and the state the index was
//!   computed at, to 0.01 K, for R08.T5.k's nine (decision-p14-t49h-gas-index §2.3).
//!
//! **The files.** The first eight are step 1's seven liquids and step 4's glass, in their names'
//! order; then R08.T5.k's nine liquids from their gases and their measurements (oxygen, argon,
//! krypton, xenon, carbon monoxide and dioxide, nitrous oxide, sulphur dioxide and hydrogen
//! sulphide; decision-p14-t49h-gas-index §1.2), in theirs, so that the earlier items keep their
//! order in the table.
//!
//! **A solid held for a melt** ([`HELD_SOLIDS`]). One file is a solid's: R08's basaltic glass
//! (`basaltic-glass.json`, Pollack, Toon and Khare 1973), whose room-temperature index and
//! absorption the silicate melts hold as they stand, uncarried (decision-r11-palette-liquids §1.8's
//! "Molten silicate" row). It is reduced to the bins as a liquid's file is, but it states no
//! temperature, so its reduction has none: the table holds its n and a and no temperature item,
//! and the sim, which never carries the glass's index, reads no state for it. No temperature is
//! invented for it.
//!
//! The optical values are not fitted: every one is the files' own, reduced by stated arithmetic.
//! R09.T2.e's hand-entered records took their bins the same way (`binmeans.py` in its lane's
//! scratch), so a record the registry resolves at a file's own state reproduces them. Which of a
//! file's members the registry takes, and on what basis, is the sim's per liquid
//! (`substance::liquid`): the task reduces every file whole, and a k that a file holds at zero,
//! unmeasured, becomes an a of zero here that the sim may leave unread.
//!
//! **The transport fit** (decision-r11-palette-liquids §4: "and any transport fit"). For each row
//! whose liquid a solution holds below the row's own triple point (methane and ethane in Titan's
//! seas, whose first melt is 71.15 K; decision-p14-t49b-condensed §2.5), the task fits the
//! Vogel–Fulcher–Tammann form `substance::liquid::SupercooledViscosity` evaluates, ln μ(T) = ln
//! μ(`T_t`) + B [1 ÷ (T − `T_0`) − 1 ÷ (`T_t` − `T_0`)], anchored at the row's own viscosity at its
//! triple point, to that viscosity along the saturated liquid from the triple point to the normal
//! boiling point (101,325 Pa), as `hyperion_sim::substance::liquid_properties` resolves it: least
//! squares in ln μ over [`VFT_SAMPLES`] even temperatures, B linear for each `T_0`, and `T_0` the
//! minimiser over [0, `T_t` − 1 K] by a scan of [`VFT_SCAN`] steps and a golden-section search,
//! held at 0 where the minimum lies below it (an Arrhenius form, VFT's limit). The fit reads the
//! sim's viscosities, which the fingerprint probes.

use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::substance::{Composition, LIQUID_BIN_EDGES_NM, by_key, liquid_properties};
use hyperion_sim::units::{Kelvin, Pascals};
use hyperion_surface::field::LIQUID_BINS;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::materials::MaterialFile;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 1 adds the transport fit; 2 reads the basaltic glass, a solid held for the
/// silicate melts ([`HELD_SOLIDS`]).
pub const VERSION: u32 = 2;

/// The files the manifest names, in its order, each one of R08's liquid files or a solid of
/// [`HELD_SOLIDS`]: the first eight by name, then R08.T5.k's nine by name.
pub const FILES: [&str; 17] = [
    "ammonia-liquid.json",
    "basaltic-glass.json",
    "ethane-liquid.json",
    "methane-liquid.json",
    "nitrogen-liquid.json",
    "sulphuric-acid-75.json",
    "sulphuric-acid-84.json",
    "water.json",
    "argon-liquid.json",
    "carbon-dioxide-liquid.json",
    "carbon-monoxide-liquid.json",
    "hydrogen-sulphide-liquid.json",
    "krypton-liquid.json",
    "nitrous-oxide-liquid.json",
    "oxygen-liquid.json",
    "sulphur-dioxide-liquid.json",
    "xenon-liquid.json",
];

/// The files of [`FILES`] that are a solid's, whose index and absorption a liquid holds as they
/// stand at no stated temperature.
///
/// The one held is R08's basaltic glass, for the silicate melts. Their reduction has no
/// temperature unless the file states one.
pub const HELD_SOLIDS: [&str; 1] = ["basaltic-glass.json"];

/// The rows whose viscosity the task continues below the triple point, by registry key, with
/// their items' prefix.
pub const SUPERCOOLED: [(&str, &str); 2] = [("CH4", "METHANE"), ("C2H6", "ETHANE")];

/// The temperatures the transport fit samples, evenly from the triple point to the normal boiling
/// point, both included.
pub const VFT_SAMPLES: u32 = 64;

/// The steps of the scan for `T_0` over [0, `T_t` − 1 K] before the golden-section search.
pub const VFT_SCAN: u32 = 400;

/// The table's items: for each file in [`FILES`]'s order, its bins of n, its bins of a and its
/// temperature (none for a solid of [`HELD_SOLIDS`] that states none); then for each row of
/// [`SUPERCOOLED`], its B and its `T_0`.
pub const ITEMS: [&str; 54] = [
    "AMMONIA_LIQUID_N",
    "AMMONIA_LIQUID_A",
    "AMMONIA_LIQUID_TEMPERATURE_K",
    "BASALTIC_GLASS_N",
    "BASALTIC_GLASS_A",
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
    "ARGON_LIQUID_N",
    "ARGON_LIQUID_A",
    "ARGON_LIQUID_TEMPERATURE_K",
    "CARBON_DIOXIDE_LIQUID_N",
    "CARBON_DIOXIDE_LIQUID_A",
    "CARBON_DIOXIDE_LIQUID_TEMPERATURE_K",
    "CARBON_MONOXIDE_LIQUID_N",
    "CARBON_MONOXIDE_LIQUID_A",
    "CARBON_MONOXIDE_LIQUID_TEMPERATURE_K",
    "HYDROGEN_SULPHIDE_LIQUID_N",
    "HYDROGEN_SULPHIDE_LIQUID_A",
    "HYDROGEN_SULPHIDE_LIQUID_TEMPERATURE_K",
    "KRYPTON_LIQUID_N",
    "KRYPTON_LIQUID_A",
    "KRYPTON_LIQUID_TEMPERATURE_K",
    "NITROUS_OXIDE_LIQUID_N",
    "NITROUS_OXIDE_LIQUID_A",
    "NITROUS_OXIDE_LIQUID_TEMPERATURE_K",
    "OXYGEN_LIQUID_N",
    "OXYGEN_LIQUID_A",
    "OXYGEN_LIQUID_TEMPERATURE_K",
    "SULPHUR_DIOXIDE_LIQUID_N",
    "SULPHUR_DIOXIDE_LIQUID_A",
    "SULPHUR_DIOXIDE_LIQUID_TEMPERATURE_K",
    "XENON_LIQUID_N",
    "XENON_LIQUID_A",
    "XENON_LIQUID_TEMPERATURE_K",
    "METHANE_SUPERCOOLED_VISCOSITY_B_K",
    "METHANE_SUPERCOOLED_VISCOSITY_T0_K",
    "ETHANE_SUPERCOOLED_VISCOSITY_B_K",
    "ETHANE_SUPERCOOLED_VISCOSITY_T0_K",
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
    /// The temperature the file states, K: always a liquid's, and `None` for a solid of
    /// [`HELD_SOLIDS`] that states none.
    pub temperature_k: Option<f64>,
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
/// [`RunTaskError::Input`] if the file is neither a liquid's nor a solid of [`HELD_SOLIDS`], is a
/// liquid's that states no temperature, or its grid does not cover 380–760 nm.
pub fn reduce(name: &'static str, file: &MaterialFile) -> Result<Reduced, RunTaskError> {
    let bad = |why: String| RunTaskError::Input {
        task: "liquid_optics",
        source: format!("{name}: {why}").into(),
    };
    let held = file.phase() == "solid" && HELD_SOLIDS.contains(&name);
    if file.phase() != "liquid" && !held {
        return Err(bad(format!(
            "its phase is {}, not liquid, and it is not a solid held for a melt",
            file.phase()
        )));
    }
    let temperature_k = file.temperature_k();
    if temperature_k.is_none() && !held {
        return Err(bad("it states no temperature".to_owned()));
    }
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

/// One row's Vogel–Fulcher–Tammann fit to its viscosity along the saturated liquid.
#[derive(Debug, Clone, PartialEq)]
pub struct SupercooledFit {
    /// The row's key.
    pub key: &'static str,
    /// The items' prefix.
    pub prefix: &'static str,
    /// The triple point, K, where the form is anchored.
    pub triple_k: f64,
    /// The window's top, the normal boiling point, K.
    pub boiling_k: f64,
    /// B, K.
    pub b_k: f64,
    /// `T_0`, K.
    pub t0_k: f64,
    /// The largest relative miss of the fitted form against the viscosity over the window.
    pub worst: f64,
}

/// The pressure of the window's top, Pa: one standard atmosphere.
const ONE_ATMOSPHERE: Pascals = Pascals::new(101_325.0);

/// A row's viscosity along its saturated liquid, from the sim, which the transport fit reads.
#[derive(Debug, Clone, PartialEq)]
pub struct SaturatedViscosities {
    /// The triple point, where the form is anchored.
    pub triple: Kelvin,
    /// The normal boiling point, the window's top.
    pub boiling: Kelvin,
    /// The [`VFT_SAMPLES`] even temperatures from the triple point to the boiling point, both
    /// included, each with the viscosity there, Pa s.
    pub samples_pa_s: Vec<(Kelvin, f64)>,
}

/// `key`'s viscosity along its saturated liquid from the sim ([`SaturatedViscosities`]).
///
/// # Errors
///
/// [`RunTaskError::Input`] if `key` is not a row with a phase diagram, does not boil at one
/// atmosphere above its triple point, `liquid_properties` refuses a sample's state, or a sample's
/// record lacks a viscosity.
pub fn saturated_viscosities(key: &'static str) -> Result<SaturatedViscosities, RunTaskError> {
    let bad = |why: String| RunTaskError::Input {
        task: "liquid_optics",
        source: format!("{key}: {why}").into(),
    };
    let id = by_key(key).ok_or_else(|| bad("not a registry row".to_owned()))?;
    let diagram = id
        .substance()
        .phase_diagram()
        .ok_or_else(|| bad("no phase diagram".to_owned()))?;
    let triple = diagram.triple_temperature();
    let boiling = diagram
        .saturation_temperature(ONE_ATMOSPHERE)
        .filter(|&b| b > triple)
        .ok_or_else(|| {
            bad("it does not boil at one atmosphere above its triple point".to_owned())
        })?;
    let last = f64::from(VFT_SAMPLES - 1);
    let mut samples_pa_s = Vec::with_capacity(usize::try_from(VFT_SAMPLES).unwrap_or(0));
    for i in 0..VFT_SAMPLES {
        let t =
            Kelvin::new(triple.value() + (boiling.value() - triple.value()) * f64::from(i) / last);
        let record =
            liquid_properties(id, t, Composition::Reference).map_err(|e| bad(e.to_string()))?;
        let mu = record
            .viscosity_pa_s
            .ok_or_else(|| bad(format!("no viscosity at {} K", t.value())))?;
        samples_pa_s.push((t, mu));
    }
    Ok(SaturatedViscosities {
        triple,
        boiling,
        samples_pa_s,
    })
}

/// For a given `T_0`, K, the least-squares B, K, of the anchored form over `samples` (the first
/// is the anchor) and the sum of squared misses in ln μ.
#[must_use]
fn vft_misses(samples: &[(Kelvin, f64)], t0: f64) -> (f64, f64) {
    let (triple, anchor) = samples[0];
    let triple = triple.value();
    let ln_anchor = math::ln(anchor);
    let mut xy = 0.0;
    let mut xx = 0.0;
    for &(t, mu) in samples {
        let t = t.value();
        let x = 1.0 / (t - t0) - 1.0 / (triple - t0);
        let y = math::ln(mu) - ln_anchor;
        xy += x * y;
        xx += x * x;
    }
    let b = xy / xx;
    let mut sum = 0.0;
    for &(t, mu) in samples {
        let t = t.value();
        let x = 1.0 / (t - t0) - 1.0 / (triple - t0);
        let miss = math::ln(mu) - ln_anchor - b * x;
        sum += miss * miss;
    }
    (b, sum)
}

/// Fits the anchored form to `key`'s viscosity (the module's note).
///
/// # Errors
///
/// As [`saturated_viscosities`].
pub fn fit_supercooled(
    key: &'static str,
    prefix: &'static str,
) -> Result<SupercooledFit, RunTaskError> {
    let viscosities = saturated_viscosities(key)?;
    let samples = &viscosities.samples_pa_s;
    let triple = viscosities.triple.value();
    let top = triple - 1.0;
    let step = top / f64::from(VFT_SCAN);
    let mut best = (f64::INFINITY, 0.0);
    for k in 0..=VFT_SCAN {
        let t0 = step * f64::from(k);
        let (_, misses) = vft_misses(samples, t0);
        if misses < best.0 {
            best = (misses, t0);
        }
    }
    let mut low = (best.1 - step).max(0.0);
    let mut high = (best.1 + step).min(top);
    let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
    for _ in 0..200 {
        let a = high - ratio * (high - low);
        let b = low + ratio * (high - low);
        if vft_misses(samples, a).1 < vft_misses(samples, b).1 {
            high = b;
        } else {
            low = a;
        }
    }
    let mut t0 = f64::midpoint(low, high);
    if vft_misses(samples, 0.0).1 <= vft_misses(samples, t0).1 {
        t0 = 0.0;
    }
    let (b, _) = vft_misses(samples, t0);
    let anchor = samples[0].1;
    let mut worst = 0.0_f64;
    for &(t, mu) in samples {
        let fitted = anchor * math::exp(b * (1.0 / (t.value() - t0) - 1.0 / (triple - t0)));
        worst = worst.max((fitted / mu - 1.0).abs());
    }
    Ok(SupercooledFit {
        key,
        prefix,
        triple_k: triple,
        boiling_k: viscosities.boiling.value(),
        b_k: b,
        t0_k: t0,
        worst,
    })
}

/// The table's contents for `reduced` and `supercooled`, in their order.
#[must_use]
pub fn render(reduced: &[Reduced], supercooled: &[SupercooledFit]) -> RustTable {
    let mut items = Vec::with_capacity(3 * reduced.len() + 2 * supercooled.len());
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
        if let Some(temperature_k) = file.temperature_k {
            items.push(TableItem::Scalar {
                name: format!("{prefix}_TEMPERATURE_K"),
                doc: vec![format!(
                    "The temperature `{}` states, K, as it writes it.",
                    file.file
                )],
                value: temperature_k,
            });
        }
    }
    for fit in supercooled {
        items.push(TableItem::Scalar {
            name: format!("{}_SUPERCOOLED_VISCOSITY_B_K", fit.prefix),
            doc: vec![
                format!(
                    "B of {}'s Vogel–Fulcher–Tammann viscosity below its triple point, K,",
                    fit.key
                ),
                format!(
                    "fitted over {:.4}–{:.4} K (worst miss {:.2e}).",
                    fit.triple_k, fit.boiling_k, fit.worst
                ),
            ],
            value: fit.b_k,
        });
        items.push(TableItem::Scalar {
            name: format!("{}_SUPERCOOLED_VISCOSITY_T0_K", fit.prefix),
            doc: vec![format!(
                "`T_0` of {}'s Vogel–Fulcher–Tammann viscosity, K.",
                fit.key
            )],
            value: fit.t0_k,
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
            "basis, is `substance::liquid`'s. The basaltic glass, a solid's file held for the silicate"
                .to_owned(),
            "melts, is reduced alike; it states no temperature, so it has none here. R08.T5.k's"
                .to_owned(),
            "nine liquids from their gases and their measurements follow the first eight. Then each"
                .to_owned(),
            "supercooled row's Vogel–Fulcher–Tammann B and `T_0`, fitted to its viscosity along the"
                .to_owned(),
            "saturated liquid from its triple point to its normal boiling point, anchored at its triple"
                .to_owned(),
            "point (`SupercooledViscosity`).".to_owned(),
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

    /// The sim values the task reads: the bins' edges, and each supercooled row's triple point,
    /// normal boiling point and viscosity at every one of the fit's samples; the material files
    /// are the inputs hash's.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes: Vec<(String, f64)> = bin_edges()
            .iter()
            .enumerate()
            .map(|(k, edge)| (format!("liquid bin edge {k} nm"), *edge))
            .collect();
        for (key, _) in SUPERCOOLED {
            match saturated_viscosities(key) {
                Ok(viscosities) => {
                    probes.push((format!("{key} triple point K"), viscosities.triple.value()));
                    probes.push((
                        format!("{key} normal boiling point K"),
                        viscosities.boiling.value(),
                    ));
                    for (t, mu) in viscosities.samples_pa_s {
                        probes.push((format!("{key} viscosity at {:.6} K, Pa s", t.value()), mu));
                    }
                }
                // A row the sim cannot resolve probes as NaN, which matches no lock file's value.
                Err(_) => probes.push((format!("{key} unresolved"), f64::NAN)),
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let reduced = fit(manifest)?;
        let supercooled = SUPERCOOLED
            .iter()
            .map(|&(key, prefix)| fit_supercooled(key, prefix))
            .collect::<Result<Vec<_>, _>>()?;
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
        let fits: Vec<String> = supercooled
            .iter()
            .map(|f| {
                format!(
                    "{} B {:.3} K, `T_0` {:.3} K, worst {:.1e} over {:.3}–{:.3} K",
                    f.key, f.b_k, f.t0_k, f.worst, f.triple_k, f.boiling_k
                )
            })
            .collect();
        let acceptance = format!(
            "{} material files ({} liquids' and {} solid's held for the melts), each to \
             {LIQUID_BINS} bins over 380–760 nm; water.json's a on its grid: {} m⁻¹ at 440 nm \
             (Mason, Cone and Fry 2016's 0.00522), {} at 550 nm and {} at 700 nm (Pope and Fry \
             1997's 0.0565 and 0.624); supercooled viscosity: {}",
            reduced.len(),
            reduced
                .iter()
                .filter(|r| !HELD_SOLIDS.contains(&r.file))
                .count(),
            reduced
                .iter()
                .filter(|r| HELD_SOLIDS.contains(&r.file))
                .count(),
            at(440.0),
            at(550.0),
            at(700.0),
            fits.join("; "),
        );
        Ok(TaskOutput {
            table: render(&reduced, &supercooled),
            source: "rendering plan R08's committed liquid material files \
                     (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), each citing its \
                     own primaries: IAPWS R9-97, Mason, Cone and Fry 2016, Pope and Fry 1997 and \
                     Kou, Labrie and Chýlek 1993 (water); Martonchik and Orton 1994 (methane); \
                     Pollack, Toon and Khare 1973 (basaltic glass, a solid held for the silicate \
                     melts); Augustin 1915 (nitrogen); Hagermann et al. 2005 with Loria 1909 \
                     (ethane); Wong and Anderson 1972 with Cuthbertson 1914 (ammonia); Palmer and \
                     Williams 1975 (sulphuric acid); R08.T5.k's nine, each its gas's dispersion in \
                     `rayleigh.ts` at the NIST Chemistry WebBook's saturated-liquid density: \
                     Sinnock and Smith 1969 (argon, krypton and xenon), Johns and Wilhelm 1937 \
                     with Thalman and Volkamer 2013's O2–O2 absorption (oxygen), Liveing and \
                     Dewar 1892 (nitrous oxide), Bleekrode 1884 with Ketteler 1865 and Bichat 1880 \
                     (sulphur dioxide) and Phillips 1920 (carbon dioxide), and unscaled \
                     Lorentz–Lorenz estimates (carbon monoxide and hydrogen sulphide); and the \
                     sim's own liquid viscosities, Sotiriadou et al. 2026 (methane) and Friend, \
                     Ingham and Ely 1991 (ethane), for the transport fit"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hyperion_sim::substance::{CondensedPhase, LiquidColumns};

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
    /// derived; the basaltic glass a measured solid's, which the melts hold as a stand-in;
    /// decision-p14-t49h-gas-index §2.3 and A.4: argon, krypton and xenon measured, oxygen, carbon
    /// dioxide, nitrous oxide and sulphur dioxide derived, and carbon monoxide and hydrogen
    /// sulphide stand-ins, whose index the record calls `Derived`). A file whose provenance
    /// changes fails here, so that its liquid's basis is revisited.
    #[test]
    fn liquid_optics_the_files_provenances_are_the_bases_assumed() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        let expected = [
            "derived", "measured", "derived", "measured", "derived", "measured", "measured",
            "measured", "measured", "derived", "standIn", "standIn", "measured", "derived",
            "derived", "derived", "measured",
        ];
        for (name, provenance) in FILES.iter().zip(expected) {
            let file = MaterialFile::read(&dir, name).unwrap();
            assert_eq!(file.provenance(), provenance, "{name}");
        }
    }

    /// The basaltic glass, a held solid, is reduced with no temperature, since its file states
    /// none, and its bins are its own: n about 1.568 and a about 10⁴ m⁻¹ in the bin across 545 nm.
    #[test]
    fn liquid_optics_the_held_glass_has_no_temperature() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        for name in HELD_SOLIDS {
            let file = MaterialFile::read(&dir, name).unwrap();
            assert_eq!(file.phase(), "solid", "{name}");
            assert!(
                file.temperature_k().is_none(),
                "{name}: {:?}",
                file.temperature_k()
            );
            let reduced = reduce(name, &file).unwrap();
            assert!(
                reduced.temperature_k.is_none(),
                "{name}: {:?}",
                reduced.temperature_k
            );
        }
        let glass = MaterialFile::read(&dir, "basaltic-glass.json").unwrap();
        let reduced = reduce("basaltic-glass.json", &glass).unwrap();
        assert!((reduced.n[6] - 1.568).abs() < 1e-3, "{:?}", reduced.n);
        assert!(
            reduced.a[6] > 0.9e4 && reduced.a[6] < 1.2e4,
            "{:?}",
            reduced.a
        );
    }

    /// One of Sinnock and Smith's tables: its temperature, K, and n at each of its atom's
    /// wavelengths.
    type SinnockTable = (f64, &'static [f64]);

    /// Sinnock and Smith 1969's tables of the liquid noble gases (Phys. Rev. 181, 1297), as
    /// refractiveindex.info's CC0 copies give them at commit c5c2f188
    /// (`main/<atom>/nk/Sinnock-liquid-<T>K.yml`, each file's SHA-256 in plan 14's P14.T49.h step
    /// 2b record): each atom's registry key, its R08 file, its rows' wavelengths, nm, and its
    /// tables. The rows at 361.2, 363.1 and 365.0 nm lie below the files' 380–780 nm grid and are
    /// left out; xenon's 161.35 K table lies below its file's state, the row's triple point, and is
    /// read by none of the windows.
    const SINNOCK_AND_SMITH: [(&str, &str, &[f64], &[SinnockTable]); 3] = [
        (
            "Ar",
            "argon-liquid.json",
            &[406.3, 435.8, 475.3, 508.6, 546.1, 578.0, 643.9],
            &[
                (
                    83.81,
                    &[1.2372, 1.2361, 1.2349, 1.2341, 1.2334, 1.2328, 1.2321],
                ),
                (
                    86.0,
                    &[1.2347, 1.2336, 1.2324, 1.2316, 1.2308, 1.2303, 1.2295],
                ),
                (
                    88.0,
                    &[1.2326, 1.2315, 1.2303, 1.2295, 1.2287, 1.2282, 1.2274],
                ),
                (
                    90.0,
                    &[1.2308, 1.2297, 1.2285, 1.2277, 1.2269, 1.2264, 1.2256],
                ),
            ],
        ),
        (
            "Kr",
            "krypton-liquid.json",
            &[406.3, 435.8, 475.3, 508.6, 546.1, 578.0, 643.9],
            &[
                (
                    115.95,
                    &[1.3090, 1.3074, 1.3056, 1.3042, 1.3032, 1.3024, 1.3011],
                ),
                (
                    118.0,
                    &[1.3065, 1.3049, 1.3031, 1.3017, 1.3008, 1.2999, 1.2986],
                ),
                (
                    122.0,
                    &[1.3018, 1.3002, 1.2984, 1.2970, 1.2960, 1.2952, 1.2939],
                ),
                (
                    126.0,
                    &[1.2973, 1.2957, 1.2938, 1.2924, 1.2915, 1.2906, 1.2893],
                ),
            ],
        ),
        (
            "Xe",
            "xenon-liquid.json",
            &[406.3, 435.8, 470.0, 480.6, 508.6, 546.1, 578.0, 643.9],
            &[
                (
                    161.35,
                    &[
                        1.4041, 1.4001, 1.3967, 1.3957, 1.3937, 1.3918, 1.3900, 1.3876,
                    ],
                ),
                (
                    166.0,
                    &[
                        1.3995, 1.3955, 1.3921, 1.3911, 1.3891, 1.3871, 1.3853, 1.3830,
                    ],
                ),
                (
                    170.0,
                    &[
                        1.3956, 1.3916, 1.3882, 1.3872, 1.3851, 1.3832, 1.3814, 1.3790,
                    ],
                ),
                (
                    174.0,
                    &[
                        1.3914, 1.3874, 1.3840, 1.3830, 1.3809, 1.3790, 1.3772, 1.3748,
                    ],
                ),
                (
                    178.0,
                    &[
                        1.3874, 1.3834, 1.3800, 1.3790, 1.3770, 1.3750, 1.3732, 1.3708,
                    ],
                ),
            ],
        ),
    ];

    /// The atoms' `index_checked` windows are Sinnock and Smith's tables'
    /// (decision-p14-t49h-gas-index §1.2's test, as the lane narrowed it): the file's own n at each
    /// table row's wavelength, linear on its 5 nm grid, carried at constant specific refraction by
    /// the row's own liquid density from the file's state to the table's temperature, against the
    /// row. Each window runs from the file's state to the warmest table at which every row, and
    /// every row of each cooler table above the state, lies within 5 × 10⁻⁴; the state alone where
    /// no warmer table does. The warmest table's worst miss and the next warmer table's are pinned
    /// to 10⁻⁷, so that a change to a file, a density or the carry is noticed: argon's window ends
    /// at its warmest table, 90 K (worst 4.17 × 10⁻⁴); krypton's at 115.95 K (4.7 × 10⁻⁵), 118 K
    /// missing by 1.25 × 10⁻³; and xenon's is its state alone, 166 K missing by 8.04 × 10⁻⁴.
    #[test]
    fn liquid_optics_the_atoms_index_windows_are_sinnock_and_smith_s() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE);
        let lorentz_lorenz = |n: f64| (n * n - 1.0) / (n * n + 2.0);
        let index_of = |l: f64| ((1.0 + 2.0 * l) / (1.0 - l)).sqrt();
        // Each atom's warmest checked table's worst miss, and the next warmer table's.
        let pinned = [
            (Some(0.000_416_9), None),
            (Some(0.000_046_5), Some(0.001_250_3)),
            (None, Some(0.000_803_8)),
        ];
        for ((key, name, wavelengths, tables), (warmest, next)) in
            SINNOCK_AND_SMITH.iter().zip(pinned)
        {
            let file = MaterialFile::read(&dir, name).unwrap();
            let row = by_key(key).unwrap().substance();
            let optics = row.liquid().and_then(LiquidColumns::optics).unwrap();
            let state = optics.state().value();
            let density = |t: f64| {
                row.grain_density(CondensedPhase::Liquid, Kelvin::new(t))
                    .unwrap()
                    .value()
            };
            let misses: Vec<(f64, f64)> = tables
                .iter()
                .filter(|(t, _)| *t >= state)
                .map(|&(t, rows)| {
                    let ratio = density(t) / density(state);
                    let worst = wavelengths
                        .iter()
                        .zip(rows)
                        .fold(0.0_f64, |worst, (&nm, &n)| {
                            let file_n = interpolate(file.wavelengths_nm(), file.n(), nm);
                            worst.max((index_of(lorentz_lorenz(file_n) * ratio) - n).abs())
                        });
                    (t, worst)
                })
                .collect();
            let checked = misses.iter().take_while(|(_, worst)| *worst < 5e-4).count();
            let top = checked.checked_sub(1).map_or(state, |k| misses[k].0);
            let (low, high) = optics.index_checked().unwrap();
            assert!(
                (low.value() - state).abs() < 1e-12 && (high.value() - top).abs() < 1e-12,
                "{key}: the window {} to {} K against {state} to {top} K ({misses:?})",
                low.value(),
                high.value()
            );
            let worst_at = |k: Option<usize>| k.and_then(|k| misses.get(k)).map(|m| m.1);
            for (ours, pin) in [
                (worst_at(checked.checked_sub(1)), warmest),
                (worst_at(Some(checked)), next),
            ] {
                assert_eq!(ours.is_some(), pin.is_some(), "{key}: {misses:?}");
                if let (Some(ours), Some(pin)) = (ours, pin) {
                    assert!((ours - pin).abs() < 1e-7, "{key}: {ours} against {pin}");
                }
            }
        }
    }

    /// A file that is neither a liquid's nor a solid of [`HELD_SOLIDS`] (a solid's file under any
    /// other name), a liquid's that states no temperature, or one whose grid does not cover
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

    /// The manifest names the task's files, and the items follow them, a held solid's without a
    /// temperature, and then the supercooled rows.
    #[test]
    fn liquid_optics_the_items_follow_the_files() {
        assert_eq!(manifest().materials(), FILES);
        let mut names: Vec<String> = FILES
            .iter()
            .flat_map(|f| {
                let p = item_prefix(f);
                let mut items = vec![format!("{p}_N"), format!("{p}_A")];
                if !HELD_SOLIDS.contains(f) {
                    items.push(format!("{p}_TEMPERATURE_K"));
                }
                items
            })
            .collect();
        for (_, prefix) in SUPERCOOLED {
            names.push(format!("{prefix}_SUPERCOOLED_VISCOSITY_B_K"));
            names.push(format!("{prefix}_SUPERCOOLED_VISCOSITY_T0_K"));
        }
        assert_eq!(names, ITEMS);
    }

    /// Each supercooled fit is anchored at the row's triple point, holds `T_0` in [0, `T_t`), meets
    /// the viscosity over its window within 1% for methane (a super-Arrhenius liquid, `T_0` above
    /// 0) and 4% for ethane (whose fit is Arrhenius, `T_0` = 0: along the saturated liquid its
    /// correlation's effective activation energy rises with temperature), and is no worse than the
    /// form at any other `T_0` of its scan.
    #[test]
    fn liquid_optics_the_supercooled_fits_are_least_squares() {
        for ((key, prefix), worst) in SUPERCOOLED.iter().zip([0.01, 0.04]) {
            let fit = fit_supercooled(key, prefix).unwrap();
            assert!(fit.t0_k >= 0.0 && fit.t0_k < fit.triple_k, "{fit:?}");
            assert!(fit.worst < worst, "{fit:?}");
            let samples = saturated_viscosities(key).unwrap().samples_pa_s;
            let (_, at_fit) = vft_misses(&samples, fit.t0_k);
            for k in 0..20 {
                let t0 = (fit.triple_k - 1.0) * f64::from(k) / 20.0;
                assert!(
                    at_fit <= vft_misses(&samples, t0).1 * (1.0 + 1e-12),
                    "{key}: {t0}"
                );
            }
        }
        let methane = fit_supercooled("CH4", "METHANE").unwrap();
        assert!(methane.t0_k > 10.0, "{methane:?}");
    }
}
