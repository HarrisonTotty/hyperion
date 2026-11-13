// The aerial-perspective volume's read (plan R08, R08.T9.a): at a surface `distanceM` along the
// ray through the screen at `uv` ((0, 0) at its top left), the light scattered into the ray in
// front of it per unit of the volume's scale (rgb) and the ray's mean transmittance (a), as the
// composite lays them over an opaque surface within the volume's reach. The composite reads the
// volume here, and so does any pass that takes it from `HillaireAtmosphere.aerialPerspective` on
// the `scene` scope (R11's), so that every reader takes one mapping. Past the reach the read holds
// the last slice; the composite takes the ray march there (`rayMarch.wgsl`). Follows `common.wgsl`
// (`texelUvFromUnit`).
//
// Moved from R05's composite (`composite.wgsl`), unchanged in its arithmetic. The slice axis is
// Bruneton's mapping already (plan R08, Design note 19): slice k holds the march to (k + 1) reach /
// slices, so the unit (slices distance / reach - 1) / (slices - 1) reads at w = distance / reach -
// 1/2 / slices, and a surface at the reach reads the last slice's centre, its march to the reach
// itself. The screen axes take the mapping here, the froxels standing at (i, j) / (size - 1) of the
// screen. Within the first slice the read fades from the camera, where nothing has scattered yet,
// to that slice's march (Bevy's fade, its `sample_aerial_view_lut`).
//
// Changes (plan R08, R08.T9.c): the volume's second texture, its transmittance per channel (rgb, the
// mean in a), is read at the same place by `aerialTransmittanceAt`, with the same fade from 1 at the
// camera, so that a surface is dimmed per channel in place of the mean.
fn aerialPerspectiveAt(
  aerial : texture_3d<f32>,
  linearClamp : sampler,
  uv : vec2f,
  distanceM : f32,
  reachM : f32,
) -> vec4f {
  let read = aerialVolumeRead(aerial, linearClamp, uv, distanceM, reachM);
  return vec4f(read.sampled.rgb * read.fade, mix(1.0, read.sampled.a, read.fade));
}

// The transmittance per channel at a surface `distanceM` along the ray through `uv` (R08.T9.c): the
// volume's second texture `transmittances` read as `aerialPerspectiveAt` reads the first, faded from
// 1 at the camera to the first slice's.
fn aerialTransmittanceAt(
  transmittances : texture_3d<f32>,
  linearClamp : sampler,
  uv : vec2f,
  distanceM : f32,
  reachM : f32,
) -> vec3f {
  let read = aerialVolumeRead(transmittances, linearClamp, uv, distanceM, reachM);
  return mix(vec3f(1.0), read.sampled.rgb, read.fade);
}

// A read of one of the volume's textures: its filtered texel and the first slice's fade.
struct AerialVolumeRead {
  sampled : vec4f,
  fade : f32,
}

// One of the volume's textures read at a surface `distanceM` along the ray through `uv`, on the slice
// axis at w = distance / reach - 1/2 / slices and the screen's through Bruneton's mapping, with the
// fade across the first slice.
fn aerialVolumeRead(
  volume : texture_3d<f32>,
  linearClamp : sampler,
  uv : vec2f,
  distanceM : f32,
  reachM : f32,
) -> AerialVolumeRead {
  let size = textureDimensions(volume);
  let slices = f32(size.z);
  let w = clamp(distanceM / reachM - 0.5 / slices, 0.0, 1.0);
  let screen = texelUvFromUnit(uv, vec2f(size.xy));
  let sampled = textureSampleLevel(volume, linearClamp, vec3f(screen, w), 0.0);
  let fade = clamp(distanceM / (reachM / slices), 0.0, 1.0);
  return AerialVolumeRead(sampled, fade);
}
