// The cloud map's pass (plan R11, R11.T7.c; Design note 10): the layer's field's coarse parts about
// the camera at five scales into one `rgba16float` texture, its levels stacked in rows, which the
// volume's march reads and refines (`cloudShape.wgsl`). The twin of `clouds/cloudMap.ts`'
// `CloudMapTwin`, whose remarks give the plane and the levels: each texel holds, at its centre's
// place on the shell over a footprint of its size, the field's resolved sum Σ w_k ρᵏ n_k, z_r ÷
// the normalisation, and the coverage's threshold z_c and wind there, eastward and northward
// (`cloudShape.ts`' `cloudMapTexelAt`).
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
  // x: the shell's a_s ÷ c_s; y: the octaves drawn at most.
  shape : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(1) var cloud_noise : texture_3d<f32>;
@group(2) @binding(2) var cloud_sampler : sampler;
@group(2) @binding(3) var<storage, read> cloud_layer : array<vec4f>;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
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
  return vec4f(field.resolved / cloud_row(CLOUD_ROW_NORM).x, cell.x, cell.zw);
}
