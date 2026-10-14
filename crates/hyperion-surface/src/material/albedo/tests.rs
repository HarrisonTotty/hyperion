//! The class photometry's tests (plan R10, T10.a): the solve meets p where the ranges allow and
//! is a bisection's in every case, no scaled albedo passes its law's ceiling in any band, the
//! scale depends on the fractions alone, every form and family is solved, and the class fractions
//! of the synthetic worlds; the kernels' golden on every target, and the worlds' natively and on
//! `wasm32-wasip1`.

use hyperion_base::units::Kelvin;
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::{MechanicsFamily, PaletteEntry, PaletteRole};
use crate::material::{Material, MaterialClass, UNASSIGNED_PHASE_ROW, form_row};
use crate::substance_key::SubstanceKey;
use crate::test_sizes::per_target;

/// The keys of a test palette's entries, in order.
const KEYS: [&str; 15] = [
    "key_a", "key_b", "key_c", "key_d", "key_e", "key_f", "key_g", "key_h", "key_i", "key_j",
    "key_k", "key_l", "key_m", "key_n", "key_o",
];

/// The phase codes an entry can carry: none, each law's, and one this build does not know.
const CODES: [u8; 5] = [UNASSIGNED_PHASE_ROW, 1, 2, 3, 200];

/// An entry of `family`, normal albedo `bvr`, phase code `code`, under key `key`.
fn entry(key: &str, family: MechanicsFamily, bvr: [f64; 3], code: u8) -> PaletteEntry {
    PaletteEntry {
        substance: SubstanceKey::new(key).expect("a test key is well formed"),
        role: PaletteRole::Deposit,
        normal_albedo_bvr: bvr,
        phase_row: code,
        grain_density_kg_m3: 2_500.0,
        transition: Kelvin::new(1_000.0),
        mechanics: family,
    }
}

/// A palette of `entries`, each (family, albedo in B, V and R, code), keyed in order.
fn palette(entries: &[(MechanicsFamily, [f64; 3], u8)]) -> MaterialPalette {
    MaterialPalette::new(
        entries
            .iter()
            .zip(KEYS)
            .map(|(&(family, bvr, code), key)| entry(key, family, bvr, code))
            .collect(),
    )
    .expect("a test palette is valid")
}

/// The material of form `class` and entry `entry`.
fn named(class: MaterialClass, entry: u8) -> GroundMaterial {
    GroundMaterial::of(class, Some(entry))
}

/// A random case: a palette of up to 15 entries of any family, albedo and code, up to 8 materials
/// of any form over it with random areas, one in ten unnamed, and a random target p.
fn random_case(rng: &mut Lcg) -> (MaterialPalette, ClassFractions, [f64; 3]) {
    let entries: Vec<(MechanicsFamily, [f64; 3], u8)> = (0..=rng.next_below(15))
        .map(|_| {
            let family = MechanicsFamily::ALL
                [usize::try_from(rng.next_below(6)).expect("a family index fits a usize")];
            // Up to 1.6, past both ceilings, and now and then black.
            let v = if rng.next_below(20) == 0 {
                0.0
            } else {
                1.6 * rng.next_f64()
            };
            let bvr = [
                v * (0.6 + 0.8 * rng.next_f64()),
                v,
                v * (0.6 + 0.8 * rng.next_f64()),
            ];
            let code = CODES[usize::try_from(rng.next_below(5)).expect("a code index fits")];
            (family, bvr, code)
        })
        .collect();
    let len = u64::try_from(entries.len()).expect("a palette's length fits a u64");
    let areas: Vec<(GroundMaterial, f64)> = (0..=rng.next_below(8))
        .map(|_| {
            let class = MaterialClass::ALL
                [usize::try_from(rng.next_below(15)).expect("a class index fits a usize")];
            let entry = (rng.next_below(10) != 0)
                .then(|| u8::try_from(rng.next_below(len)).expect("an entry fits a u8"));
            (GroundMaterial::of(class, entry), 0.01 + rng.next_f64())
        })
        .collect();
    let fractions = ClassFractions::new(areas).expect("random areas are positive");
    let p = 0.005 + 1.2 * rng.next_f64();
    (palette(&entries), fractions, [0.9 * p, p, 1.1 * p])
}

/// The named materials of `fractions` with their shares of the named ground and properties.
fn named_materials(
    fractions: &ClassFractions,
    palette: &MaterialPalette,
) -> Vec<(f64, MaterialProperties)> {
    let named: f64 = fractions
        .shares()
        .iter()
        .filter(|(m, _)| m.material().is_some())
        .map(|&(_, s)| s)
        .sum();
    fractions
        .shares()
        .iter()
        .filter_map(|&(m, share)| {
            let material = m.material()?;
            Some((
                share / named,
                properties(material, palette).expect("in the palette"),
            ))
        })
        .collect()
}

/// The clamped sum's p in V at `scale`, written out independently of the solve: each named
/// material's c `A_N` clamped to its span stretched to its own albedo and capped at its law's
/// ceiling, times (2 + L) ÷ 3, L its law's of the clamped albedo (decision-r10-limb §2).
fn reference_p(materials: &[(f64, MaterialProperties)], scale: f64) -> f64 {
    let mut p = 0.0;
    for (share, m) in materials {
        let own = m.normal_albedo_bvr()[1];
        let ceiling = match m.phase_law() {
            PhaseLaw::Lambert => 1.0,
            PhaseLaw::Regolith | PhaseLaw::Ice => 1.35,
        };
        let (low, high) = m.form().normal_albedo_v().map_or((0.0, ceiling), |span| {
            (span.low().min(own), span.high().max(own).min(ceiling))
        });
        let x = (scale * own).clamp(low, high);
        p += share * x * (2.0 + m.phase_law().limb_parameter(x)) / 3.0;
    }
    p
}

/// The largest scale a test bisects to.
const BISECTION_TOP: f64 = 1.0e7;

/// A scale at which the reference p is `target`, by 200 halvings of [`ALBEDO_SCALE_FLOOR`] to
/// [`BISECTION_TOP`] in the logarithm, where the reference reaches the target within them.
fn bisect(materials: &[(f64, MaterialProperties)], target: f64) -> Option<f64> {
    let (mut low, mut high) = (ALBEDO_SCALE_FLOOR, BISECTION_TOP);
    if reference_p(materials, low) >= target || reference_p(materials, high) <= target {
        return None;
    }
    for _ in 0..200 {
        let middle = (low * high).sqrt();
        if reference_p(materials, middle) < target {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some((low * high).sqrt())
}

/// The fractions' materials with their shares' bits, so that two sets compare bit for bit.
fn share_bits(fractions: &ClassFractions) -> Vec<(GroundMaterial, u64)> {
    fractions
        .shares()
        .iter()
        .map(|&(m, share)| (m, bits(share)))
        .collect()
}

/// The solve of a random case: its scale, or `None` where the case's ground is all unnamed, the
/// one refusal a random case can meet.
fn solve_case(
    case: usize,
    fractions: &ClassFractions,
    palette: &MaterialPalette,
    target: [f64; 3],
) -> Option<AlbedoScale> {
    match albedo_scale(fractions, palette, target) {
        Ok(solved) => Some(solved),
        Err(SolveAlbedoScaleError::NoNamedGround) => {
            assert!(
                (fractions.unnamed_share() - 1.0).abs() < 1e-15,
                "case {case}: refused with named ground"
            );
            None
        }
        Err(error) => panic!("case {case}: {error:?}"),
    }
}

/// Wherever the materials' ranges reach the body's p, the scaled materials give it, to rounding;
/// and where they do not, the residual is the reference's at the end of the range it misses.
#[test]
fn albedo_scale_gives_the_body_s_geometric_albedo_where_the_ranges_allow() {
    let mut rng = Lcg::new(0x616c_6265_646f_0001);
    let (mut met, mut missed) = (0, 0);
    for case in 0..per_target(400, 100) {
        let (palette, fractions, target) = random_case(&mut rng);
        let Some(solved) = solve_case(case, &fractions, &palette, target) else {
            continue;
        };
        let materials = named_materials(&fractions, &palette);
        let (bottom, top) = (
            reference_p(&materials, ALBEDO_SCALE_FLOOR),
            reference_p(&materials, BISECTION_TOP),
        );
        let p = target[1];
        let residual = solved.residual()[1];
        if bottom < p && p < top {
            assert!(
                residual.abs() <= 1e-12 * p,
                "case {case}: residual {residual} at p {p} within [{bottom}, {top}]"
            );
            met += 1;
        } else {
            let edge = if p <= bottom { bottom } else { top };
            assert!(
                (residual - (edge - p)).abs() <= 1e-12,
                "case {case}: residual {residual}, not the range's {}",
                edge - p
            );
            missed += 1;
        }
        assert!(
            (solved.geometric_albedo_bvr()[1] - reference_p(&materials, solved.scale())).abs()
                <= 1e-12,
            "case {case}: the stated p is the sum's"
        );
        let relative = solved.relative_residual();
        assert!(
            relative.iter().all(|r| r.is_finite()),
            "case {case}: {relative:?}"
        );
    }
    assert!(
        met > per_target(100, 25) && missed > per_target(20, 5),
        "{met} met, {missed} missed"
    );
}

/// The exact solve is a bisection of the clamped sum: the same p to rounding, and the same scale
/// wherever the sum rises at the root, so that the root is one scale.
#[test]
fn albedo_scale_is_a_bisection_of_the_clamped_sum() {
    let mut rng = Lcg::new(0x616c_6265_646f_0002);
    let mut compared = 0;
    for case in 0..per_target(400, 100) {
        let (palette, fractions, target) = random_case(&mut rng);
        let Some(solved) = solve_case(case, &fractions, &palette, target) else {
            continue;
        };
        let materials = named_materials(&fractions, &palette);
        let Some(reference) = bisect(&materials, target[1]) else {
            continue;
        };
        let (got, want) = (
            reference_p(&materials, solved.scale()),
            reference_p(&materials, reference),
        );
        assert!(
            (got - want).abs() <= 1e-12,
            "case {case}: p {got} against {want}"
        );
        let rises = reference_p(&materials, reference * (1.0 + 1e-6))
            > reference_p(&materials, reference * (1.0 - 1e-6));
        if rises {
            let c = solved.scale();
            assert!(
                (c - reference).abs() <= 1e-9 * reference,
                "case {case}: scale {c} against the bisection's {reference}"
            );
            compared += 1;
        }
    }
    assert!(compared > per_target(100, 25), "{compared} scales compared");
}

/// No scaled material's normal albedo in any band passes its law's ceiling, 1 for Lambert's and
/// 1.35 for the particulate laws, at any scale, whatever its entry's own albedo and colour
/// (decision-r10-limb §2–§4, "per band"); none falls below 0; its V lies in its range, and the
/// scale 1 keeps its own V up to the ceiling.
#[test]
fn albedo_no_scaled_class_exceeds_its_law_s_ceiling() {
    let albedos = [0.0, 0.03, 0.2, 0.6, 0.79, 0.95, 1.0, 1.2, 1.35, 1.5, 2.0];
    let scales = [ALBEDO_SCALE_FLOOR, 1e-3, 0.5, 1.0, 1.7, 10.0, 1e6];
    for &family in MechanicsFamily::ALL {
        for code in CODES {
            // Redder than grey and bluer, so that B or R passes V.
            let entries: Vec<_> = albedos
                .iter()
                .map(|&a| (family, [1.3 * a, a, 0.8 * a], code))
                .collect();
            let palette = palette(&entries);
            for &class in MaterialClass::ALL {
                for (index, &own) in (0_u8..).zip(&albedos) {
                    let material = Material::new(class, index).expect("an entry of the palette");
                    let p = properties(material, &palette).expect("in the palette");
                    let ceiling = p.phase_law().normal_albedo_ceiling();
                    let range = scale_range(&p);
                    assert!(
                        range.high() <= ceiling,
                        "{class:?} of {family:?}: {range:?}"
                    );
                    for scale in scales {
                        let drawn = scaled_normal_albedo(&p, scale);
                        let at = format!("{class:?} of {family:?} at {own}, scale {scale}");
                        assert!(
                            drawn.iter().all(|&a| a <= ceiling),
                            "{at}: {drawn:?} over {ceiling}"
                        );
                        assert!(drawn.iter().all(|&a| a >= 0.0), "{at}: {drawn:?}");
                        assert!(
                            range.contains(drawn[1]),
                            "{at}: {drawn:?} outside {range:?}"
                        );
                    }
                    let kept = scaled_normal_albedo(&p, 1.0)[1];
                    let own_v = p.normal_albedo_bvr()[1];
                    assert!(
                        (kept - own_v.min(ceiling)).abs() < 1e-15,
                        "{class:?} of {family:?}: {kept} at the scale 1, not {own_v}"
                    );
                }
            }
        }
    }
}

/// Every solved scale leaves every material inside its law's ceiling in every band, on the random
/// cases, whose colours run from 0.6 to 1.4 times their V.
#[test]
fn albedo_no_solved_class_exceeds_its_law_s_ceiling() {
    let mut rng = Lcg::new(0x616c_6265_646f_0003);
    for case in 0..per_target(300, 75) {
        let (palette, fractions, target) = random_case(&mut rng);
        let Some(solved) = solve_case(case, &fractions, &palette, target) else {
            continue;
        };
        let c = solved.scale();
        assert!(c.is_finite() && c >= ALBEDO_SCALE_FLOOR, "case {case}: {c}");
        for (_, p) in named_materials(&fractions, &palette) {
            let drawn = scaled_normal_albedo(&p, c);
            let ceiling = p.phase_law().normal_albedo_ceiling();
            assert!(
                drawn.iter().all(|&a| a <= ceiling),
                "case {case}: {drawn:?} under {:?}",
                p.phase_law()
            );
        }
    }
}

/// The scale is a function of the fractions, the palette and p: the same areas given in another
/// order, or all doubled, are the same fractions, bit for bit, and give the same scale and
/// residuals.
#[test]
fn albedo_the_solve_depends_on_the_class_fractions_only() {
    let mut rng = Lcg::new(0x616c_6265_646f_0004);
    for case in 0..per_target(100, 25) {
        let (palette, fractions, target) = random_case(&mut rng);
        // Areas of distinct materials, in the materials' reverse order.
        let areas: Vec<(GroundMaterial, f64)> = fractions
            .shares()
            .iter()
            .rev()
            .map(|&(m, _)| (m, 0.01 + rng.next_f64()))
            .collect();
        let one = ClassFractions::new(areas.iter().copied()).expect("valid");
        let reversed = ClassFractions::new(areas.iter().rev().copied()).expect("valid");
        let doubled = ClassFractions::new(areas.iter().map(|&(m, a)| (m, 2.0 * a))).expect("valid");
        assert_eq!(share_bits(&reversed), share_bits(&one), "case {case}");
        assert_eq!(share_bits(&doubled), share_bits(&one), "case {case}");
        let first = solve_case(case, &one, &palette, target);
        for other in [&reversed, &doubled] {
            let again = solve_case(case, other, &palette, target);
            assert_eq!(
                first.map(|s| bits(s.scale())),
                again.map(|s| bits(s.scale()))
            );
            assert_eq!(
                first.map(|s| s.residual().map(bits)),
                again.map(|s| s.residual().map(bits)),
                "case {case}"
            );
        }
    }
}

/// A body of one material takes its law's closed form: p = c `A_N` under L = 1 up to 0.60,
/// 0.6280 x + 0.2232 between the particulate anchors, ⅔ x under Lambert's; and where its range
/// stops short of p, the range's end, with the residual stated.
#[test]
fn albedo_one_material_takes_its_law_s_closed_form() {
    let pal = palette(&[
        (
            MechanicsFamily::Silicate,
            [0.08, 0.1, 0.12],
            UNASSIGNED_PHASE_ROW,
        ),
        (MechanicsFamily::VolatileIce, [0.5; 3], 2),
        (MechanicsFamily::WaterIce, [0.965; 3], UNASSIGNED_PHASE_ROW),
    ]);
    let one = |m: GroundMaterial| ClassFractions::new([(m, 1.0)]).expect("one material");
    let solve = |m, p: f64| albedo_scale(&one(m), &pal, [p; 3]).expect("a named material");

    // Regolith of silicate under its family's Regolith law, L = 1: p = c A_N, inside the lunar
    // span stretched to 0.1.
    let regolith = solve(named(MaterialClass::Regolith, 0), 0.07);
    assert!((regolith.scale() - 0.7).abs() < 1e-14, "{regolith:?}");
    assert!(regolith.residual()[1].abs() < 1e-15);
    // Its B and R take the V's factor: 0.7 × 0.08 and 0.7 × 0.12.
    assert!((regolith.geometric_albedo_bvr()[0] - 0.056).abs() < 1e-15);
    assert!((regolith.geometric_albedo_bvr()[2] - 0.084).abs() < 1e-15);

    // Frost of a volatile ice under its entry's Ice law, between the anchors: x L(x) is linear,
    // p = x (2 + 0.6696 ÷ x − 0.1160) ÷ 3.
    let frost = solve(named(MaterialClass::Frost, 1), 0.7);
    let x = (3.0 * 0.7 - 0.6696) / (2.0 - 0.1160);
    assert!((frost.scale() - x / 0.5).abs() < 1e-12, "{frost:?}");
    assert!(frost.residual()[1].abs() < 1e-15);

    // Glacier of water ice, Lambert's: p = ⅔ x, x = 0.786 c inside 0.69–0.89.
    let glacier_v = 0.965 * f64::midpoint(0.69, 0.89) / 0.97;
    let glacier = solve(named(MaterialClass::Glacier, 2), 0.5);
    assert!(
        (glacier.scale() - 0.75 / glacier_v).abs() < 1e-12,
        "{glacier:?}"
    );
    // Past its span's top: the scale that first reaches it, ⅔ 0.89 short of p.
    let bright = solve(named(MaterialClass::Glacier, 2), 0.7);
    assert!(
        (bright.scale() - 0.89 / glacier_v).abs() < 1e-12,
        "{bright:?}"
    );
    assert!((bright.residual()[1] - (0.89 * 2.0 / 3.0 - 0.7)).abs() < 1e-15);
    assert!((bright.clamped_high_share() - 1.0).abs() < 1e-15);
    // Below its span's floor: every scale up to the one that leaves it ties, and 0.69 ÷ 0.786,
    // the nearest 1, is taken.
    let dark = solve(named(MaterialClass::Glacier, 2), 0.1);
    assert!((dark.scale() - 0.69 / glacier_v).abs() < 1e-12, "{dark:?}");
    assert!((dark.residual()[1] - (0.69 * 2.0 / 3.0 - 0.1)).abs() < 1e-15);
}

/// Where the ranges leave the scale free, it is the one nearest 1: a p the entries' own albedos
/// give is c = 1, a black body's is 1, and ground whose spanned materials are already at their
/// floor takes the floor when p asks the rest to go darker than any positive scale allows.
#[test]
fn albedo_the_scale_is_nearest_one_where_the_ranges_leave_it_free() {
    let pal = palette(&[
        (MechanicsFamily::Silicate, [0.1; 3], UNASSIGNED_PHASE_ROW),
        (MechanicsFamily::Silicate, [0.0; 3], UNASSIGNED_PHASE_ROW),
        (MechanicsFamily::WaterIce, [0.965; 3], UNASSIGNED_PHASE_ROW),
    ]);
    let fractions = ClassFractions::new([
        (named(MaterialClass::Regolith, 0), 0.6),
        (named(MaterialClass::Soil, 0), 0.2),
        (named(MaterialClass::Glacier, 2), 0.2),
    ])
    .expect("three materials");
    let at_one = reference_p(&named_materials(&fractions, &pal), 1.0);
    let own = albedo_scale(&fractions, &pal, [at_one; 3]).expect("named");
    assert!((own.scale() - 1.0).abs() < 1e-12, "{own:?}");

    let black = ClassFractions::new([(named(MaterialClass::Bedrock, 1), 1.0)]).expect("one");
    let solved = albedo_scale(&black, &pal, [0.3; 3]).expect("named");
    assert_eq!(bits(solved.scale()), bits(1.0), "{solved:?}");
    assert!((solved.residual()[1] + 0.3).abs() < 1e-15);

    // The regolith's and the glacier's floors alone give 0.6 × 0.057 + 0.2 × ⅔ × 0.69 = 0.126,
    // above p, and only the soil can go darker: towards black, so the scale is the floor.
    let dark = albedo_scale(&fractions, &pal, [0.05; 3]).expect("named");
    assert_eq!(bits(dark.scale()), bits(ALBEDO_SCALE_FLOOR), "{dark:?}");
    assert!((dark.clamped_low_share() - 0.8).abs() < 1e-15, "{dark:?}");
    let lunar_low = form_row(MaterialClass::Regolith, MechanicsFamily::Silicate)
        .normal_albedo_v()
        .expect("the lunar span")
        .low();
    let floor_p = 0.6 * lunar_low + 0.2 * 0.69 * 2.0 / 3.0;
    assert!(
        (dark.geometric_albedo_bvr()[1] - floor_p).abs() < 1e-6,
        "{dark:?}"
    );
}

/// The unnamed ground is left out and the named ground solved for p; ground with no name at all,
/// a p that is not finite and positive and an entry past the palette are refused.
#[test]
fn albedo_scale_leaves_out_unnamed_ground_and_refuses_bad_inputs() {
    let pal = palette(&[(MechanicsFamily::Silicate, [0.1; 3], UNASSIGNED_PHASE_ROW)]);
    let unnamed = GroundMaterial::of(MaterialClass::Regolith, None);
    let half = ClassFractions::new([(unnamed, 1.0), (named(MaterialClass::Regolith, 0), 1.0)])
        .expect("two materials");
    assert!((half.unnamed_share() - 0.5).abs() < 1e-15);
    let solved = albedo_scale(&half, &pal, [0.08; 3]).expect("half named");
    assert!((solved.scale() - 0.8).abs() < 1e-14, "{solved:?}");
    assert!((solved.unnamed_share() - 0.5).abs() < 1e-15);

    let none = ClassFractions::new([(unnamed, 1.0)]).expect("one material");
    assert_eq!(
        albedo_scale(&none, &pal, [0.1; 3]).unwrap_err(),
        SolveAlbedoScaleError::NoNamedGround
    );
    let one = ClassFractions::new([(named(MaterialClass::Regolith, 0), 1.0)]).expect("one");
    for (target, band) in [
        ([0.0, 0.1, 0.1], "B"),
        ([0.1, f64::NAN, 0.1], "V"),
        ([0.1, 0.1, f64::INFINITY], "R"),
        ([0.1, -0.1, 0.1], "V"),
    ] {
        match albedo_scale(&one, &pal, target) {
            Err(SolveAlbedoScaleError::GeometricAlbedo { band: got, .. }) => assert_eq!(got, band),
            other => panic!("{target:?}: {other:?}"),
        }
    }
    let past = ClassFractions::new([(named(MaterialClass::Regolith, 3), 1.0)]).expect("one");
    let refused = albedo_scale(&past, &pal, [0.1; 3]).unwrap_err();
    assert_eq!(
        refused,
        SolveAlbedoScaleError::Properties(ResolvePropertiesError::EntryNotInPalette {
            entry: 3,
            len: 1
        })
    );
    assert_eq!(
        refused.to_string(),
        "a material's properties cannot be resolved"
    );
    assert!(std::error::Error::source(&refused).is_some());
    assert_eq!(
        SolveAlbedoScaleError::NoNamedGround.to_string(),
        "the palette names no substance for the ground"
    );
}

/// The fractions refuse an area that is not finite and non-negative, areas that sum to nothing,
/// and finite areas that sum past a finite value; they merge a material named twice, drop one of
/// no area, and sum to 1.
#[test]
fn albedo_class_fractions_are_shares_of_the_ground() {
    let (a, b) = (named(MaterialClass::Soil, 0), named(MaterialClass::Sand, 0));
    for area in [f64::NAN, -1.0, f64::INFINITY] {
        let refused = ClassFractions::new([(a, 1.0), (b, area)]).unwrap_err();
        assert!(
            matches!(refused, BuildClassFractionsError::Area { material, area: got }
                if material == b
                    && got.is_nan() == area.is_nan()
                    && (area.is_nan() || bits(got) == bits(area))),
            "{refused:?}"
        );
    }
    let refused =
        |areas: &[(GroundMaterial, f64)]| ClassFractions::new(areas.iter().copied()).unwrap_err();
    assert_eq!(refused(&[(a, 0.0)]), BuildClassFractionsError::Empty);
    assert_eq!(refused(&[]), BuildClassFractionsError::Empty);
    for areas in [
        [(a, f64::MAX), (a, f64::MAX)],
        [(a, f64::MAX), (b, f64::MAX)],
    ] {
        assert_eq!(refused(&areas), BuildClassFractionsError::TotalNotFinite);
    }
    assert_eq!(
        BuildClassFractionsError::Empty.to_string(),
        "the materials' areas sum to nothing"
    );
    let fractions = ClassFractions::new([
        (b, 1.0),
        (a, 1.0),
        (b, 2.0),
        (named(MaterialClass::Dust, 0), 0.0),
    ])
    .expect("valid");
    assert_eq!(fractions.shares().len(), 2);
    assert!((fractions.share(a) - 0.25).abs() < 1e-15);
    assert!((fractions.share(b) - 0.75).abs() < 1e-15);
    assert_eq!(bits(fractions.share(named(MaterialClass::Dust, 0))), 0);
    assert_eq!(bits(fractions.unnamed_share()), 0);
}

/// A material's range is its form's span stretched to its own albedo and capped at its law's
/// ceiling, and 0 to the ceiling where its form has no span.
#[test]
fn albedo_scale_range_stretches_the_span_to_the_entry_s_own_albedo() {
    let pal = palette(&[
        (MechanicsFamily::Silicate, [0.2; 3], UNASSIGNED_PHASE_ROW),
        (MechanicsFamily::Silicate, [0.04; 3], UNASSIGNED_PHASE_ROW),
        (MechanicsFamily::Silicate, [1.5; 3], UNASSIGNED_PHASE_ROW),
        (MechanicsFamily::WaterIce, [0.965; 3], UNASSIGNED_PHASE_ROW),
    ]);
    let range = |class, entry| {
        let p = properties(Material::new(class, entry).expect("in range"), &pal).expect("in");
        let r = scale_range(&p);
        (bits(r.low()), bits(r.high()))
    };
    let lunar = form_row(MaterialClass::Regolith, MechanicsFamily::Silicate)
        .normal_albedo_v()
        .expect("the lunar span");
    let want = |low: f64, high: f64| (bits(low), bits(high));
    assert_eq!(range(MaterialClass::Regolith, 0), want(lunar.low(), 0.2));
    assert_eq!(range(MaterialClass::Regolith, 1), want(0.04, lunar.high()));
    assert_eq!(range(MaterialClass::Regolith, 2), want(lunar.low(), 1.35));
    assert_eq!(range(MaterialClass::Bedrock, 0), want(0.0, 1.35));
    assert_eq!(range(MaterialClass::Evaporite, 2), want(0.0, 1.0));
    assert_eq!(range(MaterialClass::Snow, 3), want(0.82, 0.98));
}

/// Every form of every family, its entry under every code, at albedos from black to past the
/// ceilings, is solved for a dark, a moderate and a bright p: a scale from the floor, finite, its
/// material within its ceiling, a body whose palette has one material.
#[test]
fn albedo_every_form_and_family_has_a_scale() {
    let albedos = [0.0, 0.05, 0.5, 0.9, 1.4];
    for &family in MechanicsFamily::ALL {
        for code in CODES {
            let entries: Vec<_> = albedos.iter().map(|&a| (family, [a; 3], code)).collect();
            let pal = palette(&entries);
            for &class in MaterialClass::ALL {
                for entry in 0..5_u8 {
                    let m = named(class, entry);
                    let fractions = ClassFractions::new([(m, 1.0)]).expect("one material");
                    for p in [0.02, 0.3, 1.0] {
                        let at =
                            format!("{class:?} of {family:?}, code {code}, entry {entry}, p {p}");
                        let solved = albedo_scale(&fractions, &pal, [p; 3])
                            .unwrap_or_else(|e| panic!("{at}: {e}"));
                        let c = solved.scale();
                        assert!(c.is_finite() && c >= ALBEDO_SCALE_FLOOR, "{at}: {c}");
                        assert!(solved.residual().iter().all(|r| r.is_finite()), "{at}");
                        let props = properties(m.material().expect("named"), &pal).expect("in");
                        let v = scaled_normal_albedo(&props, c)[1];
                        assert!(v <= props.phase_law().normal_albedo_ceiling(), "{at}: {v}");
                    }
                }
            }
        }
    }
}

/// The kernel cases the golden pins: random cases, whose palettes, fractions and targets are the
/// generator's, so that a change to the solve or the ranges moves the golden.
fn kernel_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut rng = Lcg::new(0x616c_6265_646f_0005);
    for case in 0..64 {
        let (palette, fractions, target) = random_case(&mut rng);
        match albedo_scale(&fractions, &palette, target) {
            Ok(solved) => {
                out.f64(&format!("case {case} scale"), solved.scale());
                for (band, name) in ["B", "V", "R"].into_iter().enumerate() {
                    out.f64(
                        &format!("case {case} p {name}"),
                        solved.geometric_albedo_bvr()[band],
                    );
                }
                out.f64(
                    &format!("case {case} clamped low"),
                    solved.clamped_low_share(),
                );
                out.f64(
                    &format!("case {case} clamped high"),
                    solved.clamped_high_share(),
                );
            }
            Err(error) => out.line(&format!("case {case} refused: {error}")),
        }
    }
    out.finish()
}

/// The solve's kernels, bit for bit on every target (`tests/golden/albedo/kernels.golden`).
#[test]
fn albedo_kernels_match_their_golden() {
    golden!("albedo/kernels", &kernel_text());
}

/// The tests that read the synthetic worlds' whole fields, or files, off the browser target: the
/// class fractions cost about 20 µs a cell natively (a level-8 field's 393,216 cells in 8 s,
/// provisional: R10's Risks, "Deviations in T10.a, as built"), which the browser target's lib
/// binary has no time for, and the server that computes them is native (plan R04, Design note 12,
/// for the files). They run natively and on `wasm32-wasip1`.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    use super::*;
    use crate::cube::xyz_to_face_uv;
    use crate::material::{THIN_ATMOSPHERE_PRESSURE, material_at, weights_at};
    use crate::synth::test_support::{SEED, random_points_in, world};
    use crate::testing::SyntheticWorld;

    /// The albedo goldens: the kernels' and the worlds'.
    const ALBEDO_GOLDENS: usize = 2;

    /// The geometric albedo in B, V and R of the template the sim's photometry chooses for a
    /// synthetic world's state, its analogue's (`planetary::derive::photometry`'s rows, Mallama,
    /// Krobusek and Pavlov 2017, Icarus 282, 19, Table 7, and Robinson 2026, PSJ 7, 12, for
    /// Earth's), which a body of the analogue's Bond albedo is drawn with; `None` for the flat
    /// fields, which no state describes.
    fn analogue_p(world: SyntheticWorld) -> Option<[f64; 3]> {
        match world {
            // Earth's template under its thick air: the server sends no scale (main's ruling).
            SyntheticWorld::EarthLike => Some([0.263, 0.215, 0.210]),
            // Mars's, thin-aired.
            SyntheticWorld::MarsLike => Some([0.088, 0.170, 0.288]),
            // Mercury's, the generator's airless rock.
            SyntheticWorld::MoonLike | SyntheticWorld::CeresLike => Some([0.105, 0.142, 0.172]),
            SyntheticWorld::Flat | SyntheticWorld::OneCrater => None,
        }
    }

    /// The class fractions of `world` with the cache `cache`.
    fn fractions_with(world_: SyntheticWorld, cache: &mut SynthCache) -> ClassFractions {
        let synth = Synthesiser::new(world(world_), SEED);
        class_fractions(&synth, cache).expect("a coarse field holds every cell")
    }

    /// A world's fractions sum to 1, and agree with the classifier counted at the centres of
    /// random cells, area-weighted, within three points of total variation: 1,000 cells on the
    /// Moon- and Ceres-like worlds and 400 on the Mars-like one, whose finest points cost about
    /// 20 ms each (5, 4 and 39 s for 2,000, natively under load). The bound is a model-agreement
    /// tolerance, not a statistical one: the weights are each cell's expectation under R10.T2's
    /// Gaussian model, the count one point of it, and the sample's own error is under a point;
    /// the seed is fixed.
    #[test]
    fn albedo_class_fractions_agree_with_the_classes_counted() {
        let mut rng = Lcg::new(0x616c_6265_646f_0006);
        for (world_, points) in [
            (SyntheticWorld::MoonLike, 1_000),
            (SyntheticWorld::CeresLike, 1_000),
            (SyntheticWorld::MarsLike, 400),
        ] {
            let field = world(world_);
            let fractions = fractions_with(world_, &mut SynthCache::new());
            let total = fractions.shares().iter().fold(0.0, |sum, &(_, s)| sum + s);
            assert!((total - 1.0).abs() < 1e-12, "{world_:?}: {total}");
            let synth = Synthesiser::new(field, SEED);
            let mut cache = SynthCache::new();
            let level = field.header().level();
            let figure = field.header().figure();
            let mut counted: BTreeMap<GroundMaterial, f64> = BTreeMap::new();
            for _ in 0..points {
                let index = u32::try_from(rng.next_below(u64::from(level.cell_count())))
                    .expect("a cell index fits a u32");
                let cell = cell_at_index(level.get(), index).expect("a cell");
                let material = material_at(&synth, &mut cache, centre(cell)).expect("held");
                *counted.entry(material).or_insert(0.0) += cell_area(cell, &figure).value();
            }
            let counted = ClassFractions::new(counted).expect("areas");
            let mut materials: Vec<GroundMaterial> = fractions
                .shares()
                .iter()
                .chain(counted.shares())
                .map(|&(m, _)| m)
                .collect();
            materials.sort();
            materials.dedup();
            let variation = materials.iter().fold(0.0, |sum, &m| {
                sum + (fractions.share(m) - counted.share(m)).abs()
            }) / 2.0;
            assert!(
                variation < 0.03,
                "{world_:?}: {variation}, {fractions:?} against {counted:?}"
            );
        }
    }

    /// The fractions do not depend on what the caller's cache holds: a cache that has served the
    /// classifier and the weights at other points and levels of the same field gives the fresh
    /// cache's fractions, bit for bit (the sim-determinism skill's order independence).
    #[test]
    fn albedo_class_fractions_do_not_depend_on_the_cache() {
        let world_ = SyntheticWorld::CeresLike;
        let field = world(world_);
        let fresh = fractions_with(world_, &mut SynthCache::new());
        let synth = Synthesiser::new(field, SEED);
        let mut used = SynthCache::new();
        let mut rng = Lcg::new(0x616c_6265_646f_0007);
        let level = field.header().level();
        let finest = crate::geometry::finest_level(field.header().figure().equatorial_radius_m);
        for _ in 0..40 {
            let index = u32::try_from(rng.next_below(u64::from(level.cell_count())))
                .expect("a cell index fits a u32");
            let cell = cell_at_index(level.get(), index).expect("a cell");
            for dir in random_points_in(cell, 2, &mut rng) {
                let at = xyz_to_face_uv(dir);
                material_at(&synth, &mut used, at).expect("held");
                for band in [0, 4, finest / 2] {
                    let band = BandLevel::new(band).expect("a band level");
                    weights_at(&synth, &mut used, at, band).expect("held");
                }
            }
        }
        let again = fractions_with(world_, &mut used);
        assert_eq!(share_bits(&again), share_bits(&fresh));
    }

    /// The worlds' golden: each synthetic world's class fractions and the solve at its analogue's
    /// p (`tests/golden/albedo/worlds.golden`), with the stated residuals.
    fn worlds_text(fractions: &[(SyntheticWorld, ClassFractions)]) -> String {
        let mut out = GoldenWriter::new();
        out.header(crate::generator_version());
        for (world_, fractions) in fractions {
            out.line(&format!(
                "{world_:?} level {}",
                world(*world_).header().level().get()
            ));
            for &(m, share) in fractions.shares() {
                let entry = m.entry().map_or_else(|| "-".to_owned(), |e| e.to_string());
                out.f64(&format!("  {:?} {entry}", m.class()), share);
            }
            let Some(target) = analogue_p(*world_) else {
                out.line("  no analogue");
                continue;
            };
            match albedo_scale(fractions, world(*world_).header().palette(), target) {
                Ok(solved) => {
                    out.f64("  scale", solved.scale());
                    for (band, name) in ["B", "V", "R"].into_iter().enumerate() {
                        out.f64(&format!("  p {name}"), solved.geometric_albedo_bvr()[band]);
                        out.f64(&format!("  residual {name}"), solved.residual()[band]);
                    }
                    out.f64("  clamped low", solved.clamped_low_share());
                    out.f64("  clamped high", solved.clamped_high_share());
                    out.f64("  unnamed", solved.unnamed_share());
                }
                Err(error) => out.line(&format!("  refused: {error}")),
            }
        }
        out.finish()
    }

    /// Every synthetic world's fractions and scale match the golden, and the airless and
    /// thin-aired worlds, whose ground the server scales, meet their analogue's p in V to 1%:
    /// each world's fractions computed once.
    #[test]
    fn albedo_worlds_match_their_golden_and_meet_their_analogue_s_p() {
        let fractions: Vec<(SyntheticWorld, ClassFractions)> = SyntheticWorld::ALL
            .iter()
            .map(|&w| (w, fractions_with(w, &mut SynthCache::new())))
            .collect();
        golden!("albedo/worlds", &worlds_text(&fractions));
        for (world_, fractions) in &fractions {
            let header = world(*world_).header();
            let Some(target) = analogue_p(*world_) else {
                continue;
            };
            if header.surface_pressure().value() >= THIN_ATMOSPHERE_PRESSURE.value() {
                continue;
            }
            let solved = albedo_scale(fractions, header.palette(), target).expect("named");
            assert!(
                solved.relative_residual()[1].abs() < 0.01,
                "{world_:?}: {solved:?}"
            );
        }
    }

    /// Every golden file of `tests/golden/albedo/` carries `GENERATOR_VERSION`.
    #[test]
    fn albedo_every_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("albedo");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the albedo golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert_eq!(seen, ALBEDO_GOLDENS, "the goldens in {}", dir.display());
    }
}
