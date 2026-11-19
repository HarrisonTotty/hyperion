//! P14.T53.b part 1 (b)'s follow-up, the deck base (decision-absurd-states-24 R3.2; lane
//! fix-thin-air-venus-deck), on the sample's bodies: under every 35 hold Venus's deck needs the air
//! it hangs in, and R3's five keep what the version's rules give them.

use super::audit_tests::audit_walk_system;
use super::tests::{DECKED_FIVE, SEED, sample};
use super::*;
use crate::planetary::derive::atmosphere::{
    ACID_CYCLING_TEMPERATURE, Atmosphere, DECK_BASE_PRESSURE, RunawayDeck, SurfaceState, reducing,
};
use crate::planetary::testing::with_every_35_hold;
use crate::units::Pascals;
use hyperion_testkit::float::assert_same_bits;

/// The census's runaway under Venus's deck in the thinnest air of the sample's: at the version's
/// rules 493.4 K under 0.691 bar of CO₂ (the deck base's census, 2026-11-19).
const THIN: &str = "21fff6587ff80000.0100";

/// The census's first runaway under Venus's deck in a reducing air: at the version's rules a steam
/// envelope at 816.5 K under 1,728 bar, its air led by hydrogen (the deck base's census,
/// 2026-11-19).
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

/// The deck base's two cases on the sample's bodies: a runaway under Venus's deck in an air
/// thinner than its base and one in a reducing air keep the deck at the version's rules, and under
/// every 35 hold are solved again without it, their state unchanged.
#[test]
fn under_every_35_hold_a_thin_or_reducing_air_drops_venus_s_deck() {
    for (body, thin) in [(THIN, true), (REDUCING, false)] {
        let at_33 = air_of(body);
        assert_eq!(at_33.state(), SurfaceState::RunawayGreenhouse, "{body}");
        assert_eq!(at_33.runaway_deck(), RunawayDeck::Kept, "{body}");
        assert!(
            at_33.surface_temperature() >= ACID_CYCLING_TEMPERATURE,
            "{body}: arm (b)'s cold test keeps the deck"
        );
        let p_s = at_33
            .surface_pressure()
            .expect("a runaway's air has a pressure");
        assert_eq!(p_s < DECK_BASE_PRESSURE, thin, "{body}: {p_s:?}");
        assert_eq!(reducing(at_33.partial_pressures()), !thin, "{body}");
        let at_35 = with_every_35_hold(|| air_of(body));
        assert_eq!(at_35.state(), SurfaceState::RunawayGreenhouse, "{body}");
        assert_eq!(at_35.runaway_deck(), RunawayDeck::Deckless, "{body}");
        assert!(
            at_35.albedo().value() < at_33.albedo().value(),
            "{body}: without the deck its albedo falls, {:?} → {:?}",
            at_33.albedo(),
            at_35.albedo()
        );
    }
}

/// decision-absurd-states-24 R3's five keep, under every 35 hold, the state, the deck, the mean
/// surface temperature, the surface pressure and the Bond albedo the version's rules give them, to
/// the bit: none is a runaway under the deck there (three are runaways solved without it, two
/// dry `Temperate`s).
#[test]
fn under_every_35_hold_r3_s_five_keep_their_state() {
    for body in DECKED_FIVE {
        let at_33 = air_of(body);
        let at_35 = with_every_35_hold(|| air_of(body));
        assert_eq!(at_35.state(), at_33.state(), "{body}");
        assert_eq!(at_35.published_state(), at_33.published_state(), "{body}");
        assert_eq!(at_35.runaway_deck(), at_33.runaway_deck(), "{body}");
        assert_same_bits(
            at_35.surface_temperature().value(),
            at_33.surface_temperature().value(),
        );
        assert_same_bits(
            at_35.surface_pressure().map_or(f64::NAN, Pascals::value),
            at_33.surface_pressure().map_or(f64::NAN, Pascals::value),
        );
        assert_same_bits(at_35.albedo().value(), at_33.albedo().value());
        assert!(
            !(at_33.state() == SurfaceState::RunawayGreenhouse
                && at_33.runaway_deck() == RunawayDeck::Kept),
            "{body} is not under the deck at the version's rules"
        );
    }
}
