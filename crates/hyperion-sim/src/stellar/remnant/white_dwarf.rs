//! White dwarfs (plan 06, P06.T20.a): how they cool.
//!
//! A white dwarf's luminosity is a closed form in its mass, its core composition, the metal
//! fraction of the star it came from and the time since it formed, and its effective temperature
//! follows from that luminosity and the radius of its mass (P06.T11's
//! `structure::white_dwarf_radius`, HPT's equation 91).
//!
//! - Under the generator's default, [`RemnantRecipe::MandelMuller2020`], the law is the
//!   two-piece modified Mestel cooling of Hurley and Shara (2003, ApJ 589, 179, section 2)
//!   ([`hurley_shara_luminosity`]).
//! - Under [`RemnantRecipe::Hurley2000`] it is the Mestel law of Hurley, Pols and Tout (2000,
//!   MNRAS 315, 543, "HPT", section 6.2.1, equation 90) ([`hpt_luminosity`]), kept for good,
//!   since P06.T12.b compares with the published SSE code (ruling 33 of 2026-09-22).
//!
//! The same law, at the instant of formation, is the white dwarf that HPT section 6.3's
//! small-envelope perturbation draws a thinning giant towards ([`formation_luminosity`]), so that
//! under either recipe the star and its white dwarf agree where one hands over to the other. Under
//! the default the law's clock is also matched to the star's last luminosity, which removes what
//! step the hand-over leaves ([`cooling_origin`]).
//!
//! The spectral types of P06.T20.b are `wd_spectral`'s.

use crate::math;
use crate::stellar::Phase;
use crate::units::{Megayears, MetalFraction, SolarLuminosities, SolarMasses, Years};

use super::RemnantRecipe;

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

/// The inverse of [`hurley_shara_luminosity`] in time: the point of the law at which a
/// white dwarf of `mass` and `core` from a star of metal fraction `z` has `luminosity`, always
/// above −0.1 Myr.
///
/// # Panics
///
/// In debug builds, if `mass` or `luminosity` is not positive and finite.
#[must_use]
fn hurley_shara_time_at(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    luminosity: SolarLuminosities,
    z: MetalFraction,
) -> Megayears {
    debug_assert!(
        mass.value() > 0.0 && luminosity.value() > 0.0 && luminosity.value().is_finite(),
        "a white dwarf of {mass:?} at {luminosity:?}"
    );
    let a = core.baryon_number();
    let ratio = HS_SCALE * mass.value() * math::powf(z.value(), 0.4) / luminosity.value();
    let early = math::powf(ratio, 1.0 / HS_EARLY_EXPONENT) / a - CLOCK_OFFSET_MYR;
    if early < HS_BREAK_MYR {
        return Megayears::new(early);
    }
    let knee = a * (HS_BREAK_MYR + CLOCK_OFFSET_MYR);
    let late = ratio * math::powf(knee, HS_KNEE_EXPONENT);
    Megayears::new(math::powf(late, 1.0 / HS_LATE_EXPONENT) / a - CLOCK_OFFSET_MYR)
}

/// The luminosity of a white dwarf of `mass` and `core`, from a star of metal fraction `z`, at
/// `cooling_age` after its formation, under `recipe`, whose law's clock starts at `origin`
/// ([`cooling_origin`]): Hurley and Shara's law at `cooling_age` + `origin` under
/// [`RemnantRecipe::MandelMuller2020`], and HPT's equation 90 under
/// [`RemnantRecipe::Hurley2000`], which starts at zero whatever `origin` says.
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
            hurley_shara_luminosity(core, mass, law_time, z)
        }
    }
}

/// The luminosity of a white dwarf of `mass` and `core` at the instant it forms, under `recipe`'s
/// law with its clock at zero: what HPT section 6.3's small-envelope perturbation draws a thinning
/// giant's luminosity towards (HPT use equation 90 at t = 0; the published SSE code the modified
/// law at t = 0 when it cools by it, `hrdiag`, stellar types 3, 5 and 6 with `wdflag` > 0).
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
    luminosity(recipe, core, mass, Years::ZERO, Megayears::ZERO, z)
}

/// Where a white dwarf's cooling law starts under `recipe`, in the law's own clock: the point at which
/// a white dwarf of `mass` and `core` from a star of metal fraction `z` has `last_luminosity`, the
/// luminosity of the star's last living instant, so that the luminosity is continuous across the
/// hand-over (plan 06, P06.T20.a: the law "is matched to" the luminosity it takes over from).
///
/// Under [`RemnantRecipe::MandelMuller2020`] it is Hurley and Shara's law inverted at
/// `last_luminosity`, above −0.1 Myr; zero where the star's last luminosity is unknown (`None`: a
/// helium star too light to burn helium, which is a white dwarf at once). Under
/// [`RemnantRecipe::Hurley2000`] it is zero, since HPT's law and the published SSE code start
/// every white dwarf at t = 0, steps and all.
///
/// Until P06.T16 the star hands over at the loss of its envelope, directly (T10.d), and the
/// dwarf's cooling age counts from there. Carbon–oxygen dwarfs from the AGB then start within
/// about 10⁻⁴ Myr of zero, because the perturbation has drawn the star's luminosity to the law's
/// value at formation. What the match removes is the step where the dwarf is not the one the
/// perturbation drew towards: about 0.06 dex for an oxygen–neon dwarf, whose nucleus is heavier
/// than the perturbation's carbon–oxygen one, and 0.26–0.34 dex for a helium star below
/// 0.689 M☉, whose dwarf keeps the unburnt helium (ruling 46 of 2026-09-22). Once P06.T16's
/// post-AGB bridge lands, the match is to the bridge's end.
#[must_use]
pub(crate) fn cooling_origin(
    recipe: RemnantRecipe,
    core: WhiteDwarfCore,
    mass: SolarMasses,
    last_luminosity: Option<SolarLuminosities>,
    z: MetalFraction,
) -> Megayears {
    match (recipe, last_luminosity) {
        (RemnantRecipe::MandelMuller2020, Some(l)) if l.value() > 0.0 && l.value().is_finite() => {
            hurley_shara_time_at(core, mass, l, z)
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
        // The default recipe reads it, Hurley2000 keeps equation 90.
        let age = Years::new(1e9);
        let default = luminosity(
            RemnantRecipe::MandelMuller2020,
            WhiteDwarfCore::CarbonOxygen,
            m,
            age,
            Megayears::ZERO,
            z,
        );
        assert!((default.value() / at(1_000.0) - 1.0).abs() < 1e-15);
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

    /// Inverting the law in time and applying it again returns the luminosity, on both pieces and
    /// above the law's value at formation, where the clock starts before zero.
    #[test]
    fn the_cooling_origin_inverts_the_law() {
        let z = MetalFraction::new(0.02);
        let m = SolarMasses::new(0.7);
        let core = WhiteDwarfCore::OxygenNeon;
        for myr in [
            -0.09, -0.01, 0.0, 0.3, 50.0, 8_999.0, 9_000.0, 9_500.0, 13_000.0,
        ] {
            let l = hurley_shara_luminosity(core, m, Megayears::new(myr), z);
            let origin =
                cooling_origin(RemnantRecipe::MandelMuller2020, core, m, Some(l), z).value();
            assert!(
                (origin - myr).abs() < 1e-9 * (1.0 + myr.abs()),
                "{myr}: {origin}"
            );
        }
        let bright = SolarLuminosities::new(5e4);
        let origin = cooling_origin(RemnantRecipe::MandelMuller2020, core, m, Some(bright), z);
        assert!(
            origin.value() > -0.1 && origin.value() < -0.09,
            "{origin:?}"
        );
        assert!((hurley_shara_luminosity(core, m, origin, z).value() / 5e4 - 1.0).abs() < 1e-9);
        assert_same_bits(
            cooling_origin(RemnantRecipe::Hurley2000, core, m, Some(bright), z).value(),
            0.0,
        );
        assert_same_bits(
            cooling_origin(RemnantRecipe::MandelMuller2020, core, m, None, z).value(),
            0.0,
        );
    }

    /// P06.T20.a's check against a published cooling sequence: the 0.6 M☉ carbon–oxygen sequence
    /// with a thick hydrogen layer (`q_H` = 10⁻⁴) of Bédard et al. (2020, ApJ 901, 93), as the
    /// Montreal group distributes it (`seq_060_thick.txt`,
    /// <https://www.astro.umontreal.ca/~bergeron/CoolingModels/>, retrieved 2026-09-23), at the
    /// model nearest each age from 0.01 to 10 Gyr, against Hurley and Shara's law at solar Z with
    /// T11's radius (0.012 78 R☉), `T_eff` = 5,772 K (L ÷ L☉)^¼ (R ÷ R☉)^−½.
    ///
    /// **The plan's 10% is not met.** Hurley and Shara's law is within 10% only at 0.01–0.02 Gyr
    /// and at 2–3 Gyr: it is up to 20% cool from 0.05 to 1 Gyr, where the model cools more slowly
    /// than their fit to Hansen (1999), and 11–17% cool from 5 to 10 Gyr, where crystallisation's
    /// latent heat and phase separation, which the Montreal models include, hold the dwarf warm.
    /// HPT's equation 90 is 9–45% cool at every age. So the test pins the law as better than
    /// equation 90 everywhere, and pins the measured deviations to half a percentage point, so that
    /// any change to the law is seen here; the bracket is the orchestrator's to rule (plan 06's
    /// Risks, T20.a).
    #[test]
    fn the_cooling_is_compared_with_bedard_et_al_2020() {
        // (model, age in years, T_eff in K) from the sequence, and the deviation of Hurley and
        // Shara's temperature from it, per cent, as measured.
        const MODELS: [(u32, f64, f64, f64); 13] = [
            (47, 10_144_770.0, 28_895.190_5, -0.89),
            (56, 20_839_830.0, 24_288.427_3, -4.51),
            (66, 50_198_350.0, 20_600.186_5, -13.06),
            (75, 99_924_400.0, 17_883.196, -18.23),
            (87, 201_932_700.0, 14_854.944_7, -20.00),
            (118, 499_565_200.0, 10_935.176_4, -16.80),
            (148, 1_001_402_000.0, 8_374.660_5, -11.51),
            (171, 1_957_222_000.0, 6_490.504_1, -6.30),
            (181, 3_074_970_000.0, 5_593.044_2, -4.83),
            (188, 4_881_002_000.0, 5_221.204_8, -11.04),
            (195, 6_901_942_000.0, 4_911.308, -14.62),
            (206, 8_993_564_000.0, 4_345.017_7, -10.74),
            (213, 9_985_524_000.0, 3_960.314_2, -17.26),
        ];
        let z = MetalFraction::new(0.02);
        let m = SolarMasses::new(0.6);
        let radius = crate::stellar::remnant::structure::white_dwarf_radius(
            RemnantRecipe::MandelMuller2020,
            m,
        );
        let teff = |l: SolarLuminosities| {
            crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K * math::powf(l.value(), 0.25)
                / radius.value().sqrt()
        };
        for (model, age, published, deviation) in MODELS {
            let age = Years::new(age);
            let ours = teff(luminosity(
                RemnantRecipe::MandelMuller2020,
                WhiteDwarfCore::CarbonOxygen,
                m,
                age,
                Megayears::ZERO,
                z,
            ));
            let hpt = teff(hpt_luminosity(WhiteDwarfCore::CarbonOxygen, m, age, z));
            let off = 100.0 * (ours / published - 1.0);
            assert!(
                (ours - published).abs() < (hpt - published).abs(),
                "model {model}: {ours} K and equation 90's {hpt} K against {published} K"
            );
            assert!(
                (off - deviation).abs() < 0.5,
                "model {model}: {off:.2}% against the recorded {deviation}%"
            );
        }
    }

    /// Pins both cooling laws and the cooling origin bit for bit, for the three compositions at
    /// three masses and ages on both of Hurley and Shara's pieces, so that any change to their
    /// arithmetic is seen.
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
