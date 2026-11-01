//! The tests of P11.T10's hold, [`BINARY_KICKS_VERSION`]: from version 27 a system that a
//! supernova has unbound shows only the stars it still holds, chosen at one choke point,
//! `SystemStars::shown_at`, which the system's summary, state and brief and the range brief's
//! paired route read. Each fast test turns this hold's arm on alone ([`alone_rules`]), so that
//! the other 27 holds move nothing it holds equal.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Three pinned massive systems of the binaries' census sampler (`testing::massive_records`):
//!   a pair whose primary's supernova releases its 19.6 M☉ companion, a pair whose companion's
//!   supernova leaves the primary's black hole, and a quadruple whose outer pair the supernova of
//!   its heavier star unbinds. Under the held arm each shows every drawn star at the epoch; under
//!   the arm the first two show the primary alone and the third the primary's own pair on its
//!   drawn orbit, and before the disruption each is the held arm's bit for bit.
//! - The range brief counts the stars the summary shows, on every route under the arm, and a
//!   system no supernova has unbound is the same under both arms.
//!
//! The slow agreement tests of the engine's supernovae with plan 06's kicks and plan 08's
//! displaced objects and runaways (`binary_kick_*`, in `stellar/binary/supernova/agreement.rs`)
//! count the released stars over 10⁴ massive systems under both arms.
//!
//! The movers under the arm, found by turning it on at the version (and back off) over the sim's,
//! the server's and the fitting crate's tests (2026-11-01), are listed in plan 11's Risks,
//! "P11.T10, as built".

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
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

/// The massive systems the pins are among.
const RECORDS: u32 = 80;

/// A pair of 19.69 and 19.64 M☉ whose primary's supernova at 10.47 Myr unbinds it and releases
/// its companion on the main sequence (found by search, 2026-11-01: the first such pair of two
/// stars among the sampler's systems).
const RELEASED: usize = 24;

/// A pair of 38.8 M☉ whose primary leaves a black hole of 6.29 M☉ that its companion's supernova
/// unbinds at 9.07 Myr (the first such).
const BY_THE_COMPANION: usize = 53;

/// A quadruple `((0, 1), (2, 3))` of a 31.2 M☉ primary whose outer pair the supernova of its star
/// 2 unbinds at 36.0 Myr, releasing its star 3 of 13.6 M☉, and whose inner pair stays (the
/// first such).
const OUTER_PAIR: usize = 72;

/// The system's age at which every pinned system is whole: 5 Myr, before each one's first
/// supernova.
const WHOLE_AGE: Years = Years::new(5.0e6);

/// The version's galaxy rules.
fn version() -> GalaxyRules {
    GalaxyRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn alone_rules() -> GalaxyRules {
    let version = version();
    GalaxyRules {
        stellar: StellarRules {
            binary_kicks: BinaryKicksRules::Alone,
            ..version.stellar
        },
        ..version
    }
}

/// The Milky Way fixture, under the version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// [`held`]'s galaxy with this hold's arm on.
fn alone() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(alone_rules()))
}

/// The sampler's massive systems.
fn records() -> &'static [SystemRecord] {
    static RECORDS_OF: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS_OF.get_or_init(|| massive_records(held(), RECORDS, RECORDS_SEED))
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

/// The system `index` of the sampler under each arm.
fn both(index: usize) -> (SystemStars, SystemStars) {
    let record = &records()[index];
    (
        SystemStars::generate(held(), record),
        SystemStars::generate(alone(), record),
    )
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

/// The version's rules keep the held arm, every version below 27 does and 27 shows the stars a
/// system holds. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_released_stars_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < BINARY_KICKS_VERSION,
        "the bump to 27 puts P11.T10 on the record: a system a supernova has unbound shows only the \
         stars it still holds. Delete `BINARY_KICKS_VERSION`, `BinaryKicksRules` with its `Paired` \
         arm, `StellarRules::binary_kicks`, its match in `SystemStars::shown_at` and the range \
         brief's, this module and its line in `stellar/rules.rs`, and re-pin the movers plan 11's \
         records list"
    );
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).binary_kicks,
        BinaryKicksRules::Paired
    );
    let at = |v: u32| StellarRules::of_version(GeneratorVersion::new(v)).binary_kicks;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), BinaryKicksRules::Paired, "version {v}");
    }
    assert_eq!(at(27), BinaryKicksRules::Alone);
    assert_eq!(
        StellarRules::every_26_hold().binary_kicks,
        BinaryKicksRules::Paired
    );
    assert_eq!(
        StellarRules::every_27_hold().binary_kicks,
        BinaryKicksRules::Alone
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar,
        StellarRules::every_27_hold()
    );
}

/// A pinned system shows every drawn star at the epoch under the held arm, and under the arm the
/// stars that `shown` holds, with the shown hierarchy, state and brief; before its first
/// supernova both arms give it bit for bit. Returns the arm's system.
fn assert_shown(index: usize, shown: Shown) -> SystemStars {
    let (held, alone) = both(index);
    let now = UniverseTime::EPOCH;
    let count = usize::from(held.star_count());
    assert_eq!(held.shown_at(now), Shown::Drawn);
    assert_eq!(bodies(&held, now).len(), count);
    assert_eq!(held.summary_at(now).hierarchy(), Some(held.hierarchy()));
    assert_eq!(alone.shown_at(now), shown);
    let kept: Vec<u16> = (0..count)
        .map(|k| u16::try_from(k).expect("few stars"))
        .filter(|&k| shown.holds(u8::try_from(k).expect("few stars")))
        .collect();
    assert_eq!(bodies(&alone, now), kept);
    let summary = alone.summary_at(now);
    let hierarchy = summary.hierarchy().expect("the system exists");
    assert_eq!(usize::from(hierarchy.star_count()), kept.len());
    let state = alone.state_at(now).expect("the system exists");
    assert_eq!(state.stars().len(), kept.len());
    let held_state = held.state_at(now).expect("the system exists");
    for (k, star) in state.stars().iter().enumerate() {
        assert_eq!(star, &held_state.stars()[k], "star {k} is the held arm's");
    }
    let mass: f64 = state.stars().iter().map(|s| s.mass().value()).sum();
    assert!(
        (state.mass().value() - mass).abs() <= 1e-12 * mass,
        "the system's mass is its shown stars' sum: {state:?}"
    );
    // The shown hierarchy is the summary's, and its bodies index the state's stars, as the sky's
    // census walks them.
    let shown_hierarchy = alone.hierarchy_at(now);
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
    let brief = alone.brief_at(now).expect("the primary exists");
    assert_eq!(usize::from(brief.star_count()), kept.len());
    // The primary's summary is the held arm's: only what is shown beside it changes.
    assert_eq!(summary.stars()[0], held.summary_at(now).stars()[0]);
    let whole = time_at_age(WHOLE_AGE);
    assert_eq!(alone.shown_at(whole), Shown::Drawn);
    assert_eq!(alone.summary_at(whole), held.summary_at(whole));
    assert_eq!(alone.state_at(whole), held.state_at(whole));
    assert_eq!(alone.brief_at(whole), held.brief_at(whole));
    alone
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
/// drawn orbit, with that pair's state and class, the root of the shown hierarchy.
#[test]
fn a_quadruple_whose_outer_pair_is_unbound_shows_its_inner_pair() {
    let held = SystemStars::generate(held(), &records()[OUTER_PAIR]);
    let node = held
        .hierarchy()
        .pairs()
        .map(|(n, _)| n)
        .find(|n| n.get() == 1)
        .expect("the inner pair is node 1 of ((0, 1), (2, 3))");
    let alone = assert_shown(OUTER_PAIR, Shown::Pair(node));
    let now = UniverseTime::EPOCH;
    let state = alone.state_at(now).expect("exists");
    let held_state = held.state_at(now).expect("exists");
    assert_eq!(state.pairs().len(), 1);
    assert_eq!(state.pairs()[0].node(), NodeIndex::ROOT);
    let kept = held_state
        .pairs()
        .iter()
        .find(|p| p.node() == node)
        .expect("the drawn inner pair");
    assert_eq!(state.pairs()[0].orbit(), kept.orbit());
    assert_eq!(state.pairs()[0].class(), kept.class());
    let HierarchyNode::Pair { orbit, .. } = *held.hierarchy().node(node) else {
        panic!("node 1 is the inner pair");
    };
    let shown = alone.summary_at(now).hierarchy().cloned().expect("exists");
    let (root, drawn) = shown.pairs().next().expect("a pair");
    assert_eq!(root, shown.root());
    assert_eq!(drawn, &orbit);
}

/// The range brief of a system a supernova has unbound counts the stars its summary shows under
/// the arm, and the first attempt's under the held arm.
#[test]
fn the_range_brief_counts_the_stars_shown() {
    let record = &records()[RELEASED];
    let now = UniverseTime::EPOCH;
    let held_brief = BriefModel::new(held(), record)
        .brief_at(now)
        .expect("the primary exists");
    assert_eq!(held_brief.star_count(), 2);
    let alone_brief = BriefModel::new(alone(), record)
        .brief_at(now)
        .expect("the primary exists");
    assert_eq!(alone_brief.star_count(), 1);
    assert_eq!(
        Some(alone_brief),
        SystemStars::generate(alone(), record).brief_at(now)
    );
}

/// The same system twice under the arm is the same, and every sampled system that no supernova
/// has unbound by the epoch is the held arm's under it.
#[test]
fn the_arm_is_deterministic_and_moves_only_the_unbound() {
    let now = UniverseTime::EPOCH;
    let (mut unbound, mut whole) = (0, 0);
    for record in records() {
        let shown_held = SystemStars::generate(held(), record);
        let shown_alone = SystemStars::generate(alone(), record);
        assert_eq!(shown_alone, SystemStars::generate(alone(), record));
        if shown_alone.shown_at(now) == Shown::Drawn {
            whole += 1;
            assert_eq!(shown_alone.summary_at(now), shown_held.summary_at(now));
            assert_eq!(shown_alone.state_at(now), shown_held.state_at(now));
            assert_eq!(shown_alone.brief_at(now), shown_held.brief_at(now));
        } else {
            unbound += 1;
            assert_ne!(shown_alone.summary_at(now), shown_held.summary_at(now));
        }
    }
    println!("{unbound} unbound, {whole} whole of {RECORDS}");
    assert!(
        unbound > 5 && whole > 20,
        "{unbound} unbound, {whole} whole"
    );
}

/// Under the arm, every sampled system's range brief, whatever its route, counts the stars the
/// system shows at the epoch: the first attempt's count for a system shown whole, and the shown
/// count for one a supernova has unbound.
#[test]
fn under_the_arm_every_route_counts_the_stars_shown() {
    let now = UniverseTime::EPOCH;
    for record in records() {
        let stars = SystemStars::generate(alone(), record);
        let model = BriefModel::new(alone(), record);
        let shown = match stars.shown_at(now) {
            Shown::Drawn => model.star_count(),
            Shown::Pair(_) => 2,
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
