//! A star's composition: its iron abundance as drawn, the metal fraction Z the evolution formulae
//! read, and any helium excess.
//!
//! Z = 0.02 × 10^\[Fe/H\] (plan 06, design note 5): 0.02 is the solar metal fraction to which Hurley,
//! Pols and Tout (2000, MNRAS 315, 543) fitted their formulae, so a solar-abundance star is solar to
//! the formulae. \[Fe/H\] is kept as drawn for the consoles and the planetary stage; the formulae see
//! Z clamped to their range of validity, 0.0001–0.03 ([`Composition::z_fit`]). Alpha enhancement is a
//! mark of the population and does not enter the fits.

use crate::math;
use crate::units::{Dex, HeliumExcess, MetalFraction};

/// The metal fraction of a star with \[Fe/H\] = 0 as the evolution formulae see it, Z☉ = 0.02.
///
/// Hurley, Pols and Tout (2000, section 5 and Appendix) write every metallicity-dependent
/// coefficient as a function of ζ = log₁₀(Z ÷ 0.02).
pub const Z_SOLAR: MetalFraction = MetalFraction::new(0.02);

/// The lowest metal fraction the evolution formulae accept, Z = 0.0001.
///
/// Hurley, Pols and Tout (2000, section 3): the models they fit span Z = 0.0001–0.03.
pub const Z_FIT_MIN: MetalFraction = MetalFraction::new(0.0001);

/// The highest metal fraction the evolution formulae accept, Z = 0.03.
///
/// Hurley, Pols and Tout (2000, section 3).
pub const Z_FIT_MAX: MetalFraction = MetalFraction::new(0.03);

/// A star's composition: \[Fe/H\] as drawn, the metal fraction Z it implies, and its helium excess.
///
/// # Examples
///
/// A halo star three hundred times poorer in iron than the Sun is below the formulae's range, which
/// sees it at their floor while the console still reads its drawn abundance:
///
/// ```
/// use hyperion_sim::math::exp10;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{Dex, HeliumExcess};
///
/// let halo = Composition::from_fe_h(Dex::new(-2.5), HeliumExcess::ZERO);
/// assert!((halo.z().value() - 0.02 * exp10(-2.5)).abs() < 1e-12);
/// assert!((halo.z_fit().value() - 0.0001).abs() < 1e-18);
/// assert!((halo.fe_h().value() + 2.5).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Composition {
    z: MetalFraction,
    fe_h: Dex,
    helium_excess: HeliumExcess,
}

impl Composition {
    /// The Sun's composition as the formulae see it: \[Fe/H\] = 0, Z = 0.02, no helium excess.
    pub const SOLAR: Self = Self {
        z: Z_SOLAR,
        fe_h: Dex::ZERO,
        helium_excess: HeliumExcess::ZERO,
    };

    /// The composition of a star with iron abundance `fe_h` (dex relative to the Sun, finite) and
    /// helium excess `helium_excess` (ΔY, non-negative): Z = 0.02 × 10^\[Fe/H\]. The arithmetic
    /// form is output: every stellar golden inherits it.
    ///
    /// # Panics
    ///
    /// In debug builds, if `fe_h` or `helium_excess` is not finite, or `helium_excess` is negative.
    #[must_use]
    pub fn from_fe_h(fe_h: Dex, helium_excess: HeliumExcess) -> Self {
        debug_assert!(fe_h.value().is_finite(), "[Fe/H] must be finite");
        debug_assert!(
            helium_excess.value().is_finite() && helium_excess.value() >= 0.0,
            "a helium excess is finite and non-negative"
        );
        Self {
            z: Z_SOLAR * math::exp10(fe_h.value()),
            fe_h,
            helium_excess,
        }
    }

    /// The metal fraction as drawn, Z = 0.02 × 10^\[Fe/H\], unclamped.
    #[must_use]
    pub const fn z(&self) -> MetalFraction {
        self.z
    }

    /// The metal fraction the evolution formulae read: [`Composition::z`] clamped to
    /// [`Z_FIT_MIN`]–[`Z_FIT_MAX`], the range Hurley, Pols and Tout (2000) fitted.
    #[must_use]
    pub fn z_fit(&self) -> MetalFraction {
        MetalFraction::new(self.z.value().clamp(Z_FIT_MIN.value(), Z_FIT_MAX.value()))
    }

    /// The iron abundance \[Fe/H\] as drawn, dex relative to the Sun.
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The helium excess ΔY: zero for every star the grid places.
    #[must_use]
    pub const fn helium_excess(&self) -> HeliumExcess {
        self.helium_excess
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn solar_iron_gives_the_formulae_solar_metal_fraction() {
        let sun = Composition::from_fe_h(Dex::ZERO, HeliumExcess::ZERO);
        assert_same_bits(sun.z().value(), 0.02);
        assert_same_bits(sun.z_fit().value(), 0.02);
        assert_eq!(sun, Composition::SOLAR);
    }

    #[test]
    fn metal_fraction_scales_as_ten_to_the_iron_abundance() {
        let rich = Composition::from_fe_h(Dex::new(0.1), HeliumExcess::ZERO);
        let ratio = rich.z() / Z_SOLAR;
        assert!((ratio - 1.258_925_411_794_167).abs() < 1e-14, "{ratio}");
        let poor = Composition::from_fe_h(Dex::new(-1.0), HeliumExcess::ZERO);
        assert!((poor.z().value() - 0.002).abs() < 1e-17);
    }

    #[test]
    fn the_fitted_metal_fraction_is_clamped_at_both_ends() {
        let poor = Composition::from_fe_h(Dex::new(-4.0), HeliumExcess::ZERO);
        assert!(poor.z().value() < 1e-5);
        assert_same_bits(poor.z_fit().value(), 0.0001);
        let rich = Composition::from_fe_h(Dex::new(0.5), HeliumExcess::ZERO);
        assert!(rich.z().value() > 0.06);
        assert_same_bits(rich.z_fit().value(), 0.03);
        // Inside the range the drawn value passes through untouched.
        let inside = Composition::from_fe_h(Dex::new(-0.7), HeliumExcess::ZERO);
        assert_same_bits(inside.z_fit().value(), inside.z().value());
        // The clamp's edges are Z = 0.0001 and 0.03 themselves.
        let at_floor = Composition::from_fe_h(Dex::new(math::log10(0.005)), HeliumExcess::ZERO);
        assert!((at_floor.z_fit().value() - 0.0001).abs() < 1e-18);
    }

    #[test]
    fn iron_abundance_and_helium_excess_are_kept_as_given() {
        let c = Composition::from_fe_h(Dex::new(-1.3), HeliumExcess::new(0.08));
        assert_same_bits(c.fe_h().value(), -1.3);
        assert_same_bits(c.helium_excess().value(), 0.08);
    }
}
