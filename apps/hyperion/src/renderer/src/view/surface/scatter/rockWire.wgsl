// The authoritative rocks in the wireframe (plan R11, T4.b; Design note 7): each rock drawn as
// depth-writing geometry, black as the wireframe's ground is (`terrainWire.wgsl`), and stroked at
// its silhouette in `--text` at the index contour's width, so that the helm's picture at touchdown
// shows what the hull meets. Its vertices are the lit draw's (`rockPage.wgsl`'s decode, placed as
// `rock.wgsl` places them). The silhouette is where the rock's surface turns from the camera:
// each fragment's distance from it in pixels is its facing f = n · v over |∇f|, the screen gradient
// of f's length (after Golus 2021, as the contours' are), so that the stroke stands at a constant
// width, antialiased by a pixel; its derivatives are taken in uniform control flow. The depth is
// pushed away by 2⁻¹⁶ of itself, as the wireframe's ground's, so that the two order alike.
// Composed after frame.wgsl and `rockPage.wgsl`.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The wireframe's ground's fields (`terrainWire.wgsl`'s `Draw`), of which the stroke reads the
  // index contour's width (`contourWidthsPx.y`) and `--text`.
  atlas : vec4f,
  contour : vec4f,
  contourWidthsPx : vec4f,
  graticule : vec4f,
  muted : vec4f,
  text : vec4f,
  casing : vec4f,
  occluderSlopePx : f32,
  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).
  clusterBase : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The constant depth push, a fraction of the depth: 2^-16, as `terrainWire.wgsl`'s.
const ROCK_WIRE_DEPTH_FRACTION : f32 = 0.0000152587890625;

struct RockWireOut {
  @builtin(position) @invariant clip : vec4f,
  // The vertex less the camera, body-fixed, metres, and the rock's outward normal there.
  @location(0) fromCameraBf : vec3f,
  @location(1) normal : vec3f,
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> RockWireOut {
  let item = cluster_lists[draw.clusterBase + entry];
  let instance = rock_instances[item >> 8u];
  let corner = rock_corner(instance, item & 0xffu, u32(local.y), u32(local.x));
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let fromCameraM = instance.originM + rotation * corner.offset;
  var out : RockWireOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(fromCameraM, 1.0);
  if (!corner.drawn) {
    out.clip = vec4f(0.0, 0.0, 2.0, 1.0);
  }
  out.fromCameraBf = transpose(rotation) * fromCameraM;
  out.normal = corner.normal;
  return out;
}

struct RockWireFragment {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@fragment
fn fragmentMain(in : RockWireOut) -> RockWireFragment {
  let facing = max(dot(normalize(in.normal), -normalize(in.fromCameraBf)), 0.0);
  let per_px = length(vec2f(dpdxFine(facing), dpdyFine(facing)));
  // The fragment's distance from the silhouette, pixels; far inside it where f does not change.
  let from_edge = select(1e6, facing / per_px, per_px > 0.0);
  let stroke = clamp(draw.contourWidthsPx.y + 0.5 - from_edge, 0.0, 1.0);
  var out : RockWireFragment;
  out.colour = vec4f(draw.text.rgb * stroke, 1.0);
  out.depth = clamp(in.clip.z * (1.0 - ROCK_WIRE_DEPTH_FRACTION), 0.0, 1.0);
  return out;
}
