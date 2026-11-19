// The clouds' light on a surface beneath them (plan R11, R11.T7.c; Design note 10): the cloud
// volume's shadow map (`cloudShadowMap.wgsl`) where it holds the place, blended to the layer's own
// tap (`cloudShadow.wgsl`'s `terrain_cloud_light`) at its edge and wherever it holds nothing: a
// second source, the low setting, a frame that draws no volume. The twin of `clouds/cloudShadowMap.ts`'
// `cloudGroundLightAt`. Any pass that lights a surface under the clouds composes it, the terrain's
// cloudy forms (`terrain/cloudLight.ts`) first, the sea's to come.
//
// Composed after `cloudShadow.wgsl` (and so `preciseTrig.wgsl`, `cloudField.wgsl` and
// `cloudOptics.wgsl`, with `cloudField.wgsl`'s bindings); its includer declares besides:
//   - `cloud_shadow_map : texture_2d<f32>`, the shadow map, `rgba16float`, its two levels stacked in
//     rows (`cloudShadowMap.ts`), read through `cloud_sampler`, linear;
//   - `cloud_shadow_rows : array<vec4f>`, read-only storage, its CLOUD_SHADOW_ROWS rows
//     (`cloudShadowMap.ts`' `packCloudShadow`), all 0 where the map holds nothing.
//
// `cloud_ground_light(p, sun, source_radius)` takes `terrain_cloud_light`'s arguments, the place ÷
// a, and returns its three factors (`cloudShadow.wgsl`'s header). A place above the datum reads its
// direct light at the datum's place on its ray towards the source, and its sky's and scattered
// light at its own column; ground that rises to the deck's base takes the layer's tap
// (`cloudShadowMap.ts`' remarks).

const CLOUD_SHADOW_ROWS : u32 = 9u;
const CLOUD_SHADOW_ROW_SOURCE : u32 = 6u;
const CLOUD_SHADOW_ROW_DATUM : u32 = 7u;
const CLOUD_SHADOW_ROW_DECK : u32 = 8u;
// The least cosine between the map's source and the one asked of it at which the map serves it.
const CLOUD_SHADOW_SAME_SOURCE : f32 = 0.99999;

fn cloud_shadow_row(i : u32) -> vec4f {
  return cloud_shadow_rows[i];
}

// Level k of the map at a place on its plane, bilinear between its texel centres, held at its edge.
fn cloud_shadow_read_level(level : u32, plane : vec2f) -> vec3f {
  let side = cloud_shadow_row(5u).z;
  let size = cloud_shadow_row(1u).w * exp2(2.0 * f32(level));
  let origin = select(cloud_shadow_row(3u).xy, cloud_shadow_row(3u).zw, level == 1u);
  let t = clamp((plane - origin) / size - 0.5, vec2f(0.0), vec2f(side - 1.0));
  let uv = (t + 0.5) / side;
  return textureSampleLevel(cloud_shadow_map, cloud_sampler, vec2f(uv.x, (f32(level) + uv.y) * 0.5), 0.0).xyz;
}

// The map's levels at a place, m: xyz their light, w the share of it a read takes over the layer's
// tap, 0 off the map (`cloudShadowMap.ts`' `mapReadAt`).
fn cloud_shadow_map_at(place : vec3f) -> vec4f {
  let datum = cloud_shadow_row(CLOUD_SHADOW_ROW_DATUM);
  let anchor = cloud_shadow_row(0u);
  let r = length(place);
  let c = dot(place, anchor.xyz) / r;
  if (!(c > 0.05)) {
    return vec4f(0.0);
  }
  let scale = anchor.w / (c * r);
  let plane = vec2f(dot(place, cloud_shadow_row(1u).xyz), dot(place, cloud_shadow_row(2u).xyz)) * scale;
  let side = cloud_shadow_row(5u).z;
  let half0 = 0.5 * side * cloud_shadow_row(1u).w;
  let half1 = half0 * datum.z;
  let centre0 = cloud_shadow_row(3u).xy + half0;
  let centre1 = cloud_shadow_row(3u).zw + half1;
  let d0 = abs(plane - centre0);
  let d1 = abs(plane - centre1);
  let r0 = max(d0.x, d0.y) / half0;
  let r1 = max(d1.x, d1.y) / half1;
  if (r1 >= 1.0) {
    return vec4f(0.0);
  }
  let weight = 1.0 - smoothstep(datum.w, 1.0, r1);
  let outer = cloud_shadow_read_level(1u, plane);
  if (r0 >= 1.0) {
    return vec4f(outer, weight);
  }
  return vec4f(mix(cloud_shadow_read_level(0u, plane), outer, smoothstep(datum.w, 1.0, r0)), weight);
}

fn cloud_ground_light(p : vec3f, sun : vec3f, source_radius : f32) -> vec3f {
  let layer = terrain_cloud_light(p, sun, source_radius);
  let source = cloud_shadow_row(CLOUD_SHADOW_ROW_SOURCE);
  if (source.w < 0.5 || dot(source.xyz, sun) < CLOUD_SHADOW_SAME_SOURCE) {
    return layer;
  }
  let datum = cloud_shadow_row(CLOUD_SHADOW_ROW_DATUM);
  let deck = cloud_shadow_row(CLOUD_SHADOW_ROW_DECK);
  let k = datum.y;
  let place = p * datum.x;
  let r = length(place);
  let height = r - datum.x * r / length(vec3f(place.xy, place.z * k));
  let under = smoothstep(0.0, deck.z * max(deck.y - deck.x, 1.0), deck.x - height);
  if (!(under > 0.0)) {
    return layer;
  }
  let mu = max(dot(normalize(vec3f(place.xy, place.z * k * k)), sun), deck.w);
  let towards = cloud_shadow_map_at(place - (max(height, 0.0) / mu) * sun);
  let column = cloud_shadow_map_at(place);
  return vec3f(
    mix(layer.x, towards.x, under * towards.w),
    mix(layer.yz, column.yz, under * column.w),
  );
}
