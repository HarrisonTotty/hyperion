// The bake's splat (plan R06, Design note 21, T13.g): each baked star a point at its texel's centre
// on one face, adding its illuminance per channel and a count of 1 into an rgba32float scratch
// face. Composed after cubeTexel.wgsl.
//
// The points buffer: element 0 the header (face, face side in texels, tile side in texels, tile);
// then two vec4f a star, its direction (x, y, z, 0) and its illuminance (r, g, b, 1) in lx, as
// `splatCpu.ts` lays them out after the header. The target is one tile of the face, the tiles
// numbered row after row from the face's top left; a whole face is its one tile 0. A star on another
// face or tile is placed outside the clip volume.

@group(0) @binding(0) var<storage, read> points : array<vec4f>;

struct SplatOut {
  @builtin(position) position : vec4f,
  @location(0) @interpolate(flat) light : vec4f,
}

@vertex
fn main(@builtin(vertex_index) index : u32) -> SplatOut {
  let header = points[0];
  let size = header.y;
  let tile = header.z;
  // In integers, where a division is exact.
  let tiles = u32(size) / u32(tile);
  let origin = vec2i(vec2u(u32(header.w) % tiles, u32(header.w) / tiles) * u32(tile));
  let direction = points[1u + 2u * index].xyz;
  let light = points[2u + 2u * index];
  let texel = cubeTexelOf(direction, size);
  let local = vec2i(i32(texel.column), i32(texel.row)) - origin;
  var out : SplatOut;
  if (texel.face != u32(header.x) || any(local < vec2i(0)) || any(local >= vec2i(i32(tile)))) {
    out.position = vec4f(2.0, 2.0, 0.5, 1.0);
    out.light = vec4f(0.0);
    return out;
  }
  let x = (f32(local.x) + 0.5) / tile * 2.0 - 1.0;
  let y = 1.0 - (f32(local.y) + 0.5) / tile * 2.0;
  out.position = vec4f(x, y, 0.5, 1.0);
  out.light = light;
  return out;
}
