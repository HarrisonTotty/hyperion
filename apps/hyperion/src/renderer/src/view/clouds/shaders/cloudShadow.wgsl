// R11's clouds on the ground below them (plan R11, R11.T7.b; Design note 10): what of a source's
// light the cloud layer lets through to a place under it, from one tap of the field where the ray
// from the place towards the source crosses the layer's shell. The twin of `clouds/cloudShadow.ts`'
// `cloudLightAt`, its f64 reference, whose remarks give the model. Composed after
// `view/shaders/preciseTrig.wgsl`, `cloudField.wgsl` and `cloudOptics.wgsl`, whose bindings its
// includer declares (`cloudField.wgsl`'s header); it declares none of its own.
//
// `terrain_cloud_light(p, sun, source_radius)` takes the terrain's `p`, the texel's place from the
// body's centre ÷ the datum's equatorial radius a (`terrainLit.wgsl`), `sun` the unit direction to
// the source and `source_radius` its angular radius, rad, all body-fixed, and returns
//   x: on the source's direct light, (1 − f) + f T_dir, in place of R10's `terrain_shadow_factor`;
//   y: on the sky's light from the source, (1 − f) + f (T_dir + T_diff), in place of its share of
//      R10's `terrain_sky_factor`;
//   z: f T_diff, the share of the source's direct light on a horizontal surface above the layer that
//      the cloud scatters down, which the terrain adds to the sky's light;
// f the covered share of the crossing's footprint and T_dir, T_diff the δ-Eddington transmittances at
// the source's zenith cosine against the shell's normal there. A place on or above the shell, or a
// body whose layer is not drawn, keeps (1, 1, 0). The footprint is the source's disc seen across the
// crossing's distance, so that a cloud's shadow is as soft as its penumbra.

// The least zenith cosine the layer's optics are taken at (`cloudShadow.ts`' CLOUD_LEAST_MU).
const CLOUD_SHADOW_LEAST_MU : f32 = 0.02;
// The least footprint of a shadow's tap, m.
const CLOUD_SHADOW_LEAST_FOOTPRINT_M : f32 = 1.0;

fn terrain_cloud_light(p : vec3f, sun : vec3f, source_radius : f32) -> vec3f {
  let shell = cloud_row(CLOUD_ROW_SHELL);
  let datum = cloud_row(CLOUD_ROW_NORM).y;
  // The shell's quadric in units of a: |S q|² = (a_s ÷ a)², S = diag(1, 1, a_s ÷ c_s).
  let k = shell.x / shell.y;
  let outer = shell.x / datum;
  let sq = vec3f(p.xy, p.z * k);
  let sd = vec3f(sun.xy, sun.z * k);
  let a = dot(sd, sd);
  let b = dot(sq, sd);
  let c = dot(sq, sq) - outer * outer;
  if (!(c < 0.0)) {
    return vec3f(1.0, 1.0, 0.0);
  }
  // Inside the shell the ray leaves it once, ahead.
  let t = (-b + sqrt(max(b * b - a * c, 0.0))) / a;
  let crossing = (p + t * sun) * datum;
  let normal = normalize(vec3f(crossing.xy, crossing.z * k * k));
  let mu0 = clamp(dot(normal, sun), CLOUD_SHADOW_LEAST_MU, 1.0);
  let footprint = max(2.0 * source_radius * t * datum / mu0, CLOUD_SHADOW_LEAST_FOOTPRINT_M);
  let field = cloud_field_at(crossing, footprint, CLOUD_OCTAVES_MAX);
  let optics = cloud_row(CLOUD_ROW_OPTICS);
  let slab = cloud_delta_eddington(field.in_cloud_tau, optics.x, optics.y, mu0);
  let f = field.fraction;
  return vec3f(1.0 - f + f * slab.direct, 1.0 - f + f * (slab.direct + slab.diffuse), f * slab.diffuse);
}
