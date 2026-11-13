// The ray march's area and its tiles (plan R08, R08.T11.d; `decision-r08-t11-frame.md` section 2):
// the tiles of 8 x 8 texels, the march's workgroups, that the air's top shell can reach
// (`marchArea.ts`'s marchAreaOf, formed in f64), each tile's kind under the tile cull
// (`rayMarchCentre.wgsl`), and the texel an invocation takes. Prepended to the ray march's two
// entry points and to its fill (`marchFill.wgsl`); it declares nothing.

// A tile's side, texels (`marchArea.ts`'s MARCH_TILE_TEXELS).
const MARCH_TILE_TEXELS : u32 = 8u;

// A tile's kinds under the tile cull: every ray of every texel misses the top shell, widened, and
// no footprint can blend, so each texel is (0, 0, 0, 1); a texel's footprint may blend somewhere in
// it, so the footprint's entry point marches it; or neither, the centre ray alone, which the
// centre's entry point marches.
const MARCH_TILE_MISS : u32 = 0u;
const MARCH_TILE_CENTRE : u32 = 1u;
const MARCH_TILE_FOOTPRINT : u32 = 2u;

// The area of a frame's march (`hillaire.ts`'s marchArea uniform).
struct MarchArea {
  // x, y: the area's first tile on the target's grid; z, w: its tiles across and down (whole
  // numbers, as floats).
  tiles : vec4f,
  // x: 1 where the tile cull applies, 0 where every tile of the area takes the footprint's entry
  // point (the reference path, the whole frame); y: the widening of the shell and of the tile
  // cull's bounds, m, formed in f64 (`marchArea.ts`'s MARCH_AREA_WIDENING); z, w: the target's
  // size at the setting's scale, texels.
  bound : vec4f,
}

// How much farther than a half-resolution texel's own pixel one about its centre ray may stand and
// count as the same surface, a share of the distance (`tablesCpu.ts`'s MARCH_SAME_SURFACE_SHARE).
const MARCH_SAME_SURFACE_SHARE : f32 = 0.1;

// The scene's reversed-Z depth a march texel's centre ray through `uv` ends on (fix-disc-air-rows):
// at full resolution its own pixel's, the one holding the ray's point; at half, where the ray
// passes between four pixels' centres, 0 where its own pixel is sky, else the farthest of the four
// that stand within MARCH_SAME_SURFACE_SHARE of its own pixel's distance. A drawn body recedes from
// the camera across its disc, so that one pixel's depth stood nearer than the ray's own ground and
// ended the march in the air above it, in rows and columns of the march's tiles; on a surface
// convex towards the camera the farthest of the four stands at or beyond the ray's own point.
fn marchTexelDepthOf(depths : texture_depth_2d, uv : vec2f, scale : f32, full : vec2u) -> f32 {
  let at = uv * vec2f(full);
  let depth = textureLoad(depths, min(vec2u(at), full - 1u), 0);
  if (scale >= 1.0 || !(depth > 0.0)) {
    return depth;
  }
  let base = vec2i(floor(at - 0.5));
  var farthest = depth;
  for (var j = 0; j < 2; j++) {
    for (var i = 0; i < 2; i++) {
      let p = vec2u(clamp(base + vec2i(i, j), vec2i(0), vec2i(full) - 1));
      let d = textureLoad(depths, p, 0);
      // Reversed-Z depth is near ÷ distance: within the share, d (1 + share) is at least the own.
      if (d > 0.0 && d < farthest && d * (1.0 + MARCH_SAME_SURFACE_SHARE) >= depth) {
        farthest = d;
      }
    }
  }
  return farthest;
}

// The texel of the invocation at `local` in the workgroup at `group` of the dispatch over `area`.
fn marchTexelOf(area : MarchArea, group : vec2u, local : vec2u) -> vec2u {
  return (vec2u(area.tiles.xy) + group) * MARCH_TILE_TEXELS + local;
}

// The index of the dispatch's workgroup `group` among the area's tiles, row by row.
fn marchTileIndexOf(area : MarchArea, group : vec2u) -> u32 {
  return group.y * u32(area.tiles.z) + group.x;
}
