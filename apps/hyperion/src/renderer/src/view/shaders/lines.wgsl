// The wireframe's strokes (plan R02, Design note 9): every segment an instance, expanded to a
// screen-space quad, with its coverage computed analytically over the stroke's width plus a pixel.
// Composed after frame.wgsl. Drawn twice per batch, the casing at the stroke's width plus twice
// the casing's, then the stroke, both premultiplied over what is beneath; depth-tested against the
// occluders (greater-equal), never writing depth.

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

// Two per segment: one end's x, y, z and the dash's phase there, px; the other end's x, y, z, 0.
@group(2) @binding(0) var<storage, read> segments: array<vec4f>;

struct LineVarying {
  @builtin(position) position: vec4f,
  // From the segment's first end, along it and across it, px.
  @location(0) @interpolate(linear) along: f32,
  @location(1) @interpolate(linear) across: f32,
  @location(2) @interpolate(flat) lengthPx: f32,
  @location(3) @interpolate(linear) dashPx: f32,
}

// The coverage's antialiasing fringe beyond the stroke's half-width, px.
const FRINGE_PX = 1.0;

fn toClip(p: vec3f) -> vec4f {
  if (draw.space > 0.5) {
    let ndc = vec2f(p.x / frame.viewport.x * 2.0 - 1.0, 1.0 - p.y / frame.viewport.y * 2.0);
    // Depth 1, the near plane's, so that the symbology passes every depth test.
    return vec4f(ndc, 1.0, 1.0);
  }
  return frame.clipProjection * frame.viewRotation * vec4f(draw.offsetFromCameraM + p, 1.0);
}

fn toPx(clip: vec4f) -> vec2f {
  let ndc = clip.xy / clip.w;
  return vec2f((ndc.x + 1.0) * 0.5 * frame.viewport.x, (1.0 - ndc.y) * 0.5 * frame.viewport.y);
}

fn toClipAt(px: vec2f, zw: vec2f) -> vec4f {
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  return vec4f(ndc * zw.y, zw.x, zw.y);
}

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> LineVarying {
  let index = (u32(draw.firstSegment) + instance) * 2u;
  let a = segments[index];
  let b = segments[index + 1u];
  var ca = toClip(a.xyz);
  var cb = toClip(b.xyz);
  // Clip the segment to the near plane, where w is the near plane's distance.
  let nearW = frame.clipProjection[3][2];
  var out: LineVarying;
  if (ca.w < nearW && cb.w < nearW) {
    out.position = vec4f(0.0, 0.0, 0.0, 1.0);
    out.along = 0.0;
    out.across = 0.0;
    out.lengthPx = 0.0;
    out.dashPx = 0.0;
    return out;
  }
  if (ca.w < nearW) {
    ca = mix(ca, cb, (nearW - ca.w) / (cb.w - ca.w));
  } else if (cb.w < nearW) {
    cb = mix(cb, ca, (nearW - cb.w) / (ca.w - cb.w));
  }
  let sa = toPx(ca);
  let sb = toPx(cb);
  let d = sb - sa;
  let lengthPx = length(d);
  var unit = vec2f(1.0, 0.0);
  if (lengthPx > 1e-6) {
    unit = d / lengthPx;
  }
  let normal = vec2f(-unit.y, unit.x);
  let halfPx = draw.widthPx * 0.5 + FRINGE_PX;
  // corner.x: 0 at the first end, 1 at the other; corner.y: 0 or 1, the side.
  let endT = corner.x;
  let side = corner.y * 2.0 - 1.0;
  let px = mix(sa, sb, endT) + unit * (endT * 2.0 - 1.0) * halfPx + normal * side * halfPx;
  let zw = mix(ca.zw, cb.zw, endT);
  out.position = toClipAt(px, zw);
  out.along = dot(px - sa, unit);
  out.across = dot(px - sa, normal);
  out.lengthPx = lengthPx;
  out.dashPx = a.w + out.along;
  return out;
}

@fragment
fn fragmentMain(v: LineVarying) -> @location(0) vec4f {
  // The distance from the segment, with round caps.
  let u = clamp(v.along, 0.0, v.lengthPx);
  let distancePx = length(vec2f(v.along - u, v.across));
  var coverage = clamp(draw.widthPx * 0.5 + 0.5 - distancePx, 0.0, 1.0);
  if (draw.dashOffPx > 0.0) {
    let period = draw.dashOnPx + draw.dashOffPx;
    let phase = v.dashPx - floor(v.dashPx / period) * period;
    if (phase >= draw.dashOnPx) {
      coverage = 0.0;
    }
  }
  if (coverage <= 0.0) {
    discard;
  }
  // Premultiplied, over what is beneath.
  return vec4f(draw.colour.rgb * coverage, coverage);
}
