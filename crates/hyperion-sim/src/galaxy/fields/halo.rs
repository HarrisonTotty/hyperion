//! The stellar halo's smooth components (brainstorm, "Streams and accreted structure"; plan 02,
//! P02.T7.d).

use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::galaxy::params::{HaloComponentKind, HaloComponentParams};
use crate::galaxy::quad::{Gl16Panel, gl32};
use crate::math;
use crate::units::LightYears;

/// One smooth component of the halo's marked mixture: a cored, flattened, possibly broken power
/// law, `n0 (1 + m² ÷ a²)^(−γ ÷ 2) B(m)` systems per cubic light-year in the ellipsoidal radius
/// `m² = x² + y² + z² ÷ q²`, inside the sphere of the cut radius `r_c` and none beyond.
///
/// The brainstorm's components are "cored, flattened or triaxial" power laws that never rise
/// with |x|, |y| or |z|. The parameters draw one axis ratio, `q ≤ 1` (plan 02, P02.T5.a), so the
/// in-plane ratio `p` of plan 02's P02.T7.d is 1. The dominant merger's slope steepens by `Δ`
/// beyond its break `r_b` through the continuous factor `B(m) = min(1, (m ÷ r_b)^(−Δ))`, a broken
/// power law as Deason, Belokurov and Evans (2011, MNRAS 416, 2903) fit the Milky Way's; that the
/// break is where a massive progenitor's debris piles up at apocentre is Deason et al. (2013, ApJ
/// 763, 113). Every other component has `B = 1`.
///
/// The halo stops at 65,000 ly so that it fits inside the root cube, and the in-situ component
/// lies inside 50,000 ly (brainstorm, "Populations" and "Streams and accreted structure"). The cut
/// is a sphere of that radius, not an ellipsoid in `m` as plan 02's Design note 11 has it: an
/// ellipsoid of axis ratio `q` ends at `q r_c` over the poles, 45,000 ly for the Milky Way's
/// dominant merger, which steepens the spherically averaged profile between 20,000 and 60,000 ly
/// to about r^−4.2 against the brainstorm's "near r^−3.5" (plan 02, Risks, R16).
/// The core, the flattening, the break and the sphere each only lower the density as |x|, |y| or
/// |z| grows, so the nearest-corner bound stays exact, and every step of the computation is
/// monotone as well ([`envelope`](Self::envelope)).
///
/// The component is normalised inside the cut: `count = n0 × 4π ∫₀¹ s(μ)⁻³ F(r_c s(μ)) dμ`, with
/// `F(M) = ∫₀^M (1 + m² ÷ a²)^(−γ ÷ 2) B(m) m² dm` and `s(μ) = √(1 + μ² (q⁻² − 1))` the ratio of
/// `m` to `r` along the direction of cosine `μ` to the z axis. `F` is a radial quadrature on fixed
/// panels, read at every `M` from its node values ([`Gl16Panel`]); the integral over `μ` is
/// `gl32`.
///
/// `(1 + u)^(−γ ÷ 2)` is evaluated as `exp(−γ ÷ 2 × ln_1p(u))`, the same function as the power
/// at under half its cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HaloProfile {
    kind: HaloComponentKind,
    n0: f64,
    core: LightYears,
    flattening: f64,
    slope: f64,
    cut_radius: LightYears,
    break_radius: Option<LightYears>,
    steepening: f64,
    /// `count ÷ n0`, ly³.
    volume: f64,
    inv_core_sq: f64,
    inv_flattening_sq: f64,
    half_slope: f64,
    /// `r_c²`, ly².
    cut_sq: f64,
    /// `1 ÷ r_b²`, or 0 without a break.
    inv_break_sq: f64,
    half_steepening: f64,
}

impl HaloProfile {
    /// The halo component `params` holding `count` systems inside its cut.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative or not finite.
    pub fn of(count: f64, params: &HaloComponentParams) -> Result<Self, BuildFieldError> {
        Self::build(
            count,
            params.kind(),
            params.slope(),
            params.core(),
            params.flattening(),
            params.outer_break().map(|b| (b.radius(), b.steepening())),
            params.cut_radius(),
        )
    }

    /// A component of `count` systems with slope `γ`, core `a`, flattening `q`, an optional break
    /// `(r_b, Δ)` and a cut `r_c`.
    fn build(
        count: f64,
        kind: HaloComponentKind,
        slope: f64,
        core: LightYears,
        flattening: f64,
        outer_break: Option<(LightYears, f64)>,
        cut_radius: LightYears,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        BuildFieldError::check_positive("halo slope", slope)?;
        BuildFieldError::check_positive("halo core", core.value())?;
        BuildFieldError::check_fraction("halo flattening", flattening)?;
        BuildFieldError::check_positive("halo flattening", flattening)?;
        BuildFieldError::check_positive("halo cut", cut_radius.value())?;
        let (inv_break_sq, steepening) = match outer_break {
            Some((radius, steepening)) => {
                BuildFieldError::check_positive("halo break", radius.value())?;
                BuildFieldError::check_non_negative("halo steepening", steepening)?;
                let r = radius.value();
                (1.0 / (r * r), steepening)
            }
            None => (0.0, 0.0),
        };
        let (a, c) = (core.value(), cut_radius.value());
        let mut profile = Self {
            kind,
            n0: 0.0,
            core,
            flattening,
            slope,
            cut_radius,
            break_radius: outer_break.map(|(radius, _)| radius),
            steepening,
            volume: 0.0,
            inv_core_sq: 1.0 / (a * a),
            inv_flattening_sq: 1.0 / (flattening * flattening),
            half_slope: 0.5 * slope,
            cut_sq: c * c,
            inv_break_sq,
            half_steepening: 0.5 * steepening,
        };
        profile.volume = profile.unit_count();
        profile.n0 = count / profile.volume;
        Ok(profile)
    }

    /// `4π ∫₀¹ s(μ)⁻³ F(r_c s(μ)) dμ`: the count at `n0 = 1` (the type's documentation).
    fn unit_count(&self) -> f64 {
        let cut = self.cut_radius.value();
        let stretch = self.inv_flattening_sq - 1.0;
        let radial = RadialIntegral::new(self, cut / self.flattening);
        let integrand = |mu: f64| {
            let s = (1.0 + mu * mu * stretch).sqrt();
            radial.to(cut * s) / (s * s * s)
        };
        // Where the sphere meets the break, F has a kink in its second derivative: a panel edge.
        let mut edges = vec![0.0];
        if let Some(radius) = self.break_radius {
            let s = radius.value() / cut;
            if stretch > 0.0 && s > 1.0 && s * s < 1.0 + stretch {
                edges.push(((s * s - 1.0) / stretch).sqrt());
            }
        }
        edges.push(1.0);
        let over_mu = edges
            .windows(2)
            .fold(0.0, |sum, panel| sum + gl32(integrand, panel[0], panel[1]));
        4.0 * core::f64::consts::PI * over_mu
    }

    /// The unnormalised profile at `m²`, without the cut: `(1 + m² ÷ a²)^(−γ ÷ 2) B(m)`.
    ///
    /// Every step is a non-decreasing function of `m²` followed by a negation, so the computed
    /// value never rises with `m²` in floating point either, as long as `ln_1p`, `ln` and `exp` do
    /// not fall: the break is taken where `m² ÷ r_b²` itself exceeds 1, where its logarithm is
    /// positive, so the steepening can only lower the profile.
    fn profile(&self, m_sq: f64) -> f64 {
        let mut exponent = -self.half_slope * math::ln_1p(m_sq * self.inv_core_sq);
        let beyond = m_sq * self.inv_break_sq;
        if beyond > 1.0 {
            exponent -= self.half_steepening * math::ln(beyond);
        }
        math::exp(exponent)
    }

    /// Which component of the halo this is: the mark a system placed by it carries.
    #[must_use]
    pub fn kind(&self) -> HaloComponentKind {
        self.kind
    }

    /// The central density `n0`, systems per cubic light-year.
    #[must_use]
    pub fn n0(&self) -> f64 {
        self.n0
    }

    /// The power-law slope γ.
    #[must_use]
    pub fn slope(&self) -> f64 {
        self.slope
    }

    /// The core radius `a`.
    #[must_use]
    pub fn core(&self) -> LightYears {
        self.core
    }

    /// The vertical axis ratio `q`, at most 1.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        self.flattening
    }

    /// The break radius `r_b`, for the dominant merger only.
    #[must_use]
    pub fn break_radius(&self) -> Option<LightYears> {
        self.break_radius
    }

    /// How much the slope steepens beyond the break, `Δ`; 0 without one.
    #[must_use]
    pub fn steepening(&self) -> f64 {
        self.steepening
    }

    /// The radius `r_c` of the sphere outside which the component has no systems.
    #[must_use]
    pub fn cut_radius(&self) -> LightYears {
        self.cut_radius
    }

    /// The number of systems inside the cut.
    #[must_use]
    pub fn count(&self) -> f64 {
        self.volume * self.n0
    }

    /// The ellipsoidal radius squared, `m² = R² + z² ÷ q²`, ly², for `r_sq = x² + y²`.
    #[must_use]
    pub fn radius_sq(&self, r_sq: f64, z: f64) -> f64 {
        r_sq + z * z * self.inv_flattening_sq
    }

    /// The density at in-plane radius squared `r_sq = x² + y²` and height `z` (ly), systems per
    /// cubic light-year: 0 outside the sphere of the cut.
    #[must_use]
    pub fn envelope(&self, r_sq: f64, z: f64) -> f64 {
        let z_sq = z * z;
        if r_sq + z_sq > self.cut_sq {
            return 0.0;
        }
        self.n0 * self.profile(r_sq + z_sq * self.inv_flattening_sq)
    }

    /// The density at `p`, systems per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.envelope(p.x * p.x + p.y * p.y, p.z)
    }

    /// The density at `site`.
    pub(crate) fn density_at(&self, site: &Site) -> f64 {
        self.envelope(site.r_sq, site.abs_z)
    }
}

/// `F(M) = ∫₀^M f(m) m² dm` of a component's unnormalised profile `f`, for any `M` up to a reach:
/// the 16-point rule on `[0, a]` in `m` and on panels doubling in `m` beyond, in `ln m`, with the
/// break and the cut as edges, each read at `M` through its interpolating polynomial.
#[derive(Debug)]
struct RadialIntegral {
    core: f64,
    linear: Gl16Panel,
    /// Panels in `ln m`, with the integral up to each one's start.
    log: Vec<(Gl16Panel, f64, f64)>,
}

impl RadialIntegral {
    fn new(profile: &HaloProfile, reach: f64) -> Self {
        let a = profile.core.value().min(reach);
        let values = Gl16Panel::nodes(0.0, a).map(|m| profile.profile(m * m) * m * m);
        let linear = Gl16Panel::new(0.0, a, &values);
        let mut edges = vec![a];
        let mut edge = 2.0 * a;
        while edge < reach {
            edges.push(edge);
            edge *= 2.0;
        }
        edges.push(reach);
        let cut = profile.cut_radius.value();
        for kink in [Some(cut), profile.break_radius.map(LightYears::value)]
            .into_iter()
            .flatten()
        {
            let at = edges.partition_point(|&e| e < kink);
            if kink > a && at < edges.len() && edges[at] > kink {
                edges.insert(at, kink);
            }
        }
        let mut below = linear.integral();
        let log = edges
            .windows(2)
            .map(|panel| {
                let (lo, hi) = (math::ln(panel[0]), math::ln(panel[1]));
                let values = Gl16Panel::nodes(lo, hi).map(|u| {
                    let m = math::exp(u);
                    profile.profile(m * m) * m * m * m
                });
                let part = Gl16Panel::new(lo, hi, &values);
                let start = below;
                below += part.integral();
                (part, hi, start)
            })
            .collect();
        Self {
            core: a,
            linear,
            log,
        }
    }

    /// `F(m)` for `0 ≤ m ≤` the reach.
    fn to(&self, m: f64) -> f64 {
        if m <= self.core {
            return self.linear.integral_to(m);
        }
        let u = math::ln(m);
        let i = self
            .log
            .partition_point(|&(_, hi, _)| hi < u)
            .min(self.log.len() - 1);
        let (panel, _, start) = &self.log[i];
        start + panel.integral_to(u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_panels;

    fn broken(flattening: f64) -> HaloProfile {
        HaloProfile::build(
            1e9,
            HaloComponentKind::DominantMerger,
            3.5,
            LightYears::new(3_000.0),
            flattening,
            Some((LightYears::new(50_000.0), 2.0)),
            LightYears::new(65_000.0),
        )
        .unwrap()
    }

    /// The normalisation against a direct two-dimensional quadrature over `μ` and `r`, with fine
    /// panels in both and the core, the break and the cut as edges along each ray.
    #[test]
    fn the_count_is_the_integral_inside_the_sphere() {
        for q in [1.0, 0.8, 0.5] {
            let halo = broken(q);
            let stretch = 1.0 / (q * q) - 1.0;
            let ray = |mu: f64| {
                let s = (1.0 + mu * mu * stretch).sqrt();
                let mut edges: Vec<f64> = (0..=65).map(|i| 1_000.0 * f64::from(i)).collect();
                edges.extend([3_000.0 / s, 50_000.0 / s]);
                edges.sort_by(f64::total_cmp);
                edges.dedup();
                gl_panels(|r| halo.profile(r * r * s * s) * r * r, &edges)
            };
            let mu_edges: Vec<f64> = (0..=40).map(|i| f64::from(i) / 40.0).collect();
            let direct = 4.0 * core::f64::consts::PI * gl_panels(ray, &mu_edges);
            assert!(
                (halo.unit_count() / direct - 1.0).abs() < 1e-9,
                "q {q}: {} against {direct}",
                halo.unit_count()
            );
            assert!((halo.count() / 1e9 - 1.0).abs() < 1e-15);
        }
    }

    /// `F` read between the nodes against a direct quadrature to the same radius, to 10⁻¹⁰ of the
    /// whole: the normalisation reads it only beyond the cut radius.
    #[test]
    fn the_radial_integral_reads_any_radius() {
        let halo = broken(0.6);
        let radial = RadialIntegral::new(&halo, 65_000.0 / 0.6);
        let whole = radial.to(65_000.0 / 0.6);
        for m in [
            10.0, 1_500.0, 3_000.0, 7_777.0, 49_000.0, 50_000.0, 65_000.0, 100_000.0,
        ] {
            let mut edges = vec![0.0, 1_000.0, 3_000.0];
            edges.extend(
                (1..=108)
                    .map(|i| 1_000.0 * f64::from(i))
                    .filter(|&e| e > 3_000.0),
            );
            edges.push(50_000.0);
            edges.retain(|&e| e < m);
            edges.sort_by(f64::total_cmp);
            edges.dedup();
            edges.push(m);
            let direct = gl_panels(|x| halo.profile(x * x) * x * x, &edges);
            assert!(
                (radial.to(m) - direct).abs() < 1e-10 * whole,
                "F({m}): {} against {direct}",
                radial.to(m)
            );
        }
    }

    /// The break steepens the slope by Δ and keeps the density continuous.
    #[test]
    fn the_break_is_continuous_and_steepens_the_slope() {
        let halo = broken(0.7);
        let at = |m: f64| halo.envelope(m * m, 0.0);
        let r_b = 50_000.0;
        assert!((at(r_b * (1.0 + 1e-12)) / at(r_b) - 1.0).abs() < 1e-10);
        let slope =
            |m: f64| (math::ln(at(m * 1.01)) - math::ln(at(m / 1.01))) / (2.0 * math::ln(1.01));
        let core_term = |m: f64| -3.5 * m * m / (m * m + 3_000.0 * 3_000.0);
        assert!((slope(30_000.0) - core_term(30_000.0)).abs() < 1e-3);
        assert!((slope(60_000.0) - (core_term(60_000.0) - 2.0)).abs() < 1e-3);
    }

    /// Stepped one representable value at a time across the core, the break and the cut, in the
    /// plane and over the pole, the density never rises: the edge cases of the nearest-corner
    /// bound (plan 02, P02.T8).
    #[test]
    fn the_density_never_rises_across_the_core_the_break_or_the_cut() {
        let halo = broken(0.7);
        let steps = |start: f64, at: &dyn Fn(f64) -> f64| {
            let mut v = start * (1.0 - 1e-13);
            let mut previous = at(v);
            for _ in 0..20_000 {
                v = v.next_up();
                let density = at(v);
                assert!(density <= previous, "rises at {v} from {start}");
                previous = density;
            }
        };
        // The core and the break in m, in the plane and over the pole (z = q m), and the cut, a
        // sphere, in both.
        for m in [3_000.0, 50_000.0] {
            steps(m * m, &|r_sq| halo.envelope(r_sq, 0.0));
            steps(0.7 * m, &|z| halo.envelope(0.0, z));
        }
        steps(65_000.0 * 65_000.0, &|r_sq| halo.envelope(r_sq, 0.0));
        steps(65_000.0, &|z| halo.envelope(0.0, z));
    }

    /// The cut is a sphere, whatever the flattening.
    #[test]
    fn nothing_lies_beyond_the_cut() {
        let halo = broken(0.7);
        let edge = 65_000.0 * 65_000.0;
        assert!(halo.envelope(edge, 0.0) > 0.0);
        assert!(halo.envelope(edge * (1.0 + 1e-12), 0.0).abs() < f64::MIN_POSITIVE);
        assert!(halo.envelope(0.0, 65_000.0) > 0.0);
        assert!(halo.envelope(0.0, 65_000.001).abs() < f64::MIN_POSITIVE);
        assert!(halo.envelope(0.5 * edge, 0.707 * 65_000.0) > 0.0);
    }

    #[test]
    fn invalid_components_are_rejected() {
        let a = LightYears::new(3_000.0);
        let build = |q: f64| {
            HaloProfile::build(1.0, HaloComponentKind::InSitu, 3.5, a, q, None, a)
                .unwrap_err()
                .quantity()
        };
        assert_eq!(build(1.2), "halo flattening");
        assert_eq!(build(0.0), "halo flattening");
    }
}
