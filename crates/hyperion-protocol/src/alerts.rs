//! The alerts' wire types (plan 12, P12.T9.a): the `alerts` topic of `subscribe`, the requests
//! that name an alerts subscription, and the contacts and census its notifications carry.
//!
//! A subscription is a sensor at an observer the client moves with `alerts_observer` (Design note
//! 9). It is answered with [`AlertsSubscribed`], the census and every contact the universe's
//! Knowledge holds, and each [`AlertsNotification`] carries the contacts that changed, the census
//! and a `sequence` that grows by exactly one per notification sent, as the scene's does. A contact
//! is the crew's view of one transient event (Design note 10): a [`BearingContactDto`] names no
//! host, in its type, and only a [`ResolvedContactDto`] carries the host and where it is. The
//! contact's number is opaque and sequential per universe, so it cannot leak the host either.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::features::CatalogueClassWire;
use crate::primitives::{GalacticPosition, SystemIdHex, UniverseTime};

/// A band a sensor's limits and a contact's flux are in (plan 12's `AlertBand`): plan 07's
/// photometric bands, from U to radio, and X-rays, 2–10 keV.
///
/// A band's flux is its in-band power at the observer, W m⁻², after extinction: through a top hat
/// of the band's full width at half maximum about its centre (Bessell's widths for U to I,
/// Tokunaga's for J to the mid infrared), and between 2 and 10 keV in X-rays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AlertBandDto {
    /// Johnson U, 0.36 µm.
    U,
    /// Johnson B, 0.44 µm.
    B,
    /// Johnson V, 0.55 µm.
    V,
    /// Cousins R, 0.64 µm.
    R,
    /// Cousins I, 0.79 µm.
    I,
    /// J, 1.25 µm.
    J,
    /// H, 1.65 µm.
    H,
    /// K, 2.2 µm.
    K,
    /// The mid infrared's N band, 10 µm.
    MidInfrared,
    /// Radio, where a sensor times pulsars and sees no thermal light.
    Radio,
    /// X-rays, 2–10 keV.
    XRay,
}

/// A sensor's two flux limits in one band (plan 12's `BandLimits`), W m⁻²: the faintest it
/// detects and the faintest at which it resolves the host, positive, finite, and the second no
/// fainter than the first.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BandLimitsDto {
    /// The band.
    pub band: AlertBandDto,
    /// The detection limit, W m⁻².
    pub detect_w_m2: f64,
    /// The resolve limit, W m⁻², at or above `detect_w_m2`.
    pub resolve_w_m2: f64,
}

/// What a sensor can see: its limits in each band it has, at least one band and none twice (plan
/// 12's `SensorHorizon`). A sensor here is only these limits: apertures and integration belong to
/// the sensor consoles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SensorDto {
    /// The limits, a band's at most once.
    pub bands: Vec<BandLimitsDto>,
}

/// `subscribe`'s `alerts` topic: a sensor at an observer, and the catalogue classes it asks
/// about.
///
/// The observer must lie in the root cube and its time in the clock window, or the subscription
/// is refused with `bad_request` naming `observer` or `time`; a sensor with no band, a band twice,
/// or limits that are not positive and finite, or a resolve limit fainter than its detection
/// limit, with `bad_request` naming `sensor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertsSubscribeRequest {
    /// Where the observer is, at rest in the galactic frame.
    pub observer: GalacticPosition,
    /// The observer's time.
    pub time: UniverseTime,
    /// The sensor's limits.
    pub sensor: SensorDto,
    /// The classes to walk; every class when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub classes: Option<Vec<CatalogueClassWire>>,
    /// The most hosts a class may be expected to have in its reach for it to be walked, at least 1;
    /// the server's default when absent (Design note 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub host_limit: Option<u32>,
}

/// The `alerts` topic's whole state when it opens: the census, and every contact the universe's
/// Knowledge holds, in contact order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertsSubscribed {
    /// Each class's census, in class order: empty until the server scans.
    pub census: Vec<ClassCensusDto>,
    /// The contacts known in the universe.
    pub contacts: Vec<ContactDto>,
}

/// An alerts notification: what changed since the last one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertsNotification {
    /// The notification's number on its subscription: 1 for the first, one more for each after,
    /// so that a gap shows a notification lost.
    #[ts(type = "number")]
    pub sequence: u64,
    /// The contacts that changed, each as it now stands, in contact order.
    pub contacts: Vec<ContactDto>,
    /// Each class's census, in class order: empty while it has not changed.
    pub census: Vec<ClassCensusDto>,
}

/// How a class's census stands (Design note 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ClassCensusStatusDto {
    /// Every host in the class's reach was walked: its contacts are complete.
    Complete,
    /// The class expects more hosts in its reach than the subscription's host limit: it yields
    /// nothing.
    OverLimit,
    /// Its scan has not finished: it yields nothing yet.
    Pending,
}

/// One class's census.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClassCensusDto {
    /// The class.
    pub class: CatalogueClassWire,
    /// How its census stands.
    pub status: ClassCensusStatusDto,
    /// How far the sensor sees its brightest event, ly, with no extinction (Design note 5).
    pub reach_ly: f64,
    /// The hosts it is expected to have in its reach.
    pub expected_hosts: f64,
}

/// What kind of event a contact is (plan 12's `TransientKind`), or that its light does not say.
///
/// A bearing is `unclassified` unless its band alone names its kind: a glitch, the one kind timed
/// in the radio. A resolved contact names its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TransientKindDto {
    /// The light does not yet say.
    Unclassified,
    /// A core-collapse supernova.
    CoreCollapse,
    /// A Type Ia supernova.
    TypeIa,
    /// A luminous red nova, a stellar merger.
    LuminousRedNova,
    /// A kilonova, a neutron-star merger.
    Kilonova,
    /// A classical or recurrent nova.
    Nova,
    /// A dwarf nova.
    DwarfNova,
    /// A transient X-ray binary's outburst.
    XrayTransient,
    /// A thermonuclear burst on an accreting neutron star.
    XrayBurst,
    /// A Be/X-ray binary's outburst.
    BeXOutburst,
    /// A luminous blue variable's giant eruption.
    GiantEruption,
    /// The central black hole's disruption of a star.
    TidalDisruption,
    /// One of the central black hole's X-ray flares.
    CentralFlare,
    /// A star's flare.
    Flare,
    /// A pulsar glitch.
    Glitch,
    /// A magnetar's burst or giant flare.
    MagnetarBurst,
    /// An FU Orionis outburst.
    FuOrionisOutburst,
}

/// Where a contact's event stood when its light was first counted (plan 12's `DetectionPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DetectionPhaseDto {
    /// Its onset arrived while it was watched.
    Onset,
    /// It was already under way when its light was first counted.
    InProgress,
}

/// The frame a bearing's azimuth is measured in (Design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BearingFrameDto {
    /// From coreward through spinward at the observer.
    Local,
    /// From the galactic +x axis, within a light-year of the axis where coreward is undefined.
    GalacticX,
}

/// The direction a contact's light comes from at the observer (Design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BearingDto {
    /// Azimuth, degrees in [0, 360), in `frame`.
    pub azimuth_deg: f64,
    /// Elevation, degrees in [−90, 90], positive towards galactic north.
    pub elevation_deg: f64,
    /// The frame of the azimuth.
    pub frame: BearingFrameDto,
}

/// What the crew knows of one contact, tagged by `level` (Design note 10): a bearing's fields
/// alone, or a resolved host's as well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "level", rename_all = "snake_case")]
#[ts(export)]
pub enum ContactDto {
    /// Something is there: a bearing, a flux and a band, and no host.
    Bearing(BearingContactDto),
    /// The host is identified.
    Resolved(ResolvedContactDto),
}

/// A contact as a bearing shows it, its latest sighting's: no host, distance, light age or
/// position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BearingContactDto {
    /// The contact's number, opaque and sequential per universe, from 1.
    pub contact: u32,
    /// What kind of event it is, as far as the light says.
    pub kind: TransientKindDto,
    /// The observer's time of its first sighting.
    pub first_seen: UniverseTime,
    /// The observer's time of its latest sighting.
    pub last_seen: UniverseTime,
    /// Where its light comes from; absent only for a sighting from the host's own place.
    pub bearing: Option<BearingDto>,
    /// Its flux in `band`, W m⁻², after extinction.
    pub flux_w_m2: f64,
    /// The band of the flux.
    pub band: AlertBandDto,
    /// Where its event stood when its light was first counted.
    pub phase: DetectionPhaseDto,
    /// Whether the crew has acknowledged it; acknowledgement never hides it.
    pub acknowledged: bool,
}

/// A resolved contact: a bearing's fields and the host's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolvedContactDto {
    /// What a bearing shows of it.
    #[serde(flatten)]
    pub seen: BearingContactDto,
    /// The host.
    pub host: SystemIdHex,
    /// The host's catalogue designation, derived from its ID.
    pub designation: String,
    /// The latest sighting's light age, Julian years.
    pub light_age_yr: f64,
    /// Where the host was when the light left it, in the galactic frame: where it is seen.
    pub apparent_position: GalacticPosition,
    /// Where it is at the latest sighting's time, extrapolated along its motion then.
    pub present_position: GalacticPosition,
    /// The stated bound on the apparent position's error from the neglected curvature of the
    /// host's galactic orbit, ly at the host (Design note 3).
    pub curvature_error_ly: f64,
}

/// `alerts_observer`: moves an alerts subscription's observer (Design note 9), answered with an
/// empty body.
///
/// A subscription the connection does not have, or one of another topic, is refused with
/// `bad_request` naming `subscription`; an observer outside the root cube or a time outside the
/// clock window with `bad_request` naming `observer` or `time`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertsObserverRequest {
    /// The subscription, as its `subscribed` answer numbered it.
    pub subscription: u32,
    /// Where the observer now is.
    pub observer: GalacticPosition,
    /// The observer's time now.
    pub time: UniverseTime,
}

/// `alerts_acknowledge`: the crew has acknowledged a contact, answered with an empty body.
///
/// A subscription the connection does not have, or one of another topic, is refused with
/// `bad_request` naming `subscription`, and a contact the universe's Knowledge does not hold with
/// `bad_request` naming `contact`. The subscription's next notification carries the contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertsAcknowledgeRequest {
    /// The subscription, as its `subscribed` answer numbered it.
    pub subscription: u32,
    /// The contact.
    pub contact: u32,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{
        NotificationBody, RequestBody, ResponseBody, ServerMessage, SubscribeRequest, Subscribed,
        SubscriptionState, SubscriptionTopic,
    };
    use crate::primitives::UniverseIdHex;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    fn sun() -> GalacticPosition {
        GalacticPosition {
            cell_ly: [0, 26_000, 20],
            offset_m: [0.0, 0.0, 0.0],
        }
    }

    fn sun_json() -> Value {
        json!({ "cell_ly": [0, 26_000, 20], "offset_m": [0.0, 0.0, 0.0] })
    }

    fn time() -> UniverseTime {
        UniverseTime {
            seconds: 86_400,
            nanos: 0,
        }
    }

    fn seen() -> BearingContactDto {
        BearingContactDto {
            contact: 3,
            kind: TransientKindDto::Unclassified,
            first_seen: time(),
            last_seen: time(),
            bearing: Some(BearingDto {
                azimuth_deg: 180.0,
                elevation_deg: -2.5,
                frame: BearingFrameDto::Local,
            }),
            flux_w_m2: 1e-12,
            band: AlertBandDto::V,
            phase: DetectionPhaseDto::Onset,
            acknowledged: false,
        }
    }

    fn seen_json() -> Value {
        json!({
            "contact": 3,
            "kind": "unclassified",
            "first_seen": { "seconds": 86_400, "nanos": 0 },
            "last_seen": { "seconds": 86_400, "nanos": 0 },
            "bearing": { "azimuth_deg": 180.0, "elevation_deg": -2.5, "frame": "local" },
            "flux_w_m2": 1e-12,
            "band": "v",
            "phase": "onset",
            "acknowledged": false,
        })
    }

    #[test]
    fn a_subscription_names_its_observer_sensor_and_classes() {
        let request = AlertsSubscribeRequest {
            observer: sun(),
            time: time(),
            sensor: SensorDto {
                bands: vec![BandLimitsDto {
                    band: AlertBandDto::XRay,
                    detect_w_m2: 3.6e-13,
                    resolve_w_m2: 3.6e-11,
                }],
            },
            classes: Some(vec![CatalogueClassWire::CoreCollapse]),
            host_limit: Some(1_000),
        };
        assert_wire_form(
            &request,
            json!({
                "observer": sun_json(),
                "time": { "seconds": 86_400, "nanos": 0 },
                "sensor": { "bands": [
                    { "band": "x_ray", "detect_w_m2": 3.6e-13, "resolve_w_m2": 3.6e-11 },
                ] },
                "classes": ["core_collapse"],
                "host_limit": 1_000,
            }),
        );
        // Every class and the default limit, when the request leaves them out.
        let bare = AlertsSubscribeRequest {
            classes: None,
            host_limit: None,
            ..request
        };
        assert_wire_form(
            &bare,
            json!({
                "observer": sun_json(),
                "time": { "seconds": 86_400, "nanos": 0 },
                "sensor": { "bands": [
                    { "band": "x_ray", "detect_w_m2": 3.6e-13, "resolve_w_m2": 3.6e-11 },
                ] },
            }),
        );
    }

    #[test]
    fn bands_kinds_phases_and_statuses_have_their_strings() {
        assert_wire_strings(&[
            (AlertBandDto::U, "u"),
            (AlertBandDto::B, "b"),
            (AlertBandDto::V, "v"),
            (AlertBandDto::R, "r"),
            (AlertBandDto::I, "i"),
            (AlertBandDto::J, "j"),
            (AlertBandDto::H, "h"),
            (AlertBandDto::K, "k"),
            (AlertBandDto::MidInfrared, "mid_infrared"),
            (AlertBandDto::Radio, "radio"),
            (AlertBandDto::XRay, "x_ray"),
        ]);
        assert_wire_strings(&[
            (TransientKindDto::Unclassified, "unclassified"),
            (TransientKindDto::CoreCollapse, "core_collapse"),
            (TransientKindDto::TypeIa, "type_ia"),
            (TransientKindDto::LuminousRedNova, "luminous_red_nova"),
            (TransientKindDto::Kilonova, "kilonova"),
            (TransientKindDto::Nova, "nova"),
            (TransientKindDto::DwarfNova, "dwarf_nova"),
            (TransientKindDto::XrayTransient, "xray_transient"),
            (TransientKindDto::XrayBurst, "xray_burst"),
            (TransientKindDto::BeXOutburst, "be_x_outburst"),
            (TransientKindDto::GiantEruption, "giant_eruption"),
            (TransientKindDto::TidalDisruption, "tidal_disruption"),
            (TransientKindDto::CentralFlare, "central_flare"),
            (TransientKindDto::Flare, "flare"),
            (TransientKindDto::Glitch, "glitch"),
            (TransientKindDto::MagnetarBurst, "magnetar_burst"),
            (TransientKindDto::FuOrionisOutburst, "fu_orionis_outburst"),
        ]);
        assert_wire_strings(&[
            (DetectionPhaseDto::Onset, "onset"),
            (DetectionPhaseDto::InProgress, "in_progress"),
        ]);
        assert_wire_strings(&[
            (ClassCensusStatusDto::Complete, "complete"),
            (ClassCensusStatusDto::OverLimit, "over_limit"),
            (ClassCensusStatusDto::Pending, "pending"),
        ]);
        assert_wire_strings(&[
            (BearingFrameDto::Local, "local"),
            (BearingFrameDto::GalacticX, "galactic_x"),
        ]);
    }

    #[test]
    fn a_bearing_names_no_host_and_a_resolved_contact_does() {
        let mut bearing = seen_json();
        bearing["level"] = json!("bearing");
        assert_wire_form(&ContactDto::Bearing(seen()), bearing.clone());
        let keys: Vec<&str> = bearing
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in [
            "host",
            "designation",
            "light_age_yr",
            "apparent_position",
            "present_position",
            "curvature_error_ly",
        ] {
            assert!(!keys.contains(&key), "a bearing holds `{key}`");
        }

        let resolved = ContactDto::Resolved(ResolvedContactDto {
            seen: BearingContactDto {
                kind: TransientKindDto::Flare,
                ..seen()
            },
            host: SystemIdHex::from_u64(0x0200_0800_2000_0000),
            designation: "Vorth AB-C e4-17".to_owned(),
            light_age_yr: 12.5,
            apparent_position: sun(),
            present_position: sun(),
            curvature_error_ly: 0.0,
        });
        let mut wire = seen_json();
        wire["level"] = json!("resolved");
        wire["kind"] = json!("flare");
        wire["host"] = json!("0200080020000000");
        wire["designation"] = json!("Vorth AB-C e4-17");
        wire["light_age_yr"] = json!(12.5);
        wire["apparent_position"] = sun_json();
        wire["present_position"] = sun_json();
        wire["curvature_error_ly"] = json!(0.0);
        assert_wire_form(&resolved, wire);
    }

    #[test]
    fn the_state_and_a_notification_carry_the_census_and_the_contacts() {
        let census = ClassCensusDto {
            class: CatalogueClassWire::TypeIa,
            status: ClassCensusStatusDto::Pending,
            reach_ly: 11_189.0,
            expected_hosts: 4.0,
        };
        let census_json = json!({
            "class": "type_ia", "status": "pending", "reach_ly": 11_189.0, "expected_hosts": 4.0,
        });
        let mut contact = seen_json();
        contact["level"] = json!("bearing");
        assert_wire_form(
            &AlertsSubscribed {
                census: vec![census],
                contacts: vec![ContactDto::Bearing(seen())],
            },
            json!({ "census": [census_json.clone()], "contacts": [contact.clone()] }),
        );
        assert_wire_form(
            &AlertsNotification {
                sequence: 2,
                contacts: vec![ContactDto::Bearing(seen())],
                census: Vec::new(),
            },
            json!({ "sequence": 2, "contacts": [contact], "census": [] }),
        );
    }

    #[test]
    fn the_requests_name_their_subscription() {
        assert_wire_form(
            &AlertsObserverRequest {
                subscription: 2,
                observer: sun(),
                time: time(),
            },
            json!({
                "subscription": 2,
                "observer": sun_json(),
                "time": { "seconds": 86_400, "nanos": 0 },
            }),
        );
        assert_wire_form(
            &AlertsAcknowledgeRequest {
                subscription: 2,
                contact: 3,
            },
            json!({ "subscription": 2, "contact": 3 }),
        );
    }

    /// The topic, its state and its notifications, and the two requests and their answers, as the
    /// envelope carries them.
    #[test]
    fn the_alerts_kinds_ride_the_envelope() {
        let sensor = SensorDto {
            bands: vec![BandLimitsDto {
                band: AlertBandDto::V,
                detect_w_m2: 1e-12,
                resolve_w_m2: 1e-10,
            }],
        };
        let sensor_json = json!({ "bands": [
            { "band": "v", "detect_w_m2": 1e-12, "resolve_w_m2": 1e-10 },
        ] });
        assert_wire_form(
            &RequestBody::Subscribe(SubscribeRequest {
                universe: UniverseIdHex::from_u64(42),
                topic: SubscriptionTopic::Alerts(AlertsSubscribeRequest {
                    observer: sun(),
                    time: time(),
                    sensor,
                    classes: None,
                    host_limit: None,
                }),
            }),
            json!({
                "kind": "subscribe",
                "universe": "000000000000002a",
                "topic": {
                    "topic": "alerts",
                    "observer": sun_json(),
                    "time": { "seconds": 86_400, "nanos": 0 },
                    "sensor": sensor_json,
                },
            }),
        );
        assert_wire_form(
            &ResponseBody::Subscribe(Box::new(Subscribed {
                subscription: 2,
                state: SubscriptionState::Alerts(AlertsSubscribed {
                    census: Vec::new(),
                    contacts: Vec::new(),
                }),
            })),
            json!({
                "kind": "subscribe",
                "subscription": 2,
                "state": { "topic": "alerts", "census": [], "contacts": [] },
            }),
        );
        assert_wire_form(
            &ServerMessage::Notification {
                subscription: 2,
                body: NotificationBody::Alerts(AlertsNotification {
                    sequence: 1,
                    contacts: Vec::new(),
                    census: Vec::new(),
                }),
            },
            json!({
                "type": "notification",
                "subscription": 2,
                "body": { "topic": "alerts", "sequence": 1, "contacts": [], "census": [] },
            }),
        );
        assert_wire_form(
            &RequestBody::AlertsObserver(AlertsObserverRequest {
                subscription: 2,
                observer: sun(),
                time: time(),
            }),
            json!({
                "kind": "alerts_observer",
                "subscription": 2,
                "observer": sun_json(),
                "time": { "seconds": 86_400, "nanos": 0 },
            }),
        );
        assert_wire_form(
            &RequestBody::AlertsAcknowledge(AlertsAcknowledgeRequest {
                subscription: 2,
                contact: 3,
            }),
            json!({ "kind": "alerts_acknowledge", "subscription": 2, "contact": 3 }),
        );
        assert_wire_form(
            &ResponseBody::AlertsObserver,
            json!({ "kind": "alerts_observer" }),
        );
        assert_wire_form(
            &ResponseBody::AlertsAcknowledge,
            json!({ "kind": "alerts_acknowledge" }),
        );
    }
}
