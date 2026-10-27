//! P14.T24.c's inventory: on the Solar System table, on airs by hand, and against rendering plan
//! R08's index files.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::hooks::SurfaceConditions;
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::structure::StructureInputs;
use crate::planetary::hooks::surface::regime::{ClimateInputs, surface_reservoirs};
use crate::planetary::hooks::surface::tests::table;
use crate::substance::by_key;

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The table's body `name`: its derivation, its conditions with their climate regime, its figures'
/// cloud fraction stand-in (the second stage's), and its structure.
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

/// The inputs of the table's air, its decks covering `cloud_fraction` of the sky.
fn inputs_of<'a>(
    body: &DerivedBody,
    conditions: &'a SurfaceConditions,
    structure: &'a RadiativeConvective,
    cloud_fraction: f64,
) -> AerosolInputs<'a> {
    AerosolInputs {
        state: body.atmosphere().state(),
        surface_pressure: conditions.surface_pressure(),
        gases: conditions.gases(),
        mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol().unwrap_or(f64::NAN),
        gravity: body.surface_gravity(),
        structure,
        cloud_fraction,
        star_temperature: Some(Kelvin::new(5_772.0)),
        surface_liquid_cover: table_figures(body).map_or(0.0, |f| {
            f.with_surface(conditions, VolatileHistory::NONE)
                .ocean_fraction()
        }),
        outgassing: table_figures(body).map_or(0.0, |f| f.outgassing()),
        liquid_water: conditions.liquid_water(),
        runaway_deck: RunawayDeck::Kept,
    }
}

/// The table's inventory of `name` under a cloud fraction of `cloud_fraction`.
fn table_inventory(name: &str, cloud_fraction: f64) -> AerosolInventory {
    let (body, conditions, structure) = table_air(name);
    AerosolInventory::derive(&inputs_of(&body, &conditions, &structure, cloud_fraction))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The deck of `key` in `inventory`.
fn deck<'a>(inventory: &'a AerosolInventory, key: &str) -> &'a AerosolDeck {
    inventory
        .decks()
        .iter()
        .find(|d| d.condensate() == row(key))
        .unwrap_or_else(|| panic!("a {key} deck"))
}

/// An air by hand: `gases` at `t_s` kelvin and `p_s` pascals under a skin of `skin` kelvin, its
/// condensable `condensable`, of molar mass `mu` g mol⁻¹ under gravity `g` m s⁻².
struct HandAir {
    gases: Vec<(SubstanceId, f64)>,
    structure: RadiativeConvective,
    mu: f64,
    g: f64,
    p_s: f64,
}

impl HandAir {
    fn new(
        gases: &[(&str, f64)],
        condensable: Option<&str>,
        t_s: f64,
        p_s: f64,
        skin: f64,
    ) -> Self {
        let gases: Vec<(SubstanceId, f64)> = gases.iter().map(|&(k, x)| (row(k), x)).collect();
        let structure = RadiativeConvective::derive(&StructureInputs {
            surface_temperature: Kelvin::new(t_s),
            surface_pressure: Pascals::new(p_s),
            gases: &gases,
            condensable: condensable.map(row),
            skin_temperature: Kelvin::new(skin),
        })
        .expect("an air");
        let mu = gases.iter().fold(0.0, |sum, &(id, x)| {
            sum + x * id.substance().molar_mass_g_per_mol().expect("a molar mass")
        });
        Self {
            gases,
            structure,
            mu,
            g: 9.81,
            p_s,
        }
    }

    fn inputs(&self, state: SurfaceState) -> AerosolInputs<'_> {
        AerosolInputs {
            state,
            surface_pressure: Pascals::new(self.p_s),
            gases: &self.gases,
            mean_molar_mass_g_per_mol: self.mu,
            gravity: MetresPerSecondSquared::new(self.g),
            structure: &self.structure,
            cloud_fraction: 0.5,
            star_temperature: Some(Kelvin::new(5_772.0)),
            surface_liquid_cover: 0.0,
            outgassing: 0.0,
            liquid_water: false,
            runaway_deck: RunawayDeck::Kept,
        }
    }

    fn inventory(&self, state: SurfaceState) -> AerosolInventory {
        AerosolInventory::derive(&self.inputs(state)).expect("an inventory")
    }
}

/// The table's Earth carries a water deck whose vapour at the ground is T24.a's mole fraction to
/// 10⁻¹², covering the cloud fraction given, and lists no ozone: its O₂ is biotic (P14.T24.f), so
/// a generated Earth has none.
#[test]
fn the_table_s_earth_carries_water_decks_and_no_ozone() {
    let (body, conditions, structure) = table_air("Earth");
    let inventory =
        AerosolInventory::derive(&inputs_of(&body, &conditions, &structure, 0.675)).unwrap();
    let water = deck(&inventory, "H2O");
    let ground = conditions
        .gases()
        .iter()
        .find(|(id, _)| *id == row("H2O"))
        .map(|&(_, x)| x)
        .expect("Earth's air holds water");
    assert!((water.vapour_below() - ground).abs() <= 1e-12 * ground);
    assert!(water.base().value() < conditions.surface_pressure().value());
    // A cold trap at the tropopause: the skin of T24.e is warmer than the real one (214 K).
    assert!(water.vapour_above() < 0.05 * water.vapour_below());
    assert!(
        !inventory
            .absorbers()
            .iter()
            .any(|a| a.species() == row("O3")),
        "{:?}",
        inventory.absorbers()
    );
    for mode in inventory.modes().iter().filter(|m| m.deck() == row("H2O")) {
        assert_same_bits(mode.coverage(), 0.675);
        assert_eq!(mode.material(), row("H2O"));
    }
}

/// The decks' τ(550) on the table's anchors, under the serve gate's rule (the module's
/// documentation; P14.T24.c's Risks): Earth's water 1–20 in the cloud (Rossow and Schiffer 1999,
/// BAMS 80, 2261: a mean cloud optical thickness of 3.7 ± 0.3 at a cloud amount of 0.675), Mars's
/// water 0.03–3 (its thick Tharsis hazes about 1, Madeleine et al. 2012, JGR 117: 2.9 pr-µm of ice
/// at `r_eff` 2–3.5 µm), and Titan's methane 0.3–30 in the cloud, its sizes rain's (Lorenz et al.
/// 2005, GRL 32: a cover of about 1%), all under 35. As built: printed.
#[test]
fn the_table_s_decks_take_plausible_optical_depths() {
    for (name, key, low, high) in [
        ("Earth", "H2O", 1.0, 20.0),
        ("Mars", "H2O", 0.03, 3.0),
        ("Titan", "CH4", 0.3, 30.0),
    ] {
        let inventory = table_inventory(name, 1.0);
        let condensate = row(key);
        let tau = inventory
            .deck_optical_depth(condensate)
            .unwrap_or_else(|| panic!("{name}'s {key} deck has a τ"));
        let d = deck(&inventory, key);
        eprintln!(
            "{name} {key}: base {:.4} bar at {:.1} K, top {:.4} bar, f_sed {:.4e}, tau {tau:.4}",
            d.base().value() / PASCALS_PER_BAR,
            d.base_temperature().value(),
            d.top().value() / PASCALS_PER_BAR,
            d.sedimentation()
        );
        assert!((low..high).contains(&tau), "{name}'s {key} at τ {tau}");
        for mode in inventory.modes() {
            if let Ok(ParticleSize::Distribution(size)) = mode.size() {
                let r = size.effective_radius().value();
                assert!((1e-7..3e-4).contains(&r), "{name}: r_eff {r} m");
            }
        }
    }
}

/// A deck's `f_sed` is its supply's: v H ÷ K with v = v₁ `M_vap`^β, at least
/// [`LEAST_SEDIMENTATION_EFFICIENCY`], at `σ_g` [`DECK_WIDTH`].
#[test]
fn a_deck_s_sedimentation_is_its_supply_s() {
    let (body, conditions, structure) = table_air("Earth");
    let inputs = inputs_of(&body, &conditions, &structure, 1.0);
    let column = MixingColumn {
        gravity: inputs.gravity,
        molar_mass_g_per_mol: inputs.mean_molar_mass_g_per_mol,
        eddy_diffusion: CLOUD_EDDY_DIFFUSION,
        metallicity: 1.0,
        top: Pascals::new(structure.tropopause_pressure().value() * 1e-2),
        bottom: inputs.surface_pressure,
    };
    let clouds = Condensation::derive(
        inputs.gases.iter().copied(),
        |p| structure.temperature_at(p),
        &column,
        |_| Sedimentation::BASELINE,
        1.0,
    )
    .unwrap();
    let water = clouds
        .decks()
        .iter()
        .find(|d| d.condensate() == row("H2O"))
        .expect("a water deck");
    let mu = inputs.mean_molar_mass_g_per_mol;
    let g = inputs.gravity.value();
    let molar = row("H2O").substance().molar_mass_g_per_mol().unwrap();
    let supply = molar / mu * water.vapour_below() * water.base().value() / g;
    let h = MOLAR_GAS_CONSTANT * water.base_temperature().value() / (mu / 1e3 * g);
    let expected = SEDIMENTATION_SPEED_AT_UNIT_SUPPLY.value()
        * math::powf(supply, SEDIMENTATION_SUPPLY_EXPONENT)
        * h
        / 10.0;
    let rule = sedimentation_for_supply(water, &column).unwrap();
    assert!(
        (rule.efficiency() / expected - 1.0).abs() < 1e-4,
        "{rule:?}"
    );
    assert_same_bits(rule.width(), DECK_WIDTH);
    let inventory = AerosolInventory::derive(&inputs).unwrap();
    assert_same_bits(deck(&inventory, "H2O").sedimentation(), rule.efficiency());
}

/// A warm air whose vapour never saturates forms no deck, and an airless body, a magma ocean and a
/// gas envelope hold an empty inventory.
#[test]
fn an_unsaturated_or_airless_body_holds_no_aerosols() {
    let dry = HandAir::new(
        &[("N2", 1.0 - 1e-9), ("H2O", 1e-9)],
        None,
        320.0,
        1e5,
        280.0,
    );
    assert_eq!(dry.inventory(SurfaceState::Temperate).decks(), []);
    let earthlike = HandAir::new(
        &[("N2", 0.9869), ("H2O", 0.0131)],
        Some("H2O"),
        288.0,
        1e5,
        214.0,
    );
    assert_ne!(earthlike.inventory(SurfaceState::Temperate).decks(), []);
    for state in [
        SurfaceState::Airless,
        SurfaceState::MagmaOcean,
        SurfaceState::GasEnvelope,
    ] {
        assert_eq!(earthlike.inventory(state), AerosolInventory::empty());
    }
    let mut none = earthlike.inputs(SurfaceState::Temperate);
    none.surface_pressure = Pascals::new(0.0);
    assert_eq!(
        AerosolInventory::derive(&none).unwrap(),
        AerosolInventory::empty()
    );
}

/// P14.T53.c (decision-p14-t53c-tenuous-aerosols §3): the inventory's rules are not applied to a
/// `Tenuous` air, which the record sends not modelled until P14.T55.n. A Triton-like 1.4 Pa of N₂
/// over 0.78 m s⁻² with x(CH₄) 10⁻³ about a G star takes the empty inventory, where the methane
/// rule would give it τ(550) 4.3 by its index alone (Pluto's measured haze is about 0.013 at 607.6
/// nm, Gladstone et al. 2016), as the same air under a bulk air's state shows.
#[test]
fn a_tenuous_air_takes_no_rule_of_the_inventory() {
    let mut triton = HandAir::new(&[("N2", 1.0 - 1e-3), ("CH4", 1e-3)], None, 38.0, 1.4, 38.0);
    triton.g = 0.78;
    let tau = haze_optical_depth(haze_index(1e-3, 0.0));
    let bulk = triton.inventory(SurfaceState::Snowball);
    eprintln!(
        "a Triton-like 1.4 Pa N2 air at x(CH4) 1e-3: the rule's τ {tau:.4}; under a bulk air's \
         state {} modes",
        bulk.modes().len()
    );
    assert!((tau - 4.3).abs() < 0.05, "{tau}");
    assert!(!bulk.modes().is_empty(), "{bulk:?}");
    assert_eq!(
        triton.inventory(SurfaceState::Tenuous),
        AerosolInventory::empty()
    );
}

/// P14.T53.c's Pluto (decision-p14-t53c-tenuous-aerosols §3), every 24 hold in force: `Tenuous`
/// through its passes and at its measured 0.72 Bond albedo by hand, its haze not modelled at 24
/// (P14.T55.n). Stated, not asserted: its x(CH₄), T24.c's haze index and the τ(550) that rule would
/// give it. Its CH₄, on the minor branch at its mean, some 3 × 10⁻⁷ Pa against a measured x of
/// 0.006–0.0084 (Gladstone et al. 2016), is P14.T55.e's figure.
#[test]
fn under_every_24_hold_pluto_s_haze_is_not_modelled_at_24() {
    use crate::planetary::derive::atmosphere::tests::{on_pluto_s_orbit, on_pluto_s_orbit_at};
    use crate::planetary::derive::atmosphere::{Gas, atmosphere};
    crate::planetary::testing::with_every_24_hold(|| {
        let pluto = on_pluto_s_orbit(1.303e22, 1_188.3);
        for (label, air) in [
            ("through its passes", *pluto.atmosphere()),
            (
                "at its measured 0.72",
                atmosphere(&on_pluto_s_orbit_at(&pluto, 0.72)),
            ),
        ] {
            let partials = air.partial_pressures();
            let moles = |gas: Gas| partials.of(gas).value() / gas.molar_mass_g_per_mol();
            let all = Gas::ALL.iter().fold(0.0, |sum, &gas| sum + moles(gas));
            let (methane, carbon_dioxide) =
                (moles(Gas::Methane) / all, moles(Gas::CarbonDioxide) / all);
            let index = haze_index(methane, carbon_dioxide);
            eprintln!(
                "Pluto {label}: {:?} at {:.4e} Pa, x(CH4) {methane:.4e}, T24.c's haze index \
                 {index:.4e}, its rule's τ(550) {:.4}; its haze not modelled at 24 (P14.T55.n)",
                air.state(),
                air.surface_pressure().map_or(f64::NAN, Pascals::value),
                haze_optical_depth(index),
            );
            assert_eq!(air.state(), SurfaceState::Tenuous, "{label}: {air:?}");
        }
    });
}

/// The inventory refuses bad inputs by their variants: its cloud fraction, gravity and molar mass.
#[test]
fn bad_inputs_are_refused_by_their_variants() {
    let air = HandAir::new(
        &[("N2", 0.9869), ("H2O", 0.0131)],
        Some("H2O"),
        288.0,
        1e5,
        214.0,
    );
    let mut inputs = air.inputs(SurfaceState::Temperate);
    inputs.cloud_fraction = 1.5;
    assert_eq!(
        AerosolInventory::derive(&inputs),
        Err(DeriveAerosolsError::CloudFraction(1.5))
    );
    let mut inputs = air.inputs(SurfaceState::Temperate);
    inputs.gravity = MetresPerSecondSquared::new(0.0);
    assert!(matches!(
        AerosolInventory::derive(&inputs),
        Err(DeriveAerosolsError::Column {
            field: "gravity",
            ..
        })
    ));
    let mut inputs = air.inputs(SurfaceState::Temperate);
    inputs.mean_molar_mass_g_per_mol = f64::NAN;
    assert!(matches!(
        AerosolInventory::derive(&inputs),
        Err(DeriveAerosolsError::Column {
            field: "molar mass",
            ..
        })
    ));
}

/// An absorber's column and layer, a rule's τ and coverage, and a deck's sedimentation of no finite
/// supply are refused by their variants.
#[test]
fn bad_absorbers_modes_and_sedimentation_are_refused() {
    let air = HandAir::new(
        &[("N2", 0.9869), ("H2O", 0.0131)],
        Some("H2O"),
        288.0,
        1e5,
        214.0,
    );
    let so2 = row("SO2");
    let (deep, shallow) = (Pascals::new(1e5), Pascals::new(1e3));
    assert_eq!(
        Absorber::slab(so2, -1.0, deep, shallow),
        Err(BuildAbsorberError::Column(-1.0))
    );
    assert_eq!(
        Absorber::slab(so2, 1e20, shallow, deep),
        Err(BuildAbsorberError::Layer)
    );
    assert_eq!(
        Absorber::profile(so2, 1e20, [(shallow, 1.0), (deep, 1.0)]),
        Err(BuildAbsorberError::Layer)
    );
    assert_eq!(
        Absorber::profile(so2, 1e20, [(deep, 1.0)]),
        Err(BuildAbsorberError::Layer)
    );
    let dust = row("mars_dust");
    let size = SizeDistribution::gamma(DUST_EFFECTIVE_RADIUS, DUST_EFFECTIVE_VARIANCE).unwrap();
    let profile = ModeProfile::new(deep, shallow, 0.0).unwrap();
    let rule = |tau: f64, coverage: f64| {
        AerosolMode::of_rule(
            dust,
            CondensedPhase::Solid,
            ShapeClass::NonSphericalMineral,
            dust,
            size,
            tau,
            profile,
            coverage,
            Kelvin::new(210.0),
        )
    };
    assert_eq!(
        rule(-1.0, 1.0),
        Err(BuildModeError::OpticalDepth {
            optical_depth: -1.0
        })
    );
    assert_eq!(
        rule(0.5, 1.5),
        Err(BuildModeError::Coverage { coverage: 1.5 })
    );
    let tholin = row("tholin");
    let haze = Aggregate::new(TITAN_HAZE_MONOMER_RADIUS, 3_000.0, 2.0).unwrap();
    assert_eq!(
        AerosolMode::aggregate(tholin, tholin, haze, 1.0, profile, 1.5),
        Err(BuildModeError::Coverage { coverage: 1.5 })
    );
    assert!(matches!(
        AerosolMode::aggregate(tholin, tholin, haze, f64::NAN, profile, 1.0),
        Err(BuildModeError::OpticalDepth { .. })
    ));
    // A deck in a column of no finite gravity has no finite f_sed.
    let column = MixingColumn {
        gravity: MetresPerSecondSquared::new(9.81),
        molar_mass_g_per_mol: air.mu,
        eddy_diffusion: CLOUD_EDDY_DIFFUSION,
        metallicity: 1.0,
        top: Pascals::new(1e2),
        bottom: Pascals::new(1e5),
    };
    let clouds = Condensation::derive(
        air.gases.iter().copied(),
        |p| air.structure.temperature_at(p),
        &column,
        |_| Sedimentation::BASELINE,
        1.0,
    )
    .unwrap();
    let water = &clouds.decks()[0];
    let mut broken = column;
    broken.gravity = MetresPerSecondSquared::new(f64::NAN);
    assert!(matches!(
        sedimentation_for_supply(water, &broken),
        Err(BuildSedimentationError::Efficiency { .. })
    ));
}

/// A liquid mode above [`NEAR_CRITICAL_LIQUID_SHARE`] of its condensate's critical temperature
/// has its size and τ(550) not modelled for want of its grain density, its column kept, so that
/// the inventory publishes no liquid the grain-density mirror cannot evaluate
/// (decision-p14-t54a-grain-density F4): liquid nitrogen at its critical 126.19 K, against the same
/// droplets at 100 K, which keep both.
#[test]
fn a_near_critical_liquid_mode_s_size_is_not_modelled() {
    let nitrogen = row("N2");
    let critical = nitrogen
        .substance()
        .phase_diagram()
        .expect("nitrogen's phase diagram")
        .critical_temperature()
        .value();
    let size = SizeDistribution::log_normal(Metres::new(1.0e-5), 0.2).unwrap();
    let profile = ModeProfile::new(Pascals::new(4.0e6), Pascals::new(1.0e6), 1.0).unwrap();
    let mass = KilogramsPerSquareMetre::new(0.1);
    let droplets = |t: f64| {
        let mode = Mode::new(
            nitrogen,
            CondensedPhase::Liquid,
            nitrogen,
            size,
            mass,
            profile,
            1.0,
            Kelvin::new(t),
        )
        .unwrap();
        AerosolMode::of_settling(&mode, Kelvin::new(t)).unwrap()
    };
    let cold = droplets(100.0);
    assert!(
        cold.size().is_ok() && cold.optical_depth().is_ok(),
        "{cold:?}"
    );
    let near = droplets(critical);
    assert!(
        critical > 100.0 / NEAR_CRITICAL_LIQUID_SHARE,
        "the cold droplets lie below the cut"
    );
    assert!(
        matches!(
            near.size(),
            Err(SizeModeError::NoDensity {
                phase: CondensedPhase::Liquid,
                ..
            })
        ),
        "{near:?}"
    );
    assert!(
        matches!(
            near.optical_depth(),
            Err(ResolveOpticalDepthError::Size(
                SizeModeError::NoDensity { .. }
            ))
        ),
        "{near:?}"
    );
    assert_eq!(near.column_mass(), Some(mass));
    assert_eq!(near.temperature(), Some(Kelvin::new(critical)));
}

/// A mode from the loop is converted from its settling radius to its volume-to-area radius by its
/// shape class, r = Ψ `r_V`, its variance kept and its τ(550) taken at r: Mars's dust's mineral
/// grains at 0.71, a liquid's droplets at 1.
#[test]
fn a_settling_mode_converts_to_its_volume_to_area_radius() {
    let dust = row("mars_dust");
    let size = SizeDistribution::log_normal(Metres::new(2.0e-6), 0.2).unwrap();
    let profile = ModeProfile::new(Pascals::new(600.0), Pascals::new(100.0), 0.0).unwrap();
    let mass = KilogramsPerSquareMetre::new(1e-3);
    let settling = Mode::new(
        dust,
        CondensedPhase::Solid,
        dust,
        size,
        mass,
        profile,
        1.0,
        Kelvin::new(210.0),
    )
    .unwrap();
    let mode = AerosolMode::of_settling(&settling, Kelvin::new(210.0)).unwrap();
    assert_eq!(mode.shape(), ShapeClass::NonSphericalMineral);
    let Ok(ParticleSize::Distribution(converted)) = mode.size() else {
        panic!("a distribution");
    };
    assert!((converted.effective_radius().value() - 0.71 * 2.0e-6).abs() < 1e-18);
    assert_same_bits(converted.effective_variance(), 0.2);
    let direct = Mode::new(
        dust,
        CondensedPhase::Solid,
        dust,
        converted,
        mass,
        profile,
        1.0,
        Kelvin::new(210.0),
    )
    .unwrap();
    assert_same_bits(
        mode.optical_depth().unwrap().value(),
        direct.optical_depth().unwrap().value(),
    );
    let water = row("H2O");
    let droplets = Mode::new(
        water,
        CondensedPhase::Liquid,
        water,
        size,
        mass,
        profile,
        1.0,
        Kelvin::new(280.0),
    )
    .unwrap();
    let kept = AerosolMode::of_settling(&droplets, Kelvin::new(280.0)).unwrap();
    assert_eq!(kept.shape(), ShapeClass::Sphere);
    assert_eq!(kept.size(), Ok(ParticleSize::Distribution(size)));
}

/// Every index file of rendering plan R08's (`materials/*.json`) whose key is a registry row has
/// the shape class [`ShapeClass::of`] gives its row in its phase.
#[test]
fn the_shape_classes_are_r08_s() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/hyperion/src/renderer/src/view/atmosphere/materials");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("R08's materials directory")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() > 40, "{} files", files.len());
    let field = |text: &str, name: &str| -> String {
        let at = text
            .find(&format!("\"{name}\": \""))
            .unwrap_or_else(|| panic!("a {name}"));
        let rest = &text[at + name.len() + 5..];
        rest[..rest.find('"').expect("a closing quote")].to_owned()
    };
    let mut checked = 0;
    for path in &files {
        let text = std::fs::read_to_string(path).expect("a readable file");
        let key = field(&text, "key");
        let Some(id) = by_key(&key) else {
            continue;
        };
        let phase = match field(&text, "phase").as_str() {
            "solid" => CondensedPhase::Solid,
            "liquid" => CondensedPhase::Liquid,
            other => panic!("{}: phase {other}", path.display()),
        };
        let shape = field(&text, "shape");
        assert_eq!(
            ShapeClass::of(id, phase).name(),
            shape,
            "{}",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 40, "{checked} files checked");
    assert_same_bits(ShapeClass::NonSphericalMineral.sphericity(), 0.71);
    assert_same_bits(ShapeClass::Sphere.sphericity(), 1.0);
}

/// An aggregate's inputs are checked, its dimension held to 2.5, and its prefactor is (5 ÷ 3)^(D ÷
/// 2).
#[test]
fn an_aggregate_is_checked() {
    let titan = Aggregate::new(Metres::new(0.05e-6), 3_000.0, 2.0).unwrap();
    assert!((titan.prefactor() - 5.0 / 3.0).abs() < 1e-12);
    assert!(matches!(
        Aggregate::new(Metres::new(0.05e-6), 3_000.0, 2.6),
        Err(BuildAggregateError::FractalDimension { .. })
    ));
    assert!(matches!(
        Aggregate::new(Metres::new(0.0), 3_000.0, 2.0),
        Err(BuildAggregateError::MonomerRadius { .. })
    ));
    assert!(matches!(
        Aggregate::new(Metres::new(0.05e-6), 0.5, 2.0),
        Err(BuildAggregateError::Monomers { .. })
    ));
}

/// The inventory is the same bits for the same inputs, and continuous in them: a millikelvin on
/// the ground moves Earth's water deck's τ by under 10⁻³ of itself.
#[test]
fn the_inventory_is_deterministic_and_continuous() {
    let make = |t_s: f64| {
        HandAir::new(
            &[("N2", 0.9869), ("H2O", 0.0131)],
            Some("H2O"),
            t_s,
            1e5,
            214.0,
        )
        .inventory(SurfaceState::Temperate)
    };
    let a = make(288.0);
    assert_eq!(format!("{a:?}"), format!("{:?}", make(288.0)));
    let water = row("H2O");
    let (one, two) = (
        a.deck_optical_depth(water).unwrap(),
        make(288.001).deck_optical_depth(water).unwrap(),
    );
    assert!((two / one - 1.0).abs() < 1e-3, "{one} against {two}");
}

/// The files outside this module and P14.T24.g's whose code names the inventory behind a named
/// hold, each with the name of its hold, which its code (outside an inline test module) must name:
/// P14.T54.b.b's wiring of the record's inventory, and the hold's own tests (decision-p14-t54b-open
/// §3.3). **P14.T54.b.l deletes this list** with the tripwire below.
const HELD: [(&str, &str); 2] = [
    ("planetary/system/air.rs", "AIR_SECTIONS_VERSION"),
    ("planetary/system/air/tests.rs", "AIR_SECTIONS_VERSION"),
];

/// Whether the code of `text` (its lines before an inline `mod tests {`, comments left out) names
/// `hold`.
fn names_its_hold(text: &str, hold: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .take_while(|code| !code.starts_with("mod tests {"))
        .filter(|code| !code.starts_with("//"))
        .any(|code| code.contains(hold))
}

/// The hold check reads the code alone: a comment naming the hold, or code inside an inline test
/// module, does not count.
#[test]
fn the_held_files_hold_check_reads_the_code_alone() {
    let hold = "AIR_SECTIONS_VERSION";
    assert!(names_its_hold("if v >= AIR_SECTIONS_VERSION {\n", hold));
    assert!(!names_its_hold(
        "// AIR_SECTIONS_VERSION\nfn f() {}\n",
        hold
    ));
    assert!(!names_its_hold(
        "fn f() {}\n#[cfg(test)]\nmod tests {\n    const A: u32 = AIR_SECTIONS_VERSION;\n}\n",
        hold
    ));
}

/// Nothing on the record reaches the inventory before P14.T54.b's bump (decision-bump-plan-after-22
/// §1.3): no code of the crate's `src/` outside this module, P14.T24.g's, their tests and
/// [`HELD`]'s files names it, comments and the modules' own declarations (with the inventory's
/// re-export from `hooks`) left out; each of [`HELD`]'s files is a source file whose code names its
/// hold, so that a caller in any other file fails, and so does a listed file that drops its hold
/// (decision-p14-t54b-open §3.3). Under the hold's `Held` arm the record calls nothing here
/// (`the_air_sections_are_held_only_until_the_24_bump`). **P14.T54.b.l deletes this tripwire.**
#[test]
fn nothing_on_the_record_calls_the_inventory_yet() {
    fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("a source directory")
            .map(|e| e.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, files);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    assert!(files.len() > 100, "the walk found the crate's sources");
    let own = [
        "planetary/hooks/aerosols",
        "planetary/hooks/sulphur",
        "planetary/hooks/envelope_atmosphere",
    ];
    // The record's `AerosolInventory` is this module's, re-exported from `hooks`: a file that names
    // the type through it stores the member, and the calls and the inputs it would build one with
    // are caught by name.
    let patterns = [
        "hooks::aerosols",
        "aerosols::",
        "hooks::sulphur",
        "sulphur::",
        "AerosolInventory::derive",
        "AerosolInputs",
    ];
    // The atmosphere re-exports the acid's cycling temperature for P14.T53.b part 1 (b)'s deckless
    // re-solve and its docs (`RunawayDeck`): a constant, which calls nothing of the inventory.
    let declarations = [
        "pub mod aerosols;",
        "pub mod sulphur;",
        "pub use crate::planetary::hooks::sulphur::ACID_CYCLING_TEMPERATURE;",
        "pub use aerosols::AerosolInventory;",
    ];
    let relative_of = |path: &std::path::Path| {
        path.strip_prefix(&src)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/")
    };
    for (held, hold) in HELD {
        let path = files
            .iter()
            .find(|path| relative_of(path.as_path()) == held)
            .unwrap_or_else(|| panic!("{held} is a source file"));
        let text = std::fs::read_to_string(path).expect("a readable source");
        assert!(
            names_its_hold(&text, hold),
            "{held} reads the inventory behind {hold}, which its code no longer names"
        );
    }
    let mut found = Vec::new();
    for path in &files {
        let relative = relative_of(path);
        if own.iter().any(|o| relative.starts_with(o))
            || HELD.iter().any(|(held, _)| *held == relative)
        {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("a readable source");
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || declarations.contains(&code) {
                continue;
            }
            if patterns.iter().any(|p| code.contains(p)) {
                found.push(format!("{relative}:{}: {code}", n + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "nothing on the record calls the inventory before P14.T54.b: {found:#?}"
    );
}

/// A measured deck's column, set by hand: its body, its deck's condensate, its gas below the
/// decks, its temperature profile, its gravity, the column's top and bottom (bar), and the range of
/// its observed τ(550) with its log-centre (decision-bump-plan-after-22, "The decks' `f_sed`, K and
/// humidity", items 3–4).
struct Anchor {
    name: &'static str,
    deck: &'static str,
    gas: Vec<(SubstanceId, f64)>,
    profile: AnchorProfile,
    gravity: f64,
    top_bar: f64,
    bottom_bar: f64,
    range: (f64, f64),
    centre: f64,
}

/// An anchor's temperature profile.
enum AnchorProfile {
    /// A giant's: `t1` kelvin at 1 bar on a power law of exponent `k`, held at its tropopause's
    /// `trop` kelvin above.
    Power { t1: f64, k: f64, trop: f64 },
    /// A surface's air, P14.T24.e's structure.
    Air(RadiativeConvective),
}

impl Anchor {
    fn temperature(&self, p: Pascals) -> Kelvin {
        match &self.profile {
            AnchorProfile::Power { t1, k, trop } => {
                Kelvin::new((t1 * math::powf(p.value() / PASCALS_PER_BAR, *k)).max(*trop))
            }
            AnchorProfile::Air(air) => air.temperature_at(p),
        }
    }

    fn molar_mass(&self) -> f64 {
        let (mut sum, mut total) = (0.0, 0.0);
        for &(id, x) in &self.gas {
            sum += x * id.substance().molar_mass_g_per_mol().expect("a molar mass");
            total += x;
        }
        sum / total
    }

    fn column(&self) -> MixingColumn {
        MixingColumn {
            gravity: MetresPerSecondSquared::new(self.gravity),
            molar_mass_g_per_mol: self.molar_mass(),
            eddy_diffusion: CLOUD_EDDY_DIFFUSION,
            metallicity: 1.0,
            top: Pascals::new(self.top_bar * PASCALS_PER_BAR),
            bottom: Pascals::new(self.bottom_bar * PASCALS_PER_BAR),
        }
    }
}

/// A giant's gas: H₂ and He at `he_per_h2` helium per H₂, with `traces` as mole fractions.
fn giant_gas(he_per_h2: f64, traces: &[(&str, f64)]) -> Vec<(SubstanceId, f64)> {
    let rest = 1.0 - traces.iter().map(|&(_, x)| x).sum::<f64>();
    let h2 = rest / (1.0 + he_per_h2);
    let mut gas = vec![(row("H2"), h2), (row("He"), h2 * he_per_h2)];
    gas.extend(traces.iter().map(|&(k, x)| (row(k), x)));
    gas
}

/// A surface's air by hand: `dry` fractions of its dry air, with water or methane `wet` added at
/// mole fraction `x_wet`, at `t_s` kelvin and `p_s` pascals under a skin of `skin` kelvin.
fn surface_air(
    dry: &[(&str, f64)],
    wet: (&str, f64),
    t_s: f64,
    p_s: f64,
    skin: f64,
) -> (Vec<(SubstanceId, f64)>, RadiativeConvective) {
    let total: f64 = dry.iter().map(|&(_, x)| x).sum();
    let (key, x_wet) = wet;
    let mut gas: Vec<(SubstanceId, f64)> = dry
        .iter()
        .map(|&(k, x)| (row(k), x / total * (1.0 - x_wet)))
        .collect();
    gas.push((row(key), x_wet));
    let air = RadiativeConvective::derive(&StructureInputs {
        surface_temperature: Kelvin::new(t_s),
        surface_pressure: Pascals::new(p_s),
        gases: &gas,
        condensable: Some(row(key)),
        skin_temperature: Kelvin::new(skin),
    })
    .expect("an air");
    (gas, air)
}

/// The six measured decks the sedimentation rule is fitted on, and Titan's, a check (the ruling's
/// items 3–4; each figure's source beside it, [M] from memory):
///
/// - Mars: 636 Pa and 210 K (NASA's Mars fact sheet [M]), MSL's 96.0% CO₂, 1.93% Ar, 1.89% N₂ and
///   0.145% O₂ (Mahaffy et al. 2013, Science 341, 263 [M]), and 12.5 pr-µm of water, the middle of
///   the "globally-averaged water vapour columns … around 10–15 pr µm" (Steele et al. 2014, Icarus
///   237, 97), under the skin of its 209.8 K black-body temperature [M]; observed 0.2–2 (its thick
///   Tharsis hazes about 1–2 in the cloud, Madeleine et al. 2012, JGR 117).
/// - Earth: 101,325 Pa and 288 K, dry air of 78.08% N₂, 20.95% O₂, 0.93% Ar and 0.04% CO₂ [M], and
///   water at P14.T24.a's 0.77 of its saturation on T49.b's curve, under the skin of a 255 K
///   equilibrium temperature with 0.09 W m⁻² of heat flow; observed 2.5–10 (Rossow and Schiffer
///   1999, BAMS 80, 2261: a mean cloud optical thickness of 3.7 ± 0.3).
/// - Jupiter: the Galileo probe's adiabat, 166.1 K at 1 bar and 427.7 K at 22 bar (Seiff et al.
///   1998 [M]), held at 110 K; He/H₂ 0.157 (von Zahn et al. 1998 [M]), NH₃ 362 ppm (Li et al. 2017,
///   GRL 44, 5317), H₂S 7.7 × 10⁻⁵ (Wong et al. 2004's H₂S/H₂ 8.9 × 10⁻⁵ [M]), H₂O 2.5 × 10⁻³ (Li et
///   al. 2020's 2.7 times protosolar), CH₄ 2.04 × 10⁻³ (Wong et al. 2004 [M]); observed 0.8–5 (its
///   NH₃-ice cloud near 0.55 bar, τ 0.83–2.15, Irwin et al. 2026, arXiv:2607.25542, Table 1).
/// - Saturn: 134.8 K at 1 bar (Lindal et al. 1985 [M]) on Jupiter's exponent, held at 82 K; He/H₂
///   0.135 [M], NH₃ 3.6 × 10⁻⁴ (Laraia et al. 2013, Icarus 226, 641), H₂S and H₂O at three times
///   protosolar like its ammonia (a stand-in), CH₄ 4.7 × 10⁻³ (Fletcher et al. 2009 [M]); observed
///   0.5–5 (its NH₃ deck's 1–2, Sromovsky et al. 2020 [Q, the ruling]; its haze's 9–12 is P14.T52.d's).
/// - Uranus: 76 K at 1 bar (Sromovsky et al. 2011's F1) and 148 K at 6 bar (Irwin et al. 2022), held at
///   53 K; CH₄ 3% (Sromovsky et al. 2019, Icarus 317, 266: deep 2.7–3.5%), H₂S 6.4 × 10⁻⁴ (Irwin et
///   al. 2022), NH₃ 10⁻⁵, H₂ and He 0.85 : 0.15; observed 1–5 (its 1–2 bar layer, τ(0.8 µm) 2.0–3.5,
///   Irwin et al. 2022, JGR Planets 127, e2022JE007189, Table 2).
/// - Neptune: 71.5 K at 1 bar (Lindal 1992 [M]) on Uranus's exponent, held at 52 K [M]; CH₄ 3.5%
///   (Irwin et al. 2022 expect "∼3–4%"), the rest as Uranus's; observed 0.5–4 (τ(0.8 µm) 1–2,
///   about half Uranus's, Irwin et al. 2022).
/// - Titan, a check: 93.7 K and 1.467 bar with 5.65% CH₄ (Niemann et al. 2010, JGR 115, E12006), the
///   rest N₂, under its 70.4 K tropopause (Fulchignoni et al. 2005 [M]); in the cloud 0.3–30.
#[expect(
    clippy::too_many_lines,
    reason = "seven measured states, each given by hand with its figures"
)]
fn anchors() -> Vec<Anchor> {
    use crate::planetary::hooks::structure::{
        effective_temperature, internal_temperature, skin_temperature,
    };
    use crate::substance::diagram::saturation_pressure;
    use crate::units::WattsPerSquareMetre;

    let skin = |t_eq: f64, flow: f64| {
        skin_temperature(effective_temperature(
            Kelvin::new(t_eq),
            internal_temperature(WattsPerSquareMetre::new(flow)),
        ))
        .value()
    };
    let water = row("H2O");
    let mars_dry = [
        ("CO2", 0.960),
        ("Ar", 0.0193),
        ("N2", 0.0189),
        ("O2", 0.00145),
    ];
    let mars_mu = 43.34;
    let water_mu = water.substance().molar_mass_g_per_mol().unwrap();
    let mars_x = 12.5e-3 * mars_mu * 3.71 / (water_mu * 636.0);
    let (mars_gas, mars_air) =
        surface_air(&mars_dry, ("H2O", mars_x), 210.0, 636.0, skin(209.8, 0.0));
    let earth_x = 0.77
        * saturation_pressure(water, Kelvin::new(288.0))
            .unwrap()
            .value()
        / 101_325.0;
    let (earth_gas, earth_air) = surface_air(
        &[
            ("N2", 0.7808),
            ("O2", 0.2095),
            ("Ar", 0.0093),
            ("CO2", 0.0004),
        ],
        ("H2O", earth_x),
        288.0,
        101_325.0,
        skin(255.0, 0.09),
    );
    let (titan_gas, titan_air) = surface_air(&[("N2", 1.0)], ("CH4", 0.0565), 93.7, 1.467e5, 70.4);
    let jupiter_k = math::ln(427.7 / 166.1) / math::ln(22.0);
    let uranus_k = math::ln(148.0 / 76.0) / math::ln(6.0);
    let three_solar =
        |e: crate::substance::Element| 3.0 * 2.0 * e.protosolar_abundance_per_hydrogen();
    let saturn_sulphide = three_solar(crate::substance::Element::S) / 1.135;
    let saturn_water = three_solar(crate::substance::Element::O) / 1.135;
    let ice_giant = |ch4: f64| {
        let (h2s, nh3) = (6.4e-4, 1.0e-5);
        let rest = 1.0 - ch4 - h2s - nh3;
        vec![
            (row("H2"), 0.85 * rest),
            (row("He"), 0.15 * rest),
            (row("CH4"), ch4),
            (row("H2S"), h2s),
            (row("NH3"), nh3),
        ]
    };
    vec![
        Anchor {
            name: "Mars",
            deck: "H2O",
            gas: mars_gas,
            profile: AnchorProfile::Air(mars_air),
            gravity: 3.71,
            top_bar: 636.0e-7,
            bottom_bar: 636.0e-5,
            range: (0.2, 2.0),
            centre: 0.6,
        },
        Anchor {
            name: "Earth",
            deck: "H2O",
            gas: earth_gas,
            profile: AnchorProfile::Air(earth_air),
            gravity: 9.806_65,
            top_bar: 1e-3,
            bottom_bar: 1.013_25,
            range: (2.5, 10.0),
            centre: 3.7,
        },
        Anchor {
            name: "Jupiter",
            deck: "NH3",
            gas: giant_gas(
                0.157,
                &[
                    ("NH3", 3.62e-4),
                    ("H2S", 7.7e-5),
                    ("H2O", 2.5e-3),
                    ("CH4", 2.04e-3),
                ],
            ),
            profile: AnchorProfile::Power {
                t1: 166.1,
                k: jupiter_k,
                trop: 110.0,
            },
            gravity: 24.79,
            top_bar: 0.01,
            bottom_bar: 100.0,
            range: (0.8, 5.0),
            centre: 1.5,
        },
        Anchor {
            name: "Saturn",
            deck: "NH3",
            gas: giant_gas(
                0.135,
                &[
                    ("NH3", 3.6e-4),
                    ("H2S", saturn_sulphide),
                    ("H2O", saturn_water),
                    ("CH4", 4.7e-3),
                ],
            ),
            profile: AnchorProfile::Power {
                t1: 134.8,
                k: jupiter_k,
                trop: 82.0,
            },
            gravity: 10.44,
            top_bar: 0.01,
            bottom_bar: 100.0,
            range: (0.5, 5.0),
            centre: 1.5,
        },
        Anchor {
            name: "Uranus",
            deck: "CH4",
            gas: ice_giant(0.03),
            profile: AnchorProfile::Power {
                t1: 76.0,
                k: uranus_k,
                trop: 53.0,
            },
            gravity: 8.87,
            top_bar: 0.05,
            bottom_bar: 50.0,
            range: (1.0, 5.0),
            centre: 2.5,
        },
        Anchor {
            name: "Neptune",
            deck: "CH4",
            gas: ice_giant(0.035),
            profile: AnchorProfile::Power {
                t1: 71.5,
                k: uranus_k,
                trop: 52.0,
            },
            gravity: 11.15,
            top_bar: 0.05,
            bottom_bar: 50.0,
            range: (0.5, 4.0),
            centre: 1.5,
        },
        Anchor {
            name: "Titan",
            deck: "CH4",
            gas: titan_gas,
            profile: AnchorProfile::Air(titan_air),
            gravity: 1.352,
            top_bar: 1e-3,
            bottom_bar: 1.467,
            range: (0.3, 30.0),
            centre: 3.0,
        },
    ]
}

/// The measured decks take their observed τ(550) under the supply rule, each column's decks
/// derived as the inventory derives a surface's (two passes, every deck its own supply's `f_sed`, its
/// modes converted by shape; the fit, decision-bump-plan-after-22, "The decks' `f_sed`, K and
/// humidity", G2–G3): Mars, Earth, Jupiter, Saturn and Uranus inside their ranges, Neptune's
/// methane below R08's cloud-deck split of 10 and within 3 times its range's top (G3's record for
/// P14.T52.d), and Titan's, a check, inside 0.3–30. As built: Mars 1.01, Earth 2.61, Jupiter 2.59,
/// Saturn 0.87, Uranus 4.15, Neptune 5.82, Titan 1.39.
#[test]
fn the_measured_decks_take_their_observed_optical_depths() {
    for anchor in anchors() {
        let column = anchor.column();
        let profile = |p: Pascals| anchor.temperature(p);
        let first = Condensation::derive(
            anchor.gas.iter().copied(),
            profile,
            &column,
            |_| Sedimentation::BASELINE,
            1.0,
        )
        .expect("the anchor's decks");
        let rules: Vec<(SubstanceId, Sedimentation)> = first
            .decks()
            .iter()
            .map(|d| {
                (
                    d.condensate(),
                    sedimentation_for_supply(d, &column).expect("a rule"),
                )
            })
            .collect();
        let of = |id: SubstanceId| {
            rules
                .iter()
                .find(|(d, _)| *d == id)
                .map_or(Sedimentation::BASELINE, |&(_, s)| s)
        };
        let clouds = Condensation::derive(anchor.gas.iter().copied(), profile, &column, of, 1.0)
            .expect("the anchor's decks");
        let deck = clouds
            .decks()
            .iter()
            .find(|d| d.condensate() == row(anchor.deck))
            .unwrap_or_else(|| panic!("{}'s {} deck", anchor.name, anchor.deck));
        let mode = AerosolMode::of_settling(&deck.modes()[0], deck.base_temperature()).unwrap();
        let tau = mode.optical_depth().expect("a τ").value();
        let (low, high) = anchor.range;
        eprintln!(
            "{} {}: base {:.4} bar, f_sed {:.4e}, tau {tau:.4} against {low}-{high} (centre {}), \
             residual {:+.3} dex",
            anchor.name,
            anchor.deck,
            deck.base().value() / PASCALS_PER_BAR,
            deck.sedimentation(),
            anchor.centre,
            math::log10(tau / anchor.centre)
        );
        match anchor.name {
            "Neptune" => assert!(
                tau < 10.0 && tau < 3.0 * high,
                "{}'s {} at τ {tau}",
                anchor.name,
                anchor.deck
            ),
            _ => assert!(
                (low..=high).contains(&tau),
                "{}'s {} at τ {tau}",
                anchor.name,
                anchor.deck
            ),
        }
    }
}

/// Earth's dry air at a stated O₂ level `pal` (by its column), 288 K and 1 atm under a solar
/// twin, its O₂ taken from its N₂.
fn oxygenated(pal: f64) -> HandAir {
    let x = 0.209_476 * pal;
    let mut air = HandAir::new(
        &[("N2", 0.990_66 - x), ("O2", x), ("Ar", 0.009_34)],
        None,
        288.0,
        101_325.0,
        214.0,
    );
    // The U.S. Standard Atmosphere's molar mass and gravity, at which 1 PAL is Earth's column.
    air.mu = 28.964_4;
    air.g = 9.806_65;
    air
}

/// The ozone of `air`, its column and its profile's nodes.
fn ozone_of(air: &HandAir) -> Option<(f64, Vec<(f64, f64)>)> {
    let inventory = air.inventory(SurfaceState::Temperate);
    inventory
        .absorbers()
        .iter()
        .find(|a| a.species() == row("O3"))
        .map(|a| {
            let AbsorberLayer::Profile(nodes) = a.layer() else {
                panic!("ozone's layer is a profile");
            };
            (
                a.column_per_m2(),
                nodes
                    .iter()
                    .map(|n| (n.pressure().value(), n.relative_mixing_ratio()))
                    .collect(),
            )
        })
}

/// A world with no O₂ has no ozone, and the column rises monotonically with the O₂ level to
/// Earth's 300 DU at 1 PAL about a solar twin, finite and monotonic up to 10⁵ PAL (beyond the
/// generator's largest O₂, P14.T24.f's runaways' thousands of bar).
#[test]
fn the_ozone_column_rises_with_oxygen_to_earth_s() {
    assert!(ozone_of(&oxygenated(0.0)).is_none());
    let mut previous = 0.0;
    for pal in [1e-6_f64, 1e-4, 1e-2, 0.1, 0.5, 1.0, 2.0, 10.0, 1e3, 1e5] {
        let (column, _) = ozone_of(&oxygenated(pal.min(4.7))).unwrap_or((0.0, Vec::new()));
        let factor = ozone_oxygen_factor(pal);
        assert!(
            factor.is_finite() && factor >= previous,
            "F({pal}) = {factor}"
        );
        previous = factor;
        if pal <= 4.7 {
            assert!(column.is_finite(), "{pal} PAL");
        }
    }
    let (column, _) = ozone_of(&oxygenated(1.0)).unwrap();
    assert!(
        (column / EARTH_OZONE_COLUMN - 1.0).abs() < 1e-3,
        "{} DU at 1 PAL",
        column / DOBSON_UNIT
    );
    assert_same_bits(ozone_oxygen_factor(0.0), 0.0);
    assert_same_bits(ozone_star_factor(Kelvin::new(5_772.0)), 1.0);
}

/// Ozone's profile at 1 PAL on Earth's column is Earth's template to 10⁻⁹ in pressure, and every
/// profile's nodes are finite, descending in pressure and within the column.
#[test]
fn the_ozone_profile_is_earth_s_on_earth_s_column() {
    let (_, nodes) = ozone_of(&oxygenated(1.0)).unwrap();
    assert_eq!(nodes.len(), AFGL_US_STANDARD_OZONE.len());
    for ((p, x), &(mb, ppmv)) in nodes.iter().zip(&AFGL_US_STANDARD_OZONE) {
        assert!(
            (p / (mb * 100.0) - 1.0).abs() < 1e-9,
            "{p} Pa against {mb} mb"
        );
        assert!((x / ppmv - 1.0).abs() < 1e-9, "{x} against {ppmv}");
    }
    for pal in [1e-4, 1e-2, 0.3, 3.0] {
        let (_, nodes) = ozone_of(&oxygenated(pal)).unwrap();
        assert!(nodes.len() >= 2);
        assert!(nodes[0].0 <= 101_325.0 * (1.0 + 1e-12), "{pal}: {nodes:?}");
        assert!(nodes.windows(2).all(|w| w[1].0 < w[0].0));
        assert!(
            nodes
                .iter()
                .all(|&(p, x)| p.is_finite() && p > 0.0 && x > 0.0)
        );
    }
}

/// The golden's lines of `inventory`, under the name `name`.
fn pin(w: &mut hyperion_testkit::golden::GoldenWriter, name: &str, inventory: &AerosolInventory) {
    for d in inventory.decks() {
        let key = d.condensate().substance().key();
        let key = key.as_str();
        w.line(&format!("{name} deck {key}: {:?}", d.phase()));
        w.f64(&format!("{name} deck {key} base"), d.base().value());
        w.f64(
            &format!("{name} deck {key} base temperature"),
            d.base_temperature().value(),
        );
        w.f64(&format!("{name} deck {key} top"), d.top().value());
        w.f64(&format!("{name} deck {key} vapour below"), d.vapour_below());
        w.f64(&format!("{name} deck {key} vapour above"), d.vapour_above());
        w.f64(&format!("{name} deck {key} f_sed"), d.sedimentation());
    }
    for (i, m) in inventory.modes().iter().enumerate() {
        let key = m.material().substance().key();
        let tag = format!("{name} mode {i} {}", key.as_str());
        w.line(&format!(
            "{tag}: {:?} {} of deck {}",
            m.phase(),
            m.shape().name(),
            m.deck().substance().key().as_str()
        ));
        match m.size() {
            Ok(ParticleSize::Distribution(size)) => {
                w.line(&format!("{tag} size: {}", size.kind().name()));
                w.f64(&format!("{tag} r_eff"), size.effective_radius().value());
                w.f64(&format!("{tag} v_eff"), size.effective_variance());
            }
            Ok(ParticleSize::Aggregate(a)) => {
                w.f64(&format!("{tag} monomer radius"), a.monomer_radius().value());
                w.f64(&format!("{tag} monomers"), a.monomers());
                w.f64(&format!("{tag} fractal dimension"), a.fractal_dimension());
            }
            Err(error) => w.line(&format!("{tag} size: {error}")),
        }
        match m.column_mass() {
            Some(mass) => w.f64(&format!("{tag} column mass"), mass.value()),
            None => w.line(&format!("{tag} column mass: not modelled")),
        }
        match m.optical_depth() {
            Ok(depth) => {
                w.line(&format!("{tag} tau basis: {:?}", depth.basis()));
                w.f64(&format!("{tag} tau"), depth.value());
            }
            Err(error) => w.line(&format!("{tag} tau: {error}")),
        }
        w.f64(&format!("{tag} base"), m.profile().base().value());
        w.f64(&format!("{tag} top"), m.profile().top().value());
        w.f64(&format!("{tag} exponent"), m.profile().exponent());
        w.f64(&format!("{tag} coverage"), m.coverage());
    }
    for a in inventory.absorbers() {
        let key = a.species().substance().key();
        let tag = format!("{name} absorber {}", key.as_str());
        w.f64(&format!("{tag} column"), a.column_per_m2());
        match a.layer() {
            AbsorberLayer::Slab { base, top } => {
                w.f64(&format!("{tag} base"), base.value());
                w.f64(&format!("{tag} top"), top.value());
            }
            AbsorberLayer::Profile(nodes) => {
                for (k, node) in nodes.iter().enumerate() {
                    w.f64(&format!("{tag} node {k} pressure"), node.pressure().value());
                    w.f64(
                        &format!("{tag} node {k} mixing ratio"),
                        node.relative_mixing_ratio(),
                    );
                }
            }
        }
    }
    for (id, why) in inventory.not_formed() {
        w.line(&format!(
            "{name} not formed: {} {why:?}",
            id.substance().key().as_str()
        ));
    }
}

/// The inventories of the Solar System table's Earth, Venus, Mars and Titan (at their own
/// outgassing, the whole sky covered) and of an Earth at 1 PAL of O₂, pinned: each deck's base,
/// top, vapours and `f_sed`, each mode's material, phase, shape, size, column, τ(550), profile and
/// coverage, and each absorber's column and layer. Floats are their bits with Rust's shortest
/// round-trip decimal beside.
#[test]
fn the_inventory_is_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.line("# P14.T24.c's aerosol and absorber inventory, with P14.T24.g's sulphur");
    w.line("# (hyperion_sim::planetary::hooks::aerosols): the Solar System table's Earth, Venus,");
    w.line(
        "# Mars and Titan at their own outgassing under a whole sky's coverage, and an Earth at",
    );
    w.line("# 1 PAL of O2. Pure until P14.T54.b; nothing on the record reads it. The measured");
    w.line("# Mars, by hand, carries its dust.");
    for (name, inventory) in golden_cases() {
        pin(&mut w, name, &inventory);
    }
    golden!("planetary/aerosols", w.as_str());
}

/// The golden's inventories, by name: the table's Earth, Venus, Mars and Titan at their own
/// outgassing under a whole sky, an Earth at 1 PAL of O₂, and the measured Mars's dust.
fn golden_cases() -> Vec<(&'static str, AerosolInventory)> {
    let mut cases = Vec::new();
    for (name, key) in [
        ("earth", "Earth"),
        ("venus", "Venus"),
        ("mars", "Mars"),
        ("titan", "Titan"),
    ] {
        let (body, conditions, structure) = table_air(key);
        let inputs = inputs_of(&body, &conditions, &structure, 1.0);
        cases.push((name, AerosolInventory::derive(&inputs).unwrap()));
    }
    cases.push((
        "oxygenated earth",
        oxygenated(1.0).inventory(SurfaceState::Temperate),
    ));
    cases.push(("measured mars", measured_mars_inventory()));
    cases
}

/// The measured Mars's inventory (its anchor's air, a dry ground, no outgassing).
fn measured_mars_inventory() -> AerosolInventory {
    let mars = anchors()
        .into_iter()
        .find(|a| a.name == "Mars")
        .expect("Mars's anchor");
    let AnchorProfile::Air(air) = &mars.profile else {
        panic!("Mars's air");
    };
    AerosolInventory::derive(&AerosolInputs {
        state: SurfaceState::Temperate,
        surface_pressure: Pascals::new(636.0),
        gases: &mars.gas,
        mean_molar_mass_g_per_mol: mars.molar_mass(),
        gravity: MetresPerSecondSquared::new(3.71),
        structure: air,
        cloud_fraction: 0.1,
        star_temperature: Some(Kelvin::new(5_772.0)),
        surface_liquid_cover: 0.0,
        outgassing: 0.0,
        liquid_water: false,
        runaway_deck: RunawayDeck::Kept,
    })
    .unwrap()
}

/// The same inputs give the same bits twice, over every rule (the golden's cases: decks, the acid
/// deck and the sulphate layer, the haze, ozone and the dust).
#[test]
fn the_inventory_is_the_same_bits_twice() {
    for ((name, a), (_, b)) in golden_cases().into_iter().zip(golden_cases()) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name}");
    }
}

/// Each deck's base lies on its law's saturation curve along T24.e's profile, to 10⁻⁶ in pressure,
/// in the inventory's own column (two passes, the top two decades above the tropopause): the
/// table's Earth's, Mars's and Titan's decks.
#[test]
fn each_deck_s_base_lies_on_its_saturation_curve() {
    let mut checked = 0;
    for name in ["Earth", "Mars", "Titan"] {
        let inventory = table_inventory(name, 1.0);
        for deck in inventory.decks() {
            let law = law_aloft(deck.condensate()).expect("a law aloft");
            let ln_sat = law
                .ln_saturation_pressure(deck.base_temperature(), deck.base(), 0.0)
                .expect("a saturation at the base");
            let ln_vapour = math::ln(deck.vapour_below() * deck.base().value());
            assert!(
                (ln_vapour - ln_sat).abs() < 1e-6,
                "{name}'s {}: {ln_vapour} against {ln_sat}",
                deck.condensate().substance().key().as_str()
            );
            checked += 1;
        }
    }
    assert!(checked >= 3, "{checked} decks");
}

/// The methane haze forms just above each threshold and not below it: on an N₂–CH₄ world above
/// 10⁻⁵ of CH₄ (Trainer et al. 2006's least), on a CO₂ world above CH₄ ÷ CO₂ 0.2 (Arney et al.
/// 2016's thick haze); its τ is continuous through 0 there and rises with the methane to Titan's.
#[test]
fn the_methane_haze_forms_just_above_each_threshold() {
    assert_same_bits(haze_optical_depth(haze_index(HAZE_METHANE_FLOOR, 0.0)), 0.0);
    let above = haze_optical_depth(haze_index(HAZE_METHANE_FLOOR * (1.0 + 1e-9), 0.0));
    assert!(above > 0.0 && above < 1e-9, "{above}");
    assert_same_bits(
        haze_optical_depth(haze_index(0.9 * HAZE_METHANE_FLOOR, 0.0)),
        0.0,
    );
    let co2 = 0.01;
    let onset = HAZE_METHANE_TO_CARBON_DIOXIDE * co2 + HAZE_METHANE_FLOOR;
    assert_same_bits(haze_optical_depth(haze_index(onset, co2)), 0.0);
    assert!(haze_optical_depth(haze_index(onset * 1.01, co2)) > 0.0);
    assert_same_bits(haze_optical_depth(haze_index(0.15 * co2, co2)), 0.0);
    let mut previous = 0.0;
    for methane in [1e-5, 1e-4, 1e-3, 1e-2, 0.0565, 0.2] {
        let tau = haze_optical_depth(haze_index(methane, 0.0));
        assert!(tau >= previous, "τ {tau} at {methane}");
        previous = tau;
    }
    let titan = haze_optical_depth(haze_index(TITAN_SURFACE_METHANE, 0.0));
    assert!((titan / TITAN_HAZE_OPTICAL_DEPTH - 1.0).abs() < 1e-12);
}

/// The table's Titan carries an aggregate haze of tholin with Titan's monomers (0.05 µm, 3,000, `D_f`
/// 2, at most 2.5), its τ(550) within a factor of two of Tomasko et al. 2008's column (8.1 at 550
/// nm by Bazzon et al. 2014's fit), and a world with no star none.
#[test]
fn the_table_s_titan_carries_titan_s_haze() {
    let (body, conditions, structure) = table_air("Titan");
    let inputs = inputs_of(&body, &conditions, &structure, 1.0);
    let inventory = AerosolInventory::derive(&inputs).unwrap();
    let haze: Vec<&AerosolMode> = inventory
        .modes()
        .iter()
        .filter(|m| m.shape() == ShapeClass::Aggregate)
        .collect();
    assert_eq!(haze.len(), 1);
    let haze = haze[0];
    assert_eq!(haze.material(), row("tholin"));
    assert_eq!(haze.phase(), CondensedPhase::Solid);
    let Ok(ParticleSize::Aggregate(aggregate)) = haze.size() else {
        panic!("an aggregate");
    };
    assert_same_bits(aggregate.monomer_radius().value(), 0.05e-6);
    assert_same_bits(aggregate.monomers(), 3_000.0);
    assert!(aggregate.fractal_dimension() <= MAX_FRACTAL_DIMENSION);
    let tau = haze.optical_depth().unwrap();
    assert_eq!(tau.basis(), DepthBasis::Rule);
    assert!(
        (0.5..2.0).contains(&(tau.value() / TITAN_HAZE_OPTICAL_DEPTH)),
        "τ {}",
        tau.value()
    );
    assert_eq!(haze.column_mass(), None);
    let mut dark = inputs;
    dark.star_temperature = None;
    let dark = AerosolInventory::derive(&dark).unwrap();
    assert!(
        dark.modes()
            .iter()
            .all(|m| m.shape() != ShapeClass::Aggregate)
    );
}

/// A Mars-like world (the measured Mars's air) carries non-spherical dust at Mars's background
/// τ(550), 0.5; an ocean world none, and a Mars-like air over a wet ground none; and the dust's τ is
/// continuous in the pressure through its limits.
#[test]
fn a_mars_like_world_carries_dust_and_an_ocean_world_none() {
    let mars = anchors()
        .into_iter()
        .find(|a| a.name == "Mars")
        .expect("Mars's anchor");
    let AnchorProfile::Air(air) = &mars.profile else {
        panic!("Mars's air");
    };
    let inputs = AerosolInputs {
        state: SurfaceState::Temperate,
        surface_pressure: Pascals::new(636.0),
        gases: &mars.gas,
        mean_molar_mass_g_per_mol: mars.molar_mass(),
        gravity: MetresPerSecondSquared::new(3.71),
        structure: air,
        cloud_fraction: 0.1,
        star_temperature: Some(Kelvin::new(5_772.0)),
        surface_liquid_cover: 0.0,
        outgassing: 0.0,
        liquid_water: false,
        runaway_deck: RunawayDeck::Kept,
    };
    let inventory = AerosolInventory::derive(&inputs).unwrap();
    let dust: Vec<&AerosolMode> = inventory
        .modes()
        .iter()
        .filter(|m| m.material() == row("mars_dust"))
        .collect();
    assert_eq!(dust.len(), 1);
    assert_eq!(dust[0].shape(), ShapeClass::NonSphericalMineral);
    let tau = dust[0].optical_depth().unwrap().value();
    assert!(
        (tau / MARS_DUST_OPTICAL_DEPTH - 1.0).abs() < 0.02,
        "τ {tau}"
    );
    let mut wet = inputs;
    wet.surface_liquid_cover = 1.0;
    assert!(
        AerosolInventory::derive(&wet)
            .unwrap()
            .modes()
            .iter()
            .all(|m| m.material() != row("mars_dust"))
    );
    let ocean = table_inventory("Earth", 0.675);
    assert!(
        ocean
            .modes()
            .iter()
            .all(|m| m.material() != row("mars_dust"))
    );
    // Continuous through the thin-air limit and the lifting threshold.
    let at = |p: f64| {
        dust_optical_depth(
            Pascals::new(p),
            Kelvin::new(210.0),
            43.34,
            MetresPerSecondSquared::new(3.71),
            0.0,
        )
    };
    assert_same_bits(at(DUST_AIR_LIMIT.value()), 0.0);
    assert!(at(DUST_AIR_LIMIT.value() * (1.0 - 1e-9)) < 1e-8);
    assert!((at(636.0) / MARS_DUST_OPTICAL_DEPTH - 1.0).abs() < 1e-9);
    let mut previous = 0.0;
    for p in [10.0, 100.0, 300.0, 636.0, 2e3, 5e3, 9e3] {
        let tau = at(p);
        assert!(tau.is_finite() && tau >= 0.0, "{p} Pa");
        if p > 636.0 {
            assert!(tau <= previous + 1e-12, "τ {tau} at {p} Pa");
        }
        previous = tau;
    }
    assert!(at(10.0) < at(300.0), "thinner air lifts less");
}

/// P14.T53.c's census over the 400 whole systems, the golden systems and LOOK72's triaged systems
/// (a measurement, run by name), every 24 hold in force, against T53.c's own hold held:
///
/// - every `Tenuous` record, with count (xiv)'s figures (decision-p14-t53c-tenuous-aerosols §5):
///   its `p_s`, p ÷ g, x(CH₄), T24.c's haze index and the τ(550) its rule would give, and the dust's
///   τ by rule, the largest stated for P14.T55.n; asserted, under the record's air batch
///   (P14.T54.b), every `Tenuous` record's aerosols not modelled, never the empty inventory, and
///   its structure never computed as no air (decision-p14-t53c-tenuous-aerosols §2: its gases
///   are listed and its structure published);
/// - the moved states and pressures;
/// - asserted, the record says `Tenuous`, not `Airless`, where a gas is held above the floor, and
///   every `Airless` record lists no gas;
/// - asserted (decision-p14-t53c-collapse §5): with no collapse-first start, the bodies the start
///   as built moved keep their whole-column solve, as with T53.c held, to the bit
///   (02000b2cdfff0000.0200, 21fff6587ff80000.0100, 02000b2ca0000001.1000 named);
/// - asserted, the airs whose dominant condensable lies at or above 10 times its saturation
///   pressure over its ice (decision-t24i-airless-state D4) are [`ITEM_33_WORLD`], main's 24 gate
///   item (§2.1: a wrong state at 24, a `Snowball` at 100.03 Pa, N₂ ×559 over its 33.57 K ice,
///   whose steady state is `Tenuous` at 0.034 Pa), and count (vii)'s two, 01ffeb2d20000000.e101
///   and 02002b2d1ffe0000.e103, by name;
/// - asserted, no ice-held gas saturates at or above 100 Pa at its ice, and 01ffeb2c20010000.0200,
///   which the line keeps from a 0.8 bar N₂ air, keeps T13.b's verdict;
/// - asserted, no record's liquids list an ocean under its ice (count (i)), and every liquid entry
///   is listed at its own extent where the entries' total is at most 1 (decision-absurd-states-24
///   R1; W2).
#[test]
#[ignore = "a measurement for P14.T53.c's record, run by name"]
#[expect(clippy::too_many_lines, reason = "one census's lists, each stated")]
fn under_every_24_hold_t53c_s_thin_airs_are_stated() {
    use std::collections::BTreeMap;

    use crate::planetary::chem::tenuous::{TenuousRules, holds_tenuous_air, with_tenuous};
    use crate::planetary::derive::atmosphere::{AIRLESS_PRESSURE, Gas};
    use crate::planetary::system::tests::{
        ITEM_33_WORLD, ThinAirBody, thin_air_census, thin_air_sets,
    };
    /// Count (vii)'s two members (decision-p14-t53c-collapse §2.2).
    const COUNT_VII: [&str; 2] = ["01ffeb2d20000000.e101", "02002b2d1ffe0000.e103"];
    /// The bodies the collapse-first start as built moved that the ruling names.
    const RESTORED: [&str; 3] = [
        "02000b2cdfff0000.0200",
        "21fff6587ff80000.0100",
        "02000b2ca0000001.1000",
    ];
    /// The record the ice-held line keeps T13.b's verdict on.
    const KEPT_AIRLESS: &str = "01ffeb2c20010000.0200";
    let sets = thin_air_sets();
    let (held, tenuous) = crate::planetary::testing::with_every_24_hold(|| {
        (
            with_tenuous(TenuousRules::Held, || thin_air_census(&sets)),
            thin_air_census(&sets),
        )
    });
    let id_of = |b: &ThinAirBody| b.label.rsplit(' ').next().unwrap_or_default().to_owned();
    let before: BTreeMap<&str, &ThinAirBody> = held.iter().map(|b| (b.label.as_str(), b)).collect();
    let mut states: BTreeMap<String, u32> = BTreeMap::new();
    for body in &tenuous {
        *states.entry(format!("{:?}", body.state)).or_default() += 1;
    }
    eprintln!(
        "{} bodies with a surface; by state {states:?}",
        tenuous.len()
    );
    eprintln!("TENUOUS, with count (xiv)'s figures:");
    let (mut largest_haze, mut largest_dust) = (0.0_f64, 0.0_f64);
    for body in tenuous.iter().filter(|b| b.state == SurfaceState::Tenuous) {
        let (methane, carbon_dioxide) = (body.share(Gas::Methane), body.share(Gas::CarbonDioxide));
        let index = haze_index(methane, carbon_dioxide);
        let haze = haze_optical_depth(index);
        let dust = dust_optical_depth(
            Pascals::new(body.p),
            Kelvin::new(body.t_s),
            body.mu.unwrap_or(f64::NAN),
            MetresPerSecondSquared::new(body.g),
            body.liquid_cover,
        );
        largest_haze = largest_haze.max(haze);
        largest_dust = largest_dust.max(dust);
        eprintln!(
            "  {}: {}; x(CH4) {methane:.4e}, haze index {index:.4e}, haze τ by rule {haze:.4}, \
             dust τ by rule {dust:.4}; aerosols modelled {}; structure {:?}; R09 {:?}",
            body.label,
            body.line(),
            body.aerosols_modelled,
            body.structure,
            body.r09
        );
    }
    eprintln!("the largest haze τ by rule {largest_haze:.4}, dust τ {largest_dust:.4}");
    let mut structures: BTreeMap<String, u32> = BTreeMap::new();
    for body in tenuous.iter().filter(|b| b.state == SurfaceState::Tenuous) {
        *structures
            .entry(format!("{:?}", body.structure))
            .or_default() += 1;
    }
    eprintln!(
        "the tenuous records' structures (None not modelled, Some(true) a profile): {structures:?}"
    );
    eprintln!("MOVED from T53.c held (state, or p_s by more than 10⁻⁶):");
    for body in &tenuous {
        let Some(was) = before.get(body.label.as_str()) else {
            eprintln!("  {} new: {}", body.label, body.line());
            continue;
        };
        if was.state != body.state || ((body.p - was.p) / was.p.max(1e-30)).abs() > 1e-6 {
            eprintln!(
                "  {}\n    held: {}\n    now:  {}",
                body.label,
                was.line(),
                body.line()
            );
        }
    }
    let supersaturated: Vec<&ThinAirBody> = tenuous
        .iter()
        .filter(|b| b.factor().is_some_and(|f| f >= 10.0))
        .collect();
    eprintln!("SUPERSATURATED OVER THEIR ICE (×10 and more): main's gate item and count (vii):");
    for body in &supersaturated {
        eprintln!("  {}: {}", body.label, body.line());
    }
    let under_ice_held = held.iter().filter(|b| b.under_ice_item).count();
    let misread_held = held.iter().filter(|b| !b.misread.is_empty()).count();
    eprintln!(
        "OCEANS UNDER ICE LISTED: {} (T53.c held: {under_ice_held}); LIQUIDS AT OTHER THAN \
         THEIR OWN EXTENT: {} (T53.c held: {misread_held})",
        tenuous.iter().filter(|b| b.under_ice_item).count(),
        tenuous.iter().filter(|b| !b.misread.is_empty()).count(),
    );
    for id in RESTORED {
        let body = tenuous
            .iter()
            .find(|b| b.label.ends_with(id))
            .unwrap_or_else(|| panic!("{id} is in the census"));
        assert_eq!(
            body.bits,
            before[body.label.as_str()].bits,
            "{id}: {}",
            body.line()
        );
    }
    // Main's 24 gate item (decision-p14-t53c-collapse §2.1), as built: a wrong state at 24.
    let item_33 = tenuous
        .iter()
        .find(|b| b.label.ends_with(ITEM_33_WORLD))
        .expect("item 33's world is in the census");
    assert_eq!(item_33.state, SurfaceState::Snowball, "{}", item_33.line());
    assert!((100.0..100.1).contains(&item_33.p), "{}", item_33.line());
    let supersaturated: Vec<String> = supersaturated.iter().map(|b| id_of(b)).collect();
    let mut expected = vec![ITEM_33_WORLD];
    expected.extend(COUNT_VII);
    assert_eq!(supersaturated, expected, "the gate item and count (vii)");
    let kept = tenuous
        .iter()
        .find(|b| b.label.ends_with(KEPT_AIRLESS))
        .unwrap_or_else(|| panic!("{KEPT_AIRLESS} is in the census"));
    let was = before[kept.label.as_str()];
    assert!(kept.held_ice.is_empty(), "{}", kept.line());
    assert_eq!(kept.state, was.state, "{}", kept.line());
    for body in &tenuous {
        let at = || format!("{}: {}", body.label, body.line());
        for &(gas, p_sat) in &body.held_ice {
            assert!(
                p_sat < AIRLESS_PRESSURE.value(),
                "{gas:?} held at {p_sat} Pa: {}",
                at()
            );
        }
        match body.state {
            SurfaceState::Airless => {
                assert!(
                    body.gases.is_empty(),
                    "an airless record lists a gas: {}",
                    at()
                );
                assert!(
                    !holds_tenuous_air(
                        Pascals::new(body.air_p),
                        MetresPerSecondSquared::new(body.g)
                    ),
                    "an airless record's air reaches the floor ({:.4e} Pa): {}",
                    body.air_p,
                    at()
                );
            }
            SurfaceState::Tenuous => {
                assert!(!body.gases.is_empty(), "{}", at());
                assert!(body.p < AIRLESS_PRESSURE.value(), "{}", at());
                assert!(
                    holds_tenuous_air(Pascals::new(body.p), MetresPerSecondSquared::new(body.g)),
                    "{}",
                    at()
                );
                assert!(
                    !body.aerosols_modelled,
                    "a tenuous record's aerosols: {}",
                    at()
                );
                assert_ne!(
                    body.structure,
                    Some(false),
                    "a tenuous record's structure computed as no air: {}",
                    at()
                );
            }
            SurfaceState::GasEnvelope
            | SurfaceState::MagmaOcean
            | SurfaceState::RunawayGreenhouse
            | SurfaceState::Temperate
            | SurfaceState::Snowball
            | SurfaceState::SteamEnvelope => {}
        }
        assert!(!body.under_ice_item, "an ocean under ice listed: {}", at());
        assert!(body.misread.is_empty(), "{}: {:?}", at(), body.misread);
    }
}
