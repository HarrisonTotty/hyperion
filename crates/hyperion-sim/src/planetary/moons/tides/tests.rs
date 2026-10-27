//! P14.T56.d's checks at measured elements (decision-p14-moons §3.3, decision-p14-t56d-membrane
//! §3 and §4.4).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::derive::rotation::{
    SpinningBody, moment_of_inertia_factor, tidal_locking_time,
};
use crate::planetary::derive::solar::SOLAR_AGE;
use crate::planetary::derive::{MassFractions, PlanetClass};
use crate::planetary::hooks::figures::{HeatInputs, Subsurface, non_tidal_heat_flow};
use crate::planetary::moons::regular::{MOON_PRIMORDIAL_PERIOD_HOURS, ocean_layers};
use crate::planetary::params::{ICE_MELTING_VISCOSITY_PA_S, ICE_POISSON_RATIO};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Dex, EarthMasses, Seconds};

/// A moon at its measured elements, and what its tides read beyond them.
#[derive(Debug, Clone, Copy)]
struct Measured {
    name: &'static str,
    /// Its GM, km³ s⁻².
    gm: f64,
    /// Its mean radius, km.
    radius_km: f64,
    /// Its planet's GM, km³ s⁻².
    planet_gm: f64,
    /// Its semi-major axis, km.
    a_km: f64,
    /// Its eccentricity.
    e: f64,
    /// Its sidereal orbital period, days.
    period_days: f64,
    /// The mean temperature at its surface, K.
    surface: f64,
    /// Its water mass fraction.
    water: f64,
}

/// Jupiter's, Saturn's and Earth's GM, km³ s⁻² (JPL's "Planet GM values", JUP365, SAT441 and
/// DE440, read 2026-10-16).
const JUPITER_GM: f64 = 126_686_531.9;
const SATURN_GM: f64 = 37_931_206.23;
const EARTH_GM: f64 = 398_600.436;

/// The moons the checks read. GM and mean radius are JPL's "Planetary Satellite Physical
/// Parameters" (JUP365, SAT441, DE440; radii of Archinal et al. 2018), read 2026-10-16. The
/// semi-major axes are JPL's "Planetary Satellite Mean Elements" for Jupiter's moons, Titan and
/// the Moon, and NASA's Saturnian satellite fact sheet (revised 2025-07-22) for Mimas, Enceladus,
/// Dione, Rhea and Iapetus, all read 2026-10-16; the sidereal periods are NASA's Jovian and
/// Saturnian satellite fact sheets' and JPL's for the Moon. The eccentricities are the values the
/// tides read, to two figures where the tables round them: Io's 0.0041 and Europa's 0.0094 as the
/// moons' other tests take them, Ganymede's 0.0013 and Callisto's 0.0074 as decision-p14-moons's
/// checks do (JPL's 0.004, 0.009, 0.001 and 0.007; no primary source read for the second figure),
/// and Enceladus's 0.0047, "the current eccentricity" of Meyer and Wisdom (2007, Icarus 188, 535)
/// and Beuthe 2019's (Icarus 332, 66, Table 1) mean eccentricity (the fact sheet's 0.0045); Mimas's
/// 0.0202, Dione's 0.0022, Rhea's 0.0010, Titan's 0.0292 and Iapetus's 0.0283 are the fact
/// sheet's, the Moon's 0.0554 JPL's. The surface temperatures and water fractions are
/// decision-p14-moons's (its `membrane.py`): Europa 102 K and 8% water, Ganymede 110 K and 45%, Callisto 125 K and 45%,
/// Titan 94 K and 42%, Enceladus 75 K and 40%, Dione 87 K and 50%, Mimas 64 K and 35%. Rhea's and
/// Iapetus's water is what their densities hold at ruling 83.7's rock and ice (0.66 and 0.81), at
/// a nominal 76 K and 90 K; both are far from an ocean at any of these.
const MOONS: [Measured; 11] = [
    Measured {
        name: "Moon",
        gm: 4_902.800,
        radius_km: 1_737.4,
        planet_gm: EARTH_GM,
        a_km: 384_400.0,
        e: 0.0554,
        period_days: 27.322,
        surface: 250.0,
        water: 0.0,
    },
    Measured {
        name: "Io",
        gm: 5_959.915_47,
        radius_km: 1_821.49,
        planet_gm: JUPITER_GM,
        a_km: 421_800.0,
        e: 0.0041,
        period_days: 1.769_138,
        surface: 110.0,
        water: 0.0,
    },
    Measured {
        name: "Europa",
        gm: 3_202.712_10,
        radius_km: 1_560.80,
        planet_gm: JUPITER_GM,
        a_km: 671_100.0,
        e: 0.0094,
        period_days: 3.551_181,
        surface: 102.0,
        water: 0.08,
    },
    Measured {
        name: "Ganymede",
        gm: 9_887.832_75,
        radius_km: 2_631.20,
        planet_gm: JUPITER_GM,
        a_km: 1_070_400.0,
        e: 0.0013,
        period_days: 7.154_553,
        surface: 110.0,
        water: 0.45,
    },
    Measured {
        name: "Callisto",
        gm: 7_179.283_40,
        radius_km: 2_410.30,
        planet_gm: JUPITER_GM,
        a_km: 1_882_700.0,
        e: 0.0074,
        period_days: 16.689_017,
        surface: 125.0,
        water: 0.45,
    },
    Measured {
        name: "Titan",
        gm: 8_978.137_10,
        radius_km: 2_574.76,
        planet_gm: SATURN_GM,
        a_km: 1_221_900.0,
        e: 0.0292,
        period_days: 15.945_421,
        surface: 94.0,
        water: 0.42,
    },
    Measured {
        name: "Enceladus",
        gm: 7.210_37,
        radius_km: 252.10,
        planet_gm: SATURN_GM,
        a_km: 238_020.0,
        e: 0.0047,
        period_days: 1.370_218,
        surface: 75.0,
        water: 0.40,
    },
    Measured {
        name: "Dione",
        gm: 73.116_07,
        radius_km: 561.40,
        planet_gm: SATURN_GM,
        a_km: 377_400.0,
        e: 0.0022,
        period_days: 2.736_915,
        surface: 87.0,
        water: 0.50,
    },
    Measured {
        name: "Rhea",
        gm: 153.941_75,
        radius_km: 763.50,
        planet_gm: SATURN_GM,
        a_km: 527_040.0,
        e: 0.0010,
        period_days: 4.517_500,
        surface: 76.0,
        water: 0.66,
    },
    Measured {
        name: "Iapetus",
        gm: 120.515_11,
        radius_km: 734.30,
        planet_gm: SATURN_GM,
        a_km: 3_560_850.0,
        e: 0.0283,
        period_days: 79.330_183,
        surface: 90.0,
        water: 0.81,
    },
    Measured {
        name: "Mimas",
        gm: 2.503_49,
        radius_km: 198.20,
        planet_gm: SATURN_GM,
        a_km: 185_520.0,
        e: 0.0202,
        period_days: 0.942_421_8,
        surface: 64.0,
        water: 0.35,
    },
];

/// Triton at its measured elements, a capture at its own bulk, which only the bit pin reads: GM
/// and mean radius as [`MOONS`]'s (NEP097), JPL's mean elements' 354,759 km on its circular orbit
/// (as `hooks::figures`'s tests read them), its 5.877 d sidereal period (NASA's Neptunian satellite
/// fact sheet), the 38 K of its nitrogen ice (Tryka et al. 1993, Science 261, 751) and the 0.238
/// of water its density holds at ruling 83.7's rock and ice. Its circular orbit raises no
/// eccentricity tide, so Neptune's GM (NASA's Neptune fact sheet, 6.8365 × 10⁶ km³ s⁻²) moves
/// nothing.
const TRITON: Measured = Measured {
    name: "Triton",
    gm: 1_428.495_46,
    radius_km: 1_352.6,
    planet_gm: 6_836_500.0,
    a_km: 354_759.0,
    e: 0.0,
    period_days: 5.877,
    surface: 38.0,
    water: 0.238,
};

/// The moon `name` of [`MOONS`].
fn moon(name: &str) -> Measured {
    *MOONS.iter().find(|m| m.name == name).unwrap()
}

impl Measured {
    fn mass(&self) -> Kilograms {
        Kilograms::new(self.gm * 1e9 / GRAVITATIONAL_CONSTANT)
    }

    fn radius(&self) -> Metres {
        Metres::new(self.radius_km * 1e3)
    }

    fn density(&self) -> KilogramsPerCubicMetre {
        let r = self.radius().value();
        KilogramsPerCubicMetre::new(
            self.mass().value() / (4.0 / 3.0 * core::f64::consts::PI * r * r * r),
        )
    }

    fn gravity(&self) -> MetresPerSecondSquared {
        let r = self.radius().value();
        MetresPerSecondSquared::new(self.gm * 1e9 / (r * r))
    }

    /// Its mass fractions: its water, and its solids a tenth iron for a rocky moon, as the figures'
    /// measured Europa takes them, and rock alone for an icy one, whose rock conducts its heat.
    fn fractions(&self) -> MassFractions {
        if TidalMaterial::of_water(self.water) == TidalMaterial::Ice {
            MassFractions::solid(0.0, 1.0 - self.water, self.water)
        } else {
            let solids = 1.0 - self.water;
            MassFractions::solid(0.1 * solids, 0.9 * solids, self.water)
        }
    }

    fn class(&self) -> PlanetClass {
        PlanetClass::of(EarthMasses::from(self.mass()), &self.fractions())
    }

    /// Its non-tidal heat flow, P14.T24.b's, at the Sun's age about a solar host.
    fn non_tidal(&self) -> WattsPerSquareMetre {
        let r = self.radius();
        non_tidal_heat_flow(
            &HeatInputs {
                mass: EarthMasses::from(self.mass()),
                radius: r,
                surface_gravity: self.gravity(),
                class: self.class(),
                fractions: self.fractions(),
                inventory_water: Kilograms::new(self.water * self.mass().value()),
                fe_h: Dex::new(0.0),
                tidal_heat_flow: WattsPerSquareMetre::new(0.0),
                subsurface_ocean: Subsurface::NotComputed,
            },
            SOLAR_AGE,
            SurfaceState::Airless,
        )
        .unwrap()
    }

    /// Its mean motion from its measured period, rad s⁻¹, as decision-p14-moons's checks read
    /// it: Kepler's law about its planet's GM alone differs by its planet's J₂ (Enceladus's by
    /// 0.07%).
    fn mean_motion(&self) -> f64 {
        core::f64::consts::TAU / (self.period_days * 86_400.0)
    }

    fn forcing(&self) -> WattsPerSquareMetre {
        tidal_forcing(
            Kilograms::new(self.planet_gm * 1e9 / GRAVITATIONAL_CONSTANT),
            self.radius(),
            Metres::new(self.a_km * 1e3),
            self.e,
            self.mean_motion(),
        )
    }

    fn inputs(&self) -> TideInputs {
        let fractions = self.fractions();
        TideInputs {
            material: TidalMaterial::of_water(self.water),
            moment_factor: moment_of_inertia_factor(self.class(), &fractions),
            mass: self.mass(),
            radius: self.radius(),
            water: self.water,
            surface_temperature: Kelvin::new(self.surface),
            non_tidal_heat_flow: self.non_tidal(),
            forcing: self.forcing(),
            mean_motion_rad_per_s: self.mean_motion(),
        }
    }

    /// Its shell's stiffness K = ΛF, at its own period and surface (decision-p14-t56d-membrane).
    fn stiffness(&self) -> ShellStiffness {
        let surface = Kelvin::new(self.surface);
        shell_stiffness(
            shell_modulus(surface, self.mean_motion()),
            ICE_POISSON_RATIO,
            surface,
            self.gravity(),
            self.radius(),
        )
    }

    /// Its ocean's hot root, whether or not its shell then keeps an ocean.
    fn hot_root(&self) -> WattsPerSquareMetre {
        let ocean = ocean_love_number(self.density());
        ocean_heat_flow(self.non_tidal(), self.forcing(), ocean, self.stiffness())
    }

    /// Its stiff-shell gain X.
    fn gain(&self) -> f64 {
        stiff_shell_gain(
            self.forcing(),
            ocean_love_number(self.density()),
            self.stiffness(),
        )
    }

    fn tides(&self) -> MoonTides {
        MoonTides::of(&self.inputs())
    }

    fn area(&self) -> f64 {
        let r = self.radius().value();
        4.0 * core::f64::consts::PI * r * r
    }

    /// The ice shell its heat flow `f` conducts, m.
    fn shell(&self, f: WattsPerSquareMetre) -> f64 {
        ocean_layers(
            self.radius(),
            self.density(),
            self.water,
            Kelvin::new(self.surface),
            f,
        )
        .unwrap()
        .shell_thickness
        .value()
    }
}

/// `k_f` is exactly 3 ÷ 2 for a uniform body (to 10⁻¹⁵, since C = 0.4 is not a double) and the
/// elastic Love number tends to it as the rigidity vanishes; it falls as 1 ÷ μ̃ for a stiff body.
#[test]
fn the_fluid_love_number_is_three_halves_at_c_0_4_and_k2_tends_to_it_as_mu_vanishes() {
    // Exactly 3 ÷ 2 in exact arithmetic; 0.4 has no exact double, so to an ulp or so.
    assert!((fluid_love_number(0.4) - 1.5).abs() < 1e-15);
    let europa = moon("Europa");
    let at = |mu: f64| {
        elastic_love_number(
            1.5,
            Pascals::new(mu),
            europa.density(),
            europa.gravity(),
            europa.radius(),
        )
    };
    assert_same_bits(at(0.0), 1.5);
    for mu in [1e-3, 1.0, 1e3] {
        assert!((at(mu) / 1.5 - 1.0).abs() < 1e-6 * mu.max(1.0), "{mu}");
    }
    let (stiff, stiffer) = (at(1e12), at(1e13));
    assert!(
        (stiff / stiffer / 10.0 - 1.0).abs() < 0.01,
        "{stiff} {stiffer}"
    );
    // The class factors' fluid Love numbers (decision-p14-moons §3.2).
    assert!((fluid_love_number(0.33) - 0.9276).abs() < 0.001);
    assert!((fluid_love_number(0.34) - 1.000).abs() < 0.002);
}

/// The rock rigidities' calibrations: the Moon's GRAIL k₂ at its measured moment of inertia,
/// within 2%, and Io's Juno k₂, 0.125 ± 0.005, with Io's heat flux, on the heated branch at its
/// measured elements, 1–4 W m⁻² (the observed 1.4–3.8).
#[test]
fn the_moon_and_io_take_their_measured_love_numbers() {
    // Williams et al. 2014 (JGR Planets 119, 1546) give the Moon's C ÷ MR² as 0.392863 at
    // R = 1,738 km, Anderson et al. 2001 (JGR 106, 32963) Io's as 0.37685 ± 0.00035.
    let earth_moon = moon("Moon");
    let k2 = elastic_love_number(
        fluid_love_number(0.392_863),
        ROCK_RIGIDITY,
        earth_moon.density(),
        earth_moon.gravity(),
        earth_moon.radius(),
    );
    assert!((k2 / 0.0242 - 1.0).abs() < 0.02, "the Moon: {k2}");
    let io = moon("Io");
    let k2 = elastic_love_number(
        fluid_love_number(0.37685),
        HEATED_ROCK_RIGIDITY,
        io.density(),
        io.gravity(),
        io.radius(),
    );
    assert!((k2 - 0.125).abs() < 0.005, "Io: {k2}");
    let tides = io.tides();
    assert_eq!(tides.branch(), TidalBranch::Heated);
    assert_same_bits(tides.loss(), MOON_TIDAL_RESPONSE);
    let flux = io.forcing().value() * tides.loss();
    assert!((1.0..4.0).contains(&flux), "Io: {flux} W m⁻²");
    eprintln!(
        "the Moon: k2 {:.4} at C 0.392863; Io: k2 {k2:.4} at C 0.37685, {:.4} at its class's \
         (Q {:.1}), {flux:.2} W m⁻²",
        elastic_love_number(
            fluid_love_number(0.392_863),
            ROCK_RIGIDITY,
            earth_moon.density(),
            earth_moon.gravity(),
            earth_moon.radius()
        ),
        tides.k2(),
        tides.q()
    );
    // The Moon on its own elements is cold rock, Q = 38.
    let tides = earth_moon.tides();
    assert_eq!(tides.branch(), TidalBranch::Cold);
    assert_same_bits(tides.q(), COLD_ROCK_TIDAL_Q);
}

/// decision-p14-t56d-membrane §3's checks at measured elements: Europa and Titan on the ocean
/// branch at their hot roots, under Beuthe's membrane with their own Maxwell shells, and Ganymede
/// and Callisto on it from their radiogenic heat alone.
///
/// Europa's shell is held to Juno's microwave radiometer's 29 ± 10 km conductive layer, for pure
/// water ice (Levin et al. 2026, Nature Astronomy 10, 84; decision-p14-t56d-membrane §8): 19–39 km.
/// Titan's k₂ is 0.41, under Iess et al.'s (2012) 0.589 ± 0.150 (2σ) and Durante et al.'s (2019)
/// 0.616 ± 0.067 (1σ): a denser ocean is P14.T51.g's solutes'.
#[test]
fn the_ocean_worlds_take_the_ocean_branch_at_their_measured_elements() {
    for name in ["Europa", "Ganymede", "Callisto", "Titan"] {
        let body = moon(name);
        let tides = body.tides();
        let f = tides.heat_flow();
        let modulus = shell_modulus(Kelvin::new(body.surface), body.mean_motion());
        eprintln!(
            "{name}: δ {:.4}, S_R {:.4}; F_nt {:.2} mW m⁻², F {:.3} mW m⁻² ({:.2} GW) under {:.1} km, \
             k2 {:.4}, Im k2 {:.3e}, Q {:.1}",
            modulus.loss_factor(),
            modulus.storage,
            1e3 * body.non_tidal().value(),
            1e3 * f.value(),
            f.value() * body.area() / 1e9,
            body.shell(f) / 1e3,
            tides.k2(),
            tides.loss(),
            tides.q()
        );
        assert_eq!(tides.branch(), TidalBranch::Ocean, "{name}");
        // The root: its heat is its rock's and its tide's.
        let tidal = body.forcing().value() * tides.loss();
        let whole = body.non_tidal().value() + tidal;
        assert!(
            (whole / f.value() - 1.0).abs() < 1e-12,
            "{name}: {whole} {f:?}"
        );
    }
    let europa = moon("Europa").tides();
    let f = europa.heat_flow().value();
    assert!((0.22..0.24).contains(&europa.k2()), "{}", europa.k2());
    // Juno's 29 ± 10 km implies 15–35 mW m⁻².
    assert!((0.015..0.035).contains(&f), "Europa: {f} W m⁻²");
    let shell = moon("Europa").shell(europa.heat_flow());
    assert!((19e3..39e3).contains(&shell), "Europa's shell: {shell} m");
    let titan = moon("Titan").tides();
    let f = titan.heat_flow().value();
    assert!((0.38..0.45).contains(&titan.k2()), "{}", titan.k2());
    assert!((0.0045..0.008).contains(&f), "Titan: {f} W m⁻²");
    let shell = moon("Titan").shell(titan.heat_flow());
    assert!((50e3..200e3).contains(&shell), "Titan's shell: {shell} m");
    // Ganymede and Callisto with no tide at all: their rock's heat keeps their oceans.
    for name in ["Ganymede", "Callisto"] {
        let body = moon(name);
        let tides = MoonTides::of(&TideInputs {
            forcing: WattsPerSquareMetre::new(0.0),
            ..body.inputs()
        });
        eprintln!(
            "{name} on its rock's heat alone: k2 {:.4}, Im k2 {:.3e}",
            tides.k2(),
            tides.loss()
        );
        assert_eq!(tides.branch(), TidalBranch::Ocean, "{name}");
        assert_same_bits(tides.heat_flow().value(), body.non_tidal().value());
        assert!((0.37..0.45).contains(&tides.k2()), "{name}: {}", tides.k2());
        assert!(
            (0.002..0.008).contains(&tides.loss()),
            "{name}: {}",
            tides.loss()
        );
    }
}

/// decision-p14-t56d-membrane §2.4: Enceladus's own conductive shell cannot keep its ocean. Its hot
/// root is a few tenths of a GW against Howett et al.'s (2011, JGR 116, E03003) 15.8 GW from its
/// south polar terrain, as the literature finds for a conductive shell (Roberts and Nimmo 2008);
/// its stiff shell's gain X is far from 1, so it is cold. The missing heat is a porous rock core's
/// (Choblet et al. 2017), P14.T56.e's (plan 14's Risks, P14.T56.d's membrane, as ruled).
#[test]
fn enceladus_s_shell_cannot_keep_its_ocean() {
    let enceladus = moon("Enceladus");
    let tides = enceladus.tides();
    let watts = enceladus.hot_root().value() * enceladus.area();
    let gain = enceladus.gain();
    eprintln!(
        "Enceladus: {:?}; its shell's hot root {:.3} GW against the observed 15.8 GW (× {:.3}), X \
         {gain:.4}",
        tides.branch(),
        watts / 1e9,
        watts / 15.8e9
    );
    assert_eq!(tides.branch(), TidalBranch::Cold);
    assert!(watts < 1e9, "{watts} W");
    assert!(gain < 0.1, "{gain}");
}

/// decision-p14-t56d-membrane §2.4: Dione under its own shell is cold, its hot root under
/// 1.5 mW m⁻², against the ocean that its gravity and shape imply under minimum-stress isostasy
/// (Beuthe, Rivoldini and Trinh 2016, GRL 43, 10088): a porous core, a non-uniform shell or an
/// eccentricity history, P14.T56.e's, T55.k's and T55.j's.
#[test]
fn dione_is_cold_under_its_own_shell() {
    let dione = moon("Dione");
    let f = dione.hot_root().value();
    eprintln!(
        "Dione: its hot root {:.3} mW m⁻², X {:.4}",
        1e3 * f,
        dione.gain()
    );
    assert_eq!(dione.tides().branch(), TidalBranch::Cold);
    assert!(f < 1.5e-3, "{f}");
}

/// The small-Λ root, (`F_nt` + √(`F_nt`² + 4Φ `k_oc` h°`K_I`)) ÷ 2, bounds the exact root from
/// above for every moon of the table with water (decision-p14-t56d-membrane §2.1): by 3–9% for
/// Europa, whose shell is stiffest of the ocean worlds (h°Λ ≈ 0.09), and within 1% for Ganymede and
/// Callisto. It is a bound, not an approximation within 1%.
#[test]
fn the_small_lambda_root_bounds_the_exact_root_from_above() {
    for body in MOONS {
        if body.water <= 0.0 {
            continue;
        }
        let (f_nt, phi) = (body.non_tidal().value(), body.forcing().value());
        let ocean = ocean_love_number(body.density());
        let k_i = (1.0 + ocean) * body.stiffness().imaginary.value();
        let root = (f_nt * f_nt + 4.0 * phi * ocean * k_i).sqrt();
        let small = f64::midpoint(f_nt, root);
        let exact = body.hot_root().value();
        let above = small / exact - 1.0;
        eprintln!(
            "{}: the small-Λ root {small:.6e} against {exact:.6e} W m⁻², {above:+.4}",
            body.name
        );
        assert!(above >= 0.0, "{}: {above}", body.name);
        match body.name {
            "Europa" => assert!((0.03..0.09).contains(&above), "{above}"),
            "Ganymede" | "Callisto" => assert!(above < 0.01, "{}: {above}", body.name),
            _ => {}
        }
    }
}

/// Beuthe's coefficient, over the ocean's density (his eq. 28 and §4.3): at the elastic limit (all
/// storage, no loss) the membrane's Λ R ÷ d is his "Λ ∼ 3.4 d/R" for Europa (eq. 29) and
/// "Λ ∼ 240 d/R" for Enceladus (§8). Over the mean density, decision-p14-moons §F-ocean F2's
/// error, they would be 1.13 and 152.
#[test]
fn beuthe_s_coefficient_reproduces_his_europa_and_enceladus() {
    let elastic = ShellModulus {
        storage: 1.0,
        loss: 0.0,
    };
    for (name, expected, tolerance) in [("Europa", 3.40, 0.03), ("Enceladus", 244.0, 3.0)] {
        let body = moon(name);
        let surface = Kelvin::new(body.surface);
        let k = shell_stiffness(
            elastic,
            ICE_POISSON_RATIO,
            surface,
            body.gravity(),
            body.radius(),
        );
        assert_same_bits(k.imaginary.value(), 0.0);
        // Λ R ÷ d = K R ÷ (F d) = K R ÷ (651 ln(273 ÷ T_s)), whatever the heat flow.
        let conducted = ICE_CONDUCTIVITY_COEFFICIENT * math::ln(273.0 / body.surface);
        let per = k.real.value() * body.radius().value() / conducted;
        eprintln!("{name}: Λ R ÷ d {per:.3} at the elastic limit");
        assert!((per - expected).abs() < tolerance, "{name}: {per}");
    }
}

/// Beuthe's (2015) k₂–h₂ relation holds for the membrane: with h₂ = (5ρ̄ ÷ 3`ρ_w`) k₂ (his eq. 42),
/// (1 + Λ) h₂ = 1 + k₂ (his eq. 34), to 10⁻¹², for complex Λ over |Λ| 10⁻³–10³ and its argument
/// 0–π ÷ 2, and ρ̄ 1,100–3,500 kg m⁻³; at Λ = 0, k₂ is `k_oc` exactly.
#[test]
fn the_membrane_obeys_beuthe_s_k2_h2_relation() {
    let water = WATER_DENSITY.value();
    let mut checked = 0;
    for density in [1_100.0, 1_600.0, 2_400.0, 3_500.0] {
        let ocean = ocean_love_number(KilogramsPerCubicMetre::new(density));
        let rest = membrane_love_number(ocean, MembraneStiffness::default());
        assert_same_bits(rest.real, ocean);
        assert_same_bits(rest.imaginary, 0.0);
        for i in 0..=12 {
            let size = math::exp10(-3.0 + 0.5 * f64::from(i));
            for j in 0..=8 {
                let angle = core::f64::consts::FRAC_PI_2 * f64::from(j) / 8.0;
                let (sin, cos) = math::sin_cos(angle);
                let lambda = MembraneStiffness {
                    real: size * cos,
                    imaginary: size * sin,
                };
                let k2 = membrane_love_number(ocean, lambda);
                let scale = 5.0 * density / (3.0 * water);
                let (h_re, h_im) = (scale * k2.real, scale * k2.imaginary);
                // (1 + Λ) h₂, complex.
                let left = (
                    (1.0 + lambda.real) * h_re - lambda.imaginary * h_im,
                    (1.0 + lambda.real) * h_im + lambda.imaginary * h_re,
                );
                let right = (1.0 + k2.real, k2.imaginary);
                let size_of = math::hypot(right.0, right.1);
                assert!(
                    math::hypot(left.0 - right.0, left.1 - right.1) <= 1e-12 * size_of,
                    "{density} {lambda:?}: {left:?} against {right:?}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 4 * 13 * 9);
}

/// The shell's Maxwell modulus (decision-p14-t56d-membrane §2.2, with its §7.3) has its limits and
/// follows its period and surface: elastic at a fast enough tide (m₀ → 0); at its base's own
/// Maxwell value as the surface nears the melting point, `S_R` → 1 ÷ (1 + m₀²) and δ → m₀ (0.255 and
/// 1.709 at Europa's period), and no shell at it; its loss factor rising with the period at a fixed
/// surface and with the surface at a fixed period; Europa's δ 0.0395 and `S_R` 0.974; Mimas's δ
/// under Europa's; and its 512-node sum within 0.2% of a 16,384-node one over surfaces at 10–270 K
/// and periods of 0.2–100 d.
#[test]
fn the_shell_s_modulus_has_its_limits_and_follows_period_and_temperature() {
    let day = 86_400.0;
    let n = |days: f64| core::f64::consts::TAU / (days * day);
    let europa = n(3.551_181);
    let fast = shell_modulus(Kelvin::new(102.0), 1e3);
    assert!(
        (fast.storage - 1.0).abs() < 1e-6 && fast.loss < 1e-6,
        "{fast:?}"
    );
    let at_melting = ICE_RIGIDITY.value() / (europa * ICE_MELTING_VISCOSITY_PA_S);
    let warm = shell_modulus(Kelvin::new(272.999), europa);
    let base = 1.0 / (1.0 + at_melting * at_melting);
    assert!(
        (warm.storage / base - 1.0).abs() < 1e-4,
        "{warm:?} against {base}"
    );
    assert!(
        (warm.loss_factor() - at_melting).abs() < 1e-3,
        "{warm:?} against {at_melting}"
    );
    assert!((base - 0.255).abs() < 1e-3 && (at_melting - 1.709).abs() < 1e-3);
    assert_eq!(
        shell_modulus(ICE_MELTING_POINT, europa),
        ShellModulus::default()
    );
    let at = |surface: f64, days: f64| shell_modulus(Kelvin::new(surface), n(days)).loss_factor();
    assert!(at(100.0, 0.5) < at(100.0, 4.0) && at(100.0, 4.0) < at(100.0, 32.0));
    assert!(at(40.0, 4.0) < at(160.0, 4.0) && at(160.0, 4.0) < at(240.0, 4.0));
    let shell = shell_modulus(Kelvin::new(102.0), europa);
    assert!((shell.loss_factor() - 0.0395).abs() < 0.001, "{shell:?}");
    assert!((shell.storage - 0.974).abs() < 0.003, "{shell:?}");
    let mimas = moon("Mimas");
    let mimas = shell_modulus(Kelvin::new(mimas.surface), mimas.mean_motion());
    assert!(mimas.loss_factor() < shell.loss_factor(), "{mimas:?}");
    let mut worst = 0.0_f64;
    for surface in [10.0, 30.0, 60.0, 100.0, 150.0, 200.0, 250.0, 270.0] {
        for days in [0.2, 0.5, 1.0, 3.0, 10.0, 30.0, 100.0] {
            let coarse = shell_modulus(Kelvin::new(surface), n(days));
            let fine =
                shell_modulus_with(Kelvin::new(surface), n(days), IceRheology::SHELL, 16_384);
            for (x, y) in [(coarse.storage, fine.storage), (coarse.loss, fine.loss)] {
                worst = worst.max((x / y - 1.0).abs());
            }
        }
    }
    eprintln!("the 512-node modulus within {worst:.2e} of 16,384 nodes");
    assert!(worst < 2e-3, "{worst}");
}

/// The hot root is the cubic's largest, and stable: over a grid of forcings, complex stiffnesses
/// and non-tidal flows, no sign change of the cubic lies above it, and F = `F_nt` + Φ Im k₂ there.
#[test]
fn the_hot_root_is_the_largest_and_it_is_stable() {
    let mut checked = 0;
    for f_nt in [0.0, 1e-4, 4e-3, 0.03] {
        for phi in [0.0, 1e-3, 0.1, 15.0, 1e3, 1e5] {
            for k_r in [1e-6, 4.7e-4, 0.01, 0.3, 3.0] {
                for (k_oc, loss) in [(0.05, 0.01), (0.25, 0.05), (1.5, 0.4)] {
                    let watts = WattsPerSquareMetre::new;
                    let stiffness = ShellStiffness {
                        real: watts(k_r),
                        imaginary: watts(loss * k_r),
                    };
                    let root = ocean_heat_flow(watts(f_nt), watts(phi), k_oc, stiffness);
                    let r = root.value();
                    let shell = 1.0 + k_oc;
                    let (kr, ki) = (shell * k_r, shell * loss * k_r);
                    let cubic =
                        |f: f64| (f - f_nt) * ((f + kr) * (f + kr) + ki * ki) - phi * k_oc * ki * f;
                    assert!(r >= f_nt && r.is_finite(), "{f_nt} {phi} {k_r} {k_oc}: {r}");
                    // Where it is not the trivial F = 0 of a body with no heat of its own.
                    if r > 1e-15 {
                        let back =
                            f_nt + phi * membrane_love_number(k_oc, stiffness.at(root)).loss();
                        assert!(
                            (back / r - 1.0).abs() < 1e-6,
                            "{f_nt} {phi} {k_r} {k_oc}: {r}"
                        );
                    }
                    // No root above it: the cubic is positive on a log grid up to 10⁶ times it.
                    for i in 1..=600 {
                        let f = r.max(1e-12) * math::exp10(f64::from(i) * 0.01);
                        assert!(cubic(f) > 0.0, "{f_nt} {phi} {k_r} {k_oc}: {f} above {r}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 360);
}

/// P14.T17.b's test (b) as amended (decision-p14-moons §3.4 (a)): at measured elements, with the
/// generator's class moments of inertia and a 15 h primordial spin, Jupiter's four, Titan and Rhea
/// lock within 1 Gyr, and Iapetus, cold ice, within the Sun's age (the ruling's 3.4 Gyr); version
/// 22's class pair locked Iapetus 56 times faster.
#[test]
fn the_measured_moons_lock_as_t17b_amended() {
    let gyr = 1e9 * SECONDS_PER_JULIAN_YEAR;
    let lock = |body: &Measured, (k2, q): (f64, f64)| {
        let fractions = body.fractions();
        let spinning = SpinningBody::new(
            body.mass(),
            body.radius(),
            moment_of_inertia_factor(body.class(), &fractions),
            k2,
            q,
        )
        .unwrap();
        tidal_locking_time(
            &spinning,
            Seconds::new(MOON_PRIMORDIAL_PERIOD_HOURS * 3_600.0),
            Metres::new(body.a_km * 1e3),
            Kilograms::new(body.planet_gm * 1e9 / GRAVITATIONAL_CONSTANT),
        )
        .value()
            / gyr
    };
    for name in [
        "Io", "Europa", "Ganymede", "Callisto", "Titan", "Rhea", "Iapetus",
    ] {
        let body = moon(name);
        let tides = body.tides();
        let t = lock(&body, (tides.k2(), tides.q()));
        let class = lock(&body, (0.3, 100.0));
        eprintln!(
            "{name}: {:?}, k2 {:.4}, Q {:.1}: locks in {t:.3e} Gyr (the class pair's {class:.3e})",
            tides.branch(),
            tides.k2(),
            tides.q()
        );
        let bound = if name == "Iapetus" {
            SOLAR_AGE.value() / 1e9
        } else {
            1.0
        };
        assert!(t < bound, "{name}: {t} Gyr");
    }
    let iapetus = moon("Iapetus").tides();
    assert_eq!(iapetus.branch(), TidalBranch::Cold);
    assert_eq!(moon("Rhea").tides().branch(), TidalBranch::Cold);
    let t = lock(&moon("Iapetus"), (iapetus.k2(), iapetus.q()));
    assert!((2.0..4.57).contains(&t), "Iapetus: {t} Gyr");
}

/// A cold rocky moon's tidal heat flux is k₂ ÷ (38 × 0.015) of version 22's, which read Io's
/// response for every moon.
#[test]
fn a_cold_rocky_moon_s_heat_is_its_own_loss_over_io_s() {
    let earth_moon = moon("Moon");
    let tides = earth_moon.tides();
    assert_eq!(tides.branch(), TidalBranch::Cold);
    let v22 = earth_moon.forcing().value() * MOON_TIDAL_RESPONSE;
    let v23 = earth_moon.forcing().value() * tides.loss();
    let ratio = tides.k2() / (COLD_ROCK_TIDAL_Q * MOON_TIDAL_RESPONSE);
    assert!((v23 / v22 / ratio - 1.0).abs() < 1e-12);
    eprintln!("the Moon: × {ratio:.4} of version 22's tidal heat");
}

/// Mimas at its own 0.94-day period keeps a nearly elastic shell (δ 0.011), so its stiff shell's
/// gain X is 0.23 and it stays cold (decision-p14-t56d-membrane §2.2): the Mimas paradox, a cold
/// Mimas despite its forcing, is the model's at its own elements. Its inferred ocean (Lainey et al.
/// 2024, Nature 626, 280) needs heat or an eccentricity history this model lacks.
#[test]
fn mimas_stays_cold_at_its_own_period() {
    let mimas = moon("Mimas");
    let gain = mimas.gain();
    eprintln!(
        "Mimas: {:?}, X {gain:.4}, its hot root {:.3} mW m⁻²",
        mimas.tides().branch(),
        1e3 * mimas.hot_root().value()
    );
    assert_eq!(mimas.tides().branch(), TidalBranch::Cold);
    assert!((0.15..0.35).contains(&gain), "{gain}");
}

/// A Dione past X = 1 (decision-p14-t56d-membrane §2.5): at e = 0.02 instead of its 0.0022 its stiff
/// shell's gain is 1.6–2.5, and its hot root takes the thin-shell branch, the ocean branch at
/// 0.035–0.08 W m⁻² under a shell under 20 km. Its cubic has a single root above `F_nt`, the
/// largest and stable.
#[test]
fn a_dione_past_x_one_takes_the_thin_shell_branch() {
    let dione = Measured {
        e: 0.02,
        ..moon("Dione")
    };
    let gain = dione.gain();
    let tides = dione.tides();
    let f = tides.heat_flow();
    eprintln!(
        "Dione at e = 0.02: X {gain:.3}, {:?}, F {:.2} mW m⁻² under {:.1} km",
        tides.branch(),
        1e3 * f.value(),
        dione.shell(f) / 1e3
    );
    assert!((1.6..2.5).contains(&gain), "{gain}");
    assert_eq!(tides.branch(), TidalBranch::Ocean);
    assert!((0.035..0.08).contains(&f.value()), "{f:?}");
    assert!(dione.shell(f) < 20e3);
    let ocean = ocean_love_number(dione.density());
    let shell = 1.0 + ocean;
    let stiffness = dione.stiffness();
    let (kr, ki) = (
        shell * stiffness.real.value(),
        shell * stiffness.imaginary.value(),
    );
    let (f_nt, phi) = (dione.non_tidal().value(), dione.forcing().value());
    let cubic = |x: f64| (x - f_nt) * ((x + kr) * (x + kr) + ki * ki) - phi * ocean * ki * x;
    for i in 1..=400 {
        assert!(cubic(f.value() * math::exp10(f64::from(i) * 0.01)) > 0.0);
    }
    // And negative all the way down to F_nt: no other root above it.
    for i in 1..400 {
        let x = f_nt + (f.value() - f_nt) * f64::from(i) / 400.0;
        assert!(cubic(x) < 0.0, "{x}");
    }
}

/// Beuthe 2019's (Icarus 332, 66) benchmark (decision-p14-t56d-membrane §4.4): at his ice of Table
/// 1, `η_m` = 10¹³ Pa s, a uniform 23 km conductive shell over a rigid mantle at Enceladus's
/// elements, under its 75 K surface and k = 651 ÷ T, dissipates 0.8–1.2 GW in the membrane; at the
/// constant's 10¹⁴ Pa s, some 0.39 GW.
///
/// His Table 6 UNI-L, 0.96 GW, is not the same model (the science check): a 22.8 km shell, his
/// eq. 63's k(T), a 51–62 K surface, bending included, and the 0.12° forced libration (+28%, his
/// eq. 49); like for like its membrane share is some 0.65 GW. At his §4 settings (a 59 K surface,
/// k = 651 ÷ T, 23 km at 10¹³ Pa s) the membrane's Λ is exactly his Table 3's 21.267 + 0.730i,
/// which the test holds to 0.1% (21.276 + 0.730i at JPL's GM and mean radius).
#[test]
fn enceladus_s_uniform_shell_dissipates_as_beuthe_2019_computes() {
    let enceladus = moon("Enceladus");
    let surface = Kelvin::new(enceladus.surface);
    let ocean = ocean_love_number(enceladus.density());
    let thickness = 23e3;
    // The heat flow at which the conductive shell is 23 km thick, so that Λ is the uniform shell's.
    let conducted = ICE_CONDUCTIVITY_COEFFICIENT * math::ln(273.0 / enceladus.surface);
    let at = WattsPerSquareMetre::new(conducted / thickness);
    let power = |viscosity: f64| {
        let rheology = IceRheology {
            melting_viscosity_pa_s: viscosity,
            ..IceRheology::SHELL
        };
        let modulus = shell_modulus_with(
            surface,
            enceladus.mean_motion(),
            rheology,
            SHELL_QUADRATURE_NODES,
        );
        let stiffness = shell_stiffness(
            modulus,
            ICE_POISSON_RATIO,
            surface,
            enceladus.gravity(),
            enceladus.radius(),
        );
        let loss = membrane_love_number(ocean, stiffness.at(at)).loss();
        enceladus.forcing().value() * loss * enceladus.area()
    };
    let (beuthe, constant) = (power(1e13), power(ICE_MELTING_VISCOSITY_PA_S));
    eprintln!(
        "Enceladus's uniform 23 km shell: {:.3} GW at 10¹³ Pa s (Beuthe 2019: 0.96), {:.3} GW at \
         10¹⁴",
        beuthe / 1e9,
        constant / 1e9
    );
    assert!((0.8e9..1.2e9).contains(&beuthe), "{beuthe} W");
    // His §4: the same shell under a 59 K surface, Λᴹ₂ = 21.267 + 0.730i (his Table 3).
    let cold = Kelvin::new(59.0);
    let rheology = IceRheology {
        melting_viscosity_pa_s: 1e13,
        ..IceRheology::SHELL
    };
    let modulus = shell_modulus_with(
        cold,
        enceladus.mean_motion(),
        rheology,
        SHELL_QUADRATURE_NODES,
    );
    let stiffness = shell_stiffness(
        modulus,
        ICE_POISSON_RATIO,
        cold,
        enceladus.gravity(),
        enceladus.radius(),
    );
    let conducted = ICE_CONDUCTIVITY_COEFFICIENT * math::ln(273.0 / 59.0);
    let lambda = stiffness.at(WattsPerSquareMetre::new(conducted / thickness));
    eprintln!("Λ at Beuthe 2019's §4 settings: {lambda:?}");
    // To 0.1%: 21.276 + 0.730i at JPL's GM and Archinal et al.'s mean radius.
    assert!((lambda.real / 21.267 - 1.0).abs() < 1e-3, "{lambda:?}");
    assert!((lambda.imaginary / 0.730 - 1.0).abs() < 1e-3, "{lambda:?}");
}

/// A body without water, or at its melting point, takes no ocean branch; a rocky one takes the
/// heated branch exactly from `F_c`, and an icy one with no ocean is cold ice.
#[test]
fn the_branches_part_where_the_rule_says() {
    let europa = moon("Europa").inputs();
    let dry = TideInputs {
        water: 0.0,
        ..europa
    };
    assert_eq!(MoonTides::of(&dry).branch(), TidalBranch::Heated);
    let hot = TideInputs {
        surface_temperature: ICE_MELTING_POINT,
        ..europa
    };
    assert_eq!(MoonTides::of(&hot).branch(), TidalBranch::Heated);
    // At F_c exactly, by the non-tidal flow, with no forcing.
    let at = |f: f64| {
        MoonTides::of(&TideInputs {
            non_tidal_heat_flow: WattsPerSquareMetre::new(f),
            forcing: WattsPerSquareMetre::new(0.0),
            ..dry
        })
        .branch()
    };
    assert_eq!(at(SURFACE_MELT_HEAT_FLOW.value()), TidalBranch::Heated);
    assert_eq!(
        at(SURFACE_MELT_HEAT_FLOW.value() * (1.0 - 1e-12)),
        TidalBranch::Cold
    );
    let iapetus = moon("Iapetus").tides();
    assert_eq!(iapetus.branch(), TidalBranch::Cold);
    assert_same_bits(iapetus.q(), COLD_ICE_TIDAL_Q);
    let ice = TideInputs {
        surface_temperature: ICE_MELTING_POINT,
        ..moon("Ganymede").inputs()
    };
    assert_eq!(MoonTides::of(&ice).branch(), TidalBranch::Cold);
    assert_eq!(TidalMaterial::of_water(0.1), TidalMaterial::Ice);
    assert_eq!(
        TidalMaterial::of_water(0.1 * (1.0 - 1e-12)),
        TidalMaterial::Rock
    );
}

/// The measured moons' tides pinned to the bit (P14.T56.d's determinism audit, with the 22 → 23
/// batch's bless): every moon of [`MOONS`] and [`TRITON`] at its measured elements, its branch,
/// k₂, Q, loss and whole heat flow, with the `F_nt` it reads, so that a moved constant, root or
/// quadrature moves a line where every other test of the tides holds a tolerance.
#[test]
fn the_measured_moons_tides_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    for moon in MOONS.iter().chain(core::iter::once(&TRITON)) {
        let tides = moon.tides();
        let name = moon.name.to_lowercase();
        w.line(&format!("{name}_branch = {:?}", tides.branch()));
        w.f64(
            &format!("{name}_non_tidal_heat_flow"),
            moon.non_tidal().value(),
        );
        w.f64(&format!("{name}_k2"), tides.k2());
        w.f64(&format!("{name}_q"), tides.q());
        w.f64(&format!("{name}_loss"), tides.loss());
        w.f64(&format!("{name}_heat_flow"), tides.heat_flow().value());
    }
    golden!("planetary/moon_tides", w.as_str());
}

// P14.T51.g.b: T56.d's ocean branch on the solve's spherical shell (decision-p14-t51g's T56.d
// branch), its moons at their measured elements with their chlorine at CI chondrites' per unit
// rock and, beyond ammonia hydrate's line, their ice's ammonia at `ACCRETED_AMMONIA`.

use crate::planetary::chem::inventory::CI_CHLORINE_PER_MASS;
use crate::planetary::hooks::ocean::{
    ACCRETED_AMMONIA, OceanInputs, has_ocean, implausibility, with_ocean_rules,
};
use crate::substance::Element;

impl Measured {
    /// Its solutes: its rock's chlorine at CI chondrites' as sodium chloride, and where `ammonia`
    /// its ice's ammonia.
    fn solutes(&self, ammonia: bool) -> OceanSolutes {
        let cl = Element::Cl.standard_atomic_weight_g_per_mol();
        let salt = Element::Na.standard_atomic_weight_g_per_mol() + cl;
        let chlorine = CI_CHLORINE_PER_MASS * (1.0 - self.water) * self.mass().value();
        OceanSolutes {
            sodium_chloride: Kilograms::new(chlorine * salt / cl),
            ammonia_per_water: if ammonia { ACCRETED_AMMONIA } else { 0.0 },
        }
    }

    /// Its tides and its branch's ocean under P14.T51.g.b's rules.
    fn solved(&self, ammonia: bool) -> (MoonTides, Subsurface) {
        let (tides, ocean) = with_ocean_rules(OceanRules::Solved, || {
            MoonTides::with_ocean(&self.inputs(), &self.solutes(ammonia))
        });
        (tides, ocean.expect("the solve keeps the branch's ocean"))
    }

    /// The ocean inputs its branch read at the heat flow `flux`.
    fn ocean_inputs(&self, ammonia: bool, flux: WattsPerSquareMetre) -> OceanInputs {
        OceanInputs {
            radius: self.radius(),
            mass: self.mass(),
            water_fraction: self.water,
            surface_temperature: Kelvin::new(self.surface),
            heat_flow: flux,
            solutes: self.solutes(ammonia),
        }
    }
}

/// The shell's profile is [`shell_modulus`]'s mean when read at 273 K from a top above 30 K, to
/// 0.5% (its nodes run to 273.16 K, and are read in proportion); its nodes are the quadrature's.
#[test]
fn the_profile_reads_the_shell_s_modulus_to_its_base() {
    assert_eq!(usize::try_from(SHELL_QUADRATURE_NODES), Ok(PROFILE_NODES));
    for name in ["Europa", "Ganymede", "Titan", "Enceladus", "Mimas"] {
        let body = moon(name);
        let surface = Kelvin::new(body.surface);
        let n = body.mean_motion();
        let held = shell_modulus(surface, n);
        let profile = ShellProfile::new(surface, n, Kelvin::new(273.16)).to(Kelvin::new(273.0));
        assert!(
            (profile.storage / held.storage - 1.0).abs() < 5e-3,
            "{name}: {profile:?}"
        );
        assert!(
            (profile.loss / held.loss - 1.0).abs() < 5e-3,
            "{name}: {profile:?}"
        );
        // A colder base averages over a stiffer, less lossy shell.
        let cold = ShellProfile::new(surface, n, Kelvin::new(273.16)).to(Kelvin::new(240.0));
        assert!(
            cold.storage > profile.storage && cold.loss < profile.loss,
            "{name}"
        );
    }
}

/// Europa at its measured elements, with its brine: on the ocean branch under the spherical shell,
/// its k₂ 0.22–0.24, its heat flow 15–35 mW m⁻², and its branch's shell 19–39 km (the design's
/// estimates), and, without solutes, within 10% of version 23's planar root: d ÷ R differs, and so
/// does the base, at ice Ih's melting point at its pressure (some 271 K) rather than 273 K, whose
/// warmest, lossiest ice the planar shell held (7.2% under it, recorded).
#[test]
fn europa_s_branch_under_the_spherical_shell() {
    let europa = moon("Europa");
    let (tides, ocean) = europa.solved(false);
    assert_eq!(tides.branch(), TidalBranch::Ocean);
    let Subsurface::Ocean(ocean) = ocean else {
        panic!("Europa's branch keeps an ocean: {ocean:?}");
    };
    let shell = ocean.shell_thickness().value() / 1e3;
    let flux = tides.heat_flow().value() * 1e3;
    eprintln!(
        "Europa on its branch: k₂ {:.4}, Q {:.1}, F {flux:.2} mW m⁻², shell {shell:.1} km, ocean \
         {:.1} km on {:?} (version 23: k₂ {:.4}, F {:.2})",
        tides.k2(),
        tides.q(),
        ocean.thickness().value() / 1e3,
        ocean.floor(),
        europa.tides().k2(),
        europa.tides().heat_flow().value() * 1e3
    );
    assert!((0.22..=0.24).contains(&tides.k2()), "{}", tides.k2());
    assert!((15.0..=35.0).contains(&flux), "{flux}");
    assert!((19.0..=39.0).contains(&shell), "{shell} km");
    let fresh = with_ocean_rules(OceanRules::Solved, || {
        MoonTides::with_ocean(&europa.inputs(), &OceanSolutes::FRESH).0
    });
    let planar = europa.tides().heat_flow().value();
    assert!(
        (fresh.heat_flow().value() / planar - 1.0).abs() < 0.10,
        "{fresh:?} against {planar}"
    );
}

/// Titan on T56.d's ocean branch under 1,000 kg m⁻³ (decision-p14-t51g Q3): its k₂ is stated,
/// about 0.41, against 0.616 ± 0.067 (Durante et al. 2019, 1σ) and 0.589 ± 0.150 (Iess et al.
/// 2012, 2σ), about 3σ low: a miss recorded for P14.T55.k with T55.j.
#[test]
fn titan_s_love_number_on_its_ocean_is_stated() {
    let titan = moon("Titan");
    let (tides, ocean) = titan.solved(true);
    assert_eq!(tides.branch(), TidalBranch::Ocean);
    eprintln!(
        "Titan on its branch: k₂ {:.4} (Durante et al. 2019: 0.616 ± 0.067), Q {:.1}, F {:.3} mW \
         m⁻², {ocean:?}",
        tides.k2(),
        tides.q(),
        tides.heat_flow().value() * 1e3
    );
    assert!((0.35..=0.47).contains(&tides.k2()), "{}", tides.k2());
}

/// A moon's ocean branch reads `has_ocean` at its whole heat flow, with its own water and solutes:
/// on the branch the presence test holds at `F_ocean` and its branch's ocean is an ocean, absent
/// (an ice-II base), or none under a metre; a dry moon takes no ocean branch.
#[test]
fn the_ocean_branch_reads_has_ocean_at_its_whole_heat_flow() {
    for (name, ammonia) in [
        ("Europa", false),
        ("Ganymede", false),
        ("Callisto", true),
        ("Titan", true),
        ("Enceladus", true),
        ("Dione", true),
        ("Rhea", true),
        ("Iapetus", true),
        ("Mimas", true),
        ("Io", false),
        ("Moon", false),
    ] {
        let body = moon(name);
        let (tides, ocean) = body.solved(ammonia);
        let on = tides.branch() == TidalBranch::Ocean;
        eprintln!(
            "{name}: {:?}, k₂ {:.4}, F {:.3} mW m⁻², {ocean:?}",
            tides.branch(),
            tides.k2(),
            tides.heat_flow().value() * 1e3
        );
        if on {
            let inputs = body.ocean_inputs(ammonia, tides.heat_flow());
            assert!(has_ocean(&inputs), "{name}");
            if let Subsurface::Ocean(published) = ocean {
                assert_eq!(implausibility(&inputs, &published), None, "{name}");
            }
        } else {
            assert_eq!(ocean, Subsurface::NoOcean, "{name}");
        }
        let dry = with_ocean_rules(OceanRules::Solved, || {
            MoonTides::with_ocean(
                &TideInputs {
                    water: 0.0,
                    ..body.inputs()
                },
                &body.solutes(ammonia),
            )
        });
        assert_ne!(dry.0.branch(), TidalBranch::Ocean, "{name}");
    }
}

/// The branch takes its largest stable root: over 10⁴ random moons, on a 256-point grid geometric
/// in 273.16 K − T* over [T*₀, 273.16 K], the excess F(T*) − `F_nt` − Φ Im k₂ is positive at every
/// point of a larger heat flow than the root the branch took; the moons whose excess changes sign
/// more than once, a stiff shell's tidal heat peaking at an intermediate depth, are counted.
#[test]
fn the_branch_takes_its_largest_stable_root_over_ten_thousand_moons() {
    let mut state = 0x0051_0060_b000_0003_u64;
    let mut uniform = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        f64::from(u32::try_from(z >> 32).unwrap()) / 4_294_967_296.0
    };
    let (mut oceans, mut multiple, mut missed) = (0_u32, 0_u32, Vec::new());
    for index in 0..10_000 {
        let base = moon(
            [
                "Europa",
                "Ganymede",
                "Callisto",
                "Titan",
                "Enceladus",
                "Rhea",
            ][index % 6],
        );
        let scale = |u: f64, lo: f64, hi: f64| lo * crate::math::powf(hi / lo, u);
        let body = Measured {
            gm: base.gm * scale(uniform(), 0.1, 10.0),
            a_km: base.a_km * scale(uniform(), 0.5, 2.0),
            e: scale(uniform(), 1e-4, 0.05),
            surface: 30.0 + 200.0 * uniform(),
            water: 0.02 + 0.5 * uniform(),
            ..base
        };
        let inputs = body.inputs();
        let solutes = body.solutes(uniform() < 0.5);
        let (density, gravity) = MoonTides::bulk(&inputs);
        let Some(branch) = ShellBranch::of(&inputs, &solutes, density, gravity) else {
            continue;
        };
        let (lo, hi) = (branch.water.deepest().value(), branch.water.top().value());
        let states: Vec<BranchState> = (0..=256)
            .map(|k| {
                let t = hi - (hi - lo) * crate::math::exp10(-6.0 * f64::from(256 - k) / 256.0);
                branch.state(t)
            })
            .collect();
        let changes = states
            .windows(2)
            .filter(|pair| (pair[0].excess_w_per_m2 > 0.0) != (pair[1].excess_w_per_m2 > 0.0))
            .count();
        multiple += u32::from(changes > 1);
        let solved = with_ocean_rules(OceanRules::Solved, || {
            MoonTides::with_ocean(&inputs, &solutes).0
        });
        if solved.branch() != TidalBranch::Ocean {
            continue;
        }
        oceans += 1;
        let root = solved.heat_flow().value();
        // Above the root (a thinner shell, a larger heat flow) the shell conducts more than the
        // moon makes, at every point of the fine grid.
        if let Some(state) = states.iter().find(|state| {
            state.base.heat_flow.value() > root * (1.0 + 1e-6) && state.excess_w_per_m2 <= 0.0
        }) {
            missed.push((index, body.name, state.base.melting_point, root));
        }
    }
    eprintln!(
        "10⁴ moons: {oceans} on the ocean branch, {multiple} whose excess changes sign more than \
         once on the grid"
    );
    assert!(oceans > 1_000, "{oceans}");
    assert_eq!(
        missed,
        Vec::new(),
        "moons with a larger stable root than the branch's"
    );
}

/// The measured moons' tides on P14.T51.g.b's spherical shell, pinned to the bit, under the
/// solve's rules ahead of their bump (the determinism review: no golden prints a solved ocean
/// until the bump re-blesses): each moon's branch, k₂, Q, loss and whole heat flow, and its
/// branch's ocean's shell, depth, floor, liquid, solutes and base temperature, so that a moved
/// curve, halving count, scan or profile moves a line on either target.
#[test]
fn the_measured_moons_tides_on_their_shell_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    for moon in MOONS.iter().chain(core::iter::once(&TRITON)) {
        let ammonia = !matches!(moon.name, "Europa" | "Ganymede" | "Io" | "Moon");
        let (tides, ocean) = moon.solved(ammonia);
        let name = moon.name.to_lowercase();
        w.line(&format!("{name}_branch = {:?}", tides.branch()));
        w.f64(&format!("{name}_k2"), tides.k2());
        w.f64(&format!("{name}_q"), tides.q());
        w.f64(&format!("{name}_loss"), tides.loss());
        w.f64(&format!("{name}_heat_flow"), tides.heat_flow().value());
        match ocean {
            Subsurface::Ocean(ocean) => {
                w.line(&format!(
                    "{name}_ocean = {:?} {}",
                    ocean.floor(),
                    ocean.liquid().substance().key().as_str()
                ));
                w.f64(&format!("{name}_shell"), ocean.shell_thickness().value());
                w.f64(&format!("{name}_thickness"), ocean.thickness().value());
                w.f64(&format!("{name}_solutes"), ocean.solute_mass_fraction());
                w.f64(&format!("{name}_base"), ocean.base_temperature().value());
            }
            other => w.line(&format!("{name}_ocean = {other:?}")),
        }
    }
    golden!("planetary/moon_tides_on_their_shell", w.as_str());
}
