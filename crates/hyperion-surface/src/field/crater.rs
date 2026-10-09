//! The coarse craters: the craters of the boundary diameter `D_b` and wider, which the coarse pass
//! places and smooths into the elevation, and which the field lists (plan R09, Design notes 10 and
//! 12).

use hyperion_base::units::{Gigayears, Metres};

use super::cover::Cover;

coded_enum! {
    /// A crater's morphology, one byte, by its diameter D against the body's simple-to-complex
    /// transition diameter `D_t` = 19 km × (1.62 m s⁻² ÷ g) × `k_target` (Design note 12, after Pike
    /// 1980's Table 3, 19 km on the Moon), a zone rather than a line. The zone's bounds, 0.8, 1.5,
    /// 9 and 16 `D_t`, are Design note 12's, which cites no source for them; R09.T7.a checks them.
    Morphology {
        /// A bowl, below 0.8 `D_t`.
        Simple = 0,
        /// Between a bowl and a complex crater, 0.8 `D_t` to 1.5 `D_t`.
        Transitional = 1,
        /// A complex crater with a flat floor and a central peak, above 1.5 `D_t`.
        CentralPeak = 2,
        /// A peak-ring basin, from about 9 `D_t`.
        PeakRing = 3,
        /// A multi-ring basin, above about 16 `D_t`.
        MultiRingBasin = 4,
    }
}

/// One crater of the field's list: `D_b` and wider (Design note 10).
///
/// Plain data with public fields, which [`CoarseField::new`](super::CoarseField::new) checks
/// against its field: a finite unit centre, a diameter of at least `D_b`, a finite non-negative age,
/// and a reach that holds its centre's cell and no cell beyond the field's.
#[derive(Debug, Clone, PartialEq)]
pub struct CoarseCrater {
    /// The crater's centre, a unit vector in the body-fixed frame.
    pub centre: [f64; 3],
    /// The rim-to-rim diameter, metres, at least the field's boundary diameter.
    pub diameter: Metres,
    /// The morphology its diameter gives it.
    pub morphology: Morphology,
    /// When it formed, thousands of millions of years before the present.
    pub age: Gigayears,
    /// The share of its fresh rim relief that degradation has removed since, in steps of
    /// 1 ÷ 255: 0 is fresh and 255 a rim worn away (Design note 10: degradation by age).
    pub degradation: u8,
    /// Every cell of the field's level that the crater's reach touches, its ejecta reaching 2.54
    /// rim radii, 1.27 D, from its centre (Design note 13), so that a block carrying a cell
    /// carries every crater that reaches it. A cover of
    /// [`ResolutionCode::NONE`](super::ResolutionCode::NONE).
    pub reach: Cover,
}

impl CoarseCrater {
    /// The share of the fresh rim relief that degradation has removed, 0 to 1.
    #[must_use]
    pub fn degradation_fraction(&self) -> f64 {
        f64::from(self.degradation) / 255.0
    }
}
