//! The tests of P11.T10: a system that a supernova has unbound shows only the stars it still
//! holds, chosen at one choke point, `SystemStars::shown_at`, which the system's summary, state and
//! brief and the range brief's paired route read. Its hold is deleted (the 27 fold); these are its
//! tests of the new arm.
//!
//! - Three pinned massive systems of the binaries' census sampler (`testing::massive_records`):
//!   a pair whose primary's supernova releases its 19.6 M☉ companion, a pair whose companion's
//!   supernova leaves the primary's black hole, and a quadruple whose outer pair its inner pair's
//!   supernova releases. The first two show the primary alone and the third the primary's own pair
//!   on its drawn orbit, and before the disruption each shows every drawn star.
//! - The range brief counts the stars the summary shows, on every route, and the same system twice
//!   is the same.
//!
//! The slow agreement tests of the engine's supernovae with plan 06's kicks and plan 08's
//! displaced objects and runaways (`binary_kick_*`, in `stellar/binary/supernova/agreement.rs`)
//! count the released stars over 10⁴ massive systems.
//!
//! The movers of the fold, found by turning the arm on at the version (and back off) over the
//! sim's, the server's and the fitting crate's tests (2026-11-01), are listed in plan 11's Risks,
//! "P11.T10, as built".

use std::sync::OnceLock;

use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::Component;
use crate::stellar::binary::testing::massive_records;
use crate::stellar::brief::BriefModel;
use crate::stellar::multiplicity::{HierarchyNode, NodeIndex, StarIndex, SystemHierarchy};
use crate::stellar::system::{Shown, SystemStars, disrupting_supernova};
use crate::time::UniverseTime;
use crate::units::Years;

/// The seed of the Milky Way fixture, the binaries' agreement tests' (P11.T10).
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The seed of the massive systems' stream, the agreement tests'.
const RECORDS_SEED: u64 = 0x0b17_0010_5eed_000b;

/// The massive systems the pins are among: 80 at 24, and at generator version 25 as many as reach
/// [`OUTER_PAIR`]'s, the stream drawing each record from its place in it.
const RECORDS: u32 = 286;

/// A pair of 19.69 and 19.64 M☉ whose primary's supernova at 10.47 Myr unbinds it and releases
/// its companion on the main sequence (found by search, 2026-11-01: the first such pair of two
/// stars among the sampler's systems).
const RELEASED: usize = 24;

/// A pair of 34.6 M☉ whose primary leaves a black hole of 6.41 M☉ that its companion's supernova
/// unbinds at 8.48 Myr (the first such at 27; until 27 it was system 53, a 38.8 M☉ primary's black
/// hole of 6.29 M☉ unbound at 9.07 Myr, which 27's engine leaves bound).
const BY_THE_COMPANION: usize = 163;

/// A quadruple `((0, 1), (2, 3))` of a 23.8 M☉ primary whose outer pair the supernova of its star
/// 2 unbinds at 22.4 Myr, releasing its star 3, a black hole of 3.72 M☉ at the epoch, and whose
/// inner pair of two black holes stays (the first such at generator version 25). At 24 it was the
/// 73rd system, a 31.2 M☉ primary's; at 25 its inner pair's secondary unbinds that pair at
/// 11.2 Myr, and the arm shows the primary alone.
const OUTER_PAIR: usize = 285;

/// The system's age at which every pinned system is whole: 5 Myr, before each one's first
/// supernova.
const WHOLE_AGE: Years = Years::new(5.0e6);

/// The Milky Way fixture, under the version's rules.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The sampler's massive systems.
fn records() -> &'static [SystemRecord] {
    static RECORDS_OF: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS_OF.get_or_init(|| massive_records(galaxy(), RECORDS, RECORDS_SEED))
}

/// The time at which a system of the sampler is `age` old.
fn time_at_age(age: Years) -> UniverseTime {
    let record = &records()[0];
    let years = (age.value() - record.age_at_epoch().value()).round();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number of years within 60 Myr of the epoch"
    )]
    let years = years as i64;
    UniverseTime::from_julian_years(years).expect("a time within the representable range")
}

/// The system `index` of the sampler.
fn system(index: usize) -> SystemStars {
    SystemStars::generate(galaxy(), &records()[index])
}

/// Bodies of the summary's stars at `t`.
fn bodies(stars: &SystemStars, t: UniverseTime) -> Vec<u16> {
    stars
        .summary_at(t)
        .stars()
        .iter()
        .map(|s| s.body().body_index())
        .collect()
}

/// A pinned system shows at the epoch the stars that `shown` holds, with the shown hierarchy, state
/// and brief; before its first supernova it shows every drawn star. Returns the system.
fn assert_shown(index: usize, shown: Shown) -> SystemStars {
    let stars = system(index);
    let now = UniverseTime::EPOCH;
    let count = usize::from(stars.star_count());
    assert_eq!(stars.shown_at(now), shown);
    let kept: Vec<u16> = (0..count)
        .map(|k| u16::try_from(k).expect("few stars"))
        .filter(|&k| shown.holds(u8::try_from(k).expect("few stars")))
        .collect();
    assert_eq!(bodies(&stars, now), kept);
    let summary = stars.summary_at(now);
    let hierarchy = summary.hierarchy().expect("the system exists");
    assert_eq!(usize::from(hierarchy.star_count()), kept.len());
    let state = stars.state_at(now).expect("the system exists");
    assert_eq!(state.stars().len(), kept.len());
    let mass: f64 = state.stars().iter().map(|s| s.mass().value()).sum();
    assert!(
        (state.mass().value() - mass).abs() <= 1e-12 * mass,
        "the system's mass is its shown stars' sum: {state:?}"
    );
    // The shown hierarchy is the summary's, and its bodies index the state's stars, as the sky's
    // census walks them.
    let shown_hierarchy = stars.hierarchy_at(now);
    assert_eq!(Some(shown_hierarchy.as_ref()), summary.hierarchy());
    for (k, slot) in shown_hierarchy.stars().iter().enumerate() {
        assert_eq!(usize::from(slot.body().body_index()), k);
        assert!(
            k < state.stars().len(),
            "body {k} of {} stars",
            state.stars().len()
        );
    }
    for pair in state.pairs() {
        assert!(
            matches!(
                shown_hierarchy.node(pair.node()),
                HierarchyNode::Pair { .. }
            ),
            "the state's pair {:?} is a pair of the shown hierarchy",
            pair.node()
        );
    }
    let brief = stars.brief_at(now).expect("the primary exists");
    assert_eq!(usize::from(brief.star_count()), kept.len());
    // Before its first supernova the system shows every drawn star, on the drawn hierarchy.
    let whole = time_at_age(WHOLE_AGE);
    assert_eq!(stars.shown_at(whole), Shown::Drawn);
    assert_eq!(bodies(&stars, whole).len(), count);
    assert_eq!(stars.summary_at(whole).hierarchy(), Some(stars.hierarchy()));
    stars
}

/// A pair whose primary's supernova releases its companion shows its primary's remnant alone from
/// the supernova on: one star, the single hierarchy, no pair, and a brief of one star.
#[test]
fn a_companion_its_primary_s_supernova_releases_is_left_out_from_its_release() {
    let alone = assert_shown(RELEASED, Shown::Alone);
    let pair = &alone.pairs()[0];
    let sn = disrupting_supernova(pair.timeline()).expect("the pinned pair is unbound");
    assert_eq!(sn.component(), Component::Primary);
    let before = pair
        .timeline()
        .state_at(Years::new(sn.age().value() * (1.0 - 1e-9)));
    assert_eq!(before.stars()[1].phase(), Phase::MainSequence);
    let now = UniverseTime::EPOCH;
    assert_eq!(
        alone.summary_at(now).hierarchy(),
        Some(&SystemHierarchy::single(
            alone.record().id(),
            alone.record().primary_initial_mass(),
            alone.hierarchy().star(StarIndex::PRIMARY).kind(),
        ))
    );
    assert_eq!(alone.state_at(now).expect("exists").pairs().len(), 0);
    // From the supernova's own age, not before it.
    let age = |t: f64| time_at_age(Years::new(sn.age().value() + t));
    assert_eq!(alone.shown_at(age(-2.0)), Shown::Drawn);
    assert_eq!(alone.shown_at(age(1.0)), Shown::Alone);
}

/// A pair whose companion's supernova unbinds it leaves its primary, a black hole, alone: the
/// companion's remnant leaves at its kick.
#[test]
fn a_pair_its_companion_s_supernova_unbinds_leaves_the_primary_alone() {
    let alone = assert_shown(BY_THE_COMPANION, Shown::Alone);
    let sn = disrupting_supernova(alone.pairs()[0].timeline()).expect("the pinned pair is unbound");
    assert_eq!(sn.component(), Component::Secondary);
    assert_eq!(
        alone.state_at(UniverseTime::EPOCH).expect("exists").stars()[0].phase(),
        Phase::BlackHole
    );
}

/// A quadruple whose outer pair a supernova unbinds is left as its primary's own pair, on the
/// drawn orbit, with that pair's state, the root of the shown hierarchy.
#[test]
fn a_quadruple_whose_outer_pair_is_unbound_shows_its_inner_pair() {
    let node = system(OUTER_PAIR)
        .hierarchy()
        .pairs()
        .map(|(n, _)| n)
        .find(|n| n.get() == 1)
        .expect("the inner pair is node 1 of ((0, 1), (2, 3))");
    let alone = assert_shown(OUTER_PAIR, Shown::Pair(node));
    let now = UniverseTime::EPOCH;
    let state = alone.state_at(now).expect("exists");
    assert_eq!(state.pairs().len(), 1);
    assert_eq!(state.pairs()[0].node(), NodeIndex::ROOT);
    let engine = alone
        .binary_state_at(node, now)
        .expect("the engine ran the drawn inner pair");
    assert_eq!(state.pairs()[0].orbit(), engine.orbit());
    let HierarchyNode::Pair { orbit, .. } = *alone.hierarchy().node(node) else {
        panic!("node 1 is the inner pair");
    };
    let shown = alone.summary_at(now).hierarchy().cloned().expect("exists");
    let (root, drawn) = shown.pairs().next().expect("a pair");
    assert_eq!(root, shown.root());
    assert_eq!(drawn, &orbit);
}

/// The range brief of a system a supernova has unbound counts the stars its summary shows, as the
/// system's own brief does.
#[test]
fn the_range_brief_counts_the_stars_shown() {
    let record = &records()[RELEASED];
    let now = UniverseTime::EPOCH;
    let brief = BriefModel::new(galaxy(), record)
        .brief_at(now)
        .expect("the primary exists");
    assert_eq!(brief.star_count(), 1);
    assert_eq!(
        Some(brief),
        SystemStars::generate(galaxy(), record).brief_at(now)
    );
}

/// The same system twice is the same, and among the sampled systems some a supernova has unbound
/// by the epoch and many it has not.
#[test]
fn the_shown_stars_are_deterministic() {
    let now = UniverseTime::EPOCH;
    let (mut unbound, mut whole) = (0, 0);
    for record in records() {
        let stars = SystemStars::generate(galaxy(), record);
        assert_eq!(stars, SystemStars::generate(galaxy(), record));
        if stars.shown_at(now) == Shown::Drawn {
            whole += 1;
        } else {
            unbound += 1;
        }
    }
    println!("{unbound} unbound, {whole} whole of {RECORDS}");
    assert!(
        unbound > 5 && whole > 20,
        "{unbound} unbound, {whole} whole"
    );
}

/// Every sampled system's range brief, whatever its route, counts the stars the system shows at
/// the epoch: the first attempt's count for a system shown whole, and the shown count for one a
/// supernova has unbound.
#[test]
fn every_route_counts_the_stars_shown() {
    let now = UniverseTime::EPOCH;
    for record in records() {
        let stars = SystemStars::generate(galaxy(), record);
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
