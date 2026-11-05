//! P10.T10.b's registration member and P09.T37 under the 27 rules (plan 10 and plan 09): every
//! system a range query returns, the galactic centre's members, the global list's and the
//! catalogue classes' hosts among them, is placed by its ID as its row places it
//! ([`system_motion`]), a ship can be framed on it, and observed mode reads it.

use std::sync::Arc;

use hyperion_protocol::SystemsInRange;
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity, LyCell, SystemPosition};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueClassSource, ClassId, system_of,
};
use hyperion_sim::galaxy::features::centre::CentreCellOrbits;
use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
use hyperion_sim::galaxy::global_list::GlobalListSource;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::motion::StateAt;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, RangeResult, SystemHit, SystemSource};
use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemIdKind};
use hyperion_sim::observe::Observer;
use hyperion_sim::stellar::brief::BriefModel;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{KilometresPerSecond, LightYears, MetresPerSecond};

use super::super::observe::observe_row;
use super::super::scene::frame_line;
use super::registration::{Served, a_stream_member, key, request, within};
use crate::compute::{
    ListedTubes, Listing, Priority, RangeCaches, SharedCentreOrbitCache, SharedClusterModelCache,
    SharedSystemCache,
};
use crate::convert::{ListMembers, MotionReads, SystemLine, system_motion};

/// A position's cell and its offset's bits, which tell two points apart bit for bit.
fn bits(position: &GalacticPosition) -> (LyCell, [u64; 3]) {
    (position.cell(), position.offset_metres().map(f64::to_bits))
}

/// A velocity as a range row carries it, km/s, in bits.
fn row_velocity(velocity: GalacticVelocity) -> [u64; 3] {
    velocity.metres_per_second().map(|v| {
        KilometresPerSecond::from(MetresPerSecond::new(v))
            .value()
            .to_bits()
    })
}

/// A served query's result and its rows.
type Answer = (RangeResult, SystemsInRange);

/// The first hit of `answer` that `wanted` picks, and its row's velocity, km/s, in bits.
fn row_of(answer: &Answer, wanted: impl Fn(&SystemHit) -> bool) -> Option<(SystemHit, [u64; 3])> {
    let (result, wire) = answer;
    let (index, hit) = result
        .systems()
        .iter()
        .enumerate()
        .find(|(_, h)| wanted(h))?;
    Some((*hit, wire.systems[index].velocity_km_s.map(f64::to_bits)))
}

/// `years` from the epoch.
fn at_years(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).expect("inside the clock's range")
}

/// Every stellar layer.
fn stellar() -> LayerSet {
    [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with)
}

/// The first core-collapse entry of the class grid along a line through the inner disc.
fn a_catalogue_host(galaxy: &Galaxy) -> SystemRecord {
    let process = CoreCollapseProcess::new(galaxy);
    let entry = (8..28)
        .map(|y| {
            CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])
                .expect("a cell of the root cube")
        })
        .flat_map(|key| CatalogueClassCell::generate(galaxy, &process, key).into_entries())
        .next()
        .expect("the inner disc has recent deaths");
    *system_of(galaxy, entry.id())
        .expect("an entry resolves")
        .record()
}

/// The hit `source` makes of `record` in a sphere of 0.01 ly about `about` at `t`: where the
/// source that found it places it then.
fn source_hit(
    galaxy: &Galaxy,
    source: &dyn SystemSource,
    record: &SystemRecord,
    about: &GalacticPosition,
    t: UniverseTime,
) -> Option<SystemHit> {
    let sphere = QuerySphere::new(*about, LightYears::new(0.01), t, LightYears::ZERO)
        .expect("a positive radius");
    let mut hits = Vec::new();
    source.systems_in_sphere(galaxy, &sphere, stellar(), &mut hits);
    hits.into_iter().find(|hit| hit.id() == record.id())
}

/// Where the centre's source places the cell member `record` at `t`: its trajectory's state, as
/// its cell's orbits hold it.
fn cell_member_at(galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime) -> GalacticPosition {
    let SystemIdKind::Centre(id) = record.id().kind() else {
        panic!("a centre ID");
    };
    let MemberSlot::InCell {
        band, level, cell, ..
    } = id.slot()
    else {
        panic!("a cell member");
    };
    let placement = CentrePlacement::new(galaxy.centre().expect("the centre builds"));
    let cell = placement.grid().cell(level, cell).expect("an owned cell");
    let orbits = CentreCellOrbits::new(galaxy, &placement, MassBand::of_layer(band), cell);
    let trajectory = orbits
        .trajectories()
        .iter()
        .find(|trajectory| trajectory.start().record().id() == record.id())
        .expect("the cell holds its member");
    match trajectory.state_at(t) {
        StateAt::Exists(state) => *state.position(),
        StateAt::NoSuchSystem => panic!("a centre member is a system at every time"),
    }
}

/// One system of a kind, as a row found it.
struct Picked {
    what: &'static str,
    hit: SystemHit,
    velocity: [u64; 3],
    t: UniverseTime,
}

impl Picked {
    /// Whether it is a member of the galactic centre.
    fn of_the_centre(&self) -> bool {
        matches!(
            self.hit.record().origin(),
            SystemOrigin::CentreMember { .. }
        )
    }
}

/// One system of each kind a range query returns beside the grid under every 27 hold, each from a
/// served query's rows: a stream's and a dwarf core's members, a catalogue class's host, and the
/// black hole and a member of the centre's cells, at the epoch and a year on; and the listing of a
/// stream member, the galaxy's list.
async fn pick(
    served: &Served,
    caches: &RangeCaches,
    galaxy: &Arc<Galaxy>,
) -> (Vec<Picked>, Listing) {
    let at_epoch = |what, (hit, velocity)| Picked {
        what,
        hit,
        velocity,
        t: UniverseTime::EPOCH,
    };
    let (_, member) = a_stream_member(served, galaxy).await;
    let rows = served
        .answer(caches, galaxy, request(&member, 50.0, 0))
        .await;
    let streamed = row_of(&rows, |h| matches!(h.id().kind(), SystemIdKind::Stream(_)))
        .expect("a stream's member on its tube");
    let listing = within(served.lists.listing(key(), streamed.0.id()))
        .await
        .expect("the list builds");
    let mut picked = vec![at_epoch("a stream's member", streamed)];
    let held = within(served.lists.get(key(), Priority::Interactive))
        .await
        .expect("the list");
    let mut cored = None;
    for spec in held.list().debris().cores().iter().take(4) {
        let centre = GalacticPosition::ORIGIN
            .translated(spec.position())
            .expect("a core in the cube");
        let rows = served
            .answer(caches, galaxy, request(&centre, 50.0, 0))
            .await;
        cored = row_of(&rows, |h| {
            matches!(h.id().kind(), SystemIdKind::DwarfCore(_))
        });
        if cored.is_some() {
            break;
        }
    }
    picked.push(at_epoch(
        "a dwarf core's member",
        cored.expect("a dwarf core's members about its centre"),
    ));
    let host = a_catalogue_host(galaxy);
    let rows = served
        .answer(caches, galaxy, request(host.epoch_position(), 50.0, 0))
        .await;
    picked.push(at_epoch(
        "a catalogue host",
        row_of(&rows, |h| h.id() == host.id()).expect("the host is among its query's rows"),
    ));
    for years in [0, 1] {
        let rows = served
            .answer(
                caches,
                galaxy,
                request(&GalacticPosition::ORIGIN, 0.05, years),
            )
            .await;
        for (what, hole) in [("the black hole", true), ("a centre member", false)] {
            let (hit, velocity) = row_of(&rows, |h| {
                let SystemIdKind::Centre(id) = h.id().kind() else {
                    return false;
                };
                (id == CentreMemberId::CENTRAL_BLACK_HOLE) == hole
                    && (hole || matches!(id.slot(), MemberSlot::InCell { .. }))
            })
            .expect("the black hole and its neighbours");
            picked.push(Picked {
                what,
                hit,
                velocity,
                t: at_years(years),
            });
        }
    }
    (picked, listing)
}

/// [`system_motion`] places each system at its row's position and with its row's velocity at the
/// row's time, a centre member's its orbit's then (research-leans-30 §3).
fn placed_as_their_rows(galaxy: &Galaxy, reads: MotionReads<'_>, picked: &[Picked]) {
    for p in picked {
        let placed = system_motion(galaxy, reads, p.hit.record(), p.t);
        let (position, moving) =
            placed.unwrap_or_else(|error| panic!("{} at {}: {error}", p.what, p.t));
        assert_eq!(
            bits(&position),
            bits(p.hit.position()),
            "{} at {}",
            p.what,
            p.t
        );
        assert_eq!(row_velocity(moving), p.velocity, "{} at {}", p.what, p.t);
    }
}

/// At ±10³ years [`system_motion`] places each system but the centre's where a served query of 50
/// ly about that place puts its row.
async fn placed_at_the_window_s_edges(
    served: &Served,
    (caches, galaxy): (&RangeCaches, &Arc<Galaxy>),
    reads: MotionReads<'_>,
    picked: &[Picked],
) {
    for p in picked.iter().filter(|p| !p.of_the_centre()) {
        for years in [-1_000, 1_000] {
            let t = at_years(years);
            let (position, moving) = system_motion(galaxy, reads, p.hit.record(), t)
                .unwrap_or_else(|error| panic!("{} at {t}: {error}", p.what));
            let rows = served
                .answer(caches, galaxy, request(&position, 50.0, years))
                .await;
            let (hit, velocity) = row_of(&rows, |h| h.id() == p.hit.id())
                .unwrap_or_else(|| panic!("{} among its query's rows at {t}", p.what));
            assert_eq!(bits(&position), bits(hit.position()), "{} at {t}", p.what);
            assert_eq!(row_velocity(moving), velocity, "{} at {t}", p.what);
        }
    }
}

/// A centre cell member's rows at ±10³ years, the clock window's edges, carry the velocity
/// [`system_motion`] gives it then, its orbit's (research-leans-30 §3), where it places it, bit for
/// bit: a member 60 ly from the black hole, beyond the reach of any orbit the centre scans in full
/// a millennium on (some 1.4 ly), found among a query's of 5 ly, so that queries of 2 ly about its
/// places then stay cheap.
async fn the_centre_s_rows_move_on_their_orbits_at_the_window_s_edges(
    served: &Served,
    (caches, galaxy): (&RangeCaches, &Arc<Galaxy>),
    reads: MotionReads<'_>,
) {
    let far = GalacticPosition::from_light_years([60.0, 0.0, 0.0]).expect("in the cube");
    let rows = served.answer(caches, galaxy, request(&far, 5.0, 0)).await;
    let (member, _) = row_of(&rows, |h| {
        matches!(h.id().kind(), SystemIdKind::Centre(id) if matches!(id.slot(), MemberSlot::InCell { .. }))
    })
    .expect("a centre cell member 60 ly from the black hole");
    for years in [-1_000, 1_000] {
        let t = at_years(years);
        let (position, moving) = system_motion(galaxy, reads, member.record(), t)
            .unwrap_or_else(|error| panic!("a centre member at {t}: {error}"));
        let rows = served
            .answer(caches, galaxy, request(&position, 2.0, years))
            .await;
        let (hit, velocity) = row_of(&rows, |h| h.id() == member.id())
            .unwrap_or_else(|| panic!("the centre member among its query's rows at {t}"));
        assert_eq!(
            bits(&position),
            bits(hit.position()),
            "a centre member at {t}"
        );
        assert_eq!(row_velocity(moving), velocity, "a centre member at {t}");
    }
}

/// At ±10⁴ years, beyond any query, [`system_motion`] places each system where the source that
/// found it places it then: the list's source, the catalogue classes' and the centre's cells.
fn placed_as_their_sources(
    galaxy: &Galaxy,
    reads: MotionReads<'_>,
    list: &GlobalListSource<'_>,
    picked: &[Picked],
) {
    let classes = CatalogueClassSource::new();
    for p in picked {
        let record = p.hit.record();
        for years in [-10_000, 10_000] {
            let t = at_years(years);
            let (position, _) = system_motion(galaxy, reads, record, t)
                .unwrap_or_else(|error| panic!("{} at {t}: {error}", p.what));
            let found = |source: &dyn SystemSource| {
                *source_hit(galaxy, source, record, &position, t)
                    .unwrap_or_else(|| panic!("{} at {t}", p.what))
                    .position()
            };
            let expected = match record.id().kind() {
                SystemIdKind::Stream(_) | SystemIdKind::DwarfCore(_) => found(list),
                SystemIdKind::Catalogue(_) => found(&classes),
                SystemIdKind::Centre(id) if id == CentreMemberId::CENTRAL_BLACK_HOLE => {
                    GalacticPosition::ORIGIN
                }
                SystemIdKind::Centre(_) => cell_member_at(galaxy, record, t),
                other => panic!("{} has a kind no source places: {other:?}", p.what),
            };
            assert_eq!(bits(&position), bits(&expected), "{} at {t}", p.what);
        }
    }
}

/// `scene_ship` frames a ship on each system, on the path [`system_motion`] reads, and observed
/// mode reads each, a list member with its brief, which the range rows' brief model gives too and
/// either list resolves.
fn framed_and_observed(
    galaxy: &Galaxy,
    (listing, reads): (&Listing, MotionReads<'_>),
    lists: (&ListedTubes, &GlobalListSource<'_>),
    picked: &[Picked],
) {
    let observer = Observer::new(
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube"),
        UniverseTime::EPOCH,
    )
    .expect("an observer");
    let systems = SharedSystemCache::new(256 << 20);
    let offset = SystemPosition::new([1.496e11, 0.0, 0.0]);
    for p in picked {
        let record = p.hit.record();
        let line = frame_line(galaxy, (listing, reads), (record.id(), p.what), offset, p.t)
            .unwrap_or_else(|error| panic!("{} framed: {}", p.what, error.message));
        assert_eq!(
            line.at(galaxy, p.t),
            system_motion(galaxy, reads, record, p.t),
            "{}",
            p.what
        );
        let (_, brief) = observe_row(galaxy, reads, &mut systems.handle(key()), &p.hit, &observer)
            .unwrap_or_else(|error| panic!("{} observed: {error}", p.what));
        if let SystemLine::Listed(member) = &*line {
            assert!(brief.is_some(), "{} shines", p.what);
            assert_eq!(
                BriefModel::of_global_member(galaxy, member).brief_at(p.t),
                member.stars(galaxy).brief_at(p.t),
                "{}'s brief",
                p.what
            );
            assert_eq!(
                lists.0.list_member(galaxy, record.id()),
                lists.1.member(galaxy, record.id()),
                "{} resolved through either list",
                p.what
            );
        }
    }
}

/// Under every 27 hold, one system of each kind a range query returns beside the grid ([`pick`]),
/// each taken from a served query's rows:
///
/// - [`system_motion`] places it at its row's position bit for bit, at the epoch, and for a
///   centre member a year on, with its row's velocity, a centre member's its orbit's then
///   (research-leans-30 §3); from a served query of 50 ly about where it places it then, at ±10³
///   years, the clock window's edges, for every kind but the centre's, whose full scan a
///   millennium on is beyond a test, and a centre cell member's 60 ly out from queries of 2 ly,
///   its velocity its orbit's;
/// - ten thousand years either way, beyond any query, it places each where the source that found
///   it places it then, bit for bit;
/// - `scene_ship` frames a ship on each, on the path `system_motion` reads, and observed mode reads
///   each, a list member with its brief, which the range rows' brief model gives too.
#[tokio::test]
#[ignore = "slow: a galaxy under every 27 hold with its full potential, its list, the tables a \
            stream's query reads, and queries of each kind's neighbourhood"]
async fn from_27_every_system_a_query_returns_is_placed_by_its_id() {
    let served = Served::new();
    let galaxy = within(served.galaxies.get(key()))
        .await
        .expect("the galaxy builds");
    assert!(GlobalListSource::is_registered_in(&galaxy));
    let caches = Served::caches(256 << 20);
    let (picked, listing) = pick(&served, &caches, &galaxy).await;
    let Listing::Listed(tubes) = &listing else {
        panic!("a stream member's listing is the list's");
    };
    let (interiors, orbits) = (
        SharedClusterModelCache::new(256 << 20),
        SharedCentreOrbitCache::new(256 << 20),
    );
    let (interiors, orbits) = (interiors.handle(key()), orbits.handle(key()));
    let reads = MotionReads::new(&interiors, &orbits).with_list(tubes);
    placed_as_their_rows(&galaxy, reads, &picked);
    placed_at_the_window_s_edges(&served, (&caches, &galaxy), reads, &picked).await;
    the_centre_s_rows_move_on_their_orbits_at_the_window_s_edges(
        &served,
        (&caches, &galaxy),
        reads,
    )
    .await;
    let list = GlobalListSource::new(&galaxy, tubes, &orbits).expect("the centre builds");
    placed_as_their_sources(&galaxy, reads, &list, &picked);
    framed_and_observed(&galaxy, (&listing, reads), (tubes, &list), &picked);
    within(served.pool.shutdown())
        .await
        .expect("the pool stops");
}
