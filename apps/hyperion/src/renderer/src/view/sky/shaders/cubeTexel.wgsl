// The star cube's face and texel of a direction (plan R06, Design note 21, T13.g): `cube.ts`'s
// `cubeTexelOf` in WGSL, computed alike to the bit. Faces are WebGPU's layer order, +X, −X, +Y,
// −Y, +Z, −Z, row 0 a face's top. A texel along an axis is the largest c with
// c × 2m ≤ (a + m) × size, each product and sum rounded to f32: an estimate by division (which
// WGSL may round within 2.5 ulp) corrected by those comparisons alone (correctly rounded).

struct CubeTexel {
  face : u32,
  column : u32,
  row : u32,
}

fn texelAlong(a : f32, major : f32, size : f32) -> u32 {
  let span = 2.0 * major;
  let scaled = (a + major) * size;
  var texel = clamp(floor(scaled / span), 0.0, size - 1.0);
  for (var i = 0; i < 2; i += 1) {
    if (texel > 0.0 && texel * span > scaled) {
      texel -= 1.0;
    }
  }
  for (var i = 0; i < 2; i += 1) {
    if (texel + 1.0 < size && (texel + 1.0) * span <= scaled) {
      texel += 1.0;
    }
  }
  return u32(texel);
}

fn cubeTexelOf(d : vec3f, size : f32) -> CubeTexel {
  let a = abs(d);
  var face : u32;
  var u : f32;
  var v : f32;
  var major : f32;
  if (a.x >= a.y && a.x >= a.z) {
    major = a.x;
    face = select(1u, 0u, d.x >= 0.0);
    u = select(d.z, -d.z, d.x >= 0.0);
    v = -d.y;
  } else if (a.y >= a.z) {
    major = a.y;
    face = select(3u, 2u, d.y >= 0.0);
    u = d.x;
    v = select(-d.z, d.z, d.y >= 0.0);
  } else {
    major = a.z;
    face = select(5u, 4u, d.z >= 0.0);
    u = select(-d.x, d.x, d.z >= 0.0);
    v = -d.y;
  }
  return CubeTexel(face, texelAlong(u, major, size), texelAlong(v, major, size));
}
