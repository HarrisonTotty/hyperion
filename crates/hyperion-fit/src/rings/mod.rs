//! The rings' shadowing table (plan R11, R11.T9.b; Design note 15): an offline Monte Carlo of a
//! ring's particle layer whose committed output is a client module, [`MODULE_PATH`]. Like R08's
//! reference tracer ([`crate::atmosphere`]) it is not a [`FitTask`](crate::task::FitTask): it is
//! run by hand (`hyperion-fit ring-shadowing`), outside `tables.lock`, and moves no generated
//! output.
//!
//! - [`layer`]: the layer (R⁻³ radii of 0.5–5 m, a Gaussian vertical profile, D₀ tied to `τ_dyn`)
//!   and its frozen field of hard spheres;
//! - `trace`: the photons, scattered with the n = 3.09 power-law phase and tallied by order.
//!
//! **Why a dense Gaussian layer.** Salo and French 2010 (Icarus 210, 785) found the B ring's tilt
//! brightening — some 20–30% between B = 4.5° and 26°, most of it from shadowing between
//! particles and at most 10% from multiple scattering — in dynamical particle fields whose central
//! filling factor is 0.32–0.38 and which are not uniform in height; a vertically uniform layer
//! shows no elevation dependence. The table's layer is therefore Gaussian in height at D₀ = 0.35
//! for `τ_dyn` = 1.5 ([`layer::SALO_FRENCH_LAW`]), and its acceptance (§5.2 of the paper, Design
//! note 15) is the single-scattering ratio `f_e`(26°) ÷ `f_e`(4.5°) at α = 6°, `f_e` = `I_ss` ÷
//! `I_ss`(D = 0), of 1.30 ± 0.05, with a uniform layer at D = 0.05 failing it.
//!
//! **What the table holds** ([`ShadowingTable`]), on three axes (Design note 15; [`Axes`]):
//!
//! - **`τ_phot`** = −ln(normal transmission), which the Monte Carlo measures and which is what the
//!   client's slab takes as its optical depth; each node's `τ_dyn` is solved for it. Node 0 is
//!   τ = 0, where the layer is gone: `f_e` = 1 and no multiple scattering.
//! - **`μ_eff`** = 2μμ₀ ÷ (μ + μ₀), the harmonic mean of the viewer's and the star's elevation
//!   sines: the lit face's single-scattering depth weight e^(−τ(1/μ + 1/μ₀)) is e^(−2τ/`μ_eff`), so
//!   a geometry with B ≠ B′ is read as the symmetric one of B = B′ = arcsin `μ_eff`, which the
//!   Monte Carlo traces (a lean of this task, 2026-11-14). Its phase angle always lies within the
//!   symmetric geometry's range, since `B_eff` ≤ (B + B′) ÷ 2. The reduction is exact only for that
//!   weight: for the multiple scattering and the unlit face it is a model, whose error at a few
//!   B ≠ B′ geometries the run measures ([`Checks`]).
//! - **The phase angle**, as a fraction v of its range at B = B′: α = v (π − 2B) on the lit face,
//!   α = π − v (π − 2B) on the unlit face.
//!
//! Per node: the lit face's shadowing factor `f_e` (the classical layer's multiplier at
//! `τ_phot`); the lit face's I/F at ϖ = 1 of the second and of the third scattering order divided
//! by the classical prefactor μ₀ ÷ (μ + μ₀), which the client multiplies by its own geometry's; the
//! unlit face's I/F at ϖ = 1 of the same orders divided by μ₀, the light let in, which the client
//! multiplies by its own μ₀ (each a lean of this task, 2026-11-14, against the `μ_eff` reduction's
//! error in the multiple scattering); and per (τ, μ) and face the ratios
//! `q_e` and `q_o` of the even orders from the fourth to those from the second, and of the odd
//! orders from the fifth to those from the third. The client's I/F at albedo ϖ is ϖ
//! `I_classical` `f_e` + ϖ² I₂ ÷ (1 − ϖ² `q_e`) + ϖ³ I₃ ÷ (1 − ϖ² `q_o`) ([`multiple_scattering`]):
//! geometric within each parity past the third order (with particles this backscattering the
//! orders alternate), and exact at ϖ = 1 for each (τ, μ, face)'s I/F-weighted sum over its phase
//! nodes, the ratios being pooled over them; its largest error at a single node is measured
//! ([`Checks`]). The single-scattered unlit face stays the classical one.
//!
//! **Reproducible**, by the crate's convention: every draw comes from a counter-based stream keyed
//! by the seed salted with [`RING_STREAM`], the run, the node and the item, and every sum is merged
//! in index order, so the module is the same, byte for byte, on any number of threads.

pub mod layer;
pub(crate) mod trace;

use core::f64::consts::PI;
use core::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;

use self::layer::{
    BuildFieldError, DescribeLayerError, Field, FieldKey, FillingLaw, Layer, PowerLawSizes,
    ProfileShape, SALO_FRENCH_LAW,
};
use self::trace::{
    Face, Observer, Orders, PowerLawPhase, SALO_FRENCH_PHASE_EXPONENT, Tally, trace,
};
use crate::atmosphere::geometry::Vec3;
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::tasks::displaced_forms::births::Draws;

/// The salt of the shadowing table's streams: "ringshad" in ASCII.
pub const RING_STREAM: u64 = 0x7269_6e67_7368_6164;

/// The table's version, in its header: raised whenever the module's layout or meaning changes.
pub const TABLE_VERSION: u32 = 1;

/// Where the committed module lives, from the repository's root.
pub const MODULE_PATH: &str = "apps/hyperion/src/renderer/src/view/rings/shadowingTable.ts";

/// The table's radii, Salo and French 2010's own 0.5–5 m (W = 10, which their opposition fits
/// favour; Design note 15), not plan 14's 1 cm–5 m, which stays the slab's optical-depth budget.
pub const RADII_M: (f64, f64) = (0.5, 5.0);

/// The cross-check's radii, 0.1–5 m (W = 50, the widest range for which Salo and French's tilt
/// results hold; Design note 15).
pub const CROSS_CHECK_RADII_M: (f64, f64) = (0.1, 5.0);

/// The photometric match's `τ_dyn`, 2.0 (Salo and French 2010, §5.2; Design note 15).
pub const PHOTOMETRY_TAU_DYN: f64 = 2.0;

/// The photometric match's Bond albedos at 814 and 336 nm, 0.57 and 0.21 (Salo and French 2010,
/// §5.2; Design note 15).
pub const PHOTOMETRY_ALBEDOS: [f64; 2] = [0.57, 0.21];

/// The uniform layer's filling factor, D = 0.05: the brainstorm's first slab, which must fail the
/// tilt test (Design note 15).
pub const UNIFORM_FILLING_FACTOR: f64 = 0.05;

/// The albedo at which the reduction's error is measured: the B ring's at 814 nm (Salo and French
/// 2010, §5.2).
const CHECK_ALBEDO: f64 = 0.57;

/// The orders a photometric run tallies one by one; the last bucket holds the rest.
const PHOTOMETRY_BUCKETS: usize = 64;

/// The most secant steps that solve a node's `τ_dyn` for its `τ_phot`, the last field set kept.
const SOLVE_STEPS: u32 = 5;

/// The |ln(`τ_phot` ÷ target)| within which a node's solve stops: 0.2%.
const SOLVE_TOLERANCE: f64 = 0.002;

/// The least and the most d ln `τ_phot` ÷ d ln `τ_dyn` a secant step takes.
const SOLVE_SLOPES: (f64, f64) = (0.8, 2.5);

/// The classical prefactor μ₀ ÷ (μ + μ₀) at μ = μ₀, by which the lit orders are divided.
const SYMMETRIC_PREFACTOR: f64 = 0.5;

/// The I/F below which a node's relative errors are not counted: at ϖ = 1 the dark unlit face of
/// a dense ring, where a ratio would only show the noise.
const COUNTED_REFLECTANCE: f64 = 1e-3;

/// A shadowing run failed.
#[derive(Debug, thiserror::Error)]
pub enum RunShadowingError {
    /// A layer could not be described.
    #[error(transparent)]
    Layer(#[from] DescribeLayerError),
    /// A field could not be built.
    #[error(transparent)]
    Field(#[from] BuildFieldError),
    /// The thread pool could not be built.
    #[error(transparent)]
    Pool(#[from] BuildThreadPoolError),
    /// No photon of a normal-transmission count got through, so `τ_phot` cannot be measured.
    #[error("no normal ray of {rays} got through the layer at τ_dyn {tau_dyn}")]
    Opaque {
        /// The rays cast.
        rays: u64,
        /// The layer's `τ_dyn`.
        tau_dyn: f64,
    },
    /// An entry lay outside its encoding's range.
    #[error("{quantity} = {value} at node ({tau}, {mu}, {phase}) lies outside [0, {max}]")]
    Encode {
        /// The quantity.
        quantity: &'static str,
        /// The τ node.
        tau: usize,
        /// The μ node.
        mu: usize,
        /// The phase node.
        phase: usize,
        /// The value.
        value: f64,
        /// The encoding's largest value.
        max: f64,
    },
}

/// The table's three axes and their node spacings (module documentation).
///
/// - `τ_k` = `s_τ` ((1 + `τ_max` ÷ `s_τ`)^(k ÷ (`n_τ` − 1)) − 1), so `u_τ` = ln(1 + τ ÷ `s_τ`) ÷
///   ln(1 + `τ_max` ÷ `s_τ`) is the texture coordinate;
/// - `μ_m` = `μ_min` + (1 − `μ_min`) (m ÷ (`n_μ` − 1))², so `u_μ` = √((`μ_eff` − `μ_min`) ÷
///   (1 − `μ_min`));
/// - `v_j` = `s_v` ((1 + 1 ÷ `s_v`)^(j ÷ (`n_v` − 1)) − 1), so `u_v` = ln(1 + v ÷ `s_v`) ÷
///   ln(1 + 1 ÷ `s_v`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axes {
    tau_nodes: usize,
    tau_scale: f64,
    tau_max: f64,
    mu_nodes: usize,
    mu_min: f64,
    phase_nodes: usize,
    phase_scale: f64,
}

impl Axes {
    /// The fraction k ÷ (n − 1) of node `k` of `n`.
    #[must_use]
    fn fraction(k: usize, n: usize) -> f64 {
        let to_f64 = |x: usize| f64::from(u32::try_from(x).expect("axes hold a few nodes"));
        to_f64(k) / to_f64(n - 1)
    }

    /// `τ_phot` at node `k`.
    #[must_use]
    pub fn tau(&self, k: usize) -> f64 {
        let f = Self::fraction(k, self.tau_nodes);
        self.tau_scale * (math::powf(1.0 + self.tau_max / self.tau_scale, f) - 1.0)
    }

    /// `μ_eff` at node `m`.
    #[must_use]
    pub fn mu(&self, m: usize) -> f64 {
        let f = Self::fraction(m, self.mu_nodes);
        (self.mu_min + (1.0 - self.mu_min) * f * f).min(1.0)
    }

    /// v at node `j`.
    #[must_use]
    pub fn phase_fraction(&self, j: usize) -> f64 {
        let f = Self::fraction(j, self.phase_nodes);
        self.phase_scale * (math::powf(1.0 + 1.0 / self.phase_scale, f) - 1.0)
    }

    /// The phase angle of node `j` at elevation sine `mu` on `face`, rad.
    #[must_use]
    pub(crate) fn phase_rad(&self, face: Face, mu: f64, j: usize) -> f64 {
        let range = (PI - 2.0 * math::asin(mu.min(1.0))).max(0.0);
        let v = self.phase_fraction(j);
        match face {
            Face::Lit => v * range,
            Face::Unlit => PI - v * range,
        }
    }

    /// The entries of one quantity over the three axes.
    #[must_use]
    pub fn cells(&self) -> usize {
        self.tau_nodes * self.mu_nodes * self.phase_nodes
    }

    /// The flat index of node (k, m, j), v fastest.
    #[must_use]
    fn cell(&self, k: usize, m: usize, j: usize) -> usize {
        (k * self.mu_nodes + m) * self.phase_nodes + j
    }
}

/// How much a run traces: its box, its fields and its photons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Statistics {
    box_dm: u32,
    realisations: u32,
    photons: u64,
    normal_rays: u64,
}

impl Statistics {
    /// The box's side, m.
    #[must_use]
    pub fn box_m(&self) -> f64 {
        f64::from(self.box_dm) / 10.0
    }

    /// The fields, each drawn afresh.
    #[must_use]
    pub fn realisations(&self) -> u32 {
        self.realisations
    }

    /// The photons per field and geometry.
    #[must_use]
    pub fn photons(&self) -> u64 {
        self.photons
    }

    /// The normal rays per field that measure `τ_phot`.
    #[must_use]
    pub fn normal_rays(&self) -> u64 {
        self.normal_rays
    }

    /// These statistics in a box of side `box_m`, m (to the decimetre).
    #[must_use]
    pub fn in_box(self, box_m: f64) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped into u32's range first"
        )]
        let decimetres = (box_m * 10.0).round().clamp(1.0, f64::from(u32::MAX)) as u32;
        Self {
            box_dm: decimetres,
            ..self
        }
    }
}

/// A whole run: the table's inputs and the acceptance's statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableSpec {
    seed: u64,
    axes: Axes,
    table: Statistics,
    acceptance: Statistics,
}

/// The committed table's run: 10 × 12 × 20 nodes, four 200 m fields a τ node and 10⁵ photons a
/// field and elevation; the acceptance at four 160 m fields and 4 × 10⁵ photons a field.
pub const FULL: TableSpec = TableSpec {
    seed: 0,
    axes: Axes {
        tau_nodes: 10,
        tau_scale: 0.25,
        tau_max: 5.0,
        mu_nodes: 12,
        mu_min: 0.017_452_406_437_283_51,
        phase_nodes: 20,
        phase_scale: 0.002,
    },
    table: Statistics {
        box_dm: 2000,
        realisations: 4,
        photons: 100_000,
        normal_rays: 1_000_000,
    },
    acceptance: Statistics {
        box_dm: 1600,
        realisations: 4,
        photons: 400_000,
        normal_rays: 1_000_000,
    },
};

/// A smoke run of the same shape at a few nodes, in seconds: for the command's own tests.
pub const SMOKE: TableSpec = TableSpec {
    seed: 0,
    axes: Axes {
        tau_nodes: 3,
        tau_scale: 0.25,
        tau_max: 1.5,
        mu_nodes: 3,
        mu_min: 0.017_452_406_437_283_51,
        phase_nodes: 4,
        phase_scale: 0.002,
    },
    table: Statistics {
        box_dm: 600,
        realisations: 1,
        photons: 300,
        normal_rays: 3_000,
    },
    acceptance: Statistics {
        box_dm: 600,
        realisations: 1,
        photons: 300,
        normal_rays: 3_000,
    },
};

/// The slow test's acceptance statistics: the committed table's own, so that the test reruns the
/// figures in the module's header, the same bit for bit on any number of threads. Two fields of
/// 10⁵ photons gave a tilt ratio of 1.250 at α = 6° against 1.262 here, the fields' own scatter
/// (the few largest spheres a box holds) being too large for the band's lower edge, 1.25.
pub const SLOW_TEST_ACCEPTANCE: Statistics = FULL.acceptance;

impl TableSpec {
    /// The seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The acceptance's statistics.
    #[must_use]
    pub fn acceptance(&self) -> Statistics {
        self.acceptance
    }
}

/// What a stream is for, in its key's top bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Field = 1,
    Photons = 2,
    NormalRays = 3,
}

/// The runs, in a stream key's second field: the table's τ nodes are 1–255.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    TauNode(usize),
    Tilt,
    Uniform,
    CrossCheck,
    Photometry,
    Reduction,
}

impl Run {
    /// The run's code, 12 bits, a τ node's below 0x100.
    ///
    /// # Panics
    ///
    /// For a τ node past 254, whose code would be another run's.
    #[must_use]
    fn code(self) -> u64 {
        match self {
            Self::TauNode(k) => {
                let code = u64::try_from(k).expect("a few nodes") + 1;
                assert!(code < 0x100, "τ node {k} past the 255 a key holds");
                code
            }
            Self::Tilt => 0x100,
            Self::Uniform => 0x101,
            Self::CrossCheck => 0x102,
            Self::Photometry => 0x103,
            Self::Reduction => 0x104,
        }
    }
}

/// The stream key of `purpose` in `run`, for items `a` and `b`.
///
/// # Panics
///
/// If `a` or `b` is not below 2²⁴, where it would spill into another field of the key.
#[must_use]
fn stream(seed: u64, purpose: Purpose, run: Run, a: u64, b: u64) -> FieldKey {
    assert!(
        a < 1 << 24 && b < 1 << 24,
        "stream fields {a} and {b} past 2²⁴"
    );
    FieldKey {
        seed: seed ^ RING_STREAM,
        key: ((purpose as u64) << 60) | (run.code() << 48) | (a << 24) | b,
    }
}

/// The photons' streams of one trace over a run's fields: the run's seed and run, and the item
/// within it (an elevation's node, a geometry's index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhotonStreams {
    seed: u64,
    run: Run,
    item: u64,
}

/// A layer's `τ_phot` measured on its fields: −ln of the fraction of normal rays through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transmission {
    rays: u64,
    through: u64,
}

impl Transmission {
    /// `τ_phot` = −ln(through ÷ rays).
    #[must_use]
    pub fn tau(&self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
        let t = self.through as f64 / self.rays as f64;
        -math::ln(t)
    }

    /// `τ_phot`'s ray-sampling standard error, √((1 − T) ÷ (N T)): of the rays cast on these
    /// fields, not of the fields' own scatter (about 1% a field).
    #[must_use]
    pub fn tau_error(&self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
        let (n, t) = (self.rays as f64, self.through as f64 / self.rays as f64);
        ((1.0 - t) / (n * t)).sqrt()
    }

    fn add(&mut self, other: Self) {
        self.rays += other.rays;
        self.through += other.through;
    }
}

/// The normal rays through `field` that meet no sphere, of `rays` cast at points uniform over the
/// box.
fn normal_transmission(
    field: &Field,
    rays: u64,
    key: FieldKey,
    threads: NonZeroUsize,
) -> Result<Transmission, BuildThreadPoolError> {
    let l = field.box_m();
    let down = Vec3::new(0.0, 0.0, -1.0);
    let mut through = 0;
    map_reduce_chunks(
        rays,
        4096,
        threads,
        |range| {
            range
                .filter(|&i| {
                    let mut draws = Draws::new(key.seed, key.key, i);
                    let origin = Vec3::new(l * draws.uniform(), l * draws.uniform(), field.top_m());
                    !field.blocked(origin, down)
                })
                .count()
        },
        |count| through += u64::try_from(count).expect("counts fit a u64"),
    )?;
    Ok(Transmission { rays, through })
}

/// The classical layer's lit-face single-scattering I/F at ϖ = 1 (Chandrasekhar 1960, §I.11;
/// Design note 14): P μ₀ ÷ (4(μ + μ₀)) × [1 − e^(−τ(1/μ + 1/μ₀))].
///
/// `phase_value` is P(α), the phase function's value at the phase angle, normalised over 4π (not
/// an angle); `tau` the normal optical depth, at least 0; `mu` and `mu0` the viewer's and the
/// star's elevation sines, in (0, 1].
#[must_use]
pub fn classical_lit(phase_value: f64, tau: f64, mu: f64, mu0: f64) -> f64 {
    phase_value * mu0 / (4.0 * (mu + mu0)) * -math::exp_m1(-tau * (1.0 / mu + 1.0 / mu0))
}

/// The classical layer's unlit-face single-scattering I/F at ϖ = 1, the transmitted beam scattered
/// once (Design note 14): P μ₀ ÷ (4(μ − μ₀)) × [e^(−τ/μ) − e^(−τ/μ₀)], and P τ e^(−τ/μ) ÷ 4μ at
/// μ = μ₀; the arguments as [`classical_lit`]'s.
#[must_use]
fn classical_unlit(phase_value: f64, tau: f64, mu: f64, mu0: f64) -> f64 {
    if (mu - mu0).abs() <= 1e-9 * mu.max(mu0) {
        return phase_value * tau * math::exp(-tau / mu) / (4.0 * mu);
    }
    phase_value * mu0 / (4.0 * (mu - mu0)) * (math::exp(-tau / mu) - math::exp(-tau / mu0))
}

/// Hapke's shadow-hiding opposition term as a ring's shadowing factor, the table's measured
/// fallback (Design note 15): 1 + B₀ ÷ (1 + tan(α/2) ÷ h) with B₀ = 1, h = −(3/8) Y ln(1 −
/// `D_eff`), Y = √3 ÷ ln W for an R⁻³ layer of radius ratio W, and `D_eff` = D₀ `μ_eff` (Salo and
/// French 2010, eq. 18, after Hapke 1986, Icarus 67, 264; the form from memory, not re-checked:
/// the pod had no web). B₀ = 1 is this task's lean (2026-11-14), the full opposition of a
/// semi-infinite layer; R11.T9.a's `hapkeShadowHiding` is to match it.
///
/// `alpha_rad` is the phase angle, rad; `filling_factor` D₀ in [0, 1); `mu_eff` in [0, 1];
/// `ratio` W, above 1.
#[must_use]
pub fn hapke_shadow_hiding(alpha_rad: f64, filling_factor: f64, mu_eff: f64, ratio: f64) -> f64 {
    let effective = filling_factor * mu_eff;
    let y = 3.0_f64.sqrt() / math::ln(ratio);
    let h = -(3.0 / 8.0) * y * math::ln_1p(-effective);
    if h <= 0.0 {
        return 1.0;
    }
    1.0 + 1.0 / (1.0 + math::tan(0.5 * alpha_rad.max(0.0)) / h)
}

/// The tilt test's two elevations (Salo and French 2010, §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TiltElevation {
    /// B = 4.5°.
    Low,
    /// B = 26°.
    High,
}

impl TiltElevation {
    /// Both, low first.
    pub const BOTH: [Self; 2] = [Self::Low, Self::High];

    /// The elevation, deg.
    #[must_use]
    pub fn degrees(self) -> f64 {
        match self {
            Self::Low => 4.5,
            Self::High => 26.0,
        }
    }
}

/// The tilt test's two phase angles (Design note 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TiltPhase {
    /// α = 0.5°, in the opposition peak.
    Narrow,
    /// α = 6°, the test's own.
    Wide,
}

impl TiltPhase {
    /// Both, narrow first.
    pub const BOTH: [Self; 2] = [Self::Narrow, Self::Wide];

    /// The phase angle, deg.
    #[must_use]
    pub fn degrees(self) -> f64 {
        match self {
            Self::Narrow => 0.5,
            Self::Wide => 6.0,
        }
    }
}

/// The tilt test's shadowing factors `f_e` at its two elevations and two phase angles, with the
/// layer's measured `τ_phot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tilt {
    /// By elevation, then phase, in [`TiltElevation::BOTH`]'s and [`TiltPhase::BOTH`]'s orders.
    shadowing: [[f64; 2]; 2],
    tau_phot: f64,
}

impl Tilt {
    /// `f_e` at `elevation` and `phase`.
    #[must_use]
    pub fn shadowing(&self, elevation: TiltElevation, phase: TiltPhase) -> f64 {
        let e = usize::from(elevation == TiltElevation::High);
        let p = usize::from(phase == TiltPhase::Wide);
        self.shadowing[e][p]
    }

    /// `f_e`(26°) ÷ `f_e`(4.5°) at `phase`.
    #[must_use]
    pub fn ratio(&self, phase: TiltPhase) -> f64 {
        self.shadowing(TiltElevation::High, phase) / self.shadowing(TiltElevation::Low, phase)
    }

    /// The layer's `τ_phot` as measured.
    #[must_use]
    pub fn tau_phot(&self) -> f64 {
        self.tau_phot
    }
}

/// Builds `layer`'s fields under `statistics` for `run`, with their pooled `τ_phot`.
fn fields(
    layer: &Layer,
    statistics: Statistics,
    seed: u64,
    run: Run,
    threads: NonZeroUsize,
) -> Result<(Vec<Field>, Transmission), RunShadowingError> {
    let mut built = Vec::new();
    let mut transmission = Transmission {
        rays: 0,
        through: 0,
    };
    for r in 0..u64::from(statistics.realisations) {
        let field = Field::build(
            layer,
            statistics.box_m(),
            stream(seed, Purpose::Field, run, r, 0),
        )?;
        transmission.add(normal_transmission(
            &field,
            statistics.normal_rays,
            stream(seed, Purpose::NormalRays, run, r, 0),
            threads,
        )?);
        built.push(field);
    }
    if transmission.through == 0 {
        return Err(RunShadowingError::Opaque {
            rays: transmission.rays,
            tau_dyn: layer.dynamical_optical_depth(),
        });
    }
    Ok((built, transmission))
}

/// The tally of `photons` photons a field through each of `built` from a star at elevation `b`,
/// rad, towards `observers`, the field's index and `streams` keying the photons' draws.
fn trace_fields(
    built: &[Field],
    b: f64,
    observers: &[Observer],
    photons: u64,
    orders: Orders,
    streams: PhotonStreams,
    threads: NonZeroUsize,
) -> Result<Tally, RunShadowingError> {
    let mut tally: Option<Tally> = None;
    for (r, field) in (0_u64..).zip(built) {
        let key = stream(streams.seed, Purpose::Photons, streams.run, r, streams.item);
        let one = trace(field, b, observers, photons, orders, key, threads)?;
        match &mut tally {
            Some(t) => t.add(&one),
            None => tally = Some(one),
        }
    }
    Ok(tally.expect("at least one realisation"))
}

/// The tilt test on `layer` for `run`: `f_e` at B = 4.5° and 26°, α = 0.5° and 6°, single
/// scattering.
fn tilt(
    layer: &Layer,
    statistics: Statistics,
    seed: u64,
    run: Run,
    threads: NonZeroUsize,
) -> Result<Tilt, RunShadowingError> {
    let (built, transmission) = fields(layer, statistics, seed, run, threads)?;
    let tau_phot = transmission.tau();
    let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
    let mut shadowing = [[0.0; 2]; 2];
    for ((e, elevation), item) in TiltElevation::BOTH.into_iter().enumerate().zip(0_u64..) {
        let b = elevation.degrees().to_radians();
        let observers =
            TiltPhase::BOTH.map(|p| Observer::at_phase(Face::Lit, b, b, p.degrees().to_radians()));
        let streams = PhotonStreams { seed, run, item };
        let tally = trace_fields(
            &built,
            b,
            &observers,
            statistics.photons,
            Orders::First,
            streams,
            threads,
        )?;
        let mu = math::sin(b);
        for (p, observer) in observers.iter().enumerate() {
            let classical = classical_lit(phase.value(observer.phase_rad()), tau_phot, mu, mu);
            shadowing[e][p] = tally.reflectance(mu, p, observer, 0) / classical;
        }
    }
    Ok(Tilt {
        shadowing,
        tau_phot,
    })
}

/// The multiple scattering of the table's order model at albedo `albedo` (module documentation):
/// ϖ² I₂ ÷ (1 − ϖ² `q_e`) + ϖ³ I₃ ÷ (1 − ϖ² `q_o`).
///
/// `albedo` is ϖ in [0, 1]; `second` and `third` are the second and third orders' I/F at ϖ = 1
/// (the lit face's the table's times the geometry's μ₀ ÷ (μ + μ₀)); `even_ratio` and `odd_ratio`
/// are `q_e` and `q_o`, in [0, 1), so that ϖ² q stays below 1.
#[must_use]
pub fn multiple_scattering(
    albedo: f64,
    second: f64,
    third: f64,
    even_ratio: f64,
    odd_ratio: f64,
) -> f64 {
    let w2 = albedo * albedo;
    w2 * second / (1.0 - w2 * even_ratio) + w2 * albedo * third / (1.0 - w2 * odd_ratio)
}

/// I/F at ϖ = 1 by order, from a tally of [`PHOTOMETRY_BUCKETS`] buckets towards its observer
/// `index`: the exact I/F at each of `albedos`, and the ratios `q_e` and `q_o` of the same orders.
#[must_use]
fn exact_by_order(orders: &[f64], albedos: &[f64]) -> (Vec<f64>, (f64, f64)) {
    let (even, odd) =
        orders
            .iter()
            .enumerate()
            .skip(3)
            .fold((0.0, 0.0), |(e, o), (index, &value)| {
                // Index 3 is the fourth order, an even one.
                if index % 2 == 1 {
                    (e + value, o)
                } else {
                    (e, o + value)
                }
            });
    let ratio = |first: f64, rest: f64| {
        if first + rest > 0.0 {
            rest / (first + rest)
        } else {
            0.0
        }
    };
    let exact = albedos
        .iter()
        .map(|&albedo| {
            let mut weight = 1.0;
            orders
                .iter()
                .map(|value| {
                    weight *= albedo;
                    weight * value
                })
                .sum()
        })
        .collect();
    (exact, (ratio(orders[1], even), ratio(orders[2], odd)))
}

/// The photometric match's I/F at each albedo: every order summed, and the table's order model of
/// the same tally.
#[derive(Debug, Clone, PartialEq)]
pub struct Photometry {
    exact: Vec<f64>,
    model: Vec<f64>,
}

impl Photometry {
    /// I/F with every order, at each albedo asked.
    #[must_use]
    pub fn exact(&self) -> &[f64] {
        &self.exact
    }

    /// I/F by the table's order model ([`multiple_scattering`]), at each albedo asked.
    #[must_use]
    pub fn model(&self) -> &[f64] {
        &self.model
    }
}

/// The photometric match on `layer`: I/F at B = B′ = 4.5° and α = 6° at each of `albedos`, every
/// order included, and by the table's order model.
fn photometry(
    layer: &Layer,
    statistics: Statistics,
    seed: u64,
    albedos: &[f64],
    threads: NonZeroUsize,
) -> Result<Photometry, RunShadowingError> {
    let (built, _) = fields(layer, statistics, seed, Run::Photometry, threads)?;
    let b = TiltElevation::Low.degrees().to_radians();
    let alpha = TiltPhase::Wide.degrees().to_radians();
    let observer = [Observer::at_phase(Face::Lit, b, b, alpha)];
    let streams = PhotonStreams {
        seed,
        run: Run::Photometry,
        item: 0,
    };
    let tally = trace_fields(
        &built,
        b,
        &observer,
        statistics.photons,
        Orders::Buckets(PHOTOMETRY_BUCKETS),
        streams,
        threads,
    )?;
    let mu = math::sin(b);
    let orders: Vec<f64> = (0..PHOTOMETRY_BUCKETS)
        .map(|bucket| tally.reflectance(mu, 0, &observer[0], bucket))
        .collect();
    let (exact, (q_even, q_odd)) = exact_by_order(&orders, albedos);
    let model = albedos
        .iter()
        .map(|&albedo| {
            albedo * orders[0] + multiple_scattering(albedo, orders[1], orders[2], q_even, q_odd)
        })
        .collect();
    Ok(Photometry { exact, model })
}

/// The acceptance figures, in the module's header and the slow test (Design note 15).
#[derive(Debug, Clone, PartialEq)]
pub struct Acceptance {
    reference: Layer,
    field_tau_dyn: f64,
    field_filling_factor: f64,
    tilt: Tilt,
    uniform: Tilt,
    cross_check: Tilt,
    cross_check_layer: Layer,
    photometry: Photometry,
    photometry_layer: Layer,
}

impl Acceptance {
    /// The reference layer, `τ_dyn` = 1.5 under the law.
    #[must_use]
    pub fn reference(&self) -> &Layer {
        &self.reference
    }

    /// The reference's first field's own `τ_dyn` and D₀.
    #[must_use]
    pub fn field_layer(&self) -> (f64, f64) {
        (self.field_tau_dyn, self.field_filling_factor)
    }

    /// The tilt test on the reference layer.
    #[must_use]
    pub fn tilt(&self) -> &Tilt {
        &self.tilt
    }

    /// The tilt test on the uniform layer at D = 0.05.
    #[must_use]
    pub fn uniform(&self) -> &Tilt {
        &self.uniform
    }

    /// The tilt test on the cross-check's 0.1–5 m layer, and that layer.
    #[must_use]
    pub fn cross_check(&self) -> (&Tilt, &Layer) {
        (&self.cross_check, &self.cross_check_layer)
    }

    /// The photometric match's I/F at [`PHOTOMETRY_ALBEDOS`], and its layer.
    #[must_use]
    pub fn photometry(&self) -> (&[f64], &Layer) {
        (self.photometry.exact(), &self.photometry_layer)
    }

    /// The photometric match's I/F by the table's order model, beside [`Acceptance::photometry`]'s.
    #[must_use]
    pub fn photometry_model(&self) -> &[f64] {
        self.photometry.model()
    }

    /// Hapke's term's tilt ratio at `phase`, with the reference's D₀ and W.
    #[must_use]
    pub fn hapke_ratio(&self, phase: TiltPhase) -> f64 {
        let d0 = self.reference.central_filling_factor();
        let w = self.reference.sizes().radius_ratio();
        let alpha = phase.degrees().to_radians();
        let at = |e: TiltElevation| {
            hapke_shadow_hiding(alpha, d0, math::sin(e.degrees().to_radians()), w)
        };
        at(TiltElevation::High) / at(TiltElevation::Low)
    }
}

/// Runs the acceptance under `statistics` for radii `radii_m` (and the cross-check's): the
/// reference layer's D₀ and fields, the tilt test, the uniform layer's, the cross-check's and the
/// photometric match.
///
/// # Errors
///
/// [`RunShadowingError`] if a layer cannot be described, or a field built or traced.
pub fn acceptance(
    radii_m: (f64, f64),
    statistics: Statistics,
    seed: u64,
    threads: NonZeroUsize,
) -> Result<Acceptance, RunShadowingError> {
    let sizes = PowerLawSizes::new(radii_m.0, radii_m.1)?;
    let law = SALO_FRENCH_LAW;
    let reference = law.layer(sizes, law.reference_optical_depth())?;
    let first = Field::build(
        &reference,
        statistics.box_m(),
        stream(seed, Purpose::Field, Run::Tilt, 0, 0),
    )?;
    let tilt_of_reference = tilt(&reference, statistics, seed, Run::Tilt, threads)?;
    let uniform_layer = Layer::with_filling_factor(
        sizes,
        ProfileShape::Uniform,
        law.reference_optical_depth(),
        UNIFORM_FILLING_FACTOR,
    )?;
    let uniform = tilt(&uniform_layer, statistics, seed, Run::Uniform, threads)?;
    let cross_sizes = PowerLawSizes::new(CROSS_CHECK_RADII_M.0, CROSS_CHECK_RADII_M.1)?;
    let cross_check_layer = law.layer(cross_sizes, law.reference_optical_depth())?;
    let cross_check = tilt(
        &cross_check_layer,
        statistics,
        seed,
        Run::CrossCheck,
        threads,
    )?;
    let photometry_layer = law.layer(sizes, PHOTOMETRY_TAU_DYN)?;
    let photometry = photometry(
        &photometry_layer,
        statistics,
        seed,
        &PHOTOMETRY_ALBEDOS,
        threads,
    )?;
    Ok(Acceptance {
        reference,
        field_tau_dyn: first.dynamical_optical_depth(),
        field_filling_factor: first.central_filling_factor(),
        tilt: tilt_of_reference,
        uniform,
        cross_check,
        cross_check_layer,
        photometry,
        photometry_layer,
    })
}

/// A τ node as traced: its `τ_phot`, the `τ_dyn` solved for it, and its layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TauNode {
    target: f64,
    measured: Transmission,
    layer: Option<Layer>,
    particles: usize,
}

impl TauNode {
    /// The node's `τ_phot`.
    #[must_use]
    pub fn target(&self) -> f64 {
        self.target
    }

    /// `τ_phot` as measured on its fields (the target at node 0).
    #[must_use]
    pub fn measured(&self) -> f64 {
        if self.layer.is_some() {
            self.measured.tau()
        } else {
            self.target
        }
    }

    /// The measured `τ_phot`'s ray-sampling standard error ([`Transmission::tau_error`]).
    #[must_use]
    pub fn measured_error(&self) -> f64 {
        if self.layer.is_some() {
            self.measured.tau_error()
        } else {
            0.0
        }
    }

    /// The layer traced, `None` at τ = 0.
    #[must_use]
    pub fn layer(&self) -> Option<&Layer> {
        self.layer.as_ref()
    }

    /// The spheres in each of its fields.
    #[must_use]
    pub fn particles(&self) -> usize {
        self.particles
    }
}

/// The quantities over the three axes, in the module's order.
pub const QUANTITIES: [&str; 5] = [
    "shadowing",
    "litOrder2",
    "litOrder3",
    "unlitOrder2",
    "unlitOrder3",
];

/// The quantities over τ and μ alone, in the module's order.
pub const RATIOS: [&str; 4] = [
    "litEvenRatio",
    "litOddRatio",
    "unlitEvenRatio",
    "unlitOddRatio",
];

/// The codes in one unit of `f_e`: `f_e` in [0, 4).
const SHADOWING_CODES_PER_UNIT: u32 = 16_384;

/// The codes in one unit of an order's I/F (normalised as the module documentation says): each in
/// [0, 2).
const ORDER_CODES_PER_UNIT: u32 = 32_768;

/// The codes in one unit of a ratio: each in [0, 1].
const RATIO_CODES_PER_UNIT: u32 = 65_535;

/// The table's own checks of its models, in its header: Hapke's fallback against `f_e`, the order
/// model against the orders it pools, and the `μ_eff` reduction against geometries with B ≠ B′.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checks {
    hapke_near_max: f64,
    hapke_near_rms: f64,
    hapke_all_max: f64,
    hapke_all_rms: f64,
    hapke_dense_max: f64,
    hapke_dense_rms: f64,
    order_model_max: [[f64; 2]; 2],
    reduction_max: [f64; 2],
}

impl Checks {
    /// max and RMS |`B_H` − `f_e`| over the lit nodes with τ > 0 at α ≤ 30°.
    #[must_use]
    pub fn hapke_near(&self) -> (f64, f64) {
        (self.hapke_near_max, self.hapke_near_rms)
    }

    /// max and RMS |`B_H` − `f_e`| over the lit nodes with τ > 0 at every α.
    #[must_use]
    pub fn hapke_all(&self) -> (f64, f64) {
        (self.hapke_all_max, self.hapke_all_rms)
    }

    /// max and RMS |`B_H` − `f_e`| over the dense rings' lit nodes, `τ_phot` ≥ 1, at α ≤ 30°.
    #[must_use]
    pub fn hapke_dense(&self) -> (f64, f64) {
        (self.hapke_dense_max, self.hapke_dense_rms)
    }

    /// The order model's largest relative error in a node's I/F, at ϖ = 0.57 and then at ϖ = 1,
    /// each lit face then unlit, over the nodes whose I/F reaches 10⁻³: the price of the ratios'
    /// pooling over phase, greatest where the albedo is.
    #[must_use]
    pub fn order_model_max(&self) -> [[f64; 2]; 2] {
        self.order_model_max
    }

    /// The table's largest relative error in I/F at ϖ = 0.57 against a direct trace of a B ≠ B′
    /// geometry at the same `μ_eff` and node phase, lit face then unlit, over geometries whose I/F
    /// reaches 10⁻³: the price of the `μ_eff` reduction, and on the unlit face of its classical
    /// single scattering too.
    #[must_use]
    pub fn reduction_max(&self) -> [f64; 2] {
        self.reduction_max
    }
}

/// The table, decoded values beside their codes.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowingTable {
    spec: TableSpec,
    nodes: Vec<TauNode>,
    /// Each quantity over (τ, μ, v), v fastest.
    values: [Vec<f64>; 5],
    /// Each ratio over (τ, μ), μ fastest.
    ratios: [Vec<f64>; 4],
    checks: Checks,
    acceptance: Acceptance,
    lost: u64,
    photons: u64,
}

impl ShadowingTable {
    /// The table's checks of its models.
    #[must_use]
    pub fn checks(&self) -> &Checks {
        &self.checks
    }

    /// The acceptance figures.
    #[must_use]
    pub fn acceptance(&self) -> &Acceptance {
        &self.acceptance
    }

    /// The photons lost in the layers' wings or truncated, of those traced.
    #[must_use]
    pub fn lost(&self) -> (u64, u64) {
        (self.lost, self.photons)
    }

    /// The entries as u16 codes: each quantity over (τ, μ, v), then each ratio over (τ, μ).
    ///
    /// # Errors
    ///
    /// [`RunShadowingError::Encode`] for a value outside its encoding.
    pub fn codes(&self) -> Result<Vec<u16>, RunShadowingError> {
        let a = self.spec.axes;
        let mut codes = Vec::with_capacity(5 * a.cells() + 4 * a.tau_nodes * a.mu_nodes);
        let encode = |value: f64, per_unit: u32, quantity, (tau, mu, phase)| {
            let max = 65_535.0 / f64::from(per_unit);
            if !(0.0..=max).contains(&value) {
                return Err(RunShadowingError::Encode {
                    quantity,
                    tau,
                    mu,
                    phase,
                    value,
                    max,
                });
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked to lie in [0, 65,535] codes"
            )]
            let code = (value * f64::from(per_unit)).round() as u16;
            Ok(code)
        };
        for (q, values) in self.values.iter().enumerate() {
            let per_unit = if q == 0 {
                SHADOWING_CODES_PER_UNIT
            } else {
                ORDER_CODES_PER_UNIT
            };
            for (n, &value) in values.iter().enumerate() {
                let at = (
                    n / (a.mu_nodes * a.phase_nodes),
                    (n / a.phase_nodes) % a.mu_nodes,
                    n % a.phase_nodes,
                );
                codes.push(encode(value, per_unit, QUANTITIES[q], at)?);
            }
        }
        for (r, ratios) in self.ratios.iter().enumerate() {
            for (n, &value) in ratios.iter().enumerate() {
                let at = (n / a.mu_nodes, n % a.mu_nodes, 0);
                codes.push(encode(value, RATIO_CODES_PER_UNIT, RATIOS[r], at)?);
            }
        }
        Ok(codes)
    }
}

/// The layer under `law` whose fields give `τ_phot` = `target`, with those fields and their pooled
/// transmission: up to [`SOLVE_STEPS`] secant steps in ln `τ_dyn` against ln `τ_phot` from target ÷
/// (1 + D₀(target)), the first at a slope of 1 and each slope held within [`SOLVE_SLOPES`],
/// stopping within [`SOLVE_TOLERANCE`]. Below the cap ln `τ_phot` rises some 1.6 times as fast as
/// ln `τ_dyn`, D₀ rising with it, where a fixed-point step overshoots; and every step builds the
/// node's own fields, whose scatter in `τ_phot` (about 1% a field) a field of the solve's own would
/// leave in the table.
fn solve_layer(
    target: f64,
    law: FillingLaw,
    statistics: Statistics,
    seed: u64,
    k: usize,
    threads: NonZeroUsize,
) -> Result<(Layer, Vec<Field>, Transmission), RunShadowingError> {
    let sizes = PowerLawSizes::new(RADII_M.0, RADII_M.1)?;
    let mut tau_dyn = target / (1.0 + law.filling_factor(target));
    let mut last: Option<(f64, f64)> = None;
    let mut step = 1;
    loop {
        let layer = law.layer(sizes, tau_dyn)?;
        let (built, transmission) = fields(&layer, statistics, seed, Run::TauNode(k), threads)?;
        let (x, y) = (math::ln(tau_dyn), math::ln(transmission.tau()));
        let miss = math::ln(target) - y;
        if step == SOLVE_STEPS || miss.abs() < SOLVE_TOLERANCE {
            return Ok((layer, built, transmission));
        }
        let slope = match last {
            Some((x0, y0)) if (x - x0).abs() > 1e-9 => {
                ((y - y0) / (x - x0)).clamp(SOLVE_SLOPES.0, SOLVE_SLOPES.1)
            }
            Some(_) | None => 1.0,
        };
        last = Some((x, y));
        tau_dyn = math::exp(x + miss / slope);
        step += 1;
    }
}

/// A node of the table being recorded: its place, its elevation sine and its layer's figures.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeAt {
    k: usize,
    m: usize,
    mu: f64,
    tau_phot: f64,
    filling_factor: f64,
}

/// The largest of `current` and the relative error |model − exact| ÷ exact, where exact reaches
/// [`COUNTED_REFLECTANCE`].
#[must_use]
fn worst(current: f64, model: f64, exact: f64) -> f64 {
    if exact >= COUNTED_REFLECTANCE {
        current.max((model - exact).abs() / exact)
    } else {
        current
    }
}

/// The table's sums as the τ nodes are traced.
struct Accumulator {
    axes: Axes,
    values: [Vec<f64>; 5],
    ratios: [Vec<f64>; 4],
    /// Each lit node's phase angle, `τ_phot` and |`B_H` − `f_e`|.
    hapke: Vec<(f64, f64, f64)>,
    order_model_max: [[f64; 2]; 2],
    reduction_max: [f64; 2],
    lost: u64,
    photons: u64,
}

impl Accumulator {
    #[must_use]
    fn new(axes: Axes) -> Self {
        let plane = axes.tau_nodes * axes.mu_nodes;
        Self {
            axes,
            values: core::array::from_fn(|_| vec![0.0; axes.cells()]),
            ratios: core::array::from_fn(|_| vec![0.0; plane]),
            hapke: Vec::new(),
            order_model_max: [[0.0; 2]; 2],
            reduction_max: [0.0; 2],
            lost: 0,
            photons: 0,
        }
    }

    /// Node `at`'s entries from `tally` of `observers`, the lit ones and then the unlit, in
    /// [`PHOTOMETRY_BUCKETS`] buckets of orders.
    fn record(&mut self, at: NodeAt, observers: &[Observer], tally: &Tally) {
        let a = self.axes;
        let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
        self.lost += tally.lost();
        self.photons += tally.photons();
        // Each node's orders, and per face Σ I₂, Σ even from the fourth, Σ I₃, Σ odd from the
        // fifth (the last bucket's orders taken as its own's parity, a weight below 10⁻¹⁰).
        let mut by_node = Vec::with_capacity(observers.len());
        let mut tails = [[0.0; 4]; 2];
        for (index, observer) in observers.iter().enumerate() {
            let (face, j) = if index < a.phase_nodes {
                (0, index)
            } else {
                (1, index - a.phase_nodes)
            };
            let cell = a.cell(at.k, at.m, j);
            let orders: Vec<f64> = (0..PHOTOMETRY_BUCKETS)
                .map(|b| tally.reflectance(at.mu, index, observer, b))
                .collect();
            let even: f64 = orders.iter().skip(3).step_by(2).sum();
            let odd: f64 = orders.iter().skip(4).step_by(2).sum();
            if face == 0 {
                let alpha = observer.phase_rad();
                let classical = classical_lit(phase.value(alpha), at.tau_phot, at.mu, at.mu);
                let shadowing = orders[0] / classical;
                self.values[0][cell] = shadowing;
                self.values[1][cell] = orders[1] / SYMMETRIC_PREFACTOR;
                self.values[2][cell] = orders[2] / SYMMETRIC_PREFACTOR;
                let ratio = RADII_M.1 / RADII_M.0;
                let hapke = hapke_shadow_hiding(alpha, at.filling_factor, at.mu, ratio);
                self.hapke
                    .push((alpha, at.tau_phot, (hapke - shadowing).abs()));
            } else {
                // The unlit face's orders over μ₀, which is μ here: the light let in.
                self.values[3][cell] = orders[1] / at.mu;
                self.values[4][cell] = orders[2] / at.mu;
            }
            for (sum, value) in tails[face]
                .iter_mut()
                .zip([orders[1], even, orders[2], odd])
            {
                *sum += value;
            }
            by_node.push((face, orders));
        }
        let ratio = |first: f64, rest: f64| {
            if first + rest > 0.0 {
                rest / (first + rest)
            } else {
                0.0
            }
        };
        let q: [(f64, f64); 2] = core::array::from_fn(|face| {
            let [second, even, third, odd] = tails[face];
            (ratio(second, even), ratio(third, odd))
        });
        for (face, (q_even, q_odd)) in q.into_iter().enumerate() {
            self.ratios[2 * face][at.k * a.mu_nodes + at.m] = q_even;
            self.ratios[2 * face + 1][at.k * a.mu_nodes + at.m] = q_odd;
        }
        // The pooled ratios against each node's own orders.
        for (face, orders) in by_node {
            let (q_even, q_odd) = q[face];
            let (exact, _) = exact_by_order(&orders, &[CHECK_ALBEDO, 1.0]);
            for (w, (albedo, exact)) in [CHECK_ALBEDO, 1.0].into_iter().zip(exact).enumerate() {
                let model = albedo * orders[0]
                    + multiple_scattering(albedo, orders[1], orders[2], q_even, q_odd);
                self.order_model_max[w][face] = worst(self.order_model_max[w][face], model, exact);
            }
        }
    }

    /// The checks of the nodes recorded.
    #[must_use]
    fn checks(&self) -> Checks {
        let near = |alpha: f64| alpha <= 30_f64.to_radians();
        let stats = |keep: &dyn Fn(f64, f64) -> bool| -> (f64, f64) {
            let errors: Vec<f64> = self
                .hapke
                .iter()
                .filter(|&&(alpha, tau, _)| keep(alpha, tau))
                .map(|&(_, _, e)| e)
                .collect();
            if errors.is_empty() {
                return (0.0, 0.0);
            }
            let max = errors.iter().copied().fold(0.0, f64::max);
            let mean_square = errors.iter().map(|e| e * e).sum::<f64>()
                / f64::from(u32::try_from(errors.len()).expect("a few thousand nodes"));
            (max, mean_square.sqrt())
        };
        let (hapke_near_max, hapke_near_rms) = stats(&|alpha, _| near(alpha));
        let (hapke_all_max, hapke_all_rms) = stats(&|_, _| true);
        let (hapke_dense_max, hapke_dense_rms) = stats(&|alpha, tau| near(alpha) && tau >= 1.0);
        Checks {
            hapke_near_max,
            hapke_near_rms,
            hapke_all_max,
            hapke_all_rms,
            hapke_dense_max,
            hapke_dense_rms,
            order_model_max: self.order_model_max,
            reduction_max: self.reduction_max,
        }
    }

    /// The I/F at [`CHECK_ALBEDO`] the table gives node (k, m, j) on `face` for a geometry of
    /// sines `mu` and `mu0` and phase function value `phase_value`, on a layer of `τ_phot` `tau`.
    #[must_use]
    fn table_reflectance(
        &self,
        face: Face,
        node: (usize, usize, usize),
        geometry: Geometry,
    ) -> f64 {
        let (tau_node, mu_node, phase_node) = node;
        let cell = self.axes.cell(tau_node, mu_node, phase_node);
        let plane = tau_node * self.axes.mu_nodes + mu_node;
        let Geometry {
            mu,
            mu0,
            tau,
            phase_value,
        } = geometry;
        let albedo = CHECK_ALBEDO;
        match face {
            Face::Lit => {
                let prefactor = mu0 / (mu + mu0);
                albedo * classical_lit(phase_value, tau, mu, mu0) * self.values[0][cell]
                    + multiple_scattering(
                        albedo,
                        self.values[1][cell] * prefactor,
                        self.values[2][cell] * prefactor,
                        self.ratios[0][plane],
                        self.ratios[1][plane],
                    )
            }
            Face::Unlit => {
                albedo * classical_unlit(phase_value, tau, mu, mu0)
                    + multiple_scattering(
                        albedo,
                        self.values[3][cell] * mu0,
                        self.values[4][cell] * mu0,
                        self.ratios[2][plane],
                        self.ratios[3][plane],
                    )
            }
        }
    }
}

/// A geometry for [`Accumulator::table_reflectance`]: the viewer's and the star's elevation sines,
/// the layer's `τ_phot` and the phase function's value at the phase angle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Geometry {
    mu: f64,
    mu0: f64,
    tau: f64,
    phase_value: f64,
}

/// The τ node at which the `μ_eff` reduction is checked: the middle of the axis.
#[must_use]
fn reduction_node(axes: &Axes) -> usize {
    axes.tau_nodes / 2
}

/// The μ nodes at which the `μ_eff` reduction is checked: a third and a half of the way along the
/// axis, where both 2 `μ_eff` ÷ 3 and 2 `μ_eff` are sines.
#[must_use]
fn reduction_mu_nodes(axes: &Axes) -> [usize; 2] {
    [axes.mu_nodes / 3, axes.mu_nodes / 2]
}

/// Checks the `μ_eff` reduction at τ node `k` on its fields `built` (module documentation): for
/// each of [`reduction_mu_nodes`], the geometries (μ, μ₀) = (2 `μ_eff` ÷ 3, 2 `μ_eff`) and its
/// swap, whose harmonic mean is `μ_eff`, traced directly at every node phase each face allows, and
/// the table's I/F at ϖ = 0.57 held against theirs.
fn check_reduction(
    spec: &TableSpec,
    k: usize,
    built: &[Field],
    tau_phot: f64,
    sums: &mut Accumulator,
    threads: NonZeroUsize,
) -> Result<(), RunShadowingError> {
    let a = spec.axes;
    let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
    let mut item = 0_u64;
    for m in reduction_mu_nodes(&a) {
        let mu_eff = a.mu(m);
        if 2.0 * mu_eff > 1.0 {
            continue;
        }
        for (mu, mu0) in [
            (2.0 * mu_eff / 3.0, 2.0 * mu_eff),
            (2.0 * mu_eff, 2.0 * mu_eff / 3.0),
        ] {
            let (b, b0) = (math::asin(mu), math::asin(mu0));
            let mut observers = Vec::new();
            let mut nodes = Vec::new();
            for face in [Face::Lit, Face::Unlit] {
                // The phase angles the geometry reaches on each face.
                let (lo, hi) = match face {
                    Face::Lit => ((b - b0).abs(), PI - (b + b0)),
                    Face::Unlit => (b + b0, PI - (b - b0).abs()),
                };
                for j in 0..a.phase_nodes {
                    let alpha = a.phase_rad(face, mu_eff, j);
                    if alpha > lo + 1e-6 && alpha < hi - 1e-6 {
                        observers.push(Observer::at_phase(face, b, b0, alpha));
                        nodes.push((face, j));
                    }
                }
            }
            let streams = PhotonStreams {
                seed: spec.seed,
                run: Run::Reduction,
                item: (u64::try_from(k).expect("a few nodes") << 8) | item,
            };
            item += 1;
            let tally = trace_fields(
                built,
                b0,
                &observers,
                spec.table.photons,
                Orders::Buckets(PHOTOMETRY_BUCKETS),
                streams,
                threads,
            )?;
            for (index, (observer, (face, j))) in observers.iter().zip(nodes).enumerate() {
                let orders: Vec<f64> = (0..PHOTOMETRY_BUCKETS)
                    .map(|bucket| tally.reflectance(mu0, index, observer, bucket))
                    .collect();
                let (exact, _) = exact_by_order(&orders, &[CHECK_ALBEDO]);
                let geometry = Geometry {
                    mu,
                    mu0,
                    tau: tau_phot,
                    phase_value: phase.value(observer.phase_rad()),
                };
                let model = sums.table_reflectance(face, (k, m, j), geometry);
                let f = usize::from(face == Face::Unlit);
                sums.reduction_max[f] = worst(sums.reduction_max[f], model, exact[0]);
            }
        }
    }
    Ok(())
}

/// Traces τ node `k` (k ≥ 1) into `sums`: solves its `τ_dyn`, builds its fields and traces every
/// elevation, and at [`reduction_node`] checks the `μ_eff` reduction.
fn trace_node(
    spec: &TableSpec,
    k: usize,
    sums: &mut Accumulator,
    threads: NonZeroUsize,
) -> Result<TauNode, RunShadowingError> {
    let a = spec.axes;
    let target = a.tau(k);
    let (layer, built, transmission) =
        solve_layer(target, SALO_FRENCH_LAW, spec.table, spec.seed, k, threads)?;
    for (m, item) in (0..a.mu_nodes).zip(0_u64..) {
        let mu = a.mu(m);
        let b = math::asin(mu);
        let observers: Vec<Observer> = [Face::Lit, Face::Unlit]
            .into_iter()
            .flat_map(|face| {
                (0..a.phase_nodes)
                    .map(move |j| Observer::at_phase(face, b, b, a.phase_rad(face, mu, j)))
            })
            .collect();
        let streams = PhotonStreams {
            seed: spec.seed,
            run: Run::TauNode(k),
            item,
        };
        let tally = trace_fields(
            &built,
            b,
            &observers,
            spec.table.photons,
            Orders::Buckets(PHOTOMETRY_BUCKETS),
            streams,
            threads,
        )?;
        let at = NodeAt {
            k,
            m,
            mu,
            tau_phot: transmission.tau(),
            filling_factor: layer.central_filling_factor(),
        };
        sums.record(at, &observers, &tally);
    }
    if k == reduction_node(&a) {
        check_reduction(spec, k, &built, transmission.tau(), sums, threads)?;
    }
    Ok(TauNode {
        target,
        measured: transmission,
        layer: Some(layer),
        particles: built.first().map_or(0, Field::count),
    })
}

/// Runs the whole table under `spec` on `threads` threads, reporting each τ node as it finishes
/// to `progress`.
///
/// # Errors
///
/// [`RunShadowingError`] if a layer cannot be described, or a field built or traced.
pub fn run_table(
    spec: &TableSpec,
    threads: NonZeroUsize,
    progress: &mut dyn FnMut(&TauNode),
) -> Result<ShadowingTable, RunShadowingError> {
    let a = spec.axes;
    let mut sums = Accumulator::new(a);
    let mut nodes = Vec::with_capacity(a.tau_nodes);
    // τ = 0: no layer, the classical layer itself.
    sums.values[0][..a.mu_nodes * a.phase_nodes].fill(1.0);
    let empty = TauNode {
        target: a.tau(0),
        measured: Transmission {
            rays: 1,
            through: 1,
        },
        layer: None,
        particles: 0,
    };
    progress(&empty);
    nodes.push(empty);
    for k in 1..a.tau_nodes {
        let node = trace_node(spec, k, &mut sums, threads)?;
        progress(&node);
        nodes.push(node);
    }
    let acceptance = acceptance(RADII_M, spec.acceptance, spec.seed, threads)?;
    Ok(ShadowingTable {
        spec: *spec,
        nodes,
        checks: sums.checks(),
        values: sums.values,
        ratios: sums.ratios,
        acceptance,
        lost: sums.lost,
        photons: sums.photons,
    })
}

/// The module's line width, Prettier's `printWidth`.
const WIDTH: usize = 100;

/// Characters of base64 per line of the entries' string list: a multiple of 4, so each line
/// decodes alone, and short enough that a line stays within [`WIDTH`].
const BASE64_LINE: usize = 92;

/// `x` to `decimals` places, with trailing zeros and a trailing point removed, as Prettier prints
/// a number literal.
#[must_use]
fn number(x: f64, decimals: usize) -> String {
    let mut text = format!("{x:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".clone_into(&mut text);
    }
    text
}

/// Standard base64 (RFC 4648, §4, with padding).
#[must_use]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let sextet = |shift: u32| {
            char::from(ALPHABET[usize::try_from((n >> shift) & 63).expect("six bits")])
        };
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

/// Appends `text` and a line end to `out`.
fn line(out: &mut String, text: core::fmt::Arguments<'_>) {
    out.write_fmt(text)
        .expect("writing to a String cannot fail");
    out.push('\n');
}

/// A TypeScript value of the header, printed as Prettier prints it: objects always broken, one
/// property a line; arrays on one line where they fit, otherwise one element a line.
#[derive(Debug, Clone, PartialEq)]
enum Ts {
    Number(String),
    Text(String),
    Array(Vec<Ts>),
    Object(Vec<(&'static str, Ts)>),
}

impl Ts {
    #[must_use]
    fn num(x: f64, decimals: usize) -> Self {
        Self::Number(number(x, decimals))
    }

    #[must_use]
    fn int(x: impl Into<u64>) -> Self {
        Self::Number(x.into().to_string())
    }

    #[must_use]
    fn count(x: usize) -> Self {
        Self::Number(x.to_string())
    }

    #[must_use]
    fn text(s: &str) -> Self {
        Self::Text(s.to_owned())
    }

    /// The value on one line, where it has one (objects do not).
    #[must_use]
    fn inline(&self) -> Option<String> {
        match self {
            Self::Number(n) => Some(n.clone()),
            Self::Text(s) => Some(format!("\"{s}\"")),
            Self::Array(items) => {
                let parts: Option<Vec<String>> = items.iter().map(Self::inline).collect();
                parts.map(|p| format!("[{}]", p.join(", ")))
            }
            Self::Object(_) => None,
        }
    }

    /// Writes the value at `indent`, after `prefix` (a key and its colon, or nothing) on the same
    /// line, followed by `suffix`.
    fn write(&self, out: &mut String, indent: usize, prefix: &str, suffix: &str) {
        let pad = " ".repeat(indent);
        if let Some(inline) = self.inline()
            && pad.len() + prefix.len() + inline.len() + suffix.len() <= WIDTH
        {
            line(out, format_args!("{pad}{prefix}{inline}{suffix}"));
            return;
        }
        match self {
            Self::Object(fields) => {
                line(out, format_args!("{pad}{prefix}{{"));
                for (key, value) in fields {
                    value.write(out, indent + 2, &format!("{key}: "), ",");
                }
                line(out, format_args!("{pad}}}{suffix}"));
            }
            Self::Array(items) => {
                line(out, format_args!("{pad}{prefix}["));
                for item in items {
                    item.write(out, indent + 2, "", ",");
                }
                line(out, format_args!("{pad}]{suffix}"));
            }
            Self::Number(_) | Self::Text(_) => {
                // Too long for its line: Prettier would move it below its key, which the header
                // never needs.
                let inline = self.inline().expect("a scalar has a line");
                line(out, format_args!("{pad}{prefix}{inline}{suffix}"));
            }
        }
    }
}

/// The module's doc comment, line by line: the header in prose.
#[must_use]
fn doc_lines(table: &ShadowingTable) -> Vec<String> {
    let spec = table.spec;
    let t = spec.table;
    let acc = &table.acceptance;
    let sizes = acc.reference.sizes();
    let mut doc: Vec<String> = [
        "The rings' shadowing table (plan R11, R11.T9.b; Design note 15), written by",
        "`hyperion-fit ring-shadowing --out <this file>`: never edit it by hand; rerun the tool.",
        "",
        "@remarks",
        "**What it holds.** A ring's lit face is the classical single-scattering layer times the",
        "shadowing factor f_e = I_ss ÷ I_ss(D = 0) of a Monte Carlo through a dense layer, plus",
        "multiple scattering tallied by order: at albedo ϖ, I/F = ϖ I_classical(τ_phot) f_e + M, with",
        "M = ϖ² I₂ ÷ (1 − ϖ² q_e) + ϖ³ I₃ ÷ (1 − ϖ² q_o); the unlit face adds an M of its own to the",
        "classical transmitted single scattering. I₂ and I₃ are I/F at ϖ = 1 of the second and third",
        "orders: on the lit face the table's litOrder2 and litOrder3 times the geometry's own",
        "μ₀ ÷ (μ + μ₀), on the unlit face unlitOrder2 and unlitOrder3 times its own μ₀. q_e is the share",
        "of the even orders from the fourth in those from the second, q_o of the odd orders from the",
        "fifth in those from the third, per (τ, μ) and face, pooled over the phase nodes: with",
        "particles this backscattering the orders alternate, and each parity is near geometric.",
        "",
        "**Axes.** τ_phot = −ln(normal transmission), node k of n_τ at s_τ((1 + τ_max ÷",
        "s_τ)^(k ÷ (n_τ − 1)) − 1); μ_eff = 2μμ₀ ÷ (μ + μ₀), node m at μ_min + (1 − μ_min)(m ÷",
        "(n_μ − 1))²; the phase angle as the fraction v of its range at B = B′ = arcsin μ_eff,",
        "α = v(π − 2B) lit and α = π − v(π − 2B) unlit, node j at s_v((1 + 1 ÷ s_v)^(j ÷ (n_v − 1))",
        "− 1). The Monte Carlo traces B = B′: a geometry with B ≠ B′ is read at its μ_eff (a lean,",
        "whose error `checks` holds). The entries run v fastest, then μ, then τ, one block per",
        "quantity, then the ratios over (τ, μ), μ fastest. Each is an unsigned 16-bit code,",
        "little-endian in the base64 below, over its codesPerUnit.",
        "",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    doc.extend([
        format!(
            "**Header.** Table version {TABLE_VERSION}, seed {}; R⁻³ radii {}–{} m; phase c_n (π − α)^n,",
            spec.seed,
            number(sizes.min_radius_m(), 3),
            number(sizes.max_radius_m(), 3)
        ),
        format!(
            "n = {} (Salo and French 2010, eq. 11); a Gaussian layer with D₀ = {} at τ_dyn = {} and",
            number(SALO_FRENCH_PHASE_EXPONENT, 3),
            number(SALO_FRENCH_LAW.reference_filling_factor(), 3),
            number(SALO_FRENCH_LAW.reference_optical_depth(), 3)
        ),
        format!(
            "H_eff = {} m, D₀ ∝ τ_dyn, capped at {}; {} fields of {} m a τ node,",
            number(acc.reference.effective_thickness_m(), 2),
            number(SALO_FRENCH_LAW.cap(), 3),
            t.realisations,
            number(t.box_m(), 1),
        ),
        format!(
            "{} photons a field and elevation, τ_phot from {} normal rays a field;",
            t.photons, t.normal_rays
        ),
        format!(
            "{} of {} photons lost in the layer's wings or truncated.",
            table.lost, table.photons
        ),
        String::new(),
        "**τ nodes** (τ_phot, as measured ± its ray-sampling error, τ_dyn, D₀, H_eff):".to_owned(),
    ]);
    for node in &table.nodes {
        doc.push(match node.layer() {
            None => format!("- {}: no layer.", number(node.target, 4)),
            Some(layer) => format!(
                "- {}: {} ± {}, τ_dyn {}, D₀ {}, H_eff {} m;",
                number(node.target, 4),
                number(node.measured(), 4),
                number(node.measured_error(), 4),
                number(layer.dynamical_optical_depth(), 4),
                number(layer.central_filling_factor(), 4),
                number(layer.effective_thickness_m(), 2)
            ),
        });
    }
    doc.extend(acceptance_lines(table));
    doc.extend(check_lines(&table.checks));
    doc
}

/// The doc comment's acceptance figures, line by line.
#[must_use]
fn acceptance_lines(table: &ShadowingTable) -> Vec<String> {
    let spec = table.spec;
    let acc = &table.acceptance;
    let (cross, cross_layer) = acc.cross_check();
    let (photometry, photometry_layer) = acc.photometry();
    let ratio_lines = |name: &str, tilt: &Tilt| {
        [
            format!("- {name}:"),
            format!(
                "  f_e(26°) ÷ f_e(4.5°) = {} at α = 6° and {} at α = 0.5° (τ_phot {});",
                number(tilt.ratio(TiltPhase::Wide), 4),
                number(tilt.ratio(TiltPhase::Narrow), 4),
                number(tilt.tau_phot(), 4)
            ),
        ]
    };
    let [layer_name, layer_ratios] = ratio_lines(
        "the layer, against 1.30 ± 0.05 at 6° and about 1.15 at 0.5°",
        &acc.tilt,
    );
    let [uniform_name, uniform_ratios] =
        ratio_lines("a uniform layer at D = 0.05, which must fail", &acc.uniform);
    let [cross_name, cross_ratios] = ratio_lines(
        &format!(
            "the 0.1–5 m cross-check at H_eff {} m, recorded",
            number(cross_layer.effective_thickness_m(), 2)
        ),
        cross,
    );
    vec![
        String::new(),
        format!(
            "**Acceptance** (Salo and French 2010, §5.2; Design note 15), on {} fields of {} m and {}",
            spec.acceptance.realisations,
            number(spec.acceptance.box_m(), 1),
            spec.acceptance.photons
        ),
        "photons a field and elevation:".to_owned(),
        format!(
            "- D₀ = {} at τ_dyn = 1.5 with H_eff = {} m;",
            number(acc.reference.central_filling_factor(), 4),
            number(acc.reference.effective_thickness_m(), 2),
        ),
        format!(
            "  the first field's own τ_dyn {} and D₀ {};",
            number(acc.field_tau_dyn, 4),
            number(acc.field_filling_factor, 4)
        ),
        layer_name,
        layer_ratios,
        uniform_name,
        uniform_ratios,
        cross_name,
        cross_ratios,
        format!(
            "- I/F at B = B′ = 4.5°, α = 6°, τ_dyn 2.0 (D₀ {}): {} at ϖ 0.57 (814 nm, 0.44 ± 0.05),",
            number(photometry_layer.central_filling_factor(), 3),
            number(photometry[0], 4)
        ),
        format!(
            "  {} at ϖ 0.21 (336 nm, 0.16 ± 0.03); by the order model, {} and {};",
            number(photometry[1], 4),
            number(acc.photometry_model()[0], 4),
            number(acc.photometry_model()[1], 4)
        ),
        format!(
            "- Hapke's term's tilt ratio, {} at 6° and {} at 0.5°.",
            number(acc.hapke_ratio(TiltPhase::Wide), 4),
            number(acc.hapke_ratio(TiltPhase::Narrow), 4)
        ),
    ]
}

/// The doc comment's checks of the table's models, line by line.
#[must_use]
fn check_lines(checks: &Checks) -> Vec<String> {
    let (near_max, near_rms) = checks.hapke_near();
    let (all_max, all_rms) = checks.hapke_all();
    let (dense_max, dense_rms) = checks.hapke_dense();
    let [[order_lit, order_unlit], [order_lit_one, order_unlit_one]] = checks.order_model_max();
    let [reduction_lit, reduction_unlit] = checks.reduction_max();
    vec![
        String::new(),
        "**Checks** of the table's models:".to_owned(),
        format!(
            "- Hapke's fallback, |B_H − f_e| over the lit nodes: at most {} (RMS {}) at α ≤ 30°,",
            number(near_max, 4),
            number(near_rms, 4)
        ),
        format!(
            "  {} (RMS {}) at every α, and {} (RMS {}) at α ≤ 30° in the dense rings, τ_phot ≥ 1;",
            number(all_max, 4),
            number(all_rms, 4),
            number(dense_max, 4),
            number(dense_rms, 4)
        ),
        format!(
            "- the order model's pooled ratios against a node's own orders: at most {} lit and {}",
            number(order_lit, 4),
            number(order_unlit, 4)
        ),
        format!(
            "  unlit at ϖ 0.57, {} and {} at ϖ = 1, relative, in I/F;",
            number(order_lit_one, 4),
            number(order_unlit_one, 4)
        ),
        "- the μ_eff reduction against direct traces with μ₀ ÷ μ = 3 and ⅓ at its middle τ node,"
            .to_owned(),
        format!(
            "  at ϖ 0.57: at most {} lit and {} unlit, relative, in I/F.",
            number(reduction_lit, 4),
            number(reduction_unlit, 4)
        ),
    ]
}

/// The header's τ nodes as data.
#[must_use]
fn tau_nodes_data(table: &ShadowingTable) -> Ts {
    Ts::Array(
        table
            .nodes
            .iter()
            .map(|node| {
                let layer = node.layer();
                Ts::Object(vec![
                    ("tauPhot", Ts::num(node.target, 6)),
                    ("measured", Ts::num(node.measured(), 6)),
                    ("measuredError", Ts::num(node.measured_error(), 6)),
                    (
                        "tauDyn",
                        Ts::num(layer.map_or(0.0, Layer::dynamical_optical_depth), 6),
                    ),
                    (
                        "fillingFactor",
                        Ts::num(layer.map_or(0.0, Layer::central_filling_factor), 6),
                    ),
                    (
                        "thicknessM",
                        Ts::num(layer.map_or(0.0, Layer::effective_thickness_m), 4),
                    ),
                ])
            })
            .collect(),
    )
}

/// The header's acceptance figures as data: [`Acceptance`]'s alone, so that the slow test can find
/// them in the committed module ([`acceptance_text`]).
#[must_use]
fn acceptance_data(acc: &Acceptance) -> Ts {
    let (cross, _) = acc.cross_check();
    let (photometry, _) = acc.photometry();
    let model = acc.photometry_model();
    let (wide, narrow) = (TiltPhase::Wide, TiltPhase::Narrow);
    Ts::Object(vec![
        (
            "fillingFactor",
            Ts::num(acc.reference.central_filling_factor(), 4),
        ),
        (
            "thicknessM",
            Ts::num(acc.reference.effective_thickness_m(), 4),
        ),
        ("tiltRatioAt6Deg", Ts::num(acc.tilt.ratio(wide), 4)),
        ("tiltRatioAt0p5Deg", Ts::num(acc.tilt.ratio(narrow), 4)),
        (
            "uniformTiltRatioAt6Deg",
            Ts::num(acc.uniform.ratio(wide), 4),
        ),
        (
            "uniformTiltRatioAt0p5Deg",
            Ts::num(acc.uniform.ratio(narrow), 4),
        ),
        ("crossCheckTiltRatioAt6Deg", Ts::num(cross.ratio(wide), 4)),
        (
            "crossCheckTiltRatioAt0p5Deg",
            Ts::num(cross.ratio(narrow), 4),
        ),
        ("reflectanceAt814Nm", Ts::num(photometry[0], 4)),
        ("reflectanceAt336Nm", Ts::num(photometry[1], 4)),
        ("reflectanceAt814NmByOrderModel", Ts::num(model[0], 4)),
        ("reflectanceAt336NmByOrderModel", Ts::num(model[1], 4)),
        ("hapkeTiltRatioAt6Deg", Ts::num(acc.hapke_ratio(wide), 4)),
        (
            "hapkeTiltRatioAt0p5Deg",
            Ts::num(acc.hapke_ratio(narrow), 4),
        ),
    ])
}

/// The header's checks as data.
#[must_use]
fn checks_data(checks: &Checks) -> Ts {
    let (near_max, near_rms) = checks.hapke_near();
    let (all_max, all_rms) = checks.hapke_all();
    let (dense_max, dense_rms) = checks.hapke_dense();
    let [[order_lit, order_unlit], [order_lit_one, order_unlit_one]] = checks.order_model_max();
    let [reduction_lit, reduction_unlit] = checks.reduction_max();
    Ts::Object(vec![
        ("hapkeMaxErrorBelow30Deg", Ts::num(near_max, 4)),
        ("hapkeRmsErrorBelow30Deg", Ts::num(near_rms, 4)),
        ("hapkeMaxError", Ts::num(all_max, 4)),
        ("hapkeRmsError", Ts::num(all_rms, 4)),
        ("hapkeMaxErrorDenseBelow30Deg", Ts::num(dense_max, 4)),
        ("hapkeRmsErrorDenseBelow30Deg", Ts::num(dense_rms, 4)),
        ("orderModelMaxErrorLit", Ts::num(order_lit, 4)),
        ("orderModelMaxErrorUnlit", Ts::num(order_unlit, 4)),
        ("orderModelMaxErrorLitAtAlbedo1", Ts::num(order_lit_one, 4)),
        (
            "orderModelMaxErrorUnlitAtAlbedo1",
            Ts::num(order_unlit_one, 4),
        ),
        ("reductionMaxErrorLit", Ts::num(reduction_lit, 4)),
        ("reductionMaxErrorUnlit", Ts::num(reduction_unlit, 4)),
    ])
}

/// The header as data.
#[must_use]
fn header_data(table: &ShadowingTable) -> Ts {
    let spec = table.spec;
    let a = spec.axes;
    let t = spec.table;
    let sizes = table.acceptance.reference.sizes();
    Ts::Object(vec![
        ("tool", Ts::text("hyperion-fit ring-shadowing")),
        ("version", Ts::int(TABLE_VERSION)),
        ("seed", Ts::int(spec.seed)),
        (
            "radiiM",
            Ts::Array(vec![
                Ts::num(sizes.min_radius_m(), 3),
                Ts::num(sizes.max_radius_m(), 3),
            ]),
        ),
        ("phaseExponent", Ts::num(SALO_FRENCH_PHASE_EXPONENT, 3)),
        (
            "referenceTauDyn",
            Ts::num(SALO_FRENCH_LAW.reference_optical_depth(), 3),
        ),
        (
            "referenceFillingFactor",
            Ts::num(SALO_FRENCH_LAW.reference_filling_factor(), 3),
        ),
        ("fillingFactorCap", Ts::num(SALO_FRENCH_LAW.cap(), 3)),
        ("boxM", Ts::num(t.box_m(), 1)),
        ("realisations", Ts::int(t.realisations)),
        ("photons", Ts::int(t.photons)),
        ("normalRays", Ts::int(t.normal_rays)),
        (
            "tau",
            Ts::Object(vec![
                ("nodes", Ts::count(a.tau_nodes)),
                ("scale", Ts::num(a.tau_scale, 6)),
                ("max", Ts::num(a.tau_max, 6)),
            ]),
        ),
        (
            "mu",
            Ts::Object(vec![
                ("nodes", Ts::count(a.mu_nodes)),
                ("min", Ts::num(a.mu_min, 12)),
            ]),
        ),
        (
            "phase",
            Ts::Object(vec![
                ("nodes", Ts::count(a.phase_nodes)),
                ("scale", Ts::num(a.phase_scale, 6)),
            ]),
        ),
        (
            "quantities",
            Ts::Array(QUANTITIES.iter().map(|q| Ts::text(q)).collect()),
        ),
        (
            "ratios",
            Ts::Array(RATIOS.iter().map(|q| Ts::text(q)).collect()),
        ),
        (
            "codesPerUnit",
            Ts::Object(vec![
                ("shadowing", Ts::int(SHADOWING_CODES_PER_UNIT)),
                ("orders", Ts::int(ORDER_CODES_PER_UNIT)),
                ("ratios", Ts::int(RATIO_CODES_PER_UNIT)),
            ]),
        ),
        ("tauNodes", tau_nodes_data(table)),
        ("acceptance", acceptance_data(&table.acceptance)),
        ("checks", checks_data(&table.checks)),
    ])
}

/// The acceptance figures as the module's header prints them, its `acceptance` property whole:
/// the slow test finds the text of its rerun in the committed module.
#[must_use]
pub fn acceptance_text(acc: &Acceptance) -> String {
    let mut out = String::new();
    acceptance_data(acc).write(&mut out, 2, "acceptance: ", ",");
    out
}

/// The module's text: its doc comment holding the header, the header as data, and the entries.
///
/// # Errors
///
/// [`RunShadowingError::Encode`] for a value outside its encoding.
pub fn render_module(table: &ShadowingTable) -> Result<String, RunShadowingError> {
    let codes = table.codes()?;
    let bytes: Vec<u8> = codes.iter().flat_map(|c| c.to_le_bytes()).collect();
    let encoded = base64(&bytes);
    let mut out = String::new();
    out.push_str("/**\n");
    for doc in doc_lines(table) {
        if doc.is_empty() {
            out.push_str(" *\n");
        } else {
            line(&mut out, format_args!(" * {doc}"));
        }
    }
    out.push_str(" */\n\n");
    out.push_str(
        "/** The table's header as data: the tool, its inputs, its axes, its acceptance and its checks. */\n",
    );
    header_data(table).write(
        &mut out,
        0,
        "export const SHADOWING_HEADER = ",
        " as const;",
    );
    out.push('\n');
    out.push_str(
        "/** The entries' codes, base64 of little-endian unsigned 16-bit integers, a line apiece. */\n",
    );
    out.push_str("const ENTRIES_BASE64 = [\n");
    let chars: Vec<char> = encoded.chars().collect();
    for chunk in chars.chunks(BASE64_LINE) {
        let text: String = chunk.iter().collect();
        line(&mut out, format_args!("  \"{text}\","));
    }
    out.push_str("].join(\"\");\n\n");
    out.push_str(DECODER);
    Ok(out)
}

/// The module's decoder and entries, after the data.
const DECODER: &str = r"/** Decodes base64 of little-endian unsigned 16-bit integers. */
function decodeEntries(base64: string): Uint16Array {
  const bytes = atob(base64);
  const codes = new Uint16Array(bytes.length / 2);
  for (let i = 0; i < codes.length; i += 1) {
    codes[i] = bytes.charCodeAt(2 * i) + 256 * bytes.charCodeAt(2 * i + 1);
  }
  return codes;
}

/**
 * The entries' codes: each of the {@link SHADOWING_HEADER}'s `quantities` over (τ, μ, v), v
 * fastest, then each of its `ratios` over (τ, μ), μ fastest; a code over its quantity's
 * `codesPerUnit` is its value.
 */
export const SHADOWING_CODES: Uint16Array = decodeEntries(ENTRIES_BASE64);
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_as_prettier_prints_them() {
        assert_eq!(number(1.5, 4), "1.5");
        assert_eq!(number(2.0, 4), "2");
        assert_eq!(number(-0.000_01, 3), "0");
        assert_eq!(number(0.017_452_406_437, 12), "0.017452406437");
    }

    #[test]
    fn base64_is_rfc_4648s() {
        // RFC 4648, §10's test vectors.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_axes_run_from_their_ends_through_their_spacings() {
        let a = FULL.axes;
        assert!(a.tau(0).abs() < 1e-15);
        assert!((a.tau(a.tau_nodes - 1) - 5.0).abs() < 1e-12);
        assert!((a.mu(0) - math::sin(1_f64.to_radians())).abs() < 1e-15);
        assert!((a.mu(a.mu_nodes - 1) - 1.0).abs() < 1e-15);
        assert!(a.phase_fraction(0).abs() < 1e-15);
        assert!((a.phase_fraction(a.phase_nodes - 1) - 1.0).abs() < 1e-12);
        let mu = math::sin(26_f64.to_radians());
        assert!(a.phase_rad(Face::Lit, mu, 0).abs() < 1e-15);
        assert!(
            (a.phase_rad(Face::Lit, mu, a.phase_nodes - 1) - 128_f64.to_radians()).abs() < 1e-9
        );
        assert!(
            (a.phase_rad(Face::Unlit, mu, a.phase_nodes - 1) - 52_f64.to_radians()).abs() < 1e-9
        );
        // 5 × 2,400 + 4 × 120 codes, 24,960 bytes; the reduction's μ nodes are sines at 2 μ_eff.
        assert_eq!(5 * a.cells() + 4 * a.tau_nodes * a.mu_nodes, 12_480);
        assert!(reduction_mu_nodes(&a).iter().all(|&m| 2.0 * a.mu(m) <= 1.0));
    }

    #[test]
    fn hapkes_term_gives_the_tilt_ratio_of_salo_and_french() {
        // h at D_eff = 0.35 sin 26° is 0.047, at 0.35 sin 4.5° 0.0079: 1.30 at 6° and 1.17 at 0.5°.
        let at = |alpha: f64, b: f64| {
            hapke_shadow_hiding(alpha.to_radians(), 0.35, math::sin(b.to_radians()), 10.0)
        };
        assert!((at(6.0, 26.0) / at(6.0, 4.5) - 1.303).abs() < 0.002);
        assert!((at(0.5, 26.0) / at(0.5, 4.5) - 1.166).abs() < 0.002);
        assert!((at(0.0, 26.0) - 2.0).abs() < 1e-12);
        assert!((hapke_shadow_hiding(0.1, 0.0, 0.5, 10.0) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn the_order_model_sums_each_parity_as_a_geometric_series() {
        // I₂ = 0.1 with the even orders halving, I₃ = 0.05 with the odd ones quartering: at ϖ = 1
        // the sums 0.2 and 0.0667; at ϖ = 0.5, ¼ × 0.1 ÷ (1 − ¼ × ½) + ⅛ × 0.05 ÷ (1 − ¼ × ¼).
        let exact = 0.1 / 0.5 + 0.05 / 0.75;
        assert!((multiple_scattering(1.0, 0.1, 0.05, 0.5, 0.25) - exact).abs() < 1e-15);
        let half = 0.025 / 0.875 + 0.00625 / 0.9375;
        assert!((multiple_scattering(0.5, 0.1, 0.05, 0.5, 0.25) - half).abs() < 1e-15);
        assert_eq!(multiple_scattering(0.0, 0.1, 0.05, 0.5, 0.25), 0.0);
        // The exact sums of such a series give its own ratios back.
        let orders: Vec<f64> = (1..=40)
            .map(|k: i32| {
                if k == 1 {
                    1.0
                } else if k % 2 == 0 {
                    0.1 * math::powi(0.5, k / 2 - 1)
                } else {
                    0.05 * math::powi(0.25, (k - 3) / 2)
                }
            })
            .collect();
        // Forty orders leave the halving series short by about 2⁻¹⁹ of its sum.
        let (exact_at_one, (q_even, q_odd)) = exact_by_order(&orders, &[1.0]);
        assert!((q_even - 0.5).abs() < 1e-5, "{q_even}");
        assert!((q_odd - 0.25).abs() < 1e-5, "{q_odd}");
        assert!((exact_at_one[0] - (1.0 + exact)).abs() < 1e-5);
    }

    #[test]
    fn the_classical_layer_is_the_semi_infinite_one_when_thick() {
        // P μ₀ ÷ (4(μ + μ₀)) at τ → ∞: P ÷ 8 at μ = μ₀.
        assert!((classical_lit(4.0, 1e3, 0.3, 0.3) - 0.5).abs() < 1e-15);
        // And P τ ÷ 4μ at τ → 0, whatever μ₀.
        assert!((classical_lit(4.0, 1e-9, 0.3, 0.7) / (1e-9 / 0.3) - 1.0).abs() < 1e-8);
    }

    #[test]
    fn the_classical_unlit_face_meets_its_limit_at_equal_elevations() {
        let near = classical_unlit(4.0, 1.0, 0.3, 0.300_001);
        let limit = classical_unlit(4.0, 1.0, 0.3, 0.3);
        assert!((near / limit - 1.0).abs() < 1e-4, "{near} against {limit}");
        // Thin: P τ ÷ 4μ, the beam through scattered once without loss.
        assert!((classical_unlit(4.0, 1e-9, 0.3, 0.7) / (1e-9 / 0.3) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_stream_key_refuses_items_that_would_spill() {
        let key = stream(0, Purpose::Photons, Run::Tilt, (1 << 24) - 1, 0);
        assert_eq!(key.key >> 60, 2);
        let spilled = std::panic::catch_unwind(|| stream(0, Purpose::Field, Run::Tilt, 1 << 24, 0));
        assert!(spilled.is_err());
        let past = std::panic::catch_unwind(|| Run::TauNode(255).code());
        assert!(past.is_err());
    }

    /// The smoke table, traced once for the tests that read it.
    fn smoke_table() -> ShadowingTable {
        let threads = NonZeroUsize::new(4).expect("not zero");
        run_table(&SMOKE, threads, &mut |_| ()).expect("a smoke run")
    }

    #[test]
    fn a_smoke_table_renders_a_module_with_its_header_and_its_codes() {
        let threads = NonZeroUsize::new(4).expect("not zero");
        let mut seen = 0;
        let table = run_table(&SMOKE, threads, &mut |_| seen += 1).expect("a smoke run");
        assert_eq!(seen, SMOKE.axes.tau_nodes);
        let text = render_module(&table).expect("in range");
        assert!(text.starts_with("/**\n * The rings' shadowing table"));
        assert!(text.contains(
            "export const SHADOWING_HEADER = {\n  tool: \"hyperion-fit ring-shadowing\",\n"
        ));
        assert!(text.contains(&acceptance_text(table.acceptance())));
        assert!(text.contains("export const SHADOWING_CODES: Uint16Array"));
        let codes = table.codes().expect("in range");
        assert_eq!(codes.len(), 5 * 36 + 4 * 9);
        // τ = 0's shadowing is 1, 16,384 codes.
        assert!(codes[..12].iter().all(|&c| c == 16_384));
        // The same on one thread.
        let again = run_table(&SMOKE, NonZeroUsize::MIN, &mut |_| ()).expect("a smoke run");
        assert_eq!(render_module(&again).expect("in range"), text);
    }

    #[test]
    fn an_entry_outside_its_encoding_is_refused() {
        let mut table = smoke_table();
        table.values[1][5] = 2.5;
        assert!(matches!(
            table.codes(),
            Err(RunShadowingError::Encode {
                quantity: "litOrder2",
                tau: 0,
                mu: 1,
                phase: 1,
                ..
            })
        ));
    }

    #[test]
    fn ring_shadowing_bits_are_pinned() {
        // A field's sums and a trace's tallies to the last bit, so that a change to the field's
        // placement, the tracer, its draws or `births::Draws` (or `atmosphere::geometry`'s
        // `turned`) shows here before it silently parts the committed module from the code. A
        // deliberate change re-pins them (`tests/pins/rings.pins`) and the module is rerun.
        let mut pins = hyperion_testkit::pins!("rings", "ring_shadowing_bits_are_pinned");
        let layer = SALO_FRENCH_LAW
            .layer(PowerLawSizes::new(0.5, 2.0).expect("valid radii"), 1.5)
            .expect("valid layer");
        let field = Field::build(&layer, 40.0, stream(0, Purpose::Field, Run::Tilt, 0, 0))
            .expect("room for every sphere");
        pins.exact(
            "field",
            (
                field.dynamical_optical_depth(),
                field.central_filling_factor(),
                field.top_m(),
            ),
        );
        let b = 20_f64.to_radians();
        let observers = [
            Observer::at_phase(Face::Lit, b, b, 6_f64.to_radians()),
            Observer::at_phase(Face::Unlit, b, b, 140_f64.to_radians()),
        ];
        let key = stream(0, Purpose::Photons, Run::Tilt, 0, 0);
        let tally = trace(
            &field,
            b,
            &observers,
            400,
            Orders::Buckets(8),
            key,
            NonZeroUsize::MIN,
        )
        .expect("a pool");
        let mu = math::sin(b);
        let by_bucket = |index: usize| -> Vec<f64> {
            (0..8)
                .map(|bucket| tally.reflectance(mu, index, &observers[index], bucket))
                .collect()
        };
        pins.exact("lit", by_bucket(0));
        pins.exact("unlit", by_bucket(1));
    }
}
