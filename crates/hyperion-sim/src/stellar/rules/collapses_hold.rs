//! The tests of P11.T10's fix's hold, [`COLLAPSES_VERSION`]: from version 27 a pair a star of
//! which collapses by its age is run through the engine (`evolve`'s pre-test), so that the
//! collapse acts on its orbit.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - A pinned massive system of the binaries' census sampler (`testing::massive_records`): a wide
//!   pair the pre-test passes over whose primary's collapse the arm records and which it unbinds.
//!
//! The movers under the arm, found by turning it on at the version over the sim's, the server's
//! and the fitting crate's tests (2026-11-01), are listed in plan 11's records.

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
use crate::rng::Seed;
use crate::stellar::binary::testing::massive_records;
use crate::stellar::binary::{BinaryInput, Component, can_interact};
use crate::stellar::multiplicity::{HierarchyNode, NodeIndex, StarIndex};
use crate::stellar::system::{BinaryKicksRules, Shown, SystemStars, disrupting_supernova};
use crate::time::UniverseTime;

/// The version's rules keep the held arm, every version below 27 does and 27 applies every
/// collapse. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_collapses_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < COLLAPSES_VERSION,
        "the bump to 27 puts P11.T10's fix on the record: a pair a star of which collapses by its \
         age is run through the engine, and each run pair's supernova acts on the orbits above it. \
         Delete `COLLAPSES_VERSION`, `CollapseRules` with its `Gated` arm, \
         `StellarRules::collapses`, its reads in `run_pair`, `outer_releases` and the brief's \
         gate, this module and its line in `stellar/rules.rs`, and re-pin the movers plan 11's \
         records list"
    );
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).collapses,
        CollapseRules::Gated
    );
    let at = |v: u32| StellarRules::of_version(GeneratorVersion::new(v)).collapses;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), CollapseRules::Gated, "version {v}");
    }
    assert_eq!(at(27), CollapseRules::Applied);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().collapses,
        CollapseRules::Gated
    );
    assert_eq!(
        StellarRules::every_27_hold().collapses,
        CollapseRules::Applied
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar,
        StellarRules::every_27_hold()
    );
}

/// The version's rules with this hold's arm on, and P11.T10's `binary_kicks`, without which no
/// system shows what a collapse releases.
fn applied_rules() -> GalaxyRules {
    let version = GalaxyRules::of_version(GENERATOR_VERSION);
    GalaxyRules {
        stellar: StellarRules {
            collapses: CollapseRules::Applied,
            binary_kicks: BinaryKicksRules::Alone,
            ..version.stellar
        },
        ..version
    }
}

/// The Milky Way fixture, under the version's rules: the binaries' agreement tests' (P11.T10).
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// [`held`]'s galaxy under [`applied_rules`].
fn applied() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(applied_rules()))
}

/// The binaries' census sampler's massive systems the pin is among.
fn records() -> &'static [SystemRecord] {
    static RECORDS_OF: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS_OF.get_or_init(|| massive_records(held(), RECORDS, RECORDS_SEED))
}

/// The seed of the Milky Way fixture, the binaries' agreement tests'.
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The seed of the massive systems' stream, the agreement tests'.
const RECORDS_SEED: u64 = 0x0b17_0010_5eed_000b;

/// The massive systems the pin is among.
const RECORDS: u32 = 1;

/// The sampler's first system, `0x8200_9920_0000_0000`: a pair of 17.8 and 4.4 M☉ at a period of
/// 2.9 × 10⁶ days that the pre-test passes over, whose primary dies as a type II-P supernova at
/// 12.4 Myr (research-leans-26).
const WIDE_PAIR: usize = 0;

/// The system `index` of the sampler under each arm.
fn both(index: usize) -> (SystemStars, SystemStars) {
    let record = &records()[index];
    (
        SystemStars::generate(held(), record),
        SystemStars::generate(applied(), record),
    )
}

/// A wide pair the pre-test passes over is run for its primary's collapse: its supernova is at plan
/// 06's age with plan 06's remnant and kick, and unbinds it, where the held arm keeps it on its
/// drawn orbit as one detached segment with no record.
#[test]
fn a_wide_pair_records_its_primary_s_collapse_and_is_unbound() {
    let (held, applied) = both(WIDE_PAIR);
    assert_eq!(
        held.record().id(),
        crate::id::SystemId::from_raw(0x8200_9920_0000_0000).expect("an ID")
    );
    let [held_pair, applied_pair] = [&held, &applied].map(|s| {
        s.pairs()
            .iter()
            .find(|p| p.stars() == [StarIndex::PRIMARY, StarIndex::from_body(1).expect("a star")])
            .expect("the primary's pair is run for its remnant")
    });
    let input_until = held_pair.timeline().until();
    assert_eq!(held_pair.timeline().segments().len(), 1);
    assert_eq!(held_pair.timeline().supernovae(), []);
    assert!(!can_interact(&wide_input(&held), input_until));
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
    // The system shows the primary's remnant alone from the collapse, and the held arm's both stars.
    let now = UniverseTime::EPOCH;
    assert_eq!(applied.shown_at(now), Shown::Alone);
    assert_eq!(held.shown_at(now), Shown::Drawn);
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
