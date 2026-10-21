import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong, multiply, quaternionFromAxisAngle } from "../camera/quaternion";
import { memoised } from "../../test/memo";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, UNIT_BOUNDS, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, type PatchBounds, patchBounds, relativeBounds } from "./bounds";
import { fieldCellIndex } from "./coverage";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { PATCH_QUADS, vertexDir, vertexSpacing, type Xyz } from "./cube";
import {
  childKeys,
  cornerNeighbours,
  EDGES,
  FACES,
  edgeNeighbour,
  MAX_LEVEL,
  parentKey,
  type PatchKey,
  patchKeyString,
  rootKey,
} from "./patchKey";
import type { GroundContact } from "./grounded";
import {
  levelBoundM,
  levelHeightRangeM,
  type PlanetGeometry,
  planetGeometry,
  surfacePoint,
} from "./planet";
import {
  type HeightRangeLookup,
  inheritedHeightRangeM,
  PatchLeafSet,
  pruneSelectionMemo,
  SAGITTA_FACTOR,
  type Selection,
  type SelectionInput,
  selectionErrorM,
  selectPatches,
  screenSpaceErrorPx,
  type SurveyLookup,
  type ViewSelectionInput,
} from "./select";
import { SurveyedCells } from "./surveyLookup";

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
        { key: k, bounds: UNIT_BOUNDS, forced: false, seen: true, views: 1 },
      ]),
    ),
    demand: [],
    limited: false,
    limitExcess: 0,
    hiddenBaked: [],
    nearestSurveyM: Number.POSITIVE_INFINITY,
    nearestSurveyByViewM: [Number.POSITIVE_INFINITY],
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

/**
 * The leaf set's algorithm as it was before R05.T7 perf (c), its per-level maps by `patchKeyIndex`
 * kept here as one map by key string and its neighbours found as keys: the oracle the
 * tree-walking leaf set must match split for split.
 */
class ReferenceLeafSet {
  private readonly nodes = new Map<
    string,
    { key: PatchKey; children: (PatchKey | null)[] | null }
  >();
  private journal: PatchKey[] | null = null;

  addRoot(key: PatchKey): void {
    this.nodes.set(patchKeyString(key), { key, children: null });
  }

  begin(): void {
    this.journal = [];
  }

  commit(): void {
    this.journal = null;
  }

  rollback(): void {
    for (const key of (this.journal ?? []).toReversed()) {
      const node = this.nodes.get(patchKeyString(key));
      for (const child of node?.children ?? []) {
        if (child !== null) {
          this.nodes.delete(patchKeyString(child));
        }
      }
      if (node !== undefined) {
        node.children = null;
      }
    }
    this.journal = null;
  }

  /** The leaves, depth first from face 0 in `childKeys`' order. */
  leaves(): PatchKey[] {
    const out: PatchKey[] = [];
    const visit = (key: PatchKey): void => {
      const node = this.nodes.get(patchKeyString(key));
      if (node === undefined) {
        return;
      }
      if (node.children === null) {
        out.push(key);
        return;
      }
      for (const child of node.children) {
        if (child !== null) {
          visit(child);
        }
      }
    };
    for (const face of FACES) {
      visit(rootKey(face));
    }
    return out;
  }

  coarserLeaf(key: PatchKey): PatchKey | null {
    for (let level = key.level - 1; level >= 0; level -= 1) {
      const shift = 2 ** (key.level - level);
      const up: PatchKey = {
        face: key.face,
        level,
        i: Math.floor(key.i / shift),
        j: Math.floor(key.j / shift),
      };
      const node = this.nodes.get(patchKeyString(up));
      if (node !== undefined) {
        return node.children === null ? up : null;
      }
    }
    return null;
  }

  splitBalanced(key: PatchKey, keep: (child: PatchKey) => boolean): void {
    const first = this.nodes.get(patchKeyString(key));
    if (first === undefined || first.children !== null) {
      return;
    }
    const work: PatchKey[] = [];
    const split = (at: PatchKey): void => {
      const node = this.nodes.get(patchKeyString(at));
      if (node === undefined || node.children !== null) {
        return;
      }
      node.children = childKeys(at).map((child) => (keep(child) ? child : null));
      for (const child of node.children) {
        if (child !== null) {
          this.nodes.set(patchKeyString(child), { key: child, children: null });
          work.push(child);
        }
      }
      this.journal?.push(at);
    };
    split(key);
    for (let leaf = work.pop(); leaf !== undefined; leaf = work.pop()) {
      if (this.nodes.get(patchKeyString(leaf))?.children !== null) {
        continue;
      }
      for (const n of levelNeighbours(leaf)) {
        const coarse = this.coarserLeaf(n);
        if (coarse !== null && coarse.level < leaf.level - 1) {
          split(coarse);
          work.push(leaf);
          break;
        }
      }
    }
  }
}

/** Whether to keep a child: about one in seven is left out, as selection leaves out the unseen. */
function keepsChild(k: PatchKey): boolean {
  return (k.face + 3 * k.level + 7 * k.i + 11 * k.j) % 7 !== 3;
}

describe("the restricted quadtree, split by walking its trees", () => {
  it("splits, balances and undoes as the map-based leaf set did, with children left out", () => {
    const random = seededRandom(0x6c656166);
    const last = 2 ** 9 - 1;
    const edgeOr = (): number => {
      const r = random();
      return r < 0.25 ? 0 : r < 0.5 ? last : Math.floor(random() * (last + 1));
    };
    const differences: string[] = [];
    let splits = 0;
    let rollbacks = 0;
    for (let trial = 0; trial < 16; trial += 1) {
      const tree = new PatchLeafSet<PatchKey>();
      const reference = new ReferenceLeafSet();
      for (const face of FACES) {
        tree.addRoot(rootKey(face), rootKey(face));
        reference.addRoot(rootKey(face));
      }
      for (let n = 0; n < 12; n += 1) {
        // A cell at level 9, most of them on a face's edge or at its corner.
        const face = FACES[Math.floor(random() * 6)] ?? 0;
        const target: PatchKey = { face, level: 9, i: edgeOr(), j: edgeOr() };
        for (let leaf = tree.coarserLeaf(target); leaf !== null; leaf = tree.coarserLeaf(target)) {
          tree.begin();
          reference.begin();
          tree.splitBalanced(leaf, (child) => (keepsChild(child) ? child : null));
          reference.splitBalanced(leaf, keepsChild);
          splits += 1;
          const undo = random() < 0.15;
          if (undo) {
            tree.rollback();
            reference.rollback();
            rollbacks += 1;
          } else {
            tree.commit();
            reference.commit();
          }
          const got = [...tree.values()].map(patchKeyString);
          const want = reference.leaves().map(patchKeyString);
          if (got.join() !== want.join() || tree.size !== want.length) {
            differences.push(`trial ${trial}, split ${patchKeyString(leaf)}`);
          }
          if (
            JSON.stringify(tree.coarserLeaf(target)) !==
            JSON.stringify(reference.coarserLeaf(target))
          ) {
            differences.push(`trial ${trial}, leaf over ${patchKeyString(target)}`);
          }
          if (undo) {
            break;
          }
        }
      }
    }
    expect(differences).toEqual([]);
    expect(splits).toBeGreaterThan(500);
    expect(rollbacks).toBeGreaterThan(40);
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
  // when selection's output is meant to change. Re-recorded 2026-10-20 for R10.T14.e's normal
  // margin, which culls coarse patches below the horizon that R05's chord let pass (patches with no
  // budget, then under it: 12,731 to 12,721 and 300 to 299; 287 to 251 and 198 to 198; 13,220 to
  // 13,210 and 1,998 to 2,000). Re-recorded 2026-10-20 for R09.T8.d's boxes, over the subtree's
  // range with its skirts, which keep a coarse patch whose finer ground can be seen: the first
  // case's free selection, 12,721 to 12,722; every other digest as it was.
  const cases: readonly BudgetCase[] = [
    budgetCase({
      name: "one view and no baked ranges",
      views: [primary],
      budget: 300,
      bakedDepth: null,
      digests: {
        free: "12722 98bd76ce 1 84bfd6f6 false",
        budgeted: "299 3f3f0074 3 3a5eda2e true",
      },
    }),
    budgetCase({
      name: "two views over ranges baked to level 7",
      views: [primary, secondary],
      budget: 200,
      bakedDepth: 7,
      digests: {
        free: "251 c07cb76 126 2ab89143 false",
        budgeted: "198 a0d6d2df 87 8744afdf true",
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
        free: "13210 679b4a70 1 84bfd6f6 false",
        budgeted: "2000 a7ee9cb7 3 3a5eda2e true",
      },
    }),
  ];

  // Each case's selections with no budget and under its budget, made once for the tests that read
  // them: a selection is a pure function of the case, whose input is built once above, and no test
  // writes to one.
  // The cases' names are distinct.
  const freeOf = memoised(
    (c: BudgetCase): Selection => selectPatches(c.free),
    (c) => c.name,
  );
  const budgetedOf = memoised(underBudget, (c) => c.name);

  it.each(cases)("is 0 with no budget, or one not reached, with $name", (c) => {
    const free = freeOf(c);
    const roomy = selectPatches({ ...c.free, maxPatches: 2 * free.patches.size });
    expect([free.limited, free.limitExcess, roomy.limited, roomy.limitExcess]).toEqual([
      false,
      0,
      false,
      0,
    ]);
  });

  it.each(cases)("bounds every baked leaf's ρ by τ × max(1, excess ÷ w), with $name", (c) => {
    const sel = budgetedOf(c);
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
      const sel = budgetedOf(c);
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
      const sel = budgetedOf(c);
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
    expect(selectionDigest(freeOf(c))).toBe(c.digests.free);
  });

  it.each(cases)("leaves the selection under the budget as recorded, with $name", (c) => {
    expect(selectionDigest(budgetedOf(c))).toBe(c.digests.budgeted);
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

/** A filter of left-out children for {@link PatchLeafSet.bareKeys}, by the parent and the place. */
function keepsLeftOut(parent: PatchKey, n: number): boolean {
  return (parent.i + parent.j + n) % 3 !== 0;
}

/** A filter of bare split patches for {@link PatchLeafSet.bareKeys}, by level. */
function keepsBareSplit(key: PatchKey): boolean {
  return key.level % 2 === 0;
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

  it("lists, through a filter, the bare keys it keeps and no others, in the same order", () => {
    const random = seededRandom(0x62617265);
    const tree = new PatchLeafSet<PatchKey>();
    for (const face of FACES) {
      tree.addRoot(rootKey(face), rootKey(face));
    }
    // Splits down to level 7 near a cube corner, leaving children out at random, and notes which.
    const leftOut = new Set<string>();
    const makeChild = (child: PatchKey): PatchKey | null => {
      if (child.level > 2 && random() < 0.3) {
        leftOut.add(patchKeyString(child));
        return null;
      }
      return child;
    };
    for (const target of [
      { face: 0, level: 7, i: 1, j: 126 },
      { face: 2, level: 6, i: 40, j: 3 },
    ] as const) {
      for (let leaf = tree.coarserLeaf(target); leaf !== null; leaf = tree.coarserLeaf(target)) {
        tree.splitBalanced(leaf, makeChild);
      }
    }
    // Two splits that keep no child, so bare split patches.
    const keepsNone = (child: PatchKey): null => {
      leftOut.add(patchKeyString(child));
      return null;
    };
    tree.splitBalanced(rootKey(4), keepsNone);
    tree.splitBalanced(rootKey(5), keepsNone);
    const all = tree.bareKeys();
    const want = all.filter((key) => {
      const parent = parentKey(key);
      return leftOut.has(patchKeyString(key)) && parent !== null
        ? keepsLeftOut(parent, (key.i & 1) + 2 * (key.j & 1))
        : keepsBareSplit(key);
    });
    expect(tree.bareKeys({ leftOut: keepsLeftOut, split: keepsBareSplit })).toEqual(want);
    // Both kinds of bare key are found, and the filter keeps some of each.
    expect(all.filter((key) => leftOut.has(patchKeyString(key))).length).toBeGreaterThan(5);
    expect(all.filter((key) => !leftOut.has(patchKeyString(key))).length).toBeGreaterThan(0);
    expect(want.length).toBeGreaterThan(0);
    expect(want.length).toBeLessThan(all.length);
  });
});

/** A number's exact decimal form, which round-trips its bits, with −0 told from 0. */
function exact(x: number): string {
  return Object.is(x, -0) ? "-0" : String(x);
}

/**
 * Everything a selection returns, as lines: each patch in the map's order with its flags and every
 * number of its bounds, each request in order with its priority, `limited` and `limitExcess`, then
 * the hidden baked patches in order.
 */
function outputLines(sel: Selection): string[] {
  const lines: string[] = [];
  for (const [k, p] of sel.patches) {
    const { box } = p.bounds;
    const numbers = [
      p.bounds.centre.x,
      p.bounds.centre.y,
      p.bounds.centre.z,
      p.bounds.radiusM,
      p.bounds.minHeightM,
      p.bounds.maxHeightM,
      box.centre.x,
      box.centre.y,
      box.centre.z,
      ...box.axes.flatMap((a) => [a.x, a.y, a.z]),
      ...box.halfExtentsM,
    ];
    lines.push(`${k} ${patchKeyString(p.key)} ${String(p.forced)} ${String(p.seen)}`);
    lines.push(numbers.map(exact).join(" "));
  }
  for (const r of sel.demand) {
    lines.push(`${patchKeyString(r.key)} ${exact(r.priority)} ${String(r.forced)}`);
  }
  lines.push(`${String(sel.limited)} ${exact(sel.limitExcess)}`);
  lines.push(`hidden ${sel.hiddenBaked.map(patchKeyString).join(" ")}`);
  return lines;
}

/** {@link outputLines}' count of patches and requests and FNV-1a 32-bit hash. */
function outputDigest(sel: Selection): string {
  return `${sel.patches.size} ${sel.demand.length} ${fnv(outputLines(sel))}`;
}

/** A lookup of baked ranges that the test bakes into as it goes, never evicting. */
class GrowingBake implements HeightRangeLookup {
  private readonly baked = new Map<string, readonly [number, number]>();

  heightRangeM(key: PatchKey): readonly [number, number] | undefined {
    return this.baked.get(patchKeyString(key));
  }

  /** Bakes `key` at ±(100 m + 2 ε_n), a stand-in for a bake's range within its level's. */
  bake(key: PatchKey): void {
    const half = 100 + 2 * levelBoundM(PLANET, key.level);
    this.baked.set(patchKeyString(key), [-half, half]);
  }
}

/** The frames of {@link approachDigest}'s flight. */
const APPROACH_FRAMES = 48;

/**
 * The selection along an approach like the spike's (R05.T13.a's approach and flare): a glide over
 * 60 km of ground track from 20 km down to 300 m above the site, falling ever more slowly, looking
 * 20° below the horizon ahead, with a secondary view (weight 0.25, 4 px) looking 74° down, which sees
 * the contact under the craft below 1 km, at a budget of 981, baking the first 128 requests a frame.
 * The digest folds every frame's {@link outputLines}, so it pins the demand's order too, which
 * decides what is baked next.
 */
function approachDigest(): string {
  const ground = vec3(...surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), 0));
  const up = normalise(ground);
  const east = normalise(vec3(-up.y, up.x, 0));
  const along = (tiltRad: number): Vec3 =>
    normalise(
      vec3(
        east.x * Math.cos(tiltRad) - up.x * Math.sin(tiltRad),
        east.y * Math.cos(tiltRad) - up.y * Math.sin(tiltRad),
        east.z * Math.cos(tiltRad) - up.z * Math.sin(tiltRad),
      ),
    );
  const bake = new GrowingBake();
  const lines: string[] = [];
  for (let frame = 0; frame < APPROACH_FRAMES; frame += 1) {
    const s = frame / (APPROACH_FRAMES - 1);
    const heightM = 300 + 19_700 * (1 - s) ** 2;
    const trackM = -60_000 * (1 - s) ** 1.5;
    const camera = vec3(
      ground.x + east.x * trackM + up.x * heightM,
      ground.y + east.y * trackM + up.y * heightM,
      ground.z + east.z * trackM + up.z * heightM,
    );
    const beneath = vec3(
      ground.x + east.x * trackM,
      ground.y + east.y * trackM,
      ground.z + east.z * trackM,
    );
    const sel = selectPatches({
      planet: PLANET,
      views: [
        view(camera, lookAlong(along(0.35), up)),
        view(camera, lookAlong(along(1.3), up), {
          weight: 0.25,
          tauPx: 4,
          viewport: { widthPx: 960, heightPx: 540 },
        }),
      ],
      setting: "high",
      grounded: heightM < 1_000 ? [{ positionM: beneath, radiusM: 10 }] : [],
      heightRanges: bake,
      maxPatches: 981,
    });
    lines.push(`frame ${frame}`, ...outputLines(sel));
    for (const request of sel.demand.slice(0, 128)) {
      bake.bake(request.key);
    }
  }
  return `${APPROACH_FRAMES} ${fnv(lines)}`;
}

describe("selection's output, bit for bit", () => {
  // Recorded from selection before R05.T7 perf (c) (2026-10-04, with F3's `hiddenBaked`), with
  // every number it returns: the work on selection's cost changes none of them. Re-record only when
  // the output is meant to change. Re-recorded 2026-10-20 for R10.T14.e's normal margin (patches
  // 981 to 980, 1,952 to 1,952 and 12,731 to 12,721; along the approach 38,823 to 38,746 over its
  // 48 frames). Re-recorded 2026-10-20 for R09.T8.d's boxes, over the subtree's range with its
  // skirts: patches 980 to 980, 1,952 to 1,952 and 12,721 to 12,722, every box's bounds moving;
  // along the approach still 38,746 over its 48 frames, the bounds alone moving.
  const pose = view(LOW, lookingDown(LOW, 1.2));
  it.each([
    { label: "a budget of 981", budget: 981, digest: "980 3 c6b9db7c" },
    { label: "a budget of 1,952", budget: 1952, digest: "1952 3 3bc9345c" },
    { label: "no budget", budget: undefined, digest: "12722 1 903e4ada" },
  ])("is as recorded at the 1.5 km pose with $label", ({ budget, digest }) => {
    const sel = selectPatches({
      planet: PLANET,
      views: [pose],
      setting: "high",
      grounded: [],
      ...(budget === undefined ? {} : { maxPatches: budget }),
    });
    expect(outputDigest(sel)).toBe(digest);
  });

  it("is as recorded frame by frame along an approach, its bakes following its demand", () => {
    expect(approachDigest()).toBe("48 584de8a8");
  });
});

/** One view's selection on `planet` at a budget of 981. */
function selectOn(planet: PlanetGeometry, v: ViewSelectionInput): Selection {
  return selectPatches({ planet, views: [v], setting: "high", grounded: [], maxPatches: 981 });
}

describe("the bounds memo", () => {
  const near = view(LOW, lookingDown(LOW, 1.2));
  const far = view(ORBIT, lookingDown(ORBIT, 0.3));
  // Each test takes a planet of its own, so that its memo starts empty and no other call touches it.

  it("keeps through a prune the bounds the last calls read", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const selectFrom = (v: ViewSelectionInput): Selection => selectOn(planet, v);
    const first = selectFrom(near);
    expect(pruneSelectionMemo(planet)).toBeGreaterThan(first.patches.size);
    const kept = selectFrom(near);
    for (const [k, p] of kept.patches) {
      expect(p.bounds).toBe(first.patches.get(k)?.bounds);
    }
  });

  it("selects the same after a prune drops the bounds it read long ago", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const selectFrom = (v: ViewSelectionInput): Selection => selectOn(planet, v);
    const first = selectFrom(near);
    // Long enough that, of what the near view read, only the coarse cells both views read are kept.
    for (let n = 0; n < 70; n += 1) {
      selectFrom(far);
    }
    pruneSelectionMemo(planet);
    const again = selectFrom(near);
    expect(outputDigest(again)).toBe(outputDigest(first));
    const rebuilt = [...again.patches].filter(
      ([k, p]) => p.bounds !== first.patches.get(k)?.bounds,
    );
    expect(rebuilt.length).toBeGreaterThan(again.patches.size / 2);
  });
});

/** The survey's cells at the field's level 8: a 2 × 2 block under {@link SITE}. */
const SURVEY_LEVEL = 8;
const SURVEY_CELLS: ReadonlyArray<PatchKey> = [
  { face: 0, level: SURVEY_LEVEL, i: 146, j: 97 },
  { face: 0, level: SURVEY_LEVEL, i: 147, j: 97 },
  { face: 0, level: SURVEY_LEVEL, i: 146, j: 98 },
  { face: 0, level: SURVEY_LEVEL, i: 147, j: 98 },
];

/** A survey of `cells`, as the session's lookup holds a block's (R10.T16.b). */
function surveyOf(cells: ReadonlyArray<PatchKey>): SurveyedCells {
  const survey = new SurveyedCells();
  const indices = cells.map(fieldCellIndex).toSorted((a, b) => a - b);
  survey.add({
    fieldLevel: SURVEY_LEVEL,
    newlySurveyed: Uint32Array.from(indices.flatMap((index) => [index, index + 1])),
  });
  return survey;
}

/** The keys of a selection's forced patches. */
function forcedOf(sel: Selection): string[] {
  return [...sel.patches.values()].filter((p) => p.forced).map((p) => patchKeyString(p.key));
}

/** A two-view mask with its views swapped. */
function swap(mask: number): number {
  return ((mask & 1) << 1) | ((mask & 2) >> 1);
}

/** A lookup that calls every patch surveyed. */
const ALL_SURVEYED: SurveyLookup = { surveyedUnder: () => true };

/** What a selection gives, its patches' every field and the rest, for comparing two. */
function outcome(sel: Selection): unknown {
  return {
    patches: [...sel.patches.entries()].map(([k, p]) => [k, p.bounds, p.forced, p.seen, p.views]),
    demand: sel.demand,
    limited: sel.limited,
    limitExcess: sel.limitExcess,
    hiddenBaked: sel.hiddenBaked,
  };
}

describe("selection over a survey (R10.T16.b; decision-r10-t16-contracts §2–3)", () => {
  const survey = surveyOf(SURVEY_CELLS);
  const low = (): ViewSelectionInput => view(LOW, lookingDown(LOW, 1.2));
  const orbit = (): ViewSelectionInput => view(ORBIT, lookingDown(ORBIT, 0.3), { weight: 0.25 });

  it.each([
    ["two views", { views: [low(), orbit()] }],
    ["a budget", { views: [low()], maxPatches: 300 }],
    ["a contact", { views: [low()], grounded: [{ positionM: above(5), radiusM: 20 }] }],
    ["baked ranges", { views: [low()], heightRanges: bakedToDepth(6) }],
  ] as const)(
    "gives R05's selection, bit for bit, with no lookup and with one that calls everything surveyed: %s",
    (_name, given) => {
      const input: SelectionInput = { planet: PLANET, setting: "high", grounded: [], ...given };
      const plain = selectPatches(input);
      const surveyed = selectPatches({ ...input, survey: ALL_SURVEYED });
      expect(outcome(surveyed)).toEqual(outcome(plain));
      expect(plain.nearestSurveyM).toBe(Number.POSITIVE_INFINITY);
    },
  );

  it("never refines, asks for or draws a patch with no surveyed cell", () => {
    const plain = select([low()]);
    const sel = selectPatches({
      planet: PLANET,
      views: [low()],
      setting: "high",
      grounded: [],
      survey,
    });
    const bare = (key: PatchKey): boolean => !survey.surveyedUnder(key);
    expect([...sel.patches.values()].filter((p) => bare(p.key))).toEqual([]);
    expect(sel.demand.filter((r) => bare(r.key))).toEqual([]);
    // The camera sees bare ground too, which R05's selection refines.
    expect([...plain.patches.values()].some((p) => bare(p.key))).toBe(true);
    expect(sel.patches.size).toBeGreaterThan(0);
    expect(sel.patches.size).toBeLessThan(plain.patches.size);
  });

  it("selects nothing over bare ground alone, and asks for nothing", () => {
    const far = above(1_500, { face: 0, level: 19, i: 100_001, j: 400_003 });
    const sel = selectPatches({
      planet: PLANET,
      views: [view(far, lookingDown(far))],
      setting: "high",
      grounded: [],
      survey,
    });
    expect([sel.patches.size, sel.demand.length, sel.limited]).toEqual([0, 0, false]);
  });

  it("forces nothing for a contact over unsurveyed ground", () => {
    const site = above(5, { face: 0, level: 19, i: 100_001, j: 400_003 });
    const input: SelectionInput = {
      planet: PLANET,
      views: [low()],
      setting: "high",
      grounded: [{ positionM: site, radiusM: 20 }],
    };
    expect([...selectPatches(input).patches.values()].some((p) => p.forced)).toBe(true);
    const sel = selectPatches({ ...input, survey });
    expect([...sel.patches.values()].some((p) => p.forced)).toBe(false);
    expect(sel.demand.some((r) => r.forced)).toBe(false);
  });

  it("forces exactly the surveyed patches of a contact astride the survey's edge, at the finest level", () => {
    // On the low-i edge of the survey's first cell, half way along it.
    const edge = vertexDir({ face: 0, level: SURVEY_LEVEL, i: 146, j: 97 }, 0, 32);
    const [x, y, z] = surfacePoint(WGS84_FIGURE, edge, 5);
    const input: SelectionInput = {
      planet: PLANET,
      views: [low()],
      setting: "high",
      grounded: [{ positionM: vec3(x, y, z), radiusM: 20 }],
    };
    const everywhere = forcedOf(selectPatches(input));
    const sel = selectPatches({ ...input, survey });
    const surveyedOnes = everywhere.filter((k) => {
      const key = sel.patches.get(k)?.key ?? selectPatches(input).patches.get(k)?.key;
      return key !== undefined && survey.surveyedUnder(key);
    });
    expect(forcedOf(sel).toSorted()).toEqual(surveyedOnes.toSorted());
    // The edge splits the region: some of its patches lie over bare ground.
    expect(surveyedOnes.length).toBeGreaterThan(0);
    expect(surveyedOnes.length).toBeLessThan(everywhere.length);
    expect(
      [...sel.patches.values()]
        .filter((p) => p.forced)
        .every((p) => p.key.level === PLANET.finestLevel),
    ).toBe(true);
  });

  it("marks each patch with the views that see it, a patch only the instrument sees bit 1 alone", () => {
    const sel = selectPatches({
      planet: PLANET,
      views: [low(), orbit()],
      setting: "high",
      grounded: [],
    });
    const masks = new Set([...sel.patches.values()].map((p) => p.views));
    expect([...masks].every((m) => m > 0 && (m & ~0b11) === 0)).toBe(true);
    expect(masks.has(0b10)).toBe(true);
    expect(masks.has(0b01) || masks.has(0b11)).toBe(true);
  });

  it("gives the same selection with two views and a survey, in any order, called twice", () => {
    const a = low();
    const b = orbit();
    const run = (views: ViewSelectionInput[]): Selection =>
      selectPatches({ planet: PLANET, views, setting: "high", grounded: [], survey });
    const first = run([a, b]);
    const swapped = run([b, a]);
    select([view(ORBIT, lookingDown(ORBIT))]);
    const after = run([a, b]);
    expect([...swapped.patches.keys()]).toEqual([...first.patches.keys()]);
    expect(outcome(after)).toEqual(outcome(first));
    // The views' bits follow their order.
    expect([...swapped.patches.values()].map((p) => p.views)).toEqual(
      [...first.patches.values()].map((p) => swap(p.views)),
    );
  });

  it("measures the nearest surveyed ground from the cameras: no nearer than its cells, never under a finest patch, +∞ with none", () => {
    const far = above(1_500, { face: 0, level: 19, i: 100_001, j: 400_003 });
    const input: SelectionInput = {
      planet: PLANET,
      views: [view(far, lookingDown(far))],
      setting: "high",
      grounded: [],
    };
    expect(selectPatches(input).nearestSurveyM).toBe(Number.POSITIVE_INFINITY);
    expect(selectPatches({ ...input, survey: new SurveyedCells() }).nearestSurveyM).toBe(
      Number.POSITIVE_INFINITY,
    );
    const nearest = selectPatches({ ...input, survey }).nearestSurveyM;
    // The survey's nearest corner, on the datum, bounds it from above; its cells' boxes, from below.
    const corners = SURVEY_CELLS.flatMap((cell) =>
      [0, 64].flatMap((s) =>
        [0, 64].map((t) => {
          const [cx, cy, cz] = surfacePoint(WGS84_FIGURE, vertexDir(cell, s, t), 0);
          return Math.hypot(cx - far.x, cy - far.y, cz - far.z);
        }),
      ),
    );
    expect(nearest).toBeLessThanOrEqual(Math.min(...corners));
    expect(nearest).toBeGreaterThan(0.5 * Math.min(...corners));
    // Over the survey it is clamped to one finest patch.
    const over = selectPatches({ ...input, views: [low()], survey }).nearestSurveyM;
    expect(over).toBeCloseTo(
      PATCH_QUADS * vertexSpacing(PLANET.figure.equatorialRadiusM, PLANET.finestLevel).maxM,
      6,
    );
  });
});

/** A sphere of the Moon's radius whose rows to level `flatTo` reach no height and every finer one `reliefM`. */
function steppedPlanet(flatTo: number, reliefM: number): PlanetGeometry {
  const radiusM = 1_737_400;
  const table = new Float64Array(4 * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    table[4 * level + 2] = level <= flatTo ? 0 : reliefM;
    table[4 * level + 3] = vertexSpacing(radiusM, level).maxM;
  }
  return planetGeometry({ equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: null }, table);
}

describe("a box that prunes its subtree", () => {
  it("keeps a node with nothing baked where a finer row rises above the horizon and its own row does not", () => {
    // Rows to level 7 are flat and every finer one reaches 30 km. A camera 10 m above a level-7
    // patch, about 21 km across, looks along the face at another five patches off, some 100 km:
    // that patch's own row puts its box below the horizon, but its subtree's ground rises 30 km
    // above it, in view from there, so selection keeps it, and its ancestors (R09.T8.d;
    // decision-r09-t8d-reach, follow-up 1).
    const planet = steppedPlanet(7, 30_000);
    const own: PatchKey = { face: 0, level: 7, i: 40, j: 64 };
    const target: PatchKey = { face: 0, level: 7, i: 45, j: 64 };
    const [cx, cy, cz] = surfacePoint(planet.figure, vertexDir(own, 32, 32), 10);
    const camera = vec3(cx, cy, cz);
    const [tx, ty, tz] = surfacePoint(planet.figure, vertexDir(target, 32, 32), 0);
    const forward = normalise(vec3(tx - cx, ty - cy, tz - cz));
    const orientation = lookAlong(forward, normalise(camera));
    const horizon = horizonCone(planet, camera);
    const boxAbove = (range: readonly [number, number]): boolean =>
      aboveHorizon(relativeBounds(patchBounds(planet, target, range), camera), horizon);
    // The case discriminates: below the horizon by its own row, above it by its subtree's.
    expect(boxAbove(levelHeightRangeM(planet, target.level))).toBe(false);
    expect(boxAbove(inheritedHeightRangeM(planet, target, null))).toBe(true);
    const selection = selectPatches({
      planet,
      views: [view(camera, orientation)],
      setting: "high",
      grounded: [],
    });
    const [inside] = childKeys(target);
    expect(coveringLeaf(selection, inside)).not.toBeNull();
  });

  it("widens a baked range by the largest ε and skirt of the levels beneath, where ε rises", () => {
    // ε is 10 m but at level 9, where it rises to 500 m, on rows wide enough that no clamp binds.
    // From a level-6 bake, a level-8 patch's subtree reaches level 9's ground, within ε₉ of the
    // finest, and its skirts, ε₉ deeper again: ε₆ + ε₇ and ε₈ alone held neither
    // (decision-r09-t8d-reach, follow-up 2).
    const radiusM = 1_737_400;
    const table = new Float64Array(4 * (MAX_LEVEL + 1));
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      table.set([level === 9 ? 500 : 10, -1e6, 1e6, vertexSpacing(radiusM, level).maxM], 4 * level);
    }
    const planet = planetGeometry(
      { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: null },
      table,
    );
    const key: PatchKey = { face: 0, level: 8, i: 100, j: 100 };
    const baked = { level: 6, lowM: -100, highM: 100 };
    const skirtM = 2;
    const [low, high] = inheritedHeightRangeM(planet, key, baked, skirtM);
    const bound = (m: number): number => levelBoundM(planet, m);
    for (let m = key.level; m <= planet.finestLevel; m += 1) {
      // A level-m vertex or morph target lies within the larger of ε_m and ε_{m−1} of the finest
      // ground, itself within ε₆ of the bake's; its skirt hangs ε_m and the margin below.
      const reach = bound(baked.level) + Math.max(bound(m), bound(m - 1));
      expect(low).toBeLessThanOrEqual(baked.lowM - reach - bound(m) - skirtM);
      expect(high).toBeGreaterThanOrEqual(baked.highM + reach);
    }
  });
});
