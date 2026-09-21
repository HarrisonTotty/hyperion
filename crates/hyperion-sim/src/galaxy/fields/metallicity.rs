//! Metallicity as a distribution of \[Fe/H\] per component, position and age (plan 02, Design
//! note 14 and P02.T7.e).
//!
//! The brainstorm's "Fields": metallicity "falls with galactic radius (about −0.05 dex per kpc in
//! the Milky Way disc) and with age, with scatter". The system stage (plan 06) draws a system's
//! \[Fe/H\] from the distribution returned here; nothing is drawn in this module.
//!
//! Values the brainstorm does not give are plan 02's (P02.T7.e), each marked there for
//! re-checking against Bland-Hawthorn and Gerhard (2016, ARA&A 54, 529) and here as provisional:
//!
//! - thin discs, young and old: mean `0.0 + gradient × (R − 3 lengths) − 0.04 dex per Gyr × (age −
//!   4.5 Gyr)`, clamped to [−1.0, +0.5], sigma 0.15. The reference point, three scale lengths out
//!   and 4.5 Gyr old, is the Sun's, where the mean is solar;
//! - thick disc −0.55, sigma 0.25; bulge 0.0, 0.40; long bar 0.0, 0.30; nuclear disc +0.1, 0.30;
//! - halo components their own means (in situ −0.6, dominant merger −1.2, lesser progenitors drawn
//!   in −2.0 to −1.0, globular-born debris −1.5), sigma 0.3.

use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::units::{Dex, DexPerKiloparsec, LightYears, Years};

/// The thin discs' age–metallicity slope, dex per Gyr: older stars are poorer (plan 02,
/// P02.T7.e; provisional).
pub const THIN_DISC_AGE_SLOPE: f64 = -0.04;

/// The age at which a thin-disc star at the reference radius is solar: the Sun's, 4.5 Gyr.
pub const THIN_DISC_REFERENCE_AGE: Years = Years::new(4.5 * YEARS_PER_GIGAYEAR);

/// The reference radius of the thin discs' gradient in scale lengths: three, about the Sun's.
pub const THIN_DISC_REFERENCE_LENGTHS: f64 = 3.0;

/// The clamp on the thin discs' mean, dex: the young inner disc stays below +0.5 and the old
/// outer disc above −1.0 (provisional).
pub const THIN_DISC_MEAN_RANGE: (f64, f64) = (-1.0, 0.5);

/// The thin discs' scatter about the mean, dex.
pub const THIN_DISC_SIGMA: Dex = Dex::new(0.15);

/// The thick disc's \[Fe/H\].
pub const THICK_DISC: FehDistribution = FehDistribution::new(Dex::new(-0.55), Dex::new(0.25));

/// The bulge's \[Fe/H\].
pub const BULGE: FehDistribution = FehDistribution::new(Dex::new(0.0), Dex::new(0.40));

/// The long bar's \[Fe/H\].
pub const LONG_BAR: FehDistribution = FehDistribution::new(Dex::new(0.0), Dex::new(0.30));

/// The nuclear disc's \[Fe/H\].
pub const NUCLEAR_DISC: FehDistribution = FehDistribution::new(Dex::new(0.1), Dex::new(0.30));

/// The scatter of every halo component about its own mean, dex.
pub const HALO_SIGMA: Dex = Dex::new(0.3);

/// The distribution of \[Fe/H\] among a component's systems at one position and age: normal with
/// this mean and standard deviation, in dex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FehDistribution {
    mean: Dex,
    sigma: Dex,
}

impl FehDistribution {
    /// A normal distribution of \[Fe/H\] with `mean` and standard deviation `sigma`.
    #[must_use]
    pub const fn new(mean: Dex, sigma: Dex) -> Self {
        Self { mean, sigma }
    }

    /// The mean \[Fe/H\].
    #[must_use]
    pub fn mean(&self) -> Dex {
        self.mean
    }

    /// The standard deviation about the mean.
    #[must_use]
    pub fn sigma(&self) -> Dex {
        self.sigma
    }
}

/// How a component's metallicity depends on position and age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Metallicity {
    /// A thin disc's radial gradient and age–metallicity relation.
    ThinDisc {
        /// Dex per light-year.
        gradient: f64,
        /// The radius at which the gradient term is zero, ly.
        reference_radius: f64,
    },
    /// The same everywhere and at every age.
    Fixed(FehDistribution),
}

impl Metallicity {
    /// A thin disc of scale length `length` with the galaxy's radial `gradient`.
    pub(crate) fn thin_disc(gradient: DexPerKiloparsec, length: LightYears) -> Self {
        Self::ThinDisc {
            gradient: gradient.value() / LIGHT_YEARS_PER_KILOPARSEC,
            reference_radius: THIN_DISC_REFERENCE_LENGTHS * length.value(),
        }
    }

    /// The distribution at cylindrical radius `r_cyl` (ly) for systems of `age`.
    pub(crate) fn at(&self, r_cyl: f64, age: Years) -> FehDistribution {
        match *self {
            Self::ThinDisc {
                gradient,
                reference_radius,
            } => {
                let age_gyr = (age - THIN_DISC_REFERENCE_AGE).value() / YEARS_PER_GIGAYEAR;
                let mean = gradient * (r_cyl - reference_radius) + THIN_DISC_AGE_SLOPE * age_gyr;
                let (lo, hi) = THIN_DISC_MEAN_RANGE;
                FehDistribution::new(Dex::new(mean.clamp(lo, hi)), THIN_DISC_SIGMA)
            }
            Self::Fixed(distribution) => distribution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thin_disc_is_solar_at_the_sun_and_follows_its_gradient_and_age() {
        let disc = Metallicity::thin_disc(DexPerKiloparsec::new(-0.05), LightYears::new(8_480.0));
        let sun = disc.at(3.0 * 8_480.0, THIN_DISC_REFERENCE_AGE);
        assert!(sun.mean().value().abs() < 1e-15);
        assert!((sun.sigma().value() - 0.15).abs() < 1e-15);
        let outer = disc.at(
            3.0 * 8_480.0 + LIGHT_YEARS_PER_KILOPARSEC,
            THIN_DISC_REFERENCE_AGE,
        );
        assert!((outer.mean().value() + 0.05).abs() < 1e-12);
        let older = disc.at(3.0 * 8_480.0, Years::new(9.5e9));
        assert!((older.mean().value() + 0.2).abs() < 1e-12);
        // The clamp holds the extremes.
        assert!((disc.at(0.0, Years::new(-1e3)).mean().value() - 0.5).abs() < 1e-15);
        assert!((disc.at(200_000.0, Years::new(1e10)).mean().value() + 1.0).abs() < 1e-15);
    }

    #[test]
    fn fixed_distributions_ignore_position_and_age() {
        let fixed = Metallicity::Fixed(THICK_DISC);
        assert_eq!(fixed.at(0.0, Years::ZERO), THICK_DISC);
        assert_eq!(fixed.at(50_000.0, Years::new(1.1e10)), THICK_DISC);
    }
}
