// The disc regime's two draws (plan R07, T8.a and T9; Design note 2): one screen rectangle the CPU
// bounds, its fragments shaded by `disc_pixel`. Composed after frame.wgsl, litBody.wgsl and
// bodyDisc.wgsl.
//
// A disc body's rectangle lies at depth 0, at infinity under reversed-Z, and writes none: the
// painter's order by power stands for depth, and a disc that overlaps geometry writing depth is
// promoted to the mesh regime. A mesh body draws its limb with the same second draw (T9), its
// rectangle's corners on the limb's plane (the polar plane of the camera, which holds the
// silhouette of a spheroid), so that the depth test orders the limb against other geometry that
// writes depth, while it still writes none. Over a handed-over body's surveyed terrain (plan R10,
// R10.T10.c) the limb leaves the ground its survey holds to the terrain, as its figure does
// (`smoothMesh.wgsl`): a limb pixel over a cell the body's coverage mask holds surveyed is
// discarded. A disc body, and a mesh body without a survey, bind a mask of no surveyed cell.

struct Draw {
  // Unused: each disc carries its own centre.
  offsetFromCameraM : vec3f,
  // The disc's index in `discs`.
  disc : u32,
  // 0 for the wholly covered pixels, 1 for the limb's.
  edgePass : u32,
  // The rectangle's reversed-Z depth at its corners (left, top), (right, top), (left, bottom) and
  // (right, bottom): 0 for a disc body; a mesh body's limb plane's.
  depths : vec4f,
  // The coverage mask's axes, the body's own body-fixed axes (its rotation's), into the galactic
  // axes, as `mat4x4f`; any where the body has no survey (R10.T10.c).
  coverageAxes : mat4x4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The `disc cells` pass's sums, which a disc under 32 px reads, and a larger disc's limb and its
// face's near-limb band (R07.T8.d, T8.f and T8.g; `pixel_cells`).
@group(2) @binding(5) var<storage, read> cell_sums : array<vec4f>;

// The body's coverage mask (plan R10, `terrain/coverage.ts`' `CoverageMask`), as `smoothMesh.wgsl`
// binds it: one `r8unorm` layer a face of R05's cube sphere, 2ᴸ × 2ᴸ texels at the coarse field's
// level L, 1 where the cell is surveyed; a 1 × 1 layer of 0 a face where the body has no survey.
@group(2) @binding(13) var coverage_mask : texture_2d_array<f32>;

struct DiscVarying {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) corner : vec3f) -> DiscVarying {
  let rect = disc_row(0u);
  let px = mix(rect.xy, rect.zw, corner.xy);
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  var out : DiscVarying;
  out.position = vec4f(ndc, draw.depths[u32(corner.x) + 2u * u32(corner.y)], 1.0);
  return out;
}

// Whether the limb pixel whose centre is `position` lies over a cell the coverage mask holds
// surveyed, where the terrain draws the ground (R10.T10.c): its ray against the analytic spheroid
// in the scaled space, as `hit_spheroid` takes it, the hit or, where the ray passes outside, the
// spheroid's point nearest it, whose direction is the point's on R05's cube sphere; then that
// direction's texel of the mask in the body-fixed axes, as `smoothMesh.wgsl`'s `surveyed_beneath`.
fn limb_surveyed(position : vec2f) -> bool {
  let body = body_of();
  let scaled = normalize(stretch_along(body, view_ray(floor(position) + 0.5)));
  // From the centre to the ray's nearest point, then back along the ray to the near surface.
  let nearest = dot(scaled, body.scaled_centre) * scaled - body.scaled_centre;
  let h2 = body.radius * body.radius - dot(nearest, nearest);
  let d = normalize(select(nearest, nearest - sqrt(max(h2, 0.0)) * scaled, h2 >= 0.0));
  let x = to_view(draw.coverageAxes[0].xyz);
  let y = to_view(draw.coverageAxes[1].xyz);
  let z = to_view(draw.coverageAxes[2].xyz);
  let texel = class_map_texel(vec3f(dot(d, x), dot(d, y), dot(d, z)), textureDimensions(coverage_mask).x);
  return textureLoad(coverage_mask, texel.xy, texel.z, 0).r >= 0.5;
}

@fragment
fn fragmentMain(v : DiscVarying) -> @location(0) vec4f {
  // WGSL's `discard` demotes the invocation to a helper, which runs on to the stage's end (Tint
  // emulates it so on Metal and Direct3D, with a flag that discards at the end): a discarded
  // fragment would shade in full. So the light is formed only where the fragment is kept, in a loop
  // of at most one pass, not an `if`, as the composite's reads are (`composite.wgsl`): SwiftShader,
  // the checks' adapter, runs both sides of a branch under its lanes' mask, and a loop only while a
  // lane is in it. The pixels kept draw what they drew, bit for bit.
  // A limb pixel surveyed beneath keeps `pixel`'s zeros, not drawn.
  var pixel : DiscPixel;
  for (var shade = !(draw.edgePass != 0u && limb_surveyed(v.position.xy)); shade; shade = false) {
    pixel = disc_pixel(v.position.xy, draw.edgePass);
  }
  if (!pixel.drawn) {
    discard;
  }
  return pixel.colour;
}
