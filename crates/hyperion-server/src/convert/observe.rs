//! Plan 12's observation on the wire (P12.T6): the query mode a request asks in, what an observed
//! row carries, and the `resolve_system` request and its answer.
//!
//! The mode's observer is checked here as the range query's centre is, a canonical position inside
//! the root cube, and refused as `bad_request` naming `mode`. Only the query's time is held to the
//! clock window (plan 04's ±H check): the light an observer receives left its source earlier, back
//! to the source horizon, and that emission time is reported, never checked.

use hyperion_protocol::{
    ErrorCode, ObservedDto, QueryModeDto, RequestError, ResolveSystemRequest, ResolvedSystem,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity, LyCell};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::query::{QueryMode, SystemHit};
use hyperion_sim::id::SystemId;
use hyperion_sim::observe::{CurvatureError, Retardation, TraceMotionError};
use hyperion_sim::time::UniverseTime;

use super::stellar::wire_time;
use super::{ConvertRequestError, RowStellar, galactic_position, query_time, system_record_moving};
use crate::convert::unknown_system;

/// The sim's query mode for the wire's: `now`, or observed from a canonical position inside the
/// root cube.
///
/// # Errors
///
/// A [`ConvertRequestError`] naming `mode` if the observer is not a canonical position or lies
/// outside the root cube, where no observer can be (plan 12's `Observer`).
pub(super) fn query_mode(mode: &QueryModeDto) -> Result<QueryMode, ConvertRequestError> {
    match mode {
        QueryModeDto::Now => Ok(QueryMode::Now),
        QueryModeDto::Observed { observer } => {
            let position = GalacticPosition::new(LyCell::new(observer.cell_ly), observer.offset_m)
                .map_err(|error| ConvertRequestError::new("mode", error))?;
            if !position.in_root_cube() {
                return Err(ConvertRequestError::new(
                    "mode",
                    "the observer lies outside the galaxy's root cube",
                ));
            }
            Ok(QueryMode::ObservedFrom(position))
        }
    }
}

/// What an observer receives from one system, as the wire carries it: the retardation's emitted
/// time, light age and apparent position, and the stated curvature error as a length and an angle.
#[must_use]
pub(crate) fn observed_dto(retardation: &Retardation, error: CurvatureError) -> ObservedDto {
    ObservedDto {
        emitted: wire_time(retardation.emitted()),
        light_age_yr: retardation.light_age().as_julian_years_f64(),
        apparent_position: galactic_position(retardation.apparent_position()),
        curvature_error_ly: error.length().value(),
        curvature_error_arcsec: error.angle().value(),
    }
}

/// The refusal of an observed answer for a system whose motion the sim cannot trace yet: a member
/// of the galactic centre until plan 09's P09.T28 builds its orbit (P12.T2 as built). It is
/// `bad_request` naming `mode`, since the same request asked `now` is served.
#[must_use]
pub(crate) fn untraced(error: TraceMotionError) -> RequestError {
    RequestError {
        code: ErrorCode::BadRequest,
        message: format!("observed mode cannot be answered: {error}"),
        field: Some("mode".to_owned()),
    }
}

/// The refusal of one system, named by the request's `system`, whose motion the sim cannot trace
/// yet: `unknown_system`, as `system_summary` refuses a member of the galactic centre, with a
/// message that says why the system cannot be placed rather than that it does not exist.
#[must_use]
pub(crate) fn untraced_system(
    system: &hyperion_protocol::SystemIdHex,
    error: TraceMotionError,
) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownSystem,
        message: format!("system {} cannot be placed yet: {error}", system.as_str()),
        field: Some("system".to_owned()),
    }
}

/// A `resolve_system` request, checked: the time inside the clock window, the system it names,
/// decoded but not yet resolved, and the mode.
///
/// The universe is looked up before this, as every handler that names one does. Then `time` is
/// checked as the range query checks it, a `bad_request` naming `time`; then the ID is decoded, as
/// `system_summary` decodes it, a well-formed ID whose bits are no system ID being `unknown_system`
/// naming `system`; then the mode's observer, a `bad_request` naming `mode`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolveRequest {
    system: SystemId,
    time: UniverseTime,
    mode: QueryMode,
}

impl ResolveRequest {
    /// The system asked about, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.system
    }

    /// The instant asked about, inside the clock window.
    #[must_use]
    pub(crate) fn time(self) -> UniverseTime {
        self.time
    }

    /// The mode asked in, its observer inside the root cube.
    #[must_use]
    pub(crate) fn mode(self) -> QueryMode {
        self.mode
    }
}

impl TryFrom<&ResolveSystemRequest> for ResolveRequest {
    type Error = RequestError;

    /// Checks `time`, then decodes `system`, then checks `mode`.
    fn try_from(request: &ResolveSystemRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let system = SystemId::from_raw(request.system.to_u64())
            .map_err(|error| unknown_system(&request.system, error))?;
        let mode = query_mode(&request.mode)?;
        Ok(Self { system, time, mode })
    }
}

/// The answer to `resolve_system`: the system's range row at the request's time, moving at
/// `velocity` from the epoch, with `observed` in observed mode and no brief, since the request asks
/// for none.
///
/// `universe` and `time` are echoed from the request, which is why it is taken by value.
#[must_use]
pub(crate) fn resolved_system(
    galaxy: &Galaxy,
    request: ResolveSystemRequest,
    time: UniverseTime,
    (hit, velocity): (&SystemHit, GalacticVelocity),
    observed: Option<ObservedDto>,
) -> ResolvedSystem {
    ResolvedSystem {
        universe: request.universe,
        time: request.time,
        record: system_record_moving(galaxy, hit, velocity, time, RowStellar::NotAsked, observed),
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{SystemIdHex, UniverseIdHex};
    use hyperion_sim::observe::{Drift, Observer, retarded};
    use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;

    use super::*;

    fn wire_position(cell_ly: [i32; 3]) -> hyperion_protocol::GalacticPosition {
        hyperion_protocol::GalacticPosition {
            cell_ly,
            offset_m: [0.0; 3],
        }
    }

    fn request(mode: QueryModeDto) -> ResolveSystemRequest {
        ResolveSystemRequest {
            universe: UniverseIdHex::from_u64(42),
            system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
            time: hyperion_protocol::UniverseTime::default(),
            mode,
        }
    }

    #[test]
    fn now_is_the_sims_now_and_an_observer_inside_the_cube_is_taken_exactly() {
        assert_eq!(query_mode(&QueryModeDto::Now), Ok(QueryMode::Now));
        let observer = hyperion_protocol::GalacticPosition {
            cell_ly: [-1, 23_000, 40],
            offset_m: [0.25, 1.5e15, 0.0],
        };
        let expected = GalacticPosition::new(LyCell::new([-1, 23_000, 40]), [0.25, 1.5e15, 0.0])
            .expect("a canonical position");
        assert_eq!(
            query_mode(&QueryModeDto::Observed { observer }),
            Ok(QueryMode::ObservedFrom(expected))
        );
    }

    #[test]
    fn an_observer_outside_the_cube_or_off_its_grid_names_the_mode() {
        let outside = QueryModeDto::Observed {
            observer: wire_position([70_000, 0, 0]),
        };
        let refused = query_mode(&outside).unwrap_err();
        assert_eq!(RequestError::from(refused).field.as_deref(), Some("mode"));
        let off_grid = QueryModeDto::Observed {
            observer: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 26_000, 0],
                offset_m: [METRES_PER_LIGHT_YEAR * 2.0, 0.0, 0.0],
            },
        };
        let refused = RequestError::from(query_mode(&off_grid).unwrap_err());
        assert_eq!(refused.code, ErrorCode::BadRequest);
        assert_eq!(refused.field.as_deref(), Some("mode"));
    }

    #[test]
    fn a_resolve_request_checks_time_then_system_then_mode() {
        let outside = QueryModeDto::Observed {
            observer: wire_position([70_000, 0, 0]),
        };
        let late = hyperion_protocol::UniverseTime {
            seconds: 40_000_000_000,
            nanos: 0,
        };
        let mut both = request(outside);
        both.time = late;
        let error = ResolveRequest::try_from(&both).unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("time"))
        );
        // Bits that are no system ID at all, the first of a scatter over the 64 bits.
        let no_id = (0_u64..100_000)
            .map(|k| k.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .find(|&raw| SystemId::from_raw(raw).is_err())
            .expect("most 64-bit patterns are no system ID");
        let mut bad_id = request(outside);
        bad_id.system = SystemIdHex::from_u64(no_id);
        let error = ResolveRequest::try_from(&bad_id).unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::UnknownSystem, Some("system"))
        );
        let error = ResolveRequest::try_from(&request(outside)).unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("mode"))
        );
        let now = ResolveRequest::try_from(&request(QueryModeDto::Now)).unwrap();
        assert_eq!(now.mode(), QueryMode::Now);
        assert_eq!(now.time(), UniverseTime::EPOCH);
        assert_eq!(now.system().raw(), 0x0200_0800_2000_0000);
    }

    #[test]
    fn the_observed_dto_carries_the_retardation_and_the_error_exactly() {
        let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let there = GalacticPosition::from_light_years([0.0, 16_000.0, 0.0]).unwrap();
        let observer = Observer::new(here, UniverseTime::EPOCH).unwrap();
        let seen = retarded(
            &observer,
            &Drift::new(there, hyperion_sim::coords::GalacticVelocity::default()),
        );
        let wire = observed_dto(&seen, CurvatureError::ZERO);
        assert_eq!(wire.emitted, wire_time(seen.emitted()));
        assert!(wire.emitted.seconds < 0, "the light left 10,000 years ago");
        assert_eq!(
            wire.light_age_yr.to_bits(),
            seen.light_age().as_julian_years_f64().to_bits()
        );
        assert_eq!(wire.apparent_position, galactic_position(&there));
        assert_eq!(
            (wire.curvature_error_ly, wire.curvature_error_arcsec),
            (0.0, 0.0)
        );
    }

    #[test]
    fn an_untraced_system_is_refused_naming_the_mode() {
        let id = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        let error = untraced(TraceMotionError::CentreOrbitNotBuilt(id));
        assert_eq!(error.code, ErrorCode::BadRequest);
        assert_eq!(error.field.as_deref(), Some("mode"));
        assert!(error.message.contains("not built yet"), "{}", error.message);
        let hex = SystemIdHex::from_u64(id.raw());
        let error = untraced_system(&hex, TraceMotionError::CentreOrbitNotBuilt(id));
        assert_eq!(error.code, ErrorCode::UnknownSystem);
        assert_eq!(error.field.as_deref(), Some("system"));
        assert!(
            error
                .message
                .starts_with("system 0200080020000000 cannot be placed yet"),
            "{}",
            error.message
        );
    }
}
