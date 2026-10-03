// The aerial-perspective volume (plan R05, Design note 16; Hillaire 2020, section 5.3): for each
// froxel of the camera's frustum, the light scattered into the ray between the camera and the
// froxel's far face per unit sun illuminance (rgb) and the ray's mean transmittance (a), in
// `view.tables.z` slices of equal depth out to `view.tables.y` (32 km, Hillaire 2020's reach),
// rebuilt every frame. Follows `common.wgsl` and `view.wgsl`.
//
// Ported from Bevy 0.19.1's aerial_view_lut.wgsl (MIT; the notice is in common.wgsl), marching once
// through the slices and storing each slice's running integral. Changes: the integral is stored
// linear, with sebh's mean transmittance in alpha, rather than as Bevy's logarithm; each step is
// sampled at its midpoint; the sphere is the camera's own (Design note 16).

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var transmittance : texture_2d<f32>;
@group(0) @binding(3) var multiScattering : texture_2d<f32>;
@group(0) @binding(4) var aerialOut : texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(aerialOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size.xy);
  let dir = rayThrough(view, uv);
  let bottom = medium.bottomRadiusM;
  // The camera on its own sphere, centred so that its datum normal is the sphere's.
  let origin = view.up.xyz * (bottom + max(view.camera.w, 0.0));
  let slices = size.z;
  let perSlice = max(u32(view.tables.w), 1u);
  let dt = view.tables.y / f32(slices * perSlice);
  var luminance = vec3f(0.0);
  var throughput = vec3f(1.0);
  for (var slice = 0u; slice < slices; slice++) {
    for (var step = 0u; step < perSlice; step++) {
      let t = (f32(slice * perSlice + step) + 0.5) * dt;
      let p = origin + t * dir;
      let rP = length(p);
      let up = p / rP;
      let heightM = rP - bottom;
      let local = mediumAt(medium, rP);
      let extinction = max(local.extinction, vec3f(1e-12));
      let stepTransmittance = exp(-local.extinction * dt);
      let source = sourceAt(
        medium,
        transmittance,
        multiScattering,
        view.tables.x,
        heightM,
        dot(view.sun.xyz, up),
        dot(dir, view.sun.xyz),
      );
      luminance += throughput * (source - source * stepTransmittance) / extinction;
      throughput *= stepTransmittance;
    }
    let meanTransmittance = (throughput.r + throughput.g + throughput.b) / 3.0;
    textureStore(aerialOut, vec3u(id.xy, slice), vec4f(luminance, meanTransmittance));
  }
}
