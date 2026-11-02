// The composite's transmittance to space (plan R05, Design note 16; plan R08, R08.T6.c, R08.T6.f,
// R08.T8.b, R08.T8.c), moved here from `composite.wgsl` (the owner's priority, 2026-11-01), the
// centre ray now formed only where it is blended in, so that the kernel that forms it from above
// the top at the march's resolution (`compositeBehind.wgsl`) takes the same functions. Follows
// `common.wgsl`, `view.wgsl`, `oblate.wgsl` and `farLimb.wgsl`. Its includer declares `draw`, whose
// `shell` (x: the atmosphere's height H above the ground, m; y: the absorbers' count; z: the
// medium's line slots) it reads, and the tables `transmittance`, `absorberColumns`,
// `absorberCurves`, `absorberPressureColumns`, `lastAbsorberPressureColumn` and `oblateBody`.

// The sliced lookup's start at the camera (R08.T6.f; `oblate.wgsl`): its place, its ground quadric
// and local scale height formed in f64 (`view.exact`), its height, the datum's normal under it, and
// s and h* there.
fn cameraLookupStart(v : AtmosphereView, body : OblateBody) -> LookupStart {
  let n = v.up.xyz;
  return LookupStart(
    v.camera.xyz, oblateScaled(body, v.camera.xyz), v.exact.x, v.camera.w, n, n.z, length(n.xy),
    v.curvature.z, v.camera.w * v.curvature.z, v.exact.z
  );
}

// A light's transmittance to space along `dir` from the lookup's `start`, per channel: the sliced
// lookup's (R08.T6.f; `oblate.wgsl`), the slices' optical depth, absorber columns and
// pressure-weighted columns over s, with light slot `slot`'s curves of growth (R08.T6.c, R08.T4.d;
// each light's own since R08.T7), exponentiated in f32, so that a disc's light is never a stored
// half float; 0 where the ground hides space.
fn transmittanceFrom(body : OblateBody, start : LookupStart, dir : vec3f, slot : u32) -> vec3f {
  let ray = slicedRayOf(body, oblateBody, start, dir);
  if (ray.shadowed) {
    return vec3f(0.0);
  }
  let count = min(u32(draw.shell.y), ABSORBER_SLOTS);
  let stored = slicedStoredOf(
    ray, body, oblateBody, transmittance, absorberColumns, absorberPressureColumns,
    lastAbsorberPressureColumn, count
  );
  let absorbed = absorberDepth(
    absorberCurves, count, slot, u32(draw.shell.z), stored.columns, stored.pressureColumns
  );
  return exp(-(stored.depth + absorbed));
}

// A light's transmittance from the camera along `dir` to space, per channel
// (`transmittanceFrom` at the camera's start): inside the air.
fn transmittanceToSpace(v : AtmosphereView, body : OblateBody, dir : vec3f, slot : u32) -> vec3f {
  return transmittanceFrom(body, cameraLookupStart(v, body), dir, slot);
}

// The distance along the unit `dir` from a camera above the top to the ray's entry into the top's
// shell, the quadric (a + H / s_e, c + H / s_p) the ray march clips the air by (R08.T6.f), 0 where
// the camera lies within it, and -1 where the ray misses it or it lies behind (R08.T8.b). As
// `rayMarch.wgsl`'s `rayQuadric` and `quadricHits` take it: in space scaled to a sphere, its
// constant formed in f64 (`view.exact.y`), its discriminant from Lagrange's identity beyond
// h0 = a / 2, and its roots in the forms that do not cancel.
fn topEntryOf(v : AtmosphereView, body : OblateBody, dir : vec3f) -> f32 {
  let a = v.figure.x;
  let topA = draw.shell.x * body.scales.z;
  let radius = a + topA;
  let scale = vec3f(1.0, 1.0, radius / (v.figure.y + draw.shell.x * body.scales.w));
  let o = v.camera.xyz * scale;
  let d = dir * scale;
  let qa = dot(d, d);
  let qb = dot(o, d);
  let qc = v.exact.y;
  var disc = qb * qb - qa * qc;
  if (v.camera.w > 0.5 * a) {
    let across = cross(o, d);
    disc = qa * (radius * radius) - dot(across, across);
  }
  if (disc < 0.0) {
    return -1.0;
  }
  let root = sqrt(disc);
  let s = -(qb + select(-root, root, qb >= 0.0));
  if (s == 0.0) {
    return -1.0;
  }
  let first = s / qa;
  let second = qc / s;
  if (max(first, second) <= 0.0) {
    return -1.0;
  }
  return max(min(first, second), 0.0);
}

// A light's transmittance to space along `dir` from a camera above the top, per channel (R08.T8.b):
// 1 where the ray misses the air; the sliced lookup's from the camera where it lies within the
// top's shell; and otherwise from the ray's entry into it, the camera's ray carrying nothing
// before it, so that its transmittance to space is the table's from the entry to the top
// (Bruneton 2017, precomputed_atmospheric_scattering's `functions.glsl`: `GetSkyRadiance` moves a
// viewer in space to the top boundary along the view ray, 1 where the ray misses the air and 0
// where it meets the ground, and `ComputeTransmittanceToTopAtmosphereBoundary` defines the table).
// The entry's ground quadric Q = |S p|^2 - a^2 is the shell's, H_e (2a + H_e) + (k - k_t)(k + k_t)
// z^2, with H_e = H / s_e, H_p = H / s_p, k_t = (a + H_e) / (c + H_p) and k - k_t =
// (a H_p - c H_e) / (c (c + H_p)), which no difference of radii forms (Design note 18); its
// height and normal are its datum point's, s and h* there, and, on a body of several bands, the
// local scale height the body's table holds at that h*, since the composite declares no medium.
fn transmittanceFromAbove(v : AtmosphereView, body : OblateBody, dir : vec3f, slot : u32) -> vec3f {
  let entryM = topEntryOf(v, body, dir);
  if (entryM < 0.0) {
    return vec3f(1.0);
  }
  if (entryM == 0.0) {
    return transmittanceToSpace(v, body, dir, slot);
  }
  let a = v.figure.x;
  let c = v.figure.y;
  let topA = draw.shell.x * body.scales.z;
  let topC = draw.shell.x * body.scales.w;
  let k = body.scales.x;
  let kTop = (a + topA) / (c + topC);
  let kGap = (a * topC - c * topA) / (c * (c + topC));
  let p = v.camera.xyz + entryM * dir;
  let quadricM2 = topA * (2.0 * a + topA) + kGap * (k + kTop) * p.z * p.z;
  let datum = datumPointOf(body, p, quadricM2);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let z = s * datum.heightM;
  var scaleM = 0.0;
  if (oblateBandCount(body) > 1u) {
    scaleM = oblateScaleHeightAt(body, oblateBody, z);
  }
  let start = LookupStart(
    p, oblateScaled(body, p), quadricM2, datum.heightM, datum.normal, datum.normal.z,
    length(datum.normal.xy), s, z, scaleM
  );
  return transmittanceFrom(body, start, dir, slot);
}

// A light's transmittance to space from a camera above the top across the pixel's far-limb
// footprint (R08.T8.c): `transmittanceFromAbove` along the pixel's ray `dir`, blended by w_L with
// its mean over the footprint's nodes, the share above the top counted as 1, what lies there. The
// centre ray is formed only where it is blended in (the owner's priority, 2026-11-01): at w_L 1 the
// mean alone, the same bits, one lookup fewer.
fn transmittanceAcrossFromAbove(
  v : AtmosphereView,
  body : OblateBody,
  footprint : ptr<function, FarLimbFootprint>,
  dir : vec3f,
  slot : u32,
) -> vec3f {
  let blend = (*footprint).blend;
  // One call site, as the march keeps one (`rayMarchFootprint.wgsl`): SwiftShader inlines every
  // call, and its GPU process segfaulted with the lookup emitted once more here.
  var centre = vec3f(1.0);
  if (!(blend >= 1.0)) {
    centre = transmittanceFromAbove(v, body, dir, slot);
  }
  if (!(blend > 0.0)) {
    return centre;
  }
  var mean = vec3f((*footprint).spaceShare);
  for (var i = 0u; i < (*footprint).count; i++) {
    let node = farLimbNodeOf(footprint, i);
    mean += node.z * transmittanceFromAbove(v, body, rayThrough(v, node.xy), slot);
  }
  if (blend >= 1.0) {
    return mean;
  }
  return mix(centre, mean, blend);
}
