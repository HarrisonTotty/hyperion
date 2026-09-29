//! Bearings: the local direction from one point to another, in the named directions at the first
//! (plan 12, P12.T2; Design note 12).

use crate::coords::{GalacticPosition, UnitVector};
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Degrees, Radians};

/// How close to the galactic z axis, in light-years, a bearing stops using the local named
/// directions: within it coreward turns through a full circle across a light-year, so azimuth runs
/// from +x instead (Design note 12).
pub const AXIS_FRAME_RADIUS_LY: f64 = 1.0;

/// What a bearing's azimuth is measured from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BearingFrame {
    /// The local named directions: azimuth from coreward through spinward.
    #[default]
    Local,
    /// Within [`AXIS_FRAME_RADIUS_LY`] of the z axis, where coreward is undefined: azimuth from
    /// the galactic +x axis through +y, which the wire calls `galactic_x`.
    GalacticX,
}

/// A direction at an observer: azimuth in the galactic plane's local frame and elevation above
/// it (plan 12's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bearing {
    azimuth: Degrees,
    elevation: Degrees,
    frame: BearingFrame,
}

impl Bearing {
    /// The azimuth, in [0°, 360°): 0° coreward, 90° spinward, 180° rimward, 270° antispinward;
    /// from +x through +y in the [`BearingFrame::GalacticX`] frame.
    #[must_use]
    pub const fn azimuth(&self) -> Degrees {
        self.azimuth
    }

    /// The elevation, in [−90°, 90°], positive towards galactic north.
    #[must_use]
    pub const fn elevation(&self) -> Degrees {
        self.elevation
    }

    /// What the azimuth is measured from.
    #[must_use]
    pub const fn frame(&self) -> BearingFrame {
        self.frame
    }
}

/// The bearing of `to` seen from `from`, or `None` if the two are the same point, which has no
/// direction.
///
/// Azimuth runs from coreward through spinward and elevation is positive north, all at `from`
/// (Design note 12). Within a light-year of the z axis azimuth runs from +x through +y instead,
/// and the bearing says so ([`BearingFrame::GalacticX`]). The plan's sketch takes the galaxy too;
/// the directions are pure geometry, so this does not (P12.T2 as built).
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::observe::bearing;
///
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let centre = GalacticPosition::ORIGIN;
/// let to_centre = bearing(&sun, &centre).ok_or("two points")?;
/// // The centre is dead ahead coreward, in the plane.
/// assert!(to_centre.azimuth().value().abs() < 1e-9);
/// assert!(to_centre.elevation().value().abs() < 1e-9);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn bearing(from: &GalacticPosition, to: &GalacticPosition) -> Option<Bearing> {
    let displacement = from.displacement_to(to);
    if displacement.length().value() <= 0.0 {
        return None;
    }
    let d = displacement.metres();
    let near_axis =
        from.to_cylindrical().radius().value() < AXIS_FRAME_RADIUS_LY * METRES_PER_LIGHT_YEAR;
    let (forward, left, frame) = match from.directions() {
        Some(directions) if !near_axis => (
            directions.coreward(),
            directions.spinward(),
            BearingFrame::Local,
        ),
        Some(_) | None => (UnitVector::X, UnitVector::Y, BearingFrame::GalacticX),
    };
    let dot = |u: &UnitVector| {
        let c = u.components();
        c[0] * d[0] + c[1] * d[1] + c[2] * d[2]
    };
    let along = dot(&forward);
    let across = dot(&left);
    let up = d[2];
    let mut azimuth = Degrees::from(Radians::new(math::atan2(across, along))).value();
    if azimuth < 0.0 {
        azimuth += 360.0;
    }
    if azimuth >= 360.0 {
        azimuth = 0.0;
    }
    let horizontal = (along * along + across * across).sqrt();
    let elevation = Degrees::from(Radians::new(math::atan2(up, horizontal))).value();
    Some(Bearing {
        azimuth: Degrees::new(azimuth),
        elevation: Degrees::new(elevation),
        frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticDisplacement;

    fn at_ly(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).unwrap()
    }

    fn step(from: &GalacticPosition, unit: [f64; 3], ly: f64) -> GalacticPosition {
        from.translated(GalacticDisplacement::new(
            unit.map(|c| c * ly * METRES_PER_LIGHT_YEAR),
        ))
        .unwrap()
    }

    fn assert_bearing(b: Bearing, azimuth: f64, elevation: f64, frame: BearingFrame) {
        assert_eq!(b.frame(), frame);
        let az = b.azimuth().value();
        let gap = (az - azimuth).abs().min(360.0 - (az - azimuth).abs());
        assert!(gap < 1e-6, "azimuth {az} against {azimuth}");
        assert!(
            (b.elevation().value() - elevation).abs() < 1e-6,
            "elevation {:?} against {elevation}",
            b.elevation()
        );
        assert!((0.0..360.0).contains(&az));
    }

    /// P12.T2: the bearings of the six named directions, from a point off the axis where they are
    /// turned 30° from the galactic axes.
    #[test]
    fn bearing_gives_the_six_named_directions_their_angles() {
        let from = at_ly([13_000.0, 22_516.66, 150.0]);
        let directions = from.directions().unwrap();
        let in_plane = [
            (directions.coreward(), 0.0),
            (directions.spinward(), 90.0),
            (directions.rimward(), 180.0),
            (directions.antispinward(), 270.0),
        ];
        let vertical = [(directions.north(), 90.0), (directions.south(), -90.0)];
        for distance in [0.001, 1.0, 5_000.0] {
            for (unit, azimuth) in in_plane {
                let to = step(&from, unit.components(), distance);
                assert_bearing(
                    bearing(&from, &to).unwrap(),
                    azimuth,
                    0.0,
                    BearingFrame::Local,
                );
            }
            for (unit, elevation) in vertical {
                // Straight up or down the azimuth is that of a vanishing horizontal part.
                let to = step(&from, unit.components(), distance);
                let b = bearing(&from, &to).unwrap();
                assert!((b.elevation().value() - elevation).abs() < 1e-6, "{b:?}");
            }
        }
        // Coreward and 45° up: half-way between the plane and north.
        let c = directions.coreward().components();
        let to = step(&from, [c[0], c[1], 1.0], 100.0);
        assert_bearing(bearing(&from, &to).unwrap(), 0.0, 45.0, BearingFrame::Local);
    }

    /// Within a light-year of the axis the azimuth runs from +x through +y (Design note 12).
    #[test]
    fn bearing_near_the_axis_runs_from_galactic_x() {
        for from in [GalacticPosition::ORIGIN, at_ly([0.3, -0.4, 200.0])] {
            let east = step(&from, [1.0, 0.0, 0.0], 10.0);
            let north_of = step(&from, [0.0, 1.0, 0.0], 10.0);
            assert_bearing(
                bearing(&from, &east).unwrap(),
                0.0,
                0.0,
                BearingFrame::GalacticX,
            );
            assert_bearing(
                bearing(&from, &north_of).unwrap(),
                90.0,
                0.0,
                BearingFrame::GalacticX,
            );
        }
        assert_eq!(
            bearing(&GalacticPosition::ORIGIN, &GalacticPosition::ORIGIN),
            None
        );
    }
}
