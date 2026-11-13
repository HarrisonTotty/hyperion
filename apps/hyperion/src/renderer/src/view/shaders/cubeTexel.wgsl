// A body-fixed direction's texel on R05's cube sphere (plan R10, R10.T10.d; moved out of
// bodyDisc.wgsl by R10.T11): its face, its (u, v) there and its warped cell coordinates, the lookup
// of a class map's texels and of a survey's GPU coverage mask. A library with no binding, composed
// by concatenation before bodyDisc.wgsl (R07's lit bodies) and before wireCoverage.wgsl (the
// wireframe's occluder and lines over a body's surveyed terrain). The TypeScript twin is
// `bodies/discSurface.ts`' `classMapTexelOf`.

// S2's quadratic warp from a face coordinate u ∈ [−1, 1] to a cell coordinate s ∈ [0, 1]
// (`uvToSt`, R05's cube sphere).
fn uv_to_st(u : f32) -> f32 {
  if (u >= 0.0) {
    return 0.5 * sqrt(1.0 + 3.0 * u);
  }
  return 1.0 - 0.5 * sqrt(1.0 - 3.0 * u);
}

// A cell coordinate as a texel index of n.
fn texel_index(s : f32, n : u32) -> u32 {
  return u32(clamp(floor(s * f32(n)), 0.0, f32(n - 1u)));
}

// A body-fixed direction's face of R05's cube sphere and its (u, v) there.
struct CubeFaceUv {
  face : u32,
  uv : vec2f,
}

// R05's face of the largest |component| of `p` (ties to the lowest face: +x 0, +y 1, +z 2, −x 3,
// −y 4, −z 5) and its (u, v) there (`xyzToFaceUv`).
fn cube_face_uv(p : vec3f) -> CubeFaceUv {
  let faces = vec3u(select(3u, 0u, p.x > 0.0), select(4u, 1u, p.y > 0.0), select(5u, 2u, p.z > 0.0));
  let size = abs(p);
  var face = faces.x;
  var largest = size.x;
  for (var axis = 1u; axis < 3u; axis = axis + 1u) {
    if (size[axis] > largest || (size[axis] == largest && faces[axis] < face)) {
      face = faces[axis];
      largest = size[axis];
    }
  }
  var uv : vec2f;
  switch face {
    case 0u: { uv = vec2f(p.y / p.x, p.z / p.x); }
    case 1u: { uv = vec2f(-p.x / p.y, p.z / p.y); }
    case 2u: { uv = vec2f(-p.x / p.z, -p.y / p.z); }
    case 3u: { uv = vec2f(p.z / p.x, p.y / p.x); }
    case 4u: { uv = vec2f(p.z / p.y, -p.x / p.y); }
    default: { uv = vec2f(-p.y / p.z, -p.x / p.z); }
  }
  var out : CubeFaceUv;
  out.face = face;
  out.uv = uv;
  return out;
}

// The texel of an n × n grid a face holding a body-fixed direction: (i, j, face), its (u, v)
// warped and scaled by n (`classMapTexelOf`), with no gutter (R10.T10.c's coverage mask).
fn class_map_texel(p : vec3f, n : u32) -> vec3u {
  let at = cube_face_uv(p);
  return vec3u(texel_index(uv_to_st(at.uv.x), n), texel_index(uv_to_st(at.uv.y), n), at.face);
}
