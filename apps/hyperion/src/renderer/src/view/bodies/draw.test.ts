import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  aHostDisc,
  aLitBody,
  photometryFor,
  planetPhotometry,
  SUN_RADIUS_M,
} from "../../test/litFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { phaseFactorFromTable, phaseFactorTableOf } from "../appearance/law";
import { brdf } from "../appearance/brdf";
import { lawFor } from "../appearance/phase";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import {
  litNeighbours,
  PLANETSHINE_SOURCES_HIGH,
  PLANETSHINE_SOURCES_LOW,
  planetshineSources,
} from "../lighting/planetshine";
import { starIlluminance } from "../lighting/illuminance";
import { sphereIrradianceFactor } from "../lighting/sphereIrradiance";
import { oblateAlbedoScale } from "./oblate";
import { AU_M } from "../scenes/kept";
import { METER_CLASS } from "../post/meter";
import {
  compositeDiscPixels,
  DISC_ROWS,
  packDiscRecords,
  rasteriseDisc,
  viewRay,
} from "./discShading";
import { hostAnnuli, type PlacedLight } from "../lighting/hostLights";
import {
  type BodyFrameOptions,
  LitBodyRenderer,
  type LitBodyInput,
  planLitBodies,
  pointFlux,
} from "./draw";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import type { DrawItem } from "../engine/types";
import { sphereFootprint } from "./regime";
import { DISC_LIMB_DEPTHS, rotationColumns } from "./smoothMesh";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const EXPOSURE = 1e-3;
const RAD = Math.PI / 180;

/** The centre pixel's scale, px per radian. */
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));

/** A body straight ahead at `distanceM`, lit by a Sun 1 au away at phase `phaseDeg`. */
function scene(
  distanceM: number,
  phaseDeg: number,
  body: Partial<LitBodyInput> = {},
): { readonly body: LitBodyInput; readonly hosts: PlacedLight[] } {
  const centreM = vec3(0, 0, -distanceM);
  const towardsStar = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
  const lit = aLitBody();
  return {
    body: {
      id: lit.body,
      centreM,
      figure: lit.figure,
      photometry: lit.photometry,
      lighting: undefined,
      ...body,
    },
    hosts: [{ disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) }],
  };
}

const OPTIONS = {
  camera: CAMERA,
  viewport: VIEWPORT,
  exposureScale: EXPOSURE,
  annuli: 4,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high" as const,
};

/**
 * The disc's flux at the camera per channel, lx: Σ L Ω over its pixels, as drawn, with `others`
 * in the scene (as occluders).
 */
function discFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  others: ReadonlyArray<LitBodyInput> = [],
  options: BodyFrameOptions = OPTIONS,
): [number, number, number] {
  const plan = planLitBodies([body, ...others], hosts, options, new Map([[body.id, "disc"]]));
  const record = plan.discs.find((each) => each.body === body.id);
  if (record === undefined) {
    throw new Error("the body was not drawn as a disc");
  }
  const flux: [number, number, number] = [0, 0, 0];
  const drawn = rasteriseDisc(record, CAMERA, VIEWPORT);
  for (const pixel of compositeDiscPixels(drawn)) {
    const ray = viewRay(pixel.xPx + 0.5, pixel.yPx + 0.5, CAMERA, VIEWPORT);
    const omega = pixelSolidAngle(ray, CAMERA, VIEWPORT);
    for (const c of [0, 1, 2] as const) {
      flux[c] += (pixel.rgb[c] * omega) / EXPOSURE;
    }
  }
  return flux;
}

/** Midpoint intervals of the near-field oracle in the polar angle and in azimuth. */
const ORACLE_THETA = 400;
const ORACLE_PHI = 800;

/**
 * The body's exact flux at the camera per channel, lx, from a finite distance: the disc's own law
 * (its horizon term, the Lommel–Seeliger floor and its oblate albedo scale) integrated over the
 * visible, lit spheroid in `f64`, each element weighted by μ dA ÷ r² (decision-r07-t8a, follow-up
 * (b)). No eclipses: the bodies tested have no occluders.
 */
function nearFieldFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
): [number, number, number] {
  const { equatorialRadiusM: a, polarRadiusM: c } = body.figure;
  const pole = body.figure.pole === null ? vec3(0, 0, 1) : normalise(body.figure.pole);
  const seed = Math.abs(pole.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const ex = normalise(cross(seed, pole));
  const ey = cross(pole, ex);
  const { law } = body.photometry;
  const table = phaseFactorTableOf(law);
  const scaleA = oblateAlbedoScale(law.lommelSeeligerShare, c / a);
  const flux: [number, number, number] = [0, 0, 0];
  const dTheta = Math.PI / ORACLE_THETA;
  const dPhi = (2 * Math.PI) / ORACLE_PHI;
  for (const host of hosts) {
    const e = starIlluminance(host.disc, norm(sub(host.centreM, body.centreM)));
    for (let i = 0; i < ORACLE_THETA; i += 1) {
      const theta = (i + 0.5) * dTheta;
      const sinT = Math.sin(theta);
      const cosT = Math.cos(theta);
      for (let j = 0; j < ORACLE_PHI; j += 1) {
        const phi = (j + 0.5) * dPhi;
        const sx = sinT * Math.cos(phi);
        const sy = sinT * Math.sin(phi);
        const local = (u: number, v: number, w: number) =>
          vec3(
            u * ex.x + v * ey.x + w * pole.x,
            u * ex.y + v * ey.y + w * pole.y,
            u * ex.z + v * ey.z + w * pole.z,
          );
        const x = add(body.centreM, local(a * sx, a * sy, c * cosT));
        const normal = normalise(local(sx / a, sy / a, cosT / c));
        const area =
          a * sinT * Math.sqrt(c * c * sinT * sinT + a * a * cosT * cosT) * dTheta * dPhi;
        const r = norm(x);
        const toCamera = scale(x, -1 / r);
        const mu = dot(normal, toCamera);
        if (mu <= 0) {
          continue;
        }
        const toStar = sub(host.centreM, x);
        const d = norm(toStar);
        const towards = scale(toStar, 1 / d);
        const mu0 = dot(normal, towards);
        const horizon = sphereIrradianceFactor(
          d / host.disc.radius_m,
          Math.acos(Math.min(1, Math.max(-1, mu0))),
        );
        if (horizon <= 0) {
          continue;
        }
        const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towards, toCamera))));
        const f = phaseFactorFromTable(table, alpha);
        const share = law.lommelSeeligerShare;
        const floor = Math.asin(Math.min(1, host.disc.radius_m / d));
        const discTerm =
          (share * 2 * horizon) / Math.max(Math.max(mu0, 0) + mu, floor) + (1 - share) * horizon;
        const weight = (mu * area) / (r * r) / Math.PI;
        for (const ch of [0, 1, 2] as const) {
          flux[ch] += e[ch] * law.a[ch] * scaleA * f[ch] * discTerm * weight;
        }
      }
    }
  }
  return flux;
}

describe("the disc and the point at the 3 px switch", () => {
  const radius = 6.371e6;
  /** The distance at which the body is `px` across at the centre pixel's scale. */
  const distanceFor = (px: number): number => radius / Math.sin(px / 2 / PX_PER_RAD);
  /**
   * The worst |disc ÷ reference − 1| per channel over 16 sub-pixel placements of the centre: the
   * point's flux at the switch (3 and 3.3 px), or the exact near-field surface integral where no
   * point is drawn (decision-r07-t8a, follow-up (b)).
   */
  const worstOver = (
    px: number,
    phaseDeg: number,
    body: Partial<LitBodyInput>,
    reference: "point" | "near-field" = "point",
  ): number => {
    const distance = distanceFor(px);
    let worst = 0;
    let nearField: [number, number, number] | null = null;
    for (let i = 0; i < 4; i += 1) {
      for (let j = 0; j < 4; j += 1) {
        const offset = vec3(
          ((i / 4) * distance) / PX_PER_RAD,
          ((j / 4) * distance) / PX_PER_RAD,
          0,
        );
        const placed = scene(distance, phaseDeg, body);
        const moved: LitBodyInput = { ...placed.body, centreM: add(placed.body.centreM, offset) };
        const hosts = placed.hosts.map((host): PlacedLight => ({
          disc: host.disc,
          centreM: add(host.centreM, offset),
        }));
        const disc = discFlux(moved, hosts);
        // The near field moves by under 10⁻⁴ over a pixel's offsets: taken once, centred.
        nearField ??= reference === "point" ? null : nearFieldFlux(moved, hosts);
        const truth = nearField ?? pointFlux(moved, hosts, [], DISC_ANNULI_HIGH);
        for (const c of [0, 1, 2] as const) {
          worst = Math.max(worst, Math.abs(disc[c] / truth[c] - 1));
        }
      }
    }
    return worst;
  };
  const figures = [
    ["a sphere", { equatorialRadiusM: radius, polarRadiusM: radius, pole: vec3(0, 1, 0) }],
    [
      "an f = 0.098 spheroid equator-on",
      { equatorialRadiusM: radius, polarRadiusM: radius * (1 - 0.098), pole: vec3(0, 1, 0) },
    ],
  ] as const;
  for (const [name, figure] of figures) {
    for (const px of [3, 3.3, 6]) {
      for (const phaseDeg of [0, 90, 150]) {
        // At 3 and 3.3 px the disc meets the point it switches with; at 6 px, where no point is
        // drawn, the exact near-field integral of the same law, the far-field point being the less
        // accurate there (decision-r07-t8a, follow-up (b)).
        const reference = px === 6 ? "near-field" : "point";
        it(`sums to the ${reference} flux within 1% for ${name} at ${String(px)} px and ${String(phaseDeg)}°`, () => {
          expect(worstOver(px, phaseDeg, { figure }, reference)).toBeLessThan(0.01);
        });
      }
    }
  }

  it("pins the far-field point's own near-field error at 6 px: −6.1 a ÷ D at 150°, −0.59 a ÷ D at 90°", () => {
    // From D the visible cap stops asin(a ÷ D) short of the hemisphere and each element's weight
    // μ ÷ r² changes by (a ÷ D)(3μ² − 1), which dims a Lambert crescent at the limb.
    const distance = distanceFor(6);
    const aOverD = radius / distance;
    for (const [phaseDeg, coefficient] of [
      [150, 6.1],
      [90, 0.59],
    ] as const) {
      const { body, hosts } = scene(distance, phaseDeg);
      const point = pointFlux(body, hosts, [], DISC_ANNULI_HIGH)[1];
      const exact = nearFieldFlux(body, hosts)[1];
      expect(Math.abs(1 - exact / point - coefficient * aOverD)).toBeLessThan(0.002);
    }
  });

  it("holds for a lunar law (L = 1)", () => {
    const moon = {
      ...PROVISIONAL_PHOTOMETRY,
      law: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
    };
    for (const phaseDeg of [0, 90, 150]) {
      expect(worstOver(3, phaseDeg, { photometry: moon })).toBeLessThan(0.01);
    }
  });

  it("holds for a spheroid seen from 45° latitude", () => {
    const tilted = {
      equatorialRadiusM: radius,
      polarRadiusM: radius * (1 - 0.098),
      pole: vec3(0, Math.SQRT1_2, Math.SQRT1_2),
    };
    for (const phaseDeg of [0, 90, 150]) {
      expect(worstOver(3, phaseDeg, { figure: tilted })).toBeLessThan(0.01);
    }
  });

  it("keeps the agreement off the view's centre at 40 px", () => {
    const distance = distanceFor(40);
    const { body, hosts } = scene(distance, 60, {
      centreM: vec3(0.2 * distance, 0.1 * distance, -distance),
    });
    const disc = discFlux(body, hosts);
    const point = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
    expect(Math.abs(disc[1] / point[1] - 1)).toBeLessThan(0.01);
  });
});

describe("the disc's geometry", () => {
  it("draws a Saturn-like f = 0.098 disc's equatorial and polar extents to half a pixel at 100 px", () => {
    const a = 6.0268e7;
    const c = a * (1 - 0.098);
    const aOverD = Math.sin(50 / PX_PER_RAD);
    const distance = a / aOverD;
    const { body, hosts } = scene(distance, 0, {
      figure: { equatorialRadiusM: a, polarRadiusM: c, pole: vec3(0, 1, 0) },
    });
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    const pixels = compositeDiscPixels(rasteriseDisc(record, CAMERA, VIEWPORT));
    const centreX = VIEWPORT.widthPx / 2;
    const centreY = VIEWPORT.heightPx / 2;
    // The rows and columns either side of the centre line, whose coverage sums give the extents.
    const across = pixels.filter((p) => p.yPx === centreY).reduce((sum, p) => sum + p.coverage, 0);
    const down = pixels.filter((p) => p.xPx === centreX).reduce((sum, p) => sum + p.coverage, 0);
    const s = 1 / Math.tan(CAMERA.fovXRad / 2);
    const halfWidthPx = (s * VIEWPORT.widthPx * Math.tan(Math.asin(aOverD))) / 2;
    // The scaled sphere's tangent ray, stretched back by c ÷ a.
    const halfHeightPx = (s * VIEWPORT.widthPx * (c / a) * Math.tan(Math.asin(aOverD))) / 2;
    expect(Math.abs(across - 2 * halfWidthPx)).toBeLessThan(0.5);
    expect(Math.abs(down - 2 * halfHeightPx)).toBeLessThan(0.5);
    expect(2 * halfWidthPx - 2 * halfHeightPx).toBeGreaterThan(9);
  });

  it("writes the lit class on the day side, the unlit on the night side and none on the limb", () => {
    // 20 px across, every pixel sampled 8 × 8; the terminator 1.7 px left of the centre.
    const { body, hosts } = scene(6.371e6 / Math.sin(10 / PX_PER_RAD), 80);
    const plan = planLitBodies([body], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    const pixels = rasteriseDisc(record, CAMERA, VIEWPORT);
    const row = pixels.filter((p) => p.yPx === VIEWPORT.heightPx / 2);
    const cx = VIEWPORT.widthPx / 2;
    const at = (x: number) => row.find((p) => p.xPx === x);
    expect(at(cx + 5)?.meterClass).toBe(METER_CLASS.litBody);
    expect(at(cx - 7)?.meterClass).toBe(METER_CLASS.unlitBody);
    expect(at(cx - 7)?.rgb).toEqual([0, 0, 0]);
    const limb = row.filter((p) => p.draw === "limb");
    expect(limb.length).toBeGreaterThan(0);
    expect(limb.every((p) => p.meterClass === null && p.coverage <= 1)).toBe(true);
    // The terminator's pixels mix lit and unlit points, and meter as `other`.
    expect(row.some((p) => p.meterClass === METER_CLASS.other)).toBe(true);
  });
});

describe("a body no star lights", () => {
  it("draws black with the class `other`, so that LIT and DARK do not meter it", () => {
    const { body } = scene(6.371e6 / Math.sin(20 / PX_PER_RAD), 0);
    const plan = planLitBodies([body], [], OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs[0];
    if (record === undefined) {
      throw new Error("no disc");
    }
    const pixels = rasteriseDisc(record, CAMERA, VIEWPORT);
    const interior = pixels.filter((p) => p.draw === "interior");
    expect(interior.length).toBeGreaterThan(0);
    expect(interior.every((p) => p.meterClass === METER_CLASS.other)).toBe(true);
    expect(pixels.every((p) => p.rgb.every((v) => v === 0))).toBe(true);
  });

  it("draws no point sprite light", () => {
    const { body } = scene(1e12, 0);
    expect(pointFlux(body, [], [], DISC_ANNULI_HIGH)).toEqual([0, 0, 0]);
  });
});

describe("the plan", () => {
  it("draws a point behind a disc before it, and leaves the host undrawn in its place", () => {
    const near = scene(6.371e6 / Math.sin(40 / PX_PER_RAD), 30);
    const far: LitBodyInput = {
      ...near.body,
      id: "0200080020000000.0400",
      centreM: vec3(0, 0, -1e12),
    };
    const plan = planLitBodies([near.body, far], near.hosts, OPTIONS, new Map());
    expect(plan.regimes.get(far.id)).toBe("point");
    expect(plan.regimes.get(near.body.id)).toBe("disc");
    expect(plan.order.some((entry) => entry.kind === "host")).toBe(true);
    expect(plan.steps.map((step) => step.kind)).toEqual(["points", "host", "disc"]);
  });

  it("lights a body by its two brightest stars", () => {
    const { body, hosts } = scene(6.371e6 / Math.sin(40 / PX_PER_RAD), 30);
    const faint: PlacedLight = {
      disc: aHostDisc({ star: 1, absoluteV: 9 }),
      centreM: vec3(AU_M, 0, 0),
    };
    const brighter: PlacedLight = {
      disc: aHostDisc({ star: 2, absoluteV: 6 }),
      centreM: vec3(-AU_M, 0, 0),
    };
    const plan = planLitBodies(
      [body],
      [faint, ...hosts, brighter],
      OPTIONS,
      new Map([[body.id, "disc"]]),
    );
    const lights = plan.discs[0]?.lights ?? [];
    expect(lights).toHaveLength(2);
    expect(lights[0]?.radius).toBeCloseTo(SUN_RADIUS_M / 6.371e6, 6);
    const expected = starIlluminance(brighter.disc, norm(sub(brighter.centreM, body.centreM)));
    expect(lights[1]?.illuminance).toEqual(expected);
  });

  it("lists each law once in the phase table", () => {
    const { body, hosts } = scene(6.371e6 / Math.sin(40 / PX_PER_RAD), 30);
    const twin: LitBodyInput = {
      ...body,
      id: "0200080020000000.0500",
      centreM: vec3(1e8, 0, -1e9),
    };
    const other: LitBodyInput = {
      ...body,
      id: "0200080020000000.0600",
      centreM: vec3(-1e8, 0, -1e9),
      photometry: {
        ...PROVISIONAL_PHOTOMETRY,
        law: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      },
    };
    const plan = planLitBodies(
      [body, twin, other],
      hosts,
      OPTIONS,
      new Map([
        [body.id, "disc"],
        [twin.id, "disc"],
        [other.id, "disc"],
      ]),
    );
    expect(plan.laws).toEqual([body.photometry.law, other.photometry.law]);
    expect(plan.discs.map((record) => record.tableRows[0] ?? -1).toSorted((x, y) => x - y)).toEqual(
      [0, 0, 1],
    );
  });

  it("takes a moon in front of the star as the planet's occluder", () => {
    const { body, hosts } = scene(6.371e6 / Math.sin(40 / PX_PER_RAD), 0);
    const star = hosts[0]?.centreM ?? vec3(0, 0, 0);
    const towards = scale(add(star, scale(body.centreM, -1)), 1 / AU_M);
    const moon: LitBodyInput = {
      ...body,
      id: "0200080020000000.0301",
      centreM: add(body.centreM, scale(towards, 3.844e8)),
      figure: { equatorialRadiusM: 1.737e6, polarRadiusM: 1.737e6, pole: null },
    };
    const plan = planLitBodies([body, moon], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const planet = plan.discs.find((record) => record.radiusOverDistance > 1e-3);
    expect(planet?.occluders).toHaveLength(1);
    expect(planet?.occluders[0]?.radius).toBeCloseTo(1.737e6 / 6.371e6, 9);
  });

  it("does not draw a body seen from inside it", () => {
    const { body, hosts } = scene(1e6, 0);
    const plan = planLitBodies([body], hosts, OPTIONS, new Map());
    expect(plan.discs).toHaveLength(0);
  });

  it("draws no disc wholly off the view, beside the camera or behind it, and keeps its regime", () => {
    // PHASE TEST's full planet stands 90° off the axis, across the camera's plane, where its
    // rectangle was the whole view: two draws over every pixel for none of its own.
    const { body, hosts } = scene(2e8, 0);
    const placed = (id: string, centreM: Vec3): LitBodyInput => ({
      ...body,
      id,
      centreM,
    });
    const beside = placed("0200080020000000.0401", vec3(2e8, 0, 0));
    const behind = placed("0200080020000000.0402", vec3(0, 3e7, 2e8));
    const plan = planLitBodies(
      [body, beside, behind],
      hosts,
      OPTIONS,
      new Map([body, beside, behind].map((each) => [each.id, "disc" as const])),
    );
    expect(plan.discs.map((record) => record.body)).toEqual([body.id]);
    expect(plan.steps.filter((step) => step.kind === "disc")).toHaveLength(1);
    expect(plan.regimes.get(beside.id)).toBe("disc");
    expect(plan.regimes.get(behind.id)).toBe("disc");
  });
});

/** Values as the shader reads them, rounded to `f32`. */
function f32(values: ReadonlyArray<number>): number[] {
  return values.map((v) => Math.fround(v));
}

describe("the records", () => {
  it("packs DISC_ROWS rows per disc with the law, the lights, the annuli and the occluders where the shader reads them", () => {
    const { body, hosts } = scene(6.371e6 / Math.sin(40 / PX_PER_RAD), 0);
    const second: PlacedLight = {
      disc: aHostDisc({ star: 1, absoluteV: 6 }),
      centreM: vec3(-AU_M, 0, 0),
    };
    const star = hosts[0]?.centreM ?? vec3(0, 0, 0);
    const towards = normalise(sub(star, body.centreM));
    const moon: LitBodyInput = {
      ...body,
      id: "0200080020000000.0301",
      centreM: add(body.centreM, scale(towards, 3.844e8)),
      figure: { equatorialRadiusM: 1.737e6, polarRadiusM: 1.737e6, pole: null },
    };
    const plan = planLitBodies(
      [body, moon],
      [...hosts, second],
      OPTIONS,
      new Map([[body.id, "disc"]]),
    );
    const record = plan.discs.find((r) => r.radiusOverDistance > 1e-3);
    if (record === undefined) {
      throw new Error("no planet disc");
    }
    const packed = packDiscRecords([record]);
    expect(packed).toHaveLength(DISC_ROWS * 4);
    const row = (i: number): number[] => Array.from(packed.subarray(4 * i, 4 * i + 4));
    const law = body.photometry.law;
    // Row 3: samples, lights, occluders; row 4: A and L; row 5: table row, K, exposure ÷ π.
    expect(row(3)).toEqual([1, 4, 2, 1]);
    expect(row(4)).toEqual(f32([...law.a, law.lommelSeeligerShare]));
    expect(row(5)).toEqual(f32([0, 4, EXPOSURE / Math.PI, 0]));
    // Light 0 at row 6: its direction and distance ÷ a; its illuminance and radius ÷ a.
    const light = record.lights[0];
    expect(row(6)).toEqual(
      f32([
        light?.direction.x ?? 0,
        light?.direction.y ?? 0,
        light?.direction.z ?? 0,
        light?.distance ?? 0,
      ]),
    );
    expect(row(7)).toEqual(f32([...(light?.illuminance ?? [0, 0, 0]), light?.radius ?? 0]));
    // Light 1 at row 14; its red annuli's outer edges end at the limb.
    expect(row(14)[3]).toBe(Math.fround(record.lights[1]?.distance ?? 0));
    expect(row(16)[3]).toBe(1);
    // Occluder 0 at row 22: its centre from the body's ÷ a, and its radius ÷ a.
    const occluder = record.occluders[0];
    expect(row(22)).toEqual(
      f32([
        occluder?.centre.x ?? 0,
        occluder?.centre.y ?? 0,
        occluder?.centre.z ?? 0,
        1.737e6 / 6.371e6,
      ]),
    );
  });

  it("orders a host's annuli r, g, b from R06's B, V, R laws", () => {
    const disc = aHostDisc({
      limb: [
        { c: 0.846, alpha: 0.83 },
        // Maxted 2018's Sun in V, whose α only happens to resemble 1 ÷ √2.
        // oxlint-disable-next-line approx-constant
        { c: 0.771, alpha: 0.707 },
        { c: 0.712, alpha: 0.625 },
      ],
    });
    const [r, , b] = hostAnnuli(disc, 4);
    // The bluer law darkens more, so its innermost annulus is the brighter share of its flux.
    expect(b.flux[0] ?? 0).toBeGreaterThan(r.flux[0] ?? 0);
  });
});

describe("an eclipse", () => {
  // A Moon-sized body 3 px across, full, in the penumbra of an Earth-sized body 3.844 × 10⁸ m
  // towards the Sun and 6.5 × 10⁶ m to the side (umbra 4.6 × 10⁶ m, penumbra 8.2 × 10⁶ m there).
  const moonRadius = 1.737e6;
  const { body, hosts } = scene(moonRadius / Math.sin(1.5 / PX_PER_RAD), 0, {
    figure: { equatorialRadiusM: moonRadius, polarRadiusM: moonRadius, pole: null },
  });
  const star = hosts[0]?.centreM ?? vec3(0, 0, 0);
  const towards = normalise(sub(star, body.centreM));
  const earth: LitBodyInput = {
    ...body,
    id: "0200080020000000.0302",
    centreM: add(add(body.centreM, scale(towards, 3.844e8)), vec3(6.5e6, 0, 0)),
    figure: { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null },
  };
  const occluder = { id: earth.id, centreM: earth.centreM, radiusM: 6.371e6 };

  it("darkens the disc in the penumbra", () => {
    const shadowed = discFlux(body, hosts, [earth])[1];
    const free = discFlux(body, hosts)[1];
    expect(shadowed / free).toBeLessThan(0.9);
    expect(shadowed / free).toBeGreaterThan(0.05);
  });

  it("darkens the disc and the point alike, to 1%", () => {
    // The point takes the eclipse over its disc (T10.b); from its centre it was 2% off.
    const disc = discFlux(body, hosts, [earth])[1] / discFlux(body, hosts)[1];
    const point =
      pointFlux(body, hosts, [occluder], DISC_ANNULI_HIGH)[1] /
      pointFlux(body, hosts, [], DISC_ANNULI_HIGH)[1];
    expect(Math.abs(disc / point - 1)).toBeLessThan(0.01);
  });

  it("leaves no light inside the umbra, and ignores an occluder beyond the star", () => {
    // The starlight only: the occluder's planetshine is its own term. Exactly at new phase it gives
    // about 10⁻²³ lx through the rounding of sin π (T11's own test bounds a neighbour at new
    // phase), and full beyond the star some 10⁻¹⁰ of the sunlight.
    const starlight = { ...OPTIONS, planetshine: 0 };
    const umbral: LitBodyInput = {
      ...earth,
      centreM: add(body.centreM, scale(towards, 3.844e8)),
    };
    expect(discFlux(body, hosts, [umbral], starlight)[1]).toBe(0);
    const beyond: LitBodyInput = {
      ...earth,
      centreM: add(body.centreM, scale(towards, 2 * AU_M)),
    };
    expect(discFlux(body, hosts, [beyond], starlight)[1]).toBeCloseTo(
      discFlux(body, hosts, [], starlight)[1],
      12,
    );
  });
});

describe("a body's eclipse over its disc (T10.b)", () => {
  /**
   * Jupiter's semi-major axis, Io's about it and Io's radius, m (NASA GSFC, the Jupiter and Jovian
   * Satellite Fact Sheets).
   */
  const jupiterOrbitM = 7.78479e11;
  const ioOrbitM = 4.218e8;
  const ioRadiusM = 1.8215e6;
  const jupiterFigure = {
    equatorialRadiusM: 7.1492e7,
    polarRadiusM: 6.6854e7,
    pole: vec3(0, 1, 0),
  };

  it("keeps a point Jupiter 99.9% of its light in Io's central shadow transit, where its centre gave 0", () => {
    // Jupiter from Earth at opposition, 5° of phase, Io on the Sun's line to Jupiter's centre.
    const sunward = vec3(Math.sin(5 * RAD), 0, Math.cos(5 * RAD));
    const jupiter: LitBodyInput = {
      id: "0200080020000000.0005",
      centreM: vec3(0, 0, -(jupiterOrbitM - AU_M)),
      figure: jupiterFigure,
      photometry: planetPhotometry("Jupiter"),
      lighting: undefined,
    };
    const hosts: PlacedLight[] = [
      { disc: aHostDisc(), centreM: add(jupiter.centreM, scale(sunward, jupiterOrbitM)) },
    ];
    const io = {
      id: "0200080020000000.0501",
      centreM: add(jupiter.centreM, scale(sunward, ioOrbitM)),
      radiusM: ioRadiusM,
    };
    const clear = pointFlux(jupiter, hosts, [], DISC_ANNULI_HIGH);
    const eclipsed = pointFlux(jupiter, hosts, [io], DISC_ANNULI_HIGH);
    // The share Io's shadow takes, 1.5 (R_Io ÷ √(a c))² (1 + x ÷ d)², is 0.104%: the ruling's
    // "at most 0.10%" (decision-r07-earth-albedo, Q2), so 99.896% is kept.
    const share =
      1.5 *
      (ioRadiusM / Math.sqrt(7.1492e7 * 6.6854e7)) ** 2 *
      (1 + ioOrbitM / (jupiterOrbitM - ioOrbitM)) ** 2;
    for (const c of [0, 1, 2] as const) {
      expect(eclipsed[c] / clear[c]).toBeGreaterThan(0.9989);
      expect(eclipsed[c] / clear[c]).toBeLessThan(1);
      expect(Math.abs(1 - eclipsed[c] / clear[c] - share)).toBeLessThan(0.01 * share);
    }
  });

  /**
   * An Io-like moon `px` across, lit at phase `phaseDeg` by a Sun 5.2 au off, and a Jupiter-sized
   * occluder 4.218 × 10⁸ m sunward whose shadow axis passes `axisM` from its centre.
   */
  function ingress(
    px: number,
    phaseDeg: number,
    axisM: number,
    photometry: LitBodyInput["photometry"],
  ): {
    readonly body: LitBodyInput;
    readonly hosts: PlacedLight[];
    readonly jupiter: LitBodyInput;
  } {
    const distanceM = ioRadiusM / Math.sin(px / 2 / PX_PER_RAD);
    const centreM = vec3(0, 0, -distanceM);
    const sunward = vec3(Math.sin(phaseDeg * RAD), 0, Math.cos(phaseDeg * RAD));
    const across = vec3(0, 1, 0);
    const starM = add(centreM, scale(sunward, jupiterOrbitM + ioOrbitM));
    const body: LitBodyInput = {
      id: "0200080020000000.0501",
      centreM,
      figure: { equatorialRadiusM: ioRadiusM, polarRadiusM: ioRadiusM, pole: null },
      photometry,
      lighting: undefined,
    };
    const jupiter: LitBodyInput = {
      id: "0200080020000000.0005",
      centreM: add(
        add(centreM, scale(sunward, ioOrbitM)),
        scale(across, (axisM * jupiterOrbitM) / (jupiterOrbitM + ioOrbitM)),
      ),
      figure: jupiterFigure,
      photometry: planetPhotometry("Jupiter"),
      lighting: undefined,
    };
    return { body, hosts: [{ disc: aHostDisc(), centreM: starM }], jupiter };
  }

  const lunar = photometryFor([0.6, 0.6, 0.6], [0.626, 0.626, 0.626], "moon");
  const starlight = { ...OPTIONS, planetshine: 0 };
  for (const [lawName, photometry] of [
    ["the provisional Lambert law", PROVISIONAL_PHOTOMETRY],
    ["a lunar law", lunar],
  ] as const) {
    it(`draws one flux on the disc and the point at the 3 px switch through an Io-like ingress, to 1%, under ${lawName}`, () => {
      // Jupiter's umbra at Io is 71,154 km in radius and its penumbra 71,908 km: the axis from
      // the last limb's entering the umbra to first contact, at a quarter, half and three quarters.
      let worst = 0;
      for (const phaseDeg of [30, 90]) {
        for (const fraction of [0.25, 0.5, 0.75]) {
          const axisM = 6.9332e7 + fraction * 4.397e6;
          const { body, hosts, jupiter } = ingress(3, phaseDeg, axisM, photometry);
          const occluder = { id: jupiter.id, centreM: jupiter.centreM, radiusM: 7.1492e7 };
          const disc = discFlux(body, hosts, [jupiter], starlight);
          const point = pointFlux(body, hosts, [occluder], DISC_ANNULI_HIGH);
          const free = pointFlux(body, hosts, [], DISC_ANNULI_HIGH);
          // A real ingress: neither clear nor dark.
          expect(point[1] / free[1]).toBeGreaterThan(0.05);
          expect(point[1] / free[1]).toBeLessThan(0.95);
          for (const c of [0, 1, 2] as const) {
            worst = Math.max(worst, Math.abs(disc[c] / point[c] - 1));
          }
        }
      }
      expect(worst).toBeLessThan(0.01);
    });
  }
});

describe("a body's lighting frame (T10.a)", () => {
  // The eclipse's Moon, 3 px across, and an Earth drawn far clear of its light.
  const moonRadiusM = 1.737e6;
  const { body, hosts } = scene(moonRadiusM / Math.sin(1.5 / PX_PER_RAD), 0, {
    figure: { equatorialRadiusM: moonRadiusM, polarRadiusM: moonRadiusM, pole: null },
  });
  const star = hosts[0]?.centreM ?? vec3(0, 0, 0);
  const towards = normalise(sub(star, body.centreM));
  const earth: LitBodyInput = {
    ...body,
    id: "0200080020000000.0302",
    centreM: add(add(body.centreM, scale(towards, 3.844e8)), vec3(5e7, 0, 0)),
    figure: { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null },
  };
  const starlight = { ...OPTIONS, planetshine: 0 };

  /** The body lit at quarter phase by its frame, though the frame's hosts light it full. */
  const quarter = scene(norm(body.centreM), 90);
  const quarterLit: LitBodyInput = {
    ...body,
    lighting: { lights: quarter.hosts, occluders: [] },
  };

  it("lights a point by its frame's stars, not the frame's drawn hosts", () => {
    expect(pointFlux(quarterLit, hosts, [], DISC_ANNULI_HIGH)).toEqual(
      pointFlux(body, quarter.hosts, [], DISC_ANNULI_HIGH),
    );
  });

  it("lights a disc by its frame's stars, not the frame's drawn hosts", () => {
    expect(discFlux(quarterLit, hosts, [], starlight)).toEqual(
      discFlux(body, quarter.hosts, [], starlight),
    );
  });

  /** The light finds the Earth on the Moon's line to the Sun, in front of where it is drawn. */
  const umbral = {
    id: earth.id,
    centreM: add(body.centreM, scale(towards, 3.844e8)),
    radiusM: 6.371e6,
  };
  const shadowed: LitBodyInput = { ...body, lighting: { lights: hosts, occluders: [umbral] } };
  const previous = new Map([[body.id, "disc" as const]]);

  it("eclipses a body by its frame's occluders, not where they are drawn", () => {
    expect(discFlux(body, hosts, [earth], starlight)[1]).toBeGreaterThan(0);
    expect(discFlux(shadowed, hosts, [earth], starlight)[1]).toBe(0);
  });

  it("hands the disc record its frame's occluders, from the body's centre in its radii", () => {
    const plan = planLitBodies([shadowed, earth], hosts, starlight, previous);
    const record = plan.discs.find((each) => each.body === body.id);
    expect(record?.occluders).toEqual([
      {
        centre: scale(sub(umbral.centreM, body.centreM), 1 / moonRadiusM),
        radius: 6.371e6 / moonRadiusM,
      },
    ]);
  });

  it("keeps the painter's order the drawn centres give", () => {
    expect(planLitBodies([shadowed, earth], hosts, starlight, previous).order).toEqual(
      planLitBodies([body, earth], hosts, starlight, previous).order,
    );
  });

  it("keeps the regimes the drawn centres give", () => {
    expect(planLitBodies([shadowed, earth], hosts, starlight, previous).regimes).toEqual(
      planLitBodies([body, earth], hosts, starlight, previous).regimes,
    );
  });
});

describe("planetshine", () => {
  const moonRadius = 1.7374e6;
  const moonFigure = { equatorialRadiusM: moonRadius, polarRadiusM: moonRadius, pole: null };
  /**
   * A body `px` across at solar phase `phaseDeg`, and a neighbour `distanceM` from it on its
   * anti-solar side, so that the neighbour is full from it and lights its night side.
   */
  function shineScene(
    px: number,
    phaseDeg: number,
    neighbourRadiusM: number,
    distanceM: number,
    body: Partial<LitBodyInput> = {},
  ): {
    readonly body: LitBodyInput;
    readonly neighbour: LitBodyInput;
    readonly hosts: PlacedLight[];
  } {
    const radius = body.figure?.equatorialRadiusM ?? moonRadius;
    const placed = scene(radius / Math.sin(px / 2 / PX_PER_RAD), phaseDeg, {
      figure: moonFigure,
      ...body,
    });
    const star = placed.hosts[0]?.centreM ?? vec3(0, 0, 0);
    const towards = normalise(sub(star, placed.body.centreM));
    const neighbour: LitBodyInput = {
      id: "0200080020000000.0003",
      centreM: add(placed.body.centreM, scale(towards, -distanceM)),
      figure: {
        equatorialRadiusM: neighbourRadiusM,
        polarRadiusM: neighbourRadiusM,
        pole: null,
      },
      photometry: planetPhotometry("Earth"),
      lighting: undefined,
    };
    return { ...placed, neighbour };
  }

  const lunar = photometryFor([0.12, 0.12, 0.12], [0.626, 0.626, 0.626], "moon");
  const pairs = [
    ["Earth from the Moon", 6.371e6, 3.844e8],
    ["Jupiter from Io", 6.9911e7, 4.218e8],
  ] as const;
  for (const [pair, neighbourRadiusM, distanceM] of pairs) {
    for (const [lawName, photometry] of [
      ["the provisional Lambert law", PROVISIONAL_PHOTOMETRY],
      ["a lunar law", lunar],
    ] as const) {
      it(`draws one flux on the disc and the point at the 3 px switch, to 1%, for ${pair} under ${lawName}`, () => {
        let worst = 0;
        for (const phaseDeg of [90, 150]) {
          const { body, neighbour, hosts } = shineScene(3, phaseDeg, neighbourRadiusM, distanceM, {
            photometry,
          });
          const secondaries = planetshineSources(
            body,
            litNeighbours([body, neighbour], hosts, DISC_ANNULI_HIGH),
            2,
          );
          expect(secondaries).toHaveLength(1);
          const disc = discFlux(body, hosts, [neighbour]);
          const point = pointFlux(body, hosts, [], DISC_ANNULI_HIGH, secondaries);
          for (const c of [0, 1, 2] as const) {
            worst = Math.max(worst, Math.abs(disc[c] / point[c] - 1));
          }
        }
        expect(worst).toBeLessThan(0.01);
      });
    }
  }

  it("lights the night side, which the meter keeps unlit", () => {
    const { body, neighbour, hosts } = shineScene(40, 150, 6.371e6, 3.844e8);
    const nightOf = (others: ReadonlyArray<LitBodyInput>) => {
      const plan = planLitBodies([body, ...others], hosts, OPTIONS, new Map([[body.id, "disc"]]));
      const record = plan.discs.find((each) => each.body === body.id);
      if (record === undefined) {
        throw new Error("no disc");
      }
      return rasteriseDisc(record, CAMERA, VIEWPORT).filter(
        (p) => p.draw === "interior" && p.meterClass === METER_CLASS.unlitBody,
      );
    };
    const dark = nightOf([]);
    const shone = nightOf([neighbour]);
    expect(dark.length).toBeGreaterThan(100);
    expect(dark.every((p) => p.rgb.every((v) => v === 0))).toBe(true);
    // The same pixels stay unlit for the meter, and earthshine lights every one of them.
    expect(shone.map((p) => [p.xPx, p.yPx])).toEqual(dark.map((p) => [p.xPx, p.yPx]));
    expect(shone.every((p) => p.rgb.every((v) => v > 0))).toBe(true);
  });

  it("matches the law at a night-side point lit by the neighbour alone", () => {
    // 40 px across at 150° under a lunar law: the pixel at the disc's centre sees only earthshine,
    // near full Earth.
    const { body, neighbour, hosts } = shineScene(40, 150, 6.371e6, 3.844e8, {
      photometry: lunar,
    });
    const plan = planLitBodies([body, neighbour], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs.find((each) => each.body === body.id);
    const source = planetshineSources(
      body,
      litNeighbours([body, neighbour], hosts, DISC_ANNULI_HIGH),
      2,
    )[0];
    if (record === undefined || source === undefined) {
      throw new Error("no disc or no source");
    }
    const x = VIEWPORT.widthPx / 2;
    const y = VIEWPORT.heightPx / 2;
    const pixel = rasteriseDisc(record, CAMERA, VIEWPORT).find(
      (p) => p.xPx === x && p.yPx === y && p.draw === "interior",
    );
    // The point the centre ray meets, its normal, and the law at it in f64.
    const ray = viewRay(x + 0.5, y + 0.5, CAMERA, VIEWPORT);
    const centre = body.centreM;
    const along = dot(ray, centre);
    const hit = scale(
      ray,
      along - Math.sqrt(along * along - dot(centre, centre) + moonRadius ** 2),
    );
    const normal = normalise(sub(hit, centre));
    const toSource = sub(add(centre, scale(source.direction, source.distanceM)), hit);
    const towards = normalise(toSource);
    const mu0 = dot(normal, towards);
    const mu = -dot(normal, ray);
    const alpha = Math.acos(-dot(towards, ray));
    const near = source.distanceM / norm(toSource);
    // Earth wholly above the point's horizon, so its irradiance factor is μ₀.
    expect(mu0).toBeGreaterThan(Math.sin(source.angularRadiusRad) * 2);
    const reflectance = brdf(body.photometry.law, mu0, mu, alpha);
    for (const c of [0, 1, 2] as const) {
      const expected = source.illuminance[c] * near * near * (EXPOSURE / Math.PI) * reflectance[c];
      expect(Math.abs((pixel?.rgb[c] ?? 0) / expected - 1)).toBeLessThan(1e-6);
    }
  });

  it("takes two neighbours on the high setting and one on the low", () => {
    const { body, neighbour, hosts } = shineScene(40, 150, 6.371e6, 3.844e8);
    const second: LitBodyInput = {
      ...neighbour,
      id: "0200080020000000.0004",
      centreM: add(neighbour.centreM, vec3(0, 2e8, 0)),
    };
    const third: LitBodyInput = {
      ...neighbour,
      id: "0200080020000000.0005",
      centreM: add(neighbour.centreM, vec3(0, -4e8, 0)),
    };
    const count = (planetshine: number): number => {
      const plan = planLitBodies(
        [body, neighbour, second, third],
        hosts,
        { ...OPTIONS, planetshine },
        new Map([[body.id, "disc"]]),
      );
      return plan.discs.find((each) => each.body === body.id)?.secondaries.length ?? -1;
    };
    expect(count(PLANETSHINE_SOURCES_HIGH)).toBe(2);
    expect(count(PLANETSHINE_SOURCES_LOW)).toBe(1);
  });

  it("packs its sources after the classes, where the shader reads them", () => {
    const { body, neighbour, hosts } = shineScene(40, 150, 6.371e6, 3.844e8);
    const plan = planLitBodies([body, neighbour], hosts, OPTIONS, new Map([[body.id, "disc"]]));
    const record = plan.discs.find((each) => each.body === body.id);
    const source = record?.secondaries[0];
    if (record === undefined || source === undefined) {
      throw new Error("no disc or no source");
    }
    expect(source.distance).toBeCloseTo(3.844e8 / moonRadius, 6);
    expect(source.radius).toBeCloseTo(6.371e6 / moonRadius, 9);
    const packed = packDiscRecords([record]);
    expect(packed).toHaveLength(DISC_ROWS * 4);
    const row = (i: number): number[] => Array.from(packed.subarray(4 * i, 4 * i + 4));
    expect(row(46)).toEqual([1, 0, 0, 0]);
    expect(row(47)).toEqual(
      f32([source.direction.x, source.direction.y, source.direction.z, source.distance]),
    );
    expect(row(48)).toEqual(f32([...source.illuminance, source.radius]));
    expect(row(49)).toEqual([0, 0, 0, 0]);
    expect(row(50)).toEqual([0, 0, 0, 0]);
  });
});

describe("the renderer", () => {
  /** A plan of two discs and a point between them, and another point after. */
  function planOf(count = 2): ReturnType<typeof planLitBodies> {
    const { body, hosts } = scene(6.371e6 / Math.sin(20 / PX_PER_RAD), 30);
    const bodies: LitBodyInput[] = [];
    for (let i = 0; i < count; i += 1) {
      bodies.push({
        ...body,
        id: `0200080020000000.${(0x100 + i).toString(16).padStart(4, "0")}`,
        centreM: vec3((i - count / 2) * 2e7, 0, body.centreM.z * (1 + i * 0.5)),
      });
    }
    // Two small bodies as points: one beyond every disc, one nearer than all of them.
    const point = (z: number, id: string): LitBodyInput => ({
      ...body,
      id,
      centreM: vec3(0, 0, z),
      figure: { equatorialRadiusM: 1e3, polarRadiusM: 1e3, pole: null },
    });
    return planLitBodies(
      [...bodies, point(-1e13, "0200080020000000.0900"), point(-5e8, "0200080020000000.0901")],
      hosts,
      OPTIONS,
      new Map(bodies.map((b) => [b.id, "disc" as const])),
    );
  }

  it("draws each disc as its interior then its limb, and each run of points once", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = planOf();
    const hostDraw = renderer.draws(planOf())[0];
    if (hostDraw === undefined) {
      throw new Error("no draw");
    }
    const host: DrawItem = { ...hostDraw, material: { kind: "material", name: "sky:hostDisc" } };
    const draws = renderer.draws(plan, new Map([[0, [host]]]));
    const kinds = draws.map((draw) =>
      draw.material.name.startsWith("bodies:disc")
        ? `${draw.material.name} ${String(draw.uniforms["edgePass"]?.[0])}`
        : draw.material.name === "sky:hostDisc"
          ? "host"
          : `points ${String(draw.instanceCount)}`,
    );
    const expected = plan.steps.flatMap((step) => {
      let names: string[];
      switch (step.kind) {
        case "disc":
          names = ["bodies:disc 0", "bodies:discLimb 1"];
          break;
        case "host":
          names = ["host"];
          break;
        case "points":
          names = [`points ${String(step.sprites.length)}`];
          break;
        case "limb":
          names = ["bodies:discLimb 1"];
          break;
      }
      return names;
    });
    expect(kinds).toEqual(expected);
    expect(plan.steps.map((step) => step.kind)).toEqual([
      "points",
      "host",
      "disc",
      "disc",
      "points",
    ]);
    renderer.dispose();
  });

  it("writes the phase table once for the same laws, and grows the disc buffer by doubling", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    renderer.draws(planOf());
    renderer.draws(planOf());
    expect(engine.textureWritten.filter((w) => w.texture === "bodies:phase table")).toHaveLength(1);
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    // Sixteen discs need 16 × 736 bytes, past the first 4,096.
    const many = planOf(16);
    expect(many.discs).toHaveLength(16);
    renderer.draws(many);
    expect(released).toContain("bodies:discs");
    renderer.dispose();
  });

  it("makes its resources again after a device loss and rewrites the table", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    // One plan before and after the loss: its records must be written again to the new buffers.
    const plan = planOf();
    renderer.draws(plan);
    const materials = engine.counts.materials;
    engine.restore();
    // The disc's two, the sprite's and the mesh regime's.
    expect(engine.counts.materials).toBe(materials + 4);
    const writes = engine.writes.length;
    renderer.draws(plan);
    expect(engine.textureWritten.filter((w) => w.texture === "bodies:phase table")).toHaveLength(2);
    expect(engine.writes.slice(writes).map((w) => w.buffer)).toContain("bodies:discs");
    renderer.dispose();
  });
});

/** A disc draw's corner depths. */
function depthsOf(draw: DrawItem): number[] {
  return Array.from(draw.uniforms["depths"] ?? []);
}

describe("mesh bodies (T9)", () => {
  /** A planet 40 px across lit at 30°, a moon beyond it, and a third body clear of both. */
  function sceneOfThree(): {
    readonly bodies: LitBodyInput[];
    readonly hosts: PlacedLight[];
  } {
    const { body, hosts } = scene(6.371e6 / Math.sin(20 / PX_PER_RAD), 30);
    const moon: LitBodyInput = {
      ...body,
      id: "0200080020000000.0302",
      centreM: add(body.centreM, vec3(5e6, 0, -3e7)),
      figure: { equatorialRadiusM: 1.7e6, polarRadiusM: 1.7e6, pole: null },
    };
    const clear: LitBodyInput = {
      ...body,
      id: "0200080020000000.0303",
      centreM: add(body.centreM, vec3(-8e7, 0, 0)),
    };
    return { bodies: [body, moon, clear], hosts };
  }

  /** The three bodies' plan with a depth writer over the planet, standing for R10's terrain. */
  function promotedPlan(): ReturnType<typeof planLitBodies> {
    const { bodies, hosts } = sceneOfThree();
    const [planet] = bodies;
    const writer =
      planet === undefined ? null : sphereFootprint(planet.centreM, 6.371e6, CAMERA, VIEWPORT);
    return planLitBodies(
      bodies,
      hosts,
      { ...OPTIONS, depthWriters: writer === null ? [] : [writer] },
      new Map(bodies.map((b) => [b.id, "disc" as const])),
    );
  }

  const [PLANET, MOON, CLEAR] = sceneOfThree().bodies.map((b) => b.id);

  it("promotes a disc over a depth writer, and a disc overlapping it, but not one clear of both", () => {
    const plan = promotedPlan();
    expect(
      [PLANET, MOON, CLEAR].map((id) => (id === undefined ? id : plan.regimes.get(id))),
    ).toEqual(["mesh", "mesh", "disc"]);
  });

  it("gives each mesh body its record and its smooth figure", () => {
    const plan = promotedPlan();
    expect(new Set(plan.meshes.map((m) => plan.discs[m.index]?.body))).toEqual(
      new Set([PLANET, MOON]),
    );
    expect(plan.meshes.every((m) => m.mesh.patches.length > 0)).toBe(true);
  });

  it("places each mesh body's limb in the painter's sequence, the farther first", () => {
    const plan = promotedPlan();
    const limbs = plan.steps.flatMap((step) =>
      step.kind === "limb" ? [plan.discs[plan.meshes[step.mesh]?.index ?? -1]?.body] : [],
    );
    expect(limbs).toEqual([MOON, PLANET]);
    expect(plan.steps.filter((step) => step.kind === "disc")).toHaveLength(1);
  });

  it("draws no figure and no limb for a mesh body wholly off the view, and keeps its regime", () => {
    const { bodies, hosts } = sceneOfThree();
    const planet = bodies[0];
    if (planet === undefined) {
      throw new Error("the scene has no planet");
    }
    // Beside the camera, across its plane: its footprint is the whole view's, so the depth writer
    // over the planet promotes it too (and, through it, the body clear of both).
    const beside: LitBodyInput = {
      ...planet,
      id: "0200080020000000.0304",
      centreM: vec3(2e8, 0, 0),
    };
    const writer = sphereFootprint(planet.centreM, 6.371e6, CAMERA, VIEWPORT);
    const all = [...bodies, beside];
    const plan = planLitBodies(
      all,
      hosts,
      { ...OPTIONS, depthWriters: writer === null ? [] : [writer] },
      new Map(all.map((b) => [b.id, "disc" as const])),
    );
    expect(plan.regimes.get(beside.id)).toBe("mesh");
    expect(plan.meshes.map((m) => plan.discs[m.index]?.body)).not.toContain(beside.id);
  });

  it("promotes nothing without a depth writer", () => {
    const { bodies, hosts } = sceneOfThree();
    const plan = planLitBodies(bodies, hosts, OPTIONS, new Map());
    expect([plan.meshes, [...plan.regimes.values()].filter((r) => r === "mesh")]).toEqual([[], []]);
  });

  it("draws each mesh body's figure in one instanced draw of its patches", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = promotedPlan();
    const draws = renderer.meshDraws(plan);
    let first = 0;
    const expected = plan.meshes.map((body) => {
      const shape = {
        material: "bodies:smoothMesh",
        instances: body.mesh.patches.length,
        disc: body.index,
        firstInstance: first,
        rotation: Array.from(rotationColumns(body.mesh.axes)),
        buffers: ["discs", "slots", "instances"],
      };
      first += body.mesh.patches.length;
      return shape;
    });
    expect(
      draws.map((draw) => ({
        material: draw.material.name,
        instances: draw.instanceCount,
        disc: draw.uniforms["disc"]?.[0],
        firstInstance: draw.uniforms["firstInstance"]?.[0],
        rotation: Array.from(draw.uniforms["bodyRotation"] ?? []),
        buffers: Object.keys(draw.storageBuffers ?? {}),
      })),
    ).toEqual(expected);
    expect(engine.writes.map((w) => w.buffer)).toEqual(
      expect.arrayContaining(["bodies:mesh slots", "bodies:mesh instances"]),
    );
    renderer.dispose();
  });

  it("draws each mesh body's limb in the sequence at its limb plane's depths, and a disc's at 0", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = promotedPlan();
    const sequence = renderer.draws(plan);
    const limbs = sequence.filter(
      (draw) => draw.material.name === "bodies:discLimb" && depthsOf(draw).some((d) => d !== 0),
    );
    expect(limbs.map(depthsOf)).toEqual(
      plan.steps.flatMap((step) =>
        step.kind === "limb"
          ? [Array.from(new Float32Array(plan.meshes[step.mesh]?.limbDepths ?? []))]
          : [],
      ),
    );
    expect(sequence.filter((draw) => draw.material.name === "bodies:disc").map(depthsOf)).toEqual([
      Array.from(DISC_LIMB_DEPTHS),
    ]);
    renderer.dispose();
  });

  it("writes a plan's records once for its figures and its sequence", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    const plan = promotedPlan();
    renderer.meshDraws(plan);
    const writes = engine.writes.length;
    renderer.draws(plan);
    expect(engine.writes.length).toBe(writes);
    renderer.dispose();
  });
});
