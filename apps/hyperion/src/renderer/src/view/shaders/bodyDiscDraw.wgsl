// The disc regime's two draws (plan R07, T8.a and T9; Design note 2): one screen rectangle the CPU
// bounds, its fragments shaded by `disc_pixel`. Composed after frame.wgsl, litBody.wgsl and
// bodyDisc.wgsl.
//
// A disc body's rectangle lies at depth 0, at infinity under reversed-Z, and writes none: the
// painter's order by power stands for depth, and a disc that overlaps geometry writing depth is
// promoted to the mesh regime. A mesh body draws its limb with the same second draw (T9), its
// rectangle's corners on the limb's plane (the polar plane of the camera, which holds the
// silhouette of a spheroid), so that the depth test orders the limb against other geometry that
// writes depth, while it still writes none.

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
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The `disc cells` pass's sums, which a disc under 32 px reads (R07.T8.d; `pixel_cells`).
@group(2) @binding(5) var<storage, read> cell_sums : array<vec4f>;

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

@fragment
fn fragmentMain(v : DiscVarying) -> @location(0) vec4f {
  let pixel = disc_pixel(v.position.xy, draw.edgePass);
  if (!pixel.drawn) {
    discard;
  }
  return pixel.colour;
}
