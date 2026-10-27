import { describe, expect, it } from "vitest";

import { cross, normalise, scale, type Vec3 } from "../../geometry/vec3";
import { lookAlong, quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import type { AtmosphereCamera, SpheroidFigure } from "./hillaire";
import {
  MARCH_AREA_MARGIN_TEXELS,
  MARCH_TILE_REACH_TEXELS,
  MARCH_TILE_TEXELS,
  type MarchArea,
  marchAreaOf,
  marchAreaTexels,
  type MarchShell,
  marchTileKindTwin,
  type MarchTileKind,
} from "./marchArea";
import marchAreaWgsl from "./shaders/marchArea.wgsl?raw";
import rayMarchCentreWgsl from "./shaders/rayMarchCentre.wgsl?raw";
import { OBLATE_SCALE_HEIGHTS } from "./tables";
import {
  farLimbFootprintTwin,
  type FarLimbShells,
  type TwinView,
  twinView,
  unitRayTwin,
} from "./tablesCpu";

/** WGS 84's datum, m. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
};

/** Saturn's spheroid at 1 bar (Lindal et al. 1985's radii, as `oblate.ts`'s Saturn-class figure), m. */
const SATURN: SpheroidFigure = { equatorialRadiusM: 60_268_000, polarRadiusM: 54_364_000 };

/** A body of the check: its figure and its top shell's radii, a + H ÷ s_e and c + H ÷ s_p. */
interface Body {
  readonly name: string;
  readonly figure: SpheroidFigure;
  readonly shell: MarchShell;
  readonly topM: number;
}

const BODIES: readonly Body[] = [
  {
    name: "the Earth",
    figure: WGS84,
    shell: { equatorialM: WGS84.equatorialRadiusM + 100_000, polarM: WGS84.polarRadiusM + 100_000 },
    topM: 100_000,
  },
  // A giant's top over a gravity ratio that differs from the equator to the pole, the shell's axis
  // ratio unlike the datum's.
  {
    name: "a Saturn-class figure",
    figure: SATURN,
    shell: {
      equatorialM: SATURN.equatorialRadiusM + 1_200_000 / 0.9,
      polarM: SATURN.polarRadiusM + 1_200_000 / 1.08,
    },
    topM: 1_200_000,
  },
];

/** A seeded uniform deviate in [0, 1) (mulberry32), so that the cameras are the same every run. */
function deviates(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** A unit vector of uniform direction from two deviates. */
function direction(u: number, v: number): Vec3 {
  const z = 2 * u - 1;
  const r = Math.sqrt(1 - z * z);
  return { x: r * Math.cos(2 * Math.PI * v), y: r * Math.sin(2 * Math.PI * v), z };
}

/**
 * Cameras about `body` from `distanceM` from its centre, each in a direction of its own, looking at
 * the centre turned by up to `offRad` about a random axis and rolled at random, at a field of
 * `fovXRad` across `viewport`.
 */
function camerasAbout(
  distanceM: number,
  count: number,
  offRad: number,
  fovXRad: number,
  viewport: { readonly widthPx: number; readonly heightPx: number },
  seed: number,
): AtmosphereCamera[] {
  const next = deviates(seed);
  return Array.from({ length: count }, () => {
    const position = scale(direction(next(), next()), distanceM);
    const toCentre = normalise(scale(position, -1));
    const axis = normalise(cross(toCentre, direction(next(), next())));
    const forward = rotate(quaternionFromAxisAngle(axis, offRad * next()), toCentre);
    const up = rotate(quaternionFromAxisAngle(forward, 2 * Math.PI * next()), axis);
    return { positionM: position, orientation: lookAlong(forward, up), fovXRad, viewport };
  });
}

/** The far limb's shells of `body`, with a local scale height of 1 m everywhere, so that any texel the footprint's own test passes blends. */
function shellsOf(body: Body): FarLimbShells {
  return {
    groundEquatorialRadiusM: body.figure.equatorialRadiusM,
    groundPolarRadiusM: body.figure.polarRadiusM,
    topEquatorialRadiusM: body.shell.equatorialM,
    topPolarRadiusM: body.shell.polarM,
    topM: body.topM,
    scaleHeightsM: new Float64Array(OBLATE_SCALE_HEIGHTS).fill(1),
    gravity: null,
  };
}

const NOON = {
  directionBodyFixed: { x: 1, y: 0, z: 0 },
  illuminanceLx: [1, 1, 1] as const,
  angularRadiusRad: 0,
};

/** The view the twins take for `camera`, its target at `scale` of its viewport. */
function viewOf(camera: AtmosphereCamera, body: Body): TwinView {
  return twinView(camera, NOON, body.figure, 0.1);
}

/**
 * The tangent height over the top shell of the line through `uv` of `view`, m, and whether its
 * closest approach lies ahead of the camera.
 */
function topTangentOf(
  view: TwinView,
  body: Body,
  u: number,
  v: number,
): { readonly heightM: number; readonly ahead: boolean } {
  const d = unitRayTwin(view, u, v);
  const o = view.cameraM;
  const k = body.shell.equatorialM / body.shell.polarM;
  const so = { x: o.x, y: o.y, z: o.z * k };
  const sd = { x: d.x, y: d.y, z: d.z * k };
  const across = cross(so, sd);
  const qa = sd.x * sd.x + sd.y * sd.y + sd.z * sd.z;
  return {
    heightM:
      Math.sqrt((across.x ** 2 + across.y ** 2 + across.z ** 2) / qa) - body.shell.equatorialM,
    ahead: so.x * sd.x + so.y * sd.y + so.z * sd.z < 0,
  };
}

/** The tangent height over the top shell of the line through `uv` of `view`, m. */
function topTangentM(view: TwinView, body: Body, u: number, v: number): number {
  return topTangentOf(view, body, u, v).heightM;
}

/**
 * What a texel's march reads, in `f64`: whether its centre ray meets the top shell ahead of a
 * camera outside it (`rayMarch.wgsl`'s march ends where the far root is behind), and whether its
 * footprint's own test passes (`farLimbFootprintTwin` with any local scale height).
 */
function texelReads(
  view: TwinView,
  body: Body,
  area: MarchArea,
  x: number,
  y: number,
): { readonly meets: boolean; readonly blends: boolean } {
  const texel = { x, y, widthTexels: area.widthTexels, heightTexels: area.heightTexels };
  const top = topTangentOf(view, body, (x + 0.5) / area.widthTexels, (y + 0.5) / area.heightTexels);
  return {
    meets: !(top.heightM > 0) && top.ahead,
    blends: farLimbFootprintTwin(shellsOf(body), view, texel) !== null,
  };
}

/** The camera at `distanceM` on the equator looking along the limb at the datum's edge (R08.T8.c's). */
function limbCamera(
  figure: SpheroidFigure,
  distanceM: number,
  viewport: { readonly widthPx: number; readonly heightPx: number },
): AtmosphereCamera {
  const sin = figure.equatorialRadiusM / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  return {
    positionM: { x: distanceM, y: 0, z: 0 },
    orientation: lookAlong({ x: -cos, y: sin, z: 0 }, { x: sin, y: cos, z: 0 }),
    fovXRad: Math.PI / 3,
    viewport,
  };
}

describe("the ray march's area (R08.T11.d)", () => {
  const viewport = { widthPx: 320, heightPx: 180 };

  it("takes the whole target where the camera is inside the shell", () => {
    const [earth] = BODIES;
    if (earth === undefined) {
      throw new Error("BODIES opens with the Earth");
    }
    for (const camera of camerasAbout(
      WGS84.equatorialRadiusM + 50_000,
      8,
      Math.PI,
      1,
      viewport,
      1,
    )) {
      const area = marchAreaOf(camera, earth.shell, 1);
      expect(area.whole).toBe(true);
      expect([area.firstTileX, area.firstTileY, area.tilesAcross, area.tilesDown]).toEqual([
        0,
        0,
        Math.ceil(320 / MARCH_TILE_TEXELS),
        Math.ceil(180 / MARCH_TILE_TEXELS),
      ]);
    }
  });

  it("takes none where the shell lies wholly behind the camera", () => {
    const [earth] = BODIES;
    if (earth === undefined) {
      throw new Error("BODIES opens with the Earth");
    }
    const camera: AtmosphereCamera = {
      positionM: { x: 1e8, y: 0, z: 0 },
      orientation: lookAlong({ x: 1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
      fovXRad: Math.PI / 3,
      viewport,
    };
    const area = marchAreaOf(camera, earth.shell, 1);
    expect([area.tilesAcross, area.tilesDown, area.whole]).toEqual([0, 0, false]);
  });

  it("holds a far body's disc to a few tiles about its centre", () => {
    const [earth] = BODIES;
    if (earth === undefined) {
      throw new Error("BODIES opens with the Earth");
    }
    // From 10⁹ m the shell's 6.48 × 10⁶ m subtends 0.74° across, 4 texels of 320 at 60°.
    const camera: AtmosphereCamera = {
      positionM: { x: 1e9, y: 0, z: 0 },
      orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
      fovXRad: Math.PI / 3,
      viewport,
    };
    const area = marchAreaOf(camera, earth.shell, 1);
    const texels = marchAreaTexels(area);
    expect(area.whole).toBe(false);
    expect([texels.x0, texels.x1, texels.y0, texels.y1]).toEqual([152, 168, 80, 96]);
  });

  it("cuts the limb camera's frame at the top shell's highest row", () => {
    // R08.T8.c's far-limb camera at 400 km over the equator: the top's arc stands 72 rows of 1,080
    // over the frame's middle row, and the area starts within a tile and the margin of it.
    const output = { widthPx: 1_920, heightPx: 1_080 };
    const camera = limbCamera(WGS84, WGS84.equatorialRadiusM + 400_000, output);
    const [earth] = BODIES;
    if (earth === undefined) {
      throw new Error("BODIES opens with the Earth");
    }
    const view = viewOf(camera, earth);
    const area = marchAreaOf(camera, earth.shell, 1);
    const texels = marchAreaTexels(area);
    // The highest row whose centre line meets the shell, along the middle column.
    let top = 0;
    while (topTangentM(view, earth, 0.5, (top + 0.5) / 1_080) > 0) {
      top += 1;
    }
    expect(texels.y0).toBeLessThanOrEqual(top - MARCH_AREA_MARGIN_TEXELS);
    expect(texels.y0).toBeGreaterThan(top - MARCH_AREA_MARGIN_TEXELS - 2 * MARCH_TILE_TEXELS);
    expect([texels.x0, texels.x1, texels.y1]).toEqual([0, 1_920, 1_080]);
  });

  it.each(
    BODIES.flatMap((body) =>
      [1.02, 1.1, 1.6, 4, 16, 160, 1_600].flatMap((factor) =>
        [1, 0.5].map((scaleOfOutput) => [body, factor, scaleOfOutput] as const),
      ),
    ),
  )(
    "leaves outside %s's area no texel whose ray meets the top or whose footprint may blend, from %f radii, at the scale %f",
    (body, factor, scaleOfOutput) => {
      const distanceM = body.shell.equatorialM * factor;
      const angularRadius = Math.asin(Math.min(body.shell.equatorialM / distanceM, 1));
      const cameras = camerasAbout(distanceM, 6, angularRadius + 0.5, 1.05, viewport, 7 + factor);
      let outside = 0;
      for (const camera of cameras) {
        const area = marchAreaOf(camera, body.shell, scaleOfOutput);
        const texels = marchAreaTexels(area);
        const view = viewOf(camera, body);
        for (let y = 0; y < area.heightTexels; y += 1) {
          for (let x = 0; x < area.widthTexels; x += 1) {
            if (x >= texels.x0 && x < texels.x1 && y >= texels.y0 && y < texels.y1) {
              continue;
            }
            outside += 1;
            expect(texelReads(view, body, area, x, y)).toEqual({ meets: false, blends: false });
          }
        }
      }
      // From afar most of the frame lies outside the area.
      expect(outside > 0 || factor < 4).toBe(true);
    },
  );
});

describe("the tile cull's kinds (R08.T11.d, decision-r08-t8-leans.md §11 Ruling 4)", () => {
  const viewport = { widthPx: 320, heightPx: 180 };

  it.each(
    BODIES.flatMap((body) => [1.02, 1.1, 1.6, 4, 16].map((factor) => [body, factor] as const)),
  )(
    "classes as a miss only tiles every ray misses, and as the centre's only tiles no footprint blends in, %s from %f radii",
    (body, factor) => {
      const distanceM = body.shell.equatorialM * factor;
      const angularRadius = Math.asin(Math.min(body.shell.equatorialM / distanceM, 1));
      const cameras = camerasAbout(distanceM, 4, angularRadius + 0.3, 1.05, viewport, 31 + factor);
      const kinds = new Map<MarchTileKind, number>();
      for (const camera of cameras) {
        const area = marchAreaOf(camera, body.shell, 1);
        const view = viewOf(camera, body);
        const tileView = {
          cameraM: view.cameraM,
          right: view.right,
          upRay: view.upRay,
          forward: view.forward,
          above: true,
        };
        const shells = {
          groundEquatorialM: body.figure.equatorialRadiusM,
          groundPolarM: body.figure.polarRadiusM,
          topEquatorialM: body.shell.equatorialM,
          topPolarM: body.shell.polarM,
        };
        for (let ty = area.firstTileY; ty < area.firstTileY + area.tilesDown; ty += 1) {
          for (let tx = area.firstTileX; tx < area.firstTileX + area.tilesAcross; tx += 1) {
            const kind = marchTileKindTwin(tileView, shells, area, tx, ty);
            kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
            if (kind === "footprint") {
              continue;
            }
            for (let y = ty * MARCH_TILE_TEXELS; y < (ty + 1) * MARCH_TILE_TEXELS; y += 1) {
              for (let x = tx * MARCH_TILE_TEXELS; x < (tx + 1) * MARCH_TILE_TEXELS; x += 1) {
                if (x >= area.widthTexels || y >= area.heightTexels) {
                  continue;
                }
                const reads = texelReads(view, body, area, x, y);
                expect({ kind, ...reads }).toEqual({
                  kind,
                  meets: kind === "centre" && reads.meets,
                  blends: false,
                });
              }
            }
          }
        }
      }
      // Each kind is met somewhere among the cameras but the nearest's misses.
      expect((kinds.get("footprint") ?? 0) > 0 && (kinds.get("centre") ?? 0) > 0).toBe(true);
    },
  );

  it("classes no tile's footprint from inside the air", () => {
    const [earth] = BODIES;
    if (earth === undefined) {
      throw new Error("BODIES opens with the Earth");
    }
    const camera = camerasAbout(WGS84.equatorialRadiusM + 20_000, 1, 1, 1, viewport, 5)[0];
    if (camera === undefined) {
      throw new Error("one camera");
    }
    const view = viewOf(camera, earth);
    const area = marchAreaOf(camera, earth.shell, 1);
    const shells = {
      groundEquatorialM: WGS84.equatorialRadiusM,
      groundPolarM: WGS84.polarRadiusM,
      topEquatorialM: earth.shell.equatorialM,
      topPolarM: earth.shell.polarM,
    };
    const tileView = { ...view, cameraM: view.cameraM, above: false };
    const kinds = new Set<MarchTileKind>();
    for (let ty = 0; ty < area.tilesDown; ty += 1) {
      for (let tx = 0; tx < area.tilesAcross; tx += 1) {
        kinds.add(marchTileKindTwin(tileView, shells, area, tx, ty));
      }
    }
    expect([...kinds]).toEqual(["centre"]);
  });

  it("holds the kernels' constants to the twin's", () => {
    expect(marchAreaWgsl).toContain(
      `const MARCH_TILE_TEXELS : u32 = ${String(MARCH_TILE_TEXELS)}u;`,
    );
    expect(rayMarchCentreWgsl).toContain(
      `const MARCH_TILE_REACH_TEXELS : f32 = ${String(MARCH_TILE_REACH_TEXELS)};`,
    );
    // The bound as the twin forms it: |o| k² 2E (|right| ÷ width + |upRay| ÷ height), 1% and the
    // widening over it.
    expect(rayMarchCentreWgsl).toMatch(
      /let reachM = length\(origin\) \* k \* k \* 2\.0 \* MARCH_TILE_REACH_TEXELS\s*\* \(length\(view\.right\.xyz\) \/ size\.x \+ length\(view\.upRay\.xyz\) \/ size\.y\);\s*let boundM = 1\.01 \* reachM \+ marchArea\.bound\.y;/u,
    );
  });
});
