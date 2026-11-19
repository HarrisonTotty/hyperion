//! The silicate vapour's tests: its hold, the pressure against its sources, the species, the
//! escape and the composition classes, and its golden.

use super::*;
use crate::planetary::hooks::crust::CrustLithology;
use crate::units::consts::EARTH_RADIUS_M;

/// SF09's Fig. 1 BSE curve, log₁₀ bar, at [`OFFSET_NODES_K`] (read as the offsets are).
const SF09_BSE_LOG10_BAR: [f64; 4] = [-6.34, -3.82, -2.00, -0.88];

/// A vapour over a melt at `t_k` of the class `composition`, on a body of surface gravity `g`, m
/// s⁻², and radius `r`, m.
fn vapour(t_k: f64, composition: MeltComposition, g: f64, r: f64) -> SilicateVapour {
    SilicateVapour::derive(&SilicateVapourInputs {
        melt_temperature: Kelvin::new(t_k),
        composition,
        gravity: MetresPerSecondSquared::new(g),
        radius: Metres::new(r),
    })
    .unwrap()
}

/// A lava world's dayside melt: 8 M⊕ at 1.9 R⊕, g = 9.82 × 8 ÷ 1.9² m s⁻² (55 Cancri e's size).
fn lava_world(t_k: f64, composition: MeltComposition) -> SilicateVapour {
    vapour(t_k, composition, 21.8, 1.9 * EARTH_RADIUS_M)
}

/// The share of `key` in `vapour`, 0 where it lists none.
fn share(vapour: &SilicateVapour, key: &str) -> f64 {
    vapour
        .species()
        .iter()
        .find(|(id, _)| id.substance().key().as_str() == key)
        .map_or(0.0, |&(_, x)| x)
}

/// The key of the species `vapour` holds most of.
fn leader(vapour: &SilicateVapour) -> String {
    vapour.species()[0].0.substance().key().as_str().to_owned()
}

/// P14.T55.b's hold: 35's bump deletes it, and until then the version holds the vapour and the
/// active lava and only the tests' switch models them.
#[test]
fn the_silicate_vapour_is_held_only_until_the_35_bump() {
    use SilicateVapourRules::{Held, Modelled};
    assert!(
        GENERATOR_VERSION < SILICATE_VAPOUR_VERSION,
        "the 34 → 35 bump models the silicate vapour and the active lava (P14.T55.b): delete \
         `SILICATE_VAPOUR_VERSION`, `SilicateVapourRules` with its `Held` arm, \
         `with_silicate_vapour`, its line in `planetary/testing/held.rs` and in \
         `with_every_35_hold`, and this test; the `Held` paths of `record_silicate_vapour` and the \
         crust's `melt_area`, and `NOT_MODELLED`'s eruption temperature; unwrap the tests' \
         switches (the record test's held half and the census's name lose the hold); restate the \
         server's `every_surface_sends_the_melt_s_vapour_absent_and_e101_the_volatiles_zero`, the \
         fixtures' `silicate_vapour` (`planetary.json` gains `null` on its temperate world, its \
         client's `e101Earth` guard and `@hyperion/protocol`'s \"no key\" checks with it) and R09's \
         `surface_pressure` docs (`field/header.rs`, `surface/inputs.rs`); and bless by name the \
         goldens whose records hold a melt or a volcanic lid's active lava, R09's coarse inputs and \
         palettes of those lids among them, the melt area moving with the lava"
    );
    assert_eq!(SilicateVapourRules::in_force(), Held);
    assert_eq!(
        with_silicate_vapour(Modelled, SilicateVapourRules::in_force),
        Modelled
    );
    assert_eq!(
        with_silicate_vapour(Modelled, || with_silicate_vapour(
            Held,
            SilicateVapourRules::in_force
        )),
        Held
    );
    assert_eq!(SilicateVapourRules::in_force(), Held);
}

/// The BSE's pressure is Fegley and Schaefer's Eq. 13 (−3.769 log₁₀ bar at 2,000 K, +1.117 at
/// 4,000 K), within 0.15 dex of SF09's own curve from 1,500 to 3,000 K (0.14 at 3,000 K, where
/// the closed form sits between SF09's and VF13's) and within 0.06 dex of VF13's BSE at 3,500
/// and 4,000 K (+0.28 and +1.17), and is held above 6,000 K.
#[test]
fn the_bse_pressure_is_fegley_and_schaefer_s_closed_form() {
    assert!((bse_log10_pressure_bar(2_000.0) + 3.769).abs() < 1e-3);
    assert!((bse_log10_pressure_bar(4_000.0) - 1.117).abs() < 1e-3);
    for (t_k, sf09) in OFFSET_NODES_K.into_iter().zip(SF09_BSE_LOG10_BAR) {
        let p = bse_log10_pressure_bar(t_k);
        assert!((p - sf09).abs() < 0.15, "{t_k} K: {p} against {sf09}");
    }
    for (t_k, vf13) in [(3_500.0, 0.28), (4_000.0, 1.17)] {
        let p = bse_log10_pressure_bar(t_k);
        assert!((p - vf13).abs() < 0.06, "{t_k} K: {p} against {vf13}");
    }
    assert_same(
        bse_log10_pressure_bar(9_000.0),
        bse_log10_pressure_bar(6_000.0),
    );
    // It rises with the melt's temperature throughout.
    let mut last = f64::NEG_INFINITY;
    for step in 0..=60 {
        let p = bse_log10_pressure_bar(1_200.0 + 80.0 * f64::from(step));
        assert!(p > last, "{p} after {last}");
        last = p;
    }
}

/// Asserts that `a` and `b` have the same bits.
fn assert_same(a: f64, b: f64) {
    assert_eq!(
        a.total_cmp(&b),
        core::cmp::Ordering::Equal,
        "{a} against {b}"
    );
}

/// Each class's pressure is the BSE's moved by its SF09 offset at the nodes, linear in 1 ÷ T
/// between them and continuous, held below 1,500 K and fading to the BSE's at 4,000 K, so that
/// SF09's oceanic and continental crusts' curves are met within 0.15 dex, as their BSE's is.
#[test]
fn each_composition_s_pressure_meets_sf09_s_curve() {
    let sf09 = [
        (MeltComposition::OceanicCrust, [-6.31, -3.71, -1.84, -0.70]),
        (
            MeltComposition::ContinentalCrust,
            [-6.87, -4.15, -2.10, -0.70],
        ),
    ];
    for (composition, curve) in sf09 {
        for ((t_k, sf09), offset) in OFFSET_NODES_K
            .into_iter()
            .zip(curve)
            .zip(composition.offsets())
        {
            assert_same(composition.offset_dex(t_k), offset);
            let p = math::log10(lava_world(t_k, composition).pressure().value() / PASCALS_PER_BAR);
            assert!(
                (p - sf09).abs() < 0.15,
                "{composition:?} at {t_k} K: {p} against {sf09}"
            );
        }
        assert_same(composition.offset_dex(1_000.0), composition.offsets()[0]);
        assert_same(composition.offset_dex(4_000.0), 0.0);
        assert_same(composition.offset_dex(5_000.0), 0.0);
        let half = composition.offset_dex(3_500.0);
        assert!(
            (half - composition.offsets()[3] / 2.0).abs() < 1e-12,
            "{half}"
        );
        // Continuous across every node.
        for t_k in [1_500.0, 2_000.0, 2_500.0, 3_000.0, 4_000.0] {
            let (below, above) = (
                composition.offset_dex(t_k - 1e-6),
                composition.offset_dex(t_k + 1e-6),
            );
            assert!((below - above).abs() < 1e-6, "{composition:?} at {t_k} K");
        }
    }
    for t_k in [1_300.0, 2_200.0, 3_300.0, 4_500.0] {
        assert_same(MeltComposition::BulkSilicate.offset_dex(t_k), 0.0);
    }
}

/// The species lead as SF09 and VF13 find over the BSE (VF13's crossovers): Na below about
/// 3,100 K, with O₂ at about a quarter of it at 2,000 K (`Na₂O` → 2 Na + ½ O₂), and `SiO` above;
/// the fractions sum to 1, largest first, at every temperature, and at a node they are its row's.
#[test]
fn the_species_lead_as_sf09_and_vf13_find() {
    for t_k in [1_300.0, 1_684.4, 2_000.0, 2_500.0, 3_000.0] {
        assert_eq!(
            leader(&lava_world(t_k, MeltComposition::BulkSilicate)),
            "Na",
            "{t_k} K"
        );
    }
    for t_k in [3_200.0, 3_500.0, 4_000.0, 5_000.0] {
        assert_eq!(
            leader(&lava_world(t_k, MeltComposition::BulkSilicate)),
            "SiO",
            "{t_k} K"
        );
    }
    let cool = lava_world(2_000.0, MeltComposition::BulkSilicate);
    let ratio = share(&cool, "O2") / share(&cool, "Na");
    assert!((0.2..0.3).contains(&ratio), "{ratio}");
    for step in 0..=80 {
        let t_k = 1_000.0 + 60.0 * f64::from(step);
        let v = lava_world(t_k, MeltComposition::ContinentalCrust);
        let sum = v.species().iter().fold(0.0, |sum, &(_, x)| sum + x);
        assert!((sum - 1.0).abs() < 1e-12, "{t_k} K: {sum}");
        assert_eq!(v.species().len(), SPECIES.len());
        assert!(
            v.species().windows(2).all(|pair| pair[0].1 >= pair[1].1),
            "{t_k} K: {:?}",
            v.species()
        );
        assert!(v.species().iter().all(|&(_, x)| x > 0.0));
    }
    for (t_k, row) in SPECIES_NODES_K.into_iter().zip(BSE_PARTIALS_LOG10_BAR) {
        let total = row.iter().fold(0.0, |sum, &p| sum + math::exp10(p));
        for (shares, (key, p)) in bse_shares(t_k).into_iter().zip(SPECIES.iter().zip(row)) {
            let node = math::exp10(p) / total;
            assert!((shares - node).abs() < 1e-12, "{t_k} K {key}");
        }
        // Each node's sum lies near the closed form's, and its oxygen balances its metals.
        let log_sum = math::log10(total);
        assert!(
            (log_sum - bse_log10_pressure_bar(t_k)).abs() < 0.1,
            "{t_k} K"
        );
        let p = row.map(math::exp10);
        let freed = 0.5 * p[0] + p[3] + p[4] + p[5] + 0.5 * p[7];
        let held = 2.0 * p[1] + p[2];
        assert!(
            (held / freed - 1.0).abs() < 0.06,
            "{t_k} K: {}",
            held / freed
        );
    }
}

/// Every species is a registry row with a molar mass, and the mean molar mass lies between the
/// lightest and the heaviest of them.
#[test]
fn every_species_is_a_registry_row_with_a_molar_mass() {
    for key in SPECIES {
        let id = by_key(key).unwrap_or_else(|| panic!("{key} is a registry row"));
        assert!(id.substance().molar_mass_g_per_mol().is_some(), "{key}");
    }
    for t_k in [1_500.0, 2_500.0, 3_500.0] {
        let mu = lava_world(t_k, MeltComposition::BulkSilicate).mean_molar_mass_g_per_mol();
        assert!((15.999..60.1).contains(&mu), "{t_k} K: {mu}");
    }
}

/// A super-Earth's dayside vapour is bound (λ in the hundreds), and `e101`'s at 1,684.4 K, 399 km
/// in radius at g 0.254 m s⁻², is an outflow at λ 0.18, inside decision-triton-and-e101 §2.2's
/// 0.17 (Na) to 0.32 (`SiO`); the class turns at the critical value, itself bound: an atomic
/// gas's 2.1 where atoms lead the vapour, as at 1,684.4 and 3,000 K, and a diatomic gas's 2.4
/// where molecules are more than half of it, as at 3,500 K.
#[test]
fn the_vapour_is_bound_on_a_lava_world_and_an_outflow_on_e101() {
    let pool = lava_world(2_500.0, MeltComposition::BulkSilicate);
    assert_eq!(pool.escape(), VapourEscape::Bound);
    assert!(pool.jeans_parameter() > 100.0, "{}", pool.jeans_parameter());
    let e101 = vapour(1_684.4, MeltComposition::BulkSilicate, 0.254, 3.99e5);
    assert_eq!(e101.escape(), VapourEscape::Outflow);
    let lambda = e101.jeans_parameter();
    assert!((0.17..0.32).contains(&lambda), "{lambda}");
    assert_same(e101.melt_temperature().value(), 1_684.4);
    // λ = g R m ÷ (k T): a radius that puts it at the critical value is bound, a hair less not.
    let m = e101.mean_molar_mass_g_per_mol() * ATOMIC_MASS_CONSTANT_KG;
    assert_same(
        e101.critical_jeans_parameter(),
        ATOMIC_CRITICAL_JEANS_PARAMETER,
    );
    let r = ATOMIC_CRITICAL_JEANS_PARAMETER * BOLTZMANN_CONSTANT * 1_684.4 / (0.254 * m);
    let at = vapour(
        1_684.4,
        MeltComposition::BulkSilicate,
        0.254,
        r * (1.0 + 1e-9),
    );
    let under = vapour(
        1_684.4,
        MeltComposition::BulkSilicate,
        0.254,
        r * (1.0 - 1e-9),
    );
    assert_eq!(
        (at.escape(), under.escape()),
        (VapourEscape::Bound, VapourEscape::Outflow)
    );
    let critical =
        |t_k: f64| lava_world(t_k, MeltComposition::BulkSilicate).critical_jeans_parameter();
    assert_same(critical(3_000.0), ATOMIC_CRITICAL_JEANS_PARAMETER);
    assert_same(critical(3_500.0), DIATOMIC_CRITICAL_JEANS_PARAMETER);
    // At 3,500 K a diatomic vapour at λ 2.2, bound were it atomic, streams off.
    let hot = vapour(3_500.0, MeltComposition::BulkSilicate, 0.254, 1.0);
    let m = hot.mean_molar_mass_g_per_mol() * ATOMIC_MASS_CONSTANT_KG;
    let r = 2.2 * BOLTZMANN_CONSTANT * 3_500.0 / (0.254 * m);
    assert_eq!(
        vapour(3_500.0, MeltComposition::BulkSilicate, 0.254, r).escape(),
        VapourEscape::Outflow
    );
}

/// A melt's class follows its silica and alkalis: SF09's Table 1 melts read as themselves, and
/// the crust's lithologies by their melt records, a row with none read as the BSE.
#[test]
fn the_composition_class_follows_the_melt_s_silica_and_alkalis() {
    let sf09 = |si: f64, na: f64, k: f64| {
        MeltComposition::of_oxides(&[(Oxide::Silica, si), (Oxide::Soda, na), (Oxide::Potash, k)])
    };
    assert_eq!(sf09(45.97, 0.35, 0.04), MeltComposition::BulkSilicate);
    assert_eq!(sf09(50.36, 2.77, 0.13), MeltComposition::OceanicCrust);
    assert_eq!(sf09(62.93, 3.27, 2.45), MeltComposition::ContinentalCrust);
    let class = |lithology: CrustLithology| MeltComposition::of_row(lithology.substance());
    assert_eq!(class(CrustLithology::Basalt), MeltComposition::OceanicCrust);
    assert_eq!(
        class(CrustLithology::Granite),
        MeltComposition::ContinentalCrust
    );
    for lithology in [
        CrustLithology::Komatiite,
        CrustLithology::Ultramafic,
        CrustLithology::IronRichBasalt,
        CrustLithology::ReducedBasalt,
        CrustLithology::Feldspathic,
        CrustLithology::ChondriteH,
        CrustLithology::Cometary,
        CrustLithology::Water,
    ] {
        assert_eq!(
            class(lithology),
            MeltComposition::BulkSilicate,
            "{lithology:?}"
        );
    }
}

/// An input that is not finite and positive is refused, the first in order.
#[test]
fn a_vapour_s_inputs_out_of_range_are_refused() {
    let inputs = SilicateVapourInputs {
        melt_temperature: Kelvin::new(2_000.0),
        composition: MeltComposition::BulkSilicate,
        gravity: MetresPerSecondSquared::new(9.8),
        radius: Metres::new(6.4e6),
    };
    assert!(SilicateVapour::derive(&inputs).is_ok());
    let refused = |inputs: SilicateVapourInputs| SilicateVapour::derive(&inputs).unwrap_err();
    assert!(matches!(
        refused(SilicateVapourInputs {
            melt_temperature: Kelvin::new(f64::NAN),
            gravity: MetresPerSecondSquared::new(0.0),
            ..inputs
        }),
        DeriveSilicateVapourError::MeltTemperature(t) if t.is_nan()
    ));
    assert_eq!(
        refused(SilicateVapourInputs {
            gravity: MetresPerSecondSquared::new(-1.0),
            ..inputs
        }),
        DeriveSilicateVapourError::Gravity(-1.0)
    );
    assert_eq!(
        refused(SilicateVapourInputs {
            radius: Metres::new(f64::INFINITY),
            ..inputs
        }),
        DeriveSilicateVapourError::Radius(f64::INFINITY)
    );
    assert_eq!(
        DeriveSilicateVapourError::Gravity(-1.0).to_string(),
        "surface gravity -1 m s⁻² is not finite and positive"
    );
}

/// The vapour pinned by its bits (the sim-determinism skill's goldens), the same twice: at ten
/// melt temperatures from 1,300 to 7,000 K, nodes and the eruption temperatures among them, for
/// each class, on a lava world, its pressure, its species, their mean molar mass, its λ and its
/// class of escape.
#[test]
fn the_silicate_vapour_is_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    let write = || {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for composition in [
            MeltComposition::BulkSilicate,
            MeltComposition::OceanicCrust,
            MeltComposition::ContinentalCrust,
        ] {
            for t_k in [
                1_300.0, 1_473.15, 1_613.15, 2_000.0, 2_250.0, 2_500.0, 3_000.0, 3_500.0, 4_000.0,
                7_000.0,
            ] {
                let v = lava_world(t_k, composition);
                let label = format!("{composition:?} {t_k} K");
                w.f64(&format!("{label} pressure_pa"), v.pressure().value());
                for &(id, x) in v.species() {
                    w.f64(&format!("{label} {}", id.substance().key().as_str()), x);
                }
                w.f64(
                    &format!("{label} mean_molar_mass_g_per_mol"),
                    v.mean_molar_mass_g_per_mol(),
                );
                w.f64(&format!("{label} jeans_parameter"), v.jeans_parameter());
                w.line(&format!("{label} escape: {:?}", v.escape()));
            }
        }
        w
    };
    let first = write();
    assert_eq!(first.as_str(), write().as_str());
    golden!("planetary/silicate_vapour", first.as_str());
}
