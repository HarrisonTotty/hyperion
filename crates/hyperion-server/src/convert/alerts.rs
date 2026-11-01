//! Plan 12's alerts on the wire (P12.T9.a): an alerts subscription's request checked into the
//! sim's observer, sensor and classes, and the crew's contacts as the wire carries them.
//!
//! A contact leaves the server only as its [`ContactView`] (Design note 10), so a bearing's DTO
//! is made from a bearing's view, which holds no host. Its kind shows only where its band alone
//! names it: a glitch, the one kind timed in the radio (lean, 2026-10-31).

use std::num::NonZeroU32;

use hyperion_protocol::{
    AlertBandDto, AlertsSubscribeRequest, BearingContactDto, BearingDto, BearingFrameDto,
    CatalogueClassWire, ClassCensusDto, ClassCensusStatusDto, ContactDto, DetectionPhaseDto,
    ErrorCode, RequestError, ResolvedContactDto, SensorDto, SystemIdHex, TransientKindDto,
};
use hyperion_sim::alerts::{
    AlertBand, BandLimits, ClassCensus, ClassSet, ClassStatus, DEFAULT_HOST_LIMIT, DetectionPhase,
    SensorHorizon, TransientKind, class_reach,
};
use hyperion_sim::galaxy::catalogue_classes::ClassId;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::observe::{Bearing, BearingFrame, Observer};
use hyperion_sim::units::WattsPerSquareMetre;

use super::features::catalogue_class;
use super::{ConvertRequestError, clock_time, galactic_position, root_cube_position, wire_time};
use crate::knowledge::{
    AcknowledgeContactError, Acknowledgement, BearingContact, ContactView, LoadKnowledgeError,
};

/// An `alerts` subscription's request, checked: the observer, the sensor, the classes asked about
/// and their host limit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AlertsAsked {
    pub(crate) observer: Observer,
    pub(crate) sensor: SensorHorizon,
    pub(crate) classes: ClassSet,
    pub(crate) host_limit: NonZeroU32,
}

impl TryFrom<&AlertsSubscribeRequest> for AlertsAsked {
    type Error = ConvertRequestError;

    /// Refused naming `observer` for a position outside the root cube, `time` for one outside the
    /// clock window, `sensor` for limits no sensor has, and `host_limit` for a limit of zero.
    fn try_from(request: &AlertsSubscribeRequest) -> Result<Self, Self::Error> {
        let host_limit = match request.host_limit {
            None => DEFAULT_HOST_LIMIT,
            Some(limit) => NonZeroU32::new(limit)
                .ok_or_else(|| ConvertRequestError::new("host_limit", "a host limit of zero"))?,
        };
        let classes = request.classes.as_ref().map_or(ClassSet::ALL, |classes| {
            classes.iter().copied().map(class_of).collect()
        });
        Ok(Self {
            observer: observer(&request.observer, &request.time)?,
            sensor: sensor(&request.sensor)?,
            classes,
            host_limit,
        })
    }
}

/// An observer at `position` at `time`: refused naming `observer` outside the root cube, and
/// `time` outside the clock window.
pub(crate) fn observer(
    position: &hyperion_protocol::GalacticPosition,
    time: &hyperion_protocol::UniverseTime,
) -> Result<Observer, ConvertRequestError> {
    let position = root_cube_position("observer", position)?;
    let time = clock_time("time", time)?;
    Observer::new(position, time).map_err(|error| ConvertRequestError::new("observer", error))
}

/// The sensor of `dto`'s limits, refused naming `sensor`.
fn sensor(dto: &SensorDto) -> Result<SensorHorizon, ConvertRequestError> {
    let w = WattsPerSquareMetre::new;
    let bands = dto
        .bands
        .iter()
        .map(|limits| {
            BandLimits::new(w(limits.detect_w_m2), w(limits.resolve_w_m2))
                .map(|checked| (alert_band(limits.band), checked))
                .map_err(|error| ConvertRequestError::new("sensor", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    SensorHorizon::new(bands).map_err(|error| ConvertRequestError::new("sensor", error))
}

/// The sim's class of a wire class.
#[must_use]
const fn class_of(class: CatalogueClassWire) -> ClassId {
    match class {
        CatalogueClassWire::CoreCollapse => ClassId::CORE_COLLAPSE,
        CatalogueClassWire::TypeIa => ClassId::TYPE_IA,
        CatalogueClassWire::StellarMerger => ClassId::STELLAR_MERGER,
        CatalogueClassWire::NeutronStarMerger => ClassId::NEUTRON_STAR_MERGER,
        CatalogueClassWire::LuminousBlueVariable => ClassId::LUMINOUS_BLUE_VARIABLE,
        CatalogueClassWire::XrayBinary => ClassId::XRAY_BINARY,
        CatalogueClassWire::AccretingWhiteDwarf => ClassId::ACCRETING_WHITE_DWARF,
        CatalogueClassWire::TidalDisruptionVictim => ClassId::TIDAL_DISRUPTION_VICTIM,
        CatalogueClassWire::AccretingWhiteDwarfSlow => ClassId::ACCRETING_WHITE_DWARF_SLOW,
    }
}

/// The sim's band of a wire band.
#[must_use]
const fn alert_band(band: AlertBandDto) -> AlertBand {
    match band {
        AlertBandDto::U => AlertBand::Photometric(Band::U),
        AlertBandDto::B => AlertBand::Photometric(Band::B),
        AlertBandDto::V => AlertBand::Photometric(Band::V),
        AlertBandDto::R => AlertBand::Photometric(Band::R),
        AlertBandDto::I => AlertBand::Photometric(Band::I),
        AlertBandDto::J => AlertBand::Photometric(Band::J),
        AlertBandDto::H => AlertBand::Photometric(Band::H),
        AlertBandDto::K => AlertBand::Photometric(Band::K),
        AlertBandDto::MidInfrared => AlertBand::Photometric(Band::MidInfrared),
        AlertBandDto::Radio => AlertBand::Photometric(Band::Radio),
        AlertBandDto::XRay => AlertBand::XRay,
    }
}

/// The wire's band.
#[must_use]
const fn band_dto(band: AlertBand) -> AlertBandDto {
    match band {
        AlertBand::Photometric(Band::U) => AlertBandDto::U,
        AlertBand::Photometric(Band::B) => AlertBandDto::B,
        AlertBand::Photometric(Band::V) => AlertBandDto::V,
        AlertBand::Photometric(Band::R) => AlertBandDto::R,
        AlertBand::Photometric(Band::I) => AlertBandDto::I,
        AlertBand::Photometric(Band::J) => AlertBandDto::J,
        AlertBand::Photometric(Band::H) => AlertBandDto::H,
        AlertBand::Photometric(Band::K) => AlertBandDto::K,
        AlertBand::Photometric(Band::MidInfrared) => AlertBandDto::MidInfrared,
        AlertBand::Photometric(Band::Radio) => AlertBandDto::Radio,
        AlertBand::XRay => AlertBandDto::XRay,
    }
}

/// The wire's kind of event.
#[must_use]
const fn kind_dto(kind: TransientKind) -> TransientKindDto {
    match kind {
        TransientKind::CoreCollapse => TransientKindDto::CoreCollapse,
        TransientKind::TypeIa => TransientKindDto::TypeIa,
        TransientKind::LuminousRedNova => TransientKindDto::LuminousRedNova,
        TransientKind::Kilonova => TransientKindDto::Kilonova,
        TransientKind::Nova => TransientKindDto::Nova,
        TransientKind::DwarfNova => TransientKindDto::DwarfNova,
        TransientKind::XrayTransient => TransientKindDto::XrayTransient,
        TransientKind::XrayBurst => TransientKindDto::XrayBurst,
        TransientKind::BeXOutburst => TransientKindDto::BeXOutburst,
        TransientKind::GiantEruption => TransientKindDto::GiantEruption,
        TransientKind::TidalDisruption => TransientKindDto::TidalDisruption,
        TransientKind::CentralFlare => TransientKindDto::CentralFlare,
        TransientKind::Flare => TransientKindDto::Flare,
        TransientKind::Glitch => TransientKindDto::Glitch,
        TransientKind::MagnetarBurst => TransientKindDto::MagnetarBurst,
        TransientKind::FuOrionisOutburst => TransientKindDto::FuOrionisOutburst,
    }
}

/// What a bearing shows of its kind: a glitch, which its band alone names, and otherwise
/// `unclassified` (module documentation).
#[must_use]
const fn kind_at_a_bearing(kind: TransientKind) -> TransientKindDto {
    match kind {
        TransientKind::Glitch => TransientKindDto::Glitch,
        TransientKind::CoreCollapse
        | TransientKind::TypeIa
        | TransientKind::LuminousRedNova
        | TransientKind::Kilonova
        | TransientKind::Nova
        | TransientKind::DwarfNova
        | TransientKind::XrayTransient
        | TransientKind::XrayBurst
        | TransientKind::BeXOutburst
        | TransientKind::GiantEruption
        | TransientKind::TidalDisruption
        | TransientKind::CentralFlare
        | TransientKind::Flare
        | TransientKind::MagnetarBurst
        | TransientKind::FuOrionisOutburst => TransientKindDto::Unclassified,
    }
}

/// The wire's bearing.
#[must_use]
fn bearing_dto(bearing: &Bearing) -> BearingDto {
    BearingDto {
        azimuth_deg: bearing.azimuth().value(),
        elevation_deg: bearing.elevation().value(),
        frame: match bearing.frame() {
            BearingFrame::Local => BearingFrameDto::Local,
            BearingFrame::GalacticX => BearingFrameDto::GalacticX,
        },
    }
}

/// What a bearing shows of `seen`, with its kind as `kind`.
#[must_use]
fn seen_dto(seen: &BearingContact, kind: TransientKindDto) -> BearingContactDto {
    BearingContactDto {
        contact: seen.contact().get(),
        kind,
        first_seen: wire_time(seen.first_seen()),
        last_seen: wire_time(seen.last_seen()),
        bearing: seen.bearing().as_ref().map(bearing_dto),
        flux_w_m2: seen.flux().value(),
        band: band_dto(seen.band()),
        phase: match seen.phase() {
            DetectionPhase::Onset => DetectionPhaseDto::Onset,
            DetectionPhase::InProgress => DetectionPhaseDto::InProgress,
        },
        acknowledged: seen.acknowledged() == Acknowledgement::Acknowledged,
    }
}

/// The contact as the wire carries it: only its level's fields (Design note 10).
#[must_use]
pub(crate) fn contact_dto(view: &ContactView) -> ContactDto {
    match view {
        ContactView::Bearing(seen) => {
            ContactDto::Bearing(seen_dto(seen, kind_at_a_bearing(seen.kind())))
        }
        ContactView::Resolved(resolved) => {
            let seen = resolved.seen();
            ContactDto::Resolved(ResolvedContactDto {
                seen: seen_dto(seen, kind_dto(seen.kind())),
                host: SystemIdHex::from_u64(resolved.host().raw()),
                designation: resolved.host().designation().to_string(),
                light_age_yr: resolved.light_age().as_julian_years_f64(),
                apparent_position: galactic_position(resolved.apparent_position()),
                present_position: galactic_position(resolved.present_position()),
                curvature_error_ly: resolved.curvature_error().value(),
            })
        }
    }
}

/// A class's census as the wire carries it (Design note 7): `pending` for a class its scan sends
/// to the background (P12.T8.b), until the subscription's walk of it finishes, and `unwalked` for a
/// class whose source is not built or whose walk the server lends no global list (P12.T5.d). `None`
/// for a class with no wire value, which `catalogue_class`'s test keeps from being registered.
#[must_use]
pub(crate) fn census_dto(census: &ClassCensus) -> Option<ClassCensusDto> {
    let status = match census.status() {
        ClassStatus::Complete => ClassCensusStatusDto::Complete,
        ClassStatus::OverLimit => ClassCensusStatusDto::OverLimit,
        ClassStatus::NeedsBackgroundScan => ClassCensusStatusDto::Pending,
        ClassStatus::Unwalked(_) => ClassCensusStatusDto::Unwalked,
    };
    Some(ClassCensusDto {
        class: catalogue_class(census.class())?,
        status,
        reach_ly: census.reach().value(),
        expected_hosts: census.expected_hosts(),
    })
}

/// A class's census before its first scan has run: `pending`, at the reach of `sensor`, with no
/// count of its hosts; `None` as [`census_dto`] gives it.
#[must_use]
pub(crate) fn pending_census(class: ClassId, sensor: &SensorHorizon) -> Option<ClassCensusDto> {
    Some(ClassCensusDto {
        class: catalogue_class(class)?,
        status: ClassCensusStatusDto::Pending,
        reach_ly: class_reach(class, sensor).value(),
        expected_hosts: None,
    })
}

/// The refusal of an alerts subscription whose universe's Knowledge cannot be opened.
#[must_use]
pub(crate) fn open_contacts_refusal(error: &LoadKnowledgeError) -> RequestError {
    tracing::error!(?error, "failed to open a universe's contacts");
    let (code, message) = match error {
        LoadKnowledgeError::UnsupportedFormat { format, .. } => (
            ErrorCode::UnsupportedSaveFormat,
            format!(
                "the universe's contacts are in format {format}, which this server cannot read"
            ),
        ),
        LoadKnowledgeError::Io { .. }
        | LoadKnowledgeError::MalformedLine { .. }
        | LoadKnowledgeError::Interrupted => (
            ErrorCode::Internal,
            "the server could not read the universe's contacts".to_owned(),
        ),
    };
    RequestError {
        code,
        message,
        field: None,
    }
}

/// The refusal of an `alerts_acknowledge`: `bad_request` naming `contact` for a contact the
/// universe does not hold, and `storage_failed` for one whose acknowledgement could not be saved.
#[must_use]
pub(crate) fn acknowledge_refusal(error: &AcknowledgeContactError) -> RequestError {
    match error {
        AcknowledgeContactError::UnknownContact(_) => RequestError {
            code: ErrorCode::BadRequest,
            message: error.to_string(),
            field: Some("contact".to_owned()),
        },
        AcknowledgeContactError::Save(save) => {
            tracing::error!(?save, "failed to save an acknowledgement");
            RequestError {
                code: ErrorCode::StorageFailed,
                message: error.to_string(),
                field: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{BandLimitsDto, UniverseTime};

    use super::*;

    fn request() -> AlertsSubscribeRequest {
        AlertsSubscribeRequest {
            observer: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 26_000, 20],
                offset_m: [0.0; 3],
            },
            time: UniverseTime {
                seconds: 0,
                nanos: 0,
            },
            sensor: SensorDto {
                bands: vec![BandLimitsDto {
                    band: AlertBandDto::V,
                    detect_w_m2: 1e-12,
                    resolve_w_m2: 1e-10,
                }],
            },
            classes: None,
            host_limit: None,
        }
    }

    fn field(error: ConvertRequestError) -> Option<String> {
        RequestError::from(error).field
    }

    /// A subscription's request is checked field by field, every class and the sim's host limit
    /// when it names neither.
    #[test]
    fn an_alerts_request_is_refused_by_the_field_at_fault() {
        let asked = AlertsAsked::try_from(&request()).unwrap();
        assert_eq!(asked.classes, ClassSet::ALL);
        assert_eq!(asked.host_limit, DEFAULT_HOST_LIMIT);
        let named = AlertsAsked::try_from(&AlertsSubscribeRequest {
            classes: Some(vec![CatalogueClassWire::TypeIa]),
            host_limit: Some(10),
            ..request()
        })
        .unwrap();
        assert_eq!(named.classes, ClassSet::of(ClassId::TYPE_IA));
        assert_eq!(named.host_limit.get(), 10);

        let outside = AlertsSubscribeRequest {
            observer: hyperion_protocol::GalacticPosition {
                cell_ly: [80_000, 0, 0],
                offset_m: [0.0; 3],
            },
            ..request()
        };
        let late = AlertsSubscribeRequest {
            time: UniverseTime {
                seconds: 40_000_000_000,
                nanos: 0,
            },
            ..request()
        };
        let mut faint = request();
        faint.sensor.bands[0].resolve_w_m2 = 1e-14;
        let mut twice = request();
        twice.sensor.bands.push(twice.sensor.bands[0]);
        let none = AlertsSubscribeRequest {
            sensor: SensorDto { bands: Vec::new() },
            ..request()
        };
        let zero = AlertsSubscribeRequest {
            host_limit: Some(0),
            ..request()
        };
        for (refused, at) in [
            (outside, "observer"),
            (late, "time"),
            (faint, "sensor"),
            (twice, "sensor"),
            (none, "sensor"),
            (zero, "host_limit"),
        ] {
            let error = AlertsAsked::try_from(&refused).unwrap_err();
            assert_eq!(field(error).as_deref(), Some(at));
        }
    }

    /// Every wire band and class is the sim's, and back.
    #[test]
    fn bands_and_classes_cross_the_wire_both_ways() {
        for band in AlertBand::ALL {
            assert_eq!(alert_band(band_dto(band)), band);
        }
        for kind in TransientKind::ALL {
            let shown = kind_at_a_bearing(kind);
            assert!(
                shown == TransientKindDto::Unclassified || shown == kind_dto(kind),
                "{kind:?}"
            );
        }
        assert_eq!(
            kind_at_a_bearing(TransientKind::Glitch),
            TransientKindDto::Glitch
        );
        assert_eq!(
            kind_at_a_bearing(TransientKind::TypeIa),
            TransientKindDto::Unclassified
        );
        assert_eq!(
            class_of(CatalogueClassWire::AccretingWhiteDwarfSlow),
            ClassId::ACCRETING_WHITE_DWARF_SLOW
        );
    }
}
