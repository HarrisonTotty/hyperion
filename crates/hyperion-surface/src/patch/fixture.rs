//! Height sources for the patch tests: smooth, cheap, and able to fail or record their queries.

use std::cell::RefCell;

use super::HeightSource;
use crate::cube::PatchKey;
use crate::height::HeightSample;
use crate::spheroid::Spheroid;

/// A smooth height of the spheroid point P, `base + amp (x y + 0.3 z²) ÷ a²`, metres, the same
/// at every level unless `per_level` adds `level` metres; it can refuse a direction and records
/// the levels it is asked for.
#[derive(Debug)]
pub(crate) struct Fixture {
    pub(crate) figure: Spheroid,
    pub(crate) base_m: f64,
    pub(crate) amp_m: f64,
    pub(crate) per_level: bool,
    pub(crate) refuse_z_above: Option<f64>,
    pub(crate) levels: RefCell<Vec<u8>>,
}

impl Fixture {
    /// A constant height `h_m` on `figure`.
    pub(crate) fn constant(figure: Spheroid, h_m: f64) -> Self {
        Self {
            figure,
            base_m: h_m,
            amp_m: 0.0,
            per_level: false,
            refuse_z_above: None,
            levels: RefCell::new(Vec::new()),
        }
    }

    /// A smooth relief of amplitude `amp_m` on `figure`, whose levels differ by a metre each.
    pub(crate) fn relief(figure: Spheroid, amp_m: f64) -> Self {
        Self {
            amp_m,
            per_level: true,
            ..Self::constant(figure, 100.0)
        }
    }
}

/// The fixture's refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refused;

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the fixture refuses this direction")
    }
}

impl std::error::Error for Refused {}

impl HeightSource for Fixture {
    type Cache = ();
    type Error = Refused;

    fn figure(&self) -> Spheroid {
        self.figure
    }

    fn height(&self, _cache: &mut (), dir: [f64; 3], level: u8) -> Result<HeightSample, Refused> {
        self.levels.borrow_mut().push(level);
        if self.refuse_z_above.is_some_and(|z| dir[2] > z) {
            return Err(Refused);
        }
        let p = self.figure.point(dir);
        let a2 = self.figure.equatorial_radius_m * self.figure.equatorial_radius_m;
        let shift = if self.per_level {
            f64::from(level)
        } else {
            0.0
        };
        let height_m = self.base_m + shift + self.amp_m * (p[0] * p[1] + 0.3 * p[2] * p[2]) / a2;
        let gradient = [
            self.amp_m * p[1] / a2,
            self.amp_m * p[0] / a2,
            self.amp_m * 0.6 * p[2] / a2,
        ];
        Ok(HeightSample { height_m, gradient })
    }

    fn level_bound_m(&self, _: u8) -> f64 {
        1.0
    }

    fn height_range_m(&self, _: PatchKey) -> (f64, f64) {
        (self.base_m - self.amp_m, self.base_m + self.amp_m + 30.0)
    }
}
