//! Orphan streams: the tubes of globular clusters that have dissolved (plan 10, P10.T3.b and
//! Design note 10).
//!
//! - **Count**: Poisson with mean `max(0, k × expected globulars − living globulars with a tube)`,
//!   `k` the streams per globular of [`tables::streams`](crate::tables::streams) that the galaxy's
//!   `orphan_streams` hold reads: the brainstorm's 1.5, and from version 27 P15.T11.b's fit to the
//!   known streams (`ORPHAN_STREAMS_VERSION`); the expected globulars are plan 02's untruncated
//!   count, so most globular streams are orphans.
//! - **Marks**, on `stream.orphan.marks` keyed by the orphan's place among the orphans: the mass,
//!   log-normal about 10⁴ M☉ with the known thin streams' 0.5 dex of scatter, cut to 10³–10⁵ M☉
//!   by inverse transform ([`MASS_LAW`]); the dissolution time as a uniform share of the
//!   stripping time, which leaves the gap; the age and [Fe/H] by the accreted globulars' laws of
//!   plan 09 (P09.T12.b: ages uniform on 10.5–13 Gyr, [Fe/H] `N(−1.55, 0.35)`), since an orphan
//!   is a destroyed cluster of the same system.
//! - **Orbit**, on `stream.orphan.orbit`: a radius from the metal-poor globulars' law
//!   `(1 + r² ÷ r_c²)^(−7/4)` (P09.T12.a, the core [`GLOBULAR_CORE`]) out to 10⁶ ly
//!   ([`APOCENTRE_LIMIT_LY`]), an isotropic direction, and a velocity from the globular-born
//!   debris component's law there (plan 08), redrawn on that one stream until the pericentre lies
//!   outside the bar's corotation and the apocentre inside that limit. The radius is the first
//!   drawn outside corotation, since no pericentre inside it clears corotation, and is kept while
//!   only the direction and the velocity are redrawn. The turning points are plan 09's (P09.T13),
//!   so the test is the living globulars'. Attempt `a` reads its own block of [`ORBIT_WORDS`]
//!   words, its radius word first whether or not it keeps that radius. An orphan whose
//!   [`ORBIT_ATTEMPTS`] attempts all fail is dropped: about 1%, kept just outside corotation,
//!   which no velocity drawn there clears (2 of 259 in the Milky Way fixture).
//!
//! Keeping the radius keeps the orphans on the radial law outside corotation, half of which lies
//! inside 65,000 ly. Redrawn with the velocity, as P10.T3.b first built it, a radius stayed only
//! where a radially biased velocity happened to clear corotation, mostly far out, and a quarter of
//! the orphans' tube mass lay inside 65,000 ly, against half now (plan 10's records, "P10.T3.b's
//! orphans keep their radius").
//!
//! The law is the `r^−3.5` that the Harris 2010 catalogue measures over 3–40 kpc (plan 09's
//! records), extrapolated beyond: it puts 0.18 of its mass beyond 40 kpc, where the catalogue has
//! 7 of its 105 clusters of [Fe/H] < −1 (0.07), so the orphans may still sit somewhat far out (a
//! finding: plan 10's records, "P10.T3.d.a's globular part").
//!
//! The stripping time is Design note 5's with the tidal radius at the orphan's mass, and stops at
//! the cluster's age.

use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::features::interior::counts::CANONICAL_SLOPE;
use crate::galaxy::features::kinds::globular::{
    ACCRETED_AGES, GLOBULAR_CORE, METALLICITY_LAWS, RadialMotion,
};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::kinematics::{ESCAPE_CUT_ATTEMPTS, VELOCITY_WORDS_PER_ATTEMPT, draw_on_from};
use crate::galaxy::params::HaloComponentKind;
use crate::galaxy::quad::gl16;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::tables::streams::{ORPHAN_STREAMS_PER_GLOBULAR, ORPHAN_STREAMS_PER_GLOBULAR_27};
use crate::units::{Dex, LightYears, SolarMasses, Years};

use super::spec::{ClusterStars, MassLoss, SphericalOrbit, StreamSpec, stripping_time};
use super::{StreamNumber, StreamOrigin};

/// The orphans' mass law: the median (M☉), the scatter (dex) and the range (M☉) of a log-normal
/// cut by inverse transform (Design note 10, whose median and range these are), a parameter of the
/// generator version. The scatter is the known thin streams': the 70 streams at most 300 pc wide
/// and 10⁵ M☉ in Bonaca and Price-Whelan's census (2024, arXiv:2405.19410, Table A.1) have a
/// median of 7 × 10³ M☉ and a scatter of 0.53 dex (GD-1 7 × 10³, Jhelum 10⁴, Phoenix 3 × 10⁴, Pal 5
/// 5 × 10⁴ M☉), a sample biased to nearby, bright streams.
pub const MASS_LAW: MassLaw = MassLaw {
    median_solar_masses: 1e4,
    scatter_dex: 0.5,
    range_solar_masses: (1e3, 1e5),
};

/// A log-normal mass law cut by inverse transform to a range: [`MASS_LAW`]'s shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassLaw {
    /// The median, M☉.
    pub median_solar_masses: f64,
    /// The scatter of `log₁₀ M`, dex.
    pub scatter_dex: f64,
    /// The range, M☉.
    pub range_solar_masses: (f64, f64),
}

impl MassLaw {
    /// The law's mean mass, M☉, in closed form.
    ///
    /// With `z = (log₁₀ M − log₁₀ M̃) ÷ σ` a standard normal cut to `[a, b]` and `t = σ ln 10`,
    /// `E[M] = M̃ e^(t² ÷ 2) (Φ(b − t) − Φ(a − t)) ÷ (Φ(b) − Φ(a))`, the cut normal's
    /// moment-generating function. [`MASS_LAW`]'s is 1.63 × 10⁴ M☉, the mass plan 10's Design
    /// note 1 gives every globular tube (P10.T3.d.a).
    #[must_use]
    pub fn mean_solar_masses(&self) -> f64 {
        let (lo, hi) = self.range_solar_masses;
        let centre = math::log10(self.median_solar_masses);
        let sigma = self.scatter_dex;
        let cdf = |x: f64| 0.5 * math::erfc(-x * core::f64::consts::FRAC_1_SQRT_2);
        let (a, b) = (
            (math::log10(lo) - centre) / sigma,
            (math::log10(hi) - centre) / sigma,
        );
        let t = sigma * core::f64::consts::LN_10;
        self.median_solar_masses * math::exp(0.5 * t * t) * (cdf(b - t) - cdf(a - t))
            / (cdf(b) - cdf(a))
    }
}

/// The largest apocentre an orphan's progenitor may have, ly (P10.T3.b).
pub const APOCENTRE_LIMIT_LY: f64 = 1e6;

/// The attempts at an orphan's orbit before it is dropped.
pub const ORBIT_ATTEMPTS: u32 = 256;

/// The words of one attempt at an orphan's orbit: the radius, two for the direction, then plan
/// 08's velocity draw with every attempt of its escape cut.
// `From` is not callable in a const; the u32-to-u64 widening is lossless.
pub const ORBIT_WORDS: u64 = 3 + VELOCITY_WORDS_PER_ATTEMPT * ESCAPE_CUT_ATTEMPTS as u64;

/// Knots of the radial law's cumulative function, in `ln(1 + r ÷ r_c)`.
const RADIAL_KNOTS: u32 = 512;

/// The generator version from which a galaxy's orphans, and the globular tubes its halo's discrete
/// share counts, take P15.T11.b's fitted streams per globular, [`ORPHAN_STREAMS_PER_GLOBULAR_27`]
/// (the hold of [`OrphanStreamsRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`OrphanStreamsRules::Brainstorm`], the brainstorm's 1.5, so every generated value keeps its
/// bits; the tests reach the new arm through a galaxy built under `GalaxyRules::every_27_hold()`.
/// The bump to 27 deletes this hold, [`OrphanStreamsRules`] with its `Brainstorm` arm, the
/// galaxy's `orphan_streams` field and its tripwire
/// (`the_orphan_streams_are_held_only_until_the_27_bump`).
pub(crate) const ORPHAN_STREAMS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// The streams per globular a galaxy's orphans and its halo's discrete share read (the hold of
/// [`ORPHAN_STREAMS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OrphanStreamsRules {
    /// Version 26's and before: the brainstorm's 1.5, [`ORPHAN_STREAMS_PER_GLOBULAR`].
    Brainstorm,
    /// Version 27's: P15.T11.b's fit to the known streams, [`ORPHAN_STREAMS_PER_GLOBULAR_27`].
    Fitted,
}

impl OrphanStreamsRules {
    /// The streams per globular k, living clusters' tubes and orphans together.
    #[must_use]
    pub(crate) const fn streams_per_globular(self) -> f64 {
        match self {
            Self::Brainstorm => ORPHAN_STREAMS_PER_GLOBULAR,
            Self::Fitted => ORPHAN_STREAMS_PER_GLOBULAR_27,
        }
    }
}

/// The expected number of orphans in a galaxy of `globulars` expected globulars with `living`
/// living ones given tubes, at `streams_per_globular` streams per globular (Design note 10).
#[must_use]
pub(crate) fn expected_count(streams_per_globular: f64, globulars: f64, living: u32) -> f64 {
    (streams_per_globular * globulars - f64::from(living)).max(0.0)
}

/// The orphans' draw in one galaxy before any orphan is drawn: how many attempts at an orphan it
/// makes, and what each reads (module documentation). Orphan `k` is a pure function of the galaxy
/// and `k` ([`orphan`](Self::orphan)), its every draw keyed by `k`, so a caller's pool may draw
/// them in any order and gather them by `k` (P10.T5.b).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrphanDraw {
    /// The seed of the galaxy drawn in.
    seed: Seed,
    /// The orphans drawn: none if the galaxy has no globular-born debris.
    count: u64,
    /// The component whose velocity law the progenitors follow.
    debris: Option<ComponentId>,
    radial: RadialLaw,
    /// The bar's corotation radius, ly, inside which no pericentre is kept.
    corotation: f64,
    last_merger: Years,
}

impl OrphanDraw {
    /// The draw in `galaxy` given `living` living globulars with tubes.
    #[must_use]
    pub(crate) fn new(galaxy: &Galaxy, living: u32) -> Self {
        let seed = galaxy.seed();
        let params = galaxy.params();
        let mean = expected_count(
            galaxy.rules().orphan_streams.streams_per_globular(),
            f64::from(params.accretion().globular_count()),
            living,
        );
        let count =
            Stream::open(seed, tags::STREAM_ORPHAN_COUNT, ObjectKey::galaxy()).poisson(mean);
        let debris = debris_component(galaxy);
        Self {
            seed,
            count: if debris.is_some() { count } else { 0 },
            debris,
            radial: RadialLaw::new(),
            corotation: galaxy.potential().bar_corotation().value(),
            last_merger: params.accretion().last_major_merger(),
        }
    }

    /// The attempts at an orphan the draw makes: orphan `k` for each `k` below it.
    #[must_use]
    pub(crate) const fn count(&self) -> u64 {
        self.count
    }

    /// Orphan `k`'s stream, numbered 0 until the list numbers it, or `None` if none of its
    /// attempts gave an orbit that clears corotation inside [`APOCENTRE_LIMIT_LY`]: a pure
    /// function of `galaxy` and `k` (module documentation).
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not that of the galaxy the draw was made in, if `k` is not below
    /// [`count`](Self::count), or if `galaxy` holds no kinematic tables
    /// ([`Galaxy::with_full_potential`]).
    #[must_use]
    pub(crate) fn orphan(&self, galaxy: &Galaxy, k: u64) -> Option<StreamSpec> {
        assert!(k < self.count, "orphan {k} of {}", self.count);
        self.orphan_at(galaxy, k)
    }

    /// Orphan `k`'s stream for any `k`, the count aside.
    ///
    /// It is what a draw of more orphans, under a larger multiplier, would give as its orphan `k`,
    /// since every draw of an orphan is keyed by `k` alone and the multiplier sets only how many
    /// there are (plan 15's fit of it, P15.T11.b). `None` for a galaxy with no globular-born
    /// debris, which draws none, or for an orphan none of whose attempts finds an orbit.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not that of the galaxy the draw was made in, or if `galaxy` holds no
    /// kinematic tables ([`Galaxy::with_full_potential`]).
    #[must_use]
    pub(crate) fn orphan_at(&self, galaxy: &Galaxy, k: u64) -> Option<StreamSpec> {
        assert_eq!(
            galaxy.seed(),
            self.seed,
            "an orphan is drawn in the galaxy its draw was made in"
        );
        let debris = self.debris?;
        let (seed, tables) = (self.seed, galaxy.potential());
        let (radial, corotation, last_merger) = (&self.radial, self.corotation, self.last_merger);
        let key = ObjectKey::galaxy_item(k);
        let mut marks = Stream::open(seed, tags::STREAM_ORPHAN_MARKS, key);
        let mass = mass_quantile(marks.uniform());
        let dissolved = marks.uniform();
        let age = Years::new(marks.uniform_in(ACCRETED_AGES.0, ACCRETED_AGES.1));
        let (fe_h_mean, fe_h_sigma) = METALLICITY_LAWS.1;
        let fe_h = Dex::new(marks.normal(fe_h_mean, fe_h_sigma));
        let mut stream = Stream::open(seed, tags::STREAM_ORPHAN_ORBIT, key);
        // The first radius drawn outside corotation, inside which no pericentre clears it;
        // later attempts redraw only the direction and the velocity (module documentation).
        let mut kept = None;
        let (position, velocity, orbit) = (0..ORBIT_ATTEMPTS).find_map(|attempt| {
            let first = u64::from(attempt) * ORBIT_WORDS;
            stream.seek(first);
            // Every attempt reads its radius word, kept or not, so that each block's words
            // keep their places.
            let drawn = radial.quantile(stream.uniform());
            if kept.is_none() && drawn > corotation {
                kept = Some(drawn);
            }
            let r = kept?;
            let cos_theta = stream.uniform_in(-1.0, 1.0);
            let azimuth = stream.uniform_in(0.0, 2.0 * core::f64::consts::PI);
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let (sin_phi, cos_phi) = math::sin_cos(azimuth);
            let at = GalacticPosition::from_light_years([
                r * sin_theta * cos_phi,
                r * sin_theta * sin_phi,
                r * cos_theta,
            ])
            .expect("a radius under 10⁶ ly fits the i32 light-year cells");
            let velocity = draw_on_from(galaxy, debris, &at, &mut stream, first + 3).velocity();
            let motion = RadialMotion::new(galaxy, &at, velocity);
            let peri = motion.pericentre();
            // Only an attempt whose pericentre clears corotation pays for its apocentre's
            // search, and only an accepted one for the orbit's quadrature. Most attempts fall
            // inside corotation: searching both turning points of every attempt cost the
            // Milky Way fixture's debris 158 ms against 110 (P10.T4's list bench, pod, 16
            // vCPU).
            let apo = (peri.value() > corotation)
                .then(|| motion.apocentre())
                .filter(|apo| apo.value() < APOCENTRE_LIMIT_LY)?;
            let orbit = SphericalOrbit::between(tables, peri, apo)?;
            Some((
                GalacticPosition::ORIGIN.displacement_to(&at),
                velocity,
                orbit,
            ))
        })?;
        let tidal = LightYears::from(
            tables.tidal_radius(mass, &PointLy::new(orbit.pericentre().value(), 0.0, 0.0)),
        );
        let window = stripping_time(&orbit, tidal, [last_merger, age]);
        Some(StreamSpec {
            number: StreamNumber(0),
            origin: StreamOrigin::Orphan,
            position,
            velocity,
            orbit,
            stripping_time: window,
            mass,
            progenitor_mass: SolarMasses::new(0.0),
            tidal_radius: tidal,
            mass_loss: MassLoss::Dissolved {
                ago: Years::new(dissolved * window.value()),
            },
            stars: Some(ClusterStars {
                fe_h,
                age,
                kept: 0.0,
                depleted_slope: CANONICAL_SLOPE,
            }),
        })
    }
}

/// The component whose velocity law the orphans' progenitors follow: the globular-born debris.
#[must_use]
fn debris_component(galaxy: &Galaxy) -> Option<ComponentId> {
    let fields = galaxy.fields();
    fields.component_ids().find(|&id| {
        fields.component(id).halo_component() == Some(HaloComponentKind::GlobularDebris)
    })
}

/// [`MASS_LAW`]'s quantile at `u`: `log₁₀ M = log₁₀ M̃ + σ Φ⁻¹(Φ(a) + u (Φ(b) − Φ(a)))`, with `a`
/// and `b` the range's ends in units of σ.
#[must_use]
fn mass_quantile(u: f64) -> SolarMasses {
    let MassLaw {
        median_solar_masses: median,
        scatter_dex: sigma,
        range_solar_masses: (lo, hi),
    } = MASS_LAW;
    let centre = math::log10(median);
    let cdf = |x: f64| 0.5 * math::erfc(-x * core::f64::consts::FRAC_1_SQRT_2);
    let (a, b) = (
        cdf((math::log10(lo) - centre) / sigma),
        cdf((math::log10(hi) - centre) / sigma),
    );
    let z = math::normal_quantile(a + u * (b - a));
    SolarMasses::new(math::exp10(centre + sigma * z).clamp(lo, hi))
}

/// The metal-poor globulars' radial law out to [`APOCENTRE_LIMIT_LY`], tabulated as its cumulative
/// function in `t = ln(1 + r ÷ r_c)`.
#[derive(Debug, Clone, PartialEq)]
struct RadialLaw {
    /// The cumulative share at knot `k`, `t = k × step`.
    cdf: Vec<f64>,
    step: f64,
}

impl RadialLaw {
    #[must_use]
    fn new() -> Self {
        let top = math::ln_1p(APOCENTRE_LIMIT_LY / GLOBULAR_CORE);
        let step = top / f64::from(RADIAL_KNOTS - 1);
        // 4π r² (1 + r² ÷ r_c²)^(−7/4) dr ÷ dt, with dr ÷ dt = r + r_c.
        let integrand = |t: f64| {
            let r = GLOBULAR_CORE * math::exp_m1(t);
            let x = r / GLOBULAR_CORE;
            r * r * math::powf(1.0 + x * x, -1.75) * (r + GLOBULAR_CORE)
        };
        let mut cdf = Vec::with_capacity(usize::try_from(RADIAL_KNOTS).expect("512 fits usize"));
        let mut running = 0.0;
        for k in 0..RADIAL_KNOTS {
            if k > 0 {
                let t = step * f64::from(k);
                running += gl16(integrand, t - step, t);
            }
            cdf.push(running);
        }
        for c in &mut cdf {
            *c /= running;
        }
        Self { cdf, step }
    }

    /// The radius (ly) below which a share `u` of the law lies, linear in `t` between knots.
    #[must_use]
    fn quantile(&self, u: f64) -> f64 {
        let knots = self.cdf.len();
        let k = self.cdf.partition_point(|&c| c <= u).clamp(1, knots - 1) - 1;
        let (below, above) = (self.cdf[k], self.cdf[k + 1]);
        let frac = if above > below {
            ((u - below) / (above - below)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        #[expect(clippy::cast_precision_loss, reason = "a knot index below 512")]
        let t = self.step * (k as f64 + frac);
        GLOBULAR_CORE * math::exp_m1(t)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::rules::GalaxyRules;
    use crate::rng::Seed;

    #[test]
    fn orphan_masses_follow_their_cut_log_normal() {
        let MassLaw {
            median_solar_masses: median,
            scatter_dex: sigma,
            range_solar_masses: (lo, hi),
        } = MASS_LAW;
        let mut stream = Stream::open(
            Seed::new(0x0a10_003b),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let mut logs: Vec<f64> = (0..20_000)
            .map(|_| math::log10(mass_quantile(stream.uniform()).value()))
            .collect();
        for &x in &logs {
            assert!((math::log10(lo)..=math::log10(hi)).contains(&x), "{x}");
        }
        let cdf = |x: f64| 0.5 * math::erfc(-x * core::f64::consts::FRAC_1_SQRT_2);
        let centre = math::log10(median);
        let (a, b) = (
            cdf((math::log10(lo) - centre) / sigma),
            cdf((math::log10(hi) - centre) / sigma),
        );
        let ks = ks_one_sample(&mut logs, |x| (cdf((x - centre) / sigma) - a) / (b - a));
        assert_p_value("orphan masses", ks.p_value, ALPHA);
        // The median is the law's, 10⁴ M☉: the cut is symmetric in log M.
        assert!((mass_quantile(0.5).value() / median - 1.0).abs() < 1e-9);
        assert!((mass_quantile(0.0).value() - lo).abs() < 1e-6 * lo);
        assert!((mass_quantile(1.0).value() - hi).abs() < 1e-6 * hi);
    }

    #[test]
    fn orphan_radii_follow_the_metal_poor_law() {
        let law = RadialLaw::new();
        let mut stream = Stream::open(
            Seed::new(0x0a10_03b1),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let mut radii: Vec<f64> = (0..20_000)
            .map(|_| law.quantile(stream.uniform()))
            .collect();
        // The law's cumulative function by direct quadrature in r, panel by panel.
        let shell = |r: f64| {
            let x = r / GLOBULAR_CORE;
            r * r * math::powf(1.0 + x * x, -1.75)
        };
        let edges: Vec<f64> = (0..=64)
            .map(|k| APOCENTRE_LIMIT_LY * math::powf(f64::from(k) / 64.0, 3.0))
            .collect();
        let total = crate::galaxy::quad::gl_panels(shell, &edges);
        let cumulative = |r: f64| {
            let mut cut: Vec<f64> = edges.iter().copied().filter(|&e| e < r).collect();
            cut.push(r);
            crate::galaxy::quad::gl_panels(shell, &cut) / total
        };
        let ks = ks_one_sample(&mut radii, cumulative);
        assert_p_value("orphan radii", ks.p_value, ALPHA);
        assert!(
            radii
                .iter()
                .all(|&r| (0.0..=APOCENTRE_LIMIT_LY * (1.0 + 1e-12)).contains(&r))
        );
    }

    #[test]
    fn the_mass_law_s_mean_is_its_quantile_s_average() {
        // The quantile averaged over u by a midpoint rule of 2²⁰ points: the mean to the rule's
        // error, which the law's steep ends keep near 10⁻⁷.
        let n = 1_u32 << 20;
        let sum = (0..n)
            .map(|k| mass_quantile((f64::from(k) + 0.5) / f64::from(n)).value())
            .fold(0.0, |s, m| s + m);
        let average = sum / f64::from(n);
        let mean = MASS_LAW.mean_solar_masses();
        assert!((average / mean - 1.0).abs() < 1e-6, "{average} {mean}");
        assert!((mean - 16_284.3).abs() < 0.1, "{mean}");
    }

    #[test]
    fn the_orphan_count_makes_up_the_streams_per_globular() {
        assert!((expected_count(1.5, 160.0, 40) - 200.0).abs() < 1e-12);
        assert!(expected_count(1.5, 10.0, 40).abs() < 1e-12);
        assert!((expected_count(2.0, 160.0, 40) - 280.0).abs() < 1e-12);
    }

    /// Each arm reads its own item of the table: the held arm the brainstorm's 1.5, the 27 arm
    /// P15.T11.b's fit, bit for bit.
    #[test]
    fn each_arm_reads_its_streams_per_globular() {
        assert_same_bits(OrphanStreamsRules::Brainstorm.streams_per_globular(), 1.5);
        assert_same_bits(
            OrphanStreamsRules::Fitted.streams_per_globular(),
            ORPHAN_STREAMS_PER_GLOBULAR_27,
        );
    }

    /// A galaxy's orphans are drawn at its own rules' streams per globular: the count is the
    /// Poisson draw of the galaxy's count stream at that mean, the version's and 27's.
    #[test]
    fn the_orphans_count_is_drawn_at_the_galaxy_s_streams_per_globular() {
        let seed = Seed::new(0x0f11_b000_0000_0001);
        let mut counts = Vec::new();
        for rules in [
            GalaxyRules::of_version(GENERATOR_VERSION),
            GalaxyRules::every_27_hold(),
        ] {
            let galaxy =
                Galaxy::from_params_with_rules(seed, GalaxyParams::milky_way_like(), rules)
                    .expect("the Milky Way fixture builds");
            let draw = OrphanDraw::new(&galaxy, 40);
            let globulars = f64::from(galaxy.params().accretion().globular_count());
            let k = rules.orphan_streams.streams_per_globular();
            let mean = expected_count(k, globulars, 40);
            let want =
                Stream::open(seed, tags::STREAM_ORPHAN_COUNT, ObjectKey::galaxy()).poisson(mean);
            assert_eq!(draw.count(), want, "k {k}");
            counts.push(draw.count());
        }
        assert!(counts[1] > counts[0], "{counts:?}");
    }

    /// The tripwire of the hold: below 27 every galaxy reads the brainstorm's 1.5.
    #[test]
    fn the_orphan_streams_are_held_only_until_the_27_bump() {
        assert!(
            GENERATOR_VERSION < ORPHAN_STREAMS_VERSION,
            "the 26 → 27 bump puts P15.T11.b's fitted streams per globular on the record: the \
             orphans' count, the numbers of the dwarfs' streams after them, the halo's discrete \
             share and the dwarfs' scale move. Delete `ORPHAN_STREAMS_VERSION`, \
             `OrphanStreamsRules` with its `Brainstorm` arm, `GalaxyRules::orphan_streams` and \
             this test in the bump's own commit, with the `streams` table's held item and its \
             test, measure `share::GLOBULAR_TUBES_INSIDE` again \
             (`the_globular_tubes_inside_share_is_the_sweep_s_mean`), and re-bless the goldens \
             plan 15's \"T11.b as built\" lists"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).orphan_streams,
            OrphanStreamsRules::Brainstorm
        );
    }
}
