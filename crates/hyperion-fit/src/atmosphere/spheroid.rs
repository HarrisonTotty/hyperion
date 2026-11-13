//! The level spheroid an oblate body's atmosphere stands on, and the quadric shells the tracer
//! walks over it (plan R08, R08.T12.d; Design note 17).
//!
//! A hydrostatic atmosphere under one T(p) stands at heights proportional to 1 ÷ g(φ), so the
//! client builds its medium once at `g_ref` = √(γₑ γₚ) and reads it at the **gravity-scaled
//! height** h\* = s h, s = γ(φ) ÷ `g_ref`, with h the geodetic height above the datum and γ(φ) the
//! normal gravity at the geodetic latitude φ (Design note 17): the level spheroid's geopotential
//! height (W₀ − W) ÷ `g_ref` to first order in h ÷ R, with g held at its datum value γ(φ) along the
//! normal; the client's column adds the second order in the U.S. Standard Atmosphere 1976's form at
//! `R_ref` and `g_ref` (`column.ts`), which the case's profiles carry (decision-r08-spheroid). The
//! tracer reads every density profile at h\* in the same way, so that a case's profiles, its shell
//! heights and its `topHeightM` are all gravity-scaled heights. Where the client and the tracer
//! meet, they agree by construction:
//!
//! - the figure is the client's `LevelSpheroid` (`view/atmosphere/oblate.ts`), field for field, its
//!   ω being Design note 17's `ω_fig`, the spin under which plan 14's figure is level (the client's
//!   `bodyGravity` picks it by figure law), and the gravity added at every latitude its
//!   `BodyGravity.gravityOffsetMS2` (ω²R on a `rotational_and_tidal` figure, 0 otherwise);
//! - γ(φ), γₑ and γₚ are its `normalGravity`, in the same arithmetic and with the same series below
//!   e′ = 0.25; g(φ) = γ(φ) + the offset, `g_ref` = √(gₑ gₚ) and s = g(φ) ÷ `g_ref` are its
//!   `BodyGravity.referenceGravityMS2` and `gravityRatio` (all held to the client's printed values
//!   to 10⁻¹² by tests, for rotational, tidal and capped figures);
//! - latitudes are geodetic, as R05's `geodeticOf` and R08.T6.e's closed form give them;
//! - the tracer marches the true spheroid, which the client's slices over κ = s `R_α` ÷ `R_ref` and
//!   bands over s (R08.T3.d's `oblateSlicing`) approximate, so the gate (R08.T13) measures what
//!   the slicing costs. R05's `tableRadiusM`, (2a + c) ÷ 3, is the radius of the
//!   [`Shells::Sphere`](super::case::Shells::Sphere) a body takes when it is traced on one sphere,
//!   as the client's per-planet tables are built on it; the spheroid needs none, and at a = c the
//!   two are the same sphere.
//!
//! Sources:
//!
//! - **Normal gravity**, Somigliana's closed form for a level ellipsoid of any flattening:
//!   γ(φ) = (a γₑ cos²φ + c γₚ sin²φ) ÷ √(a² cos²φ + c² sin²φ) (Heiskanen and Moritz 1967,
//!   _Physical Geodesy_, eq. 2-78; NIMA TR8350.2, Third Edition, Amendment 1, 2000, eq. 4-1, in the
//!   equivalent form γₑ (1 + k sin²φ) ÷ √(1 − e² sin²φ)), with
//!   γₑ = GM ÷ (ac) × (1 − m − m e′q₀′ ÷ 6q₀) and γₚ = GM ÷ a² × (1 + m e′q₀′ ÷ 3q₀) (eqs. 2-73
//!   and 2-74), m = ω²a²c ÷ GM (eq. 2-70), e′ = √(a² − c²) ÷ c, q₀ (eq. 2-58) and q₀′ (eq. 2-67).
//!   Below e′ = 0.25, e′q₀′ ÷ q₀ is summed from arctan's Maclaurin series, where the closed forms
//!   cancel, as the client does ([`q_ratio`]).
//! - **Geocentric from geodetic**: X = (N + h) cos φ, Z = (N(1 − e²) + h) sin φ on the meridian of
//!   longitude 0, N = a ÷ √(1 − e² sin²φ) (NIMA TR8350.2 eqs. 4-14 and 4-15; Heiskanen and Moritz
//!   1967, eqs. 5-3 and 5-5).
//! - **Geodetic from geocentric**: Vermeille 2002 (J. Geodesy 76, 451–454,
//!   doi:10.1007/s00190-002-0273-6), a closed form, in the forms Karney 2011 (arXiv:1102.1215,
//!   App. B) restates it in ([`LevelSpheroid::geodetic`]).
//! - **The shells** are quadrics, ellipsoids of revolution, so that a ray crosses each in closed
//!   form; the quadric through h\* = H at the equator and the poles is (a + H ÷ sₑ, c + H ÷ sₚ).
//!   Between them it departs from the surface h\* = H by up to 1.7% of H on Saturn (at 46.5°),
//!   0.5% on Jupiter and 4 × 10⁻⁶ on Earth (computed here), which costs the delta tracking a looser
//!   majorant, never a bias: each shell's majorant bounds the density over every h\* the shell can
//!   hold ([`SpheroidShells`]). The ground is the datum itself; the top is the quadric through
//!   h\* = `topHeightM`, which the medium fills, so between the equator and the poles it holds h\*
//!   up to 1.7% past `topHeightM` on Saturn, where every profile is read as it is past its top
//!   (an exponential's 40 scale heights hold e⁻⁴⁰ of its density; a table's last node holds).
//!
//! The client's caveats hold here unchanged (`oblate.ts`'s module documentation): zonal winds, the
//! real 1-bar surface's departure from the spheroid and T(p) varying with latitude are not
//! modelled, and every figure is taken as a level surface of its own spin, which a
//! `rotational_and_tidal`, `sphere` or `capped` figure law is not quite.
//!
//! **Scope.** Any oblate spheroid, 0 < c ≤ a, with any spin below breakup (γₑ > 0), under every
//! figure law plan 14 gives, through `ω_fig` and the added gravity: the generator's figures are
//! spheroids of revolution with flattening at most 0.2 (`FLATTENING_CAP`, `planetary::params`),
//! and the geodetic conversion is tested to c ÷ a = 0.2, past the flattening (c ÷ a < 1 ÷ √2) at
//! which the evolute pierces the surface above the poles. Triaxial and prolate figures are not
//! traced: the generator makes none (a synchronous body's 4 : 1 : 3 tidal figure is drawn as its
//! spheroid, decision-p14-phase-j, 3), a triaxial body's geodetic conversion has no closed form in
//! radicals (it reduces to the positive root of a sextic: Diaz-Toca, Marín and Necula 2020,
//! arXiv:1909.06452), and Somigliana's normal gravity is a level ellipsoid of revolution's.

use core::f64::consts::FRAC_PI_2;

use hyperion_sim::math;

use super::geometry::{Exit, LocalFrame, Next, Vec3, boundary_heights};

/// e′ below which e′q₀′ ÷ q₀ is summed as a series, where its closed form cancels (the client's
/// `SERIES_BELOW_E_PRIME`).
const SERIES_BELOW_E_PRIME: f64 = 0.25;

/// Terms of the q-ratio series: e′² < 1 ÷ 16, so 16 terms leave under 10⁻¹⁹.
const SERIES_TERMS: u32 = 16;

/// Latitudes from the equator to the pole, both ends included, over which [`SpheroidShells`]
/// bounds the gravity-scaled height each shell can hold.
const BOUND_SCAN_STEPS: u32 = 1024;

/// e′q₀′ ÷ q₀, the one function of the second eccentricity e′ that γₑ and γₚ need (Heiskanen
/// and Moritz 1967, eqs. 2-58 and 2-67; the client's `qRatio`, in the same arithmetic).
///
/// q₀ = ½[(1 + 3 ÷ e′²) arctan e′ − 3 ÷ e′] and q₀′ = 3(1 + 1 ÷ e′²)(1 − arctan e′ ÷ e′) − 1 vanish
/// as e′³ and e′², so below [`SERIES_BELOW_E_PRIME`] both are summed from arctan's Maclaurin
/// series, term by term: q₀ = Σ (−1)^(k+1) 2k e′^(2k+1) ÷ ((2k + 1)(2k + 3)) and
/// q₀′ = Σ (−1)^(k+1) 6 e′^(2k) ÷ ((2k + 1)(2k + 3)), k ≥ 1 (derived in R08.T3.d and re-derived
/// here). The ratio is 3 at the sphere.
#[must_use]
fn q_ratio(e_prime: f64) -> f64 {
    if e_prime >= SERIES_BELOW_E_PRIME {
        let atan = math::atan(e_prime);
        let e2 = e_prime * e_prime;
        let q0 = 0.5 * ((1.0 + 3.0 / e2) * atan - 3.0 / e_prime);
        let q0_prime = 3.0 * (1.0 + 1.0 / e2) * (1.0 - atan / e_prime) - 1.0;
        return e_prime * q0_prime / q0;
    }
    // Both series divided by e′³, in y = e′², summed from the smallest term (Horner's rule).
    let y = e_prime * e_prime;
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for k in (1..=SERIES_TERMS).rev() {
        let sign = if k % 2 == 1 { 1.0 } else { -1.0 };
        let k = f64::from(k);
        let d = (2.0 * k + 1.0) * (2.0 * k + 3.0);
        numerator = numerator * y + sign * 6.0 / d;
        denominator = denominator * y + sign * 2.0 * k / d;
    }
    numerator / denominator
}

/// A point's geodetic coordinates on a [`LevelSpheroid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geodetic {
    /// cos φ of the geodetic latitude φ, not negative.
    pub(crate) cos_latitude: f64,
    /// sin φ.
    pub(crate) sin_latitude: f64,
    /// The height above the datum along its normal, m; negative below it.
    pub(crate) height_m: f64,
    /// The datum's outward unit normal at the point's foot.
    pub(crate) normal: Vec3,
}

/// A body's level spheroid: the ellipsoid of revolution (a, a, c) about its spin axis, z, that is
/// a surface of constant gravity potential for its GM and spin ω (Heiskanen and Moritz 1967,
/// §2-7), with its normal gravity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LevelSpheroid {
    /// a, m.
    equatorial_radius_m: f64,
    /// c, m.
    polar_radius_m: f64,
    /// e² = (a² − c²) ÷ a².
    e2: f64,
    /// 1 − e² = c² ÷ a².
    one_minus_e2: f64,
    /// γₑ, m s⁻².
    equatorial_gravity_m_s2: f64,
    /// γₚ, m s⁻².
    polar_gravity_m_s2: f64,
    /// The gravity added to γ(φ) at every latitude, m s⁻² (the client's `gravityOffsetMS2`).
    gravity_offset_m_s2: f64,
    /// `g_ref` = √(gₑ gₚ) of g(φ) = γ(φ) + the offset, m s⁻².
    reference_gravity_m_s2: f64,
    /// 1 ÷ a².
    inv_a2_per_m2: f64,
    /// a γₑ ÷ `g_ref` and c γₚ ÷ `g_ref`, m: Somigliana's numerator scaled for s.
    scaled_weights_m: (f64, f64),
    /// The offset ÷ `g_ref`.
    scaled_offset: f64,
}

/// Why a level spheroid could not be built ([`LevelSpheroid::new`]).
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub(crate) enum BuildLevelSpheroidError {
    /// The radii are not finite with 0 < c ≤ a.
    #[error(
        "a level spheroid's radii must be finite with 0 < c <= a, not a = {equatorial_radius_m} m and c = {polar_radius_m} m"
    )]
    Radii {
        /// a, m.
        equatorial_radius_m: f64,
        /// c, m.
        polar_radius_m: f64,
    },
    /// GM is not finite and positive.
    #[error("a level spheroid's GM must be finite and positive, not {0} m^3/s^2")]
    GravitationalParameter(f64),
    /// ω is not finite.
    #[error("a level spheroid's spin must be finite, not {0} rad/s")]
    Spin(f64),
    /// The spin flings the equator off: γₑ ≤ 0.
    #[error("a spin of {0} rad/s is past breakup: the equator's normal gravity is not positive")]
    PastBreakup(f64),
    /// The added gravity is not finite, or leaves g ≤ 0 at the equator or the poles.
    #[error("a gravity offset of {0} m/s^2 must be finite and leave the gravity positive")]
    GravityOffset(f64),
    /// The normal gravity overflows or underflows, at radii or a GM out of any body's range.
    #[error("the level spheroid's normal gravity is not finite and positive")]
    Gravity,
}

impl LevelSpheroid {
    /// The level spheroid of radii `equatorial_radius_m` (a) and `polar_radius_m` (c), GM
    /// `gm_m3_s2` and spin `omega_rad_s` (only ω² enters; Design note 17's `ω_fig`), with
    /// `gravity_offset_m_s2` added to its normal gravity at every latitude.
    ///
    /// # Errors
    ///
    /// [`BuildLevelSpheroidError::Radii`] unless a and c are finite with 0 < c ≤ a;
    /// [`GravitationalParameter`](BuildLevelSpheroidError::GravitationalParameter) unless GM is
    /// finite and positive; [`Spin`](BuildLevelSpheroidError::Spin) for an ω that is not finite;
    /// [`PastBreakup`](BuildLevelSpheroidError::PastBreakup) where the equator's normal gravity is
    /// not positive; [`GravityOffset`](BuildLevelSpheroidError::GravityOffset) for an offset that
    /// is not finite or leaves g ≤ 0 at the equator or the poles; and
    /// [`Gravity`](BuildLevelSpheroidError::Gravity) where the normal gravity is not finite.
    pub(crate) fn new(
        equatorial_radius_m: f64,
        polar_radius_m: f64,
        gm_m3_s2: f64,
        omega_rad_s: f64,
        gravity_offset_m_s2: f64,
    ) -> Result<Self, BuildLevelSpheroidError> {
        let (a, c, gm, omega) = (equatorial_radius_m, polar_radius_m, gm_m3_s2, omega_rad_s);
        if !(a.is_finite() && c.is_finite() && c > 0.0 && c <= a) {
            return Err(BuildLevelSpheroidError::Radii {
                equatorial_radius_m: a,
                polar_radius_m: c,
            });
        }
        if !(gm.is_finite() && gm > 0.0) {
            return Err(BuildLevelSpheroidError::GravitationalParameter(gm));
        }
        if !omega.is_finite() {
            return Err(BuildLevelSpheroidError::Spin(omega));
        }
        // The client's `poleAndEquator`, operation for operation.
        let e_prime = ((a - c) * (a + c)).sqrt() / c;
        let m = omega * omega * a * a * c / gm;
        let ratio = q_ratio(e_prime);
        let equatorial = gm / (a * c) * (1.0 - m - m * ratio / 6.0);
        let polar = gm / (a * a) * (1.0 + m * ratio / 3.0);
        if !(equatorial.is_finite() && polar.is_finite() && polar > 0.0) {
            return Err(BuildLevelSpheroidError::Gravity);
        }
        if equatorial <= 0.0 {
            return Err(BuildLevelSpheroidError::PastBreakup(omega));
        }
        let offset = gravity_offset_m_s2;
        let (equator_g, pole_g) = (equatorial + offset, polar + offset);
        if !(offset.is_finite() && equator_g > 0.0 && pole_g > 0.0) {
            return Err(BuildLevelSpheroidError::GravityOffset(offset));
        }
        let b_over_a = c / a;
        // The client's `offsetReferenceGravity`.
        let reference = (equator_g * pole_g).sqrt();
        Ok(Self {
            equatorial_radius_m: a,
            polar_radius_m: c,
            e2: (a - c) * (a + c) / (a * a),
            one_minus_e2: b_over_a * b_over_a,
            equatorial_gravity_m_s2: equatorial,
            polar_gravity_m_s2: polar,
            gravity_offset_m_s2: offset,
            reference_gravity_m_s2: reference,
            inv_a2_per_m2: 1.0 / (a * a),
            scaled_weights_m: (a * equatorial / reference, c * polar / reference),
            scaled_offset: offset / reference,
        })
    }

    /// Somigliana's normal gravity γ at the geodetic latitude of cosine `cos_latitude` and sine
    /// `sin_latitude`, m s⁻², without the offset (module documentation; the client's `somigliana`,
    /// operation for operation).
    #[must_use]
    pub(crate) fn normal_gravity_m_s2(&self, cos_latitude: f64, sin_latitude: f64) -> f64 {
        let (a, c) = (self.equatorial_radius_m, self.polar_radius_m);
        let a_cos2 = a * cos_latitude * cos_latitude;
        let c_sin2 = c * sin_latitude * sin_latitude;
        (a_cos2 * self.equatorial_gravity_m_s2 + c_sin2 * self.polar_gravity_m_s2)
            / (a * a_cos2 + c * c_sin2).sqrt()
    }

    /// s = (γ(φ) + the offset) ÷ `g_ref`, the factor geodetic heights are scaled by (the client's
    /// `gravityRatio`).
    #[must_use]
    pub(crate) fn gravity_ratio(&self, cos_latitude: f64, sin_latitude: f64) -> f64 {
        (self.normal_gravity_m_s2(cos_latitude, sin_latitude) + self.gravity_offset_m_s2)
            / self.reference_gravity_m_s2
    }

    /// The point at geodetic height `height_m` above the datum on the normal of the datum at the
    /// geodetic latitude whose cosine and sine are `up.x` and `up.z`, on the meridian of
    /// longitude 0: ((N + h) cos φ, 0, (N(1 − e²) + h) sin φ) (NIMA TR8350.2 eq. 4-14).
    #[must_use]
    pub(crate) fn point(&self, up: Vec3, height_m: f64) -> Vec3 {
        let (cos, sin) = (up.x, up.z);
        let n = self.equatorial_radius_m / (1.0 - self.e2 * sin * sin).sqrt();
        Vec3::new(
            (n + height_m) * cos,
            0.0,
            (n * self.one_minus_e2 + height_m) * sin,
        )
    }

    /// The geodetic latitude, height and datum normal of `x`, by Vermeille 2002's closed form.
    ///
    /// Vermeille's p = ρ² ÷ a², q = (1 − e²) z² ÷ a², r = (p + q − e⁴) ÷ 6; u, the largest root of
    /// the cubic u²(u − 3r) = e⁴pq ÷ 2 that his t solves; v = √(u² + e⁴q),
    /// w = e²(u + v − q) ÷ 2v, k = √(u + v + w²) − w, D = kρ ÷ (k + e²), and then
    /// h = (k + e² − 1) √(D² + z²) ÷ k and φ = atan2(z, D), whose cosine and sine are D and z over
    /// √(D² + z²). Two forms of the same arithmetic stand in for his, held to a brute-force foot
    /// point by the tests:
    ///
    /// - Outside the evolute (8r³ + e⁴pq > 0) his u = r(1 + t + 1 ÷ t), t = ∛(1 + s + √(s(2 + s))),
    ///   s = e⁴pq ÷ 4r³, is written as r + ½R + 2r² ÷ R with R = ∛((√(e⁴pq) + √(8r³ + e⁴pq))²),
    ///   since {rt, r ÷ t} = {½R, 2r² ÷ R} for either sign of r: Karney 2011's u = r + T + r² ÷ T,
    ///   T = ½R (arXiv:1102.1215, App. B), with no division by r, which vanishes on the axis at the
    ///   evolute's cusp, and no cancellation where r < 0, where Vermeille's own form loses digits.
    /// - Inside it, which only points above the poles of a body flatter than c ÷ a = 1 ÷ √2 reach,
    ///   the cubic has three real roots, and u is the largest, |r|(2 cos(θ ÷ 3) − 1) with
    ///   cos θ = e⁴pq ÷ 4|r|³ − 1 (Viète's trigonometric form), continuous with the first at the
    ///   evolute. Any real root serves, since Karney's quartic in k has a single positive root, so
    ///   every root of the cubic gives the same k (Karney's u = r(1 + 2 cos(ψ ÷ 3)) is the smallest).
    ///
    /// k is taken as (u + v) ÷ (√(u + v + w²) + w) where w > 0, the same value without the
    /// cancellation. Valid at every point on or outside the datum and just below it; not inside the
    /// body's equatorial disc ρ ≤ ae², where Karney's special case applies, which the tracer never
    /// reaches.
    #[must_use]
    pub(crate) fn geodetic(&self, x: Vec3) -> Geodetic {
        let (k, along) = self.vermeille(x);
        let d = along * (x.x * x.x + x.y * x.y).sqrt();
        let length = (d * d + x.z * x.z).sqrt();
        Geodetic {
            cos_latitude: d / length,
            sin_latitude: x.z / length,
            height_m: (k + self.e2 - 1.0) / k * length,
            // (cos φ cos λ, cos φ sin λ, sin φ), with cos λ = x ÷ ρ and cos φ = D ÷ √(D² + z²).
            normal: Vec3::new(x.x * along / length, x.y * along / length, x.z / length),
        }
    }

    /// Vermeille's k of `x`, and k ÷ (k + e²), which takes ρ to D ([`geodetic`](Self::geodetic)).
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "Vermeille 2002's own p, q, r, u, v, w and k, so that the code reads against the paper"
    )]
    fn vermeille(&self, x: Vec3) -> (f64, f64) {
        let e2 = self.e2;
        let e4 = e2 * e2;
        let p = (x.x * x.x + x.y * x.y) * self.inv_a2_per_m2;
        let q = self.one_minus_e2 * x.z * x.z * self.inv_a2_per_m2;
        let r = (p + q - e4) / 6.0;
        let e4pq = e4 * p * q;
        let evolute = 8.0 * r * r * r + e4pq;
        let u = if evolute > 0.0 {
            let sum = e4pq.sqrt() + evolute.sqrt();
            let big_r = math::cbrt(sum * sum);
            r + 0.5 * big_r + 2.0 * r * r / big_r
        } else if r < 0.0 {
            let magnitude = -r;
            let cos_theta =
                (e4pq / (4.0 * magnitude * magnitude * magnitude) - 1.0).clamp(-1.0, 1.0);
            magnitude * (2.0 * math::cos(math::acos(cos_theta) / 3.0) - 1.0)
        } else {
            // r = 0 and e⁴pq = 0: on the axis at the evolute's cusp, z = ±(a² − c²) ÷ c, where u = 0
            // is the root; outside the body only where c ÷ a ≤ 1 ÷ √2.
            0.0
        };
        let v = (u * u + e4 * q).sqrt();
        let w = e2 * (u + v - q) / (2.0 * v);
        let uv = u + v;
        let root = (uv + w * w).sqrt();
        let k = if w > 0.0 { uv / (root + w) } else { root - w };
        (k, k / (k + e2))
    }

    /// The gravity-scaled height h\* = s h of `x`, m (module documentation).
    ///
    /// With cos φ and sin φ as D and z over L = √(D² + z²), h = (k + e² − 1) L ÷ k and
    /// s = (a γₑ D² + c γₚ z²) ÷ (L √(a² D² + c² z²) `g_ref`) + the offset ÷ `g_ref`, so L cancels
    /// from Somigliana's part of their product: the same h\* as [`geodetic`](Self::geodetic)'s
    /// height times [`gravity_ratio`](Self::gravity_ratio), to rounding, for a square root fewer.
    /// The tracer reads every density through it.
    #[must_use]
    pub(crate) fn scaled_height_m(&self, x: Vec3) -> f64 {
        let (k, along) = self.vermeille(x);
        let d2 = along * along * (x.x * x.x + x.y * x.y);
        let z2 = x.z * x.z;
        let (a, c) = (self.equatorial_radius_m, self.polar_radius_m);
        let (weight_e, weight_p) = self.scaled_weights_m;
        let somigliana = (weight_e * d2 + weight_p * z2) / (a * a * d2 + c * c * z2).sqrt();
        (k + self.e2 - 1.0) / k * (somigliana + self.scaled_offset * (d2 + z2).sqrt())
    }
}

/// One quadric boundary: the ellipsoid of revolution (A, A, C), as 1 ÷ A² and 1 ÷ C².
#[derive(Debug, Clone, Copy, PartialEq)]
struct Quadric {
    /// 1 ÷ A².
    inv_a2_per_m2: f64,
    /// 1 ÷ C².
    inv_c2_per_m2: f64,
}

impl Quadric {
    /// The quadric of equatorial radius `equatorial_m` and polar radius `polar_m`.
    #[must_use]
    fn new(equatorial_m: f64, polar_m: f64) -> Self {
        Self {
            inv_a2_per_m2: 1.0 / (equatorial_m * equatorial_m),
            inv_c2_per_m2: 1.0 / (polar_m * polar_m),
        }
    }

    /// (x² + y²) ÷ A² + z² ÷ C², dimensionless: below 1 inside, 1 on it.
    #[must_use]
    fn scaled_square(self, x: Vec3) -> f64 {
        (x.x * x.x + x.y * x.y) * self.inv_a2_per_m2 + x.z * x.z * self.inv_c2_per_m2
    }

    /// [`scaled_square`](Self::scaled_square) − 1: negative inside, zero on it, positive outside.
    #[must_use]
    fn level(self, x: Vec3) -> f64 {
        self.scaled_square(x) - 1.0
    }

    /// Half the derivative of [`level`](Self::level) along the unit vector `d` at `x`, m⁻¹:
    /// positive moving out.
    #[must_use]
    fn slope_per_m(self, x: Vec3, d: Vec3) -> f64 {
        (x.x * d.x + x.y * d.y) * self.inv_a2_per_m2 + x.z * d.z * self.inv_c2_per_m2
    }

    /// The coefficient of t² in [`level`](Self::level) along the unit vector `d`, m⁻².
    #[must_use]
    fn curvature_per_m2(self, d: Vec3) -> f64 {
        (d.x * d.x + d.y * d.y) * self.inv_a2_per_m2 + d.z * d.z * self.inv_c2_per_m2
    }

    /// The distance along `d` from `x`, inside the quadric, to where the ray leaves it, m.
    #[must_use]
    fn far_distance_m(self, x: Vec3, d: Vec3) -> f64 {
        let level = self.level(x);
        let slope = self.slope_per_m(x, d);
        let curvature = self.curvature_per_m2(d);
        let disc = (slope * slope - curvature * level).max(0.0);
        let distance_m = if slope > 0.0 {
            -level / (slope + disc.sqrt())
        } else {
            (-slope + disc.sqrt()) / curvature
        };
        // Not `max`, which may return either zero where the quotient is −0 (a point on the
        // quadric moving out), and so differ between targets.
        if distance_m > 0.0 { distance_m } else { 0.0 }
    }

    /// The distance along `d` from `x`, outside the quadric and moving in, to where the ray
    /// reaches it, m; `None` if it does not.
    #[must_use]
    fn near_distance_m(self, x: Vec3, d: Vec3) -> Option<f64> {
        let slope = self.slope_per_m(x, d);
        if slope >= 0.0 {
            return None;
        }
        let level = self.level(x);
        let disc = slope * slope - self.curvature_per_m2(d) * level;
        (disc >= 0.0).then(|| {
            let distance_m = level / (-slope + disc.sqrt());
            if distance_m > 0.0 { distance_m } else { 0.0 }
        })
    }

    /// `x` moved along the line from the centre onto the quadric.
    #[must_use]
    fn snap(self, x: Vec3) -> Vec3 {
        x * (1.0 / self.scaled_square(x).sqrt())
    }
}

/// The shells over a level spheroid: quadrics through the gravity-scaled heights of the
/// boundaries at the equator and the poles (module documentation).
///
/// Boundary k at gravity-scaled height Hₖ is the ellipsoid of radii a + Hₖ ÷ sₑ and c + Hₖ ÷ sₚ:
/// its geodetic height is Hₖ ÷ sₑ at the equator and Hₖ ÷ sₚ at the poles, where
/// sₑ and sₚ are s there, so h\* = Hₖ at both; boundary 0 is the datum itself. Each shell carries
/// bounds of the h\* it can hold, for the majorant of the delta tracking (Design note 17: "the
/// density at the lowest gravity-scaled height the path can reach"). A point of shell k, between
/// boundaries k and k + 1, lies on the datum's normal at its own latitude φ between where that
/// normal crosses the two quadrics, hₖ(φ) and hₖ₊₁(φ), since every quadric is convex and holds
/// the datum; so its h\* lies in [min s hₖ, max s hₖ₊₁] over φ. The two are found over
/// [`BOUND_SCAN_STEPS`] latitudes and widened by the largest step between neighbouring latitudes:
/// an extremum of the smooth f = s hₖ(φ) stands at most ½ max |f″| Δφ² past the nearer sample,
/// 300 times less than that step on Saturn (computed).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpheroidShells {
    figure: LevelSpheroid,
    /// Boundary 0 the datum, the last the top.
    boundaries: Vec<Quadric>,
    /// Per shell, bounds of the h\* it can hold, m.
    bounds_m: Vec<(f64, f64)>,
}

impl SpheroidShells {
    /// The shells over `figure` from the datum to the gravity-scaled height `top_height_m`, split
    /// at every height of `heights_m` strictly between the two (as [`boundary_heights`] merges
    /// them).
    #[must_use]
    pub(crate) fn new(
        figure: LevelSpheroid,
        top_height_m: f64,
        heights_m: impl IntoIterator<Item = f64>,
    ) -> Self {
        let s_equator = figure.gravity_ratio(1.0, 0.0);
        let s_pole = figure.gravity_ratio(0.0, 1.0);
        let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
        let boundaries: Vec<Quadric> = boundary_heights(top_height_m, heights_m)
            .into_iter()
            .map(|h| Quadric::new(a + h / s_equator, c + h / s_pole))
            .collect();
        // Per boundary, the least and greatest s hₖ(φ) and the largest step between neighbours.
        let count = boundaries.len();
        let mut low = vec![f64::INFINITY; count];
        let mut high = vec![f64::NEG_INFINITY; count];
        let mut step = vec![0.0_f64; count];
        let mut previous = vec![f64::NAN; count];
        for i in 0..=BOUND_SCAN_STEPS {
            let latitude = FRAC_PI_2 * f64::from(i) / f64::from(BOUND_SCAN_STEPS);
            let up = LocalFrame::at_latitude(latitude).up;
            let foot = figure.point(up, 0.0);
            let s = figure.gravity_ratio(up.x, up.z);
            for k in 1..count {
                let value = s * boundaries[k].far_distance_m(foot, up);
                low[k] = low[k].min(value);
                high[k] = high[k].max(value);
                if i > 0 {
                    step[k] = step[k].max((value - previous[k]).abs());
                }
                previous[k] = value;
            }
        }
        let bounds_m = (0..count - 1)
            .map(|k| {
                let floor = if k == 0 { 0.0 } else { low[k] - step[k] };
                (floor, high[k + 1] + step[k + 1])
            })
            .collect();
        Self {
            figure,
            boundaries,
            bounds_m,
        }
    }

    /// How many shells there are.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.boundaries.len() - 1
    }

    /// Bounds of the gravity-scaled height in shell `shell`, m.
    #[must_use]
    pub(crate) fn scaled_height_bounds_m(&self, shell: usize) -> (f64, f64) {
        self.bounds_m[shell]
    }

    #[must_use]
    fn top(&self) -> Quadric {
        self.boundaries[self.boundaries.len() - 1]
    }

    /// The shell a ray at `point` moving along `dir` is in, as [`SphereShells::locate`] has it.
    ///
    /// [`SphereShells::locate`]: super::geometry::SphereShells::locate
    #[must_use]
    pub(crate) fn locate(&self, point: Vec3, dir: Vec3) -> Option<usize> {
        let top = self.top();
        let level = top.level(point);
        if level > 0.0 || (level >= 0.0 && top.slope_per_m(point, dir) >= 0.0) {
            return None;
        }
        // The boundaries the point is on or outside, a prefix since each quadric holds the last.
        let above = self
            .boundaries
            .partition_point(|boundary| boundary.level(point) >= 0.0);
        let mut shell = above.saturating_sub(1).min(self.len() - 1);
        let floor = self.boundaries[shell];
        if shell > 0 && floor.level(point) <= 0.0 && floor.slope_per_m(point, dir) < 0.0 {
            shell -= 1;
        }
        Some(shell)
    }

    /// The distance along the unit vector `dir` at which a ray from `point`, outside the
    /// atmosphere, enters it through the top, m; `None` if it misses.
    #[must_use]
    pub(crate) fn entry_distance_m(&self, point: Vec3, dir: Vec3) -> Option<f64> {
        self.top().near_distance_m(point, dir)
    }

    /// Where a ray from `point` in shell `shell` along the unit vector `dir` leaves it: through
    /// the floor if it reaches it, otherwise through the ceiling.
    #[must_use]
    pub(crate) fn exit(&self, point: Vec3, dir: Vec3, shell: usize) -> Exit {
        if let Some(distance_m) = self.boundaries[shell].near_distance_m(point, dir) {
            return Exit {
                distance_m,
                next: if shell == 0 {
                    Next::Ground
                } else {
                    Next::Shell(shell - 1)
                },
            };
        }
        Exit {
            distance_m: self.boundaries[shell + 1].far_distance_m(point, dir),
            next: if shell + 1 == self.len() {
                Next::Space
            } else {
                Next::Shell(shell + 1)
            },
        }
    }

    /// The point `distance_m` along `dir` from `point`, set onto the quadric of the boundary that
    /// `next` says it crossed from shell `shell`.
    #[must_use]
    pub(crate) fn cross(
        &self,
        point: Vec3,
        dir: Vec3,
        distance_m: f64,
        shell: usize,
        next: Next,
    ) -> Vec3 {
        let boundary = match next {
            Next::Ground => 0,
            Next::Space => self.boundaries.len() - 1,
            Next::Shell(j) if j < shell => shell,
            Next::Shell(_) => shell + 1,
        };
        self.boundaries[boundary].snap(point + dir * distance_m)
    }

    /// The gravity-scaled height of `x`, m.
    #[must_use]
    pub(crate) fn scaled_height_m(&self, x: Vec3) -> f64 {
        self.figure.scaled_height_m(x)
    }

    /// The local vertical at `x`: the datum's outward normal at its foot.
    #[must_use]
    pub(crate) fn vertical(&self, x: Vec3) -> Vec3 {
        self.figure.geodetic(x).normal
    }

    /// The point at geodetic height `height_m` above the datum under `frame`.
    #[must_use]
    pub(crate) fn point(&self, frame: &LocalFrame, height_m: f64) -> Vec3 {
        self.figure.point(frame.up, height_m)
    }

    /// The point where the datum's normal under `frame` meets the top.
    #[must_use]
    pub(crate) fn top_point(&self, frame: &LocalFrame) -> Vec3 {
        let foot = self.figure.point(frame.up, 0.0);
        let top = self.top();
        top.snap(foot + frame.up * top.far_distance_m(foot, frame.up))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use core::f64::consts::PI;

    use super::*;

    const HOUR_S: f64 = 3_600.0;

    /// WGS 84's four defining parameters (NIMA TR8350.2, Third Edition, Amendment 1, 2000, Table
    /// 3.1): a, 1 ÷ f, GM with the atmosphere, and ω.
    pub(crate) const WGS84: (f64, f64, f64, f64) = (
        6_378_137.0,
        6_378_137.0 * (1.0 - 1.0 / 298.257_223_563),
        3.986_004_418e14,
        7.292_115e-5,
    );

    /// Saturn at 1 bar as the client's `oblate.test.ts` gives it: Archinal et al. 2018's radii,
    /// Jacobson et al. 2006's GM to five figures and a 10.656 h spin.
    pub(crate) const SATURN: (f64, f64, f64, f64) =
        (60_268e3, 54_364e3, 3.793_1e16, 2.0 * PI / (10.656 * HOUR_S));

    /// Jupiter at 1 bar as the client's `oblate.test.ts` gives it: Archinal et al. 2018's radii,
    /// IAU 2015 Resolution B3's nominal GM and System III's 870.536° d⁻¹.
    const JUPITER: (f64, f64, f64, f64) = (
        71_492e3,
        66_854e3,
        1.266_865_3e17,
        870.536 * PI / 180.0 / 86_400.0,
    );

    /// The level spheroid of (a, c, GM, ω), with nothing added to its gravity.
    pub(crate) fn figure((a, c, gm, omega): (f64, f64, f64, f64)) -> LevelSpheroid {
        LevelSpheroid::new(a, c, gm, omega, 0.0).unwrap()
    }

    fn gravity_at(f: &LevelSpheroid, latitude_deg: f64) -> f64 {
        let (sin, cos) = math::sin_cos(latitude_deg.to_radians());
        f.normal_gravity_m_s2(cos, sin)
    }

    fn relative(value: f64, expected: f64) -> f64 {
        (value / expected - 1.0).abs()
    }

    #[test]
    fn atmosphere_normal_gravity_reproduces_wgs84s() {
        // NIMA TR8350.2, Table 3.4: γ_e and γ_p, m s⁻².
        let wgs84 = figure(WGS84);
        assert!(relative(gravity_at(&wgs84, 0.0), 9.780_325_335_9) < 1e-9);
        assert!(relative(gravity_at(&wgs84, 90.0), 9.832_184_937_8) < 1e-9);
    }

    #[test]
    fn atmosphere_normal_gravity_agrees_with_the_clients() {
        // g_ref, then γ at 0°, 30°, 45°, 60° and 90°, m s⁻², as the client's `normalGravity` and
        // `referenceGravity` (`view/atmosphere/oblate.ts`, R08.T3.d at 8719a966) print them for
        // the same three bodies, to 17 figures.
        let printed = [
            (
                WGS84,
                9.806_220_854_900_12,
                [
                    9.780_325_335_903_893,
                    9.793_247_269_219_322,
                    9.806_197_769_377_379,
                    9.819_176_953_118_639,
                    9.832_184_937_863_401,
                ],
            ),
            (
                SATURN,
                10.452_505_926_092_714,
                [
                    9.076_628_614_761_937,
                    9.751_752_453_062_3,
                    10.466_679_804_152_758,
                    11.226_419_722_563_385,
                    12.036_945_078_629_161,
                ],
            ),
            (
                JUPITER,
                24.976_288_778_892_89,
                [
                    23.124_120_157_590_784,
                    24.029_861_628_154_745,
                    24.971_856_153_158_49,
                    25.953_065_040_600_848,
                    26.976_810_227_388_29,
                ],
            ),
        ];
        for (body, g_ref, gravities) in printed {
            let f = figure(body);
            assert!(
                relative(f.reference_gravity_m_s2, g_ref) < 1e-12,
                "{body:?}: {} against {g_ref}",
                f.reference_gravity_m_s2
            );
            for (latitude, expected) in [0.0, 30.0, 45.0, 60.0, 90.0].into_iter().zip(gravities) {
                let gravity = gravity_at(&f, latitude);
                assert!(
                    relative(gravity, expected) < 1e-12,
                    "{body:?} at {latitude}°: {gravity} against {expected}"
                );
            }
        }
    }

    #[test]
    fn atmosphere_gravity_ratio_agrees_with_the_clients_under_every_figure_law() {
        // One figure (a 99,000 km, c 97,000 km, 1.31 × 10²⁷ kg, a 3.5247 d spin, C ÷ Ma² 0.25)
        // under three laws, as the client's `bodyGravity` and `gravityRatio` (`oblate.ts` at
        // 533bbfbc) print them to 17 figures: its spheroid's GM and ω_fig, its added gravity, its
        // g_ref and s at 0°, 30°, 45°, 60° and 90°. `rotational_and_tidal` takes √2.5 ω and adds
        // ω²R; `capped` takes the Darwin–Radau spin of the drawn f; `rotational` the true ω.
        let gm = 8.743_333e16;
        let printed = [
            (
                "rotational_and_tidal",
                3.262_225_451_350_502_5e-5,
                0.041_857_052_471_940_874,
                9.027_637_104_535_104,
                [
                    0.995_574_367_534_568_1,
                    0.997_719_946_727_827_5,
                    0.999_912_762_391_520_4,
                    1.002_154_103_177_734_8,
                    1.004_445_305_754_899_7,
                ],
            ),
            (
                "capped",
                5.005_887_502_987_018e-5,
                0.0,
                8.946_673_735_461_207,
                [
                    0.975_834_919_477_612_6,
                    0.987_842_606_195_474,
                    0.999_997_510_160_929_6,
                    1.012_303_230_003_175_8,
                    1.024_763_492_308_026_7,
                ],
            ),
            (
                "rotational",
                2.063_212_533_447_680_4e-5,
                0.0,
                9.001_957_759_785_97,
                [
                    1.004_360_055_534_770_7,
                    1.002_179_151_684_419_4,
                    1.000_001_797_193_540_9,
                    0.997_828_272_706_634_5,
                    0.995_658_872_024_286_8,
                ],
            ),
        ];
        for (law, omega_fig, offset, g_ref, ratios) in printed {
            let f = LevelSpheroid::new(9.9e7, 9.7e7, gm, omega_fig, offset).unwrap();
            assert!(
                relative(f.reference_gravity_m_s2, g_ref) < 1e-12,
                "{law}: {} against {g_ref}",
                f.reference_gravity_m_s2
            );
            for (latitude, expected) in [0.0_f64, 30.0, 45.0, 60.0, 90.0].into_iter().zip(ratios) {
                let (sin, cos) = math::sin_cos(latitude.to_radians());
                let ratio = f.gravity_ratio(cos, sin);
                assert!(
                    relative(ratio, expected) < 1e-12,
                    "{law} at {latitude}°: {ratio} against {expected}"
                );
            }
        }
    }

    #[test]
    fn atmosphere_normal_gravity_of_a_still_sphere_is_gm_over_r_squared() {
        let f = figure((6_371_000.0, 6_371_000.0, 3.986_004_418e14, 0.0));
        for latitude in [0.0, 23.5, 45.0, 89.0, 90.0, -60.0] {
            let (sin, cos) = math::sin_cos(f64::to_radians(latitude));
            let expected = 3.986_004_418e14 / (6_371_000.0 * 6_371_000.0);
            let gravity = f.normal_gravity_m_s2(cos, sin);
            assert!(
                relative(gravity, expected) < 1e-15,
                "{latitude}°: {gravity}"
            );
            let ratio = f.gravity_ratio(cos, sin);
            assert!((ratio - 1.0).abs() < 1e-15, "{latitude}°: {ratio}");
        }
    }

    #[test]
    fn atmosphere_normal_gravity_joins_its_series_and_closed_form() {
        // e′ = 0.25 either side of the join, as the client's test has it.
        let a = 1e7;
        for latitude in [0.0, 90.0] {
            let at = |e_prime: f64| {
                let c = a / (1.0 + e_prime * e_prime).sqrt();
                gravity_at(&figure((a, c, 1e17, 1e-4)), latitude)
            };
            let (below, above) = (at(0.25 - 1e-13), at(0.25 + 1e-13));
            assert!(
                relative(below, above) < 1e-12,
                "{latitude}°: {below} {above}"
            );
        }
        // And the series is the closed form below the join too, at e′ = 0.2 and 0.1, where the
        // closed form's cancellation leaves some 3 × 10⁻¹⁴ and 2 × 10⁻¹² (the science check's).
        for (e_prime, tolerance) in [(0.2_f64, 1e-12), (0.1, 1e-10)] {
            let atan = math::atan(e_prime);
            let e2 = e_prime * e_prime;
            let q0 = 0.5 * ((1.0 + 3.0 / e2) * atan - 3.0 / e_prime);
            let q0_prime = 3.0 * (1.0 + 1.0 / e2) * (1.0 - atan / e_prime) - 1.0;
            let closed = e_prime * q0_prime / q0;
            assert!(
                relative(q_ratio(e_prime), closed) < tolerance,
                "{e_prime}: {} against {closed}",
                q_ratio(e_prime)
            );
        }
        // The sphere's limit, 3.
        assert!((q_ratio(0.0) - 3.0).abs() < 1e-15);
    }

    #[test]
    fn atmosphere_level_spheroids_are_refused_out_of_range() {
        use BuildLevelSpheroidError as E;
        let (a, c, gm, omega) = SATURN;
        let build = |a, c, gm, omega, offset| LevelSpheroid::new(a, c, gm, omega, offset).err();
        let radii = |a, c| {
            Some(E::Radii {
                equatorial_radius_m: a,
                polar_radius_m: c,
            })
        };
        assert_eq!(build(a, a * 1.001, gm, omega, 0.0), radii(a, a * 1.001));
        assert_eq!(build(a, 0.0, gm, omega, 0.0), radii(a, 0.0));
        assert_eq!(
            build(f64::INFINITY, c, gm, omega, 0.0),
            radii(f64::INFINITY, c)
        );
        assert_eq!(
            build(a, c, 0.0, omega, 0.0),
            Some(E::GravitationalParameter(0.0))
        );
        assert!(matches!(build(a, c, gm, f64::NAN, 0.0), Some(E::Spin(w)) if w.is_nan()));
        // Ten times Saturn's spin is past breakup: γₑ < 0.
        assert_eq!(
            build(a, c, gm, 10.0 * omega, 0.0),
            Some(E::PastBreakup(10.0 * omega))
        );
        // An offset that takes the equator's gravity below zero, or is not finite.
        assert_eq!(build(a, c, gm, omega, -9.1), Some(E::GravityOffset(-9.1)));
        assert_eq!(
            build(a, c, gm, omega, f64::INFINITY),
            Some(E::GravityOffset(f64::INFINITY))
        );
        // Radii no body has: the gravity underflows.
        assert_eq!(build(1e200, 1e200, gm, 0.0, 0.0), Some(E::Gravity));
        assert_eq!(build(a, c, gm, -omega, 0.0), None);
        assert_eq!(build(a, c, gm, omega, 0.5), None);
    }

    /// Whether `x` is inside the evolute of the figure (a, c): 8r³ + e⁴pq ≤ 0 in Vermeille's
    /// variables.
    #[expect(
        clippy::many_single_char_names,
        reason = "Vermeille 2002's own p, q and r, as in `LevelSpheroid::vermeille`"
    )]
    fn inside_evolute(a: f64, c: f64, x: Vec3) -> bool {
        let e2 = (a - c) * (a + c) / (a * a);
        let p = (x.x * x.x + x.y * x.y) / (a * a);
        let q = (c / a) * (c / a) * x.z * x.z / (a * a);
        let r = (p + q - e2 * e2) / 6.0;
        8.0 * r * r * r + e2 * e2 * p * q <= 0.0
    }

    /// The geodetic latitude and height of the point (ρ, z) on the figure (a, c) by search, with
    /// no closed form: every root of ρ sin φ − z cos φ − e² N sin φ cos φ, the condition that the
    /// datum's normal at φ passes through the point, by a scan and bisection, the nearest kept,
    /// with h = ρ cos φ + z sin φ − a √(1 − e² sin²φ) along that normal.
    fn foot_point_by_search(a: f64, c: f64, rho: f64, z: f64) -> (f64, f64) {
        let e2 = (a - c) * (a + c) / (a * a);
        let condition = |phi: f64| {
            let (sin, cos) = math::sin_cos(phi);
            let n = a / (1.0 - e2 * sin * sin).sqrt();
            rho * sin - z * cos - e2 * n * sin * cos
        };
        let height = |phi: f64| {
            let (sin, cos) = math::sin_cos(phi);
            rho * cos + z * sin - a * (1.0 - e2 * sin * sin).sqrt()
        };
        let steps = 4000;
        let at = |i: i32| -FRAC_PI_2 + PI * f64::from(i) / f64::from(steps);
        let mut best: Option<(f64, f64)> = None;
        for i in 0..steps {
            let (mut low, mut high) = (at(i), at(i + 1));
            if condition(low) * condition(high) > 0.0 {
                continue;
            }
            for _ in 0..200 {
                let mid = f64::midpoint(low, high);
                if condition(low) * condition(mid) <= 0.0 {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            let phi = f64::midpoint(low, high);
            let h = height(phi);
            if best.is_none_or(|(_, b)| h.abs() < b.abs()) {
                best = Some((phi, h));
            }
        }
        best.unwrap()
    }

    #[test]
    fn atmosphere_geodetic_coordinates_round_trip_on_any_oblate_spheroid() {
        let a = 1e7;
        for c_over_a in [
            1.0,
            WGS84.1 / WGS84.0,
            SATURN.1 / SATURN.0,
            0.8,
            0.6,
            0.4,
            0.2,
        ] {
            let c = a * c_over_a;
            let body = figure((a, c, 1e17, 0.0));
            let mut inside = 0;
            for i in -30..=30 {
                let up = LocalFrame::at_latitude(f64::from(3 * i).to_radians()).up;
                for height in [0.0, 1e-7, 1e-4, 0.01, 0.1, 1.0, 3.0].map(|h| h * a) {
                    let x = body.point(up, height);
                    inside += usize::from(inside_evolute(a, c, x));
                    let g = body.geodetic(x);
                    let context = format!("c/a {c_over_a}, {}°, h {height}", 3 * i);
                    assert!((g.cos_latitude - up.x).abs() < 1e-14, "{context}: {g:?}");
                    assert!((g.sin_latitude - up.z).abs() < 1e-14, "{context}: {g:?}");
                    assert!((g.height_m - height).abs() < 1e-13 * a, "{context}: {g:?}");
                    assert!((g.normal - up).length() < 1e-14, "{context}: {g:?}");
                }
            }
            // Past c ÷ a = 1 ÷ √2 the evolute pierces the surface above the poles, and the
            // trigonometric branch is taken there.
            assert_eq!(inside > 0, c_over_a < 0.7, "c/a {c_over_a}: {inside}");
        }
        // Off the meridian of longitude 0, the normal turns with the point.
        let saturn = figure(SATURN);
        let up = LocalFrame::at_latitude(0.6).up;
        let x = saturn.point(up, 1e5);
        let (sin, cos) = math::sin_cos(0.7);
        let turned = Vec3::new(x.x * cos, x.x * sin, x.z);
        let g = saturn.geodetic(turned);
        assert!((g.normal - Vec3::new(up.x * cos, up.x * sin, up.z)).length() < 1e-14);
        assert!((g.height_m - 1e5).abs() < 1e-6);
    }

    #[test]
    fn atmosphere_geodetic_coordinates_match_a_search_for_the_foot_point() {
        // A grid of points on and outside the datum, for Saturn and for c ÷ a = 0.3, whose
        // evolute reaches 10c up the axis, past the grid; off the axis, where the search's scan cannot
        // see the double root at the pole (the round trip covers the axis).
        for (a, c) in [(SATURN.0, SATURN.1), (1e7, 3e6)] {
            let f = figure((a, c, 1e17, 0.0));
            for i in 1..=12 {
                for j in 0..=12 {
                    let (rho, z) = (a * 0.2 * f64::from(i), c * 0.2 * f64::from(j));
                    if (rho / a) * (rho / a) + (z / c) * (z / c) < 1.0 {
                        continue;
                    }
                    let g = f.geodetic(Vec3::new(rho, 0.0, z));
                    let (latitude, height) = foot_point_by_search(a, c, rho, z);
                    let (sin, cos) = math::sin_cos(latitude);
                    assert!(
                        (g.cos_latitude - cos).abs() < 1e-12
                            && (g.sin_latitude - sin).abs() < 1e-12,
                        "({rho}, {z}): {g:?} against {latitude}"
                    );
                    assert!(
                        (g.height_m - height).abs() < 1e-9 * a,
                        "({rho}, {z}): {} against {height}",
                        g.height_m
                    );
                }
            }
        }
    }

    #[test]
    fn atmosphere_scaled_height_is_the_geodetic_height_times_the_gravity_ratio() {
        // Three rotational figures, and the tidal one of the figure-law test, with its ω²R added.
        let bodies = [SATURN, WGS84, (1e7, 3e6, 1e17, 1e-4)]
            .map(figure)
            .into_iter()
            .chain([LevelSpheroid::new(
                9.9e7,
                9.7e7,
                8.743_333e16,
                3.262_225_451_350_502_5e-5,
                0.041_857_052_471_940_874,
            )
            .unwrap()]);
        for f in bodies {
            for i in -18..=18 {
                let up = LocalFrame::at_latitude(f64::from(5 * i).to_radians()).up;
                for height in [0.0, 10.0, 4.7e4, 1.9e6, 4e7] {
                    let x = f.point(up, height);
                    let g = f.geodetic(x);
                    let expected = g.height_m * f.gravity_ratio(g.cos_latitude, g.sin_latitude);
                    let scaled = f.scaled_height_m(x);
                    assert!(
                        (scaled - expected).abs()
                            <= 1e-13 * expected.abs() + 1e-15 * f.equatorial_radius_m,
                        "{}°, {height} m: {scaled} against {expected}",
                        5 * i
                    );
                }
            }
        }
    }

    /// Saturn's shells for a 47 km scale height: a boundary at every scale height to 36 of them,
    /// and the top at 40, all gravity-scaled.
    fn saturn_shells() -> (SpheroidShells, Vec<f64>) {
        let (top, splits) = (40.0 * 47_000.0, (1..=36).map(|k| f64::from(k) * 47_000.0));
        let heights = boundary_heights(top, splits.clone());
        (SpheroidShells::new(figure(SATURN), top, splits), heights)
    }

    #[test]
    fn atmosphere_spheroid_shells_meet_their_heights_at_the_equator_and_the_poles() {
        let (shells, heights) = saturn_shells();
        let f = shells.figure;
        let g_ref = f.reference_gravity_m_s2;
        for (k, &height) in heights.iter().enumerate() {
            for (latitude, gravity) in [
                (0.0, f.equatorial_gravity_m_s2),
                (FRAC_PI_2, f.polar_gravity_m_s2),
                (-FRAC_PI_2, f.polar_gravity_m_s2),
            ] {
                let up = LocalFrame::at_latitude(latitude).up;
                let x = f.point(up, height * g_ref / gravity);
                assert!(
                    shells.boundaries[k].level(x).abs() < 1e-14,
                    "{k} at {latitude}"
                );
                assert!(
                    (shells.scaled_height_m(x) - height).abs() < 1e-9 * height + 1e-6,
                    "{k} at {latitude}: {}",
                    shells.scaled_height_m(x)
                );
            }
        }
        assert_eq!(shells.len(), heights.len() - 1);
    }

    #[test]
    fn atmosphere_spheroid_shells_bound_the_scaled_height_they_hold() {
        let (shells, heights) = saturn_shells();
        let f = shells.figure;
        let mut seen = vec![0_u32; shells.len()];
        for i in -180..=180 {
            let up = LocalFrame::at_latitude(f64::from(i).to_radians() * 0.5).up;
            for j in 0..2_200 {
                let x = f.point(up, f64::from(j) * 1_000.0);
                let Some(shell) = shells.locate(x, up) else {
                    continue;
                };
                seen[shell] += 1;
                let scaled = shells.scaled_height_m(x);
                let (low, high) = shells.scaled_height_bounds_m(shell);
                // Within rounding: some 10⁻⁸ m on the ground, where a profile reads a negative
                // height as the ground's.
                assert!(
                    (low - 1e-6..=high + 1e-6).contains(&scaled),
                    "shell {shell} at {}°, {j} km: {scaled} outside [{low}, {high}]",
                    0.5 * f64::from(i)
                );
            }
        }
        for (shell, &count) in seen.iter().enumerate() {
            assert!(count > 0, "shell {shell} unvisited");
            // The bounds stand no further out than the quadrics' 1.7% departure, and a little.
            let (low, high) = shells.scaled_height_bounds_m(shell);
            let (floor, ceiling) = (heights[shell], heights[shell + 1]);
            assert!(low <= floor && high >= ceiling, "{shell}: [{low}, {high}]");
            assert!(
                floor - low <= 0.02 * floor && high - ceiling <= 0.02 * ceiling,
                "{shell}: [{low}, {high}] about [{floor}, {ceiling}]"
            );
        }
    }

    /// Walks a ray from `point` in `shell` to the ground or space, returning the distance, how it
    /// ended and the crossings it took.
    fn walk(
        shells: &SpheroidShells,
        mut point: Vec3,
        dir: Vec3,
        mut shell: usize,
    ) -> (f64, Next, usize) {
        let mut total = 0.0;
        let mut crossings = 0;
        loop {
            let exit = shells.exit(point, dir, shell);
            total += exit.distance_m;
            crossings += 1;
            assert!(crossings < 10_000, "the walk does not end");
            match exit.next {
                Next::Shell(j) => {
                    point = shells.cross(point, dir, exit.distance_m, shell, exit.next);
                    shell = j;
                }
                Next::Ground | Next::Space => return (total, exit.next, crossings),
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_shell_walks_cover_the_chord() {
        let (shells, _) = saturn_shells();
        let (datum, top) = (shells.boundaries[0], shells.top());
        for (latitude, height) in [
            (0.0, 1_234.0),
            (0.8, 60_000.0),
            (FRAC_PI_2, 5_000.0),
            (-1.2, 3e5),
        ] {
            let frame = LocalFrame::at_latitude(latitude);
            let start = shells.point(&frame, height);
            for (zenith, azimuth) in [(0.3, 1.0), (1.5, 0.0), (1.5, 1.6), (2.4, 4.0), (3.0, 2.0)] {
                let dir = frame.direction(zenith, azimuth);
                let shell = shells.locate(start, dir).unwrap();
                let (distance, end, _) = walk(&shells, start, dir, shell);
                let expected = match end {
                    Next::Ground => datum.near_distance_m(start, dir).unwrap(),
                    Next::Space => top.far_distance_m(start, dir),
                    Next::Shell(_) => unreachable!(),
                };
                assert_eq!(
                    end == Next::Ground,
                    datum.near_distance_m(start, dir).is_some()
                );
                assert!(
                    (distance - expected).abs() < 1e-6,
                    "{latitude} {height} ({zenith}, {azimuth}): {distance} {expected}"
                );
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_rays_grazing_a_boundary_pass_it() {
        let (shells, heights) = saturn_shells();
        let f = shells.figure;
        // From the 10th boundary at the equator, where its quadric's normal is the vertical:
        // horizontal, north and east, and a hair below and above.
        let frame = LocalFrame::at_latitude(0.0);
        let start = f.point(
            frame.up,
            heights[10] * f.reference_gravity_m_s2 / f.equatorial_gravity_m_s2,
        );
        for heading in [frame.north, frame.east] {
            for dip in [0.0, 1e-12, -1e-12, 1e-9] {
                let dir = (heading + frame.up * dip).normalised();
                let shell = shells.locate(start, dir).unwrap();
                let (distance, end, crossings) = walk(&shells, start, dir, shell);
                assert_eq!(end, Next::Space, "{dip}");
                let expected = shells.top().far_distance_m(start, dir);
                assert!(
                    (distance - expected).abs() < 1e-3,
                    "{dip}: {distance} {expected}"
                );
                assert!(crossings < 70, "{dip}: {crossings}");
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_rays_enter_from_orbit_and_locate_on_boundaries() {
        let (shells, heights) = saturn_shells();
        let f = shells.figure;
        let pole = LocalFrame::at_latitude(FRAC_PI_2);
        let top_at_pole =
            heights[heights.len() - 1] * f.reference_gravity_m_s2 / f.polar_gravity_m_s2;
        let orbit = shells.point(&pole, 1e7);
        let down = -pole.up;
        let entry = shells.entry_distance_m(orbit, down).unwrap();
        assert!((entry - (1e7 - top_at_pole)).abs() < 1e-6, "{entry}");
        assert_eq!(shells.entry_distance_m(orbit, pole.up), None);
        assert_eq!(shells.entry_distance_m(orbit, pole.north), None);
        let on_top = shells.top_point(&pole);
        assert!((on_top - shells.point(&pole, top_at_pole)).length() < 1e-6);
        assert_eq!(shells.locate(on_top, down), Some(shells.len() - 1));
        assert_eq!(shells.locate(on_top, pole.up), None);
        let equator = LocalFrame::at_latitude(0.0);
        let on_fifth = f.point(
            equator.up,
            heights[5] * f.reference_gravity_m_s2 / f.equatorial_gravity_m_s2,
        );
        assert_eq!(shells.locate(on_fifth, -equator.up), Some(4));
        assert_eq!(shells.locate(on_fifth, equator.up), Some(5));
        let ground = shells.point(&equator, 0.0);
        assert_eq!(shells.locate(ground, equator.up), Some(0));
        assert_eq!(shells.locate(ground, -equator.up), Some(0));
        let exit = shells.exit(ground, -equator.up, 0);
        assert_eq!(exit.next, Next::Ground);
        assert!(exit.distance_m.abs() < 1e-6, "{exit:?}");
        // The vertical at the ground is the datum's normal, the gradient of its quadric.
        for latitude in [0.0, 0.4, 1.1, FRAC_PI_2] {
            let frame = LocalFrame::at_latitude(latitude);
            let x = shells.point(&frame, 0.0);
            let gradient = Vec3::new(
                x.x / (f.equatorial_radius_m * f.equatorial_radius_m),
                0.0,
                x.z / (f.polar_radius_m * f.polar_radius_m),
            )
            .normalised();
            assert!(
                (shells.vertical(x) - gradient).length() < 1e-14,
                "{latitude}"
            );
            assert!(
                (shells.vertical(x) - frame.up).length() < 1e-14,
                "{latitude}"
            );
        }
    }
}
