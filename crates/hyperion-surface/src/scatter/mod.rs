//! What R11 scatters over the ground: the authoritative rocks (plan R11, Design notes 4 and 5),
//! and the per-patch layers it adds to R10's bake through [`BakeHook`](crate::bake::BakeHook).
//!
//! # The authoritative rocks
//!
//! Every rigid instance 0.2 m tall or more is authoritative, placed alike by the server and the
//! client and answered for at contact ([`AUTHORITATIVE_ROCK_HEIGHT`], the rendering brainstorm's
//! open question 11). A rock's exposed height is drawn per rock as a share of its diameter from
//! [`ROCK_HEIGHT_RATIO`], so authority goes by diameter: every rock of
//! [`AUTHORITATIVE_ROCK_DIAMETER`] or wider, a superset of those 0.2 m tall that keeps the split a
//! diameter threshold, and the decorative scatter owns only the narrower ones, so no rock is in
//! both.
//!
//! - [`law`]: Golombek and Rapp 1997's size–frequency law, the number of rocks a rock abundance k
//!   gives, in closed form through the exponential integral.
//! - [`site`]: what decides k at a point, from the material, the surface's age and atmosphere and
//!   a coarse crater's ejecta, every input one both sides hold.
//! - [`place`]: where the rocks lie, drawn by hash in cells per diameter octave.
//! - [`shape`]: each rock's superquadric, half buried on the ground, and its top along a normal.
//! - [`contact`]: the rocks near a point, and the ground with its rocks there, which collision
//!   reads.
//! - [`hold`]: the rocks are placed from generator version 27 ([`ROCKS_VERSION`]); below it the
//!   crate places none.
//!
//! # The layers
//!
//! R10.T6.a declares the layer's type and nothing more, so that the bake can carry R11's layers in
//! the same bake and cache before R11 lands; R11 owns the module and gives the type its variants
//! (its `Rocks(Vec<Rock>)`, R11.T4.a) and the hook its implementation (`RockBakeHook`).

pub mod contact;
#[cfg(test)]
mod golden;
pub mod hold;
pub mod law;
pub mod place;
pub mod shape;
pub mod site;

use hyperion_base::units::Metres;

pub use contact::{RockCache, rocks_near, surface_height_with_rocks};
pub use hold::{ROCKS_VERSION, RockRules};
#[cfg(any(test, feature = "testing"))]
pub use hold::{with_rock_rules, with_rocks_placed};
pub use law::{MAX_ROCK_ABUNDANCE, RockAbundance, exponential_integral_e1};
pub use place::{
    REACH_MARGIN, ROCK_OCTAVE_COUNT, ROCK_OCTAVES, RockCellKey, RockError, rock_octave_levels,
    rocks_in_cell,
};
pub use shape::{MAX_REACH_SHARE, ROCK_ASPECT, ROCK_EXPONENT, Rock, RockShape};
pub use site::{
    AIRLESS_PRESSURE, AtmosphereClass, CraterEjecta, RockSite, atmosphere_class, rock_abundance,
    rock_site,
};

/// The height at and above which a rigid instance must be authoritative: 0.2 m, the rendering
/// brainstorm's ruling on its open question 11, below the 0.3 m hazard that landing-hazard
/// detection is specified against (Epp and Smith 2007, for NASA's ALHAT), since a rock is a hazard
/// well below a gear's relief tolerance (Apollo's lunar module's engine skirt cleared about
/// 0.34 m, NASA TN D-6850). A generator constant.
pub const AUTHORITATIVE_ROCK_HEIGHT: Metres = Metres::new(0.2);

/// The range of a rock's exposed height over its diameter, drawn per rock: 0.25 to 0.50, about
/// the 0.29–0.41 measured at the landing sites and capped at the hazard convention of 0.5
/// (Design note 4).
pub const ROCK_HEIGHT_RATIO: (f64, f64) = (0.25, 0.50);

/// The diameter from which every rock is authoritative: [`AUTHORITATIVE_ROCK_HEIGHT`] over the
/// largest drawn height ratio, 0.2 m ÷ 0.5 = 0.4 m (Design note 4), so that every rock 0.2 m tall
/// is at least this wide.
pub const AUTHORITATIVE_ROCK_DIAMETER: Metres =
    Metres::new(AUTHORITATIVE_ROCK_HEIGHT.value() / ROCK_HEIGHT_RATIO.1);

/// A per-patch layer R11 adds to a patch's bake ([`TerrainLayers::extra`]), through a
/// [`BakeHook`](crate::bake::BakeHook).
///
/// Empty until R11 gives it its variants: no layer exists yet, so every bake's list is empty. It
/// derives only what a bake needs and R11's rock list, of `f64` positions, can keep: `Eq`, `Hash`
/// and `Ord` would have to be withdrawn when its first variant lands.
///
/// [`TerrainLayers::extra`]: crate::bake::TerrainLayers::extra
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExtraLayer {}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The authoritative diameter is the height over the ratio's upper bound, exactly 0.4 m, and
    /// the ratio's range is the plan's.
    #[test]
    fn the_authoritative_diameter_is_the_height_over_the_largest_ratio() {
        assert_same_bits(
            AUTHORITATIVE_ROCK_DIAMETER.value(),
            AUTHORITATIVE_ROCK_HEIGHT.value() / ROCK_HEIGHT_RATIO.1,
        );
        assert_same_bits(AUTHORITATIVE_ROCK_DIAMETER.value(), 0.4);
        assert_same_bits(ROCK_HEIGHT_RATIO.0, 0.25);
        assert_same_bits(ROCK_HEIGHT_RATIO.1, 0.5);
    }
}
