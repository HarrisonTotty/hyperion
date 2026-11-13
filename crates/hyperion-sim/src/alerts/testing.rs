//! Test helpers for alerts, compiled for this crate's tests and behind the `testing` feature.
//!
//! [`Detection`] has no public constructor, since only the alert walks build one. The server's
//! alert schedule (plan 12, P12.T8.a) is tested on detections made by hand as well as on the
//! walks', and [`detection`] makes them.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::catalogue_classes::ClassId;
use crate::id::{EventId, SystemId};
use crate::observe::{Retardation, bearing};
use crate::time::UniverseTime;
use crate::units::WattsPerSquareMetre;

use super::{AlertBand, Detection, DetectionLight, DetectionPhase, TransientKind};

/// The detection of `event` of `host`, of `class` and `kind`, in `phase`, seen in `band` at
/// `flux`, and whether it is `resolvable`: its light left a host at rest at `site` at `emitted`,
/// and reaches an observer at rest at `observer` one light time later.
///
/// Nothing checks that the parts agree with each other, or with any walk: a test sets the arrival
/// through `emitted` and the distance, and the flux it wants. Its light is
/// [`DetectionLight::Unlit`] whatever its kind: a test of what reads lights gives its own.
///
/// # Panics
///
/// If `site` is `observer`, from which no light comes with a bearing, or if the arrival falls off
/// the clock.
#[expect(
    clippy::too_many_arguments,
    reason = "a detection's fields, as `Detection::new` takes them, with the light's place and \
              time in place of its retardation and bearing"
)]
#[must_use]
pub fn detection(
    event: EventId,
    host: SystemId,
    class: Option<ClassId>,
    kind: TransientKind,
    phase: DetectionPhase,
    band: AlertBand,
    flux: WattsPerSquareMetre,
    resolvable: bool,
    site: GalacticPosition,
    emitted: UniverseTime,
    observer: &GalacticPosition,
) -> Detection {
    let retardation =
        Retardation::of_emission(observer, emitted, site, GalacticVelocity::default(), &site);
    Detection::new(
        event,
        host,
        class,
        kind,
        phase,
        band,
        flux,
        bearing(observer, &site).expect("a site apart from the observer has a bearing"),
        retardation,
        resolvable,
        DetectionLight::Unlit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::gas::ccm::Band;
    use crate::id::{EventBin, EventSubject, EventWord, event_tags};
    use crate::observe::light_time;

    #[test]
    fn a_hand_made_detection_arrives_one_light_time_after_it_left() {
        let host: SystemId = "0200080020000000".parse().unwrap();
        let event = EventId::new(
            EventSubject::System(host),
            EventWord::new(event_tags::ALL[0], EventBin::new(0).unwrap(), 0),
        );
        let observer = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
        let site = GalacticPosition::from_light_years([0.0, 26_010.0, 20.0]).unwrap();
        let made = detection(
            event,
            host,
            None,
            TransientKind::Flare,
            DetectionPhase::Onset,
            AlertBand::Photometric(Band::V),
            WattsPerSquareMetre::new(1e-12),
            false,
            site,
            UniverseTime::EPOCH,
            &observer,
        );
        let light = light_time(site.distance_to(&observer));
        assert_eq!(made.emitted(), UniverseTime::EPOCH);
        assert_eq!(
            made.arrival(),
            UniverseTime::EPOCH.checked_add(light).unwrap()
        );
        assert_eq!(made.retardation().observed(), made.arrival());
        assert_eq!((made.event(), made.host()), (event, host));
    }
}
