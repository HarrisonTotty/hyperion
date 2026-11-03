// The sources' transmittance table (plan R08, R08.T9.b's lever; `AtmosphereSettings`'
// `sourceTransmittance`, `table`): for each height and zenith cosine, the optical depth from a lit
// point to space under one light slot's curves, one layer a latitude band and light slot, as the
// sky's irradiance is laid out, which the lit passes read in place of the sliced lookup a source
// (`surfaceLighting.wgsl`'s `atmosphereTabledTransmittance`). Built once after each build of the
// tables, as the irradiance is. Each texel is the sliced lookup itself (`oblate.wgsl`'s
// `slicedRayOf` and `slicedStoredOf`, the slot's curves added by `absorberDepth`) from the point
// `h* ÷ s_b` above the datum at the band's latitude, towards the texel's cosine at
// SOURCE_TABLE_AZIMUTH, with no ground test: the reader takes the ground's shadow from the reference
// sphere, so that a texel at the horizon keeps its finite depth for the read beside it.
//
// The axes are the transmittance tables' (`common.wgsl`'s `transmittanceUnitAt`; Bruneton 2017's
// parameterisation and texel mapping, Design note 19) on the body's reference sphere R_ref in h*:
// the height by ρ = √(r² − R_ref²) ÷ H, the cosine by the distance to the top from its zenith's to
// the horizon ray's, so that the columns crowd where a source's light falls fastest. Each stored
// value is held to the half float's largest, as the transmittance tables' are. Composed after
// `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `oblate.wgsl`, `belowDeck.wgsl` and
// `surfaceLighting.wgsl` (`SURFACE_LIGHTING_WGSL`), whose tables it declares.

@group(0) @binding(0) var atmosphere_body : texture_2d<f32>;
@group(0) @binding(1) var atmosphere_transmittance : texture_2d_array<f32>;
@group(0) @binding(2) var atmosphere_absorber_columns : texture_2d_array<f32>;
@group(0) @binding(3) var atmosphere_absorber_pressure_columns : texture_2d_array<f32>;
@group(0) @binding(4) var atmosphere_last_absorber_pressure_column : texture_2d_array<f32>;
@group(0) @binding(5) var atmosphere_absorber_curves : texture_2d<f32>;
// The three a build never reads, declared because `surfaceLighting.wgsl` names them, and bound to
// any table (the kernel's every declaration is bound, `compute.ts`' `kernelBindings`).
@group(0) @binding(6) var atmosphere_irradiance : texture_2d_array<f32>;
@group(0) @binding(7) var atmosphere_below_deck : texture_2d_array<f32>;
@group(0) @binding(8) var atmosphere_source_table : texture_2d_array<f32>;
@group(0) @binding(9) var sourceTableOut : texture_storage_2d_array<rgba16float, write>;
// One layer's build: x the layer it writes (band × light slots + slot), y the light slot whose
// curves it takes, z the band's geodetic latitude, rad (`TablesBand.latitudeRad`), w unused.
@group(0) @binding(10) var<uniform> sourceTableBuild : vec4f;

// The azimuth from north towards east each texel's ray takes, rad: π ÷ 4, where the vertical
// plane's radius of curvature is the harmonic mean of the meridian's and the prime vertical's
// (Euler's theorem, 1 ÷ R_α = cos²α ÷ M + sin²α ÷ N), the middle of those a lit point's sources span.
const SOURCE_TABLE_AZIMUTH : f32 = 0.78539816339744831;

// rgba16float's largest finite value (IEEE 754-2008 binary16), to which every stored depth is held.
const SOURCE_TABLE_HALF_MAX : f32 = 65504.0;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(sourceTableOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  // The phony reads keep the unread tables in the layout, which binds every declaration.
  _ = textureDimensions(atmosphere_irradiance);
  _ = textureDimensions(atmosphere_below_deck);
  _ = textureDimensions(atmosphere_source_table);
  let body = oblateBodyOf(atmosphere_body);
  let unit = unitFromTexelUv((vec2f(id.xy) + 0.5) / vec2f(size), vec2f(size));
  // The texel's h* and cosine on R_ref (`medium.wgsl`'s `transmittanceTexelOf`, on the body's
  // sphere and top): h from ρ without cancellation (Design note 18).
  let bottom = body.figure.z;
  let top = body.counts.w;
  let bigH = sqrt(top * (2.0 * bottom + top));
  let rho = bigH * unit.y;
  let scaledHeightM = rho * rho / (sqrt(rho * rho + bottom * bottom) + bottom);
  let dMin = top - scaledHeightM;
  let dMax = rho + bigH;
  let d = dMin + unit.x * (dMax - dMin);
  var mu = 1.0;
  if (d != 0.0) {
    mu = (shellBelowTop(bottom, top, scaledHeightM) - d * d) / (2.0 * (bottom + scaledHeightM) * d);
  }
  let latitudeRad = sourceTableBuild.z;
  let latitude = atmosphereSinCos(latitudeRad);
  let s = gravityRatioAt(body, latitude.x * latitude.x);
  let place = atmosphereSurfacePlace(body, scaledHeightM / s, latitudeRad);
  let direction = atmosphereDirection(place, clamp(mu, -1.0, 1.0), SOURCE_TABLE_AZIMUTH);
  let ray = slicedRayOf(body, atmosphere_body, place.start, direction);
  let depth = atmosphereDepthOf(body, ray, u32(sourceTableBuild.y));
  textureStore(
    sourceTableOut, id.xy, u32(sourceTableBuild.x), vec4f(min(depth, vec3f(SOURCE_TABLE_HALF_MAX)), 0.0)
  );
}
