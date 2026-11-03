//! The tests of P11.T10's fix: every core collapse acts on the orbits it unbinds, a pair a star of
//! which collapses by its age run through the engine (`evolve`'s pre-test) and each run pair's
//! supernova applied to the orbits above it (`SystemStars`' outer releases, read by
//! `SystemStars::shown_at`). Its hold is deleted (the 27 fold); these are its tests of the new arm.
//!
//! - Pinned massive systems of the binaries' census sampler (`testing::massive_records`): a wide
//!   pair the pre-test passes over whose primary's collapse is recorded and unbinds it, and two
//!   triples whose inner pair stays bound through a low-mode collapse, one releasing its outer
//!   star and one keeping it, with the kernel's escape speed checked on a constructed orbit.
//! - The range brief counts the stars the summary shows, on every route.
//! - A detached step held at its floor stops at a pinned primary's collapse inside it, on the
//!   census galaxy's system the floor stepped past.
//!
//! The slow agreement tests (`binary_kick_*`, in `stellar/binary/supernova/agreement.rs`) count
//! what the fix releases over 10⁴ massive systems under every 27 hold.
//!
//! The movers of the fold, found by turning the arm on at the version over the sim's, the server's
//! and the fitting crate's tests (2026-11-01), are listed in plan 11's records.

use std::sync::OnceLock;

use crate::coords::SystemVelocity;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
use crate::orbit::KeplerElements;
use crate::rng::Seed;
use crate::stellar::binary::collapses::{OuterOrbit, PointLoss, outer_orbit_after};
use crate::stellar::binary::testing::massive_records;
use crate::stellar::binary::{BinaryInput, Component, can_interact};
use crate::stellar::brief::BriefModel;
use crate::stellar::multiplicity::{HierarchyNode, NodeIndex, StarIndex};
use crate::stellar::system::{
    PairTimeline, Shown, SystemStars, clock_time_of_death, disrupting_supernova,
};
use crate::time::UniverseTime;
use crate::units::Years;

/// The Milky Way fixture, under the version's rules: the binaries' agreement tests' (P11.T10).
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The binaries' census sampler's massive systems the pins are among.
fn records() -> &'static [SystemRecord] {
    static RECORDS_OF: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS_OF.get_or_init(|| massive_records(galaxy(), RECORDS, RECORDS_SEED))
}

/// The seed of the Milky Way fixture, the binaries' agreement tests'.
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The seed of the massive systems' stream, the agreement tests'.
const RECORDS_SEED: u64 = 0x0b17_0010_5eed_000b;

/// The massive systems the pins are among.
const RECORDS: u32 = 1_100;

/// The sampler's first system, `0x8200_9920_0000_0000`: a pair of 17.8 and 4.4 M☉ at a period of
/// 2.9 × 10⁶ days that the pre-test passes over, whose primary dies as a type II-P supernova at
/// 12.4 Myr (research-leans-26).
const WIDE_PAIR: usize = 0;

/// A triple `((0, 1), 2)` of 11.4, 7.2 and 2.7 M☉, its outer star at 557 au (e = 0.82), whose
/// inner pair stays bound through its primary's low-mode collapse at 21.75 Myr (a 4.9 km/s
/// kick) and recoils at 31 km/s (found by search, 2026-11-01).
const RELEASING_TRIPLE: usize = 284;

/// A triple `((0, 1), 2)` of 10.8, 4.5 and 3.3 M☉, its outer star at 143 au (e = 0.67), whose
/// inner pair stays bound through its primary's low-mode collapse at 24.9 Myr (a 10.3 km/s kick)
/// and recoils at 6.3 km/s (the first such, 2026-11-01).
const KEEPING_TRIPLE: usize = 1_099;

/// A quadruple `(((0, 1), 2), 3)` of 48.3, 13.1, 6.9 and 6.7 M☉ whose inner pair, at 1.1 au,
/// stays bound through its primary's supernova at 4.90 Myr and recoils at 18.5 km/s: the orbit
/// of its star 2, at 17.6 au, stays bound, and passes its recoil to the outer one, at 472 au,
/// which it unbinds (found by search, 2026-11-01).
const PASSED_UP: usize = 67;

/// A quadruple `(((0, 1), 2), 3)` of 12.3, 1.6, 8.2 and 1.9 M☉ whose inner pair, at 4.3 au, merges,
/// and whose product's supernova at 18.9 Myr, kicked at 102 km/s, unbinds the orbit of its star 2,
/// at 86 au, and then the outer one, at 848 au (found by search, 2026-11-01).
const MERGED_PRODUCT: usize = 6;

/// The system `index` of the sampler.
fn system(index: usize) -> SystemStars {
    SystemStars::generate(galaxy(), &records()[index])
}

/// The clock time at which `stars` is `age` old.
fn on_the_clock(stars: &SystemStars, age: Years) -> UniverseTime {
    clock_time_of_death(stars.record().age_at_epoch(), age).expect("a sampled age is on the clock")
}

/// The age just before and just after `age`, years.
fn either_side(age: Years) -> [Years; 2] {
    [1.0 - 1e-9, 1.0 + 1e-9].map(|f| Years::new(age.value() * f))
}

/// A wide pair the pre-test passes over is run for its primary's collapse: its supernova is at plan
/// 06's age with plan 06's remnant and kick, and unbinds it (under the held arm, deleted, P11.T4.l's
/// closed form recorded the same supernova and unbound the drawn orbit).
#[test]
fn a_wide_pair_records_its_primary_s_collapse_and_is_unbound() {
    let applied = system(WIDE_PAIR);
    assert_eq!(
        applied.record().id(),
        crate::id::SystemId::from_raw(0x8200_9920_0000_0000).expect("an ID")
    );
    let applied_pair = applied
        .pairs()
        .iter()
        .find(|p| p.stars() == [StarIndex::PRIMARY, StarIndex::from_body(1).expect("a star")])
        .expect("the primary's pair is run for its remnant");
    let input_until = applied_pair.timeline().until();
    assert!(!can_interact(&wide_input(&applied), input_until));
    let model = applied.primary();
    let death = model.death().expect("the primary collapses by 60 Myr");
    assert!(death.kind().is_sudden());
    let sn = disrupting_supernova(applied_pair.timeline()).expect("the collapse unbinds the pair");
    assert_eq!(sn.component(), Component::Primary);
    let at = death.age().value();
    assert!(
        (sn.age().value() - at).abs() <= 4.0 * f64::EPSILON * at,
        "at {} against plan 06's {at}",
        sn.age().value()
    );
    assert_eq!(sn.kick(), model.natal_kick());
    assert_eq!(Some(sn.remnant().kind()), model.remnant().map(|r| r.kind()));
    assert!(!sn.bound());
    // The system shows the primary's remnant alone from the collapse.
    let now = UniverseTime::EPOCH;
    assert_eq!(applied.shown_at(now), Shown::Alone);
    assert_eq!(applied.summary_at(now).stars().len(), 1);
}

/// The input of the pair of `stars`' first two stars, as `run_pairs` builds it.
fn wide_input(stars: &SystemStars) -> BinaryInput {
    let [first, second] = [0, 1].map(|k| &stars.stars()[k]);
    let HierarchyNode::Pair { orbit, .. } = *stars.hierarchy().node(NodeIndex::ROOT) else {
        panic!("a pair");
    };
    BinaryInput::new(
        first.initial_mass(),
        second.initial_mass(),
        *first.composition(),
        orbit,
        [first.draws().clone(), second.draws().clone()],
        first.age_at_epoch(),
    )
    .expect("a pair of two stars")
}

/// An outer orbit of point masses is unbound by its inner member's recoil where the relative
/// velocity after reaches the escape speed of the mass left at the separation, and only there:
/// a circular orbit at 500 au of an inner pair of 10 M☉ and an outer star of 3 M☉, the pair
/// losing 0.3 M☉ in a low-mode collapse. With `v` the orbital speed and `v_e` that escape speed,
/// a recoil across the orbit's plane releases it above √(`v_e`² − `v`²) and keeps it below; any
/// recoil above `v_e` + `v` releases it and any below `v_e` − `v` keeps it. Its winds' loss
/// before, in Jeans mode, never unbinds it.
#[test]
fn an_outer_orbit_is_unbound_by_a_recoil_above_its_escape_speed() {
    use crate::orbit::{Eccentricity, Orientation};
    use crate::units::consts::{GRAVITATIONAL_CONSTANT, METRES_PER_AU, SOLAR_MASS_KG};
    use crate::units::{GravitationalParameter, Radians, Seconds, SolarMasses};
    let (inner, outer, lost) = (10.0, 3.0, 0.3);
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(inner + outer));
    let a = 500.0 * METRES_PER_AU;
    let period = std::f64::consts::TAU * (a * a * a / mu.value()).sqrt();
    let orbit = KeplerElements::from_period(
        Seconds::new(period),
        mu,
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(1.0),
    )
    .expect("an orbit");
    let t = UniverseTime::EPOCH;
    let v = (mu.value() / a).sqrt();
    let escape = (2.0 * GRAVITATIONAL_CONSTANT * SOLAR_MASS_KG * (inner + outer - lost) / a).sqrt();
    let after = |kick: [f64; 3], before: [f64; 2]| {
        outer_orbit_after(
            &orbit,
            t,
            inner + outer,
            PointLoss {
                member: 0,
                before_msun: before,
                after_msun: before[0] - lost,
                kick: SystemVelocity::new(kick),
            },
        )
    };
    let threshold = (escape * escape - v * v).sqrt();
    let across = |speed: f64| after([0.0, 0.0, speed], [inner, outer]);
    assert!(matches!(across(0.99 * threshold), OuterOrbit::Bound(_)));
    assert!(matches!(across(1.01 * threshold), OuterOrbit::Unbound(_)));
    for direction in [
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.6, 0.0, 0.8],
        [-0.48, 0.6, 0.64],
    ] {
        let kick = |speed: f64| direction.map(|c| c * speed);
        assert!(
            matches!(
                after(kick(1.001 * (escape + v)), [inner, outer]),
                OuterOrbit::Unbound(_)
            ),
            "{direction:?}"
        );
        assert!(
            matches!(
                after(kick(0.999 * (escape - v)), [inner, outer]),
                OuterOrbit::Bound(_)
            ),
            "{direction:?}"
        );
    }
    // A bound orbit's barycentre recoils at the inner member's share of the kick, its mass after.
    let OuterOrbit::Bound(recoil) = across(0.5 * threshold) else {
        panic!("bound");
    };
    let expected = 0.5 * threshold * (inner - lost) / (inner + outer - lost);
    assert!((recoil.metres_per_second()[2] - expected).abs() <= 1e-9 * expected);
    // Winds that have taken half of each member widen the orbit in Jeans mode: with no kick it
    // stays bound, where the drawn velocity at half the mass would be at the escape speed.
    let halved = after([0.0; 3], [0.5 * inner, 0.5 * outer]);
    assert!(matches!(halved, OuterOrbit::Bound(_)), "{halved:?}");
}

/// A triple whose inner pair stays bound through a low-mode collapse releases its outer star
/// where the pair's recoil exceeds the outer orbit's escape speed there, and shows the primary's
/// own pair from the collapse; where the recoil is below it, the outer star stays on its drawn
/// orbit. Before the collapse each shows every drawn star.
#[test]
fn a_triple_s_outer_star_is_released_by_its_inner_pair_s_recoil_above_its_escape_speed() {
    let now = UniverseTime::EPOCH;
    for (index, releases) in [(RELEASING_TRIPLE, true), (KEEPING_TRIPLE, false)] {
        let applied = system(index);
        assert_eq!(applied.star_count(), 3, "{index}");
        let inner = applied
            .pairs()
            .iter()
            .find(|p| p.stars()[0] == StarIndex::PRIMARY)
            .expect("the inner pair is run");
        let sn = inner.timeline().supernovae()[0];
        assert!(sn.bound(), "{index}");
        assert_eq!(
            sn.kick().map(|k| k.mode()),
            Some(crate::stellar::remnant::KickMode::Low),
            "{index}"
        );
        let recoil = sn.velocities()[0].speed().value();
        let escape = escape_speed_at(&applied, inner, sn.age());
        assert_eq!(
            recoil > escape,
            releases,
            "{index}: {recoil} against {escape} m/s"
        );
        let [early, late] = either_side(sn.age());
        let at = |age: Years| on_the_clock(&applied, age);
        assert_eq!(applied.shown_at(at(early)), Shown::Drawn, "{index}");
        assert_eq!(applied.summary_at(at(early)).stars().len(), 3, "{index}");
        if releases {
            let release = applied.releases()[0];
            assert_eq!(release.node(), NodeIndex::ROOT);
            assert_eq!(release.age(), sn.age());
            // The members part faster than the escape speed of what is left.
            let [a, b] = release.velocities().map(|v| v.metres_per_second());
            let parting = (0..3)
                .map(|k| (b[k] - a[k]) * (b[k] - a[k]))
                .sum::<f64>()
                .sqrt();
            assert!(parting > escape, "{index}: {parting} against {escape} m/s");
            assert_eq!(applied.shown_at(at(late)), Shown::Pair(inner.node()));
            assert_eq!(applied.summary_at(now).stars().len(), 2);
            assert_eq!(applied.hierarchy_at(now).star_count(), 2);
            let brief = applied.brief_at(now).expect("the primary exists");
            assert_eq!(brief.star_count(), 2);
        } else {
            assert!(applied.releases().is_empty(), "{index}");
            assert_eq!(applied.shown_at(now), Shown::Drawn);
            assert_eq!(applied.summary_at(now).stars().len(), 3);
        }
    }
}

/// The escape speed of the outer orbit of `stars`, a triple, at the supernova of its inner pair
/// `inner` at `age`, m/s: of the mass left at the separation, both widened in Jeans mode as
/// `outer_orbit_after` reads them.
fn escape_speed_at(stars: &SystemStars, inner: &PairTimeline, age: Years) -> f64 {
    use crate::units::consts::{GRAVITATIONAL_CONSTANT, SOLAR_MASS_KG};
    let HierarchyNode::Pair { orbit, .. } = *stars.hierarchy().node(NodeIndex::ROOT) else {
        panic!("a triple's root is a pair");
    };
    let [early, late] = either_side(age);
    let pair = |at: Years| inner.timeline().state_at(at).total_mass().value();
    let t = on_the_clock(stars, age);
    let outer = stars.stars()[2].state_at(t).expect("formed").mass().value();
    let f = stars.hierarchy().node_mass(NodeIndex::ROOT).value() / (pair(early) + outer);
    let (r, _) = orbit.relative_state_at(t);
    let separation = r.metres().iter().map(|c| c * c).sum::<f64>().sqrt() * f;
    (2.0 * GRAVITATIONAL_CONSTANT * SOLAR_MASS_KG * (pair(late) + outer) / separation).sqrt()
}

/// Every sampled system's range brief, whatever its route, counts the stars the system shows at
/// the epoch, and the same system twice is the same.
#[test]
fn every_route_counts_the_stars_shown() {
    let now = UniverseTime::EPOCH;
    for record in records().iter().take(80) {
        let stars = SystemStars::generate(galaxy(), record);
        assert_eq!(stars, SystemStars::generate(galaxy(), record));
        let model = BriefModel::new(galaxy(), record);
        let shown = match stars.shown_at(now) {
            Shown::Drawn => model.star_count(),
            Shown::Pair(_) => 2,
            Shown::Part { stars, .. } => stars,
            Shown::Alone => 1,
        };
        assert_eq!(
            model.brief_at(now).map(|b| b.star_count()),
            Some(shown),
            "{:?} on the {:?} route",
            record.id(),
            model.route()
        );
    }
}

/// A supernova's recoil passes up an orbit it leaves bound: a quadruple's inner pair recoils
/// through its primary's collapse, the orbit of its star 2 stays bound, and the outer orbit takes
/// that orbit's recoil and is unbound, which releases star 3 and shows the triple below it on its
/// drawn orbits.
#[test]
fn a_bound_orbit_passes_its_recoil_to_the_orbit_above_it() {
    let applied = system(PASSED_UP);
    let h = applied.hierarchy();
    let inner = applied
        .pairs()
        .iter()
        .find(|p| p.stars()[0] == StarIndex::PRIMARY)
        .expect("the inner pair is run");
    let sn = inner.timeline().supernovae()[0];
    assert!(sn.bound());
    let middle = h
        .pairs()
        .map(|(n, _)| n)
        .find(|&n| matches!(*h.node(n), HierarchyNode::Pair { inner: i, .. } if i == inner.node()))
        .expect("the inner pair's orbit");
    assert_ne!(middle, NodeIndex::ROOT);
    let releases = applied.releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].node(), NodeIndex::ROOT);
    assert_eq!(releases[0].age(), sn.age());
    let now = UniverseTime::EPOCH;
    assert_eq!(
        applied.shown_at(now),
        Shown::Part {
            node: middle,
            stars: 3
        }
    );
    // The shown triple is the drawn one below the outer orbit, its pairs renumbered from it.
    let shown = applied.hierarchy_at(now);
    assert_eq!(shown.as_ref(), &h.part(middle));
    assert_eq!(shown.star_count(), 3);
    let state = applied.state_at(now).expect("exists");
    assert_eq!(state.stars().len(), 3);
    assert_eq!(state.pairs().len(), 2);
    for pair in state.pairs() {
        assert!(matches!(
            shown.node(pair.node()),
            HierarchyNode::Pair { .. }
        ));
    }
    assert_eq!(applied.summary_at(now).stars().len(), 3);
    assert_eq!(applied.brief_at(now).map(|b| b.star_count()), Some(3));
    assert_eq!(
        BriefModel::new(galaxy(), &records()[PASSED_UP])
            .brief_at(now)
            .map(|b| b.star_count()),
        Some(3)
    );
}

/// A merged pair's product is a point mass to the orbit above it: its supernova's kick unbinds the
/// orbit of the quadruple's star 2, and the product goes on at its velocity after, which unbinds
/// the outer orbit of star 3 in turn. That leaves the primary's pair alone, its star 1 merged into
/// the primary.
#[test]
fn a_merged_product_s_kick_unbinds_the_orbit_above_it() {
    let applied = system(MERGED_PRODUCT);
    let inner = applied
        .pairs()
        .iter()
        .find(|p| p.stars()[0] == StarIndex::PRIMARY)
        .expect("the inner pair is run");
    let sn = inner.timeline().supernovae()[0];
    let [early, _] = either_side(sn.age());
    let before = inner.timeline().state_at(early);
    assert_eq!(before.orbit(), None);
    assert_eq!(before.stars()[1].phase(), crate::stellar::Phase::NoRemnant);
    assert!(sn.kick().is_some_and(|k| k.speed().value() > 5.0e4));
    let releases = applied.releases();
    assert_eq!(releases.len(), 2);
    assert!(releases.iter().all(|r| r.age() == sn.age()));
    assert_eq!(releases[1].node(), NodeIndex::ROOT);
    let HierarchyNode::Pair {
        inner: kept, outer, ..
    } = *applied.hierarchy().node(releases[0].node())
    else {
        panic!("a release is a pair's");
    };
    assert_eq!(kept, inner.node());
    assert!(matches!(
        *applied.hierarchy().node(outer),
        HierarchyNode::Star(s) if s.get() == 2
    ));
    let now = UniverseTime::EPOCH;
    assert_eq!(applied.shown_at(now), Shown::Pair(inner.node()));
}

/// The census galaxy's system `0x81c9_b51f_c000_0001`, in layer E's cell (−55, 212, −4): a
/// thick-disc pair of 75.6 and 23.0 M☉ at Z 0.0063, about 1,800 au apart (e = 0.55), which the
/// engine runs for its primary's collapse. The primary, stripped by its mark, dies at its pin of
/// 4,237,646.62 yr, which a detached step held at its floor passed (found by P15.T10.b's refit at
/// 27, plan 11's records).
const FLOORED_PIN: u64 = 0x81c9_b51f_c000_0001;

/// A detached step held at its floor stops at a pinned primary's collapse inside it: under every
/// 27 hold [`FLOORED_PIN`]'s primary's supernova is recorded at its pin, plan 06's age, where the
/// floor stepped past the pin and the engine never acted on it (its debug assertion fired).
#[test]
fn a_floored_step_stops_at_the_pin() {
    use crate::galaxy::catalogue_classes::binary::BinaryTableRules;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::id::{Layer, SystemId};
    use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
    use crate::stellar::system::{StarModel, draw_metallicity, primary_draws};
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(0x0b17_15a0_0000_0001),
        GalaxyParams::milky_way_like(),
        GalaxyRules {
            binary_tables: BinaryTableRules::Scratch,
            ..GalaxyRules::every_27_hold()
        },
    )
    .expect("the Milky Way fixture builds");
    let key = CellKey::new(Layer::E, [-55, 212, -4]).expect("a cell");
    let mut cell = Vec::new();
    generate_cell(&galaxy, key, &mut cell);
    let id = SystemId::from_raw(FLOORED_PIN).expect("an ID");
    let record = cell.iter().find(|r| r.id() == id).expect("the system");
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        draw_metallicity(&galaxy, record),
        primary_draws(&galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
    )
    .expect("a star");
    let stars = SystemStars::at_attempt(
        &galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    );
    let pair = stars
        .pairs()
        .iter()
        .find(|p| p.stars() == [StarIndex::PRIMARY, StarIndex::from_body(1).expect("a star")])
        .expect("the primary's pair is run for its collapse");
    let at = stars
        .primary()
        .death()
        .expect("the primary collapses")
        .age()
        .value();
    let sn = pair
        .timeline()
        .supernovae()
        .iter()
        .find(|sn| sn.component() == Component::Primary)
        .expect("the primary's supernova is recorded");
    assert!(
        (sn.age().value() - at).abs() <= 4.0 * f64::EPSILON * at,
        "at {} against the pin's {at}",
        sn.age().value()
    );
}
