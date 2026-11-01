//! The thermal columns' and the regolith law's tests (P14.T49.j).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::substance::by_key;

/// A condensed row with no thermal columns, of a family with a form: quenched lava glass.
const NO_COLUMNS_EXAMPLE: &str = "lava_glass";

/// The row of `key`, which the registry holds.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a registry row"))
}

/// The inertia of `key` at its family's reference form at `t` kelvin.
fn reference(key: &str, t: f64) -> RegolithInertia {
    reference_regolith_thermal_inertia(row(key), Kelvin::new(t))
        .unwrap_or_else(|e| panic!("{key} at {t} K: {e}"))
}

/// Ice Ih's specific heat reproduces IAPWS R10-06(2009)'s Table 6 check values to their twelve
/// printed digits, at the triple point, at the normal-pressure melting point and at 100 K and
/// 10⁸ Pa, and the column is the one-atmosphere value.
#[test]
fn substance_thermal_ice_ih_heat_capacity_matches_iapws_table_6() {
    for (t, p, expected) in [
        (273.16, 611.657, 0.209_678_431_622e4),
        (273.152_519, 101_325.0, 0.209_671_391_024e4),
        (100.0, 1e8, 0.866_333_195_517e3),
    ] {
        let c_p = ice_ih_heat_capacity(t, p);
        assert!(
            (c_p / expected - 1.0).abs() < 5e-12,
            "{c_p} J kg⁻¹ K⁻¹ at {t} K, {p} Pa, against {expected}"
        );
    }
    let ice = row("H2O").substance().thermal_columns().unwrap();
    for t in [20.0, 90.0, 200.0, 273.15] {
        assert_same_bits(
            ice.heat_capacity_j_per_kg_k(Kelvin::new(t)),
            ice_ih_heat_capacity(t, ONE_ATMOSPHERE_PA),
        );
    }
}

/// A column's read points return their own values to the bit, interpolate linearly between, and
/// hold their ends; a polynomial is Horner's rule over its coefficients; a NaN stays a NaN.
#[test]
fn substance_thermal_forms_evaluate_as_documented() {
    const CONSTANT: [ThermalPiece; 1] = [ThermalPiece {
        phase: "",
        low: Kelvin::new(10.0),
        high: Kelvin::new(20.0),
        form: ThermalForm::Constant(3.0),
    }];
    const PIECES: [ThermalPiece; 2] = [
        ThermalPiece {
            phase: "α",
            low: Kelvin::new(10.0),
            high: Kelvin::new(20.0),
            form: ThermalForm::Polynomial(&[1.0]),
        },
        ThermalPiece {
            phase: "β",
            low: Kelvin::new(20.0),
            high: Kelvin::new(30.0),
            form: ThermalForm::Polynomial(&[2.0]),
        },
    ];
    let points: &'static [(f64, f64)] = &[(100.0, 1.0), (200.0, 3.0), (300.0, 4.0)];
    let form = ThermalForm::Points(points);
    for &(t, v) in points {
        assert_same_bits(form.evaluate(t), v);
    }
    assert!((form.evaluate(150.0) - 2.0).abs() < 1e-15);
    assert_same_bits(form.evaluate(50.0), 1.0);
    assert_same_bits(form.evaluate(400.0), 4.0);
    assert!(form.evaluate(f64::NAN).is_nan());
    let cubic = ThermalForm::Polynomial(&[1.0, 2.0, 3.0, 4.0]);
    assert_same_bits(cubic.evaluate(2.0), 1.0 + 2.0 * 2.0 + 3.0 * 4.0 + 4.0 * 8.0);
    let reciprocal = ThermalForm::Reciprocal { a: 567.0, b: 0.5 };
    assert_same_bits(reciprocal.evaluate(90.0), 567.0 / 90.0 + 0.5);
    let column = ThermalColumn {
        pieces: &PIECES,
        provenance: Provenance::Own,
    };
    for (t, v) in [
        (5.0, 1.0),
        (15.0, 1.0),
        (20.0, 1.0),
        (25.0, 2.0),
        (40.0, 2.0),
    ] {
        assert_same_bits(column.at(Kelvin::new(t)), v);
    }
    assert!(column.at(Kelvin::new(f64::NAN)).is_nan());
    let constant = ThermalColumn {
        pieces: &CONSTANT,
        provenance: Provenance::Own,
    };
    assert_same_bits(constant.at(Kelvin::new(15.0)), 3.0);
    assert!(constant.at(Kelvin::new(f64::NAN)).is_nan());
}

/// The law's parts multiply back to Γ² within 10⁻¹² relative, `ρ_bulk` is the grain density times
/// one less the porosity to the bit, and `k_eff` is the contact term plus the radiative, for every
/// row with a form at every temperature its solid holds.
#[test]
fn substance_thermal_parts_multiply_back_to_the_inertia() {
    for (id, _) in rows() {
        let row = id.substance();
        let family = row.condensed().unwrap().mechanics();
        let Some(form) = RegolithForm::reference(family) else {
            continue;
        };
        let mut evaluated = 0;
        for t in [30.0, 60.0, 90.0, 150.0, 200.0, 273.0, 350.0] {
            let Ok(gamma) = regolith_thermal_inertia(id, &form, Kelvin::new(t)) else {
                continue;
            };
            evaluated += 1;
            let product = gamma.conductivity_w_per_m_k()
                * gamma.bulk_density().value()
                * gamma.heat_capacity_j_per_kg_k();
            let squared = gamma.inertia_j_per_m2_k_sqrt_s() * gamma.inertia_j_per_m2_k_sqrt_s();
            assert!(
                (squared / product - 1.0).abs() < 1e-12,
                "{:?} at {t} K",
                row.key()
            );
            let grain = row
                .grain_density(CondensedPhase::Solid, Kelvin::new(t))
                .unwrap();
            assert_same_bits(gamma.grain_density().value(), grain.value());
            assert_same_bits(
                gamma.bulk_density().value(),
                grain.value() * (1.0 - form.porosity()),
            );
            assert_same_bits(
                gamma.conductivity_w_per_m_k(),
                gamma.contact_conductivity_w_per_m_k() + gamma.radiative_conductivity_w_per_m_k(),
            );
        }
        assert!(
            evaluated >= 3,
            "{:?} holds a solid at most temperatures",
            row.key()
        );
    }
}

/// The radiative term scales as T³ and with the grain size, and the contact term with the grain
/// size to the −⅓ (Gundlach and Blum 2013, §3: "λ ∝ r^−1/3" for small grains, "λ ∝ r" for
/// large).
#[test]
fn substance_thermal_radiative_term_scales_as_t_cubed_and_with_grain_size() {
    let id = row("feldspathic");
    let form = RegolithForm::reference(MechanicsFamily::Silicate).unwrap();
    let coarse = RegolithForm::new(
        MechanicsFamily::Silicate,
        form.porosity(),
        Metres::new(8.0 * form.grain_diameter().value()),
    )
    .unwrap();
    let at =
        |form: &RegolithForm, t: f64| regolith_thermal_inertia(id, form, Kelvin::new(t)).unwrap();
    let (cool, warm) = (at(&form, 150.0), at(&form, 300.0));
    let ratio = warm.radiative_conductivity_w_per_m_k() / cool.radiative_conductivity_w_per_m_k();
    assert!((ratio - 8.0).abs() < 1e-12, "{ratio}");
    let (fine, big) = (at(&form, 250.0), at(&coarse, 250.0));
    let ratio = big.radiative_conductivity_w_per_m_k() / fine.radiative_conductivity_w_per_m_k();
    assert!((ratio - 8.0).abs() < 1e-12, "{ratio}");
    let ratio = big.contact_conductivity_w_per_m_k() / fine.contact_conductivity_w_per_m_k();
    assert!((ratio - 0.5).abs() < 1e-12, "{ratio}");
}

/// The contact term rises as the porosity falls, at every family with a form.
#[test]
fn substance_thermal_contact_term_rises_as_the_porosity_falls() {
    for (key, t) in [("feldspathic", 250.0), ("H2O", 90.0)] {
        let id = row(key);
        let family = id.substance().condensed().unwrap().mechanics();
        let reference = RegolithForm::reference(family).unwrap();
        let mut last = 0.0;
        for porosity in [0.95, 0.85, 0.7, 0.5, 0.3, 0.1] {
            let form = RegolithForm::new(family, porosity, reference.grain_diameter()).unwrap();
            let contact = regolith_thermal_inertia(id, &form, Kelvin::new(t))
                .unwrap()
                .contact_conductivity_w_per_m_k();
            assert!(
                contact > last,
                "{key} at a porosity of {porosity}: {contact}"
            );
            last = contact;
        }
    }
}

/// A regolith's inertia is below its own dense solid's √(`k_s` `ρ_g` c) at every porosity above
/// 0, bedrock's thousands against a regolith's tens, at every row with a form.
#[test]
fn substance_thermal_regolith_is_below_its_solid() {
    for (id, columns) in rows() {
        let row = id.substance();
        let family = row.condensed().unwrap().mechanics();
        let Some(reference) = RegolithForm::reference(family) else {
            continue;
        };
        let t = columns.heat_capacity().high().value().min(273.0);
        for porosity in [1e-6, 0.01, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99] {
            let form = RegolithForm::new(family, porosity, reference.grain_diameter()).unwrap();
            let gamma = regolith_thermal_inertia(id, &form, Kelvin::new(t))
                .unwrap_or_else(|e| panic!("{:?} at {t} K: {e}", row.key()));
            assert!(
                gamma.inertia_j_per_m2_k_sqrt_s() < gamma.solid_inertia_j_per_m2_k_sqrt_s(),
                "{:?} at a porosity of {porosity}",
                row.key()
            );
        }
    }
}

/// Each error: a gas row, a temperature that is none, a row above its transition, a row without
/// thermal columns, a family without a form, a form of another family, and a form's bad
/// porosity, grain size or family.
#[test]
fn substance_thermal_errors_name_their_cause() {
    let silicate = RegolithForm::reference(MechanicsFamily::Silicate).unwrap();
    let helium = row("He");
    assert_eq!(
        regolith_thermal_inertia(helium, &silicate, Kelvin::new(4.0)),
        Err(EvaluateRegolithInertiaError::NotCondensed { id: helium })
    );
    assert_eq!(
        reference_regolith_thermal_inertia(helium, Kelvin::new(4.0)),
        Err(EvaluateRegolithInertiaError::NotCondensed { id: helium })
    );
    let basalt = row("basalt");
    for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            regolith_thermal_inertia(basalt, &silicate, Kelvin::new(t)),
            Err(EvaluateRegolithInertiaError::Temperature { .. })
        ));
    }
    let hot = Kelvin::new(1_300.0);
    assert_eq!(
        regolith_thermal_inertia(basalt, &silicate, hot),
        Err(EvaluateRegolithInertiaError::NoSolid {
            id: basalt,
            temperature: hot
        })
    );
    let water = row("H2O");
    let thaw = Kelvin::new(280.0);
    assert_eq!(
        reference_regolith_thermal_inertia(water, thaw),
        Err(EvaluateRegolithInertiaError::NoSolid {
            id: water,
            temperature: thaw
        })
    );
    let unmodelled = row(NO_COLUMNS_EXAMPLE);
    assert_eq!(
        regolith_thermal_inertia(unmodelled, &silicate, Kelvin::new(200.0)),
        Err(EvaluateRegolithInertiaError::NoThermalColumns { id: unmodelled })
    );
    let iron = row("Fe");
    assert_eq!(
        reference_regolith_thermal_inertia(iron, Kelvin::new(200.0)),
        Err(EvaluateRegolithInertiaError::NoRegolithForm {
            family: MechanicsFamily::Metal
        })
    );
    assert_eq!(
        regolith_thermal_inertia(water, &silicate, Kelvin::new(90.0)),
        Err(EvaluateRegolithInertiaError::FamilyMismatch {
            row: MechanicsFamily::WaterIce,
            form: MechanicsFamily::Silicate
        })
    );
    let d = silicate.grain_diameter();
    for porosity in [0.0, 1.0, -0.1, f64::NAN] {
        assert!(matches!(
            RegolithForm::new(MechanicsFamily::Silicate, porosity, d),
            Err(BuildRegolithFormError::Porosity { .. })
        ));
    }
    for diameter in [0.0, -1e-6, f64::INFINITY, f64::NAN] {
        assert!(matches!(
            RegolithForm::new(MechanicsFamily::Silicate, 0.5, Metres::new(diameter)),
            Err(BuildRegolithFormError::GrainDiameter { .. })
        ));
    }
    assert_eq!(
        RegolithForm::new(MechanicsFamily::Metal, 0.5, d),
        Err(BuildRegolithFormError::NoForm {
            family: MechanicsFamily::Metal
        })
    );
}

/// The Moon's highlands, `feldspathic` at the silicate form, lie within 45–65 J m⁻² K⁻¹ s^−½ at
/// 273.15 K, Hayne et al. 2017's own convention of 273 K taken as 0 °C: "55 ± 2" over the top
/// 4–7 cm, at 273 K (JGR Planets 122, 2371, abstract).
#[test]
fn substance_thermal_moon_s_highlands_match_diviner() {
    let gamma = reference("feldspathic", 273.15).inertia_j_per_m2_k_sqrt_s();
    assert!((45.0..=65.0).contains(&gamma), "{gamma}");
}

/// A Ceres-like `phyllosilicate` regolith, at the silicate form, lies within 15–60 J m⁻² K⁻¹
/// s^−½, the span from Spencer 1990's 15 through Saint-Pé et al. 1993's 38 ± 14 to Rognini et
/// al.'s "up to 60" over ordinary terrain (Li, Moullet, Titus, Hsieh and Sykes 2020, AJ,
/// arXiv:2003.11045, §1): at 235 K, the temperature Saint-Pé et al.'s value is given at (Gundlach
/// and Blum 2013, Table 3), and at 173 K, Li et al.'s brightness temperature at 1.1 mm (§4).
#[test]
fn substance_thermal_ceres_phyllosilicate_matches_its_inertias() {
    for t in [235.0, 173.0] {
        let gamma = reference("phyllosilicate", t).inertia_j_per_m2_k_sqrt_s();
        assert!((15.0..=60.0).contains(&gamma), "at {t} K: {gamma}");
    }
}

/// An icy satellite's `H2O` regolith, at the icy form, lies within 5–70 J m⁻² K⁻¹ s^−½, the
/// plan's range from Howett, Spencer, Pearl and Segura 2010 (Icarus 206, 573, Cassini CIRS; not
/// re-read), at the temperatures the Saturnian moons' inertias were fitted at, which "vary between
/// 50 and more than 100 K" (Ferrari and Lucas 2016, A&A 588, A133, §3.1, whose Table 2 quotes
/// Howett et al.'s Enceladus 6–39, Tethys 5–19, Dione 5–29 and Rhea 3–20). The form's porosity is
/// main's ruling of 2026-10-16 on this issue, not tuned to the range. In the same range, as
/// cross-checks: Europa's, at the 67–148 K of its day and night (Lange et al. 2026,
/// arXiv:2604.14374, abstract: 56 ± 17 from Galileo PPR), and Pluto's, at 40–63 K (Lellouch et al.
/// 2011, Icarus 214, 701, abstract and §6.2: 20–30), though Pluto's ice is mostly nitrogen's.
#[test]
fn substance_thermal_icy_satellite_matches_cassini() {
    for t in [50.0, 60.0, 80.0, 100.0, 110.0, 67.0, 148.0, 40.0, 63.0] {
        let gamma = reference("H2O", t).inertia_j_per_m2_k_sqrt_s();
        assert!((5.0..=70.0).contains(&gamma), "H2O at {t} K: {gamma}");
    }
}

/// The silicate form reads R10's regolith of silicate through its public API: the 0–15 cm
/// layer's porosity, 1 − 1.50 ÷ 3.1, the narrowest from the surface at either end of the Moon's
/// 4–7 cm skin depth.
#[test]
fn substance_thermal_silicate_form_is_r10_s_regolith() {
    let form = RegolithForm::reference(MechanicsFamily::Silicate).unwrap();
    assert_same_bits(form.porosity(), 1.0 - 1.50 / 3.1);
    let shear = form_row(MaterialClass::Regolith, MechanicsFamily::Silicate).shear();
    for depth in [0.04, 0.07] {
        let layer = narrowest_surface_layer(shear, Metres::new(depth)).unwrap();
        assert_same_bits(layer.porosity(), form.porosity());
        assert_same_bits(layer.bottom().value(), 0.15);
    }
    assert_same_bits(form.emissivity(), REGOLITH_EMISSIVITY);
    assert_same_bits(form.grain_diameter().value(), 70e-6);
}

/// Every row with thermal columns is a registry row with condensed columns, in registry order,
/// once each, and [`rows`] lists them so.
#[test]
fn substance_thermal_rows_are_registry_rows_in_order() {
    let mut last: Option<SubstanceId> = None;
    for (key, _) in rows::THERMAL_ROWS {
        let id = by_key(key.as_str()).unwrap_or_else(|| panic!("{key} is a registry row"));
        assert!(id.substance().condensed().is_some(), "{key} is condensed");
        if let Some(last) = last {
            assert!(
                id > last,
                "{key} follows the row before it in registry order"
            );
        }
        last = Some(id);
    }
    let listed: Vec<SubstanceId> = rows().map(|(id, _)| id).collect();
    let table: Vec<SubstanceId> = rows::THERMAL_ROWS
        .iter()
        .map(|(key, _)| by_key(key.as_str()).unwrap())
        .collect();
    assert_eq!(listed, table);
}

/// Each column has a piece; each piece's range is finite, positive and increasing, each begins
/// where the last ends, and the column's top lies below the row's transition; read points lie in
/// increasing temperature from the piece's low to its high; and each value is finite and positive
/// over the range.
#[test]
fn substance_thermal_ranges_and_values_are_physical() {
    for (id, columns) in rows() {
        let row = id.substance();
        let key = row.key();
        let transition = row.condensed().unwrap().transition().temperature().value();
        for column in [columns.heat_capacity(), columns.conductivity()] {
            assert!(!column.pieces().is_empty(), "{key}");
            for (k, piece) in column.pieces().iter().enumerate() {
                let (low, high) = (piece.low().value(), piece.high().value());
                assert!(low.is_finite() && high.is_finite(), "{key}");
                assert!(low > 0.0 && low < high, "{key}: {low}–{high} K");
                if k > 0 {
                    assert_same_bits(low, column.pieces()[k - 1].high().value());
                }
                if let ThermalForm::Points(points) = piece.form() {
                    assert!(points.len() >= 2, "{key}");
                    assert!(points[0].0 >= low, "{key}");
                    assert!(points[points.len() - 1].0 <= high, "{key}");
                    assert!(points.windows(2).all(|w| w[0].0 < w[1].0), "{key}");
                }
                for i in 0..=200 {
                    let t = low + (high - low) * f64::from(i) / 200.0;
                    let v = column.at(Kelvin::new(t));
                    assert!(v.is_finite() && v > 0.0, "{key} at {t} K: {v}");
                }
            }
            assert!(column.high().value() < transition, "{key}");
        }
    }
}

/// A row with a phase diagram takes its diagram's solid phases: each piece names one of them, a
/// phase's pieces lie together in the diagram's order, and where a piece begins a new phase it
/// begins at the phase's start to the bit, so that the two cannot disagree on where a phase gives
/// way; a phase may hold several fitted pieces. A row without a diagram has one solid, unnamed.
#[test]
fn substance_thermal_pieces_follow_the_diagram_s_phases() {
    for (id, columns) in rows() {
        let row = id.substance();
        for column in [columns.heat_capacity(), columns.conductivity()] {
            let pieces = column.pieces();
            let Some(diagram) = row.phase_diagram() else {
                assert!(
                    pieces.iter().all(|p| p.phase().is_empty()),
                    "{:?}",
                    row.key()
                );
                continue;
            };
            let phases = diagram.solid_phases();
            let mut last: Option<usize> = None;
            for (k, piece) in pieces.iter().enumerate() {
                let index = phases
                    .iter()
                    .position(|phase| phase.name() == piece.phase())
                    .unwrap_or_else(|| panic!("{:?}: {}", row.key(), piece.phase()));
                if let Some(last) = last {
                    assert!(index == last || index == last + 1, "{:?}", row.key());
                    if index == last + 1 {
                        assert_same_bits(piece.low().value(), phases[index].start().value());
                    }
                } else if k == 0 && index > 0 {
                    assert!(piece.low() >= phases[index].start(), "{:?}", row.key());
                }
                last = Some(index);
            }
        }
    }
}

/// Every column names a source that covers it, with a basis; and each stand-in column's source
/// says it is one and is adopted.
#[test]
fn substance_thermal_columns_name_their_sources() {
    for (id, columns) in rows() {
        let key = id.substance().key();
        for s in columns.sources() {
            assert!(!s.citation().is_empty(), "{key}");
        }
        for (value, column) in [
            ("heat capacity", columns.heat_capacity()),
            ("conductivity", columns.conductivity()),
        ] {
            let covering: Vec<&Source> = columns
                .sources()
                .iter()
                .filter(|s| s.covers().split(", ").any(|covered| covered == value))
                .collect();
            assert!(!covering.is_empty(), "{key}'s {value} names a source");
            if column.is_stand_in() {
                assert!(
                    covering
                        .iter()
                        .any(|s| s.basis() == Basis::Adopted && s.citation().contains("stand-in")),
                    "{key}'s {value} is a stand-in, adopted and labelled so"
                );
            }
        }
    }
}

/// Whether `line` names `pattern` as a whole name: at the start of the line, or after a character
/// that cannot continue an identifier. So `isothermal::` (R09.T13's isothermal climate model) does
/// not read as `thermal::`. The substance module's tripwires read by it: `substance::aerosol`'s
/// and `substance::formation`'s, which hold to 24. This module's own, which held its inertia off
/// the record, P14.T54.a deleted when it wired the inertia at the lid; its walk
/// ([`rust_files`], [`out_of_line_test_modules`], [`before_inline_tests`]) stays theirs.
pub(in crate::substance) fn names(line: &str, pattern: &str) -> bool {
    line.match_indices(pattern).any(|(at, _)| {
        line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

#[test]
fn the_tripwire_reads_whole_names_only() {
    assert!(names(
        "use crate::substance::thermal::ThermalColumn;",
        "thermal::"
    ));
    assert!(names("thermal::regolith_thermal_inertia(x)", "thermal::"));
    assert!(names("let a = row.thermal_columns()?;", "thermal_columns("));
    assert!(!names(
        "let climate = isothermal::solve(inputs, &grid);",
        "thermal::"
    ));
    assert!(!names("geothermal::flux()", "thermal::"));
}

/// The rows P14.T49.j requires (its brief, as plan 14's record "Deviations in P14.T49.j, as
/// built" gives them): every row P14.T51.c can name as a crust, a frost R09 or R10 draws on an
/// airless or thin-aired world, or a solid key of R09's reference and synthetic worlds.
const REQUIRED: [&str; 30] = [
    "feldspathic",
    "anorthosite",
    "basalt",
    "basalt_fe",
    "basalt_reduced",
    "komatiite",
    "ultramafic",
    "granite",
    "ferric",
    "phyllosilicate",
    "ca_al_residue",
    "graphite",
    "chondrite_ci",
    "chondrite_cm",
    "chondrite_cv",
    "chondrite_h",
    "chondrite_l",
    "chondrite_ll",
    "chondrite_e",
    "cometary",
    "carbonaceous",
    "H2O",
    "CO2",
    "N2",
    "CH4",
    "CO",
    "Na2CO3",
    "mars_dust",
    "O2",
    "Ar",
];

/// The bullets' keys of the section headed `heading` in `rows.rs`'s module documentation: each
/// `` //! - `key`: … `` line until the next heading.
fn documented(heading: &str) -> Vec<String> {
    let text = include_str!("rows.rs");
    let mut keys = Vec::new();
    let mut inside = false;
    for line in text.lines().take_while(|line| line.starts_with("//!")) {
        if let Some(title) = line.strip_prefix("//! # ") {
            inside = title == heading;
            continue;
        }
        if inside && let Some(rest) = line.strip_prefix("//! - `") {
            let key = rest.split('`').next().expect("a bullet names its key");
            keys.push(key.to_owned());
        }
    }
    keys
}

/// The rows not modelled are exactly the condensed rows without thermal columns, in registry
/// order, and exactly the module documentation's list.
#[test]
fn substance_thermal_not_modelled_is_the_documented_list() {
    let unmodelled: Vec<String> = ids()
        .filter(|id| {
            let row = id.substance();
            row.condensed().is_some() && row.thermal_columns().is_none()
        })
        .map(|id| id.substance().key().as_str().to_owned())
        .collect();
    assert_eq!(unmodelled, rows::NOT_MODELLED);
    assert_eq!(documented("Not modelled"), rows::NOT_MODELLED);
}

/// The stand-ins are a closed list: the rows with a stand-in column, in registry order, are
/// exactly the module documentation's, each naming the column it stands in for.
#[test]
fn substance_thermal_stand_ins_are_the_documented_list() {
    let stand_ins: Vec<(String, &ThermalColumns)> = rows()
        .filter(|(_, columns)| columns.is_stand_in())
        .map(|(id, columns)| (id.substance().key().as_str().to_owned(), columns))
        .collect();
    let keys: Vec<&str> = stand_ins.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(documented("Stand-ins"), keys);
    let text = include_str!("rows.rs");
    for (key, columns) in &stand_ins {
        let bullet = text
            .lines()
            .find(|line| line.starts_with(&format!("//! - `{key}`:")))
            .unwrap();
        let both = bullet.contains("both columns");
        assert_eq!(
            columns.heat_capacity().is_stand_in(),
            both || bullet.contains("heat capacity"),
            "{bullet}"
        );
        assert_eq!(
            columns.conductivity().is_stand_in(),
            both || bullet.contains("conductivity"),
            "{bullet}"
        );
    }
}

/// Each required row has thermal columns or is in the closed list of rows not modelled, and the
/// required rows left out are exactly four, each with its reason and the record's lean for
/// P14.T54.a.
#[test]
fn substance_thermal_required_rows_are_modelled_or_listed() {
    let mut left_out = Vec::new();
    for key in REQUIRED {
        if row(key).substance().thermal_columns().is_none() {
            assert!(
                rows::NOT_MODELLED.contains(&key),
                "{key} is modelled or listed"
            );
            left_out.push(key);
        }
    }
    assert_eq!(
        left_out,
        ["ca_al_residue", "graphite", "carbonaceous", "Na2CO3"]
    );
}

/// The families without a regolith form are a closed list: metal, the volatile ices, salt and the
/// organics, none with cited grain-contact constants.
#[test]
fn substance_thermal_families_without_a_form_are_listed() {
    let without: Vec<MechanicsFamily> = MechanicsFamily::ALL
        .iter()
        .copied()
        .filter(|&family| RegolithForm::reference(family).is_none())
        .collect();
    assert_eq!(
        without,
        [
            MechanicsFamily::Metal,
            MechanicsFamily::VolatileIce,
            MechanicsFamily::Salt,
            MechanicsFamily::Organic
        ]
    );
}

/// A stand-in's column equals the one it borrows, which the table writes as the same named
/// constant, never as a copied number.
#[test]
fn substance_thermal_stand_ins_borrow_the_same_constant() {
    let columns = |key: &str| *row(key).substance().thermal_columns().unwrap();
    let pieces = |column: &ThermalColumn| column.pieces().to_vec();
    let c = |key: &str| pieces(columns(key).heat_capacity());
    let k = |key: &str| pieces(columns(key).conductivity());
    for (borrower, lender) in [("basalt", "basalt_fe"), ("basalt_reduced", "basalt_fe")] {
        assert_eq!(c(borrower), c(lender), "{borrower}");
    }
    for (borrower, lender) in [
        ("feldspathic", "anorthosite"),
        ("basalt_fe", "basalt"),
        ("basalt_reduced", "basalt"),
        ("komatiite", "ultramafic"),
        ("Mg2SiO4", "ultramafic"),
        ("chondrite_ci", "chondrite_cm"),
        ("chondrite_ll", "chondrite_h"),
        ("cometary", "chondrite_cm"),
    ] {
        assert_eq!(k(borrower), k(lender), "{borrower}");
    }
    for (borrower, lender) in [
        ("ferric", "basalt_fe"),
        ("mars_dust", "basalt_fe"),
        ("phyllosilicate", "chondrite_cm"),
    ] {
        assert_eq!(c(borrower), c(lender), "{borrower}");
        assert_eq!(k(borrower), k(lender), "{borrower}");
    }
}

/// Ice Ih's 567 ÷ T agrees with the measurements, as digitised by Wolfenbarger et al. 2021 (Data
/// in Brief, CC BY 4.0), within their stated errors: Klinger 1975's own crystals, "lower than
/// 12 %" (J. Glaciol. 14, 517), and Andersson and Suga 1994's within 5 per cent; asserted values,
/// a cross-check.
#[test]
fn substance_thermal_ice_ih_conductivity_matches_the_measurements() {
    let ice = row("H2O").substance().thermal_columns().unwrap();
    for (t, k, within) in [
        (62.9, 8.99, 0.12),
        (79.2, 7.09, 0.12),
        (100.0, 5.55, 0.12),
        (81.8, 7.14, 0.05),
        (98.2, 5.82, 0.05),
        (150.1, 3.85, 0.05),
        (199.8, 2.90, 0.05),
        (252.0, 2.17, 0.05),
    ] {
        let column = ice.conductivity_w_per_m_k(Kelvin::new(t));
        assert_same_bits(column, rows::ICE_IH_CONDUCTIVITY_A_W_PER_M / t);
        assert!(
            (column / k - 1.0).abs() < within,
            "{column} at {t} K against {k}"
        );
    }
}

/// The highland soil's specific heat agrees with Hayne et al. 2017's lunar polynomial within 4 per
/// cent from 100 to 350 K (JGR Planets 122, 2371, Table A1, read as arXiv:1711.00977: c₀ −3.6125,
/// c₁ 2.7431, c₂ 2.3616 × 10⁻³, c₃ −1.2340 × 10⁻⁵, c₄ 8.9093 × 10⁻⁹), and with Hemingway, Robie and
/// Wilson 1973's fit to their soils, "better than 10 percent between 90 and 350°K" (their eq. 1, in
/// J g⁻¹ K⁻¹); asserted cross-checks.
#[test]
fn substance_thermal_highland_soil_matches_the_lunar_fits() {
    let soil = row("feldspathic").substance().thermal_columns().unwrap();
    let hayne = [-3.6125, 2.7431, 2.3616e-3, -1.2340e-5, 8.9093e-9];
    let soils = [-2.3173e-2, 2.1270e-3, 1.5009e-5, -7.3699e-8, 9.6552e-11];
    let poly = |c: &[f64], t: f64| c.iter().rev().fold(0.0, |sum, &ci| sum * t + ci);
    for t in [100.0, 150.0, 200.0, 250.0, 300.0, 350.0] {
        let c = soil.heat_capacity_j_per_kg_k(Kelvin::new(t));
        let fit = poly(&hayne, t);
        assert!((c / fit - 1.0).abs() < 0.04, "{c} at {t} K against {fit}");
    }
    for t in [90.0, 100.0, 200.0, 300.0, 350.0] {
        let c = soil.heat_capacity_j_per_kg_k(Kelvin::new(t));
        let fit = 1000.0 * poly(&soils, t);
        assert!((c / fit - 1.0).abs() < 0.10, "{c} at {t} K against {fit}");
    }
}

/// The columns reproduce their sources where the sources print a value: each mineral's mode at its
/// read temperatures is its molar figure over its formula weight; the ordinary chondrites' class
/// polynomials give their Table 4's liquid-nitrogen `C_p` within 1 per cent, as close as Macke,
/// Consolmagno and Britt 2019 find that average to `C_p` at 175 K (482.9, 496.3 and 501.4 J kg⁻¹
/// K⁻¹); and Winter and Saari's form gives Groussin et al. 2019's 770 J kg⁻¹ K⁻¹ at 300 K within
/// its rounding.
#[test]
fn substance_thermal_columns_reproduce_their_printed_values() {
    let at = |key: &str, t: f64| {
        row(key)
            .substance()
            .thermal_columns()
            .unwrap()
            .heat_capacity_j_per_kg_k(Kelvin::new(t))
    };
    for (t, molar) in [(100.0, 32.26), (298.15, 118.6)] {
        assert_same_bits(at("Mg2SiO4", t), 1.0 * molar / (140.694 / 1000.0));
    }
    for (t, molar) in [(100.0, 23.21), (350.0, 90.35)] {
        assert_same_bits(at("MgSiO3", t), 1.0 * molar / (100.389 / 1000.0));
    }
    for (key, c) in [
        ("chondrite_h", 482.9),
        ("chondrite_l", 496.3),
        ("chondrite_ll", 501.4),
    ] {
        let model = at(key, 175.0);
        assert!((model / c - 1.0).abs() < 0.01, "{key}: {model} against {c}");
    }
    let dust = at("cometary", 300.0);
    assert!((dust - 770.0).abs() < 5.0, "{dust}");
}

/// The volatile ices' molar masses, with which their calorimetry is converted, are the registry
/// rows' to the bit.
#[test]
fn substance_thermal_volatile_molar_masses_are_the_registry_s() {
    for (key, molar_mass) in [
        ("CO2", rows::CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL),
        ("N2", rows::NITROGEN_MOLAR_MASS_G_PER_MOL),
        ("CH4", rows::METHANE_MOLAR_MASS_G_PER_MOL),
        ("CO", rows::CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL),
        ("O2", rows::OXYGEN_MOLAR_MASS_G_PER_MOL),
        ("Ar", rows::ARGON_MOLAR_MASS_G_PER_MOL),
    ] {
        assert_same_bits(
            row(key).substance().molar_mass_g_per_mol().unwrap(),
            molar_mass,
        );
    }
}

/// Each anchor's temperatures lie inside both of its row's column ranges, so that no anchor rests
/// on a held end value.
#[test]
fn substance_thermal_anchor_temperatures_lie_in_the_column_ranges() {
    for (key, temperatures) in [
        ("feldspathic", &[273.15][..]),
        ("phyllosilicate", &[235.0, 173.0][..]),
        (
            "H2O",
            &[50.0, 60.0, 80.0, 100.0, 110.0, 67.0, 148.0, 40.0, 63.0][..],
        ),
    ] {
        let columns = row(key).substance().thermal_columns().unwrap();
        for &t in temperatures {
            for column in [columns.heat_capacity(), columns.conductivity()] {
                assert!(
                    column.low().value() <= t && t <= column.high().value(),
                    "{key} at {t} K"
                );
            }
        }
    }
}

/// Every reference form names a source, with a basis, for each of its six figures.
#[test]
fn substance_thermal_forms_name_their_sources() {
    for &family in MechanicsFamily::ALL {
        let Some(form) = RegolithForm::reference(family) else {
            continue;
        };
        for value in [
            "porosity",
            "grain diameter",
            "emissivity",
            "Young's modulus",
            "Poisson's ratio",
            "surface energy",
        ] {
            assert!(
                form.sources().iter().any(|s| !s.citation().is_empty()
                    && s.covers().split(", ").any(|covered| covered == value)),
                "{family:?}'s {value} names a source"
            );
        }
    }
}

/// The volatile ices' heat capacities are their sources' polynomials in cal mol⁻¹ K⁻¹ times 4,184
/// J kg⁻¹ per cal mol⁻¹ over the molar mass in g mol⁻¹, within rounding, at a point of a piece of
/// each.
#[test]
fn substance_thermal_volatile_ices_reproduce_their_sources() {
    let poly = |c: &[f64], t: f64| c.iter().rev().fold(0.0, |sum, &ci| sum * t + ci);
    let c = |key: &str, t: f64| {
        row(key)
            .substance()
            .thermal_columns()
            .unwrap()
            .heat_capacity_j_per_kg_k(Kelvin::new(t))
    };
    let per_kg = |molar_mass: f64| 4.184 / (molar_mass / 1000.0);
    for (key, molar_mass, t, cal) in [
        (
            "N2",
            rows::NITROGEN_MOLAR_MASS_G_PER_MOL,
            40.0,
            &[
                24.326_576,
                -1.622_356_1,
                0.057_461_088,
                -8.466_459_4e-4,
                4.623_712_0e-6,
            ][..],
        ),
        (
            "CO2",
            rows::CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL,
            150.0,
            &[2.337_682_1, 0.116_987_73, -5.988_331_3e-4, 1.476_079_2e-6][..],
        ),
        (
            "CH4",
            rows::METHANE_MOLAR_MASS_G_PER_MOL,
            50.0,
            &[
                -0.208_860_34,
                0.268_328_81,
                -2.527_947_7e-3,
                5.395_418_2e-6,
                4.592_320_1e-8,
            ][..],
        ),
        (
            "Ar",
            rows::ARGON_MOLAR_MASS_G_PER_MOL,
            60.0,
            &[-2.801_427_0, 0.388_329_82, -6.011_375_8e-3, 3.548_374_5e-5][..],
        ),
        (
            "O2",
            rows::OXYGEN_MOLAR_MASS_G_PER_MOL,
            50.0,
            &[16.908_081, -0.241_817_77, 2.480_908_9e-3][..],
        ),
        (
            "CO",
            rows::CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL,
            64.0,
            &[7.162_162_1, 0.077_562_9][..],
        ),
    ] {
        let expected = poly(cal, t) * per_kg(molar_mass);
        let column = c(key, t);
        assert!(
            (column / expected - 1.0).abs() < 1e-12,
            "{key} at {t} K: {column} against {expected}"
        );
    }
}

/// The volatile ices' sources' printed cross-checks hold: oxygen's α within 5 per cent of
/// Kostriukova 1956's measurements (Sov. Phys. JETP 3, 771, Table, cal mol⁻¹ K⁻¹), nitrogen's β
/// within 5 per cent of Umurhan et al. 2017's 1.35–1.65 kJ kg⁻¹ K⁻¹ over 36–63 K, and carbon
/// monoxide's α below 44 K within 6 per cent of the report's other fit, to Clusius and Eucken's
/// data. CO₂'s conductivity and methane's print no value to check against.
#[test]
fn substance_thermal_volatile_ices_match_their_cross_checks() {
    let poly = |c: &[f64], t: f64| c.iter().rev().fold(0.0, |sum, &ci| sum * t + ci);
    let c = |key: &str, t: f64| {
        row(key)
            .substance()
            .thermal_columns()
            .unwrap()
            .heat_capacity_j_per_kg_k(Kelvin::new(t))
    };
    let per_kg = |molar_mass: f64| 4.184 / (molar_mass / 1000.0);
    for (t, cal) in [
        (8.70, 0.346),
        (10.38, 0.599),
        (12.50, 1.06),
        (15.06, 1.66),
        (17.68, 2.59),
        (19.35, 3.08),
    ] {
        let measured = cal * per_kg(rows::OXYGEN_MOLAR_MASS_G_PER_MOL);
        let column = c("O2", t);
        assert!(
            (column / measured - 1.0).abs() < 0.05,
            "O2 at {t} K: {column} against {measured}"
        );
    }
    for t in [36.0, 45.0, 55.0, 63.0] {
        let column = c("N2", t);
        assert!(
            (1_350.0 * 0.95..=1_650.0 * 1.05).contains(&column),
            "N2 at {t} K: {column}"
        );
    }
    let clusius_eucken = [
        0.0,
        0.059_584_621,
        -1.224_473e-3,
        6.928_083_3e-4,
        -2.292_259_9e-5,
        2.162_280_3e-7,
    ];
    for t in [20.0, 30.0, 40.0] {
        let other = poly(&clusius_eucken, t) * per_kg(rows::CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL);
        let column = c("CO", t);
        assert!(
            (column / other - 1.0).abs() < 0.06,
            "CO at {t} K: {column} against {other}"
        );
    }
}

/// The reduced conductivities are their stated arithmetic: the Balsam Gap dunite's mean, 11.1 CU,
/// 4.64 W m⁻¹ K⁻¹; and Abee's 5.33 W m⁻¹ K⁻¹ corrected by Maxwell's formula at 2.19 per cent,
/// 5.51.
#[test]
fn substance_thermal_reduced_conductivities_are_their_arithmetic() {
    let k = |key: &str, t: f64| {
        row(key)
            .substance()
            .thermal_columns()
            .unwrap()
            .conductivity_w_per_m_k(Kelvin::new(t))
    };
    assert!((k("ultramafic", 300.0) - 4.644).abs() < 1e-3);
    assert!((k("chondrite_e", 200.0) - 5.51).abs() < 5e-3);
    assert!((k("chondrite_cm", 200.0) - 1.61).abs() < 1e-12);
    assert!((k("chondrite_h", 200.0) - 2.18).abs() < 1e-12);
}

/// The air in a regolith's pores of the one gas `key` at `pressure_pa` and `t` kelvin (P14.T49.l):
/// the registry's molar mass and ideal-gas `c_p` at the temperature, and the gas's Lennard-Jones
/// constants.
fn pure_gas(key: &str, pressure_pa: f64, t: f64) -> PoreGas {
    use crate::planetary::chem::condense::COLLISION_CONSTANTS;
    use crate::substance::mixture_heat_capacity_over_r_at;
    let id = row(key);
    let mu = id.substance().molar_mass_g_per_mol().unwrap();
    let cp_over_r = mixture_heat_capacity_over_r_at(&[(id, 1.0)], Kelvin::new(t)).unwrap();
    let collision = *COLLISION_CONSTANTS.iter().find(|c| c.key == key).unwrap();
    PoreGas::new(
        Pascals::new(pressure_pa),
        mu,
        cp_over_r * MOLAR_GAS_CONSTANT / (mu / GRAMS_PER_KILOGRAM),
        collision,
    )
    .unwrap()
}

/// Mars's surface pressure, Pa: 610, the classical 6.1 mbar of Mars's old topographic datum, near
/// water's triple point (Mariner 9's datum; not re-read), within the NASA Mars fact sheet's 4.0–8.7
/// mb by season and 4 per cent below its 6.36 mb at the mean radius (not re-read), as a pure CO₂
/// air.
const MARS_PRESSURE_PA: f64 = 610.0;

/// Mars's surface temperature at which T49.j's record states its `basalt_fe` lid, K: 214.
const MARS_T_K: f64 = 214.0;

/// Titan's surface pressure and temperature: 1,467 hPa and 93.65 K, Huygens's at its landing site
/// (Fulchignoni et al. 2005, Nature 438, 785, abstract; not re-read).
const TITAN: (f64, f64) = (1.467e5, 93.65);

/// The inertia of `key` at its family's form under P14.T49.l's rules on a lid under `air`, its
/// pores holding `pores`.
fn under_pore_gas(key: &str, t: f64, air: LidAir, pores: &Pores) -> RegolithInertia {
    reference_regolith_thermal_inertia_under(
        row(key),
        Kelvin::new(t),
        RegolithInertiaRules::PoreGas,
        air,
        pores,
    )
    .unwrap_or_else(|e| panic!("{key} at {t} K: {e}"))
}

/// P14.T49.l is held until the 25 → 26 bump: the rules in force are P14.T49.j's vacuum, and the
/// tests' switch and 26's helper reach the pore gas.
#[test]
fn the_pore_gas_is_held_only_until_the_26_bump() {
    assert!(
        crate::GENERATOR_VERSION < PORE_GAS_VERSION,
        "the 25 → 26 bump puts P14.T49.l's pore gas and outer-system ice on the record: delete \
         `PORE_GAS_VERSION`, `RegolithInertiaRules` with its `Vacuum` arm and the rules of \
         `reference_regolith_thermal_inertia_under` (its pores then always read), \
         `GroundInertia::under`'s guard, the tests' `with_regolith_inertia`, its switch in \
         `with_every_26_hold` (and the helper, if no other 26 hold is left), and this test; give \
         `RegolithForm::reference` the lid's air (`LidAir`), its airless water-ice form the outer \
         system's"
    );
    assert_eq!(
        RegolithInertiaRules::in_force(),
        RegolithInertiaRules::Vacuum
    );
    assert_eq!(
        with_regolith_inertia(
            RegolithInertiaRules::PoreGas,
            RegolithInertiaRules::in_force
        ),
        RegolithInertiaRules::PoreGas
    );
    assert_eq!(
        crate::planetary::testing::with_every_26_hold(RegolithInertiaRules::in_force),
        RegolithInertiaRules::PoreGas
    );
    assert_eq!(
        crate::planetary::testing::with_every_26_hold(|| {
            with_regolith_inertia(RegolithInertiaRules::Vacuum, RegolithInertiaRules::in_force)
        }),
        RegolithInertiaRules::Vacuum
    );
    assert_eq!(
        RegolithInertiaRules::in_force(),
        RegolithInertiaRules::Vacuum
    );
}

/// Every part of `a` is `b`'s, bit for bit.
fn assert_same_parts(a: &RegolithInertia, b: &RegolithInertia) {
    let parts = |x: &RegolithInertia| {
        [
            x.inertia_j_per_m2_k_sqrt_s(),
            x.conductivity_w_per_m_k(),
            x.contact_conductivity_w_per_m_k(),
            x.radiative_conductivity_w_per_m_k(),
            x.gas_conductivity_w_per_m_k(),
            x.solid_conductivity_w_per_m_k(),
            x.grain_density().value(),
            x.bulk_density().value(),
            x.heat_capacity_j_per_kg_k(),
        ]
    };
    for (x, y) in parts(a).into_iter().zip(parts(b)) {
        assert_same_bits(x, y);
    }
}

/// Empty pores, a gas at zero pressure, and the vacuum's rules whatever the pores hold, each give
/// P14.T49.j's law to the bit, for every row with a form at every pinned temperature; the gas term
/// reads zero. Under the pore gas's rules each form's empty pores are its vacuum's, to the bit: the
/// airless lid's and, a lid under an air, P14.T49.j's law itself.
#[test]
fn substance_thermal_empty_pores_are_the_vacuum_s_to_the_bit() {
    let mars = Pores::Gas(pure_gas("CO2", MARS_PRESSURE_PA, MARS_T_K));
    let none = Pores::Gas(pure_gas("CO2", 0.0, MARS_T_K));
    let mut compared = 0;
    for (id, _) in rows() {
        let family = id.substance().condensed().unwrap().mechanics();
        let Some(form) = RegolithForm::reference(family) else {
            continue;
        };
        for t in [40.0, 90.0, 150.0, 200.0, 273.15] {
            let t = Kelvin::new(t);
            let Ok(vacuum) = regolith_thermal_inertia(id, &form, t) else {
                continue;
            };
            for pores in [&Pores::Vacuum, &none] {
                let same = regolith_thermal_inertia_with_pores(id, &form, t, pores).unwrap();
                assert_same_parts(&same, &vacuum);
                assert_same_bits(same.gas_conductivity_w_per_m_k(), 0.0);
            }
            let held = reference_regolith_thermal_inertia_under(
                id,
                t,
                RegolithInertiaRules::Vacuum,
                LidAir::UnderAir,
                &mars,
            )
            .unwrap();
            assert_same_parts(&held, &reference_regolith_thermal_inertia(id, t).unwrap());
            for air in [LidAir::Airless, LidAir::UnderAir] {
                let own = RegolithForm::reference_under(family, RegolithInertiaRules::PoreGas, air)
                    .unwrap();
                let own_vacuum = regolith_thermal_inertia(id, &own, t).unwrap();
                for pores in [&Pores::Vacuum, &none] {
                    let empty = reference_regolith_thermal_inertia_under(
                        id,
                        t,
                        RegolithInertiaRules::PoreGas,
                        air,
                        pores,
                    )
                    .unwrap();
                    assert_same_parts(&empty, &own_vacuum);
                }
                if air == LidAir::UnderAir {
                    assert_same_parts(&own_vacuum, &vacuum);
                }
            }
            compared += 1;
        }
    }
    assert!(compared > 50, "{compared} compared");
}

/// A pore gas at a pressure so small that its jump length overflows is the vacuum's, every part to
/// the bit, and one just above it is finite with a gas term under 10⁻¹² W m⁻¹ K⁻¹: the term's
/// p → 0 limit, never a NaN.
#[test]
fn substance_thermal_a_vanishing_pore_gas_is_the_vacuum() {
    let silicate = RegolithForm::reference(MechanicsFamily::Silicate).unwrap();
    let t = Kelvin::new(MARS_T_K);
    let vacuum = regolith_thermal_inertia(row("basalt_fe"), &silicate, t).unwrap();
    for p in [f64::MIN_POSITIVE, 5e-324, 1e-300, 1e-12] {
        let gas = Pores::Gas(pure_gas("CO2", p, MARS_T_K));
        let thin = regolith_thermal_inertia_with_pores(row("basalt_fe"), &silicate, t, &gas)
            .unwrap_or_else(|e| panic!("{p} Pa: {e}"));
        let k = thin.gas_conductivity_w_per_m_k();
        assert!(k.is_finite() && (0.0..1e-12).contains(&k), "{p} Pa: {k}");
        assert!(thin.inertia_j_per_m2_k_sqrt_s().is_finite(), "{p} Pa");
        if p < 1e-307 {
            assert_same_parts(&thin, &vacuum);
        }
    }
}

/// The cell's moments J₁ and J₂ agree across the series and the closed form at the bound, and
/// with Simpson's rule over the cell, from a cell near the continuum's x → 1 to a dense bed's x <
/// −1.
#[test]
fn substance_thermal_cell_moments_are_their_integrals() {
    for x in [
        -3.0, -1.2, -0.5, -0.25, -0.1, 0.0, 0.1, 0.249_999, 0.25, 0.5, 0.9, 0.999,
    ] {
        let (first, second) = cell_moments(x, 1.0 - x);
        let simpson = |m: i32| {
            let n = 20_000;
            let h = 1.0 / f64::from(n);
            let f = |s: f64| math::powi(s, m) / (1.0 - x * s);
            let sum = (1..n).fold(0.0, |sum, k| {
                let w = if k % 2 == 1 { 4.0 } else { 2.0 };
                sum + w * f(f64::from(k) * h)
            });
            (f(0.0) + sum + f(1.0)) * h / 3.0
        };
        let tolerance = if x > 0.99 { 1e-6 } else { 1e-11 };
        assert!((first - simpson(1)).abs() < tolerance, "J1 at {x}");
        assert!((second - simpson(2)).abs() < tolerance, "J2 at {x}");
    }
    let below = cell_moments(0.25 - 1e-16, 0.75 + 1e-16);
    let above = cell_moments(0.25, 0.75);
    assert!((below.0 - above.0).abs() < 1e-13 && (below.1 - above.1).abs() < 1e-13);
}

/// With no jump, the packed bed is Zehner and Schlünder's continuum bed, whose closed form is
/// 1 − √φ + 2√φ ÷ N [B (1 − 1 ÷ κ) ÷ N² · ln(κ ÷ B) − (B + 1) ÷ 2 − (B − 1) ÷ N], N = 1 − B ÷ κ
/// (Zehner and Schlünder 1970; not re-read), the closed form's N = 0 avoided; and the bed's
/// conduction falls to nothing as the jump grows, never above the continuum's.
#[test]
fn substance_thermal_packed_bed_is_zehner_and_schlunder_s() {
    for porosity in [0.3_f64, 0.4, 0.5161, 0.64, 0.8] {
        let shape = PACKED_BED_SHAPE_FACTOR * math::powf((1.0 - porosity) / porosity, 10.0 / 9.0);
        for ratio in [3.0, 30.0, 200.0, 700.0] {
            let n = 1.0 - shape / ratio;
            let root = (1.0 - porosity).sqrt();
            let closed = 1.0 - root
                + 2.0 * root / n
                    * (shape * (1.0 - 1.0 / ratio) / (n * n) * math::ln(ratio / shape)
                        - f64::midpoint(shape, 1.0)
                        - (shape - 1.0) / n);
            let continuum = packed_bed_ratio(porosity, ratio, 0.0);
            assert!(
                (continuum - closed).abs() < 1e-9 * closed,
                "ψ {porosity}, κ {ratio}: {continuum} against {closed}"
            );
            let mut last = continuum;
            for jump in [1e-4, 1e-2, 0.1, 1.0, 10.0, 1e3] {
                let bed = packed_bed_ratio(porosity, ratio, jump);
                assert!(bed > 0.0 && bed < last, "ψ {porosity}, κ {ratio}, ℓ {jump}");
                last = bed;
            }
            assert!(last < 1e-2 * continuum, "ψ {porosity}, κ {ratio}: {last}");
        }
    }
}

/// The pore gas's conductivity is the free gas's, Eucken's on Chapman and Enskog's viscosity: the
/// same at every pressure, rising with the temperature, carbon dioxide's within 0.009–0.013 W m⁻¹
/// K⁻¹ at Mars's 214 K and nitrogen's within 0.008–0.011 at Titan's 94 K (the measured gases'
/// about 0.0105 and 0.0090, as the lane recalled them; not re-read); its jump length about 14 µm
/// at Mars and under 0.1 µm at Titan, inversely as the pressure.
#[test]
fn substance_thermal_pore_gas_is_kinetic_theory_s() {
    let mars = pure_gas("CO2", MARS_PRESSURE_PA, MARS_T_K);
    let doubled = pure_gas("CO2", 2.0 * MARS_PRESSURE_PA, MARS_T_K);
    let titan = pure_gas("N2", TITAN.0, TITAN.1);
    let t = Kelvin::new(MARS_T_K);
    let k = mars.conductivity_w_per_m_k(t);
    assert!((0.009..=0.013).contains(&k), "CO₂ {k}");
    assert_same_bits(doubled.conductivity_w_per_m_k(t), k);
    assert!(mars.conductivity_w_per_m_k(Kelvin::new(250.0)) > k);
    let k_titan = titan.conductivity_w_per_m_k(Kelvin::new(TITAN.1));
    assert!((0.008..=0.011).contains(&k_titan), "N₂ {k_titan}");
    let jump = mars.jump_length(t).value();
    assert!((10e-6..=20e-6).contains(&jump), "Mars's jump {jump}");
    assert!(
        (doubled.jump_length(t).value() * 2.0 - jump).abs() < 1e-12 * jump,
        "inversely as the pressure"
    );
    let titan_jump = titan.jump_length(Kelvin::new(TITAN.1)).value();
    assert!(titan_jump < 1e-7, "Titan's jump {titan_jump}");
    eprintln!(
        "pore gas: CO₂ at {MARS_T_K} K k {k:.5}, η {:.4e}, jump {:.2} µm; N₂ at {} K k \
         {k_titan:.5}, jump {:.1} nm",
        mars.viscosity_pa_s(t),
        jump * 1e6,
        TITAN.1,
        titan_jump * 1e9
    );
}

/// Presley and Christensen's measured fit for particulate soils under Mars-like carbon dioxide, k =
/// C P^0.6 d^(−0.11 log₁₀(P ÷ K)), C 0.0015, K 8.1 × 10⁴ torr, P in torr and d in µm (Presley and
/// Christensen 1997, JGR 102, 6551, as the literature quotes it; not re-read), is matched within a
/// factor of 1.5 by the joined law at the silicate form's porosity over 70–500 µm grains of
/// `basalt_fe` at Mars's pressure, and both rise with the grain size.
#[test]
fn substance_thermal_pore_gas_matches_presley_and_christensen() {
    let gas = Pores::Gas(pure_gas("CO2", MARS_PRESSURE_PA, MARS_T_K));
    let silicate = RegolithForm::reference(MechanicsFamily::Silicate).unwrap();
    let torr = MARS_PRESSURE_PA / 133.322_368;
    let mut last = 0.0;
    for d_um in [70.0, 100.0, 200.0, 500.0] {
        let form = RegolithForm::new(
            MechanicsFamily::Silicate,
            silicate.porosity(),
            Metres::new(d_um * 1e-6),
        )
        .unwrap();
        let law = regolith_thermal_inertia_with_pores(
            row("basalt_fe"),
            &form,
            Kelvin::new(MARS_T_K),
            &gas,
        )
        .unwrap()
        .conductivity_w_per_m_k();
        let measured: f64 =
            0.0015 * math::powf(torr, 0.6) * math::powf(d_um, -0.11 * math::log10(torr / 8.1e4));
        eprintln!("Presley and Christensen at {d_um} µm: {measured:.4}, the law {law:.4}");
        assert!(law > last, "rising with the grain size");
        assert!(
            (law / measured) > 1.0 / 1.5 && (law / measured) < 1.5,
            "{d_um} µm: {law} against {measured}"
        );
        last = law;
    }
}

/// At Titan's 1.5 bar the gas term barely depends on the grain size, within 10 per cent over
/// 50 µm–2 mm (Piqueux and Christensen 2009, JGR 114, E09005; not re-read), the logarithm of the gas
/// gap at the contacts all that is left of it, while at Mars's 6 mbar it rises by more than half
/// over the same grains.
#[test]
fn substance_thermal_at_titan_s_pressure_the_gas_ignores_the_grain_size() {
    let span = |key: &str, row_key: &str, family: MechanicsFamily, (p, t): (f64, f64)| {
        let gas = Pores::Gas(pure_gas(key, p, t));
        let porosity = RegolithForm::reference(family).unwrap().porosity();
        let gas_term = |d: f64| {
            let form = RegolithForm::new(family, porosity, Metres::new(d)).unwrap();
            regolith_thermal_inertia_with_pores(row(row_key), &form, Kelvin::new(t), &gas)
                .unwrap()
                .gas_conductivity_w_per_m_k()
        };
        (gas_term(50e-6), gas_term(2e-3))
    };
    let (fine, coarse) = span("N2", "H2O", MechanicsFamily::WaterIce, TITAN);
    assert!(
        (coarse / fine - 1.0).abs() < 0.1,
        "Titan: {fine} to {coarse}"
    );
    let (fine, coarse) = span(
        "CO2",
        "basalt_fe",
        MechanicsFamily::Silicate,
        (MARS_PRESSURE_PA, MARS_T_K),
    );
    assert!(coarse / fine > 1.5, "Mars: {fine} to {coarse}");
}

/// Under P14.T49.l's rules, the binding figures, J m⁻² K⁻¹ s^−½ (P14.T49.l's tests), each pinned as
/// it stands where it misses its window (the lane's `findings.txt`):
///
/// - Mars's `basalt_fe` at the silicate form, 214 K under 6.1 mbar of CO₂: against the global
///   mode's 180–250 (Mellon et al. 2000, Icarus 148, 437; Putzig et al. 2005, Icarus 173, 325;
///   neither re-read), pinned at 145–153, the lunar form's 70 µm grains being finer than Mars's
///   sand;
/// - Titan's `H2O` under Huygens's air, a lid under an air keeping P14.T49.j's form: against
///   MacKenzie et al. 2019's about 236 for its sand (JGR Planets 124, 1728; its tables not
///   reachable from the lane, so not re-read), pinned at 129–138, Titan's sand being organic;
/// - the airless `H2O` form at Ganymede's 110 K, its equilibrium temperature at a Bond albedo of
///   0.35: inside Ganymede's 20–40 (Bockelée-Morvan et al. 2024, A&A 681, A27; not re-read);
/// - the airless `H2O` form at 40 K, a trans-Neptunian object's at 40–45 AU: against the
///   trans-Neptunian mean's 2.5 ± 0.5 (Santos-Sanz et al. 2017, A&A 604, A95; not re-read), pinned
///   at 3.3–3.6, one form's radiative term carrying no more of the temperature.
#[test]
fn substance_thermal_pore_gas_s_binding_figures() {
    let mars_air = Pores::Gas(pure_gas("CO2", MARS_PRESSURE_PA, MARS_T_K));
    let titan_air = Pores::Gas(pure_gas("N2", TITAN.0, TITAN.1));
    let mars = under_pore_gas("basalt_fe", MARS_T_K, LidAir::UnderAir, &mars_air);
    let titan = under_pore_gas("H2O", TITAN.1, LidAir::UnderAir, &titan_air);
    let ganymede = under_pore_gas("H2O", 110.0, LidAir::Airless, &Pores::Vacuum);
    let tno = under_pore_gas("H2O", 40.0, LidAir::Airless, &Pores::Vacuum);
    let gamma = RegolithInertia::inertia_j_per_m2_k_sqrt_s;
    eprintln!(
        "P14.T49.l: Mars {:.2} (k {:.4}: gas {:.4}), Titan {:.2} (k {:.4}: gas {:.4}), Ganymede \
         at 110 K {:.2}, a TNO at 40 K {:.3}",
        gamma(&mars),
        mars.conductivity_w_per_m_k(),
        mars.gas_conductivity_w_per_m_k(),
        gamma(&titan),
        titan.conductivity_w_per_m_k(),
        titan.gas_conductivity_w_per_m_k(),
        gamma(&ganymede),
        gamma(&tno)
    );
    assert!((145.0..=153.0).contains(&gamma(&mars)), "Mars {mars:?}");
    assert!((129.0..=138.0).contains(&gamma(&titan)), "Titan {titan:?}");
    assert!(
        (20.0..=40.0).contains(&gamma(&ganymede)),
        "Ganymede {ganymede:?}"
    );
    assert!((3.3..=3.6).contains(&gamma(&tno)), "a TNO {tno:?}");
}

/// Under P14.T49.l's rules an airless lid's water-ice form is the outer system's, its own
/// conductivity amorphous ice's, every other form, a water-ice lid's under an air among them,
/// P14.T49.j's; it names a source for each of its figures, its conductivity among them, and its
/// radiative term carries Ganymede's inertia.
#[test]
fn substance_thermal_outer_system_ice_is_fluffy_amorphous_ice() {
    for &family in MechanicsFamily::ALL {
        let reference = RegolithForm::reference(family);
        for air in [LidAir::Airless, LidAir::UnderAir] {
            assert_eq!(
                RegolithForm::reference_under(family, RegolithInertiaRules::Vacuum, air),
                reference,
                "{family:?}"
            );
        }
        assert_eq!(
            RegolithForm::reference_under(family, RegolithInertiaRules::PoreGas, LidAir::UnderAir),
            reference,
            "{family:?}"
        );
        if family == MechanicsFamily::WaterIce {
            continue;
        }
        assert_eq!(
            RegolithForm::reference_under(family, RegolithInertiaRules::PoreGas, LidAir::Airless),
            reference,
            "{family:?}"
        );
    }
    let outer = RegolithForm::reference_under(
        MechanicsFamily::WaterIce,
        RegolithInertiaRules::PoreGas,
        LidAir::Airless,
    )
    .unwrap();
    assert_same_bits(outer.porosity(), OUTER_ICE_POROSITY);
    assert_same_bits(outer.grain_diameter().value(), 2e-3);
    let GrainConductivity::Form(own) = outer.grain_conductivity() else {
        panic!("the outer system's ice conducts as amorphous ice");
    };
    assert_same_bits(own.evaluate(100.0), 2.34e-3 * 100.0 + 2.8e-2);
    for value in [
        "porosity",
        "grain diameter",
        "conductivity",
        "emissivity",
        "Young's modulus",
        "Poisson's ratio",
        "surface energy",
    ] {
        assert!(
            outer.sources().iter().any(|s| !s.citation().is_empty()
                && s.covers().split(", ").any(|covered| covered == value)),
            "the outer system's ice's {value} names a source"
        );
    }
    let ganymede = under_pore_gas("H2O", 110.0, LidAir::Airless, &Pores::Vacuum);
    assert!(
        ganymede.radiative_conductivity_w_per_m_k()
            > 10.0 * ganymede.contact_conductivity_w_per_m_k()
    );
    assert_same_bits(ganymede.solid_conductivity_w_per_m_k(), own.evaluate(110.0));
}

/// A pore gas refuses a pressure that is not finite and at least zero, a molar mass that is not
/// positive, a specific heat that leaves no positive `c_v`, and constants that are not positive.
#[test]
fn substance_thermal_pore_gas_refuses_what_is_not_a_gas() {
    use crate::planetary::chem::condense::COLLISION_CONSTANTS;
    let co2 = COLLISION_CONSTANTS[6];
    assert_eq!(co2.key, "CO2");
    let p = Pascals::new(600.0);
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            PoreGas::new(Pascals::new(bad), 44.0, 735.0, co2),
            Err(BuildPoreGasError::Pressure { .. })
        ));
    }
    assert!(matches!(
        PoreGas::new(p, 0.0, 735.0, co2),
        Err(BuildPoreGasError::MolarMass { .. })
    ));
    assert!(matches!(
        PoreGas::new(p, 44.0, 150.0, co2),
        Err(BuildPoreGasError::SpecificHeat { .. })
    ));
    let none = crate::planetary::chem::condense::CollisionConstants {
        diameter_m: 0.0,
        ..co2
    };
    assert!(matches!(
        PoreGas::new(p, 44.0, 735.0, none),
        Err(BuildPoreGasError::Collision { key: "CO2" })
    ));
    assert!(PoreGas::new(Pascals::ZERO, 44.0, 735.0, co2).is_ok());
}
