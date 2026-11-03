/**
 * The ground a view drew, as the shader mixes its heights (plan R10, R10.T16.c after R10.T16.i):
 * a vertex's own height, its morph target by CDLOD's factor and its fades' floor, a rise out of
 * the datum, a refine's parent mix, the rim read from edge bakes alone, and its revision.
 */

import { describe, expect, it } from "vitest";

import { type Vec3, vec3 } from "../../geometry/vec3";
import { faceUvToDir, stToUv } from "./cube";
import { DrawnGround, type DrawnGroundBakes, type DrawnInstance } from "./drawnGround";
import { DATUM_SLOT, fadeWord, MIX_STEPS, PARENT_MIX_SLOT } from "./gpu/uniforms";
import { type PatchKey, patchKeyString } from "./patchKey";
import type { ResidentBake, SurveyStanding } from "./residentHeights";

const RADIUS_M = 1e6;
const FIGURE = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null };
const IDENTITY: readonly [Vec3, Vec3, Vec3] = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)];
const KEY: PatchKey = { face: 0, level: 6, i: 20, j: 22 };
const PARENT: PatchKey = { face: 0, level: 5, i: 10, j: 11 };

/** The direction of the point (x, y), in quads, of `key`'s patch. */
function at(key: PatchKey, x: number, y: number): Vec3 {
  const n = 2 ** key.level;
  return faceUvToDir(key.face, stToUv((key.i + x / 64) / n), stToUv((key.j + y / 64) / n));
}

/** A bake whose own height is `ownM` and morph target `morphM` everywhere, surveyed where told. */
function bakeOf(
  standing: SurveyStanding,
  ownM: number,
  morphM: number,
  surveyed: (x: number, y: number) => boolean = () => standing !== "bare",
): ResidentBake {
  return { standing, ownAtM: () => ownM, morphAtM: () => morphM, surveyedAt: surveyed };
}

/** The bakes, by key, and their rim revision. */
function bakesOf(bakes: ReadonlyMap<string, ResidentBake>, rimRevision = 0): DrawnGroundBakes {
  return { bakeAt: (key) => bakes.get(patchKeyString(key)) ?? null, rimRevision };
}

/** The instance drawing `KEY` whole, its band from 1 km to 2 km, with no holds or fades. */
const PLAIN: DrawnInstance = {
  key: KEY,
  area: KEY,
  morphStartM: 1_000,
  morphEndM: 2_000,
  hasParent: true,
  holds: 0,
  edgeFades: 0,
  cornerFades: 0,
  fades: 0,
};

/** The camera `rangeM` above the middle of `KEY`'s patch, from the body's centre. */
function cameraOver(rangeM: number): Vec3 {
  const d = at(KEY, 32, 32);
  return vec3(d.x * (RADIUS_M + rangeM), d.y * (RADIUS_M + rangeM), d.z * (RADIUS_M + rangeM));
}

/**
 * The ground drawn by `instance` over `bakes`, seen from `rangeM` up, at the middle of KEY, as the
 * rim reads it: by the shader's mix whatever the instance's fades.
 */
function drawnAt(
  instance: DrawnInstance,
  bakes: ReadonlyMap<string, ResidentBake>,
  rangeM: number,
): number | null {
  const ground = new DrawnGround(bakesOf(bakes), FIGURE);
  ground.begin();
  ground.push(instance);
  ground.end(cameraOver(rangeM), IDENTITY);
  return ground.rimHeightAtM(at(KEY, 32.25, 32.25));
}

const EDGE = new Map([[patchKeyString(KEY), bakeOf("edge", 100, 60)]]);

describe("the ground a view drew (R10.T16.c after R10.T16.i)", () => {
  it("stands at its bake's own height within its morph band's start", () => {
    expect(drawnAt(PLAIN, EDGE, 500)).toBeCloseTo(100, 6);
  });

  it("stands at its morph target past its band's end", () => {
    expect(drawnAt(PLAIN, EDGE, 5_000)).toBeCloseTo(60, 6);
  });

  it("is raised to its morph target by its own fade's floor", () => {
    expect(drawnAt({ ...PLAIN, fades: fadeWord(255, 0, 0) }, EDGE, 500)).toBeCloseTo(60, 6);
  });

  it("rises out of the datum by its mix's fade", () => {
    const rising = { ...PLAIN, fades: fadeWord(0, Math.round(MIX_STEPS / 4), DATUM_SLOT) };
    expect(drawnAt(rising, EDGE, 500)).toBeCloseTo(
      100 * (1 - Math.round(MIX_STEPS / 4) / MIX_STEPS),
      6,
    );
  });

  it("moves by its parent's own surface less its morph target, by its mix, on a refine", () => {
    const bakes = new Map([...EDGE, [patchKeyString(PARENT), bakeOf("surveyed", 80, 40)]]);
    const refining = { ...PLAIN, fades: fadeWord(0, MIX_STEPS, PARENT_MIX_SLOT) };
    // own 100 + (parent 80 − morph 60) at the band's start.
    expect(drawnAt(refining, bakes, 500)).toBeCloseTo(120, 6);
  });

  it("takes the datum for its parent where the parent held the ground unsurveyed", () => {
    const bakes = new Map([...EDGE, [patchKeyString(PARENT), bakeOf("edge", 80, 40, () => false)]]);
    const refining = { ...PLAIN, fades: fadeWord(0, MIX_STEPS, PARENT_MIX_SLOT) };
    expect(drawnAt(refining, bakes, 500)).toBeCloseTo(40, 6);
  });

  it("makes no parent mix where the shader is given no parent slot", () => {
    const bakes = new Map([...EDGE, [patchKeyString(PARENT), bakeOf("surveyed", 80, 40)]]);
    const orphan = { ...PLAIN, hasParent: false, fades: fadeWord(0, MIX_STEPS, PARENT_MIX_SLOT) };
    expect(drawnAt(orphan, bakes, 500)).toBeCloseTo(100, 6);
  });

  it("reads the ground of an instance at rest by its bake's own height, and of one fading by the drawn mix", () => {
    // Past its band's end, the drawn ground stands at the morph target, 60 m.
    const read = (fades: number): number | null => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      ground.begin();
      ground.push({ ...PLAIN, fades });
      ground.end(cameraOver(5_000), IDENTITY);
      return ground.groundHeightAtM(at(KEY, 32.25, 32.25));
    };
    expect([read(0), read(fadeWord(1, 0, 0))]).toEqual([
      expect.closeTo(100, 6),
      expect.closeTo(60, 6),
    ]);
  });

  it("reads a fading sub-patch's ground off its boundary quads as the drawn mix, as its rim reads it", () => {
    const within: PatchKey = { face: 0, level: 7, i: 41, j: 45 };
    const sloped: ResidentBake = {
      standing: "edge",
      ownAtM: (x, y) => 100 + 3 * x + y * y,
      morphAtM: (x, y) => 40 + x - y,
      surveyedAt: () => true,
    };
    const ground = new DrawnGround(bakesOf(new Map([[patchKeyString(KEY), sloped]])), FIGURE);
    ground.begin();
    ground.push({
      ...PLAIN,
      area: within,
      morphStartM: 300,
      morphEndM: 30_000,
      fades: fadeWord(90, 0, 0),
    });
    ground.end(cameraOver(500), IDENTITY);
    const place = at(within, 20.3, 41.8);
    expect(ground.groundHeightAtM(place)).toBeCloseTo(ground.rimHeightAtM(place) ?? Number.NaN, 6);
  });

  it("reads the datum over unsurveyed ground, and nothing where no instance lies", () => {
    const bare = new Map([[patchKeyString(KEY), bakeOf("edge", 100, 60, () => false)]]);
    const ground = new DrawnGround(bakesOf(bare), FIGURE);
    ground.begin();
    ground.push(PLAIN);
    ground.end(cameraOver(500), IDENTITY);
    expect([
      ground.groundHeightAtM(at(KEY, 32, 32)),
      ground.groundHeightAtM(at({ face: 0, level: 6, i: 30, j: 30 }, 32, 32)),
    ]).toEqual([0, null]);
  });

  it("reads its rim from an instance drawing a bake that holds the edge alone", () => {
    const rim = (standing: SurveyStanding): number | null => {
      const ground = new DrawnGround(
        bakesOf(new Map([[patchKeyString(KEY), bakeOf(standing, 100, 60)]])),
        FIGURE,
      );
      ground.begin();
      ground.push(PLAIN);
      ground.end(cameraOver(500), IDENTITY);
      return ground.rimHeightAtM(at(KEY, 32, 30));
    };
    expect([rim("edge"), rim("surveyed")]).toEqual([expect.closeTo(100, 6), null]);
  });

  it("draws a sub-patch's heights from its slot's patch at the area's place there", () => {
    // The area (41, 45) at level 7 within KEY's slot: its (0, 0) is KEY's (32, 32).
    const within: PatchKey = { face: 0, level: 7, i: 41, j: 45 };
    const sloped: ResidentBake = {
      standing: "edge",
      ownAtM: (x) => x,
      morphAtM: (x) => x,
      surveyedAt: () => true,
    };
    const ground = new DrawnGround(bakesOf(new Map([[patchKeyString(KEY), sloped]])), FIGURE);
    ground.begin();
    ground.push({ ...PLAIN, area: within });
    ground.end(cameraOver(500), IDENTITY);
    expect(ground.groundHeightAtM(at(within, 0, 10))).toBeCloseTo(32, 6);
  });

  it("reads a sub-patch's vertex between its slot's vertices on the slot's triangles", () => {
    // The area (40, 44) at level 7 within KEY's slot: its vertex (1, 1) is the slot's (0.5, 0.5),
    // on the diagonal of a quad whose heights are x × y: (0 + 1) ÷ 2 there, not the bilinear 0.25.
    const within: PatchKey = { face: 0, level: 7, i: 40, j: 44 };
    const saddle: ResidentBake = {
      standing: "edge",
      ownAtM: (x, y) => x * y,
      morphAtM: (x, y) => x * y,
      surveyedAt: () => true,
    };
    const ground = new DrawnGround(bakesOf(new Map([[patchKeyString(KEY), saddle]])), FIGURE);
    ground.begin();
    ground.push({ ...PLAIN, area: within });
    ground.end(cameraOver(500), IDENTITY);
    expect(ground.groundHeightAtM(at(within, 1, 1))).toBeCloseTo(0.5, 4);
  });

  it("reads each point from the instance over it, after a read over its neighbour", () => {
    const east: PatchKey = { ...KEY, i: KEY.i + 1 };
    const ground = new DrawnGround(
      bakesOf(
        new Map([
          [patchKeyString(KEY), bakeOf("edge", 100, 100)],
          [patchKeyString(east), bakeOf("edge", 300, 300)],
        ]),
      ),
      FIGURE,
    );
    ground.begin();
    ground.push(PLAIN);
    ground.push({ ...PLAIN, key: east, area: east });
    ground.end(cameraOver(500), IDENTITY);
    const reads = [at(KEY, 32, 32), at(east, 32, 32), at(KEY, 40, 32)].map((d) =>
      ground.groundHeightAtM(d),
    );
    expect(reads).toEqual([100, 300, 100].map((h) => expect.closeTo(h, 6)));
  });

  it("takes its rim's morph factor at each vertex, and its ground's at the place read (R10.T16.c's residual)", () => {
    // 400 m from the camera, vertex (32, 32) stands inside a band from 450 m to 451 m, its
    // neighbours (33, 32) and (33, 33), some 550 m out, past it, and the place (32.5, 32.25)
    // between them, some 454 m out, past it too: weights 0.5, 0.25 and 0.25.
    const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
    ground.begin();
    ground.push({ ...PLAIN, morphStartM: 450, morphEndM: 451 });
    ground.end(cameraOver(500), IDENTITY);
    const place = at(KEY, 32.5, 32.25);
    const fading = new DrawnGround(bakesOf(EDGE), FIGURE);
    fading.begin();
    fading.push({ ...PLAIN, morphStartM: 450, morphEndM: 451, fades: fadeWord(1, 0, 0) });
    fading.end(cameraOver(500), IDENTITY);
    expect([ground.rimHeightAtM(place), fading.groundHeightAtM(place)]).toEqual([
      expect.closeTo(0.5 * 100 + 0.5 * 60, 6),
      expect.closeTo(60, 6),
    ]);
  });

  describe("its revision", () => {
    const frame = (ground: DrawnGround, instance: DrawnInstance, rangeM: number): number => {
      ground.begin();
      ground.push(instance);
      ground.end(cameraOver(rangeM), IDENTITY);
      return ground.revision;
    };

    it("stands while a frame draws what the one before drew, from where it did", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      expect(frame(ground, PLAIN, 500)).toBe(frame(ground, PLAIN, 500));
    });

    it("moves with a fade of an instance drawing an edge bake", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      const first = frame(ground, PLAIN, 500);
      expect(frame(ground, { ...PLAIN, fades: fadeWord(10, 0, 0) }, 500)).toBeGreaterThan(first);
    });

    it("stands as the camera moves", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      expect(frame(ground, PLAIN, 500)).toBe(frame(ground, PLAIN, 900));
    });

    it("stands as its morph band slews", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      const first = frame(ground, PLAIN, 500);
      expect(frame(ground, { ...PLAIN, morphStartM: 1_001 }, 500)).toBe(first);
    });

    it("stands as an instance drawing a bake that holds no edge fades", () => {
      const surveyed = new Map([[patchKeyString(KEY), bakeOf("surveyed", 100, 60)]]);
      const ground = new DrawnGround(bakesOf(surveyed), FIGURE);
      const first = frame(ground, PLAIN, 500);
      expect(frame(ground, { ...PLAIN, fades: fadeWord(10, 0, 0) }, 500)).toBe(first);
    });

    it("stamps a rim read with its instance's generation, which stands as the camera moves and falls as the instance's fade does (R10.T16.i)", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      frame(ground, PLAIN, 500);
      ground.rimHeightAtM(at(KEY, 32, 30));
      const stamp = ground.lastRimStamp();
      frame(ground, PLAIN, 900);
      const moved = ground.rimStampStands(stamp);
      frame(ground, { ...PLAIN, fades: fadeWord(10, 0, 0) }, 900);
      expect([stamp > 0, moved, ground.rimStampStands(stamp)]).toEqual([true, true, false]);
    });

    it("keeps a rim read's stamp as another instance's fade moves", () => {
      const east: PatchKey = { ...KEY, i: KEY.i + 1 };
      const ground = new DrawnGround(
        bakesOf(new Map([...EDGE, [patchKeyString(east), bakeOf("edge", 300, 300)]])),
        FIGURE,
      );
      const both = (fades: number): void => {
        ground.begin();
        ground.push(PLAIN);
        ground.push({ ...PLAIN, key: east, area: east, fades });
        ground.end(cameraOver(500), IDENTITY);
      };
      both(0);
      ground.rimHeightAtM(at(KEY, 32, 30));
      const stamp = ground.lastRimStamp();
      both(fadeWord(10, 0, 0));
      expect(ground.rimStampStands(stamp)).toBe(true);
    });

    it("stamps a read over no edge instance with the drawn set's own, which falls as one comes", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      ground.begin();
      ground.end(cameraOver(500), IDENTITY);
      const none = ground.rimHeightAtM(at(KEY, 32, 30));
      const stamp = ground.lastRimStamp();
      ground.begin();
      ground.end(cameraOver(500), IDENTITY);
      const stood = ground.rimStampStands(stamp);
      frame(ground, PLAIN, 500);
      expect([none, stood, ground.rimStampStands(stamp)]).toEqual([null, true, false]);
    });

    it("reads a rim again by its read's token as its instance's fade moves, as a fresh read does", () => {
      const sloped: ResidentBake = {
        standing: "edge",
        ownAtM: (x, y) => 100 + x + 2 * y,
        morphAtM: (x) => 40 + x,
        surveyedAt: () => true,
      };
      const ground = new DrawnGround(bakesOf(new Map([[patchKeyString(KEY), sloped]])), FIGURE);
      const band = { ...PLAIN, morphStartM: 300, morphEndM: 30_000 };
      frame(ground, band, 500);
      const place = at(KEY, 20.4, 30.7);
      ground.rimHeightAtM(place);
      const token = ground.lastRimToken();
      frame(ground, { ...band, fades: fadeWord(120, 0, 0), edgeFades: 0x40 }, 500);
      const again = ground.rimAgain(token);
      const stood = ground.rimStampStands(ground.lastRimStamp());
      expect([again, stood]).toEqual([
        expect.closeTo(ground.rimHeightAtM(place) ?? Number.NaN, 9),
        true,
      ]);
    });

    it("reads none again by a token once its instance mixes, or its bake changes", () => {
      const ground = new DrawnGround(bakesOf(EDGE), FIGURE);
      frame(ground, PLAIN, 500);
      ground.rimHeightAtM(at(KEY, 32, 30));
      const token = ground.lastRimToken();
      frame(ground, { ...PLAIN, fades: fadeWord(0, 100, DATUM_SLOT) }, 500);
      const mixing = ground.rimAgain(token);
      const rebaked = new DrawnGround(
        bakesOf(new Map([[patchKeyString(KEY), bakeOf("edge", 100, 60)]])),
        FIGURE,
      );
      frame(rebaked, PLAIN, 500);
      expect([mixing, rebaked.rimAgain(token), ground.rimAgain({})]).toEqual([
        undefined,
        undefined,
        undefined,
      ]);
    });

    it("moves as the bakes' rim revision does", () => {
      const bakes = { ...bakesOf(EDGE), rimRevision: 0 };
      const ground = new DrawnGround(bakes, FIGURE);
      const first = frame(ground, PLAIN, 500);
      bakes.rimRevision = 1;
      expect(frame(ground, PLAIN, 500)).toBeGreaterThan(first);
    });
  });
});
