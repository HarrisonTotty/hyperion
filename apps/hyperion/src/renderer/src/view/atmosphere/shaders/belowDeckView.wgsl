// The per-frame kernels' reads beneath a cloud deck (plan R08, R08.T15.b.2; `thick/deckRead.ts`,
// "Below the deck"): the camera's h*, the view's sources' light along a ray at an h* and a path's light
// to a target, which the sky view, aerial perspective and march beneath a deck share. Follows `belowDeck.wgsl`; its
// includer declares `view`, `oblateBody` and `belowDeckTable`.

// The camera's h* beneath the deck: the split's plus s times its height above the kernels' datum,
// the deck's, held to [0, split].
fn belowDeckCameraHeightStar() -> f32 {
  let split = belowDeckSplitOf(belowDeckTable);
  return clamp(split + view.curvature.z * view.camera.w, 0.0, split);
}

// The view's sources' light along the unit `dir`, every source's sum_k w_k I_k, at h* `heightStarM`
// under the zenith `n`.
fn belowDeckSkyAt(body : OblateBody, n : vec3f, dir : vec3f, heightStarM : f32) -> vec3f {
  let mu = dot(dir, n);
  var sum = vec3f(0.0);
  for (var k = 0u; k < skySourceCount(view); k++) {
    let source = view.sources[k];
    let to = source.direction.xyz;
    let muSun = dot(to, n);
    let horizontal = dot(dir, to) - mu * muSun;
    sum += source.weight.rgb * belowDeckRadianceAt(
      belowDeckTable, body, oblateBody, view.curvature.z, heightStarM, muSun, -mu, horizontal,
      skySlotOf(source)
    );
  }
  return sum;
}

// The path's light from the camera along the unit `dir` to `t` beneath a deck, and T's mean over the
// channels (`tablesCpu.ts`'s belowDeckAerialPerspectiveTwin): I(h*_camera, Omega) - T I(h*_t, Omega)
// held at 0, `atCamera` the first, T = e^(-tau) of the path's depth, h_t the height along the ray on
// its own sphere R_alpha from the camera's unclamped height (`common.wgsl`'s shellHeightAlong's
// form), each read in the camera's frame.
fn belowDeckPathTo(body : OblateBody, dir : vec3f, atCamera : vec3f, fromM : f32, t : f32) -> vec4f {
  let n = view.up.xyz;
  let bottom = cameraCurvatureAlong(view.curvature.x, view.curvature.y, n, dir);
  let heightM = view.camera.w;
  let r = bottom + heightM;
  let mu = dot(dir, n);
  let s = view.curvature.z;
  let split = belowDeckSplitOf(belowDeckTable);
  let rT = sqrt(max(t * t + 2.0 * r * mu * t + r * r, 0.0));
  let heightAlongM = (t * t + 2.0 * r * mu * t + heightM * (2.0 * bottom + heightM)) / (rT + bottom);
  let toM = clamp(split + s * heightAlongM, 0.0, split);
  let atTarget = belowDeckSkyAt(body, n, dir, toM);
  let transmitted = exp(-belowDeckPathDepth(belowDeckTable, body, oblateBody, s, fromM, toM, t));
  return vec4f(
    max(atCamera - transmitted * atTarget, vec3f(0.0)),
    (transmitted.x + transmitted.y + transmitted.z) / 3.0,
  );
}
