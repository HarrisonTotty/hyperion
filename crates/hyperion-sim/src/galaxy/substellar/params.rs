//! The generator version's substellar parameters: the two abundances, the rogue planets' slope and
//! the four band limits (plan 13, P13.T1).
//!
//! Everything here belongs to the generator version: changing an abundance changes every
//! substellar cell's candidate count, and changing a band limit or the slope changes every mass
//! (plan 13, "Generator version").

use crate::math;
use crate::rng::PowerLaw;
use crate::units::consts::{EARTH_MASS_KG, JUPITER_MASS_KG, SOLAR_MASS_KG};
use crate::units::{EarthMasses, JupiterMasses, SolarMasses};

/// The lightest brown dwarf, 13 Jupiter masses: the deuterium-burning limit that divides brown
/// dwarfs from planets (brainstorm, "Between the stars": "13–80 Jupiter masses"; ruling 42 of
/// 2026-09-22). It is also the heaviest rogue planet.
pub const BROWN_DWARF_MIN: JupiterMasses = JupiterMasses::new(13.0);

/// The heaviest brown dwarf, 0.08 M☉: the stellar layers' lower edge, plan 02's
/// [`MASS_LIMIT_LO`](crate::galaxy::imf::MASS_LIMIT_LO), so that no mass falls between the layers.
/// The brainstorm's "80 Jupiter masses" (0.0764 M☉) is read as its rounding (plan 13, Design note
/// 5).
pub const BROWN_DWARF_MAX: SolarMasses = SolarMasses::new(crate::galaxy::imf::MASS_LIMIT_LO);

/// The lightest rogue planet, a third of an Earth mass: the lower end of Sumi et al.'s (2023, AJ
/// 166, 108) measured mass function, 0.33 M⊕.
pub const ROGUE_PLANET_MIN: EarthMasses = EarthMasses::new(1.0 / 3.0);

/// The heaviest rogue planet, 13 Jupiter masses, where the brown dwarfs begin.
pub const ROGUE_PLANET_MAX: JupiterMasses = BROWN_DWARF_MIN;

/// [`BROWN_DWARF_MIN`] in solar masses, 0.01241: the same bits as `SolarMasses::from` gives, since
/// both multiply by the Jovian mass and then divide by the solar.
pub(crate) const BROWN_DWARF_MIN_MSUN: f64 =
    BROWN_DWARF_MIN.value() * JUPITER_MASS_KG / SOLAR_MASS_KG;

/// [`ROGUE_PLANET_MIN`] in solar masses, 1.001 × 10⁻⁶, bit for bit as `SolarMasses::from`.
pub(crate) const ROGUE_PLANET_MIN_MSUN: f64 =
    ROGUE_PLANET_MIN.value() * EARTH_MASS_KG / SOLAR_MASS_KG;

/// [`ROGUE_PLANET_MAX`] in solar masses.
pub(crate) const ROGUE_PLANET_MAX_MSUN: f64 = BROWN_DWARF_MIN_MSUN;

/// The pivot of Sumi et al.'s (2023) mass function, 8 M⊕: dN ÷ dlog₁₀ M = Z (M ÷ 8 M⊕)^−α.
const ROGUE_PIVOT_EARTH_MASSES: f64 = 8.0;

/// The generator version's substellar parameters.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::substellar::SubstellarParams;
///
/// let p = SubstellarParams::generator_default();
/// // One free-floating brown dwarf for every five and a half stars, and 21 rogue planets per star.
/// assert!((p.brown_dwarfs_per_star() - 1.0 / 5.5).abs() < 1e-15);
/// assert!((p.rogue_planets_per_star() - 21.0).abs() < 1e-15);
/// // Sumi et al.'s normalisation at 8 M⊕, 2.18 per dex per star, follows from the count.
/// assert!((p.rogue_planets_per_dex_at_pivot() / 2.18 - 1.0).abs() < 0.02);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubstellarParams {
    brown_dwarfs_per_star: f64,
    rogue_planets_per_star: f64,
    rogue_mass_slope: f64,
}

impl SubstellarParams {
    /// This generator version's values.
    ///
    /// - One free-floating brown dwarf per 5.5 stars (brainstorm, "Between the stars": "one for
    ///   every five or six stars").
    /// - 21 rogue planets per star from ⅓ M⊕ to 13 `M_Jup`, with a slope of 0.96 in
    ///   dN ÷ dlog₁₀ M: Sumi et al. (2023, AJ 166, 108), 21 (+23, −13) per star and
    ///   α₄ = 0.96 (+0.47, −0.27), from the MOA-II survey towards the bulge. Sumi's 21 counts
    ///   from 0.33 M⊕ (two thirds of it below 1 M⊕; about 7 per star from 1 M⊕ up), per star
    ///   including brown dwarfs, so per hydrogen-burning star it is about 26–27. His 53 per M☉
    ///   gives 21.3 per star at the model's 0.55–0.59 M☉ per system, so the default matches like
    ///   for like (ruling 125).
    #[must_use]
    pub const fn generator_default() -> Self {
        Self {
            brown_dwarfs_per_star: 1.0 / 5.5,
            rogue_planets_per_star: 21.0,
            rogue_mass_slope: 0.96,
        }
    }

    /// Free-floating brown dwarfs per star, 1 ÷ 5.5.
    #[must_use]
    pub const fn brown_dwarfs_per_star(&self) -> f64 {
        self.brown_dwarfs_per_star
    }

    /// Rogue planets per star over the whole band, before the per-galaxy cap: 21.
    #[must_use]
    pub const fn rogue_planets_per_star(&self) -> f64 {
        self.rogue_planets_per_star
    }

    /// The rogue planets' slope α in dN ÷ dlog₁₀ M ∝ M^−α, 0.96: dN ÷ dM ∝ M^−(1 + α).
    #[must_use]
    pub const fn rogue_mass_slope(&self) -> f64 {
        self.rogue_mass_slope
    }

    /// The same parameters with another rogue-planet abundance per star, for tests and tuning.
    ///
    /// # Panics
    ///
    /// If `n` is negative or not finite: an abundance is a count.
    #[must_use]
    pub fn with_rogue_planets_per_star(self, n: f64) -> Self {
        assert!(
            n.is_finite() && n >= 0.0,
            "a rogue-planet abundance is a finite count, not {n}"
        );
        Self {
            rogue_planets_per_star: n,
            ..self
        }
    }

    /// The normalisation Z of dN ÷ dlog₁₀ M = Z (M ÷ 8 M⊕)^−α, per dex per star, derived from the
    /// abundance, the slope and the band (plan 13, Design note 7): 2.20 at the defaults, against
    /// Sumi et al.'s 2.18 (+0.52, −1.40).
    #[must_use]
    pub fn rogue_planets_per_dex_at_pivot(&self) -> f64 {
        let alpha = self.rogue_mass_slope;
        let (lo, hi) = rogue_pivot_ratios();
        self.rogue_planets_per_star * alpha * core::f64::consts::LN_10
            / (math::powf(lo, -alpha) - math::powf(hi, -alpha))
    }

    /// The rogue planets' mass law in solar masses, a density ∝ M^−(1 + α) on
    /// [`ROGUE_PLANET_MIN`]–[`ROGUE_PLANET_MAX`], which [`Stream::power_law`](crate::rng::Stream)
    /// draws.
    ///
    /// # Panics
    ///
    /// Never for a slope this module builds: the band is positive and ordered, and a power law of
    /// finite exponent over it is normalisable.
    #[must_use]
    pub fn rogue_mass_law(&self) -> PowerLaw {
        PowerLaw::new(
            1.0 + self.rogue_mass_slope,
            ROGUE_PLANET_MIN_MSUN,
            ROGUE_PLANET_MAX_MSUN,
        )
        .expect("the rogue-planet band is positive and ordered, and its slope finite")
    }
}

impl Default for SubstellarParams {
    fn default() -> Self {
        Self::generator_default()
    }
}

/// The band's two limits over Sumi et al.'s pivot of 8 M⊕.
fn rogue_pivot_ratios() -> (f64, f64) {
    let hi = EarthMasses::from(ROGUE_PLANET_MAX).value();
    (
        ROGUE_PLANET_MIN.value() / ROGUE_PIVOT_EARTH_MASSES,
        hi / ROGUE_PIVOT_EARTH_MASSES,
    )
}

/// How many rogue planets per star lie above `m` (the abundance before any cap): the closed-form
/// integral of the mass function from `m` to the band's top, clamped to the band.
pub(crate) fn rogue_planets_above_per_star(p: &SubstellarParams, m: EarthMasses) -> f64 {
    let alpha = p.rogue_mass_slope();
    let (lo, hi) = rogue_pivot_ratios();
    let x = (m.value() / ROGUE_PIVOT_EARTH_MASSES).clamp(lo, hi);
    let above = math::powf(x, -alpha) - math::powf(hi, -alpha);
    let all = math::powf(lo, -alpha) - math::powf(hi, -alpha);
    p.rogue_planets_per_star() * above / all
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn the_band_limits_in_solar_masses_are_the_conversions_bits() {
        assert_same_bits(
            BROWN_DWARF_MIN_MSUN,
            SolarMasses::from(BROWN_DWARF_MIN).value(),
        );
        assert_same_bits(
            ROGUE_PLANET_MIN_MSUN,
            SolarMasses::from(ROGUE_PLANET_MIN).value(),
        );
        assert!((0.012_40..0.012_42).contains(&BROWN_DWARF_MIN_MSUN));
        assert!((1.0e-6..1.002e-6).contains(&ROGUE_PLANET_MIN_MSUN));
        // 13 M_Jup is 4,131 M⊕, the heaviest rogue planet's readout (plan 13, Design note 14).
        let top = EarthMasses::from(ROGUE_PLANET_MAX).value();
        assert!((4_131.0..4_132.0).contains(&top), "{top}");
    }

    /// Design note 7: with Z derived from 21 per star, the value at 8 M⊕ is Sumi et al.'s 2.18 per
    /// dex to 2%.
    #[test]
    fn the_derived_normalisation_is_sumi_s() {
        let z = SubstellarParams::generator_default().rogue_planets_per_dex_at_pivot();
        assert!((z / 2.18 - 1.0).abs() < 0.02, "Z = {z}");
    }

    #[test]
    fn the_count_above_runs_from_the_abundance_to_zero() {
        let p = SubstellarParams::generator_default();
        assert!((rogue_planets_above_per_star(&p, ROGUE_PLANET_MIN) - 21.0).abs() < 1e-12);
        assert!((rogue_planets_above_per_star(&p, EarthMasses::new(0.01)) - 21.0).abs() < 1e-12);
        assert_same_bits(
            rogue_planets_above_per_star(&p, EarthMasses::from(ROGUE_PLANET_MAX)),
            0.0,
        );
    }

    #[test]
    #[should_panic(expected = "a rogue-planet abundance is a finite count")]
    fn a_negative_abundance_is_refused() {
        let _ = SubstellarParams::generator_default().with_rogue_planets_per_star(-1.0);
    }
}
