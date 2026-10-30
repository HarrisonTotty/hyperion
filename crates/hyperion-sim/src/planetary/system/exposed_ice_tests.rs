//! P14.T53.c's exposed share through a lag over the record (decision-p14-t54a-ganymede §C3.1 and
//! §C3.4; decision-bump-plan-after-22 V3.9, W3 (a)): under every 25 hold, against every 25 hold
//! but the exposed share's.

use std::collections::BTreeMap;

use super::audit_tests::{AuditSet, audit_walk};
use super::tests::{GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, sample};
use super::*;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::hooks::crust::CrustLithology;
use crate::planetary::hooks::figures::lag::{
    ExposedIceRules, WHOLE_COVER_ROUNDING, exposed_ice_in_force, with_exposed_ice,
};
use crate::planetary::hooks::surface::regime::Forcing;
use crate::planetary::testing::with_every_25_hold;
use crate::substance::SubstanceId;
use crate::units::Pascals;
use hyperion_testkit::float;

/// How many of the sample's systems the fast census walks, the first in its order, as §C3.1's test
/// walks them; its slow twin walks the rest.
const SAMPLE_SYSTEMS: usize = 60;

/// The two ice bulks of the 400 with no surface section, each under a supercritical envelope,
/// whose photometry's template the passes' rule 5 moves from `Snowball` with the exposed share
/// (P14.T53.e's state from the published ice cover, which their dark lid's whole cover read).
const ENVELOPED: [&str; 2] = ["02001b2ce0020000.0400", "02003b2c20000000.0100"];

/// One body's record at the epoch, by its set and ID.
type Records = Vec<(String, BodyRecord)>;

/// The fast census's systems: the first [`SAMPLE_SYSTEMS`] of the sample and the 15 golden ones.
fn fast_sets() -> Vec<AuditSet> {
    sample()[..SAMPLE_SYSTEMS]
        .iter()
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .chain(
            golden_contexts()
                .into_iter()
                .map(|(name, ctx)| (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .collect()
}

/// The records of the systems `sets`, each generated afresh under the rules in force on this
/// thread, in body order.
fn records(sets: &[AuditSet]) -> Records {
    let mut records = Vec::new();
    for (set, seed, ctx) in sets {
        let system = generate(*seed, ctx);
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            records.push((format!("{set} {}", record.identity().id()), record.clone()));
        }
    }
    records
}

/// The lid of the record's surface `surface`, `None` where its crust is not modelled.
fn lid_of(surface: &Surface) -> Option<CrustLithology> {
    let row = surface.crust().ok()?.lid_lithology();
    CrustLithology::ALL
        .into_iter()
        .find(|lid| lid.substance() == row)
}

/// Every section of the record `record` but its photometry, as the census compares them.
fn but_photometry(record: &BodyRecord) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        record.level(),
        record.identity(),
        record.position(),
        record.mass(),
        record.orbit(),
        record.moons(),
        record.rings(),
        record.population(),
        record.bulk(),
        record.surface(),
        record.hooks(),
        record.figure(),
        record.rotation(),
        record.envelope(),
    )
}

/// The share of the record's ices `ices` that is water's, 0 where it lists none.
fn water_share(ices: &[(SubstanceId, f64)]) -> f64 {
    let water = Gas::Water.substance();
    ices.iter()
        .find(|&&(id, _)| id == water)
        .map_or(0.0, |&(_, share)| share)
}

/// What a census found ([`census`]): its records under every 25 hold, its count of ice bulks under
/// a dark lid, the states and climate regimes that moved among them, the other records that moved
/// with a surface, and the IDs of those with none whose photometry moved.
struct Census {
    exposed: Records,
    dark: u32,
    state_moved: u32,
    regime_moved: u32,
    others: Vec<String>,
    enveloped: Vec<String>,
}

/// The census of the systems `sets` (every planet, member and moon at the epoch), every 25 hold
/// against every 25 hold but P14.T53.c's exposed share.
///
/// - **Under a dark lid** (an ice bulk whose lid is not `H2O`, [`DarkLag::of`]), the record's
///   water ice lies within its lag's exposed share of the surface, and the condensates list no
///   `H2O` solid. The ice cover is at most the other ices' and that share, since the other
///   substances' frosts are the condensates' own and not W3 (a)'s item, and below 1, which the
///   belt test reads ([`lag::covers_part`](crate::planetary::hooks::figures::lag::covers_part)).
///   Where the surface did not scale the held list (its sum under 1 by more than
///   [`WHOLE_COVER_ROUNDING`]), the water ice is its lag's share held to its held share, to the
///   bit. Each is listed, with its lid, its held and exposed water ice and its state.
/// - **Nothing else moves under a dark lid** but the cover and what reads it: the condensates, the
///   crust, the bulk and the liquids to the bit, the photometry too unless the passes' rule 5
///   (P14.T53.e) moves the state; the states and the climate regimes that move, counted.
/// - **No other record moves**, but in its climate regime alone where its ice cover lies within
///   [`WHOLE_COVER_ROUNDING`] of 1, an ice belt that the belt test now reads as whole, and in its
///   photometry alone an ice bulk with no surface (an envelope over it), whose template reads the
///   state its passes' rule 5 published from its lag's share (each listed).
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the census's records, each section compared against the held rule"
)]
fn census(sets: &[AuditSet]) -> Census {
    let water = Gas::Water.substance();
    let exposed = with_every_25_hold(|| {
        assert!(exposed_ice_in_force());
        records(sets)
    });
    let held = with_every_25_hold(|| with_exposed_ice(ExposedIceRules::Held, || records(sets)));
    assert_eq!(exposed.len(), held.len(), "one walk's bodies, both rules");
    let (mut surfaces, mut dark, mut scaled, mut state_moved, mut regime_moved) =
        (0_u32, 0_u32, 0_u32, 0_u32, 0_u32);
    let mut by_lag: BTreeMap<String, u32> = BTreeMap::new();
    let (mut lines, mut others, mut enveloped) = (Vec::new(), Vec::new(), Vec::new());
    for ((id, record), (held_id, before)) in exposed.iter().zip(&held) {
        assert_eq!(id, held_id, "the same bodies in the same order");
        let (Section::Ok(surface), Section::Ok(old)) = (record.surface(), before.surface()) else {
            // A record with no surface moves only in its photometry, whose template reads the
            // state its passes publish: an ice bulk whose passes' rule 5 read its lag's share.
            assert_eq!(
                but_photometry(record),
                but_photometry(before),
                "{id}: a record with no surface moves only in its photometry"
            );
            let template = |r: &BodyRecord| r.photometry().ok().map(BodyPhotometry::template);
            if template(record) != template(before) || record.photometry() != before.photometry() {
                let water = record
                    .bulk()
                    .ok()
                    .map_or(0.0, |bulk| bulk.fractions().water());
                assert!(water > 0.0, "{id}: no ice bulk, yet its photometry moved");
                eprintln!(
                    "{id}: no surface, water {water:.4}, its template {:?} (held {:?})",
                    template(record),
                    template(before)
                );
                enveloped.push(id.clone());
            }
            continue;
        };
        surfaces += 1;
        let Some(lag) = DarkLag::of(surface.material(), lid_of(surface)) else {
            if format!("{record:?}") == format!("{before:?}") {
                continue;
            }
            // Only the regime, where the cover lies within rounding of 1 (a list the surface
            // scaled), which the belt test now reads as whole.
            let cover = surface.figures().ice_fraction();
            assert!(cover >= 1.0 - WHOLE_COVER_ROUNDING, "{id}: {cover:e}");
            let masked = |r: &BodyRecord, s: &Surface| {
                format!("{r:?}").replace(&format!("{:?}", s.conditions().climate_regime()), "_")
            };
            assert_eq!(masked(record, surface), masked(before, old), "{id}");
            let regime = |s: &Surface| s.conditions().climate_regime().map(|r| r.forcing());
            assert_eq!(regime(old), Some(Forcing::IceBelt), "{id}");
            assert_eq!(regime(surface), Some(Forcing::Seasonal), "{id}");
            others.push(format!("{id}: cover {cover:e}, an ice belt no longer"));
            continue;
        };
        dark += 1;
        *by_lag.entry(format!("{lag:?}")).or_default() += 1;
        // The partition lists the crust's water under no dark lid (§C3.1), under either rule.
        for s in [surface, old] {
            assert!(
                s.condensates()
                    .entry(water, CondensatePhase::Solid)
                    .is_none(),
                "{id}: an H2O solid under a {lag:?} lid"
            );
        }
        let (ices, old_ices) = (
            surface.figures().surface_ices(),
            old.figures().surface_ices(),
        );
        let (shown, was) = (water_share(ices), water_share(old_ices));
        let bound = lag.exposed_share();
        assert!(
            shown <= bound,
            "{id}: {lag:?} water ice {shown:e} over {bound:e}"
        );
        let others_sum = ices
            .iter()
            .filter(|&&(s, _)| s != water)
            .fold(0.0, |sum, &(_, share)| sum + share);
        let cover = surface.figures().ice_fraction();
        assert!(
            cover <= others_sum + bound + 1e-15,
            "{id}: cover {cover:e}, others {others_sum:e}, bound {bound:e}"
        );
        assert!(
            cover < 1.0,
            "{id}: the belt test's `ice < 1.0`, cover {cover:e}"
        );
        // A held list the surface scaled sums back to 1 only to rounding; one it did not scale,
        // whose water ice is the cover's own area, sums below it.
        let old_sum = old_ices.iter().fold(0.0, |sum, &(_, f)| sum + f);
        if old_sum < 1.0 - WHOLE_COVER_ROUNDING {
            float::assert_same_bits(shown, was.min(bound));
        } else {
            scaled += 1;
        }
        // Nothing but the cover and what reads it moves.
        for (what, now, then) in [
            (
                "bulk",
                format!("{:?}", record.bulk()),
                format!("{:?}", before.bulk()),
            ),
            (
                "condensates",
                format!("{:?}", surface.condensates()),
                format!("{:?}", old.condensates()),
            ),
            (
                "crust",
                format!("{:?}", surface.crust()),
                format!("{:?}", old.crust()),
            ),
            (
                "liquids",
                format!("{:?}", surface.figures().surface_liquids()),
                format!("{:?}", old.figures().surface_liquids()),
            ),
        ] {
            assert_eq!(now, then, "{id}: its {what}");
        }
        let state = surface.state() != old.state();
        let photometry =
            format!("{:?}", record.photometry()) != format!("{:?}", before.photometry());
        assert!(
            state || !photometry,
            "{id}: its photometry moves only with its state (rule 5): {:?}",
            surface.state()
        );
        state_moved += u32::from(state);
        let regime = surface.conditions().climate_regime() != old.conditions().climate_regime();
        regime_moved += u32::from(regime);
        lines.push(format!(
            "{id}: {lag:?} lid {:?}, {:?} {:?}{}, water ice {shown:.3e} (held {was:.6}), cover \
             {cover:.3e} (held {:.6}), others {others_sum:.3e}{}",
            lid_of(surface).map(CrustLithology::key),
            old.state(),
            surface.material(),
            if state {
                format!(" → {:?}", surface.state())
            } else {
                String::new()
            },
            old.figures().ice_fraction(),
            if regime { ", its regime moved" } else { "" },
        ));
    }
    eprintln!(
        "{surfaces} surfaces; {dark} ice bulks under a dark lid, by lag {by_lag:?} ({scaled} with \
         a held list the surface scaled; {state_moved} states and {regime_moved} regimes \
         moved):\n  {}\n{} other records moved:\n  {}\n{} with no surface moved: {enveloped:?}",
        lines.join("\n  "),
        others.len(),
        others.join("\n  "),
        enveloped.len(),
    );
    Census {
        exposed,
        dark,
        state_moved,
        regime_moved,
        others,
        enveloped,
    }
}

/// The census ([`census`]) over the first [`SAMPLE_SYSTEMS`] sample systems and the 15 golden
/// ones, which hold §C3.1's dark lids: 65 ice bulks under one, no state, climate regime or other
/// record moved; and the same records generated twice under every 25 hold.
#[test]
fn under_every_25_hold_an_ice_bulk_shows_its_lag_s_exposed_share_of_water_ice() {
    let sets = fast_sets();
    let census = census(&sets);
    assert!(census.dark > 20, "§C3.1's dark lids: {}", census.dark);
    assert_eq!((census.state_moved, census.regime_moved), (0, 0));
    assert_eq!(census.others, Vec::<String>::new());
    assert_eq!(census.enveloped, Vec::<String>::new());
    let again = with_every_25_hold(|| records(&sets));
    assert_eq!(
        format!("{:?}", census.exposed),
        format!("{again:?}"),
        "the same bits twice"
    );
}

/// The census's slow twin ([`census`]) over the rest of the 400 sample systems, run by name: no
/// state, climate regime or other record with a surface moved, and of those with none only the two
/// enveloped ice bulks ([`ENVELOPED`]).
#[test]
#[ignore = "slow: 340 sample systems generated twice, run by name"]
fn under_every_25_hold_the_rest_of_the_400_show_their_lag_s_exposed_share_of_water_ice() {
    let sets: Vec<AuditSet> = sample()[SAMPLE_SYSTEMS..]
        .iter()
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .collect();
    let census = census(&sets);
    assert!(census.dark > 0, "{}", census.dark);
    assert_eq!((census.state_moved, census.regime_moved), (0, 0));
    assert_eq!(census.others, Vec::<String>::new());
    let ids: Vec<String> = census
        .enveloped
        .iter()
        .map(|id| id.trim_start_matches("sample ").to_owned())
        .collect();
    assert_eq!(ids, ENVELOPED);
}

/// The passes' rule 5 reads the exposed share (P14.T53.e's state from the published ice cover):
/// each of the two enveloped ice bulks ([`ENVELOPED`]) publishes a `Snowball`'s photometry template
/// under every 25 hold but the exposed share's, its dark lid's whole cover read as ice, and another
/// with it. Each is printed with its states, its mean surface temperature and its pressure.
#[test]
fn under_every_25_hold_two_enveloped_ice_bulks_leave_the_snowball_by_their_lag_s_share() {
    use crate::planetary::derive::photometry::PhaseTemplate;

    let systems: Vec<u64> = ENVELOPED
        .iter()
        .map(|id| u64::from_str_radix(&id[..16], 16).expect("a body's ID opens with its system's"))
        .collect();
    let sets: Vec<AuditSet> = sample()
        .iter()
        .filter(|ctx| systems.contains(&ctx.id().raw()))
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .collect();
    assert_eq!(sets.len(), ENVELOPED.len(), "each lies in the sample");
    let walk = || {
        let mut found = BTreeMap::new();
        audit_walk(&sets, &mut |case| {
            let id = case.record.identity().id().to_string();
            if ENVELOPED.contains(&id.as_str()) {
                let air = case.derived.atmosphere();
                let template = case.record.photometry().ok().map(BodyPhotometry::template);
                found.insert(
                    id,
                    (
                        template,
                        format!(
                            "{:?} (carried {:?}), T_s {:.2} K, p_s {:.4e} Pa",
                            air.published_state(),
                            air.carried_state(),
                            air.surface_temperature().value(),
                            air.surface_pressure().map_or(f64::NAN, Pascals::value),
                        ),
                    ),
                );
            }
        });
        found
    };
    let exposed = with_every_25_hold(walk);
    let held = with_every_25_hold(|| with_exposed_ice(ExposedIceRules::Held, walk));
    assert_eq!(exposed.len(), ENVELOPED.len());
    for id in ENVELOPED {
        let ((now, line), (then, held_line)) = (&exposed[id], &held[id]);
        eprintln!("{id}: {now:?}, {line}\n  held: {then:?}, {held_line}");
        assert_eq!(*then, Some(PhaseTemplate::Snowball), "{id}");
        assert_ne!(*now, Some(PhaseTemplate::Snowball), "{id}");
    }
}
