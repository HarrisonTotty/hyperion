//! Microlensing along one monitored line of sight (plan 12, P12.T4; brainstorm, "Orbits and
//! time": "Microlensing is a ray walked at query time along one monitored line of sight over real
//! moving objects").
//!
//! [`lenses_along`] walks the systems near the segment from an observer to a background source and
//! returns every one that passes within a given number of Einstein radii of the source's direction
//! during a window of observer time, with the point-lens light curve it causes
//! ([`point_lens_magnification`]). A lens is a point mass of its system's mass when the light
//! passes it, and its position is taken then, t − `D_l` ÷ c (Design note 13). A survey of lensing
//! over the whole sky is not offered, because its lenses could not be real objects; neither are
//! binary lenses or a lensing display. This is an API and its tests only.

mod point_lens;
mod walk;

pub use point_lens::{einstein_angle, point_lens_magnification};
pub use walk::{
    BuildLensQueryError, DEFAULT_LENS_CELL_BUDGET, DEFAULT_MAX_IMPACT, FindLensesError, LensCensus,
    LensEvent, LensQuery, LensQueryBuilder, LensResult, LensSightline, lens_mass_at, lenses_along,
    magnification_at,
};
