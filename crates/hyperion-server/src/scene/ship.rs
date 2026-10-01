//! The ship stand-in: a pose in a frame, moving in a straight line at its velocity in that frame,
//! until sessions supply a ship (rendering plan R03, Design note 2).

use hyperion_protocol::KinematicsDto;
use hyperion_sim::coords::{
    BodyPosition, GalacticDisplacement, GalacticPosition, SystemPosition, SystemVector,
};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Seconds;

use super::Ship;
use crate::convert::ShipRequest;

/// Where a ship is, in the frame it was set in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ShipPosition {
    /// In the galactic frame.
    Galactic(GalacticPosition),
    /// In a system's frame: metres from its barycentre, along the galactic axes.
    System {
        /// The system.
        system: SystemId,
        /// The offset from the barycentre.
        offset: SystemPosition,
    },
    /// In a body's non-rotating frame: metres from its centre, along the galactic axes.
    Body {
        /// The body.
        body: BodyId,
        /// The offset from the centre.
        offset: BodyPosition,
    },
}

/// The ship stand-in: `position` at `time`, moving at `velocity_m_s` relative to its frame's
/// origin, in a straight line in that frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShipStandIn {
    position: ShipPosition,
    velocity_m_s: [f64; 3],
    time: UniverseTime,
    /// The pose as the client set it, which the scene sends back.
    wire: KinematicsDto,
}

impl ShipStandIn {
    /// The stand-in at `position` at `time`, moving at `velocity_m_s`, which the wire states as
    /// `wire`. Every component must be finite, the speed below c and the time inside the clock
    /// window, as a checked [`ShipRequest`] carries them, by which the server builds every
    /// stand-in but the unset one.
    #[must_use]
    pub(super) fn new(
        position: ShipPosition,
        velocity_m_s: [f64; 3],
        time: UniverseTime,
        wire: KinematicsDto,
    ) -> Self {
        Self {
            position,
            velocity_m_s,
            time,
            wire,
        }
    }
}

impl From<ShipRequest> for ShipStandIn {
    /// The stand-in a checked `scene_ship` sets: its components finite, its speed below c and its
    /// time inside the clock window, so that two thousand years of motion cannot leave the
    /// addressable range.
    fn from(checked: ShipRequest) -> Self {
        Self::new(
            checked.position(),
            checked.velocity_m_s(),
            checked.time(),
            checked.into_wire(),
        )
    }
}

impl Ship for ShipStandIn {
    fn position_at(&self, t: UniverseTime) -> ShipPosition {
        let elapsed = Seconds::new(
            t.checked_since(self.time)
                .expect("two times inside the clock window are 2,000 years apart at most")
                .as_seconds_f64(),
        );
        let moved = self.velocity_m_s.map(|v| v * elapsed.value());
        match self.position {
            ShipPosition::Galactic(position) => ShipPosition::Galactic(
                position.translated(GalacticDisplacement::new(moved)).expect(
                    "below c for 2,000 years moves a position in the root cube by 2,000 ly, which \
                     stays far inside the addressable range",
                ),
            ),
            ShipPosition::System { system, offset } => ShipPosition::System {
                system,
                offset: offset.translated(SystemVector::new(moved)),
            },
            ShipPosition::Body { body, offset } => {
                let [x, y, z] = offset.metres();
                ShipPosition::Body {
                    body,
                    offset: BodyPosition::new([x + moved[0], y + moved[1], z + moved[2]]),
                }
            }
        }
    }

    fn velocity_m_s(&self) -> [f64; 3] {
        self.velocity_m_s
    }

    fn kinematics(&self) -> KinematicsDto {
        self.wire.clone()
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::FramePositionDto;
    use hyperion_sim::coords::LyCell;
    use hyperion_sim::time::Span;

    use super::*;

    fn wire() -> KinematicsDto {
        KinematicsDto {
            position: FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition::default(),
            },
            velocity_m_s: [0.0; 3],
            time: hyperion_protocol::UniverseTime::default(),
        }
    }

    #[test]
    fn the_stand_in_moves_in_a_straight_line_in_its_frame() {
        let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        let t0 = UniverseTime::new(100, 0).unwrap();
        let later = t0.checked_add(Span::from_seconds(10)).unwrap();
        let earlier = t0.checked_sub(Span::from_seconds(10)).unwrap();
        let in_system = ShipStandIn::new(
            ShipPosition::System {
                system,
                offset: SystemPosition::new([1.0e11, 0.0, 0.0]),
            },
            [1_000.0, -2_000.0, 0.5],
            t0,
            wire(),
        );
        assert_eq!(
            in_system.position_at(later),
            ShipPosition::System {
                system,
                offset: SystemPosition::new([1.0e11 + 10_000.0, -20_000.0, 5.0]),
            }
        );
        assert_eq!(
            in_system.position_at(earlier),
            ShipPosition::System {
                system,
                offset: SystemPosition::new([1.0e11 - 10_000.0, 20_000.0, -5.0]),
            }
        );
        let body = BodyId::new(system, 0x0100);
        let in_body = ShipStandIn::new(
            ShipPosition::Body {
                body,
                offset: BodyPosition::new([7.0e6, 0.0, 0.0]),
            },
            [0.0, 7_500.0, 0.0],
            t0,
            wire(),
        );
        assert_eq!(
            in_body.position_at(later),
            ShipPosition::Body {
                body,
                offset: BodyPosition::new([7.0e6, 75_000.0, 0.0]),
            }
        );
        assert_eq!(
            in_body.velocity_m_s().map(f64::to_bits),
            [0.0_f64, 7_500.0, 0.0].map(f64::to_bits)
        );
        assert_eq!(in_body.kinematics(), wire());
        let start = GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap();
        let galactic =
            ShipStandIn::new(ShipPosition::Galactic(start), [3.0e5, 0.0, 0.0], t0, wire());
        let ShipPosition::Galactic(moved) = galactic.position_at(later) else {
            panic!("a galactic stand-in stays galactic");
        };
        assert_eq!(
            start.displacement_to(&moved).metres().map(f64::to_bits),
            [3.0e6_f64, 0.0, 0.0].map(f64::to_bits)
        );
    }
}
