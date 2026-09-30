//! Globular clusters as they are today, and their history derived backwards (plan 09, P09.T12
//! and P09.T13).
//!
//! # The system (P09.T12.a)
//!
//! A galaxy has plan 02's `AccretionHistory::globular_count` globulars (the dark halo's mass ÷
//! 6.5 × 10⁹ M☉ with 0.2 dex of scatter; Burkert and Forbes 2020), the expected number of the
//! untruncated system. Their density is a cored `(1 + r² ÷ r_c²)^(−7/4)`, an `r^−3.5` beyond a
//! core of [`GLOBULAR_CORE`] = 6,800 ly (ruling 126.5: a fit to Harris 2010's catalogue,
//! arXiv:1012.3224, by research r-feat09b finds `r^−3.5` over 3–40 kpc, steepening beyond),
//! normalised at its cut. It has two parts:
//!
//! - **Metal-rich, in situ** (30%): flattened to 0.5 in z and cut at an ellipsoidal radius of
//!   26,000 ly; they count against the bulge where they lie inside its figure, `(x ÷ a)² + (y ÷
//!   b)² + (z ÷ c)² < 1` in the bulge's scale lengths (ours), and against the thick disc elsewhere.
//! - **Metal-poor** (70%): spherical, cut at 65,000 ly. Of these, 10 of the 70 are in situ and
//!   follow the halo's in-situ component; the other 60 are accreted and follow a progenitor's
//!   halo component, the dominant merger or a lesser one, chosen in proportion to its mass
//!   (Massari et al. 2019). Recent progenitors' globulars are plan 10's dwarf cores and streams.
//!
//! The metal-rich part is normalised to its truncated law, so all of its share is placed; the
//! metal-poor part places [`POOR_PLACED_SHARE`] = 0.82 of its share inside 65,000 ly (Harris 2010:
//! 87 of 106 metal-poor clusters inside 20 kpc), so the catalogue places 0.874 of the count. Inside
//! their cuts the metal-poor median is about 5.3 kpc and the metal-rich about 3.05. The density
//! never rises with `|x|`, `|y|` or `|z|` (the cuts are indicators that do not either), so the
//! catalogue's uniform proposal is bounded at a cell's nearest corner.
//!
//! # Marks (P09.T12.b)
//!
//! - **Mass**: today's, from `(M + Δ)⁻² e^(−(M + Δ) ÷ M_c)` with Δ = 2.0 × 10⁵ and `M_c` = 1.07
//!   × 10⁶ M☉ over 10³–10⁷ M☉ ([`GlobularMassFunction`], an inverse transform of its cumulative
//!   function tabulated at 512 knots in `log M`), the same at every radius.
//! - **Half-mass radius**: 2.6 pc × (r ÷ kpc)^0.41 with 0.21 dex of scatter.
//! - **Core**: `log₁₀(r_h ÷ r_c)` normal about 0.72 with 0.37 dex, held to 0.2–2.3: the
//!   Baumgardt–Hilker catalogue's own distribution (ours; the plan says only "drawn").
//! - **Metallicity**: `N(−0.55, 0.25)` in situ and metal-rich, `N(−1.55, 0.35)` otherwise.
//! - **Age**: 12.8 Gyr in situ, uniform on 10.5–13 Gyr accreted.
//! - **Bulk velocity**: the origin component's law (plan 08), on the feature's own stream.
//!
//! # History (P09.T13)
//!
//! The orbit's peri- and apocentre come from the cluster's energy and angular momentum in the
//! mid-plane potential table, taken as spherical, by 60 bisections each (Design note 11). The
//! dissolution time is Baumgardt and Makino's (2003, MNRAS 340, 227, eqs. 10 and 12): `t_dis = 1.91
//! Myr × (N ÷ ln 0.02N)^0.75 × (R_A ÷ kpc)(V_c ÷ 220 km/s)^−1 (1 − ε)`, `N = M₀ ÷ 0.547 M☉`, `R_A` the
//! apocentre, `V_c` the circular speed there and ε the eccentricity. The initial mass solves `M =
//! 0.70 M₀ (1 − t ÷ t_dis(M₀))` by 40 bisections in `log M₀`, and the mass lost to the tails is
//! `0.70 M₀ ÷ t_dis`. The birth half-mass radius is today's (ruling 126.2): Gieles, Heggie and
//! Zhao's (2011, MNRAS 413, 2509, eq. 28) expansion is an upper envelope in which clusters forget
//! their start, and cannot be inverted once a cluster evaporates, so the birth escape speed is
//! today's times `√(M₀ ÷ M)`. BM03's models span about 4.5 × 10³–7.2 × 10⁴ M☉, 2.8–15 kpc and ε ≤
//! 0.8; the globulars above and beyond are an extrapolation of its `N` scaling, as Baumgardt et
//! al. 2019 also make.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
use crate::galaxy::fields::ComponentId;
use crate::galaxy::params::{HaloComponentKind, ProgenitorKind};
use crate::galaxy::quad::gl_panels;
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::math;
use crate::rng::Mark;
use crate::units::{Dex, LightYears, SolarMasses, Years};

/// The core of the globulars' `r^−3.5` law, ly (P09.T12.a; ruling 126.5, which puts the
/// metal-poor median inside the cut at Harris 2010's 5.2 kpc).
pub const GLOBULAR_CORE: f64 = 6_800.0;

/// The share of the metal-poor globulars placed inside their cut (ruling 126.5; Harris 2010).
pub const POOR_PLACED_SHARE: f64 = 0.82;

/// The metal-poor globulars' cut, ly.
pub const GLOBULAR_CUT: f64 = 65_000.0;

/// The metal-rich in-situ globulars' ellipsoidal cut, ly.
pub const METAL_RICH_CUT: f64 = 26_000.0;

/// The metal-rich globulars' flattening in z.
pub const METAL_RICH_FLATTENING: f64 = 0.5;

/// The metal-rich, in-situ share of the globulars.
pub const METAL_RICH_SHARE: f64 = 0.3;

/// The in-situ share of the metal-poor globulars: 10 of 70.
pub const METAL_POOR_IN_SITU_SHARE: f64 = 1.0 / 7.0;

/// The evolved Schechter function's Δ, M☉ (the form of Jordán et al. 2007, ApJS 171, 101; the
/// brainstorm's fit to the Baumgardt–Hilker catalogue).
pub const MASS_FUNCTION_DELTA: f64 = 2.0e5;

/// The evolved Schechter function's `M_c`, M☉.
pub const MASS_FUNCTION_CUTOFF: f64 = 1.07e6;

/// The globulars' mass range, M☉.
pub const MASS_RANGE: (f64, f64) = (1e3, 1e7);

/// The knots of the mass function's tabulated cumulative function.
const MASS_KNOTS: usize = 512;

/// The half-mass radius at 1 kpc, pc, and its slope with radius and scatter, dex.
pub const HALF_MASS_RADIUS_LAW: (f64, f64, f64) = (2.6, 0.41, 0.21);

/// `log₁₀(r_h ÷ r_c)`: mean, scatter and range (module documentation).
pub const CORE_LAW: (f64, f64, (f64, f64)) = (0.72, 0.37, (0.2, 2.3));

/// Metallicity laws: metal-rich (mean, σ), metal-poor (mean, σ).
pub const METALLICITY_LAWS: ((f64, f64), (f64, f64)) = ((-0.55, 0.25), (-1.55, 0.35));

/// The in-situ globulars' age, years.
pub const IN_SITU_AGE: f64 = 12.8e9;

/// The accreted globulars' age range, years.
pub const ACCRETED_AGES: (f64, f64) = (10.5e9, 13.0e9);

/// Where a globular came from (P09.T12.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GlobularOrigin {
    /// Metal-rich and in situ, in the bulge's figure.
    InSituBulge,
    /// Metal-rich and in situ, outside the bulge's figure: the thick disc's.
    InSituThickDisc,
    /// Metal-poor and in situ: the halo's in-situ component.
    InSituHalo,
    /// Accreted with a progenitor, whose halo component it follows.
    Accreted(ProgenitorKind),
}

impl GlobularOrigin {
    /// The population whose budget the cluster's members come from (Design note 4).
    #[must_use]
    pub const fn population(self) -> Population {
        match self {
            Self::InSituBulge => Population::Bulge,
            Self::InSituThickDisc => Population::ThickDisc,
            Self::InSituHalo | Self::Accreted(_) => Population::Halo,
        }
    }

    /// Whether it formed in situ.
    #[must_use]
    pub const fn is_in_situ(self) -> bool {
        !matches!(self, Self::Accreted(_))
    }

    /// Whether it is metal-rich.
    #[must_use]
    pub const fn is_metal_rich(self) -> bool {
        matches!(self, Self::InSituBulge | Self::InSituThickDisc)
    }
}

/// A globular's marks (P09.T12.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobularMarks {
    pub(crate) origin: GlobularOrigin,
    pub(crate) mass: SolarMasses,
    pub(crate) half_mass_radius: LightYears,
    pub(crate) log_half_mass_over_core: f64,
    pub(crate) fe_h: Dex,
    pub(crate) age: Years,
}

impl GlobularMarks {
    /// Where it came from.
    #[must_use]
    pub const fn origin(&self) -> GlobularOrigin {
        self.origin
    }

    /// Its present mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Its present half-mass radius.
    #[must_use]
    pub const fn half_mass_radius(&self) -> LightYears {
        self.half_mass_radius
    }

    /// Its core radius, `r_h ÷ 10^u`.
    #[must_use]
    pub fn core_radius(&self) -> LightYears {
        LightYears::new(self.half_mass_radius.value() / math::exp10(self.log_half_mass_over_core))
    }

    /// Its [Fe/H].
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// Its age at the epoch.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }
}

/// The globulars' present mass function (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct GlobularMassFunction {
    /// `log₁₀ M` at the knots.
    log_mass: Box<[f64]>,
    /// The cumulative share at the knots, 0 to 1.
    cdf: Box<[f64]>,
}

impl GlobularMassFunction {
    /// The tabulated function (module documentation).
    #[must_use]
    pub fn new() -> Self {
        let (lo, hi) = (math::log10(MASS_RANGE.0), math::log10(MASS_RANGE.1));
        let pdf_log = |x: f64| {
            let m = math::exp10(x);
            m * density(m)
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "the knot count is small and exact"
        )]
        let step = (hi - lo) / (MASS_KNOTS - 1) as f64;
        let mut log_mass = Vec::with_capacity(MASS_KNOTS);
        let mut cdf = Vec::with_capacity(MASS_KNOTS);
        let mut running = 0.0;
        for k in 0..MASS_KNOTS {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a knot index is small and exact"
            )]
            let x = lo + step * k as f64;
            if k > 0 {
                running += crate::galaxy::quad::gl16(pdf_log, x - step, x);
            }
            log_mass.push(x);
            cdf.push(running);
        }
        for c in &mut cdf {
            *c /= running;
        }
        Self {
            log_mass: log_mass.into_boxed_slice(),
            cdf: cdf.into_boxed_slice(),
        }
    }

    /// The mass below which a share `u` lies, linear in `log M` between knots.
    #[must_use]
    pub fn quantile(&self, u: f64) -> SolarMasses {
        let share = u.clamp(0.0, 1.0);
        let knot = self
            .cdf
            .partition_point(|&c| c <= share)
            .clamp(1, MASS_KNOTS - 1)
            - 1;
        let (below, above) = (self.cdf[knot], self.cdf[knot + 1]);
        let frac = if above > below {
            (share - below) / (above - below)
        } else {
            0.0
        };
        SolarMasses::new(math::exp10(
            self.log_mass[knot] + frac * (self.log_mass[knot + 1] - self.log_mass[knot]),
        ))
    }

    /// The exact cumulative share below `m`, by quadrature of the closed form, for the tests.
    #[must_use]
    pub fn cumulative(&self, m: SolarMasses) -> f64 {
        let (lo, hi) = (math::log10(MASS_RANGE.0), math::log10(MASS_RANGE.1));
        let x = math::log10(m.value()).clamp(lo, hi);
        let pdf_log = |x: f64| {
            let m = math::exp10(x);
            m * density(m)
        };
        let panels = |a: f64, b: f64| {
            let edges: Vec<f64> = (0..=32)
                .map(|k| a + (b - a) * f64::from(k) / 32.0)
                .collect();
            gl_panels(pdf_log, &edges)
        };
        panels(lo, x) / panels(lo, hi)
    }
}

impl Default for GlobularMassFunction {
    fn default() -> Self {
        Self::new()
    }
}

/// `(M + Δ)⁻² e^(−(M + Δ) ÷ M_c)`, unnormalised.
fn density(m: f64) -> f64 {
    let s = m + MASS_FUNCTION_DELTA;
    math::exp(-s / MASS_FUNCTION_CUTOFF) / (s * s)
}

/// The shape of the globulars' law at (ellipsoidal) radius `r`, ly.
fn shape(r: f64) -> f64 {
    let x = r / GLOBULAR_CORE;
    math::powf(1.0 + x * x, -1.75)
}

/// `∫ 4π r² shape(r) dr` from 0 to `cut` (ly; infinite for `f64::INFINITY`), by 64 nodes in
/// `ln(1 + r ÷ r_c)`, and beyond 10⁸ ly by the `r^−3.5` tail's closed form.
#[must_use]
pub fn shape_integral(cut: f64) -> f64 {
    let far = 1e8_f64.min(cut);
    let top = math::ln_1p(far / GLOBULAR_CORE);
    let integrand = |t: f64| {
        let r = GLOBULAR_CORE * math::exp_m1(t);
        4.0 * core::f64::consts::PI * r * r * shape(r) * (r + GLOBULAR_CORE)
    };
    let edges: Vec<f64> = (0..=8).map(|k| top * f64::from(k) / 8.0).collect();
    let near = gl_panels(integrand, &edges);
    if cut > far {
        // ∫ 4π r² (r ÷ r_c)^−3.5 dr from `far` to ∞ = 8π r_c^3.5 far^−0.5.
        near + 8.0 * core::f64::consts::PI * math::powf(GLOBULAR_CORE, 3.5) / far.sqrt()
    } else {
        near
    }
}

/// The globulars' laws of one galaxy: count and normalisations (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct GlobularSystem {
    count: f64,
    rich_norm: f64,
    poor_norm: f64,
    mass_function: GlobularMassFunction,
    bulge_axes: [f64; 3],
    progenitors: Vec<(ProgenitorKind, f64)>,
}

impl GlobularSystem {
    /// The system of `galaxy`'s parameters.
    #[must_use]
    pub fn new(params: &crate::galaxy::params::GalaxyParams) -> Self {
        let count = f64::from(params.accretion().globular_count());
        let bulge = params.bulge();
        let progenitors = params
            .accretion()
            .progenitors()
            .iter()
            .filter(|p| {
                matches!(
                    p.kind(),
                    ProgenitorKind::DominantMerger | ProgenitorKind::Lesser(_)
                )
            })
            .map(|p| (p.kind(), p.mass().value()))
            .collect();
        Self {
            count,
            rich_norm: count * METAL_RICH_SHARE
                / (shape_integral(METAL_RICH_CUT) * METAL_RICH_FLATTENING),
            poor_norm: count * (1.0 - METAL_RICH_SHARE) * POOR_PLACED_SHARE
                / shape_integral(GLOBULAR_CUT),
            mass_function: GlobularMassFunction::new(),
            bulge_axes: [
                bulge.scale_x().value(),
                bulge.scale_y().value(),
                bulge.scale_z().value(),
            ],
            progenitors,
        }
    }

    /// The expected number of the untruncated law.
    #[must_use]
    pub const fn count(&self) -> f64 {
        self.count
    }

    /// The mass function.
    #[must_use]
    pub const fn mass_function(&self) -> &GlobularMassFunction {
        &self.mass_function
    }

    /// The metal-rich part's density at `p`, globulars per cubic light-year.
    #[must_use]
    pub fn rich_density(&self, p: &PointLy) -> f64 {
        let z = p.z / METAL_RICH_FLATTENING;
        let m = (p.x * p.x + p.y * p.y + z * z).sqrt();
        if m >= METAL_RICH_CUT {
            0.0
        } else {
            self.rich_norm * shape(m)
        }
    }

    /// The metal-poor part's density at `p`, globulars per cubic light-year.
    #[must_use]
    pub fn poor_density(&self, p: &PointLy) -> f64 {
        let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        if r >= GLOBULAR_CUT {
            0.0
        } else {
            self.poor_norm * shape(r)
        }
    }

    /// Every globular's density at `p`, per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.rich_density(p) + self.poor_density(p)
    }

    /// The share of the count the catalogue places: all of the metal-rich part and
    /// [`POOR_PLACED_SHARE`] of the metal-poor, 0.874.
    #[must_use]
    pub fn placed_share() -> f64 {
        METAL_RICH_SHARE + (1.0 - METAL_RICH_SHARE) * POOR_PLACED_SHARE
    }

    /// The radius inside which half of the metal-poor globulars placed lie, ly: the median of the
    /// spherical law inside its cut, by 60 bisections.
    #[must_use]
    pub fn poor_median_radius() -> f64 {
        let half = 0.5 * shape_integral(GLOBULAR_CUT);
        let (mut lo, mut hi) = (0.0, GLOBULAR_CUT);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if shape_integral(mid) < half {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    /// Whether `p` lies inside the bulge's figure (module documentation).
    #[must_use]
    pub fn in_bulge(&self, p: &PointLy) -> bool {
        let [a, b, c] = self.bulge_axes;
        (p.x / a) * (p.x / a) + (p.y / b) * (p.y / b) + (p.z / c) * (p.z / c) < 1.0
    }

    /// The origin two marks pick at `p`: the part by the parts' odds there, then in situ or a
    /// progenitor, the progenitors in proportion to their mass (module documentation), each
    /// through plan 01's [`Mark::pick_weighted`](crate::rng::Mark::pick_weighted).
    #[must_use]
    pub fn origin(&self, p: &PointLy, part: Mark, within: Mark) -> GlobularOrigin {
        let (rich, poor) = (self.rich_density(p), self.poor_density(p));
        if part.pick_weighted(&[rich, poor], rich + poor) == Some(0) {
            return if self.in_bulge(p) {
                GlobularOrigin::InSituBulge
            } else {
                GlobularOrigin::InSituThickDisc
            };
        }
        let total: f64 = self.progenitors.iter().map(|(_, m)| m).sum();
        if total <= 0.0 {
            return GlobularOrigin::InSituHalo;
        }
        // The in-situ share, then each progenitor's share of the rest by its mass.
        let accreted = 1.0 - METAL_POOR_IN_SITU_SHARE;
        let weights: Vec<f64> = std::iter::once(METAL_POOR_IN_SITU_SHARE)
            .chain(self.progenitors.iter().map(|(_, m)| accreted * m / total))
            .collect();
        match within.pick_weighted(&weights, 1.0 + 1e-12) {
            Some(0) | None => GlobularOrigin::InSituHalo,
            Some(i) => GlobularOrigin::Accreted(self.progenitors[i - 1].0),
        }
    }

    /// The expected mass of the globulars the catalogue places, by origin population, M☉: each
    /// part's share of the untruncated number inside its cut times the mass function's mean
    /// (bulge, thick disc, halo). The metal-rich share is split by the share of its law inside
    /// the bulge's figure, by a quadrature over its flattened shells (Design note 4; P09.T2.b).
    #[must_use]
    pub fn population_masses(&self) -> (f64, f64, f64) {
        let mean = self.mean_mass();
        let rich = self.count * METAL_RICH_SHARE * mean;
        let poor = self.count * (1.0 - METAL_RICH_SHARE) * POOR_PLACED_SHARE * mean;
        let in_bulge = self.rich_share_in_bulge();
        (rich * in_bulge, rich * (1.0 - in_bulge), poor)
    }

    /// The mass function's mean, M☉.
    #[must_use]
    pub fn mean_mass(&self) -> f64 {
        let (lo, hi) = (math::log10(MASS_RANGE.0), math::log10(MASS_RANGE.1));
        let edges: Vec<f64> = (0..=32)
            .map(|k| lo + (hi - lo) * f64::from(k) / 32.0)
            .collect();
        let n = gl_panels(|x| math::exp10(x) * density(math::exp10(x)), &edges);
        let m = gl_panels(
            |x| {
                let m = math::exp10(x);
                m * m * density(m)
            },
            &edges,
        );
        m / n
    }

    /// The share of the metal-rich law inside the bulge's figure, by a 32 × 32 quadrature over
    /// the flattened sphere in (ellipsoidal radius, cos θ), the bulge taken axisymmetric at the
    /// geometric mean of its two long axes.
    fn rich_share_in_bulge(&self) -> f64 {
        let [long, middle, short] = self.bulge_axes;
        let in_plane = (long * middle).sqrt();
        let (mut inside, mut total) = (0.0, 0.0);
        let nodes = 32;
        for i in 0..nodes {
            let radius = METAL_RICH_CUT * (f64::from(i) + 0.5) / f64::from(nodes);
            let weight = radius * radius * shape(radius);
            for j in 0..nodes {
                let mu = (f64::from(j) + 0.5) / f64::from(nodes);
                let r_cyl = radius * (1.0 - mu * mu).sqrt();
                let height = radius * mu * METAL_RICH_FLATTENING;
                total += weight;
                if (r_cyl / in_plane) * (r_cyl / in_plane) + (height / short) * (height / short)
                    < 1.0
                {
                    inside += weight;
                }
            }
        }
        inside / total
    }
}

/// The component of `galaxy` whose velocity law an `origin`'s globular follows: the bulge, the
/// thick disc, or the halo component of the in-situ halo or of its progenitor.
#[must_use]
pub fn origin_component(galaxy: &Galaxy, origin: GlobularOrigin) -> Option<ComponentId> {
    let fields = galaxy.fields();
    fields.component_ids().find(|&id| {
        let c = fields.component(id);
        match origin {
            GlobularOrigin::InSituBulge => c.population() == Population::Bulge,
            GlobularOrigin::InSituThickDisc => c.population() == Population::ThickDisc,
            GlobularOrigin::InSituHalo => c.halo_component() == Some(HaloComponentKind::InSitu),
            GlobularOrigin::Accreted(ProgenitorKind::DominantMerger) => {
                c.halo_component() == Some(HaloComponentKind::DominantMerger)
            }
            GlobularOrigin::Accreted(ProgenitorKind::Lesser(n)) => {
                c.halo_component() == Some(HaloComponentKind::Lesser(n))
            }
            GlobularOrigin::Accreted(ProgenitorKind::Recent(_)) => false,
        }
    })
}

/// A globular's history (P09.T13; module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobularHistory {
    /// Its orbit's pericentre, ly.
    pub pericentre: LightYears,
    /// Its orbit's apocentre, ly.
    pub apocentre: LightYears,
    /// `(r_a − r_p) ÷ (r_a + r_p)`.
    pub eccentricity: f64,
    /// Its dissolution time from birth.
    pub dissolution_time: Years,
    /// Its initial mass.
    pub initial_mass: SolarMasses,
    /// Its half-mass radius at birth.
    pub birth_half_mass_radius: LightYears,
    /// The mass it is losing, M☉ per year.
    pub mass_loss_rate: f64,
}

/// Baumgardt and Makino's β, years (W₀ = 5).
const BM03_BETA: f64 = 1.91e6;
/// Their exponent x.
const BM03_EXPONENT: f64 = 0.75;
/// Their mean stellar mass, M☉.
const BM03_MEAN_MASS: f64 = 0.547;
/// The bound share of the inverted mass law, `M = 0.70 M₀ (1 − t ÷ t_dis)`.
pub const BOUND_SHARE: f64 = 0.70;

/// Baumgardt and Makino's dissolution time of a cluster of initial mass `m0` on an orbit of
/// apocentre `apo` (ly) and eccentricity `e`, in a galaxy whose circular speed at the apocentre is
/// `v_c` (km/s).
#[must_use]
pub fn dissolution_time(m0: f64, apo: f64, v_c: f64, e: f64) -> Years {
    let n = (m0 / BM03_MEAN_MASS).max(100.0);
    let r = apo / LIGHT_YEARS_PER_KILOPARSEC;
    Years::new(
        BM03_BETA
            * math::powf(n / math::ln(0.02 * n), BM03_EXPONENT)
            * r
            * (220.0 / v_c)
            * (1.0 - e).max(0.0),
    )
}

/// The peri- and apocentre, ly, of an orbit through `position` at `velocity` (galactic, m/s) in
/// `galaxy`'s mid-plane potential taken as spherical, or `(r, r)` for a circular orbit where
/// `velocity` is `None` (Design note 11).
#[must_use]
pub fn orbit(
    galaxy: &Galaxy,
    position: &GalacticPosition,
    velocity: Option<GalacticVelocity>,
) -> (LightYears, LightYears) {
    let tables = galaxy.potential();
    let p = PointLy::from(position);
    let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt().max(1.0);
    let phi = |x: f64| tables.potential_in_plane(LightYears::new(x.max(1.0)));
    let Some(v) = velocity else {
        return (LightYears::new(r), LightYears::new(r));
    };
    let [vx, vy, vz] = v.metres_per_second().map(|c| c * 1e-3);
    let energy = 0.5 * (vx * vx + vy * vy + vz * vz) + phi(r);
    // L = |r × v| in ly km/s.
    let (lx, ly, lz) = (
        p.y * vz - p.z * vy,
        p.z * vx - p.x * vz,
        p.x * vy - p.y * vx,
    );
    let l2 = lx * lx + ly * ly + lz * lz;
    let g = |x: f64| 2.0 * (energy - phi(x)) - l2 / (x * x);
    (
        LightYears::new(turning_point(&g, 1.0, r)),
        LightYears::new(turning_point(&g, 1e7, r)),
    )
}

/// The history of a globular of present `mass`, `half_mass_radius` and `age` at `position` moving
/// at `velocity` (galactic, m/s) in `galaxy`, or on a circular orbit where `velocity` is `None`
/// (module documentation).
#[must_use]
pub fn history(
    galaxy: &Galaxy,
    position: &GalacticPosition,
    velocity: Option<GalacticVelocity>,
    mass: SolarMasses,
    half_mass_radius: LightYears,
    age: Years,
) -> GlobularHistory {
    let (peri, apo) = orbit(galaxy, position, velocity);
    history_on_orbit(galaxy, peri, apo, mass, half_mass_radius, age)
}

/// The history of a globular on an orbit of `peri`- and `apo`centre (module documentation).
#[must_use]
pub fn history_on_orbit(
    galaxy: &Galaxy,
    peri: LightYears,
    apo: LightYears,
    mass: SolarMasses,
    half_mass_radius: LightYears,
    age: Years,
) -> GlobularHistory {
    let (peri, apo) = (peri.value(), apo.value());
    let e = if apo + peri > 0.0 {
        (apo - peri) / (apo + peri)
    } else {
        0.0
    };
    let v_c = galaxy
        .potential()
        .v_circ(LightYears::new(apo.max(1.0)))
        .value()
        .max(1.0);
    let t = age.value();
    let m = mass.value();
    // Solve M = 0.70 M₀ (1 − t ÷ t_dis(M₀)) in log M₀.
    let bound = |m0: f64| BOUND_SHARE * m0 * (1.0 - t / dissolution_time(m0, apo, v_c, e).value());
    let (mut lo, mut hi) = (math::log10(m / BOUND_SHARE), 9.0_f64);
    let initial = if bound(math::exp10(hi)) < m {
        math::exp10(hi)
    } else {
        for _ in 0..40 {
            let mid = f64::midpoint(lo, hi);
            if bound(math::exp10(mid)) < m {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        math::exp10(hi)
    };
    let t_dis = dissolution_time(initial, apo, v_c, e);
    let birth_radius = half_mass_radius.value();
    GlobularHistory {
        pericentre: LightYears::new(peri),
        apocentre: LightYears::new(apo),
        eccentricity: e,
        dissolution_time: t_dis,
        initial_mass: SolarMasses::new(initial),
        birth_half_mass_radius: LightYears::new(birth_radius),
        mass_loss_rate: BOUND_SHARE * initial / t_dis.value(),
    }
}

/// The root of `g` between `from` and `r` (where `g(r) ≥ 0`) by 60 bisections in `ln x`: the
/// turning point on `from`'s side, or `r` itself if `g` does not change sign.
fn turning_point(g: &impl Fn(f64) -> f64, from: f64, r: f64) -> f64 {
    if g(from) >= 0.0 {
        return from;
    }
    let (mut a, mut b) = (math::ln(from), math::ln(r));
    for _ in 0..60 {
        let mid = f64::midpoint(a, b);
        if g(math::exp(mid)) >= 0.0 {
            b = mid;
        } else {
            a = mid;
        }
    }
    math::exp(b)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::rng::{ObjectKey, Seed, Stream, tags};

    /// P09.T12.a (ruling 126.5): 0.874 of the count placed; the metal-poor median inside the cut
    /// 5.0–5.6 kpc.
    #[test]
    fn the_law_is_normalised_at_its_cut() {
        assert!((GlobularSystem::placed_share() - 0.874).abs() < 1e-12);
        let median = GlobularSystem::poor_median_radius() / LIGHT_YEARS_PER_KILOPARSEC;
        assert!((5.0..=5.6).contains(&median), "{median} kpc");
        let system = GlobularSystem::new(&crate::galaxy::params::GalaxyParams::milky_way_like());
        let (bulge, thick, halo) = system.population_masses();
        let placed = system.count() * GlobularSystem::placed_share() * system.mean_mass();
        assert!(((bulge + thick + halo) / placed - 1.0).abs() < 1e-12);
    }

    /// Baumgardt and Makino 2003's Table 1: 71,236 M☉ on a circular orbit at 8.5 kpc lasts 23,769
    /// Myr (±2%), and 11,675 Myr at ε = 0.5 (±3%), in a 220 km/s galaxy (ruling 126.1).
    #[test]
    fn the_dissolution_time_matches_baumgardt_and_makino_s_table() {
        let apo = 8.5 * LIGHT_YEARS_PER_KILOPARSEC;
        let circular = dissolution_time(71_236.0, apo, 220.0, 0.0).value() / 1e6;
        assert!((circular / 23_769.0 - 1.0).abs() < 0.02, "{circular} Myr");
        let eccentric = dissolution_time(71_236.0, apo, 220.0, 0.5).value() / 1e6;
        assert!((eccentric / 11_675.0 - 1.0).abs() < 0.03, "{eccentric} Myr");
    }

    #[test]
    fn the_mass_function_s_inverse_transform_is_its_closed_form() {
        let f = GlobularMassFunction::new();
        let mut stream = Stream::open(
            Seed::new(0x0912),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let mut masses: Vec<f64> = (0..20_000)
            .map(|_| f.quantile(stream.uniform()).value())
            .collect();
        let ks = ks_one_sample(&mut masses, |m| f.cumulative(SolarMasses::new(m)));
        assert_p_value("globular masses", ks.p_value, ALPHA);
        for m in &masses {
            assert!((MASS_RANGE.0..=MASS_RANGE.1).contains(m), "{m}");
        }
    }

    #[test]
    fn the_dissolution_time_follows_baumgardt_and_makino() {
        // 10⁵ M☉ on a circular orbit at 8.5 kpc in a 220 km/s galaxy: 1.91 Myr × (N ÷ ln 0.02N)^0.75
        // × 8.5.
        let n = 1e5 / 0.547;
        let want = 1.91e6 * math::powf(n / math::ln(0.02 * n), 0.75) * 8.5;
        let got = dissolution_time(1e5, 8.5 * LIGHT_YEARS_PER_KILOPARSEC, 220.0, 0.0).value();
        assert!((got / want - 1.0).abs() < 1e-12);
        assert!(dissolution_time(1e5, 8.5 * LIGHT_YEARS_PER_KILOPARSEC, 220.0, 0.5).value() < got);
    }
}
