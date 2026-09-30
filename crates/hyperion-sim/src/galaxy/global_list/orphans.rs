//! Orphan streams: the tubes of globular clusters that have dissolved (plan 10, P10.T3.b and
//! Design note 10).
//!
//! - **Count**: Poisson with mean `max(0, k × expected globulars − living globulars with a tube)`,
//!   `k` the streams per globular of [`tables::streams`](crate::tables::streams), 1.5, and the
//!   expected globulars plan 02's untruncated count; so most globular streams are orphans.
//! - **Marks**, on `stream.orphan.marks` keyed by the orphan's place among the orphans: the mass,
//!   log-normal about 10⁴ M☉ with the known thin streams' 0.5 dex of scatter, cut to 10³–10⁵ M☉
//!   by inverse transform ([`MASS_LAW`]); the dissolution time as a uniform share of the
//!   stripping time, which leaves the gap; the age and [Fe/H] by the accreted globulars' laws of
//!   plan 09 (P09.T12.b: ages uniform on 10.5–13 Gyr, [Fe/H] `N(−1.55, 0.35)`), since an orphan
//!   is a destroyed cluster of the same system.
//! - **Orbit**, on `stream.orphan.orbit`, redrawn on that one stream until the pericentre lies
//!   outside the bar's corotation and the apocentre inside 10⁶ ly ([`APOCENTRE_LIMIT_LY`]): a radius
//!   from the metal-poor globulars' law `(1 + r² ÷ r_c²)^(−7/4)` (P09.T12.a, the core
//!   [`GLOBULAR_CORE`]) out to that limit, an isotropic direction, and a velocity from the
//!   globular-born debris component's law there (plan 08). The turning points are plan 09's
//!   (P09.T13), so the test is the living globulars'. Attempt `a` reads its own block of
//!   [`ORBIT_WORDS`] words. An orphan whose [`ORBIT_ATTEMPTS`] attempts all fail is dropped; at
//!   the Milky Way's parameters none is.
//!
//! The stripping time is Design note 5's with the tidal radius at the orphan's mass, and stops at
//! the cluster's age.

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::features::kinds::globular::{
    ACCRETED_AGES, GLOBULAR_CORE, METALLICITY_LAWS, orbit as turning_points,
};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::kinematics::{ESCAPE_CUT_ATTEMPTS, VELOCITY_WORDS_PER_ATTEMPT, draw_on_from};
use crate::galaxy::params::HaloComponentKind;
use crate::galaxy::quad::gl16;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::tables::streams::ORPHAN_STREAMS_PER_GLOBULAR;
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

/// The expected number of orphans in a galaxy of `globulars` expected globulars with `living`
/// living ones given tubes (Design note 10).
#[must_use]
pub(crate) fn expected_count(globulars: f64, living: u32) -> f64 {
    (ORPHAN_STREAMS_PER_GLOBULAR * globulars - f64::from(living)).max(0.0)
}

/// The orphan streams of `galaxy`, given `living` living globulars with tubes, at most `room`
/// of them, in their order, numbered 0 until the list numbers them (module documentation).
///
/// # Panics
///
/// If `galaxy` holds no kinematic tables ([`Galaxy::with_full_potential`]).
#[must_use]
pub(crate) fn orphans(galaxy: &Galaxy, living: u32, room: usize) -> Vec<StreamSpec> {
    let seed = galaxy.seed();
    let params = galaxy.params();
    let tables = galaxy.potential();
    let mean = expected_count(f64::from(params.accretion().globular_count()), living);
    let count = Stream::open(seed, tags::STREAM_ORPHAN_COUNT, ObjectKey::galaxy()).poisson(mean);
    let Some(debris) = debris_component(galaxy) else {
        return Vec::new();
    };
    let radial = RadialLaw::new();
    let corotation = tables.bar_corotation().value();
    let last_merger = params.accretion().last_major_merger();
    (0..count)
        .filter_map(|k| {
            let key = ObjectKey::galaxy_item(k);
            let mut marks = Stream::open(seed, tags::STREAM_ORPHAN_MARKS, key);
            let mass = mass_quantile(marks.uniform());
            let dissolved = marks.uniform();
            let age = Years::new(marks.uniform_in(ACCRETED_AGES.0, ACCRETED_AGES.1));
            let (fe_h_mean, fe_h_sigma) = METALLICITY_LAWS.1;
            let fe_h = Dex::new(marks.normal(fe_h_mean, fe_h_sigma));
            let mut stream = Stream::open(seed, tags::STREAM_ORPHAN_ORBIT, key);
            let (position, velocity, orbit) = (0..ORBIT_ATTEMPTS).find_map(|attempt| {
                let first = u64::from(attempt) * ORBIT_WORDS;
                stream.seek(first);
                let r = radial.quantile(stream.uniform());
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
                let (peri, apo) = turning_points(galaxy, &at, Some(velocity));
                let accepted = peri.value() > corotation && apo.value() < APOCENTRE_LIMIT_LY;
                let orbit = SphericalOrbit::between(tables, peri, apo).filter(|_| accepted)?;
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
                stars: Some(ClusterStars { fe_h, age }),
            })
        })
        .take(room)
        .collect()
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
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
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
    fn the_orphan_count_makes_up_the_streams_per_globular() {
        assert!((expected_count(160.0, 40) - 200.0).abs() < 1e-12);
        assert!(expected_count(10.0, 40).abs() < 1e-12);
    }
}
