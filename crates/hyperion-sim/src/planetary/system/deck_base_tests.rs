//! P14.T53.b part 1 (b)'s follow-up, the deck base (decision-absurd-states-24 R3.2; lane
//! fix-thin-air-venus-deck), on the sample's bodies: from generator version 35 Venus's deck needs
//! the air it hangs in, and none of R3's five is a runaway under it.

use super::audit_tests::audit_walk_system;
use super::tests::{DECKED_FIVE, SEED, sample};
use super::*;
use crate::planetary::derive::atmosphere::{
    ACID_CYCLING_TEMPERATURE, Atmosphere, DECK_BASE_PRESSURE, RunawayDeck, SurfaceState, reducing,
};

/// The census's runaway under Venus's deck in the thinnest air of the sample's: before the 35 bump,
/// solved under the deck, 493.4 K under 0.691 bar of CO₂ (the deck base's census, 2026-11-19).
const THIN: &str = "21fff6587ff80000.0100";

/// The census's first runaway under Venus's deck in a reducing air: before the 35 bump, solved
/// under the deck, a steam envelope at 816.5 K under 1,728 bar, its air led by hydrogen (the deck
/// base's census, 2026-11-19).
const REDUCING: &str = "02001b2ca0020000.0200";

/// The published atmosphere of the sample's body `body` at the epoch, under the rules in force on
/// this thread, its system generated afresh.
fn air_of(body: &str) -> Atmosphere {
    let (system_id, _) = body.split_once('.').expect("a body ID");
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == system_id)
        .unwrap_or_else(|| panic!("{body}'s system is in the sample"));
    let system = generate(SEED, ctx);
    let mut found = None;
    audit_walk_system("sample", ctx, &system, &mut |case| {
        if case.record.identity().id().to_string() == body {
            assert!(found.is_none(), "{body} is walked once");
            found = Some(*case.derived.atmosphere());
        }
    });
    found.unwrap_or_else(|| panic!("{body} is present at the epoch"))
}

/// The deck base's two cases on the sample's bodies: a runaway in an air thinner than the deck's
/// base and one in a reducing air, each at or above the acid's cycling temperature, so that arm
/// (b)'s cold test would keep its deck, are solved without it, their state unchanged.
#[test]
fn a_thin_or_reducing_air_drops_venus_s_deck() {
    for (body, thin) in [(THIN, true), (REDUCING, false)] {
        let air = air_of(body);
        eprintln!(
            "{body}: {:?} {:?}, T_s {:.1} K, p_s {:?}, A {:.3}",
            air.state(),
            air.runaway_deck(),
            air.surface_temperature().value(),
            air.surface_pressure(),
            air.albedo().value()
        );
        assert_eq!(air.state(), SurfaceState::RunawayGreenhouse, "{body}");
        assert_eq!(air.runaway_deck(), RunawayDeck::Deckless, "{body}");
        assert!(
            air.surface_temperature() >= ACID_CYCLING_TEMPERATURE,
            "{body}: arm (b)'s cold test would keep the deck"
        );
        let p_s = air
            .surface_pressure()
            .expect("a runaway's air has a pressure");
        assert_eq!(p_s < DECK_BASE_PRESSURE, thin, "{body}: {p_s:?}");
        assert_eq!(reducing(air.partial_pressures()), !thin, "{body}");
    }
}

/// decision-absurd-states-24 R3's five: none is a runaway under Venus's deck (three are runaways
/// solved without it, two dry `Temperate`s).
#[test]
fn r3_s_five_are_not_under_venus_s_deck() {
    for body in DECKED_FIVE {
        let air = air_of(body);
        assert!(
            !(air.state() == SurfaceState::RunawayGreenhouse
                && air.runaway_deck() == RunawayDeck::Kept),
            "{body} is not under the deck"
        );
    }
}
