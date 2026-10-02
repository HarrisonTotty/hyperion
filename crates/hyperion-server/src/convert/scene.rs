//! The scene's requests, checked (rendering plan R03, R03.T6): what `scene_ship` sets, before the
//! frame it names is resolved against the universe's galaxy.
//!
//! A field that cannot be used is `bad_request` naming it, the pose's fields under `ship.`: the
//! rate (`time_rate`), the time (`ship.time`, inside the clock window), the velocity
//! (`ship.velocity_m_s`, finite and below c, which the observer's Lorentz factor needs) and the
//! position (`ship.position`, finite, and a galactic position canonical and inside the root
//! cube). A frame's ID whose bits are no system's is `unknown_system`, and a body index outside
//! plan 14's layout `bad_request`, both naming `ship.position`; the handler resolves the rest.

use hyperion_protocol::{
    BodyIdHex, ErrorCode, FramePositionDto, KinematicsDto, RequestError, SceneShipRequest,
};
use hyperion_sim::coords::{BodyPosition, GalacticPosition, LyCell, SystemPosition};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::planetary::BodyIndex;
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::consts::SPEED_OF_LIGHT;

use super::ConvertRequestError;
use crate::scene::{ShipPosition, TimeRate};

/// The field a `scene_ship` position is refused on.
const POSITION: &str = "ship.position";

/// A `scene_ship` request, checked: the rate, the pose's time and velocity, and its position in a
/// frame whose ID has been decoded but not yet resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShipRequest {
    rate: TimeRate,
    time: UniverseTime,
    velocity_m_s: [f64; 3],
    position: ShipPosition,
    wire: KinematicsDto,
}

impl ShipRequest {
    /// The clock's rate.
    #[must_use]
    pub(crate) fn rate(&self) -> TimeRate {
        self.rate
    }

    /// The pose's time, inside the clock window: the scene time set.
    #[must_use]
    pub(crate) fn time(&self) -> UniverseTime {
        self.time
    }

    /// The velocity relative to the frame's origin, finite and below c.
    #[must_use]
    pub(crate) fn velocity_m_s(&self) -> [f64; 3] {
        self.velocity_m_s
    }

    /// The position, in a frame not yet resolved.
    #[must_use]
    pub(crate) fn position(&self) -> ShipPosition {
        self.position
    }

    /// The pose as the client sent it.
    #[must_use]
    pub(crate) fn into_wire(self) -> KinematicsDto {
        self.wire
    }
}

impl TryFrom<&SceneShipRequest> for ShipRequest {
    type Error = RequestError;

    /// Checks `time_rate`, then the pose's time, velocity and position, in that order.
    fn try_from(request: &SceneShipRequest) -> Result<Self, Self::Error> {
        let rate = TimeRate::new(request.time_rate)
            .map_err(|error| ConvertRequestError::new("time_rate", error))?;
        let ship = &request.ship;
        let time = UniverseTime::new(ship.time.seconds, ship.time.nanos)
            .map_err(|error| ConvertRequestError::new("ship.time", error))?;
        if !ClockWindow::contains(time) {
            return Err(ConvertRequestError::new(
                "ship.time",
                format!("{time} lies outside the clock window, 1,000 Julian years either side of the epoch"),
            )
            .into());
        }
        let velocity_m_s = ship.velocity_m_s;
        if !velocity_m_s.iter().all(|v| v.is_finite()) {
            return Err(
                ConvertRequestError::new("ship.velocity_m_s", "a component is not finite").into(),
            );
        }
        let speed_squared: f64 = velocity_m_s.iter().map(|v| v * v).sum();
        if speed_squared >= SPEED_OF_LIGHT * SPEED_OF_LIGHT {
            return Err(ConvertRequestError::new(
                "ship.velocity_m_s",
                "the speed is not below the speed of light",
            )
            .into());
        }
        let position = ship_position(&ship.position)?;
        Ok(Self {
            rate,
            time,
            velocity_m_s,
            position,
            wire: ship.clone(),
        })
    }
}

/// The pose's position, its frame's ID decoded.
fn ship_position(position: &FramePositionDto) -> Result<ShipPosition, RequestError> {
    match position {
        FramePositionDto::Galactic { position } => {
            let galactic = GalacticPosition::new(LyCell::new(position.cell_ly), position.offset_m)
                .map_err(|error| ConvertRequestError::new(POSITION, error))?;
            if !galactic.in_root_cube() {
                return Err(ConvertRequestError::new(
                    POSITION,
                    "the position lies outside the galaxy's root cube",
                )
                .into());
            }
            Ok(ShipPosition::Galactic(galactic))
        }
        FramePositionDto::System { system, offset_m } => {
            finite_offset(*offset_m)?;
            let id = SystemId::from_raw(system.to_u64()).map_err(|error| {
                unknown_frame_system(&format!("system {}", system.as_str()), error)
            })?;
            Ok(ShipPosition::System {
                system: id,
                offset: SystemPosition::new(*offset_m),
            })
        }
        FramePositionDto::Body { body, offset_m } => {
            finite_offset(*offset_m)?;
            Ok(ShipPosition::Body {
                body: frame_body(body)?,
                offset: BodyPosition::new(*offset_m),
            })
        }
    }
}

/// The body a body frame names, its system decoded and its index inside plan 14's layout.
///
/// # Errors
///
/// `unknown_system` for a system part that is no system ID, and `bad_request` for an index
/// outside the layout, both naming `ship.position`.
fn frame_body(body: &BodyIdHex) -> Result<BodyId, RequestError> {
    let (system, index) = body.to_parts();
    let system = SystemId::from_raw(system)
        .map_err(|error| unknown_frame_system(&format!("body {}", body.as_str()), error))?;
    let index = BodyIndex::try_from(index).map_err(|error| {
        ConvertRequestError::new(POSITION, format!("body {}: {error}", body.as_str()))
    })?;
    Ok(index.body_id(system))
}

/// Refuses an offset with a component that is not finite.
fn finite_offset(offset_m: [f64; 3]) -> Result<(), ConvertRequestError> {
    if offset_m.iter().all(|m| m.is_finite()) {
        Ok(())
    } else {
        Err(ConvertRequestError::new(
            POSITION,
            "an offset component is not finite",
        ))
    }
}

/// The refusal of a frame whose system is none of this universe's: `unknown_system`, naming
/// `ship.position`.
#[must_use]
pub(crate) fn unknown_frame_system(what: &str, reason: impl std::fmt::Display) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownSystem,
        message: format!("{what} names no system of this universe: {reason}"),
        field: Some(POSITION.to_owned()),
    }
}

/// The refusal of a body frame whose body its system does not hold, or that is not present at
/// the pose's time: `unknown_body`, naming `ship.position`.
#[must_use]
pub(crate) fn unknown_frame_body(body: BodyId, reason: &str) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownBody,
        message: format!(
            "body {} cannot be the ship's frame: {reason}",
            BodyIdHex::from_parts(body.system().raw(), body.body_index()).as_str()
        ),
        field: Some(POSITION.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{SystemIdHex, UniverseIdHex};

    use super::*;

    fn request(
        position: FramePositionDto,
        velocity_m_s: [f64; 3],
        time_rate: u32,
    ) -> SceneShipRequest {
        SceneShipRequest {
            universe: UniverseIdHex::from_u64(1),
            ship: KinematicsDto {
                position,
                velocity_m_s,
                time: hyperion_protocol::UniverseTime {
                    seconds: 10,
                    nanos: 0,
                },
            },
            time_rate,
        }
    }

    fn galactic() -> FramePositionDto {
        FramePositionDto::Galactic {
            position: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 26_000, 0],
                offset_m: [0.0; 3],
            },
        }
    }

    fn field(result: Result<ShipRequest, RequestError>) -> (ErrorCode, Option<String>) {
        let error = result.unwrap_err();
        (error.code, error.field)
    }

    fn bad(field: &str) -> (ErrorCode, Option<String>) {
        (ErrorCode::BadRequest, Some(field.to_owned()))
    }

    #[test]
    fn a_good_request_is_read_as_sent() {
        let checked = ShipRequest::try_from(&request(galactic(), [1.0, 2.0, 3.0], 1_000)).unwrap();
        assert_eq!(checked.rate().get(), 1_000);
        assert_eq!(checked.time(), UniverseTime::new(10, 0).unwrap());
        assert_eq!(
            checked.velocity_m_s().map(f64::to_bits),
            [1.0_f64, 2.0, 3.0].map(f64::to_bits)
        );
    }

    #[test]
    fn each_bad_field_is_refused_naming_it() {
        assert_eq!(
            field(ShipRequest::try_from(&request(galactic(), [0.0; 3], 3))),
            bad("time_rate")
        );
        let mut late = request(galactic(), [0.0; 3], 1);
        late.ship.time.seconds = i64::MAX;
        assert_eq!(field(ShipRequest::try_from(&late)), bad("ship.time"));
        let mut malformed = request(galactic(), [0.0; 3], 1);
        malformed.ship.time.nanos = 1_000_000_000;
        assert_eq!(field(ShipRequest::try_from(&malformed)), bad("ship.time"));
        for velocity in [
            [f64::NAN, 0.0, 0.0],
            [SPEED_OF_LIGHT, 0.0, 0.0],
            [2.5e8, 2.5e8, 0.0],
        ] {
            assert_eq!(
                field(ShipRequest::try_from(&request(galactic(), velocity, 1))),
                bad("ship.velocity_m_s")
            );
        }
        let outside = FramePositionDto::Galactic {
            position: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 900_000, 0],
                offset_m: [0.0; 3],
            },
        };
        assert_eq!(
            field(ShipRequest::try_from(&request(outside, [0.0; 3], 1))),
            bad(POSITION)
        );
        let not_finite = FramePositionDto::System {
            system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
            offset_m: [f64::INFINITY, 0.0, 0.0],
        };
        assert_eq!(
            field(ShipRequest::try_from(&request(not_finite, [0.0; 3], 1))),
            bad(POSITION)
        );
        let bad_index = FramePositionDto::Body {
            body: BodyIdHex::from_parts(0x0200_0800_2000_0000, 0xFFFF),
            offset_m: [0.0; 3],
        };
        assert_eq!(
            field(ShipRequest::try_from(&request(bad_index, [0.0; 3], 1))),
            bad(POSITION)
        );
    }
}
