//! The system and body frames: `f64` metres from an origin that moves with the object.
//!
//! Both frames are translations of the galactic frame: axes parallel to the galactic axes, origin
//! at the system's barycentre or the body's centre. Rotating body-fixed frames are not these.
//!
//! Every conversion takes the frame's origin explicitly, and no arithmetic mixes the frames:
//!
//! ```compile_fail
//! use hyperion_sim::coords::{BodyPosition, SystemPosition};
//! let _ = SystemPosition::new([1.0, 0.0, 0.0]) + BodyPosition::new([1.0, 0.0, 0.0]);
//! ```

use super::galactic::{GalacticDisplacement, GalacticPosition};
use super::vec3;
use crate::id::{BodyId, SystemId};
use crate::units::Metres;

/// The frame a position is expressed in: galactic, a system's, or a body's.
///
/// A ship is always in exactly one frame and changes frame at defined boundaries: entering a
/// system's sphere of influence, entering a body's. The positions of each frame have their own
/// type, [`GalacticPosition`], [`SystemPosition`] and [`BodyPosition`]; this enum names which one
/// applies and whose origin it is measured from. Which frame a ship is in is decided by the
/// frame-selection rule, which needs the systems around the ship and so lives with the range query.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::Frame;
/// use hyperion_sim::id::{BodyId, SystemId};
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let frames = [Frame::Galactic, Frame::System(system), Frame::Body(BodyId::new(system, 1))];
/// assert!(frames.iter().all(|f| *f == Frame::Galactic || f.system() == Some(system)));
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Frame {
    /// The galactic frame: a light-year cell and a metre offset from the galactic centre.
    Galactic,
    /// A system's frame: metres from the system's barycentre, along the galactic axes.
    System(SystemId),
    /// A body's frame: metres from the body's centre, along the galactic axes.
    Body(BodyId),
}

impl Frame {
    /// The system whose frame this is or whose body's frame this is; `None` for the galactic
    /// frame.
    #[must_use]
    pub const fn system(self) -> Option<SystemId> {
        match self {
            Self::Galactic => None,
            Self::System(system) => Some(system),
            Self::Body(body) => Some(body.system()),
        }
    }
}

/// A position in a system's frame: `f64` metres from the barycentre, along the galactic axes.
///
/// Resolution is about 1 mm at 50 au.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, LyCell, SystemPosition};
///
/// let barycentre = GalacticPosition::new(LyCell::new([26_000, 0, 0]), [0.0; 3])?;
/// let planet = SystemPosition::new([1.5e11, 0.0, 0.0]);
/// let galactic = planet.to_galactic(&barycentre).expect("an au from a star is in range");
/// let back = SystemPosition::from_galactic(&galactic, &barycentre);
/// assert!((back.metres()[0] - 1.5e11).abs() < 1e-3);
/// # Ok::<(), hyperion_sim::coords::BuildGalacticPositionError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SystemPosition([f64; 3]);

impl SystemPosition {
    /// The barycentre.
    pub const ORIGIN: Self = Self([0.0; 3]);

    /// A position from its components in metres from the barycentre.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres from the barycentre.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The distance from the barycentre.
    #[must_use]
    pub fn distance_from_origin(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }

    /// The system-frame position of a galactic position, given the system's barycentre.
    #[must_use]
    pub fn from_galactic(position: &GalacticPosition, barycentre: &GalacticPosition) -> Self {
        Self(barycentre.displacement_to(position).metres())
    }

    /// The galactic position of this point, given the system's barycentre, or `None` if the
    /// result leaves the galactic frame's range.
    #[must_use]
    pub fn to_galactic(&self, barycentre: &GalacticPosition) -> Option<GalacticPosition> {
        barycentre.translated(GalacticDisplacement::new(self.0))
    }
}

/// A position in a body's frame: `f64` metres from the body's centre, along the galactic axes.
///
/// Resolution is sub-micrometre in low orbit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BodyPosition([f64; 3]);

impl BodyPosition {
    /// The body's centre.
    pub const ORIGIN: Self = Self([0.0; 3]);

    /// A position from its components in metres from the body's centre.
    #[must_use]
    pub const fn new(metres: [f64; 3]) -> Self {
        Self(metres)
    }

    /// The components in metres from the body's centre.
    #[must_use]
    pub const fn metres(&self) -> [f64; 3] {
        self.0
    }

    /// The distance from the body's centre.
    #[must_use]
    pub fn distance_from_origin(&self) -> Metres {
        Metres::new(vec3::length(self.0))
    }

    /// The body-frame position of a system-frame position, given the body's centre in the
    /// system frame.
    #[must_use]
    pub fn from_system(position: &SystemPosition, body_centre: &SystemPosition) -> Self {
        Self(vec3::sub(position.0, body_centre.0))
    }

    /// The system-frame position of this point, given the body's centre in the system frame.
    #[must_use]
    pub fn to_system(&self, body_centre: &SystemPosition) -> SystemPosition {
        SystemPosition(vec3::add(self.0, body_centre.0))
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::super::cell::LyCell;
    use super::*;

    #[test]
    fn system_and_galactic_conversions_invert_each_other() {
        let barycentre =
            GalacticPosition::new(LyCell::new([-40_000, 12, 7]), [123.0, 4.5e15, 0.0]).unwrap();
        let p = SystemPosition::new([7.5e12, -1.0e9, 3.0]);
        let g = p.to_galactic(&barycentre).unwrap();
        let back = SystemPosition::from_galactic(&g, &barycentre);
        for axis in 0..3 {
            assert!((back.metres()[axis] - p.metres()[axis]).abs() <= 2.0);
        }
        assert_same_bits(
            SystemPosition::from_galactic(&barycentre, &barycentre).metres()[1],
            0.0,
        );
        assert_same_bits(
            p.distance_from_origin().value(),
            (7.5e12_f64 * 7.5e12 + 1e18 + 9.0).sqrt(),
        );
    }

    #[test]
    fn frames_name_their_origin_owner() {
        let system = SystemId::from_raw(0xF000_0007_0000_0000).unwrap();
        let body = BodyId::new(system, 3);
        assert_eq!(Frame::Galactic.system(), None);
        assert_eq!(Frame::System(system).system(), Some(system));
        assert_eq!(Frame::Body(body).system(), Some(system));
        assert_ne!(Frame::System(system), Frame::Body(BodyId::new(system, 0)));
        assert!(Frame::Galactic < Frame::System(system));
    }

    #[test]
    fn body_and_system_conversions_invert_each_other() {
        let centre = SystemPosition::new([1.5e11, 2.0, -3.0]);
        let orbit = BodyPosition::new([0.0, 6.771e6, 0.0]);
        let in_system = orbit.to_system(&centre);
        assert_eq!(
            in_system,
            SystemPosition::new([1.5e11, 6.771e6 + 2.0, -3.0])
        );
        assert_eq!(BodyPosition::from_system(&in_system, &centre), orbit);
        assert_same_bits(orbit.distance_from_origin().value(), 6.771e6);
        assert_eq!(BodyPosition::ORIGIN.to_system(&centre), centre);
    }
}
