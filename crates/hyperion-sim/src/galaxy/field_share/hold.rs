//! The tests of P09.T2.c's field share, on the record from generator version 27, against the
//! budget the tests build without it ([`FieldShareRules::None`]; decision-galaxy-25-train §1.5 and
//! §1.6): the version's rules (D3); under every 26 hold but plan 08's displaced classes, the
//! field's factors and its young ages against `1 − φ`, the budget's map and fields
//! bit for bit, the field's records a subset of the budget's with only counts moving (D2, pinned
//! for the WebAssembly suite, D5), the same seed twice (D4), the expected counts, the catalogue
//! classes' and the class table's field, and the census C1.
//!
//! The bit-identity of the version in force is held by the goldens, the young field's pre-pin
//! (`placement/young_field`, D1) among them.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, chi_square_gof};

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::GalacticPosition;
use crate::galaxy::ages::{FieldShareRules, YOUNG_AGE_LIMIT};
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::lbv::LbvProcess;
use crate::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use crate::galaxy::displaced::hold::field_only_rules;
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::features::shares::phi_young;
use crate::galaxy::map::{MapSelection, MapSpec, MapView, render_rows};
use crate::galaxy::placement::{
    CellKey, STELLAR_LAYERS, SUBSTELLAR_LAYERS, SystemRecord, candidate_count, generate_cell,
};
use crate::galaxy::quad::gl_panels;
use crate::galaxy::query::{SubstellarRequest, expected_counts};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::math;
use crate::rng::Seed;
use crate::units::LightYears;

/// The seed of the hold's Milky Way fixture.
const SEED: u64 = 0x0902_2c00_0000_0001;

/// The fixture's budget: [`field`]'s rules with no field share and no carve-out, whose field is
/// the budget.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                field_share: FieldShareRules::None,
                carve_out: CarveOutRules::Unclaimed,
                ..field_only_rules()
            },
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The fixture built under every 26 hold but plan 08's displaced classes: the field gives up φ.
/// The classes redraw layers D and E from their own marks and add their own columns to the budget
/// (P08.T12), so φ's tests hold them back; their composition with φ is P08.T12's tests'
/// (`displaced::hold`, `placement::displaced_hold` and `placement::marks_hold`).
fn field() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            field_only_rules(),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// [`field`]'s field share.
fn share() -> &'static FieldShare {
    field()
        .field_share()
        .expect("every 26 hold installs the field share")
}

/// The young thin disc's component.
fn young_id(galaxy: &Galaxy) -> ComponentId {
    let fields = galaxy.fields();
    fields
        .component_ids()
        .find(|&id| fields.component(id).population() == Population::YoungThinDisc)
        .expect("a galaxy has a young disc")
}

/// Every band, stellar then substellar.
fn bands() -> impl Iterator<Item = MassBand> {
    MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR)
}

/// A point on the young disc's first arm ridge at `r` ly from the centre, in the plane.
fn on_the_ridge(galaxy: &Galaxy, r: f64) -> GalacticPosition {
    let (sin, cos) = math::sin_cos(galaxy.fields().arms().ridge_azimuth(r, 0));
    GalacticPosition::from_light_years([r * cos, r * sin, 0.0]).expect("in the cube")
}

/// The cells of the hold's records: one of every layer at the Sun-like point, on the young arm
/// ridge at the solar circle and in the outer bulge, and layer E's and D's well off the plane.
fn sample_cells(galaxy: &Galaxy) -> Vec<CellKey> {
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
    let bulge = GalacticPosition::from_light_years([2_263.0, 2_263.0, 0.0]).expect("in the cube");
    let high = GalacticPosition::from_light_years([0.0, 26_000.0, 3_000.0]).expect("in the cube");
    let ridge = on_the_ridge(galaxy, 26_000.0);
    let mut cells = Vec::new();
    for spec in STELLAR_LAYERS.iter().chain(&SUBSTELLAR_LAYERS) {
        for at in [&sun, &ridge] {
            cells.push(CellKey::containing(spec.layer(), at).expect("a cell of the cube"));
        }
    }
    for layer in [Layer::A, Layer::E] {
        cells.push(CellKey::containing(layer, &bulge).expect("a cell of the cube"));
    }
    for layer in [Layer::D, Layer::E] {
        cells.push(CellKey::containing(layer, &high).expect("a cell of the cube"));
    }
    cells
}

/// `galaxy`'s records of `key`.
fn cell(galaxy: &Galaxy, key: CellKey) -> Vec<SystemRecord> {
    let mut records = Vec::new();
    generate_cell(galaxy, key, &mut records);
    records
}

/// D3: a galaxy built the usual way carries the version's field share; one built under the
/// version's rules explicitly is the same galaxy; the budget holds none; under every 26 hold it
/// holds the field share, and the other builders keep it.
#[test]
fn a_galaxy_carries_the_version_s_field_share() {
    let explicit = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds");
    let built =
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like()).expect("it builds");
    assert_eq!(explicit, built);
    assert!(explicit.field_share().is_some());
    assert_eq!(held().rules().field_share, FieldShareRules::None);
    assert!(held().field_share().is_none());
    assert_ne!(field(), held());
    assert_eq!(field().rules(), field_only_rules());
    let substellar = field()
        .clone()
        .with_substellar_params(crate::galaxy::substellar::SubstellarParams::generator_default());
    assert_eq!(substellar.field_share(), field().field_share());
    assert!(share().heap_bytes() > 0);
}

/// `Galaxy::with_rules` refuses a change of the field share, which the galaxy reads when it is
/// built.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "the field share is read when a galaxy is built")]
fn with_rules_refuses_to_change_the_field_share() {
    let _ = held().clone().with_rules(GalaxyRules::every_27_hold());
}

/// Each component keeps its field factor of its budget in every band: its population's
/// `field_factor`, but a sub-disc of the old thin disc its own `1 − φ_k` less the carved classes'
/// share, the sub-discs together the old thin disc's; and the young disc's share by age, over the
/// budget's born ages, averages to its factor, its ages being the budget's times `1 − φ`.
#[test]
fn every_component_keeps_its_field_factor() {
    let galaxy = field();
    let (fields, shares, field) = (galaxy.fields(), galaxy.feature_shares(), share());
    for band in bands() {
        let (mut old, mut count) = (0.0, 0.0);
        for (id, component) in fields.component_ids().zip(fields.components()) {
            // The oldest sub-disc's clusters are all dissolved, and a substellar band gives up no
            // carved class, so its factor there is 1.
            let factor = field.factor(id, band);
            assert!(factor > 0.0 && factor <= 1.0, "{id:?} {band:?}: {factor}");
            if component.sub_disc().is_some() {
                old += component.count() * factor;
                count += component.count();
            } else {
                assert_same_bits(factor, shares.field_factor(component.population(), band));
            }
        }
        let population = shares.field_factor(Population::OldThinDisc, band);
        assert!((old / count - population).abs() < 1e-14, "{band:?}");
    }
    // The young disc: its share by age averages, over the budget's ages born and unborn, to its
    // factor, to the quadrature's rounding.
    let young = young_id(galaxy);
    let budget = fields.component(young).ages();
    // The field's pieces' ends hold the budget's, −H and 100 Myr, and every panel's.
    let edges = galaxy.field_ages(young).edges();
    for band in [MassBand::A, MassBand::E, MassBand::BrownDwarf] {
        let mean = gl_panels(
            |a| budget.pdf(Years::new(a)) * field.keeps(young, band, Years::new(a)),
            &edges,
        );
        let factor = field.factor(young, band);
        assert!(
            (mean / factor - 1.0).abs() < 1e-9,
            "{band:?}: {mean} against {factor}"
        );
    }
    // Every other component keeps its factor at every age.
    let thick = fields
        .component_ids()
        .find(|&id| fields.component(id).population() == Population::ThickDisc)
        .unwrap();
    assert_same_bits(
        field.keeps(thick, MassBand::C, Years::new(1.1e10)),
        field.factor(thick, MassBand::C),
    );
    // The young field's ages are the budget's times 1 − φ, renormalised, and its born share is
    // the budget's.
    let ages = galaxy.field_ages(young);
    assert_ne!(ages, budget);
    assert!((ages.born_fraction() - budget.born_fraction()).abs() < 1e-15);
    for a in [-500.0, 1e3, 2e6, 2.05e7, 3.55e7, 6.05e7, 9.95e7, 1e8] {
        let age = Years::new(a);
        let ratio = ages.pdf(age) / budget.pdf(age);
        let keep = field.keeps(young, MassBand::A, age) / field.factor(young, MassBand::A);
        assert!(
            (ratio / keep - 1.0).abs() < 1e-9,
            "{a}: {ratio} against {keep}"
        );
    }
    // Every other component's ages are its budget's, the same value.
    assert!(std::ptr::eq(
        galaxy.field_ages(thick),
        fields.component(thick).ages()
    ));
}

/// The plan's "`map` keeps the budget", and everything else the fields feed: under every 26 hold
/// the fields, the gas, the shares, the substellar abundance and the feature shares are the
/// budget's, and the map is the same bits.
#[test]
fn the_map_and_the_budget_are_unchanged_bit_for_bit() {
    let (held, field) = (held(), field());
    assert_eq!(held.fields(), field.fields());
    assert_eq!(held.gas(), field.gas());
    assert_eq!(held.shares(), field.shares());
    assert_eq!(held.substellar(), field.substellar());
    assert_eq!(held.feature_shares(), field.feature_shares());
    assert_eq!(held.potential(), field.potential());
    assert_eq!(held.params(), field.params());
    for (view, selection) in [
        (MapView::FaceOn, MapSelection::AllSystems),
        (MapView::FaceOn, MapSelection::YoungOnly),
        (MapView::EdgeOn, MapSelection::AllSystems),
    ] {
        let spec =
            MapSpec::new(view, selection, [32, 32], [0.0, 0.0], 2_000.0).expect("a valid map");
        let (mut a, mut b) = (Vec::new(), Vec::new());
        render_rows(held.fields(), &spec, 0..32, &mut a);
        render_rows(field.fields(), &spec, 0..32, &mut b);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_same_bits(*x, *y);
        }
    }
}

/// What the field did to one cell's records: the budget's, kept or not.
#[derive(Debug, Default)]
struct Moves {
    /// The budget's records, by population.
    budget: BTreeMap<&'static str, u64>,
    /// The field's, by population.
    field: BTreeMap<&'static str, u64>,
}

/// The field's records of `key` against the budget's: the same candidates, and the field's
/// records the budget's that it keeps, bit for bit and in order.
fn compare_cell(key: CellKey, moves: &mut Moves) -> Vec<SystemRecord> {
    let (held, field) = (held(), field());
    assert_eq!(candidate_count(held, key), candidate_count(field, key));
    let budget = cell(held, key);
    let kept = cell(field, key);
    let mut rest = budget.iter();
    for record in &kept {
        assert!(
            rest.any(|other| other == record),
            "{:?} of the field is no record of the budget's, or out of order",
            record.id()
        );
    }
    for record in &budget {
        *moves.budget.entry(record.population().name()).or_default() += 1;
    }
    for record in &kept {
        *moves.field.entry(record.population().name()).or_default() += 1;
    }
    kept
}

/// The digest (`f64_digest`) of the field's records of the sample cells in D2 under every 26
/// hold: each record's ID (high, low 32 bits), mass and age, in cell and index order. Blessed
/// natively on x86-64; the wasip1 suite compares it. Re-pinned when P09.T35's carve-out joined
/// every 26 hold, its layer D giving up its ancient Type Ia, and at generator version 25 (it was
/// `0xd107_4ee6_ab56_1bb7` at 24) and 26 (`0x69cc_001d_339e_4831` at 25), whose trains move the
/// records.
const FIELD_RECORDS_DIGEST: u64 = 0x72e0_ec75_6dd6_1e1c;

/// D2 under every 26 hold, and the plan's "only counts move, never the layout of an unaffected
/// cell's survivors": in a cell of every layer at the Sun-like point and on the young arm ridge,
/// two of the outer bulge and two off the plane, the field draws the budget's candidates and keeps
/// a subset of the budget's records, each the budget's bit for bit and in its order; the young
/// disc gives up some, and no other population more than its field factor allows. Named `pinned`
/// for the WebAssembly suite's filter (D5): the arm adds arithmetic on the record's path, so the
/// count and a digest of the records it keeps are pinned, as blessed natively.
#[test]
fn the_field_keeps_a_subset_of_the_budget_s_records_pinned() {
    let mut moves = Moves::default();
    let mut values = Vec::new();
    for key in sample_cells(held()) {
        for record in compare_cell(key, &mut moves) {
            let raw = record.id().raw();
            values.push(f64::from(u32::try_from(raw >> 32).unwrap()));
            values.push(f64::from(u32::try_from(raw & 0xffff_ffff).unwrap()));
            values.push(record.primary_initial_mass().value());
            values.push(record.age_at_epoch().value());
        }
    }
    eprintln!("{moves:?}");
    let young = Population::YoungThinDisc.name();
    let (budget, kept) = (moves.budget[young], moves.field[young]);
    assert!(kept < budget, "the young disc keeps {kept} of {budget}");
    let total = |m: &BTreeMap<&str, u64>| m.values().sum::<u64>();
    assert!(total(&moves.field) < total(&moves.budget));
    let digest = f64_digest(&values);
    eprintln!("{} records, digest {digest:#018x}", values.len() / 4);
    assert_eq!(digest, FIELD_RECORDS_DIGEST, "{digest:#018x}");
}

/// D4: the same seed twice under [`field`]'s rules gives the same field, and the same records.
#[test]
fn the_same_seed_twice_gives_the_same_field() {
    let again = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        field_only_rules(),
    )
    .expect("the Milky Way fixture builds");
    assert_eq!(&again, field());
    for key in sample_cells(&again) {
        assert_eq!(cell(&again, key), cell(field(), key), "{key:?}");
    }
}

/// The plan's test: the young field's ages, placed, against `1 − φ` × the budget's by
/// chi-square, in ten bins of 10 Myr of the born ages. The records are the young disc's born
/// records of layer E's cells in the plane about the young arm ridge at the solar circle, ring by
/// ring outwards, until 2,000 are placed; the expectation is `∫ (1 − φ(a)) p(a) da` over each bin,
/// with φ evaluated at every node ([`phi_young`]), not on the field's panels.
#[test]
fn the_young_field_s_ages_follow_one_less_phi() {
    const WANTED: usize = 2_000;
    let galaxy = field();
    let young = young_id(galaxy);
    let centre = CellKey::containing(Layer::E, &on_the_ridge(galaxy, 26_000.0)).unwrap();
    let [x, y, _] = centre.gen_cell().to_array();
    let mut ages = Vec::with_capacity(WANTED);
    let mut ring = 0_i32;
    while ages.len() < WANTED {
        assert!(ring < 64, "{} young records by ring {ring}", ages.len());
        for i in -ring..=ring {
            for j in -ring..=ring {
                if i.abs().max(j.abs()) != ring {
                    continue;
                }
                for z in [-1, 0] {
                    let key = CellKey::new(Layer::E, [x + i, y + j, z]).unwrap();
                    ages.extend(cell(galaxy, key).iter().filter_map(|record| {
                        let age = record.age_at_epoch().value();
                        (record.component() == Some(young) && age >= 0.0).then_some(age)
                    }));
                }
            }
        }
        ring += 1;
    }
    let bin = 1e7;
    let mut observed = vec![0_u64; 10];
    for &a in &ages {
        // The bins' lower edges at or below the age, from 10 Myr: its bin.
        observed[(1..10).filter(|&k| a >= f64::from(k) * bin).count()] += 1;
    }
    let budget = galaxy.fields().component(young).ages();
    let weight = |lo: f64, hi: f64| {
        let mut edges = vec![lo];
        edges.extend(
            [3e6, 5e6, 3e7, 7.53e7]
                .into_iter()
                .filter(|&k| k > lo && k < hi),
        );
        edges.push(hi);
        gl_panels(
            |a| (1.0 - phi_young(Years::new(a))) * budget.pdf(Years::new(a)),
            &edges,
        )
    };
    let weights: Vec<f64> = (0..10)
        .map(|k| weight(f64::from(k) * bin, f64::from(k + 1) * bin))
        .collect();
    let total: f64 = weights.iter().sum();
    let count = u32::try_from(ages.len()).unwrap();
    let n = f64::from(count);
    let expected: Vec<f64> = weights.iter().map(|w| n * w / total).collect();
    eprintln!("{count} after {ring} rings: {observed:?} against {expected:.1?}");
    let fit = chi_square_gof(&observed, &expected);
    assert_p_value("the young field's ages", fit.p_value, ALPHA);
    // The budget's would put some three in ten below 30 Myr, where nine in ten are in features.
    assert!(observed[..3].iter().sum::<u64>() < u64::from(count) * 15 / 100);
    assert_same_bits(YOUNG_AGE_LIMIT.value(), 10.0 * bin);
}

/// The expected counts read the field: a 100 ly sphere at the Sun-like point expects, in each
/// stellar layer, fewer systems than the budget's, by at most the largest field factor's
/// complement, and the field's systems placed there of layers C to E are a Poisson count of them.
#[test]
fn the_expected_counts_are_the_field_s() {
    let (held, field) = (held(), field());
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let radius = LightYears::new(100.0);
    let all = SubstellarRequest::BrownDwarfsAndRoguePlanets;
    let budget = expected_counts(held, &sun, radius, all);
    let counts = expected_counts(field, &sun, radius, all);
    for layer in Layer::ALL {
        let (b, f) = (budget.get(layer), counts.get(layer));
        assert!(f < b && f > 0.5 * b, "{layer:?}: {f} against {b}");
    }
    let sphere = crate::galaxy::query::QuerySphere::new(
        sun,
        radius,
        crate::time::UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .unwrap();
    for layer in [Layer::C, Layer::D, Layer::E] {
        let mut placed = 0_u64;
        for key in crate::galaxy::query::cells_in_sphere(layer, &sphere) {
            let inside = cell(field, key)
                .iter()
                .filter(|r| {
                    r.epoch_position().distance_to(&sun).value()
                        <= radius.value() * crate::units::consts::METRES_PER_LIGHT_YEAR
                })
                .count();
            placed += u64::try_from(inside).unwrap();
        }
        assert_poisson_count(
            &format!("layer {layer:?}'s field at the Sun"),
            placed,
            counts.get(layer),
            ALPHA,
        );
    }
}

/// The field where the rogue planets saturate (ruling 125), at 60 a star, as the fixture's central
/// cells do: the field thins inside each component's slice of the scaled weights. The centre's
/// rogue-planet cell keeps a subset of the budget's records, each the budget's bit for bit and in
/// its order, as many as the chances the field keeps each with sum to; and a sphere at the centre,
/// every node saturated, expects no more rogue planets than the budget's, between the components'
/// smallest and largest field factors of it. The central populations give the features almost
/// none of their substellar systems, so nearly every record stays.
#[test]
fn where_the_rogue_planets_saturate_the_field_keeps_its_share() {
    let crowded = || {
        crate::galaxy::substellar::SubstellarParams::generator_default()
            .with_rogue_planets_per_star(60.0)
    };
    let budget = held().clone().with_substellar_params(crowded());
    let kept = field().clone().with_substellar_params(crowded());
    let key = CellKey::new(Layer::RoguePlanet, [0, 0, 0]).unwrap();
    assert_eq!(candidate_count(&budget, key), candidate_count(&kept, key));
    let all = cell(&budget, key);
    let survivors = cell(&kept, key);
    let mut rest = all.iter();
    for record in &survivors {
        assert!(
            rest.any(|other| other == record),
            "{:?} of the field is no record of the budget's, or out of order",
            record.id()
        );
    }
    let share = kept
        .field_share()
        .expect("every 26 hold installs the field");
    let chances: Vec<f64> = all
        .iter()
        .map(|record| {
            let id = record.component().expect("a grid record has a component");
            share.keeps(id, MassBand::RoguePlanet, record.age_at_epoch())
        })
        .collect();
    let mean: f64 = chances.iter().sum();
    let variance: f64 = chances.iter().map(|k| k * (1.0 - k)).sum();
    let count = f64::from(u32::try_from(survivors.len()).unwrap());
    let z = (count - mean) / variance.sqrt();
    eprintln!(
        "the centre's rogue planets: {} of {} kept, {mean:.1} expected, z = {z:.2}",
        survivors.len(),
        all.len()
    );
    assert!(survivors.len() <= all.len() && !survivors.is_empty());
    assert_p_value(
        "the saturated cell's field against its chances",
        2.0 * hyperion_testkit::stats::normal_cdf(-z.abs()),
        ALPHA,
    );
    let all_kinds = SubstellarRequest::BrownDwarfsAndRoguePlanets;
    let radius = LightYears::new(0.5);
    let rogue = |galaxy: &Galaxy| {
        expected_counts(galaxy, &GalacticPosition::ORIGIN, radius, all_kinds)
            .get(Layer::RoguePlanet)
    };
    let (b, f) = (rogue(&budget), rogue(&kept));
    let factors = budget
        .fields()
        .component_ids()
        .map(|id| share.factor(id, MassBand::RoguePlanet));
    let (lo, hi) = factors.fold((1.0_f64, 0.0_f64), |(lo, hi), x| (lo.min(x), hi.max(x)));
    eprintln!("the centre's sphere: {f} rogue planets against the budget's {b}, factors {lo}–{hi}");
    assert!(f <= b, "{f} against {b}");
    assert!(
        f >= lo * b * (1.0 - 1e-12) && f <= hi * b * (1.0 + 1e-12),
        "{f} against {b} × [{lo}, {hi}]"
    );
}

/// The catalogue classes of the field's deaths read the field: the core-collapse class's
/// components take their field's share of layer E and the young field's largest death rate, and
/// the Type Ia class's components their field's explosions, each the budget's times the field
/// factor but the young disc's, over the field's ages; and the luminous blue variables' class
/// takes the field's layer-E share and, for the young disc, the young field's ages, the fewer for
/// its hosts' few million years, nine in ten of whose stars are in their nurseries.
#[test]
fn the_catalogue_classes_follow_the_field() {
    let (held, field, share) = (held(), field(), share());
    let young = young_id(field);
    let (budget, kept) = (
        CoreCollapseProcess::new(held),
        CoreCollapseProcess::new(field),
    );
    assert_eq!(budget.components().len(), kept.components().len());
    for (b, k) in budget.components().iter().zip(kept.components()) {
        assert_eq!(b.id(), k.id());
        assert_same_bits(k.share(), b.share() * share.factor(b.id(), MassBand::E));
        if b.id() == young {
            // The young field's ages are weighted to the old, where its stars' deaths are.
            assert!(k.deaths_per_system() > b.deaths_per_system());
        } else {
            assert_same_bits(k.deaths_per_system(), b.deaths_per_system());
        }
    }
    let (budget, kept) = (TypeIaProcess::new(held), TypeIaProcess::new(field));
    assert_eq!(budget.components().len(), kept.components().len());
    for (b, k) in budget.components().iter().zip(kept.components()) {
        assert_eq!(b.id(), k.id());
        let factor = share.factor(b.id(), MassBand::D);
        if b.id() == young {
            // The young field's ages are weighted to the old, past the delay-time's 40 Myr.
            assert!(k.explosions_per_system() > b.explosions_per_system() * factor);
        } else {
            assert_same_bits(
                k.explosions_per_system(),
                b.explosions_per_system() * factor,
            );
        }
    }
    let (lbv_budget, lbv_kept) = (LbvProcess::new(held), LbvProcess::new(field));
    assert_eq!(lbv_budget.components().len(), lbv_kept.components().len());
    for (b, k) in lbv_budget.components().iter().zip(lbv_kept.components()) {
        assert_eq!(b.id(), k.id());
        assert_same_bits(k.share(), b.share() * share.factor(b.id(), MassBand::E));
        if b.id() == young {
            assert!(k.rate_per_year() < b.rate_per_year());
        } else {
            assert_same_bits(k.rate_per_year(), b.rate_per_year());
        }
    }
    let rate = |p: &TypeIaProcess| p.delays().rate_per_year();
    assert!(
        rate(&kept) < rate(&budget),
        "{} {}",
        rate(&kept),
        rate(&budget)
    );
}

/// A grid record's kind, for the census: its layer and population.
fn kind(record: &SystemRecord) -> (Layer, &'static str) {
    (record.layer(), record.population().name())
}

/// The census's galaxies: the fixture and two drawn from their seeds, each under [`field`]'s
/// rules with the field share off and on.
fn census_galaxies() -> Vec<(String, Galaxy, Galaxy)> {
    let off = GalaxyRules {
        field_share: FieldShareRules::None,
        ..field_only_rules()
    };
    let fixture = GalaxyParams::milky_way_like();
    let budget = Galaxy::from_params_with_rules(Seed::new(SEED), fixture, off).unwrap();
    let mut galaxies = vec![("the fixture".to_owned(), budget, field().clone())];
    for seed in [0x0902_2c00_0000_0010_u64, 0x0902_2c00_0000_0011] {
        let params = GalaxyParams::from_seed(
            Seed::new(seed),
            crate::galaxy::imf::MassFunctionKind::default(),
        );
        let budget = Galaxy::from_params_with_rules(Seed::new(seed), params.clone(), off).unwrap();
        let kept =
            Galaxy::from_params_with_rules(Seed::new(seed), params, field_only_rules()).unwrap();
        galaxies.push((format!("seed {seed:#x}"), budget, kept));
    }
    galaxies
}

/// The census's cells of `galaxy`: the sample cells of D2, and every cell of layers C, D and E
/// in the plane, within two cells of it, out to 1,000, 3,000 and 3,000 ly of the Sun-like point.
fn census_cells(galaxy: &Galaxy) -> Vec<CellKey> {
    let mut keys = sample_cells(galaxy);
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    for (layer, reach_ly) in [(Layer::C, 1_000), (Layer::D, 3_000), (Layer::E, 3_000)] {
        let edge = i32::try_from(layer.cell_size_ly()).unwrap();
        let reach = reach_ly / edge;
        let [x, y, z] = CellKey::containing(layer, &sun)
            .unwrap()
            .gen_cell()
            .to_array();
        for i in -reach..=reach {
            for j in -reach..=reach {
                for k in -2..=1 {
                    keys.push(CellKey::new(layer, [x + i, y + j, z + k]).unwrap());
                }
            }
        }
    }
    keys
}

/// C1, the census of the field against the budget under every 26 hold, the field share on against
/// off (decision-galaxy-25-train §1.6), over three galaxies' census cells ([`census_cells`]): the
/// records each layer and population keeps, as a table; the largest move by count and by share,
/// the young disc's; every kept record finite; and every change of class, of which there is none,
/// a survivor of the field being the budget's record bit for bit and in its order. It also counts
/// each galaxy's core-collapse and Type Ia candidates in the core-collapse class's sample cells
/// under either arm.
#[test]
#[ignore = "slow: three galaxies' census cells under either arm, some 10⁵ cells"]
fn census_of_the_field_against_the_budget() {
    for (name, budget_galaxy, field_galaxy) in census_galaxies() {
        let mut budget: BTreeMap<(Layer, &str), u64> = BTreeMap::new();
        let mut kept: BTreeMap<(Layer, &str), u64> = BTreeMap::new();
        for key in census_cells(&budget_galaxy) {
            assert_eq!(
                candidate_count(&budget_galaxy, key),
                candidate_count(&field_galaxy, key)
            );
            let all = cell(&budget_galaxy, key);
            let survivors = cell(&field_galaxy, key);
            let mut rest = all.iter();
            for record in &survivors {
                assert!(
                    rest.any(|other| other == record),
                    "{name}: {:?} of the field changed",
                    record.id()
                );
                assert!(
                    record.age_at_epoch().value().is_finite()
                        && record.primary_initial_mass().value().is_finite()
                );
                *kept.entry(kind(record)).or_default() += 1;
            }
            for record in &all {
                *budget.entry(kind(record)).or_default() += 1;
            }
        }
        let as_f64 = |n: u64| f64::from(u32::try_from(n).unwrap());
        let (mut by_count, mut by_share) = ((0_u64, (Layer::A, "")), (0.0_f64, (Layer::A, "")));
        eprintln!("{name}:");
        eprintln!("| layer | population | budget | field | moved | kept |");
        eprintln!("| --- | --- | --- | --- | --- | --- |");
        for (&key, &b) in &budget {
            let f = kept.get(&key).copied().unwrap_or(0);
            let moved = b - f;
            let share = as_f64(moved) / as_f64(b);
            if moved > by_count.0 {
                by_count = (moved, key);
            }
            if b >= 100 && share > by_share.0 {
                by_share = (share, key);
            }
            eprintln!(
                "| {} | {} | {b} | {f} | {moved} | {:.4} |",
                key.0.letter(),
                key.1,
                1.0 - share
            );
        }
        let total = |m: &BTreeMap<(Layer, &str), u64>| m.values().sum::<u64>();
        eprintln!(
            "{name}: {} of {} records kept; the largest move {by_count:?}, by share {by_share:?}; \
             no record changed class",
            total(&kept),
            total(&budget)
        );
        assert!(by_count.0 > 0);
        assert_eq!(by_share.1.1, Population::YoungThinDisc.name(), "{name}");
        // The catalogue classes of the field's deaths, over the core-collapse class's cells in
        // the plane along +y through the inner disc, and the centre's.
        let class_cells: Vec<[i32; 3]> = (8..28).map(|y| [0, y, 0]).chain([[0, 0, 0]]).collect();
        let deaths = |galaxy: &Galaxy| {
            use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
            let (core, ia) = (CoreCollapseProcess::new(galaxy), TypeIaProcess::new(galaxy));
            class_cells.iter().fold([0_usize; 2], |[c, i], &at| {
                let key = |class| CatalogueCellKey::new(class, at).unwrap();
                [
                    c + CatalogueClassCell::generate(galaxy, &core, key(ClassId::CORE_COLLAPSE))
                        .entries()
                        .len(),
                    i + CatalogueClassCell::generate(galaxy, &ia, key(ClassId::TYPE_IA))
                        .entries()
                        .len(),
                ]
            })
        };
        let ([cc_budget, ia_budget], [cc_field, ia_field]) =
            (deaths(&budget_galaxy), deaths(&field_galaxy));
        eprintln!(
            "{name}: class cells' core-collapse entries {cc_budget} → {cc_field}, Type Ia \
             {ia_budget} → {ia_field}"
        );
        assert!(cc_field < cc_budget && ia_field <= ia_budget, "{name}");
    }
}

/// Plan 08's stay shares take the field factor, and its budget-fed class weights do not: under
/// every 26 hold the class table's class weights and closures are the budget's bit for bit; every
/// component's stay share is the budget's times its field factor, but the young disc's in layers D
/// and E, which are taken over the young field's ages and are the smaller, its massive stars dying
/// young, where nine in ten are in their nurseries; outside layers D and E the stay share is the
/// field factor; and the budget's stay share is the held table's, bit for bit.
#[test]
fn the_class_table_s_stay_shares_take_the_field_factor() {
    use crate::galaxy::displaced::class_table::{ClassTable, FormTable, SOURCES};
    let forms = FormTable::committed();
    let (held, field, share) = (held(), field(), share());
    let budget = ClassTable::build(held, &forms);
    let table = ClassTable::build(field, &forms);
    for band in [MassBand::D, MassBand::E] {
        for (b, k) in budget.classes().iter().zip(table.classes()) {
            assert_same_bits(k.weight(band), b.weight(band));
        }
        for source in SOURCES {
            assert_same_bits(table.closure(band, source), budget.closure(band, source));
        }
    }
    let young = young_id(field);
    for id in field.fields().component_ids() {
        for band in bands() {
            let factor = share.factor(id, band);
            let (b, k) = (budget.stay_share(band, id), table.stay_share(band, id));
            assert_same_bits(table.budget_stay_share(band, id), b);
            if id == young && matches!(band, MassBand::D | MassBand::E) {
                // The young field's ages, older, hold fewer living stars of layer E, and of
                // layer D more already ejected: 0.37 against 0.93 × 0.40 at the fixture.
                eprintln!("the young disc's {band:?}: {k} against the budget's {b} × {factor}");
                assert!(
                    k < b * factor && k > 0.0,
                    "{band:?}: {k} against {b} × {factor}"
                );
            } else {
                assert_same_bits(k, b * factor);
            }
        }
    }
}
