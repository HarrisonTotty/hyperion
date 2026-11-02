// The wireframe's strokes' geometry and coverage (plan R02, Design note 9; a library since
// R10.T11, shared by lines.wgsl and linesSurveyed.wgsl): every segment an instance, expanded to a
// screen-space quad, with its coverage computed analytically over the stroke's width plus a pixel.
// Composed after frame.wgsl, before a stroke's own `Draw` and entry points; its includer's `draw`
// has R02's `offsetFromCameraM`, `widthPx`, `firstSegment`, `space`, `dashOnPx` and `dashOffPx`,
// and R10.T16.c's `ground`.

// Two per segment: one end's x, y, z and the dash's phase there, px; the other end's x, y, z, 0.
@group(2) @binding(0) var<storage, read> segments: array<vec4f>;

struct LineVarying {
  @builtin(position) position: vec4f,
  // From the segment's first end, along it and across it, px.
  @location(0) @interpolate(linear) along: f32,
  @location(1) @interpolate(linear) across: f32,
  @location(2) @interpolate(flat) lengthPx: f32,
  @location(3) @interpolate(linear) dashPx: f32,
  // The point of the segment the fragment draws, m from the camera along the camera frame's axes,
  // by its ends: for a view batch, read where a line is discarded over a body's surveyed ground
  // (linesSurveyed.wgsl, R10.T11).
  @location(4) chordM: vec3f,
  // The stroke's unit normal on the view, px, y down: the direction `across` is measured along
  // (R10.T16.c, `line_depth`).
  @location(5) @interpolate(flat) normalPx: vec2f,
  // The segment's reversed depth at its two ends as drawn, clipped to the near plane (`line_depth`).
  @location(6) @interpolate(flat) endDepths: vec2f,
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

// Corner `corner` of segment `instance` of the batch's quad.
fn line_vertex(corner: vec3f, instance: u32) -> LineVarying {
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
    out.chordM = vec3f(0.0);
    out.normalPx = vec2f(0.0);
    out.endDepths = vec2f(0.0);
    return out;
  }
  var pa = a.xyz;
  var pb = b.xyz;
  if (ca.w < nearW) {
    let t = (nearW - ca.w) / (cb.w - ca.w);
    ca = mix(ca, cb, t);
    pa = mix(pa, pb, t);
  } else if (cb.w < nearW) {
    let t = (nearW - cb.w) / (ca.w - cb.w);
    cb = mix(cb, ca, t);
    pb = mix(pb, pa, t);
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
  out.chordM = draw.offsetFromCameraM + mix(pa, pb, endT);
  out.normalPx = normal;
  out.endDepths = vec2f(ca.z / ca.w, cb.z / cb.w);
  return out;
}

// A fragment's coverage of its stroke, its dash's gaps none, with round caps.
fn line_coverage(v: LineVarying) -> f32 {
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
  return coverage;
}

// The depth a fragment of a stroke is tested at (plan R10, R10.T16.c): its own, or for a batch
// drawn on a body's ground (the survey edge; `draw.ground.w` `GROUND_PULL_PX`, 0 for any other
// batch, `draw.ground.xyz` the body's centre from the camera) its segment's at the point the
// fragment lies across from, pulled to the depth that the plane through that point, parallel to the
// datum's tangent plane there, has at the fragment's own pixel, where that is nearer, the plane's
// depth gradient across the fragment's offset from the point, and by `draw.ground.w` pixels of the
// plane's depth slope besides, for the rim drawn off the drawn ground. The ground beneath the
// stroke's nearer half then stays behind it while the terrain's depth is pushed by its constant
// fraction alone, and a nearer ridge hides its centre line by more than that pixel
// (`groundPull.ts`' twin). A segment's reversed depth is affine along it on the view, so the
// point's is its ends' mixed by its place along it; the quad's own, interpolated over its round
// caps too, stands a part of a cap's length off it. Under the reversed, infinite projection a
// plane's depth n (r . N) / (Q . N) is affine in the ray r, so its gradient is n / (Q . N) times
// the plane's tilt into the screen's two axes, y down on the view.
fn line_depth(v: LineVarying) -> f32 {
  if (!(draw.ground.w > 0.0)) {
    return v.position.z;
  }
  let up = normalize(v.chordM - draw.ground.xyz);
  let q = (frame.viewRotation * vec4f(v.chordM, 0.0)).xyz;
  let n = (frame.viewRotation * vec4f(up, 0.0)).xyz;
  let qn = dot(q, n);
  let heightM = select(-1.0, 1.0, qn >= 0.0) * max(abs(qn), 1e-6);
  let gradient = frame.clipProjection[3][2] / heightM * vec2f(
    n.x * 2.0 / (frame.viewport.x * frame.clipProjection[0][0]),
    -n.y * 2.0 / (frame.viewport.y * frame.clipProjection[1][1]),
  );
  let u = clamp(v.along, 0.0, v.lengthPx);
  let point = mix(v.endDepths.x, v.endDepths.y, u / max(v.lengthPx, 1e-6));
  // From the point to the fragment, px: along the segment past a cap's end, and across it.
  let unit = vec2f(v.normalPx.y, -v.normalPx.x);
  let offset = unit * (v.along - u) + v.normalPx * v.across;
  return min(point + draw.ground.w * length(gradient) + max(dot(gradient, offset), 0.0), 1.0);
}
