//! The discs' vertical profiles: the vertical Jeans equation's solution in the galaxy's potential,
//! tabulated once per galaxy (brainstorm, "Fields" and "Orbits and time"; plan 02, Design note 9).
//!
//! Every disc is exponential in radius and cored in height. A disc of stars whose vertical
//! dispersion is `σ(z) = σ₀ s(|z|)`, `s(z) = 1 + γ min(z, z_γ)`, in the vertical force `K_z(R_ref,
//! z)` of the [`MassModel`](crate::galaxy::potential::MassModel) at a reference radius, satisfies
//! the vertical Jeans equation `d(n σ²) ÷ dz = −n K_z`, whose solution is
//!
//! `n(z) ÷ n(0) = (σ₀ ÷ σ(z))² exp(−∫₀^|z| K_z ÷ σ² dz′) = exp(−E(|z|))`, with
//!
//! `E(z) = 2 ln s(z) + (1 ÷ σ₀²) ∫₀^z K_z(z′) ÷ s(z′)² dz′`.
//!
//! The dispersion rises with height as far as `z_γ` and stays level above it: the heating law's
//! rise is measured only so far ([`sub_discs`](super::sub_discs)), and carried on to the cube's
//! edge it would leave every disc a tail falling as `z⁻²` that outnumbers the halo.
//!
//! `K_z` vanishes in the plane, so the profile is all but flat there, and it is close to
//! exponential only well above the disc, as measured profiles are (Bovy 2017, MNRAS 470, 1360:
//! every population's is cored at the mid-plane). Its one slope at the plane, `−2γ`, comes from
//! the dispersion's rise with `|z|`: an e-fold in 2.5 kpc at Sharma et al.'s γ, against an
//! exponential disc's e-fold in one scale height. A disc's height is its effective height, `h = Σ ÷ 2ρ₀ = ∫₀^∞ n ÷
//! n(0) dz`, and a drawn height is met by choosing `σ₀`, not by stretching the profile
//! ([`JeansIntegral::solve`]). The shape is solved once at the reference radius and holds at
//! every radius, so a disc's density separates into `exp(−R ÷ L)` times the profile.
//!
//! # The table
//!
//! `E` is tabulated at 705 knots: every light-year up to 128 ly, then 64 segments in each octave
//! `[128 × 2ʲ, 128 × 2ʲ⁺¹)` up to the root cube's half-width, 65,536 ly, each 1/64 to 1/128 of its
//! height wide. Between knots `E` is linear, so the profile is exponential across each segment and
//! its integral is exact ([`VerticalProfile::integral_to`]); beyond the last knot `E` keeps the
//! last segment's slope. Between knots the table's `E` is within 10⁻⁴ × max(1, E) of the exact one
//! (tested), so the density is within 10⁻⁴ of itself wherever it is above a third of the
//! mid-plane's, and this moves no count: the normalisation integrates the table itself.
//!
//! `K_z` costs about a millisecond to evaluate: the model holds several hundred Gaussians, each a
//! 32-node quadrature. It is therefore evaluated once per reference radius, at the 16
//! Gauss–Legendre nodes of three panels in `ln z` over 2–131,072 ly, and read between them from
//! each panel's polynomial of degree 15 ([`Gl16Panel::value`]); below 2 ly it is taken as linear in
//! z, as an odd, smooth function is, and above 131,072 ly as falling as z⁻². The integral of `K_z ÷
//! s(z)²` is then taken by `gl16` on every segment of the table, split where the rise stops.
//!
//! # Floating point
//!
//! A disc's envelope is `n0 exp(−(R ÷ L + E(|z|)))`, and the nearest-corner bound of
//! [`bounds`](crate::galaxy::bounds) needs it never to rise with |z| *as computed*, not only as
//! written. So `E(|z|)` is non-decreasing bit for bit, by construction:
//!
//! - **The segment is found exactly.** Every knot and every segment's width is a small integer
//!   times a power of two. [`locate`] finds a height's segment from its integer part, and its
//!   offset `t ∈ [0, 1)` as `z ÷ w − ⌊z ÷ w⌋` with `w` the width: division by a power of two is
//!   exact, and so is the subtraction (Sterbenz: the two lie within a factor of two). So the
//!   segment is `k` exactly when `zₖ ≤ z < zₖ₊₁`, and `t` is exactly `(z − zₖ) ÷ w`.
//! - **Within a segment**, `E = eₖ + t dₖ` with `dₖ ≥ 0`: a product and a sum of monotone
//!   roundings of inputs that do not fall as `z` grows.
//! - **At a knot**, the stored `eₖ₊₁` is not the true `E(zₖ₊₁)` but `eₖ + dₖ` as rounded, with
//!   `dₖ = max(0, E(zₖ₊₁) − eₖ)`. Just below the knot `t < 1`, so `t dₖ ≤ dₖ` and `eₖ + t dₖ ≤
//!   eₖ + dₖ = eₖ₊₁` after rounding; at the knot `t = 0` and `E = eₖ₊₁ + 0 = eₖ₊₁`. The same holds
//!   at the table's end, beyond which `E = e_end + (z − 65,536) × rate` with `rate ≥ 0`.
//!
//! Then `R ÷ L + E`, its negation and `exp` are monotone steps, given a monotone `libm`, exactly
//! as for the exponential discs before them, and the discs need no margin of their own. The tests
//! step across every knot a unit in the last place at a time.

use crate::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
use crate::galaxy::quad::{Gl16Panel, bisect, gl16};
use crate::math;
use crate::units::{KilometresPerSecond, LightYears};

/// The heights below this are tabulated every light-year, ly.
const LINEAR_END: f64 = 128.0;

/// The segments below [`LINEAR_END`], each 1 ly wide.
const LINEAR_SEGMENTS: usize = 128;

/// The segments in each octave above [`LINEAR_END`].
const OCTAVE_SEGMENTS: usize = 64;

/// The octaves `[128 × 2ʲ, 128 × 2ʲ⁺¹)`, j = 0 to 8, which reach [`TABLE_END`].
const OCTAVES: usize = 9;

/// `1 ÷ w` in octave j, whose segments are `w = 2ʲ⁺¹` ly wide: exact powers of two.
const INVERSE_WIDTHS: [f64; OCTAVES] = [
    0.5,
    0.25,
    0.125,
    0.062_5,
    0.031_25,
    0.015_625,
    0.007_812_5,
    0.003_906_25,
    0.001_953_125,
];

/// The number of segments in the table: 128 + 9 × 64.
pub(crate) const SEGMENTS: usize = LINEAR_SEGMENTS + OCTAVES * OCTAVE_SEGMENTS;

/// The height of the table's last knot, ly: the root cube's half-width, so that every point of the
/// cube reads the table.
const TABLE_END: f64 = 65_536.0;

/// The heights, in ly, between which `K_z` is tabulated.
const FORCE_RANGE: (f64, f64) = (2.0, 131_072.0);

/// The panels of the force table in `ln z`, equally wide, by their offsets from the lowest in
/// widths.
const FORCE_PANELS: [f64; 3] = [0.0, 1.0, 2.0];

/// The bracket of the mid-plane dispersion `σ₀` in which [`JeansIntegral::solve`] looks, km/s.
const DISPERSION_BRACKET: (f64, f64) = (0.25, 1_024.0);

/// Bisections of `ln σ₀` over [`DISPERSION_BRACKET`]: 48 halvings narrow it to 3 × 10⁻¹⁴.
pub(crate) const BISECTIONS: u32 = 48;

/// The width of segment `k`, ly: 1 below 128 ly, `2ʲ⁺¹` in octave j.
#[must_use]
fn segment_width(k: usize) -> f64 {
    if k < LINEAR_SEGMENTS {
        1.0
    } else {
        1.0 / INVERSE_WIDTHS[(k - LINEAR_SEGMENTS) / OCTAVE_SEGMENTS]
    }
}

/// The height of knot `k`, 0 to [`SEGMENTS`], ly: exact.
#[must_use]
pub(crate) fn knot(k: usize) -> f64 {
    let small = |n: usize| f64::from(u32::try_from(n).expect("a knot index below 1,000"));
    if k <= LINEAR_SEGMENTS {
        small(k)
    } else if k >= SEGMENTS {
        TABLE_END
    } else {
        // Octave j starts at 64 of its segments' widths.
        let width = segment_width(k);
        (64.0 + small((k - LINEAR_SEGMENTS) % OCTAVE_SEGMENTS)) * width
    }
}

/// Where a height lies in the table: the segment and the offset into it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Height {
    /// The segment, 0 to [`SEGMENTS`] − 1, or [`SEGMENTS`] beyond the table's end.
    segment: usize,
    /// Inside the table, the fraction of the segment's width below the height, in `[0, 1)`;
    /// beyond it, the height above [`TABLE_END`] in ly.
    offset: f64,
}

/// The segment of the table that holds the height `abs_z ≥ 0` (ly), exactly (module
/// documentation, "Floating point").
///
/// Every caller passes `|z|`: [`Site`](super::Site) and the profile's public methods take the
/// absolute value first.
#[must_use]
pub(crate) fn locate(abs_z: f64) -> Height {
    debug_assert!(
        abs_z >= 0.0 || abs_z.is_nan(),
        "a height located as |z|, got {abs_z}"
    );
    if abs_z < LINEAR_END {
        let floor = abs_z.floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number in [0, 128)"
        )]
        let segment = floor as usize;
        Height {
            segment,
            offset: abs_z - floor,
        }
    } else if abs_z < TABLE_END {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a height in [128, 65,536) truncates to its whole part, which fits a u32"
        )]
        let whole = abs_z as u32;
        // The octave from the whole part: 128 × 2ʲ ≤ z < 128 × 2ʲ⁺¹ exactly when 2⁷⁺ʲ ≤ ⌊z⌋ < 2⁸⁺ʲ.
        let octave = usize::try_from(whole.ilog2() - 7).expect("an octave index below 9");
        let scaled = abs_z * INVERSE_WIDTHS[octave];
        let floor = scaled.floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number in [64, 128)"
        )]
        let within = floor as usize - OCTAVE_SEGMENTS;
        Height {
            segment: LINEAR_SEGMENTS + OCTAVE_SEGMENTS * octave + within,
            offset: scaled - floor,
        }
    } else {
        Height {
            segment: SEGMENTS,
            offset: abs_z - TABLE_END,
        }
    }
}

/// `(1 − e^(−d)) ÷ d`, the mean of `e^(−d t)` over `t ∈ [0, 1]`; 1 at `d = 0`.
#[must_use]
fn mean_decay(d: f64) -> f64 {
    if d > 0.0 { -math::exp_m1(-d) / d } else { 1.0 }
}

/// A disc's vertical profile, `n(z) ÷ n(0) = exp(−E(|z|))`, as the vertical Jeans equation shapes
/// it (module documentation), with its dispersion and effective height.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::Fields;
/// use hyperion_sim::galaxy::fields::Shape;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// let Shape::Disc(thick) = fields.components()[6].shape() else { unreachable!() };
/// let profile = thick.profile();
/// // Cored: all but flat at the plane, falling ever faster above it.
/// let h = profile.effective_height().value();
/// assert!((profile.value(0.0) - 1.0).abs() < 1e-15);
/// assert!(profile.value(10.0) > 0.998);
/// assert!(profile.value(2.0 * h) < profile.value(h) * profile.value(h));
/// // The effective height Σ ÷ 2ρ₀ is the integral of the profile, and the drawn height.
/// assert!((h / params.thick_disc().height().value() - 1.0).abs() < 1e-12);
/// assert!((profile.integral_to(-1e9).value() / h - 1.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct VerticalProfile {
    dispersion: KilometresPerSecond,
    /// `γ`, per ly.
    gradient: f64,
    /// `z_γ`, ly.
    reach: f64,
    reference_radius: LightYears,
    effective_height: LightYears,
    /// Per segment: `E` at its lower knot, and `E`'s rise across it.
    segments: Box<[[f64; 2]]>,
    /// `∫₀^zₖ n ÷ n(0) dz` at each segment's lower knot, ly.
    below: Box<[f64]>,
    /// `E` at the table's end, and its slope beyond, per ly.
    end: f64,
    end_rate: f64,
    /// `∫₀^end n ÷ n(0) dz`, ly.
    table_integral: f64,
}

impl VerticalProfile {
    /// The profile whose exponent is `exact` at the knots above the plane, made monotone and
    /// continuous in floating point (module documentation, "Floating point"); it is 0 in the plane.
    ///
    /// # Panics
    ///
    /// If `exact` does not hold one value per knot above the plane, or if the exponent does not
    /// rise across the last segment, which a positive vertical force rules out, so that the
    /// effective height is positive and finite.
    #[must_use]
    fn from_exponents(
        exact: impl ExactSizeIterator<Item = f64>,
        dispersion: KilometresPerSecond,
        (gradient, reach): (f64, f64),
        reference_radius: LightYears,
    ) -> Self {
        assert_eq!(
            exact.len(),
            SEGMENTS,
            "one exponent per knot above the plane"
        );
        let mut segments = Vec::with_capacity(SEGMENTS);
        let mut below = Vec::with_capacity(SEGMENTS);
        let mut e = 0.0;
        let mut integral = 0.0;
        for (k, next) in exact.enumerate() {
            let rise = (next - e).max(0.0);
            segments.push([e, rise]);
            below.push(integral);
            integral += segment_width(k) * math::exp(-e) * mean_decay(rise);
            e += rise;
        }
        let end_rate = segments[SEGMENTS - 1][1] / segment_width(SEGMENTS - 1);
        assert!(
            end_rate > 0.0,
            "a vertical profile's exponent rises at the table's end"
        );
        let effective_height = integral + math::exp(-e) / end_rate;
        assert!(
            effective_height.is_finite() && effective_height > 0.0,
            "a vertical profile's effective height is {effective_height}"
        );
        Self {
            dispersion,
            gradient,
            reach,
            reference_radius,
            effective_height: LightYears::new(effective_height),
            segments: segments.into_boxed_slice(),
            below: below.into_boxed_slice(),
            end: e,
            end_rate,
            table_integral: integral,
        }
    }

    /// `E` at the located height `height`: never falls as the height grows, bit for bit.
    #[must_use]
    pub(crate) fn exponent_at(&self, height: Height) -> f64 {
        match self.segments.get(height.segment) {
            Some(&[e, rise]) => e + height.offset * rise,
            None => self.end + height.offset * self.end_rate,
        }
    }

    /// The exponent `E(|z|)` at the height `z` (ly, either side of the plane): `n(z) = n(0)
    /// exp(−E)`.
    #[must_use]
    pub fn exponent(&self, z: f64) -> f64 {
        self.exponent_at(locate(z.abs()))
    }

    /// The density at the height `z` (ly, either side of the plane) relative to the mid-plane's,
    /// `n(z) ÷ n(0)`: 1 in the plane, never rising with `|z|`.
    #[must_use]
    pub fn value(&self, z: f64) -> f64 {
        math::exp(-self.exponent(z))
    }

    /// `∫₀^|z| n ÷ n(0) dz` up to the height `z` (ly, either side of the plane).
    ///
    /// It is exact for the table, which is exponential across each segment. A disc's column
    /// between two heights is its mid-plane density times the difference (plan 02, P02.T10).
    #[must_use]
    pub fn integral_to(&self, z: f64) -> LightYears {
        let height = locate(z.abs());
        LightYears::new(match self.segments.get(height.segment) {
            Some(&[e, rise]) => {
                let width = segment_width(height.segment);
                self.below[height.segment]
                    + width * math::exp(-e) * height.offset * mean_decay(height.offset * rise)
            }
            None => {
                self.table_integral
                    + math::exp(-self.end)
                        * height.offset
                        * mean_decay(height.offset * self.end_rate)
            }
        })
    }

    /// The effective height `Σ ÷ 2ρ₀ = ∫₀^∞ n ÷ n(0) dz`: for the discs, the drawn height.
    #[must_use]
    pub fn effective_height(&self) -> LightYears {
        self.effective_height
    }

    /// The vertical dispersion in the mid-plane, `σ₀`, at the reference radius.
    #[must_use]
    pub fn dispersion(&self) -> KilometresPerSecond {
        self.dispersion
    }

    /// The vertical dispersion at the height `z` (ly, either side of the plane) and the reference
    /// radius, `σ₀ (1 + γ min(|z|, z_γ))`: the dispersion the profile is the Jeans solution for.
    #[must_use]
    pub fn dispersion_at(&self, z: f64) -> KilometresPerSecond {
        self.dispersion * (1.0 + self.gradient * z.abs().min(self.reach))
    }

    /// `γ`, how fast the dispersion grows with height, per kpc: 0.20 for every disc (Sharma et
    /// al. 2021).
    #[must_use]
    pub fn gradient_per_kpc(&self) -> f64 {
        self.gradient * LIGHT_YEARS_PER_KILOPARSEC
    }

    /// `z_γ`, the height above which the dispersion stops rising.
    #[must_use]
    pub fn gradient_reach(&self) -> LightYears {
        LightYears::new(self.reach)
    }

    /// The radius at which the profile was solved.
    #[must_use]
    pub fn reference_radius(&self) -> LightYears {
        self.reference_radius
    }
}

/// `K_z(R_ref, z)` at one radius, tabulated on panels in `u = ln z` (module documentation, "The
/// table").
#[derive(Debug)]
pub(crate) struct VerticalForce {
    panels: [Gl16Panel; 3],
    /// `ln z` at the lowest edge, and each panel's width in `ln z`.
    lo: f64,
    width: f64,
    /// `K_z(z_lo) ÷ z_lo`, (km/s)² per ly².
    linear: f64,
    /// `K_z(z_hi) z_hi²`, (km/s)² ly.
    far: f64,
}

impl VerticalForce {
    /// The table of the force `k_z(z)`, (km/s)² per ly, for `z > 0` in ly, evaluated at the 48
    /// nodes of its panels in ascending order.
    #[must_use]
    pub(crate) fn new(mut k_z: impl FnMut(f64) -> f64) -> Self {
        let lo = math::ln(FORCE_RANGE.0);
        let hi = math::ln(FORCE_RANGE.1);
        let width = (hi - lo) / 3.0;
        let panels = FORCE_PANELS.map(|i| {
            let (a, b) = (lo + i * width, lo + (i + 1.0) * width);
            let values = Gl16Panel::nodes(a, b).map(|u| k_z(math::exp(u)));
            Gl16Panel::new(a, b, &values)
        });
        let (z_lo, z_hi) = FORCE_RANGE;
        Self {
            linear: panels[0].value(lo) / z_lo,
            far: panels[2].value(hi) * z_hi * z_hi,
            panels,
            lo,
            width,
        }
    }

    /// `K_z` at the height `z > 0` (ly), (km/s)² per ly.
    #[must_use]
    pub(crate) fn at(&self, z: f64) -> f64 {
        let u = math::ln(z);
        let offset = (u - self.lo) / self.width;
        if offset < 0.0 {
            self.linear * z
        } else if offset > 3.0 {
            self.far / (z * z)
        } else {
            let panel = if offset < 1.0 {
                0
            } else if offset < 2.0 {
                1
            } else {
                2
            };
            self.panels[panel].value(u)
        }
    }
}

/// The part of `E` shared by every dispersion at one reference radius, gradient `γ` and reach
/// `z_γ`: `2 ln s(z)` and `∫₀^z K_z ÷ s(z′)² dz′` at every knot, so that a profile for any `σ₀`
/// costs one pass over the knots (module documentation).
#[derive(Debug)]
pub(crate) struct JeansIntegral {
    gradient: f64,
    reach: f64,
    reference_radius: LightYears,
    /// `2 ln s(zₖ)` at every knot.
    log_term: Vec<f64>,
    /// `∫₀^zₖ K_z ÷ s(z)² dz` at every knot, (km/s)².
    potential: Vec<f64>,
}

impl JeansIntegral {
    /// The integrals of `force`, tabulated at `reference_radius`, for the gradient
    /// `gradient_per_kpc` up to the height `reach`: `gl16` on every segment, split where the rise
    /// stops, and summed upwards.
    #[must_use]
    pub(crate) fn new(
        force: &VerticalForce,
        gradient_per_kpc: f64,
        reach: LightYears,
        reference_radius: LightYears,
    ) -> Self {
        let gradient = gradient_per_kpc / LIGHT_YEARS_PER_KILOPARSEC;
        let reach = reach.value();
        let rise = |z: f64| 1.0 + gradient * z.min(reach);
        let integrand = |z: f64| {
            let s = rise(z);
            force.at(z) / (s * s)
        };
        let mut potential = Vec::with_capacity(SEGMENTS + 1);
        let mut log_term = Vec::with_capacity(SEGMENTS + 1);
        let mut sum = 0.0;
        potential.push(sum);
        log_term.push(0.0);
        for k in 0..SEGMENTS {
            let (a, b) = (knot(k), knot(k + 1));
            sum += if a < reach && reach < b {
                gl16(integrand, a, reach) + gl16(integrand, reach, b)
            } else {
                gl16(integrand, a, b)
            };
            potential.push(sum);
            log_term.push(2.0 * math::ln_1p(gradient * b.min(reach)));
        }
        Self {
            gradient,
            reach,
            reference_radius,
            log_term,
            potential,
        }
    }

    /// The profile of a disc whose mid-plane dispersion is `dispersion`.
    #[must_use]
    pub(crate) fn profile(&self, dispersion: KilometresPerSecond) -> VerticalProfile {
        let inverse = 1.0 / (dispersion.value() * dispersion.value());
        let exact = self.log_term[1..]
            .iter()
            .zip(&self.potential[1..])
            .map(|(l, p)| l + p * inverse);
        VerticalProfile::from_exponents(
            exact,
            dispersion,
            (self.gradient, self.reach),
            self.reference_radius,
        )
    }

    /// The profile whose effective height is `height`, by [`BISECTIONS`] bisections of `ln σ₀`
    /// between 0.25 and 1,024 km/s: the effective height grows with the dispersion.
    #[must_use]
    pub(crate) fn solve(&self, height: LightYears) -> VerticalProfile {
        let height = height.value();
        let ln_sigma = bisect(
            |u| {
                self.profile(KilometresPerSecond::new(math::exp(u)))
                    .effective_height()
                    .value()
                    - height
            },
            math::ln(DISPERSION_BRACKET.0),
            math::ln(DISPERSION_BRACKET.1),
            BISECTIONS,
        );
        self.profile(KilometresPerSecond::new(math::exp(ln_sigma)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::consts::{G, LIGHT_YEARS_PER_KILOPARSEC};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;
    use crate::galaxy::quad::gl_panels;

    const PI: f64 = core::f64::consts::PI;

    /// Every knot is located as itself, every height inside a segment exactly, and the widths and
    /// knots tile `[0, 65,536]`.
    #[test]
    fn heights_are_located_exactly() {
        assert!((knot(0)).abs() < f64::MIN_POSITIVE);
        assert!((knot(LINEAR_SEGMENTS) - 128.0).abs() < f64::MIN_POSITIVE);
        assert!((knot(SEGMENTS) - TABLE_END).abs() < f64::MIN_POSITIVE);
        for k in 0..SEGMENTS {
            let (a, b) = (knot(k), knot(k + 1));
            assert!(
                (b - a - segment_width(k)).abs() < f64::MIN_POSITIVE,
                "segment {k}"
            );
            let at = locate(a);
            assert_eq!(at.segment, k, "knot {k} at {a}");
            assert!(at.offset.abs() < f64::MIN_POSITIVE, "knot {k}");
            let below = locate(b.next_down());
            assert_eq!(below.segment, k, "just below knot {}", k + 1);
            assert!(below.offset < 1.0);
        }
        assert_eq!(locate(TABLE_END).segment, SEGMENTS);
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x7ab1e);
        for _ in 0..100_000 {
            let z = math::exp(lcg.next_f64() * math::ln(70_000.0)) - 1.0;
            let at = locate(z);
            if at.segment < SEGMENTS {
                let rebuilt = knot(at.segment) + at.offset * segment_width(at.segment);
                assert!(
                    (rebuilt - z).abs() < f64::MIN_POSITIVE,
                    "{z} rebuilt as {rebuilt}"
                );
                assert!((0.0..1.0).contains(&at.offset));
            } else {
                assert!((at.offset - (z - TABLE_END)).abs() < f64::MIN_POSITIVE);
            }
        }
    }

    /// The fixture's force table at its thin-disc and nuclear-disc reference radii.
    fn fixture_forces() -> (MassModel, [(f64, VerticalForce); 2]) {
        let params = GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let radii = [
            3.0 * params.thin_disc().length().value(),
            2.0 * params.nuclear_disc().length().value(),
        ];
        let forces = radii.map(|r| {
            (
                r,
                VerticalForce::new(|z| {
                    model.vertical_force(LightYears::new(r), LightYears::new(z))
                }),
            )
        });
        (model, forces)
    }

    /// The tabulated force against direct evaluation across its range, at both reference radii.
    #[test]
    fn the_force_table_matches_direct_evaluation() {
        let (model, forces) = fixture_forces();
        for (r, force) in &forces {
            for i in 0..=24 {
                let z = 2.5 * math::exp(f64::from(i) * 0.4);
                let direct = model.vertical_force(LightYears::new(*r), LightYears::new(z));
                let table = force.at(z);
                assert!(
                    (table / direct - 1.0).abs() < 1e-4,
                    "K_z({r}, {z}): {table} against {direct}"
                );
            }
        }
    }

    /// Above the plane of the isothermal sheet, `K_z = 2πGΣ tanh(z ÷ z₀)`, a population of
    /// dispersion `σ² = πGΣz₀` has exactly the sheet's own profile, `sech²(z ÷ z₀)`, whose
    /// effective height is `z₀` (Spitzer 1942, ApJ 95, 329): the Jeans solution, the table and the
    /// solve for a height, all at once. The table's exponent is linear between knots and the true
    /// one convex, so the effective height comes out short, by 3 × 10⁻⁵ at z₀ = 60 ly.
    #[test]
    fn an_isothermal_sheet_is_its_own_jeans_solution() {
        for z0 in [60.0, 300.0, 2_000.0] {
            let sigma = 2.0; // M☉ per ly²
            let force = VerticalForce::new(|z| 2.0 * PI * G * sigma * math::tanh(z / z0));
            let integral = JeansIntegral::new(
                &force,
                0.0,
                LightYears::new(TABLE_END),
                LightYears::new(1.0),
            );
            let dispersion = (PI * G * sigma * z0).sqrt();
            let profile = integral.profile(KilometresPerSecond::new(dispersion));
            for t in [0.0, 0.1, 0.5, 1.0, 2.0, 5.0, 12.0] {
                let z = t * z0;
                let sech = 1.0 / math::cosh(t);
                let expected = sech * sech;
                assert!(
                    (profile.value(z) / expected - 1.0).abs() < 2e-4,
                    "z₀ {z0}, z {z}: {} against {expected}",
                    profile.value(z)
                );
            }
            let h = profile.effective_height().value();
            assert!((h / z0 - 1.0).abs() < 1e-4, "z₀ {z0}: effective height {h}");
            let solved = integral.solve(LightYears::new(z0));
            assert!(
                (solved.dispersion().value() / dispersion - 1.0).abs() < 1e-4,
                "z₀ {z0}: σ {} against {dispersion}",
                solved.dispersion().value()
            );
            assert!((solved.effective_height().value() / z0 - 1.0).abs() < 1e-13);
        }
    }

    /// With `σ = σ₀ (1 + γ min(z, z_γ))` the exponent at every knot is `2 ln s(z)` plus the
    /// integral of `K_z ÷ σ²`, here against a direct quadrature of the force itself, `K tanh(z ÷
    /// 100 ly)`, with the rise's end at 7,000 ly as a panel edge: to 10⁻⁶ of itself, below the end
    /// of the rise and above it, where the exponent grows by `K_z ÷ σ(z_γ)²` per light-year.
    #[test]
    fn a_rising_dispersion_follows_its_closed_form() {
        let k = 0.004; // (km/s)² per ly
        let gamma = 0.2 / LIGHT_YEARS_PER_KILOPARSEC;
        let reach = 7_000.0;
        let force_of = |z: f64| k * math::tanh(z / 100.0);
        let force = VerticalForce::new(force_of);
        let integral =
            JeansIntegral::new(&force, 0.2, LightYears::new(reach), LightYears::new(1.0));
        let sigma = 15.0;
        let profile = integral.profile(KilometresPerSecond::new(sigma));
        let s = |z: f64| 1.0 + gamma * z.min(reach);
        // Knots: every light-year below 128, then 2, 8, 64, 128 and 512 ly apart.
        for z in [
            1.0, 7.0, 150.0, 1_000.0, 6_912.0, 7_040.0, 9_984.0, 40_960.0, 65_024.0,
        ] {
            let mut edges: Vec<f64> = (0..=SEGMENTS)
                .map(knot)
                .filter(|&x| x < z)
                .chain([z, reach])
                .filter(|&x| x <= z)
                .collect();
            edges.sort_by(f64::total_cmp);
            edges.dedup();
            let potential = gl_panels(|x| force_of(x) / (s(x) * s(x)), &edges);
            let expected = 2.0 * math::ln(s(z)) + potential / (sigma * sigma);
            let e = profile.exponent(z);
            assert!(
                (e / expected - 1.0).abs() < 1e-6,
                "z {z}: {e} against {expected}"
            );
        }
        let at = |z: f64| profile.dispersion_at(z).value() / sigma;
        assert!((at(LIGHT_YEARS_PER_KILOPARSEC) - 1.2).abs() < 1e-14);
        assert!((at(-LIGHT_YEARS_PER_KILOPARSEC) - 1.2).abs() < 1e-14);
        assert!((at(20_000.0) - s(reach)).abs() < 1e-14);
        assert!((profile.gradient_per_kpc() - 0.2).abs() < 1e-15);
        assert!((profile.gradient_reach().value() - reach).abs() < f64::MIN_POSITIVE);
        // Either side of the plane alike.
        for z in [0.5, 150.0, 40_960.0] {
            hyperion_testkit::float::assert_same_bits(profile.exponent(-z), profile.exponent(z));
        }
    }

    /// The profile's integral against a direct quadrature of its value, and the effective height
    /// as the whole integral.
    #[test]
    fn integral_to_matches_a_quadrature_of_the_profile() {
        let (_, forces) = fixture_forces();
        let integral = JeansIntegral::new(
            &forces[0].1,
            0.0,
            LightYears::new(TABLE_END),
            LightYears::new(forces[0].0),
        );
        let profile = integral.profile(KilometresPerSecond::new(20.0));
        for z in [0.3, 50.0, 127.5, 128.0, 700.0, 3_333.0, 65_536.0, 80_000.0] {
            let mut edges: Vec<f64> = (0..=SEGMENTS).map(knot).filter(|&k| k < z).collect();
            edges.push(z);
            let direct = gl_panels(|x| profile.value(x), &edges);
            let ours = profile.integral_to(z).value();
            assert!(
                (ours / direct - 1.0).abs() < 1e-12,
                "z {z}: {ours} against {direct}"
            );
        }
        let h = profile.effective_height().value();
        assert!((profile.integral_to(1e12).value() / h - 1.0).abs() < 1e-12);
    }

    /// Profiles of the fixture's forces, cold and hot, with and without a rising dispersion.
    fn fixture_profiles() -> Vec<VerticalProfile> {
        let (_, forces) = fixture_forces();
        let mut profiles = Vec::new();
        for (r, force) in &forces {
            for g in [0.0, 0.2] {
                let integral =
                    JeansIntegral::new(force, g, LightYears::new(7_828.0), LightYears::new(*r));
                for sigma in [3.0, 20.0, 90.0] {
                    profiles.push(integral.profile(KilometresPerSecond::new(sigma)));
                }
            }
        }
        profiles
    }

    /// Stepped a unit in the last place at a time across every knot, and across the table's end,
    /// the exponent never falls, so the density never rises: the nearest-corner bound needs no
    /// margin for the discs (module documentation, "Floating point"; plan 02, P02.T8).
    #[test]
    fn the_exponent_never_falls_across_any_knot() {
        for profile in fixture_profiles() {
            for k in 1..=SEGMENTS {
                let z = knot(k);
                let mut v = z;
                for _ in 0..64 {
                    v = v.next_down();
                }
                let mut previous = profile.exponent(v);
                for _ in 0..128 {
                    v = v.next_up();
                    let e = profile.exponent(v);
                    assert!(
                        e >= previous,
                        "the exponent falls at {v} (knot {k}): {e} after {previous}"
                    );
                    previous = e;
                }
            }
            // Across each segment in sixteen steps and at random pairs.
            let mut lcg = hyperion_testkit::lcg::Lcg::new(0x3e0);
            let mut previous = 0.0;
            for k in 0..SEGMENTS {
                for i in 0..16 {
                    let z = knot(k) + segment_width(k) * f64::from(i) / 16.0;
                    let e = profile.exponent(z);
                    assert!(e >= previous, "falls at {z}");
                    previous = e;
                }
            }
            for _ in 0..20_000 {
                let a = 70_000.0 * lcg.next_f64() * lcg.next_f64();
                let b = a + 500.0 * lcg.next_f64() * lcg.next_f64();
                assert!(profile.exponent(b) >= profile.exponent(a), "{a} to {b}");
            }
        }
    }

    /// The table stays within 10⁻⁴ × max(1, E) of the exponent computed without it, at points
    /// between the knots: `E` at the midpoint of each segment against the direct integral to there.
    #[test]
    fn the_table_is_close_to_the_exact_exponent() {
        let (_, forces) = fixture_forces();
        let (r, force) = &forces[0];
        let gamma = 0.2 / LIGHT_YEARS_PER_KILOPARSEC;
        let integral =
            JeansIntegral::new(force, 0.2, LightYears::new(TABLE_END), LightYears::new(*r));
        let sigma = 10.0;
        let profile = integral.profile(KilometresPerSecond::new(sigma));
        for k in (0..SEGMENTS).step_by(7) {
            let z = knot(k) + 0.5 * segment_width(k);
            let mut edges: Vec<f64> = (0..=k).map(knot).collect();
            edges.push(z);
            let potential = gl_panels(
                |x| {
                    let s = 1.0 + gamma * x;
                    force.at(x) / (s * s)
                },
                &edges,
            );
            let exact = 2.0 * math::ln_1p(gamma * z) + potential / (sigma * sigma);
            let e = profile.exponent(z);
            assert!(
                (e - exact).abs() <= 1e-4 * exact.max(1.0),
                "z {z}: {e} against {exact}"
            );
        }
    }
}
