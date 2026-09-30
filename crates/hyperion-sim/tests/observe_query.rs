//! Plan 12, P12.T3: the observed range query among a feature's members, whose velocities need
//! the feature's interior.
//!
//! The fast test in `galaxy::query::mode` pins the grid alone over 100 random queries. This one
//! adds plan 09's member source in a galaxy with its full potential, so it builds interiors and is
//! slow: queries inside the first feature with members within 3,000 ly of the Sun find the same
//! members and census in both modes, and each observed member is its own `observe_hit`, which
//! resolves the member afresh, although the query builds each feature's interior once. A member's
//! stars are its cluster's composition (`stars_of`), not a grid system's.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::members::{
    FeatureInterior, FeatureMemberSource, KeepInteriors,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{NoCache, SystemOrigin};
use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, range_query, range_query_observed};
use std::collections::BTreeMap;

use hyperion_sim::galaxy::placement::SystemRecord;
use hyperion_sim::id::SystemId;
use hyperion_sim::observe::{Observer, StarsCache, observe_hit, stars_of};
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_testkit::lcg::Lcg;

/// The seed of the galaxy the queries run in.
const SEED: u64 = 0x1203_7000_0000_0000;

/// Keeps every system's stars, their features' interiors from `interiors`.
struct KeepStars<'a> {
    interiors: &'a KeepInteriors<'a>,
    kept: BTreeMap<SystemId, SystemStars>,
}

impl StarsCache for KeepStars<'_> {
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R {
        let interiors = self.interiors;
        f(self
            .kept
            .entry(record.id())
            .or_insert_with(|| stars_of(galaxy, interiors, record)))
    }
}

/// P12.T3: the IDs and the census are identical in both modes among a feature's members, and every
/// observed member is its own `observe_hit`.
#[test]
#[ignore = "slow: a full-potential galaxy and a feature interior, 20 queries in each mode"]
fn observed_query_agrees_with_now_among_feature_members() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential();
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let interior = FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find_map(|f| FeatureInterior::of(&galaxy, &f))
        .expect("a feature with members lies within 3,000 ly of the Sun");
    let [x, y, z] = interior.feature().position().to_light_years_f64();
    let interiors = KeepInteriors::new(&galaxy);
    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    let mut stars = KeepStars {
        interiors: &interiors,
        kept: BTreeMap::new(),
    };
    let mut lcg = Lcg::new(0x0001_2037);
    let mut members = 0;
    for _ in 0..20 {
        let mut offset = || (2.0 * lcg.next_f64() - 1.0) * 5.0;
        let centre = GalacticPosition::from_light_years([x + offset(), y + offset(), z + offset()])
            .expect("in range");
        let radius = LightYears::new(2.0 + 6.0 * lcg.next_f64());
        let years = i64::try_from(lcg.next_below(2_001)).expect("small") - 1_000;
        let time = UniverseTime::from_julian_years(years).expect("in range");
        let sensor_at = GalacticPosition::from_light_years([
            (2.0 * lcg.next_f64() - 1.0) * 60_000.0,
            (2.0 * lcg.next_f64() - 1.0) * 60_000.0,
            (2.0 * lcg.next_f64() - 1.0) * 3_000.0,
        ])
        .expect("in range");
        let builder = RangeQuery::builder(centre, radius).time(time);
        let now = builder.clone().build().expect("a query");
        let observed = builder
            .mode(QueryMode::ObservedFrom(sensor_at))
            .build()
            .expect("a query");
        let present = range_query(&galaxy, &mut NoCache::new(), &[&source], &now);
        let seen = range_query_observed(
            &galaxy,
            &mut NoCache::new(),
            &mut stars,
            &[&source],
            &observed,
        )
        .expect("no centre member lies near the Sun");
        assert_eq!(seen.systems(), present.systems());
        assert_eq!(seen.census(), present.census());
        assert_eq!(seen.stats(), present.stats());
        let sensor = Observer::new(sensor_at, time).expect("inside");
        for (row, hit) in seen.observed().iter().zip(seen.systems()) {
            let system = &stars.kept[&hit.id()];
            assert_eq!(
                *row,
                observe_hit(&galaxy, hit, system, &sensor).expect("not a centre member")
            );
            if matches!(hit.record().origin(), SystemOrigin::FeatureMember { .. }) {
                members += 1;
            }
        }
    }
    assert!(members > 50, "the queries found {members} members in all");
}
