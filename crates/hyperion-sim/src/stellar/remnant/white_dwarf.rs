//! White dwarfs (plan 06, P06.T20.a): how they cool.
//!
//! A white dwarf's luminosity is a closed form in its mass, its core composition, the metal
//! fraction of the star it came from and the time since it formed, and its effective temperature
//! follows from that luminosity and the radius of its mass (P06.T11's
//! `structure::white_dwarf_radius`, HPT's equation 91).
//!
//! - Under the generator's default, [`RemnantRecipe::MandelMuller2020`], the law is the fit to
//!   the Montreal evolutionary sequences of Bédard et al. (2020, ApJ 901, 93) in `cooling`
//!   (ruling 57.2 of 2026-09-22), which replaced the two-piece modified Mestel cooling of Hurley
//!   and Shara (2003, ApJ 589, 179, section 2) ([`hurley_shara_luminosity`]): that law ran 13–20%
//!   cool in `T_eff` against the 0.6 M☉ sequence.
//! - Under [`RemnantRecipe::Hurley2000`] it is the Mestel law of Hurley, Pols and Tout (2000,
//!   MNRAS 315, 543, "HPT", section 6.2.1, equation 90) ([`hpt_luminosity`]), kept for good,
//!   since P06.T12.b compares with the published SSE code (ruling 33 of 2026-09-22).
//!
//! The white dwarf that HPT section 6.3's small-envelope perturbation draws a thinning giant
//! towards is a closed-form law at the instant of formation ([`formation_luminosity`]): equation 90
//! under `Hurley2000` and Hurley and Shara's law under the default. The Montreal sequences cannot
//! stand in there, since each starts where its model was started (0.2 L☉ at 0.2 M☉, 56 L☉ at
//! 0.6 M☉), not at the dwarf's formation. Under the default the cooling law's clock is then matched
//! to the star's last luminosity, so the luminosity is continuous at the hand-over whatever the
//! perturbation's target ([`cooling_origin`]).
//!
//! The spectral types of P06.T20.b are `wd_spectral`'s.

use crate::math;
use crate::stellar::Phase;
use crate::units::{Megayears, MetalFraction, SolarLuminosities, SolarMasses, Years};

use super::{RemnantRecipe, cooling};

/// What a white dwarf is made of, which sets the effective baryon number A of its cooling law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhiteDwarfCore {
    /// Helium: A = 4.
    Helium,
    /// Carbon and oxygen: A = 15.
    CarbonOxygen,
    /// Oxygen and neon: A = 17.
    OxygenNeon,
}

impl WhiteDwarfCore {
    /// The white dwarf of `phase`, or `None` if it is not a white dwarf's.
    #[must_use]
    pub(crate) const fn of(phase: Phase) -> Option<Self> {
        match phase {
            Phase::HeliumWhiteDwarf => Some(Self::Helium),
            Phase::CarbonOxygenWhiteDwarf => Some(Self::CarbonOxygen),
            Phase::OxygenNeonWhiteDwarf => Some(Self::OxygenNeon),
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch
            | Phase::PostAgb
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => None,
        }
    }

    /// HPT's effective baryon number A for the composition (section 6.2.1): 4, 15 or 17.
    ///
    /// Hurley and Shara (2003, section 2) define A as the baryon number of the white dwarf's
    /// material, carbon–oxygen dwarfs as 20% carbon and 80% oxygen, and oxygen–neon ones as 80%
    /// oxygen and 20% neon, which is 15.2 and 16.8: HPT's 15 and 17, to the nearest whole number,
    /// which both laws take.
    #[must_use]
    const fn baryon_number(self) -> f64 {
        match self {
            Self::Helium => 4.0,
            Self::CarbonOxygen => 15.0,
            Self::OxygenNeon => 17.0,
        }
    }
}

/// The luminosity of a white dwarf of `mass` and composition `core`, `cooling_age` after it
/// formed, from a star of metal fraction `z`, by HPT's form of Mestel cooling (equation 90):
/// L = 635 M Z^0.4 ÷ (A (t + 0.1))^1.4 L☉, with t in Myr.
///
/// HPT add the 0.1 Myr so that the law starts at a finite luminosity, as if at a cooling age of
/// 10⁵ years, and note that it is adequate for old white dwarfs; the small-envelope perturbation of
/// their section 6.3 evaluates it at t = 0 with A = 4 or 15 for the white dwarf a giant's core
/// would become. The published SSE code carries 16 for A of both carbon–oxygen and oxygen–neon
/// dwarfs where the paper prints 15 and 17, and its distributed input file selects the cooling of
/// Hurley and Shara (2003) instead; P06.T12.b does not compare white-dwarf luminosities.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `cooling_age` is negative.
#[must_use]
pub(crate) fn hpt_luminosity(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    cooling_age: Years,
    z: MetalFraction,
) -> SolarLuminosities {
    debug_assert!(
        mass.value() > 0.0 && cooling_age.value() >= 0.0,
        "a white dwarf of {mass:?} at a cooling age of {cooling_age:?}"
    );
    let t_myr = cooling_age.value() * 1e-6;
    SolarLuminosities::new(
        635.0 * mass.value() * math::powf(z.value(), 0.4)
            / math::powf(core.baryon_number() * (t_myr + 0.1), 1.4),
    )
}

/// Hurley and Shara's factor b of the modified Mestel law before 9 Gyr, 300 (their section 2).
const HS_SCALE: f64 = 300.0;

/// The exponent x of the modified law before [`HS_BREAK_MYR`], 1.18 (Hurley and Shara 2003,
/// section 2).
const HS_EARLY_EXPONENT: f64 = 1.18;

/// The exponent x of the modified law from [`HS_BREAK_MYR`] on, 6.48 (Hurley and Shara 2003,
/// section 2), where crystallisation and the Debye regime speed the cooling.
const HS_LATE_EXPONENT: f64 = 6.48;

/// The exponent of the late piece's factor, 5.3 = 6.48 − 1.18 (Hurley and Shara 2003, section 2),
/// which makes the two pieces meet at [`HS_BREAK_MYR`].
const HS_KNEE_EXPONENT: f64 = 5.3;

/// The white dwarf's age, Myr, at which the modified law passes from its early to its late piece:
/// 9,000 Myr (Hurley and Shara 2003, section 2).
const HS_BREAK_MYR: f64 = 9_000.0;

/// HPT's offset of the cooling clock, 0.1 Myr: both laws read t + 0.1 so that they start at a
/// finite luminosity (HPT section 6.2.1; Hurley and Shara 2003, section 2).
const CLOCK_OFFSET_MYR: f64 = 0.1;

/// The luminosity of a white dwarf of `mass` and composition `core` from a star of metal fraction
/// `z`, `law_time` into its cooling law, by the two-piece modified Mestel cooling of Hurley
/// and Shara (2003, ApJ 589, 179, section 2): L = b M Z^0.4 ÷ (A (t + 0.1))^x L☉, with b = 300 and
/// x = 1.18 before 9,000 Myr, and b = 300 (9,000.1 A)^5.3 and x = 6.48 from then on.
///
/// Hurley and Shara fit the split to the detailed models of Hansen (1999), whose cooling slows at
/// first, with neutrino losses and true atmospheric opacities, and quickens once the crystallised
/// core enters the Debye regime; HPT's single Mestel law (b = 635, x = 1.4) misses both. They
/// print the late factor as 300 (9,000 A)^5.3, which leaves the two pieces 6 × 10⁻⁵ apart in L at
/// 9,000 Myr; their published SSE code writes 9,000.1 A, which makes them meet (`hrdiag`, stellar
/// types 10–12, `wdflag` > 0), and so does this.
///
/// `law_time` is the white dwarf's cooling age plus its [`cooling_origin`], above −0.1 Myr.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `law_time` is not above −0.1 Myr.
#[must_use]
pub(crate) fn hurley_shara_luminosity(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    law_time: Megayears,
    z: MetalFraction,
) -> SolarLuminosities {
    let law_time = law_time.value();
    debug_assert!(
        mass.value() > 0.0 && law_time > -CLOCK_OFFSET_MYR,
        "a white dwarf of {mass:?} {law_time} Myr into its cooling"
    );
    let a = core.baryon_number();
    let scaled = HS_SCALE * mass.value() * math::powf(z.value(), 0.4);
    let clock = a * (law_time + CLOCK_OFFSET_MYR);
    SolarLuminosities::new(if law_time < HS_BREAK_MYR {
        scaled / math::powf(clock, HS_EARLY_EXPONENT)
    } else {
        let knee = a * (HS_BREAK_MYR + CLOCK_OFFSET_MYR);
        scaled * math::powf(knee, HS_KNEE_EXPONENT) / math::powf(clock, HS_LATE_EXPONENT)
    })
}

/// The luminosity of a white dwarf of `mass` and `core`, from a star of metal fraction `z`, at
/// `cooling_age` after its formation, under `recipe`, whose law's clock starts at `origin`
/// ([`cooling_origin`]): the Montreal law (`cooling::luminosity`) at `cooling_age` + `origin`
/// under [`RemnantRecipe::MandelMuller2020`], which does not read `z`, and HPT's equation 90
/// under [`RemnantRecipe::Hurley2000`], which starts at zero whatever `origin` says.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive, `cooling_age` is negative, or `origin` is not above
/// −0.1 Myr.
#[must_use]
pub(crate) fn luminosity(
    recipe: RemnantRecipe,
    core: WhiteDwarfCore,
    mass: SolarMasses,
    cooling_age: Years,
    origin: Megayears,
    z: MetalFraction,
) -> SolarLuminosities {
    match recipe {
        RemnantRecipe::Hurley2000 => hpt_luminosity(core, mass, cooling_age, z),
        RemnantRecipe::MandelMuller2020 => {
            debug_assert!(
                cooling_age.value() >= 0.0,
                "a white dwarf's cooling age is not negative: {cooling_age:?}"
            );
            let law_time = Megayears::new(cooling_age.value() * 1e-6 + origin.value());
            cooling::luminosity(core, mass, law_time)
        }
    }
}

/// The luminosity of a white dwarf of `mass` and `core` at the instant it forms: what HPT section
/// 6.3's small-envelope perturbation draws a thinning giant's luminosity towards. HPT use equation
/// 90 at t = 0, and so does [`RemnantRecipe::Hurley2000`]; the published SSE code uses Hurley and
/// Shara's law at t = 0 when it cools by it (`hrdiag`, stellar types 3, 5 and 6 with `wdflag` >
/// 0), and so does the default, whose cooling law, the Montreal fit, has no instant of formation
/// (see the module's documentation). The cooling law is matched to where the perturbation leaves
/// the star ([`cooling_origin`]).
///
/// # Panics
///
/// In debug builds, if `mass` is not positive.
#[must_use]
pub(crate) fn formation_luminosity(
    recipe: RemnantRecipe,
    core: WhiteDwarfCore,
    mass: SolarMasses,
    z: MetalFraction,
) -> SolarLuminosities {
    match recipe {
        RemnantRecipe::Hurley2000 => hpt_luminosity(core, mass, Years::ZERO, z),
        RemnantRecipe::MandelMuller2020 => hurley_shara_luminosity(core, mass, Megayears::ZERO, z),
    }
}

/// Where a white dwarf's cooling law starts under `recipe`, in the law's own clock: the point at which
/// a white dwarf of `mass` and `core` has `last_luminosity`, the
/// luminosity of the star's last living instant, so that the luminosity is continuous across the
/// hand-over (plan 06, P06.T20.a: the law "is matched to" the luminosity it takes over from).
///
/// Under [`RemnantRecipe::MandelMuller2020`] it is the Montreal law inverted at
/// `last_luminosity` (`cooling::time_at`), above −0.1 Myr, which does not read the metal
/// fraction (`_z`, kept for the call's shape); zero where the star's last luminosity is unknown (`None`: a
/// helium star too light to burn helium, which is a white dwarf at once). Under
/// [`RemnantRecipe::Hurley2000`] it is zero, since HPT's law and the published SSE code start
/// every white dwarf at t = 0, steps and all.
///
/// Until P06.T16 the star hands over at the loss of its envelope, directly (T10.d), and the
/// dwarf's cooling age counts from there. The perturbation has drawn the star's luminosity to
/// Hurley and Shara's value at formation ([`formation_luminosity`]), 23 L☉ for a 0.6 M☉
/// carbon–oxygen dwarf, which the Montreal law reaches 0.18 Myr after its sequence's first
/// model; the match puts the clock there, so there is no step: not the 0.06 dex an oxygen–neon
/// dwarf would take, nor the 0.26–0.34 dex of a helium star below 0.689 M☉, whose dwarf keeps the
/// unburnt helium (ruling 46 of 2026-09-22), nor any the Montreal law's own start would leave.
/// Once P06.T16's post-AGB bridge lands, the match is to the bridge's end (ruling 46.2).
#[must_use]
pub(crate) fn cooling_origin(
    recipe: RemnantRecipe,
    core: WhiteDwarfCore,
    mass: SolarMasses,
    last_luminosity: Option<SolarLuminosities>,
    _z: MetalFraction,
) -> Megayears {
    match (recipe, last_luminosity) {
        (RemnantRecipe::MandelMuller2020, Some(l)) if l.value() > 0.0 && l.value().is_finite() => {
            cooling::time_at(core, mass, l)
        }
        (RemnantRecipe::MandelMuller2020 | RemnantRecipe::Hurley2000, _) => Megayears::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// A 0.6 M☉ carbon–oxygen dwarf at solar Z: 635 × 0.6 × 0.02^0.4 ÷ (15 × 0.1)^1.4 = 45.2 L☉
    /// at formation and 3.2 × 10⁻⁴ L☉ after 10 Gyr (Python's double precision), fainter with age
    /// and for a heavier nucleus.
    #[test]
    fn a_carbon_oxygen_dwarf_follows_equation_90() {
        let z = MetalFraction::new(0.02);
        let m = SolarMasses::new(0.6);
        let at = |core, years| hpt_luminosity(core, m, Years::new(years), z).value();
        let young = at(WhiteDwarfCore::CarbonOxygen, 0.0);
        assert!(
            (young / 45.165_748_435_515_77 - 1.0).abs() < 1e-12,
            "{young}"
        );
        let old = at(WhiteDwarfCore::CarbonOxygen, 1e10);
        let expected = 635.0 * 0.6 * math::powf(0.02, 0.4) / math::powf(15.0 * 10_000.1, 1.4);
        assert!((old / expected - 1.0).abs() < 1e-14, "{old}");
        assert!(old < young);
        assert!(at(WhiteDwarfCore::Helium, 1e8) > at(WhiteDwarfCore::CarbonOxygen, 1e8));
        assert!(at(WhiteDwarfCore::CarbonOxygen, 1e8) > at(WhiteDwarfCore::OxygenNeon, 1e8));
    }

    /// Hurley and Shara's law for a 0.6 M☉ carbon–oxygen dwarf at solar Z against Python's double
    /// precision: 300 × 0.6 × 0.02^0.4 ÷ 1.5^1.18 = 23.33 L☉ at formation, 4.44 × 10⁻⁴ L☉ at
    /// 1 Gyr, and 1.68 × 10⁻⁵ L☉ at 10 Gyr on the late piece.
    #[test]
    fn a_carbon_oxygen_dwarf_follows_hurley_and_shara() {
        let z = MetalFraction::new(0.02);
        let m = SolarMasses::new(0.6);
        let at = |myr: f64| {
            hurley_shara_luminosity(WhiteDwarfCore::CarbonOxygen, m, Megayears::new(myr), z).value()
        };
        for (myr, expected) in [
            (0.0, 23.329_037_310_165_237),
            (1_000.0, 4.444_731_935_153_361_6e-4),
            (10_000.0, 1.680_271_845_892_194e-5),
        ] {
            let l = at(myr);
            assert!((l / expected - 1.0).abs() < 1e-12, "{myr} Myr: {l}");
        }
        // The default recipe's perturbation reads it at formation, and its cooling is the
        // Montreal law's, at the cooling age plus the origin; Hurley2000 keeps equation 90.
        let formation = formation_luminosity(
            RemnantRecipe::MandelMuller2020,
            WhiteDwarfCore::CarbonOxygen,
            m,
            z,
        );
        assert_same_bits(formation.value(), at(0.0));
        let age = Years::new(1e9);
        let default = luminosity(
            RemnantRecipe::MandelMuller2020,
            WhiteDwarfCore::CarbonOxygen,
            m,
            age,
            Megayears::new(0.3),
            z,
        );
        let montreal =
            cooling::luminosity(WhiteDwarfCore::CarbonOxygen, m, Megayears::new(1_000.3));
        assert_same_bits(default.value(), montreal.value());
        assert_same_bits(
            formation_luminosity(
                RemnantRecipe::Hurley2000,
                WhiteDwarfCore::CarbonOxygen,
                m,
                z,
            )
            .value(),
            hpt_luminosity(WhiteDwarfCore::CarbonOxygen, m, Years::ZERO, z).value(),
        );
        let hpt = luminosity(
            RemnantRecipe::Hurley2000,
            WhiteDwarfCore::CarbonOxygen,
            m,
            age,
            Megayears::new(0.7),
            z,
        );
        assert_eq!(hpt, hpt_luminosity(WhiteDwarfCore::CarbonOxygen, m, age, z));
    }

    /// The two pieces meet at 9 Gyr, and the luminosity falls with age throughout, faster on the
    /// late piece, for every composition.
    #[test]
    fn the_modified_law_is_continuous_and_falls_with_age() {
        let z = MetalFraction::new(0.004);
        for core in [
            WhiteDwarfCore::Helium,
            WhiteDwarfCore::CarbonOxygen,
            WhiteDwarfCore::OxygenNeon,
        ] {
            let m = SolarMasses::new(0.9);
            let at = |myr: f64| hurley_shara_luminosity(core, m, Megayears::new(myr), z).value();
            let (before, after) = (at(HS_BREAK_MYR * (1.0 - 1e-15)), at(HS_BREAK_MYR));
            assert!(
                (after / before - 1.0).abs() < 1e-13,
                "{core:?}: {before} to {after}"
            );
            let mut last = f64::INFINITY;
            for i in 0..=400 {
                let x = f64::from(i) / 400.0;
                let myr = -0.099 + 14_000.0 * x * x * x;
                let l = at(myr);
                assert!(
                    l < last && l > 0.0,
                    "{core:?} at {myr} Myr: {l} after {last}"
                );
                last = l;
            }
        }
    }

    /// Under the default the origin is the Montreal law inverted at the star's last luminosity,
    /// and applying the law there returns that luminosity: for the perturbation's target at
    /// formation, which a 0.6 M☉ carbon–oxygen dwarf reaches 0.18 Myr into the law, and for a
    /// giant's luminosity far above it, where the clock starts before zero. Under `Hurley2000`, and
    /// where the last luminosity is unknown, it is zero.
    #[test]
    fn the_cooling_origin_matches_the_last_luminosity() {
        let z = MetalFraction::new(0.02);
        let recipe = RemnantRecipe::MandelMuller2020;
        for core in [
            WhiteDwarfCore::Helium,
            WhiteDwarfCore::CarbonOxygen,
            WhiteDwarfCore::OxygenNeon,
        ] {
            for m in [0.3, 0.6, 1.2, 1.37] {
                let mass = SolarMasses::new(m);
                for l in [
                    formation_luminosity(recipe, core, mass, z).value(),
                    5e4,
                    3.0,
                    1e-4,
                ] {
                    let last = SolarLuminosities::new(l);
                    let origin = cooling_origin(recipe, core, mass, Some(last), z);
                    assert!(origin.value() > -0.1, "{core:?} {m} at {l}: {origin:?}");
                    let first = luminosity(recipe, core, mass, Years::ZERO, origin, z).value();
                    assert!(
                        (first / l - 1.0).abs() < 1e-9,
                        "{core:?} {m} M☉ from {l} L☉: {first} L☉ at the hand-over"
                    );
                }
            }
        }
        let co = WhiteDwarfCore::CarbonOxygen;
        let m = SolarMasses::new(0.6);
        let target = formation_luminosity(recipe, co, m, z);
        let origin = cooling_origin(recipe, co, m, Some(target), z).value();
        assert!(origin > 0.15 && origin < 0.2, "{origin} Myr");
        let bright = SolarLuminosities::new(5e4);
        assert!(cooling_origin(recipe, co, m, Some(bright), z).value() < 0.0);
        assert_same_bits(
            cooling_origin(RemnantRecipe::Hurley2000, co, m, Some(bright), z).value(),
            0.0,
        );
        assert_same_bits(cooling_origin(recipe, co, m, None, z).value(), 0.0);
    }

    /// Pins the three cooling laws and the cooling origin bit for bit, for the three compositions
    /// at three masses and ages on both of Hurley and Shara's pieces and across the Montreal
    /// table, so that any change to their arithmetic is seen.
    #[test]
    fn the_cooling_laws_are_pinned() {
        let mut w = hyperion_testkit::golden::GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        let z = MetalFraction::new(0.008);
        for core in [
            WhiteDwarfCore::Helium,
            WhiteDwarfCore::CarbonOxygen,
            WhiteDwarfCore::OxygenNeon,
        ] {
            for m in [0.3, 0.6, 1.2] {
                let mass = SolarMasses::new(m);
                w.line(&format!("{core:?} {m} M_sun"));
                for myr in [-0.05, 0.0, 1.0, 100.0, 1_000.0, 8_999.0, 9_000.0, 12_000.0] {
                    let l = hurley_shara_luminosity(core, mass, Megayears::new(myr), z);
                    w.f64(&format!("  Hurley-Shara at {myr} Myr"), l.value());
                    let back =
                        cooling_origin(RemnantRecipe::MandelMuller2020, core, mass, Some(l), z);
                    w.f64("  origin of that L", back.value());
                }
                for myr in [0.0, 100.0, 12_000.0] {
                    let l = hpt_luminosity(core, mass, Years::new(myr * 1e6), z);
                    w.f64(&format!("  HPT at {myr} Myr"), l.value());
                }
                for myr in [-0.05, 0.0, 1.0, 100.0, 1_000.0, 5_000.0, 12_000.0] {
                    let l = cooling::luminosity(core, mass, Megayears::new(myr));
                    w.f64(&format!("  Montreal at {myr} Myr"), l.value());
                }
            }
        }
        hyperion_testkit::golden!("stellar/white_dwarf_cooling", w.as_str());
    }

    #[test]
    fn only_white_dwarf_phases_have_a_white_dwarf_core() {
        let dwarfs = Phase::ALL
            .into_iter()
            .filter(|p| WhiteDwarfCore::of(*p).is_some())
            .count();
        assert_eq!(dwarfs, 3);
    }
}
