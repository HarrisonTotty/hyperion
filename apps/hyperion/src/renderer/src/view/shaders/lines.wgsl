// The wireframe's strokes (plan R02, Design note 9): every segment an instance, expanded to a
// screen-space quad, with its coverage computed analytically over the stroke's width plus a pixel
// (lineStroke.wgsl). Composed after frame.wgsl and lineStroke.wgsl. Drawn twice per batch, the
// casing at the stroke's width plus twice the casing's, then the stroke, both premultiplied over
// what is beneath; depth-tested against the occluders (greater-equal), never writing depth.

struct Draw {
  // The batch's origin from the camera, m: zero but for a hull's (Design note 2).
  offsetFromCameraM: vec3f,
  // The stroke's linear colour, alpha 1.
  colour: vec4f,
  // The stroke's full width, px.
  widthPx: f32,
  // The batch's first segment in `segments`.
  firstSegment: f32,
  // 0: view space, metres from the origin; 1: screen space, px from the top left with z 0.
  space: f32,
  // The dash's on and off lengths, px; an off length of 0 is a solid stroke.
  dashOnPx: f32,
  dashOffPx: f32,
}

@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> LineVarying {
  return line_vertex(corner, instance);
}

@fragment
fn fragmentMain(v: LineVarying) -> @location(0) vec4f {
  let coverage = line_coverage(v);
  if (coverage <= 0.0) {
    discard;
  }
  // Premultiplied, over what is beneath.
  return vec4f(draw.colour.rgb * coverage, coverage);
}
