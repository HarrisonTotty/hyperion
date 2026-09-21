//! Spiral arms: logarithmic spirals that modulate the discs' densities (plan 02, Design note 10).
//!
//! Arms are traced by young stars, not by mass (brainstorm, "Fields" and "Populations"): the old
//! discs carry a gentle cosine ripple of 10–30% and the young disc a sharp profile, both averaging
//! exactly 1 around every circle, so that the arms move no systems between radii.
//!
//! - The **phase** is `φ = n (θ + ln(R ÷ L) ÷ tan p)` for `n` arms of pitch `p` and the bar's
//!   half-length `L`, with `θ` the azimuth from +x. A ridge lies where `φ` is a multiple of 2π. The
//!   arms trail a counter-clockwise rotation (a ridge's azimuth falls as R grows), and at `R = L`
//!   the ridges leave the bar's ends: with two arms they lie on the x axis there. The phase's
//!   gradient has the magnitude `n ÷ (R sin p)` ([`ArmGeometry::phase_rate`]).
//! - The **fade-in** `f(R) = ½ (1 + tanh((R − L) ÷ 0.1 L))` switches the arms on outside the bar.
//!   It is computed as `1 ÷ (1 + e^(−2u))`, the same function with one exponential and no `tanh`.
//! - The **gentle arm** of the old thin disc is `1 + f(R) a cos φ`.
//! - The **sharp arm** of the young disc is `1 + f(R) A (g − 1)` with `g = exp(k (cos φ − 1)) ÷
//!   I₀ₑ(k)` and `I₀ₑ(k) = e^(−k) I₀(k)`. Because `(1 ÷ 2π) ∫ exp(k cos φ) dφ = I₀(k)`, `g`
//!   averages exactly 1 around a circle for any `k`, which is the closed-form mean the brainstorm
//!   asks for. The width is set in light-years, not in phase: `k(R) = (R sin p ÷ (n σ_w))²`, so
//!   that near a ridge `g ≈ exp(−d² ÷ 2σ_w²)` in the perpendicular distance `d`. Near the centre
//!   `k` is small and the profile degrades smoothly to a cosine.
//!
//! At `R = 0` the phase is undefined; every factor is exactly 1 there, as `f(0)` is below 10⁻⁸
//! anyway.
//!
//! Lengths are light-years in the galactic frame, angles radians.

use super::BuildFieldError;
use crate::galaxy::params::{ArmCount, GalaxyParams};
use crate::galaxy::special::bessel_i0e;
use crate::math;
use crate::units::{LightYears, Radians};

/// The fade-in's width over the bar's half-length: `f(R) = ½ (1 + tanh((R − L) ÷ 0.1 L))` (plan
/// 02, Design note 10).
pub const FADE_WIDTH_RATIO: f64 = 0.1;

/// The arms' shape, shared by every arm factor: the stars' sharp and gentle arms here and the gas
/// field's lanes (plan 07).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::arms::ArmGeometry;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let arms = ArmGeometry::of(&GalaxyParams::milky_way_like());
/// // The arms fade in at the bar's end: half strength there, all but absent at half of it.
/// let end = arms.bar_half_length().value();
/// assert!((arms.fade(end) - 0.5).abs() < 1e-15);
/// assert!(arms.fade(0.5 * end) < 1e-4);
/// // A ridge leaves the bar's end on the x axis.
/// assert!(arms.phase(end, 0.0).unwrap().abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmGeometry {
    count: ArmCount,
    /// `n` as a float.
    arms: f64,
    pitch: Radians,
    sin_pitch: f64,
    /// `1 ÷ tan p`.
    cot_pitch: f64,
    /// `L`, ly.
    bar_half_length: f64,
    /// `2 ÷ (0.1 L)`, ly⁻¹: the fade-in's `e^(−2u)` is `exp(rate × (L − R))`.
    fade_rate: f64,
}

impl ArmGeometry {
    /// Arms of `count` and `pitch` whose fade-in is centred on the bar's end, `bar_half_length`.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the pitch is not strictly between 0 and π ÷ 2, or the half-length is
    /// not positive and finite.
    pub fn new(
        count: ArmCount,
        pitch: Radians,
        bar_half_length: LightYears,
    ) -> Result<Self, BuildFieldError> {
        let p = pitch.value();
        if !(p > 0.0 && p < 0.5 * core::f64::consts::PI) {
            return Err(BuildFieldError::new("arm pitch", p));
        }
        BuildFieldError::check_positive("bar half-length", bar_half_length.value())?;
        let l = bar_half_length.value();
        let (sin_pitch, cos_pitch) = math::sin_cos(p);
        Ok(Self {
            count,
            arms: f64::from(count.get()),
            pitch,
            sin_pitch,
            cot_pitch: cos_pitch / sin_pitch,
            bar_half_length: l,
            fade_rate: 2.0 / (FADE_WIDTH_RATIO * l),
        })
    }

    /// The arms of a galaxy: its arm count and pitch, fading in at its bar's end.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose pitch is 10–18° and bar half-length 10,000–18,000 ly.
    #[must_use]
    pub fn of(params: &GalaxyParams) -> Self {
        let arms = params.arms();
        Self::new(arms.count(), arms.pitch(), params.bar().half_length())
            .expect("built parameters hold a pitch of 10–18° and a positive bar")
    }

    /// Two arms or four.
    #[must_use]
    pub fn count(&self) -> ArmCount {
        self.count
    }

    /// The pitch angle `p`.
    #[must_use]
    pub fn pitch(&self) -> Radians {
        self.pitch
    }

    /// The bar's half-length `L`, where the arms start.
    #[must_use]
    pub fn bar_half_length(&self) -> LightYears {
        LightYears::new(self.bar_half_length)
    }

    /// The fade-in `f(R) = ½ (1 + tanh((R − L) ÷ 0.1 L))` at radius `r` (ly), which rises from
    /// about 2 × 10⁻⁹ at the centre to 1 outside the bar.
    #[must_use]
    pub fn fade(&self, r: f64) -> f64 {
        1.0 / (1.0 + math::exp(self.fade_rate * (self.bar_half_length - r)))
    }

    /// The phase `φ = n (θ + ln(R ÷ L) ÷ tan p)` at the in-plane point `(x, y)` (ly), unreduced;
    /// `None` at `R = 0`, where it is undefined.
    #[must_use]
    pub fn phase(&self, x: f64, y: f64) -> Option<f64> {
        let r_sq = x * x + y * y;
        (r_sq > 0.0).then(|| self.phase_polar(r_sq.sqrt(), math::atan2(y, x)))
    }

    /// The phase at radius `r > 0` (ly) and azimuth `theta` (radians from +x, counter-clockwise).
    #[must_use]
    pub fn phase_polar(&self, r: f64, theta: f64) -> f64 {
        self.arms * (theta + math::ln(r / self.bar_half_length) * self.cot_pitch)
    }

    /// `|∇φ| = n ÷ (R sin p)` at radius `r > 0` (ly), radians per light-year: how fast the phase
    /// changes with distance in the plane, which sets a cell's range of phase (plan 02, P02.T8.b).
    #[must_use]
    pub fn phase_rate(&self, r: f64) -> f64 {
        self.arms / (r * self.sin_pitch)
    }

    /// The azimuth (radians, unreduced) at which ridge `j` (0 to n − 1) crosses the circle of
    /// radius `r > 0`: `2πj ÷ n − ln(R ÷ L) ÷ tan p`. It falls as `r` grows: the arms trail.
    #[must_use]
    pub fn ridge_azimuth(&self, r: f64, j: u32) -> f64 {
        2.0 * core::f64::consts::PI * f64::from(j) / self.arms
            - math::ln(r / self.bar_half_length) * self.cot_pitch
    }

    /// The arm quantities at the in-plane point `(x, y)` (ly), which every arm factor there reads.
    #[must_use]
    pub fn point(&self, x: f64, y: f64) -> ArmPoint {
        let r_sq = x * x + y * y;
        self.point_with(x, y, r_sq, r_sq.sqrt())
    }

    /// [`point`](Self::point) at radius `r` and azimuth `theta`, for a caller that works in polar
    /// coordinates, such as the gas field's lanes at a shifted radius (plan 07).
    #[must_use]
    pub fn point_polar(&self, r: f64, theta: f64) -> ArmPoint {
        if r > 0.0 {
            ArmPoint {
                r_sq: r * r,
                fade: self.fade(r),
                cos_phase: math::cos(self.phase_polar(r, theta)),
            }
        } else {
            ArmPoint::CENTRE
        }
    }

    /// [`point`](Self::point) given `r_sq = x² + y²` and `r = √r_sq`, which the densities have
    /// already computed.
    ///
    /// `cos φ` is taken without an `atan2`: `cos nθ` and `sin nθ` follow from `x ÷ R` and `y ÷ R`
    /// by the double-angle formulae, once for two arms and twice for four, and `cos φ = cos nθ cos β
    /// − sin nθ sin β` with `β = n ln(R ÷ L) ÷ tan p`. It is the same function as
    /// `cos(`[`phase`](Self::phase)`)`, to rounding, for a third less time in the densities' inner
    /// loop.
    pub(crate) fn point_with(&self, x: f64, y: f64, r_sq: f64, r: f64) -> ArmPoint {
        if r_sq > 0.0 {
            let inv_r = 1.0 / r;
            let (cos, sin) = (x * inv_r, y * inv_r);
            let (mut cos_n, mut sin_n) = (cos * cos - sin * sin, 2.0 * cos * sin);
            if matches!(self.count, ArmCount::Four) {
                (cos_n, sin_n) = (cos_n * cos_n - sin_n * sin_n, 2.0 * cos_n * sin_n);
            }
            let (sin_b, cos_b) =
                math::sin_cos(self.arms * math::ln(r / self.bar_half_length) * self.cot_pitch);
            // Rounding can carry the product a few ulps past ±1; clamped, the sharp profile never
            // exceeds its ridge value 1 ÷ I₀ₑ(k), which the cell bounds take as its peak.
            ArmPoint {
                r_sq,
                fade: self.fade(r),
                cos_phase: (cos_n * cos_b - sin_n * sin_b).clamp(-1.0, 1.0),
            }
        } else {
            ArmPoint::CENTRE
        }
    }
}

/// The arm quantities at one point of the plane: `R²`, the fade-in `f(R)` and `cos φ`.
///
/// Computing them costs a logarithm, a sine and cosine and an exponential; the densities compute
/// them once per point and hand them to every arm factor there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmPoint {
    r_sq: f64,
    fade: f64,
    cos_phase: f64,
}

impl ArmPoint {
    /// The centre, where the phase is undefined and every factor is exactly 1.
    pub const CENTRE: Self = Self {
        r_sq: 0.0,
        fade: 0.0,
        cos_phase: 1.0,
    };

    /// `R²`, ly².
    #[must_use]
    pub fn r_sq(&self) -> f64 {
        self.r_sq
    }

    /// The fade-in `f(R)`; 0 at the centre.
    #[must_use]
    pub fn fade(&self) -> f64 {
        self.fade
    }

    /// `cos φ`; 1 at the centre.
    #[must_use]
    pub fn cos_phase(&self) -> f64 {
        self.cos_phase
    }

    fn is_centre(&self) -> bool {
        self.r_sq == 0.0
    }
}

/// The young disc's sharp arm, `1 + f(R) A (g − 1)` with `g = exp(k (cos φ − 1)) ÷ I₀ₑ(k)` and
/// `k = (R sin p ÷ (n σ_w))²` (plan 02, Design note 10).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::arms::{ArmGeometry, SharpArm};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::units::LightYears;
///
/// let params = GalaxyParams::milky_way_like();
/// let arm = SharpArm::young_disc(&params);
/// // On a ridge at 26,000 ly the young disc is several times its mean; between arms, 1 − A.
/// let r = 26_000.0;
/// let theta = arm.geometry().ridge_azimuth(r, 0);
/// let on = arm.factor(&arm.geometry().point_polar(r, theta));
/// let off = arm.factor(&arm.geometry().point_polar(r, theta + 0.25 * std::f64::consts::PI));
/// assert!(on > 3.0 && off < 0.25);
/// // Built with another width and fraction, as the gas field's lanes are (plan 07).
/// let lane = SharpArm::new(*arm.geometry(), LightYears::new(150.0), 0.5)?;
/// assert!(lane.factor(&arm.geometry().point_polar(r, theta)) > on);
/// # Ok::<(), hyperion_sim::galaxy::fields::BuildFieldError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharpArm {
    geometry: ArmGeometry,
    width: LightYears,
    fraction: f64,
    /// `(sin p ÷ (n σ_w))²`, ly⁻²: `k = R² ×` this.
    k_per_r_sq: f64,
}

impl SharpArm {
    /// A sharp arm on `geometry` with the Gaussian width `width` (`σ_w`) across a ridge and the
    /// fraction `fraction` (A) of the density gathered into the arms.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the width is not positive and finite, or the fraction lies outside
    /// `[0, 1]`.
    pub fn new(
        geometry: ArmGeometry,
        width: LightYears,
        fraction: f64,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_positive("arm width", width.value())?;
        BuildFieldError::check_fraction("arm fraction", fraction)?;
        let per_r = geometry.sin_pitch / (geometry.arms * width.value());
        Ok(Self {
            geometry,
            width,
            fraction,
            k_per_r_sq: per_r * per_r,
        })
    }

    /// The young disc's arm: the galaxy's geometry with its drawn `σ_w` and A.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose width is 250–500 ly and fraction 0.7–0.9.
    #[must_use]
    pub fn young_disc(params: &GalaxyParams) -> Self {
        let arms = params.arms();
        Self::new(
            ArmGeometry::of(params),
            arms.young_width(),
            arms.young_fraction(),
        )
        .expect("built parameters hold a width of 250–500 ly and a fraction of 0.7–0.9")
    }

    /// The geometry the arm follows.
    #[must_use]
    pub fn geometry(&self) -> &ArmGeometry {
        &self.geometry
    }

    /// `σ_w`, the Gaussian width across a ridge far from the centre.
    #[must_use]
    pub fn width(&self) -> LightYears {
        self.width
    }

    /// A, the fraction of the density gathered into the arms.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.fraction
    }

    /// The concentration `k(R) = (R sin p ÷ (n σ_w))²` at radius `r` (ly). It rises with R.
    #[must_use]
    pub fn k(&self, r: f64) -> f64 {
        r * r * self.k_per_r_sq
    }

    /// The profile `g = exp(k (cos φ − 1)) ÷ I₀ₑ(k)` for `cos φ` and `k`, which averages 1 around
    /// a circle. Its peak, on a ridge, is `1 ÷ I₀ₑ(k)`, which rises with `k`.
    #[must_use]
    pub fn profile(cos_phase: f64, k: f64) -> f64 {
        math::exp(k * (cos_phase - 1.0)) / bessel_i0e(k)
    }

    /// The factor `1 + f(R) A (g − 1)` at a point; exactly 1 at the centre.
    #[must_use]
    pub fn factor(&self, at: &ArmPoint) -> f64 {
        if at.is_centre() {
            return 1.0;
        }
        let g = Self::profile(at.cos_phase, at.r_sq * self.k_per_r_sq);
        1.0 + at.fade * self.fraction * (g - 1.0)
    }
}

/// The old thin disc's gentle arm, `1 + f(R) a cos φ` (plan 02, Design note 10): the brainstorm's
/// "10–30% ripple in the old stars that carry the mass".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GentleArm {
    geometry: ArmGeometry,
    amplitude: f64,
}

impl GentleArm {
    /// A gentle arm on `geometry` with the cosine amplitude `amplitude` (a).
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the amplitude lies outside `[0, 1]`, where the factor could turn
    /// negative.
    pub fn new(geometry: ArmGeometry, amplitude: f64) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_fraction("arm amplitude", amplitude)?;
        Ok(Self {
            geometry,
            amplitude,
        })
    }

    /// The old discs' arm: the galaxy's geometry with its drawn amplitude.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose amplitude is 0.10–0.30.
    #[must_use]
    pub fn old_disc(params: &GalaxyParams) -> Self {
        Self::new(ArmGeometry::of(params), params.arms().old_amplitude())
            .expect("built parameters hold an amplitude of 0.10–0.30")
    }

    /// The geometry the arm follows.
    #[must_use]
    pub fn geometry(&self) -> &ArmGeometry {
        &self.geometry
    }

    /// a, the cosine amplitude.
    #[must_use]
    pub fn amplitude(&self) -> f64 {
        self.amplitude
    }

    /// The factor `1 + f(R) a cos φ` at a point; exactly 1 at the centre.
    #[must_use]
    pub fn factor(&self, at: &ArmPoint) -> f64 {
        if at.is_centre() {
            return 1.0;
        }
        1.0 + at.fade * self.amplitude * at.cos_phase
    }
}

/// A disc's arm modulation: sharp for the young disc, gentle for the old thin disc's sub-discs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arm {
    /// The young disc's sharp arm.
    Sharp(SharpArm),
    /// The old thin disc's gentle arm.
    Gentle(GentleArm),
}

impl Arm {
    /// The geometry the arm follows.
    #[must_use]
    pub fn geometry(&self) -> &ArmGeometry {
        match self {
            Self::Sharp(arm) => arm.geometry(),
            Self::Gentle(arm) => arm.geometry(),
        }
    }

    /// The factor at a point.
    #[must_use]
    pub fn factor(&self, at: &ArmPoint) -> f64 {
        match self {
            Self::Sharp(arm) => arm.factor(at),
            Self::Gentle(arm) => arm.factor(at),
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::params::GalaxyParamsBuilder;
    use crate::units::Degrees;

    const PI: f64 = core::f64::consts::PI;

    fn geometry(count: ArmCount, pitch_degrees: f64) -> ArmGeometry {
        ArmGeometry::new(
            count,
            Radians::from(Degrees::new(pitch_degrees)),
            LightYears::new(15_000.0),
        )
        .unwrap()
    }

    /// Every combination of arm count, pitch and width at the ends of their ranges.
    fn arms() -> Vec<(SharpArm, GentleArm)> {
        let mut all = Vec::new();
        for count in [ArmCount::Two, ArmCount::Four] {
            for pitch in [10.0, 18.0] {
                for width in [250.0, 500.0] {
                    let g = geometry(count, pitch);
                    all.push((
                        SharpArm::new(g, LightYears::new(width), 0.9).unwrap(),
                        GentleArm::new(g, 0.3).unwrap(),
                    ));
                }
            }
        }
        all
    }

    /// The azimuthal mean of each factor is 1 to 10⁻⁶ at 40 radii by 4,096-point sums (P02.T7.a).
    /// The sum over equally spaced azimuths is the trapezoid rule of a periodic function, which
    /// converges geometrically, so the mean is checked, not the rule.
    #[test]
    fn every_arm_factor_averages_one_around_a_circle() {
        for (sharp, gentle) in arms() {
            for i in 1..=40 {
                let r = 70_000.0 * f64::from(i) / 40.0;
                let (mut sharp_sum, mut gentle_sum) = (0.0, 0.0);
                for j in 0..4_096 {
                    let theta = 2.0 * PI * f64::from(j) / 4_096.0;
                    let at = sharp.geometry().point_polar(r, theta);
                    sharp_sum += sharp.factor(&at);
                    gentle_sum += gentle.factor(&at);
                }
                let sharp_mean = sharp_sum / 4_096.0;
                let gentle_mean = gentle_sum / 4_096.0;
                assert!(
                    (sharp_mean - 1.0).abs() < 1e-6,
                    "sharp at {r}: {sharp_mean} (k = {})",
                    sharp.k(r)
                );
                assert!((gentle_mean - 1.0).abs() < 1e-6, "gentle at {r}");
            }
        }
    }

    /// The ridge's full width at half maximum, measured across the ridge along the phase's
    /// gradient, is 2.355 `σ_w` to 3% at 20,000 and 40,000 ly (P02.T7.a).
    #[test]
    fn the_sharp_ridge_is_as_wide_as_its_width() {
        let fwhm_per_sigma = 2.0 * (2.0 * core::f64::consts::LN_2).sqrt();
        for (sharp, _) in arms() {
            let g = sharp.geometry();
            for r in [20_000.0, 40_000.0] {
                let theta = g.ridge_azimuth(r, 1);
                let (x0, y0) = (r * math::cos(theta), r * math::sin(theta));
                let k = sharp.k(r);
                let peak = SharpArm::profile(1.0, k);
                // The direction of ∇φ: radial and azimuthal parts in the ratio 1 ÷ tan p : 1.
                let (sin, cos) = math::sin_cos(g.pitch().value());
                let radial = (x0 / r, y0 / r);
                let tangential = (-y0 / r, x0 / r);
                let dir = (
                    cos * radial.0 + sin * tangential.0,
                    cos * radial.1 + sin * tangential.1,
                );
                let half = |sign: f64| {
                    crate::galaxy::quad::bisect(
                        |d| {
                            let at = g.point(x0 + sign * d * dir.0, y0 + sign * d * dir.1);
                            SharpArm::profile(at.cos_phase(), k) - 0.5 * peak
                        },
                        0.0,
                        3.0 * sharp.width().value(),
                        60,
                    )
                };
                let fwhm = half(1.0) + half(-1.0);
                let expected = fwhm_per_sigma * sharp.width().value();
                assert!(
                    (fwhm / expected - 1.0).abs() < 0.03,
                    "{fwhm} ly against {expected} at {r} ly, k = {k}"
                );
            }
        }
    }

    /// With two arms both ridges leave the bar's ends on the x axis; with four the other two are
    /// on the y axis.
    #[test]
    fn ridges_leave_the_bar_ends() {
        for count in [ArmCount::Two, ArmCount::Four] {
            let g = geometry(count, 14.0);
            let l = g.bar_half_length().value();
            let n = count.get();
            for j in 0..n {
                let theta = g.ridge_azimuth(l, j);
                assert!(
                    (theta - 2.0 * PI * f64::from(j) / f64::from(n)).abs() < 1e-15,
                    "ridge {j}"
                );
                let phase = g.phase_polar(l, theta);
                assert!((math::cos(phase) - 1.0).abs() < 1e-15);
            }
            assert!(g.phase(l, 0.0).unwrap().abs() < 1e-15);
            assert!((g.phase(-l, 0.0).unwrap() - f64::from(n) * PI).abs() < 1e-12);
            if n == 2 {
                // Two arms: the far bar end is the other ridge.
                assert!((math::cos(g.phase(-l, 0.0).unwrap()) - 1.0).abs() < 1e-12);
            }
        }
    }

    /// The azimuth of a ridge falls as R grows: the arms trail a counter-clockwise rotation.
    #[test]
    fn ridges_trail() {
        for (sharp, _) in arms() {
            let g = sharp.geometry();
            let mut previous = g.ridge_azimuth(1_000.0, 0);
            for i in 1..=100 {
                let r = 1_000.0 * f64::from(i + 1);
                let theta = g.ridge_azimuth(r, 0);
                assert!(theta < previous, "at {r}");
                previous = theta;
                // The factor peaks on that azimuth, not beside it.
                let on = sharp.factor(&g.point_polar(r, theta));
                assert!(on >= sharp.factor(&g.point_polar(r, theta + 1e-3)));
                assert!(on >= sharp.factor(&g.point_polar(r, theta - 1e-3)));
            }
        }
    }

    /// The densities' `cos φ`, taken by double angles, is `cos` of the phase to rounding and never
    /// leaves [−1, 1].
    #[test]
    fn the_arm_point_is_the_cosine_of_the_phase() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0xa7);
        for count in [ArmCount::Two, ArmCount::Four] {
            let g = geometry(count, 13.0);
            for _ in 0..20_000 {
                let r = 80_000.0 * lcg.next_f64() * lcg.next_f64();
                let theta = 2.0 * PI * lcg.next_f64();
                let (x, y) = (r * math::cos(theta), r * math::sin(theta));
                let at = g.point(x, y);
                let direct = math::cos(g.phase(x, y).unwrap());
                assert!((at.cos_phase() - direct).abs() < 1e-12, "at ({x}, {y})");
                assert!((-1.0..=1.0).contains(&at.cos_phase()));
                assert!((at.fade() - g.fade(r)).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn the_phase_gradient_has_the_stated_magnitude() {
        let g = geometry(ArmCount::Four, 12.0);
        for r in [3_000.0, 20_000.0, 60_000.0] {
            let h = 1e-3;
            let d_r = (g.phase_polar(r + h, 0.3) - g.phase_polar(r - h, 0.3)) / (2.0 * h);
            let d_t = (g.phase_polar(r, 0.3 + h) - g.phase_polar(r, 0.3 - h)) / (2.0 * h) / r;
            let magnitude = (d_r * d_r + d_t * d_t).sqrt();
            assert!((magnitude / g.phase_rate(r) - 1.0).abs() < 1e-6, "at {r}");
        }
    }

    #[test]
    fn every_factor_is_one_at_the_centre_and_the_fade_is_the_tanh() {
        for (sharp, gentle) in arms() {
            let g = sharp.geometry();
            assert_eq!(g.phase(0.0, 0.0), None);
            let centre = g.point(0.0, 0.0);
            assert_eq!(centre, ArmPoint::CENTRE);
            assert_same_bits(sharp.factor(&centre), 1.0);
            assert_same_bits(gentle.factor(&centre), 1.0);
            assert_eq!(g.point_polar(0.0, 1.0), ArmPoint::CENTRE);
            let l = g.bar_half_length().value();
            assert!(g.fade(0.0) < 1e-8);
            for r in [0.0, 0.5 * l, 0.9 * l, l, 1.1 * l, 2.0 * l] {
                let tanh = f64::midpoint(1.0, math::tanh((r - l) / (0.1 * l)));
                assert!((g.fade(r) - tanh).abs() < 1e-15, "at {r}");
            }
        }
    }

    #[test]
    fn the_galaxy_s_arms_carry_its_parameters() {
        let params = GalaxyParamsBuilder::new()
            .arm_count(ArmCount::Two)
            .arm_pitch(Degrees::new(15.0))
            .build()
            .unwrap();
        let sharp = SharpArm::young_disc(&params);
        let gentle = GentleArm::old_disc(&params);
        assert_eq!(sharp.geometry(), gentle.geometry());
        assert_eq!(sharp.geometry().count(), ArmCount::Two);
        assert_eq!(sharp.width(), params.arms().young_width());
        assert!((sharp.fraction() - params.arms().young_fraction()).abs() < 1e-15);
        assert!((gentle.amplitude() - params.arms().old_amplitude()).abs() < 1e-15);
        assert_eq!(
            sharp.geometry().bar_half_length(),
            params.bar().half_length()
        );
    }

    #[test]
    fn invalid_arms_are_rejected() {
        let g = geometry(ArmCount::Two, 12.0);
        let quantity = |e: BuildFieldError| e.quantity();
        assert_eq!(
            quantity(
                ArmGeometry::new(ArmCount::Two, Radians::new(0.0), LightYears::new(1.0))
                    .unwrap_err()
            ),
            "arm pitch"
        );
        assert_eq!(
            quantity(
                ArmGeometry::new(ArmCount::Two, Radians::new(0.2), LightYears::new(-1.0))
                    .unwrap_err()
            ),
            "bar half-length"
        );
        assert_eq!(
            quantity(SharpArm::new(g, LightYears::new(0.0), 0.5).unwrap_err()),
            "arm width"
        );
        assert_eq!(
            quantity(SharpArm::new(g, LightYears::new(300.0), 1.5).unwrap_err()),
            "arm fraction"
        );
        assert_eq!(
            quantity(GentleArm::new(g, f64::NAN).unwrap_err()),
            "arm amplitude"
        );
    }
}
