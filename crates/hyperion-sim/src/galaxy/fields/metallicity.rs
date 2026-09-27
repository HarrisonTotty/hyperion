//! Metallicity as a distribution of \[Fe/H\] per component, position and age (plan 02, Design
//! note 14 and P02.T7.e).
//!
//! The brainstorm's "Fields": metallicity "falls with galactic radius (about −0.05 dex per kpc in
//! the Milky Way disc) and, beyond about 8 Gyr, with age, with scatter". The decline with age
//! belongs to the thick disc, not the thin: Gaia-ESO finds the thin disc's age–metallicity relation
//! "nearly flat" for 0–8 Gyr and the decline among the older, α-enhanced stars of the thick disc
//! (Bergemann et al. 2014, A&A 565, A89; ruling 7 of 2026-09-22, built by ruling 42.5 in plan 02's
//! P02.T12.c). So the thin discs' mean at a given radius is flat at every age, with a scatter of
//! 0.20 dex, and the thick disc's falls 0.2 dex per Gyr (ruling 106.3). The system stage (plan 06)
//! draws a system's \[Fe/H\] from the distribution returned here; nothing is drawn in this module.
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
//! - thick disc: −0.5 at its mean age of 11 Gyr, falling 0.2 dex per Gyr of age, sigma 0.20 at
//!   every age (ruling 106.3, revising ruling 76.7's 0.1 dex per Gyr, −0.55 and 0.25). The slope is
//!   the sources' middle: Haywood et al. (2013, A&A 560, A109, §4) give about 0.15 dex per Gyr for
//!   the thick-disc phase, Bensby, Feltzing and Oey (2014, A&A 562, A71, Conclusion 2) at least 0.2
//!   ("from ∼10 Gyr below \[Fe/H\] < −0.4 to around 8 Gyr at \[Fe/H\] ≈ 0"), and Xiang and Rix
//!   (2022, Nature 603, 599) about 0.25 (1.5 dex over 13 to 7 Gyr ago). Haywood's and Xiang and Rix's
//!   relations, made linear, both give −0.5 at 11 Gyr, and Kordopatis et al. (2011, A&A 535, A107,
//!   Conclusions) measure the thick disc's mean at −0.45, "a canonical thick disc metallicity of
//!   −0.5 dex" (§7). The scatter at fixed age is under Xiang and Rix's upper limit of 0.22 dex, and
//!   Haywood calls the relation "tight". Over the uniform 10–12 Gyr the population keeps its mean of
//!   −0.5, and its whole spread is √(0.20² + 0.4²/12) = 0.23 dex: a distribution centred at −0.5
//!   whose tails reach about −1.2 (its 0.1% point) and solar (1.5% lie above), as Kordopatis et al.'s does, although
//!   their metal-poor tail runs on to −1.8, which a normal at every age does not reach;
//!   bulge 0.0, 0.40; long bar 0.0, 0.30; nuclear disc +0.1, 0.30;
//! - halo components their own means (in situ −0.6, dominant merger −1.2, lesser progenitors drawn
//!   in −2.0 to −1.0, globular-born debris −1.5), sigma 0.3.

use super::SOLAR_RADIUS_LENGTHS;
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::units::{Dex, DexPerKiloparsec, LightYears, Years};

/// The thick disc's age–metallicity slope, dex per Gyr: its older stars are poorer. The figure is
/// ruling 106.3's, between Haywood et al.'s (2013) 0.15, Bensby et al.'s (2014) at least 0.2 and
/// Xiang and Rix's (2022) about 0.25 (module documentation). It was 0.1, the brainstorm's decline
/// beyond 8 Gyr (ruling 76.7), until the batch after version 12, and until version 12 it acted on
/// the thin discs beyond 8 Gyr (ruling 42.5 of 2026-09-22).
pub const THICK_DISC_AGE_SLOPE_DEX_PER_GYR: f64 = -0.2;

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

/// The thick disc's \[Fe/H\] at [`THICK_DISC_MEAN_AGE`], and so the mean over its whole
/// population: −0.5 (Haywood et al. 2013 and Xiang and Rix 2022, made linear; Kordopatis et al.
/// 2011's −0.45), with a scatter at every age of 0.20 dex, under Xiang and Rix's upper limit of
/// 0.22 (ruling 106.3).
pub const THICK_DISC: FehDistribution = FehDistribution::new(Dex::new(-0.5), Dex::new(0.20));

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

    /// The thick disc is −0.5 at its mean age and 0.2 dex poorer per Gyr older, everywhere, with a
    /// scatter of 0.20 dex at every age (ruling 106.3); over its uniform 10–12 Gyr its mean stays
    /// −0.5.
    #[test]
    fn the_thick_disc_falls_with_age_about_its_mean() {
        let thick = Metallicity::ThickDisc;
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        let at = |r: f64, t: f64| thick.at(r, gyr(t)).mean().value();
        assert!((at(26_000.0, 11.0) + 0.5).abs() < 1e-15);
        assert!((at(0.0, 10.0) + 0.3).abs() < 1e-12);
        assert!((at(60_000.0, 12.0) + 0.7).abs() < 1e-12);
        // Bensby et al. (2014, Conclusion 2): at least 0.4 dex over 2 Gyr, from about 10 to 8 Gyr.
        assert!(at(8_000.0, 10.0) - at(8_000.0, 12.0) >= 0.4 - 1e-12);
        for t in [10.0, 10.5, 11.0, 12.0] {
            let sigma = thick.at(1.0, gyr(t)).sigma().value();
            assert!((sigma - 0.20).abs() < 1e-15, "{t} Gyr");
            // Xiang and Rix (2022): under 0.22 dex at a given age.
            assert!(sigma < 0.22);
        }
        let [lo, hi] = crate::galaxy::ages::THICK_DISC_AGES.map(|t| t.value() / YEARS_PER_GIGAYEAR);
        assert!(
            (f64::midpoint(lo, hi) - THICK_DISC_MEAN_AGE.value() / YEARS_PER_GIGAYEAR).abs()
                < 1e-12
        );
    }

    /// The thick disc's whole metallicity distribution, the field's normal at each age mixed over
    /// its uniform 10–12 Gyr by the midpoint rule, against the sources (research notes to ruling
    /// 106): centred at −0.5 (Kordopatis et al. 2011's mean −0.45, "canonical" −0.5), a spread of
    /// about 0.23 dex (√(0.20² + 0.4²/12)), its 0.1% point near −1.2 and its metal-rich tail
    /// reaching solar with a small share (Kordopatis et al.: "up to solar and super-solar values").
    #[test]
    fn the_thick_discs_distribution_is_centred_at_minus_half_with_tails_to_minus_1_2_and_solar() {
        let thick = Metallicity::ThickDisc;
        let [lo, hi] = crate::galaxy::ages::THICK_DISC_AGES.map(Years::value);
        let n = 4_000_u32;
        let fields: Vec<FehDistribution> = (0..n)
            .map(|i| {
                let age = lo + (hi - lo) * (f64::from(i) + 0.5) / f64::from(n);
                thick.at(26_000.0, Years::new(age))
            })
            .collect();
        let count = f64::from(n);
        let mean = fields.iter().map(|f| f.mean().value()).sum::<f64>() / count;
        let variance = fields
            .iter()
            .map(|f| {
                let d = f.mean().value() - mean;
                f.sigma().value() * f.sigma().value() + d * d
            })
            .sum::<f64>()
            / count;
        let spread = variance.sqrt();
        let cdf = |x: f64| {
            fields
                .iter()
                .map(|f| {
                    let z = (x - f.mean().value()) / f.sigma().value();
                    0.5 * crate::math::erfc(-z / core::f64::consts::SQRT_2)
                })
                .sum::<f64>()
                / count
        };
        let quantile = |p: f64| {
            let (mut a, mut b) = (-3.0_f64, 2.0_f64);
            for _ in 0..60 {
                let m = f64::midpoint(a, b);
                if cdf(m) < p {
                    a = m;
                } else {
                    b = m;
                }
            }
            f64::midpoint(a, b)
        };
        let (poor, rich) = (quantile(0.001), quantile(0.999));
        let above_solar = 1.0 - cdf(0.0);
        assert!((mean + 0.5).abs() < 1e-9, "mean {mean}");
        let expected = (0.20_f64 * 0.20 + 0.4 * 0.4 / 12.0).sqrt();
        assert!((spread - expected).abs() < 1e-6, "spread {spread}");
        assert!((0.225..=0.235).contains(&spread), "spread {spread}");
        assert!((poor + 1.2).abs() < 0.05, "0.1% point {poor}");
        assert!((rich - 0.2).abs() < 0.05, "99.9% point {rich}");
        assert!(
            (0.005..=0.05).contains(&above_solar),
            "above solar {above_solar}"
        );
        let within = cdf(0.0) - cdf(-1.2);
        assert!(within > 0.98, "{within} between −1.2 and solar");
    }

    #[test]
    fn fixed_distributions_ignore_position_and_age() {
        let fixed = Metallicity::Fixed(BULGE);
        assert_eq!(fixed.at(0.0, Years::ZERO), BULGE);
        assert_eq!(fixed.at(50_000.0, Years::new(1.1e10)), BULGE);
    }
}
