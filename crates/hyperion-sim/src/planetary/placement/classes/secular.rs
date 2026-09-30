//! Secular and near-resonant forced eccentricities of a tidally damped planet (ruling 133.1,
//! amending ruling 112.8; P14.T8.e).
//!
//! Tides damp a planet's free eccentricity. What stays is the eccentricity its neighbours force:
//!
//! - **Secular.** Each pair of planets is a two-planet Laplace–Lagrange system (Wu and Goldreich
//!   2002, ApJ 564, 1024, eq. 8, citing Murray and Dermott 1999): inner i, outer o, α = `a_i` ÷ `a_o`,
//!   `B₁` = b⁽¹⁾₃⁄₂(α), `B₂` = b⁽²⁾₃⁄₂(α),
//!   `A_ii` = (`n_i`/4)(`m_o`/M★) α² `B₁` + ϖ̇, `A_io` = −(`n_i`/4)(`m_o`/M★) α² `B₂`,
//!   `A_oi` = −(`n_o`/4)(`m_i`/M★) α `B₂`, `A_oo` = (`n_o`/4)(`m_i`/M★) α `B₁` + ϖ̇.
//!   The extra precession ϖ̇ is general relativity's, 3nGM★ ÷ (ac²) (Wu and Goldreich eq. 3c, at
//!   e = 0), on both, and on the damped planet also its tidal bulge's, 7.5 n `k₂` (M★/m)(R/a)⁵
//!   (eq. 3a). Mardling (2007, MNRAS 382, 1768, eq. 36): "the effect of `W_GR` is to decrease the
//!   equilibrium eccentricity". The two modes g± have `e_i` ÷ `e_o` = `A_io` ÷ (`A_ii` − g) (eq. 10).
//!   Tides drain the mode through the damped planet at its share of the mode's angular-momentum
//!   deficit, J = m √a (eq. 11), so the mode that survives is the one with the smaller share there;
//!   in it the damped planet holds ρ = |`e_damped` ÷ `e_neighbour`| of its neighbour's eccentricity.
//! - **Near-resonant.** A pair near a first-order commensurability j:j−1 also has a forced
//!   eccentricity that dissipation does not remove (Lithwick, Xie and Wu 2012, ApJ 761, 122,
//!   eq. A15 and Table 3): μ′|f| ÷ (√α j|Δ|) for the inner planet and μ|g| ÷ (j|Δ|) for the outer,
//!   Δ = ((j − 1)/j)(P′/P) − 1, μ and μ′ the inner's and outer's masses over the star's.
//!
//! Laplace coefficients come from their hypergeometric series, which uses only +, × and ÷, so they
//! are bit-reproducible.

use crate::math;
use crate::planetary::placement::classes::Commensurability;
use crate::units::consts::{EARTH_MASS_KG, GM_SUN, SOLAR_MASS_KG, SPEED_OF_LIGHT};
use crate::units::{EarthMasses, Metres, SolarMasses};

/// The relative size of the last term at which [`laplace_coefficient`]'s series stops: 10⁻¹⁷
/// (ruling 133.1).
pub const LAPLACE_TOLERANCE: f64 = 1e-17;

/// The most terms [`laplace_coefficient`]'s series takes: 2,000 (ruling 133.1). α = 0.95 needs
/// 395.
pub const LAPLACE_MAX_TERMS: u32 = 2_000;

/// The Laplace coefficient `b_s^(j)`(α), 0 ≤ α < 1, from its hypergeometric series (Murray and
/// Dermott 1999, eq. 6.68): 2 [(s)_j ÷ j!] α^j Σₙ [(s)ₙ (s + j)ₙ ÷ ((j + 1)ₙ n!)] α^(2n), summed
/// until a term falls below [`LAPLACE_TOLERANCE`] of the sum or [`LAPLACE_MAX_TERMS`] are taken.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::placement::classes::secular::laplace_coefficient;
///
/// // research/r-pfix14's table: b⁽¹⁾₃⁄₂(0.7) = 7.5430, b⁽²⁾₃⁄₂(0.7) = 6.1179.
/// assert!((laplace_coefficient(1.5, 1, 0.7) - 7.5430).abs() < 1e-4);
/// assert!((laplace_coefficient(1.5, 2, 0.7) - 6.1179).abs() < 1e-4);
/// ```
#[must_use]
pub fn laplace_coefficient(s: f64, j: u32, alpha: f64) -> f64 {
    debug_assert!(
        (0.0..1.0).contains(&alpha),
        "α must lie in [0, 1), got {alpha}"
    );
    let jf = f64::from(j);
    let mut prefactor = 2.0;
    for i in 0..j {
        let i = f64::from(i);
        prefactor *= (s + i) / (i + 1.0);
    }
    prefactor *= math::powi(alpha, i32::try_from(j).unwrap_or(i32::MAX));
    let (mut term, mut total) = (1.0, 1.0);
    let a2 = alpha * alpha;
    for n in 0..LAPLACE_MAX_TERMS {
        let n = f64::from(n);
        term *= (s + n) * (s + jf + n) / ((n + 1.0) * (jf + 1.0 + n)) * a2;
        total += term;
        if term < LAPLACE_TOLERANCE * total {
            break;
        }
    }
    prefactor * total
}

/// A planet as the secular theory reads it: its mass, semi-major axis, radius and Love number
/// `k₂` (the last two for its tidal bulge's precession).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecularPlanet {
    mass: EarthMasses,
    semi_major_axis: Metres,
    radius: Metres,
    love_number: f64,
}

impl SecularPlanet {
    /// A planet of `mass` at `semi_major_axis`, of `radius` and Love number `love_number`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the mass, axis and radius are positive and the Love number not
    /// negative.
    #[must_use]
    pub fn new(
        mass: EarthMasses,
        semi_major_axis: Metres,
        radius: Metres,
        love_number: f64,
    ) -> Self {
        debug_assert!(mass.value() > 0.0 && semi_major_axis.value() > 0.0 && radius.value() > 0.0);
        debug_assert!(love_number >= 0.0);
        Self {
            mass,
            semi_major_axis,
            radius,
            love_number,
        }
    }

    /// Its mean motion about a host of `host`, rad s⁻¹.
    fn mean_motion(&self, host: SolarMasses) -> f64 {
        let a = self.semi_major_axis.value();
        let solar_masses = self.mass.value() * EARTH_MASS_KG / SOLAR_MASS_KG;
        let mu = GM_SUN * (host.value() + solar_masses);
        (mu / (a * a * a)).sqrt()
    }

    /// Its mass over `host`'s.
    fn mass_ratio(&self, host: SolarMasses) -> f64 {
        self.mass.value() * EARTH_MASS_KG / (host.value() * SOLAR_MASS_KG)
    }

    /// Its apsidal precession from general relativity about `host`, 3nGM★ ÷ (ac²), rad s⁻¹ (Wu
    /// and Goldreich 2002, eq. 3c, at e = 0).
    fn relativistic_precession(&self, host: SolarMasses) -> f64 {
        3.0 * self.mean_motion(host) * GM_SUN * host.value()
            / (self.semi_major_axis.value() * SPEED_OF_LIGHT * SPEED_OF_LIGHT)
    }

    /// Its apsidal precession from the tidal bulge its host raises, 7.5 n `k₂` (M★/m)(R/a)⁵, rad
    /// s⁻¹ (Wu and Goldreich 2002, eq. 3a, at e = 0).
    fn tidal_precession(&self, host: SolarMasses) -> f64 {
        let size = self.radius.value() / self.semi_major_axis.value();
        7.5 * self.mean_motion(host) * self.love_number / self.mass_ratio(host)
            * math::powi(size, 5)
    }

    /// Its angular-momentum deficit weight, m √a (the constant √(GM★) dropped).
    fn deficit_weight(&self) -> f64 {
        self.mass.value() * self.semi_major_axis.value().sqrt()
    }
}

/// The secular mode that survives tidal damping of one planet of a pair (ruling 133.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurvivingMode {
    ratio: f64,
    share: f64,
}

impl SurvivingMode {
    /// ρ = |`e_damped` ÷ `e_neighbour`| in the mode: the damped planet's forced eccentricity per unit
    /// of its neighbour's.
    #[must_use]
    pub const fn ratio(&self) -> f64 {
        self.ratio
    }

    /// The damped planet's share of the mode's angular-momentum deficit, s in [0, 1]: the mode
    /// decays at s ÷ τ through it (Wu and Goldreich 2002, eq. 11).
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }
}

/// The mode of the pair `damped` and `neighbour` about a host of `host` that survives `damped`'s
/// tides: of the two eigenmodes of Wu and Goldreich's (2002) eq. 8 with the extra precession of
/// the [module](self) documentation, the one with the smaller share of its deficit in `damped`.
/// `None` for planets on the same axis, or a degenerate matrix.
///
/// # Examples
///
/// HD 83443 b (1.14 Saturn masses at 0.0376 au) under c (0.53 at 0.175 au), about 0.79 M☉:
/// general relativity and b's tidal bulge hold b at about a tenth of c's eccentricity, where the
/// leading-order (5⁄4) α gives 0.27 (Wu and Goldreich 2002: observed 0.19).
///
/// ```
/// use hyperion_sim::planetary::placement::classes::secular::{SecularPlanet, surviving_mode};
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::new(x * METRES_PER_AU);
/// let saturns = |x: f64| EarthMasses::new(x * 95.16);
/// let b = SecularPlanet::new(saturns(1.14), au(0.0376), Metres::new(7.1492e7), 0.5);
/// let c = SecularPlanet::new(saturns(0.53), au(0.175), Metres::new(7.1492e7), 0.5);
/// let mode = surviving_mode(SolarMasses::new(0.79), &b, &c).expect("a pair");
/// assert!((0.08..0.25).contains(&mode.ratio()), "{}", mode.ratio());
/// ```
#[must_use]
pub fn surviving_mode(
    host: SolarMasses,
    damped: &SecularPlanet,
    neighbour: &SecularPlanet,
) -> Option<SurvivingMode> {
    let damped_inner = damped.semi_major_axis < neighbour.semi_major_axis;
    let (inner, outer) = if damped_inner {
        (damped, neighbour)
    } else {
        (neighbour, damped)
    };
    let alpha = inner.semi_major_axis.value() / outer.semi_major_axis.value();
    if !(alpha > 0.0 && alpha < 1.0) {
        return None;
    }
    let (b1, b2) = (
        laplace_coefficient(1.5, 1, alpha),
        laplace_coefficient(1.5, 2, alpha),
    );
    let (ni, no) = (inner.mean_motion(host), outer.mean_motion(host));
    let (mi, mo) = (inner.mass_ratio(host), outer.mass_ratio(host));
    let tide = damped.tidal_precession(host);
    let mut a_ii = ni / 4.0 * mo * alpha * alpha * b1 + inner.relativistic_precession(host);
    let inner_outer = -ni / 4.0 * mo * alpha * alpha * b2;
    let outer_inner = -no / 4.0 * mi * alpha * b2;
    let mut a_oo = no / 4.0 * mi * alpha * b1 + outer.relativistic_precession(host);
    if damped_inner {
        a_ii += tide;
    } else {
        a_oo += tide;
    }
    let root = ((a_ii - a_oo) * (a_ii - a_oo) + 4.0 * inner_outer * outer_inner).sqrt();
    if !root.is_finite() {
        return None;
    }
    let (ji, jo) = (inner.deficit_weight(), outer.deficit_weight());
    let modes = [0.5 * (a_ii + a_oo + root), 0.5 * (a_ii + a_oo - root)].map(|g| {
        // The eigenvector (e_i, e_o), from whichever row is the better conditioned.
        let (first, second) = ((inner_outer, g - a_ii), (g - a_oo, outer_inner));
        let (ei, eo) = if first.0.abs() + first.1.abs() >= second.0.abs() + second.1.abs() {
            first
        } else {
            second
        };
        let (wi, wo) = (ji * ei * ei, jo * eo * eo);
        let total = wi + wo;
        let (ed, en, wd) = if damped_inner {
            (ei, eo, wi)
        } else {
            (eo, ei, wo)
        };
        (ed, en, if total > 0.0 { wd / total } else { f64::NAN })
    });
    let [first, second] = modes;
    let (ed, en, share) = if first.2 <= second.2 { first } else { second };
    let ratio = (ed / en).abs();
    (ratio.is_finite() && share.is_finite()).then_some(SurvivingMode { ratio, share })
}

/// The near-resonant forced eccentricities of the pair `inner` and `outer` of masses `inner_mass`
/// and `outer_mass` about `host`, near `commensurability` with fractional offset `offset` (the
/// period ratio (1 + offset) times the commensurability's): Lithwick, Xie and Wu's (2012) eq. A15
/// with their Table 3's f and g, (inner, outer). `None` for a second-order commensurability (5:3),
/// which LXW do not cover.
#[must_use]
pub fn resonant_forced(
    host: SolarMasses,
    (inner_mass, inner_axis): (EarthMasses, Metres),
    (outer_mass, outer_axis): (EarthMasses, Metres),
    commensurability: Commensurability,
    offset: f64,
) -> Option<(f64, f64)> {
    // Table 3: f = f₀ + f₁Δ, g = g₀ + g₁Δ, for j:j−1.
    let (j, f0, f1, g0, g1) = match commensurability {
        Commensurability::TwoToOne => (2.0, -1.190, 2.20, 0.4284, -3.69),
        Commensurability::ThreeToTwo => (3.0, -2.025, 6.21, 2.484, -5.99),
        Commensurability::FourToThree => (4.0, -2.840, 12.20, 3.283, -11.9),
        Commensurability::FiveToThree => return None,
    };
    // Δ = ((j − 1)/j)(P′/P) − 1 with P′/P = (1 + ε) j/(j − 1) is ε.
    let delta = offset;
    let ratio = |m: EarthMasses| m.value() * EARTH_MASS_KG / (host.value() * SOLAR_MASS_KG);
    let alpha = inner_axis.value() / outer_axis.value();
    let (f, g) = (f0 + f1 * delta, g0 + g1 * delta);
    let denominator = j * delta.abs();
    let inner = ratio(outer_mass) * f.abs() / (alpha.sqrt() * denominator);
    let outer = ratio(inner_mass) * g.abs() / denominator;
    Some((inner, outer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::METRES_PER_AU;

    fn au(x: f64) -> Metres {
        Metres::new(x * METRES_PER_AU)
    }

    /// The series against direct quadrature of (1/π) ∫ cos jψ (1 − 2α cos ψ + α²)^(−s) dψ over a
    /// turn, α 0.3–0.95, to 10⁻¹² relative.
    #[test]
    fn the_series_matches_quadrature() {
        let quadrature = |s: f64, j: u32, a: f64| {
            let n = 20_000_u32;
            let h = core::f64::consts::TAU / f64::from(n);
            (0..n)
                .map(|k| {
                    let psi = f64::from(k) * h;
                    math::cos(f64::from(j) * psi)
                        / math::powf(1.0 - 2.0 * a * math::cos(psi) + a * a, s)
                })
                .sum::<f64>()
                * h
                / core::f64::consts::PI
        };
        for a in [0.3, 0.5, 0.7, 0.8, 0.9, 0.95] {
            for j in [1, 2] {
                let (series, direct) = (laplace_coefficient(1.5, j, a), quadrature(1.5, j, a));
                assert!(
                    ((series - direct) / direct).abs() < 1e-12,
                    "{a} {j}: {series} {direct}"
                );
            }
        }
        // research/r-pfix14's ratio b⁽²⁾ ÷ b⁽¹⁾ at 0.8, against the withdrawn (5⁄4) α = 1.
        let r = laplace_coefficient(1.5, 2, 0.8) / laplace_coefficient(1.5, 1, 0.8);
        assert!((r - 0.8979).abs() < 1e-4, "{r}");
    }

    /// Mardling (2007) eq. 36 at small α, `e_c` ≪ 1: `e_p` ÷ `e_c` = (5⁄4) α ÷ |1 − √α (`m_p`/`m_c`) +
    /// γ|, γ = 4 (`n_p` `a_p`/c)² (m★/`m_c`) α⁻³, with no tidal bulge (k₂ = 0).
    #[test]
    fn mardling_s_equilibrium_is_recovered_at_small_alpha() {
        let host = SolarMasses::new(1.0);
        for (mp, mc, ap, ac) in [
            (10.0, 317.8, 0.05, 2.0),
            (317.8, 317.8, 0.04, 1.5),
            (5.0, 100.0, 0.1, 3.0),
        ] {
            let p = SecularPlanet::new(EarthMasses::new(mp), au(ap), Metres::new(1e7), 0.0);
            let c = SecularPlanet::new(EarthMasses::new(mc), au(ac), Metres::new(7e7), 0.0);
            let mode = surviving_mode(host, &p, &c).unwrap();
            let alpha = ap / ac;
            let np = p.mean_motion(host);
            let v = np * au(ap).value() / SPEED_OF_LIGHT;
            let gamma = 4.0 * v * v * (1.0 / c.mass_ratio(host)) / (alpha * alpha * alpha);
            let expected = 1.25 * alpha / (1.0 - alpha.sqrt() * mp / mc + gamma).abs();
            let off = mode.ratio() / expected - 1.0;
            assert!(
                off.abs() < 0.05,
                "{mp} {mc} {ap} {ac}: {} against {expected}",
                mode.ratio()
            );
        }
    }

    /// A light hot planet under a massive companion holds a tiny share of the surviving mode, so
    /// its floor lasts; an equal pair at α 0.75 shares it about equally, so the floor damps with
    /// the planet (research/r-pfix14, §1).
    #[test]
    fn the_surviving_mode_s_share_follows_the_masses() {
        let host = SolarMasses::new(1.0);
        let close = SecularPlanet::new(EarthMasses::new(317.8), au(0.05), Metres::new(8.6e7), 0.5);
        let cold = SecularPlanet::new(EarthMasses::new(317.8), au(1.0), Metres::new(7e7), 0.5);
        let mode = surviving_mode(host, &close, &cold).unwrap();
        assert!(mode.share() < 1e-4, "{}", mode.share());
        // GR and the bulge cut the floor well below (5⁄4) α = 0.0625 (research: 0.0039).
        assert!(mode.ratio() < 0.01, "{}", mode.ratio());
        let a = SecularPlanet::new(EarthMasses::new(5.0), au(0.0375), Metres::new(1e7), 0.3);
        let b = SecularPlanet::new(EarthMasses::new(5.0), au(0.05), Metres::new(1e7), 0.3);
        let equal = surviving_mode(host, &a, &b).unwrap();
        assert!((0.3..0.6).contains(&equal.share()), "{}", equal.share());
        assert!(surviving_mode(host, &a, &a).is_none());
    }

    /// A TRAPPIST-1-like chain near 3:2 and 4:3 holds 10⁻³–10⁻² by LXW's forced term (Agol et
    /// al. 2021's 0.002–0.008; the masses and periods illustrative).
    #[test]
    fn a_trappist_like_resonant_chain_keeps_a_few_thousandths() {
        let host = SolarMasses::new(0.0898);
        let pairs = [
            // The offsets from the axes: (a′/a)^1.5 ÷ the commensurability − 1.
            (
                (0.388, 0.02227),
                (0.692, 0.02925),
                Commensurability::ThreeToTwo,
                0.0035,
            ),
            (
                (0.692, 0.02925),
                (1.039, 0.03849),
                Commensurability::ThreeToTwo,
                0.0064,
            ),
            (
                (1.039, 0.03849),
                (1.321, 0.04683),
                Commensurability::FourToThree,
                0.0066,
            ),
        ];
        for ((mi, ai), (mo, ao), c, offset) in pairs {
            let (inner, outer) = resonant_forced(
                host,
                (EarthMasses::new(mi), au(ai)),
                (EarthMasses::new(mo), au(ao)),
                c,
                offset,
            )
            .unwrap();
            for e in [inner, outer] {
                assert!((1e-3..1e-2).contains(&e), "{c:?}: {inner} {outer}");
            }
        }
        let far = resonant_forced(
            host,
            (EarthMasses::new(1.0), au(0.02)),
            (EarthMasses::new(1.0), au(0.03)),
            Commensurability::FiveToThree,
            0.01,
        );
        assert_eq!(far, None);
    }
}
