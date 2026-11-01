//! P10.T10.b's registration member and P09.T37.b under the 27 rules (plan 10 and plan 09): a range
//! query served as the `systems_in_range` handler serves it, through [`list_for`] and
//! [`served_range_query`] on a pool of its own, takes the global list's members, the galactic
//! centre's among them, and its rows carry their origins.
//!
//! The server builds its galaxies at the generator version in force, so the test's galaxy cache
//! builds the server tests' galaxy under every 27 hold instead, with its full potential, which the
//! list needs.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hyperion_protocol::{
    MassLayer, MemberClassWire, QueryModeDto, SystemOriginWire, SystemsInRange,
    SystemsInRangeRequest,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::members::{StreamBandShares, stream_cell_members};
use hyperion_sim::galaxy::global_list::tube::KNOTS_PER_ARM;
use hyperion_sim::galaxy::global_list::{GlobalListSource, StreamNumber};
use hyperion_sim::galaxy::imf::{MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::SystemOrigin;
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, RangeResult};
use hyperion_sim::id::{Layer, SystemIdKind};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};

use super::{list_for, row_origins};
use crate::FeatureMembers;
use crate::compute::{
    CancelToken, CpuPool, GalaxyCache, GalaxyKey, GlobalListService, Priority, RangeCaches,
    ServedQuery, SharedCellCache, SharedCentreOrbitCache, SharedClassCellCache,
    SharedClusterModelCache, SharedFeatureCellCache, SharedSprayForces, TubeTableCache,
    served_range_query, served_sources,
};
use crate::convert::origins::SEED;
use crate::convert::{RangeRequest, systems_in_range, wire_time};
use crate::limits::{
    BULK_QUEUE_CAPACITY, GLOBAL_LIST_CACHE_BYTES, INTERACTIVE_QUEUE_CAPACITY,
    TUBE_TABLE_CACHE_BYTES,
};

/// The pool's workers, as the slow profile reserves for the test (`.config/nextest.toml`).
const WORKERS: usize = 8;

/// `future` within twenty minutes, which a stalled pool would never meet.
async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_mins(20), future)
        .await
        .expect("the pool does not stall")
}

/// The key the server tests' galaxy is cached under.
fn key() -> GalaxyKey {
    GalaxyKey::new(SEED, GENERATOR_VERSION)
}

/// What the test serves its queries with: a pool, a galaxy cache that builds under every 27 hold,
/// the global list's service over it with its tube tables, and the range query's caches.
struct Served {
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
    lists: GlobalListService,
    tubes: Arc<TubeTableCache>,
}

impl Served {
    fn new() -> Self {
        let pool = Arc::new(
            CpuPool::new(
                NonZeroUsize::new(WORKERS).expect("at least one worker"),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .expect("the pool starts"),
        );
        let galaxies = Arc::new(GalaxyCache::with_builder(Arc::clone(&pool), |seed| {
            Galaxy::from_params_at_version(
                seed,
                GalaxyParams::from_seed(seed, MassFunctionKind::default()),
                GeneratorVersion::new(27),
            )
            .expect("a drawn galaxy builds")
            .with_full_potential()
        }));
        let tubes = Arc::new(TubeTableCache::new(TUBE_TABLE_CACHE_BYTES));
        let lists = GlobalListService::new(
            Arc::clone(&pool),
            Arc::clone(&galaxies),
            (
                Arc::new(SharedSprayForces::new(1 << 20)),
                Arc::clone(&tubes),
            ),
            GLOBAL_LIST_CACHE_BYTES,
        );
        Self {
            pool,
            galaxies,
            lists,
            tubes,
        }
    }

    /// Fresh range query caches of 256 MiB each, the centre's orbits' of `centre_bytes`.
    fn caches(centre_bytes: usize) -> RangeCaches {
        let bytes = 256 << 20;
        RangeCaches::new(
            Arc::new(SharedCellCache::new(bytes)),
            Arc::new(SharedFeatureCellCache::new(bytes)),
            Arc::new(SharedClusterModelCache::new(bytes)),
            (
                Arc::new(SharedClassCellCache::new(bytes)),
                Arc::new(SharedCentreOrbitCache::new(centre_bytes)),
            ),
        )
    }

    /// `request` answered as the handler answers it, through `caches`: the sim's result and the
    /// wire's rows. It prints how long it took.
    async fn answer(
        &self,
        caches: &RangeCaches,
        galaxy: &Arc<Galaxy>,
        request: SystemsInRangeRequest,
    ) -> (RangeResult, SystemsInRange) {
        let started = Instant::now();
        let (radius, time) = (request.radius_ly, request.time.seconds);
        let query = RangeRequest::try_from(&request)
            .expect("a valid request")
            .into_query();
        let list = within(list_for(
            (&self.lists, &self.tubes),
            FeatureMembers::Served,
            (galaxy, key()),
            &query,
        ))
        .await
        .expect("the list and its tables build")
        .expect("a query under every 27 hold reads the list");
        let served = ServedQuery::new(
            Arc::clone(galaxy),
            key(),
            query.clone(),
            FeatureMembers::Served,
        )
        .with_list(list);
        let answered = within(served_range_query(
            &self.pool,
            caches,
            &served,
            &CancelToken::new(),
            move |galaxy, result, reads| {
                let wire = systems_in_range(
                    galaxy,
                    request,
                    &query,
                    &result,
                    None,
                    None,
                    row_origins(FeatureMembers::Served, reads),
                );
                (result, wire)
            },
        ))
        .await
        .expect("the query's jobs run");
        eprintln!(
            "  {radius} ly at {time} s: {} rows in {:.2} s",
            answered.1.systems.len(),
            started.elapsed().as_secs_f64()
        );
        answered
    }
}

/// A request of every stellar layer within `radius_ly` of `centre` at `years` from the epoch.
fn request(centre: &GalacticPosition, radius_ly: f64, years: i64) -> SystemsInRangeRequest {
    SystemsInRangeRequest {
        universe: hyperion_protocol::UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
        centre: hyperion_protocol::GalacticPosition {
            cell_ly: centre.cell().to_array(),
            offset_m: centre.offset_metres(),
        },
        radius_ly,
        time: wire_time(UniverseTime::from_julian_years(years).expect("in the clock window")),
        min_layer: MassLayer::A,
        limit: 20_000,
        include_stellar: false,
        mode: QueryModeDto::Now,
    }
}

/// `p` ly from the galactic centre.
fn at(p: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(p).expect("in the cube")
}

/// Whether no system ID appears twice among `result`'s hits.
fn unique(result: &RangeResult) -> bool {
    let mut ids: Vec<u64> = result.systems().iter().map(|hit| hit.id().raw()).collect();
    ids.sort_unstable();
    ids.windows(2).all(|pair| pair[0] != pair[1])
}

/// A member of a stream of `galaxy`'s list, in band A inside its shell, and its stream: the first
/// stream with one about its leading arm's last knot, its table built on the pool.
async fn a_stream_member(
    served: &Served,
    galaxy: &Arc<Galaxy>,
) -> (StreamNumber, GalacticPosition) {
    let list = within(served.lists.get(key()))
        .await
        .expect("the list builds");
    let mut cell_members = Vec::new();
    for spec in list.list().debris().streams() {
        let Some(entry) = list.list().stream_entry(spec.number()) else {
            continue;
        };
        let tube = within(
            served
                .lists
                .tube((galaxy, key()), spec, Priority::Interactive),
        )
        .await
        .expect("a drawn stream's tube builds");
        let shares = StreamBandShares::of(galaxy, spec);
        let Some(knot) = tube
            .knots()
            .nth(usize::try_from(KNOTS_PER_ARM).expect("a count"))
        else {
            continue;
        };
        let middle = knot.centre();
        for cell in tube.cells_touching(&middle, LightYears::new(200.0)) {
            stream_cell_members(galaxy, &tube, &shares, cell, MassBand::A, &mut cell_members);
            let shell = entry.shell();
            if let Some(member) = cell_members.iter().find(|m| {
                let p = m.record().epoch_position().to_light_years_f64();
                let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                shell.inner().value() <= r && r <= shell.outer().value()
            }) {
                return (spec.number(), *member.record().epoch_position());
            }
        }
    }
    panic!("no stream of the galaxy has a member about its middle")
}

/// The stream query's rows: stream `number`'s members are among them, each with the `stream`
/// origin and a member class.
fn check_stream_rows(wire: &SystemsInRange, number: StreamNumber) {
    let streamed: Vec<_> = wire
        .systems
        .iter()
        .filter(|row| matches!(row.origin, Some(SystemOriginWire::Stream { .. })))
        .collect();
    let own = Some(SystemOriginWire::Stream {
        stream: number.get(),
    });
    assert!(
        streamed.iter().any(|row| row.origin == own),
        "stream {}'s members on its tube",
        number.get()
    );
    assert!(streamed.iter().all(|row| row.member_class.is_some()));
}

/// The centre query's rows: more than ten of the centre, the black hole's with no class and a cell
/// member's with its centre class.
fn check_centre_rows(wire: &SystemsInRange) {
    let centre_rows: Vec<_> = wire
        .systems
        .iter()
        .filter(|row| row.origin == Some(SystemOriginWire::CentreMember))
        .collect();
    eprintln!(
        "0.05 ly about the black hole a year on: {} rows, {} of the centre",
        wire.systems.len(),
        centre_rows.len()
    );
    assert!(centre_rows.len() > 10);
    let hole = hyperion_protocol::SystemIdHex::from_u64(
        hyperion_sim::id::SystemId::from(hyperion_sim::id::CentreMemberId::CENTRAL_BLACK_HOLE)
            .raw(),
    );
    assert!(
        centre_rows
            .iter()
            .any(|row| row.id == hole && row.member_class.is_none())
    );
    assert!(
        centre_rows
            .iter()
            .any(|row| matches!(row.member_class, Some(MemberClassWire::Centre { .. })))
    );
}

/// No system ID appears twice among the sources' hits over 100 centres at the epoch: 50 about the
/// black hole out to 5 ly, from 0.01 to 0.1 ly across, and 50 about the stream's `member` out to
/// 20 ly, 5 ly across, each with the sources a served query takes beside the grid; and the hits
/// hold the centre's members and the list's, each centre ID of the centre's origin. The
/// grid places grid IDs alone, which no source does, so the grid's own walk, some ten minutes at
/// the black hole (the lane's outbox), cannot add a second copy.
async fn no_id_twice(
    served: &Served,
    caches: &RangeCaches,
    galaxy: &Arc<Galaxy>,
    member: &GalacticPosition,
) {
    let m = member.to_light_years_f64();
    let mut origins = [0_usize; 3];
    for k in 0..100_u32 {
        let (along, spread) = (f64::from(k % 50) / 50.0, f64::from(k % 7) - 3.0);
        let (centre, radius) = if k < 50 {
            let r = 5.0 * along * along;
            (at([r, 0.3 * spread * along, 0.1 * r]), 0.01 + 0.09 * along)
        } else {
            let p = [
                m[0] + 20.0 * along,
                m[1] + 2.0 * spread,
                m[2] - 10.0 * along,
            ];
            (at(p), 5.0)
        };
        let query = RangeRequest::try_from(&request(&centre, radius, 0))
            .expect("a valid request")
            .into_query();
        let list = within(list_for(
            (&served.lists, &served.tubes),
            FeatureMembers::Served,
            (galaxy, key()),
            &query,
        ))
        .await
        .expect("the list and its tables build")
        .expect("a query under every 27 hold reads the list");
        let sphere = QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a positive radius");
        let stellar = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .fold(LayerSet::EMPTY, LayerSet::with);
        let served_query =
            ServedQuery::new(Arc::clone(galaxy), key(), query, FeatureMembers::Served)
                .with_list(list);
        let hits = served_sources(caches, &served_query, |sources| {
            let mut hits = Vec::new();
            for source in sources {
                source.systems_in_sphere(galaxy, &sphere, stellar, &mut hits);
            }
            hits
        });
        let mut ids: Vec<u64> = hits.iter().map(|hit| hit.id().raw()).collect();
        ids.sort_unstable();
        assert!(
            ids.windows(2).all(|pair| pair[0] != pair[1]),
            "centre {k}: an ID twice"
        );
        for hit in &hits {
            let origin = hit.record().origin();
            match origin {
                SystemOrigin::CentreMember { .. } => origins[0] += 1,
                SystemOrigin::GlobalListMember { .. } => origins[1] += 1,
                _ => origins[2] += 1,
            }
            match hit.id().kind() {
                SystemIdKind::Centre(_) => {
                    assert!(matches!(origin, SystemOrigin::CentreMember { .. }));
                }
                SystemIdKind::Grid(_) => panic!("a source placed a grid ID"),
                SystemIdKind::FeatureMember(_)
                | SystemIdKind::Stream(_)
                | SystemIdKind::DwarfCore(_)
                | SystemIdKind::Pinned(_)
                | SystemIdKind::Catalogue(_) => {}
            }
        }
    }
    eprintln!(
        "over 100 centres: {} centre members, {} list members and {} others, none twice",
        origins[0], origins[1], origins[2]
    );
    assert!(origins[0] > 0 && origins[1] > 0, "{origins:?}");
}

/// P10.T10.b's registration and P09.T37.b, under every 27 hold, served as the handler serves them:
///
/// - a query on a stream's tube returns its members with the `stream` origin and a member class,
///   and one at the black hole returns `centre_member` rows, the black hole's with no class and a
///   cell member's with its centre class;
/// - P09.T37's census lines are identical whatever is cached: each query's result is the same
///   cold, warm and with the centre's orbits kept nowhere;
/// - P09.T37's query at the very centre: nothing fits at 50 ly, and the census is complete at
///   0.05 ly (a year on);
/// - no system ID appears twice among the sources' hits, over 100 centres: the galactic centre's
///   neighbourhood and a stream's crossing, where the centre's members and the list's are beside
///   the features' and the catalogue classes' ([`no_id_twice`]).
#[tokio::test]
#[ignore = "slow: a galaxy under every 27 hold with its full potential, its list and the tables a \
            stream's query reads"]
async fn from_27_a_range_query_serves_the_list_s_members_and_the_centre_s() {
    let started = Instant::now();
    let served = Served::new();
    let galaxy = within(served.galaxies.get(key()))
        .await
        .expect("the galaxy builds");
    assert!(GlobalListSource::is_registered_in(&galaxy));
    let warm = Served::caches(256 << 20);
    eprintln!(
        "the galaxy built in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    let (number, member) = a_stream_member(&served, &galaxy).await;
    let started = Instant::now();
    let on_stream = request(&member, 5.0, 0);
    let (stream_result, wire) = served.answer(&warm, &galaxy, on_stream.clone()).await;
    eprintln!(
        "stream {}: {} rows in 5 ly, the list and its tables built in {:.1} s",
        number.get(),
        wire.systems.len(),
        started.elapsed().as_secs_f64()
    );
    check_stream_rows(&wire, number);
    // A year on: a century on, the grid's Kepler-regime records and the centre's full scan on
    // their exact orbits take some ten minutes cold (the lane's outbox).
    let at_centre = request(&GalacticPosition::ORIGIN, 0.05, 1);
    let (centre_result, wire) = served.answer(&warm, &galaxy, at_centre.clone()).await;
    check_centre_rows(&wire);
    // P09.T37's census lines, whatever is cached: the answers above were cold, and the same warm
    // and with the centre's orbits kept nowhere.
    for (asked, expected) in [(on_stream, &stream_result), (at_centre, &centre_result)] {
        assert!(unique(expected));
        let (warm_again, _) = served.answer(&warm, &galaxy, asked.clone()).await;
        assert_eq!(&warm_again, expected, "warm");
        let (unkept, _) = served.answer(&Served::caches(0), &galaxy, asked).await;
        assert_eq!(&unkept, expected, "the centre's orbits kept nowhere");
    }
    // P09.T37's query at the very centre.
    let (fifty, _) = served
        .answer(&warm, &galaxy, request(&GalacticPosition::ORIGIN, 50.0, 0))
        .await;
    assert_eq!(
        fifty.census().complete_down_to(),
        None,
        "{:?}",
        fifty.census()
    );
    assert_eq!(fifty.systems(), &[]);
    assert_eq!(
        centre_result.census().complete_down_to(),
        Some(Layer::A),
        "{:?}",
        centre_result.census()
    );
    no_id_twice(&served, &warm, &galaxy, &member).await;
    within(served.pool.shutdown())
        .await
        .expect("the pool stops");
}
