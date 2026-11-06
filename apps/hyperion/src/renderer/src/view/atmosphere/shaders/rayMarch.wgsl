// The per-pixel ray march (plan R05, Design note 16): the light scattered into a pixel's ray,
// summed over the view's drawn sources and weighted by each one's illuminance times the frame's
// scale (plan R08, R08.T7) (rgb), and the ray's mean transmittance (a), where the tables do not
// serve: the sky seen from above the atmosphere, which Hillaire 2020 (section 7) also marches on
// screen, and surfaces beyond the aerial-perspective volume's reach (32 km, his section 5.4), past
// which nothing of his marches. Elsewhere it writes nothing the composite reads. At
// `view.output.z` of the output's resolution (1 high, 0.5 low, with the composite's depth-aware
// upsample). Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's `raymarch_atmosphere` (functions.wgsl, MIT; the notice is in
// common.wgsl). Changes (Design note 16): the ray is clipped against the datum spheroid's shells,
// a + top and c + top for the atmosphere and a and c for the ground, not spheres; each sample's
// height is its height above the spheroid along the radius and its sun cosine is taken against the
// spheroid's normal, and the tables are read at that height; the ground's bounce reads the sun's
// transmittance at the ground (Bevy reads it at radius 0); the steps are placed toward the ray's
// lowest point, the point nearest the body's centre, or, for the camera side of a two-sided ray
// from inside the atmosphere to a drawn surface, toward the camera (`marchSplit` in common.wgsl,
// R05.T12.e; every two-sided sky segment under the sun-aware warp since R08.T8, below), where
// Bevy spaces them evenly, and each is sampled at its midpoint; each term's density is evaluated
// once a sample; a step's in-scattering is taken without f32's cancellation (`stepFactor` in
// common.wgsl). The placement takes the centre alone: it needs no spheroid.
//
// Changes (plan R08, R08.T6.g, Design note 18): no height, root or discriminant is formed from the
// camera's f32 position as a difference of radii, which in f32 near 6.4e6 m carry 0.25 m (2 m near
// 6e7 m). Each shell is a quadric along the ray, Q(t) = q_a t^2 + 2 q_b t + q_c in space scaled to
// a sphere, whose constant q_c comes formed in f64 (`view.exact.xy`); the camera's position enters
// only in products with unit directions, where its rounding moves a height by about 6e-8 t. The
// ray meets each shell at the roots that do not cancel, and a sample's height above the spheroid
// along its radius is |p| q / (sqrt(1 + q)(sqrt(1 + q) + 1)), q = Q / a^2. Beyond h0 = a / 2, Q is
// taken about the ray's closest approach t*, q_a (t - t*)^2 + Q*, with Q* = -disc / q_a from
// Lagrange's identity and t - t* carried from the steps' own offsets (`marchStep`), where the
// constant's own error would grow as h0 (`decision-r08-f32.md` section 4.2).
//
// Changes (plan R08, R08.T6.c, Design notes 5 and 8): the ray's own path takes its absorbers by
// the path's column, under the sun's curves of growth (`viewMarchStep` in source.wgsl), and
// the sun's transmittance, the bare ground's included, is the table's optical depth with them.
//
// Changes (plan R08, R08.T6.i; `decision-r08-t6d-findings.md` section 1): where the medium has
// layer edges, each side of the ray's split is cut into pieces where the ray crosses them
// (`MarchWalk` in common.wgsl), at the same count; a medium with none takes the steps as before.
// Each edge e's shell is the quadric (a + e, c + e), as the top's is (a + H, c + H)
// (`marchWalkCrossShell`).
//
// Changes (plan R08, R08.T7, Design note 7): one march sums every drawn source, each light slot's
// share carried on its own path (`ViewMarch` in source.wgsl), alpha slot 0's mean transmittance;
// the bare ground is lit by every source through its own slot's throughput.
//
// Changes (plan R08, R08.T6.f, Design note 17, _Per-view marches_): the shells stand through h*: the
// top and each layer edge e are the quadrics (a + e / s_e, c + e / s_p), as R08.T12.d's tracer takes
// them, in place of (a + e, c + e). Each sample is read at its height on the datum along the
// similar spheroid's normal, the root of the exact quadratic from its ground quadric
// (`oblate.wgsl`'s datumPointOf), in place of its radial height, which departs from it by
// h (1 / cos psi - 1) (0.5% of h on a Saturn-class figure at 45°), since h* = s h is geodetic; its
// normal is the datum's at that height's foot. The medium is read at h* = s h, s at the sample's
// latitude, and the sources through the sliced lookup and the bands (`source.wgsl`); the bare
// ground is lit through the lookup from its own place. As `tablesCpu.ts`'s oblateRayMarchTwin.
//
// Changes (plan R08, R08.T8): the march also draws every pixel from inside the air once the camera's
// h* reaches the setting's march band (`view.exact.w` above 0, `hillaire.ts`'s marchWeightOf), where
// the composite blends it with the sky view and the aerial-perspective volume, as it draws every
// pixel from above the top. Every two-sided sky segment it marches (nothing drawn along it), from
// inside the air or outside it, takes the sun-aware warp by the first source (`marchSkyWarp`,
// `marchEvenStep`, `marchWalkEvenStep`): even where the Sun is ahead or across the ray, R05's
// one-sided placement from the segment's start where it is behind. From inside, R05.T12.e's gate
// failed both placements on its sky families from the band's cameras (about the lowest point five
// twilight rays with the Sun behind the camera, 17.6% at worst; toward the camera one, 8.12%), and
// the placement about the lowest point from outside failed the night-side and terminator limb (the
// Sun 90-105 degrees from the zenith at the tangent point; 12.19%), where even steps hold them
// (4.32% and 4.85%) and keep the march one rule on both sides of the top, which it would otherwise
// cross with a step of 13-36%. Even steps in turn drew the sky under the horizontal at the camera's
// last light (the Sun just over the camera's visible horizon, behind) 45-124 codes off under a seam
// at the horizontal, which the warp's placement from the start holds within 1.7 codes on high and
// 10.8 on low. One-sided segments and every drawn surface keep R05.T12.e's placement, a drawn
// surface's camera side from inside toward the camera (`decision-r08-t8-leans.md` sections 2, 4 and
// 5.4; plan R08's Risks, "Deviations in T8.a, as built").
//
// Changes (plan R08, R08.T8.c): from above the top, a texel whose footprint meets the shell takes
// the march's mean over its footprint across the limb, blended in by its span (`farLimb.wgsl`;
// `decision-r08-t8-leans.md` section 6.1): from about one scale height a texel one ray cannot stand
// for its footprint, and the limb broke into specks and gaps that flickered as the body moved by a
// fraction of a pixel. A ray's march is `marchRay`, once for the centre and once a node.
//
// Changes (plan R08, R08.T8.d; `decision-r08-t8-leans.md` section 8): no step takes or returns an
// array by value (source.wgsl's header): `MarchRay` holds neither the sources nor their angles,
// each step reading them from the view (`viewSourcesAt`), and the ground's bounce takes its slot's
// throughput through `viewThroughputOf`; the footprint's nodes are formed as each is marched
// (`farLimbNodeOf`), not held; the arithmetic unchanged.
//
// Changes (plan R08, R08.T8.e; lever 3 of `decision-r08-t8-leans.md` sections 8 to 10): a ray that
// ends on the bare ground, nothing drawn, takes fewer steps where it is optically thin: the
// ground's floor (20 on high, 4 more for each of the medium's layer edges) while its green optical
// depth from the ground back along the ray, absorbers included, is at most 2, climbing back to the
// setting's count in even steps by 8 (`rayMarchSamplesOf`, `view.marchGround`; `marchSteps.ts`'s
// marchSamplesOf is its twin). A sky segment whose optical depth from its lowest point, along each
// side, is under 0.05 takes 28 on high (`view.marchSky`). A drawn surface kept the setting's count
// until R08.T11.b (below). From orbit the face is most of the march's step work; R05.T12.e's gate
// holds every ray at the rule on all three media at 1 and 6 sources, at no wider tolerance.
//
// Changes (plan R08, R08.T8.e; lever 5 of `decision-r08-t8-leans.md` sections 10 and 13): two kernels
// are made from this module, each with a file of its own appended that gives the step its sources'
// light and the bare ground its light (`rayMarchSourcesAt`, `rayMarchGroundLightAt`):
// `rayMarchCurves.wgsl`, the shared table's read with each slot's curves applied on read, as before;
// and `rayMarchSlots.wgsl`, one read a source a step of its light slot's layer of the per-planet slot
// layers, for a body whose tables hold them (`tables.ts`'s slotDepthsRefusal), chosen when its
// pipeline is made. The ray's count (lever 3, `rayMarchDepthAlong`), the forward lobes' depth, the
// view's own path and every other kernel keep the shared read.
//
// Changes (plan R08, R08.T11.b; lever A of `decision-r08-t11-frame.md` sections 2 and 8): a ray
// that ends on a drawn surface on the datum takes the bare ground's count, by its optical depth from
// the datum's hit back along the ray: on a ray that meets the datum, the surface at or beyond the
// hit, where the march ends at the hit as on the bare ground (R07's face, drawn on its chords), or
// before it within the band `view.marchSky.z` above the datum, 25 km on high (terrain's heights).
// Only the count changes: the ray's end, its transmittance and the drawn surface's light are as
// before, and no bare ground is added, which stays keyed on depth 0. From orbit the face's rays
// return to the counts R05.T12.e's gate accepted for them as bare ground before R08.T10.b drew the
// face with depth; anything else drawn keeps the setting's count.
//
// Changes (plan R08, R08.T11.d; `decision-r08-t11-frame.md` section 2, `decision-r08-t8-leans.md`
// section 11, Ruling 4): the march is dispatched over the air's area alone, the tiles its top shell
// can reach (`marchArea.ts`), and has two entry points, each appended after the source's read and
// each with one `marchRay` call site: `rayMarchCentre.wgsl`'s, first, which classifies each tile of
// 8 x 8 texels by a conservative form of the footprint's own test (`marchArea.wgsl`'s kinds), writes
// (0, 0, 0, 1) on a tile every ray misses and marches the centre ray on a tile where no footprint
// can blend; and `rayMarchFootprint.wgsl`'s, the march over the far limb's footprint as before, on
// the tiles that may blend. Per texel each marches what the one kernel marched. A view's frame takes
// the footprint's entry point alone over the area (`hillaire.ts`'s `MarchCover` `area`): on the RTX
// 3080 the tile cull's two dispatches cost 0.4-1.0 ms from 400 km and 1e7 m, and it is a cover of
// its own, off. This module holds the march and declares its bindings; the entry points declare
// the area's.
//
// Changes (plan R08, R08.T16.a.2): every segment but a two-sided sky segment's warped walk shares
// each side's steps among its pieces by the light each can return along the view, half by its drop
// in the view's transmittance lit by the first source and half by the square root of its length,
// R08.T6.i's share, at the same count, and places each piece as R05.T12.e does unless its other end
// is far the brighter (`rayMarchDepthShares`, `rayMarchWalkStep`; `marchSteps.ts`'s
// depthSharedPieces): each piece's ends read once a ray, the depth `rayMarchDepthAlong` reads, the
// source's light and the medium's extinction (`rayMarchPieceAt`). Each crossing of an edge's shell is
// taken onto the edge's own level h* = e by a Newton step (`rayMarchEdgeLevel`). The sheet decks
// stacked above a gas envelope's floor took a few steps of a long ray, their tau at their pieces'
// ends, and the hand Jupiter's routed march at the high setting's 36 read 12-24% dark in red against
// its own at 32 times them; it reads within 1% in every channel, and a segment onto an envelope's
// floor keeps R05.T12.e's placement again.
//
// Changes (plan R08, R08.T9.c): a ray's march returns its transmittance per channel beside the mean
// (`MarchedRay`), slot 0's throughput, which the entry points store in the second target
// (`rayMarchTransmittanceOut`, rgb, the mean in a; `storeMarched`), the footprint's mean taken over
// it as over the first; the composite dims a drawn surface by it in place of the mean. The first
// target is as before, to the bit.
//
// Changes (plan R08, fix-r08-decked-air-opacity; `hillaire.ts`'s SurfaceLobes): where the medium
// has forward lobes, the view's `forward.w` is 1 and no bake's table is read (the thick read's
// rewrite of `surfaceLobes`: its J already re-emits the truncated lobe's diffuse field, the
// ground's light among it), the second target is a drawn surface's
// transmittance with each drawn lobe's in-lobe scattering dropped from its extinction,
// e^(-integral (sigma_t - sum P sigma_s) ds) per channel along slot 0's path, its absorbers' curves
// included: R08.T7's delta-M (Wiscombe 1977, J. Atmos. Sci. 34, 1408), which the sun's beam and the
// multiple-scattering table already take (`scattering.wgsl`'s lobeOrdersOf, `multiScattering.wgsl`).
// A surface's light scattered within a lobe, 30 degrees, lands within its own pixel from orbit, as a
// sky's does about the sun; dropped, a served water deck of tau 1.15 left the ground at 0.30-0.41 of
// the tracer's (`hyperion-fit atmosphere-reference`). Its own path from the first step, never the
// throughput times e^(x P), so that a deep path whose throughput f32 cannot hold keeps its own. The
// first target, its mean and a medium with no lobe are as before, to the bit.
//
// Changes (plan R08, fix-r08-decked-air-route; research-leans-46 section 1): the term is formed only
// on a ray that ends on a drawn surface (depth above 0), the only ray whose second target the
// composite reads (`composite.wgsl`'s `marchedAt`), and its optical depth is summed along the path,
// max(step depth - P sigma_s dt, 0) a step, with one exp a ray at its end, in place of one a step.
// Each step forms it in a loop of at most one pass, not an `if`: SwiftShader, the checks' software
// adapter, runs both sides of a branch under its lanes' mask whatever they take, and a loop only
// while a lane is in it, so that a march whose rays keep no term (a sky's, a limb's) pays for none
// of its steps there. Its code still costs SwiftShader's compile of every march that holds it, so
// a deck's march, which reads a bake's J and never keeps it, compiles it out (`hillaire.ts`'s
// withDeckRead). A sum rounds otherwise than a product, so the second target moves by f32's
// rounding where it is kept.
//
// Changes (plan R08, fix-r08-decked-air-route; research-leans-46 section 2; `hillaire.ts`'s
// DeckedAirSurface): under a decked thin air's bake (`forward.w` 2, the thick read alone), whose J
// re-emits the light of one uniform ground, the bake's, scattered within the medium's forward
// lobes, a ray that ends on a drawn surface keeps its own surface's lobes' light in its second
// target, T_lobe, and its in-scatter loses the bake's ground's share of that light,
// S_bar (T_lobe - T_direct), at least 0, the in-scatter held at 0 or more. S_bar is the drawn
// ground's light at the ray's end with the bake's ground albedo: each source's transmittance to it
// and the sky's irradiance there (the table the lit passes read, `thickDeparture.wgsl`), at the
// datum's normal, times albedo / pi. Over a ground of the bake's albedo the pixel is the route's,
// L_in + S T_direct; over a drawn map it is L_routed(rho_bar) + (S - S_bar) T_lobe, 6S's form with
// the surroundings at the bake's mean (Tanre, Herman and Deschamps 1981, Appl. Opt. 20, 3676;
// Vermote et al. 1997, IEEE TGRS 35, 675), so that the drawn ground keeps its contrast through the
// lobes. Its lookups are the bare ground's, at their one call site (`rayMarchPieceAt`'s note on
// SwiftShader's stack), its own work in loops of at most one pass; a thin kernel's `departs` is
// false and a deck's march compiles it out, and the bare ground's arithmetic is as before.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d_array<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var sceneDepth : texture_depth_2d;
@group(0) @binding(8) var rayMarchOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(10) var absorberCurves : texture_2d<f32>;
@group(0) @binding(11) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(12) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(13) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(14) var lobeTables : texture_2d_array<f32>;
@group(0) @binding(15) var oblateBody : texture_2d<f32>;
@group(0) @binding(19) var rayMarchTransmittanceOut : texture_storage_2d<rgba16float, write>;

// One ray's march (`marchRay`): the first target's texel, its in-scatter (rgb) and mean
// transmittance (a), and its transmittance per channel, the second's (R08.T9.c).
struct MarchedRay {
  light : vec4f,
  transmittance : vec3f,
}

// What a ray that misses the air marches: no light, and a transmittance of 1.
fn marchMissed() -> MarchedRay {
  return MarchedRay(vec4f(0.0, 0.0, 0.0, 1.0), vec3f(1.0));
}

// Stores `marched` at `texel` of both targets: the first's texel, and the second's transmittance
// per channel with the mean in a (R08.T9.c).
fn storeMarched(texel : vec2u, marched : MarchedRay) {
  textureStore(rayMarchOut, texel, marched.light);
  textureStore(rayMarchTransmittanceOut, texel, vec4f(marched.transmittance, marched.light.a));
}

// A ray's quadric about a shell of equatorial radius `radius`, in space scaled by
// S = diag(1, 1, k) to a sphere of that radius: Q(t) = |S (P + t d)|^2 - radius^2 = qa t^2 +
// 2 qb t + qc, its constant qc formed in f64. `disc` is qb^2 - qa qc without its cancellation: as
// written near the body, where qc is small, and from Lagrange's identity, qa radius^2 - |S P x S d|^2,
// far from it (`far`), where qb^2 and qa qc nearly cancel.
struct RayQuadric {
  qa : f32,
  qb : f32,
  qc : f32,
  disc : f32,
}

fn rayQuadric(origin : vec3f, dir : vec3f, k : f32, radius : f32, qc : f32, far : bool) -> RayQuadric {
  let scale = vec3f(1.0, 1.0, k);
  let o = origin * scale;
  let d = dir * scale;
  let qa = dot(d, d);
  let qb = dot(o, d);
  var disc : f32;
  if (far) {
    let c = cross(o, d);
    disc = qa * (radius * radius) - dot(c, c);
  } else {
    disc = qb * qb - qa * qc;
  }
  return RayQuadric(qa, qb, qc, disc);
}

// The near and far distances along the ray to the quadric's surface, or (-1, -1) if missed: its
// roots s / qa and qc / s, s = -(qb + sign(qb) sqrt(disc)), neither of which cancels.
fn quadricHits(q : RayQuadric) -> vec2f {
  if (q.disc < 0.0) {
    return vec2f(-1.0, -1.0);
  }
  let root = sqrt(q.disc);
  let s = -(q.qb + select(-root, root, q.qb >= 0.0));
  if (s == 0.0) {
    return vec2f(0.0, 0.0);
  }
  let first = s / q.qa;
  let second = q.qc / s;
  return vec2f(min(first, second), max(first, second));
}

// A pixel's ray and what its samples' heights are formed from (Design note 18): the camera's
// position and the ray's direction, the datum's radii, 1 / s at the equator and at the pole (the
// shells' scales through h*, R08.T6.f), whether the ground's quadric is taken about the closest
// approach (`far`), that quadric, Q* = -disc / qa there, t_low - t* (the split's lowest point from
// the quadric's own closest approach). The sources' directions and their scattering angles are
// formed at each step from the view (`viewSourcesAt`, R08.T8.d), never held here.
struct MarchRay {
  origin : vec3f,
  dir : vec3f,
  a : f32,
  c : f32,
  equatorialShellScale : f32,
  polarShellScale : f32,
  far : bool,
  groundQ : RayQuadric,
  qStar : f32,
  lowestFromStar : f32,
}

// The scattering in a sample's drawn forward lobes, sum P sigma_s, m^-1, P each tabulated term's
// forward share in its phase.xyz, 0 where its forward part is no lobe (R08.T7; `medium.wgsl`'s
// mediumAtHeight forms the same for the multiple-scattering table).
fn rayMarchLobeScatteringOf(local : SampleMedium) -> vec3f {
  var lobe = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    if (term.phase.w > 2.5) {
      lobe += term.phase.xyz * term.scattering.rgb * termDensity(local.densities, i);
    }
  }
  return lobe;
}

// One step of the march, at its midpoint: its ground quadric, about t* beyond h0 = a / 2 and from
// the camera within it, its datum point (its height and the datum's normal there) and s at its
// latitude, then the medium at h* = s h, its sources through the sliced lookup and their
// in-scattering (R08.T6.f). Where `surfaceLobes`, a drawn surface's own optical depth `surfaceDepth`
// one step on: slot 0's step depth, as `viewMarchStep` forms it, less the step's in-lobe scattering,
// at least 0 (fix-r08-decked-air-opacity; summed since fix-r08-decked-air-route).
fn rayMarchStep(
  march : ptr<function, ViewMarch>,
  surfaceDepth : ptr<function, vec3f>,
  ray : MarchRay,
  body : OblateBody,
  stepAt : MarchStep,
  lobes : bool,
  surfaceLobes : bool,
) {
  let p = ray.origin + stepAt.tM * ray.dir;
  let groundQ = ray.groundQ;
  var bigQ : f32;
  if (ray.far) {
    let fromStar = stepAt.fromLowestM + ray.lowestFromStar;
    bigQ = groundQ.qa * fromStar * fromStar + ray.qStar;
  } else {
    bigQ = stepAt.tM * (groundQ.qa * stepAt.tM + 2.0 * groundQ.qb) + groundQ.qc;
  }
  let datum = datumPointOf(body, p, bigQ);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let local = sampleMediumAt(s * datum.heightM);
  let start = sampleStartOf(body, p, bigQ, datum.heightM, datum.normal, datum.normal, s, local);
  let shares = rayMarchSourcesAt(body, start, datum.normal, ray.dir, lobes, local);
  let absorbedBefore = (*march).path0.depth;
  viewMarchStep(march, local, shares, stepAt.dtM);
  rayMarchSurfaceStep(surfaceDepth, march, local, absorbedBefore, stepAt.dtM, surfaceLobes);
}

// A drawn surface's own optical depth `surfaceDepth` one step `dtM` on, where `surfaceLobes`
// (fix-r08-decked-air-opacity; summed since fix-r08-decked-air-route): slot 0's step depth, as
// `viewMarchStep` forms it, its absorbers' depth since `absorbedBefore`, less the step's in-lobe
// scattering, at least 0. A loop of at most one pass, not an `if` (the header's
// fix-r08-decked-air-route). A deck's march compiles its call out (`hillaire.ts`'s withDeckRead).
fn rayMarchSurfaceStep(
  surfaceDepth : ptr<function, vec3f>,
  march : ptr<function, ViewMarch>,
  local : SampleMedium,
  absorbedBefore : vec3f,
  dtM : f32,
  surfaceLobes : bool,
) {
  for (var lobed = surfaceLobes; lobed; lobed = false) {
    let stepDepth = local.extinction * dtM + ((*march).path0.depth - absorbedBefore);
    let kept = rayMarchLobeScatteringOf(local) * dtM;
    *surfaceDepth += max(stepDepth - kept, vec3f(0.0));
  }
}

// Step `i` of a split segment as the ray march steps it (R08.T8): a two-sided sky segment's under
// the sun-aware warp `warp`, every other segment's R05.T12.e's.
fn rayMarchStepOf(split : MarchSplit, i : u32, evenSky : bool, warp : f32) -> MarchStep {
  if (evenSky) {
    return marchEvenStep(split, i, warp);
  }
  return marchStep(split, i);
}

// Step `i` of a segment cut at the medium's layer edges as the ray march steps it (R08.T8): a
// two-sided sky segment's warped walk (made ready by `marchWalkEvenReady`, which holds its warp),
// every other segment's R08.T6.i walk (`marchWalkReady`), shared and placed since R08.T16.a.2 by the
// light each piece returns (`rayMarchDepthShares`, `rayMarchWalkStep`).
fn rayMarchWalkStepOf(
  walk : ptr<function, MarchWalk>,
  placement : ptr<function, RayMarchPlacement>,
  i : u32,
  evenSky : bool,
) -> MarchStep {
  if (evenSky) {
    return marchWalkEvenStep(walk, i);
  }
  return rayMarchWalkStep(walk, placement, i);
}

// A place on the ray `fromLowestM` from the split's lowest point `lowestM`: its body-fixed position
// and its ground quadric, about t* beyond h0 = a / 2 and from the camera within it, formed as a
// step's are (`rayMarchStep`; plan R08, R08.T16.a.2).
struct RayPlace {
  p : vec3f,
  quadricM2 : f32,
}

fn rayMarchPlaceAt(ray : MarchRay, lowestM : f32, fromLowestM : f32) -> RayPlace {
  let tM = lowestM + fromLowestM;
  let groundQ = ray.groundQ;
  var bigQ : f32;
  if (ray.far) {
    let fromStar = fromLowestM + ray.lowestFromStar;
    bigQ = groundQ.qa * fromStar * fromStar + ray.qStar;
  } else {
    bigQ = tM * (groundQ.qa * tM + 2.0 * groundQ.qb) + groundQ.qc;
  }
  return RayPlace(ray.origin + tM * ray.dir, bigQ);
}

// The least rise of h* along the ray, |s (d . n)|, at which a crossing is taken onto its edge's
// level (plan R08, R08.T16.a.2; `marchSteps.ts`'s MARCH_EDGE_LEAST_RISE): a ray within about 3
// degrees of the level there keeps its shell's root.
const MARCH_EDGE_LEAST_RISE : f32 = 0.05;

// A crossing `fromLowestM` from t_low of the shell of the march edge `edgeM`, taken onto the edge's
// own level h* = s h = e by one Newton step along the ray (plan R08, R08.T16.a.2; `tablesCpu.ts`'s
// edgeLevelM): fromLowestM - (s h - e) / (s (d . n)), at the place's height h on the datum, its
// normal n there and s at its latitude, as a step reads them; kept where |s (d . n)| is under
// MARCH_EDGE_LEAST_RISE. The shell stands at h* = e at the equator and the poles alone; between
// them it stands off the level by up to about 1 km at 45 degrees on Jupiter's figure, as thick as a
// sheet deck's tau above its base.
fn rayMarchEdgeLevel(
  ray : MarchRay,
  body : OblateBody,
  lowestM : f32,
  fromLowestM : f32,
  edgeM : f32,
) -> f32 {
  let place = rayMarchPlaceAt(ray, lowestM, fromLowestM);
  let datum = datumPointOf(body, place.p, place.quadricM2);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let rise = s * dot(ray.dir, datum.normal);
  if (!(abs(rise) >= MARCH_EDGE_LEAST_RISE)) {
    return fromLowestM;
  }
  return fromLowestM - (s * datum.heightM - edgeM) / rise;
}

// Adds to the walk where the ray crosses the shell of the march edge `edgeM` above the datum in h*
// (plan R08, R08.T6.i and R08.T6.f): the quadric (a + e_a, c + e_c), e_a = e / s_e and e_c = e / s_p,
// as the top's shell (a + H / s_e, c + H / s_p) stands for the top, in space scaled by
// S_e = diag(1, 1, k_e), k_e = (a + e_a) / (c + e_c), crossed at most twice, each crossing as its
// signed distance from t_low, the split's lowest point. Within h0 = a / 2 the crossings are the
// quadric's roots (`quadricHits`), its constant |S_e P|^2 - (a + e_a)^2 formed from the ground's
// q_g, which comes formed in f64, as q_g + (k_e - k)(k_e + k) P_z^2 - e_a (2a + e_a), with
// k_e - k = (c e_a - a e_c) / (c (c + e_c)): no difference of large values (Design note 18). Beyond
// it they stand about the quadric's own closest approach t*_e, at -/+ sqrt(disc) / qa with disc
// from Lagrange's identity, t*_e taken from the ground's t* as t_o - t* is (`nearestFromStar`, with
// k_e), so that each lies from t_low without a difference of large distances, as the steps do.
// Since R08.T16.a.2 each is taken onto the edge's level (`rayMarchEdgeLevel`).
fn marchWalkCrossShell(
  walk : ptr<function, MarchWalk>,
  ray : MarchRay,
  body : OblateBody,
  nearestFromStar : f32,
  acrossZ : f32,
  edgeM : f32,
) {
  let lowestM = (*walk).split.lowestM;
  let a = ray.a;
  let c = ray.c;
  let edgeA = edgeM * ray.equatorialShellScale;
  let edgeC = edgeM * ray.polarShellScale;
  let kE = (a + edgeA) / (c + edgeC);
  var crossed : vec2f;
  if (ray.far) {
    let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeA, 0.0, true);
    if (q.disc < 0.0) {
      return;
    }
    let halfChord = sqrt(q.disc) / q.qa;
    let edgeNearestFromStar = (kE - 1.0) * (kE + 1.0) * ray.dir.z * acrossZ / q.qa;
    // t*_e from t_low: (t*_e - t_o) + (t_o - t*) - (t_low - t*).
    let centre = (nearestFromStar - edgeNearestFromStar) - ray.lowestFromStar;
    crossed = vec2f(centre - halfChord, centre + halfChord);
  } else {
    let k = a / c;
    let kGap = (c * edgeA - a * edgeC) / (c * (c + edgeC));
    let z2 = ray.origin.z * ray.origin.z;
    let qc = (view.exact.x + kGap * (kE + k) * z2) - edgeA * (2.0 * a + edgeA);
    let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeA, qc, false);
    if (q.disc < 0.0) {
      return;
    }
    crossed = quadricHits(q) - vec2f(lowestM);
  }
  for (var j = 0u; j < 2u; j++) {
    marchWalkCross(walk, rayMarchEdgeLevel(ray, body, lowestM, crossed[j], edgeM));
  }
}

// The green optical depth, absorbers included, along the unit body-fixed `d` from the place `p` to
// the top, through the sliced lookup at light slot 0 from `p`'s own datum point (`sourceLightAt`;
// plan R08, R08.T8.e): what a ray's count is chosen by. `bigQ` is `p`'s ground quadric, 0 on the
// ground. As `tablesCpu.ts`'s twins read it.
fn rayMarchDepthAlong(body : OblateBody, p : vec3f, bigQ : f32, d : vec3f) -> f32 {
  let datum = datumPointOf(body, p, bigQ);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let local = sampleMediumAt(s * datum.heightM);
  let start = sampleStartOf(body, p, bigQ, datum.heightM, datum.normal, datum.normal, s, local);
  return -log(sourceLightAt(body, start, d, 0u).toSun.g);
}

// The most green optical depth a piece's end is read at, from it to the top (plan R08, R08.T16.a.2;
// `marchSteps.ts`'s MARCH_PIECE_DEPTH_CAP): under the 87.3 at which f32's exp(-tau) leaves its
// normal range, so that -log of the lookup's transmittance stays finite.
const MARCH_PIECE_DEPTH_CAP : f32 = 64.0;

// What the march shares and places a cut side's steps by at the place `fromLowestM` from the split's
// lowest point `lowestM` (plan R08, R08.T16.a.2; `tablesCpu.ts`'s MarchShareReads), read from the
// place's own datum point as a step reads it (`rayMarchStep`): x, the green optical depth from it to
// the top along the unit `d` (`rayMarchDepthAlong`'s), held to MARCH_PIECE_DEPTH_CAP, a value that
// is not a number reading the cap; y, the march's first source's light there, its green
// transmittance under light slot 0's curves, 0 where the ground shadows it, 1 where no source is
// drawn; z, the medium's green extinction at its h*, m^-1. The two lookups share one call site, so
// that the kernel inlines it once: SwiftShader's LLVM refused the ray march with a second
// ("Local variable exceeds stack size limit").
fn rayMarchPieceAt(
  ray : MarchRay,
  body : OblateBody,
  lowestM : f32,
  fromLowestM : f32,
  d : vec3f,
) -> vec3f {
  let place = rayMarchPlaceAt(ray, lowestM, fromLowestM);
  let datum = datumPointOf(body, place.p, place.quadricM2);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let local = sampleMediumAt(s * datum.heightM);
  let start = sampleStartOf(
    body, place.p, place.quadricM2, datum.heightM, datum.normal, datum.normal, s, local
  );
  // The view's depth, then the first source's light where one is drawn.
  var looked = vec2f(0.0, 1.0);
  let lookups = select(1u, 2u, viewSourceCount() > 0u);
  for (var k = 0u; k < lookups; k++) {
    let light = sourceLightAt(body, start, select(d, view.sources[0].direction.xyz, k == 1u), 0u);
    if (k == 0u) {
      looked.x = -log(light.toSun.g);
    } else {
      looked.y = light.lit * light.toSun.g;
    }
  }
  let depth = select(MARCH_PIECE_DEPTH_CAP, looked.x, looked.x < MARCH_PIECE_DEPTH_CAP);
  return vec3f(depth, looked.y, local.extinction.g);
}

// How much brighter a piece's other end must be than the end its placement is fine at before it is
// turned there (`marchSteps.ts`'s MARCH_PIECE_TURN), and what a shadowed piece's light counts for
// (MARCH_PIECE_SHADE).
const MARCH_PIECE_TURN : f32 = 100.0;
const MARCH_PIECE_SHADE : f32 = 0.01;

// `steps` among `pieces` pieces by the weights in shares[at + p], `total` their sum, above 0, each
// piece's into shares[at + p] (plan R08, R08.T16.a.2; `marchSteps.ts`'s sharesOf): one a piece, and
// the rest by the weights, rounded at each piece's end, the last the rest, as `common.wgsl`'s
// marchSideShares shares by root length.
fn marchSharesOf(walk : ptr<function, MarchWalk>, at : u32, pieces : u32, steps : u32, total : f32) {
  let rest = steps - pieces;
  var spent = 0u;
  var cumulative = 0.0;
  for (var p = 0u; p < pieces; p++) {
    cumulative += (*walk).shares[at + p];
    var share = rest;
    if (p + 1u < pieces) {
      share = u32(floor(f32(rest) * cumulative / total + 0.5));
    }
    (*walk).shares[at + p] = f32(1u + share - spent);
    spent = share;
  }
}

// One side of a walk made ready (`marchWalkReady`) that is cut into pieces, its `pieces` pieces'
// shares at shares[at + p], from `startM` to `endM` from t* through the crossings from `first`, its
// depths read back along the ray (`back`, the side before t*) or on along it: shares its `steps`
// by the light each piece can return along the view and places each toward its far brighter end
// (plan R08, R08.T16.a.2; `marchSteps.ts`'s depthSharedPieces). Each piece's weight is
// 1/2 D_p / sum D + 1/2 sqrt(L_p) / sum sqrt(L), D_p = exp(-c_p) (1 - exp(-tau_p)) (eps + l_p) its
// lit drop in the side's view transmittance from the side's start, tau_p the difference of the
// depths at its ends, at least 0, c_p the side's tau before it, l_p the mean of its ends' light, eps
// MARCH_PIECE_SHADE, L_p its length; one a piece and the rest by the weights, rounded at each
// piece's end (`marchSharesOf`). Each piece is placed as R05.T12.e places it, `towardStart` (toward
// its end nearer the segment's start) or not, unless its other end's b = sigma exp(-c) (eps + l) is
// more than MARCH_PIECE_TURN times this end's, each into `placement`'s bit of its shares' index.
// False where the drops sum to 0, its shares then the drops.
fn rayMarchSideByDepth(
  walk : ptr<function, MarchWalk>,
  placement : ptr<function, RayMarchPlacement>,
  ray : MarchRay,
  body : OblateBody,
  first : u32,
  pieces : u32,
  startM : f32,
  endM : f32,
  steps : u32,
  at : u32,
  back : bool,
  towardStart : bool,
) -> bool {
  let lowestM = (*walk).split.lowestM;
  let d = select(ray.dir, -ray.dir, back);
  var dropTotal = 0.0;
  var rootTotal = 0.0;
  var before = 0.0;
  var near = 0.0;
  var nearLight = 0.0;
  var nearBright = 0.0;
  var pieceStartM = startM;
  // The side's start, then each piece's end, one read a pass (one call site, as rayMarchPieceAt's).
  for (var q = 0u; q <= pieces; q++) {
    var pieceEndM = endM;
    if (q == 0u) {
      pieceEndM = startM;
    } else if (q < pieces) {
      pieceEndM = (*walk).fromLowestM[first + q - 1u];
    }
    let end = rayMarchPieceAt(ray, body, lowestM, pieceEndM, d);
    if (q == 0u) {
      near = end.x;
      nearLight = end.y;
      nearBright = end.z * (MARCH_PIECE_SHADE + end.y);
      continue;
    }
    let p = q - 1u;
    let depth = max(select(near - end.x, end.x - near, back), 0.0);
    let drop = exp(-before) * (1.0 - exp(-depth)) * (MARCH_PIECE_SHADE + 0.5 * (nearLight + end.y));
    (*walk).shares[at + p] = drop;
    dropTotal += drop;
    rootTotal += sqrt(pieceEndM - pieceStartM);
    before += depth;
    let farBright = end.z * exp(-before) * (MARCH_PIECE_SHADE + end.y);
    var toward = towardStart;
    if (towardStart) {
      toward = !(farBright > MARCH_PIECE_TURN * nearBright);
    } else {
      toward = nearBright > MARCH_PIECE_TURN * farBright;
    }
    let bit = 1u << ((at + p) % 32u);
    let word = (at + p) / 32u;
    if (toward) {
      (*placement).towardStart[word] |= bit;
    } else {
      (*placement).towardStart[word] &= ~bit;
    }
    near = end.x;
    nearLight = end.y;
    nearBright = farBright;
    pieceStartM = pieceEndM;
  }
  if (!(dropTotal > 0.0)) {
    return false;
  }
  var total = 0.0;
  pieceStartM = startM;
  for (var p = 0u; p < pieces; p++) {
    var pieceEndM = endM;
    if (p + 1u < pieces) {
      pieceEndM = (*walk).fromLowestM[first + p];
    }
    let weight = 0.5 * (*walk).shares[at + p] / dropTotal
      + 0.5 * sqrt(pieceEndM - pieceStartM) / rootTotal;
    (*walk).shares[at + p] = weight;
    total += weight;
    pieceStartM = pieceEndM;
  }
  marchSharesOf(walk, at, pieces, steps, total);
  return true;
}

// Each piece's placement of the ray march's walk (plan R08, R08.T16.a.2): bit k % 32 of word k / 32
// set where the walk's piece k (its shares' index) is placed toward its end nearer the segment's
// start, and whether each side's pieces take these (`rayMarchDepthShares`) or the walk's own,
// R05.T12.e's.
struct RayMarchPlacement {
  towardStart : vec2u,
  ownBefore : bool,
  ownAfter : bool,
}

// Each side of a walk made ready (`marchWalkReady`) that is cut into pieces shares its steps among
// them by the light each can return along the view and places each toward its far brighter end
// (`rayMarchSideByDepth`; plan R08, R08.T16.a.2), at the side's count; a side whose drops sum to 0
// keeps R08.T6.i's shares, made again, and the walk's placement. The walk's first piece is entered
// again. `marchSteps.ts`'s depthSharedPieces is its twin.
fn rayMarchDepthShares(
  walk : ptr<function, MarchWalk>,
  placement : ptr<function, RayMarchPlacement>,
  ray : MarchRay,
  body : OblateBody,
) {
  let split = (*walk).split;
  let before = (*walk).crossingsBefore;
  let crossings = (*walk).crossings;
  (*placement).towardStart = vec2u(0u);
  (*placement).ownBefore = false;
  (*placement).ownAfter = false;
  // The side before t*, then the side after it, through one call site each.
  for (var side = 0u; side < 2u; side++) {
    let back = side == 0u;
    if (!select((*walk).piecedAfter, (*walk).piecedBefore, back)) {
      continue;
    }
    var first = 0u;
    var count = before;
    var startM = -split.beforeM;
    var endM = 0.0;
    var steps = split.stepsBefore;
    var at = 0u;
    var towardStart = split.beforeTowardStart;
    if (!back) {
      first = before;
      count = crossings - before;
      startM = 0.0;
      endM = split.afterM;
      steps = split.stepsAfter;
      at = (*walk).firstAfter;
      towardStart = true;
    }
    let own = rayMarchSideByDepth(
      walk, placement, ray, body, first, count + 1u, startM, endM, steps, at, back, towardStart
    );
    if (back) {
      (*placement).ownBefore = own;
    } else {
      (*placement).ownAfter = own;
    }
    if (!own) {
      marchSideShares(walk, first, count, startM, endM, steps, at);
    }
  }
  (*walk).piece = 0u;
  (*walk).taken = 0u;
  if ((*walk).piecedBefore) {
    rayMarchWalkEnter(walk, placement, 0u, true);
  }
}

// Enters piece `piece` of the ray march's walk on the side before t* or after it, as `common.wgsl`'s
// marchWalkEnter enters it, then places it as `placement` holds where its side takes its own: its
// anchor its end nearer the segment's start, lowM, or its other, highM, each taken as
// marchWalkEnter takes them (plan R08, R08.T16.a.2).
fn rayMarchWalkEnter(
  walk : ptr<function, MarchWalk>,
  placement : ptr<function, RayMarchPlacement>,
  piece : u32,
  before : bool,
) {
  marchWalkEnter(walk, piece, before);
  if (!select((*placement).ownAfter, (*placement).ownBefore, before)) {
    return;
  }
  let toward = (((*placement).towardStart[piece / 32u] >> (piece % 32u)) & 1u) == 1u;
  if (toward == (*walk).towardLow) {
    return;
  }
  let split = (*walk).split;
  var lowM : f32;
  var highM : f32;
  if (before) {
    lowM = -split.beforeM;
    if (piece > 0u) {
      lowM = (*walk).fromLowestM[piece - 1u];
    }
    highM = 0.0;
    if (piece < (*walk).crossingsBefore) {
      highM = (*walk).fromLowestM[piece];
    }
  } else {
    let at = (*walk).crossingsBefore + piece - (*walk).firstAfter;
    lowM = 0.0;
    if (at > (*walk).crossingsBefore) {
      lowM = (*walk).fromLowestM[at - 1u];
    }
    highM = split.afterM;
    if (at < (*walk).crossings) {
      highM = (*walk).fromLowestM[at];
    }
  }
  (*walk).towardLow = toward;
  (*walk).anchorM = select(highM, lowM, toward);
}

// Step `i` of the ray march's walk cut at its layer edges, the steps taken in order from 0
// (plan R08, R08.T16.a.2): `common.wgsl`'s marchWalkStep, each piece of a side that takes its own
// placement entered by `rayMarchWalkEnter` as the walk reaches it.
fn rayMarchWalkStep(
  walk : ptr<function, MarchWalk>,
  placement : ptr<function, RayMarchPlacement>,
  i : u32,
) -> MarchStep {
  let before = i < (*walk).split.stepsBefore;
  if ((before && (*placement).ownBefore) || (!before && (*placement).ownAfter)) {
    if (!before && (*walk).piece < (*walk).firstAfter) {
      rayMarchWalkEnter(walk, placement, (*walk).firstAfter, false);
    } else if (f32((*walk).taken) >= (*walk).pieceSteps) {
      rayMarchWalkEnter(walk, placement, (*walk).piece + 1u, before);
    }
  }
  return marchWalkStep(walk, i);
}

// The ray march's count for one ray (plan R08, R08.T8.e, lever 3 of `decision-r08-t8-leans.md`
// sections 8 to 10; `marchSteps.ts`'s marchSamplesOf is its twin): `samples`, the setting's, but
// where the view holds the rules: on the bare ground (`kind` 1: nothing drawn, the ray ending on
// the ground, or since R08.T11.b a drawn surface on the datum), the ground's floor at the medium's
// `edges` layer edges while the ray's green optical depth `depthG` is at most the ramp's first
// depth, climbing back to `samples` in even steps by its last (`view.marchGround`); on a sky
// segment (`kind` 2: nothing drawn, the ground not reached), the thin sky's count where its depth
// is under the thin sky's (`view.marchSky`). Any other drawn surface (`kind` 0) keeps `samples`.
// R05.T12.e's gate holds every ray at them, on all three media, at 1 and 6 sources. A depth that is
// not finite takes `samples`.
fn rayMarchSamplesOf(kind : u32, depthG : f32, samples : u32, edges : u32) -> u32 {
  if (kind == 0u || !(abs(depthG) < 3.4e38)) {
    return samples;
  }
  if (kind == 2u) {
    let sky = view.marchSky;
    if (sky.x > 0.0 && depthG < sky.y) {
      return min(samples, u32(sky.x));
    }
    return samples;
  }
  let rule = view.marchGround;
  if (rule.x <= 0.0) {
    return samples;
  }
  let floorSteps = min(samples, u32(rule.x) + u32(rule.y) * edges);
  if (depthG <= rule.z) {
    return floorSteps;
  }
  if (depthG >= rule.w) {
    return samples;
  }
  let pairs = ceil(f32(samples - floorSteps) * (depthG - rule.z) / (2.0 * (rule.w - rule.z)));
  return min(floorSteps + 2u * u32(pairs), samples);
}

// One ray of the march, through `uv` of the output, the scene's reversed-Z depth under the texel
// `depth` (0 where nothing is drawn): its in-scatter (rgb) and mean transmittance (a), and its
// transmittance per channel (R08.T9.c), `marchMissed` where it misses the air. The texel's centre
// ray, and each node of its far-limb footprint (R08.T8.c), each at the scene's depth under its own
// point.
fn marchRay(uv : vec2f, depth : f32) -> MarchedRay {
  let topM = medium.topHeightM;
  let body = oblateBodyOf(oblateBody);
  var tSurface = 3.4e38;
  if (depth > 0.0) {
    tSurface = distanceAtDepth(view, uv, depth);
  }
  let dir = rayThrough(view, uv);
  let origin = view.camera.xyz;
  let a = view.figure.x;
  let c = view.figure.y;
  let k = a / c;
  // Beyond half the equatorial radius, the ground's quadric is taken about the closest approach.
  let far = view.camera.w > 0.5 * a;
  // The top's shell through h* = H at the equator and the poles (R08.T6.f).
  let topA = topM * body.scales.z;
  let topC = topM * body.scales.w;
  let shellQ = rayQuadric(origin, dir, (a + topA) / (c + topC), a + topA, view.exact.y, far);
  let groundQ = rayQuadric(origin, dir, k, a, view.exact.x, far);
  let shell = quadricHits(shellQ);
  let ground = quadricHits(groundQ);
  let tStart = max(shell.x, 0.0);
  var tEnd = shell.y;
  let hitsGround = ground.x > 0.0;
  if (hitsGround) {
    tEnd = min(tEnd, ground.x);
  }
  tEnd = min(tEnd, tSurface);
  if (shell.y <= 0.0 || tEnd <= tStart) {
    return marchMissed();
  }
  // The medium's layer edges are the same for every pixel, so each branch on them is uniform
  // (R08.T6.i).
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  let nearest = -dot(origin, dir);
  // The closest-approach form's terms (below), formed before the count, which a sky segment's
  // lowest point takes.
  let qStar = -groundQ.disc / groundQ.qa;
  let acrossZ = cross(dir, cross(origin, dir)).z;
  let nearestFromStar = (k - 1.0) * (k + 1.0) * dir.z * acrossZ / groundQ.qa;
  // The ray's count (lever 3, R08.T8.e), before the march's own state is made, each depth through
  // the sliced lookup: on the bare ground by its optical depth from the ground back along the ray;
  // on a sky segment by its optical depth from its lowest point t* = clamp(-o.d, tStart, tEnd),
  // the split's, along each side it has, the lookup started there as a step's would be.
  var kind = 0u;
  if (depth == 0.0) {
    kind = select(2u, 1u, hitsGround);
  } else if (hitsGround && view.marchGround.x > 0.0) {
    // Since R08.T11.b (lever A of `decision-r08-t11-frame.md` section 2) a drawn surface on the
    // datum counts as the bare ground where the setting has the rule (high): at or beyond the
    // datum's hit, where the march ends at the hit as on the bare ground, or before it within the
    // band `view.marchSky.z` above the datum, R07's face and terrain's heights (`marchSteps.ts`'s
    // drawnSurfaceKind). Only the count changes; anything else drawn keeps the setting's.
    var drawnHeightM = 0.0;
    if (tSurface < ground.x) {
      var drawnQ : f32;
      if (far) {
        let fromStar = (tSurface - nearest) + nearestFromStar;
        drawnQ = groundQ.qa * fromStar * fromStar + qStar;
      } else {
        drawnQ = tSurface * (groundQ.qa * tSurface + 2.0 * groundQ.qb) + groundQ.qc;
      }
      drawnHeightM = datumPointOf(body, origin + tSurface * dir, drawnQ).heightM;
    }
    if (drawnHeightM <= view.marchSky.z) {
      kind = 1u;
    }
  }
  var depthG = 0.0;
  if (kind == 1u && view.marchGround.x > 0.0) {
    depthG = rayMarchDepthAlong(body, origin + ground.x * dir, 0.0, -dir);
  } else if (kind == 2u && view.marchSky.x > 0.0) {
    let tLow = clamp(nearest, tStart, tEnd);
    let pLow = origin + tLow * dir;
    var bigQ : f32;
    if (far) {
      let fromStar = (tLow - nearest) + nearestFromStar;
      bigQ = groundQ.qa * fromStar * fromStar + qStar;
    } else {
      bigQ = tLow * (groundQ.qa * tLow + 2.0 * groundQ.qb) + groundQ.qc;
    }
    if (tLow > tStart) {
      depthG += rayMarchDepthAlong(body, pLow, bigQ, -dir);
    }
    if (tEnd > tLow) {
      depthG += rayMarchDepthAlong(body, pLow, bigQ, dir);
    }
  }
  let setting = max(u32(view.figure.w), 2u);
  let samples = max(rayMarchSamplesOf(kind, depthG, setting, edgeCount), 2u);
  // Every two-sided sky segment, from inside the air or outside it, under the sun-aware warp
  // (R08.T8); a drawn surface's camera side from inside toward the camera, and every other segment's
  // sides about its lowest point, R05.T12.e's.
  let split = marchSplit(tStart, tEnd, nearest, samples, shell.x <= 0.0 && depth > 0.0);
  let evenSky = depth == 0.0 && split.beforeM > 0.0 && split.afterM > 0.0;
  let count = viewSourceCount();
  // The warp a, once a pixel: from the march's first source, 0 where no source is drawn.
  var warp = 0.0;
  if (evenSky && count > 0u) {
    warp = marchSkyWarp(dir, view.sources[0].direction.xyz);
  }
  // The closest-approach form's offsets: t - t* = (t - t_low) + (t_low - t*), t_low the split's
  // lowest point. Unclamped, t_low is t_o, the unscaled closest approach the split is placed about,
  // and t_o - t* = (k^2 - 1) d_z [d x (P x d)]_z / qa, 0 on a sphere. Clamped at the ground's near
  // root, t_low - t* is that root's own -sqrt(disc) / qa, with no difference of large distances,
  // so that Q is 0 where the segment meets the ground. Clamped elsewhere (a surface drawn before
  // t_o, or a segment's start), t_low - t_o is formed as a difference, which rounds to about
  // eps |P|, the depth's or the shell root's own precision there.
  var lowestFromStar = (split.lowestM - nearest) + nearestFromStar;
  if (hitsGround && split.lowestM == ground.x) {
    lowestFromStar = -sqrt(max(groundQ.disc, 0.0)) / groundQ.qa;
  }
  let ray = MarchRay(
    origin, dir, a, c, body.scales.z, body.scales.w, far, groundQ, qStar, lowestFromStar
  );
  var march = viewMarchStart();
  let lobes = hasForwardLobes();
  // A drawn surface's transmittance with its lobes' in-lobe scattering kept (forward.w, the view's
  // SurfaceLobes; fix-r08-decked-air-opacity), on a ray that ends on a drawn surface alone, whose
  // second target the composite reads (fix-r08-decked-air-route): its optical depth, summed.
  let surfaceLobes = lobes && view.forward.w > 0.5 && depth > 0.0;
  var surfaceDepth = vec3f(0.0);
  // A medium with no layer edge takes R05.T12.e's steps as they were, and declares no walk.
  // One loop a path, each step chosen in it (R08.T8), so that the step's work is compiled twice,
  // not once a placement and path.
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(
        &march,
        &surfaceDepth,
        ray,
        body,
        rayMarchStepOf(split, i, evenSky, warp),
        lobes,
        surfaceLobes,
      );
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    var placement = RayMarchPlacement(vec2u(0u), false, false);
    for (var e = 0u; e < edgeCount; e++) {
      let edgeM = layerEdges.middles[e / 4u][e % 4u];
      marchWalkCrossShell(&walk, ray, body, nearestFromStar, acrossZ, edgeM);
    }
    if (evenSky) {
      marchWalkEvenReady(&walk, warp);
    } else {
      marchWalkReady(&walk);
      rayMarchDepthShares(&walk, &placement, ray, body);
    }
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(
        &march,
        &surfaceDepth,
        ray,
        body,
        rayMarchWalkStepOf(&walk, &placement, i, evenSky),
        lobes,
        surfaceLobes,
      );
    }
  }
  var luminance = march.luminance;
  // The bare ground under the atmosphere, seen from above it where nothing is drawn: each source's
  // light on it through the sliced lookup from its own place (R08.T6.f), weighted as the sky is,
  // through its own slot's throughput (R08.T7), and the source's cosine against the datum's normal
  // there; its light the kernel's read (`rayMarchGroundLightAt`, R08.T8.e). Or, where the ray
  // departs under a decked air's bake (the header's fix-r08-decked-air-route; false in a thin
  // kernel), the drawn ground's light at the ray's end with the bake's ground, S_bar, through the
  // same lookups, its sky's irradiance the thick read's (`skyLit`).
  let bare = depth == 0.0 && hitsGround && ground.x <= tEnd;
  let departs = false;
  if (bare || departs) {
    var p = origin + ground.x * dir;
    var quadricM2 = 0.0;
    for (var drawn = departs; drawn; drawn = false) {
      let place = rayMarchPlaceAt(ray, split.lowestM, tEnd - split.lowestM);
      p = place.p;
      quadricM2 = place.quadricM2;
    }
    let datum = datumPointOf(body, p, quadricM2);
    let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
    let local = sampleMediumAt(s * datum.heightM);
    let start = sampleStartOf(
      body, p, quadricM2, datum.heightM, datum.normal, datum.normal, s, local
    );
    var lit = vec3f(0.0);
    for (var k = 0u; k < count; k++) {
      let source = view.sources[k];
      let slot = skySlotOf(source);
      let muSun = dot(source.direction.xyz, datum.normal);
      let toSun = rayMarchGroundLightAt(body, start, source.direction.xyz, slot);
      if (bare) {
        luminance += viewThroughputOf(&march, slot) * source.weight.rgb * toSun
          * max(muSun, 0.0) * medium.groundAlbedo.rgb / PI_VIEW;
      }
      for (var drawn = departs; drawn; drawn = false) {
        let skyLit = vec3f(0.0);
        lit += source.weight.rgb * (toSun * max(muSun, 0.0) + skyLit);
      }
    }
    for (var drawn = departs; drawn; drawn = false) {
      let groundShare = lit * medium.groundAlbedo.rgb / PI_VIEW;
      let kept = max(exp(-surfaceDepth) - march.throughput0, vec3f(0.0));
      luminance = max(luminance - groundShare * kept, vec3f(0.0));
    }
  }
  // A sky pixel whose ray meets the ground sees nothing of space behind it: no sun's disc there.
  var meanTransmittance = viewMeanTransmittance(&march);
  var transmittanceRgb = march.throughput0;
  if (surfaceLobes) {
    transmittanceRgb = exp(-surfaceDepth);
  }
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    meanTransmittance = 0.0;
    transmittanceRgb = vec3f(0.0);
  }
  return MarchedRay(vec4f(luminance, meanTransmittance), transmittanceRgb);
}
