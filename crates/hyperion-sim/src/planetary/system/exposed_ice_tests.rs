//! P14.T53.c's exposed share through a lag over the record (decision-p14-t54a-ganymede §C3.1 and
//! §C3.4; decision-bump-plan-after-22 V3.9, W3 (a)), in force from generator version 25: a census
//! of the record under every 25 hold.

use std::collections::BTreeMap;

use super::audit_tests::{AuditSet, audit_walk};
use super::tests::{GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, sample};
use super::*;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::hooks::crust::CrustLithology;
use crate::planetary::testing::with_every_25_hold;
use crate::substance::SubstanceId;
use crate::units::Pascals;

/// How many of the sample's systems the fast census walks, the first in its order, as §C3.1's test
/// walks them; its slow twin walks the rest.
const SAMPLE_SYSTEMS: usize = 60;

/// The two ice bulks of the 400 with no surface section, each under a supercritical envelope,
/// whose photometry's template the passes' rule 5 does not make a `Snowball`'s on their lag's
/// exposed share (P14.T53.e's state from the published ice cover), where their dark lid's whole
/// cover read as ice made one.
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

/// The share of the record's ices `ices` that is water's, 0 where it lists none.
fn water_share(ices: &[(SubstanceId, f64)]) -> f64 {
    let water = Gas::Water.substance();
    ices.iter()
        .find(|&&(id, _)| id == water)
        .map_or(0.0, |&(_, share)| share)
}

/// What a census found ([`census`]): its records under every 25 hold and its count of ice bulks
/// under a dark lid.
struct Census {
    records: Records,
    dark: u32,
}

/// The census of the systems `sets` (every planet, member and moon at the epoch) under every 25
/// hold.
///
/// Under a dark lid (an ice bulk whose lid is not `H2O`, [`DarkLag::of`]), the record's water ice
/// lies within its lag's exposed share of the surface, and the condensates list no `H2O` solid.
/// The ice cover is at most the other ices' and that share, since the other substances' frosts are
/// the condensates' own and not W3 (a)'s item, and below 1, which the belt test reads
/// ([`lag::covers_part`](crate::planetary::hooks::figures::lag::covers_part)). Each is listed, with
/// its lid, its water ice, its cover and its state.
fn census(sets: &[AuditSet]) -> Census {
    let water = Gas::Water.substance();
    let records = with_every_25_hold(|| records(sets));
    let (mut surfaces, mut dark) = (0_u32, 0_u32);
    let mut by_lag: BTreeMap<String, u32> = BTreeMap::new();
    let mut lines = Vec::new();
    for (id, record) in &records {
        let Section::Ok(surface) = record.surface() else {
            continue;
        };
        surfaces += 1;
        let Some(lag) = DarkLag::of(surface.material(), lid_of(surface)) else {
            continue;
        };
        dark += 1;
        *by_lag.entry(format!("{lag:?}")).or_default() += 1;
        // The partition lists the crust's water under no dark lid (§C3.1).
        assert!(
            surface
                .condensates()
                .entry(water, CondensatePhase::Solid)
                .is_none(),
            "{id}: an H2O solid under a {lag:?} lid"
        );
        let ices = surface.figures().surface_ices();
        let shown = water_share(ices);
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
        lines.push(format!(
            "{id}: {lag:?} lid {:?}, {:?} {:?}, water ice {shown:.3e}, cover {cover:.3e}, others \
             {others_sum:.3e}",
            lid_of(surface).map(CrustLithology::key),
            surface.state(),
            surface.material(),
        ));
    }
    eprintln!(
        "{surfaces} surfaces; {dark} ice bulks under a dark lid, by lag {by_lag:?}:\n  {}",
        lines.join("\n  "),
    );
    Census { records, dark }
}

/// The census ([`census`]) over the first [`SAMPLE_SYSTEMS`] sample systems and the 15 golden
/// ones, which hold §C3.1's dark lids: more than 20 ice bulks under one; and the same records
/// generated twice under every 25 hold.
#[test]
fn under_every_25_hold_an_ice_bulk_shows_its_lag_s_exposed_share_of_water_ice() {
    let sets = fast_sets();
    let census = census(&sets);
    assert!(census.dark > 20, "§C3.1's dark lids: {}", census.dark);
    let again = with_every_25_hold(|| records(&sets));
    assert_eq!(
        format!("{:?}", census.records),
        format!("{again:?}"),
        "the same bits twice"
    );
}

/// The census's slow twin ([`census`]) over the rest of the 400 sample systems, run by name: its
/// dark lids under the same bounds.
#[test]
#[ignore = "slow: 340 sample systems, run by name"]
fn under_every_25_hold_the_rest_of_the_400_show_their_lag_s_exposed_share_of_water_ice() {
    let sets: Vec<AuditSet> = sample()[SAMPLE_SYSTEMS..]
        .iter()
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .collect();
    let census = census(&sets);
    assert!(census.dark > 0, "{}", census.dark);
}

/// The passes' rule 5 reads the exposed share (P14.T53.e's state from the published ice cover):
/// neither of the two enveloped ice bulks ([`ENVELOPED`]) publishes a `Snowball`'s photometry
/// template under every 25 hold. Each is printed with its states, its mean surface temperature and
/// its pressure.
#[test]
fn under_every_25_hold_two_enveloped_ice_bulks_are_no_snowballs_by_their_lag_s_share() {
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
    assert_eq!(exposed.len(), ENVELOPED.len());
    for id in ENVELOPED {
        let (now, line) = &exposed[id];
        eprintln!("{id}: {now:?}, {line}");
        assert_ne!(*now, Some(PhaseTemplate::Snowball), "{id}");
    }
}
