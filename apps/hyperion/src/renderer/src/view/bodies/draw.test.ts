import { describe, expect, it } from "vitest";

import { add, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import { aHostDisc, aLitBody, SUN_RADIUS_M } from "../../test/litFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { phaseFactorTableOf } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { pixelSolidAngle, type ProjectionCamera, type Viewport } from "../camera/projection";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { starIlluminance } from "../lighting/illuminance";
import { AU_M } from "../scenes/kept";
import { METER_CLASS } from "../post/meter";
import {
  compositeDiscPixels,
  DISC_ROWS,
  packDiscRecords,
  rasteriseDisc,
  viewRay,
} from "./discShading";
import type { PlacedLight } from "../lighting/hostLights";
import { hostAnnuli, LitBodyRenderer, type LitBodyInput, planLitBodies, pointFlux } from "./draw";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";

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
      ...body,
    },
    hosts: [{ disc: aHostDisc(), centreM: add(centreM, scale(towardsStar, AU_M)) }],
  };
}

const OPTIONS = { camera: CAMERA, viewport: VIEWPORT, exposureScale: EXPOSURE, annuli: 4 };

/**
 * The disc's flux at the camera per channel, lx: Σ L Ω over its pixels, as drawn, with `others`
 * in the scene (as occluders).
 */
function discFlux(
  body: LitBodyInput,
  hosts: ReadonlyArray<PlacedLight>,
  others: ReadonlyArray<LitBodyInput> = [],
): [number, number, number] {
  const plan = planLitBodies([body, ...others], hosts, OPTIONS, new Map([[body.id, "disc"]]));
  const record = plan.discs.find((each) => each.body === body.id);
  if (record === undefined) {
    throw new Error("the body was not drawn as a disc");
  }
  const flux: [number, number, number] = [0, 0, 0];
  const drawn = rasteriseDisc(record, phaseFactorTableOf(record.law), CAMERA, VIEWPORT);
  for (const pixel of compositeDiscPixels(drawn)) {
    const ray = viewRay(pixel.xPx + 0.5, pixel.yPx + 0.5, CAMERA, VIEWPORT);
    const omega = pixelSolidAngle(ray, CAMERA, VIEWPORT);
    for (const c of [0, 1, 2] as const) {
      flux[c] += (pixel.rgb[c] * omega) / EXPOSURE;
    }
  }
  return flux;
}

describe("the disc and the point at the 3 px switch", () => {
  const radius = 6.371e6;
  /** The distance at which the body is `px` across at the centre pixel's scale. */
  const distanceFor = (px: number): number => radius / Math.sin(px / 2 / PX_PER_RAD);
  /** The worst |disc ÷ point − 1| per channel over 16 sub-pixel placements of the centre. */
  const worstOver = (px: number, phaseDeg: number, body: Partial<LitBodyInput>): number => {
    const distance = distanceFor(px);
    let worst = 0;
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
        const point = pointFlux(moved, hosts, [], DISC_ANNULI_HIGH);
        for (const c of [0, 1, 2] as const) {
          worst = Math.max(worst, Math.abs(disc[c] / point[c] - 1));
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
        // At 6 px and 150° the far-field point is 1.1% above the disc's exact integral (an f64
        // surface integral): from D the visible cap stops asin(a ÷ D) short of the hemisphere,
        // and each element's weight μ ÷ r² changes by (a ÷ D)(3μ² − 1), which cancels for a
        // uniform disc but dims a crescent at the limb, −6.1 a ÷ D at 150° and −0.59 a ÷ D at
        // 90° for Lambert. The sampling adds 0.5%.
        const tolerance = px === 6 && phaseDeg === 150 ? 0.02 : 0.01;
        it(`sums to the point's flux within ${String(tolerance * 100)}% for ${name} at ${String(px)} px and ${String(phaseDeg)}°`, () => {
          expect(worstOver(px, phaseDeg, { figure })).toBeLessThan(tolerance);
        });
      }
    }
  }

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
    const pixels = compositeDiscPixels(
      rasteriseDisc(record, phaseFactorTableOf(record.law), CAMERA, VIEWPORT),
    );
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
    const pixels = rasteriseDisc(record, phaseFactorTableOf(record.law), CAMERA, VIEWPORT);
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
    const pixels = rasteriseDisc(record, phaseFactorTableOf(record.law), CAMERA, VIEWPORT);
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
    expect(plan.steps.map((step) => step.kind)).toEqual(["points", "disc"]);
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
    expect(plan.discs.map((record) => record.tableRow).toSorted((x, y) => x - y)).toEqual([
      0, 0, 1,
    ]);
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

  it("darkens the disc and the point alike, to 2%", () => {
    const disc = discFlux(body, hosts, [earth])[1] / discFlux(body, hosts)[1];
    const point =
      pointFlux(body, hosts, [occluder], DISC_ANNULI_HIGH)[1] /
      pointFlux(body, hosts, [], DISC_ANNULI_HIGH)[1];
    expect(Math.abs(disc / point - 1)).toBeLessThan(0.02);
  });

  it("leaves no light inside the umbra, and ignores an occluder beyond the star", () => {
    const umbral: LitBodyInput = {
      ...earth,
      centreM: add(body.centreM, scale(towards, 3.844e8)),
    };
    expect(discFlux(body, hosts, [umbral])[1]).toBe(0);
    const beyond: LitBodyInput = {
      ...earth,
      centreM: add(body.centreM, scale(towards, 2 * AU_M)),
    };
    expect(discFlux(body, hosts, [beyond])[1]).toBeCloseTo(discFlux(body, hosts)[1], 12);
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
    const draws = renderer.draws(plan);
    const kinds = draws.map((draw) =>
      draw.material.name.startsWith("bodies:disc")
        ? `${draw.material.name} ${String(draw.uniforms["edgePass"]?.[0])}`
        : `points ${String(draw.instanceCount)}`,
    );
    const expected = plan.steps.flatMap((step) =>
      step.kind === "disc"
        ? ["bodies:disc 0", "bodies:discLimb 1"]
        : [`points ${String(step.sprites.length)}`],
    );
    expect(kinds).toEqual(expected);
    expect(plan.steps.map((step) => step.kind)).toEqual(["points", "disc", "disc", "points"]);
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
    // Sixteen discs need 16 × 384 bytes, past the first 4,096.
    const many = planOf(16);
    expect(many.discs).toHaveLength(16);
    renderer.draws(many);
    expect(released).toContain("bodies:discs");
    renderer.dispose();
  });

  it("makes its resources again after a device loss and rewrites the table", async () => {
    const engine = await countingRenderEngine();
    const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
    renderer.draws(planOf());
    const materials = engine.counts.materials;
    engine.restore();
    expect(engine.counts.materials).toBe(materials + 3);
    renderer.draws(planOf());
    expect(engine.textureWritten.filter((w) => w.texture === "bodies:phase table")).toHaveLength(2);
    renderer.dispose();
  });
});
