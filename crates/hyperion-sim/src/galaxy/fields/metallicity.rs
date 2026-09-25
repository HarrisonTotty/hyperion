//! Metallicity as a distribution of \[Fe/H\] per component, position and age (plan 02, Design
//! note 14 and P02.T7.e).
//!
//! The brainstorm's "Fields": metallicity "falls with galactic radius (about −0.05 dex per kpc in
//! the Milky Way disc) and, beyond about 8 Gyr, with age, with scatter". The decline with age
//! belongs to the thick disc, not the thin: Gaia-ESO finds the thin disc's age–metallicity relation
//! "nearly flat" for 0–8 Gyr and the decline among the older, α-enhanced stars of the thick disc
//! (Bergemann et al. 2014, A&A 565, A89; ruling 7 of 2026-09-22, built by ruling 42.5 in plan 02's
//! P02.T12.c). So the thin discs' mean at a given radius is flat at every age, with a scatter of
//! 0.20 dex, and the thick disc's falls about 0.1 dex per Gyr. The system stage (plan 06) draws a
//! system's \[Fe/H\] from the distribution returned here; nothing is drawn in this module.
//!
//! The means and scatters, the thin discs' from the rulings and the rest plan 02's (P02.T7.e),
//! provisional and marked there for re-checking against Bland-Hawthorn and Gerhard (2016, ARA&A
//! 54, 529):
//!
//! - thin discs, young and old: mean `gradient × (R − 3.8 lengths)`, clamped to [−1.0, +0.5],
//!   sigma 0.20, at every age. Gaia-ESO finds the relation "nearly flat" for 0–8 Gyr with "a
//!   significant scatter of \[Fe/H\] at any age" (Bergemann et al. 2014, §5). The flat part is
//!   radial migration seen at a fixed radius. The scatter is the width of the Geneva–Copenhagen
//!   survey's local distribution over every age, σ 0.22 and half its FWHM 0.19 (Casagrande et al.
//!   2011, A&A 530, A138, Table 1), held at every age as the rulings have it, although the survey
//!   finds young stars' narrower. The mean is solar at the Sun's radius, as the youngest local
//!   stars and the gas are: Fe 7.52 ± 0.03 against the Sun's 7.50 ± 0.04 (Nieva and Przybilla
//!   2012, A&A 539, A143, Table 7). The Sun's radius is taken in units of the thin disc's scale
//!   length, the Milky Way's R₀ ÷ `R_d` = 3.8 ([`SOLAR_RADIUS_LENGTHS`]), because discs' gradients
//!   are self-similar in those units, so every drawn disc is solar at its own solar circle. On the
//!   fixture's 7,000 ly disc that is 26,600 ly. The clamp is plan 02's;
//! - thick disc: −0.55 at its mean age of 11 Gyr, falling 0.1 dex per Gyr of age, sigma 0.25, so
//!   its whole population keeps its mean of −0.55. The slope is the brainstorm's decline beyond
//!   8 Gyr (Fields, "Metallicity", citing Bergemann et al. 2014), which ruling 42.5 moved from the
//!   thin discs to the thick disc; the −0.55 is plan 02's, and no paper checked gives it at 11 Gyr.
//!   Bensby et al. (2014) imply about 0.2 dex per Gyr, which is left to the owner (ruling 76.7); bulge 0.0, 0.40; long bar 0.0, 0.30; nuclear disc +0.1, 0.30;
//! - halo components their own means (in situ −0.6, dominant merger −1.2, lesser progenitors drawn
//!   in −2.0 to −1.0, globular-born debris −1.5), sigma 0.3.

use super::SOLAR_RADIUS_LENGTHS;
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::units::{Dex, DexPerKiloparsec, LightYears, Years};

/// The thick disc's age–metallicity slope, dex per Gyr: its older stars are poorer. The figure is
/// the brainstorm's decline beyond 8 Gyr (module documentation; ruling 76.7). Until version 12 it acted on the thin
/// discs beyond 8 Gyr (ruling 42.5 of 2026-09-22).
pub const THICK_DISC_AGE_SLOPE_DEX_PER_GYR: f64 = -0.1;

/// The age at which the thick disc's mean is [`THICK_DISC`]'s: the middle of its uniform 10–12 Gyr
/// ([`THICK_DISC_AGES`](crate::galaxy::ages::THICK_DISC_AGES)), so that the slope moves no mean of
/// the whole population.
pub const THICK_DISC_MEAN_AGE: Years = Years::new(11.0 * YEARS_PER_GIGAYEAR);

/// The clamp on the thin discs' mean, dex: the young inner disc stays below +0.5 and the old
/// outer disc above −1.0 (provisional).
pub const THIN_DISC_MEAN_RANGE: (f64, f64) = (-1.0, 0.5);

/// The thin discs' scatter about the mean at every age, dex: the local distribution's width over
/// every age (Casagrande et al. 2011, Table 1: σ 0.22, FWHM/2 0.19).
pub const THIN_DISC_SIGMA: Dex = Dex::new(0.20);

/// The thick disc's \[Fe/H\] at [`THICK_DISC_MEAN_AGE`], and so over its whole population.
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
    /// A thin disc's radial gradient, flat in age.
    ThinDisc {
        /// Dex per light-year.
        gradient: f64,
        /// The radius at which the gradient term is zero, ly.
        reference_radius: f64,
    },
    /// The thick disc's: [`THICK_DISC`] at [`THICK_DISC_MEAN_AGE`], falling with age by
    /// [`THICK_DISC_AGE_SLOPE_DEX_PER_GYR`], the same everywhere.
    ThickDisc,
    /// The same everywhere and at every age.
    Fixed(FehDistribution),
}

impl Metallicity {
    /// A thin disc of scale length `length` with the galaxy's radial `gradient`.
    pub(crate) fn thin_disc(gradient: DexPerKiloparsec, length: LightYears) -> Self {
        Self::ThinDisc {
            gradient: gradient.value() / LIGHT_YEARS_PER_KILOPARSEC,
            reference_radius: SOLAR_RADIUS_LENGTHS * length.value(),
        }
    }

    /// The distribution at cylindrical radius `r_cyl` (ly) for systems of `age`.
    pub(crate) fn at(&self, r_cyl: f64, age: Years) -> FehDistribution {
        match *self {
            Self::ThinDisc {
                gradient,
                reference_radius,
            } => {
                let mean = gradient * (r_cyl - reference_radius);
                let (lo, hi) = THIN_DISC_MEAN_RANGE;
                FehDistribution::new(Dex::new(mean.clamp(lo, hi)), THIN_DISC_SIGMA)
            }
            Self::ThickDisc => {
                let older = (age - THICK_DISC_MEAN_AGE).value() / YEARS_PER_GIGAYEAR;
                FehDistribution::new(
                    THICK_DISC.mean() + Dex::new(THICK_DISC_AGE_SLOPE_DEX_PER_GYR * older),
                    THICK_DISC.sigma(),
                )
            }
            Self::Fixed(distribution) => distribution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thin_disc_is_solar_at_the_sun_follows_its_gradient_and_is_flat_in_age() {
        let disc = Metallicity::thin_disc(DexPerKiloparsec::new(-0.05), LightYears::new(7_000.0));
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        // The anchor is R₀ ÷ R_d scale lengths out, 26,600 ly on a 7,000 ly disc.
        let sun = 26_600.0;
        assert!((SOLAR_RADIUS_LENGTHS * 7_000.0 - sun).abs() < 1e-9);
        // Flat at every age, the oldest included (ruling 42.5), and solar at the anchor.
        for age in [-1e3, 0.0, 0.05, 4.5, 8.0, 9.5, 10.0] {
            let at_sun = disc.at(sun, gyr(age));
            assert!(at_sun.mean().value().abs() < 1e-15, "{age} Gyr");
            assert!((at_sun.sigma().value() - 0.20).abs() < 1e-15);
        }
        let outer = disc.at(sun + LIGHT_YEARS_PER_KILOPARSEC, gyr(9.0));
        assert!((outer.mean().value() + 0.05).abs() < 1e-12);
        // The clamp holds the extremes: the steepest gradient in the longest disc reaches +0.94
        // at its centre.
        let steep = Metallicity::thin_disc(DexPerKiloparsec::new(-0.07), LightYears::new(11_500.0));
        assert!((steep.at(0.0, Years::new(-1e3)).mean().value() - 0.5).abs() < 1e-15);
        assert!((disc.at(200_000.0, Years::new(1e10)).mean().value() + 1.0).abs() < 1e-15);
    }

    /// The thick disc is −0.55 at its mean age and 0.1 dex poorer per Gyr older, everywhere; over
    /// its uniform 10–12 Gyr its mean stays −0.55.
    #[test]
    fn the_thick_disc_falls_with_age_about_its_mean() {
        let thick = Metallicity::ThickDisc;
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        let at = |r: f64, t: f64| thick.at(r, gyr(t)).mean().value();
        assert!((at(26_000.0, 11.0) + 0.55).abs() < 1e-15);
        assert!((at(0.0, 10.0) + 0.45).abs() < 1e-12);
        assert!((at(60_000.0, 12.0) + 0.65).abs() < 1e-12);
        assert!((thick.at(1.0, gyr(10.5)).sigma().value() - 0.25).abs() < 1e-15);
        let [lo, hi] = crate::galaxy::ages::THICK_DISC_AGES.map(|t| t.value() / YEARS_PER_GIGAYEAR);
        assert!(
            (f64::midpoint(lo, hi) - THICK_DISC_MEAN_AGE.value() / YEARS_PER_GIGAYEAR).abs()
                < 1e-12
        );
    }

    #[test]
    fn fixed_distributions_ignore_position_and_age() {
        let fixed = Metallicity::Fixed(BULGE);
        assert_eq!(fixed.at(0.0, Years::ZERO), BULGE);
        assert_eq!(fixed.at(50_000.0, Years::new(1.1e10)), BULGE);
    }
}
