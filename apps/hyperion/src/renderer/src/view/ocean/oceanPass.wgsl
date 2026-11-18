// The `ocean` pass's far sea (plan R11, R11.T8.b; Design notes 11 and 12): one draw a body whose sea
// the frame draws, a full-screen triangle, each pixel's ray met with the body's sea through
// oceanFar.wgsl's `ocean_far_hit` (the sea-level spheroid where the ocean map holds open water, a
// closed basin's raised one where its texel says so) and lit there by each of the body's two
// brightest sources through R08's surface lighting (`atmosphere_sun_transmittance` and
// `atmosphere_sky_irradiance` at the sea's height above the datum, its geodetic latitude and the
// source's azimuth from local north, R08 Design note 17), each source's light through
// `ocean_far_light`, the glint capped at the bloom threshold, which the frame's glare source carries
// past (`glint.ts`). Each source's light, its sky's with it, is dimmed by the eclipses of the bodies
// between it and the sea (R07's eclipse term, litBody.wgsl's `eclipse_visible`, copied here as
// `ocean_eclipse_visible` since that library needs the phase table, which this pass does not bind),
// read from the body's disc record (`ocean_eclipse`, `oceanPass.ts`' `packOceanEclipse`).
//
// Drawn after the opaque passes and before R08's composite, opaque: the sea's light holds its bed's
// seen through the water, so it replaces what lies under it, and it writes its depth (frag_depth,
// reversed-Z, tested against the scene's), so that the composite lays the air over the sea and not
// over the bed beneath it (a lean, 2026-11-18). Where the body's figure is drawn as a mesh on its
// datum, the sea's depth is the nearer of its own and the datum's along the ray, so that a sea level
// below the datum is not hidden by the figure it stands in for; under 20 km, where the near sea
// (oceanNear.wgsl) draws the sea, it draws the closed basins alone. Its alpha is R07's meter class: a
// lit body's where the brightest source is above the sea's horizon, an unlit one's elsewhere.
//
// The ray is the pass's own: the pixel's through the frame's projection (a still's tile and its
// sub-pixel offset included), in the view's axes, turned into the body-fixed ones, as
// occluderSphereHit.wgsl's is formed.
//
// Composed after frame.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`), shaders/cubeTexel.wgsl,
// oceanFar.wgsl and oceanShared.wgsl, whose bindings and helpers it shares with oceanNear.wgsl.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m; w: 1 where the body's figure is drawn as
  // a mesh on its datum, else 0.
  camera : vec4f,
  // The view's x, y and z axes in the body-fixed ones.
  viewX : vec4f,
  viewY : vec4f,
  viewZ : vec4f,
  // x, y: the datum's equatorial and polar radii, m; z: the sea level above it, m; w: the sources
  // drawn, 0 to 2.
  datum : vec4f,
  // Each source: xyz its unit direction, body-fixed; w its light slot.
  source0 : vec4f,
  // Its illuminance face-on per channel times the frame's pre-exposure, lx.
  sourceLight0 : vec4f,
  source1 : vec4f,
  sourceLight1 : vec4f,
  // oceanFar.wgsl's `OceanFar`, row by row (`packOceanFar`): its light and cap the pre-exposed ones;
  // `farBed`'s w 1 where the near sea draws the sea and this pass its closed basins alone (R11.T8.c).
  farFigure : vec4f,
  farSun : vec4f,
  farSunLux : vec4f,
  farSkyLux : vec4f,
  farIndex : vec4f,
  farAttenuation : vec4f,
  farDeep : vec4f,
  farBed : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

struct OceanVertex {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> OceanVertex {
  // A full-screen triangle; each fragment's depth is its own hit's.
  return OceanVertex(vec4f(position.xy, 1.0, 1.0));
}

struct OceanOut {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

// The draw's `OceanFar`.
fn ocean_pass_far() -> OceanFar {
  return OceanFar(
    draw.farFigure,
    draw.farSun,
    draw.farSunLux,
    draw.farSkyLux,
    draw.farIndex,
    draw.farAttenuation,
    draw.farDeep,
    draw.farBed,
  );
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> OceanOut {
  // The pixel's ray in the view's axes, through the frame's projection, off-axis terms included.
  let ndc = vec2f(
    position.x * frame.viewport.z * 2.0 - 1.0,
    1.0 - position.y * frame.viewport.w * 2.0,
  );
  let viewRay = vec3f(
    (ndc.x + frame.clipProjection[2][0]) / frame.clipProjection[0][0],
    (ndc.y + frame.clipProjection[2][1]) / frame.clipProjection[1][1],
    -1.0,
  );
  let rayLength = length(viewRay);
  let d = normalize(viewRay.x * draw.viewX.xyz + viewRay.y * draw.viewY.xyz + viewRay.z * draw.viewZ.xyz);
  let o = draw.camera.xyz;
  var far = ocean_pass_far();
  let hit = ocean_far_hit(ocean_map, far, o, d);
  if (!hit.found || (draw.farBed.w > 0.5 && hit.texel.level == 0.0)) {
    discard;
  }
  let height = draw.datum.z + hit.texel.level;
  let latitude = ocean_atan2(hit.normal.z, length(hit.normal.xy));
  var radiance = vec3f(0.0);
  var lit = false;
  let sources = min(u32(draw.datum.w), 2u);
  for (var j = 0u; j < sources; j = j + 1u) {
    var direction = draw.source0;
    var light = draw.sourceLight0.xyz;
    if (j == 1u) {
      direction = draw.source1;
      light = draw.sourceLight1.xyz;
    }
    let s = direction.xyz;
    let slot = u32(direction.w);
    let mu0 = dot(hit.normal, s);
    // Its sky shares its eclipse, as an umbra's air does (terrainLit.wgsl's reading).
    let visible = ocean_source_visible(j, hit.point);
    let transmitted = visible * atmosphere_sun_transmittance(height, mu0, latitude, ocean_azimuth(hit.normal, s), slot);
    let sky = visible * atmosphere_sky_irradiance(height, mu0, latitude, slot);
    far.sun = vec4f(s, draw.farSun.w);
    radiance = radiance + ocean_far_light(far, OceanLight(light * transmitted, light * sky), -d, hit.normal, hit.texel, hit.t);
    if (j == 0u) {
      lit = mu0 > 0.0 && any(transmitted > vec3f(OCEAN_LIT_SHARE));
    }
  }
  // The nearer of the sea's and the datum's along the ray where the figure stands on the datum.
  var depthT = hit.t;
  if (draw.camera.w > 0.5) {
    let datum = ocean_spheroid_hit(o, d, draw.datum.x, draw.datum.y);
    if (datum.found) {
      depthT = min(depthT, datum.t);
    }
  }
  var out : OceanOut;
  // Reversed-Z, infinite far: n ÷ (−z), −z = t ÷ |viewRay| (occluderSphereHit.wgsl's).
  out.depth = clamp(frame.clipProjection[3][2] * rayLength / depthT, 0.0, 1.0);
  out.colour = vec4f(
    min(radiance, vec3f(OCEAN_HDR_MAX)),
    select(OCEAN_METER_UNLIT_BODY, OCEAN_METER_LIT_BODY, lit),
  );
  return out;
}
