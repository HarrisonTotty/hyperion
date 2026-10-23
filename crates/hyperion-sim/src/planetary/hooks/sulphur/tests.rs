//! P14.T24.g's sulphur: on Venus's measured state given by hand, on the Solar System table, and
//! over the outgassing.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::hooks::SurfaceConditions;
use crate::planetary::hooks::aerosols::{
    AerosolInputs, AerosolInventory, AerosolMode, ParticleSize,
};
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::structure::{RadiativeConvective, StructureInputs};
use crate::planetary::hooks::surface::regime::{ClimateInputs, surface_reservoirs};
use crate::planetary::hooks::surface::tests::table;
use crate::substance::SubstanceId;
use crate::units::Kelvin;

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// Seconds in a Julian year.
const SECONDS_PER_YEAR: f64 = 365.25 * 86_400.0;

/// Venus's surface gravity, m s⁻²: 8.87 (NASA's Venus fact sheet).
const VENUS_GRAVITY: f64 = 8.87;

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// An air by hand: its gases, structure, molar mass, gravity, surface pressure and state.
struct Air {
    gases: Vec<(SubstanceId, f64)>,
    structure: RadiativeConvective,
    mu: f64,
    g: f64,
    p_s: f64,
    state: SurfaceState,
    liquid_water: bool,
}

impl Air {
    fn inputs(&self, outgassing: f64) -> AerosolInputs<'_> {
        AerosolInputs {
            state: self.state,
            surface_pressure: Pascals::new(self.p_s),
            gases: &self.gases,
            mean_molar_mass_g_per_mol: self.mu,
            gravity: MetresPerSecondSquared::new(self.g),
            structure: &self.structure,
            cloud_fraction: 1.0,
            star_temperature: Some(Kelvin::new(5_772.0)),
            surface_liquid_cover: 0.0,
            outgassing,
            liquid_water: self.liquid_water,
        }
    }

    fn inventory(&self, outgassing: f64) -> AerosolInventory {
        AerosolInventory::derive(&self.inputs(outgassing)).expect("an inventory")
    }
}

/// Venus given its measured state by hand (P14.T24.g's tests): P14.T57.a's measured inventory, 4.69
/// × 10²⁰ kg of CO₂ and 1.08 × 10¹⁹ kg of N₂ (about 90.4 and 2.09 bar by weight), over its 6,051.8 km
/// radius under 8.87 m s⁻², so that its air column is [`VENUS_AIR_COLUMN`]; at a ground of `t_s`
/// kelvin under the skin of an effective temperature `t_eff` (its 226.6 K black-body temperature,
/// NASA's fact sheet, as P14.T24.e's test takes it).
fn measured_venus(t_s: f64, t_eff: f64) -> Air {
    let (co2, n2) = (row("CO2"), row("N2"));
    // The molar masses VENUS_AIR_COLUMN takes, kg mol⁻¹.
    let (c, n) = (4.69e20 / 44.009_5e-3, 1.08e19 / 28.013_4e-3);
    let gases = vec![(co2, c / (c + n)), (n2, n / (c + n))];
    let mu = (4.69e20 + 1.08e19) / (c + n) * 1_000.0;
    let area = 4.0 * core::f64::consts::PI * 6.051_8e6 * 6.051_8e6;
    let p_s = (4.69e20 + 1.08e19) * VENUS_GRAVITY / area;
    let structure = RadiativeConvective::derive(&StructureInputs {
        surface_temperature: Kelvin::new(t_s),
        surface_pressure: Pascals::new(p_s),
        gases: &gases,
        condensable: None,
        skin_temperature: Kelvin::new(t_eff * math::powf(0.5, 0.25)),
    })
    .expect("an air");
    Air {
        gases,
        structure,
        mu,
        g: VENUS_GRAVITY,
        p_s,
        state: SurfaceState::RunawayGreenhouse,
        liquid_water: false,
    }
}

/// The table's body `name`: its derivation, conditions with their regime, and structure.
fn table_air(name: &str) -> (DerivedBody, SurfaceConditions, RadiativeConvective) {
    let (_, body, conditions) = table()
        .into_iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("{name} is in the table"));
    let figures = table_figures(&body).expect("the table's bodies have figures");
    let published = figures
        .clone()
        .with_surface(&conditions, VolatileHistory::NONE);
    let reservoirs = surface_reservoirs(body.atmosphere());
    let conditions = conditions.with_climate(&ClimateInputs {
        mass: body.mass(),
        reservoirs: &reservoirs,
        from_colder: false,
        ice_fraction: published.ice_fraction(),
        ice_threshold: published.ice_threshold(),
    });
    let inputs = StructureInputs::of_surface(
        &conditions,
        body.equilibrium_temperature(),
        figures.heat_flow(),
    )
    .expect("the conditions carry their regime");
    let structure = RadiativeConvective::derive(&inputs).expect("an air");
    (body, conditions, structure)
}

/// The table's inventory of `name` at its own outgassing.
fn table_inventory(name: &str) -> (AerosolInventory, f64) {
    let (body, conditions, structure) = table_air(name);
    let outgassing = table_figures(&body).map_or(0.0, |f| f.outgassing());
    let inputs = AerosolInputs {
        state: body.atmosphere().state(),
        surface_pressure: conditions.surface_pressure(),
        gases: conditions.gases(),
        mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol().unwrap_or(f64::NAN),
        gravity: body.surface_gravity(),
        structure: &structure,
        cloud_fraction: 1.0,
        star_temperature: Some(Kelvin::new(5_772.0)),
        surface_liquid_cover: 0.0,
        outgassing,
        liquid_water: conditions.liquid_water(),
    };
    (
        AerosolInventory::derive(&inputs).unwrap_or_else(|e| panic!("{name}: {e}")),
        outgassing,
    )
}

/// The acid's modes in `inventory`, by its deck.
fn acid_modes(inventory: &AerosolInventory) -> Vec<&AerosolMode> {
    let acid = row("H2SO4");
    inventory
        .modes()
        .iter()
        .filter(|m| m.deck() == acid)
        .collect()
}

/// Every τ in `inventory` is finite and non-negative, and every mode's base is no shallower than
/// its top.
fn assert_sound(inventory: &AerosolInventory) {
    for mode in inventory.modes() {
        if let Ok(depth) = mode.optical_depth() {
            assert!(
                depth.value().is_finite() && depth.value() >= 0.0,
                "{mode:?}"
            );
        }
        assert!(mode.profile().base().value() >= mode.profile().top().value());
    }
}

/// Venus given its measured state by hand carries a deck of four modes, mode 2
/// `acid_uv_absorber` and the rest `H2SO4`: its SO₂ 130 ppm to 10⁻⁶ (by
/// [`DRY_SULPHUR_LIFETIME`]), its vapour at the base 5–10 ppm (by `ε_v`), its τ(550) 34.5 to 10⁻⁶
/// (by [`ACID_DECK_CROSS_SECTION`]), its shares as tabled (over their sum, 0.99) to 10⁻⁹, and its
/// base at 345–390 K with
/// its top at the skin level. As built: the base 1.313 bar at 365.8 K and the top 0.0556 bar,
/// against VIRA's 1.38 and 0.037 bar.
#[test]
fn the_measured_venus_carries_its_four_mode_deck() {
    let venus = measured_venus(735.0, 226.6);
    let inventory = venus.inventory(VENUS_OUTGASSING);
    assert_sound(&inventory);
    let sulphur = Sulphur::derive(&SulphurInputs {
        state: venus.state,
        outgassing: VENUS_OUTGASSING,
        starlit: true,
        liquid_water: false,
        surface_pressure: Pascals::new(venus.p_s),
        mean_molar_mass_g_per_mol: venus.mu,
        gravity: MetresPerSecondSquared::new(venus.g),
        structure: &venus.structure,
    });
    assert_eq!(sulphur.cycle(), SulphurCycle::Cycling);
    assert!((sulphur.mole_fraction() / VENUS_SULPHUR_DIOXIDE - 1.0).abs() < 1e-6);
    let deck = inventory
        .decks()
        .iter()
        .find(|d| d.condensate() == row("H2SO4"))
        .expect("an acid deck");
    assert!(
        (5e-6..10e-6).contains(&deck.vapour_below()),
        "{}",
        deck.vapour_below()
    );
    let base_t = deck.base_temperature().value();
    assert!((345.0..390.0).contains(&base_t), "base at {base_t} K");
    assert_same_bits(
        deck.top().value(),
        venus.structure.tropopause_pressure().value(),
    );
    eprintln!(
        "measured Venus: base {:.4} bar at {base_t:.1} K, top {:.4} bar",
        deck.base().value() / PASCALS_PER_BAR,
        deck.top().value() / PASCALS_PER_BAR
    );
    let modes = acid_modes(&inventory);
    assert_eq!(modes.len(), 4);
    let tau = inventory.deck_optical_depth(row("H2SO4")).unwrap();
    assert!(
        (tau / VENUS_CLOUD_OPTICAL_DEPTH - 1.0).abs() < 1e-6,
        "τ {tau}"
    );
    for (mode, tabled) in modes.iter().zip(&ACID_MODES) {
        assert_eq!(mode.material(), row(tabled.material));
        assert_eq!(mode.phase(), CondensedPhase::Liquid);
        assert_eq!(mode.shape(), ShapeClass::Sphere);
        let share = mode.optical_depth().unwrap().value() / tau;
        let shares: f64 = ACID_MODES.iter().map(|m| m.share).sum();
        let expected = tabled.share / shares;
        assert!(
            (share - expected).abs() < 1e-9,
            "{share} against {expected}"
        );
        assert_same_bits(mode.profile().exponent(), 0.0);
    }
    let so2 = inventory
        .absorbers()
        .iter()
        .find(|a| a.species() == row("SO2"))
        .expect("SO₂ is an absorber");
    assert!((so2.column_per_m2() / (VENUS_SULPHUR_DIOXIDE * VENUS_AIR_COLUMN) - 1.0).abs() < 1e-6);
}

/// The acid modes' sizes are Pollack et al. 1993's log-normals: mode 2's `r_eff` 1.17 µm and
/// `v_eff` 0.064, beside Hansen and Hovenier 1974's 1.05 ± 0.10 µm and 0.07 ± 0.02.
#[test]
fn the_acid_modes_are_pollack_s() {
    let inventory = measured_venus(735.0, 226.6).inventory(VENUS_OUTGASSING);
    let modes = acid_modes(&inventory);
    let Ok(ParticleSize::Distribution(upper)) = modes[1].size() else {
        panic!("a distribution");
    };
    assert!((upper.effective_radius().value() * 1e6 - 1.17).abs() < 0.01);
    assert!((upper.effective_variance() - 0.064).abs() < 0.001);
    for (mode, (r_eff, v_eff)) in
        modes
            .iter()
            .zip([(0.49, 0.21), (1.17, 0.064), (1.56, 0.045), (4.27, 0.064)])
    {
        let Ok(ParticleSize::Distribution(size)) = mode.size() else {
            panic!("a distribution");
        };
        assert!((size.effective_radius().value() * 1e6 / r_eff - 1.0).abs() < 0.01);
        assert!((size.effective_variance() / v_eff - 1.0).abs() < 0.03);
    }
}

/// The table's median Venus carries a deck of four modes with τ(550) above 10, so that R08 classes
/// it `cloudDeck`, its base below 3 bar and above its top. As built: printed for the bless note.
#[test]
fn the_table_s_venus_carries_a_thick_deck() {
    let (inventory, outgassing) = table_inventory("Venus");
    assert_sound(&inventory);
    let deck = inventory
        .decks()
        .iter()
        .find(|d| d.condensate() == row("H2SO4"))
        .expect("an acid deck");
    let tau = inventory.deck_optical_depth(row("H2SO4")).unwrap();
    // Its SO₂ by number, the acid's vapour below over ε_v (the law's share).
    let share = crate::substance::aloft::law_aloft(row("H2SO4"))
        .expect("the acid's law")
        .vapour_share();
    eprintln!(
        "the table's Venus: O {outgassing}, base {:.4} bar at {:.1} K, top {:.4} bar, τ {tau:.3}, \
         SO2 {:.1} ppm",
        deck.base().value() / PASCALS_PER_BAR,
        deck.base_temperature().value(),
        deck.top().value() / PASCALS_PER_BAR,
        deck.vapour_below() / share * 1e6
    );
    assert_eq!(acid_modes(&inventory).len(), 4);
    assert!(tau > 10.0, "τ {tau}");
    assert!(deck.base().value() < 3.0 * PASCALS_PER_BAR && deck.base() > deck.top());
}

/// The table's Earth carries no acid deck, but a sulphate layer from its tropopause to a fifth of
/// it with τ(550) between 10⁻³ and 10⁻², and under 1 ppb of SO₂ (P14.T24.g computed τ 0.0032 and
/// 14 ppt; as built 0.00324, from 0.135 to 0.027 bar).
#[test]
fn the_table_s_earth_carries_a_thin_sulphate_layer() {
    let (inventory, _) = table_inventory("Earth");
    assert_sound(&inventory);
    let modes = acid_modes(&inventory);
    assert_eq!(modes.len(), 1, "{modes:?}");
    assert!(
        !inventory
            .decks()
            .iter()
            .any(|d| d.condensate() == row("H2SO4"))
    );
    let layer = modes[0];
    let tau = layer.optical_depth().unwrap().value();
    assert!((1e-3..1e-2).contains(&tau), "τ {tau}");
    let (base, top) = (
        layer.profile().base().value(),
        layer.profile().top().value(),
    );
    assert!((top / base - SULPHATE_LAYER_TOP_RATIO).abs() < 1e-12);
    assert!((0.1e5..0.2e5).contains(&base), "{base} Pa");
    let so2 = inventory
        .absorbers()
        .iter()
        .find(|a| a.species() == row("SO2"))
        .expect("SO₂ is an absorber");
    let (body, conditions, _) = table_air("Earth");
    let mu = conditions.mean_molar_mass_g_per_mol().unwrap();
    let air = conditions.surface_pressure().value() * AVOGADRO
        / (mu / 1_000.0 * body.surface_gravity().value());
    assert!(
        so2.column_per_m2() / air < 1e-9,
        "{} of the air",
        so2.column_per_m2() / air
    );
    assert!(modes.iter().all(|m| m.material() == row("H2SO4")));
}

/// The Moon, Mercury, Mars and Titan carry no sulphur (O is 0, or there is no air), and neither does
/// a body with no star as host.
#[test]
fn bodies_without_outgassing_air_or_starlight_carry_no_sulphur() {
    for name in ["Moon", "Mercury", "Mars", "Titan"] {
        if let Some((_, body, _)) = table().into_iter().find(|(n, ..)| *n == name) {
            let outgassing = table_figures(&body).map_or(0.0, |f| f.outgassing());
            let airless = body.atmosphere().state() == SurfaceState::Airless;
            assert!(airless || outgassing <= 0.0, "{name}: O {outgassing}");
            if !airless {
                let (inventory, _) = table_inventory(name);
                assert!(acid_modes(&inventory).is_empty(), "{name}");
                assert!(
                    !inventory
                        .absorbers()
                        .iter()
                        .any(|a| a.species() == row("SO2"))
                );
            }
        }
    }
    let venus = measured_venus(735.0, 226.6);
    let mut inputs = venus.inputs(VENUS_OUTGASSING);
    inputs.star_temperature = None;
    let dark = AerosolInventory::derive(&inputs).unwrap();
    assert_eq!(acid_modes(&dark), Vec::<&AerosolMode>::new());
    assert_eq!(dark.absorbers(), []);
}

/// On a measured Venus τ(550) rises monotonically with the outgassing and tends to zero with it;
/// raising its equilibrium temperature, with its ground's in proportion, thins the deck and removes
/// it by 400 K (Lincowski et al. 2018's TRAPPIST-1 b, near 400 K, condenses no acid).
#[test]
fn the_deck_follows_the_outgassing_and_thins_with_heat() {
    let venus = measured_venus(735.0, 226.6);
    let tau_at = |air: &Air, o: f64| {
        air.inventory(o)
            .deck_optical_depth(row("H2SO4"))
            .unwrap_or(0.0)
    };
    let mut previous = 0.0;
    for o in [1e-6, 1e-3, 1e-2, 0.1, VENUS_OUTGASSING, 1.0, 3.0] {
        let tau = tau_at(&venus, o);
        assert!(tau >= previous, "τ {tau} at O {o}");
        previous = tau;
    }
    assert!(tau_at(&venus, 1e-6) < 1e-3 * tau_at(&venus, VENUS_OUTGASSING));
    let mut previous = f64::INFINITY;
    for t_eq in [226.6, 260.0, 300.0, 350.0, 400.0] {
        let scale = t_eq / 226.6;
        let hot = measured_venus(735.0 * scale, t_eq);
        let tau = tau_at(&hot, VENUS_OUTGASSING);
        assert!(tau <= previous, "τ {tau} at T_eq {t_eq} K");
        previous = tau;
    }
    assert!(previous <= 0.0, "a deck at 400 K: τ {previous}");
}

/// No temperate or snowball world carries an acid deck at any outgassing up to 10³ Earth's, and its
/// layer stays under τ 1 (Loftus et al. 2019); no case-2 layer, and no world without a case-1 deck,
/// carries `acid_uv_absorber`.
#[test]
fn a_temperate_world_keeps_a_thin_layer_at_any_outgassing() {
    let (body, conditions, structure) = table_air("Earth");
    for o in [0.1, 1.0, 10.0, 100.0, 1e3] {
        for (state, wet) in [
            (SurfaceState::Temperate, true),
            (SurfaceState::Temperate, false),
            (SurfaceState::Snowball, false),
        ] {
            let inputs = AerosolInputs {
                state,
                surface_pressure: conditions.surface_pressure(),
                gases: conditions.gases(),
                mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol().unwrap(),
                gravity: body.surface_gravity(),
                structure: &structure,
                cloud_fraction: 0.675,
                star_temperature: Some(Kelvin::new(5_772.0)),
                surface_liquid_cover: 0.7,
                outgassing: o,
                liquid_water: wet,
            };
            let inventory = AerosolInventory::derive(&inputs).unwrap();
            assert_sound(&inventory);
            assert!(
                !inventory
                    .decks()
                    .iter()
                    .any(|d| d.condensate() == row("H2SO4"))
            );
            let modes = acid_modes(&inventory);
            assert_eq!(modes.len(), 1);
            assert!(modes[0].optical_depth().unwrap().value() < 1.0);
            assert!(
                inventory
                    .modes()
                    .iter()
                    .all(|m| m.material() != row("acid_uv_absorber"))
            );
        }
    }
    assert!((sulphate_layer_optical_depth(EARTH_SULPHUR_DIOXIDE_FLUX) - 0.00324).abs() < 1e-5);
}

/// The source: Earth's 23 Tg SO₂ yr⁻¹ over its surface is [`EARTH_SULPHUR_DIOXIDE_FLUX`] to 0.3%,
/// the dry lifetime is about 4.4 Myr at O = 1 (P14.T24.g), and [`VENUS_OUTGASSING`] is the table's
/// Venus's present outgassing (T24.b), at which the lifetime is fixed.
#[test]
fn the_dry_lifetime_is_fixed_on_venus() {
    let flux = 23e12 / 64.066 * AVOGADRO
        / SECONDS_PER_YEAR
        / (4.0 * core::f64::consts::PI * 6.371e6 * 6.371e6);
    assert!(
        (flux / EARTH_SULPHUR_DIOXIDE_FLUX - 1.0).abs() < 3e-3,
        "{flux}"
    );
    let at_one = VENUS_SULPHUR_DIOXIDE * VENUS_AIR_COLUMN / EARTH_SULPHUR_DIOXIDE_FLUX;
    let myr = at_one / SECONDS_PER_YEAR / 1e6;
    assert!((myr - 4.4).abs() < 0.1, "{myr} Myr");
    let (_, outgassing) = table_inventory("Venus");
    assert!(
        (outgassing / VENUS_OUTGASSING - 1.0).abs() < 1e-12,
        "{outgassing}"
    );
    assert!((VENUS_AIR_COLUMN / 1.445e31 - 1.0).abs() < 1e-3);
}

/// Sulphur is continuous in the outgassing and in the ground's temperature away from a state
/// change: a part in 10⁶ of either moves the measured Venus's τ by under 10⁻⁴ of itself.
#[test]
fn the_sulphur_is_continuous() {
    let tau = |t_s: f64, o: f64| {
        measured_venus(t_s, 226.6)
            .inventory(o)
            .deck_optical_depth(row("H2SO4"))
            .unwrap()
    };
    let at = tau(735.0, VENUS_OUTGASSING);
    for (t_s, o) in [
        (735.0 * (1.0 + 1e-6), VENUS_OUTGASSING),
        (735.0, VENUS_OUTGASSING * (1.0 + 1e-6)),
    ] {
        assert!((tau(t_s, o) / at - 1.0).abs() < 1e-4);
    }
}

/// Once through, SO₂'s column is its flux times its life: [`WET_SULPHUR_LIFETIME`] over liquid
/// water and [`DRY_ONCE_THROUGH_LIFETIME`] without; and a cycling world's is the flux times
/// [`DRY_SULPHUR_LIFETIME`], whatever its air.
#[test]
fn the_column_is_the_flux_times_its_life() {
    let (_, conditions, structure) = table_air("Earth");
    let inputs = |state: SurfaceState, liquid_water: bool| SulphurInputs {
        state,
        outgassing: 2.0,
        starlit: true,
        liquid_water,
        surface_pressure: conditions.surface_pressure(),
        mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol().unwrap(),
        gravity: MetresPerSecondSquared::new(9.81),
        structure: &structure,
    };
    let flux = 2.0 * EARTH_SULPHUR_DIOXIDE_FLUX;
    for (state, wet, life) in [
        (SurfaceState::Temperate, true, WET_SULPHUR_LIFETIME),
        (SurfaceState::Temperate, false, DRY_ONCE_THROUGH_LIFETIME),
        (SurfaceState::Snowball, false, DRY_ONCE_THROUGH_LIFETIME),
        (SurfaceState::RunawayGreenhouse, false, DRY_SULPHUR_LIFETIME),
    ] {
        let sulphur = Sulphur::derive(&inputs(state, wet));
        assert_same_bits(sulphur.flux(), flux);
        assert!(
            (sulphur.column_per_m2() / (flux * life.value()) - 1.0).abs() < 1e-15,
            "{state:?}"
        );
    }
    for state in [
        SurfaceState::Airless,
        SurfaceState::MagmaOcean,
        SurfaceState::GasEnvelope,
    ] {
        assert_eq!(Sulphur::derive(&inputs(state, false)), Sulphur::NONE);
    }
}

/// The same inputs give the same bits twice: the measured Venus's and the table's Earth's.
#[test]
fn the_sulphur_is_the_same_bits_twice() {
    let venus = || measured_venus(735.0, 226.6).inventory(VENUS_OUTGASSING);
    assert_eq!(format!("{:?}", venus()), format!("{:?}", venus()));
    let earth = || table_inventory("Earth").0;
    assert_eq!(format!("{:?}", earth()), format!("{:?}", earth()));
}
