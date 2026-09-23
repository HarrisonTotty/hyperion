//! The structure of what a star leaves (plan 06, P06.T11; Hurley, Pols and Tout 2000, MNRAS 315,
//! 543, "HPT", sections 5.4, 6.1 and 6.2): which kind of white dwarf an envelope loss exposes, the
//! radii of white dwarfs, neutron stars and black holes, and HPT's original masses of neutron stars
//! and black holes, which only [`RemnantRecipe::Hurley2000`] uses.
//!
//! Masses are in M☉ and radii in R☉ (the nominal values of IAU 2015 Resolution B3), as in
//! [`StarState`](crate::stellar::StarState). Equation numbers are the journal's. The published SSE
//! code (`hrdiag.f`) agrees with the paper on everything here except two guards for white dwarfs
//! far lighter than any single star leaves, named at [`white_dwarf_radius`].

use crate::math;
use crate::stellar::Phase;
use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::{SolarMasses, SolarRadii};

use super::{CompactRemnant, RemnantKind, RemnantRecipe};

/// The Chandrasekhar mass HPT use at all times, 1.44 M☉ (section 6.2.1): they write it as about
/// 5.8 ÷ μₑ² M☉, 1.45 for a mean molecular weight per electron μₑ = 2, and take 1.44.
pub(crate) const CHANDRASEKHAR_MASS: SolarMasses = SolarMasses::new(1.44);

/// The core mass at the base of the AGB from which carbon ignites off-centre under
/// semi-degenerate conditions, so that the white dwarf left is oxygen–neon rather than
/// carbon–oxygen: 1.6 M☉, the `M_c,BAGB` of the initial mass HPT call `M_up` (section 5.4, after
/// Pols et al. 1998).
pub(crate) const OXYGEN_NEON_MC_BAGB: SolarMasses = SolarMasses::new(1.6);

/// The largest neutron-star mass, and the smallest black-hole mass, of HPT's original prescription:
/// 1.8 M☉, where [`hurley_supernova_remnant`]'s equation 92 meets their black-hole criterion
/// Mc,SN > 7.0 M☉ (section 6.2.2; SSE's `mxns` with `nsflag` = 0).
pub(crate) const HURLEY_MAX_NEUTRON_STAR_MASS: SolarMasses = SolarMasses::new(1.8);

/// HPT equation 92: M = 1.17 + 0.09 Mc,SN M☉, the (constant, slope).
const HURLEY_REMNANT_MASS: (f64, f64) = (1.17, 0.09);

/// The scale of the white-dwarf mass–radius relation, R☉ (HPT equation 91, after Tout et al.
/// 1997).
const WHITE_DWARF_RADIUS_SCALE: f64 = 0.0115;

/// HPT's neutron-star radius, 1.4 × 10⁻⁵ R☉ (section 6.2.2), which they call 10 km; it is 9.74 km
/// with the nominal R☉, and SSE holds the same 1.4 × 10⁻⁵.
const HURLEY_NEUTRON_STAR_RADIUS: SolarRadii = SolarRadii::new(1.4e-5);

/// The neutron-star radius, metres: 12.2 km, the radius of a 1.4 M☉ neutron star that Koehn et
/// al. (2025, Phys. Rev. X 15, 021014) find by combining every constraint they compile, 12.20
/// (+0.50 −0.48) km at 95% credibility. Nuclear theory and experiment are combined there with
/// the NICER pulse-profile radii of PSR J0030+0451 (Riley et al. 2019, ApJ 887, L21: 12.71 km;
/// Miller et al. 2019, ApJ 887, L24: 13.02 km) and PSR J0740+6620 (Salmi et al. 2024, ApJ 974,
/// 294: 12.49 km; Dittmann et al. 2024, ApJ 974, 295: 12.92 km), the gravitational waves and
/// kilonova of GW170817 and the signal of GW190425, and other X-ray and radio measurements.
/// Other combined analyses centre within 0.4 km of it: 12.45 ± 0.65 km (Miller et al. 2021, ApJ
/// 918, L28), 12.33 and 12.18 km (Raaijmakers et al. 2021, ApJ 918, L29), 12.28 and 12.01 km
/// with PSR J0437−4715 (Rutherford et al. 2024, ApJ 971, L19), and 11.8–11.9 km in analyses of
/// 2025–2026 that add PSR J0614−3329 (Imam et al. 2025, Phys. Rev. D, arXiv:2509.07109; Jacobi
/// et al. 2026, arXiv:2608.04092). Gravitational waves with nuclear theory alone give 11.0 (+0.9
/// −0.6) km at 90% (Capano et al. 2020, Nature Astronomy 4, 625). Real radii change by only a few
/// tenths of a kilometre from 1.2 to 2.0 M☉, so one radius serves every mass.
const NEUTRON_STAR_RADIUS_M: f64 = 12_200.0;

/// HPT's Schwarzschild radius per M☉, R☉ (equation 94), as SSE holds it: 2GM☉ ÷ c² in an R☉ of
/// 6.96 × 10⁸ m (4.2432 × 10⁻⁶) rounded, 0.12% below the value in the nominal R☉.
const HURLEY_SCHWARZSCHILD_PER_SOLAR_MASS: f64 = 4.24e-6;

/// The Schwarzschild radius per M☉, 2GM☉ ÷ c², in nominal R☉: 2,953.25 m, from the nominal GM☉
/// and R☉ of IAU 2015 Resolution B3 and the exact speed of light.
const SCHWARZSCHILD_PER_SOLAR_MASS: f64 =
    2.0 * GM_SUN / (SPEED_OF_LIGHT * SPEED_OF_LIGHT) / SOLAR_RADIUS_M;

/// The degenerate core that a star's envelope loss exposes, which decides the kind of white dwarf
/// it becomes (HPT sections 5.4, 6.1 and 6.2.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DegenerateCore {
    /// The helium core of a star whose envelope is lost on the Hertzsprung gap or the first giant
    /// branch while its core is degenerate (initial mass up to `M_HeF`). Above `M_HeF` the same
    /// loss leaves a naked helium star instead, which is not a white dwarf.
    Helium,
    /// The carbon–oxygen core of a star whose envelope is lost on the AGB, or of an evolved naked
    /// helium star, with the core mass at the base of the AGB, `M_c,BAGB`; for a naked helium star
    /// HPT use the helium star's initial mass in its place (section 6.1).
    CarbonOxygen {
        /// `M_c,BAGB`, M☉.
        mc_bagb: SolarMasses,
    },
}

/// The kind of white dwarf an exposed `core` becomes: helium; carbon–oxygen when `M_c,BAGB` is
/// below 1.6 M☉; oxygen–neon from 1.6 M☉, where carbon burned in the degenerate core (HPT sections
/// 5.4 and 6.2.1).
///
/// Only a core below the Chandrasekhar mass leaves a white dwarf; the track decides that, and what
/// happens otherwise, before it asks.
#[must_use]
pub(crate) fn white_dwarf_kind(core: DegenerateCore) -> Phase {
    match core {
        DegenerateCore::Helium => Phase::HeliumWhiteDwarf,
        DegenerateCore::CarbonOxygen { mc_bagb } => {
            if mc_bagb < OXYGEN_NEON_MC_BAGB {
                Phase::CarbonOxygenWhiteDwarf
            } else {
                Phase::OxygenNeonWhiteDwarf
            }
        }
    }
}

/// The radius of a white dwarf of `mass`, R☉ (HPT equation 91, after Tout et al. 1997):
/// max(`R_NS`, 0.0115 √((`M_Ch` ÷ M)^⅔ − (M ÷ `M_Ch`)^⅔)), with `M_Ch` the
/// [`CHANDRASEKHAR_MASS`] and `R_NS` the recipe's [`neutron_star_radius`].
///
/// The relation falls as the mass rises and vanishes at `M_Ch`, so the neutron star's radius is a
/// floor that binds only within 3 × 10⁻⁶ M☉ of `M_Ch` (and at or beyond it). A 0.6 M☉ dwarf has
/// 0.012 78 R☉, 8,900 km. SSE adds two guards the paper does not print, for white dwarfs of
/// under 0.002 M☉ that only mass transfer could make: a cap of 0.1 R☉ and fixed radii below
/// 5 × 10⁻⁴ M☉. They are left out.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
#[must_use]
pub(crate) fn white_dwarf_radius(recipe: RemnantRecipe, mass: SolarMasses) -> SolarRadii {
    let floor = neutron_star_radius(recipe);
    let relation = SolarRadii::new(mass_radius_relation(mass.value()));
    if relation > floor { relation } else { floor }
}

/// 0.0115 √((`M_Ch` ÷ M)^⅔ − (M ÷ `M_Ch`)^⅔) R☉ for a mass `m` in M☉, and zero from `M_Ch` up.
#[must_use]
fn mass_radius_relation(m: f64) -> f64 {
    debug_assert!(
        m.is_finite() && m > 0.0,
        "a white dwarf has a positive mass: {m}"
    );
    let m_ch = CHANDRASEKHAR_MASS.value();
    let radicand = math::powf(m_ch / m, 2.0 / 3.0) - math::powf(m / m_ch, 2.0 / 3.0);
    if radicand > 0.0 {
        WHITE_DWARF_RADIUS_SCALE * radicand.sqrt()
    } else {
        0.0
    }
}

/// The radius of a neutron star of any mass, R☉: HPT's 1.4 × 10⁻⁵ R☉ under
/// [`RemnantRecipe::Hurley2000`], and 12.2 km, 1.754 × 10⁻⁵ R☉, from current measurements under
/// the default (see [`NEUTRON_STAR_RADIUS_M`] for the sources).
#[must_use]
pub(crate) fn neutron_star_radius(recipe: RemnantRecipe) -> SolarRadii {
    match recipe {
        RemnantRecipe::Hurley2000 => HURLEY_NEUTRON_STAR_RADIUS,
        RemnantRecipe::MandelMuller2020 => SolarRadii::new(NEUTRON_STAR_RADIUS_M / SOLAR_RADIUS_M),
    }
}

/// The radius of a black hole of `mass`, R☉: its Schwarzschild radius 2GM ÷ c², 2.953 km per M☉,
/// with HPT's rounded 4.24 × 10⁻⁶ R☉ per M☉ (equation 94) under [`RemnantRecipe::Hurley2000`].
#[must_use]
pub(crate) fn black_hole_radius(recipe: RemnantRecipe, mass: SolarMasses) -> SolarRadii {
    let per_solar_mass = match recipe {
        RemnantRecipe::Hurley2000 => HURLEY_SCHWARZSCHILD_PER_SOLAR_MASS,
        RemnantRecipe::MandelMuller2020 => SCHWARZSCHILD_PER_SOLAR_MASS,
    };
    SolarRadii::new(per_solar_mass * mass.value())
}

/// The remnant of a supernova under HPT's original prescription, from `mc_sn`, the
/// carbon–oxygen core mass at the explosion (M☉, at least `M_Ch` by their equation 75):
/// gravitational mass 1.17 + 0.09 Mc,SN M☉ (equation 92), a neutron star up to
/// [`HURLEY_MAX_NEUTRON_STAR_MASS`] and a black hole above it, so the kind changes at
/// Mc,SN = 7.0 M☉ (section 6.2.2).
///
/// This is SSE's `nsflag` = 0 with `mxns` = 1.8; the input file SSE distributes selects instead
/// the remnant masses of Belczynski et al. (2002) and a largest neutron star of 3 M☉.
///
/// # Panics
///
/// In debug builds, if `mc_sn` is not finite and non-negative.
#[must_use]
pub(crate) fn hurley_supernova_remnant(mc_sn: SolarMasses) -> CompactRemnant {
    debug_assert!(
        mc_sn.value().is_finite() && mc_sn.value() >= 0.0,
        "a core mass is finite and non-negative: {mc_sn:?}"
    );
    let (constant, slope) = HURLEY_REMNANT_MASS;
    let mass = SolarMasses::new(constant + slope * mc_sn.value());
    let kind = if mass <= HURLEY_MAX_NEUTRON_STAR_MASS {
        RemnantKind::NeutronStar
    } else {
        RemnantKind::BlackHole
    };
    CompactRemnant::new(kind, mass)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Metres;
    use hyperion_testkit::float::assert_same_bits;

    const RECIPES: [RemnantRecipe; 2] =
        [RemnantRecipe::Hurley2000, RemnantRecipe::MandelMuller2020];

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    #[track_caller]
    fn assert_close(what: &str, actual: f64, expected: f64, relative: f64) {
        assert!(
            (actual / expected - 1.0).abs() < relative,
            "{what}: {actual:e} against {expected:e}"
        );
    }

    #[test]
    fn the_exposed_core_decides_the_kind_of_white_dwarf() {
        assert_eq!(
            white_dwarf_kind(DegenerateCore::Helium),
            Phase::HeliumWhiteDwarf
        );
        let carbon_oxygen = |mc_bagb| {
            white_dwarf_kind(DegenerateCore::CarbonOxygen {
                mc_bagb: mass(mc_bagb),
            })
        };
        for (mc_bagb, kind) in [
            (0.5, Phase::CarbonOxygenWhiteDwarf),
            (1.2, Phase::CarbonOxygenWhiteDwarf),
            (1.6 - 1e-12, Phase::CarbonOxygenWhiteDwarf),
            (1.6, Phase::OxygenNeonWhiteDwarf),
            (2.2, Phase::OxygenNeonWhiteDwarf),
        ] {
            assert_eq!(carbon_oxygen(mc_bagb), kind, "M_c,BAGB = {mc_bagb}");
        }
    }

    /// The plan's check: a 0.6 M☉ white dwarf has a radius of 0.012–0.013 R☉ (equation 91 gives
    /// 0.012 78 R☉), whichever recipe sets the floor.
    #[test]
    fn a_typical_white_dwarf_is_the_size_of_the_earth() {
        for recipe in RECIPES {
            let r = white_dwarf_radius(recipe, mass(0.6)).value();
            assert!((0.012..=0.013).contains(&r), "{recipe:?}: {r}");
        }
    }

    /// The relation falls monotonically with mass over 0.05–1.44 M☉ and vanishes at the
    /// Chandrasekhar mass, where the radius meets its floor, the neutron star's.
    #[test]
    fn the_radius_falls_with_mass_and_vanishes_at_the_chandrasekhar_mass() {
        let m_ch = CHANDRASEKHAR_MASS.value();
        let mut last = f64::INFINITY;
        for i in 0..=2_000 {
            let m = 0.05 + (m_ch - 0.05) * f64::from(i) / 2_000.0;
            let r = mass_radius_relation(m);
            assert!(r < last, "the relation does not fall at {m} M☉: {r}");
            last = r;
        }
        assert_same_bits(mass_radius_relation(m_ch), 0.0);
        assert_same_bits(mass_radius_relation(1.5), 0.0);
        // Close to `M_Ch` the relation goes as the square root of the distance to it.
        let near = mass_radius_relation(m_ch * (1.0 - 1e-6));
        assert!(near < 2e-5 && near > 0.0, "{near}");
        for recipe in RECIPES {
            let floor = neutron_star_radius(recipe);
            let at = |m: f64| white_dwarf_radius(recipe, mass(m)).value();
            assert_same_bits(at(m_ch), floor.value());
            assert_same_bits(at(m_ch * (1.0 - 1e-7)), floor.value());
            assert!(
                at(1.43) > floor.value(),
                "{recipe:?}: {} at 1.43 M☉",
                at(1.43)
            );
        }
    }

    /// White-dwarf, neutron-star and black-hole radii from the published SSE code's `hrdiag`
    /// (see [`sse`](crate::stellar::sse) for the package; run of 2026-09-23, Z = 0.02, cooling age
    /// 1 Myr, `nsflag` = 0, `mxns` = 1.8), to 10⁻⁹. SSE gives helium, carbon–oxygen and oxygen–neon
    /// dwarfs of one mass the same radius.
    #[test]
    fn remnant_radii_match_the_published_sse_code() {
        let recipe = RemnantRecipe::Hurley2000;
        for &(kw, m, r_sse) in SSE_REMNANT_RADII {
            let r = match kw {
                11 => white_dwarf_radius(recipe, mass(m)),
                13 => neutron_star_radius(recipe),
                14 => black_hole_radius(recipe, mass(m)),
                _ => unreachable!("no row of type {kw}"),
            };
            assert_close(&format!("type {kw}, {m} M☉"), r.value(), r_sse, 1e-9);
        }
    }

    /// (SSE type, M, R from `hrdiag`).
    const SSE_REMNANT_RADII: &[(u8, f64, f64)] = &[
        (11, 0.15, 0.023_834_664_119_317_41),
        (11, 0.3, 0.018_161_570_897_203_53),
        (11, 0.45, 0.015_042_730_407_869_517),
        (11, 0.6, 0.012_778_467_098_439_529),
        (11, 0.8, 0.010_311_127_424_985_122),
        (11, 1.0, 0.008_058_157_517_676_339),
        (11, 1.2, 0.005_677_022_808_780_932),
        (11, 1.35, 0.003_373_989_345_368_869),
        (11, 1.43, 0.001_108_518_430_019_855_4),
        (11, 1.4399, 0.000_110_660_722_846_386_97),
        (13, 1.4, 1.4e-5),
        (13, 1.8, 1.4e-5),
        (14, 3.0, 1.272e-5),
        (14, 30.0, 0.000_127_2),
    ];

    /// HPT's neutron star is 1.4 × 10⁻⁵ R☉; the default is 12.2 km.
    #[test]
    fn neutron_stars_have_one_radius_per_recipe() {
        assert_same_bits(
            neutron_star_radius(RemnantRecipe::Hurley2000).value(),
            1.4e-5,
        );
        let modern = Metres::from(neutron_star_radius(RemnantRecipe::MandelMuller2020));
        assert_close("12.2 km", modern.value(), 12_200.0, 1e-15);
        let hurley = Metres::from(neutron_star_radius(RemnantRecipe::Hurley2000));
        assert!((hurley.value() - 9_740.0).abs() < 1.0, "{hurley:?}");
    }

    /// A black hole's radius is 2GM ÷ c²: 29.53 km for 10 M☉ (2 × 1.327 124 4 × 10²¹ m³ s⁻² ÷ c²),
    /// and HPT's 4.24 × 10⁻⁵ R☉ under their recipe.
    #[test]
    fn a_black_hole_is_its_schwarzschild_radius() {
        let modern = Metres::from(black_hole_radius(
            RemnantRecipe::MandelMuller2020,
            mass(10.0),
        ));
        assert_close("2GM ÷ c²", modern.value(), 29_532.500_761_002_5, 1e-12);
        let hurley = black_hole_radius(RemnantRecipe::Hurley2000, mass(10.0));
        assert_close("HPT", hurley.value(), 4.24e-5, 1e-15);
        // The rounding of HPT's constant is 0.12%.
        assert_close(
            "rounding",
            hurley / black_hole_radius(RemnantRecipe::MandelMuller2020, mass(10.0)),
            1.0,
            1.2e-3,
        );
    }

    /// Under HPT's prescription a supernova leaves 1.17 + 0.09 Mc,SN M☉, a neutron star up to
    /// 1.8 M☉ and a black hole above it: the kind changes exactly where the formula passes the
    /// largest neutron-star mass, at Mc,SN = 7.0 M☉, and nowhere else, while the mass itself
    /// runs on without a jump.
    #[test]
    fn hurleys_remnant_turns_from_neutron_star_to_black_hole_where_its_mass_passes_1_8() {
        let (constant, slope) = HURLEY_REMNANT_MASS;
        let crossing = (HURLEY_MAX_NEUTRON_STAR_MASS.value() - constant) / slope;
        assert!((crossing - 7.0).abs() < 1e-12, "{crossing}");
        let below = hurley_supernova_remnant(mass(crossing - 1e-9));
        let above = hurley_supernova_remnant(mass(crossing + 1e-9));
        assert_eq!(below.kind(), RemnantKind::NeutronStar);
        assert_eq!(above.kind(), RemnantKind::BlackHole);
        let step = above.mass().value() - below.mass().value();
        assert!(
            step.abs() < 1e-9,
            "the mass steps by {step:e} at the switch"
        );
        let mut changes = 0;
        let mut last = hurley_supernova_remnant(CHANDRASEKHAR_MASS);
        assert_close("lightest", last.mass().value(), 1.299_6, 1e-12);
        assert_eq!(last.kind(), RemnantKind::NeutronStar);
        for i in 1..=3_000 {
            let mc = CHANDRASEKHAR_MASS.value() + 30.0 * f64::from(i) / 3_000.0;
            let remnant = hurley_supernova_remnant(mass(mc));
            assert!(remnant.mass() > last.mass(), "mass falls at Mc,SN = {mc}");
            if remnant.kind() != last.kind() {
                changes += 1;
                assert_eq!(remnant.kind(), RemnantKind::BlackHole, "at {mc}");
                assert!(
                    mc > crossing && mc - crossing <= 0.01,
                    "the kind changes at {mc}"
                );
            }
            last = remnant;
        }
        assert_eq!(changes, 1);
    }

    /// Supernova remnants of stars evolved at constant mass by the published SSE code's
    /// `hrdiag` to the end of their nuclear life (run of 2026-09-23, `nsflag` = 0, `mxns` = 1.8),
    /// from its Mc,SN, to 10⁻⁹: kind and mass.
    #[test]
    fn supernova_remnants_match_the_published_sse_code() {
        for &(kw, z, m0, mc_sn, m_sse) in SSE_SUPERNOVAE {
            let remnant = hurley_supernova_remnant(mass(mc_sn));
            let kind = if kw == 13 {
                RemnantKind::NeutronStar
            } else {
                RemnantKind::BlackHole
            };
            assert_eq!(remnant.kind(), kind, "{m0} M☉ at Z = {z}");
            assert_close(
                &format!("{m0} M☉ at Z = {z}"),
                remnant.mass().value(),
                m_sse,
                1e-9,
            );
        }
    }

    /// (SSE type, Z, initial mass, Mc,SN, remnant mass from `hrdiag`).
    const SSE_SUPERNOVAE: &[(u8, f64, f64, f64, f64)] = &[
        (
            13,
            0.0001,
            12.0,
            2.922_883_287_478_894,
            1.433_059_495_873_100_4,
        ),
        (
            13,
            0.0001,
            20.0,
            6.358_611_519_193_109,
            1.742_275_036_727_379_6,
        ),
        (
            14,
            0.0001,
            25.0,
            8.068_352_173_025_737,
            1.896_151_695_572_316_3,
        ),
        (
            14,
            0.0001,
            80.0,
            32.570_088_717_341_75,
            4.101_307_984_560_757,
        ),
        (
            13,
            0.004,
            20.0,
            5.777_624_571_266_042,
            1.689_986_211_413_943_7,
        ),
        (
            14,
            0.004,
            25.0,
            7.651_941_886_974_232,
            1.858_674_769_827_680_9,
        ),
        (13, 0.02, 8.0, 1.44, 1.299_599_999_999_999_9),
        (
            13,
            0.02,
            20.0,
            5.457_632_020_673_298,
            1.661_186_881_860_596_8,
        ),
        (
            14,
            0.02,
            25.0,
            7.405_039_894_135_53,
            1.836_453_590_472_197_7,
        ),
        (
            14,
            0.02,
            40.0,
            13.509_241_483_546_642,
            2.385_831_733_519_198,
        ),
    ];
}
