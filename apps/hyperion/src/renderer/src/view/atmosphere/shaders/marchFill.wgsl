// The ray march's target outside the air's area (plan R08, R08.T11.d), for a reader of the whole
// target (`hillaire.ts`'s frameTables, the smoke page's read-backs): (0, 0, 0, 1), what the march
// writes where its ray misses the air, on every texel of the target's share outside the area, which
// the march leaves as it was and the composite does not read. Follows `marchArea.wgsl`.

@group(0) @binding(0) var<uniform> marchArea : MarchArea;
@group(0) @binding(1) var rayMarchOut : texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = vec2u(marchArea.bound.zw);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let first = vec2u(marchArea.tiles.xy) * MARCH_TILE_TEXELS;
  let end = first + vec2u(marchArea.tiles.zw) * MARCH_TILE_TEXELS;
  if (all(id.xy >= first) && all(id.xy < end)) {
    return;
  }
  textureStore(rayMarchOut, id.xy, vec4f(0.0, 0.0, 0.0, 1.0));
}
