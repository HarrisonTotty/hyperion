// The sky-irradiance table (plan R08, R08.T9.b; Design notes 7, 17, 18 and 19): for each height and
// sun zenith cosine, the sky's diffuse irradiance on a horizontal surface per unit of the sun's
// illuminance, E = ∫ L(ω) μ_v dω over the directions above the horizon, Bruneton 2017's irradiance
// texture (64 in μ_s by 16 in height, his layout and texel mapping: precomputed_atmospheric_scattering's
// `GetIrradianceTextureUvFromRMuS` and `ComputeIndirectIrradianceTexture`, BSD-3-Clause; the notice
// is in common.wgsl), here from Hillaire's two tables rather than Bruneton's scattering texture, and
// its height axis x_r = √(h ÷ H) in place of his (r − R) ÷ (R_top − R), its rows dense near the
// ground, where a haze's scale height of a kilometre or so lies (Earth's aerosol's 1.2 km). The
// lit passes read it through `surfaceLighting.wgsl`, each sun's illuminance times it on a surface's
// diffuse share. Follows `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `medium.wgsl` and
// `scattering.wgsl`.
//
// Each direction's L is a per-frame march's, as `source.wgsl`'s `sourceAt` forms it on a sphere's
// tables: the single scattering of each term's own phase, shadowed by the ground, with the forward
// lobes' higher orders (R08.T7), and Hillaire's multiple scattering, through the ray's own throughput
// with its absorbers under the light slot's curves; its steps placed toward the texel's point and
// cut at the layer edges, as the multiple-scattering table's (R08.T6.d, R08.T6.i). The directions
// stand in the sun's frame, γ from the sun integrated in u = √(γ ÷ π), the phase tables' own
// coordinate, on two pieces (the rings wholly above the horizon and those it cuts), Gauss–Legendre
// on each, and each cut ring over its arc above the horizon alone, so that no node straddles the
// horizon and the forward peak is resolved (`irradianceQuadrature.ts`, whose rules a test holds
// these constants to): 2 × 8 × 6 directions, one an invocation of the texel's workgroup, summed in
// their order by the first. The sun's frame and every angle are direction cosines and
// `preciseTrig.wgsl`'s forms (Design note 18); heights come from heights.
//
// One layer a latitude band and light slot, as the multiple-scattering table's: each dispatch builds
// layer `buildMultiScatteringLayer()` in the band's gravity-scaled frame (its sphere, every
// coefficient times `buildFactor()`) over the band's own transmittance tables and that layer of the
// multiple-scattering table, with light slot `curveSlot()`'s curves (Design note 17, _Latitude
// bands_; `decision-r08-t6b-floor.md` section 8). Alpha is 0.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
// The transmittance tables the build reads, at layer `buildTransmittanceLayer()`, as the
// multiple-scattering kernel reads them.
@group(0) @binding(3) var transmittance : texture_2d_array<f32>;
// `rgba32float`, so that the twilight sky's irradiance keeps its steps to the Sun's −12° and beyond:
// the sky there is about 6 × 10⁻⁸ of the Sun's illuminance, one half-float step (2⁻²⁴).
@group(0) @binding(4) var irradianceOut : texture_storage_2d_array<rgba32float, write>;
@group(0) @binding(5) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(6) var absorberCurves : texture_2d<f32>;
@group(0) @binding(7) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(8) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(9) var lastAbsorberPressureColumn : texture_2d_array<f32>;
// The multiple-scattering table, read at the layer this build writes.
@group(0) @binding(10) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(11) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(12) var lobeTables : texture_2d_array<f32>;

// Gauss–Legendre nodes in u on each piece, and in ψ on each arc (`irradianceQuadrature.ts`'s
// IRRADIANCE_RING_NODES and IRRADIANCE_ARC_NODES), and the directions a texel takes.
const RING_NODES : u32 = 8u;
const ARC_NODES : u32 = 6u;
const DIRECTIONS : u32 = 96u;

// The rules on [0, 1] (`gaussLegendre`; Abramowitz and Stegun 1964, Table 25.4, mapped).
const RING_T = array<f32, 8>(
  0.0198550717512319, 0.1016667612931866, 0.2372337950418355, 0.4082826787521751,
  0.5917173212478248, 0.7627662049581645, 0.8983332387068135, 0.9801449282487682,
);
const RING_W = array<f32, 8>(
  0.0506142681451884, 0.1111905172266872, 0.1568533229389437, 0.1813418916891810,
  0.1813418916891810, 0.1568533229389437, 0.1111905172266872, 0.0506142681451884,
);
const ARC_T = array<f32, 6>(
  0.0337652428984240, 0.1693953067668677, 0.3806904069584015,
  0.6193095930415985, 0.8306046932331324, 0.9662347571015760,
);
const ARC_W = array<f32, 6>(
  0.0856622461895852, 0.1803807865240693, 0.2339569672863456,
  0.2339569672863456, 0.1803807865240693, 0.0856622461895852,
);

var<workgroup> gathered : array<vec3f, DIRECTIONS>;

// One direction of the texel's hemisphere and its weight in E = Σ w L, sr: its quadrature weight
// times its μ_v; 0 for a node whose ring or piece holds no sky.
struct HemisphereDirection {
  dir : vec3f,
  weight : f32,
}

// Direction k (piece by piece, ring by ring, arc node by arc node; `irradianceDirections`) for a sun
// at zenith cosine `muSun`, the sun at (0, √(1 − μ_s²), μ_s) and the texel's vertical z. The first
// piece holds the rings wholly above the horizon, u in [0, u_a] for a sun above it and [u_b, 1] below
// it, the second the rings it cuts, [u_a, u_b], with u_a = √(γ_a ÷ π), u_b = √(1 − γ_a ÷ π) and
// γ_a = atan2(|μ_s|, √(1 − μ_s²)); a ring at γ with μ_v = A + B cos ψ along it, A = μ_s cos γ and
// B = √(1 − μ_s²) sin γ, takes ψ over [0, atan2(√(B² − A²), −A)] where the horizon cuts it, counted
// twice by the symmetry about the sun's vertical plane.
fn hemisphereDirection(muSun : f32, sun : vec3f, k : u32) -> HemisphereDirection {
  let piece = k / (RING_NODES * ARC_NODES);
  let i = (k / ARC_NODES) % RING_NODES;
  let j = k % ARC_NODES;
  let across = sun.y;
  let gammaA = preciseAtan2(abs(muSun), across);
  let uA = sqrt(gammaA / TRIG_PI);
  let uB = sqrt(max(1.0 - gammaA / TRIG_PI, 0.0));
  var low = uA;
  var high = uB;
  let whole = piece == 0u;
  if (whole) {
    if (muSun >= 0.0) {
      low = 0.0;
      high = uA;
    } else {
      low = uB;
      high = 1.0;
    }
  }
  let span = high - low;
  let u = low + span * RING_T[i];
  let ring = preciseSinCos(TRIG_PI * u * u);
  let a = muSun * ring.y;
  let b = across * ring.x;
  var psiMax = TRIG_PI;
  if (!whole) {
    if (a <= -b) {
      psiMax = 0.0;
    } else if (a < b) {
      psiMax = preciseAtan2(sqrt(max((b - a) * (b + a), 0.0)), -a);
    }
  }
  if (!(psiMax > 0.0)) {
    return HemisphereDirection(sun, 0.0);
  }
  let arc = preciseSinCos(psiMax * ARC_T[j]);
  // cos γ s + sin γ (cos ψ e1 + sin ψ e2), e1 = (0, −μ_s, √(1 − μ_s²)), e2 = (1, 0, 0).
  let dir = vec3f(
    ring.x * arc.x,
    ring.y * across - ring.x * arc.y * muSun,
    ring.y * muSun + ring.x * arc.y * across,
  );
  // The piece's and ring's weights, du's 2π u sin(π u²), the mirror's 2, the arc's and μ_v.
  let ringWeight = span * RING_W[i] * 2.0 * TRIG_PI * u * ring.x * 2.0;
  return HemisphereDirection(dir, ringWeight * psiMax * ARC_W[j] * max(dir.z, 0.0));
}

// A direction's ray from the texel's point: the point's height and position, the direction, its
// zenith cosine, the sun, and the angle between the two, constant along the ray.
struct SkyRay {
  heightM : f32,
  mu : f32,
  origin : vec3f,
  dir : vec3f,
  sun : vec3f,
  angle : ScatteringAngle,
  lobes : bool,
}

// A direction's march so far: the radiance it has gathered, its throughput and its absorber path.
struct SkyMarch {
  luminance : vec3f,
  throughput : vec3f,
  path : AbsorberPath,
}

// One step of a direction's march, at its midpoint: the medium at the sample in the build's frame,
// the sun's light there through the build's transmittance tables under the slot's curves, shadowed
// by the ground, scattered by each term's phase towards the texel with the lobes' higher orders, and
// the multiple scattering, gathered through the ray's own throughput (`source.wgsl`'s `sourceAt` and
// `viewMarchStep` on a sphere's tables).
fn skyStep(march : ptr<function, SkyMarch>, ray : SkyRay, stepAt : MarchStep) {
  let t = stepAt.tM;
  let dt = stepAt.dtM;
  let p = ray.origin + t * ray.dir;
  let rP = length(p);
  let muSun = dot(ray.sun, p / rP);
  let sampleHeightM = heightAlong(ray.heightM, ray.mu, t, rP);
  let local = sampleMediumAt(sampleHeightM);
  let k = buildFactor();
  let bottom = medium.bottomRadiusM;
  let topM = medium.topHeightM;
  let unit = transmittanceUnitAt(bottom, topM, clamp(sampleHeightM, 0.0, topM), muSun);
  let count = absorberCount();
  let slot = curveSlot();
  let layer = buildTransmittanceLayer();
  let toSun = transmittanceThroughLayer(
    transmittance, absorberColumns, absorberPressureColumns, lastAbsorberPressureColumn,
    absorberCurves, layer, count, slot, lineSlots(), unit
  );
  var phased = phasedScatteringOf(local, ray.angle);
  if (ray.lobes) {
    phased += lobeOrdersOf(local, ray.angle, tableAtUnitLayer(transmittance, layer, unit).rgb);
  }
  let lit = select(1.0, 0.0, groundHitFromHeight(bottom, max(sampleHeightM, 0.0), muSun));
  let multiple = tableAtUnitLayer(
    multiScattering, buildMultiScatteringLayer(),
    multiScatteringUnitAt(topM, max(sampleHeightM, 0.0), muSun)
  ).rgb;
  // A latitude band's frame takes every coefficient and absorber column per metre times k.
  let source = (lit * toSun * phased + multiple * local.scattering) * k;
  let none = AbsorberColumns(0.0, vec4f(0.0));
  let path = (*march).path;
  let next = absorberStep(
    absorberCurves, count, slot, lineSlots(), path, addColumns(none, local.columns, k),
    addColumns(none, local.pressureColumns, k), dt
  );
  let stepDepth = local.extinction * k * dt + (next.depth - path.depth);
  (*march).path = next;
  (*march).luminance += (*march).throughput * source * dt * stepFactor(stepDepth);
  (*march).throughput *= exp(-stepDepth);
}

// The sky's radiance per unit of the sun's illuminance along `dir` from `heightM`, over the steps
// the multiple-scattering kernel places on its rays.
fn skyAlong(heightM : f32, dir : vec3f, sun : vec3f, lobes : bool) -> vec3f {
  let bottom = medium.bottomRadiusM;
  let origin = vec3f(0.0, 0.0, bottom + heightM);
  let mu = dir.z;
  let tMax = maxDistanceFromHeight(heightM, mu);
  let samples = max(u32(medium.samples), 2u);
  let split = marchSplit(0.0, tMax, -(bottom + heightM) * mu, samples, true);
  let ray = SkyRay(heightM, mu, origin, dir, sun, scatteringAngle(dir, sun), lobes);
  var march = SkyMarch(vec3f(0.0), vec3f(1.0), absorberPathStart());
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      skyStep(&march, ray, marchStep(split, i));
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    for (var e = 0u; e < edgeCount; e++) {
      marchWalkCrossSphere(&walk, bottom, heightM, mu, layerEdges.middles[e / 4u][e % 4u]);
    }
    marchWalkReady(&walk);
    for (var i = 0u; i < samples; i++) {
      skyStep(&march, ray, marchWalkStep(&walk, i));
    }
  }
  return march.luminance;
}

@compute @workgroup_size(96, 1, 1)
fn main(
  @builtin(workgroup_id) texel : vec3u,
  @builtin(local_invocation_index) k : u32,
) {
  let size = vec2f(textureDimensions(irradianceOut).xy);
  // The texel's unit range (Bruneton's mapping): x the sun's zenith cosine, y the root of the height
  // over the atmosphere's, from the ground to the top.
  let unit = clamp(unitFromTexelUv((vec2f(texel.xy) + 0.5) / size, size), vec2f(0.0), vec2f(1.0));
  let heightM = unit.y * unit.y * medium.topHeightM;
  let muSun = unit.x * 2.0 - 1.0;
  let sun = vec3f(0.0, sqrt(max((1.0 - muSun) * (1.0 + muSun), 0.0)), muSun);
  let one = hemisphereDirection(muSun, sun, k);
  var value = vec3f(0.0);
  if (one.weight > 0.0) {
    value = one.weight * skyAlong(heightM, one.dir, sun, hasForwardLobes());
  }
  gathered[k] = value;
  workgroupBarrier();
  if (k != 0u) {
    return;
  }
  // The directions summed in their order, as the twin sums them.
  var sum = vec3f(0.0);
  for (var d = 0u; d < DIRECTIONS; d++) {
    sum += gathered[d];
  }
  textureStore(irradianceOut, texel.xy, buildMultiScatteringLayer(), vec4f(sum, 0.0));
}
