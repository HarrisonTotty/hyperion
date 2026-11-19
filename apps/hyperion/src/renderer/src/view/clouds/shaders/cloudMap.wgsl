// The cloud map's pass (plan R11, R11.T7.c; Design note 10): the layer's field about the camera at
// five scales into one `rgba16float` texture, its levels stacked in rows, which the volume's march
// reads (`cloudShape.wgsl`). The twin of `clouds/cloudMap.ts`' `CloudMapTwin`, whose remarks give
// the plane and the levels: each texel holds the column at its centre's place on the shell over a
// footprint of its size, its covered share f, mean τ, top share η_top and unresolved deviation s
// (`cloudShape.ts`' `cloudColumnAt`).
//
// Composed after frame.wgsl, `view/shaders/preciseTrig.wgsl` and `cloudField.wgsl`, whose bindings
// it declares.

struct Draw {
  offsetFromCameraM : vec3f,
  // `cloudMap.ts`' `cloudMapUniforms`: the anchor and a_s; east and level 0's texel, m; north and
  // the levels; each level's corner on the plane, m; the side, px, and the inner share.
  mapAnchor : vec4f,
  mapEast : vec4f,
  mapNorth : vec4f,
  mapOrigins01 : vec4f,
  mapOrigins23 : vec4f,
  mapOrigins4 : vec4f,
  // x: the shell's a_s ÷ c_s; y: the octaves drawn at most; z: 1 where the deck may be cumuliform.
  shape : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(1) var cloud_noise : texture_3d<f32>;
@group(2) @binding(2) var cloud_sampler : sampler;
@group(2) @binding(3) var<storage, read> cloud_layer : array<vec4f>;

// `cloudShape.ts`' EXCESS_FULL, TOP_EXPONENT, STRATIFORM_LEAST_TOP, CUMULUS_COVER, STRATUS_COVER
// and LEAST_TOP_SHARE.
const CLOUD_MAP_EXCESS_FULL : f32 = 2.0;
const CLOUD_MAP_TOP_EXPONENT : f32 = 0.6;
const CLOUD_MAP_STRATIFORM_LEAST_TOP : f32 = 0.7;
const CLOUD_MAP_CUMULUS_COVER : f32 = 0.5;
const CLOUD_MAP_STRATUS_COVER : f32 = 0.85;
const CLOUD_MAP_LEAST_TOP_SHARE : f32 = 0.02;
const CLOUD_MAP_LEAST_SCALE : f32 = 1e-9;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

// `cloudShape.ts`' `deckTopShare`.
fn cloud_map_top_share(excess : f32, coverage : f32, cumuliform : bool) -> f32 {
  let thickness = pow(max(clamp(excess / CLOUD_MAP_EXCESS_FULL, 0.0, 1.0), 1e-9), CLOUD_MAP_TOP_EXPONENT);
  let sheet = CLOUD_MAP_STRATIFORM_LEAST_TOP + (1.0 - CLOUD_MAP_STRATIFORM_LEAST_TOP) * thickness;
  var cumulus = 0.0;
  if (cumuliform) {
    cumulus = clamp(
      (CLOUD_MAP_STRATUS_COVER - coverage) / (CLOUD_MAP_STRATUS_COVER - CLOUD_MAP_CUMULUS_COVER), 0.0, 1.0
    );
  }
  return max(sheet + cumulus * (thickness - sheet), CLOUD_MAP_LEAST_TOP_SHARE);
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let side = draw.mapOrigins4.z;
  let texel = vec2u(position.xy);
  let level = texel.y / u32(side);
  let i = f32(texel.x);
  let j = f32(texel.y - level * u32(side));
  var origin = draw.mapOrigins01.xy;
  if (level == 1u) {
    origin = draw.mapOrigins01.zw;
  } else if (level == 2u) {
    origin = draw.mapOrigins23.xy;
  } else if (level == 3u) {
    origin = draw.mapOrigins23.zw;
  } else if (level == 4u) {
    origin = draw.mapOrigins4.xy;
  }
  let size = draw.mapEast.w * exp2(2.0 * f32(level));
  let plane = origin + (vec2f(i, j) + 0.5) * size;
  let v = draw.mapAnchor.xyz * draw.mapAnchor.w + plane.x * draw.mapEast.xyz + plane.y * draw.mapNorth.xyz;
  let k = draw.shape.x;
  let a_s = cloud_row(CLOUD_ROW_SHELL).x;
  let place = v * (a_s / length(vec3f(v.xy, v.z * k)));
  let field = cloud_field_at(place, size, u32(draw.shape.y));
  let cell = cloud_coverage_at(place);
  let excess = field.in_cloud_tau / max(cloud_row(CLOUD_ROW_SHELL).w * cell.y, CLOUD_MAP_LEAST_SCALE);
  let top = cloud_map_top_share(excess, cloud_normal_tail(cell.x), draw.shape.z > 0.5);
  return vec4f(field.fraction, field.mean_tau, top, field.unresolved_sd);
}
