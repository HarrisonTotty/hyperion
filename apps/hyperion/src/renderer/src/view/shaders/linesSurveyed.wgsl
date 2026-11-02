// A body's own lines over its surveyed terrain in the wireframe (plan R10, R10.T11; Design note
// 14): its limb and graticule at the datum, R02's strokes (lines.wgsl), discarded wherever the
// body's coverage mask holds the cell beneath the stroke's point surveyed (wireCoverage.wgsl), where
// the terrain draws the graticule on its own height (`terrainWire.wgsl`), so that no datum line
// floats over a surveyed valley. Composed after frame.wgsl, lineStroke.wgsl, cubeTexel.wgsl and
// wireCoverage.wgsl. Drawn as lines.wgsl's are: the casing, then the stroke, premultiplied.

struct Draw {
  // The batch's origin from the camera, m: zero for a body's lines.
  offsetFromCameraM: vec3f,
  // The stroke's linear colour, alpha 1.
  colour: vec4f,
  // The stroke's full width, px.
  widthPx: f32,
  // The batch's first segment in `segments`.
  firstSegment: f32,
  // 0: view space, metres from the origin, a body's lines' always.
  space: f32,
  // The dash's on and off lengths, px; 0 off for a solid stroke, a body's lines' always.
  dashOnPx: f32,
  dashOffPx: f32,
  // A batch on a body's ground (R10.T16.c, `line_depth`): xyz the body's centre from the camera,
  // m, w the pixels of the datum's depth slope it is pulled by beside each fragment's own offset
  // (`GROUND_PULL_PX`); w 0 for any other batch.
  ground: vec4f,
  // The body's body-fixed axes into the camera frame's, as `mat4x4f` (its rotation).
  coverageAxes: mat4x4f,
  // x the body's equatorial radius over its polar, a ÷ c; y, z, w unused.
  coverageStretch: vec4f,
  // The body's centre from the camera, m, along the camera frame's axes; w unused.
  coverageCentreM: vec4f,
}

@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> LineVarying {
  return line_vertex(corner, instance);
}

struct LineFragment {
  @location(0) colour: vec4f,
  @builtin(frag_depth) depth: f32,
}

@fragment
fn fragmentMain(v: LineVarying) -> LineFragment {
  let coverage = line_coverage(v);
  if (coverage <= 0.0 || coverage_surveyed(v.chordM - draw.coverageCentreM.xyz)) {
    discard;
  }
  var out: LineFragment;
  // Premultiplied, over what is beneath.
  out.colour = vec4f(draw.colour.rgb * coverage, coverage);
  out.depth = line_depth(v);
  return out;
}
