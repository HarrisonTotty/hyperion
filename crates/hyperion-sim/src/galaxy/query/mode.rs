//! The query's mode: the present, or what an observer's sensors receive (plan 12, P12.T3; Design
//! note 4).
//!
//! Observed mode changes what is reported, never what is found. [`range_query_observed`] runs plan
//! 03's [`range_query`] unchanged — cells, padding, distance tests, the census and the unborn
//! filter all at the present — and then reads each system found at the retarded time
//! ([`observe_hit`](crate::observe::observe_hit)), so the systems, the census and the statistics
//! are the same in both modes.

use super::{RangeQuery, RangeResult, SystemSource, range_query};
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::features::members::KeepInteriors;
use crate::galaxy::placement::{CellCache, SystemKind};
use crate::observe::{Drift, Observer, StarsCache, TraceMotionError, observe_on_line};

/// What a query reports (plan 12's Provides).
///
/// The navigation computer's chart is the present ([`Now`](Self::Now)). A sensor's view is the
/// past light cone ([`ObservedFrom`](Self::ObservedFrom)): each system as its light, arriving at
/// the observer at the query's time, shows it. The systems found are the same in both (Design
/// note 4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum QueryMode {
    /// The present at the query's time: positions and states now.
    #[default]
    Now,
    /// The present as found, and with each system what an observer at this position receives
    /// from it at the query's time. The position must lie in the root cube, which
    /// [`RangeQueryBuilder::build`](super::RangeQueryBuilder::build) checks.
    ObservedFrom(GalacticPosition),
}

/// [`range_query`], and in observed mode what the query's observer receives from each system found
/// (plan 12, P12.T3).
///
/// The query is plan 03's, run unchanged, so the systems, the census and the statistics are
/// exactly [`range_query`]'s in either mode (Design note 4). In
/// [`QueryMode::ObservedFrom`] the result's [`observed`](RangeResult::observed) holds, parallel to
/// its systems, each one read at the retarded time by an observer at that position at the query's
/// time: its [`Retardation`](crate::observe::Retardation), its existence and brief when the light
/// left it, and the stated curvature error, as [`observe_hit`](crate::observe::observe_hit) gives
/// them. A system not yet born then is found and reported
/// [`NotYetBorn`](crate::stellar::system::SystemExistence::NotYetBorn). A rogue planet has no
/// stars and so no brief then. In [`QueryMode::Now`] the result is [`range_query`]'s and
/// `observed` is empty.
///
/// `cells` and `sources` are [`range_query`]'s; `stars` lends each system's stars, generating those
/// it has not got ([`NoStarsCache`](crate::observe::NoStarsCache) keeps none; a rogue planet's are
/// never asked for). A feature member's velocity is resolved from its feature's interior, which is
/// built once per feature for the call.
///
/// # Errors
///
/// [`TraceMotionError::CentreOrbitNotBuilt`] if a system found is a member of the galactic
/// centre, whose Kepler orbit waits for plan 09's P09.T28 (P12.T2 as built, provisional): the
/// whole observed answer is refused rather than one with a present row among observed ones. The
/// refusal comes before any system's stars are asked for.
///
/// # Panics
///
/// As [`range_query`], and if a feature member found does not resolve in `galaxy`, which only a
/// source that lends another galaxy's members can cause.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, range_query, range_query_observed};
/// use hyperion_sim::observe::NoStarsCache;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(12));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// // What a sensor 3,000 ly coreward of the Sun sees of the Sun's neighbourhood now.
/// let away = GalacticPosition::from_light_years([0.0, 23_000.0, 0.0]).ok_or("in range")?;
/// let query = RangeQuery::builder(sun, LightYears::new(10.0))
///     .mode(QueryMode::ObservedFrom(away))
///     .build()?;
/// let seen = range_query_observed(&galaxy, &mut NoCache::new(), &mut NoStarsCache, &[], &query)?;
/// let now = range_query(&galaxy, &mut NoCache::new(), &[], &query);
/// // The same systems and census as the present, and each one seen as it was 3,000 years ago.
/// assert_eq!(seen.systems(), now.systems());
/// assert_eq!(seen.census(), now.census());
/// for observed in seen.observed() {
///     let age = observed.retardation().light_age().as_julian_years_f64();
///     assert!((age - 3_000.0).abs() < 11.0);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn range_query_observed<C: CellCache, S: StarsCache>(
    galaxy: &Galaxy,
    cells: &mut C,
    stars: &mut S,
    sources: &[&dyn SystemSource],
    query: &RangeQuery,
) -> Result<RangeResult, TraceMotionError> {
    let result = range_query(galaxy, cells, sources, query);
    let QueryMode::ObservedFrom(position) = query.mode() else {
        return Ok(result);
    };
    let observer = Observer::new(position, query.time())
        .expect("a built query's observer is in the root cube and its time in the clock window");
    // The source that found a feature member had its interior; the hit does not carry the
    // member's velocity, so each feature's interior is built once more here, not once a member.
    let interiors = KeepInteriors::new(galaxy);
    let lines = result
        .systems()
        .iter()
        .map(|hit| Drift::of_record_in(galaxy, &interiors, hit.record()))
        .collect::<Result<Vec<_>, _>>()?;
    let seen = result
        .systems()
        .iter()
        .zip(&lines)
        .map(|(hit, line)| {
            if hit.record().kind() == SystemKind::RoguePlanet {
                observe_on_line(galaxy, hit, line, None, &observer)
            } else {
                stars.with_stars(galaxy, hit.record(), |system| {
                    observe_on_line(galaxy, hit, line, Some(system), &observer)
                })
            }
        })
        .collect();
    Ok(result.with_observed(seen))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::ROOT_HALF_WIDTH_LY;
    use crate::galaxy::features::members::NoInteriorCache;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{NoCache, SystemRecord, resolve};
    use crate::galaxy::query::SubstellarRequest;
    use crate::galaxy::query::{
        BuildRangeQueryError, LayerCounts, LayerSet, QuerySphere, SystemHit,
    };
    use crate::id::{CentreMemberId, SystemId};
    use crate::math;
    use crate::observe::{NoStarsCache, observe_hit, stars_of};
    use crate::rng::Seed;
    use crate::stellar::system::SystemStars;
    use crate::time::UniverseTime;
    use crate::units::LightYears;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x1203_0000_0000_0000),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn sun() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// Keeps every system's stars and counts what it built.
    #[derive(Debug, Default)]
    struct KeepStars {
        kept: BTreeMap<SystemId, SystemStars>,
        built: usize,
    }

    impl StarsCache for KeepStars {
        fn with_stars<R>(
            &mut self,
            galaxy: &Galaxy,
            record: &SystemRecord,
            f: impl FnOnce(&SystemStars) -> R,
        ) -> R {
            let built = &mut self.built;
            f(self.kept.entry(record.id()).or_insert_with(|| {
                *built += 1;
                stars_of(galaxy, &NoInteriorCache, record)
            }))
        }
    }

    /// A source that places one record at its epoch position and expects nothing.
    #[derive(Debug)]
    struct Placing(SystemRecord);

    impl SystemSource for Placing {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            LayerCounts::ZERO
        }

        fn systems_in_sphere(
            &self,
            _galaxy: &Galaxy,
            sphere: &QuerySphere,
            _layers: LayerSet,
            out: &mut Vec<SystemHit>,
        ) {
            let at = *self.0.epoch_position();
            let distance = LightYears::from(sphere.centre().distance_to(&at));
            out.push(SystemHit::new(self.0, at, distance));
        }

        fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
            false
        }
    }

    /// A point uniform in `[-half, half)` light-years about `about` on each axis.
    fn scatter(lcg: &mut Lcg, about: [f64; 3], half: [f64; 3]) -> GalacticPosition {
        let mut ly = about;
        for (axis, half) in ly.iter_mut().zip(half) {
            *axis += (2.0 * lcg.next_f64() - 1.0) * half;
        }
        GalacticPosition::from_light_years(ly).unwrap()
    }

    /// P12.T3: the set of IDs and the census are identical in both modes for 100 random queries,
    /// and every observed row is the system's own [`observe_hit`], whatever the stars cache held.
    ///
    /// Centres are scattered over the disc (galactocentric radii of 3,000–40,000 ly within 500 ly
    /// of the plane), radii 1–8 ly so that each query holds a few systems (one in ten, with the
    /// substellar layers, up to 1.5 ly), times over the whole clock window, and observers anywhere
    /// in the root cube.
    #[test]
    fn query_mode_finds_the_same_systems_and_census_in_both_modes() {
        let galaxy = galaxy();
        let mut lcg = Lcg::new(0x1203);
        let mut kept = KeepStars::default();
        let mut rows = 0;
        let mut rogues = 0;
        let cube = f64::from(ROOT_HALF_WIDTH_LY) - 1.0;
        for _ in 0..100 {
            let r = 3_000.0 + 37_000.0 * lcg.next_f64();
            let phi = std::f64::consts::TAU * lcg.next_f64();
            let centre = scatter(
                &mut lcg,
                [r * math::cos(phi), r * math::sin(phi), 0.0],
                [0.0, 0.0, 500.0],
            );
            let radius = LightYears::new(1.0 + 7.0 * lcg.next_f64());
            let years = i64::try_from(lcg.next_below(2_001)).unwrap() - 1_000;
            let time = UniverseTime::from_julian_years(years).unwrap();
            let sensor_at = scatter(&mut lcg, [0.0; 3], [cube; 3]);
            // One query in ten asks for the substellar layers too, within 1.5 ly, since rogue
            // planets have no stars and are observed without them.
            let builder = if lcg.next_below(10) == 0 {
                RangeQuery::builder(centre, LightYears::new(radius.value().min(1.5)))
                    .substellar(SubstellarRequest::BrownDwarfsAndRoguePlanets)
            } else {
                RangeQuery::builder(centre, radius)
            }
            .time(time);
            let now = builder.clone().build().unwrap();
            let observed = builder
                .mode(QueryMode::ObservedFrom(sensor_at))
                .build()
                .unwrap();

            let present = range_query(&galaxy, &mut NoCache::new(), &[], &now);
            let as_now =
                range_query_observed(&galaxy, &mut NoCache::new(), &mut NoStarsCache, &[], &now)
                    .unwrap();
            assert_eq!(as_now, present, "now mode is plan 03's answer");
            assert!(as_now.observed().is_empty());

            let plain = range_query(&galaxy, &mut NoCache::new(), &[], &observed);
            let seen =
                range_query_observed(&galaxy, &mut NoCache::new(), &mut kept, &[], &observed)
                    .unwrap();
            assert_eq!(plain, present, "the mode does not reach plan 03's query");
            assert_eq!(seen.systems(), present.systems());
            assert_eq!(seen.census(), present.census());
            assert_eq!(seen.stats(), present.stats());
            assert_eq!(seen.observed().len(), seen.systems().len());

            let sensor = Observer::new(sensor_at, time).unwrap();
            for (row, hit) in seen.observed().iter().zip(seen.systems()) {
                if hit.record().kind() == SystemKind::RoguePlanet {
                    assert_eq!(row.brief_then(), None);
                    rogues += 1;
                    continue;
                }
                let stars = &kept.kept[&hit.id()];
                assert_eq!(*row, observe_hit(&galaxy, hit, stars, &sensor).unwrap());
            }
            rows += seen.observed().len();
        }
        assert!(rows > 100, "the queries held {rows} systems in all");
        assert!(rogues > 0, "no rogue planet was observed");
        // The keeping cache built each system once, and was asked for each row.
        assert_eq!(kept.built, kept.kept.len());
    }

    /// The observer of an observed mode must lie in the root cube; a query in now mode ignores
    /// nothing, since it has none.
    #[test]
    fn query_mode_refuses_an_observer_outside_the_root_cube() {
        let edge = f64::from(ROOT_HALF_WIDTH_LY);
        let outside = GalacticPosition::from_light_years([edge + 10.0, 0.0, 0.0]).unwrap();
        assert!(!outside.in_root_cube());
        let built = RangeQuery::builder(sun(), LightYears::new(5.0))
            .mode(QueryMode::ObservedFrom(outside))
            .build();
        assert_eq!(built, Err(BuildRangeQueryError::ObserverOutsideRootCube));
        let query = RangeQuery::builder(sun(), LightYears::new(5.0))
            .build()
            .unwrap();
        assert_eq!(query.mode(), QueryMode::Now);
        assert_eq!(QueryMode::default(), QueryMode::Now);
    }

    /// A member of the galactic centre found by an observed query refuses the whole answer, with
    /// obs12a's typed error, until plan 09's P09.T28 builds its orbit; the present answer holds
    /// it. The black hole, member zero, stands in for a centre source, which none yet is; the
    /// source hands it to a small query at the Sun, since the centre's own cells are the densest
    /// in the galaxy and a test need not generate them.
    #[test]
    fn query_mode_refuses_a_centre_member_until_its_orbit_is_built() {
        let galaxy = galaxy();
        let id = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        let black_hole = resolve(&galaxy, id).unwrap();
        let source = Placing(black_hole);
        let observer = GalacticPosition::from_light_years([0.0, 5_000.0, 0.0]).unwrap();
        let query = RangeQuery::builder(sun(), LightYears::new(3.0))
            .mode(QueryMode::ObservedFrom(observer))
            .build()
            .unwrap();
        let present = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
        assert!(present.systems().iter().any(|hit| hit.id() == id));
        let mut kept = KeepStars::default();
        assert_eq!(
            range_query_observed(&galaxy, &mut NoCache::new(), &mut kept, &[&source], &query),
            Err(TraceMotionError::CentreOrbitNotBuilt(id))
        );
        assert_eq!(kept.built, 0, "no stars are built for a refused answer");
    }
}
