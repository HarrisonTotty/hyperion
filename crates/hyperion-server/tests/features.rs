//! Plan 09's feature requests over a real socket (P09.T40.b): `features_in_range`,
//! `galaxy_features`, `feature_detail` and `black_hole_state`, each answered from the sim's own
//! models at the request's time, an unknown feature refused, a radius that fits nothing answered
//! with nothing, and every field a request cannot use refused naming it. The features exist at the
//! version in force, so the four are served whatever P09.T40.a's gate holds back.

mod common;

use common::origins::{SEED, at_years, galaxy};
use common::{TestClient, TestServer, pretty_json_frame};
use hyperion_protocol::{
    BlackHoleStateRequest, ErrorCode, FeatureDetailRequest, FeatureIdHex, FeatureKindStatus,
    FeatureKindWire, FeatureStructure, FeatureSummary, FeaturesInRange, FeaturesInRangeRequest,
    GalacticPosition, GalaxyFeaturesRequest, MapFeatureKindWire, MapFeatureList, MapFeatures,
    MapShells, RequestBody, RequestError, ResponseBody, UniverseIdHex, UniverseTime,
};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::galaxy::features::FeatureId;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks, NoFeatureCache};
use hyperion_sim::galaxy::features::cluster::ClusterModel;
use hyperion_sim::id::FeatureCell;
use hyperion_sim::units::LightYears;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The wire's word for the galactic centre as a feature.
const CENTRE: u64 = 0xF000_0000_0000_0000;

/// Every kind a range can ask for, in the wire's order.
const EVERY_KIND: [FeatureKindWire; 8] = [
    FeatureKindWire::Globular,
    FeatureKindWire::OpenCluster,
    FeatureKindWire::Association,
    FeatureKindWire::StarFormingRegion,
    FeatureKindWire::MolecularCloud,
    FeatureKindWire::GalacticCentre,
    FeatureKindWire::Stream,
    FeatureKindWire::DwarfCore,
];

/// The centre of a light-year cell, as the wire carries a position.
fn at_cell(cell_ly: [i32; 3]) -> GalacticPosition {
    GalacticPosition {
        cell_ly,
        offset_m: [0.0; 3],
    }
}

/// A sim position as the wire carries it.
fn wire(position: &hyperion_sim::coords::GalacticPosition) -> GalacticPosition {
    GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

/// The distance between two wire positions, in light-years.
fn apart_ly(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    (0..3)
        .map(|i| {
            let d = f64::from(a.cell_ly[i] - b.cell_ly[i])
                + (a.offset_m[i] - b.offset_m[i]) / METRES_PER_LIGHT_YEAR;
            d * d
        })
        .sum::<f64>()
        .sqrt()
}

/// A server, a client that has said hello, and a universe of [`SEED`].
async fn a_universe() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Feature Reach", SEED).await.id;
    (server, client, universe)
}

/// `features_in_range` of `kinds` within `radius_ly` of `centre` at `time`.
fn in_range(
    universe: &UniverseIdHex,
    centre: GalacticPosition,
    radius_ly: f64,
    time: UniverseTime,
    kinds: &[FeatureKindWire],
) -> RequestBody {
    RequestBody::FeaturesInRange(FeaturesInRangeRequest {
        universe: universe.clone(),
        centre,
        radius_ly,
        time,
        kinds: kinds.to_vec(),
    })
}

async fn features(client: &mut TestClient, body: RequestBody) -> FeaturesInRange {
    match client.request(body).await {
        Ok(ResponseBody::FeaturesInRange(answer)) => answer,
        other => panic!("expected features in range, got {other:?}"),
    }
}

async fn refused(client: &mut TestClient, body: RequestBody) -> RequestError {
    client
        .request(body)
        .await
        .expect_err("the request is refused")
}

/// The cluster with members nearest the Sun-like point, as the sim's catalogue has it.
fn a_cluster() -> hyperion_sim::galaxy::features::catalogue::FeatureRecord {
    let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("in the root cube");
    let mut near: Vec<_> =
        FeatureCatalogue::near(galaxy(), &sun, LightYears::new(3_000.0), &NoFeatureCache)
            .filter(|feature| ClusterModel::from_record(galaxy(), feature).is_some())
            .collect();
    near.sort_by(|a, b| {
        let away = |f: &hyperion_sim::galaxy::features::catalogue::FeatureRecord| {
            f.position().distance_to(&sun).value()
        };
        away(a).total_cmp(&away(b))
    });
    near.into_iter()
        .next()
        .expect("a cluster lies within 3,000 ly of the Sun")
}

/// `answer`, a range of every kind within `radius_ly` of `centre`, has a census line per kind in the
/// request's order, streams and dwarf cores not generated, each line counting its kind's features;
/// and every feature meets the sphere, is finite, and comes nearest first.
fn assert_a_whole_range(answer: &FeaturesInRange, centre: &GalacticPosition, radius_ly: f64) {
    let kinds: Vec<_> = answer.census.iter().map(|line| line.kind).collect();
    assert_eq!(kinds, EVERY_KIND);
    for line in &answer.census {
        let expected = match line.kind {
            FeatureKindWire::Stream | FeatureKindWire::DwarfCore => FeatureKindStatus::NotGenerated,
            _ => FeatureKindStatus::Included,
        };
        assert_eq!(line.status, expected, "{:?}", line.kind);
        let returned = answer
            .features
            .iter()
            .filter(|feature| feature.kind == line.kind)
            .count();
        assert_eq!(line.returned as usize, returned, "{:?}", line.kind);
    }
    let distances: Vec<f64> = answer
        .features
        .iter()
        .map(|feature| apart_ly(&feature.position, centre))
        .collect();
    for (feature, away) in answer.features.iter().zip(&distances) {
        assert!(
            *away <= radius_ly + feature.extent_ly,
            "{} lies {away} ly off, extent {}",
            feature.designation,
            feature.extent_ly
        );
        assert!(feature.extent_ly > 0.0 && feature.extent_ly.is_finite());
        assert!(feature.mass_msun > 0.0 && feature.mass_msun.is_finite());
        assert!(feature.expected_members >= 0.0);
        assert_eq!(
            feature.age_myr.is_none(),
            feature.kind == FeatureKindWire::MolecularCloud
        );
    }
    assert!(distances.is_sorted_by(|a, b| a <= b), "nearest first");
}

/// A range of features about the Sun-like point at the epoch and a thousand years on is whole
/// ([`assert_a_whole_range`]), and a cluster near the Sun is among it as the sim describes it,
/// moved by its bulk motion.
#[tokio::test]
async fn features_in_range_are_the_catalogue_s_at_the_time() {
    let reference = std::thread::spawn(a_cluster);
    let (server, mut client, universe) = a_universe().await;
    let cluster = reference.join().expect("the cluster is found");
    let sun = at_cell([0, 26_000, 0]);
    for years in [0, 1_000] {
        let answer = features(
            &mut client,
            in_range(&universe, sun, 1_000.0, at_years(years), &EVERY_KIND),
        )
        .await;
        assert!(
            answer.features.len() > 5,
            "{} features",
            answer.features.len()
        );
        assert_a_whole_range(&answer, &sun, 1_000.0);
        let found = answer
            .features
            .iter()
            .find(|feature| feature.id == FeatureIdHex::from_u64(cluster.id().object_word()))
            .expect("the cluster nearest the Sun is within 1,000 ly");
        assert_eq!(found.designation, cluster.id().designation().to_string());
        if years == 0 {
            assert_eq!(
                found.position,
                wire(cluster.position()),
                "at the epoch, unmoved"
            );
        } else {
            let moved = apart_ly(&found.position, &wire(cluster.position()));
            assert!(
                moved > 0.0 && moved < 2.0,
                "moved {moved} ly in 1,000 years"
            );
        }
        assert!(found.expected_members > 0.0, "a cluster has members");
    }
    client.close().await;
    server.stop().await;
}

/// A radius that fits nothing is answered with nothing; a sphere whose lookup would read past the
/// cell budget leaves the catalogue's kinds out, the centre still answered; and each field a
/// request cannot use is refused naming it, in design note 24's order.
#[tokio::test]
async fn features_in_range_answer_nothing_or_refuse_what_they_cannot_use() {
    let (server, mut client, universe) = a_universe().await;
    let sun = at_cell([0, 26_000, 0]);
    // Far out in the halo, a light-year holds nothing: every included kind returns none.
    let halo = at_cell([60_000, 60_000, 60_000]);
    let nothing = features(
        &mut client,
        in_range(&universe, halo, 1.0, at_years(0), &EVERY_KIND),
    )
    .await;
    assert!(nothing.features.is_empty(), "{:?}", nothing.features);
    assert_a_whole_range(&nothing, &halo, 1.0);
    assert!(nothing.census.iter().all(|line| line.returned == 0));
    let wide = features(
        &mut client,
        in_range(
            &universe,
            sun,
            40_000.0,
            at_years(0),
            &[FeatureKindWire::Globular, FeatureKindWire::GalacticCentre],
        ),
    )
    .await;
    assert_eq!(wide.census[0].status, FeatureKindStatus::OverCellBudget);
    assert_eq!(wide.census[0].returned, 0);
    assert_eq!(wide.census[1].status, FeatureKindStatus::Included);
    assert_eq!(wide.features.len(), 1, "the centre alone");
    assert_eq!(wide.features[0].id, FeatureIdHex::from_u64(CENTRE));
    let twice = [FeatureKindWire::Globular, FeatureKindWire::Globular];
    let far = at_cell([70_000, 0, 0]);
    for (body, field) in [
        (
            in_range(&universe, sun, 10.0, at_years(1_001), &EVERY_KIND),
            "time",
        ),
        (
            in_range(&universe, far, 10.0, at_years(0), &EVERY_KIND),
            "centre",
        ),
        (
            in_range(&universe, sun, 0.0, at_years(0), &EVERY_KIND),
            "radius_ly",
        ),
        (in_range(&universe, sun, 10.0, at_years(0), &[]), "kinds"),
        (in_range(&universe, sun, 10.0, at_years(0), &twice), "kinds"),
    ] {
        let error = refused(&mut client, body).await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some(field))
        );
    }
    let unknown = UniverseIdHex::from_u64(42);
    let error = refused(
        &mut client,
        in_range(&unknown, sun, 10.0, at_years(0), &EVERY_KIND),
    )
    .await;
    assert_eq!(error.code, ErrorCode::UnknownUniverse);
    client.close().await;
    server.stop().await;
}

/// `feature_detail` of `feature` at `time`.
fn detail(universe: &UniverseIdHex, feature: u64, time: UniverseTime) -> RequestBody {
    RequestBody::FeatureDetail(FeatureDetailRequest {
        universe: universe.clone(),
        feature: FeatureIdHex::from_u64(feature),
        time,
    })
}

/// A feature's detail: a cluster's structure and classes, with the summary a range lists it with;
/// the galactic centre's; a cloud's Plummer profile; and an unknown feature refused naming
/// `feature`, as is a word that is no feature's.
#[tokio::test]
async fn a_feature_s_detail_is_its_structure_at_the_time() {
    let reference = std::thread::spawn(a_cluster);
    let (server, mut client, universe) = a_universe().await;
    let cluster = reference.join().expect("the cluster is found");
    let word = cluster.id().object_word();
    let Ok(ResponseBody::FeatureDetail(answer)) =
        client.request(detail(&universe, word, at_years(100))).await
    else {
        panic!("expected the cluster's detail");
    };
    assert_eq!(answer.universe, universe);
    assert_eq!(answer.time, at_years(100));
    let FeatureStructure::Cluster(structure) = &answer.structure else {
        panic!("a cluster's structure: {:?}", answer.structure);
    };
    assert!(structure.core_radius_ly < structure.half_mass_radius_ly);
    assert!(structure.half_mass_radius_ly < structure.tidal_radius_ly);
    assert!(structure.central_escape_speed_km_s > 0.0 && structure.relaxation_time_myr > 0.0);
    assert!(!structure.classes.is_empty(), "a cluster has classes");
    let classes: f64 = structure.classes.iter().map(|class| class.expected).sum();
    assert!(
        (classes - answer.feature.expected_members).abs() <= 1e-9 * classes,
        "{classes} and {}",
        answer.feature.expected_members
    );
    assert!((answer.feature.extent_ly - structure.tidal_radius_ly).abs() < 1e-12);
    // The same summary a range lists it with at the same time.
    let listed = features(
        &mut client,
        in_range(
            &universe,
            answer.feature.position,
            1.0,
            at_years(100),
            &EVERY_KIND,
        ),
    )
    .await;
    let summary: &FeatureSummary = listed
        .features
        .iter()
        .find(|feature| feature.id == answer.feature.id)
        .expect("the cluster is in range of its own centre");
    assert_eq!(*summary, answer.feature);

    // The galactic centre.
    let Ok(ResponseBody::FeatureDetail(centre)) =
        client.request(detail(&universe, CENTRE, at_years(0))).await
    else {
        panic!("expected the centre's detail");
    };
    assert_eq!(centre.feature.kind, FeatureKindWire::GalacticCentre);
    assert_eq!(centre.feature.designation, "CENTRE");
    let FeatureStructure::Centre(structure) = &centre.structure else {
        panic!("the centre's structure: {:?}", centre.structure);
    };
    assert!(structure.loss_cone_radius_ly < structure.influence_radius_ly);
    assert!(!structure.classes.is_empty(), "the centre has classes");

    // A cloud's Plummer profile.
    let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("in the root cube");
    let cloud = FeatureCatalogue::near(galaxy(), &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find(|feature| matches!(feature.marks(), FeatureMarks::Cloud(_)))
        .expect("a cloud lies within 3,000 ly of the Sun");
    let Ok(ResponseBody::FeatureDetail(answer)) = client
        .request(detail(&universe, cloud.id().object_word(), at_years(0)))
        .await
    else {
        panic!("expected the cloud's detail");
    };
    assert_eq!(answer.feature.kind, FeatureKindWire::MolecularCloud);
    assert!(matches!(answer.structure, FeatureStructure::Cloud(_)));

    // A feature's word that names no feature: the cell's last index, past its candidates.
    let none = FeatureId::of(FeatureCell::new([0, 6, 0]).expect("a cell"), 16_383).expect("an ID");
    let error = refused(
        &mut client,
        detail(&universe, none.object_word(), at_years(0)),
    )
    .await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::UnknownFeature, Some("feature")),
        "{error:?}"
    );
    // A word that is no feature's at all, a member's with its slot set, is a bad request.
    let error = refused(&mut client, detail(&universe, word | 1, at_years(0))).await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("feature")),
        "{error:?}"
    );
    let error = refused(&mut client, detail(&universe, word, at_years(-1_001))).await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("time"))
    );
    client.close().await;
    server.stop().await;
}

/// The galaxy map's features: every globular that is one at the time, the centre, and the bright
/// shells withheld, in the request's order; asked again, the globulars are the kept list's.
#[tokio::test]
async fn the_galaxy_map_draws_every_globular_and_the_centre() {
    let (server, mut client, universe) = a_universe().await;
    let request = |kinds: Vec<MapFeatureKindWire>, time| {
        RequestBody::GalaxyFeatures(GalaxyFeaturesRequest {
            universe: universe.clone(),
            kinds,
            time,
        })
    };
    let kinds = vec![
        MapFeatureKindWire::BrightShell,
        MapFeatureKindWire::Globular,
        MapFeatureKindWire::GalacticCentre,
    ];
    let Ok(ResponseBody::GalaxyFeatures(map)) =
        client.request(request(kinds.clone(), at_years(0))).await
    else {
        panic!("expected the map's features");
    };
    assert_eq!(map.lists.len(), 3);
    assert!(matches!(
        map.lists[0],
        MapFeatureList::BrightShell(MapShells::NotGenerated)
    ));
    let MapFeatureList::Globular(MapFeatures::Complete {
        features: globulars,
    }) = &map.lists[1]
    else {
        panic!("the globulars: {:?}", map.lists[1]);
    };
    assert!(
        (100..=200).contains(&globulars.len()),
        "{} globulars",
        globulars.len()
    );
    assert!(
        globulars
            .iter()
            .all(|g| g.kind == FeatureKindWire::Globular)
    );
    assert!(globulars.iter().all(|g| g.expected_members > 0.0));
    let MapFeatureList::GalacticCentre(MapFeatures::Complete { features: centre }) = &map.lists[2]
    else {
        panic!("the centre: {:?}", map.lists[2]);
    };
    assert_eq!(centre.len(), 1);
    assert_eq!(centre[0].id, FeatureIdHex::from_u64(CENTRE));
    // Again at another time: the list kept, each globular where it is then.
    let Ok(ResponseBody::GalaxyFeatures(later)) = client
        .request(request(vec![MapFeatureKindWire::Globular], at_years(1_000)))
        .await
    else {
        panic!("expected the map's features");
    };
    let MapFeatureList::Globular(MapFeatures::Complete { features: moved }) = &later.lists[0]
    else {
        panic!("the globulars");
    };
    assert_eq!(moved.len(), globulars.len());
    assert_ne!(moved[0].position, globulars[0].position, "globulars move");
    // And at the first time again, from the kept list: the first answer, whole.
    let Ok(ResponseBody::GalaxyFeatures(again)) = client.request(request(kinds, at_years(0))).await
    else {
        panic!("expected the map's features");
    };
    assert_eq!(again, map, "the kept list answers as the first");
    let counters = server.stats().galaxy_features();
    assert_eq!(
        (counters.entries(), counters.hits()),
        (1, 2),
        "{counters:?}"
    );
    for (kinds, field) in [
        (vec![], "kinds"),
        (
            vec![MapFeatureKindWire::Globular, MapFeatureKindWire::Globular],
            "kinds",
        ),
    ] {
        let error = refused(&mut client, request(kinds, at_years(0))).await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some(field))
        );
    }
    client.close().await;
    server.stop().await;
}

/// The black hole at the epoch and at the clock window's edges: its mass, its luminosity by part,
/// and its disruptions up to the time, newest first; never a failure at any time of the window.
#[tokio::test]
async fn the_black_hole_s_state_is_answered_at_every_time() {
    let (server, mut client, universe) = a_universe().await;
    let mut seen = Vec::new();
    for years in [-1_000, -1, 0, 1, 1_000] {
        let Ok(ResponseBody::BlackHoleState(state)) = client
            .request(RequestBody::BlackHoleState(BlackHoleStateRequest {
                universe: universe.clone(),
                time: at_years(years),
            }))
            .await
        else {
            panic!("expected the black hole's state at {years} years");
        };
        assert_eq!(state.id.as_str(), "f000000700000000");
        // The seed's own galaxy's black hole, as the sim solves it.
        assert_eq!(
            state.mass_msun.to_bits(),
            galaxy().params().black_hole().mass().value().to_bits(),
            "{} M☉",
            state.mass_msun
        );
        let luminosity = state.luminosity.expect("the centre's events build");
        for part in [
            luminosity.quiescent_w,
            luminosity.flares_w,
            luminosity.tails_w,
        ] {
            assert!(part >= 0.0 && part.is_finite(), "{luminosity:?}");
            assert!(part <= luminosity.total_w * (1.0 + 1e-12), "{luminosity:?}");
        }
        let times: Vec<_> = state
            .disruptions
            .iter()
            .map(|d| (d.time.seconds, d.time.nanos))
            .collect();
        assert!(times.is_sorted_by(|a, b| a >= b), "newest first: {times:?}");
        assert!(
            times
                .iter()
                .all(|&(seconds, _)| seconds <= at_years(years).seconds)
        );
        seen.push(state.disruptions.len());
    }
    assert!(seen.is_sorted(), "disruptions only accumulate: {seen:?}");
    assert!(
        seen[2] > 0,
        "the black hole has torn stars apart before the epoch"
    );
    let error = refused(
        &mut client,
        RequestBody::BlackHoleState(BlackHoleStateRequest {
            universe: universe.clone(),
            time: at_years(1_001),
        }),
    )
    .await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("time"))
    );
    client.close().await;
    server.stop().await;
}

/// The frame of the answer to `body`, as the client received it.
async fn frame_of(client: &mut TestClient, body: RequestBody) -> String {
    let _ = client.send_request(body).await;
    client.next_text().await
}

/// The four kinds' answers, pinned whole as the client receives their frames, under the golden
/// header that ties them to the generator version: a range about the Sun-like point at the epoch
/// and a thousand years on, the details of the cluster nearest the Sun, of a cloud and of the
/// centre, the black hole at the epoch, and the map at the epoch. Each float is the wire's
/// shortest round-trip form, so a changed bit is a changed line.
#[tokio::test]
async fn the_feature_answers_are_the_golden_frames() {
    let reference = std::thread::spawn(a_cluster);
    let (server, mut client, universe) = a_universe().await;
    let cluster = reference.join().expect("the cluster is found");
    let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("in the root cube");
    let cloud = FeatureCatalogue::near(galaxy(), &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find(|feature| matches!(feature.marks(), FeatureMarks::Cloud(_)))
        .expect("a cloud lies within 3,000 ly of the Sun");
    let near_the_sun = |years| {
        in_range(
            &universe,
            at_cell([0, 26_000, 0]),
            300.0,
            at_years(years),
            &EVERY_KIND,
        )
    };
    let requests = [
        ("features in range, epoch", near_the_sun(0)),
        ("features in range, +1,000 yr", near_the_sun(1_000)),
        (
            "a cluster's detail, +100 yr",
            detail(&universe, cluster.id().object_word(), at_years(100)),
        ),
        (
            "a cloud's detail",
            detail(&universe, cloud.id().object_word(), at_years(0)),
        ),
        (
            "the centre's detail",
            detail(&universe, CENTRE, at_years(0)),
        ),
        (
            "the black hole",
            RequestBody::BlackHoleState(BlackHoleStateRequest {
                universe: universe.clone(),
                time: at_years(0),
            }),
        ),
        (
            "the galaxy map",
            RequestBody::GalaxyFeatures(GalaxyFeaturesRequest {
                universe: universe.clone(),
                kinds: vec![
                    MapFeatureKindWire::Globular,
                    MapFeatureKindWire::GalacticCentre,
                    MapFeatureKindWire::BrightShell,
                ],
                time: at_years(0),
            }),
        ),
    ];
    let mut golden = GoldenWriter::new();
    golden.header(GENERATOR_VERSION.get());
    for (what, body) in requests {
        golden.line(&format!("# {what}"));
        let frame = frame_of(&mut client, body).await;
        for line in pretty_json_frame(&frame).lines() {
            golden.line(line);
        }
    }
    golden!("features", &golden.finish());
    client.close().await;
    server.stop().await;
}
