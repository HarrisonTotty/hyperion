import { describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { memoised } from "../../test/memo";
import {
  checkerNormalMap,
  datumNormalMap,
  slopedNormalMap,
  UNREAD_TEXTURES,
  unreadNormalMap,
} from "../../test/normalMapFixtures";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { type BodyPhotometry, PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { PhotometricLaw } from "../appearance/law";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import {
  IDENTITY_ROTATION,
  type Rotation3,
  rotateToBody,
  rotation3FromRows,
} from "../coords/rotation";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import type { PlacedLight } from "../lighting/hostLights";
import type { LightingBody } from "../lighting/occluders";
import { PLANETSHINE_SOURCES_HIGH, type SecondarySource } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import { faceUvToXyz, stToUv } from "../terrain/cube";
import type { Face } from "../terrain/patchKey";
import type { BodyFigure } from "../terrain/planet";
import { classRowLaw, PHASE_LAW, type PhaseLawCode } from "../terrain/terrainMaterial";
import {
  classMapPointCacheAngleRad,
  classMapPointNormalSamples,
  classMapPointReflection,
  classMapReflectionRatio,
} from "./classMapPoint";
import { type NormalMapTexels, normalMapNormalAt } from "./discNormals";
import {
  compositeDiscPixels,
  type DiscRecord,
  discReflectance,
  rasteriseDisc,
  viewRay,
} from "./discShading";
import {
  type ClassMapTexels,
  classMapSharesAt,
  classMapSurface,
  discSurfaceLaws,
  packClassMap,
} from "./discSurface";
import { type LitBodyInput, planLitBodies, pointFlux } from "./draw";
import { bodyReflection } from "./oblate";
import { POINT_BELOW_PX } from "./regime";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const EXPOSURE = 1e-3;
const RAD = Math.PI / 180;
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: EXPOSURE,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/** A body of about Iapetus's size, m; the fluxes compared are scale-free, a ÷ D set by the px. */
const RADIUS_M = 7.35e5;

/** The apex of the leading side, body-fixed, about which the dark cap lies. */
const APEX = vec3(0, 1, 0);

/** The dark cap's half-angle about the apex. */
const CAP_RAD = 70 * RAD;

/** A class law of R10.T10.d's construction: its row's law at L(0), with its A. */
function classLaw(code: PhaseLawCode, l0: number, a: number): PhotometricLaw {
  return { ...classRowLaw(code, l0), a: [a, a, a] };
}

/** The dark lag on the Regolith row and the bright ice on the Ice row, both at L(0) 1 (A ≤ 0.60). */
const DARK = classLaw(PHASE_LAW.regolith, 1, 0.04);
const BRIGHT = classLaw(PHASE_LAW.ice, 1, 0.55);

/** The body's photometry, its unsurveyed ground's law: the classes' mean p on the Regolith row. */
const ELSEWHERE = classLaw(PHASE_LAW.regolith, 1, 0.38);
const PHOTOMETRY: BodyPhotometry = { ...PROVISIONAL_PHOTOMETRY, law: ELSEWHERE };

/** A texel's centre's unit direction. */
function texelCentre(face: Face, i: number, j: number, n: number): Vec3 {
  const [x, y, z] = faceUvToXyz(face, stToUv((i + 0.5) / n), stToUv((j + 0.5) / n));
  return normalise(vec3(x, y, z));
}

/**
 * The synthetic Iapetus at R10.T10.d's level, 64 texels a face, through `packClassMap`: the dark
 * class where a texel's centre lies within 70° of the apex, the bright elsewhere, surveyed whole
 * or where `surveyed` holds.
 */
function iapetusMap(surveyed: (direction: Vec3) => boolean = () => true): ClassMapTexels {
  return packClassMap(64, 2, ({ face, i, j }) => {
    const d = texelCentre(face, i, j, 64);
    if (!surveyed(d)) {
      return null;
    }
    return dot(d, APEX) > Math.cos(CAP_RAD) ? [1, 0] : [0, 1];
  });
}

const IAPETUS = iapetusMap();

/** Half the Iapetus surveyed, the survey's edge a circle across the cap and the bright side. */
const HALF_IAPETUS = iapetusMap((d) => d.x + 0.3 * d.z > 0.1);

/** A surface over `map` by R07's construction, a distinct object, so that its cache is its own. */
function surfaceOf(
  map: ClassMapTexels,
  laws: ReadonlyArray<PhotometricLaw> = [DARK, BRIGHT],
  elsewhere: PhotometricLaw = ELSEWHERE,
): ClassMapDiscSurface {
  return classMapSurface(UNREAD_TEXTURES, "iapetus", map, laws, elsewhere);
}

/** The views, body-fixed: the observer's direction and the one the light turns towards. */
const VIEWS = [
  ["the leading side", APEX, vec3(1, 0, 0)],
  ["the trailing side", vec3(0, -1, 0), vec3(1, 0, 0)],
  ["a pole", vec3(0, 0, 1), APEX],
] as const;

/** The task's phases, deg. */
const PHASES_DEG = [0, 60, 120] as const;

/** The light's body-fixed direction at phase α from the observer's, turned towards `across`. */
function lightAt(observer: Vec3, across: Vec3, alphaRad: number): Vec3 {
  return add(scale(observer, Math.cos(alphaRad)), scale(across, Math.sin(alphaRad)));
}

/**
 * The rotation, body-fixed to galactic, that puts the body-fixed `observer` along galactic +z,
 * towards the camera at the origin, turned by 25° about it.
 */
function rotationFacing(observer: Vec3): Rotation3 {
  const seed = Math.abs(observer.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const u = normalise(cross(seed, observer));
  const w = cross(observer, u);
  const e1 = add(scale(u, Math.cos(25 * RAD)), scale(w, Math.sin(25 * RAD)));
  return rotation3FromRows([e1, cross(observer, e1), observer]);
}

const SPHERE: BodyFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null };
const OBLATE: BodyFigure = { ...SPHERE, polarRadiusM: 0.9 * RADIUS_M };

/**
 * The body `diameterPx` across straight ahead with its body-fixed `observer` towards the camera,
 * lit by a Sun 1 au away at phase α, its figure's pole the rotation's.
 */
function iapetusAt(
  surface: ClassMapDiscSurface,
  observer: Vec3,
  across: Vec3,
  alphaRad: number,
  diameterPx: number,
  figure: BodyFigure = SPHERE,
): { readonly body: LitBodyInput; readonly hosts: PlacedLight[] } {
  const rotation = rotationFacing(observer);
  const distanceM = figure.equatorialRadiusM / Math.sin(diameterPx / 2 / PX_PER_RAD);
  const centreM = vec3(0, 0, -distanceM);
  const towards = rotateToBody(rotation, lightAt(observer, across, alphaRad));
  return {
    body: {
      id: "0200080020000000.0301",
      centreM,
      figure: { ...figure, pole: rotateToBody(rotation, vec3(0, 0, 1)) },
      photometry: PHOTOMETRY,
      surface,
      rotation,
      lighting: undefined,
    },
    hosts: [{ disc: aHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
  };
}

/**
 * Sub-pixel samples a side of each pixel of the switch's disc: R07.T8.c's 8 × 8 at 3 px puts its
 * near-limb band, two cells deep and integrated across their profile in √δ as the light of a
 * uniform law needs, over the Iapetus's bright rim 0.06 a inside the limb seen from the leading
 * side, whose albedo step the profile does not follow; at 64 the band lies inside the rim.
 */
const SWITCH_SAMPLES = 64;

/** The disc's flux at the camera per channel, lx: Σ L Ω over its pixels, at `samples` a side. */
function discFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  samples: number,
): [number, number, number] {
  const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
  const drawn = plan.discs[0];
  if (drawn === undefined || body.surface?.kind !== "class-map") {
    throw new Error("the body was not drawn as a class-map disc");
  }
  const record: DiscRecord = { ...drawn, interiorSamples: samples, limbSamples: samples };
  const flux: [number, number, number] = [0, 0, 0];
  for (const pixel of compositeDiscPixels(
    rasteriseDisc(record, CAMERA, VIEWPORT, body.surface.texels),
  )) {
    const omega = pixelSolidAngle(
      viewRay(pixel.xPx + 0.5, pixel.yPx + 0.5, CAMERA, VIEWPORT),
      CAMERA,
      VIEWPORT,
    );
    for (const c of [0, 1, 2] as const) {
      flux[c] += (pixel.rgb[c] * omega) / EXPOSURE;
    }
  }
  return flux;
}

/** The directions of the Fibonacci lattice on the unit sphere, each the centre of 4π ÷ count. */
function fibonacci(count: number): Float64Array {
  const out = new Float64Array(3 * count);
  const turn = Math.PI * (3 - Math.sqrt(5));
  for (let n = 0; n < count; n += 1) {
    const z = 1 - (2 * (n + 0.5)) / count;
    const r = Math.sqrt(1 - z * z);
    out[3 * n] = r * Math.cos(n * turn);
    out[3 * n + 1] = r * Math.sin(n * turn);
    out[3 * n + 2] = z;
  }
  return out;
}

const LATTICE = fibonacci(200_000);

/** A map's shares at each of the lattice's directions, by the reconstruction. */
function latticeShares(map: ClassMapTexels): number[][] {
  return Array.from({ length: LATTICE.length / 3 }, (_, n) =>
    classMapSharesAt(map, [LATTICE[3 * n] ?? 0, LATTICE[3 * n + 1] ?? 0, LATTICE[3 * n + 2] ?? 0]),
  );
}

/**
 * The brute force: the light reflected towards a distant observer over the illuminance and
 * (a ÷ Δ)², (1 ÷ π) Σ `discReflectance` μ dS over the lattice's directions on the spheroid (R05's
 * point of d at (d_x, d_y, d_z c ÷ a), its area 4π ÷ count × c ÷ a |(d_x, d_y, d_z a ÷ c)|), in
 * `f64`, body-fixed, not normalised.
 */
function bruteForce(
  surface: ClassMapDiscSurface,
  cOverA: number,
  shares: ReadonlyArray<ReadonlyArray<number>>,
  light: Vec3,
  observer: Vec3,
): [number, number, number] {
  const laws = discSurfaceLaws(surface);
  const count = LATTICE.length / 3;
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(light, observer))));
  const out: [number, number, number] = [0, 0, 0];
  for (let n = 0; n < count; n += 1) {
    const dx = LATTICE[3 * n] ?? 0;
    const dy = LATTICE[3 * n + 1] ?? 0;
    const nz = (LATTICE[3 * n + 2] ?? 0) / cOverA;
    const length = Math.hypot(dx, dy, nz);
    const mu = (dx * observer.x + dy * observer.y + nz * observer.z) / length;
    const mu0 = (dx * light.x + dy * light.y + nz * light.z) / length;
    if (!(mu > 0 && mu0 > 0)) {
      continue;
    }
    const area = ((4 * Math.PI) / count) * cOverA * length;
    const reflectance = discReflectance(laws, cOverA, shares[n] ?? [], mu0, mu, alpha);
    for (const c of [0, 1, 2] as const) {
      out[c] += (reflectance[c] * mu * area) / Math.PI;
    }
  }
  return out;
}

/** The body as a uniform one of its photometry: no surface, its rotation kept. */
function uniformBody(body: LitBodyInput): LitBodyInput {
  return {
    id: body.id,
    centreM: body.centreM,
    figure: body.figure,
    photometry: body.photometry,
    lighting: body.lighting,
    ...(body.rotation === undefined ? {} : { rotation: body.rotation }),
  };
}

/** The body with its surface but no rotation. */
function unrotatedBody(body: LitBodyInput): LitBodyInput {
  return {
    id: body.id,
    centreM: body.centreM,
    figure: body.figure,
    photometry: body.photometry,
    lighting: body.lighting,
    ...(body.surface === undefined ? {} : { surface: body.surface }),
  };
}

/** Each channel's |measured ÷ expected − 1|, at most. */
function worstRatio(measured: ReadonlyArray<number>, expected: ReadonlyArray<number>): number {
  return Math.max(...measured.map((m, c) => Math.abs(m / (expected[c] ?? Number.NaN) - 1)));
}

/** The uniform reflection of the sphere under `elsewhere`, from `light` to `observer`. */
function uniformAt(light: Vec3, observer: Vec3): ReadonlyArray<number> {
  return bodyReflection({ ...SPHERE, pole: vec3(0, 0, 1) }, ELSEWHERE, light, observer);
}

/** `d` turned by `angleRad` towards the k-th of a spread of tangent directions. */
function turned(d: Vec3, k: number, angleRad: number): Vec3 {
  const seed = Math.abs(d.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const u = normalise(cross(seed, d));
  const w = cross(d, u);
  const t = k * Math.PI * (3 - Math.sqrt(5));
  const along = add(scale(u, Math.cos(t)), scale(w, Math.sin(t)));
  return normalise(add(scale(d, Math.cos(angleRad)), scale(along, Math.sin(angleRad))));
}

describe("the point under a class map (R10.T10.f)", () => {
  for (const [name, observer, across] of VIEWS) {
    for (const phaseDeg of PHASES_DEG) {
      it(`meets the CPU rasteriser's disc at the 3 px switch to 1%, facing ${name} at ${String(phaseDeg)}°`, () => {
        const surface = surfaceOf(IAPETUS);
        const { body, hosts } = iapetusAt(
          surface,
          observer,
          across,
          phaseDeg * RAD,
          POINT_BELOW_PX,
        );
        const point = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
        expect(worstRatio(discFlux(body, hosts, SWITCH_SAMPLES), point)).toBeLessThan(0.01);
      });
    }
  }

  it("equals an f64 brute force over 2 × 10⁵ Fibonacci directions to 0.3%, surveyed whole or in half, on a sphere and a spheroid", () => {
    for (const map of [IAPETUS, HALF_IAPETUS]) {
      const surface = surfaceOf(map);
      const shares = latticeShares(map);
      for (const figure of [SPHERE, OBLATE]) {
        const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
        for (const [, observer, across] of VIEWS) {
          for (const phaseDeg of PHASES_DEG) {
            const light = lightAt(observer, across, phaseDeg * RAD);
            const point = classMapPointReflection(
              figure,
              surface,
              IDENTITY_ROTATION,
              light,
              observer,
            );
            const exact = bruteForce(surface, cOverA, shares, light, observer);
            expect(worstRatio(point, exact)).toBeLessThan(0.003);
          }
        }
      }
    }
    // About 4 s here at a load average of 4.6 (2026-10-19), CPU-bound: ten times that, rounded up
    // to 60 s (`vitest.config.mts`).
  }, 60_000);

  it("gives the uniform point to 10⁻⁶ under a one-class map equal to the uniform law, by starlight and planetshine", () => {
    // A law equal to the photometry's, not the same object, on a map surveyed whole and in half.
    const same = classLaw(PHASE_LAW.regolith, 1, 0.38);
    const neighbour: SecondarySource = {
      body: "0200080020000000.0300",
      direction: normalise(vec3(0.6, 0.3, 0.74)),
      angularRadiusRad: 0.01,
      distanceM: 3.56e9,
      radiusM: 6.0e7,
      illuminance: [0.02, 0.02, 0.02],
    };
    for (const map of [
      packClassMap(64, 1, () => [1]),
      packClassMap(64, 1, ({ i }) => (i < 32 ? [1] : null)),
    ]) {
      const surface = surfaceOf(map, [same]);
      for (const [, observer, across] of VIEWS) {
        for (const phaseDeg of [0, 60, 120, 150]) {
          const { body, hosts } = iapetusAt(surface, observer, across, phaseDeg * RAD, 2, OBLATE);
          const uniform = pointFlux(uniformBody(body), hosts, [], DISC_ANNULI_HIGH, [neighbour]);
          const mapped = pointFlux(body, hosts, [], DISC_ANNULI_HIGH, [neighbour]);
          expect(worstRatio(mapped, uniform)).toBeLessThan(1e-6);
        }
      }
    }
  });

  /** The Iapetus from its leading side at 60°, and its photometry's uniform point unrotated. */
  function leadingAt60(): {
    readonly body: LitBodyInput;
    readonly hosts: PlacedLight[];
    readonly photometry: ReturnType<typeof pointFlux>;
  } {
    const { body, hosts } = iapetusAt(surfaceOf(IAPETUS), APEX, vec3(1, 0, 0), 60 * RAD, 2, OBLATE);
    return {
      body,
      hosts,
      photometry: pointFlux(uniformBody(unrotatedBody(body)), hosts, [], DISC_ANNULI_HIGH),
    };
  }

  it("gives the photometry's point exactly without a rotation", () => {
    const { body, hosts, photometry } = leadingAt60();
    expect(pointFlux(unrotatedBody(body), hosts, [], DISC_ANNULI_HIGH)).toEqual(photometry);
  });

  it("gives the photometry's point exactly under a uniform surface", () => {
    const { body, hosts, photometry } = leadingAt60();
    const uniform: LitBodyInput = {
      ...unrotatedBody(body),
      surface: { kind: "uniform", law: DARK },
    };
    expect(pointFlux(uniform, hosts, [], DISC_ANNULI_HIGH)).toEqual(photometry);
  });

  it("integrates the map once it is oriented, the leading side's dark cap dimming the point", () => {
    const { body, hosts, photometry } = leadingAt60();
    expect(pointFlux(body, hosts, [], DISC_ANNULI_HIGH)[1]).toBeLessThan(0.5 * photometry[1]);
  });

  it("cuts each star's integral by the photometry's eclipse over the disc, as the uniform point's", () => {
    const surface = surfaceOf(IAPETUS);
    const { body, hosts } = iapetusAt(surface, APEX, vec3(1, 0, 0), 30 * RAD, 2);
    const star = hosts[0];
    if (star === undefined) {
      throw new Error("no star");
    }
    // An occluder a tenth of the way to the star, as large as it seen from the body and off its
    // centre by under its radius: the body in its penumbra.
    const towards = normalise(add(star.centreM, scale(body.centreM, -1)));
    const occluder: LightingBody = {
      id: "0200080020000000.0302",
      centreM: add(body.centreM, add(scale(towards, 0.1 * AU_M), vec3(0, 5e7, 0))),
      radiusM: 7e7,
    };
    const share = (input: LitBodyInput): number =>
      pointFlux(input, hosts, [occluder], DISC_ANNULI_HIGH)[1] /
      pointFlux(input, hosts, [], DISC_ANNULI_HIGH)[1];
    const eclipsed = share(body);
    expect(eclipsed).toBeGreaterThan(0.05);
    expect(eclipsed).toBeLessThan(0.95);
    expect(Math.abs(eclipsed - share(uniformBody(body)))).toBeLessThan(1e-12);
  });

  /**
   * Each view and phase of the cache's checks: a fresh surface whose point was found there, the
   * light there, and the ratio it was found with.
   */
  function primed(): Array<{
    readonly surface: ClassMapDiscSurface;
    readonly light: Vec3;
    readonly observer: Vec3;
    readonly phaseRad: number;
    readonly held: ReadonlyArray<number>;
  }> {
    return VIEWS.flatMap(([, observer, across]) =>
      [0, 30, 60, 90, 120, 150].map((phaseDeg) => {
        const light = lightAt(observer, across, phaseDeg * RAD);
        const surface = surfaceOf(IAPETUS);
        const held = classMapReflectionRatio(SPHERE, surface, light, observer);
        classMapPointReflection(SPHERE, surface, IDENTITY_ROTATION, light, observer);
        return { surface, light, observer, phaseRad: phaseDeg * RAD, held };
      }),
    );
  }

  it("reuses the ratio it found while both directions stay within δ of where it found it", () => {
    const misses = primed().filter(({ surface, light, observer, phaseRad, held }) => {
      // Both directions moved by just under δ, which the moved phase cannot narrow past.
      const delta = 0.98 * classMapPointCacheAngleRad(phaseRad);
      return Array.from({ length: 8 }, (_, k) => k).some((k) => {
        const movedLight = turned(light, k, delta);
        const movedObserver = turned(observer, 3 * k + 1, delta);
        const cached = classMapPointReflection(
          SPHERE,
          surface,
          IDENTITY_ROTATION,
          movedLight,
          movedObserver,
        );
        const expected = uniformAt(movedLight, movedObserver).map(
          (u, c) => u * (held[c] ?? Number.NaN),
        );
        return cached.some((value, c) => value !== expected[c]);
      });
    });
    expect(misses).toEqual([]);
  });

  it("keeps a cached value within 0.3% of a fresh one over the Iapetus's phases", () => {
    let worst = 0;
    for (const { surface, light, observer, phaseRad } of primed()) {
      const delta = 0.98 * classMapPointCacheAngleRad(phaseRad);
      for (let k = 0; k < 8; k += 1) {
        const movedLight = turned(light, k, delta);
        const movedObserver = turned(observer, 3 * k + 1, delta);
        const cached = classMapPointReflection(
          SPHERE,
          surface,
          IDENTITY_ROTATION,
          movedLight,
          movedObserver,
        );
        const fresh = classMapReflectionRatio(SPHERE, surface, movedLight, movedObserver);
        worst = Math.max(
          worst,
          worstRatio(
            cached,
            uniformAt(movedLight, movedObserver).map((u, c) => u * (fresh[c] ?? 0)),
          ),
        );
      }
    }
    expect(worst).toBeLessThan(0.003);
  });

  it("finds the ratio afresh once the light moves past δ", () => {
    const stale = primed().filter(({ surface, light, observer, phaseRad }) => {
      const far = turned(light, 5, 1.05 * classMapPointCacheAngleRad(phaseRad));
      const ratio = classMapReflectionRatio(SPHERE, surface, far, observer);
      const expected = uniformAt(far, observer).map((u, c) => u * (ratio[c] ?? Number.NaN));
      return classMapPointReflection(SPHERE, surface, IDENTITY_ROTATION, far, observer).some(
        (value, c) => value !== expected[c],
      );
    });
    expect(stale).toEqual([]);
  });

  it("refuses a surface whose laws are not one per class of its map", () => {
    const surface = surfaceOf(IAPETUS);
    expect(() => classMapReflectionRatio(SPHERE, { ...surface, laws: [DARK] }, APEX, APEX)).toThrow(
      /2 classes needs as many laws, got 1/,
    );
  });
});

describe("the point under a class map with the field's normal map (R10.T10.g)", () => {
  /** The cells' slopes: ±12° along each face's s in a checkerboard at the field's level 6. */
  const TILT_RAD = 12 * RAD;
  const CHECKER = checkerNormalMap(6, TILT_RAD);
  /**
   * Band-limited slopes: ridges along each face's t, the slope 12° sin(2π (i + ½) ÷ 4) along s,
   * four cells a period, on a sphere and the spheroid; and one slope of 12° along +s everywhere.
   */
  const RIDGES = slopedNormalMap(6, ({ i }) => TILT_RAD * Math.sin((2 * Math.PI * (i + 0.5)) / 4));
  const RIDGES_OBLATE = slopedNormalMap(
    6,
    ({ i }) => TILT_RAD * Math.sin((2 * Math.PI * (i + 0.5)) / 4),
    0.9,
  );
  const SLANT = slopedNormalMap(6, () => TILT_RAD);
  /**
   * The checkerboard and the ridges at field level 8, 256 normal texels a face's side, where each of
   * the point's elements, 128 a side, centres on a corner four normal texels share and so takes
   * 4 × 4 normals (`classMapPointNormalSamples`): made once, on first use, since a level-8 map
   * takes a second.
   */
  const levelEight = memoised(
    (kind: "checkerboard" | "ridges"): NormalMapTexels =>
      kind === "checkerboard"
        ? checkerNormalMap(8, TILT_RAD)
        : slopedNormalMap(8, ({ i }) => TILT_RAD * Math.sin((2 * Math.PI * (i + 0.5)) / 4)),
    (kind) => kind,
  );

  it("takes two normals a normal texel's side in each element: one at levels 5 and 6, 2 × 2 at 7, 4 × 4 at 8", () => {
    expect([32, 64, 128, 256].map((n) => classMapPointNormalSamples(128, n))).toEqual([1, 1, 2, 4]);
  });

  /** The Iapetus's surface with `normals`, a distinct object, so that its cache is its own. */
  function slopedOf(map: ClassMapTexels, normals: NormalMapTexels): ClassMapDiscSurface {
    return classMapSurface(
      UNREAD_TEXTURES,
      "iapetus",
      map,
      [DARK, BRIGHT],
      ELSEWHERE,
      unreadNormalMap(normals, "iapetus normals"),
    );
  }

  /** A map's normal at each of the lattice's directions, `null` where it holds none. */
  function latticeNormals(normals: NormalMapTexels): Array<Vec3 | null> {
    return Array.from({ length: LATTICE.length / 3 }, (_, n) =>
      normalMapNormalAt(normals, [
        LATTICE[3 * n] ?? 0,
        LATTICE[3 * n + 1] ?? 0,
        LATTICE[3 * n + 2] ?? 0,
      ]),
    );
  }

  /**
   * The brute force with the slopes: {@link bruteForce}'s sum with each direction's I/F on its
   * shading normal n′ (the datum's where `normals` holds none), μ′ = max(n′ · v, μ ÷ 2), and its
   * projected area the datum's, over the directions lit on both normals and visible on the datum's.
   */
  function slopedBruteForce(
    surface: ClassMapDiscSurface,
    cOverA: number,
    shares: ReadonlyArray<ReadonlyArray<number>>,
    normals: ReadonlyArray<Vec3 | null>,
    light: Vec3,
    observer: Vec3,
  ): [number, number, number] {
    const laws = discSurfaceLaws(surface);
    const count = LATTICE.length / 3;
    const alpha = Math.acos(Math.min(1, Math.max(-1, dot(light, observer))));
    const out: [number, number, number] = [0, 0, 0];
    for (let n = 0; n < count; n += 1) {
      const dx = LATTICE[3 * n] ?? 0;
      const dy = LATTICE[3 * n + 1] ?? 0;
      const nz = (LATTICE[3 * n + 2] ?? 0) / cOverA;
      const length = Math.hypot(dx, dy, nz);
      const mu = (dx * observer.x + dy * observer.y + nz * observer.z) / length;
      const mu0 = (dx * light.x + dy * light.y + nz * light.z) / length;
      if (!(mu > 0 && mu0 > 0)) {
        continue;
      }
      const tilted = normals[n] ?? null;
      const mu0Shaded = tilted === null ? mu0 : dot(tilted, light);
      const muShaded = tilted === null ? mu : Math.max(dot(tilted, observer), 0.5 * mu);
      const area = ((4 * Math.PI) / count) * cOverA * length;
      const reflectance = discReflectance(
        laws,
        cOverA,
        shares[n] ?? [],
        mu0Shaded,
        muShaded,
        alpha,
      );
      for (const c of [0, 1, 2] as const) {
        out[c] += (reflectance[c] * mu * area) / Math.PI;
      }
    }
    return out;
  }

  for (const [mapName, normalsOf] of [
    ["a checkerboard of ±12° cells", (): NormalMapTexels => CHECKER],
    ["one slope of 12°", (): NormalMapTexels => SLANT],
    ["a level-8 checkerboard of ±12° cells", (): NormalMapTexels => levelEight("checkerboard")],
    ["level-8 ridges four cells a period", (): NormalMapTexels => levelEight("ridges")],
  ] as const) {
    for (const [name, observer, across] of VIEWS) {
      for (const phaseDeg of PHASES_DEG) {
        it(`meets the CPU rasteriser's disc at the 3 px switch to 1% under ${mapName}, facing ${name} at ${String(phaseDeg)}°`, () => {
          const { body, hosts } = iapetusAt(
            slopedOf(IAPETUS, normalsOf()),
            observer,
            across,
            phaseDeg * RAD,
            POINT_BELOW_PX,
          );
          const point = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
          expect(worstRatio(discFlux(body, hosts, SWITCH_SAMPLES), point)).toBeLessThan(0.01);
        });
      }
    }
  }

  /** The point's largest error against the brute force over the views and phases. */
  function worstAgainstBruteForce(figure: BodyFigure, normals: NormalMapTexels): number {
    const shares = latticeShares(IAPETUS);
    const surface = slopedOf(IAPETUS, normals);
    const tilted = latticeNormals(normals);
    const cOverA = figure.polarRadiusM / figure.equatorialRadiusM;
    let worst = 0;
    for (const [, observer, across] of VIEWS) {
      for (const phaseDeg of PHASES_DEG) {
        const light = lightAt(observer, across, phaseDeg * RAD);
        const point = classMapPointReflection(figure, surface, IDENTITY_ROTATION, light, observer);
        const exact = slopedBruteForce(surface, cOverA, shares, tilted, light, observer);
        worst = Math.max(worst, worstRatio(point, exact));
      }
    }
    return worst;
  }

  it("equals the f64 brute force with band-limited and coherent slopes to 0.3%, on a sphere and a spheroid", () => {
    // Measured (2026-10-21, against 2 × 10⁶ directions): ridges four cells a period at most
    // 1.8 × 10⁻³, eight cells 7.4 × 10⁻⁴, one slope 1.1 × 10⁻³.
    for (const [figure, normals] of [
      [SPHERE, RIDGES],
      [OBLATE, RIDGES_OBLATE],
      [SPHERE, SLANT],
    ] as const) {
      expect(worstAgainstBruteForce(figure, normals)).toBeLessThan(0.003);
    }
    // About 2 s here (2026-10-21): ten times that, rounded up.
  }, 60_000);

  it("equals the f64 brute force to 1% with slopes at the map's own resolution, a ±12° checkerboard of cells", () => {
    // The elements, two a cell's side at its quarter points, see the bilinear saddle between a
    // cell and its neighbours of the other sign at a quarter of its tilt: 6.5 × 10⁻³ at most,
    // facing a pole at 120° (2026-10-21, against 8 × 10⁶ directions), R07's 1% the bound.
    expect(worstAgainstBruteForce(SPHERE, CHECKER)).toBeLessThan(0.01);
  }, 60_000);

  /** The ratio under `normals` against the ratio without a map, at most over the views at 120°. */
  function movedAt120(normals: NormalMapTexels): number {
    return Math.max(
      ...VIEWS.map(([, observer, across]) => {
        const light = lightAt(observer, across, 120 * RAD);
        const flat = classMapReflectionRatio(SPHERE, surfaceOf(IAPETUS), light, observer);
        const mapped = classMapReflectionRatio(SPHERE, slopedOf(IAPETUS, normals), light, observer);
        return worstRatio(mapped, flat);
      }),
    );
  }

  it("moves the point by a slope of 12° at high phase", () => {
    expect(movedAt120(SLANT)).toBeGreaterThan(0.01);
  });

  it("keeps the point within 10⁻³ of the map's without one under the datum's own normals", () => {
    expect(movedAt120(datumNormalMap(6))).toBeLessThan(1e-3);
  });

  it("keeps a cached value within 0.3% of a fresh one over the Iapetus's phases with sloped cells", () => {
    let worst = 0;
    for (const [, observer, across] of VIEWS) {
      for (const phaseDeg of [0, 30, 60, 90, 120, 150]) {
        const light = lightAt(observer, across, phaseDeg * RAD);
        const surface = slopedOf(IAPETUS, CHECKER);
        classMapPointReflection(SPHERE, surface, IDENTITY_ROTATION, light, observer);
        const delta = 0.98 * classMapPointCacheAngleRad(phaseDeg * RAD);
        for (let k = 0; k < 8; k += 1) {
          const movedLight = turned(light, k, delta);
          const movedObserver = turned(observer, 3 * k + 1, delta);
          const cached = classMapPointReflection(
            SPHERE,
            surface,
            IDENTITY_ROTATION,
            movedLight,
            movedObserver,
          );
          const fresh = classMapReflectionRatio(SPHERE, surface, movedLight, movedObserver);
          worst = Math.max(
            worst,
            worstRatio(
              cached,
              uniformAt(movedLight, movedObserver).map((u, c) => u * (fresh[c] ?? 0)),
            ),
          );
        }
      }
    }
    expect(worst).toBeLessThan(0.003);
  });
});
