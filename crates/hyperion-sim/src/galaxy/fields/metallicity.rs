//! Metallicity as a distribution of \[Fe/H\] per component, position and age (plan 02, Design
//! note 14 and P02.T7.e).
//!
//! The brainstorm's "Fields": metallicity "falls with galactic radius (about −0.05 dex per kpc in
//! the Milky Way disc) and, beyond about 8 Gyr, with age, with scatter". In the thin disc the mean
//! at a given radius is flat for ages up to 8 Gyr and then falls about 0.1 dex per Gyr, with a
//! scatter of 0.20 dex at every age (brainstorm, Decisions, "2026-09-21: local density rulings",
//! 6). The system stage (plan 06) draws a system's \[Fe/H\] from the distribution returned here;
//! nothing is drawn in this module.
//!
//! The means and scatters, the thin discs' from the rulings and the rest plan 02's (P02.T7.e),
//! provisional and marked there for re-checking against Bland-Hawthorn and Gerhard (2016, ARA&A
//! 54, 529):
//!
//! - thin discs, young and old: mean `gradient × (R − 3 lengths) − 0.1 dex per Gyr × max(0, age − 8
//!   Gyr)`, clamped to [−1.0, +0.5], sigma 0.20. Gaia-ESO finds the relation "nearly flat" for 0–8
//!   Gyr, falling beyond 9 Gyr, with "a significant scatter of \[Fe/H\] at any age" (Bergemann et
//!   al. 2014, A&A 565, A89, §5); the slope beyond is read from their Fig. 6, which gives no
//!   number. The scatter is the width of the Geneva–Copenhagen survey's local distribution over
//!   every age, σ 0.22 and half its FWHM 0.19 (Casagrande et al. 2011, A&A 530, A138, Table 1),
//!   held at every age as the rulings have it, although the survey finds young stars' narrower.
//!   The flat part is solar three scale lengths out, about the Sun's radius, as the youngest local
//!   stars and the gas are: Fe 7.52 ± 0.03 against the Sun's 7.50 ± 0.04 (Nieva and Przybilla
//!   2012, A&A 539, A143, Table 7). With the declining formation history the local mean over every
//!   age then comes to −0.03 to −0.04, against the survey's −0.06 over its own, differently
//!   weighted, sample. The clamp is plan 02's;
//! - thick disc −0.55, sigma 0.25; bulge 0.0, 0.40; long bar 0.0, 0.30; nuclear disc +0.1, 0.30;
//! - halo components their own means (in situ −0.6, dominant merger −1.2, lesser progenitors drawn
//!   in −2.0 to −1.0, globular-born debris −1.5), sigma 0.3.

use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::units::{Dex, DexPerKiloparsec, LightYears, Years};

/// The thin discs' age–metallicity slope beyond [`THIN_DISC_FLAT_AGE`], dex per Gyr: the oldest
/// stars are poorer (read from Bergemann et al. 2014, Fig. 6; module documentation).
pub const THIN_DISC_AGE_SLOPE: f64 = -0.1;

/// The age up to which the thin discs' mean \[Fe/H\] does not depend on age (Bergemann et al.
/// 2014: "nearly flat" for 0–8 Gyr).
pub const THIN_DISC_FLAT_AGE: Years = Years::new(8.0 * YEARS_PER_GIGAYEAR);

/// The reference radius of the thin discs' gradient in scale lengths: three, about the Sun's,
/// where the flat part of the mean is solar.
pub const THIN_DISC_REFERENCE_LENGTHS: f64 = 3.0;

/// The clamp on the thin discs' mean, dex: the young inner disc stays below +0.5 and the old
/// outer disc above −1.0 (provisional).
pub const THIN_DISC_MEAN_RANGE: (f64, f64) = (-1.0, 0.5);

/// The thin discs' scatter about the mean at every age, dex: the local distribution's width over
/// every age (Casagrande et al. 2011, Table 1: σ 0.22, FWHM/2 0.19).
pub const THIN_DISC_SIGMA: Dex = Dex::new(0.20);

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
    /// A thin disc's radial gradient and age–metallicity relation, flat to 8 Gyr.
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
                let beyond = (age - THIN_DISC_FLAT_AGE).value().max(0.0) / YEARS_PER_GIGAYEAR;
                let mean = gradient * (r_cyl - reference_radius) + THIN_DISC_AGE_SLOPE * beyond;
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
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        // Flat to 8 Gyr, and solar there at the reference radius.
        for age in [-1e3, 0.0, 0.05, 4.5, 8.0] {
            let sun = disc.at(3.0 * 8_480.0, gyr(age));
            assert!(sun.mean().value().abs() < 1e-15, "{age} Gyr");
            assert!((sun.sigma().value() - 0.20).abs() < 1e-15);
        }
        let outer = disc.at(3.0 * 8_480.0 + LIGHT_YEARS_PER_KILOPARSEC, gyr(3.0));
        assert!((outer.mean().value() + 0.05).abs() < 1e-12);
        // Then 0.1 dex poorer for every gigayear beyond.
        let older = disc.at(3.0 * 8_480.0, gyr(9.5));
        assert!((older.mean().value() + 0.15).abs() < 1e-12);
        let oldest = disc.at(3.0 * 8_480.0, gyr(10.0));
        assert!((oldest.mean().value() + 0.2).abs() < 1e-12);
        // The clamp holds the extremes: the steepest gradient in the longest disc reaches +0.74
        // at its centre.
        let steep = Metallicity::thin_disc(DexPerKiloparsec::new(-0.07), LightYears::new(11_500.0));
        assert!((steep.at(0.0, Years::new(-1e3)).mean().value() - 0.5).abs() < 1e-15);
        assert!((disc.at(200_000.0, Years::new(1e10)).mean().value() + 1.0).abs() < 1e-15);
    }

    #[test]
    fn fixed_distributions_ignore_position_and_age() {
        let fixed = Metallicity::Fixed(THICK_DISC);
        assert_eq!(fixed.at(0.0, Years::ZERO), THICK_DISC);
        assert_eq!(fixed.at(50_000.0, Years::new(1.1e10)), THICK_DISC);
    }
}
