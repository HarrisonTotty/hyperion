//! The long bar (brainstorm, "Populations"; plan 02, P02.T7.c).

use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::galaxy::params::BarParams;
use crate::math;
use crate::units::LightYears;

/// Where the bar's level part ends, in half-lengths.
///
/// The long bar "is level along most of its length and falls off at the end" (brainstorm,
/// "Populations"; Wegg, Gerhard and Portail 2015, MNRAS 450, 4050). The level part and the
/// Gaussian end are plan 02's (P02.T6.a, P02.T7.c), the same as `MGE_BAR`'s fit in
/// `hyperion-fit`.
pub const BAR_LEVEL: f64 = 0.85;

/// The Gaussian width of the bar's end beyond [`BAR_LEVEL`], in half-lengths.
pub const BAR_END: f64 = 0.15;

/// The long bar along the x axis, `n0 L(|x|) exp(−y² ÷ 2σ_y²) exp(−|z| ÷ h)` systems per cubic
/// light-year.
///
/// `L` is 1 out to 0.85 of the half-length and falls as a Gaussian of 0.15 half-lengths beyond:
/// level along most of the bar, Gaussian across it, exponential in height. Every factor never
/// rises with its coordinate's magnitude, so neither does the product. It is normalised in
/// closed form over all space: `count = n0 × 2 (0.85 + 0.15 √(π ÷ 2)) L_bar × √(2π) σ_y × 2h`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::fields::bar::LongBar;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let bar = LongBar::of(1.1e10, GalaxyParams::milky_way_like().bar())?;
/// let centre = bar.density(&PointLy::new(0.0, 0.0, 0.0));
/// // Level to 0.85 of the half-length, then falling.
/// let level = bar.density(&PointLy::new(0.8 * bar.half_length().value(), 0.0, 0.0));
/// assert!((level / centre - 1.0).abs() < 1e-15);
/// assert!(bar.density(&PointLy::new(bar.half_length().value(), 0.0, 0.0)) < 0.7 * centre);
/// # Ok::<(), hyperion_sim::galaxy::fields::BuildFieldError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongBar {
    n0: f64,
    half_length: LightYears,
    width: LightYears,
    height: LightYears,
    /// `0.85 L_bar`, ly.
    level_end: f64,
    /// `1 ÷ (0.15 L_bar)`, ly⁻¹.
    inv_end_width: f64,
    /// `1 ÷ 2σ_y²`, ly⁻².
    inv_two_width_sq: f64,
    /// `1 ÷ h`, ly⁻¹.
    inv_height: f64,
}

impl LongBar {
    /// A bar of `count` systems over all space with this half-length, Gaussian width `σ_y` across
    /// it and exponential scale height.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative, a length is not positive, or any value is
    /// not finite.
    pub fn new(
        count: f64,
        half_length: LightYears,
        width: LightYears,
        height: LightYears,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        BuildFieldError::check_positive("bar half-length", half_length.value())?;
        BuildFieldError::check_positive("bar width", width.value())?;
        BuildFieldError::check_positive("bar height", height.value())?;
        let (l, s, h) = (half_length.value(), width.value(), height.value());
        let volume = Self::volume(l, s, h);
        Ok(Self {
            n0: count / volume,
            half_length,
            width,
            height,
            level_end: BAR_LEVEL * l,
            inv_end_width: 1.0 / (BAR_END * l),
            inv_two_width_sq: 1.0 / (2.0 * s * s),
            inv_height: 1.0 / h,
        })
    }

    /// The bar of a galaxy's parameters holding `count` systems.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative or not finite.
    pub fn of(count: f64, bar: &BarParams) -> Result<Self, BuildFieldError> {
        Self::new(count, bar.half_length(), bar.width(), bar.height())
    }

    /// `∫ L(|x|) dx × ∫ exp(−y² ÷ 2σ²) dy × ∫ exp(−|z| ÷ h) dz`, ly³.
    fn volume(l: f64, s: f64, h: f64) -> f64 {
        let pi = core::f64::consts::PI;
        2.0 * (BAR_LEVEL + BAR_END * (0.5 * pi).sqrt()) * l * ((2.0 * pi).sqrt() * s) * (2.0 * h)
    }

    /// The central density `n0`, systems per cubic light-year.
    #[must_use]
    pub fn n0(&self) -> f64 {
        self.n0
    }

    /// The half-length.
    #[must_use]
    pub fn half_length(&self) -> LightYears {
        self.half_length
    }

    /// `σ_y`, the Gaussian width across the bar.
    #[must_use]
    pub fn width(&self) -> LightYears {
        self.width
    }

    /// The exponential scale height.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }

    /// The number of systems over all space.
    #[must_use]
    pub fn count(&self) -> f64 {
        Self::volume(
            self.half_length.value(),
            self.width.value(),
            self.height.value(),
        ) * self.n0
    }

    /// The density at `(|x|, y, |z|)` (ly), systems per cubic light-year: `n0 exp(−(e² ÷ 2 +
    /// y² ÷ 2σ_y² + |z| ÷ h))`, where `e` is how far `|x|` lies beyond the level part, in units of
    /// the Gaussian end, and 0 inside it.
    #[must_use]
    pub fn envelope(&self, abs_x: f64, y: f64, abs_z: f64) -> f64 {
        let end = (abs_x - self.level_end).max(0.0) * self.inv_end_width;
        self.n0
            * math::exp(
                -(0.5 * end * end + y * y * self.inv_two_width_sq + abs_z * self.inv_height),
            )
    }

    /// The density at `p`, systems per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.envelope(p.x.abs(), p.y, p.z.abs())
    }

    /// The density at `site`.
    pub(crate) fn density_at(&self, site: &Site) -> f64 {
        self.envelope(site.abs_x, site.abs_y, site.abs_z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_panels;

    /// Each factor's integral by quadrature of the density along one axis, against the closed
    /// form.
    #[test]
    fn the_closed_form_count_matches_quadratures_along_each_axis() {
        let bar = LongBar::new(
            1e10,
            LightYears::new(16_000.0),
            LightYears::new(1_600.0),
            LightYears::new(590.0),
        )
        .unwrap();
        let n0 = bar.n0();
        let along = 2.0
            * gl_panels(
                |x| bar.envelope(x, 0.0, 0.0) / n0,
                &[0.0, 13_600.0, 16_000.0, 20_000.0, 30_000.0, 40_000.0],
            );
        let across = 2.0
            * gl_panels(
                |y| bar.envelope(0.0, y, 0.0) / n0,
                &[0.0, 1_600.0, 4_000.0, 8_000.0, 16_000.0],
            );
        let up = 2.0
            * gl_panels(
                |z| bar.envelope(0.0, 0.0, z) / n0,
                &[0.0, 590.0, 2_000.0, 6_000.0, 15_000.0, 30_000.0],
            );
        let count = along * across * up * n0;
        assert!((count / 1e10 - 1.0).abs() < 1e-12, "{count}");
        assert!((bar.count() / 1e10 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn the_end_is_continuous_and_falls() {
        let bar = LongBar::new(
            1.0,
            LightYears::new(10_000.0),
            LightYears::new(1_000.0),
            LightYears::new(500.0),
        )
        .unwrap();
        let at_level = bar.envelope(8_500.0, 0.0, 0.0);
        assert!((bar.envelope(8_500.0 + 1e-6, 0.0, 0.0) / at_level - 1.0).abs() < 1e-12);
        let one_sigma = bar.envelope(8_500.0 + 1_500.0, 0.0, 0.0) / at_level;
        assert!((one_sigma - math::exp(-0.5)).abs() < 1e-15);
    }

    #[test]
    fn invalid_bars_are_rejected() {
        let l = LightYears::new(1.0);
        assert_eq!(
            LongBar::new(1.0, l, LightYears::new(f64::INFINITY), l)
                .unwrap_err()
                .quantity(),
            "bar width"
        );
    }
}
