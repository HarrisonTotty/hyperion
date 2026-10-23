//! `run space_weathering`: the space-weathering law's table (rendering plan R10, R10.T1.d;
//! decision-p14-t51d-weathering §1.1 and §1.4, and its addendum of 2026-10-19), which
//! `hyperion_surface::material`'s weathering law reads as `tables::space_weathering`.
//!
//! # What it tabulates
//!
//! 1. **The nodes**: single-scattering albedos w from 0 to 1, uniform in γ = √(1 − w), so dense
//!    towards 1 where the albedos turn fastest ([`nodes`]).
//! 2. **Each reference form's albedos at each node** (the lunar regolith's and fresh frost's, the
//!    sim's [`ReferenceForm`]s), by `substance_albedos`' own Hapke solution: the normal albedo
//!    `A_N` by [`Medium::new`]`(w, form.photometry()).normal_albedo()`, and the spherical albedo
//!    `A_S` and the directional-hemispherical albedos `A_h` by [`energy_albedos`] at the form's
//!    phase-function asymmetry ([`columns`]). A column that is not strictly increasing in w, or an
//!    `A_S` or `A_h` above 1, is refused: the law inverts each column.
//! 3. **Each form's equivalent slab** at its host index `n_h` and k = 0, `S_e` and `S_i` by Hapke
//!    2012's fits as `substance_albedos` takes them ([`slab`]): R(0) = ((n − 1) ÷ (n + 1))²,
//!    `S_e` = 0.0587 + 0.8543 R(0) + 0.0870 R(0)², `S_i` = 1 − (0.9413 − 0.8543 R(0) − 0.0870
//!    R(0)²) ÷ n².
//! 4. **Iron's z(λ) at each form's host index** on a wavelength grid ([`iron_z_grid`]), z = `n_h`³
//!    n k ÷ [(n² − k² + 2 `n_h`²)² + (2 n k)²] ([`iron_z`]), with n and ln k linear in λ between
//!    the datasets' rows (rendering plan R08's reduction rule): Johnson and Christy 1974 up to
//!    their last row, 1,937 nm, and Ordal et al. 1988 beyond, times the join factor `z_JC` ÷
//!    `z_Ordal` at that row, so that z is continuous there (Ordal's n there is 3.82 against Johnson
//!    and Christy's 3.17, a step in z of 18% at `n_h` 1.7 and 16% at 1.31 otherwise).
//! 5. **Λ = `y_sat` ⟨D⟩**, the law's one fitted constant ([`fit_lambda`]): by golden-section search
//!    in ln Λ, the least squares of the weathered `A_N` at m = 1 against the Moon's mature highland
//!    and mare normal albedos in B, V and R, each residual divided by its anchor's σ = (P90 − P10)
//!    ÷ 2.563, the normal's 10–90% width. Each terrain is a fresh row of the sim's registry (its
//!    w, `A_N` and grain density) at its anchor soil's measured `FeO_T`, every one a manifest
//!    parameter. The frost's Λ is the regolith's times the ratio of the forms' mean paths ⟨D⟩ =
//!    ⅔ [n² − (n² − 1)^{3/2} ÷ n] D at each one's host index and grain diameter ([`mean_path`]).
//!
//! The law it fits is `hyperion_surface::material`'s, restated here on the tables in memory with
//! the same operations in the same order ([`Law`]); a test holds the two to the bit.
//!
//! # The checks, reported and never fitted
//!
//! The run refuses only on an internal failure: a non-monotone column, an albedo above 1, a value
//! that is not finite. An anchor missed is reported, never refused:
//!
//! - each terrain's weathered B, V and R against its anchor, with z = (model − centre) ÷ σ and
//!   whether it lies inside its 10–90% bounds, and each terrain's own best Λ;
//! - the implied yield `y_sat` = Λ ÷ ⟨D⟩ of the regolith form against Morris 1980's Eq. 4 at the
//!   maturity index T51.d's `τ_L` holds, within a factor of 3, beside the mature soils' medians.
//!   Eq. 4 as printed is `Fe⁰` = 3.2 × 10⁻⁴ × `FeO` × (`I_s`/`FeO`) (p. 1701), per `FeO`; the run
//!   takes it per `FeO`-iron, × `M_FeO` ÷ `M_Fe`, 4.117 × 10⁻⁴ ([`Checks::yield_centre`]);
//! - each terrain's fresh/mature contrast against fresh lunar craters (Dollfus and Bowell 1971,
//!   Table 4);
//! - the join factors, the frost's Λ, and how closely each fresh row's `A_N` inverts to its own w.
//!
//! # Sources
//!
//! - The law: Hapke 2001 (JGR 106, 10039, *from memory*), as decision-p14-t51d-weathering §1.1
//!   re-derives it; the slab and the form's photometry, `substance_albedos`' own (its sources).
//! - Iron: Johnson and Christy 1974 (Phys. Rev. B 9, 5056) and Ordal et al. 1988 (Appl. Opt. 27,
//!   1203), through the refractiveindex.info database (Polyanskiy 2024, Sci. Data 11, 94), the
//!   datasets `johnson_christy_1974_fe` and `ordal_1988_fe`.
//! - The anchors: Dollfus and Bowell 1971 (A&A 10, 29, Table 4, read), the 600 nm normal albedos
//!   of the maria and highlands (each median, P10 and P90), taken to B, V and R by the paper's own
//!   whole-Moon spectral law, Eq. 11 (p. 50, read), log Ā = 0.83 log λ − 0.80 with λ in µm,
//!   fitted to Gehrels et al. 1964's Table XIII, of which only the slope is used: R(λ) ÷ R(600
//!   nm) = (λ ÷ 600 nm)^0.83 ([`Colour`]). One colour for both terrains; the terrains' mean B/V
//!   differ by < 1% at zero phase (Gehrels et al. 1964, Table XIII); the maria's own ±3.5% spread
//!   is not modelled (decision-r10-t1d-anchor-colour). The stated check, in the manifest's
//!   comments, is Gehrels, Coffeen and Owings 1964's zero-phase B/V and Lane and Irvine 1973's.
//!   The scale is built from catalogues at phase angles 1.5–12° (their Table 7), its whole-Moon
//!   mean 0.101 (Eq. 8) against `p_V` 0.12, so the anchors are not zero-phase albedos
//!   (decision-p14-t51d-weathering's addendum, D6). The decision record compared these colours
//!   with laboratory LSCC/RELAB soil spectra. RELAB's terms are non-commercial, so nothing from
//!   them is committed;
//!   the anchor soils' `FeO_T` and the yield, Morris 1980 (Proc. LPSC 11, 1697, Table 1 and
//!   Eqs. 3–4, read). The manifest's comments carry each figure's place.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::substance::optics::{
    self, BAND_WAVELENGTHS_NM, HEMISPHERICAL_INCIDENCE_COSINES, OppositionEffect, PhaseFunction,
    ReferenceForm,
};
use hyperion_sim::substance::{GrainDensity, by_key};
use hyperion_surface::material::{
    FEO_MOLAR_MASS_G_PER_MOL, ICE_HOST_INDEX, IRON_DENSITY_KG_M3, IRON_MOLAR_MASS_G_PER_MOL,
    REGOLITH_HOST_INDEX,
};

use super::substance_albedos::{Medium, energy_albedos};
use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::optimise::golden_section;
use crate::task::{FitTask, RunTaskError, TableCrate, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The task's name.
const NAME: &str = "space_weathering";

/// The table's items, in order.
pub const ITEMS: [&str; 16] = [
    "W",
    "HEMISPHERICAL_INCIDENCE_COSINES",
    "IRON_Z_FIRST_NM",
    "IRON_Z_STEP_NM",
    "REGOLITH_NORMAL_ALBEDO",
    "REGOLITH_SPHERICAL_ALBEDO",
    "REGOLITH_HEMISPHERICAL_ALBEDO",
    "REGOLITH_SLAB",
    "REGOLITH_IRON_Z",
    "REGOLITH_LAMBDA_M",
    "FROST_NORMAL_ALBEDO",
    "FROST_SPHERICAL_ALBEDO",
    "FROST_HEMISPHERICAL_ALBEDO",
    "FROST_SLAB",
    "FROST_IRON_Z",
    "FROST_LAMBDA_M",
];

/// The dataset of iron's optical constants below the join: Johnson and Christy 1974.
pub const NEAR_DATASET: &str = "johnson_christy_1974_fe";

/// Its file.
pub const NEAR_FILE: &str = "Johnson.yml";

/// The dataset of iron's optical constants beyond the join: Ordal et al. 1988.
pub const FAR_DATASET: &str = "ordal_1988_fe";

/// Its file.
pub const FAR_FILE: &str = "Ordal.yml";

/// The fingerprint's marker for a regolith-form row without a fixed solid grain density: −1, no
/// density's value. Finite, so that the lock file records it and a recomputed fingerprint can match
/// it; a NaN matches nothing, and would leave the table stale for good.
pub const NO_FIXED_DENSITY: f64 = -1.0;

/// The bands' names, in the order of [`BAND_WAVELENGTHS_NM`].
const BANDS: [&str; 3] = ["B", "V", "R"];

/// Why the task could not compute its table.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitWeatheringError {
    /// A form's tabulated albedo does not rise strictly with w, which the law's inversion needs.
    #[error("the {form} form's {column} does not rise strictly with w at node {node}")]
    NotIncreasing {
        /// The form.
        form: &'static str,
        /// The column.
        column: String,
        /// The node at which it fails to rise.
        node: usize,
    },
    /// A form's energy albedo passes 1.
    #[error("the {form} form's {column} is {value} at node {node}, above 1")]
    AboveOne {
        /// The form.
        form: &'static str,
        /// The column.
        column: String,
        /// The node.
        node: usize,
        /// Its value.
        value: f64,
    },
    /// An iron dataset's file could not be read as a refractiveindex.info table.
    #[error("{dataset}'s file cannot be read: {why}")]
    ReadIndex {
        /// The dataset.
        dataset: &'static str,
        /// What is wrong.
        why: &'static str,
    },
    /// The iron datasets do not reach a wavelength the grid or the join needs.
    #[error("{dataset} does not reach {wavelength_nm} nm")]
    OffIndex {
        /// The dataset.
        dataset: &'static str,
        /// The wavelength, nm.
        wavelength_nm: f64,
    },
    /// A terrain's row is not in the registry.
    #[error("the registry has no row `{key}`")]
    UnknownRow {
        /// Its key.
        key: String,
    },
    /// A terrain's row has no condensed optics.
    #[error("`{key}` has no condensed optics")]
    NoOptics {
        /// Its key.
        key: String,
    },
    /// A terrain's row is not of the regolith reference form, whose Λ the fit calibrates.
    #[error("`{key}`'s reference form is not the regolith")]
    NotRegolith {
        /// Its key.
        key: String,
    },
    /// A terrain's row has no fixed solid grain density.
    #[error("`{key}` has no fixed solid grain density")]
    NoFixedDensity {
        /// Its key.
        key: String,
    },
    /// A value the fit produced is not finite.
    #[error("the fit's {what} is not finite")]
    NotFinite {
        /// Which.
        what: String,
    },
}

/// The task's input error for `error`.
#[must_use]
fn input(error: FitWeatheringError) -> RunTaskError {
    RunTaskError::Input {
        task: NAME,
        source: Box::new(error),
    }
}

/// The equivalent slab's `S_e` and `S_i` of a grain of real index `n` and k = 0: Hapke 2012's fits
/// to the Fresnel reflectances over a grain's surface, as `substance_albedos` takes them
/// (Davalos 2026, eqs. 9–11), with R(0) = ((n − 1) ÷ (n + 1))².
#[must_use]
pub fn slab(n: f64) -> [f64; 2] {
    let r0 = (n - 1.0) * (n - 1.0) / ((n + 1.0) * (n + 1.0));
    let s_e = 0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0;
    let s_i = 1.0 - (0.9413 - 0.8543 * r0 - 0.0870 * r0 * r0) / (n * n);
    [s_e, s_i]
}

/// The equivalent slab's mean path through a grain of real index `n` (at least 1) and diameter
/// `diameter_m`, m: ⅔ [n² − (n² − 1)^{3/2} ÷ n] D, as `substance_albedos` takes it.
#[must_use]
pub fn mean_path(n: f64, diameter_m: f64) -> f64 {
    let n2 = n * n;
    2.0 / 3.0 * (n2 - (n2 - 1.0) * (n2 - 1.0).sqrt() / n) * diameter_m
}

/// The host index of `form`: the surface crate's.
#[must_use]
pub const fn host_index(form: ReferenceForm) -> f64 {
    match form {
        ReferenceForm::Regolith => REGOLITH_HOST_INDEX,
        ReferenceForm::Frost => ICE_HOST_INDEX,
    }
}

/// `form`'s name, as its table's items begin.
#[must_use]
const fn form_name(form: ReferenceForm) -> &'static str {
    match form {
        ReferenceForm::Regolith => "regolith",
        ReferenceForm::Frost => "frost",
    }
}

/// The mean path of `form`'s grains, m: [`mean_path`] at its host index and grain diameter.
#[must_use]
pub fn form_mean_path(form: ReferenceForm) -> f64 {
    mean_path(host_index(form), form.grain_diameter().value())
}

/// `count` single-scattering albedos from 0 to 1, uniform in γ = √(1 − w): wᵢ = 1 − (1 − i ÷ (count
/// − 1))², exactly 0 first and 1 last.
///
/// # Panics
///
/// If `count` is below 2.
#[must_use]
pub fn nodes(count: u32) -> Vec<f64> {
    assert!(count >= 2, "the nodes need two ends, not {count}");
    let last = f64::from(count - 1);
    (0..count)
        .map(|i| {
            let gamma = 1.0 - f64::from(i) / last;
            1.0 - gamma * gamma
        })
        .collect()
}

/// A form's albedos at each node.
#[derive(Debug, Clone, PartialEq)]
pub struct Columns {
    /// `A_N`.
    pub normal: Vec<f64>,
    /// `A_S`.
    pub spherical: Vec<f64>,
    /// `A_h` at each of [`HEMISPHERICAL_INCIDENCE_COSINES`].
    pub hemispherical: Vec<[f64; 4]>,
}

/// `form`'s albedos at each of `nodes`: `A_N` by [`Medium`] under its photometry, `A_S` and `A_h`
/// by [`energy_albedos`] at its phase function's asymmetry.
#[must_use]
pub fn columns(form: ReferenceForm, nodes: &[f64]) -> Columns {
    let photometry = form.photometry();
    let asymmetry = photometry.phase_function().asymmetry();
    let mut out = Columns {
        normal: Vec::with_capacity(nodes.len()),
        spherical: Vec::with_capacity(nodes.len()),
        hemispherical: Vec::with_capacity(nodes.len()),
    };
    for &w in nodes {
        out.normal.push(Medium::new(w, photometry).normal_albedo());
        let (plane, sphere) = energy_albedos(w, asymmetry);
        out.spherical.push(sphere);
        out.hemispherical.push(plane);
    }
    out
}

/// Checks that each of `columns` rises strictly with w and that no energy albedo passes 1.
///
/// # Errors
///
/// [`FitWeatheringError::NotIncreasing`] or [`FitWeatheringError::AboveOne`], naming the column
/// and the node; a NaN fails the first.
pub fn check_columns(form: ReferenceForm, columns: &Columns) -> Result<(), FitWeatheringError> {
    let name = form_name(form);
    // The A_N and A_S columns are borrowed; each A_h column is gathered from its rows.
    let mut each: Vec<(String, Cow<'_, [f64]>, bool)> = vec![
        (
            "A_N".to_owned(),
            Cow::Borrowed(columns.normal.as_slice()),
            false,
        ),
        (
            "A_S".to_owned(),
            Cow::Borrowed(columns.spherical.as_slice()),
            true,
        ),
    ];
    for (incidence, mu0) in HEMISPHERICAL_INCIDENCE_COSINES.iter().enumerate() {
        each.push((
            format!("A_h at μ₀ = {mu0}"),
            Cow::Owned(columns.hemispherical.iter().map(|a| a[incidence]).collect()),
            true,
        ));
    }
    for (column, values, energy) in each {
        let rises = |pair: &[f64]| pair[1].partial_cmp(&pair[0]) == Some(Ordering::Greater);
        if let Some(node) = values.windows(2).position(|pair| !rises(pair)) {
            return Err(FitWeatheringError::NotIncreasing {
                form: name,
                column,
                node: node + 1,
            });
        }
        if energy && let Some(node) = values.iter().position(|&v| v > 1.0) {
            return Err(FitWeatheringError::AboveOne {
                form: name,
                column,
                node,
                value: values[node],
            });
        }
    }
    Ok(())
}

/// A metal's optical constants as a refractiveindex.info file tabulates them: rows of the
/// wavelength (here in nm, ascending), n and k.
#[derive(Debug, Clone, PartialEq)]
pub struct IronIndex {
    rows: Vec<[f64; 3]>,
}

impl IronIndex {
    /// The table of `yml`, a refractiveindex.info `tabulated nk` file of dataset `dataset`: the
    /// indented rows of λ (µm), n and k under its `data: |` key, the wavelengths taken to nm.
    ///
    /// # Errors
    ///
    /// [`FitWeatheringError::ReadIndex`] if there is no such block, a row is not three numbers,
    /// a value is not finite or a k not positive (ln k is interpolated), the wavelengths do not
    /// rise, or there are fewer than two rows.
    pub fn from_refractiveindex_yml(
        yml: &str,
        dataset: &'static str,
    ) -> Result<Self, FitWeatheringError> {
        let bad = |why| FitWeatheringError::ReadIndex { dataset, why };
        let (before, block) = yml
            .split_once("data: |")
            .ok_or_else(|| bad("it has no `data: |` block"))?;
        // The block holds the lines indented deeper than its key, as YAML's literal block does.
        let key_indent = before.len() - before.rfind('\n').map_or(0, |i| i + 1);
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut rows = Vec::new();
        for line in block
            .lines()
            .skip(1)
            .take_while(|l| l.trim().is_empty() || indent(l) > key_indent)
            .filter(|l| !l.trim().is_empty())
        {
            let fields: Vec<f64> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| bad("a row is not numbers"))?;
            let [um, n, k] = fields[..] else {
                return Err(bad("a row is not λ, n and k"));
            };
            if !(um.is_finite() && n.is_finite() && k.is_finite() && k > 0.0) {
                return Err(bad(
                    "a row's values are not finite, or its k is not positive",
                ));
            }
            rows.push([um * 1_000.0, n, k]);
        }
        if rows.len() < 2 {
            return Err(bad("it holds fewer than two rows"));
        }
        if !rows.windows(2).all(|pair| pair[0][0] < pair[1][0]) {
            return Err(bad("its wavelengths do not rise"));
        }
        Ok(Self { rows })
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether it has no rows: never, for one that was read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The first row's wavelength, nm.
    #[must_use]
    pub fn first_nm(&self) -> f64 {
        self.rows[0][0]
    }

    /// The last row's wavelength, nm.
    #[must_use]
    pub fn last_nm(&self) -> f64 {
        self.rows[self.rows.len() - 1][0]
    }

    /// n and k at `wavelength_nm`: a row's own at its wavelength, and between two rows n linear
    /// and ln k linear in λ; `None` outside the rows.
    #[must_use]
    pub fn at(&self, wavelength_nm: f64) -> Option<(f64, f64)> {
        let upper = self.rows.iter().position(|row| row[0] >= wavelength_nm)?;
        let [w1, n1, k1] = self.rows[upper];
        if w1.total_cmp(&wavelength_nm).is_eq() {
            return Some((n1, k1));
        }
        let [w0, n0, k0] = self.rows[upper.checked_sub(1)?];
        let t = (wavelength_nm - w0) / (w1 - w0);
        let n = n0 + t * (n1 - n0);
        let k = math::exp(math::ln(k0) + t * (math::ln(k1) - math::ln(k0)));
        Some((n, k))
    }
}

/// The Rayleigh absorption factor z of a small metal sphere of index n + ik in a host of real
/// index `host_index`: `n_h`³ n k ÷ [(n² − k² + 2 `n_h`²)² + (2 n k)²] (Hapke 2001, *from memory*;
/// decision-p14-t51d-weathering §1.1).
#[must_use]
pub fn iron_z(host_index: f64, n: f64, k: f64) -> f64 {
    let nh2 = host_index * host_index;
    let real = n * n - k * k + 2.0 * nh2;
    let imaginary = 2.0 * n * k;
    nh2 * host_index * n * k / (real * real + imaginary * imaginary)
}

/// The wavelength grid of iron's z: `count` nodes from `first_nm` every `step_nm`, nm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IronGrid {
    /// The first node, nm.
    pub first_nm: f64,
    /// The step, nm.
    pub step_nm: f64,
    /// The number of nodes.
    pub count: u32,
}

impl IronGrid {
    /// The wavelength of node `i`, nm.
    #[must_use]
    pub fn wavelength_nm(&self, i: u32) -> f64 {
        self.first_nm + self.step_nm * f64::from(i)
    }
}

/// Iron's z on a grid at one host index, and its join.
#[derive(Debug, Clone, PartialEq)]
pub struct IronZ {
    /// z at each node.
    pub z: Vec<f64>,
    /// The wavelength of the join, nm: the near dataset's last row.
    pub join_nm: f64,
    /// The far dataset's factor there, `z_near` ÷ `z_far`.
    pub join_factor: f64,
}

/// z at `wavelength_nm` from `index` at `host_index`, or [`FitWeatheringError::OffIndex`].
fn z_of(
    index: &IronIndex,
    dataset: &'static str,
    host_index: f64,
    wavelength_nm: f64,
) -> Result<f64, FitWeatheringError> {
    let (n, k) = index
        .at(wavelength_nm)
        .ok_or(FitWeatheringError::OffIndex {
            dataset,
            wavelength_nm,
        })?;
    Ok(iron_z(host_index, n, k))
}

/// Iron's z at `host_index` on `grid`: `near`'s (Johnson and Christy's) up to its last row and
/// `far`'s (Ordal et al.'s) beyond, times the join factor that meets `near` at that row.
///
/// # Errors
///
/// [`FitWeatheringError::OffIndex`] if `near` does not reach the grid's first node or `far` the
/// join or the grid's last node.
pub fn iron_z_grid(
    near: &IronIndex,
    far: &IronIndex,
    host_index: f64,
    grid: IronGrid,
) -> Result<IronZ, FitWeatheringError> {
    let join_nm = near.last_nm();
    let join_factor = z_of(near, NEAR_DATASET, host_index, join_nm)?
        / z_of(far, FAR_DATASET, host_index, join_nm)?;
    let z = (0..grid.count)
        .map(|i| {
            let wavelength_nm = grid.wavelength_nm(i);
            if wavelength_nm <= join_nm {
                z_of(near, NEAR_DATASET, host_index, wavelength_nm)
            } else {
                z_of(far, FAR_DATASET, host_index, wavelength_nm).map(|z| z * join_factor)
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(IronZ {
        z,
        join_nm,
        join_factor,
    })
}

/// One form's tables.
#[derive(Debug, Clone, PartialEq)]
pub struct FormTables {
    /// Its albedos at each node.
    pub columns: Columns,
    /// `S_e` and `S_i`.
    pub slab: [f64; 2],
    /// Iron's z.
    pub iron: IronZ,
}

impl FormTables {
    /// `form`'s tables on `nodes` and `grid`, iron's index `near` and `far`.
    ///
    /// # Errors
    ///
    /// As [`check_columns`] and [`iron_z_grid`].
    pub fn of(
        form: ReferenceForm,
        nodes: &[f64],
        near: &IronIndex,
        far: &IronIndex,
        grid: IronGrid,
    ) -> Result<Self, FitWeatheringError> {
        let columns = columns(form, nodes);
        check_columns(form, &columns)?;
        Ok(Self {
            columns,
            slab: slab(host_index(form)),
            iron: iron_z_grid(near, far, host_index(form), grid)?,
        })
    }
}

/// The surface crate's weathering law over one form's tables in memory, at a trial Λ: the same
/// operations in the same order as `hyperion_surface::material`'s, so that at the committed
/// table's Λ the two agree to the bit (a test holds it).
#[derive(Debug, Clone, Copy)]
pub struct Law<'a> {
    /// The nodes.
    pub nodes: &'a [f64],
    /// Iron's grid.
    pub grid: IronGrid,
    /// The form.
    pub form: &'a FormTables,
    /// Λ, m.
    pub lambda_m: f64,
}

impl Law<'_> {
    /// `A_N` at `w`, linear in w between the nodes.
    #[must_use]
    pub fn normal_at(&self, w: f64) -> f64 {
        let column = &self.form.columns.normal;
        let last = self.nodes.len() - 1;
        let above = self.nodes.partition_point(|&node| node <= w);
        if above == 0 {
            return column[0];
        }
        if above > last {
            return column[last];
        }
        let k = above - 1;
        let (a0, a1) = (column[k], column[k + 1]);
        a0 + (w - self.nodes[k]) / (self.nodes[k + 1] - self.nodes[k]) * (a1 - a0)
    }

    /// The w whose `A_N` is `fresh`: the column inverted linearly; 1 above its top, 0 at or below
    /// 0.
    #[must_use]
    pub fn grain_albedo_of(&self, fresh: f64) -> f64 {
        let column = &self.form.columns.normal;
        let last = self.nodes.len() - 1;
        if fresh <= 0.0 {
            return 0.0;
        }
        if fresh > column[last] {
            return 1.0;
        }
        let above = column.partition_point(|&a| a <= fresh);
        if above == 0 {
            return self.nodes[0];
        }
        if above > last {
            return self.nodes[last];
        }
        let k = above - 1;
        let (a0, a1) = (column[k], column[k + 1]);
        self.nodes[k] + (fresh - a0) / (a1 - a0) * (self.nodes[k + 1] - self.nodes[k])
    }

    /// Iron's z at `wavelength_nm`, linear between the grid's nodes and held beyond its ends.
    ///
    /// # Panics
    ///
    /// Never: the grid's node count is a `u32`, so a node's index is one too.
    #[must_use]
    pub fn iron_z_at(&self, wavelength_nm: f64) -> f64 {
        let z = &self.form.iron.z;
        let last = z.len() - 1;
        let last_node = f64::from(u32::try_from(last).expect("the grid's count is a u32"));
        let t = (wavelength_nm - self.grid.first_nm) / self.grid.step_nm;
        if t <= 0.0 {
            return z[0];
        }
        if t >= last_node {
            return z[last];
        }
        let below = t.floor();
        let k = node_index(below);
        z[k] + (t - below) * (z[k + 1] - z[k])
    }

    /// τ at `wavelength_nm` for grains of density `grain_density_kg_m3`, `FeO_T`
    /// `feo_mass_fraction` and maturity `maturity`.
    #[must_use]
    pub fn optical_depth(
        &self,
        wavelength_nm: f64,
        grain_density_kg_m3: f64,
        feo_mass_fraction: f64,
        maturity: f64,
    ) -> f64 {
        let iron =
            maturity * feo_mass_fraction * IRON_MOLAR_MASS_G_PER_MOL / FEO_MOLAR_MASS_G_PER_MOL;
        let volume = grain_density_kg_m3 / IRON_DENSITY_KG_M3;
        let absorption =
            36.0 * core::f64::consts::PI * self.iron_z_at(wavelength_nm) / (wavelength_nm * 1.0e-9);
        iron * volume * absorption * self.lambda_m
    }

    /// The weathered normal albedo of fresh `fresh` at `wavelength_nm`, as the surface crate's
    /// `weathered_normal_albedo` gives it under the form's law.
    #[must_use]
    pub fn weathered_normal_albedo(
        &self,
        fresh: f64,
        wavelength_nm: f64,
        grain_density_kg_m3: f64,
        feo_mass_fraction: f64,
        maturity: f64,
    ) -> f64 {
        let tau = self.optical_depth(
            wavelength_nm,
            grain_density_kg_m3,
            feo_mass_fraction,
            maturity,
        );
        if tau <= 0.0 {
            return fresh;
        }
        let w0 = self.grain_albedo_of(fresh);
        let [s_e, s_i] = self.form.slab;
        if w0 <= s_e {
            return fresh;
        }
        let excess = w0 - s_e;
        let theta0 = excess / ((1.0 - s_e) * (1.0 - s_i) + s_i * excess);
        let theta = theta0 * math::exp(-tau);
        let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
        if w >= w0 {
            return fresh;
        }
        let weathered = fresh * self.normal_at(w) / self.normal_at(w0);
        if weathered > fresh { fresh } else { weathered }
    }
}

/// The index of grid node `below`, a whole number from 0 to below the grid's last node, which the
/// caller bounds: cast to a `u32`, never through a `usize`'s saturation, then widened, as the surface crate takes it.
///
/// # Panics
///
/// Never on a target this crate builds for, whose `usize` holds any `u32`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "below is a whole number from 0 to below the grid's last node, a `u32` count, which a \
              u32 holds exactly: the caller compares it with the last node first"
)]
fn node_index(below: f64) -> usize {
    let node = below as u32;
    usize::try_from(node).expect("a usize holds a u32 on every target")
}

/// Every table of the law.
#[derive(Debug, Clone, PartialEq)]
pub struct Tables {
    /// The nodes.
    pub nodes: Vec<f64>,
    /// Iron's grid.
    pub grid: IronGrid,
    /// The regolith form's.
    pub regolith: FormTables,
    /// The frost form's.
    pub frost: FormTables,
}

impl Tables {
    /// The tables on `node_count` nodes and `grid`, iron's index `near` and `far`.
    ///
    /// # Errors
    ///
    /// As [`FormTables::of`].
    pub fn new(
        node_count: u32,
        grid: IronGrid,
        near: &IronIndex,
        far: &IronIndex,
    ) -> Result<Self, FitWeatheringError> {
        let nodes = nodes(node_count);
        let regolith = FormTables::of(ReferenceForm::Regolith, &nodes, near, far, grid)?;
        let frost = FormTables::of(ReferenceForm::Frost, &nodes, near, far, grid)?;
        Ok(Self {
            nodes,
            grid,
            regolith,
            frost,
        })
    }

    /// The regolith form's law at Λ `lambda_m`.
    #[must_use]
    pub fn regolith_law(&self, lambda_m: f64) -> Law<'_> {
        Law {
            nodes: &self.nodes,
            grid: self.grid,
            form: &self.regolith,
            lambda_m,
        }
    }
}

/// A fresh row of the registry, as the fit takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct FreshRow {
    /// Its key.
    pub key: String,
    /// w in B, V and R.
    pub w: [f64; 3],
    /// `A_N` in B, V and R.
    pub normal: [f64; 3],
    /// Its fixed solid grain density, kg m⁻³.
    pub density_kg_m3: f64,
}

/// The fresh row `key`: its condensed optics' w and `A_N` in B, V and R, and its fixed solid grain
/// density.
///
/// # Errors
///
/// [`FitWeatheringError::UnknownRow`], [`FitWeatheringError::NoOptics`],
/// [`FitWeatheringError::NotRegolith`] or [`FitWeatheringError::NoFixedDensity`].
pub fn fresh_row(key: &str) -> Result<FreshRow, FitWeatheringError> {
    let owned = || key.to_owned();
    let substance = by_key(key)
        .ok_or_else(|| FitWeatheringError::UnknownRow { key: owned() })?
        .substance();
    let optics = substance
        .condensed_optics()
        .ok_or_else(|| FitWeatheringError::NoOptics { key: owned() })?;
    if optics.reference_form() != ReferenceForm::Regolith {
        return Err(FitWeatheringError::NotRegolith { key: owned() });
    }
    Ok(FreshRow {
        key: owned(),
        w: optics.single_scattering_albedo_bvr(),
        normal: optics.normal_albedo_bvr(),
        density_kg_m3: fixed_solid_density(key)
            .ok_or_else(|| FitWeatheringError::NoFixedDensity { key: owned() })?,
    })
}

/// The fixed solid grain density of row `key`, kg m⁻³, if it has one: a rock's, at every
/// temperature (`GrainDensity::Fixed`), or a melt's solid beside its liquid by temperature
/// (`GrainDensity::MeasuredLiquid`), the same value at generator version 23 and 24.
#[must_use]
fn fixed_solid_density(key: &str) -> Option<f64> {
    match by_key(key)?.substance().condensed()?.grain_density() {
        GrainDensity::Fixed {
            solid: Some(solid), ..
        }
        | GrainDensity::MeasuredLiquid { solid, .. } => Some(solid.density().value()),
        GrainDensity::Fixed { solid: None, .. } | GrainDensity::OfDiagram => None,
    }
}

/// The colour law that takes an anchor's normal albedo from its measured wavelength to the bands:
/// A ∝ λ^e, Dollfus and Bowell 1971's whole-Moon spectral law (A&A 10, 29, Eq. 11, p. 50: log Ā
/// = 0.83 log λ − 0.80, λ in µm, so e = 0.83), of which only the slope is used. One colour for
/// both terrains; the terrains' mean B/V differ by < 1% at zero phase (Gehrels et al. 1964, Table
/// XIII); the maria's own ±3.5% spread is not modelled (decision-r10-t1d-anchor-colour).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colour {
    /// The wavelength the anchors are measured at, nm: Dollfus and Bowell's 600.
    pub anchor_wavelength_nm: f64,
    /// The exponent e.
    pub exponent: f64,
}

impl Colour {
    /// R(λ) ÷ R(λ₀) = (λ ÷ λ₀)^e at `wavelength_nm`, λ₀ the anchors' wavelength.
    #[must_use]
    pub fn ratio(&self, wavelength_nm: f64) -> f64 {
        math::powf(wavelength_nm / self.anchor_wavelength_nm, self.exponent)
    }
}

/// One terrain's anchor: its fresh row, the anchor soil's `FeO_T`, and its mature normal albedos'
/// centres and 10–90% bounds in B, V and R, each its value at the anchors' wavelength times
/// [`Colour::ratio`] at the band's.
#[derive(Debug, Clone, PartialEq)]
pub struct Terrain {
    /// Its name, `highland` or `mare`.
    pub name: &'static str,
    /// Its fresh row's key.
    pub row: String,
    /// The anchor soil's `FeO_T`.
    pub feo_mass_fraction: f64,
    /// The centres in B, V and R.
    pub centres: [f64; 3],
    /// The 10% bounds.
    pub p10: [f64; 3],
    /// The 90% bounds.
    pub p90: [f64; 3],
}

impl Terrain {
    /// σ in `band`: (P90 − P10) ÷ `spread_to_sigma`.
    #[must_use]
    pub fn sigma(&self, band: usize, spread_to_sigma: f64) -> f64 {
        (self.p90[band] - self.p10[band]) / spread_to_sigma
    }
}

/// A terrain with its fresh row, as the objective takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The anchor.
    pub terrain: Terrain,
    /// The fresh row.
    pub row: FreshRow,
}

/// The weathered `A_N` in B, V and R of `sample` at m = 1 under `law`.
#[must_use]
pub fn mature(law: &Law<'_>, sample: &Sample) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (band, value) in out.iter_mut().enumerate() {
        *value = law.weathered_normal_albedo(
            sample.row.normal[band],
            BAND_WAVELENGTHS_NM[band],
            sample.row.density_kg_m3,
            sample.terrain.feo_mass_fraction,
            1.0,
        );
    }
    out
}

/// The weighted sum of squares of `samples` at Λ `lambda_m`: Σ ((model − centre) ÷ σ)², terrains
/// in order and bands B, V, R within each.
#[must_use]
pub fn objective(tables: &Tables, samples: &[Sample], spread_to_sigma: f64, lambda_m: f64) -> f64 {
    let law = tables.regolith_law(lambda_m);
    let mut sum = 0.0;
    for sample in samples {
        let model = mature(&law, sample);
        for (band, value) in model.iter().enumerate() {
            let z = (value - sample.terrain.centres[band])
                / sample.terrain.sigma(band, spread_to_sigma);
            sum += z * z;
        }
    }
    sum
}

/// The search for Λ: its bracket, m, and its golden-section steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Search {
    /// The least Λ, m.
    pub lambda_min_m: f64,
    /// The greatest Λ, m.
    pub lambda_max_m: f64,
    /// Golden-section steps in ln Λ.
    pub iterations: u32,
}

/// The Λ, m, that minimises [`objective`] over `samples`: golden-section search in ln Λ over
/// `search`'s bracket.
#[must_use]
pub fn fit_lambda(
    tables: &Tables,
    samples: &[Sample],
    spread_to_sigma: f64,
    search: Search,
) -> f64 {
    let ln_lambda = golden_section(
        |ln_lambda| objective(tables, samples, spread_to_sigma, math::exp(ln_lambda)),
        math::ln(search.lambda_min_m),
        math::ln(search.lambda_max_m),
        search.iterations,
    );
    math::exp(ln_lambda)
}

/// The figures the run reports beside its fit, never fitted (the manifest's check parameters).
#[derive(Debug, Clone, PartialEq)]
pub struct Checks {
    /// Morris 1980's Eq. 4 slope as printed, `Fe⁰` ÷ `FeO` per unit of `I_s`/`FeO`: 3.2 × 10⁻⁴,
    /// per `FeO` (p. 1701). [`Checks::yield_centre`] takes it per `FeO`-iron.
    pub morris_slope: f64,
    /// The mature background's `I_s`/`FeO` at which it is read: T51.d's X.
    pub morris_index: f64,
    /// The plausibility window's factor about it.
    pub yield_factor: f64,
    /// The mature mare soils' median yield.
    pub yield_mare_median: f64,
    /// The mature highland soils' median yield.
    pub yield_highland_median: f64,
    /// Tycho's fresh/median contrast at 600 nm, its range.
    pub tycho: [f64; 2],
    /// Proclus's, its range.
    pub proclus: [f64; 2],
    /// Kepler's.
    pub kepler: f64,
}

impl Checks {
    /// The yield per `FeO`-iron that Morris 1980's Eq. 4 gives at the mature background's index:
    /// the slope per `FeO` × `M_FeO` ÷ `M_Fe` × X, 3.2 × 10⁻⁴ × 1.2865 × 75 = 0.0309, the slope per
    /// `FeO`-iron being 4.117 × 10⁻⁴.
    #[must_use]
    pub fn yield_centre(&self) -> f64 {
        self.morris_slope * FEO_MOLAR_MASS_G_PER_MOL / IRON_MOLAR_MASS_G_PER_MOL * self.morris_index
    }
}

/// The run's parameters, every one from the manifest's `[params]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The nodes in w.
    pub nodes: u32,
    /// Iron's grid.
    pub grid: IronGrid,
    /// The search for Λ.
    pub search: Search,
    /// The 10–90% width of a normal distribution in σ: 2.563.
    pub spread_to_sigma: f64,
    /// The colour law that takes the anchors to the bands.
    pub colour: Colour,
    /// The highland anchor.
    pub highland: Terrain,
    /// The mare anchor.
    pub mare: Terrain,
    /// The check figures.
    pub checks: Checks,
}

/// The finite, positive float parameter `key` of `manifest`.
///
/// # Errors
///
/// [`ManifestParamError`] if it is missing, not a number, or not finite and positive.
fn positive_param(manifest: &Manifest, key: &str) -> Result<f64, ManifestParamError> {
    let v = manifest.f64(key)?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(ManifestParamError::new(key, "finite and positive"))
    }
}

/// The anchor of terrain `name` from `manifest`: its row, its anchor soil's `FeO_T`, and its median
/// and 10–90% bounds at the anchors' wavelength, taken to B, V and R by `colour`.
///
/// # Errors
///
/// [`ManifestParamError`] for a key that is missing or out of range: an `FeO_T` outside [0, 1], a
/// value that is not finite and positive, or a median outside its P10 < P90.
fn terrain_params(
    manifest: &Manifest,
    name: &'static str,
    colour: Colour,
) -> Result<Terrain, ManifestParamError> {
    let feo_key = format!("{name}_feo_mass_fraction");
    let feo_mass_fraction = manifest.f64(&feo_key)?;
    if !(0.0..=1.0).contains(&feo_mass_fraction) {
        return Err(ManifestParamError::new(&feo_key, "in [0, 1]"));
    }
    let mut out = Terrain {
        name,
        row: manifest.str(&format!("{name}_row"))?.to_owned(),
        feo_mass_fraction,
        centres: [0.0; 3],
        p10: [0.0; 3],
        p90: [0.0; 3],
    };
    let median = positive_param(manifest, &format!("{name}_median"))?;
    let p10 = positive_param(manifest, &format!("{name}_p10"))?;
    let p90 = positive_param(manifest, &format!("{name}_p90"))?;
    if !(p10 < median && median < p90) {
        return Err(ManifestParamError::new(
            &format!("{name}_median"),
            "inside its P10 and P90",
        ));
    }
    for (band, wavelength_nm) in BAND_WAVELENGTHS_NM.into_iter().enumerate() {
        let ratio = colour.ratio(wavelength_nm);
        out.centres[band] = median * ratio;
        out.p10[band] = p10 * ratio;
        out.p90[band] = p90 * ratio;
    }
    Ok(out)
}

/// The keys of `[params]`, every one read.
#[must_use]
fn param_keys() -> Vec<String> {
    let mut keys: Vec<String> = [
        "nodes",
        "iron_z_first_nm",
        "iron_z_step_nm",
        "iron_z_nodes",
        "lambda_min_m",
        "lambda_max_m",
        "search_iterations",
        "spread_to_sigma",
        "anchor_wavelength_nm",
        "colour_exponent",
        "morris_slope",
        "morris_index",
        "yield_factor",
        "yield_mare_median",
        "yield_highland_median",
        "tycho_contrast_low",
        "tycho_contrast_high",
        "proclus_contrast_low",
        "proclus_contrast_high",
        "kepler_contrast",
    ]
    .map(str::to_owned)
    .to_vec();
    for terrain in ["highland", "mare"] {
        keys.push(format!("{terrain}_row"));
        keys.push(format!("{terrain}_feo_mass_fraction"));
        for what in ["median", "p10", "p90"] {
            keys.push(format!("{terrain}_{what}"));
        }
    }
    keys
}

impl Params {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a key that is missing, extra or out of range: counts of at least
    /// 2 nodes (at most 100,000), positive finite figures, a finite colour exponent, an `FeO_T` in
    /// [0, 1], a bracket with 0 < min < max, 1 to 10,000 steps, and each anchor's median inside its
    /// P10 < P90.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let keys = param_keys();
        if let Some(extra) = manifest.params().keys().find(|k| !keys.contains(k)) {
            return Err(ManifestParamError::new(
                extra,
                "absent: the task reads no such key",
            ));
        }
        let positive = |key: &str| positive_param(manifest, key);
        let count = |key: &str, least: u64, most: u64| -> Result<u32, ManifestParamError> {
            let n = manifest.u64(key)?;
            if n < least || n > most {
                return Err(ManifestParamError::new(
                    key,
                    &format!("an integer from {least} to {most}"),
                ));
            }
            u32::try_from(n).map_err(|_| ManifestParamError::new(key, "a u32"))
        };
        let exponent = manifest.f64("colour_exponent")?;
        if !exponent.is_finite() {
            return Err(ManifestParamError::new("colour_exponent", "finite"));
        }
        let colour = Colour {
            anchor_wavelength_nm: positive("anchor_wavelength_nm")?,
            exponent,
        };
        let terrain = |name| terrain_params(manifest, name, colour);
        let search = Search {
            lambda_min_m: positive("lambda_min_m")?,
            lambda_max_m: positive("lambda_max_m")?,
            iterations: count("search_iterations", 1, 10_000)?,
        };
        if search.lambda_min_m >= search.lambda_max_m {
            return Err(ManifestParamError::new(
                "lambda_min_m and lambda_max_m",
                "min < max",
            ));
        }
        Ok(Self {
            nodes: count("nodes", 2, 100_000)?,
            grid: IronGrid {
                first_nm: positive("iron_z_first_nm")?,
                step_nm: positive("iron_z_step_nm")?,
                count: count("iron_z_nodes", 2, 100_000)?,
            },
            search,
            spread_to_sigma: positive("spread_to_sigma")?,
            colour,
            highland: terrain("highland")?,
            mare: terrain("mare")?,
            checks: Checks {
                morris_slope: positive("morris_slope")?,
                morris_index: positive("morris_index")?,
                yield_factor: positive("yield_factor")?,
                yield_mare_median: positive("yield_mare_median")?,
                yield_highland_median: positive("yield_highland_median")?,
                tycho: [
                    positive("tycho_contrast_low")?,
                    positive("tycho_contrast_high")?,
                ],
                proclus: [
                    positive("proclus_contrast_low")?,
                    positive("proclus_contrast_high")?,
                ],
                kepler: positive("kepler_contrast")?,
            },
        })
    }
}

/// One terrain's result.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainFit {
    /// The terrain and its row.
    pub sample: Sample,
    /// Its weathered `A_N` at m = 1 under the fitted Λ, in B, V and R.
    pub mature: [f64; 3],
    /// The Λ that fits this terrain alone, m.
    pub own_lambda_m: f64,
}

/// The fit.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// The tables.
    pub tables: Tables,
    /// The regolith's Λ, m.
    pub lambda_m: f64,
    /// The frost's Λ, m.
    pub frost_lambda_m: f64,
    /// The highlands.
    pub highland: TerrainFit,
    /// The maria.
    pub mare: TerrainFit,
    /// The largest difference, over both rows and bands, between a row's own w and the w its
    /// `A_N` inverts to on the tables.
    pub inversion_error: f64,
}

/// The iron index of dataset `name`, file `file`, from `manifest`.
fn load_index(
    manifest: &Manifest,
    name: &'static str,
    file: &str,
) -> Result<IronIndex, RunTaskError> {
    let dataset = manifest.load_dataset(name)?;
    let bytes = dataset
        .files
        .iter()
        .find(|(n, _)| n == file)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| {
            input(FitWeatheringError::ReadIndex {
                dataset: name,
                why: "its file is missing",
            })
        })?;
    let text = std::str::from_utf8(bytes).map_err(|_| {
        input(FitWeatheringError::ReadIndex {
            dataset: name,
            why: "its file is not UTF-8",
        })
    })?;
    IronIndex::from_refractiveindex_yml(text, name).map_err(input)
}

/// The fit of `params` on the tables `tables`.
///
/// # Errors
///
/// [`FitWeatheringError`] for a row the fit cannot take, or a result that is not finite.
pub fn fit_on(tables: Tables, params: &Params) -> Result<Fit, FitWeatheringError> {
    let sample = |terrain: &Terrain| -> Result<Sample, FitWeatheringError> {
        Ok(Sample {
            terrain: terrain.clone(),
            row: fresh_row(&terrain.row)?,
        })
    };
    let samples = [sample(&params.highland)?, sample(&params.mare)?];
    let lambda_m = fit_lambda(&tables, &samples, params.spread_to_sigma, params.search);
    let frost_lambda_m =
        lambda_m * form_mean_path(ReferenceForm::Frost) / form_mean_path(ReferenceForm::Regolith);
    let law = tables.regolith_law(lambda_m);
    let mut inversion_error: f64 = 0.0;
    for sample in &samples {
        for band in 0..3 {
            let w = law.grain_albedo_of(sample.row.normal[band]);
            inversion_error = inversion_error.max((w - sample.row.w[band]).abs());
        }
    }
    let terrain_fit = |sample: &Sample| TerrainFit {
        sample: sample.clone(),
        mature: mature(&law, sample),
        own_lambda_m: fit_lambda(
            &tables,
            core::slice::from_ref(sample),
            params.spread_to_sigma,
            params.search,
        ),
    };
    let highland = terrain_fit(&samples[0]);
    let mare = terrain_fit(&samples[1]);
    let figures = [
        ("Λ", lambda_m),
        ("frost Λ", frost_lambda_m),
        ("highland Λ", highland.own_lambda_m),
        ("mare Λ", mare.own_lambda_m),
        ("inversion error", inversion_error),
    ];
    for (what, value) in figures
        .into_iter()
        .chain(highland.mature.map(|v| ("highland albedo", v)))
        .chain(mare.mature.map(|v| ("mare albedo", v)))
    {
        if !value.is_finite() {
            return Err(FitWeatheringError::NotFinite {
                what: what.to_owned(),
            });
        }
    }
    Ok(Fit {
        tables,
        lambda_m,
        frost_lambda_m,
        highland,
        mare,
        inversion_error,
    })
}

/// The fit of `manifest`.
///
/// # Errors
///
/// [`RunTaskError::Param`] for a bad parameter, [`RunTaskError::Dataset`] for an iron dataset that
/// cannot be read, and [`RunTaskError::Input`] as [`Tables::new`] and [`fit_on`].
pub fn fit(manifest: &Manifest) -> Result<(Fit, Params), RunTaskError> {
    let params = Params::of(manifest)?;
    let near = load_index(manifest, NEAR_DATASET, NEAR_FILE)?;
    let far = load_index(manifest, FAR_DATASET, FAR_FILE)?;
    let tables = Tables::new(params.nodes, params.grid, &near, &far).map_err(input)?;
    let fit = fit_on(tables, &params).map_err(input)?;
    Ok((fit, params))
}

/// `values` as B/V/R text at `places` decimals.
#[must_use]
fn bvr(values: [f64; 3], places: usize) -> String {
    values
        .iter()
        .map(|v| format!("{v:.places$}"))
        .collect::<Vec<_>>()
        .join("/")
}

/// Writes `terrain`'s part of the acceptance line into `out`: its row, its fresh and mature
/// albedos against its anchor with each z and whether it lies inside its bounds, and its own Λ.
fn write_terrain(out: &mut String, terrain: &TerrainFit, spread_to_sigma: f64) {
    let t = &terrain.sample.terrain;
    let row = &terrain.sample.row;
    let mut z = [0.0; 3];
    let mut inside = ["out"; 3];
    for band in 0..3 {
        z[band] = (terrain.mature[band] - t.centres[band]) / t.sigma(band, spread_to_sigma);
        if t.p10[band] <= terrain.mature[band] && terrain.mature[band] <= t.p90[band] {
            inside[band] = "in";
        }
    }
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "; {} `{}` at `FeO_T` {}, {} kg m⁻³: fresh {}, mature {} against {} (z {}; {} of \
         {:.3}–{:.3}, {:.3}–{:.3}, {:.3}–{:.3}), its own best Λ {:.4} µm",
        t.name,
        row.key,
        t.feo_mass_fraction,
        row.density_kg_m3,
        bvr(row.normal, 3),
        bvr(terrain.mature, 4),
        bvr(t.centres, 3),
        z.map(|v| format!("{v:+.2}")).join("/"),
        inside.join("/"),
        t.p10[0],
        t.p90[0],
        t.p10[1],
        t.p90[1],
        t.p10[2],
        t.p90[2],
        terrain.own_lambda_m * 1.0e6,
    );
}

/// The run's acceptance line (the [module documentation](self), "The checks").
#[must_use]
pub fn acceptance(fit: &Fit, params: &Params) -> String {
    let mut out = String::new();
    let regolith_path = form_mean_path(ReferenceForm::Regolith);
    let frost_path = form_mean_path(ReferenceForm::Frost);
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "Λ {:.4} µm, weighted least squares at m = 1 over B, V and R of both terrains (σ = (P90 − \
         P10) ÷ {}), the anchors at {} nm taken to the bands by A ∝ λ^{} (Dollfus and Bowell's Eq. \
         11, p. 50, one colour for both terrains)",
        fit.lambda_m * 1.0e6,
        params.spread_to_sigma,
        params.colour.anchor_wavelength_nm,
        params.colour.exponent,
    );
    for terrain in [&fit.highland, &fit.mare] {
        write_terrain(&mut out, terrain, params.spread_to_sigma);
    }
    let checks = &params.checks;
    let y_sat = fit.lambda_m / regolith_path;
    let centre = checks.yield_centre();
    let (low, high) = (centre / checks.yield_factor, centre * checks.yield_factor);
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "; `y_sat` = Λ ÷ ⟨D⟩ ({:.2} µm) {y_sat:.4} per unit FeO-iron, {} Morris 1980 Eq. 4's \
         {:.1e} per `FeO` (× `M_FeO` ÷ `M_Fe`, {:.3e} per FeO-iron) × {} = {centre:.4} within a factor {} \
         ({low:.4}–{high:.4}), beside the mature soils' medians, mare {:.3} and highland {:.3}",
        regolith_path * 1.0e6,
        if (low..=high).contains(&y_sat) {
            "inside"
        } else {
            "outside"
        },
        checks.morris_slope,
        checks.morris_slope * FEO_MOLAR_MASS_G_PER_MOL / IRON_MOLAR_MASS_G_PER_MOL,
        checks.morris_index,
        checks.yield_factor,
        checks.yield_mare_median,
        checks.yield_highland_median,
    );
    let contrast = |t: &TerrainFit| -> [f64; 3] {
        core::array::from_fn(|band| t.sample.row.normal[band] / t.mature[band])
    };
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "; fresh/mature contrast B/V/R highland {} and mare {}, against fresh craters over the \
         medians at 600 nm (Tycho {}–{}, Proclus {}–{}, Kepler {}; Dollfus and Bowell, Table 4)",
        bvr(contrast(&fit.highland), 2),
        bvr(contrast(&fit.mare), 2),
        checks.tycho[0],
        checks.tycho[1],
        checks.proclus[0],
        checks.proclus[1],
        checks.kepler,
    );
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "; iron's z joined at {} nm, Ordal × {:.4} at `n_h` {REGOLITH_HOST_INDEX} and × {:.4} \
         at `n_h` {ICE_HOST_INDEX}; the frost's Λ {:.4} µm (⟨D⟩ {:.2} µm); each row's `A_N` \
         inverts to its own w within {:.1e}",
        fit.tables.regolith.iron.join_nm,
        fit.tables.regolith.iron.join_factor,
        fit.tables.frost.iron.join_factor,
        fit.frost_lambda_m * 1.0e6,
        frost_path * 1.0e6,
        fit.inversion_error,
    );
    out
}

/// The items of one form's tables, its Λ given.
#[must_use]
fn form_items(
    form: ReferenceForm,
    tables: &FormTables,
    grid: IronGrid,
    lambda_m: f64,
    lambda_doc: Vec<String>,
) -> Vec<TableItem> {
    let prefix = form_name(form).to_ascii_uppercase();
    let what = match form {
        ReferenceForm::Regolith => "The lunar regolith form's",
        ReferenceForm::Frost => "The fresh frost form's",
    };
    // The photometry's source, as two doc lines, so that the table's lines keep to 100 columns.
    let [photometry, computed] = match form {
        ReferenceForm::Regolith => [
            "Helfenstein and Veverka 1987's Legendre phase function and shadow-hiding",
            "opposition effect, as `substance_albedos` computes a row's.",
        ],
        ReferenceForm::Frost => [
            "Domingue, Hartman and Verbiscer 1997's Henyey–Greenstein lobe, as",
            "`substance_albedos` computes a row's.",
        ],
    };
    let host = host_index(form);
    vec![
        TableItem::Array {
            name: format!("{prefix}_NORMAL_ALBEDO"),
            doc: vec![
                format!("{what} normal albedo `A_N` at each node of [`W`]: the reflectance"),
                "factor with isotropic multiple scattering and the exact H-function under"
                    .to_owned(),
                photometry.to_owned(),
                computed.to_owned(),
            ],
            values: tables.columns.normal.clone(),
        },
        TableItem::Array {
            name: format!("{prefix}_SPHERICAL_ALBEDO"),
            doc: vec![
                format!("{what} spherical albedo `A_S` at each node of [`W`], by van de Hulst's"),
                "similarity relation, as `substance_albedos` computes a row's.".to_owned(),
            ],
            values: tables.columns.spherical.clone(),
        },
        TableItem::Nested {
            name: format!("{prefix}_HEMISPHERICAL_ALBEDO"),
            doc: vec![
                format!("{what} directional-hemispherical albedos `A_h` at each node of [`W`], at"),
                "each of [`HEMISPHERICAL_INCIDENCE_COSINES`], by the similarity relation."
                    .to_owned(),
            ],
            rows: tables
                .columns
                .hemispherical
                .iter()
                .map(|r| r.to_vec())
                .collect(),
        },
        TableItem::Array {
            name: format!("{prefix}_SLAB"),
            doc: vec![
                format!(
                    "{what} equivalent slab, `S_e` and `S_i`, at the host index {host} and k = 0"
                ),
                "(Hapke 2012's fits).".to_owned(),
            ],
            values: tables.slab.to_vec(),
        },
        TableItem::Array {
            name: format!("{prefix}_IRON_Z"),
            doc: vec![
                "Iron's z = `n_h`³ n k ÷ [(n² − k² + 2 `n_h`²)² + (2 n k)²] at the form's host"
                    .to_owned(),
                format!(
                    "index `n_h` {host}, at {} nodes from [`IRON_Z_FIRST_NM`] every",
                    grid.count
                ),
                format!(
                    "[`IRON_Z_STEP_NM`]: Johnson and Christy 1974 to {} nm, Ordal et al. 1988",
                    tables.iron.join_nm
                ),
                format!("× {:.6} beyond.", tables.iron.join_factor),
            ],
            values: tables.iron.z.clone(),
        },
        TableItem::Scalar {
            name: format!("{prefix}_LAMBDA_M"),
            doc: lambda_doc,
            value: lambda_m,
        },
    ]
}

/// The table's contents for `fit`.
#[must_use]
pub fn render(fit: &Fit) -> RustTable {
    let tables = &fit.tables;
    let regolith_path = form_mean_path(ReferenceForm::Regolith);
    let frost_path = form_mean_path(ReferenceForm::Frost);
    let mut items = vec![
        TableItem::Array {
            name: "W".to_owned(),
            doc: vec![
                "The single-scattering albedos w of the nodes, ascending from 0 to 1, uniform in"
                    .to_owned(),
                format!("√(1 − w): wᵢ = 1 − (1 − i ÷ {})².", tables.nodes.len() - 1),
            ],
            values: tables.nodes.clone(),
        },
        TableItem::Array {
            name: "HEMISPHERICAL_INCIDENCE_COSINES".to_owned(),
            doc: vec![
                "The incidence cosines μ₀ of each row of a form's directional-hemispherical"
                    .to_owned(),
                "albedos: the sim's.".to_owned(),
            ],
            values: HEMISPHERICAL_INCIDENCE_COSINES.to_vec(),
        },
        TableItem::Scalar {
            name: "IRON_Z_FIRST_NM".to_owned(),
            doc: vec!["The wavelength of iron's z's first node, nm.".to_owned()],
            value: tables.grid.first_nm,
        },
        TableItem::Scalar {
            name: "IRON_Z_STEP_NM".to_owned(),
            doc: vec!["The step of iron's z's nodes, nm.".to_owned()],
            value: tables.grid.step_nm,
        },
    ];
    items.extend(form_items(
        ReferenceForm::Regolith,
        &tables.regolith,
        tables.grid,
        fit.lambda_m,
        vec![
            "Λ = `y_sat` ⟨D⟩ of the regolith form, m: fitted to the Moon's mature highland and mare"
                .to_owned(),
            "normal albedos in B, V and R (the header's acceptance).".to_owned(),
        ],
    ));
    items.extend(form_items(
        ReferenceForm::Frost,
        &tables.frost,
        tables.grid,
        fit.frost_lambda_m,
        vec![format!(
            "Λ of the frost form, m: the regolith's × ⟨D⟩_frost ÷ ⟨D⟩_regolith ({:.4} ÷ {:.4} µm).",
            frost_path * 1.0e6,
            regolith_path * 1.0e6
        )],
    ));
    RustTable {
        summary: vec![
            "The space-weathering law's tables (rendering plan R10, R10.T1.d;".to_owned(),
            "decision-p14-t51d-weathering §1), which `material`'s weathering reads.".to_owned(),
        ],
        notes: vec![
            "For each reference form: its normal, spherical and directional-hemispherical albedos"
                .to_owned(),
            "at each single-scattering albedo of [`W`] (read linearly in w, and inverted so), its"
                .to_owned(),
            "equivalent slab, iron's z on its wavelength grid (read linearly in λ, the end node"
                .to_owned(),
            "held beyond the grid) and Λ, the npFe⁰ yield times the form's mean path.".to_owned(),
        ],
        items,
    }
}

/// The task: R10.T1.d's space-weathering law, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SpaceWeatheringTask;

impl FitTask for SpaceWeatheringTask {
    fn name(&self) -> &'static str {
        NAME
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_crate(&self) -> TableCrate {
        TableCrate::Surface
    }

    fn table_path(&self) -> &'static str {
        "space_weathering.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The sim's and the surface crate's values the table is made of, so that a change to any
    /// finds it stale (the iron datasets' are the inputs hash's):
    ///
    /// - the bands' wavelengths and the hemispherical albedos' incidence cosines;
    /// - each reference form's grain diameter, phase-function parameters, asymmetry and opposition
    ///   effect, and its opposition amplitude at w = 0.1 and 0.9;
    /// - the host indices, iron's density and the molar masses;
    /// - every regolith-form row's w and `A_N` in B, V and R and its fixed solid density
    ///   ([`NO_FIXED_DENSITY`] for none), since the fingerprint cannot know which two rows the manifest names: a change to
    ///   any row of the form finds the table stale, conservatively.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::new();
        for (band, wavelength) in BANDS.into_iter().zip(BAND_WAVELENGTHS_NM) {
            probes.push((format!("{band} wavelength nm"), wavelength));
        }
        for (incidence, mu0) in HEMISPHERICAL_INCIDENCE_COSINES.into_iter().enumerate() {
            probes.push((format!("incidence {incidence} cosine"), mu0));
        }
        for form in [ReferenceForm::Frost, ReferenceForm::Regolith] {
            let label = form_name(form);
            let photometry = form.photometry();
            probes.push((
                format!("{label} grain diameter m"),
                form.grain_diameter().value(),
            ));
            match photometry.phase_function() {
                PhaseFunction::HenyeyGreenstein { asymmetry } => {
                    probes.push((format!("{label} Henyey-Greenstein asymmetry"), asymmetry));
                }
                PhaseFunction::Legendre { b, c } => {
                    probes.push((format!("{label} Legendre b"), b));
                    probes.push((format!("{label} Legendre c"), c));
                }
            }
            if let OppositionEffect::ShadowHiding {
                surface_share,
                width,
            } = photometry.opposition()
            {
                probes.push((format!("{label} S(0)"), surface_share));
                probes.push((format!("{label} h"), width));
            }
            probes.push((
                format!("{label} asymmetry"),
                photometry.phase_function().asymmetry(),
            ));
            for w in [0.1, 0.9] {
                probes.push((
                    format!("{label} B0 at w {w}"),
                    photometry.opposition_amplitude(w),
                ));
            }
            probes.push((format!("{label} host index"), host_index(form)));
        }
        probes.push(("iron density kg m-3".to_owned(), IRON_DENSITY_KG_M3));
        probes.push((
            "iron molar mass g mol-1".to_owned(),
            IRON_MOLAR_MASS_G_PER_MOL,
        ));
        probes.push((
            "FeO molar mass g mol-1".to_owned(),
            FEO_MOLAR_MASS_G_PER_MOL,
        ));
        for (id, row) in optics::rows() {
            if row.reference_form() != ReferenceForm::Regolith {
                continue;
            }
            let key = id.substance().key();
            for (band, (w, a)) in BANDS.into_iter().zip(
                row.single_scattering_albedo_bvr()
                    .into_iter()
                    .zip(row.normal_albedo_bvr()),
            ) {
                probes.push((format!("{key} w in {band}"), w));
                probes.push((format!("{key} A_N in {band}"), a));
            }
            probes.push((
                format!("{key} solid density kg m-3"),
                fixed_solid_density(key.as_str()).unwrap_or(NO_FIXED_DENSITY),
            ));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let (fit, params) = fit(manifest)?;
        let acceptance = acceptance(&fit, &params);
        Ok(TaskOutput {
            table: render(&fit),
            source: "Hapke 2001's nanophase-iron absorption in the equivalent slab (as \
                     decision-p14-t51d-weathering §1.1 re-derives it), on each reference form's \
                     albedos by `substance_albedos`' Hapke solution and Hapke 2012's slab; iron \
                     from Johnson and Christy 1974 (Phys. Rev. B 9, 5056) and Ordal et al. 1988 \
                     (Appl. Opt. 27, 1203) via refractiveindex.info; Λ fitted to Dollfus and \
                     Bowell 1971's (A&A 10, 29, Table 4) mature highland and mare normal albedos \
                     at 600 nm (catalogues at phase 1.5–12°, not zero-phase: the addendum's D6) \
                     taken to B, V and R by their whole-Moon spectral law (Eq. 11, p. 50, A ∝ \
                     λ^0.83, one colour for both terrains; decision-r10-t1d-anchor-colour), at the \
                     anchor soils' `FeO_T` of Morris 1980 (Proc. LPSC 11, 1697, Table 1), checked \
                     against its Eq. 4 (decision-p14-t51d-weathering, addendum 2026-10-19)"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests;
