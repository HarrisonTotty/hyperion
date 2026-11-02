//! The tests of P08.T12's hold, [`DISPLACED_VERSION`] (decision-galaxy-25-train §1.5 and §1.6):
//! its tripwire; the version's galaxy holding no class (D3); under every 26 hold, the classes
//! moving layers D and E and nothing else (D2) and the same seed twice (D4); and P08.T12.a's own
//! tests of the shares and components: layer E's expected count against direct integration, the
//! classes' order, their bounds, the share matrix's columns and the map.
//!
//! The bit-identity of the version in force is held by the goldens, the pre-pin of layers D and E
//! (`tests/galaxy_displaced_placement.rs`, D1) among them, and by each test here comparing the held
//! arm with the field. The bump to 26 deletes the tripwire and the held arm, and these become tests
//! of the version's own galaxy.

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::GalacticPosition;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::fields::Fields;
use crate::galaxy::imf::MassBand;
use crate::galaxy::map::{MapSelection, column_density_edge_on, column_density_face_on};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::check_index_headroom;
use crate::galaxy::query::{SubstellarRequest, expected_counts};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::shares::{POPULATION_COLUMNS, ShareMatrix};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::LightYears;

/// The seed of the tests' Milky Way fixture: plan 03's golden fixture's.
pub(crate) const MILKY_WAY_SEED: u64 = 0x0308_d000_0000_0000;

/// The Milky Way fixture under the generator version's rules: the held arm, the field alone.
pub(crate) fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(MILKY_WAY_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// Every 26 hold but plan 08's displaced classes: P09.T2.c's field share and P09.T35's carve-out
/// with the field alone in layers D and E.
pub(crate) const fn field_only_rules() -> GalaxyRules {
    GalaxyRules {
        displaced: DisplacedRules::FieldOnly,
        ..GalaxyRules::every_26_hold_with_its_retargets()
    }
}

/// The Milky Way fixture under every 26 hold but the displaced classes ([`field_only_rules`]): the
/// field the classes are installed over, so that against [`displaced`] the classes alone move.
pub(crate) fn field_only() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(MILKY_WAY_SEED),
            GalaxyParams::milky_way_like(),
            field_only_rules(),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The Milky Way fixture under every 26 hold: the displaced classes installed. A shared fixture
/// built under explicit rules (decision-galaxy-25-train §1.5), the same value whichever test reads
/// it first.
pub(crate) fn displaced() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(MILKY_WAY_SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_26_hold_with_its_retargets(),
        )
        .expect("the Milky Way fixture builds its displaced classes")
    })
}

/// Every class's form takes its density at a shared point ([`forms::FormPoint`], the census's
/// per node) as at the point itself, bit for bit, and every class's arm shares the first arm's
/// geometry, so the census finds one arm point a node: at points over the disc and the halo, the
/// centre and the axis among them (P08.T12's census).
#[test]
fn every_form_s_density_at_a_shared_point_is_its_own() {
    let classes = classes_of(displaced()).classes();
    let geometry = classes
        .iter()
        .find_map(|class| class.form().and_then(DisplacedForm::arm_geometry));
    let geometry = geometry.expect("the young disc's classes keep its arm");
    let arms = classes
        .iter()
        .filter_map(|class| class.form().and_then(DisplacedForm::arm_geometry))
        .inspect(|arm| assert_eq!(*arm, geometry, "an arm of another geometry"))
        .count();
    assert!(arms > 5, "{arms} arms");
    let mut stream = Stream::open(
        Seed::new(MILKY_WAY_SEED),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(5),
    );
    let mut points = vec![PointLy::new(0.0, 0.0, 0.0), PointLy::new(0.0, 0.0, 300.0)];
    points.extend((0..200).map(|_| {
        let r = 60_000.0 * stream.uniform();
        let theta = core::f64::consts::TAU * stream.uniform();
        let z = 6_000.0 * (stream.uniform() - 0.5);
        PointLy::new(r * math::cos(theta), r * math::sin(theta), z)
    }));
    for p in &points {
        let at = forms::FormPoint::new(p, Some(geometry));
        for class in classes {
            if let Some(form) = class.form() {
                assert_same_bits(form.density_at(&at), form.density(p));
            }
        }
    }
}

/// The installed classes of a galaxy that holds them.
fn classes_of(galaxy: &Galaxy) -> &DisplacedFields {
    galaxy
        .fields()
        .displaced()
        .expect("a galaxy under every 26 hold holds its displaced classes")
}

/// The Sun-like point, in the plane 26,000 ly out on the +y axis.
fn sunlike() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube")
}

/// Points over the root cube, most of them near the plane, from the self-test stream `key`.
fn sample_points(n: usize, key: u64) -> Vec<PointLy> {
    let mut stream = Stream::open(
        Seed::new(MILKY_WAY_SEED),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(key),
    );
    (0..n)
        .map(|i| {
            let span = if i % 2 == 0 { 30_000.0 } else { 65_000.0 };
            let height = if i % 3 == 0 { 3_000.0 } else { 300.0 };
            PointLy::new(
                span * (2.0 * stream.uniform() - 1.0),
                span * (2.0 * stream.uniform() - 1.0),
                height * (2.0 * stream.uniform() - 1.0),
            )
        })
        .collect()
}

/// P08.T12 is held until the bump to 27: the rules in force are version 26's and before, under
/// which layers D and E hold the field components alone.
#[test]
fn the_displaced_classes_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < DISPLACED_VERSION,
        "the bump to 27 puts P08.T12 on the record (decision-galaxy-25-train §1; S2 with plan \
         15's production `displaced_forms`): layers D and E with their displaced classes, their \
         stay shares and their candidates. Delete `DISPLACED_VERSION`, `DisplacedRules` with its \
         `FieldOnly` arm, `GalaxyRules::displaced`, the build's `match` on it and this test, in \
         the bump's own checkpoint commit, blessed at 25 (§1.6, C2), and re-pin the 26 arm's \
         figures by name on the production form table"
    );
    let galaxy = held();
    assert_eq!(galaxy.rules().displaced, DisplacedRules::FieldOnly);
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).displaced,
        DisplacedRules::FieldOnly
    );
    assert_eq!(
        GalaxyRules::every_27_hold().displaced,
        DisplacedRules::Displaced
    );
    assert!(galaxy.fields().displaced().is_none());
    assert!(galaxy.class_table().is_none());
    assert_eq!(galaxy.shares().column_count(), POPULATION_COLUMNS);
}

/// D3: the version's galaxy is the one built under the version's rules explicitly, holds no class,
/// and its share matrix and fields are the field galaxy's; under every 26 hold the galaxy is
/// another value.
#[test]
fn the_version_s_galaxy_holds_no_displaced_class() {
    let explicit = Galaxy::from_params_with_rules(
        Seed::new(MILKY_WAY_SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds");
    assert_eq!(held(), &explicit);
    assert_eq!(format!("{:?}", held()), format!("{explicit:?}"));
    assert_ne!(held(), displaced());
    assert_eq!(
        displaced().rules(),
        GalaxyRules::every_26_hold_with_its_retargets()
    );
    // The field galaxy under the new arm is the held galaxy: what the classes are built from.
    assert_eq!(held().params(), displaced().params());
    assert_eq!(
        held().fields().components(),
        displaced().fields().components()
    );
}

/// `with_rules` refuses another displaced arm, which the build reads.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "the displaced classes' arm is read when a galaxy is built")]
fn with_rules_refuses_another_displaced_arm() {
    let _ = field_only()
        .clone()
        .with_rules(GalaxyRules::every_26_hold_with_its_retargets());
}

/// `with_rules` refuses other stellar rules for a galaxy that holds the classes, whose class table
/// read the kick law under its own.
#[test]
#[cfg(debug_assertions)]
#[should_panic(
    expected = "the displaced classes' table was built under the galaxy's stellar rules"
)]
fn with_rules_refuses_other_stellar_rules_once_the_classes_are_installed() {
    let galaxy = displaced();
    let _ = galaxy.clone().with_rules(GalaxyRules {
        stellar: crate::stellar::rules::StellarRules::of_version(GENERATOR_VERSION),
        ..galaxy.rules()
    });
}

/// D4: the same seed twice under every 26 hold gives the same galaxy, classes, shares and
/// densities.
#[test]
fn the_same_seed_twice_gives_the_same_classes() {
    let again = Galaxy::from_params_with_rules(
        Seed::new(MILKY_WAY_SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold_with_its_retargets(),
    )
    .expect("the Milky Way fixture builds its displaced classes");
    assert_eq!(&again, displaced());
    assert_eq!(format!("{again:?}"), format!("{:?}", displaced()));
    for p in sample_points(200, 1) {
        for band in [MassBand::D, MassBand::E] {
            assert_same_bits(
                again.fields().layer_density(again.shares(), band, &p),
                displaced()
                    .fields()
                    .layer_density(displaced().shares(), band, &p),
            );
        }
    }
}

/// The digest (`f64_digest`) of the new arm's layer-D and layer-E densities at the D2 test's 400
/// points, then its classes' counts in both layers, in class order. Blessed natively on x86-64;
/// the wasip1 suite compares it (D5). It is a 26-arm figure on plan 15's provisional form table,
/// re-pinned by name when the production table lands; re-pinned at generator versions 25 and 26,
/// whose layers D and E move.
const DISPLACED_DENSITIES_DIGEST: u64 = 0x4615_5273_32ef_919a;

/// D2 under every 26 hold: the classes move layers D and E, and nothing else. Against the field
/// under every other 26 hold ([`field_only`]), at 400 points over the cube layers A to C and the
/// substellar layers keep their densities bit for bit, and so do the census's counts of them in
/// three spheres, while every layer-D and layer-E density moves at some point and every one stays
/// finite and non-negative; the share matrix's population columns are the field's, bit for bit. Named `pinned` for the WebAssembly suite's filter (D5):
/// the new arm adds the class table's quadrature and the forms' normalisations to the build, so
/// a digest of what it gives is pinned, as blessed natively.
#[test]
fn the_displaced_classes_move_layers_d_and_e_and_no_other_pinned() {
    let (held, displaced) = (field_only(), displaced());
    let (old, new) = (held.fields(), displaced.fields());
    let mut moved = [0_u32; 2];
    let mut values = Vec::new();
    for p in sample_points(400, 2) {
        for band in [MassBand::A, MassBand::B, MassBand::C, MassBand::BrownDwarf] {
            assert_same_bits(
                new.layer_density(displaced.shares(), band, &p),
                old.layer_density(held.shares(), band, &p),
            );
        }
        for (slot, band) in [MassBand::D, MassBand::E].into_iter().enumerate() {
            let (a, b) = (
                old.layer_density(held.shares(), band, &p),
                new.layer_density(displaced.shares(), band, &p),
            );
            assert!(b.is_finite() && b >= 0.0, "{band:?} at {p:?}: {b}");
            if a.total_cmp(&b).is_ne() {
                moved[slot] += 1;
            }
            values.push(b);
        }
    }
    println!(
        "movers at 400 points: layer D {}, layer E {}",
        moved[0], moved[1]
    );
    assert!(moved.iter().all(|&m| m > 0), "{moved:?}");
    for centre in [
        sunlike(),
        GalacticPosition::from_light_years([3_000.0, 1_000.0, 100.0]).expect("in range"),
        GalacticPosition::from_light_years([0.0, 40_000.0, 2_000.0]).expect("in range"),
    ] {
        let (a, b) = (
            expected_counts(
                held,
                &centre,
                LightYears::new(50.0),
                SubstellarRequest::BrownDwarfsAndRoguePlanets,
            ),
            expected_counts(
                displaced,
                &centre,
                LightYears::new(50.0),
                SubstellarRequest::BrownDwarfsAndRoguePlanets,
            ),
        );
        for layer in [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::BrownDwarf,
            Layer::RoguePlanet,
        ] {
            assert_same_bits(b.get(layer), a.get(layer));
        }
        assert!(b.get(Layer::E).total_cmp(&a.get(Layer::E)).is_ne());
    }
    for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
        let (a, b) = (held.shares().row(band), displaced.shares().row(band));
        for (x, y) in a.iter().zip(&b[..POPULATION_COLUMNS]) {
            assert_same_bits(*y, *x);
        }
    }
    let classes = classes_of(displaced);
    for class in classes.classes() {
        values.extend([class.count(MassBand::D), class.count(MassBand::E)]);
    }
    let digest = f64_digest(&values);
    println!("digest {digest:#018x}");
    assert_eq!(digest, DISPLACED_DENSITIES_DIGEST, "re-pin by name");
}

/// P08.T12.a: the classes are listed in the class table's order, those with weight in neither
/// layer have no form, and each layer's order runs by descending systems inside the cube, ties by
/// class index; the share matrix holds each class's systems in its displaced columns and each
/// component's stay share as its factor in rows D and E, and 1 elsewhere.
#[test]
fn the_classes_are_ordered_by_weight_and_the_weightless_are_left_out() {
    let galaxy = displaced();
    let classes = classes_of(galaxy);
    let table = galaxy.class_table().expect("installed");
    let shares = galaxy.shares();
    assert_eq!(classes.classes().len(), class_table::CLASS_COUNT);
    assert_eq!(
        shares.column_count(),
        POPULATION_COLUMNS + class_table::CLASS_COUNT
    );
    for (index, class) in classes.classes().iter().enumerate() {
        let id = DisplacedClassId::new(u16::try_from(index).expect("under 97"));
        assert_eq!(class.key().id(), id);
        for band in [MassBand::D, MassBand::E] {
            assert_same_bits(class.count(band), table.class_count(band, id));
            assert_same_bits(
                shares.displaced_share(band, id),
                table.class_count(band, id),
            );
        }
        for band in [MassBand::A, MassBand::B, MassBand::C, MassBand::BrownDwarf] {
            assert_same_bits(shares.displaced_share(band, id), 0.0);
        }
        let weighted = class.count(MassBand::D) > 0.0 || class.count(MassBand::E) > 0.0;
        assert_eq!(class.form().is_some(), weighted, "class {index}");
    }
    for band in [MassBand::D, MassBand::E] {
        let order = classes.order(band);
        assert!(order.windows(2).all(|w| {
            let (a, b) = (
                classes.class(w[0]).count(band),
                classes.class(w[1]).count(band),
            );
            a > b || (a.total_cmp(&b).is_eq() && w[0] < w[1])
        }));
        assert!(order.iter().all(|&id| classes.class(id).count(band) > 0.0));
        let weighted = classes
            .classes()
            .iter()
            .filter(|c| c.count(band) > 0.0)
            .count();
        assert_eq!(order.len(), weighted);
        println!("layer {band:?}: {} classes with weight", order.len());
    }
    assert_eq!(classes.order(MassBand::A), []);
    let fields = galaxy.fields();
    for (c, component) in fields.component_ids().zip(fields.components()) {
        for band in [MassBand::D, MassBand::E] {
            assert_same_bits(
                shares.component_share(band, component),
                shares.share(band, component.population()) * table.budget_stay_share(band, c),
            );
        }
        for band in [MassBand::A, MassBand::B, MassBand::C, MassBand::BrownDwarf] {
            assert_same_bits(
                shares.component_share(band, component),
                shares.share(band, component.population()),
            );
        }
    }
}

/// Every source's layer-E and layer-D systems are accounted for by the installed shares: its
/// components' counts times their field shares and its classes' systems, with the gone share,
/// make its budget (P08.T9's closure, through the share matrix as placement reads it).
#[test]
fn the_installed_shares_close_each_source_s_budget() {
    let galaxy = displaced();
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let table = galaxy.class_table().expect("installed");
    let classes = classes_of(galaxy);
    for band in [MassBand::D, MassBand::E] {
        for source in class_table::SOURCES {
            let budget = table.source_budget(band, source);
            if budget <= 0.0 {
                continue;
            }
            let staying: f64 = fields
                .component_ids()
                .zip(fields.components())
                .filter(|&(c, _)| table.component_source(c) == source)
                .map(|(_, component)| component.count() * shares.component_share(band, component))
                .sum();
            let displaced: f64 = classes
                .classes()
                .iter()
                .filter(|class| class.key().source() == Some(source))
                .map(|class| class.count(band))
                .sum();
            let gone = if band == MassBand::E {
                table.gone_share(source) * budget
            } else {
                0.0
            };
            let closure = (staying + displaced + gone) / budget;
            assert!(
                (closure - 1.0).abs() < 1e-9,
                "{band:?} {source:?}: {closure}"
            );
        }
    }
}

/// The midpoint integral of layer `band`'s density over the sphere of `radius` about `centre`, in
/// `steps` heights, half as many radii and a quarter as many azimuths: an independent reference
/// for `expected_counts`, sharing no node with its rule, as plan 03's tests take it
/// (`tests/common`'s `reference_sphere_integral`). `density` is the layer's density at a point.
fn midpoint_sphere(
    centre: &PointLy,
    radius: f64,
    steps: u32,
    density: impl Fn(&PointLy) -> f64,
) -> f64 {
    let (nz, nr, na) = (steps, steps / 2, steps / 4);
    let dz = 2.0 * radius / f64::from(nz);
    let da = core::f64::consts::TAU / f64::from(na);
    let mut total = 0.0;
    for iz in 0..nz {
        let z = -radius + (f64::from(iz) + 0.5) * dz;
        let rho = (radius * radius - z * z).max(0.0).sqrt();
        let dr = rho / f64::from(nr);
        for ir in 0..nr {
            let r = (f64::from(ir) + 0.5) * dr;
            for ia in 0..na {
                let (sin, cos) = math::sin_cos((f64::from(ia) + 0.5) * da);
                let p = PointLy::new(centre.x + r * cos, centre.y + r * sin, centre.z + z);
                total += dz * dr * r * da * density(&p);
            }
        }
    }
    total
}

/// P08.T12.a: layer E's expected count in a 50 ly sphere at the Sun-like point is the field's, at
/// its stay shares and the field share φ's factors (P09.T2.c), plus the displaced classes', each
/// by direct integration, to 1%; and the displaced part is there.
#[test]
fn layer_e_in_a_50_ly_sphere_at_the_sun_is_field_plus_displaced() {
    let galaxy = displaced();
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let classes = classes_of(galaxy);
    let centre = PointLy::from(&sunlike());
    let field = midpoint_sphere(&centre, 50.0, 64, |p| {
        let mut densities = [0.0; crate::galaxy::fields::MAX_COMPONENTS];
        fields.densities(p, &mut densities);
        fields
            .component_ids()
            .zip(densities)
            .map(|(id, d)| galaxy.field_component_share(MassBand::E, id) * d)
            .sum::<f64>()
    });
    let moved = midpoint_sphere(&centre, 50.0, 64, |p| {
        classes
            .order(MassBand::E)
            .iter()
            .map(|&id| shares.displaced_share(MassBand::E, id) * classes.density(id, p))
            .sum::<f64>()
    });
    let counted = expected_counts(
        galaxy,
        &sunlike(),
        LightYears::new(50.0),
        SubstellarRequest::None,
    )
    .get(Layer::E);
    let held_count = expected_counts(
        field_only(),
        &sunlike(),
        LightYears::new(50.0),
        SubstellarRequest::None,
    )
    .get(Layer::E);
    println!(
        "layer E in 50 ly at the Sun: {counted:.4} counted, field {field:.4} + displaced \
         {moved:.4} = {:.4} integrated; the field alone held {held_count:.4}",
        field + moved
    );
    assert!(moved > 0.0, "the displaced classes reach the Sun");
    assert!(
        (counted / (field + moved) - 1.0).abs() < 0.01,
        "{counted} against {}",
        field + moved
    );
}

/// The bound of every class and of each layer holds the density: at 32 points of each of 400
/// random cells of layers D and E, every class's density is under its bound and the layer's
/// density under the layer's bound.
#[test]
fn every_class_and_layer_density_is_under_its_cell_bound() {
    let galaxy = displaced();
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let classes = classes_of(galaxy);
    let mut stream = Stream::open(
        Seed::new(MILKY_WAY_SEED),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(3),
    );
    for i in 0..400 {
        let (band, edge): (MassBand, i32) = if i % 2 == 0 {
            (MassBand::D, 64)
        } else {
            (MassBand::E, 128)
        };
        let span = if i % 4 < 2 { 30_000.0 } else { 4_000.0 };
        let at = |s: &mut Stream, scale: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a draw inside ±30,000 ly over a cell's edge fits an i32"
            )]
            let n = (scale * (2.0 * s.uniform() - 1.0) / f64::from(edge)).floor() as i32;
            n * edge
        };
        let min = [
            at(&mut stream, span),
            at(&mut stream, span),
            at(&mut stream, 2_000.0),
        ];
        let cell = CellBox::new(min, edge.unsigned_abs()).expect("an aligned cell");
        let layer = fields.layer_bound(shares, band, &cell);
        let bounds: Vec<f64> = classes
            .order(band)
            .iter()
            .map(|&id| classes.bound(id, &cell))
            .collect();
        for _ in 0..32 {
            let p = PointLy::new(
                f64::from(min[0]) + f64::from(edge) * stream.uniform(),
                f64::from(min[1]) + f64::from(edge) * stream.uniform(),
                f64::from(min[2]) + f64::from(edge) * stream.uniform(),
            );
            for (&id, &bound) in classes.order(band).iter().zip(&bounds) {
                let d = classes.density(id, &p);
                assert!(d <= bound, "class {id:?} at {p:?}: {d} over {bound}");
            }
            let d = fields.layer_density(shares, band, &p);
            assert!(d <= layer, "{band:?} at {p:?}: {d} over {layer}");
        }
    }
}

/// P08.T12.a: index headroom holds for every layer, at the centre and everywhere, under every 26
/// hold, for 50 seeds a shard (`check_index_headroom`).
fn headroom_holds_for_seeds(seeds: std::ops::Range<u64>) {
    for i in seeds {
        let seed = Seed::new(0x0812_a000_0000_0000 + i);
        let params = GalaxyParams::from_seed(seed, crate::galaxy::imf::MassFunctionKind::default());
        let galaxy = Galaxy::from_params_with_rules(
            seed,
            params,
            GalaxyRules::every_26_hold_with_its_retargets(),
        )
        .expect("a drawn galaxy builds its displaced classes");
        assert!(galaxy.fields().displaced().is_some());
        check_index_headroom(&galaxy).unwrap_or_else(|e| panic!("seed {i}: {e}"));
    }
}

/// [`headroom_holds_for_seeds`] at the fixture, for the fast suite.
#[test]
fn index_headroom_holds_under_every_26_hold_at_the_fixture() {
    check_index_headroom(displaced()).expect("the fixture's layers have room");
}

/// P08.T12.a's 200 seeds, the first quarter.
#[test]
#[ignore = "slow: 50 galaxies with their class tables and every layer's headroom"]
fn index_headroom_holds_under_every_26_hold_for_200_seeds_0_to_49() {
    headroom_holds_for_seeds(0..50);
}

/// P08.T12.a's 200 seeds, the second quarter.
#[test]
#[ignore = "slow: 50 galaxies with their class tables and every layer's headroom"]
fn index_headroom_holds_under_every_26_hold_for_200_seeds_50_to_99() {
    headroom_holds_for_seeds(50..100);
}

/// P08.T12.a's 200 seeds, the third quarter.
#[test]
#[ignore = "slow: 50 galaxies with their class tables and every layer's headroom"]
fn index_headroom_holds_under_every_26_hold_for_200_seeds_100_to_149() {
    headroom_holds_for_seeds(100..150);
}

/// P08.T12.a's 200 seeds, the last quarter.
#[test]
#[ignore = "slow: 50 galaxies with their class tables and every layer's headroom"]
fn index_headroom_holds_under_every_26_hold_for_200_seeds_150_to_199() {
    headroom_holds_for_seeds(150..200);
}

/// The face-on column at `(x, y)` (ly) by brute force: the midpoint rule in z over the cube's
/// height, in 2¹⁴ steps, of each component's density at its field's kept share and each class's
/// at its map count.
fn brute_face_on(fields: &Fields, x: f64, y: f64, remnants_only: bool) -> f64 {
    let classes = fields.displaced().expect("installed");
    let steps = 1_u32 << 14;
    let dz = 2.0 * 65_536.0 / f64::from(steps);
    let mut densities = [0.0; crate::galaxy::fields::MAX_COMPONENTS];
    let mut total = 0.0;
    for k in 0..steps {
        let p = PointLy::new(x, y, -65_536.0 + (f64::from(k) + 0.5) * dz);
        if !remnants_only {
            fields.densities(&p, &mut densities);
            for (c, d) in fields.component_ids().zip(densities) {
                total += dz * classes.kept_share(c) * d;
            }
        }
        for (index, class) in classes.classes().iter().enumerate() {
            if let Some(form) = class.form() {
                let id = DisplacedClassId::new(u16::try_from(index).expect("under 97"));
                let selection = if remnants_only {
                    MapSelection::LayerERemnants
                } else {
                    MapSelection::AllSystems
                };
                let count = classes.map_count(id, selection).expect("counted");
                total += dz * count * form.density(&p);
            }
        }
    }
    total
}

/// Each class's face-on column, closed-form or 32-node, against the brute force over the cube's
/// height, at four points: the worst relative error among the classes whose column is over 10⁻⁹ of
/// the largest there.
#[test]
fn each_class_s_face_on_column_matches_a_brute_force_integral() {
    let classes = classes_of(displaced());
    let steps = 1_u32 << 16;
    let dz = 2.0 * 65_536.0 / f64::from(steps);
    for (x, y) in [
        (0.0, 26_000.0),
        (-17_000.0, -19_700.0),
        (2_000.0, 1_000.0),
        (40_000.0, 5_000.0),
    ] {
        let mut worst = (0.0_f64, 0);
        let columns: Vec<(usize, f64, f64)> = classes
            .classes()
            .iter()
            .enumerate()
            .filter_map(|(i, class)| {
                let form = class.form()?;
                let brute: f64 = (0..steps)
                    .map(|k| {
                        dz * form.density(&PointLy::new(
                            x,
                            y,
                            -65_536.0 + (f64::from(k) + 0.5) * dz,
                        ))
                    })
                    .sum();
                Some((i, form.face_on_column(x, y), brute))
            })
            .collect();
        let largest = columns.iter().map(|c| c.2).fold(0.0, f64::max);
        for &(i, ours, brute) in &columns {
            if brute > 1e-9 * largest {
                let error = (ours / brute - 1.0).abs();
                if error > worst.0 {
                    worst = (error, i);
                }
            }
        }
        println!("({x}, {y}): worst class {} at {:e}", worst.1, worst.0);
        assert!(
            worst.0 < 1e-3,
            "({x}, {y}): class {} at {:e}",
            worst.1,
            worst.0
        );
    }
}

/// P08.T12.a: the map counts the displaced classes. Face-on, layer E's remnants alone match a
/// brute-force column to 10⁻³ at four points and every system to 3 × 10⁻³ (the field's own
/// columns carry plan 02's 10⁻³ each), the field at its kept shares and the classes through
/// their closed-form or 32-node columns; the held galaxy's map has no remnants to show; and the
/// all-systems column barely moves, since layers D and E are a few per cent of the systems.
#[test]
fn the_map_counts_the_displaced_classes() {
    let galaxy = displaced();
    for (x, y) in [
        (0.0, 26_000.0),
        (-17_000.0, -19_700.0),
        (2_000.0, 1_000.0),
        (40_000.0, 5_000.0),
    ] {
        for (selection, remnants, tolerance) in [
            (MapSelection::AllSystems, false, 3e-3),
            (MapSelection::LayerERemnants, true, 1e-3),
        ] {
            let map = column_density_face_on(galaxy.fields(), x, y, selection);
            let brute = brute_face_on(galaxy.fields(), x, y, remnants);
            assert!(
                (map / brute - 1.0).abs() < tolerance,
                "{selection:?} at ({x}, {y}): {map} against {brute}"
            );
        }
        let all = column_density_face_on(galaxy.fields(), x, y, MapSelection::AllSystems);
        let field = column_density_face_on(held().fields(), x, y, MapSelection::AllSystems);
        assert!((all / field - 1.0).abs() < 0.02, "{all} against {field}");
        assert_same_bits(
            column_density_face_on(held().fields(), x, y, MapSelection::LayerERemnants),
            0.0,
        );
    }
}

/// The share matrix built by hand from the class table is the galaxy's: the installation is
/// [`ShareMatrix::with_displaced`] of the table's stay shares and counts, nothing else.
#[test]
fn the_installed_share_matrix_is_the_class_table_s() {
    let galaxy = displaced();
    let classes = classes_of(galaxy);
    let (stay, counts) = classes.share_columns(galaxy.fields());
    let by_hand: ShareMatrix = held().shares().clone().with_displaced(&stay, &counts);
    assert_eq!(&by_hand, galaxy.shares());
}

/// P08.T12.a's edge-on view: a pixel of layer E's remnants alone matches a brute-force mean column
/// to 3 × 10⁻³ (plan 02's edge-on tolerance) at two pixels, and the all-systems pixel is the
/// field's, bit for bit, since the view keeps the field's picture for every system (lean,
/// 2026-10-31).
#[test]
fn the_edge_on_map_counts_the_displaced_classes() {
    let fields = displaced().fields();
    let classes = classes_of(displaced());
    for (x, z_lo, z_hi) in [(26_000.0, 0.0, 256.0), (2_000.0, -1_024.0, 1_024.0)] {
        let map = column_density_edge_on(fields, x, z_lo, z_hi, MapSelection::LayerERemnants);
        let (ny, nz) = (1_u32 << 12, 32_u32);
        let (dy, dz) = (
            2.0 * 65_536.0 / f64::from(ny),
            (z_hi - z_lo) / f64::from(nz),
        );
        let mut brute = 0.0;
        for (index, class) in classes.classes().iter().enumerate() {
            let Some(form) = class.form() else { continue };
            let id = DisplacedClassId::new(u16::try_from(index).expect("under 97"));
            let mut integral = 0.0;
            for j in 0..ny {
                let y = -65_536.0 + (f64::from(j) + 0.5) * dy;
                for k in 0..nz {
                    let z = z_lo + (f64::from(k) + 0.5) * dz;
                    integral += dy * dz * form.density(&PointLy::new(x, y, z));
                }
            }
            let count = classes
                .map_count(id, MapSelection::LayerERemnants)
                .expect("counted");
            brute += count * integral / (z_hi - z_lo);
        }
        assert!(
            (map / brute - 1.0).abs() < 3e-3,
            "({x}, {z_lo}–{z_hi}): {map} against {brute}"
        );
        let all = column_density_edge_on(fields, x, z_lo, z_hi, MapSelection::AllSystems);
        let field =
            column_density_edge_on(held().fields(), x, z_lo, z_hi, MapSelection::AllSystems);
        assert_same_bits(all, field);
    }
}

/// A displaced class's form that cannot be built fails the galaxy's build with its own error,
/// whose text names it and whose source is the form's.
#[test]
fn a_form_that_cannot_be_built_is_the_galaxy_s_error() {
    use std::error::Error as _;
    let form = forms::BuildFormError {
        quantity: "beta",
        value: -1.0,
    };
    let error = crate::galaxy::BuildGalaxyError::DisplacedForm(form);
    assert_eq!(
        error.to_string(),
        format!("a displaced class's form cannot be built: {form}")
    );
    assert_eq!(
        error.source().map(ToString::to_string),
        Some(form.to_string())
    );
}
