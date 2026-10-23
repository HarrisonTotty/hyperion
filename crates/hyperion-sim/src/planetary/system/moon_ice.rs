//! A moon's bulk ice by its birthplace and its history (plan 14, P14.T56.f;
//! decision-p14-t54a-ganymede §C3.2; decision-bump-plan-after-22, "Rulings before T53.a lands"
//! R1.6 and "p14-gate-24's questions" G2, with main's corrections of 2026-10-22).
//!
//! Two arms read a moon's stated or derived bulk, both held at 24 ([`MOON_BULK_VERSION`]):
//!
//! - **The birthplace arm** (a capture's): a capture formed inside its parent's snow line by
//!   `MoonBirth::formed`, the rule its crust reads (the Phobos and Deimos rule), is rock, its
//!   1,700 kg m⁻³ read as a rubble pile's porosity; one formed beyond keeps the ice its density
//!   holds.
//! - **The retention arm** (every moon's, after the birthplace arm): a bulk's ice is kept only where
//!   it survives its time above ice's melting point ([`kept_water`]).
//!
//! The record holds the moon's mass, so a stripped bulk's lost ice is not taken from it, and its
//! radius stays, its density read as porosity (recorded for P14.T56.g).

use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature, luminosity_flux};
use crate::planetary::hooks::figures::cover::UNDER_ICE_MELTING_POINT;
use crate::planetary::hooks::figures::interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{EarthFluxes, Kelvin, Metres, SolarLuminosities, Years};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a moon's bulk reads its birthplace and its ice's history
/// (P14.T56.f): 24, the 23 → 24 batch's bump, P14.T54.b, before the serve gate
/// (decision-bump-plan-after-22 §6).
///
/// Below it [`MoonBulkRules::in_force`] is [`MoonBulkRules::Held`], so that every moon's bulk is
/// 23's bit for bit. P14.T54.b deletes this hold, [`MoonBulkRules`] with its `Held` arm and the
/// tests' switch, and its tripwire (`the_moon_bulk_is_held_only_until_the_24_bump`).
pub(crate) const MOON_BULK_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether a moon's bulk reads its birthplace and its ice's history (the hold,
/// [`MOON_BULK_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MoonBulkRules {
    /// Versions 23's and before: a capture's bulk holds the ice its density holds, wherever it
    /// formed, and no bulk loses its ice.
    Held,
    /// The 23 → 24 batch's: the birthplace arm and the retention arm.
    Batch,
}

impl MoonBulkRules {
    /// The rules this generator version gives: the batch's from [`MOON_BULK_VERSION`], 23's below
    /// it; in the tests, the rules a test's switch sets on its thread ([`with_moon_bulk`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = MOON_BULK_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= MOON_BULK_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The moon bulk's rules a test sets on its thread, `None` for the version's.
    static MOON_BULK_RULES: core::cell::Cell<Option<MoonBulkRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the moon bulk's rules `rules` on this thread, for the tests to reach 24's
/// bulks ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// P14.T54.b deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_moon_bulk<R>(rules: MoonBulkRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<MoonBulkRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MOON_BULK_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(MOON_BULK_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The temperature above which a bulk loses its ice as fast as heat reaches it: ice's melting
/// point, [`UNDER_ICE_MELTING_POINT`]'s 273.15 K (0 °C; ice Ih melts at 273.152 519 K under
/// 101,325 Pa and at the triple point, 273.16 K under 611.657 Pa; IAPWS R10-06(2009), Tables 1
/// and 6).
///
/// Not Schorghofer 2008's 145 K (ApJ 682, 697): that is the limit of ice in the top few metres
/// under a dusty lag ("the shallowly buried snow line"), and his deeply buried line lies near
/// 170 K or above. Below the melting point a bulk keeps its ice under its lag (Prialnik and
/// Rosenberg 2009, MNRAS 399, L79: crystalline ice survives at about 50–150 m for 4.6 Gyr), and
/// that lag's loss is P14.T56.g's. Above it the heated ice leaves as vapour at 611 Pa or more, so
/// the loss is limited by the heat. Vapour pressure is continuous through it, so the limit is a
/// stand-in on the side of loss (the science check, 2026-10-22).
pub(crate) const HOT_ICE_LIMIT: Kelvin = UNDER_ICE_MELTING_POINT;

/// What a parent's moons' ice reads of their sky and their hosts' history (P14.T56.f), built once
/// for the parent at the time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MoonIce {
    /// The parent's hosts' flux at its orbit now, which its moons share.
    pub(crate) stellar_flux: EarthFluxes,
    /// The parent's own glow, its moons' other light.
    pub(crate) glow: SolarLuminosities,
    /// How long its zone has been hot, as its moons' ice reads it.
    pub(crate) history: HotHistory,
}

/// How long a moon's zone may have held it above [`HOT_ICE_LIMIT`] (P14.T56.f's `t_hot`, a stated
/// upper bound, decision-bump-plan-after-22 G2 (c)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HotHistory {
    /// Every star of its zone on its main sequence or before it: the age, though the host
    /// brightened over it.
    MainSequence {
        /// The system's age at the time.
        age: Years,
    },
    /// A star of its zone past its main sequence: the time since its main sequence ended, or the
    /// age where the moon was above the limit already at that end's luminosity, though the body
    /// crossed the limit later.
    Evolved {
        /// The system's age at the time.
        age: Years,
        /// The time since the earliest main-sequence end among its zone's evolved stars.
        since_main_sequence: Years,
        /// The zone's luminosity at that end over its luminosity now, which scales the moon's
        /// zero-albedo T⁴ back to then.
        brightness_then: f64,
    },
}

impl HotHistory {
    /// The time a moon of zero-albedo temperature `t0` now has spent above [`HOT_ICE_LIMIT`]: none
    /// at or below it.
    #[must_use]
    pub(crate) fn hot_for(self, t0: Kelvin) -> Years {
        let limit = HOT_ICE_LIMIT.value();
        // A temperature that is not a number is not hot.
        if t0.value().is_nan() || t0.value() <= limit {
            return Years::new(0.0);
        }
        match self {
            Self::MainSequence { age } => age,
            Self::Evolved {
                age,
                since_main_sequence,
                brightness_then,
            } => {
                let then = t0.value() * math::powf(brightness_then.max(0.0), 0.25);
                if then > limit {
                    age
                } else {
                    since_main_sequence
                }
            }
        }
    }
}

impl MoonIce {
    /// The zero-albedo equilibrium temperature `T0` of a moon on the orbit `orbit` about this
    /// parent: its hosts' flux at the parent's orbit and the
    /// parent's glow at the moon's, absorbed whole (G2's "T0, always": before the passes, a
    /// capture's `T_eq` would read the bulk this arm gives).
    #[must_use]
    pub(crate) fn t0(&self, orbit: &KeplerElements) -> Kelvin {
        let a = orbit.semi_major_axis();
        let glow = if self.glow.value() > 0.0 && a.value() > 0.0 {
            luminosity_flux(self.glow, a, orbit.eccentricity().value())
        } else {
            EarthFluxes::ZERO
        };
        equilibrium_temperature(self.stellar_flux + glow, BondAlbedo::from_fraction(0.0))
    }
}

/// The water a bulk of water fraction `water` and radius `radius` keeps after `hot_for` above
/// [`HOT_ICE_LIMIT`] (P14.T56.f's retention arm; G2 amendment 2, with main's mass-balance
/// correction of 2026-10-22).
///
/// The heat reaches a depth d = √(κt), κ [`ROCK_THERMAL_DIFFUSIVITY_M2_PER_S`]'s 10⁻⁶ m² s⁻¹ (a
/// consolidated rock's, about 1 mm² s⁻¹, Whittington, Hofmeister and Nabelek 2009, Nature 458,
/// 319; Turcotte and Schubert, _Geodynamics_, 3rd ed., 2014, §4.13, not re-read), and the ice of
/// the heated shell leaves: the unheated core keeps f = (1 − d ÷ R)³ of it, none where d ≥ R. The
/// kept water fraction is that of the remaining mass, w′ = w f ÷ (1 − w + w f): the shell loses its
/// water only, and iron and rock keep their ratio. The record holds the moon's mass, so the lost
/// mass is P14.T56.g's.
///
/// - **The front.** d = √(κt) is a first-order Stefan front. For a flat front with the
///   sublimation's latent heat (2.834 MJ kg⁻¹, Schorghofer 2008 eq. 5) at Stefan numbers of
///   0.17–0.58, Neumann's front 2λ√(κt) has λ = 0.28–0.50, so √(κt) runs ahead of it, on the side
///   of loss, below St 0.59. In a sphere the front speeds up as it closes on the centre, so the
///   shell form is not one-sided there: at subgiant 0101 (St 0.58) the sphere keeps f = 0.150
///   against the shell form's 0.212 (the science check's enthalpy solution; P14.T56.g's).
/// - **Its zero** is R² = κt, the lane's lean, and it is continuous in R and t for a water fraction
///   below 1. A bulk of all water (`ice_fraction_of` gives exactly 1 at or below ice's density)
///   keeps w′ = 1 at any depth short of R, its remaining mass being all ice, and none at R.
///
/// `water` itself where nothing is lost (`hot_for` zero, or no water), to the bit.
#[must_use]
pub(crate) fn kept_water(water: f64, radius: Metres, hot_for: Years) -> f64 {
    let (r, t) = (radius.value(), hot_for.value() * SECONDS_PER_JULIAN_YEAR);
    if !(water > 0.0 && t > 0.0 && r > 0.0) {
        return water;
    }
    let depth = (ROCK_THERMAL_DIFFUSIVITY_M2_PER_S * t).sqrt();
    if depth >= r {
        return 0.0;
    }
    let core = 1.0 - depth / r;
    let kept = water * core * core * core;
    kept / (1.0 - water + kept)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// The arm's figures as main corrected G2 (2026-10-22): subgiant 0101, 1,184 km at 0.3749 water
    /// for 7.25 Gyr, keeps w′ = 0.113 (Icy, a knife edge, w₀f = 0.079); a 120 km capture of
    /// `wide_binary`'s for 2.32 Gyr keeps none; and nothing changes with no time hot.
    #[test]
    fn the_kept_water_is_the_unheated_core_s_share_of_the_remaining_mass() {
        let w = kept_water(0.3749, Metres::new(1.184e6), Years::new(7.25e9));
        assert!((w - 0.113).abs() < 0.001, "{w}");
        assert_same_bits(
            kept_water(0.375, Metres::new(1.2e5), Years::new(2.32e9)),
            0.0,
        );
        assert_same_bits(
            kept_water(0.375, Metres::new(1.2e5), Years::new(0.0)),
            0.375,
        );
        assert_same_bits(kept_water(0.0, Metres::new(1.2e5), Years::new(9e9)), 0.0);
        // red_giant 0503 and 0504: below 0.1 only beyond 1.28 and 1.34 Gyr hot.
        for (w0, km, gyr) in [(0.4545, 411.0, 1.28), (0.3713, 482.0, 1.34)] {
            let before = kept_water(w0, Metres::new(km * 1e3), Years::new((gyr - 0.02) * 1e9));
            let after = kept_water(w0, Metres::new(km * 1e3), Years::new((gyr + 0.02) * 1e9));
            assert!(before > 0.1 && after < 0.1, "{km} km: {before} {after}");
        }
    }

    /// A moon at or below the limit is never hot; about a main-sequence zone it has been hot for
    /// the age; about an evolved one since the main sequence ended, or for the age where it was
    /// already above the limit then.
    #[test]
    fn the_hot_time_is_the_age_or_the_time_since_the_main_sequence() {
        let age = Years::new(9e9);
        let ms = HotHistory::MainSequence { age };
        assert_eq!(ms.hot_for(Kelvin::new(273.15)), Years::new(0.0));
        assert_eq!(ms.hot_for(Kelvin::new(660.0)), age);
        let evolved = |brightness_then| HotHistory::Evolved {
            age,
            since_main_sequence: Years::new(5e8),
            brightness_then,
        };
        // L_TAMS ÷ L_now of 1.5 ÷ 351: T0 664 K then 172 K, below the limit.
        assert_eq!(
            evolved(1.5 / 351.0).hot_for(Kelvin::new(664.0)),
            Years::new(5e8)
        );
        // At 0.9 of today's light, still above the limit then.
        assert_eq!(evolved(0.9).hot_for(Kelvin::new(889.0)), age);
        assert_eq!(evolved(0.9).hot_for(Kelvin::new(200.0)), Years::new(0.0));
    }

    /// The moon bulk is held until P14.T54.b's bump: below it every moon's bulk is 23's, and the
    /// tests' switch reaches 24's ahead of it.
    #[test]
    fn the_moon_bulk_is_held_only_until_the_24_bump() {
        assert!(
            GENERATOR_VERSION < MOON_BULK_VERSION,
            "the 23 → 24 bump (P14.T54.b) puts P14.T56.f on the record: a capture formed inside \
             its parent's snow line is rock, and every moon's bulk keeps only the ice that \
             survives its time above the melting point. Delete `MOON_BULK_VERSION`, \
             `MoonBulkRules` with its `Held` arm, `with_moon_bulk` and its thread's rules, the \
             `Held` return of `PlanetarySystem::moon_ice`, and this test"
        );
        assert_eq!(MoonBulkRules::in_force(), MoonBulkRules::Held);
        assert_eq!(
            with_moon_bulk(MoonBulkRules::Batch, MoonBulkRules::in_force),
            MoonBulkRules::Batch
        );
        assert_eq!(MoonBulkRules::in_force(), MoonBulkRules::Held);
    }
}
