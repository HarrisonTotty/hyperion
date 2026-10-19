//! Aerosol extinction at 550 nm (galaxy plan 14, P14.T49.e): each aerosol material's area-weighted
//! extinction efficiency and single-scattering albedo over Hansen and Travis's gamma and log-normal
//! size distributions, which `hyperion_sim::substance::aerosol` turns into a mode's mass extinction
//! with the row's grain density.
//!
//! # What it reads
//!
//! Every one of rendering plan R08's committed index files whose key is an aerosol material
//! (`ClientOptics::aerosol_material`), read where it lies ([`crate::materials`]), its data hash in
//! the inputs hash. The index at 550 nm is found by wavelength on the file's own grid
//! ([`index_at`]), never by position: two files' grids start at 360 nm, the rest at 380 nm. Every
//! shape class is a Mie sphere at that index ([`mie`]): a `nonSphericalMineral` or `crystal` file
//! is the sphere of equal volume to projected area (science-r08-mmf.md §2).
//!
//! # What it computes
//!
//! - **One shared fine radius grid per file**, even in ln r with step [`FINE_STEP`] from x =
//!   [`MIN_SIZE_PARAMETER`] to [`MAX_SIZE_PARAMETER`] (x = 2πr ÷ λ at λ = 550 nm in vacuum; the
//!   medium is taken as index 1, a relative error of 3 × 10⁻⁴ in air at the surface and less above,
//!   as the client's quadrature takes it). Each node holds the mean `Q_ext` and `Q_sca` of the
//!   spheres across its cell, from sub-samples spaced at most [`SPIKE_SPACING`] in x to x =
//!   [`SPIKE_REACH`] and [`RIPPLE_SPACING`] to [`RIPPLE_REACH`] (`sub_samples`): the first keeps a
//!   lossless sphere's narrow low-order resonances (P. Chýlek, J. T. Kiehl and M. K. W. Ko, "Narrow
//!   resonance structure in the Mie scattering characteristics", Appl. Opt. 17, 3019–3021, 1978), the
//!   second averages its ripple within each cell, whose period is the distance between neighbouring
//!   resonances (P. Chýlek, J. Opt. Soc. Am. 66, 285–287, 1976), Δx ≈ 0.45–0.9 at these indices
//!   (Chýlek, J. Opt. Soc. Am. A 7, 1609–1613, 1990). Beyond, the ripple is left to the
//!   distributions, at least √0.04 wide in ln r. Mie is computed once per sub-sample and file.
//! - **The size averages** are sums over that grid: a distribution's area weight r² n(r) dr in u =
//!   ln(r ÷ `r_eff`) depends on u and `v_eff` alone (`ln_area_weight`), so one kernel per variance
//!   serves every radius, and a table radius is a fine node. `Q̄_ext` = Σ w `Q_ext` ÷ Σ w and 1 − ω =
//!   Σ w (`Q_ext` − `Q_sca`) ÷ Σ w `Q_ext`, with w cut below where the area weight falls under
//!   [`WEIGHT_FLOOR`] of its peak and above where both it and the Rayleigh scattering weight r⁶ n(r)
//!   do, as the client's `SIZE_WEIGHT_FLOOR` cuts. A file whose k is 0 at 550 nm has a co-albedo of
//!   exactly zero. The table holds ln ω = ln(1 − (1 − ω)), which keeps both ω and 1 − ω to its
//!   digits: ω where a small absorbing particle scatters little (ω ≪ 1, ln ω ≈ 3 ln x), 1 − ω where a
//!   weakly absorbing one is almost white (ln ω ≈ −(1 − ω)), and 0 exactly for a lossless file.
//! - **The grid** (`radius_offsets`, `variance_nodes`): `r_eff` from 1 nm, where a narrow mode is
//!   Rayleigh's to 2 × 10⁻⁴ (x = 0.011), to 101 µm for the gamma distribution, where `Q̄_ext` lies
//!   within 1% of the geometric limit 2 (x = 1,150; 1.3% at `v_eff` 0.45), and to 33 µm for the
//!   log-normal, whose heavier tail reaches the top of the fine grid sooner; `v_eff` from 0.04
//!   (Venus's mode 2′ is 0.045) to 0.45 for the gamma distribution and to 0.65 for the log-normal (a
//!   geometric standard deviation e^σ = 2.03, past the `σ_g` = 2 of Ackerman and Marley 2001, ApJ 556,
//!   872). The radius steps are 0.152 in ln r to 21 nm, 0.08 through the resonance region to 4.1 µm
//!   and 0.16 above; the variances, 12 and 14, are even in ln(`v_eff` + 0.05). Their density is the
//!   half-step check's: Lagrange's cubic through the table's rounded values
//!   ([`hyperion_sim::substance::aerosol::interpolate_grid`]) at every half step in either coordinate
//!   meets the direct value within [`HALVING_BOUND`] for `Q̄_ext` and for ω, for every file
//!   (`halving_check`; the task refuses otherwise).
//! - **The values** are rounded to [`SIGNIFICANT_DIGITS`] significant digits, 5 × 10⁻⁵ of each at
//!   most, which the half-step check includes.
//!
//! # The tables
//!
//! The 49 files' grids are some 3 MB at five digits, so they are split into seven tables of seven
//! files each, in file-name order, to keep each file under the repository's 500 kB limit, as the
//! reddening tables are split ([`PARTS`]). The first also holds the grid's axes. Each table
//! lists its files' entries (`hyperion_sim::substance::aerosol::ExtinctionEntry`): key, phase,
//! variant, stand-in flag and grids.

pub mod mie;

use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::substance::CondensedPhase;
use hyperion_sim::substance::aerosol::{
    EXTINCTION_WAVELENGTH_NM, GAMMA_VARIANCE_NODES, GridSpace, LOG_NORMAL_VARIANCE_NODES,
    SizeDistributionKind, interpolate_grid,
};

use self::mie::{
    ComplexIndex, ComputeMieError, MAX_SIZE_PARAMETER, MIN_SIZE_PARAMETER, efficiencies,
};
use crate::data::LoadDatasetError;
use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::materials::MaterialFile;
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};
use crate::tasks::substance_albedos::index_at;

/// The task's revision.
pub const VERSION: u32 = 0;

/// The fine grid's step in ln r: about 100 nodes per σ of the narrowest distribution (√ln 1.04 =
/// 0.198 in ln r), so its trapezoid sum is exact to rounding, and the table's steps are whole
/// multiples of it.
pub const FINE_STEP: f64 = 0.002;

/// The table's first effective radius, m: 1 nm.
pub const FIRST_RADIUS_M: f64 = 1e-9;

/// The table's radius steps: (count, step in [`FINE_STEP`]s), 0.152 in ln r to 21 nm, 0.08 to
/// 4.1 µm and 0.16 to 101 µm (x = 1,150). Each step is an even number of fine steps, so its half
/// is a node.
pub const RADIUS_SEGMENTS: [(usize, usize); 3] = [(20, 76), (66, 40), (20, 80)];

/// The gamma distribution's radius nodes: every node, to 101 µm.
pub const GAMMA_RADII: usize = 107;

/// The log-normal's radius nodes: the first 100, to 33 µm.
pub const LOG_NORMAL_RADII: usize = 100;

/// The offset of the variances' coordinate, ln(`v_eff` + offset): between ln `v_eff`, which crowds
/// the nodes at small variances, and ln(1 + `v_eff`), which crowds them at large ones; the half-step
/// check measured 0.05 the better of 0.03, 0.05, 0.08, 0.15 and 0.3.
pub const VARIANCE_OFFSET: f64 = 0.05;

/// The gamma distribution's variances: from 0.04 to 0.45.
pub const GAMMA_VARIANCES: (f64, f64) = (0.04, 0.45);

/// The log-normal's variances: from 0.04 to 0.65.
pub const LOG_NORMAL_VARIANCES: (f64, f64) = (0.04, 0.65);

/// The fraction of its own peak below which a size weight is cut, the client's
/// `SIZE_WEIGHT_FLOOR`: ±5.3σ for a Gaussian weight.
pub const WEIGHT_FLOOR: f64 = 1e-6;

/// The largest share of a cell's area or extinction that may lie off the fine grid, bounded by the
/// spectrum's largest `Q_ext` (beyond x = 20,000 a sphere's `Q_ext` is within 0.01 of 2).
pub const CLIP_TOLERANCE: f64 = 1e-6;

/// The half-step check's bound on the relative miss of `Q̄_ext` and ω.
pub const HALVING_BOUND: f64 = 1e-3;

/// The values' significant digits.
pub const SIGNIFICANT_DIGITS: usize = 5;

/// The widest spacing in x of the sub-samples to [`SPIKE_REACH`]: at 0.005, halving it moved
/// titanium dioxide's cells near x = 4, where its sharpest low-order resonances lie, by 7 × 10⁻⁴;
/// at 0.000 5 the convergence test holds every cell below 30 µm to 2.5 × 10⁻⁴, for 3.6 × 10⁶
/// terms of the series per file.
pub const SPIKE_SPACING: f64 = 0.000_5;

/// The size parameter to which the sub-samples keep [`SPIKE_SPACING`].
pub const SPIKE_REACH: f64 = 60.0;

/// The widest spacing in x of the sub-samples from [`SPIKE_REACH`] to [`RIPPLE_REACH`]: 4.5 to 9
/// a period of the ripple, Δx ≈ 0.45–0.9 over the files' n of 1.2–2.9 (P. Chýlek, "Resonance
/// structure of Mie scattering: distance between resonances", J. Opt. Soc. Am. A 7, 1609–1613,
/// 1990: Δx ≈ arctan √(m² − 1) ÷ √(m² − 1)).
pub const RIPPLE_SPACING: f64 = 0.1;

/// The size parameter beyond which each node is one sphere, a radius of 105 µm, just above the
/// table's last (101 µm, x = 1,150).
///
/// Beyond it the ripple is left to the distributions; doubling it moves titanium dioxide's cells
/// at 101 µm by 3.5 × 10⁻⁴, at twice the spectrum's cost.
pub const RIPPLE_REACH: f64 = 1_200.0;

/// The fine nodes per chunk of the parallel map.
const CHUNK: u64 = 32;

/// One table: its name, its file, its index files and whether it holds the grid's axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Part {
    /// The table's name, its task's.
    name: &'static str,
    /// Its file, in the sim's `src/tables/`.
    table_path: &'static str,
    /// Its index files, in name order.
    files: &'static [&'static str],
    /// Its items, in order.
    items: &'static [&'static str],
    /// Whether it holds the grid's axes (the first table).
    axes: bool,
}

impl Part {
    /// The table's name, its task's.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Its index files, in name order.
    #[must_use]
    pub const fn files(&self) -> &'static [&'static str] {
        self.files
    }
}

/// The four items of each file, from its prefix ([`item_prefix`]).
macro_rules! file_items {
    ($($prefix:literal),* $(,)?) => {
        &[$(
            concat!($prefix, "_GAMMA_EXTINCTION"),
            concat!($prefix, "_GAMMA_LN_ALBEDO"),
            concat!($prefix, "_LOG_NORMAL_EXTINCTION"),
            concat!($prefix, "_LOG_NORMAL_LN_ALBEDO"),
        )*]
    };
    (axes; $($prefix:literal),* $(,)?) => {
        &[
            "EFFECTIVE_RADII_M",
            "GAMMA_EFFECTIVE_VARIANCES",
            "LOG_NORMAL_EFFECTIVE_VARIANCES",
            "EFFECTIVE_VARIANCE_OFFSET",
            $(
                concat!($prefix, "_GAMMA_EXTINCTION"),
                concat!($prefix, "_GAMMA_LN_ALBEDO"),
                concat!($prefix, "_LOG_NORMAL_EXTINCTION"),
                concat!($prefix, "_LOG_NORMAL_LN_ALBEDO"),
            )*
        ]
    };
}

/// The seven tables, the aerosol files in name order, seven to a table.
pub const PARTS: [Part; 7] = [
    Part {
        name: "condensate_extinction",
        table_path: "condensate_extinction.rs",
        files: &[
            "aluminium-oxide.json",
            "ammonia-ice.json",
            "ammonia-liquid.json",
            "ammonium-hydrosulphide.json",
            "argon-ice.json",
            "argon-liquid.json",
            "basalt.json",
        ],
        items: file_items!(
            axes;
            "ALUMINIUM_OXIDE",
            "AMMONIA_ICE",
            "AMMONIA_LIQUID",
            "AMMONIUM_HYDROSULPHIDE",
            "ARGON_ICE",
            "ARGON_LIQUID",
            "BASALT",
        ),
        axes: true,
    },
    Part {
        name: "condensate_extinction_2",
        table_path: "condensate_extinction_2.rs",
        files: &[
            "basaltic-glass.json",
            "calcium-titanate.json",
            "carbon-dioxide-ice.json",
            "carbon-dioxide-liquid.json",
            "carbon-monoxide-ice.json",
            "carbon-monoxide-liquid.json",
            "chromium.json",
        ],
        items: file_items!(
            "BASALTIC_GLASS",
            "CALCIUM_TITANATE",
            "CARBON_DIOXIDE_ICE",
            "CARBON_DIOXIDE_LIQUID",
            "CARBON_MONOXIDE_ICE",
            "CARBON_MONOXIDE_LIQUID",
            "CHROMIUM",
        ),
        axes: false,
    },
    Part {
        name: "condensate_extinction_3",
        table_path: "condensate_extinction_3.rs",
        files: &[
            "enstatite-glass.json",
            "ethane-liquid.json",
            "forsterite-amorphous.json",
            "graphite.json",
            "hydrogen-sulphide-ice.json",
            "hydrogen-sulphide-liquid.json",
            "ice-giant-haze.json",
        ],
        items: file_items!(
            "ENSTATITE_GLASS",
            "ETHANE_LIQUID",
            "FORSTERITE_AMORPHOUS",
            "GRAPHITE",
            "HYDROGEN_SULPHIDE_ICE",
            "HYDROGEN_SULPHIDE_LIQUID",
            "ICE_GIANT_HAZE",
        ),
        axes: false,
    },
    Part {
        name: "condensate_extinction_4",
        table_path: "condensate_extinction_4.rs",
        files: &[
            "iron-sulphide.json",
            "iron.json",
            "jovian-chromophore.json",
            "manganese-sulphide.json",
            "mars-dust.json",
            "mars-regolith-simulant.json",
            "methane-ice.json",
        ],
        items: file_items!(
            "IRON_SULPHIDE",
            "IRON",
            "JOVIAN_CHROMOPHORE",
            "MANGANESE_SULPHIDE",
            "MARS_DUST",
            "MARS_REGOLITH_SIMULANT",
            "METHANE_ICE",
        ),
        axes: false,
    },
    Part {
        name: "condensate_extinction_5",
        table_path: "condensate_extinction_5.rs",
        files: &[
            "methane-liquid.json",
            "nitrogen-ice.json",
            "nitrogen-liquid.json",
            "oxygen-ice.json",
            "oxygen-liquid.json",
            "potassium-chloride.json",
            "seawater.json",
        ],
        items: file_items!(
            "METHANE_LIQUID",
            "NITROGEN_ICE",
            "NITROGEN_LIQUID",
            "OXYGEN_ICE",
            "OXYGEN_LIQUID",
            "POTASSIUM_CHLORIDE",
            "SEAWATER",
        ),
        axes: false,
    },
    Part {
        name: "condensate_extinction_6",
        table_path: "condensate_extinction_6.rs",
        files: &[
            "silicon-dioxide.json",
            "sodium-chloride.json",
            "sodium-sulphide.json",
            "soot.json",
            "sulphur-liquid.json",
            "sulphur.json",
            "sulphuric-acid-75.json",
        ],
        items: file_items!(
            "SILICON_DIOXIDE",
            "SODIUM_CHLORIDE",
            "SODIUM_SULPHIDE",
            "SOOT",
            "SULPHUR_LIQUID",
            "SULPHUR",
            "SULPHURIC_ACID_75",
        ),
        axes: false,
    },
    Part {
        name: "condensate_extinction_7",
        table_path: "condensate_extinction_7.rs",
        files: &[
            "sulphuric-acid-84.json",
            "sulphuric-acid-uv-absorber.json",
            "tholin.json",
            "titanium-dioxide.json",
            "water-ice.json",
            "water.json",
            "zinc-sulphide.json",
        ],
        items: file_items!(
            "SULPHURIC_ACID_84",
            "SULPHURIC_ACID_UV_ABSORBER",
            "THOLIN",
            "TITANIUM_DIOXIDE",
            "WATER_ICE",
            "WATER",
            "ZINC_SULPHIDE",
        ),
        axes: false,
    },
];

/// The table item prefix of file `name`: its stem in capitals, a hyphen as an underscore.
#[must_use]
pub(crate) fn item_prefix(name: &str) -> String {
    name.trim_end_matches(".json")
        .replace('-', "_")
        .to_ascii_uppercase()
}

/// The wavelength, m.
const WAVELENGTH_M: f64 = EXTINCTION_WAVELENGTH_NM * 1e-9;

/// The size parameter of radius `radius_m` at 550 nm.
#[must_use]
pub(crate) fn size_parameter(radius_m: f64) -> f64 {
    2.0 * std::f64::consts::PI * radius_m / WAVELENGTH_M
}

/// The fine grid: its nodes below the table's first radius and its node count.
///
/// Node j is at ln r = ln [`FIRST_RADIUS_M`] + (j − `below`) [`FINE_STEP`], from the last node at
/// or above x = [`MIN_SIZE_PARAMETER`] to the last at or below [`MAX_SIZE_PARAMETER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FineGrid {
    below: usize,
    nodes: usize,
}

/// The whole fine steps of ln r from `from` up to `to` (both positive), rounded down.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a non-negative count below 10,000 for the grid's radii, exact in a usize"
)]
#[must_use]
fn fine_steps_between(from: f64, to: f64) -> usize {
    ((math::ln(to) - math::ln(from)) / FINE_STEP)
        .floor()
        .max(0.0) as usize
}

/// The fine grid ([`FineGrid`]).
#[must_use]
pub(crate) fn fine_grid() -> FineGrid {
    let lowest = MIN_SIZE_PARAMETER * WAVELENGTH_M / (2.0 * std::f64::consts::PI);
    let highest = MAX_SIZE_PARAMETER * WAVELENGTH_M / (2.0 * std::f64::consts::PI);
    let below = fine_steps_between(lowest, FIRST_RADIUS_M);
    let above = fine_steps_between(FIRST_RADIUS_M, highest);
    FineGrid {
        below,
        nodes: below + above + 1,
    }
}

/// ln r of fine node `j`, m.
#[expect(
    clippy::cast_precision_loss,
    reason = "node indices below 2^53, exact in an f64"
)]
#[must_use]
pub(crate) fn fine_ln_radius(grid: FineGrid, j: usize) -> f64 {
    math::ln(FIRST_RADIUS_M) + (j as f64 - grid.below as f64) * FINE_STEP
}

/// The radius of fine node `j`, m: [`FIRST_RADIUS_M`] e^((j − `below`) [`FINE_STEP`]), the same
/// expression as the table's radii, so that a fine node that is a table radius has its bits.
#[expect(
    clippy::cast_precision_loss,
    reason = "node indices below 2^53, exact in an f64"
)]
#[must_use]
pub(crate) fn fine_radius_m(grid: FineGrid, j: usize) -> f64 {
    FIRST_RADIUS_M * math::exp((j as f64 - grid.below as f64) * FINE_STEP)
}

/// The fine-node offset of each table radius from the first, by [`RADIUS_SEGMENTS`].
#[must_use]
pub(crate) fn radius_offsets() -> Vec<usize> {
    let mut offsets = vec![0];
    let mut at = 0;
    for (count, step) in RADIUS_SEGMENTS {
        for _ in 0..count {
            at += step;
            offsets.push(at);
        }
    }
    offsets
}

/// The table's effective radii, m: [`FIRST_RADIUS_M`] e^(offset × [`FINE_STEP`]), the first
/// exactly 1 nm.
#[expect(
    clippy::cast_precision_loss,
    reason = "offsets below 2^53, exact in an f64"
)]
#[must_use]
pub(crate) fn effective_radii_m() -> Vec<f64> {
    radius_offsets()
        .iter()
        .map(|&offset| FIRST_RADIUS_M * math::exp(offset as f64 * FINE_STEP))
        .collect()
}

/// `count` (at least 2) effective variances from `range.0` to `range.1`, both exactly, even in
/// ln(`v_eff` + [`VARIANCE_OFFSET`]).
#[expect(
    clippy::cast_precision_loss,
    reason = "a node count below 100, exact in an f64"
)]
#[must_use]
fn variance_nodes(range: (f64, f64), count: usize) -> Vec<f64> {
    let (c0, c1) = (
        math::ln(range.0 + VARIANCE_OFFSET),
        math::ln(range.1 + VARIANCE_OFFSET),
    );
    let last = count.saturating_sub(1);
    (0..count)
        .map(|i| match i {
            0 => range.0,
            i if i == last => range.1,
            i => math::exp(c0 + (c1 - c0) * i as f64 / last as f64) - VARIANCE_OFFSET,
        })
        .collect()
}

/// The variance nodes of `kind` ([`GAMMA_VARIANCES`], [`LOG_NORMAL_VARIANCES`]).
#[must_use]
pub(crate) fn variances_of(kind: SizeDistributionKind) -> Vec<f64> {
    match kind {
        SizeDistributionKind::Gamma => variance_nodes(GAMMA_VARIANCES, GAMMA_VARIANCE_NODES),
        SizeDistributionKind::LogNormal => {
            variance_nodes(LOG_NORMAL_VARIANCES, LOG_NORMAL_VARIANCE_NODES)
        }
    }
}

/// The radius nodes `kind` uses ([`GAMMA_RADII`], [`LOG_NORMAL_RADII`]).
#[must_use]
pub(crate) const fn radii_of(kind: SizeDistributionKind) -> usize {
    match kind {
        SizeDistributionKind::Gamma => GAMMA_RADII,
        SizeDistributionKind::LogNormal => LOG_NORMAL_RADII,
    }
}

/// The sub-samples of a fine node at size parameter `x`: as many as keep their spacing across the
/// node's cell, x × [`FINE_STEP`], within [`SPIKE_SPACING`] to [`SPIKE_REACH`] and
/// [`RIPPLE_SPACING`] to [`RIPPLE_REACH`], and one beyond (or for a NaN).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a count from 1 to 240 for x up to RIPPLE_REACH, exact in a u32"
)]
#[must_use]
pub(crate) fn sub_samples(x: f64) -> u32 {
    let spacing = if x <= SPIKE_REACH {
        SPIKE_SPACING
    } else if x <= RIPPLE_REACH {
        RIPPLE_SPACING
    } else {
        return 1;
    };
    (x * FINE_STEP / spacing).ceil().max(1.0) as u32
}

/// Why a file's table could not be fitted.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitExtinctionError {
    /// The file's phase is neither `solid` nor `liquid`.
    #[error("its phase is {0}, not solid or liquid")]
    Phase(String),
    /// The file's grid does not reach 550 nm.
    #[error("its grid does not reach 550 nm")]
    NoIndexAt550,
    /// A sphere's optics could not be computed.
    #[error(transparent)]
    Mie(#[from] ComputeMieError),
    /// More than [`CLIP_TOLERANCE`] of a cell lies off the fine grid.
    #[error("{share:.2e} of the cell at fine node {centre} lies off the fine grid")]
    ClippedCell {
        /// The cell's fine node.
        centre: usize,
        /// The share of its area, or of its extinction, off the grid.
        share: f64,
    },
    /// A half step of the check lies off the table's grid, a broken invariant.
    #[error("the half step at r_eff {radius_m} m, v_eff {variance} lies off the table's grid")]
    HalfStepOffGrid {
        /// The half step's radius, m.
        radius_m: f64,
        /// Its variance.
        variance: f64,
    },
    /// A grid misses its half steps by more than [`HALVING_BOUND`].
    #[error(
        "the {kind} grid misses its half steps by {extinction:.2e} in Q_ext and {albedo:.2e} in \
         ω (at r_eff {radius_m:.4e} m, v_eff {variance:.4}), above 1e-3"
    )]
    MissedHalfStep {
        /// The family.
        kind: &'static str,
        /// The worst relative miss of `Q̄_ext`.
        extinction: f64,
        /// The worst relative miss of ω.
        albedo: f64,
        /// The worse of the two's radius, m.
        radius_m: f64,
        /// Its variance.
        variance: f64,
    },
    /// The manifest names other files than the part's.
    #[error("the manifest names other materials than {part}'s")]
    WrongMaterials {
        /// The part.
        part: &'static str,
    },
}

/// A file's [`FitExtinctionError`], with the file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{file}")]
pub struct FitFileError {
    /// The file's name.
    pub file: &'static str,
    /// Why.
    #[source]
    pub cause: FitExtinctionError,
}

/// `cause` for `file` as a task input error.
fn input_error(file: &'static str, cause: FitExtinctionError) -> RunTaskError {
    RunTaskError::Input {
        task: "condensate_extinction",
        source: Box::new(FitFileError { file, cause }),
    }
}

/// A file's spheres on the fine grid: each node's mean `Q_ext` and `Q_sca` across its cell, and
/// whether its index absorbs nothing (k = 0), whose co-albedo is then exactly 0.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Spectrum {
    q_ext: Vec<f64>,
    q_sca: Vec<f64>,
    largest_extinction: f64,
    lossless: bool,
}

/// The spectrum of index `index` on the fine grid (`sub_samples`), on `threads` threads.
///
/// # Errors
///
/// [`FitExtinctionError::Mie`], through [`RunTaskError::Input`], if a sphere's optics cannot be
/// computed, and [`RunTaskError::Threads`] if the pool cannot be built.
pub(crate) fn spectrum(
    index: ComplexIndex,
    threads: NonZeroUsize,
) -> Result<Spectrum, RunTaskError> {
    spectrum_with(index, threads, sub_samples)
}

/// The spectrum with `samples` sub-samples at each node's size parameter, for the convergence
/// test.
fn spectrum_with(
    index: ComplexIndex,
    threads: NonZeroUsize,
    samples: impl Fn(f64) -> u32 + Sync,
) -> Result<Spectrum, RunTaskError> {
    let grid = fine_grid();
    let mut out = Spectrum {
        lossless: index.k() <= 0.0,
        ..Spectrum::default()
    };
    let mut failure: Option<ComputeMieError> = None;
    let node_count = u64::try_from(grid.nodes).unwrap_or(u64::MAX);
    map_reduce_chunks(
        node_count,
        CHUNK,
        threads,
        |range| {
            range
                .map(|node| {
                    let node = usize::try_from(node)
                        .expect("a node index below the fine grid's 11,860 nodes fits a usize");
                    let ln_r = fine_ln_radius(grid, node);
                    let count = samples(size_parameter(math::exp(ln_r)));
                    let mut q_ext = 0.0;
                    let mut q_sca = 0.0;
                    for sample in 0..count {
                        let shift = (f64::from(sample) + 0.5) / f64::from(count) - 0.5;
                        let size = size_parameter(math::exp(ln_r + FINE_STEP * shift))
                            .clamp(MIN_SIZE_PARAMETER, MAX_SIZE_PARAMETER);
                        let sphere = efficiencies(size, index)?;
                        q_ext += sphere.q_ext();
                        q_sca += sphere.q_sca();
                    }
                    Ok((q_ext / f64::from(count), q_sca / f64::from(count)))
                })
                .collect::<Vec<_>>()
        },
        |chunk| {
            for node in chunk {
                match node {
                    Ok((q_ext, q_sca)) => {
                        out.q_ext.push(q_ext);
                        out.q_sca.push(q_sca);
                    }
                    Err(e) => {
                        failure.get_or_insert(e);
                    }
                }
            }
        },
    )?;
    if let Some(e) = failure {
        return Err(RunTaskError::Input {
            task: "condensate_extinction",
            source: Box::new(FitExtinctionError::Mie(e)),
        });
    }
    out.largest_extinction = out.q_ext.iter().copied().fold(0.0, f64::max);
    Ok(out)
}

/// ln of a distribution's area weight r² n(r) dr per unit u = ln(r ÷ `r_eff`), relative to its
/// peak (0 for both): for the gamma distribution (u − eᵘ + 1) ÷ `v_eff`, the area-weighted radius
/// being Gamma-distributed with shape 1 ÷ `v_eff` and mean `r_eff`; for the log-normal −(u +
/// σ²/2)² ÷ 2σ² with σ² = ln(1 + `v_eff`), a Gaussian in ln r about ln `r_eff` − σ²/2 (Hansen and
/// Travis 1974, (2.56) and (2.60), with ⟨r³⟩ ÷ ⟨r²⟩ = `r_eff`). `variance` is positive.
#[must_use]
pub(crate) fn ln_area_weight(kind: SizeDistributionKind, variance: f64, u: f64) -> f64 {
    match kind {
        SizeDistributionKind::Gamma => (u - math::exp(u) + 1.0) / variance,
        SizeDistributionKind::LogNormal => {
            let s2 = math::ln_1p(variance);
            let d = u + 0.5 * s2;
            -d * d / (2.0 * s2)
        }
    }
}

/// A distribution's weights on the fine grid, at offsets `first` … from the cell's radius.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Kernel {
    /// The offset of the first weight, in fine steps (at most 0).
    first: isize,
    /// The weights, unnormalised: e^(`ln_area_weight`).
    weights: Vec<f64>,
}

/// u at offset `i` fine steps.
#[expect(
    clippy::cast_precision_loss,
    reason = "offsets below 2^53 in magnitude, exact in an f64"
)]
#[must_use]
fn offset_u(i: isize) -> f64 {
    i as f64 * FINE_STEP
}

/// The kernel of `kind` at variance `variance` (one of the grid's, or a half step's: positive):
/// down to where the area weight falls below [`WEIGHT_FLOOR`] of its peak (the absorption and
/// scattering weights r³ n and r⁶ n fall faster there), and up to where both the area weight and
/// the Rayleigh scattering weight r⁶ n(r) do, the latter relative to its own peak.
#[must_use]
pub(crate) fn kernel(kind: SizeDistributionKind, variance: f64) -> Kernel {
    debug_assert!(variance > 0.0, "a kernel's variance {variance} is positive");
    // Both weights' ln peaks at 0 (`ln_area_weight`): the gamma distribution's at u = 0, the
    // log-normal's at u = −σ²/2, below which its weight falls again.
    let floor = math::ln(WEIGHT_FLOOR);
    let area = |i: isize| ln_area_weight(kind, variance, offset_u(i));
    let mut first = 0;
    while area(first - 1) >= floor {
        first -= 1;
    }
    let scattering = |i: isize| area(i) + 4.0 * offset_u(i);
    let mut last = 0;
    let mut scattering_peak = scattering(0);
    loop {
        let next = last + 1;
        scattering_peak = scattering_peak.max(scattering(next));
        let area_kept = area(next) >= floor;
        let scattering_kept = scattering(next) - scattering_peak >= floor;
        if !(area_kept || scattering_kept) {
            break;
        }
        last = next;
    }
    Kernel {
        first,
        weights: (first..=last).map(|i| math::exp(area(i))).collect(),
    }
}

/// A cell's area-weighted `Q̄_ext` and co-albedo 1 − ω.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub(crate) struct Cell {
    /// `Q̄_ext` = ⟨`C_ext`⟩ ÷ ⟨πr²⟩.
    extinction: f64,
    /// 1 − ω = ⟨`C_abs`⟩ ÷ ⟨`C_ext`⟩, 0 for a lossless file.
    coalbedo: f64,
}

/// The cell of `kernel` about fine node `centre` of `spectrum`.
///
/// # Errors
///
/// [`FitExtinctionError::ClippedCell`] if more than [`CLIP_TOLERANCE`] of the cell's area or
/// extinction lies off the fine grid (or the share is NaN).
pub(crate) fn cell(
    spectrum: &Spectrum,
    centre: usize,
    kernel: &Kernel,
) -> Result<Cell, FitExtinctionError> {
    let (mut area, mut extinction, mut absorption, mut clipped) = (0.0, 0.0, 0.0, 0.0);
    let start = isize::try_from(centre).map_or(isize::MAX, |c| c.saturating_add(kernel.first));
    for (i, &w) in (0_isize..).zip(&kernel.weights) {
        // A negative node lies below the fine grid: `try_from` refuses it, and it is clipped.
        let node = usize::try_from(start.saturating_add(i)).ok();
        match node.and_then(|j| spectrum.q_ext.get(j).zip(spectrum.q_sca.get(j))) {
            Some((&q_ext, &q_sca)) => {
                area += w;
                extinction += w * q_ext;
                absorption += w * (q_ext - q_sca);
            }
            None => clipped += w,
        }
    }
    let share = (clipped / area).max(clipped * spectrum.largest_extinction / extinction);
    if share.is_nan() || share > CLIP_TOLERANCE {
        return Err(FitExtinctionError::ClippedCell { centre, share });
    }
    Ok(Cell {
        extinction: extinction / area,
        coalbedo: if spectrum.lossless {
            0.0
        } else {
            absorption / extinction
        },
    })
}

/// `value` rounded to [`SIGNIFICANT_DIGITS`] significant digits, by Rust's correctly rounded
/// formatting.
///
/// # Panics
///
/// Never: the formatted value of a finite or infinite f64 parses back as one.
#[must_use]
pub(crate) fn significant(value: f64) -> f64 {
    let precision = SIGNIFICANT_DIGITS - 1;
    format!("{value:.precision$e}")
        .parse()
        .expect("an f64 formatted in scientific notation parses back")
}

/// One distribution's grid of a file: rows by radius, columns by variance.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Grid {
    /// `Q̄_ext`, rounded.
    extinction: Vec<Vec<f64>>,
    /// ln ω, rounded.
    ln_albedo: Vec<Vec<f64>>,
}

/// The half-step check's worst relative misses.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub(crate) struct Misses {
    /// Of `Q̄_ext`.
    extinction: f64,
    /// Of ω.
    albedo: f64,
    /// Of 1 − ω, reported and not bounded.
    coalbedo: f64,
    /// Where the worst of `Q̄_ext` lies: its radius, m, and variance.
    extinction_at: (f64, f64),
    /// Where the worst of ω lies.
    albedo_at: (f64, f64),
}

/// Whether `candidate` is worse than `current`: larger, or NaN.
#[must_use]
fn worse(candidate: f64, current: f64) -> bool {
    candidate.is_nan() || candidate > current
}

impl Misses {
    /// The worse of `self` and `other`, measure by measure; a NaN counts as the worst.
    #[must_use]
    fn worst(self, other: Self) -> Self {
        let (extinction, extinction_at) = if worse(other.extinction, self.extinction) {
            (other.extinction, other.extinction_at)
        } else {
            (self.extinction, self.extinction_at)
        };
        let (albedo, albedo_at) = if worse(other.albedo, self.albedo) {
            (other.albedo, other.albedo_at)
        } else {
            (self.albedo, self.albedo_at)
        };
        Self {
            extinction,
            albedo,
            coalbedo: if worse(other.coalbedo, self.coalbedo) {
                other.coalbedo
            } else {
                self.coalbedo
            },
            extinction_at,
            albedo_at,
        }
    }

    /// Whether either bounded miss exceeds [`HALVING_BOUND`] (or is NaN).
    #[must_use]
    fn exceed_the_bound(self) -> bool {
        worse(self.extinction, HALVING_BOUND) || worse(self.albedo, HALVING_BOUND)
    }
}

/// The table's fine-node centre of radius node `k` of a grid with offsets `offsets`.
#[must_use]
fn centre_of(grid: FineGrid, offsets: &[usize], k: usize) -> usize {
    grid.below + offsets[k]
}

/// The grid of `kind` for `spectrum`, rounded.
///
/// # Errors
///
/// As [`cell`].
pub(crate) fn grid_of(
    kind: SizeDistributionKind,
    spectrum: &Spectrum,
) -> Result<Grid, FitExtinctionError> {
    let fine = fine_grid();
    let offsets = radius_offsets();
    let kernels: Vec<Kernel> = variances_of(kind)
        .iter()
        .map(|&v| kernel(kind, v))
        .collect();
    let mut grid = Grid::default();
    for k in 0..radii_of(kind) {
        let centre = centre_of(fine, &offsets, k);
        let cells = kernels
            .iter()
            .map(|kern| cell(spectrum, centre, kern))
            .collect::<Result<Vec<_>, _>>()?;
        grid.extinction
            .push(cells.iter().map(|c| significant(c.extinction)).collect());
        grid.ln_albedo.push(
            cells
                .iter()
                .map(|c| significant(math::ln_1p(-c.coalbedo)))
                .collect(),
        );
    }
    Ok(grid)
}

/// The half-step check of `grid` (of `kind`, from `spectrum`): at every half step in radius, in
/// variance and in both, the table's interpolation (the sim's `interpolate_grid`, as
/// `substance::aerosol` reads the committed table) against the cell computed there.
///
/// # Errors
///
/// As [`cell`], and [`FitExtinctionError::HalfStepOffGrid`] if a half step lies off the grid, a
/// broken invariant.
pub(crate) fn halving_check(
    kind: SizeDistributionKind,
    spectrum: &Spectrum,
    grid: &Grid,
) -> Result<Misses, FitExtinctionError> {
    let fine = fine_grid();
    let offsets = radius_offsets();
    let radii = effective_radii_m();
    let variances = variances_of(kind);
    let n_r = radii_of(kind);
    // The half steps: each radius node and each midpoint (a fine node), each variance node and
    // each midpoint in the variances' coordinate.
    let mut radius_points = Vec::with_capacity(2 * n_r);
    for k in 0..n_r {
        radius_points.push((centre_of(fine, &offsets, k), false));
        if k + 1 < n_r {
            let mid = usize::midpoint(offsets[k], offsets[k + 1]);
            radius_points.push((fine.below + mid, true));
        }
    }
    let mut variance_points = Vec::with_capacity(2 * variances.len());
    for (j, &v) in variances.iter().enumerate() {
        variance_points.push((v, false));
        if let Some(&next) = variances.get(j + 1) {
            let c = f64::midpoint(
                math::ln(v + VARIANCE_OFFSET),
                math::ln(next + VARIANCE_OFFSET),
            );
            variance_points.push((math::exp(c) - VARIANCE_OFFSET, true));
        }
    }
    let interpolate = |rows: &[Vec<f64>], radius_m: f64, variance: f64, space: GridSpace| {
        interpolate_grid(
            rows,
            &radii,
            &variances,
            VARIANCE_OFFSET,
            radius_m,
            variance,
            space,
        )
        .ok_or(FitExtinctionError::HalfStepOffGrid { radius_m, variance })
    };
    let mut misses = Misses::default();
    for &(v, v_mid) in &variance_points {
        let kern = kernel(kind, v);
        for &(centre, r_mid) in &radius_points {
            if !(r_mid || v_mid) {
                continue;
            }
            let direct = cell(spectrum, centre, &kern)?;
            let radius = fine_radius_m(fine, centre);
            let extinction = interpolate(&grid.extinction, radius, v, GridSpace::Logarithmic)?;
            let ln_albedo = interpolate(&grid.ln_albedo, radius, v, GridSpace::Linear)?;
            let coalbedo = -math::exp_m1(ln_albedo);
            let miss = Misses {
                extinction: (extinction / direct.extinction - 1.0).abs(),
                albedo: math::exp_m1(ln_albedo - math::ln_1p(-direct.coalbedo)).abs(),
                coalbedo: if direct.coalbedo > 0.0 {
                    (coalbedo / direct.coalbedo - 1.0).abs()
                } else {
                    coalbedo.abs()
                },
                extinction_at: (radius, v),
                albedo_at: (radius, v),
            };
            misses = misses.worst(miss);
        }
    }
    Ok(misses)
}

/// One file's result: its identity and its two grids.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FileExtinction {
    /// The file's name.
    file: &'static str,
    /// The row's key.
    key: String,
    /// The file's phase.
    phase: CondensedPhase,
    /// The file's variant.
    variant: Option<String>,
    /// Whether the file is a stand-in.
    stand_in: bool,
    /// The gamma distribution's grid.
    gamma: Grid,
    /// The log-normal's grid.
    log_normal: Grid,
    /// The half-step check's worst misses, of both grids.
    misses: Misses,
}

/// Computes file `name` ([`FileExtinction`]) on `threads` threads.
///
/// # Errors
///
/// [`RunTaskError::Input`] carrying a [`FitFileError`] if the file is neither a solid's nor a
/// liquid's, has no index at 550 nm on its grid or one the Mie refuses, a cell lies off the fine
/// grid, a half step lies off the table's grid, or the half-step check misses by more than
/// [`HALVING_BOUND`]; as [`spectrum`] otherwise.
pub(crate) fn fit_file(
    name: &'static str,
    file: &MaterialFile,
    threads: NonZeroUsize,
) -> Result<FileExtinction, RunTaskError> {
    let fail = |cause: FitExtinctionError| input_error(name, cause);
    let phase = match file.phase() {
        "solid" => CondensedPhase::Solid,
        "liquid" => CondensedPhase::Liquid,
        other => return Err(fail(FitExtinctionError::Phase(other.to_owned()))),
    };
    let (n, k) = index_at(file, EXTINCTION_WAVELENGTH_NM)
        .ok_or_else(|| fail(FitExtinctionError::NoIndexAt550))?;
    let index = ComplexIndex::new(n, k).map_err(|e| fail(FitExtinctionError::Mie(e)))?;
    let spectrum = spectrum(index, threads)?;
    let mut misses = Misses::default();
    let mut fit_kind = |kind: SizeDistributionKind| -> Result<Grid, RunTaskError> {
        let grid = grid_of(kind, &spectrum).map_err(fail)?;
        let check = halving_check(kind, &spectrum, &grid).map_err(fail)?;
        if check.exceed_the_bound() {
            let at = if worse(check.extinction, HALVING_BOUND) {
                check.extinction_at
            } else {
                check.albedo_at
            };
            return Err(fail(FitExtinctionError::MissedHalfStep {
                kind: kind.name(),
                extinction: check.extinction,
                albedo: check.albedo,
                radius_m: at.0,
                variance: at.1,
            }));
        }
        misses = misses.worst(check);
        Ok(grid)
    };
    let gamma = fit_kind(SizeDistributionKind::Gamma)?;
    let log_normal = fit_kind(SizeDistributionKind::LogNormal)?;
    Ok(FileExtinction {
        file: name,
        key: file.key().to_owned(),
        phase,
        variant: file.variant().map(str::to_owned),
        stand_in: file.provenance() == "standIn",
        gamma,
        log_normal,
        misses,
    })
}

/// Computes every file of `part` in `manifest`'s material directory.
///
/// # Errors
///
/// [`RunTaskError::Input`] carrying [`FitExtinctionError::WrongMaterials`] if the manifest names
/// other files than the part's, or as [`fit_file`]; [`RunTaskError::Dataset`] if a file cannot
/// be read.
pub(crate) fn fit(
    part: &Part,
    manifest: &Manifest,
    threads: NonZeroUsize,
) -> Result<Vec<FileExtinction>, RunTaskError> {
    if manifest.materials() != part.files {
        return Err(RunTaskError::Input {
            task: "condensate_extinction",
            source: Box::new(FitExtinctionError::WrongMaterials { part: part.name }),
        });
    }
    part.files
        .iter()
        .map(|&name| {
            let file = manifest
                .load_material(name)
                .map_err(|e| RunTaskError::Dataset(LoadDatasetError::Material(Box::new(e))))?;
            fit_file(name, &file, threads)
        })
        .collect()
}

/// The axes' items: the radii, the two families' variances and the variances' offset.
#[must_use]
fn axes_items() -> Vec<TableItem> {
    vec![
        TableItem::Array {
            name: "EFFECTIVE_RADII_M".to_owned(),
            doc: vec![
                "The effective radii of the grid, m, rising: from 1 nm by 0.152 in ln r to 21 nm,"
                    .to_owned(),
                "0.08 to 4.1 µm and 0.16 to 101 µm. The gamma distribution's grids hold every one,"
                    .to_owned(),
                "the log-normal's the first 100.".to_owned(),
            ],
            values: effective_radii_m(),
        },
        TableItem::Array {
            name: "GAMMA_EFFECTIVE_VARIANCES".to_owned(),
            doc: vec![
                "The gamma distribution's effective variances, even in ln(`v_eff` + 0.05) from 0.04"
                    .to_owned(),
                "to 0.45.".to_owned(),
            ],
            values: variances_of(SizeDistributionKind::Gamma),
        },
        TableItem::Array {
            name: "LOG_NORMAL_EFFECTIVE_VARIANCES".to_owned(),
            doc: vec![
                "The log-normal's effective variances, even in ln(`v_eff` + 0.05) from 0.04 to 0.65."
                    .to_owned(),
            ],
            values: variances_of(SizeDistributionKind::LogNormal),
        },
        TableItem::Scalar {
            name: "EFFECTIVE_VARIANCE_OFFSET".to_owned(),
            doc: vec![
                "The offset of the variances' coordinate, ln(`v_eff` + 0.05), in which the grids are"
                    .to_owned(),
                "interpolated.".to_owned(),
            ],
            value: VARIANCE_OFFSET,
        },
    ]
}

/// A file's four grid items.
#[must_use]
fn file_items(file: &FileExtinction) -> Vec<TableItem> {
    let prefix = item_prefix(file.file);
    let grid_item = |suffix: &str, what: &str, family: &str, rows: &[Vec<f64>]| TableItem::Nested {
        name: format!("{prefix}_{suffix}"),
        doc: vec![
            format!("`{}`'s {what} at 550 nm over {family},", file.file),
            "rows by effective radius, columns by effective variance.".to_owned(),
        ],
        rows: rows.to_vec(),
    };
    vec![
        grid_item(
            "GAMMA_EXTINCTION",
            "area-weighted extinction efficiency",
            "the gamma distribution",
            &file.gamma.extinction,
        ),
        grid_item(
            "GAMMA_LN_ALBEDO",
            "single-scattering albedo's natural logarithm ln ω",
            "the gamma distribution",
            &file.gamma.ln_albedo,
        ),
        grid_item(
            "LOG_NORMAL_EXTINCTION",
            "area-weighted extinction efficiency",
            "the log-normal",
            &file.log_normal.extinction,
        ),
        grid_item(
            "LOG_NORMAL_LN_ALBEDO",
            "single-scattering albedo's natural logarithm ln ω",
            "the log-normal",
            &file.log_normal.ln_albedo,
        ),
    ]
}

/// The entries' source: each file's identity and its grids.
#[must_use]
fn entries_source(files: &[FileExtinction]) -> String {
    use std::fmt::Write;
    let mut out = format!(
        "/// The table's index files, in name order, each with its key, phase, variant, stand-in \
         flag and grids.\n#[rustfmt::skip]\npub static ENTRIES: [ExtinctionEntry; {}] = [\n",
        files.len()
    );
    for file in files {
        let prefix = item_prefix(file.file);
        let phase = match file.phase {
            CondensedPhase::Solid => "Solid",
            CondensedPhase::Liquid => "Liquid",
        };
        let variant = file
            .variant
            .as_ref()
            .map_or_else(|| "None".to_owned(), |v| format!("Some({v:?})"));
        // Writing to a String cannot fail.
        let _ = writeln!(
            out,
            "    ExtinctionEntry {{\n        file: {:?},\n        key: {:?},\n        phase: \
             crate::substance::CondensedPhase::{phase},\n        variant: {variant},\n        \
             stand_in: {},\n        gamma_extinction: &{prefix}_GAMMA_EXTINCTION,\n        \
             gamma_ln_albedo: &{prefix}_GAMMA_LN_ALBEDO,\n        log_normal_extinction: \
             &{prefix}_LOG_NORMAL_EXTINCTION,\n        log_normal_ln_albedo: \
             &{prefix}_LOG_NORMAL_LN_ALBEDO,\n    }},",
            file.file, file.key, file.stand_in
        );
    }
    out.push_str("];\n");
    out
}

/// The table of `part` for `files`.
#[must_use]
pub(crate) fn render(part: &Part, files: &[FileExtinction]) -> RustTable {
    let mut items = vec![TableItem::Source(
        "use crate::substance::aerosol::ExtinctionEntry;\n".to_owned(),
    )];
    if part.axes {
        items.extend(axes_items());
    }
    for file in files {
        items.extend(file_items(file));
    }
    items.push(TableItem::Source(entries_source(files)));
    let mut notes = vec![
        "Each index file's Mie spheres at its index at 550 nm, found by wavelength on its own grid"
            .to_owned(),
        "(every shape class a sphere of equal volume to projected area), averaged over Hansen and"
            .to_owned(),
        "Travis 1974's gamma distribution and the log-normal on a fine grid even in ln r: the"
            .to_owned(),
        "area-weighted `Q̄_ext` and ln ω, the albedo's logarithm, to five significant digits, on the"
            .to_owned(),
        "grid of `EFFECTIVE_RADII_M` and the variances. `substance::aerosol` interpolates them (ln"
            .to_owned(),
        "`Q̄_ext` and ln ω) by Lagrange's cubic in ln `r_eff` and ln(`v_eff` + 0.05) and applies the"
            .to_owned(),
        "row's grain density.".to_owned(),
    ];
    if !part.axes {
        notes.push(String::new());
        notes.push(
            "The grid's axes are `condensate_extinction`'s; this is one of its six other parts."
                .to_owned(),
        );
    }
    RustTable {
        summary: vec![
            "Aerosol extinction at 550 nm (plan 14, P14.T49.e), which `substance::aerosol`"
                .to_owned(),
            "reads.".to_owned(),
        ],
        notes,
        items,
    }
}

/// One of the seven tables' tasks ([`PARTS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CondensateExtinctionTask {
    part: usize,
}

impl CondensateExtinctionTask {
    /// Its table.
    #[must_use]
    pub fn part(&self) -> &'static Part {
        &PARTS[self.part]
    }
}

/// The first table, with the axes.
pub static TASK_1: CondensateExtinctionTask = CondensateExtinctionTask { part: 0 };
/// The second table.
pub static TASK_2: CondensateExtinctionTask = CondensateExtinctionTask { part: 1 };
/// The third table.
pub static TASK_3: CondensateExtinctionTask = CondensateExtinctionTask { part: 2 };
/// The fourth table.
pub static TASK_4: CondensateExtinctionTask = CondensateExtinctionTask { part: 3 };
/// The fifth table.
pub static TASK_5: CondensateExtinctionTask = CondensateExtinctionTask { part: 4 };
/// The sixth table.
pub static TASK_6: CondensateExtinctionTask = CondensateExtinctionTask { part: 5 };
/// The seventh table.
pub static TASK_7: CondensateExtinctionTask = CondensateExtinctionTask { part: 6 };

impl FitTask for CondensateExtinctionTask {
    fn name(&self) -> &'static str {
        self.part().name
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        self.part().table_path
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        self.part().items
    }

    /// The sim values the table depends on: the two families' variance-node counts, which the
    /// sim's entry type fixes; the material files are the inputs hash's.
    fn fingerprint(&self) -> SimFingerprint {
        #[expect(
            clippy::cast_precision_loss,
            reason = "node counts below 100, exact in an f64"
        )]
        SimFingerprint::new(vec![
            (
                "gamma variance nodes".to_owned(),
                GAMMA_VARIANCE_NODES as f64,
            ),
            (
                "log-normal variance nodes".to_owned(),
                LOG_NORMAL_VARIANCE_NODES as f64,
            ),
        ])
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let part = self.part();
        let files = fit(part, manifest, threads)?;
        let worst = files
            .iter()
            .fold(Misses::default(), |m, file| m.worst(file.misses));
        let worst_file = |pick: fn(&Misses) -> f64| {
            files
                .iter()
                .max_by(|a, b| pick(&a.misses).total_cmp(&pick(&b.misses)))
                .map_or("none", |f| f.file)
        };
        let acceptance = format!(
            "{} index files at 550 nm, found by wavelength on each file's grid; the half-step check: \
             `Q̄_ext` within {:.2e} ({}) and ω within {:.2e} ({}) of the direct values, against \
             {HALVING_BOUND:e}; 1 − ω within {:.2e}",
            files.len(),
            worst.extinction,
            worst_file(|m| m.extinction),
            worst.albedo,
            worst_file(|m| m.albedo),
            worst.coalbedo,
        );
        Ok(TaskOutput {
            table: render(part, &files),
            source: "rendering plan R08's committed index files \
                     (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), each citing its \
                     own primaries; Mie spheres by Wiscombe 1980 (Appl. Opt. 19, 1505; \
                     NCAR/TN-140+STR, 1996 edition), held to MIEV0's test cases; the size \
                     distributions of Hansen and Travis 1974 (Space Sci. Rev. 16, 527, (2.53)–(2.61))"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use hyperion_sim::substance::by_key;
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::materials::MATERIALS_FROM_CRATE;

    /// The repository's material directory.
    fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE)
    }

    fn read(name: &str) -> MaterialFile {
        MaterialFile::read(&dir(), name).unwrap()
    }

    fn threads() -> NonZeroUsize {
        NonZeroUsize::new(4).unwrap()
    }

    fn index(n: f64, k: f64) -> ComplexIndex {
        ComplexIndex::new(n, k).unwrap()
    }

    /// The cell of `kind` at variance `variance` about any radius `radius_m`, not only a fine
    /// node's: the kernel's weights at each fine node's own u, over the kernel's reach.
    fn cell_at(
        spectrum: &Spectrum,
        kind: SizeDistributionKind,
        variance: f64,
        radius_m: f64,
    ) -> Cell {
        let grid = fine_grid();
        let reach = kernel(kind, variance);
        let low = offset_u(reach.first);
        let high = offset_u(reach.first + isize::try_from(reach.weights.len()).unwrap());
        let (mut area, mut extinction, mut absorption) = (0.0, 0.0, 0.0);
        for j in 0..grid.nodes {
            let u = fine_ln_radius(grid, j) - math::ln(radius_m);
            if (low..=high).contains(&u) {
                let w = math::exp(ln_area_weight(kind, variance, u));
                area += w;
                extinction += w * spectrum.q_ext[j];
                absorption += w * (spectrum.q_ext[j] - spectrum.q_sca[j]);
            }
        }
        Cell {
            extinction: extinction / area,
            coalbedo: if spectrum.lossless {
                0.0
            } else {
                absorption / extinction
            },
        }
    }

    /// 550 nm is found by wavelength: the 75 wt% acid's grid starts at 360 nm and water's at 380
    /// nm, so their 550 nm nodes sit at different positions, and each is read where it is.
    #[test]
    fn condensate_extinction_finds_550_nm_by_wavelength() {
        let mut positions = Vec::new();
        for (name, first) in [("sulphuric-acid-75.json", 360.0), ("water.json", 380.0)] {
            let file = read(name);
            assert!((file.wavelengths_nm()[0] - first).abs() < 1e-12, "{name}");
            let at = file
                .wavelengths_nm()
                .iter()
                .position(|&w| (w - EXTINCTION_WAVELENGTH_NM).abs() < 1e-9)
                .unwrap();
            let (n, k) = index_at(&file, EXTINCTION_WAVELENGTH_NM).unwrap();
            assert_same_bits(n, file.n()[at]);
            assert_same_bits(k, file.k()[at]);
            positions.push(at);
        }
        assert_ne!(positions[0], positions[1]);
    }

    /// Each kernel is its distribution's: its area-weighted mean radius is `r_eff` and its
    /// area-weighted variance `v_eff` `r_eff²` (Hansen and Travis 1974, (2.53) and (2.54)), to
    /// 10⁻⁶ and 10⁻⁵ of each, at every node and half step of the grid's variances.
    #[test]
    fn condensate_extinction_kernels_hold_the_effective_radius_and_variance() {
        for kind in [SizeDistributionKind::Gamma, SizeDistributionKind::LogNormal] {
            let nodes = variances_of(kind);
            let mids = nodes.windows(2).map(|w| {
                let c = f64::midpoint(
                    math::ln(w[0] + VARIANCE_OFFSET),
                    math::ln(w[1] + VARIANCE_OFFSET),
                );
                math::exp(c) - VARIANCE_OFFSET
            });
            for v in nodes.iter().copied().chain(mids) {
                let reach = kernel(kind, v);
                let (mut area, mut mean, mut spread) = (0.0, 0.0, 0.0);
                for (i, &w) in (reach.first..).zip(&reach.weights) {
                    let r = math::exp(offset_u(i));
                    area += w;
                    mean += w * r;
                    spread += w * (r - 1.0) * (r - 1.0);
                }
                assert!(
                    (mean / area - 1.0).abs() < 1e-6,
                    "{kind:?} {v}: {}",
                    mean / area
                );
                assert!(
                    (spread / area / v - 1.0).abs() < 1e-5,
                    "{kind:?} {v}: {}",
                    spread / area
                );
            }
        }
    }

    /// Venus's four deck modes at n = 1.44 (science-r08-sulphur-co2ice.md), combined by mass at
    /// the 75 wt% acid's 1,662.6 kg m⁻³, give the note's 417 m² kg⁻¹ within 2% and its τ(550) of
    /// 34.5. The modal radii 0.30, 1.00, 1.40 and 3.65 µm and ln σ 0.44, 0.25, 0.21 and 0.25 are
    /// J. B. Pollack et al.'s (Icarus 103, 1–42, 1993); the columns 3.97 × 10⁸, 7.50 × 10⁷,
    /// 6.05 × 10⁷ and 1.33 × 10⁷ cm⁻² are N₀ (`z_c` + `H_up` + `H_lo`) of R. Haus et al.'s profiles
    /// (Icarus 272, 178–205, 2016), both as Reyes-Guerrero, Pérez-Hoyos and Garate-Lopez 2026
    /// tabulate them (arXiv:2607.22187, §3.1 and Table 2).
    #[test]
    fn condensate_extinction_venus_modes_give_the_sulphur_note_s_417() {
        let spectrum = spectrum(index(1.44, 0.0), threads()).unwrap();
        let density = 1_662.6;
        let (mut tau, mut mass) = (0.0, 0.0);
        for (modal_radius_m, ln_sigma, column_per_cm2) in [
            (0.30e-6, 0.44, 3.97e8),
            (1.00e-6, 0.25, 7.50e7),
            (1.40e-6, 0.21, 6.05e7),
            (3.65e-6, 0.25, 1.33e7),
        ] {
            let s2: f64 = ln_sigma * ln_sigma;
            let radius = modal_radius_m * math::exp(2.5 * s2);
            let c = cell_at(
                &spectrum,
                SizeDistributionKind::LogNormal,
                math::exp_m1(s2),
                radius,
            );
            let area = std::f64::consts::PI * modal_radius_m * modal_radius_m * math::exp(2.0 * s2);
            let column = column_per_cm2 * 1e4;
            tau += column * c.extinction * area;
            mass += column * density * 4.0 / 3.0 * radius * area;
        }
        let kappa = tau / mass;
        assert!((kappa / 417.0 - 1.0).abs() < 0.02, "{kappa} m² kg⁻¹");
        assert!((tau / 34.5 - 1.0).abs() < 0.02, "τ {tau}");
    }

    /// K = (m² − 1) ÷ (m² + 2) for m = n + ik, as (re, im).
    fn polarisability(n: f64, k: f64) -> (f64, f64) {
        let (re, im) = (n * n - k * k, 2.0 * n * k);
        let den = (re + 2.0) * (re + 2.0) + im * im;
        (
            ((re - 1.0) * (re + 2.0) + im * im) / den,
            (im * (re + 2.0) - (re - 1.0) * im) / den,
        )
    }

    /// At the grid's first radius, 1 nm (x = 0.011), the size averages are Rayleigh's (Bohren and
    /// Huffman 1983, §5.1): a lossless sphere's `Q̄_ext` is (8/3) |K|² `x_eff`⁴ times the
    /// distribution's ⟨r⁶⟩ ÷ (⟨r²⟩ `r_eff`⁴), (1 + b)(1 + 2b)(1 + 3b) for the gamma distribution
    /// and e^(6σ²) for the log-normal, and an absorbing one's absorption 4 `x_eff` Im K at every
    /// variance, since ⟨r³⟩ ÷ ⟨r²⟩ is `r_eff`.
    #[test]
    fn condensate_extinction_meets_the_rayleigh_limit() {
        let x = size_parameter(FIRST_RADIUS_M);
        let fine = fine_grid();
        for (n, k) in [(1.33, 0.0), (1.57, 0.53)] {
            let spectrum = spectrum(index(n, k), threads()).unwrap();
            let (k_re, k_im) = polarisability(n, k);
            for kind in [SizeDistributionKind::Gamma, SizeDistributionKind::LogNormal] {
                for v in [0.04, 0.2] {
                    let c = cell(&spectrum, fine.below, &kernel(kind, v)).unwrap();
                    if k <= 0.0 {
                        let moment = match kind {
                            SizeDistributionKind::Gamma => {
                                (1.0 + v) * (1.0 + 2.0 * v) * (1.0 + 3.0 * v)
                            }
                            SizeDistributionKind::LogNormal => math::exp(6.0 * math::ln_1p(v)),
                        };
                        let rayleigh =
                            8.0 / 3.0 * (k_re * k_re + k_im * k_im) * math::powi(x, 4) * moment;
                        let miss = c.extinction / rayleigh - 1.0;
                        assert!(miss.abs() < 1e-3, "{kind:?} {v}: {miss:e}");
                    } else {
                        let absorption = c.extinction * c.coalbedo;
                        let miss = absorption / (4.0 * x * k_im) - 1.0;
                        assert!(miss.abs() < 1e-3, "{kind:?} {v}: {miss:e}");
                    }
                }
            }
        }
    }

    /// At the gamma grid's last radius, 101 µm (x = 1,150), `Q̄_ext` lies above the geometric limit
    /// 2 by less than 2% for every index and variance; a transparent sphere's narrow distribution
    /// (`v_eff` 0.04) by Nussenzveig and Wiscombe 1980's edge term, about 2 x^(−2/3) = 0.018, to 10%
    /// of it. The edge term grows with the index (chromium's is larger), and a wide distribution's
    /// smaller particles add to it.
    #[test]
    fn condensate_extinction_meets_the_geometric_limit() {
        let fine = fine_grid();
        let top = fine.below + radius_offsets()[GAMMA_RADII - 1];
        let x = size_parameter(effective_radii_m()[GAMMA_RADII - 1]);
        for (n, k) in [(1.334, 2.5e-9), (1.547, 0.0), (1.57, 0.53), (3.18, 3.33)] {
            let spectrum = spectrum(index(n, k), threads()).unwrap();
            for v in [0.04, 0.45] {
                let c = cell(&spectrum, top, &kernel(SizeDistributionKind::Gamma, v)).unwrap();
                let excess = c.extinction - 2.0;
                assert!(
                    excess > 0.0 && excess < 0.04,
                    "{n} + {k}i, v {v}: {}",
                    c.extinction
                );
                if v < 0.1 && k < 0.1 {
                    let edge = 2.0 * math::powf(x, -2.0 / 3.0);
                    assert!(
                        (excess / edge - 1.0).abs() < 0.1,
                        "{n} + {k}i, v {v}: {excess} against {edge}"
                    );
                }
            }
        }
    }

    /// The hardest files meet the half-step bound: the lossless high-index titanium dioxide and
    /// zinc sulphide, whose narrow resonances are the sharpest, manganese sulphide (n 2.87), the
    /// worst of the fit, and the absorbing tholin, whose ω falls furthest at small sizes.
    #[test]
    fn condensate_extinction_the_hardest_files_meet_the_half_step_bound() {
        for name in [
            "titanium-dioxide.json",
            "zinc-sulphide.json",
            "manganese-sulphide.json",
            "tholin.json",
        ] {
            let fitted = fit_file("test.json", &read(name), threads()).unwrap();
            assert!(
                !fitted.misses.exceed_the_bound(),
                "{name}: {:?}",
                fitted.misses
            );
            assert!(
                fitted.misses.extinction <= HALVING_BOUND,
                "{name}: {:?}",
                fitted.misses
            );
            assert!(
                fitted.misses.albedo <= HALVING_BOUND,
                "{name}: {:?}",
                fitted.misses
            );
        }
    }

    /// The spectrum's sub-samples have converged: halving both spacings and doubling both
    /// reaches moves no cell of the hardest files below 30 µm by more than 2.5 × 10⁻⁴ of `Q̄_ext` or
    /// of ω, nor any by more than 4 × 10⁻⁴ (measured 3.5 × 10⁻⁴ for titanium dioxide's at 101 µm,
    /// whose spheres beyond x = 1,200 are single and their ripple left to the distribution).
    #[test]
    fn condensate_extinction_the_spectrum_has_converged() {
        let refined = |x: f64| {
            let spacing = if x <= 2.0 * SPIKE_REACH {
                SPIKE_SPACING / 2.0
            } else if x <= 2.0 * RIPPLE_REACH {
                RIPPLE_SPACING / 2.0
            } else {
                return 1;
            };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a count from 1 to 960 for x up to twice RIPPLE_REACH"
            )]
            let count = (x * FINE_STEP / spacing).ceil().max(1.0) as u32;
            count
        };
        for name in ["titanium-dioxide.json", "manganese-sulphide.json"] {
            let (n, k) = index_at(&read(name), EXTINCTION_WAVELENGTH_NM).unwrap();
            let standard = spectrum(index(n, k), threads()).unwrap();
            let fine = spectrum_with(index(n, k), threads(), refined).unwrap();
            // The worst change below 30 µm, and above it, of `Q̄_ext` and of ω.
            let mut worst = [0.0_f64; 4];
            let radii = effective_radii_m();
            for kind in [SizeDistributionKind::Gamma, SizeDistributionKind::LogNormal] {
                let coarse = grid_of(kind, &standard).unwrap();
                let refined_grid = grid_of(kind, &fine).unwrap();
                for (ri, radius) in radii.iter().enumerate().take(coarse.extinction.len()) {
                    let top = usize::from(*radius > 30e-6);
                    for vi in 0..coarse.extinction[ri].len() {
                        let q_change =
                            (coarse.extinction[ri][vi] / refined_grid.extinction[ri][vi] - 1.0)
                                .abs();
                        let albedo_change =
                            math::exp_m1(coarse.ln_albedo[ri][vi] - refined_grid.ln_albedo[ri][vi])
                                .abs();
                        worst[top] = worst[top].max(q_change);
                        worst[2 + top] = worst[2 + top].max(albedo_change);
                    }
                }
            }
            assert!(worst[0] < 2.5e-4 && worst[2] < 2.5e-4, "{name}: {worst:?}");
            assert!(worst[1] < 4e-4 && worst[3] < 4e-4, "{name}: {worst:?}");
        }
    }

    /// The parts hold every file whose key is an aerosol material, and no other, seven to a table
    /// in name order, the first with the axes; each part's items follow its files, and its manifest
    /// names them.
    #[test]
    fn condensate_extinction_the_parts_are_the_aerosol_files() {
        let mut aerosol: Vec<String> = std::fs::read_dir(dir())
            .unwrap()
            .filter_map(|e| {
                let name = e.unwrap().file_name().into_string().unwrap();
                let is_material = Path::new(&name)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                    && std::fs::read_to_string(dir().join(&name))
                        .is_ok_and(|text| text.contains("\"wavelengthsNm\""));
                is_material.then_some(name)
            })
            .filter(|name| {
                by_key(read(name).key())
                    .is_some_and(|id| id.substance().optics().aerosol_material())
            })
            .collect();
        aerosol.sort();
        let listed: Vec<&str> = PARTS.iter().flat_map(|p| p.files.iter().copied()).collect();
        assert_eq!(listed, aerosol);
        for (i, part) in PARTS.iter().enumerate() {
            assert!(part.files.len() <= 7, "{}", part.name);
            assert_eq!(part.axes, i == 0, "{}", part.name);
            let mut items: Vec<String> = Vec::new();
            if part.axes {
                items.extend(
                    [
                        "EFFECTIVE_RADII_M",
                        "GAMMA_EFFECTIVE_VARIANCES",
                        "LOG_NORMAL_EFFECTIVE_VARIANCES",
                        "EFFECTIVE_VARIANCE_OFFSET",
                    ]
                    .map(str::to_owned),
                );
            }
            for file in part.files {
                let prefix = item_prefix(file);
                for suffix in [
                    "GAMMA_EXTINCTION",
                    "GAMMA_LN_ALBEDO",
                    "LOG_NORMAL_EXTINCTION",
                    "LOG_NORMAL_LN_ALBEDO",
                ] {
                    items.push(format!("{prefix}_{suffix}"));
                }
            }
            assert_eq!(items, part.items, "{}", part.name);
            let manifest = Manifest::load(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("manifests")
                    .join(format!("{}.toml", part.name)),
            )
            .unwrap();
            assert_eq!(manifest.materials(), part.files, "{}", part.name);
            assert_eq!(manifest.task(), part.name);
        }
    }

    /// The cause a refused fit carries.
    fn cause(result: Result<FileExtinction, RunTaskError>) -> FitExtinctionError {
        match result {
            Err(RunTaskError::Input { source, .. }) => {
                source.downcast_ref::<FitFileError>().unwrap().cause.clone()
            }
            other => panic!("not an input error: {other:?}"),
        }
    }

    /// Each refusal is its own: a manifest naming other files, a file whose phase is not a solid's
    /// or a liquid's, one whose grid does not reach 550 nm, a cell off the fine grid, and a grid
    /// that misses its half steps.
    #[test]
    fn condensate_extinction_refuses_what_it_cannot_fit() {
        let other = Manifest::from_bytes(
            Path::new("crates/hyperion-fit/manifests/condensate_extinction.toml"),
            b"task = \"condensate_extinction\"\nmaterials = [\"water.json\"]\n".to_vec(),
        )
        .unwrap();
        match fit(&PARTS[0], &other, threads()) {
            Err(RunTaskError::Input { source, .. }) => assert!(matches!(
                source.downcast_ref::<FitExtinctionError>(),
                Some(FitExtinctionError::WrongMaterials { .. })
            )),
            other => panic!("{other:?}"),
        }
        let scratch = tempfile::tempdir().unwrap();
        let write = |name: &str, phase: &str, grid: &str| {
            std::fs::write(
                scratch.path().join(name),
                format!(
                    r#"{{"key":"X","phase":"{phase}","shape":"sphere","provenance":"measured",
                       "wavelengthsNm":{grid},"n":[1.3,1.3],"k":[0,0]}}"#
                ),
            )
            .unwrap();
            MaterialFile::read(scratch.path(), name).unwrap()
        };
        let red = write("red.json", "solid", "[600,780]");
        assert_eq!(
            cause(fit_file("red.json", &red, threads())),
            FitExtinctionError::NoIndexAt550
        );
        let vapour = write("vapour.json", "gas", "[380,780]");
        assert!(matches!(
            cause(fit_file("vapour.json", &vapour, threads())),
            FitExtinctionError::Phase(_)
        ));
        // A cell whose kernel reaches below the fine grid's first node.
        let spectrum = spectrum(index(1.33, 0.0), threads()).unwrap();
        let wide = kernel(SizeDistributionKind::Gamma, 0.45);
        assert!(matches!(
            cell(&spectrum, 0, &wide),
            Err(FitExtinctionError::ClippedCell { .. })
        ));
        // A grid one of whose values is 1% off misses its half steps.
        let mut grid = grid_of(SizeDistributionKind::Gamma, &spectrum).unwrap();
        grid.extinction[60][5] *= 1.01;
        let misses = halving_check(SizeDistributionKind::Gamma, &spectrum, &grid).unwrap();
        assert!(misses.exceed_the_bound(), "{misses:?}");
        assert!(misses.extinction > 2e-3, "{misses:?}");
    }

    /// A file whose k is 0 at 550 nm has ω = 1 exactly in every cell, and an absorbing one below
    /// 1 everywhere.
    #[test]
    fn condensate_extinction_a_lossless_file_scatters_all_it_extinguishes() {
        let salt = spectrum(index(1.546_91, 0.0), threads()).unwrap();
        let soot = spectrum(index(1.57, 0.53), threads()).unwrap();
        for kind in [SizeDistributionKind::Gamma, SizeDistributionKind::LogNormal] {
            let lossless = grid_of(kind, &salt).unwrap();
            assert!(
                lossless
                    .ln_albedo
                    .iter()
                    .flatten()
                    .all(|&a| a.abs() < f64::MIN_POSITIVE)
            );
            let absorbing = grid_of(kind, &soot).unwrap();
            assert!(absorbing.ln_albedo.iter().flatten().all(|&a| a < 0.0));
        }
    }
}
