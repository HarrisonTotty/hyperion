//! The wrong classes P14.T33.c's census found under every 25 hold, stated by name under every 26
//! hold (research-census-25): the 13 locked `Snowball`s above 273.15 K (`LOCKED_STATE_VERSION`),
//! the three airless planets over a hot ice bulk (`PLANET_LAG_VERSION`), the 29 runaways over a
//! sea (`SEA_DECK_VERSION`) and the two hot `Snowball`s a locked world's deck set off
//! (`LOCKED_CLIMATE_VERSION`), each body generated afresh in its system, as the census walks it;
//! and the Solar System table, unchanged to the bit by the eyeball's rule and the dry deck.
//!
//! The bodies are named by their system's rank in the census's sample and their ID. The tests are
//! native only, as the census is: on WebAssembly their dozens of systems would run on one thread.

use super::audit_tests::{AuditCase, audit_walk_system};
use super::census_tests::{WATCH_TIMES, watch_time};
use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::atmosphere::{
    Gas, LockedStateRules, SNOWBALL_ICE_COVER, SeaDeckRules, SurfaceMaterial, SurfaceState,
    WATER_MELTING_POINT, with_locked_state, with_sea_deck,
};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::derive::passes::Convergence;
use crate::planetary::derive::photometry::{BodyPhotometry, PhaseTemplate};
use crate::planetary::derive::solar::historic_table;
use crate::planetary::hooks::surface::Forcing;
use crate::planetary::record::{BodyRecord, RecordSection, SectionState};
use crate::planetary::system::planet_lag::{PlanetLagRules, with_planet_lag};
use crate::planetary::testing::{
    SampleFilter, sample_records, with_every_25_hold, with_every_26_hold,
};

/// The census's 13 `Snowball`s above 273.15 K under every 25 hold, every one locked 1:1.
const EYEBALLS: [(usize, &str); 13] = [
    (1_141, "02006b2ca0020001.0600"),
    (7_521, "01ff5b2d60060000.0500"),
    (12_305, "01ffdb2c1ff10000.0200"),
    (16_228, "02004b289fff0000.0700"),
    (19_542, "01ff6b2c200e0000.0300"),
    (24_881, "01ffeb30200b0000.0200"),
    (30_404, "02013b2ce0040000.0100"),
    (40_660, "02011b2fa0000001.0200"),
    (41_398, "0200cb311fff0000.0200"),
    (44_130, "02010b30a0020000.0200"),
    (48_422, "02013b2f9ff90000.0100"),
    (55_088, "2201365480280000.1600"),
    (60_282, "02015b2de00c0000.0500"),
];

/// The census's three airless `Icy` planets at a T0 above 273.15 K under every 25 hold.
const AIRLESS_ICE_PLANETS: [(usize, &str); 3] = [
    (39_279, "42006cc200c00007.0500"),
    (44_920, "4200acaa01000006.0400"),
    (58_712, "42012ca200400002.0400"),
];

/// The census's 29 runaways with liquid water at their surface under every 25 hold.
const SEA_RUNAWAYS: [(usize, &str); 29] = [
    (522, "01ffeb2b60000000.0200"),
    (1_894, "02002b2d9ff90000.0200"),
    (4_398, "02001b2e5ff80000.0500"),
    (7_476, "01ffeb2cdff30000.0100"),
    (12_344, "42006cbdff400007.0300"),
    (12_462, "41ff6ca600000006.0700"),
    (14_596, "01ffab291ffa0001.0200"),
    (18_656, "02000b28a0070000.0b00"),
    (18_987, "6200597802000004.1100"),
    (24_383, "01ff1b2aa0090001.0200"),
    (28_421, "01ffeb299ff00000.0200"),
    (30_446, "01ff6b28a0070000.0300"),
    (33_411, "01fecb2b20030000.0300"),
    (35_305, "01ff0b291ffa0001.0c00"),
    (37_775, "02012b2e9ff90000.0300"),
    (38_611, "01ff5b311ffa0002.0100"),
    (39_106, "01ff1b309fff0001.0100"),
    (42_995, "01fedb299ff90000.0200"),
    (43_079, "0200ab2bdfec0000.0100"),
    (44_182, "01feab2a9ffb0000.1200"),
    (49_737, "02013b2ddff30000.0200"),
    (52_618, "02006b26a0020000.0200"),
    (54_359, "01ff6b271ffb0000.0100"),
    (59_452, "01fffb329ffa0000.0200"),
    (61_501, "01ffcb269ff70000.0100"),
    (66_441, "02007b2a60170001.0100"),
    (66_447, "41feaca20000000d.0800"),
    (67_168, "01ff0b279ff70000.0a00"),
    (67_833, "02005b261ffa0000.0200"),
];

/// The two `Snowball`s above 273.15 K that P14.T24.j's deck set off under every 26 hold while its
/// gate read only the listed liquid (research-leans-8 §3): #17937, at the open water's edge, and
/// #2488, under 14.9 bar of nitrogen.
const DECK_SNOWBALLS: [(usize, &str); 2] = [
    (2_488, "02000b2de0060000.0300"),
    (17_937, "0200fb2d1ffb0000.0400"),
];

/// The two N₂ envelopes the slow census found changing section inside the clock window under every
/// 25 hold and under every 26 hold (research-leans-8 §1): `Snowball`s whose mean surface
/// temperatures lie above N₂'s critical point and whose reservoirs the passes settle on it.
const CRITICAL_N2_ENVELOPES: [(usize, &str); 2] = [
    (14_276, "01ff1b2e1fff0000.0700"),
    (23_283, "0200cb291ffc0000.0600"),
];

/// The census's sample through the rank `last`, and its galaxy.
fn sample_through(last: usize) -> (Galaxy, Vec<SystemRecord>) {
    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, last + 1, SampleFilter::ALL)
        .expect("the solar circle holds the census's sample");
    (galaxy, records)
}

/// `read` of the body `id` of the sample's system `rank`, generated afresh under the rules on this
/// thread and walked as the census walks it; `None` where the walk meets no such body.
fn read_body<R>(
    (galaxy, records): (&Galaxy, &[SystemRecord]),
    (rank, id): (usize, &str),
    read: &dyn Fn(&AuditCase<'_>) -> R,
) -> Option<R> {
    let ctx = SystemContext::from_record(galaxy, &records[rank]);
    let system = generate(SEED, &ctx);
    let mut found = None;
    audit_walk_system("census", &ctx, &system, &mut |case| {
        if case.record.identity().id().to_string() == id {
            found = Some(read(case));
        }
    });
    found
}

/// The envelope section of the body `id` of the sample's system `rank` at each of the census's
/// [`WATCH_TIMES`] times across the clock window ([`watch_time`]), its system generated afresh
/// under the rules on this thread.
fn envelope_across_the_window(
    (galaxy, records): (&Galaxy, &[SystemRecord]),
    (rank, id): (usize, &str),
) -> Vec<SectionState> {
    let index = read_body((galaxy, records), (rank, id), &|case| case.body.index)
        .expect("the census's body is in its system");
    let ctx = SystemContext::from_record(galaxy, &records[rank]);
    let system = generate(SEED, &ctx);
    (0..WATCH_TIMES)
        .map(|k| {
            system
                .body_at(&ctx, index, watch_time(k))
                .expect("a walked body is in its system")
                .section_state(RecordSection::Envelope)
        })
        .collect()
}

/// Whether the published condensates of `case`'s surface list liquid water.
fn lists_liquid_water(case: &AuditCase<'_>) -> bool {
    case.surface().is_some_and(|surface| {
        surface
            .condensates()
            .entry(Gas::Water.substance(), CondensatePhase::Liquid)
            .is_some()
    })
}

/// The highest rank of `bodies`, through which their sample is drawn.
fn last_rank(bodies: &[(usize, &str)]) -> usize {
    bodies.iter().map(|body| body.0).max().unwrap_or(0)
}

/// Asserts that under every 26 hold each of `bodies`, of the census's locked `Snowball`s above
/// 273.15 K under every 25 hold, is `Temperate` (an eyeball, its open substellar sea under a frozen
/// night, or a two-state cycle's last pass labelled by its own figures), or a `Snowball` its own
/// figures make, frozen over at or below 273.15 K; and gives their states.
fn assert_eyeballs_temperate_or_frozen_over(bodies: &[(usize, &str)]) -> Vec<SurfaceState> {
    let sample = sample_through(last_rank(bodies));
    let read = |case: &AuditCase<'_>| {
        let surface = case.surface().expect("an eyeball has a surface");
        (
            surface.state(),
            surface.conditions().mean_surface_temperature().value(),
            surface.figures().ice_fraction(),
            lists_liquid_water(case),
            surface.climate_regime().forcing(),
        )
    };
    let (mut lines, mut states) = (Vec::new(), Vec::new());
    for &body in bodies {
        let (state, t_s, ice, liquid, forcing) =
            with_every_26_hold(|| read_body((&sample.0, &sample.1), body, &read))
                .expect("the census's body is in its system");
        lines.push(format!(
            "{} {}: {state:?}, T_s {t_s:.2} K, ice {ice:.4}, liquid {liquid}, {forcing:?}",
            body.0, body.1
        ));
        assert_eq!(forcing, Forcing::Locked, "{}", body.1);
        match state {
            SurfaceState::Temperate => {}
            SurfaceState::Snowball => {
                assert!(
                    ice >= SNOWBALL_ICE_COVER && !liquid && t_s <= WATER_MELTING_POINT.value(),
                    "{}: a Snowball its figures do not make, {lines:#?}",
                    body.1
                );
            }
            SurfaceState::GasEnvelope
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::RunawayGreenhouse
            | SurfaceState::Tenuous
            | SurfaceState::SteamEnvelope => panic!("{}: {state:?}", body.1),
        }
        states.push(state);
    }
    eprintln!(
        "the census's eyeballs under every 26 hold:\n  {}",
        lines.join("\n  ")
    );
    states
}

/// Asserts that under every 26 hold each of `bodies`, of the census's airless planets over a hot
/// ice bulk, takes the rock lag: `Rock` under `Mercury`'s template, its equilibrium temperature
/// near T0 at the lag's albedo; and that every other record of its system is the one the holds'
/// 26 give without the planets' lag, to the bit.
fn assert_airless_ice_planets_take_the_rock_lag(bodies: &[(usize, &str)]) {
    let sample = sample_through(last_rank(bodies));
    let read = |case: &AuditCase<'_>| {
        let surface = case.surface().expect("an airless planet has a surface");
        let t0 = equilibrium_temperature(case.derived.flux(), BondAlbedo::from_fraction(0.0));
        (
            surface.material(),
            case.record.photometry().ok().map(BodyPhotometry::template),
            case.derived.equilibrium_temperature().value() / t0.value(),
        )
    };
    for &body in bodies {
        let (material, template, t_eq_over_t0) =
            with_every_26_hold(|| read_body((&sample.0, &sample.1), body, &read))
                .expect("the census's body is in its system");
        eprintln!(
            "{} {}: {material:?}, {template:?}, T_eq {t_eq_over_t0:.4} T0",
            body.0, body.1
        );
        assert_eq!(material, SurfaceMaterial::Rock, "{}", body.1);
        assert_eq!(template, Some(PhaseTemplate::Mercury), "{}", body.1);
        assert!(
            (0.9..=1.0).contains(&t_eq_over_t0),
            "{}: {t_eq_over_t0}",
            body.1
        );
        let records = |rules| {
            with_every_26_hold(|| {
                with_planet_lag(rules, || {
                    let ctx = SystemContext::from_record(&sample.0, &sample.1[body.0]);
                    generate(SEED, &ctx)
                        .snapshot_at(&ctx, crate::time::UniverseTime::EPOCH)
                        .bodies()
                        .to_vec()
                })
            })
        };
        let (held, lagged) = (
            records(PlanetLagRules::Held),
            records(PlanetLagRules::Airless),
        );
        let moved: Vec<String> = held
            .iter()
            .zip(&lagged)
            .filter(|(a, b)| a != b)
            .map(|(_, record): (_, &BodyRecord)| record.identity().id().to_string())
            .collect();
        assert_eq!(
            moved,
            [body.1],
            "only the lagged planet moves in its system"
        );
    }
}

/// Asserts that each of `bodies`, of the census's runaways over a sea, is a `RunawayGreenhouse`
/// with liquid water at its surface under every 25 hold and none under every 26 hold, where #522
/// and #18656, whose first solve part 3 labels a steam envelope, publish `SteamEnvelope`.
fn assert_no_runaway_holds_a_sea(bodies: &[(usize, &str)]) {
    let sample = sample_through(last_rank(bodies));
    let read = |case: &AuditCase<'_>| {
        (
            case.derived.atmosphere().published_state(),
            case.record.surface().ok().map(Surface::state),
            lists_liquid_water(case),
            case.derived.atmosphere().runaway_deck(),
            case.derived.atmosphere().surface_temperature().value(),
        )
    };
    let mut lines = Vec::new();
    for &body in bodies {
        let at_25 = with_every_25_hold(|| read_body((&sample.0, &sample.1), body, &read))
            .expect("the census's body is in its system");
        let at_26 = with_every_26_hold(|| read_body((&sample.0, &sample.1), body, &read))
            .expect("the census's body is in its system");
        lines.push(format!("{} {}: 25 {at_25:?}; 26 {at_26:?}", body.0, body.1));
        assert_eq!(
            (at_25.1, at_25.2),
            (Some(SurfaceState::RunawayGreenhouse), true),
            "{}: under 25 a runaway over a sea, {lines:#?}",
            body.1
        );
        let (published, surface, liquid, ..) = at_26;
        assert_ne!(
            (surface, liquid),
            (Some(SurfaceState::RunawayGreenhouse), true),
            "{}: a runaway over a sea, {lines:#?}",
            body.1
        );
        if matches!(body.0, 522 | 18_656) {
            assert_eq!(published, SurfaceState::SteamEnvelope, "{}", body.1);
        }
    }
    eprintln!(
        "the census's sea runaways, under 25 and 26:\n  {}",
        lines.join("\n  ")
    );
}

/// Under every 26 hold an eyeball of the census (#30404) settles under its deck at the open water's
/// edge (P14.T24.j), a `Snowball` its own figures make below 273.15 K, and a two-state cycle of the
/// census (#19542, open and ice-free) is `Temperate`; each a `Snowball` above 273.15 K under every
/// 25 hold.
#[test]
fn under_every_26_hold_a_census_eyeball_settles_at_the_edge_and_a_two_state_cycle_is_temperate() {
    assert_eq!(
        assert_eyeballs_temperate_or_frozen_over(&[EYEBALLS[6], EYEBALLS[4]]),
        [SurfaceState::Snowball, SurfaceState::Temperate]
    );
}

/// Under every 26 hold the deck's gate reads the open water its ramp reads (P14.T24.j): #17937,
/// whose decked pass lists only ice but melts its water in its warmest zone, keeps its deck and
/// settles as a `Snowball` at the open water's edge, below 273.15 K, its warmest above water's
/// melting point, with no two-state cycle; and #2488 is no `Snowball` above 273.15 K. While the
/// gate read only the listing each was a `Snowball` above it (273.72 K and 274.80 K): the decked
/// pass's ice held the deckless passes on a cold branch.
#[test]
fn under_every_26_hold_the_deck_s_edge_worlds_are_no_hot_snowballs() {
    let sample = sample_through(last_rank(&DECK_SNOWBALLS));
    let read = |case: &AuditCase<'_>| {
        let surface = case.surface().expect("a locked world has a surface");
        let conditions = surface.conditions();
        let (coldest, warmest) = conditions.climate_range();
        (
            surface.state(),
            conditions.mean_surface_temperature().value(),
            (coldest.value(), warmest.value()),
            surface.figures().ice_fraction(),
            lists_liquid_water(case),
            case.derived.atmosphere_passes(),
            case.derived.convergence(),
            case.derived.atmosphere().melts_water_under_deck(),
        )
    };
    let bodies = DECK_SNOWBALLS.map(|body| {
        with_every_26_hold(|| read_body((&sample.0, &sample.1), body, &read))
            .expect("the census's body is in its system")
    });
    let lines: Vec<String> = DECK_SNOWBALLS
        .iter()
        .zip(&bodies)
        .map(
            |(body, (state, t_s, range, ice, liquid, passes, convergence, melts))| {
                format!(
                    "{} {}: {state:?}, T_s {t_s:.2} K over [{:.1}, {:.1}] K, ice {ice:.3}, liquid \
                 {liquid}, melts under the deck {melts}, {passes} passes, {convergence:?}",
                    body.0, body.1, range.0, range.1
                )
            },
        )
        .collect();
    eprintln!(
        "the deck's edge worlds under every 26 hold:\n  {}",
        lines.join("\n  ")
    );
    let melting = WATER_MELTING_POINT.value();
    for (body, &(state, t_s, ..)) in DECK_SNOWBALLS.iter().zip(&bodies) {
        assert!(
            !(state == SurfaceState::Snowball && t_s > melting),
            "{}: a Snowball above 273.15 K, {lines:#?}",
            body.1
        );
    }
    let [
        _,
        (state, t_s, (_, warmest), ice, liquid, _, convergence, melts),
    ] = bodies;
    assert_eq!(state, SurfaceState::Snowball, "#17937, {lines:#?}");
    assert!(
        t_s < melting && warmest > melting,
        "#17937 at the open water's edge, {lines:#?}"
    );
    assert!(
        ice >= SNOWBALL_ICE_COVER && !liquid && melts,
        "#17937, {lines:#?}"
    );
    assert_eq!(convergence, Convergence::Converged, "#17937, {lines:#?}");
}

/// [`assert_eyeballs_temperate_or_frozen_over`] over all 13 of the census's.
#[test]
#[ignore = "slow: the 13 eyeballs' systems, generated afresh"]
fn under_every_26_hold_the_census_s_eyeballs_are_temperate_or_frozen_over() {
    let states = assert_eyeballs_temperate_or_frozen_over(&EYEBALLS);
    assert_eq!(states.len(), EYEBALLS.len());
}

/// Under every 26 hold an airless planet of the census over a hot ice bulk (#44920) takes the rock
/// lag, and nothing else in its system moves.
#[test]
fn under_every_26_hold_a_census_airless_ice_planet_takes_the_rock_lag() {
    assert_airless_ice_planets_take_the_rock_lag(&[AIRLESS_ICE_PLANETS[1]]);
}

/// [`assert_airless_ice_planets_take_the_rock_lag`] over all three of the census's.
#[test]
#[ignore = "slow: the three planets' systems, each generated three times"]
fn under_every_26_hold_the_census_s_airless_ice_planets_take_the_rock_lag() {
    assert_airless_ice_planets_take_the_rock_lag(&AIRLESS_ICE_PLANETS);
}

/// Under every 26 hold two runaways over a sea of the census hold none: #522, labelled a steam
/// envelope, and #1894, solved without the deck.
#[test]
fn under_every_26_hold_two_census_runaways_hold_no_sea() {
    assert_no_runaway_holds_a_sea(&SEA_RUNAWAYS[..2]);
}

/// [`assert_no_runaway_holds_a_sea`] over all 29 of the census's.
#[test]
#[ignore = "slow: the 29 runaways' systems, each generated under 25 and 26"]
fn under_every_26_hold_no_census_runaway_holds_a_sea() {
    assert_no_runaway_holds_a_sea(&SEA_RUNAWAYS);
}

/// Under every 25 hold and under every 26 hold the census's two critical N₂ envelopes
/// ([`CRITICAL_N2_ENVELOPES`]) hide their surfaces, their envelopes not modelled, at every one of
/// the census's [`WATCH_TIMES`] times across the clock window: a reservoir counts by its solve's
/// mean surface temperature, above N₂'s critical point, not by the temperature the passes settle
/// it at, on that point (research-leans-8 §1).
#[test]
fn under_every_25_and_26_hold_the_census_s_critical_n2_envelopes_stay_hidden() {
    let sample = sample_through(last_rank(&CRITICAL_N2_ENVELOPES));
    let across = |body| envelope_across_the_window((&sample.0, &sample.1), body);
    let times = usize::try_from(WATCH_TIMES).expect("the census's 41 times");
    for &body in &CRITICAL_N2_ENVELOPES {
        for (rules, sections) in [
            (25, with_every_25_hold(|| across(body))),
            (26, with_every_26_hold(|| across(body))),
        ] {
            assert_eq!(
                sections,
                vec![SectionState::NotModelled; times],
                "#{} {} under every {rules} hold",
                body.0,
                body.1
            );
        }
    }
}

/// The Solar System table (Venus, Earth, Mars and Titan among its bodies) is derived the same, to
/// the bit, under every 25 hold with the eyeball's rule and the dry deck each on and held (the
/// planets' lag is a generated system's, which the table is not).
#[test]
fn the_solar_system_table_is_unchanged_by_the_census_s_26_rules() {
    let held = with_every_25_hold(historic_table);
    assert_eq!(
        with_every_25_hold(|| with_locked_state(LockedStateRules::Eyeball, historic_table)),
        held
    );
    assert_eq!(
        with_every_25_hold(|| with_sea_deck(SeaDeckRules::Dry, historic_table)),
        held
    );
}
