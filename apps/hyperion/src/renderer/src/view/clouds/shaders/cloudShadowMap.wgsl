// The cloud volume's shadow map's pass (plan R11, R11.T7.c; Design note 10): the volume's light on the
// ground about the camera, for the frame's brightest source, into the shadow map's two levels
// stacked in rows, which `cloudGround.wgsl` reads. The twin of `clouds/cloudShadowMap.ts`'
// `shadowTexelAt`, whose remarks give the model: x e^(−τ_sun) through the deck towards the source,
// y δ-Eddington's total transmittance at 0.6 of the column above, z its T_diff at the source's
// cosine.
//
// Composed after frame.wgsl, `view/shaders/preciseTrig.wgsl`, `cloudShape.wgsl`, `cloudOptics.wgsl`
// and `cloudVolume.wgsl`, whose bindings it declares.

struct Draw {
  offsetFromCameraM : vec3f,
  // The cloud map's plane (`cloudMap.ts`' `cloudMapUniforms`).
  mapAnchor : vec4f,
  mapEast : vec4f,
  mapNorth : vec4f,
  mapOrigins01 : vec4f,
  mapOrigins23 : vec4f,
  mapOrigins4 : vec4f,
  // xyz: the unit direction to the source, body-fixed; w: the datum's equatorial radius a, m.
  sun : vec4f,
  // x, y: the deck's base and top above the datum, m; z: its spheroids' a ÷ c; w: the layer's a_s, m.
  deck : vec4f,
  // x: g; y: 1 − ϖ₀; z: a level's side, px.
  optics : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var cloud_map : texture_2d<f32>;
@group(2) @binding(1) var cloud_map_sampler : sampler;
@group(2) @binding(2) var<storage, read> cloud_layer : array<vec4f>;
@group(2) @binding(3) var<storage, read> cloud_shape : array<vec4f>;
@group(2) @binding(4) var cloud_base_noise : texture_3d<f32>;
@group(2) @binding(5) var cloud_detail_noise : texture_3d<f32>;
@group(2) @binding(6) var cloud_curl_noise : texture_2d<f32>;
@group(2) @binding(7) var cloud_field_noise : texture_3d<f32>;
@group(2) @binding(8) var cloud_noise_repeat : sampler;
@group(2) @binding(9) var cloud_octave_correction : texture_2d<f32>;

// `cloudShadowMap.ts`' SHADOW_SUN_STEPS and SHADOW_COLUMN_STEPS, and the least cosine and the
// sky's.
const CLOUD_SHADOW_SUN_STEPS : u32 = 12u;
const CLOUD_SHADOW_COLUMN_STEPS : u32 = 8u;
const CLOUD_SHADOW_LEAST_MU_MAP : f32 = 0.02;
const CLOUD_SHADOW_SKY_COSINE : f32 = 0.6;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

// The optical depth from `g` along unit `d` through the deck, `steps` equal steps of its stretch.
fn cloud_shadow_depth(m : CloudMapPlane, deck : CloudMarchDeck, g : vec3f, d : vec3f, steps : u32, footprint : f32) -> f32 {
  let stretch = cloud_deck_stretch(g, d, deck.k, deck.datum, deck.base_m, deck.top_m);
  if (stretch.y <= 0.0) {
    return 0.0;
  }
  let dt = (stretch.y - stretch.x) / f32(steps);
  let none = CloudColumn(0.0, 0.0, 1.0, 0.0, 0.0, 0.0);
  var tau = 0.0;
  for (var s = 0u; s < steps; s = s + 1u) {
    let x = g + (stretch.x + (f32(s) + 0.5) * dt) * d;
    tau = tau + cloud_density_at(m, deck, x, footprint, 1.0, false, false, none, false).sigma * dt;
  }
  return tau;
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let side = draw.optics.z;
  let texel = vec2u(position.xy);
  let level = texel.y / u32(side);
  let i = f32(texel.x);
  let j = f32(texel.y - level * u32(side));
  var origin = draw.mapOrigins01.xy;
  if (level == 1u) {
    origin = draw.mapOrigins01.zw;
  }
  let size = draw.mapEast.w * exp2(2.0 * f32(level));
  let plane = origin + (vec2f(i, j) + 0.5) * size;
  let v = draw.mapAnchor.xyz * draw.mapAnchor.w + plane.x * draw.mapEast.xyz + plane.y * draw.mapNorth.xyz;
  let a = draw.sun.w;
  let k = draw.deck.z;
  let g = v * (a / length(vec3f(v.xy, v.z * k)));
  let m = CloudMapPlane(
    draw.mapAnchor, draw.mapEast, draw.mapNorth, draw.mapOrigins01, draw.mapOrigins23, draw.mapOrigins4
  );
  let deck = CloudMarchDeck(
    draw.deck.x, draw.deck.y, max(draw.deck.y - draw.deck.x, 1.0), a, k, draw.optics.x, draw.optics.y,
    0.0, draw.deck.w
  );
  let sun = draw.sun.xyz;
  let up = normalize(vec3f(g.xy, g.z * k * k));
  let tau_sun = cloud_shadow_depth(m, deck, g, sun, CLOUD_SHADOW_SUN_STEPS, size);
  let tau_column = cloud_shadow_depth(m, deck, g, up, CLOUD_SHADOW_COLUMN_STEPS, size);
  let mu0 = max(dot(up, sun), CLOUD_SHADOW_LEAST_MU_MAP);
  let slab = cloud_delta_eddington(tau_column, draw.optics.x, draw.optics.y, mu0);
  let sky = cloud_delta_eddington(tau_column, draw.optics.x, draw.optics.y, CLOUD_SHADOW_SKY_COSINE);
  return vec4f(exp(-tau_sun), sky.direct + max(sky.diffuse, 0.0), max(slab.diffuse, 0.0), 1.0);
}
