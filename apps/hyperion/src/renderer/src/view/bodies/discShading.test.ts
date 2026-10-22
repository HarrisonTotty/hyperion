import { describe, expect, it } from "vitest";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { memoised } from "../../test/memo";
import {
  constantNormalMap,
  datumNormalMap,
  UNREAD_TEXTURES,
  unreadNormalMap,
} from "../../test/normalMapFixtures";
import { wgslFunction } from "../../test/wgslText";
import type { PhotometricLaw } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { AIRLESS_LIGHTING, type SurfaceLightingReader } from "../atmosphere/surfaceLighting";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { annulusEdges } from "../lighting/annuli";
import type { PlacedLight } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH, planetshineIrradiance } from "../lighting/planetshine";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import maturityNodesWgsl from "../shaders/maturityNodes.wgsl?raw";
import { faceUvToXyz, stToUv } from "../terrain/cube";
import {
  MATURITY_CODE_MAX,
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  maturitySegment,
} from "../terrain/maturityNodes";
import type { BodyFigure } from "../terrain/planet";
import {
  type CompositePixel,
  compositeDiscPixels,
  DISC_ROWS,
  discLawTerms,
  type DiscRecord,
  discReflectance,
  type DiscSamples,
  discSamples,
  FINE_DISC_PX,
  FIRST_NODE_ROW,
  type OrientedClassMap,
  packDiscRecords,
  pointSampleDisc,
  rasteriseDisc,
  shadeDiscPoint,
  viewRay,
} from "./discShading";
import { type NormalMapTexels, normalMapNormalAt } from "./discNormals";
import {
  classMapAlbedoAt,
  type ClassMapTexel,
  classMapMaturityLayer,
  classMapSharesAt,
  classMapSurface,
  MATURITY_HIGH_CHANNEL,
  MAX_DISC_CLASSES,
  packClassMap,
} from "./discSurface";
import {
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_MATERIALS,
  type LitBodyInput,
  planLitBodies,
  SMOOTH_MESH_MATERIAL,
} from "./draw";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const RAD = Math.PI / 180;

/** The centre pixel's scale, px per radian. */
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));

const RADIUS_M = 6.371e6;

const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  presentedPx: VIEWPORT,
  exposureScale: 1e-3,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/** The figures the ruling's gates take (decision-r07-small-disc-cost §2). */
const FIGURES: ReadonlyArray<readonly [string, BodyFigure]> = [
  ["a sphere", { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: vec3(0, 1, 0) }],
  [
    "an f = 0.098 spheroid equator-on",
    { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M * (1 - 0.098), pole: vec3(0, 1, 0) },
  ],
  [
    "an f = 0.098 spheroid seen from 45° latitude",
    {
      equatorialRadiusM: RADIUS_M,
      polarRadiusM: RADIUS_M * (1 - 0.098),
      pole: vec3(0, Math.SQRT1_2, Math.SQRT1_2),
    },
  ],
];

const SPHERE: BodyFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null };

/**
 * The disc record of a body `px` across at the centre pixel's scale, its centre moved `offsetPx`
 * from the view's centre, lit by a Sun 1 au away at phase `phaseDeg`, as `planLitBodies` plans it.
 */
function recordAt(
  px: number,
  phaseDeg: number,
  figure: BodyFigure,
  offsetPx: readonly [number, number] = [0, 0],
): DiscRecord {
  const distanceM = figure.equatorialRadiusM / Math.sin(px / 2 / PX_PER_RAD);
  const centreM = vec3(
    (offsetPx[0] * distanceM) / PX_PER_RAD,
    (offsetPx[1] * distanceM) / PX_PER_RAD,
    -distanceM,
  );
  const towardsStar = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  const lit = aLitBody();
  const body: LitBodyInput = {
    id: lit.body,
    centreM,
    figure,
    photometry: lit.photometry,
    lighting: undefined,
  };
  const hosts: PlacedLight[] = [
    { disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) },
  ];
  const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
  const record = plan.discs.find((each) => each.body === body.id);
  if (record === undefined) {
    throw new Error(`a body ${String(px)} px across was not drawn as a disc`);
  }
  return record;
}

/** The record with other cells per axis in its interior and limb pixels. */
function withSamples(record: DiscRecord, samples: DiscSamples): DiscRecord {
  return { ...record, interiorSamples: samples.interior, limbSamples: samples.limb };
}

/** The disc's flux at the camera per channel, in the twin's units (pre-exposed light × sr). */
function fluxOf(record: DiscRecord): Rgb {
  const flux: [number, number, number] = [0, 0, 0];
  for (const pixel of compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT))) {
    const ray = viewRay(pixel.xPx + 0.5, pixel.yPx + 0.5, CAMERA, VIEWPORT);
    const omega = pixelSolidAngle(ray, CAMERA, VIEWPORT);
    for (const c of [0, 1, 2] as const) {
      flux[c] += pixel.rgb[c] * omega;
    }
  }
  return flux;
}

/** The 16 placements of the gates: centres on a 4 × 4 sub-pixel grid, px. */
const PLACEMENTS: ReadonlyArray<readonly [number, number]> = Array.from(
  { length: 16 },
  (_, k) => [Math.floor(k / 4) / 4, (k % 4) / 4] as const,
);

/** One pixel's light and covered share, whichever way it was drawn. */
interface PixelValue {
  readonly rgb: Rgb;
  readonly coverage: number;
}

/** How one drawing's pixels depart from another's (decision-r07-small-disc-cost, G5). */
interface PixelErrors {
  /** The worst |drawn − reference| over the pixels and channels, as a share of the channel's peak. */
  readonly worst: number;
  /** The RMS of that share over the pixels either drawing covers, the worst channel's. */
  readonly rms: number;
  /** The worst |drawn − reference| covered share over the pixels either covers only in part. */
  readonly coverage: number;
}

function pixelMap(
  pixels: ReadonlyArray<PixelValue & { readonly xPx: number; readonly yPx: number }>,
): Map<string, PixelValue> {
  return new Map(pixels.map((p) => [`${String(p.xPx)},${String(p.yPx)}`, p]));
}

const DARK: PixelValue = { rgb: [0, 0, 0], coverage: 0 };

/** The errors of `drawn` against `reference`, each channel's against the reference's peak in it. */
function pixelErrors(
  drawn: ReadonlyMap<string, PixelValue>,
  reference: ReadonlyMap<string, PixelValue>,
): PixelErrors {
  const keys = new Set([...reference.keys(), ...drawn.keys()]);
  let worst = 0;
  let rms = 0;
  let coverage = 0;
  for (const c of [0, 1, 2] as const) {
    const peak = Math.max(...[...reference.values()].map((p) => p.rgb[c]));
    let sumSq = 0;
    for (const key of keys) {
      const r = reference.get(key) ?? DARK;
      const g = drawn.get(key) ?? DARK;
      const share = Math.abs(g.rgb[c] - r.rgb[c]) / peak;
      worst = Math.max(worst, share);
      sumSq += share * share;
      if (r.coverage < 1 || g.coverage < 1) {
        coverage = Math.max(coverage, Math.abs(g.coverage - r.coverage));
      }
    }
    rms = Math.max(rms, Math.sqrt(sumSq / keys.size));
  }
  return { worst, rms, coverage };
}

/** The twin's composite of a record as a map from pixel to its value. */
function drawnPixels(record: DiscRecord): Map<string, PixelValue> {
  return pixelMap(compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT)));
}

/**
 * The reference twin (G5, G5′): the same record at 32 × 32 cells in every pixel, and at 64 × 64
 * under {@link FINE_DISC_PX}. A 3 px crescent at 150° is 0.2 px deep, more than 32 × 32's
 * near-limb band (about 0.08 px) reaches: its terminator falls in centre-sampled cells, and the
 * 32 × 32 twin stands 0.32% of the peak from a converged brute force there, the 64 × 64 0.11%.
 */
function referenceSamples(px: number): DiscSamples {
  const cells = px < FINE_DISC_PX ? 64 : 32;
  return { interior: cells, limb: cells };
}

/** The one placement off the sub-pixel grid's centre that G5 and G5′ take, px. */
const OFF_CENTRE: readonly [number, number] = [0.25, 0.75];

/**
 * The reference twin's pixels of a sphere `px` across at `phaseDeg`, placed {@link OFF_CENTRE}:
 * made once for G5 and G5′, which both compare against the 3 px crescent's. It is a pure function
 * of the size and the phase (the figure, the placement and the counts being fixed), and no test
 * writes to it.
 */
const referenceTwinOf = memoised(
  ([px, phaseDeg]: readonly [number, number]): ReadonlyMap<string, PixelValue> =>
    drawnPixels(withSamples(recordAt(px, phaseDeg, SPHERE, OFF_CENTRE), referenceSamples(px))),
  ([px, phaseDeg]) => `${String(px)} ${String(phaseDeg)}`,
);
const referenceTwin = (px: number, phaseDeg: number): ReadonlyMap<string, PixelValue> =>
  referenceTwinOf([px, phaseDeg]);

describe("the cells a disc takes by its size (T8.c)", () => {
  for (const [diameterPx, samples] of [
    [3.99, { interior: 8, limb: 8 }],
    [4, { interior: 4, limb: 4 }],
    [31.99, { interior: 4, limb: 4 }],
    [32, { interior: 1, limb: 4 }],
    [Number.POSITIVE_INFINITY, { interior: 1, limb: 4 }],
  ] as const) {
    it(`takes ${String(samples.interior)} inside and ${String(samples.limb)} on the limb per axis at ${String(diameterPx)} px`, () => {
      expect(discSamples(diameterPx)).toEqual(samples);
    });
  }
});

describe("the flux steps where the count changes (T8.c, G4)", () => {
  // At 4 px, 8 × 8 against 4 × 4; at 31.9 px, 4 × 4 against one inside and 4 × 4 on the limb.
  const steps = [
    [4, { interior: 8, limb: 8 }, { interior: 4, limb: 4 }],
    [31.9, { interior: 4, limb: 4 }, { interior: 1, limb: 4 }],
  ] as const;
  for (const [px, below, above] of steps) {
    for (const [name, figure] of FIGURES) {
      for (const phaseDeg of [0, 90, 120, 150]) {
        it(`changes the flux by at most 0.5% at ${String(px)} px for ${name} at ${String(phaseDeg)}°`, () => {
          let worst = 0;
          for (const offset of PLACEMENTS) {
            const record = recordAt(px, phaseDeg, figure, offset);
            const before = fluxOf(withSamples(record, below));
            const after = fluxOf(withSamples(record, above));
            for (const c of [0, 1, 2] as const) {
              worst = Math.max(worst, Math.abs(after[c] / before[c] - 1));
            }
          }
          expect(worst).toBeLessThan(0.005);
        });
      }
    }
  }
});

describe("each pixel of a small disc against the reference twin (T8.c, G5)", () => {
  // One placement off the sub-pixel grid's centre; the disc drawn with the counts it is planned
  // with. 4.01 px stands for the ruling's 4 px, which a placement off the centre, a little farther
  // off, takes just under 4 px and back to 8 × 8.
  const offset = OFF_CENTRE;
  for (const px of [3, 4.01, 6, 12, 24]) {
    for (const phaseDeg of [0, 90, 150]) {
      it(`keeps every pixel within 1.5% of the peak, the RMS within 0.3% and the coverage within 0.0025 at ${String(px)} px and ${String(phaseDeg)}°`, () => {
        const record = recordAt(px, phaseDeg, SPHERE, offset);
        const errors = pixelErrors(drawnPixels(record), referenceTwin(px, phaseDeg));
        expect(errors.worst, "the worst pixel").toBeLessThan(0.015);
        expect(errors.rms, "the RMS").toBeLessThan(0.003);
        expect(errors.coverage, "the coverage").toBeLessThan(0.0025);
      });
    }
  }

  it("draws each size at the counts the ruling sets", () => {
    const counts = [3, 4.01, 6, 12, 24].map((px) => {
      const record = recordAt(px, 0, SPHERE, offset);
      return [record.interiorSamples, record.limbSamples];
    });
    expect(counts).toEqual([
      [8, 8],
      [4, 4],
      [4, 4],
      [4, 4],
      [4, 4],
    ]);
  });
});

describe("the reference twin against brute force (T8.c, G5′)", () => {
  it("draws a 4 px crescent at 150° within 0.3% of its peak of 256 × 256 point samples in each pixel", () => {
    const record = withSamples(recordAt(4, 150, SPHERE, OFF_CENTRE), referenceSamples(4));
    const brute = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 256));
    expect(pixelErrors(drawnPixels(record), brute).worst).toBeLessThan(0.003);
  });

  // At 3 px the brute force takes 512 × 512 samples a pixel: 256 × 256 is itself 0.20% of the
  // peak from 1,024 × 1,024 on the 0.2 px crescent, 512 × 512 0.017%. It takes several seconds.
  it("draws a 3 px crescent at 150° within 0.3% of its peak of 512 × 512 point samples in each pixel", () => {
    const record = withSamples(recordAt(3, 150, SPHERE, OFF_CENTRE), referenceSamples(3));
    const brute = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 512));
    expect(pixelErrors(referenceTwin(3, 150), brute).worst).toBeLessThan(0.003);
  }, 60_000);
});

describe("the brute force", () => {
  it("covers a pixel at the disc's centre wholly", () => {
    const pixels = pixelMap(pointSampleDisc(recordAt(20, 0, SPHERE), CAMERA, VIEWPORT, 4));
    const centre = `${String(VIEWPORT.widthPx / 2)},${String(VIEWPORT.heightPx / 2)}`;
    expect(pixels.get(centre)?.coverage).toBe(1);
  });

  it("leaves out a corner of the disc's rectangle, which no sample meets", () => {
    const record = recordAt(20, 0, SPHERE);
    const pixels = pixelMap(pointSampleDisc(record, CAMERA, VIEWPORT, 4));
    const corner = `${String(Math.floor(record.rect.leftPx))},${String(Math.floor(record.rect.topPx))}`;
    expect(pixels.has(corner)).toBe(false);
  });

  it("refuses a sample count that is not a positive whole number", () => {
    const record = recordAt(4, 0, SPHERE);
    expect(() => pointSampleDisc(record, CAMERA, VIEWPORT, 0)).toThrow(/positive whole number/u);
  });
});

/** An air whose sky gives every point the same light, and whose ground hides a set sun. */
const AIR: SurfaceLightingReader = {
  sunTransmittance: (_altitudeM, muSun) => (muSun > 0 ? [0.8, 0.8, 0.8] : [0, 0, 0]),
  skyIrradiance: () => [0.05, 0.08, 0.12],
  sourceTransmittance: () => [1, 1, 1],
};

/** A sphere's record straight ahead of the camera, lit by one far star in direction `towards`. */
function litBy(towards: Vec3): DiscRecord {
  const set = annulusEdges(0.6, 0.6, 4);
  return {
    body: "body",
    rect: { leftPx: 0, topPx: 0, rightPx: 1, bottomPx: 1 },
    direction: vec3(0, 0, -1),
    radiusOverDistance: 1e-3,
    pole: vec3(0, 0, 1),
    polarOverEquatorial: 1,
    interiorSamples: 1,
    limbSamples: 4,
    surface: { kind: "uniform", law: lawFor([0.3, 0.25, 0.2], [1.1, 1.1, 1.1], "mars") },
    tableRows: [0],
    exposureOverPi: 1e-4 / Math.PI,
    lights: [
      {
        direction: towards,
        distance: 1e6,
        radius: 1,
        illuminance: [1.1e5, 1e5, 0.9e5],
        annuli: [set, set, set],
      },
    ],
    occluders: [],
    secondaries: [],
  };
}

describe("a star's sky light through an atmosphere (R08.T9.b)", () => {
  it("puts a star's sky light in its eclipse, the air above the point in the shadow with it", () => {
    const latitude = 40 * RAD;
    const q = vec3(Math.cos(latitude), 0, Math.sin(latitude));
    const north = vec3(-Math.sin(latitude), 0, Math.cos(latitude));
    // A star 2° below the point's horizon, its sky light alone, and one 30° up.
    for (const mu of [-Math.sin(2 * RAD), 0.5]) {
      const towards = add(scale(q, mu), scale(north, Math.sqrt(1 - mu * mu)));
      const open = litBy(towards);
      // A body ten radii off along the star's direction, its 11.5° hiding the star's whole disc.
      const umbra: DiscRecord = {
        ...open,
        occluders: [{ centre: add(q, scale(towards, 10)), radius: 2 }],
      };
      const lit = shadeDiscPoint(open, CAMERA, q, scale(q, -1), AIR);
      const hidden = shadeDiscPoint(umbra, CAMERA, q, scale(q, -1), AIR);
      for (const c of [0, 1, 2] as const) {
        expect(lit.radiance[c]).toBeGreaterThan(0);
        expect(hidden.radiance[c]).toBe(0);
      }
    }
  });
});

describe("a disc's laws split by their two terms (R10.T10.f)", () => {
  it("gives the disc's own reflectance by their shares to 10⁻¹⁵ at every point, on a sphere and a spheroid", () => {
    // A lunar law under McEwen's L(α), a terrestrial one, and Lambert's.
    const laws = [
      lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      lawFor([0.3, 0.25, 0.2], [1.3, 1.3, 1.3], "earth"),
      lawFor([0.2, 0.2, 0.2], [1.5, 1.5, 1.5], "lambert"),
    ];
    const shares = [0.5, 0.2, 0.3];
    let worst = 0;
    for (const cOverA of [1, 0.9]) {
      for (const alphaDeg of [0, 30, 90, 150]) {
        const alpha = (alphaDeg * Math.PI) / 180;
        const terms = discLawTerms(laws, cOverA, alpha);
        for (const [mu0, mu] of [
          [1, 1],
          [0.5, 0.1],
          [0.05, 0.9],
        ] as const) {
          const disc = discReflectance(laws, cOverA, shares, mu0, mu, alpha);
          for (const c of [0, 1, 2] as const) {
            const split = terms.reduce(
              (sum, term, m) =>
                sum +
                (shares[m] ?? 0) *
                  (term.lommelSeeliger[c] * ((2 * mu0) / (mu0 + mu)) + term.lambert[c] * mu0),
              0,
            );
            worst = Math.max(worst, Math.abs(split / disc[c] - 1));
          }
        }
      }
    }
    expect(worst).toBeLessThan(1e-15);
  });
});

/** The unit vector `from` turned by `angleRad` towards `to` in their plane. */
function turned(from: Vec3, to: Vec3, angleRad: number): Vec3 {
  const across = normalise(sub(to, scale(from, dot(to, from))));
  return add(scale(from, Math.cos(angleRad)), scale(across, Math.sin(angleRad)));
}

/** A point of a unit sphere whose μ from galactic +z is `mu`, on the x–z plane. */
function pointAt(mu: number): Vec3 {
  return vec3(Math.sqrt(1 - mu * mu), 0, mu);
}

/** A packed record's flag of the field's normal map, row 24's w (R10.T10.g). */
function normalsFlag(record: DiscRecord): number | undefined {
  return packDiscRecords([record])[24 * 4 + 3];
}

/** The lookups a {@link recordingAir} was asked, each by the μ it took. */
interface AskedAir {
  readonly reader: SurfaceLightingReader;
  /** `skyIrradiance`'s μ of the sun, each call in turn. */
  readonly sky: number[];
  /** `sunTransmittance`'s μ of the sun. */
  readonly sun: number[];
  /** `sourceTransmittance`'s μ of the source's centre. */
  readonly source: number[];
}

/** {@link AIR}'s light, recording the μ each lookup is asked at. */
function recordingAir(): AskedAir {
  const asked: AskedAir = {
    reader: {
      skyIrradiance: (altitudeM, muSun, latitudeRad, curve) => {
        asked.sky.push(muSun);
        return AIR.skyIrradiance(altitudeM, muSun, latitudeRad, curve);
      },
      sunTransmittance: (altitudeM, muSun, latitudeRad, azimuthRad, curve) => {
        asked.sun.push(muSun);
        return AIR.sunTransmittance(altitudeM, muSun, latitudeRad, azimuthRad, curve);
      },
      sourceTransmittance: (altitudeM, muCentre, latitudeRad, azimuthRad, radiusRad, curve) => {
        asked.source.push(muCentre);
        return AIR.sourceTransmittance(
          altitudeM,
          muCentre,
          latitudeRad,
          azimuthRad,
          radiusRad,
          curve,
        );
      },
    },
    sky: [],
    sun: [],
    source: [],
  };
  return asked;
}

/** Whether `asked` are `expected`, one for one, each to 10⁻¹². */
function allClose(asked: ReadonlyArray<number>, expected: ReadonlyArray<number>): boolean {
  return (
    asked.length === expected.length &&
    asked.every((mu, k) => Math.abs(mu - (expected[k] ?? Number.NaN)) < 1e-12)
  );
}

/** A lunar regolith class and the body's own law; a Lambert law, whose I/F is A f(α) h. */
const REGOLITH = lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon");
const OWN = lawFor([0.15, 0.14, 0.13], [0.6, 0.6, 0.6], "moon");
const LAMBERT = lawFor([0.2, 0.2, 0.2], [1.5, 1.5, 1.5], "lambert");
/** The camera's direction from every point of {@link litBy}'s far sphere: galactic +z. */
const TO_CAMERA = vec3(0, 0, 1);
/** The ray to each point of {@link litBy}'s far sphere. */
const RAY = vec3(0, 0, -1);
const NORTH = vec3(0, 1, 0);
const EAST = vec3(1, 0, 0);

/**
 * A one-class map surveyed whole under `law`, `elsewhere` its unsurveyed ground's, carrying
 * `normals` (none for `null`), body-fixed as galactic.
 */
function mapped(
  normals: NormalMapTexels | null,
  law: PhotometricLaw = REGOLITH,
  elsewhere: PhotometricLaw = OWN,
): OrientedClassMap {
  const surface = classMapSurface(
    UNREAD_TEXTURES,
    "test",
    packClassMap(64, 1, () => [1]),
    [law],
    elsewhere,
    normals === null ? null : unreadNormalMap(normals),
  );
  return { ...surface, rotation: IDENTITY_ROTATION };
}

/** The map's normal held as the twin decodes it, at the body-fixed point `q`. */
function heldAt(surface: OrientedClassMap, q: Vec3): Vec3 {
  const held =
    surface.normals === null ? null : normalMapNormalAt(surface.normals.texels, [q.x, q.y, q.z]);
  if (held === null) {
    throw new Error("the map holds no normal at the point");
  }
  return held;
}

/** A map holding the one body-fixed `normal` about the body-fixed point `q`. */
function slopeAt(
  q: Vec3,
  normal: Vec3,
  law: PhotometricLaw = REGOLITH,
  elsewhere: PhotometricLaw = OWN,
): OrientedClassMap {
  return mapped(constantNormalMap(5, normal, q, 0.3), law, elsewhere);
}

/** {@link litBy}'s sphere under `surface`, lit by one point-like star towards `towards`. */
function recordOver(
  towards: Vec3,
  surface: OrientedClassMap,
  secondaries: DiscRecord["secondaries"] = [],
): DiscRecord {
  return { ...litBy(towards), surface, tableRows: [0, 1], secondaries };
}

/** The twin's light of `record` at its point `q` along {@link RAY}, through `lighting`. */
function shadeAt(
  record: DiscRecord,
  q: Vec3,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
): ReturnType<typeof shadeDiscPoint> {
  const texels = record.surface.kind === "class-map" ? record.surface.texels : null;
  return shadeDiscPoint(record, CAMERA, q, RAY, lighting, texels);
}

/** {@link shadeAt} of {@link recordOver}. */
function shadeOver(
  towards: Vec3,
  surface: OrientedClassMap,
  q: Vec3,
  lighting: SurfaceLightingReader = AIRLESS_LIGHTING,
): ReturnType<typeof shadeDiscPoint> {
  return shadeAt(recordOver(towards, surface), q, lighting);
}

/** A neighbour 60 radii off towards `direction`, 1 radius across, lighting by planetshine. */
function neighbourTowards(direction: Vec3): DiscRecord["secondaries"][number] {
  return { direction, distance: 60, radius: 1, illuminance: [0.02, 0.025, 0.03] };
}

/**
 * The point at the datum's μ 0.2 lit along its datum's normal, its slope turned from the camera
 * by 0° to 30° in 0.5° steps: past 11.5° its shading normal faces away from the camera, and from
 * 5.8° the law's μ is floored at half the datum's, 0.1.
 */
function turnedAwayNearTheLimb(): Array<ReturnType<typeof shadeDiscPoint>> {
  const q = pointAt(0.2);
  return Array.from({ length: 61 }, (_, k) =>
    shadeOver(q, slopeAt(q, turned(q, RAY, (k * 0.5 * Math.PI) / 180)), q),
  );
}

describe("a disc under the field's normal map (R10.T10.g)", () => {
  it("packs row 24's w 1 where the class map carries a normal map, 0 without one and for a uniform disc", () => {
    const towards = normalise(vec3(1, 0, 1));
    expect([
      normalsFlag(recordOver(towards, mapped(datumNormalMap(6)))),
      normalsFlag(recordOver(towards, mapped(null))),
      normalsFlag(litBy(towards)),
    ]).toEqual([1, 0, 0]);
  });

  it("draws every pixel bit for bit as without one where the map holds no normal", () => {
    // Normals on the far side alone, which the camera does not see: some hundred pixels drawn.
    const far = mapped(datumNormalMap(6, 1, (cell) => cell.centre.z < -0.3));
    const record = recordAt(24, 60, SPHERE);
    const draw = (surface: OrientedClassMap): ReturnType<typeof rasteriseDisc> =>
      rasteriseDisc({ ...record, surface, tableRows: [0, 1] }, CAMERA, VIEWPORT, surface.texels);
    const drawn = draw(far);
    expect([drawn.length > 100, drawn]).toEqual([true, draw(mapped(null))]);
  });

  it("never draws black where a cell's slope faces away near the limb", () => {
    expect(dot(turned(pointAt(0.2), RAY, (30 * Math.PI) / 180), TO_CAMERA)).toBeLessThan(0);
    expect(
      turnedAwayNearTheLimb().filter((value) => !(value.lit && value.radiance[1] > 0)),
    ).toEqual([]);
  });

  it("draws a continuous light as the slope turns away near the limb, under 2% a 0.5° step", () => {
    // No jump at the floor's kink or where the slope turns away.
    const values = turnedAwayNearTheLimb();
    const steps = values
      .slice(1)
      .map((next, k) => Math.abs(next.radiance[1] / (values[k]?.radiance[1] ?? Number.NaN) - 1));
    expect(Math.max(...steps)).toBeLessThan(0.02);
  });

  it("keeps a slope facing the star dark beyond the geometric terminator", () => {
    // The sub-camera point, the star 5° below its datum's horizon, its slope 20° towards the star,
    // whose own plane the star stands 15° above: the datum's tangent plane hides it.
    const q = TO_CAMERA;
    const set = turned(q, EAST, (95 * Math.PI) / 180);
    const facingSet = turned(q, set, (20 * Math.PI) / 180);
    expect(dot(facingSet, set)).toBeGreaterThan(0.25);
    expect(shadeOver(set, slopeAt(q, facingSet), q)).toEqual({ radiance: [0, 0, 0], lit: false });
  });

  it("lights the same slope with the star 5° above the datum's horizon", () => {
    const q = TO_CAMERA;
    const up = turned(q, EAST, (85 * Math.PI) / 180);
    expect(shadeOver(up, slopeAt(q, turned(q, up, (20 * Math.PI) / 180)), q).lit).toBe(true);
  });

  it("takes R08's sky on the datum by a tilted surface's share of it, (1 + n′ · ν) ÷ 2", () => {
    // A sky of the same light everywhere and a ground that hides the sun's direct light.
    const skyAlone: SurfaceLightingReader = { ...AIR, sunTransmittance: () => [0, 0, 0] };
    const q = pointAt(0.8);
    const towards = turned(q, NORTH, (30 * Math.PI) / 180);
    const plain = shadeOver(towards, mapped(null), q, skyAlone);
    const ratios = [10, 25, 40].flatMap((tiltDeg) => {
      const surface = slopeAt(q, turned(q, NORTH, (tiltDeg * Math.PI) / 180));
      const tiltedSky = shadeOver(towards, surface, q, skyAlone);
      const factor = 0.5 * (1 + dot(heldAt(surface, q), q));
      return [0, 1, 2].map((c) => (tiltedSky.radiance[c] ?? 0) / (plain.radiance[c] ?? 0) / factor);
    });
    expect(ratios.map((ratio) => Math.abs(ratio - 1) < 1e-12)).toEqual(ratios.map(() => true));
  });

  it("asks R08's sky, the sun's and a source's transmittance at the datum's μ₀ under a tilted map", () => {
    // A point at μ 0.8, its slope 20° to the north, a star 30° to the north and a neighbour 40° to
    // the south of its datum's normal, both up on both normals.
    const q = pointAt(0.8);
    const star = turned(q, NORTH, (30 * Math.PI) / 180);
    const neighbour = neighbourTowards(turned(q, scale(NORTH, -1), (40 * Math.PI) / 180));
    const surface = slopeAt(q, turned(q, NORTH, (20 * Math.PI) / 180));
    const record = recordOver(star, surface, [neighbour]);
    const air = recordingAir();
    shadeAt(record, q, air.reader);
    // The cosines against the datum's normal (the unit sphere's: q), from the point.
    const light = record.lights[0];
    const towardsStar = normalise(sub(scale(light?.direction ?? star, light?.distance ?? 0), q));
    const towardsNeighbour = normalise(sub(scale(neighbour.direction, neighbour.distance), q));
    const datumStar = dot(q, towardsStar);
    const datumNeighbour = dot(q, towardsNeighbour);
    // Each differs from the shading normal's by more than 0.1, so the test tells them apart.
    const held = heldAt(surface, q);
    expect(
      Math.min(
        Math.abs(dot(held, towardsStar) - datumStar),
        Math.abs(dot(held, towardsNeighbour) - datumNeighbour),
      ),
    ).toBeGreaterThan(0.1);
    expect([
      allClose(air.sky, [datumStar, datumNeighbour]),
      allClose(air.sun, [datumStar]),
      allClose(air.source, [datumNeighbour]),
    ]).toEqual([true, true, true]);
  });

  it("lights a night-side point by planetshine on the shading normal, by planetshineIrradiance(n′) ÷ planetshineIrradiance(ν)", () => {
    // The star 30° below the point's datum horizon; a neighbour 50° from its datum's normal; under
    // Lambert's law, whose I/F is A f(α) times the source's irradiance factor.
    const q = pointAt(0.8);
    const star = turned(q, EAST, (120 * Math.PI) / 180);
    const neighbour = neighbourTowards(turned(q, NORTH, (50 * Math.PI) / 180));
    const towardsNeighbourFromQ = sub(scale(neighbour.direction, neighbour.distance), q);
    const ratios = [10, 25].flatMap((tiltDeg) => {
      const sloped = slopeAt(q, turned(q, NORTH, (tiltDeg * Math.PI) / 180), LAMBERT, LAMBERT);
      const lit = shadeAt(recordOver(star, sloped, [neighbour]), q);
      const plain = shadeAt(recordOver(star, mapped(null, LAMBERT, LAMBERT), [neighbour]), q);
      const expected =
        planetshineIrradiance(
          towardsNeighbourFromQ,
          neighbour.radius,
          neighbour.distance,
          heldAt(sloped, q),
        ) / planetshineIrradiance(towardsNeighbourFromQ, neighbour.radius, neighbour.distance, q);
      return [0, 1, 2].map(
        (c) => (lit.radiance[c] ?? 0) / (plain.radiance[c] ?? Number.NaN) / expected,
      );
    });
    expect(ratios.map((ratio) => Math.abs(ratio - 1) < 1e-12)).toEqual(ratios.map(() => true));
  });
});

// --- A disc under its class map's maturity (plan R10, R10.T1.c's client half) --------------------

/** A `u32` or `f32` constant of a WGSL source. */
function wgslConstant(source: string, name: string): number {
  const found = new RegExp(`const ${name} : (?:u32|f32) = ([0-9.]+)u?;`, "u").exec(source);
  if (found === null) {
    throw new Error(`no constant ${name}`);
  }
  return Number(found[1]);
}

/**
 * Monotone nodes of one class, as `ClassMapMaturity.nodes` holds them: node j in channel c the
 * background `background[c]` times 1 + 1.5 (1 − j ÷ 15), fresh ground 2.5 times it, `f32`.
 */
function oneClassNodes(background: readonly [number, number, number]): Float32Array {
  const nodes = new Float32Array(3 * MATURITY_NODES);
  for (let j = 0; j < MATURITY_NODES; j += 1) {
    for (const c of [0, 1, 2] as const) {
      nodes[3 * j + c] = background[c] * (1 + 1.5 * (1 - j / (MATURITY_NODES - 1)));
    }
  }
  return nodes;
}

/** Node K − 1's channels of a one-class table: its background, as `f32` holds it. */
function backgroundOf(nodes: Float32Array): Rgb {
  const at = 3 * (MATURITY_NODES - 1);
  return [nodes[at] ?? Number.NaN, nodes[at + 1] ?? Number.NaN, nodes[at + 2] ?? Number.NaN];
}

/**
 * A one-class map surveyed whole under `law` with its A set to `nodes`' background, as the store's
 * `classMapLawsOf` sets it, each texel's maturity code `codeOf`'s; body-fixed as galactic.
 */
function matured(
  nodes: Float32Array,
  codeOf: (texel: ClassMapTexel) => number,
  law: PhotometricLaw = LAMBERT,
): OrientedClassMap {
  const surface = classMapSurface(
    UNREAD_TEXTURES,
    "matured",
    packClassMap(64, 1, () => [1], { codeOf, nodes, drawn: [true] }),
    [{ ...law, a: backgroundOf(nodes) }],
    OWN,
  );
  return { ...surface, rotation: IDENTITY_ROTATION };
}

describe("a disc under its class map's maturity (R10.T1.c)", () => {
  const NODES = oneClassNodes([0.3, 0.25, 0.2]);

  it("carries as many rows as the shader, its node rows from the shader's first", () => {
    expect([
      wgslConstant(bodyDiscWgsl, "DISC_ROWS"),
      wgslConstant(bodyDiscWgsl, "FIRST_NODE_ROW"),
    ]).toEqual([DISC_ROWS, FIRST_NODE_ROW]);
    expect(DISC_ROWS).toBe(FIRST_NODE_ROW + MAX_DISC_CLASSES * MATURITY_NODES);
    // K from the nodes' library, which the shader composes and does not restate.
    expect(wgslConstant(maturityNodesWgsl, "MATURITY_NODES")).toBe(MATURITY_NODES);
    expect(bodyDiscWgsl).not.toMatch(/const MATURITY_NODES/u);
    expect(wgslFunction(bodyDiscWgsl, "class_node")).toBe(
      "{ return disc_row(FIRST_NODE_ROW + MATURITY_NODES * k + j); }",
    );
  });

  it("reads a texel's code from the layer past the weights' its packing writes, rebuilt exactly from its two bytes", () => {
    expect(wgslFunction(bodyDiscWgsl, "class_map_maturity")).toBe(
      "{ let bytes = textureLoad(class_weights, texel, face + 6u * ((count + 3u) / 4u), 0); " +
        "let code = round(bytes.r * UNORM8_MAX) * (UNORM8_MAX + 1.0) + round(bytes.g * UNORM8_MAX); " +
        "return code / MATURITY_CODE_MAX; }",
    );
    expect([
      wgslConstant(bodyDiscWgsl, "UNORM8_MAX"),
      wgslConstant(bodyDiscWgsl, "MATURITY_CODE_MAX"),
      MATURITY_HIGH_CHANNEL,
    ]).toEqual([255, MATURITY_CODE_MAX, 0]);
    const misplaced: string[] = [];
    for (let classes = 1; classes <= MAX_DISC_CLASSES; classes += 1) {
      for (const face of [0, 1, 2, 3, 4, 5] as const) {
        if (face + 6 * Math.floor((classes + 3) / 4) !== classMapMaturityLayer(face, classes)) {
          misplaced.push(`${String(classes)} classes, face ${String(face)}`);
        }
      }
    }
    expect(misplaced).toEqual([]);
  });

  it("takes a corner's code only under the record's flag, through the nodes' library and never its own formula", () => {
    const body = wgslFunction(bodyDiscWgsl, "surface_at");
    expect(body).toContain(
      "if (body.maturity) { corner_b[corner] = b; corner_segment[corner] = maturity_segment( class_map_maturity(vec2u(texel + vec2i(1)), at.face, count) ); }",
    );
    expect(body).toContain("maturity_mix(lo, hi, segment.fraction)");
    expect(bodyDiscWgsl).not.toMatch(/fn maturity_segment|fn maturity_mix|pow\(/u);
    expect(wgslFunction(bodyDiscWgsl, "body_of")).toContain("body.maturity = y_row.w > 0.5;");
  });

  it("skips a class of no share before it reads a node, and renormalises its albedo as the shares", () => {
    const body = wgslFunction(bodyDiscWgsl, "surface_at");
    const skip = body.indexOf("if (!(out.shares[k + 1u] > 0.0) || !class_drawn(k)) { continue; }");
    expect(skip).toBeGreaterThan(0);
    expect(skip).toBeLessThan(body.indexOf("class_node(k, segment.node)"));
    expect(body.indexOf("class_node(")).toBe(body.indexOf("class_node(k, segment.node)"));
    expect(body).toContain("out.albedo[k + 1u] = albedo / norm;");
    expect(wgslFunction(bodyDiscWgsl, "class_drawn")).toBe("{ return class_node(k, 0u).w > 0.5; }");
  });

  it("mixes each surveyed corner's nodes j and j + 1 of its class by its segment, weighted by its bilinear weight, as the twin does", () => {
    // The twin, `classMapAlbedoAt`: Σ b T_c(x) ÷ Σ b over the surveyed corners, T_c(x) its
    // `maturityNodeAlbedo`, (1 − f) Tⱼ + f Tⱼ₊₁ of class c's nodes (its weighting against the
    // bilinear mean and an unsurveyed corner in `discSurface.test.ts`).
    expect(wgslFunction(bodyDiscWgsl, "surface_at")).toContain(
      "for (var corner = 0u; corner < 4u; corner = corner + 1u) { " +
        "if (!(corner_b[corner] > 0.0)) { continue; } " +
        "let segment = corner_segment[corner]; " +
        "let lo = class_node(k, segment.node).xyz; " +
        "let hi = class_node(k, segment.node + 1u).xyz; " +
        "albedo = albedo + corner_b[corner] * maturity_mix(lo, hi, segment.fraction); }",
    );
    // The twin under one code everywhere: its class's nodes j and j + 1, read from the table.
    const d = [0.3, -0.5, 0.8] as const;
    const misses = [9_000, 50_000].flatMap((code) => {
      const { node, fraction } = maturitySegment(maturityCodeRatio(code));
      const { texels } = matured(NODES, () => code);
      const albedo = classMapAlbedoAt(texels, d, classMapSharesAt(texels, d))?.[0] ?? null;
      return ([0, 1, 2] as const).map((c) => {
        const lo = NODES[3 * node + c] ?? Number.NaN;
        const hi = NODES[3 * (node + 1) + c] ?? Number.NaN;
        return Math.abs((albedo?.[c] ?? Number.NaN) - ((1 - fraction) * lo + fraction * hi));
      });
    });
    expect(Math.max(...misses)).toBeLessThan(1e-15);
  });

  it("shades each law by its albedo at the point, in the reflectance and the sky's mean A", () => {
    expect(wgslFunction(bodyDiscWgsl, "surface_reflectance")).toContain(
      "reflectance = reflectance + share * surface.albedo[m] * row.rgb * disc_term;",
    );
    expect(wgslFunction(bodyDiscWgsl, "surface_reflectance")).not.toContain("law.a");
    const shade = wgslFunction(bodyDiscWgsl, "shade");
    expect(shade).toContain("mean_a = mean_a + surface.shares[m] * surface.albedo[m];");
    expect(shade.match(/surface_reflectance\(surface, laws,/gu)).toHaveLength(2);
    // Each law's own A where nothing else is drawn.
    expect(wgslFunction(bodyDiscWgsl, "surface_at")).toContain(
      "for (var m = 0u; m <= count; m = m + 1u) { out.albedo[m] = surface_law(m).a; }",
    );
  });

  it("composes the nodes' library before the disc's shading in every module that draws it", () => {
    for (const source of [
      BODY_DISC_MATERIALS.interior.fragmentWgsl,
      BODY_DISC_MATERIALS.limb.fragmentWgsl,
      SMOOTH_MESH_MATERIAL.fragmentWgsl,
      BODY_DISC_CELLS_KERNEL.reference,
    ]) {
      expect(source.indexOf(maturityNodesWgsl)).toBeGreaterThan(0);
      expect(source.indexOf(maturityNodesWgsl)).toBeLessThan(source.indexOf(bodyDiscWgsl));
    }
  });

  it("draws a class's A from its nodes at the point's code: its law's light by T(x) ÷ T(m_body), to 10⁻¹²", () => {
    // Lambert's law, whose I/F is A f(α) μ₀, so that the light is linear in A; one code everywhere.
    const q = pointAt(0.8);
    const towards = turned(q, EAST, (30 * Math.PI) / 180);
    const background = shadeOver(
      towards,
      matured(NODES, () => 0),
      q,
    );
    const misses = [1, 9_000, 32_768, 50_000, 65_535].flatMap((code) => {
      const drawn = shadeOver(
        towards,
        matured(NODES, () => code),
        q,
      );
      const albedo = maturityNodeAlbedo(NODES, 0, maturitySegment(maturityCodeRatio(code)));
      const own = backgroundOf(NODES);
      return ([0, 1, 2] as const).map(
        (c) => drawn.radiance[c] / (background.radiance[c] * (albedo[c] / own[c])) - 1,
      );
    });
    expect(Math.max(...misses.map(Math.abs))).toBeLessThan(1e-12);
    // Fresh ground at 2.5 times the background.
    const fresh = shadeOver(
      towards,
      matured(NODES, () => 65_535),
      q,
    );
    expect(fresh.radiance[1] / background.radiance[1]).toBeCloseTo(2.5, 6);
  });

  it("draws a map whose codes are all 0 as without a maturity, its class's A its node K − 1, to 10⁻¹²", () => {
    const record = recordAt(24, 60, SPHERE);
    const draw = (surface: OrientedClassMap): Map<string, PixelValue> =>
      pixelMap(
        compositeDiscPixels(
          rasteriseDisc(
            { ...record, surface, tableRows: [0, 1] },
            CAMERA,
            VIEWPORT,
            surface.texels,
          ),
        ),
      );
    const moon = lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon");
    const withMaturity = draw(matured(NODES, () => 0, moon));
    const plain = mapped(null, { ...moon, a: backgroundOf(NODES) });
    const without = draw(plain);
    expect(withMaturity.size).toBe(without.size);
    expect(withMaturity.size).toBeGreaterThan(300);
    expect(pixelErrors(withMaturity, without).worst).toBeLessThan(1e-12);
  });

  it("draws a young crater's halo where its codes are, the background elsewhere", () => {
    // Fresh ground within 0.2 rad of the sub-camera point of a 24 px disc, mature elsewhere.
    const record = recordAt(24, 0, SPHERE);
    const centre = vec3(0, 0, 1);
    const surface = matured(NODES, ({ face, i, j }) => {
      const [x, y, z] = faceUvToXyz(face, stToUv((i + 0.5) / 64), stToUv((j + 0.5) / 64));
      return dot(normalise(vec3(x, y, z)), centre) > Math.cos(0.2) ? 65_535 : 0;
    });
    const pixels = compositeDiscPixels(
      rasteriseDisc({ ...record, surface, tableRows: [0, 1] }, CAMERA, VIEWPORT, surface.texels),
    );
    const mature = matured(NODES, () => 0);
    const plain = pixelMap(
      compositeDiscPixels(
        rasteriseDisc(
          { ...record, surface: mature, tableRows: [0, 1] },
          CAMERA,
          VIEWPORT,
          mature.texels,
        ),
      ),
    );
    const cx = VIEWPORT.widthPx / 2;
    const cy = VIEWPORT.heightPx / 2;
    const near = pixels.filter((p) => Math.hypot(p.xPx + 0.5 - cx, p.yPx + 0.5 - cy) < 1);
    const far = pixels.filter((p) => Math.hypot(p.xPx + 0.5 - cx, p.yPx + 0.5 - cy) > 8);
    const brightening = (p: CompositePixel): number =>
      p.rgb[1] / (plain.get(`${String(p.xPx)},${String(p.yPx)}`)?.rgb[1] ?? Number.NaN);
    expect(near.length).toBeGreaterThan(0);
    expect(Math.min(...near.map(brightening))).toBeGreaterThan(2.4);
    expect(Math.max(...far.map((p) => Math.abs(brightening(p) - 1)))).toBeLessThan(1e-12);
  });
});
