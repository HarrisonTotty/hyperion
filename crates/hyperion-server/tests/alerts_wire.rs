//! The alerts' wire over a socket (plan 12, P12.T9.a): a subscription opens with its universe's
//! contacts, each only at its level, its observer moves, a contact is acknowledged and pushed,
//! and the requests that name no alerts subscription, or name what no sensor or contact has, are
//! refused by the field at fault.
//!
//! The contacts are recorded in the universe's Knowledge before the server opens it. The
//! subscription asks about no class, through an X-ray sensor that no local event lights, so that
//! its scans find nothing and push nothing: this test is the wire's, and `alerts_scans` is the
//! scans' (P12.T8.b).

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    AlertBandDto, AlertsAcknowledgeRequest, AlertsObserverRequest, AlertsSubscribeRequest,
    BandLimitsDto, ContactDto, DetailLevelDto, ErrorCode, GalacticPosition, NotificationBody,
    RequestBody, RequestError, ResponseBody, SceneCamerasRequest, SceneSubscribeRequest, SensorDto,
    ServerMessage, SubscribeRequest, SubscriptionState, SubscriptionTopic, UniverseIdHex,
    UniverseTime, UnsubscribeRequest,
};
use hyperion_server::knowledge::{KnowledgeLevel, PersistedKnowledge, Sighting};
use hyperion_server::universe::{UniverseId, UniverseStore};
use hyperion_sim::alerts::testing::detection;
use hyperion_sim::alerts::{AlertBand, DetectionPhase, TransientKind};
use hyperion_sim::coords::{GalacticPosition as SimPosition, GalacticVelocity};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::id::{EventBin, EventId, EventSubject, EventWord, SystemId, event_tags};
use hyperion_sim::observe::{Drift, Observer, retarded};
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};
use serde_json::Value;

fn sun() -> GalacticPosition {
    GalacticPosition {
        cell_ly: [0, 26_000, 20],
        offset_m: [0.0; 3],
    }
}

fn epoch() -> UniverseTime {
    UniverseTime {
        seconds: 0,
        nanos: 0,
    }
}

fn subscribe(universe: &UniverseIdHex) -> RequestBody {
    RequestBody::Subscribe(SubscribeRequest {
        universe: universe.clone(),
        topic: SubscriptionTopic::Alerts(AlertsSubscribeRequest {
            observer: sun(),
            time: epoch(),
            sensor: SensorDto {
                bands: vec![BandLimitsDto {
                    band: AlertBandDto::XRay,
                    detect_w_m2: 1e-12,
                    resolve_w_m2: 1e-10,
                }],
            },
            classes: Some(Vec::new()),
            host_limit: None,
        }),
    })
}

/// Event `number` of `host`, a flare's.
fn event(host: SystemId, number: u8) -> EventId {
    EventId::new(
        EventSubject::System(host),
        EventWord::new(event_tags::ALL[0], EventBin::new(0).unwrap(), number),
    )
}

/// Records two flares' sightings in `universe`'s Knowledge: contact 1 a bearing, and contact 2
/// resolved.
async fn record_contacts(store: &UniverseStore, universe: UniverseId) {
    let knowledge = PersistedKnowledge::open(store, universe).await.unwrap();
    let host: SystemId = "0200080020000000".parse().unwrap();
    let site = SimPosition::from_light_years([0.0, 26_010.0, 20.0]).unwrap();
    let here = SimPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
    let observer = Observer::new(here, hyperion_sim::time::UniverseTime::EPOCH).unwrap();
    let light = retarded(&observer, &Drift::new(site, GalacticVelocity::default()));
    for (number, level) in [(1, KnowledgeLevel::Bearing), (2, KnowledgeLevel::Resolved)] {
        let seen = detection(
            event(host, number),
            host,
            None,
            TransientKind::Flare,
            DetectionPhase::Onset,
            AlertBand::Photometric(Band::V),
            WattsPerSquareMetre::new(1e-11),
            level == KnowledgeLevel::Resolved,
            site,
            hyperion_sim::time::UniverseTime::EPOCH,
            &here,
        );
        let sighting = Sighting::new(
            &observer,
            &light,
            &seen,
            WattsPerSquareMetre::new(2e-11),
            level,
            LightYears::new(0.01),
        );
        knowledge
            .record_sighting(event(host, number), sighting)
            .await
            .unwrap();
    }
}

/// The `bad_request` field of a refusal.
fn refused_at(answer: Result<ResponseBody, RequestError>) -> Option<String> {
    match answer {
        Err(RequestError {
            code: ErrorCode::BadRequest,
            field,
            ..
        }) => field,
        other => panic!("expected `bad_request`, got {other:?}"),
    }
}

/// A subscription opens with the universe's contacts, a bearing with no host key in its JSON and a
/// resolved contact with its host; moving the observer is answered; acknowledging a contact is
/// answered and pushes it; and every refusal names its field.
#[tokio::test]
async fn alerts_wire_opens_with_contacts_moves_acknowledges_and_refuses() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Alerts: wire", 12).await.id;
    let store = UniverseStore::new(server.data_dir());
    record_contacts(&store, UniverseId::from(&universe)).await;
    let subscription = opens_with_its_contacts(&mut client, &universe).await;
    // The observer moves.
    let moved = AlertsObserverRequest {
        subscription,
        observer: GalacticPosition {
            cell_ly: [10, 26_000, 20],
            offset_m: [0.0; 3],
        },
        time: UniverseTime {
            seconds: 86_400,
            nanos: 0,
        },
    };
    assert_eq!(
        client
            .request(RequestBody::AlertsObserver(moved.clone()))
            .await,
        Ok(ResponseBody::AlertsObserver)
    );

    acknowledges_and_pushes(&mut client, subscription).await;
    refuses_by_the_field_at_fault(&mut client, &universe, moved).await;
    client.close().await;
    server.stop().await;
}

/// Subscribes in `universe` and checks its state: two contacts, a bearing with no host key in its
/// JSON and a resolved one with its host, and no census. Returns the subscription.
async fn opens_with_its_contacts(client: &mut TestClient, universe: &UniverseIdHex) -> u32 {
    // The subscription's answer, read as JSON: a bearing names no host.
    let id = client.send_request(subscribe(universe)).await;
    let text = client.next_text().await;
    let json: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["id"], id.0);
    let contacts = json["body"]["state"]["contacts"].as_array().unwrap();
    assert_eq!(contacts.len(), 2, "{json}");
    assert_eq!(contacts[0]["level"], "bearing");
    assert_eq!(contacts[0]["kind"], "unclassified");
    for key in ["host", "designation", "light_age_yr", "apparent_position"] {
        assert!(
            contacts[0].get(key).is_none(),
            "a bearing holds `{key}`: {json}"
        );
    }
    assert_eq!(contacts[1]["level"], "resolved");
    assert_eq!(contacts[1]["kind"], "flare");
    assert_eq!(contacts[1]["host"], "0200080020000000");
    let ServerMessage::Response {
        body: ResponseBody::Subscribe(subscribed),
        ..
    } = serde_json::from_str(&text).unwrap()
    else {
        panic!("expected `subscribed`: {text}");
    };
    let SubscriptionState::Alerts(state) = subscribed.state else {
        panic!("an alerts subscription opens with the alerts' state");
    };
    assert_eq!(state.census, []);
    subscribed.subscription
}

/// Acknowledging contact 1 on `subscription` pushes it acknowledged; again, it is answered and
/// nothing changes.
async fn acknowledges_and_pushes(client: &mut TestClient, subscription: u32) {
    let acknowledge = |contact| {
        RequestBody::AlertsAcknowledge(AlertsAcknowledgeRequest {
            subscription,
            contact,
        })
    };
    let (answer, mut pushed) = client.request_among_notifications(acknowledge(1)).await;
    assert_eq!(answer, Ok(ResponseBody::AlertsAcknowledge));
    if pushed.is_empty() {
        pushed.push(client.next_notification().await);
    }
    let [(on, NotificationBody::Alerts(notification))] = &pushed[..] else {
        panic!("one alerts notification: {pushed:?}");
    };
    assert_eq!(*on, subscription);
    assert_eq!(notification.sequence, 1);
    let [ContactDto::Bearing(seen)] = &notification.contacts[..] else {
        panic!("contact 1, a bearing: {notification:?}");
    };
    assert_eq!((seen.contact, seen.acknowledged), (1, true));
    assert_eq!(
        client.request(acknowledge(1)).await,
        Ok(ResponseBody::AlertsAcknowledge)
    );
}

/// Each refusal names its field: unknown contacts, an observer outside the cube or the clock,
/// cameras on an alerts subscription, an observer on a scene subscription, an unknown
/// subscription, a sensor no sensor has, and a subscription once ended.
async fn refuses_by_the_field_at_fault(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    moved: AlertsObserverRequest,
) {
    let subscription = moved.subscription;
    let acknowledge = |contact| {
        RequestBody::AlertsAcknowledge(AlertsAcknowledgeRequest {
            subscription,
            contact,
        })
    };
    assert_eq!(
        refused_at(client.request(acknowledge(9)).await).as_deref(),
        Some("contact")
    );
    assert_eq!(
        refused_at(client.request(acknowledge(0)).await).as_deref(),
        Some("contact")
    );
    let outside = AlertsObserverRequest {
        observer: GalacticPosition {
            cell_ly: [90_000, 0, 0],
            offset_m: [0.0; 3],
        },
        ..moved.clone()
    };
    assert_eq!(
        refused_at(client.request(RequestBody::AlertsObserver(outside)).await).as_deref(),
        Some("observer")
    );
    let late = AlertsObserverRequest {
        time: UniverseTime {
            seconds: 40_000_000_000,
            nanos: 0,
        },
        ..moved.clone()
    };
    assert_eq!(
        refused_at(client.request(RequestBody::AlertsObserver(late)).await).as_deref(),
        Some("time")
    );
    let cameras = RequestBody::SceneCameras(SceneCamerasRequest {
        subscription,
        cameras: Vec::new(),
    });
    assert_eq!(
        refused_at(client.request(cameras).await).as_deref(),
        Some("subscription"),
        "an alerts subscription takes no cameras"
    );
    let unknown = AlertsObserverRequest {
        subscription: 77,
        ..moved.clone()
    };
    assert_eq!(
        refused_at(client.request(RequestBody::AlertsObserver(unknown)).await).as_deref(),
        Some("subscription")
    );
    let mut faint = subscribe(universe);
    if let RequestBody::Subscribe(SubscribeRequest {
        topic: SubscriptionTopic::Alerts(alerts),
        ..
    }) = &mut faint
    {
        alerts.sensor.bands[0].resolve_w_m2 = 1e-14;
    }
    assert_eq!(
        refused_at(client.request(faint).await).as_deref(),
        Some("sensor")
    );

    // A scene subscription takes no observer.
    let scene = RequestBody::Subscribe(SubscribeRequest {
        universe: universe.clone(),
        topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
            detail: DetailLevelDto::Bulk,
            cameras: Vec::new(),
        }),
    });
    let (opened, _) = client.request_among_notifications(scene).await;
    let Ok(ResponseBody::Subscribe(scene)) = opened else {
        panic!("a scene subscription: {opened:?}");
    };
    let on_the_scene = AlertsObserverRequest {
        subscription: scene.subscription,
        ..moved.clone()
    };
    let (answer, _) = client
        .request_among_notifications(RequestBody::AlertsObserver(on_the_scene))
        .await;
    assert_eq!(refused_at(answer).as_deref(), Some("subscription"));

    // Once unsubscribed, the subscription is unknown.
    assert_eq!(
        client
            .request(RequestBody::Unsubscribe(UnsubscribeRequest {
                subscription
            }))
            .await,
        Ok(ResponseBody::Unsubscribe)
    );
    assert_eq!(
        refused_at(client.request(RequestBody::AlertsObserver(moved)).await).as_deref(),
        Some("subscription")
    );
}
