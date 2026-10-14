//! What R11 scatters over the ground and bakes with a patch (plan R11, Design notes 5 to 7): the
//! per-patch layers it adds to R10's bake through [`BakeHook`](crate::bake::BakeHook).
//!
//! R10.T6.a declares the layer's type and nothing more, so that the bake can carry R11's layers in
//! the same bake and cache before R11 lands; R11 owns the module and gives the type its variants
//! (its `Rocks(Vec<Rock>)`, R11.T4.a) and the hook its implementation (`RockBakeHook`).

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
