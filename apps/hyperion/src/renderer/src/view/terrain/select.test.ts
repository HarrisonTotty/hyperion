import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong, multiply, quaternionFromAxisAngle } from "../camera/quaternion";
import { goldenLevelTable, UNIT_BOUNDS, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, type PatchBounds, patchBounds, relativeBounds } from "./bounds";
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
  inheritedHeightRangeM,
  PatchLeafSet,
  SAGITTA_FACTOR,
  type Selection,
  type SelectionInput,
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

  it("reuses each patch's bounds on a second call with unchanged input", () => {
    const v = view(LOW, lookingDown(LOW, 1.2));
    const first = select([v]);
    const second = select([v]);
    expect([...second.patches.keys()]).toEqual([...first.patches.keys()]);
    for (const [k, p] of second.patches) {
      expect(p.bounds).toBe(first.patches.get(k)?.bounds);
    }
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
    tree.addRoot(rootKey(face), rootKey(face));
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
    limitExcess: 0,
    hiddenBaked: [],
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
      tree.addRoot(rootKey(face), rootKey(face));
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

/** A lookup in which every patch down to `depth` is baked at ±100 m. */
function bakedToDepth(depth: number): HeightRangeLookup {
  return { heightRangeM: (key) => (key.level <= depth ? [-100, 100] : undefined) };
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

  it("selects no deeper than one level below what is baked, where ranges are given", () => {
    for (const depth of [-1, 0, 3, 7]) {
      const sel = selectPatches({
        planet: PLANET,
        views: [view(LOW, lookingDown(LOW, 1.2))],
        setting: "high",
        grounded: [],
        heightRanges: bakedToDepth(depth),
      });
      const deepest = Math.max(...[...sel.patches.values()].map((p) => p.key.level));
      expect(deepest).toBe(depth + 1);
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

/**
 * Each view's w × ρ ÷ τ for a patch at `level` with `bounds`, `null` where the view does not see
 * it: ρ at the nearest point of the patch's box, floored at the near plane as selection floors it.
 */
function weightedExcesses(
  bounds: PatchBounds,
  level: number,
  views: readonly ViewSelectionInput[],
): (number | null)[] {
  return views.map((v) => {
    const rel = relativeBounds(bounds, v.camera.positionM);
    const frustum = frustumOf({
      orientation: v.camera.orientation,
      fovXRad: v.fovXRad,
      viewport: v.viewport,
    });
    if (!inFrustum(rel, frustum) || !aboveHorizon(rel, horizonCone(PLANET, v.camera.positionM))) {
      return null;
    }
    const distanceM = Math.max(distanceToBoxM(rel), NEAR_PLANE_M);
    return (v.weight * screenSpaceErrorPx(selectionErrorM(PLANET, level), distanceM, v)) / v.tauPx;
  });
}

/** The patches of a key's level sharing an edge or a corner with it, across face edges too. */
function levelNeighbours(key: PatchKey): PatchKey[] {
  const around = EDGES.map((e) => edgeNeighbour(key, e));
  for (const corner of cornerNeighbours(key)) {
    if (corner !== null) {
      around.push(corner);
    }
  }
  return around;
}

/** The FNV-1a 32-bit hash of `lines`, each ended by a newline, in hexadecimal. */
function fnv(lines: readonly string[]): string {
  let hash = 0x811c9dc5;
  for (const line of lines) {
    for (const char of `${line}\n`) {
      hash = Math.imul(hash ^ (char.codePointAt(0) ?? 0), 0x01000193) >>> 0;
    }
  }
  return hash.toString(16);
}

/**
 * The selection's sorted patch keys and its demand's keys in order, as counts and FNV-1a 32-bit
 * hashes, and `limited`.
 */
function selectionDigest(sel: Selection): string {
  const keys = [...sel.patches.keys()].toSorted();
  const demand = sel.demand.map((r) => patchKeyString(r.key));
  return `${keys.length} ${fnv(keys)} ${demand.length} ${fnv(demand)} ${String(sel.limited)}`;
}

/** A selection to put under a budget that binds, as the case gives it. */
interface BudgetCaseSpec {
  readonly name: string;
  readonly views: ReadonlyArray<ViewSelectionInput>;
  /** A budget it reaches. */
  readonly budget: number;
  /**
   * The deepest level baked, every patch to it at ±100 m (`bakedToDepth`), or `null` where the case
   * gives no ranges.
   */
  readonly bakedDepth: number | null;
  /** {@link selectionDigest} of the selection with no budget and under the budget, as recorded. */
  readonly digests: { readonly free: string; readonly budgeted: string };
}

/** A case with its input built. */
interface BudgetCase extends BudgetCaseSpec {
  /** The input on the high setting with no budget, its ranges from `bakedDepth`. */
  readonly free: SelectionInput;
}

/** The case with its input built from `spec`. */
function budgetCase(spec: BudgetCaseSpec): BudgetCase {
  const base: SelectionInput = { planet: PLANET, views: spec.views, setting: "high", grounded: [] };
  return {
    ...spec,
    free:
      spec.bakedDepth === null ? base : { ...base, heightRanges: bakedToDepth(spec.bakedDepth) },
  };
}

/** Whether the case counts `key` as baked: every patch, where it gives no ranges. */
function isBaked(c: BudgetCase, key: PatchKey): boolean {
  return c.bakedDepth === null || key.level <= c.bakedDepth;
}

/**
 * A split patch's bounds as selection builds them: from its level's range where the case gives no
 * ranges, or else from its own baked range, since selection splits only baked patches then.
 */
function boundsOfSplit(c: BudgetCase, key: PatchKey): PatchBounds {
  return c.bakedDepth === null
    ? patchBounds(PLANET, key)
    : patchBounds(
        PLANET,
        key,
        inheritedHeightRangeM(PLANET, key, { level: key.level, lowM: -100, highM: 100 }),
      );
}

/** The case's selection under its budget. */
function underBudget(c: BudgetCase): Selection {
  return selectPatches({ ...c.free, maxPatches: c.budget });
}

describe("the budget's limit excess", () => {
  const primary = view(LOW, lookingDown(LOW, 1.2));
  // A secondary view at the wireframe's tolerance, looking more steeply down.
  const secondary = view(LOW, lookingDown(LOW, 0.4), { weight: 0.25, tauPx: 4 });
  // The digests were recorded from selection before `limitExcess` was added (2026-10-04), and
  // guard the work on selection's cost (decision-r05-high-bound.md, item 4): re-record them only
  // when selection's output is meant to change.
  const cases: readonly BudgetCase[] = [
    budgetCase({
      name: "one view and no baked ranges",
      views: [primary],
      budget: 300,
      bakedDepth: null,
      digests: {
        free: "12731 98452de7 1 84bfd6f6 false",
        budgeted: "300 e43ad952 3 3a5eda2e true",
      },
    }),
    budgetCase({
      name: "two views over ranges baked to level 7",
      views: [primary, secondary],
      budget: 200,
      bakedDepth: 7,
      digests: {
        free: "287 5d6b78f0 159 ea8fbb82 false",
        budgeted: "198 187cb71c 84 7a9121f2 true",
      },
    }),
    // The refused split is one only the secondary view wants, its weighted excess under 1.
    budgetCase({
      name: "a primary view from orbit and a secondary from 1.5 km",
      views: [
        view(ORBIT, lookingDown(ORBIT, 0.3)),
        view(LOW, lookingDown(LOW, 1.2), { weight: 0.25 }),
      ],
      budget: 2000,
      bakedDepth: null,
      digests: {
        free: "13220 d29cc345 1 84bfd6f6 false",
        budgeted: "1998 1bfeb10d 1 84bfd6f6 true",
      },
    }),
  ];

  it.each(cases)("is 0 with no budget, or one not reached, with $name", (c) => {
    const free = selectPatches(c.free);
    const roomy = selectPatches({ ...c.free, maxPatches: 2 * free.patches.size });
    expect([free.limited, free.limitExcess, roomy.limited, roomy.limitExcess]).toEqual([
      false,
      0,
      false,
      0,
    ]);
  });

  it.each(cases)("bounds every baked leaf's ρ by τ × max(1, excess ÷ w), with $name", (c) => {
    const sel = underBudget(c);
    expect(sel.limited).toBe(true);
    expect(sel.limitExcess).toBeGreaterThan(0);
    const over: string[] = [];
    for (const p of sel.patches.values()) {
      if (!p.seen || !isBaked(c, p.key)) {
        continue;
      }
      weightedExcesses(p.bounds, p.key.level, c.free.views).forEach((weighted, n) => {
        const v = c.free.views[n];
        if (weighted === null || v === undefined) {
          return;
        }
        const rhoPx = (weighted * v.tauPx) / v.weight;
        const effectiveTauPx = v.tauPx * Math.max(1, sel.limitExcess / v.weight);
        if (rhoPx > effectiveTauPx * (1 + 1e-9)) {
          over.push(`${patchKeyString(p.key)} in view ${n}: ${rhoPx} px`);
        }
      });
    }
    expect(over).toEqual([]);
  });

  it.each(cases)(
    "equals the largest weighted excess of a leaf still wanting a split, with $name",
    (c) => {
      const sel = underBudget(c);
      expect(sel.limited).toBe(true);
      // The greedy splits the largest weighted excess first, so the refused patch is the leaf that
      // most wants a split, and stays one.
      let largest = 0;
      for (const p of sel.patches.values()) {
        if (!p.seen || !isBaked(c, p.key) || p.key.level >= PLANET.finestLevel) {
          continue;
        }
        const excesses = weightedExcesses(p.bounds, p.key.level, c.free.views);
        const wanted = excesses.some((weighted, n) => {
          const v = c.free.views[n];
          return weighted !== null && v !== undefined && weighted / v.weight > 1;
        });
        if (wanted) {
          largest = Math.max(largest, ...excesses.map((weighted) => weighted ?? 0));
        }
      }
      expect(Math.abs(largest - sel.limitExcess) / sel.limitExcess).toBeLessThan(1e-9);
    },
  );

  it.each(cases)(
    "is at most the weighted excess of every split the balance did not make, with $name",
    (c) => {
      const sel = underBudget(c);
      expect(sel.limited).toBe(true);
      // Every split patch: every ancestor of a selected one.
      const split = new Map<string, PatchKey>();
      for (const p of sel.patches.values()) {
        for (let k = parentKey(p.key); k !== null; k = parentKey(k)) {
          split.set(patchKeyString(k), k);
        }
      }
      const unexplained: string[] = [];
      for (const key of split.values()) {
        const weighted = weightedExcesses(boundsOfSplit(c, key), key.level, c.free.views);
        if (Math.max(0, ...weighted.map((w) => w ?? 0)) >= sel.limitExcess * (1 - 1e-9)) {
          continue;
        }
        // Only the balance splits a patch below the refused excess: one that a split patch a level
        // finer touches, whose children it would otherwise meet two levels apart.
        const own = patchKeyString(key);
        const balanced = childKeys(key).some((child) =>
          levelNeighbours(child).some((n) => {
            const parent = parentKey(n);
            return (
              parent !== null && patchKeyString(parent) !== own && split.has(patchKeyString(n))
            );
          }),
        );
        if (!balanced) {
          unexplained.push(patchKeyString(key));
        }
      }
      expect(unexplained).toEqual([]);
    },
  );

  it.each(cases)("leaves the selection with no budget as recorded, with $name", (c) => {
    expect(selectionDigest(selectPatches(c.free))).toBe(c.digests.free);
  });

  it.each(cases)("leaves the selection under the budget as recorded, with $name", (c) => {
    expect(selectionDigest(underBudget(c))).toBe(c.digests.budgeted);
  });
});

/** The patches split above the selected ones: every ancestor of a selected patch. */
function splitAbove(sel: Selection): Map<string, PatchKey> {
  const split = new Map<string, PatchKey>();
  for (const p of sel.patches.values()) {
    for (let k = parentKey(p.key); k !== null; k = parentKey(k)) {
      split.set(patchKeyString(k), k);
    }
  }
  return split;
}

describe("the baked patches selection finds hidden (the high-bound ruling's F3)", () => {
  const towardsHorizon = (): Selection =>
    selectPatches({
      planet: PLANET,
      views: [view(LOW, lookingDown(LOW, 1.45))],
      setting: "high",
      grounded: [],
      heightRanges: bakedToDepth(6),
    });

  it("lists only baked patches it neither selected nor split above a selected one", () => {
    const sel = towardsHorizon();
    const split = splitAbove(sel);
    expect(sel.hiddenBaked.length).toBeGreaterThan(0);
    for (const key of sel.hiddenBaked) {
      const k = patchKeyString(key);
      expect(key.level).toBeLessThanOrEqual(6);
      expect(sel.patches.has(k)).toBe(false);
      expect(split.has(k)).toBe(false);
    }
  });

  it("lists every baked child of a split patch that it neither selected nor split", () => {
    const sel = towardsHorizon();
    const split = splitAbove(sel);
    const hidden = new Set(sel.hiddenBaked.map(patchKeyString));
    const missed: string[] = [];
    for (const key of split.values()) {
      for (const child of childKeys(key)) {
        const c = patchKeyString(child);
        if (child.level <= 6 && !sel.patches.has(c) && !split.has(c) && !hidden.has(c)) {
          missed.push(c);
        }
      }
    }
    expect(missed).toEqual([]);
  });

  it("gives the same list, in the same order, for the same input", () => {
    expect(towardsHorizon().hiddenBaked).toEqual(towardsHorizon().hiddenBaked);
  });

  it("lists nothing without baked ranges", () => {
    expect(select([view(LOW, lookingDown(LOW, 1.45))]).hiddenBaked).toEqual([]);
  });

  it("finds bare the children a split left out and the splits with no leaf beneath", () => {
    const root = rootKey(0);
    const [c0, c1, c2, c3] = childKeys(root);
    const tree = new PatchLeafSet<PatchKey>();
    tree.addRoot(root, root);
    tree.splitBalanced(root, (child) =>
      patchKeyString(child) === patchKeyString(c0) ? child : null,
    );
    expect(tree.bareKeys()).toEqual([c1, c2, c3]);
    tree.splitBalanced(c0, () => null);
    expect(tree.bareKeys()).toEqual([...childKeys(c0), c0, c1, c2, c3, root]);
  });
});
