//! The space-weathering law's tests (plan R10, R10.T1.d): the identity where nothing weathers, no
//! rise with maturity or iron, the reddening, no brightening and no energy albedo above 1, τ linear
//! in m and f, and the palette entry's law and escape valve. The refusals are in the crate's
//! `tests/panics.rs` (`mod weathering`); the law's bits on a grid are the material golden's.

use hyperion_testkit::float::assert_same_bits;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::{MechanicsFamily, PaletteRole};
use crate::substance_key::SubstanceKey;
use hyperion_base::units::Kelvin;

/// The two particulate laws, whose forms weather.
const WEATHERING: [PhaseLaw; 2] = [PhaseLaw::Regolith, PhaseLaw::Ice];

/// A host's grain density, kg m⁻³.
const DENSITY: f64 = 3_000.0;

/// Fresh albedos spanning the forms' tables, past their tops included.
const FRESH: [f64; 10] = [0.0, 0.01, 0.05, 0.093, 0.2, 0.41, 0.667, 0.9, 1.0, 1.2];

/// Wavelengths, nm, inside and outside iron's grid.
const WAVELENGTHS: [f64; 8] = [250.0, 300.0, 440.0, 550.0, 640.0, 1_000.0, 2_200.0, 4_500.0];

/// A silicate entry of fresh albedo `fresh` in every band, phase code `phase_row` and `FeO_T`
/// `feo`.
fn entry(fresh: f64, phase_row: u8, mechanics: MechanicsFamily, feo: Option<f64>) -> PaletteEntry {
    PaletteEntry {
        substance: SubstanceKey::new_const("basalt_fe"),
        role: PaletteRole::PrimaryCrust,
        normal_albedo_bvr: [fresh, fresh * 1.1, fresh * 1.2],
        phase_row,
        grain_density_kg_m3: 3_310.0,
        transition: Kelvin::new(1_323.15),
        mechanics,
        liquid: None,
        feo_mass_fraction: feo,
    }
}

#[test]
fn weathering_is_the_identity_at_zero_maturity_or_iron_and_under_lambert() {
    for law in PhaseLaw::ALL.iter().copied() {
        for fresh in FRESH {
            for wavelength in WAVELENGTHS {
                for (feo, maturity) in [(0.16, 0.0), (0.0, 1.0), (0.0, 0.0), (0.16, -0.0)] {
                    let normal =
                        weathered_normal_albedo(fresh, wavelength, law, DENSITY, feo, maturity);
                    assert_same_bits(normal, fresh);
                    let sphere =
                        weathered_spherical_albedo(fresh, wavelength, law, DENSITY, feo, maturity);
                    assert_same_bits(sphere, fresh);
                    let plane = [fresh, fresh * 0.5, fresh * 0.25, 0.0]
                        .map(|a| if a > 1.0 { 1.0 } else { a });
                    let hemispherical = weathered_hemispherical_albedos(
                        plane, wavelength, law, DENSITY, feo, maturity,
                    );
                    for (a, b) in hemispherical.into_iter().zip(plane) {
                        assert_same_bits(a, b);
                    }
                    let tau = npfe_optical_depth(law, wavelength, DENSITY, feo, maturity);
                    assert!(tau.abs() <= 0.0, "{law:?}: τ {tau}");
                }
                // Lambert's form never weathers, whatever the iron and the maturity.
                assert_same_bits(
                    weathered_normal_albedo(
                        fresh,
                        wavelength,
                        PhaseLaw::Lambert,
                        DENSITY,
                        1.0,
                        1.0,
                    ),
                    fresh,
                );
                assert_same_bits(
                    npfe_optical_depth(PhaseLaw::Lambert, wavelength, DENSITY, 1.0, 1.0),
                    0.0,
                );
            }
        }
    }
}

#[test]
fn weathering_is_the_identity_for_an_opaque_grain() {
    for (law, form) in [(PhaseLaw::Regolith, &REGOLITH), (PhaseLaw::Ice, &FROST)] {
        let [s_e, _] = form.slab;
        for share in [0.0, 0.25, 0.5, 0.99] {
            let w = s_e * share;
            let normal = form.albedo_at(Quantity::Normal, w);
            let sphere = form.albedo_at(Quantity::Spherical, w);
            for wavelength in WAVELENGTHS {
                assert!(npfe_optical_depth(law, wavelength, DENSITY, 1.0, 1.0) > 0.0);
                assert_same_bits(
                    weathered_normal_albedo(normal, wavelength, law, DENSITY, 1.0, 1.0),
                    normal,
                );
                assert_same_bits(
                    weathered_spherical_albedo(sphere, wavelength, law, DENSITY, 1.0, 1.0),
                    sphere,
                );
            }
        }
    }
}

#[test]
fn weathering_does_not_rise_with_maturity_or_iron() {
    for law in WEATHERING {
        for fresh in FRESH {
            for wavelength in WAVELENGTHS {
                let mut previous = fresh;
                for i in 0..=20_u32 {
                    let m = f64::from(i) / 20.0;
                    let a = weathered_normal_albedo(fresh, wavelength, law, DENSITY, 0.16, m);
                    assert!(
                        a <= previous,
                        "{law:?} {fresh} at {wavelength} nm, m {m}: {a}"
                    );
                    previous = a;
                }
                let mut previous = fresh;
                for i in 0..=20_u32 {
                    let f = f64::from(i) / 20.0;
                    let a = weathered_spherical_albedo(fresh, wavelength, law, DENSITY, f, 1.0);
                    assert!(
                        a <= previous,
                        "{law:?} {fresh} at {wavelength} nm, f {f}: {a}"
                    );
                    previous = a;
                }
            }
        }
    }
}

#[test]
fn weathering_darkens_blue_more_than_red() {
    let [b, _, r] = BVR_EFFECTIVE_WAVELENGTHS_NM;
    for (law, greys) in [
        (PhaseLaw::Regolith, [0.1, 0.2, 0.4, 0.667]),
        (PhaseLaw::Ice, [0.3, 0.5, 0.7, 0.9]),
    ] {
        for fresh in greys {
            for maturity in [0.2, 0.5, 1.0] {
                for feo in [0.055, 0.16] {
                    let blue = weathered_normal_albedo(fresh, b, law, DENSITY, feo, maturity);
                    let red = weathered_normal_albedo(fresh, r, law, DENSITY, feo, maturity);
                    assert!(
                        blue < red && red < fresh,
                        "{law:?} {fresh}, m {maturity}, f {feo}: B {blue}, R {red}"
                    );
                }
            }
        }
        assert!(
            npfe_optical_depth(law, b, DENSITY, 0.16, 1.0)
                > npfe_optical_depth(law, r, DENSITY, 0.16, 1.0)
        );
    }
}

#[test]
fn weathering_never_brightens_and_keeps_a_h_within_one() {
    for law in WEATHERING {
        for fresh in FRESH {
            for wavelength in WAVELENGTHS {
                for (feo, maturity) in [(0.02, 0.1), (0.16, 0.5), (0.2, 1.0), (1.0, 1.0)] {
                    let normal =
                        weathered_normal_albedo(fresh, wavelength, law, DENSITY, feo, maturity);
                    assert!((0.0..=fresh).contains(&normal), "{normal} from {fresh}");
                    let sphere =
                        weathered_spherical_albedo(fresh, wavelength, law, DENSITY, feo, maturity);
                    assert!((0.0..=fresh).contains(&sphere), "{sphere} from {fresh}");
                    let plane = [1.0, 0.98, 0.9, 0.7].map(|a| a * fresh / 1.2);
                    let hemispherical = weathered_hemispherical_albedos(
                        plane, wavelength, law, DENSITY, feo, maturity,
                    );
                    for (a, b) in hemispherical.into_iter().zip(plane) {
                        assert!((0.0..=b).contains(&a) && a <= 1.0, "{a} from {b}");
                    }
                }
            }
        }
    }
    // A lossless grain's A_h, 1, weathers below 1.
    let lossless = weathered_hemispherical_albedos([1.0; 4], 550.0, PhaseLaw::Ice, 917.0, 0.1, 1.0);
    assert!(lossless.iter().all(|&a| a < 1.0), "{lossless:?}");
}

#[test]
fn weathering_s_optical_depth_is_linear_in_maturity_and_iron() {
    for law in WEATHERING {
        for wavelength in WAVELENGTHS {
            let whole = npfe_optical_depth(law, wavelength, DENSITY, 0.16, 1.0);
            assert!(whole > 0.0 && whole.is_finite());
            // Halving is exact in binary.
            assert_same_bits(
                npfe_optical_depth(law, wavelength, DENSITY, 0.16, 0.5),
                0.5 * whole,
            );
            assert_same_bits(
                npfe_optical_depth(law, wavelength, DENSITY, 0.08, 1.0),
                0.5 * whole,
            );
            for share in [0.1, 0.3, 0.7] {
                let by_m = npfe_optical_depth(law, wavelength, DENSITY, 0.16, share);
                let by_f = npfe_optical_depth(law, wavelength, DENSITY, 0.16 * share, 1.0);
                for tau in [by_m, by_f] {
                    assert!(
                        (tau / (share * whole) - 1.0).abs() < 1e-15,
                        "{law:?} at {wavelength} nm: {tau} against {share} × {whole}"
                    );
                }
            }
        }
        // Beyond the grid's ends z is held, so τ goes as 1 ÷ λ there.
        let last = f64::from(u32::try_from(table::REGOLITH_IRON_Z.len() - 1).unwrap());
        let ends = [
            (100.0, table::IRON_Z_FIRST_NM),
            (
                8_000.0,
                table::IRON_Z_FIRST_NM + last * table::IRON_Z_STEP_NM,
            ),
        ];
        for (outside, end) in ends {
            let ratio = npfe_optical_depth(law, outside, DENSITY, 0.16, 1.0)
                / npfe_optical_depth(law, end, DENSITY, 0.16, 1.0);
            assert!((ratio * outside / end - 1.0).abs() < 1e-14, "{ratio}");
        }
    }
}

#[test]
fn weathering_an_entry_without_iron_keeps_its_albedo_to_the_bit() {
    for maturity in [0.0, 0.5, 1.0] {
        for feo in [None, Some(0.0)] {
            let e = entry(0.2, 0, MechanicsFamily::Silicate, feo);
            let weathered = weathered_entry_bvr(&e, maturity);
            for (a, b) in weathered.into_iter().zip(e.normal_albedo_bvr) {
                assert_same_bits(a, b);
            }
        }
    }
    // An iron-bearing silicate under the family's Regolith law darkens in every band.
    let rock = entry(0.2, 0, MechanicsFamily::Silicate, Some(0.16));
    let weathered = weathered_entry_bvr(&rock, 1.0);
    for (band, wavelength) in BVR_EFFECTIVE_WAVELENGTHS_NM.into_iter().enumerate() {
        let fresh = rock.normal_albedo_bvr[band];
        assert_same_bits(
            weathered[band],
            weathered_normal_albedo(fresh, wavelength, PhaseLaw::Regolith, 3_310.0, 0.16, 1.0),
        );
        assert!(weathered[band] < fresh);
    }
    // A code names its law: Lambert's is the identity, the Ice law the frost's; an unknown code
    // takes the family's.
    let lambert = entry(
        0.2,
        u8::from(PhaseLaw::Lambert),
        MechanicsFamily::Silicate,
        Some(0.16),
    );
    for (a, b) in weathered_entry_bvr(&lambert, 1.0)
        .into_iter()
        .zip(lambert.normal_albedo_bvr)
    {
        assert_same_bits(a, b);
    }
    let ice = entry(0.5, 0, MechanicsFamily::WaterIce, Some(0.01));
    let unknown = entry(0.5, 200, MechanicsFamily::WaterIce, Some(0.01));
    for (band, wavelength) in BVR_EFFECTIVE_WAVELENGTHS_NM.into_iter().enumerate() {
        let want = weathered_normal_albedo(
            ice.normal_albedo_bvr[band],
            wavelength,
            PhaseLaw::Ice,
            3_310.0,
            0.01,
            0.7,
        );
        assert_same_bits(weathered_entry_bvr(&ice, 0.7)[band], want);
        assert_same_bits(weathered_entry_bvr(&unknown, 0.7)[band], want);
    }
}

#[test]
fn weathering_s_table_is_one_the_law_can_read() {
    let nodes = &table::W;
    assert_same_bits(nodes[0], 0.0);
    assert_same_bits(nodes[nodes.len() - 1], 1.0);
    assert!(nodes.windows(2).all(|pair| pair[0] < pair[1]));
    for form in [&REGOLITH, &FROST] {
        assert_eq!(form.normal.len(), nodes.len());
        assert_eq!(form.spherical.len(), nodes.len());
        assert_eq!(form.hemispherical.len(), nodes.len());
        for quantity in [
            Quantity::Normal,
            Quantity::Spherical,
            Quantity::Hemispherical(0),
            Quantity::Hemispherical(1),
            Quantity::Hemispherical(2),
            Quantity::Hemispherical(3),
        ] {
            assert_same_bits(form.column(quantity, 0), 0.0);
            for k in 1..nodes.len() {
                assert!(form.column(quantity, k) > form.column(quantity, k - 1));
            }
            // A node's own albedo inverts to the node.
            for k in [1, 40, 100, 127] {
                let back = form.grain_albedo_of(quantity, form.column(quantity, k));
                assert!(
                    (back - nodes[k]).abs() < 1e-15,
                    "{back} against {}",
                    nodes[k]
                );
            }
        }
        let [s_e, s_i] = form.slab;
        assert!(0.0 < s_e && s_e < s_i && s_i < 1.0, "{s_e} {s_i}");
        assert!(form.lambda_m > 0.0 && form.lambda_m.is_finite());
        assert!(form.iron_z.iter().all(|&z| z > 0.0));
    }
    for (cosine, expected) in table::HEMISPHERICAL_INCIDENCE_COSINES
        .into_iter()
        .zip([1.0, 0.5, 0.2, 0.05])
    {
        assert_same_bits(cosine, expected);
    }
    assert_same_bits(table::IRON_Z_FIRST_NM, 300.0);
    assert_same_bits(table::IRON_Z_STEP_NM, 10.0);
}

/// The mature colour is held until the 30 → 31 bump: the rules in force are the slab's, and the
/// tests' switch reaches 31's.
#[test]
fn the_mature_colour_is_held_only_until_the_31_bump() {
    assert!(
        GENERATOR_VERSION < MATURE_COLOUR_VERSION,
        "the 30 → 31 bump draws a weathered entry's colour toward the mature lunar colour: delete \
         `MATURE_COLOUR_VERSION`, `MatureColourRules` with its `Slab` arm and its match in \
         `weathered_entry_bvr`, the tests' `with_mature_colour`, and this test; have \
         `under_the_mature_colour_the_moon_s_terrains_take_its_colour` read the version's rules \
         alone"
    );
    assert_eq!(MatureColourRules::in_force(), MatureColourRules::Slab);
    assert_eq!(
        with_mature_colour(MatureColourRules::Mature, MatureColourRules::in_force),
        MatureColourRules::Mature
    );
}

/// The mature colour keeps the slab's albedo to the bit at τ 0, outside the law's measured range
/// and for an albedo of 0; a fully weathered grain (τ 60) takes the mature lunar colour on V; and
/// it never passes the fresh albedo.
#[test]
fn toward_the_mature_colour_a_fresh_grain_keeps_its_own_and_a_saturated_one_takes_the_moon_s() {
    let v = (0.05, 550.0);
    for (weathered, wavelength, tau) in [
        (0.04, 440.0, 0.0),
        (0.04, 300.0, 3.0),
        (0.04, 1_000.0, 3.0),
        (0.0, 440.0, 3.0),
    ] {
        assert_same_bits(
            toward_mature_colour(weathered, 0.4, wavelength, tau, v),
            weathered,
        );
    }
    for wavelength in [440.0, 640.0, 900.0] {
        let mature = toward_mature_colour(0.03, 0.4, wavelength, 60.0, v);
        let moon = 0.05 * hyperion_base::math::powf(wavelength / 550.0, MATURE_COLOUR_EXPONENT);
        assert!((mature - moon).abs() < 1e-12, "{wavelength}: {mature}");
    }
    assert_same_bits(toward_mature_colour(0.03, 0.04, 640.0, 60.0, v), 0.04);
    // Between the two, the slab's colour in e^(−τ) of the light.
    let half = toward_mature_colour(0.03, 0.4, 440.0, core::f64::consts::LN_2, v);
    let moon = 0.05 * hyperion_base::math::powf(0.8, MATURE_COLOUR_EXPONENT);
    assert!((half - (0.03 * moon).sqrt()).abs() < 1e-12, "{half}");
}

/// The Moon's terrains under the mature colour, Λ's fit's anchor rows at their anchor soils'
/// `FeO_T` (the table's acceptance: the fresh `feldspathic`, 0.613/0.667/0.698 at 0.055 and
/// 2,900 kg m⁻³, and `basalt_fe`, 0.367/0.410/0.425 at 0.16 and 3,310 kg m⁻³) at m = 1: their V is
/// the slab's to the bit; both are red-sloped, R over V above 1.10, and B over V between Mercury's
/// 0.739 (Mallama, Krobusek and Pavlov 2017, Icarus 282, 19, Table 7) and the bluest mare's 0.860
/// (Gehrels, Coffeen and Owings 1964, Table XIII); the maria within 1% of the Moon's 0.831 and
/// 1.134 (Dollfus and Bowell's eq. 11), where the slab's are 0.911 and 1.056; the highlands, 0.715
/// under the slab, stated against the Moon's. A fresh entry (m = 0) keeps its albedos to the bit.
#[test]
fn under_the_mature_colour_the_moon_s_terrains_take_its_colour() {
    let anchor = |fresh: [f64; 3], feo: f64, density: f64| PaletteEntry {
        normal_albedo_bvr: fresh,
        grain_density_kg_m3: density,
        ..entry(0.2, 0, MechanicsFamily::Silicate, Some(feo))
    };
    let highland = anchor([0.613, 0.667, 0.698], 0.055, 2_900.0);
    let mare = anchor([0.367, 0.410, 0.425], 0.16, 3_310.0);
    let moon = |nm: f64| hyperion_base::math::powf(nm / 550.0, MATURE_COLOUR_EXPONENT);
    let [b_nm, _, r_nm] = BVR_EFFECTIVE_WAVELENGTHS_NM;
    for (name, terrain) in [("highland", highland), ("mare", mare)] {
        let slab = weathered_entry_bvr(&terrain, 1.0);
        let (mature, fresh) = with_mature_colour(MatureColourRules::Mature, || {
            (
                weathered_entry_bvr(&terrain, 1.0),
                weathered_entry_bvr(&terrain, 0.0),
            )
        });
        assert_same_bits(mature[1], slab[1]);
        for (a, b) in fresh.into_iter().zip(terrain.normal_albedo_bvr) {
            assert_same_bits(a, b);
        }
        let (b, r) = (mature[0] / mature[1], mature[2] / mature[1]);
        assert!(r > 1.10 && (0.739..=0.860).contains(&b), "{name}: {b}, {r}");
        if name == "mare" {
            assert!((b / moon(b_nm) - 1.0).abs() < 0.01, "{b}");
            assert!((r / moon(r_nm) - 1.0).abs() < 0.01, "{r}");
        }
        eprintln!(
            "{name} at m = 1: B/V {b:.3}, R/V {r:.3} against the Moon's {:.3} and {:.3} (the slab's \
             {:.3} and {:.3})",
            moon(b_nm),
            moon(r_nm),
            slab[0] / slab[1],
            slab[2] / slab[1]
        );
    }
}
