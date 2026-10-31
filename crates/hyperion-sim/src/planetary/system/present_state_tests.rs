//! P14.T53.e's tests over the record: the state from the present climate, rules 5–7 of the
//! published state (decision-p14-t54a-states §3.4), in force from generator version 25, under
//! every 25 hold.

use std::time::Instant;

use super::audit_tests::{AuditCase, AuditSet, audit_walk};
use super::tests::{GOLDEN_SYSTEMS_SEED, ITEM_33_WORLD, SEED, golden_contexts, sample};
use super::*;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::atmosphere::{
    PresentClimate, PresentFigures, Rotation, RunawayDeck, SILICATE_SOLIDUS, SNOWBALL_ICE_COVER,
    SurfaceState, present_climate_state, water_saturation_temperature, with_deck_counts,
};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::hooks::surface::Forcing;
use crate::planetary::testing::with_every_25_hold;
use crate::units::Pascals;

/// The six `Snowball`s P14.T54.b.l's bless left above 273.15 K at 24 (bless-note-24 §3, T53.a
/// part C's counts), each `Icy` under an H₂-led air of 250–751 bar at a `T_eq` of 20–130 K, by
/// their IDs and their 24 mean surface temperatures, K.
const HOT_SNOWBALLS: [(&str, f64); 6] = [
    ("01fffb2d60000000.0600", 647.1),
    ("01ffdb2c1fff0000.0900", 519.6),
    ("02002b2c9ffb0000.0600", 416.7),
    ("02001b2ca0020000.0500", 380.8),
    ("02004b2c60010001.0400", 374.9),
    ("02001b2ba0000000.0300", 273.5),
];

/// One body's published state and figures as the tests read them: its derivation's state, mean
/// surface temperature and surface pressure, and where its record's surface is `Ok` its state,
/// ice cover and liquid water.
#[derive(Debug, Clone, PartialEq)]
struct Published {
    id: String,
    /// The derivation's published state ([`Atmosphere::published_state`]).
    state: SurfaceState,
    /// The record's surface's, its ice cover and whether it lists liquid water; `None` where it is
    /// not `Ok`.
    surface: Option<(SurfaceState, f64, bool)>,
    t_s: f64,
    p_s: f64,
    /// The state its published pass carries ([`Atmosphere::carried_state`]).
    carried: Option<PresentClimate>,
    passes: u32,
    jump: bool,
}

impl Published {
    /// The case's.
    fn of(case: &AuditCase<'_>) -> Self {
        let air = case.derived.atmosphere();
        let water = Gas::Water.substance();
        Self {
            id: case.record.identity().id().to_string(),
            state: air.published_state(),
            surface: case.surface().map(|surface| {
                (
                    surface.state(),
                    surface.figures().ice_fraction(),
                    surface
                        .condensates()
                        .entry(water, CondensatePhase::Liquid)
                        .is_some(),
                )
            }),
            t_s: air.surface_temperature().value(),
            p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value),
            carried: air.carried_state(),
            passes: case.derived.atmosphere_passes(),
            jump: case.derived.convergence().is_jump(),
        }
    }

    /// One line of a list.
    fn line(&self) -> String {
        let saturation = water_saturation_temperature(Pascals::new(self.p_s))
            .map_or_else(|| "—".to_owned(), |t| format!("{:.1}", t.value()));
        let surface = self.surface.map_or_else(
            || "no surface section".to_owned(),
            |(state, ice, liquid)| {
                format!("surface {state:?}, ice {ice:.3}, liquid water {liquid}")
            },
        );
        format!(
            "{} {:?}, T_s {:.1} K, p_s {:.4e} Pa (T_sat {saturation} K), {surface}, {} passes{}",
            self.id,
            self.state,
            self.t_s,
            self.p_s,
            self.passes,
            if self.jump { ", a jump" } else { "" }
        )
    }
}

/// A frozen `Temperate` of the sample at 24 that rule 5 publishes a `Snowball` from 25: ice over
/// its whole surface at 199.8 K under 0.86 bar, its passes settling (P14.T53.e's census).
const FROZEN_TEMPERATE: &str = "01fffb2c9fff0000.0300";

/// The sample systems that hold the six hot `Snowball`s, one each, and [`FROZEN_TEMPERATE`], as
/// audit sets.
fn flipping_sets() -> Vec<AuditSet> {
    sample()
        .iter()
        .filter(|ctx| {
            let id = ctx.id().to_string();
            HOT_SNOWBALLS
                .iter()
                .map(|(body, _)| *body)
                .chain([FROZEN_TEMPERATE])
                .any(|body| body.starts_with(&id))
        })
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .collect()
}

/// P14.T53.e on the record, its flips' fast test: the six `Snowball`s P14.T54.b.l's bless left
/// above 273.15 K at 24 under the insolation class's state ([`HOT_SNOWBALLS`]) and a frozen
/// `Temperate` of 24's ([`FROZEN_TEMPERATE`]), under every 25 hold.
///
/// - None of the six is a `Snowball`: each takes its climate's state, where its surface is `Ok`
///   the state its figures give by rules 5–7 (or a jump of its passes); and the frozen world is a
///   `Snowball` by rule 5, ice over at least half its surface, its passes settled and carrying the
///   state they publish.
/// - Generated twice under every 25 hold, every body publishes the same figures: the lagged passes
///   are deterministic.
#[test]
fn under_every_25_hold_the_hot_snowballs_and_a_frozen_temperate_take_their_climate_s_state() {
    let sets = flipping_sets();
    assert_eq!(sets.len(), 7, "each body lies in a system of its own");
    let ids: Vec<&str> = HOT_SNOWBALLS
        .iter()
        .map(|(id, _)| *id)
        .chain([FROZEN_TEMPERATE])
        .collect();
    let at_25 = with_every_25_hold(|| census_of(&sets, SecondStarts::None));
    let again = with_every_25_hold(|| census_of(&sets, SecondStarts::None));
    let lines = |bodies: &[CensusBody]| -> Vec<String> {
        bodies
            .iter()
            .map(|body| format!("{:?} {:?}", body.published, body.verdict))
            .collect()
    };
    assert_eq!(lines(&at_25), lines(&again), "the same bits twice");
    for id in ids {
        let now = at_25
            .iter()
            .find(|body| body.published.id == id)
            .unwrap_or_else(|| panic!("{id} at 25"));
        eprintln!("{id}: at 25 {}", now.published.line());
        if id == FROZEN_TEMPERATE {
            assert_eq!(now.published.state, SurfaceState::Snowball, "{id}");
            let (_, ice, _) = now.published.surface.expect("its surface is Ok");
            assert!(ice >= SNOWBALL_ICE_COVER, "{id}: ice {ice}");
            assert_eq!(
                now.published.carried,
                Some(PresentClimate::Snowball),
                "{id}"
            );
            assert!(!now.published.jump, "{id}");
        } else {
            assert_ne!(now.published.state, SurfaceState::Snowball, "{id}");
            if let Some(verdict) = now.verdict {
                assert!(
                    verdict == now.published.state || now.published.jump,
                    "{id}: rules 5–7 give {verdict:?}"
                );
            }
        }
    }
}

/// The rules 5–7 verdict on a record's published figures: its surface's ice cover and liquid water
/// and its atmosphere's mean surface temperature and surface pressure, `None` where its surface is
/// not `Ok`, its state is one rules 1–4 decide, or it is a dry runaway under Venus's deck, which
/// rule 6 judges on its climate without the deck (`state_to_carry`), which no pass of it solves.
fn verdict_of(case: &AuditCase<'_>) -> Option<SurfaceState> {
    let surface = case.surface()?;
    let air = case.derived.atmosphere();
    let liquid_water = surface
        .condensates()
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .is_some();
    match (surface.state(), air.runaway_deck()) {
        (SurfaceState::RunawayGreenhouse, RunawayDeck::Kept) if !liquid_water => return None,
        (
            SurfaceState::RunawayGreenhouse | SurfaceState::Temperate | SurfaceState::Snowball,
            RunawayDeck::Kept | RunawayDeck::Deckless,
        ) => {}
        (
            SurfaceState::GasEnvelope
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::Tenuous
            | SurfaceState::SteamEnvelope,
            RunawayDeck::Kept | RunawayDeck::Deckless,
        ) => return None,
    }
    let figures = PresentFigures {
        ice_cover: surface.figures().ice_fraction(),
        liquid_water,
        surface_temperature: air.surface_temperature(),
        surface_pressure: air.surface_pressure(),
        rotation: if surface.climate_regime().forcing() == Forcing::Locked {
            Rotation::Locked
        } else {
            Rotation::Turning
        },
    };
    Some(present_climate_state(&figures).state())
}

/// One body of the census ([`census_of`]): its label, what it publishes, the rules' verdict on its
/// record's figures, whether its passes judged it a hothouse ((c)'s re-solve, counted only where
/// the verdict and the state differ) and, for a published runaway, what its passes settle to from
/// the melt's albedo (item 26's second start).
struct CensusBody {
    set: String,
    label: String,
    published: Published,
    verdict: Option<SurfaceState>,
    hothouse: bool,
    from_the_melt: Option<Published>,
}

/// Whether a census solves each published runaway again from the melt's albedo ([`census_of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecondStarts {
    /// No second start.
    None,
    /// Item 26's second start from the melt's 0.10 (`PassDriver::From`).
    FromTheMelt,
}

/// The census of `sets` under the rules the caller's switches set: every body with a derivation
/// at the epoch ([`audit_walk`]), each published runaway solved again from the melt's albedo where
/// `starts` asks.
fn census_of(sets: &[AuditSet], starts: SecondStarts) -> Vec<CensusBody> {
    let mut bodies = Vec::new();
    audit_walk(sets, &mut |case| {
        let published = Published::of(case);
        let verdict = verdict_of(case);
        let differs = verdict.is_some_and(|verdict| verdict != published.state);
        let hothouse = differs && {
            let (_, counts) = with_deck_counts(|| (case.again)(PassDriver::Solve));
            counts.hothouses > 0
        };
        let from_the_melt = (starts == SecondStarts::FromTheMelt
            && published.state == SurfaceState::RunawayGreenhouse)
            .then(|| {
                let melt = SurfaceState::MagmaOcean.albedo(SurfaceMaterial::Rock);
                let again = (case.again)(PassDriver::From(BondAlbedo::from_fraction(melt.value())));
                let air = again.atmosphere();
                Published {
                    id: published.id.clone(),
                    state: air.published_state(),
                    surface: None,
                    t_s: air.surface_temperature().value(),
                    p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value),
                    carried: air.carried_state(),
                    passes: again.atmosphere_passes(),
                    jump: again.convergence().is_jump(),
                }
            });
        bodies.push(CensusBody {
            set: case.set.to_owned(),
            label: format!("{} {}", case.set, case.kind),
            published,
            verdict,
            hothouse,
            from_the_melt,
        });
    });
    bodies
}

/// The 400 sample systems and the 15 golden systems, as audit sets.
fn the_400_and_the_goldens() -> Vec<AuditSet> {
    sample()
        .iter()
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .chain(
            golden_contexts()
                .into_iter()
                .map(|(name, ctx)| (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .collect()
}

/// The bodies whose states decision-p14-t54a-states §3.4 anchors (its tests' list), by set and
/// ID: the golden `e101`, a `MagmaOcean` (decision-triton-and-e101 §2); the two frozen `Temperate`s
/// of its §4, `close_binary` .0600 and `m_dwarf_resonant_chain` .0100, each by rule 5 at its
/// climate after P14.T53.a's weathering thermostat; and decision-pre-23-review item 33's world
/// ([`ITEM_33_WORLD`]), not a runaway. A golden system's ID may name another system in the
/// sample's universe, so the set is part of the key.
const ANCHORS: [(&str, &str); 4] = [
    ("close_binary", "42002cb200000009.e101"),
    ("close_binary", "42002cb200000009.0600"),
    ("m_dwarf_resonant_chain", "02002b2ca0000000.0100"),
    ("sample", ITEM_33_WORLD),
];

/// `n` of `of` as a percentage.
fn percent(n: usize, of: usize) -> f64 {
    let n = u32::try_from(n).expect("a census counts under 2³² bodies");
    let of = u32::try_from(of.max(1)).expect("a census counts under 2³² bodies");
    100.0 * f64::from(n) / f64::from(of)
}

/// Every body's passes in `bodies`, summed: the census's measure of the rules' cost.
fn passes_of(bodies: &[CensusBody]) -> u64 {
    bodies
        .iter()
        .map(|body| u64::from(body.published.passes))
        .sum()
}

/// P14.T53.e's census over the 400 sample systems and the 15 golden systems, a measurement for
/// the 25 bless, run by name: under every 25 hold, every present planet, member and moon with a
/// derivation at the epoch.
///
/// - **No `Snowball` lies above 273.15 K** under every 25 hold, asserted, and the six that
///   P14.T54.b.l's bless left there ([`HOT_SNOWBALLS`]) take their climate's state, each printed.
/// - **Every state agrees with its published figures by rules 5–7** where its record's surface is
///   `Ok` and its state is one of theirs, not a runaway under Venus's deck (judged without it), but
///   at a jump of its passes (decision-pre-23-review item 32's discrete jumps) and where its passes
///   judged it a hothouse (P14.T53.b part 1 (c), whose state is fixed within its re-solve:
///   decision-p14-t54b-k-hothouses), asserted; each such exception listed.
/// - **The jumps:** the passes' jumps, counted against item 32's 1% cap of the bodies with a
///   surface (stated, not asserted: a count over goes to the findings).
/// - **Item 26's second start** for every published runaway, from the melt's 0.10: the bistable
///   magma oceans, each a runaway at the generator's start that settles molten from the melt's.
/// - **The anchors** (decision-p14-t54a-states §3.4, [`ANCHORS`]): `e101` a `MagmaOcean`, the two
///   frozen `Temperate`s by rule 5 and item 33's 46 K world not a runaway, asserted.
/// - **The cost:** every body's passes summed, and the walk's time, printed.
#[test]
#[ignore = "a measurement for the 25 bless, run by name"]
fn under_every_25_hold_the_present_climate_sets_the_state_over_the_400() {
    let sets = the_400_and_the_goldens();
    let start = Instant::now();
    let at_25 = with_every_25_hold(|| census_of(&sets, SecondStarts::None));
    let walk_25 = start.elapsed();
    let second = with_every_25_hold(|| census_of(&sets, SecondStarts::FromTheMelt));
    eprintln!(
        "the walk (pod): every 25 hold {walk_25:?}, {} passes",
        passes_of(&at_25)
    );
    // The anchors.
    for (set, id) in ANCHORS {
        let body = at_25
            .iter()
            .find(|body| body.set == set && body.published.id == id)
            .unwrap_or_else(|| panic!("{set} {id} is in the census"));
        eprintln!("anchor {} {}", body.label, body.published.line());
        match id {
            "42002cb200000009.e101" => {
                assert_eq!(body.published.state, SurfaceState::MagmaOcean, "{id}");
            }
            ITEM_33_WORLD => {
                assert_ne!(
                    body.published.state,
                    SurfaceState::RunawayGreenhouse,
                    "{id}"
                );
            }
            _ => assert_eq!(body.verdict, Some(body.published.state), "{id}"),
        }
    }
    let with_surface = at_25
        .iter()
        .filter(|body| body.published.surface.is_some())
        .count();
    eprintln!(
        "bodies with a derivation {}, with a surface {with_surface}",
        at_25.len()
    );
    // The six.
    for (id, t_s_24) in HOT_SNOWBALLS {
        let body = at_25
            .iter()
            .find(|body| body.set == "sample" && body.published.id == id)
            .unwrap_or_else(|| panic!("{id} is in the sample"));
        eprintln!(
            "hot snowball at 24 ({t_s_24} K) → {}",
            body.published.line()
        );
        assert_ne!(body.published.state, SurfaceState::Snowball, "{id}");
    }
    // No hot snowball.
    let hot: Vec<String> = at_25
        .iter()
        .filter(|body| {
            body.published.state == SurfaceState::Snowball && body.published.t_s > 273.15
        })
        .map(|body| format!("{} {}", body.label, body.published.line()))
        .collect();
    eprintln!(
        "snowballs above 273.15 K: {}:\n  {}",
        hot.len(),
        hot.join("\n  ")
    );
    let faults = report_agreement(&at_25);
    let jumps = at_25.iter().filter(|body| body.published.jump).count();
    eprintln!(
        "jumps {jumps} under every 25 hold ({:.2}% of the bodies with a surface, item 32's cap 1%)",
        percent(jumps, with_surface)
    );
    report_second_starts(&second);
    assert!(hot.is_empty(), "{hot:?}");
    assert!(
        faults.is_empty(),
        "{}:\n  {}",
        faults.len(),
        faults.join("\n  ")
    );
}

/// Every judged state of `at_25` against its figures' verdict ([`verdict_of`]): the exceptions (a
/// jump, or (c)'s hothouse) printed, the faults returned.
fn report_agreement(at_25: &[CensusBody]) -> Vec<String> {
    let mut exceptions = Vec::new();
    let mut faults = Vec::new();
    let mut judged = 0;
    for body in at_25 {
        let Some(verdict) = body.verdict else {
            continue;
        };
        judged += 1;
        let state = body.published.surface.map_or(body.published.state, |s| s.0);
        if verdict == state {
            continue;
        }
        let line = format!(
            "{} {}: rules 5–7 give {verdict:?}",
            body.label,
            body.published.line()
        );
        if body.published.jump || body.hothouse {
            exceptions.push(line);
        } else {
            faults.push(line);
        }
    }
    eprintln!(
        "states judged by rules 5–7 {judged}; exceptions (a jump, or (c)'s hothouse) {}:\n  {}",
        exceptions.len(),
        exceptions.join("\n  ")
    );
    faults
}

/// Item 26's second start of every published runaway in `second`, from the melt's 0.10: the
/// bistable magma oceans, printed.
fn report_second_starts(second: &[CensusBody]) {
    let mut bistable = Vec::new();
    let mut runaways = 0;
    for body in second {
        let Some(melt) = &body.from_the_melt else {
            continue;
        };
        runaways += 1;
        if melt.state == SurfaceState::MagmaOcean || melt.t_s >= SILICATE_SOLIDUS.value() {
            bistable.push(format!(
                "{} {}; from the melt's 0.10: {}",
                body.label,
                body.published.line(),
                melt.line()
            ));
        }
    }
    eprintln!(
        "runaways {runaways}; molten from the melt's albedo (bistable) {}:\n  {}",
        bistable.len(),
        bistable.join("\n  ")
    );
}
