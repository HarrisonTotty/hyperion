//! The cooling of white dwarfs under the generator's default recipe (plan 06, P06.T20.a, ruling
//! 57.2 of 2026-09-22): the evolutionary sequences of Bédard et al. (2020, ApJ 901, 93), fitted by
//! `hyperion-fit run wd_cooling` into [`tables::wd_cooling`](crate::tables::wd_cooling).
//!
//! The table holds, at each of the 23 thick-hydrogen (DA) sequences' masses from 0.2 to 1.3 M☉,
//! log₁₀ of the cooling age plus 0.1 Myr at 96 luminosities. At a mass between two sequences the
//! clock is interpolated linearly in mass at fixed log L; the luminosity at a given age is then the
//! inverse of that column, which falls strictly, so the inverse exists and is continuous and the
//! luminosity falls with age at every mass. Past the sequences' ends each column goes on as a
//! straight line in log–log.
//!
//! # Where there is no sequence
//!
//! - **Cores of helium and of oxygen and neon.** The Montreal sequences all have equimassic
//!   carbon–oxygen cores. A dwarf's store of heat is its ions', so at a given temperature it is
//!   proportional to the number of ions per unit mass, 1 ÷ A with A the mean mass number, and in
//!   Mestel's law the time to cool to a given luminosity scales as 1 ÷ A (Mestel 1952, MNRAS 112,
//!   583; the A of Hurley, Pols and Tout 2000, section 6.2.1, and Hurley and Shara 2003,
//!   section 2). So a dwarf of another core reads the table at its law time × A ÷ `A_CO`
//!   ([`clock_scale`]), with A = 4 for helium, 16.7 for 80:20 oxygen–neon by mass (Hurley and
//!   Shara's mixture), and 13.7 for Montreal's 50:50 carbon–oxygen: a helium dwarf takes 3.4 times
//!   as long, and an oxygen–neon dwarf 0.82 times as long, as a carbon–oxygen one to reach a given
//!   luminosity. Both factors come from the Mestel scaling alone. Detailed models agree in
//!   direction only: helium-core dwarfs cool slowly (Althaus et al. 2013, A&A 557, A19), mostly
//!   by residual hydrogen burning rather than by their heat capacity, and carbon–oxygen
//!   ultra-massive dwarfs cool markedly slower than oxygen–neon ones, partly for their larger
//!   thermal content (Camisassa et al. 2022, MNRAS 511, 5198; the oxygen–neon models are
//!   Camisassa et al. 2019, A&A 625, A87).
//! - **Masses outside 0.2–1.3 M☉** read the nearest sequence: the oxygen–neon dwarfs up to the
//!   1.37 M☉ cap cool as 1.3 M☉ ones of their core, and helium dwarfs below 0.2 M☉ as 0.2 M☉ ones.
//! - **Metallicity.** The sequences take no progenitor metallicity, and neither does this law.
//!   Real cooling does depend on it a little, through residual hydrogen burning at low Z (Renedo et
//!   al. 2010, ApJ 717, 183) and the sedimentation of ²²Ne (Camisassa et al. 2016, ApJ 823, 158),
//!   which the sequences omit; Hurley and Shara's Z^0.4 was a fit of their law's brightness, not
//!   a model of either.
//! - **Below about 1,500 K**, past each sequence's last model, the table fades as Mestel's
//!   L ∝ t^−1.4 (`hyperion-fit`'s `FAINT_EXPONENT`). A heavy dwarf there is in the Debye regime and
//!   really fades faster, so this is an upper bound on its luminosity.

use crate::math;
use crate::tables::wd_cooling::{CLOCK_OFFSET_YR, LOG_CLOCK, LOG_LUMINOSITY_NODES, MASS_NODES};
use crate::units::{Megayears, SolarLuminosities, SolarMasses};

use super::white_dwarf::WhiteDwarfCore;

/// The number of luminosity nodes in each column.
const LUMINOSITIES: usize = LOG_LUMINOSITY_NODES.len();

/// The mean mass number of the Montreal sequences' cores, equal masses of ¹²C and ¹⁶O:
/// 1 ÷ (0.5 ÷ 12 + 0.5 ÷ 16) = 13.714.
const MONTREAL_MASS_NUMBER: f64 = 96.0 / 7.0;

/// The mean mass number of an oxygen–neon core of 80% ¹⁶O and 20% ²⁰Ne by mass (Hurley and Shara
/// 2003, section 2): 1 ÷ (0.8 ÷ 16 + 0.2 ÷ 20) = 16.667.
const OXYGEN_NEON_MASS_NUMBER: f64 = 50.0 / 3.0;

/// The mass number of a helium core.
const HELIUM_MASS_NUMBER: f64 = 4.0;

/// The factor by which a dwarf of `core` reads the carbon–oxygen table's clock: A ÷ `A_CO`, which is
/// 0.292 for helium, one for carbon–oxygen and 1.215 for oxygen–neon.
#[must_use]
fn clock_scale(core: WhiteDwarfCore) -> f64 {
    match core {
        WhiteDwarfCore::Helium => HELIUM_MASS_NUMBER / MONTREAL_MASS_NUMBER,
        WhiteDwarfCore::CarbonOxygen => 1.0,
        WhiteDwarfCore::OxygenNeon => OXYGEN_NEON_MASS_NUMBER / MONTREAL_MASS_NUMBER,
    }
}

/// The table's column at `mass`, clamped to the sequences' range: log₁₀ of the clock at each
/// luminosity node, interpolated linearly in mass between the two sequences either side.
#[must_use]
fn column(mass: SolarMasses) -> [f64; LUMINOSITIES] {
    let n = MASS_NODES.len();
    let m = mass.value().clamp(MASS_NODES[0], MASS_NODES[n - 1]);
    let i = MASS_NODES[1..n - 1]
        .iter()
        .take_while(|&&node| m >= node)
        .count();
    let f = (m - MASS_NODES[i]) / (MASS_NODES[i + 1] - MASS_NODES[i]);
    let (lower, upper) = (&LOG_CLOCK[i], &LOG_CLOCK[i + 1]);
    std::array::from_fn(|j| (1.0 - f) * lower[j] + f * upper[j])
}

/// The luminosity of a white dwarf of `mass` and `core`, `law_time` into the Montreal law: the
/// table's column at `mass` read at log₁₀((t + 0.1 Myr) × [`clock_scale`]), with t in years.
///
/// `law_time` is the dwarf's cooling age plus its origin (`white_dwarf::cooling_origin`), above
/// −0.1 Myr; the luminosity rises without bound as it approaches −0.1 Myr, past the table's
/// brightest node.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `law_time` is not above −0.1 Myr.
#[must_use]
pub(crate) fn luminosity(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    law_time: Megayears,
) -> SolarLuminosities {
    let clock_yr = (law_time.value() * 1e6 + CLOCK_OFFSET_YR) * clock_scale(core);
    debug_assert!(
        mass.value() > 0.0 && clock_yr > 0.0,
        "a white dwarf of {mass:?} {law_time:?} into its cooling"
    );
    let x = math::log10(clock_yr);
    let column = column(mass);
    let j = column[1..LUMINOSITIES - 1]
        .iter()
        .take_while(|&&c| x <= c)
        .count();
    let f = (x - column[j]) / (column[j + 1] - column[j]);
    let y = LOG_LUMINOSITY_NODES[j] + f * (LOG_LUMINOSITY_NODES[j + 1] - LOG_LUMINOSITY_NODES[j]);
    SolarLuminosities::new(math::exp10(y))
}

/// The inverse of [`luminosity`] in time: the point of the law at which a white dwarf of `mass`
/// and `core` has `luminosity`, always above −0.1 Myr.
///
/// # Panics
///
/// In debug builds, if `mass` or `luminosity` is not positive and finite.
#[must_use]
pub(crate) fn time_at(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    luminosity: SolarLuminosities,
) -> Megayears {
    debug_assert!(
        mass.value() > 0.0 && luminosity.value() > 0.0 && luminosity.value().is_finite(),
        "a white dwarf of {mass:?} at {luminosity:?}"
    );
    let y = math::log10(luminosity.value());
    let column = column(mass);
    let j = LOG_LUMINOSITY_NODES[1..LUMINOSITIES - 1]
        .iter()
        .take_while(|&&node| y >= node)
        .count();
    let f = (y - LOG_LUMINOSITY_NODES[j]) / (LOG_LUMINOSITY_NODES[j + 1] - LOG_LUMINOSITY_NODES[j]);
    let x = column[j] + f * (column[j + 1] - column[j]);
    Megayears::new((math::exp10(x) / clock_scale(core) - CLOCK_OFFSET_YR) * 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::remnant::RemnantRecipe;
    use crate::stellar::remnant::structure::white_dwarf_radius;
    use crate::units::consts::{SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_LUMINOSITY_W};

    const CORES: [WhiteDwarfCore; 3] = [
        WhiteDwarfCore::Helium,
        WhiteDwarfCore::CarbonOxygen,
        WhiteDwarfCore::OxygenNeon,
    ];

    fn co_at(mass: f64, age_yr: f64) -> f64 {
        luminosity(
            WhiteDwarfCore::CarbonOxygen,
            SolarMasses::new(mass),
            Megayears::new(age_yr * 1e-6),
        )
        .value()
    }

    /// P06.T20.a's check, as ruling 57.2 restates it: against the 0.6 M☉ thick-hydrogen sequence
    /// of Bédard et al. (2020), `T_eff` within 10% from 0.01 to 10 Gyr, at models the fit held
    /// out. These are every held-out model (number a multiple of five) of `seq_060_thick.txt`
    /// (<https://www.astro.umontreal.ca/~bergeron/CoolingModels/>, retrieved 2026-09-24) from
    /// 8.8 Myr to 10.2 Gyr, quoted as (model, age in years, `T_eff` in K). The temperature is the
    /// sim's: the table's luminosity at solar Z, since the law does not read it, and T11's radius
    /// (0.012 78 R☉), `T_eff` = 5,772 K (L ÷ L☉)^¼ (R ÷ R☉)^−½.
    ///
    /// The worst error is +5.5%, at the youngest model, and all of it is the radius: at 9 Myr the
    /// model's radius is 1.11 times T11's cold one, and the table's luminosity is within
    /// 0.01 dex of the model's (0.6%). Past 0.5 Gyr the error is under 2.2%, where T11's radius is
    /// 4% above the models' cold radius.
    #[test]
    fn the_cooling_meets_bedard_et_al_2020_at_held_out_models() {
        const MODELS: [(u32, f64, f64); 35] = [
            (45, 8_758_917.0, 30_141.726_5),
            (50, 12_744_730.0, 27_178.371),
            (55, 19_137_820.0, 24_725.068_2),
            (60, 29_627_230.0, 22_690.024_4),
            (65, 46_086_110.0, 20_929.481_9),
            (70, 69_489_350.0, 19_341.355_8),
            (75, 99_924_400.0, 17_883.196),
            (80, 137_312_100.0, 16_539.781_2),
            (85, 181_929_500.0, 15_312.605_1),
            (90, 234_298_000.0, 14_199.949_6),
            (95, 294_343_000.0, 13_193.118_8),
            (100, 355_750_300.0, 12_367.300_1),
            (105, 396_131_100.0, 11_904.564_7),
            (110, 432_684_300.0, 11_530.395_1),
            (115, 472_152_700.0, 11_166.662_8),
            (120, 519_288_300.0, 10_778.122_2),
            (125, 573_418_000.0, 10_382.794_9),
            (130, 638_495_700.0, 9_965.766_2),
            (135, 718_828_200.0, 9_521.441_4),
            (140, 814_883_900.0, 9_070.503_2),
            (145, 926_509_800.0, 8_630.429),
            (150, 1_054_896_000.0, 8_207.917_4),
            (155, 1_202_310_000.0, 7_805.043_8),
            (160, 1_369_830_000.0, 7_425.368_7),
            (165, 1_554_088_000.0, 7_076.987_7),
            (170, 1_844_462_000.0, 6_635.09),
            (175, 2_364_268_000.0, 6_050.225_5),
            (180, 2_917_002_000.0, 5_656.427_4),
            (185, 3_989_304_000.0, 5_363.738_8),
            (190, 5_508_258_000.0, 5_130.547_7),
            (195, 6_901_942_000.0, 4_911.308),
            (200, 7_974_398_000.0, 4_655.191),
            (205, 8_836_190_000.0, 4_398.765_3),
            (210, 9_580_760_000.0, 4_121.673_8),
            (215, 10_246_660_000.0, 3_857.009_6),
        ];
        let m = SolarMasses::new(0.6);
        let radius = white_dwarf_radius(RemnantRecipe::MandelMuller2020, m).value();
        let mut worst = 0.0_f64;
        for (model, age, published) in MODELS {
            let l = co_at(0.6, age);
            let ours = SOLAR_EFFECTIVE_TEMPERATURE_K * math::powf(l, 0.25) / radius.sqrt();
            let off = ours / published - 1.0;
            assert!(
                off.abs() < 0.10,
                "model {model} at {age} yr: {ours} K against {published} K"
            );
            let bound = if age > 5e8 { 0.022 } else { 0.056 };
            assert!(
                off.abs() < bound,
                "model {model}: {:.2}% against {bound}",
                100.0 * off
            );
            if off.abs() > worst.abs() {
                worst = off;
            }
        }
        eprintln!("the worst T_eff error at the held-out models: {worst}");
        assert!(worst > 0.05 && worst < 0.056, "the worst error is {worst}");
    }

    /// Held-out models across the range, in luminosity: at 0.2, 0.45, 0.9 and 1.3 M☉, models
    /// 50, 100, 150 and 200 of the Montreal files, quoted as (M☉, age in years, L in erg s⁻¹),
    /// within 0.02 dex.
    #[test]
    fn the_cooling_meets_held_out_models_across_the_masses() {
        const MODELS: [(f64, f64, f64); 15] = [
            (0.2, 2.325_716e8, 8.754_622e30),
            (0.2, 1.581_252e9, 8.011_172e29),
            (0.2, 1.008_134e10, 1.952_803e28),
            (0.45, 6.817_389e7, 7.541_452e31),
            (0.45, 4.528_969e8, 7.948_195e30),
            (0.45, 1.566_454e9, 1.065_700e30),
            (0.45, 9.126_276e9, 8.294_920e28),
            (0.9, 2.694_336e7, 2.903_332e32),
            (0.9, 4.950_255e8, 1.227_492e31),
            (0.9, 3.288_722e9, 9.399_400e29),
            (0.9, 1.164_692e10, 5.879_264e28),
            (1.3, 6.625_131e7, 3.509_216e32),
            (1.3, 1.290_648e9, 4.312_752e30),
            (1.3, 3.483_982e9, 1.519_577e29),
            (1.3, 5.484_617e9, 9.081_729e27),
        ];
        for (mass, age, erg_s) in MODELS {
            let published = erg_s / (SOLAR_LUMINOSITY_W * 1e7);
            let d = math::log10(co_at(mass, age) / published);
            assert!(d.abs() < 0.02, "{mass} M☉ at {age} yr: {d} dex");
        }
    }

    /// The luminosity falls with age at every mass and for every core, from the brightest
    /// hand-over to 15 Gyr, and is continuous: no stride of a small share of the clock moves it by
    /// more than a small amount.
    #[test]
    fn the_luminosity_falls_with_age_and_has_no_step() {
        for core in CORES {
            for k in 0..=24 {
                let mass = SolarMasses::new(0.15 + 0.05 * f64::from(k));
                let mut last = f64::INFINITY;
                for i in 0..=3_000 {
                    let x = f64::from(i) / 3_000.0;
                    let law_time = Megayears::new(-0.099_9 + 15_000.0 * x * x * x * x);
                    let l = luminosity(core, mass, law_time).value();
                    assert!(
                        l > 0.0 && l < last,
                        "{core:?} {mass:?} at {law_time:?}: {l} after {last}"
                    );
                    if i > 1 {
                        assert!(
                            math::log10(last / l) < 0.2,
                            "{core:?} {mass:?} at {law_time:?}: {last} to {l}"
                        );
                    }
                    last = l;
                }
            }
        }
    }

    /// Continuity in mass across the fitted range: at fixed age the luminosity moves by little
    /// between masses a thousandth of a solar mass apart, at the sequences' own masses as between
    /// them, by next to nothing between masses 10⁻⁷ M☉ apart, and it meets each sequence at its
    /// mass.
    ///
    /// Up to 3 Gyr no step of 0.001 M☉ moves it by 0.015 dex. At 10 Gyr the steps reach 0.12 dex
    /// between 1.10 and 1.15 M☉, smoothly: the 1.10 M☉ sequence is still in its Debye plunge
    /// there and the 1.15 M☉ one has ended, so the luminosity falls by about 1.5 dex across the
    /// interval, 10⁻⁶·⁵ to 10⁻⁸ L☉, below 2,000 K.
    #[test]
    fn the_luminosity_is_continuous_in_mass() {
        for age in [1e6, 1e7, 1e8, 1e9, 3e9, 1e10] {
            let bound = if age > 5e9 { 0.15 } else { 0.015 };
            let mut last = co_at(0.2, age);
            for k in 1..=1_100 {
                let m = 0.2 + f64::from(k) * 1e-3;
                let l = co_at(m, age);
                let d = math::log10(l / last).abs();
                assert!(d < bound, "{age} yr, {m} M☉: {d} dex from the last");
                let near = math::log10(co_at(m + 1e-7, age) / l).abs();
                assert!(near < 1e-4, "{age} yr, {m} M☉: {near} dex over 10⁻⁷ M☉");
                last = l;
            }
            for &node in &MASS_NODES {
                let at = co_at(node, age);
                let (below, above) = (co_at(node - 1e-9, age), co_at(node + 1e-9, age));
                assert!((below / at - 1.0).abs() < 1e-6 && (above / at - 1.0).abs() < 1e-6);
            }
        }
        // Outside the range the nearest sequence holds.
        let l = co_at(1.3, 1e9);
        assert!((co_at(1.37, 1e9) / l - 1.0).abs() < 1e-15);
        assert!((co_at(0.1, 1e9) / co_at(0.2, 1e9) - 1.0).abs() < 1e-15);
    }

    /// Inverting the law and applying it again returns the luminosity, from beyond the table's
    /// brightest node to beyond its faintest, and the clock of a lighter core runs slower.
    #[test]
    fn the_time_inverts_the_luminosity() {
        for core in CORES {
            for m in [0.17, 0.6, 0.93, 1.37] {
                let mass = SolarMasses::new(m);
                for log_l in [4.0, 2.5, 1.3, 0.0, -2.2, -4.7, -7.5, -8.5] {
                    let l = SolarLuminosities::new(math::exp10(log_l));
                    let t = time_at(core, mass, l);
                    assert!(t.value() > -0.1, "{core:?} {m} at {log_l}: {t:?}");
                    let back = luminosity(core, mass, t).value();
                    assert!(
                        (math::log10(back) - log_l).abs() < 1e-9,
                        "{core:?} {m} at {log_l}: {back}"
                    );
                }
            }
        }
        let mass = SolarMasses::new(0.6);
        let l = SolarLuminosities::new(1e-3);
        let t = |core| time_at(core, mass, l).value();
        let co = t(WhiteDwarfCore::CarbonOxygen);
        assert!(t(WhiteDwarfCore::Helium) > 3.0 * co && t(WhiteDwarfCore::OxygenNeon) < co);
    }
}
