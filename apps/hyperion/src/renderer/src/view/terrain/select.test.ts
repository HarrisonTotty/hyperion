import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { lookAlong, multiply, quaternionFromAxisAngle } from "../camera/quaternion";
import { goldenLevelTable, UNIT_BOUNDS, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, patchBounds, relativeBounds } from "./bounds";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { PATCH_QUADS, vertexDir, vertexSpacing, type Xyz } from "./cube";
import {
  childKeys,
  cornerNeighbours,
  EDGES,
  FACES,
  edgeNeighbour,
  parentKey,
  type PatchKey,
  patchKeyString,
  rootKey,
} from "./patchKey";
import type { GroundContact } from "./grounded";
import { levelHeightRangeM, planetGeometry, surfacePoint } from "./planet";
import {
  type HeightRangeLookup,
  PatchLeafSet,
  SAGITTA_FACTOR,
  type Selection,
  selectionErrorM,
  selectPatches,
  screenSpaceErrorPx,
  type ViewSelectionInput,
} from "./select";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const HD = { widthPx: 1920, heightPx: 1080 };
const SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };

function above(altitudeM: number, key: PatchKey = SITE): Vec3 {
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), altitudeM);
  return vec3(x, y, z);
}

function view(
  positionM: Vec3,
  orientation: Quaternion,
  overrides: Partial<ViewSelectionInput> = {},
): ViewSelectionInput {
  return {
    camera: { positionM, orientation },
    fovXRad: Math.PI / 3,
    viewport: HD,
    weight: 1,
    tauPx: 1,
    ...overrides,
  };
}

/** A camera at `positionM` looking down at the ground, tilted `tiltRad` towards the horizon. */
function lookingDown(positionM: Vec3, tiltRad = 0): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const forward = normalise(
    vec3(
      -up.x * Math.cos(tiltRad) + east.x * Math.sin(tiltRad),
      -up.y * Math.cos(tiltRad) + east.y * Math.sin(tiltRad),
      -up.z * Math.cos(tiltRad) + east.z * Math.sin(tiltRad),
    ),
  );
  return lookAlong(forward, up);
}

function select(views: ViewSelectionInput[]): Selection {
  return selectPatches({ planet: PLANET, views, setting: "high", grounded: [] });
}

/** The selected leaf covering `key`'s area, at its level or coarser, or `null`. */
function coveringLeaf(sel: Selection, key: PatchKey): PatchKey | null {
  let k: PatchKey | null = key;
  while (k !== null) {
    if (sel.patches.has(patchKeyString(k))) {
      return k;
    }
    k = parentKey(k);
  }
  return null;
}

/** Whether no patch of `fine` is coarser than `coarse`'s at the same place. */
function neverCoarser(fine: Selection, coarse: Selection): boolean {
  for (const p of coarse.patches.values()) {
    const leaf = coveringLeaf(fine, p.key);
    if (leaf !== null && leaf.level < p.key.level) {
      return false;
    }
  }
  return true;
}

const ORBIT = above(400_000);
const LOW = above(1_500);

describe("patch selection", () => {
  it("meets τ with every selected patch in every view that sees it, or is at the finest level", () => {
    const views = [view(LOW, lookingDown(LOW, 1.2)), view(ORBIT, lookingDown(ORBIT, 0.3))];
    const sel = select(views);
    expect(sel.patches.size).toBeGreaterThan(50);
    const over: string[] = [];
    for (const p of sel.patches.values()) {
      if (p.key.level === PLANET.finestLevel) {
        continue;
      }
      for (const v of views) {
        const rel = relativeBounds(p.bounds, v.camera.positionM);
        const frustum = frustumOf({
          orientation: v.camera.orientation,
          fovXRad: v.fovXRad,
          viewport: v.viewport,
        });
        const sees =
          inFrustum(rel, frustum) && aboveHorizon(rel, horizonCone(PLANET, v.camera.positionM));
        if (sees) {
          const rho = screenSpaceErrorPx(
            selectionErrorM(PLANET, p.key.level),
            distanceToBoxM(rel),
            v,
          );
          if (rho > v.tauPx) {
            over.push(`${patchKeyString(p.key)}: ${rho} px`);
          }
        }
      }
    }
    expect(over).toEqual([]);
  });

  it("never lets two neighbours differ by more than one level, across face edges and corners", () => {
    // A camera 1 km above the datum over a cube corner, where three faces meet.
    const corner = normalise(vec3(1, 1, 1));
    const [cx, cy, cz] = surfacePoint(WGS84_FIGURE, [corner.x, corner.y, corner.z], 1_000);
    const site = vec3(cx, cy, cz);
    const sel = select([view(site, lookingDown(site, 1.0)), view(site, lookingDown(site, -1.0))]);
    const steps: string[] = [];
    for (const p of sel.patches.values()) {
      const around = EDGES.map((e) => edgeNeighbour(p.key, e));
      for (const c of cornerNeighbours(p.key)) {
        if (c !== null) {
          around.push(c);
        }
      }
      for (const n of around) {
        const leaf = coveringLeaf(sel, n);
        if (leaf !== null && p.key.level - leaf.level > 1) {
          steps.push(`${patchKeyString(p.key)} beside ${patchKeyString(leaf)}`);
        }
      }
    }
    expect(steps).toEqual([]);
    // The cube corner's three faces all take part.
    expect(new Set([...sel.patches.values()].map((p) => p.key.face)).size).toBeGreaterThanOrEqual(
      3,
    );
  });

  it("gives the same selection for views in any order, called twice", () => {
    const a = view(LOW, lookingDown(LOW, 1.2));
    const b = view(ORBIT, lookingDown(ORBIT, 0.3), { weight: 0.25 });
    const first = select([a, b]);
    const again = select([a, b]);
    const swapped = select([b, a]);
    // Calls on another planet and from another pose in between change nothing.
    selectPatches({
      planet: planetGeometry(WGS84_FIGURE, null),
      views: [a],
      setting: "low",
      grounded: [],
    });
    const far = above(20_000_000);
    select([view(far, lookingDown(far))]);
    const after = select([a, b]);
    expect([...again.patches.keys()]).toEqual([...first.patches.keys()]);
    expect([...swapped.patches.keys()]).toEqual([...first.patches.keys()]);
    expect([...after.patches.keys()]).toEqual([...first.patches.keys()]);
  });

  it("changes with a turn of the camera only through culling", () => {
    // Six views of a cube map see every direction, so nothing is culled by a frustum; turning the
    // whole rig about the camera's position must then change nothing.
    const axes = [
      vec3(1, 0, 0),
      vec3(-1, 0, 0),
      vec3(0, 1, 0),
      vec3(0, -1, 0),
      vec3(0, 0, 1),
      vec3(0, 0, -1),
    ];
    const rig = (turn: Quaternion): ViewSelectionInput[] =>
      axes.map((f) => {
        const up = Math.abs(f.z) > 0.5 ? vec3(1, 0, 0) : vec3(0, 0, 1);
        return view(LOW, multiply(turn, lookAlong(f, up)), {
          fovXRad: Math.PI / 2 + 0.2,
          viewport: { widthPx: 1024, heightPx: 1024 },
        });
      });
    const still = select(rig({ w: 1, x: 0, y: 0, z: 0 }));
    const turned = select(rig(quaternionFromAxisAngle(normalise(vec3(1, 2, 3)), 0.7)));
    expect([...turned.patches.keys()]).toEqual([...still.patches.keys()]);
  });

  it("refines with a narrower field of view", () => {
    const pose = lookingDown(LOW, 1.0);
    const wide = select([view(LOW, pose, { fovXRad: Math.PI / 2 })]);
    const narrow = select([view(LOW, pose, { fovXRad: Math.PI / 6 })]);
    expect(neverCoarser(narrow, wide)).toBe(true);
    const finer = [...narrow.patches.values()].some((p) => {
      const leaf = coveringLeaf(wide, p.key);
      return leaf !== null && leaf.level < p.key.level;
    });
    expect(finer).toBe(true);
  });

  it("never selects a finer level at τ = 2 px than at τ = 1 px", () => {
    const pose = lookingDown(LOW, 1.0);
    const one = select([view(LOW, pose, { tauPx: 1 })]);
    const two = select([view(LOW, pose, { tauPx: 2 })]);
    expect(neverCoarser(one, two)).toBe(true);
    expect(two.patches.size).toBeLessThan(one.patches.size);
  });

  it("refines to the finest level under a camera inside a patch's bounds", () => {
    const camera = above(2);
    const sel = select([view(camera, lookingDown(camera, 0.5))]);
    const under = coveringLeaf(sel, SITE);
    expect(under).not.toBeNull();
    expect(under?.level).toBe(PLANET.finestLevel);
    expect(distanceToBoxM(relativeBounds(patchBounds(PLANET, SITE), camera))).toBe(0);
  });

  it("selects only the six faces' roots, or nothing finer than needed, from far away", () => {
    const far = above(5e9);
    const sel = select([view(far, lookingDown(far))]);
    expect(Math.max(...[...sel.patches.values()].map((p) => p.key.level))).toBe(0);
  });

  it("refines a zero-height spheroid by its chords' sagitta alone", () => {
    const smooth = planetGeometry(WGS84_FIGURE, null);
    const sel = selectPatches({
      planet: smooth,
      views: [view(LOW, lookingDown(LOW, 0.3))],
      setting: "high",
      grounded: [],
    });
    // From 1.5 km the chords of level 4 sag about 4.6 m, 5 px at 1080p: it must refine past them.
    expect(Math.max(...[...sel.patches.values()].map((p) => p.key.level))).toBeGreaterThan(4);
  });
});

/** The squared distance between two points. */
function d2(p: Xyz, q: Xyz): number {
  return (p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2;
}

/** The radius squared of the smallest disc holding the triangle a, b, c. */
function enclosingRadiusSq(a: Xyz, b: Xyz, c: Xyz): number {
  const ab = d2(a, b);
  const bc = d2(b, c);
  const ca = d2(c, a);
  const longest = Math.max(ab, bc, ca);
  // An obtuse or right triangle's disc is its longest side's; an acute one's is its circumcircle,
  // R² = ab · bc · ca ÷ (16 A²) with 16 A² from the squared sides (Heron).
  if (longest >= ab + bc + ca - longest) {
    return longest / 4;
  }
  return (ab * bc * ca) / (2 * (ab * bc + bc * ca + ca * ab) - (ab * ab + bc * bc + ca * ca));
}

describe("the chords' sagitta", () => {
  it("bounds the mesh triangles' enclosing discs within its factor", () => {
    // Every triangle of a face's 64 × 64 grid, split on the (0, 0)–(1, 1) diagonal, on the unit
    // sphere: r² ≤ 1.023 h² ÷ 2 for h the largest vertex spacing (the ratio the factor covers).
    const root: PatchKey = { face: 0, level: 0, i: 0, j: 0 };
    const h = vertexSpacing(1, 0).maxM;
    let worst = 0;
    for (let y = 0; y < PATCH_QUADS; y += 1) {
      for (let x = 0; x < PATCH_QUADS; x += 1) {
        const p00 = vertexDir(root, x, y);
        const p10 = vertexDir(root, x + 1, y);
        const p01 = vertexDir(root, x, y + 1);
        const p11 = vertexDir(root, x + 1, y + 1);
        worst = Math.max(
          worst,
          enclosingRadiusSq(p00, p10, p11) / ((h * h) / 2),
          enclosingRadiusSq(p00, p11, p01) / ((h * h) / 2),
        );
      }
    }
    expect(worst).toBeGreaterThan(1.02);
    expect(worst).toBeLessThan(1.025);
    expect(worst).toBeLessThan(SAGITTA_FACTOR);
  });
});

/**
 * The leaves after splitting the six faces' roots down the path to `target`, each split balanced as
 * selection balances it, every child kept.
 */
function splitDownTo(target: PatchKey): PatchKey[] {
  const tree = new PatchLeafSet<PatchKey>();
  for (const face of [0, 1, 2, 3, 4, 5] as const) {
    tree.add(rootKey(face), rootKey(face));
  }
  const path: PatchKey[] = [];
  for (let up = parentKey(target); up !== null; up = parentKey(up)) {
    path.unshift(up);
  }
  for (const key of path) {
    tree.splitBalanced(key, (child) => child);
  }
  return [...tree.values()];
}

/** How many leaves are more than one level finer than a neighbour's covering leaf. */
function levelGaps(keys: readonly PatchKey[]): number {
  const sel: Selection = {
    patches: new Map(
      keys.map((k) => [
        patchKeyString(k),
        { key: k, bounds: UNIT_BOUNDS, forced: false, seen: true },
      ]),
    ),
    demand: [],
    limited: false,
  };
  let gaps = 0;
  for (const k of keys) {
    const around = EDGES.map((e) => edgeNeighbour(k, e));
    for (const c of cornerNeighbours(k)) {
      if (c !== null) {
        around.push(c);
      }
    }
    for (const n of around) {
      const leaf = coveringLeaf(sel, n);
      if (leaf !== null && k.level - leaf.level > 1) {
        gaps += 1;
      }
    }
  }
  return gaps;
}

describe("the restricted quadtree", () => {
  it("splits the neighbouring face when a cell beside its edge is split two levels deeper", () => {
    // A cell of face 0 at level 4 on its +u edge, which meets face 1.
    const out = splitDownTo({ face: 0, level: 4, i: 15, j: 7 });
    expect(levelGaps(out)).toBe(0);
    expect(out.some((k) => k.face === 1 && k.level >= 3)).toBe(true);
  });

  it("splits about a cube corner, where three faces meet", () => {
    const out = splitDownTo({ face: 0, level: 5, i: 31, j: 31 });
    expect(levelGaps(out)).toBe(0);
    expect(out.some((k) => k.face === 1 && k.level >= 4)).toBe(true);
    expect(out.some((k) => k.face === 2 && k.level >= 4)).toBe(true);
  });

  it("undoes a split and the balance it brought", () => {
    const tree = new PatchLeafSet<PatchKey>();
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      tree.add(rootKey(face), rootKey(face));
    }
    for (const key of [rootKey(0), { face: 0 as const, level: 1, i: 1, j: 1 }]) {
      tree.splitBalanced(key, (child) => child);
    }
    const before = [...tree.values()].map(patchKeyString);
    tree.begin();
    tree.splitBalanced({ face: 0, level: 2, i: 3, j: 3 }, (child) => child);
    expect(tree.size).toBeGreaterThan(before.length + 3);
    tree.rollback();
    expect([...tree.values()].map(patchKeyString).toSorted()).toEqual(before.toSorted());
    expect(tree.size).toBe(before.length);
    // The undone split's patch is a leaf again, so it covers its grandchildren's area.
    expect(tree.coarserLeaf({ face: 0, level: 4, i: 12, j: 12 })).toEqual({
      face: 0,
      level: 2,
      i: 3,
      j: 3,
    });
  });
});

describe("selection under a patch budget", () => {
  const pose = (): ViewSelectionInput => view(LOW, lookingDown(LOW, 1.2));

  it("never exceeds the budget, and says so when the budget stops a split", () => {
    const capped = selectPatches({
      planet: PLANET,
      views: [pose()],
      setting: "high",
      grounded: [],
      maxPatches: 200,
    });
    expect(capped.patches.size).toBeLessThanOrEqual(200);
    expect(capped.limited).toBe(true);
  });

  it("equals the selection with no budget when the budget is not reached", () => {
    const free = select([pose()]);
    expect(free.limited).toBe(false);
    const roomy = selectPatches({
      planet: PLANET,
      views: [pose()],
      setting: "high",
      grounded: [],
      // A split can drop children no view sees, so a run may pass through more leaves than it ends with.
      maxPatches: 2 * free.patches.size,
    });
    expect(roomy.limited).toBe(false);
    expect([...roomy.patches.keys()]).toEqual([...free.patches.keys()]);
  });

  it("leaves a patch above τ only when limited", () => {
    const capped = selectPatches({
      planet: PLANET,
      views: [pose()],
      setting: "high",
      grounded: [],
      maxPatches: 300,
    });
    const v = pose();
    const overTau = [...capped.patches.values()].some((p) => {
      if (p.key.level >= PLANET.finestLevel) {
        return false;
      }
      const rel = relativeBounds(p.bounds, v.camera.positionM);
      const sees =
        inFrustum(
          rel,
          frustumOf({
            orientation: v.camera.orientation,
            fovXRad: v.fovXRad,
            viewport: v.viewport,
          }),
        ) && aboveHorizon(rel, horizonCone(PLANET, v.camera.positionM));
      return (
        sees &&
        screenSpaceErrorPx(selectionErrorM(PLANET, p.key.level), distanceToBoxM(rel), v) > v.tauPx
      );
    });
    expect([capped.limited, overTau]).toEqual([true, true]);
  });

  it("keeps every forced patch whatever the budget", () => {
    const site = above(5);
    const contact: GroundContact = { positionM: site, radiusM: 20 };
    const free = selectPatches({
      planet: PLANET,
      views: [pose()],
      setting: "high",
      grounded: [contact],
    });
    const forced = [...free.patches.values()]
      .filter((p) => p.forced)
      .map((p) => patchKeyString(p.key));
    expect(forced.length).toBeGreaterThan(4);
    const tight = selectPatches({
      planet: PLANET,
      views: [pose()],
      setting: "high",
      grounded: [contact],
      maxPatches: 10,
    });
    expect(forced.filter((k) => tight.patches.get(k)?.forced !== true)).toEqual([]);
  });
});

/** A lookup of baked ranges in which `resident` are baked at their level's range. */
function residentRanges(resident: readonly PatchKey[]): HeightRangeLookup {
  const baked = new Map(
    resident.map((k) => [patchKeyString(k), levelHeightRangeM(PLANET, k.level)]),
  );
  return { heightRangeM: (key) => baked.get(patchKeyString(key)) };
}

describe("demand", () => {
  it("asks only for patches whose parent is baked, roots apart", () => {
    const roots = FACES.map((f) => rootKey(f));
    const resident = [...roots, ...roots.flatMap((r) => childKeys(r))];
    const sel = selectPatches({
      planet: PLANET,
      views: [view(LOW, lookingDown(LOW, 1.2))],
      setting: "high",
      grounded: [],
      heightRanges: residentRanges(resident),
    });
    const bakedStrings = new Set(resident.map(patchKeyString));
    expect(sel.demand.length).toBeGreaterThan(0);
    for (const r of sel.demand) {
      const parent = parentKey(r.key);
      expect(parent === null || bakedStrings.has(patchKeyString(parent))).toBe(true);
      expect(bakedStrings.has(patchKeyString(r.key))).toBe(false);
    }
  });

  it("asks for the roots first when nothing is baked", () => {
    const sel = select([view(LOW, lookingDown(LOW, 1.2))]);
    expect(sel.demand.every((r) => r.key.level === 0)).toBe(true);
  });

  it("selects coarser where baked ranges tighten the bounds", () => {
    const none = select([view(LOW, lookingDown(LOW, 1.2))]);
    // Everything down to level 6 baked, at a range of ±100 m.
    const tight: HeightRangeLookup = {
      heightRangeM: (key) => (key.level <= 6 ? [-100, 100] : undefined),
    };
    const baked = selectPatches({
      planet: PLANET,
      views: [view(LOW, lookingDown(LOW, 1.2))],
      setting: "high",
      grounded: [],
      heightRanges: tight,
    });
    expect(baked.patches.size).toBeLessThan(none.patches.size);
  });
});
