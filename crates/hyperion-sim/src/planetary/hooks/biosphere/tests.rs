//! P14.T53.d's hook: the table's Earth given Earth's fluxes by hand, the refusals of fluxes with no
//! stated sink, the sinks' and lifetimes' own laws, and every generated body abiotic over the
//! census's galaxy, every layer.

use std::collections::BTreeMap;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{SystemRecord, generate_cell};
use crate::galaxy::query::{QuerySphere, cells_in_sphere};
use crate::id::Layer;
use crate::planetary::derive::atmosphere::{RunawayDeck, SurfaceState};
use crate::planetary::hooks::aerosols::{AerosolInputs, AerosolInventory, DOBSON_UNIT};
use crate::planetary::hooks::structure::{RadiativeConvective, StructureInputs};
use crate::planetary::hooks::surface::tests::table;
use crate::planetary::record::{BodyKind, Modelled};
use crate::planetary::system::tests::SEED;
use crate::planetary::testing::SAMPLE_CENTRE_LY;
use crate::planetary::{SystemContext, generate};
use crate::time::UniverseTime;
use crate::units::{LightYears, Metres};

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// The Sun's effective temperature, K: 5,772 (IAU 2015 Resolution B3's nominal), at which
/// P14.T24.c's stellar factor is 1.
const SUN: Kelvin = Kelvin::new(5_772.0);

/// Earth's O₂ at the ground today, 0.21 bar: 20.95% of its 1,013.25 hPa (the U.S. Standard
/// Atmosphere 1976's 0.209476 by volume), the plan's target.
const EARTH_OXYGEN_BAR: f64 = 0.21;

/// Earth's ozone column, DU: 300, its global mean (WMO/UNEP 2018, as P14.T24.c's
/// [`EARTH_OZONE_COLUMN`](crate::planetary::hooks::aerosols::EARTH_OZONE_COLUMN) states it), the
/// plan's target.
const EARTH_OZONE_DU: f64 = 300.0;

/// Earth's CH₄ and N₂O at the ground today, by volume: about 1.9 and 0.33 ppm, NOAA's global means
/// of the early 2020s (from memory, not re-read), checks of the units within a factor of two only:
/// the air is not in steady state with today's rising emissions.
const EARTH_METHANE_AND_NITROUS_OXIDE: (f64, f64) = (1.9e-6, 0.33e-6);

/// Earth's fluxes, in Earth's units: one of each.
fn earth_fluxes() -> Biosphere {
    Biosphere::new([(row("O2"), 1.0), (row("CH4"), 1.0), (row("N2O"), 1.0)])
        .expect("Earth's fluxes are three gases' finite ones")
}

/// The table's Earth (the Solar System table's, P14.T24.a's conditions): its abiotic air at the
/// ground, its mean molar mass and its gravity.
fn table_earth() -> (
    Pascals,
    Vec<(SubstanceId, f64)>,
    f64,
    MetresPerSecondSquared,
) {
    let (_, body, conditions) = table()
        .into_iter()
        .find(|(name, ..)| *name == "Earth")
        .expect("the table holds Earth");
    (
        conditions.surface_pressure(),
        conditions.gases().to_vec(),
        conditions
            .mean_molar_mass_g_per_mol()
            .expect("Earth's air has a molar mass"),
        body.surface_gravity(),
    )
}

/// The inputs of the air `gases` at `p_s` under `mu` and `g`, about the Sun, with Earth's O₂
/// sinks.
fn inputs_of(
    p_s: Pascals,
    gases: &[(SubstanceId, f64)],
    mu: f64,
    g: MetresPerSecondSquared,
) -> BioticAirInputs<'_> {
    BioticAirInputs {
        surface_pressure: p_s,
        gases,
        mean_molar_mass_g_per_mol: mu,
        gravity: g,
        star_temperature: Some(SUN),
        oxygen_sinks: Some(OxygenSinks::EARTH),
    }
}

/// An air of nitrogen alone.
fn nitrogen() -> [(SubstanceId, f64); 1] {
    [(row("N2"), 1.0)]
}

/// The plan's test: the table's Earth, an abiotic air of nitrogen, water and carbon dioxide (it
/// lists no O₂: P14.T24.f), given Earth's fluxes and sinks by hand, reaches 0.21 bar of O₂ within a
/// factor of 2 and an ozone column within a factor of 2 of 300 DU; its CH₄ and N₂O land within a
/// factor of 2 of Earth's measured abundances, and its surface pressure rises by the O₂'s weight.
#[test]
fn the_table_s_earth_given_earth_s_fluxes_reaches_earth_s_oxygen_and_ozone() {
    let (p_s, gases, mu, g) = table_earth();
    assert_eq!(
        gases.iter().find(|&&(id, _)| id == row("O2")),
        None,
        "the table's Earth is abiotic"
    );
    let air = BioticAir::derive(&earth_fluxes(), &inputs_of(p_s, &gases, mu, g))
        .expect("Earth's fluxes have their sinks");
    let oxygen = air.partial_pressure(row("O2")).value() / PASCALS_PER_BAR;
    let ozone = air.ozone_column_per_m2() / DOBSON_UNIT;
    let (methane, nitrous_oxide) = (air.fraction(row("CH4")), air.fraction(row("N2O")));
    eprintln!(
        "the table's Earth: {:.4} bar abiotic, {:.4} bar with its biosphere, μ {:.3} → {:.3} \
         g/mol; O₂ {oxygen:.4} bar at {:.6} PAL, O₃ {ozone:.1} DU, CH₄ {:.3} ppm, N₂O {:.3} ppm",
        p_s.value() / PASCALS_PER_BAR,
        air.surface_pressure().value() / PASCALS_PER_BAR,
        mu,
        air.mean_molar_mass_g_per_mol(),
        air.oxygen_level(),
        methane * 1e6,
        nitrous_oxide * 1e6,
    );
    let within_two = |got: f64, want: f64| (0.5..=2.0).contains(&(got / want));
    assert!(within_two(oxygen, EARTH_OXYGEN_BAR), "O₂ {oxygen} bar");
    assert!(within_two(ozone, EARTH_OZONE_DU), "O₃ {ozone} DU");
    let (methane_now, nitrous_oxide_now) = EARTH_METHANE_AND_NITROUS_OXIDE;
    assert!(within_two(methane, methane_now), "CH₄ {methane}");
    assert!(
        within_two(nitrous_oxide, nitrous_oxide_now),
        "N₂O {nitrous_oxide}"
    );
    // Earth's flux against Earth's sinks is Earth's present level, and the ozone T24.c's at 1 PAL.
    assert!((air.oxygen_level() - 1.0).abs() < 1e-12, "{air:?}");
    assert!((ozone - EARTH_OZONE_DU).abs() < 1e-9, "{ozone}");
    // Every gas's fraction sums to the abiotic air's sum, and the pressure rises by the O₂'s
    // weight, about a fifth of a bar.
    let sum = |gases: &[(SubstanceId, f64)]| gases.iter().map(|&(_, x)| x).sum::<f64>();
    assert!((sum(air.gases()) - sum(&gases)).abs() < 1e-12);
    let rise = (air.surface_pressure().value() - p_s.value()) / PASCALS_PER_BAR;
    assert!((0.2..0.25).contains(&rise), "{rise} bar");
}

/// The biotic air's O₂ is T24.c's own: its gases, pressure and molar mass, given to P14.T24.c's
/// inventory as a record's surface would give them, make the inventory's ozone absorber of the
/// biotic air's column to 10⁻¹².
#[test]
fn the_inventory_of_the_biotic_air_carries_its_ozone() {
    let (p_s, gases, mu, g) = table_earth();
    let air = BioticAir::derive(&earth_fluxes(), &inputs_of(p_s, &gases, mu, g)).unwrap();
    let structure = RadiativeConvective::derive(&StructureInputs {
        surface_temperature: Kelvin::new(288.0),
        surface_pressure: air.surface_pressure(),
        gases: air.gases(),
        condensable: Some(row("H2O")),
        skin_temperature: Kelvin::new(214.0),
    })
    .expect("an air");
    let inventory = AerosolInventory::derive(&AerosolInputs {
        state: SurfaceState::Temperate,
        surface_pressure: air.surface_pressure(),
        gases: air.gases(),
        mean_molar_mass_g_per_mol: air.mean_molar_mass_g_per_mol(),
        gravity: g,
        structure: &structure,
        cloud_fraction: 0.5,
        star_temperature: Some(SUN),
        surface_liquid_cover: 0.7,
        outgassing: 1.0,
        liquid_water: true,
        runaway_deck: RunawayDeck::Kept,
    })
    .expect("an inventory");
    let ozone = inventory
        .absorbers()
        .iter()
        .find(|a| a.species() == row("O3"))
        .expect("the biotic air's O₂ makes ozone");
    let column = ozone.column_per_m2();
    assert!(
        (column / air.ozone_column_per_m2() - 1.0).abs() < 1e-12,
        "{column} against {}",
        air.ozone_column_per_m2()
    );
}

/// The plan's test: fluxes with no sinks stated are refused, each by its variant. O₂ with no sinks
/// given; a gas the chemistry states no sink of; CH₄ and N₂O into an anoxic air (no O₂ flux, or one
/// the reductants consume) and N₂O into an air outside its range; and a net sink of a gas whose only
/// sink is its lifetime.
#[test]
fn fluxes_with_no_sinks_stated_are_refused() {
    let gases = nitrogen();
    let inputs = inputs_of(
        Pascals::new(8.0e4),
        &gases,
        28.013,
        MetresPerSecondSquared::new(9.81),
    );
    let derive = |fluxes: &[(&str, f64)], inputs: &BioticAirInputs<'_>| {
        let biosphere = Biosphere::new(fluxes.iter().map(|&(key, flux)| (row(key), flux)))
            .expect("finite fluxes of gases");
        BioticAir::derive(&biosphere, inputs)
    };
    let unsunk = BioticAirInputs {
        oxygen_sinks: None,
        ..inputs
    };
    assert_eq!(
        derive(&[("O2", 1.0)], &unsunk),
        Err(DeriveBioticAirError::NoSink { species: row("O2") })
    );
    for key in ["NH3", "H2S", "CO", "H2"] {
        assert_eq!(
            derive(&[("O2", 1.0), (key, 1.0)], &inputs),
            Err(DeriveBioticAirError::NoSink { species: row(key) }),
            "{key}"
        );
    }
    for key in ["CH4", "N2O"] {
        assert_eq!(
            derive(&[(key, 1.0)], &inputs),
            Err(DeriveBioticAirError::SinkOutOfRange {
                species: row(key),
                oxygen_level: 0.0
            }),
            "{key} in an anoxic air"
        );
        // The reductants consume a flux of 0.1 of Earth's: the air stays anoxic.
        assert_eq!(
            derive(&[("O2", 0.1), (key, 1.0)], &inputs),
            Err(DeriveBioticAirError::SinkOutOfRange {
                species: row(key),
                oxygen_level: 0.0
            }),
            "{key} beside O₂ the reductants consume"
        );
        assert_eq!(
            derive(&[("O2", 1.0), (key, -0.5)], &inputs),
            Err(DeriveBioticAirError::NetSink {
                species: row(key),
                flux_earth: -0.5
            }),
            "a net sink of {key}"
        );
    }
    // A third of Earth's O₂ flux holds about 0.06 PAL: CH₄'s lifetime is stated there, N₂O's not.
    let thin = derive(&[("O2", 0.35), ("CH4", 1.0)], &inputs).expect("CH₄ in an oxic air");
    assert!((0.01..0.5).contains(&thin.oxygen_level()), "{thin:?}");
    assert_eq!(
        derive(&[("O2", 0.35), ("N2O", 1.0)], &inputs),
        Err(DeriveBioticAirError::SinkOutOfRange {
            species: row("N2O"),
            oxygen_level: thin.oxygen_level()
        })
    );
    // A zero flux needs no sink.
    assert!(derive(&[("O2", 0.0), ("NH3", 0.0)], &unsunk).is_ok());
}

/// Inputs out of their range are refused, each by its variant.
#[test]
fn bad_inputs_are_refused_by_their_variants() {
    let gases = nitrogen();
    let good = inputs_of(
        Pascals::new(8.0e4),
        &gases,
        28.013,
        MetresPerSecondSquared::new(9.81),
    );
    let earth = earth_fluxes();
    let derive = |inputs: &BioticAirInputs<'_>| BioticAir::derive(&earth, inputs);
    for p in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let inputs = BioticAirInputs {
            surface_pressure: Pascals::new(p),
            ..good
        };
        assert!(
            matches!(
                derive(&inputs),
                Err(DeriveBioticAirError::SurfacePressure(_))
            ),
            "{p}"
        );
    }
    let inputs = BioticAirInputs {
        mean_molar_mass_g_per_mol: 0.0,
        ..good
    };
    assert_eq!(
        derive(&inputs),
        Err(DeriveBioticAirError::MolarMass {
            molar_mass_g_per_mol: 0.0
        })
    );
    let inputs = BioticAirInputs {
        gravity: MetresPerSecondSquared::new(-9.81),
        ..good
    };
    assert_eq!(
        derive(&inputs),
        Err(DeriveBioticAirError::Gravity(MetresPerSecondSquared::new(
            -9.81
        )))
    );
    let inputs = BioticAirInputs {
        star_temperature: Some(Kelvin::new(0.0)),
        ..good
    };
    assert_eq!(
        derive(&inputs),
        Err(DeriveBioticAirError::StarTemperature(Kelvin::new(0.0)))
    );
    let bad = [(row("N2"), 1.5)];
    let inputs = BioticAirInputs {
        gases: &bad,
        ..good
    };
    assert_eq!(
        derive(&inputs),
        Err(DeriveBioticAirError::MoleFraction {
            species: row("N2"),
            fraction: 1.5
        })
    );
    // An unlit body makes no ozone, its O₂ kept.
    let dark = BioticAirInputs {
        star_temperature: None,
        ..good
    };
    let air = derive(&dark).expect("an unlit air");
    assert_same_bits(air.ozone_column_per_m2(), 0.0);
    assert!(air.fraction(row("O2")) > 0.1);
}

/// A biosphere's fluxes are refused where they are not finite, not a gas's or given twice, and
/// kept in the order of their keys with a −0 as +0.
#[test]
fn a_biosphere_is_checked_and_kept_in_key_order() {
    assert_eq!(
        Biosphere::new([(row("tholin"), 1.0)]),
        Err(BuildBiosphereError::NotAGas(row("tholin")))
    );
    assert_eq!(
        Biosphere::new([(row("CH4"), f64::INFINITY)]),
        Err(BuildBiosphereError::NotFinite {
            species: row("CH4"),
            flux_earth: f64::INFINITY
        })
    );
    assert!(matches!(
        Biosphere::new([(row("CH4"), f64::NAN)]),
        Err(BuildBiosphereError::NotFinite { .. })
    ));
    assert_eq!(
        Biosphere::new([(row("O2"), 1.0), (row("O2"), 2.0)]),
        Err(BuildBiosphereError::Repeated(row("O2")))
    );
    let biosphere = Biosphere::new([(row("O2"), 1.0), (row("N2O"), -0.0), (row("CH4"), 2.0)])
        .expect("three gases");
    let keys: Vec<String> = biosphere
        .fluxes()
        .iter()
        .map(|&(id, _)| id.substance().key().as_str().to_owned())
        .collect();
    assert_eq!(keys, ["CH4", "N2O", "O2"]);
    assert_same_bits(biosphere.flux(row("N2O")), 0.0);
    assert_same_bits(biosphere.flux(row("CH4")), 2.0);
    assert_same_bits(biosphere.flux(row("NH3")), 0.0);
    for text in [
        BuildBiosphereError::NotAGas(row("tholin")).to_string(),
        DeriveBioticAirError::NoSink {
            species: row("NH3"),
        }
        .to_string(),
    ] {
        assert!(
            !text.ends_with('.') && text.starts_with(char::is_lowercase),
            "{text}"
        );
    }
}

/// O₂'s steady state: Earth's sinks balance Earth's flux at Earth's present level; the level rises
/// with the flux as ((F − R) ÷ W)², from 0 at the reductants; and sinks out of range are refused.
#[test]
fn the_oxygen_steady_state_balances_its_sinks() {
    let earth = OxygenSinks::EARTH;
    assert!((earth.reductants() + earth.weathering() - 1.0).abs() < 1e-15);
    assert!((earth.steady_level(1.0) - 1.0).abs() < 1e-12);
    assert_same_bits(earth.steady_level(earth.reductants()), 0.0);
    assert_same_bits(earth.steady_level(-1.0), 0.0);
    assert_same_bits(earth.steady_level(f64::NAN), 0.0);
    let mut last = 0.0;
    for k in 1..=40 {
        let flux = 0.1 * f64::from(k);
        let level = earth.steady_level(flux);
        assert!(
            level > last || flux <= earth.reductants(),
            "{flux}: {level}"
        );
        // At the level, the sinks take the flux.
        if level > 0.0 {
            let taken = earth.reductants()
                + earth.weathering() * math::powf(level, OXIDATIVE_WEATHERING_EXPONENT);
            assert!((taken / flux - 1.0).abs() < 1e-12, "{flux}: {taken}");
        }
        last = level;
    }
    assert_eq!(
        OxygenSinks::new(-0.1, 1.0),
        Err(BuildOxygenSinksError::Reductants(-0.1))
    );
    assert_eq!(
        OxygenSinks::new(0.1, 0.0),
        Err(BuildOxygenSinksError::Weathering(0.0))
    );
    assert!(matches!(
        OxygenSinks::new(f64::NAN, 1.0),
        Err(BuildOxygenSinksError::Reductants(_))
    ));
    let stated = OxygenSinks::new(-0.0, 2.0).expect("in range");
    assert_same_bits(stated.reductants(), 0.0);
    assert!((stated.steady_level(2.0) - 1.0).abs() < 1e-15);
}

/// CH₄'s lifetime is Earth's present one at 1 PAL and Cooke et al.'s ratio of it at each of their
/// levels, rises monotonically with the O₂ level between, and is not stated outside 10⁻³–1.5 PAL;
/// N₂O's is its present one over its range alone.
#[test]
fn the_lifetimes_are_stated_over_their_ranges() {
    for &(level, ratio) in &METHANE_LIFETIME_BY_OXYGEN {
        let lifetime = methane_lifetime(level).expect("a node is in range").value();
        assert!(
            (lifetime / (ratio * PRESENT_METHANE_LIFETIME.value()) - 1.0).abs() < 1e-12,
            "{level}: {lifetime}"
        );
    }
    let mut last = 0.0;
    for k in 0..=200 {
        let level = 1e-3 * math::powf(1.5e3, f64::from(k) / 200.0);
        let lifetime = methane_lifetime(level.min(1.5)).expect("in range").value();
        assert!(lifetime > last, "{level}: {lifetime}");
        last = lifetime;
    }
    for level in [0.0, 9.9e-4, 1.5001, 10.0, f64::NAN] {
        assert_eq!(methane_lifetime(level), None, "{level}");
    }
    assert_eq!(
        nitrous_oxide_lifetime(1.0),
        Some(PRESENT_NITROUS_OXIDE_LIFETIME)
    );
    for level in [0.5, 1.5] {
        assert!(nitrous_oxide_lifetime(level).is_some(), "{level}");
    }
    for level in [0.0, 0.49, 1.51, f64::NAN] {
        assert_eq!(nitrous_oxide_lifetime(level), None, "{level}");
    }
}

/// A biosphere of no fluxes, or of zeros, leaves its air as it is to the bit but its O₂ level,
/// whose own O₂ a desiccated world's air keeps; and an air's own O₂ is added to the biotic level.
#[test]
fn an_empty_biosphere_leaves_its_air_and_an_air_s_own_oxygen_is_added() {
    let (p_s, gases, mu, g) = table_earth();
    let inputs = inputs_of(p_s, &gases, mu, g);
    for biosphere in [
        Biosphere::default(),
        Biosphere::new([(row("O2"), 0.0), (row("CH4"), 0.0)]).unwrap(),
    ] {
        let air = BioticAir::derive(&biosphere, &inputs).unwrap();
        assert_same_bits(air.surface_pressure().value(), p_s.value());
        assert_eq!(air.gases().len(), gases.len());
        for (&(id, x), &(own, y)) in air.gases().iter().zip(&gases) {
            assert_eq!(id, own);
            assert!((x - y).abs() <= 1e-15 * y.max(1e-300), "{x} against {y}");
        }
        assert_same_bits(air.oxygen_level(), 0.0);
        assert_same_bits(air.ozone_column_per_m2(), 0.0);
    }
    // A desiccated world's 0.05 bar of abiotic O₂ beside 0.75 bar of N₂, given Earth's O₂ flux.
    let own = [(row("N2"), 0.75 / 0.8), (row("O2"), 0.05 / 0.8)];
    let mu = 0.75 / 0.8 * 28.013 + 0.05 / 0.8 * 31.998;
    let g = MetresPerSecondSquared::new(9.80665);
    let alone = BioticAir::derive(
        &Biosphere::default(),
        &inputs_of(Pascals::new(8.0e4), &own, mu, g),
    )
    .unwrap();
    let both = BioticAir::derive(
        &Biosphere::new([(row("O2"), 1.0)]).unwrap(),
        &inputs_of(Pascals::new(8.0e4), &own, mu, g),
    )
    .unwrap();
    assert!((both.oxygen_level() - alone.oxygen_level() - 1.0).abs() < 1e-12);
    assert_eq!(both.gases().len(), 2, "O₂ joins its own entry");
}

/// The chemistry is deterministic: the same biosphere and air give the same bits twice.
#[test]
fn the_biotic_air_is_the_same_bits_twice() {
    let (p_s, gases, mu, g) = table_earth();
    let inputs = inputs_of(p_s, &gases, mu, g);
    let once = BioticAir::derive(&earth_fluxes(), &inputs).unwrap();
    let again = BioticAir::derive(&earth_fluxes(), &inputs).unwrap();
    assert_same_bits(
        once.surface_pressure().value(),
        again.surface_pressure().value(),
    );
    assert_same_bits(once.ozone_column_per_m2(), again.ozone_column_per_m2());
    assert_same_bits(
        once.mean_molar_mass_g_per_mol(),
        again.mean_molar_mass_g_per_mol(),
    );
    assert_eq!(once.gases().len(), again.gases().len());
    for (&(id, x), &(other, y)) in once.gases().iter().zip(again.gases()) {
        assert_eq!(id, other);
        assert_same_bits(x, y);
    }
}

/// The palette's cover carries its `biological` flag: P14.T51.d's tholin abiotic, a living cover
/// biological, each of its substance.
#[test]
fn a_cover_carries_its_biological_flag() {
    let tholin = Cover::abiotic(row("tholin"));
    assert!(!tholin.is_biological());
    assert_eq!(tholin.substance(), row("tholin"));
    let living = Cover::living(row("tholin"));
    assert!(living.is_biological());
    assert_ne!(tholin, living);
}

/// The systems nearest the census's Sun-like point ([`SAMPLE_CENTRE_LY`]) of layer `layer` at the
/// epoch, `n` of them, nearest first, ties broken by ID, in spheres doubling from 16 ly, as
/// `sample_records` draws the stellar layers' sample.
fn nearest_of_layer(galaxy: &Galaxy, layer: Layer, n: usize) -> Vec<SystemRecord> {
    let centre = GalacticPosition::from_light_years(SAMPLE_CENTRE_LY).unwrap();
    let mut radius = 16.0;
    let mut cell = Vec::new();
    loop {
        let sphere = QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .unwrap();
        let limit = Metres::from(LightYears::new(radius));
        let mut found = Vec::new();
        for key in cells_in_sphere(layer, &sphere) {
            generate_cell(galaxy, key, &mut cell);
            for record in &cell {
                let distance = centre.distance_to(record.epoch_position());
                if distance <= limit {
                    found.push((distance, *record));
                }
            }
        }
        if found.len() >= n {
            found
                .sort_by(|(d1, r1), (d2, r2)| d1.total_cmp(d2).then_with(|| r1.id().cmp(&r2.id())));
            return found.into_iter().take(n).map(|(_, r)| r).collect();
        }
        radius *= 2.0;
    }
}

/// The systems a layer gives the census of every layer.
const SYSTEMS_PER_LAYER: usize = 8;

/// The plan's test: every generated body is abiotic, its biosphere member absent, over the census's
/// galaxy (the system tests' universe, P14.T33.c's) and every layer, A to E, the brown dwarfs'
/// and the rogue planets', the [`SYSTEMS_PER_LAYER`] systems of each nearest the Sun-like point,
/// each generated from its record and snapshot at the epoch, at the version and under every 26
/// hold. Every layer's records are walked; every record with a surface is counted by kind
/// (a rogue planet's bodies have none, with no star to model one by).
#[test]
fn every_generated_body_is_abiotic_over_every_layer() {
    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records: Vec<(Layer, SystemRecord)> = Layer::ALL
        .iter()
        .flat_map(|&layer| {
            nearest_of_layer(&galaxy, layer, SYSTEMS_PER_LAYER)
                .into_iter()
                .map(move |record| (layer, record))
        })
        .collect();
    let census = || {
        let mut walked: BTreeMap<Layer, usize> = BTreeMap::new();
        let mut surfaces: BTreeMap<(Layer, &'static str), usize> = BTreeMap::new();
        for (layer, record) in &records {
            let ctx = SystemContext::from_record(&galaxy, record);
            let system = generate(SEED, &ctx);
            for body in system.snapshot_at(&ctx, UniverseTime::EPOCH).bodies() {
                *walked.entry(*layer).or_default() += 1;
                let Some(surface) = body.surface().ok() else {
                    continue;
                };
                assert_eq!(
                    surface.biosphere(),
                    Modelled::NotModelled,
                    "{}",
                    body.identity().id()
                );
                let kind = match body.identity().kind() {
                    BodyKind::Planet => "planets",
                    BodyKind::DwarfPlanet => "members",
                    BodyKind::Moon(_) => "moons",
                    other => panic!("{other:?} with a surface"),
                };
                *surfaces.entry((*layer, kind)).or_default() += 1;
            }
        }
        assert_eq!(
            walked.keys().copied().collect::<Vec<_>>(),
            Layer::ALL,
            "every layer's records walked: {walked:?}"
        );
        surfaces
    };
    let at_25 = census();
    let under_26 = census();
    for (rules, surfaces) in [("25", &at_25), ("26", &under_26)] {
        eprintln!("surfaces under {rules}: {surfaces:?}");
        for kind in ["planets", "members", "moons"] {
            assert!(
                surfaces.iter().any(|(&(_, k), &n)| k == kind && n > 0),
                "{rules}: no {kind} with a surface"
            );
        }
    }
    // The stellar layers give surfaces at every rule; the free-floating hosts from 26.
    for layer in [Layer::A, Layer::B, Layer::C] {
        assert!(
            at_25.keys().any(|&(l, _)| l == layer),
            "layer {layer:?} at 25"
        );
    }
    assert!(
        under_26
            .keys()
            .any(|&(l, _)| matches!(l, Layer::BrownDwarf | Layer::RoguePlanet)),
        "{under_26:?}"
    );
}
