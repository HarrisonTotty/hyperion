import { describe, expect, it, vi } from "vitest";

import { cross, norm, normalise, sub, vec3, type Vec3 } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  SurfaceFields,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { lawFor } from "../appearance/phase";
import type { LitRegime } from "../bodies/regime";
import { project, type Viewport } from "../camera/projection";
import type { Quaternion } from "../camera/pose";
import { conjugate, multiply, quaternionFromRows } from "../camera/quaternion";
import { rotateToBodyFixed } from "../coords/rotation";
import { relativeToCamera } from "../coords/relative";
import type { QualitySetting } from "../quality/qualitySetting";
import { cameraSceneOf } from "../scene/model";
import {
  TERRAIN_TEST_SCRIPT,
  TERRAIN_TEST_UNIVERSE,
  terrainScene,
  terrainSceneFixture,
  terrainTestGolden,
} from "../scenes/terrainScene";
import { distanceToBoxM, relativeBounds } from "./bounds";
import { fadedMorph, FADE_STEPS } from "./coarseFineHolds";
import { vertexDir } from "./cube";
import { packSurveyMask, surveyCellCode, surveyedAt } from "./gpu/resources";
import {
  DATUM_SLOT,
  INSTANCE_RECORD_BYTES,
  MAX_SUB_PATCH_LEVELS,
  NO_PARENT_SLOT,
} from "./gpu/uniforms";
import { handoverTauPx, handsOver, limbDistanceM, pixelAngleRad } from "./handover";
import { parentKey, type PatchKey, patchKeyString } from "./patchKey";
import { type BodyFigure, spheroidNormal, surfacePoint } from "./planet";
import {
  isDrawnHalving,
  POP_NORMAL_DEG,
  POP_PX,
  type SeamChainLink,
  type SeamCollapse,
  type SeamEvent,
  type SeamFrame,
  type SeamFrameChange,
  type SeamPop,
  type SeamPopCause,
  type SeamRun,
  type SeamSummary,
  summariseSeamRun,
} from "./seamlessSummary";
import type { Selection, SelectionInput, ViewSelectionInput } from "./select";
import { SELECTION_MARGIN } from "./selectionTolerance";
import { TerrainSession, terrainPassViewOf, type TerrainViewReport } from "./session";
import { decodeOctahedral } from "./shadingNormal";
import {
  effectiveTauPx,
  morphRangeM,
  type TerrainPassView,
  type TerrainViewFrameInput,
} from "./terrainPass";
import { MORPH_IN_MS } from "./timeMorph";
import { answerRequest, type HeightModule, WorkerFields } from "./workers/heightBake";
import type { BakedPatch, HeightWorkerReply, HeightWorkerRequest } from "./workers/messages";
import type { WorkerLike } from "./workers/pool";

/**
 * The seamless probe (the owner's priority, 2026-11-01: "terrain pops in with very distinct
 * chunks"; research-terrain-seamless's probe, landed): TERRAIN TEST's script played through the real
 * terrain session over the scene's fixture, with the real pass, selection, cache, resolver and pool
 * and real WebAssembly bakes, recording every change of the drawn ground and its size on screen
 * (`seamlessSummary.ts`).
 *
 * @remarks
 * Only the bake's duration is modelled: W workers, first in first out, C ms a bake on vitest's fake
 * clock, which fakes `Date` and the timers but not `performance`. The view's hand-over is R07's
 * rule over the field header's relief (`handsOver`, its hysteresis kept), as the drawer's regime
 * plan takes it but without `handoverReliefM`'s coverage condition. The probe ignores the
 * coarse–fine holds (one edge row) in its events. Each frame it also draws the primary view
 * (`TerrainSession.drawTerrain`) and reads its instance records, as the vertex shader does
 * (`patchVertexOf`: the sub-patch's place, the rebake's mix, the holds and the fades' floor on k,
 * the shading normal's mix to the parent's tile), to measure each frame's change of the drawn
 * ground against the frame before's ({@link SeamFrameChange}), from one camera, the frame's.
 *
 * A whole run takes 10 to 25 minutes (pod, 16 vCPU), so `just ci` plays only the first 42 s, to the
 * hand-over and the first draws on high; the whole script on both settings, with the acceptance's
 * bounds, runs only under `HYPERION_SLOW_TESTS=1`. Its knobs are read from the environment
 * (`import.meta.env`), and it writes nothing: each test's figures, and with `SEAM_RECORD=1` its
 * whole record, go to the task's `meta.seamless`, which vitest's JSON reporter saves
 * (`--reporter=json --outputFile.json=<file>`).
 *
 * - `SEAM_SETTING`: `high` or `low`, one setting alone; both by default.
 * - `SEAM_COST_MS` (41: R10.T6.a's one-thread bake) and `SEAM_WORKERS` (3: R05 Design note 11's
 *   count on the target's 16 threads, at which the research measured; `defaultHeightWorkers` gives
 *   8 there since R05.T18).
 * - `SEAM_DRAFT_SHARE` (1): the share of C a drafted bake takes (R10.T6.a's progressive bake), its
 *   refinement the rest; at 1, as before, a draft takes C and its refinement nothing. The pool
 *   reckons a draft at half a whole bake's cost (`HeightWorkerPool`'s `send`).
 * - `SEAM_FROM_S`, `SEAM_TO_S` (the script's span) and `SEAM_HZ` (60).
 * - `SEAM_STRIDE` (2): every how many vertices a change is measured at, and `SEAM_FRAME_STRIDE` (8)
 *   a frame's changed instances.
 * - `SEAM_MORPH_MS`: the time morph's span (`TerrainPassOptions.morphInMs`), `MORPH_IN_MS` unset.
 * - `SEAM_TRACE_FROM_S` and `SEAM_TRACE_TO_S`: each selection between them in `meta.seamless.trace`
 *   (its margin, size, seen patches, nearest ground and levels).
 * - `SEAM_RECORD=1`: the frames, events and latencies too.
 * - `SEAM_LOOKAHEAD=0`: the session's passes look ahead of nothing; by default they look ahead on
 *   the fake clock, as the app's do on its own (`TerrainSessionOptions.lookaheadClockMs`; R10.T14.b).
 */
const SLOW = import.meta.env["HYPERION_SLOW_TESTS"] === "1";

/** The knob `name`'s value, or `undefined` where unset or empty. */
function knob(name: string): string | undefined {
  const value: unknown = import.meta.env[name];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** The knob `name` as a finite number, or `fallback`. */
function numberKnob(name: string, fallback: number): number {
  const text = knob(name);
  const value = text === undefined ? Number.NaN : Number(text);
  return Number.isFinite(value) ? value : fallback;
}

const COST_MS = numberKnob("SEAM_COST_MS", 41);
const WORKERS = numberKnob("SEAM_WORKERS", 3);
const HZ = numberKnob("SEAM_HZ", 60);
const STRIDE = numberKnob("SEAM_STRIDE", 2);
const FRAME_STRIDE = numberKnob("SEAM_FRAME_STRIDE", 8);
const MORPH_MS = numberKnob("SEAM_MORPH_MS", MORPH_IN_MS);
const TRACE_FROM_S = numberKnob("SEAM_TRACE_FROM_S", Number.POSITIVE_INFINITY);
const TRACE_TO_S = numberKnob("SEAM_TRACE_TO_S", Number.NEGATIVE_INFINITY);
const RECORD = knob("SEAM_RECORD") === "1";
const DRAFT_SHARE = Math.min(1, Math.max(0, numberKnob("SEAM_DRAFT_SHARE", 1)));
const LOOKAHEAD = knob("SEAM_LOOKAHEAD") !== "0";

/** The settings the slow run plays: `SEAM_SETTING`'s alone, or both. */
function settingsPlayed(): QualitySetting[] {
  const setting = knob("SEAM_SETTING");
  if (setting === "high" || setting === "low") {
    return [setting];
  }
  return ["high", "low"];
}

/** The quads along a patch's side, and the vertices of its 65 × 65 grid along it. */
const QUADS = 64;
const SIDE = QUADS + 1;

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const MODULE: HeightModule = {
  bakePatch,
  testPlanetVersion,
  createFields: () => new SurfaceFields(),
};

/** What the workers answered over a run: the latest bake by key, which the pass stores as it lands. */
interface Answers {
  readonly bakes: Map<string, BakedPatch>;
  count: number;
}

/**
 * A height worker on the fake clock: first in first out, a bake taking `costMs`, a drafted one
 * {@link DRAFT_SHARE} of it and its refinement the rest, everything else ~0.
 */
class LatencyWorker implements WorkerLike {
  readonly #fields = new WorkerFields(MODULE);
  readonly #events = new EventTarget();
  readonly #wrappers = new Map<unknown, EventListenerObject>();
  readonly #answers: Answers;
  readonly #costMs: number;
  #busyUntilMs = 0;
  #terminated = false;

  constructor(answers: Answers, costMs: number) {
    this.#answers = answers;
    this.#costMs = costMs;
  }

  postMessage(message: HeightWorkerRequest): void {
    if (this.#terminated) {
      return;
    }
    const now = Date.now();
    let cost = 0;
    if (message.kind === "bake") {
      cost = message.draft === true ? DRAFT_SHARE * this.#costMs : this.#costMs;
    } else if (message.kind === "refine-normals") {
      cost = (1 - DRAFT_SHARE) * this.#costMs;
    }
    this.#busyUntilMs = Math.max(now, this.#busyUntilMs) + cost;
    setTimeout(() => {
      if (this.#terminated) {
        return;
      }
      const reply: HeightWorkerReply = answerRequest(MODULE, message, this.#fields).reply;
      if (reply.kind === "baked") {
        this.#answers.count += 1;
        this.#answers.bakes.set(patchKeyString(reply.bake.key), reply.bake);
      }
      this.#events.dispatchEvent(new MessageEvent("message", { data: reply }));
    }, this.#busyUntilMs - now);
  }

  terminate(): void {
    this.#terminated = true;
    this.#fields.free();
  }

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = { handleEvent: listener };
    this.#wrappers.set(listener, wrapper);
    this.#events.addEventListener(type, wrapper);
  }

  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = this.#wrappers.get(listener);
    if (wrapper !== undefined) {
      this.#events.removeEventListener(type, wrapper);
      this.#wrappers.delete(listener);
    }
  }
}

/** A bake the probe holds of a resident patch. */
interface Held {
  readonly key: PatchKey;
  readonly bake: BakedPatch;
}

/** The ground drawn over an area: the bake drawn there, whole or a part of it (a sub-patch). */
interface DrawnArea {
  readonly area: PatchKey;
  readonly held: Held;
  readonly standIn: boolean;
}

/** The primary view as the last selection took it, and its morph bands, level by level. */
interface Camera {
  readonly view: ViewSelectionInput;
  readonly bands: Float64Array;
}

/** A frame's camera as its draw has it, body-fixed: the per-frame layer's (R10.T16.i). */
interface FrameCamera {
  readonly positionM: Vec3;
  readonly orientation: Quaternion;
  readonly fovXRad: number;
  readonly viewport: Viewport;
}

/** The body-fixed camera of a pass view's draw, as the pass's selection takes it. */
function frameCameraOf(passView: TerrainPassView): FrameCamera {
  const { rotation, cameraM, orientation, fovXRad, viewport } = passView.view;
  return {
    positionM: rotateToBodyFixed(rotation, cameraM),
    orientation: multiply(conjugate(quaternionFromRows(rotation.rows)), orientation),
    fovXRad,
    viewport,
  };
}

/** The drawn height of `held` at its vertex (x, y): own and morph mixed by CDLOD's k; 0 off survey. */
function drawnAt(
  figure: BodyFigure,
  cam: Camera,
  held: Held,
  x: number,
  y: number,
): { readonly h: number; readonly k: number } {
  const v = y * SIDE + x;
  const mask = held.bake.layers?.surveyMask;
  if (mask !== undefined && mask[v] !== 255) {
    return { h: 0, k: 0 };
  }
  const h0 = held.bake.heights[2 * v] ?? 0;
  const h1 = held.bake.heights[2 * v + 1] ?? 0;
  const level = held.key.level;
  const start = cam.bands[2 * level] ?? 0;
  const end = cam.bands[2 * level + 1] ?? 0;
  const p = surfacePoint(figure, vertexDir(held.key, x, y), h0);
  const d = norm(sub(vec3(p[0], p[1], p[2]), cam.view.camera.positionM));
  const k = end > start ? Math.min(1, Math.max(0, (d - start) / (end - start))) : 0;
  return { h: (1 - k) * h0 + k * h1, k };
}

/** The height `heightAt` gives a bake's vertices at (u, v) of its quads, on its (0, 0)–(1, 1) diagonal. */
function sampleOn(heightAt: (x: number, y: number) => number, u: number, v: number): number {
  if (Number.isInteger(u) && Number.isInteger(v)) {
    return heightAt(u, v);
  }
  const x = Math.min(QUADS - 1, Math.max(0, Math.floor(u)));
  const y = Math.min(QUADS - 1, Math.max(0, Math.floor(v)));
  const fx = u - x;
  const fy = v - y;
  const h00 = heightAt(x, y);
  const h11 = heightAt(x + 1, y + 1);
  if (fx >= fy) {
    const h10 = heightAt(x + 1, y);
    return h00 + fx * (h10 - h00) + fy * (h11 - h10);
  }
  const h01 = heightAt(x, y + 1);
  return h00 + fy * (h01 - h00) + fx * (h11 - h01);
}

/** `held`'s drawn surface at (u, v) of its quads. */
function sampleDrawn(figure: BodyFigure, cam: Camera, held: Held, u: number, v: number): number {
  return sampleOn((x, y) => drawnAt(figure, cam, held, x, y).h, u, v);
}

/** `held`'s morph target, its surface at k = 1, at (u, v) of its quads; the datum off survey. */
function sampleTarget(held: Held, u: number, v: number): number {
  const mask = held.bake.layers?.surveyMask;
  return sampleOn(
    (x, y) => {
      const at = y * SIDE + x;
      return mask !== undefined && mask[at] !== 255 ? 0 : (held.bake.heights[2 * at + 1] ?? 0);
    },
    u,
    v,
  );
}

/** `held`'s own shading normal at (u, v) of its quads, bilinear over its normal samples. */
function normalAt(held: Held, u: number, v: number): Vec3 | null {
  const n = held.bake.normals;
  const side = Math.round(Math.sqrt(n.length / 2));
  if (side < 2) {
    return null;
  }
  const q = side - 1;
  const fu = Math.min(q, Math.max(0, (u * q) / QUADS));
  const fv = Math.min(q, Math.max(0, (v * q) / QUADS));
  const a = Math.min(q - 1, Math.floor(fu));
  const b = Math.min(q - 1, Math.floor(fv));
  const tu = fu - a;
  const tv = fv - b;
  let acc = vec3(0, 0, 0);
  const add = (aa: number, bb: number, w: number): void => {
    const at = 2 * (bb * side + aa);
    const d = decodeOctahedral(n[at] ?? 0, n[at + 1] ?? 0);
    acc = vec3(acc.x + w * d.x, acc.y + w * d.y, acc.z + w * d.z);
  };
  add(a, b, (1 - tu) * (1 - tv));
  add(a + 1, b, tu * (1 - tv));
  add(a, b + 1, (1 - tu) * tv);
  add(a + 1, b + 1, tu * tv);
  return normalise(acc);
}

/** A change's size: its largest on-screen move, normal turn, and the least k on its finer side. */
interface Delta {
  readonly maxPx: number;
  /** The largest of the finer side's on-screen distance from its own morph target. */
  readonly levelPx: number;
  readonly maxNormalDeg: number;
  readonly minFineK: number;
}

/** The place of vertex (`x`, `y`) of `area` in the grid of `patch`, its ancestor or itself, quads. */
function placeIn(patch: PatchKey, area: PatchKey, x: number, y: number): readonly [number, number] {
  const span = 2 ** (area.level - patch.level);
  return [
    (area.i * QUADS + x) / span - patch.i * QUADS,
    (area.j * QUADS + y) / span - patch.j * QUADS,
  ];
}

/**
 * The change from `coarse`'s drawn surface to `fine`'s over `area`, which each draws over, whole
 * or in part (a sub-patch), measured at `area`'s vertices.
 */
function deltaOf(figure: BodyFigure, cam: Camera, coarse: Held, fine: Held, area: PatchKey): Delta {
  const { widthPx, heightPx } = cam.view.viewport;
  const pxPerRad = widthPx / 2 / Math.tan(cam.view.fovXRad / 2);
  let maxPx = 0;
  let levelPx = 0;
  let maxNormalDeg = 0;
  let minFineK = 1;
  for (let y = 0; y <= QUADS; y += STRIDE) {
    for (let x = 0; x <= QUADS; x += STRIDE) {
      const [uc, vc] = placeIn(coarse.key, area, x, y);
      const [uf, vf] = placeIn(fine.key, area, x, y);
      const fineH = sampleDrawn(figure, cam, fine, uf, vf);
      const fineK = drawnAt(figure, cam, fine, Math.round(uf), Math.round(vf)).k;
      const dh = Math.abs(fineH - sampleDrawn(figure, cam, coarse, uc, vc));
      const fromTarget = Math.abs(fineH - sampleTarget(fine, uf, vf));
      const dir = vertexDir(area, x, y);
      const p = surfacePoint(figure, dir, fineH);
      const rel = sub(vec3(p[0], p[1], p[2]), cam.view.camera.positionM);
      const at = project(
        rel,
        { orientation: cam.view.camera.orientation, fovXRad: cam.view.fovXRad },
        cam.view.viewport,
      );
      if (!at.inFront || at.xPx < 0 || at.xPx > widthPx || at.yPx < 0 || at.yPx > heightPx) {
        continue;
      }
      const up = spheroidNormal(figure, dir);
      const sinIncidence = norm(cross(vec3(up[0], up[1], up[2]), normalise(rel)));
      maxPx = Math.max(maxPx, (dh * sinIncidence * pxPerRad) / norm(rel));
      levelPx = Math.max(levelPx, (fromTarget * sinIncidence * pxPerRad) / norm(rel));
      minFineK = Math.min(minFineK, fineK);
      const before = normalAt(coarse, uc, vc);
      const own = normalAt(fine, uf, vf);
      if (before !== null && own !== null) {
        const after = normalise(
          vec3(
            (1 - fineK) * own.x + fineK * before.x,
            (1 - fineK) * own.y + fineK * before.y,
            (1 - fineK) * own.z + fineK * before.z,
          ),
        );
        const cos = after.x * before.x + after.y * before.y + after.z * before.z;
        maxNormalDeg = Math.max(
          maxNormalDeg,
          (Math.acos(Math.min(1, Math.max(-1, cos))) * 180) / Math.PI,
        );
      }
    }
  }
  return { maxPx, levelPx, maxNormalDeg, minFineK };
}

/** The 32-bit words of an instance record (`terrainPatch.wgsl`'s `Instance`). */
const WORDS = INSTANCE_RECORD_BYTES / 4;

/** The record's words a drawn instance's surface reads beside its origin and its morph bands. */
const SURFACE_WORDS = [3, 6, 7, 8, 9, 10, 11] as const;

/** A frame's instance record, with the bakes its slots held then, as the probe's shader reads it. */
interface DrawnInstance {
  /** The area it draws: its slot's patch, or a sub-patch's part of it. */
  readonly area: PatchKey;
  readonly held: Held;
  /** {@link SURFACE_WORDS}, joined: an instance of the same bake and words draws the same. */
  readonly words: string;
  /** A sub-patch's levels below its slot's patch and its part (i, j) there; 0 for the whole. */
  readonly levels: number;
  readonly partI: number;
  readonly partJ: number;
  readonly startM: number;
  readonly endM: number;
  /** The bake of its parent slot, whose normals its shading mixes to by k; `null` for none. */
  readonly parent: Held | null;
  readonly quadrant: number;
  readonly holds: number;
  readonly edgeFades: number;
  readonly cornerFades: number;
  readonly ownFade: number;
  /** A rebake's fade, 0 to 1, and the older bake it mixes from, `null` for the datum's. */
  readonly rebakeFade: number;
  readonly older: Held | null;
  /** Whether it rises out of the datum (its older slot the datum's, fading). */
  readonly rises: boolean;
}

/** The primary view's drawn instances in `engine`'s latest record writes, by area, read by slot. */
function instancesOf(
  engine: CountingRenderEngine,
  slots: ReadonlyMap<number, Held>,
): Map<string, DrawnInstance> {
  const drawn = new Map<string, DrawnInstance>();
  for (const name of [
    `terrain instances (${PRIMARY_VIEW_ID})`,
    `terrain interior instances (${PRIMARY_VIEW_ID})`,
  ]) {
    const data = engine.writes.findLast((w) => w.buffer === name)?.data;
    if (data === undefined) {
      continue;
    }
    const u = new Uint32Array(data.buffer, data.byteOffset, data.byteLength / 4);
    const f = new Float32Array(data.buffer, data.byteOffset, data.byteLength / 4);
    for (let base = 0; base + WORDS <= u.length; base += WORDS) {
      const word = (n: number): number => u[base + n] ?? 0;
      const held = slots.get(word(3));
      if (held === undefined) {
        continue;
      }
      const part = word(8);
      const levels = part & 31;
      const partI = (part >>> 5) & 0x1fff;
      const partJ = (part >>> 18) & 0x1fff;
      const s = 2 ** levels;
      const { face, level, i, j } = held.key;
      const area: PatchKey = { face, level: level + levels, i: i * s + partI, j: j * s + partJ };
      const parentSlot = word(6);
      const fades = word(11);
      const olderSlot = fades >>> 16;
      const rebakeFade = ((fades >>> 8) & 0xff) / FADE_STEPS;
      drawn.set(patchKeyString(area), {
        area,
        held,
        words: SURFACE_WORDS.map(word).join(),
        levels,
        partI,
        partJ,
        startM: f[base + 4] ?? 0,
        endM: f[base + 5] ?? 0,
        parent: parentSlot === NO_PARENT_SLOT ? null : (slots.get(parentSlot) ?? null),
        quadrant: word(7) & 3,
        holds: word(7) >>> 2,
        edgeFades: word(9),
        cornerFades: word(10),
        ownFade: fades & 0xff,
        rebakeFade,
        older: rebakeFade > 0 && olderSlot !== DATUM_SLOT ? (slots.get(olderSlot) ?? null) : null,
        rises: olderSlot === DATUM_SLOT && rebakeFade > 0,
      });
    }
  }
  return drawn;
}

/** The survey masks a run's bakes give the lit mesh (`packSurveyMask`), each made once. */
class SurveyMasks {
  readonly #packed = new WeakMap<BakedPatch, Uint8Array | null>();

  /** Whether (u, v) of `held`'s quads lies on surveyed ground, as the lit mesh has it (`surveyedAt`). */
  surveyed(held: Held, u: number, v: number): boolean {
    let packed = this.#packed.get(held.bake);
    if (packed === undefined) {
      const layers = held.bake.layers;
      packed =
        layers === null
          ? null
          : packSurveyMask(layers.surveyMask, surveyCellCode(held.key.level, layers.fieldLevel));
      this.#packed.set(held.bake, packed);
    }
    return packed === null || surveyedAt(packed, u, v);
  }
}

/** `held`'s own height (`which` 0) or morph target (1) at its vertex (x, y). */
function heightOf(held: Held, which: 0 | 1): (x: number, y: number) => number {
  return (x, y) => held.bake.heights[2 * (y * SIDE + x) + which] ?? 0;
}

/** `a` mixed to `b` by `t`, normalised. */
function mixed(a: Vec3, b: Vec3, t: number): Vec3 {
  return normalise(vec3(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y), a.z + t * (b.z - a.z)));
}

/**
 * The shading normal `terrainPatch.wgsl`'s `shadingNormalAt` gives `held`'s tile at (u, v) of its
 * quads, mixed to `inst`'s parent's tile by k.
 */
function morphedNormal(
  held: Held,
  inst: DrawnInstance,
  u: number,
  v: number,
  k: number,
): Vec3 | null {
  const own = normalAt(held, u, v);
  if (own === null || inst.parent === null || !(k > 0)) {
    return own;
  }
  const coarse = normalAt(
    inst.parent,
    ((inst.quadrant & 1) * QUADS + u) / 2,
    (((inst.quadrant >> 1) & 1) * QUADS + v) / 2,
  );
  return coarse === null ? own : mixed(own, coarse, Math.min(k, 1));
}

/**
 * The drawn ground at a place: its height above the datum, its shading normal (`null` with no
 * normals), and whether the survey holds it, or the reference surface stands there instead.
 */
interface DrawnPlace {
  readonly h: number;
  readonly normal: Vec3 | null;
  readonly surveyed: boolean;
}

/** The drawn ground of a frame's instances seen from one camera: each vertex made once. */
class DrawnSurfaces {
  readonly #figure: BodyFigure;
  readonly #cam: FrameCamera;
  readonly #masks: SurveyMasks;
  readonly #made = new Map<DrawnInstance, Float64Array>();

  constructor(figure: BodyFigure, cam: FrameCamera, masks: SurveyMasks) {
    this.#figure = figure;
    this.#cam = cam;
    this.#masks = masks;
  }

  /**
   * The drawn height and morph factor of `inst`'s grid vertex (x, y), as `patchVertexOf` makes
   * them: its slot's heights at its place there, mixed from the older bake's by the rebake's fade,
   * and k, CDLOD's from the vertex's distance, held and floored at the fades (`fadedMorph`).
   */
  vertex(inst: DrawnInstance, x: number, y: number): readonly [number, number] {
    let made = this.#made.get(inst);
    if (made === undefined) {
      made = new Float64Array(2 * SIDE * SIDE).fill(Number.NaN);
      this.#made.set(inst, made);
    }
    const at = 2 * (y * SIDE + x);
    if (Number.isNaN(made[at])) {
      const s = 2 ** inst.levels;
      const u = (inst.partI * QUADS + x) / s;
      const v = (inst.partJ * QUADS + y) / s;
      const surveyed = this.#masks.surveyed(inst.held, u, v);
      let own = surveyed ? sampleOn(heightOf(inst.held, 0), u, v) : 0;
      let morph = surveyed ? sampleOn(heightOf(inst.held, 1), u, v) : 0;
      if (inst.rebakeFade > 0) {
        const older = inst.older;
        const drawn = older !== null && this.#masks.surveyed(older, u, v);
        own += inst.rebakeFade * ((drawn ? sampleOn(heightOf(older, 0), u, v) : 0) - own);
        morph += inst.rebakeFade * ((drawn ? sampleOn(heightOf(older, 1), u, v) : 0) - morph);
      }
      const p = surfacePoint(this.#figure, vertexDir(inst.area, x, y), own);
      const d = norm(sub(vec3(p[0], p[1], p[2]), this.#cam.positionM));
      const span = inst.endM - inst.startM;
      const cdlod = span > 0 ? Math.min(1, Math.max(0, (d - inst.startM) / span)) : 0;
      const k = fadedMorph(inst.holds, inst.edgeFades, inst.cornerFades, inst.ownFade, x, y, cdlod);
      made[at] = own + k * (morph - own);
      made[at + 1] = k;
    }
    return [made[at] ?? 0, made[at + 1] ?? 0];
  }

  /** `inst`'s morph factor at (u, v) of its area's quads, on the mesh's triangle there. */
  kAt(inst: DrawnInstance, u: number, v: number): number {
    return sampleOn((x, y) => this.vertex(inst, x, y)[1], u, v);
  }

  /**
   * `inst`'s drawn surface at (u, v) of its area's quads: its height on the mesh's triangle there,
   * and the shading normal at its k, `terrainLitShade.wgsl`'s, the datum's `datum` off survey.
   */
  at(inst: DrawnInstance, u: number, v: number, datum: Vec3): DrawnPlace {
    const h = sampleOn((x, y) => this.vertex(inst, x, y)[0], u, v);
    const k = this.kAt(inst, u, v);
    const s = 2 ** inst.levels;
    const su = (inst.partI * QUADS + u) / s;
    const sv = (inst.partJ * QUADS + v) / s;
    if (!this.#masks.surveyed(inst.held, su, sv)) {
      return { h, normal: datum, surveyed: false };
    }
    let normal = morphedNormal(inst.held, inst, su, sv, k);
    if (normal !== null && inst.rebakeFade > 0) {
      const older =
        inst.older !== null && this.#masks.surveyed(inst.older, su, sv)
          ? morphedNormal(inst.older, inst, su, sv, k)
          : datum;
      normal = older === null ? normal : mixed(normal, older, inst.rebakeFade);
    }
    return { h, normal, surveyed: true };
  }
}

/** The keys of every strict ancestor of `drawn`'s areas, as `patchKeyString`s. */
function ancestorsOf(drawn: ReadonlyMap<string, DrawnInstance>): Set<string> {
  const ancestors = new Set<string>();
  for (const inst of drawn.values()) {
    for (let k = parentKey(inst.area); k !== null; k = parentKey(k)) {
      const keyString = patchKeyString(k);
      if (ancestors.has(keyString)) {
        break;
      }
      ancestors.add(keyString);
    }
  }
  return ancestors;
}

/**
 * The instance of `drawn` over vertex (x, y) of `area`, and the place there in its own area's
 * quads: `area`'s own, an ancestor's or a descendant's (the draw set never overlaps); `null` where
 * none was. `ancestors` is {@link ancestorsOf}'s of `drawn`.
 */
function coverOf(
  drawn: ReadonlyMap<string, DrawnInstance>,
  ancestors: ReadonlySet<string>,
  area: PatchKey,
  x: number,
  y: number,
): { readonly inst: DrawnInstance; readonly u: number; readonly v: number } | null {
  const same = drawn.get(patchKeyString(area));
  if (same !== undefined) {
    return { inst: same, u: x, v: y };
  }
  for (let k = parentKey(area); k !== null; k = parentKey(k)) {
    const coarse = drawn.get(patchKeyString(k));
    if (coarse !== undefined) {
      const [u, v] = placeIn(k, area, x, y);
      return { inst: coarse, u, v };
    }
  }
  let key = area;
  let gx = area.i * QUADS + x;
  let gy = area.j * QUADS + y;
  while (ancestors.has(patchKeyString(key)) && key.level < area.level + MAX_SUB_PATCH_LEVELS) {
    gx *= 2;
    gy *= 2;
    const span = 2 ** (key.level + 1 - area.level);
    // A vertex on the area's far edge is its last descendant's.
    const i = Math.min(Math.floor(gx / QUADS), (area.i + 1) * span - 1);
    const j = Math.min(Math.floor(gy / QUADS), (area.j + 1) * span - 1);
    key = { face: area.face, level: key.level + 1, i, j };
    const fine = drawn.get(patchKeyString(key));
    if (fine !== undefined) {
      return { inst: fine, u: gx - i * QUADS, v: gy - j * QUADS };
    }
  }
  return null;
}

/**
 * Whether ground `rel` from `cam`'s camera, whose datum's normal is `up`, falls in its viewport, in
 * front of it, and faces it: the camera above its tangent plane, as no ground past the horizon is.
 */
function onScreen(cam: FrameCamera, rel: Vec3, up: Vec3): boolean {
  if (rel.x * up.x + rel.y * up.y + rel.z * up.z >= 0) {
    return false;
  }
  const { widthPx, heightPx } = cam.viewport;
  const at = project(rel, { orientation: cam.orientation, fovXRad: cam.fovXRad }, cam.viewport);
  return at.inFront && at.xPx >= 0 && at.xPx <= widthPx && at.yPx >= 0 && at.yPx <= heightPx;
}

/** The grid lines a frame's changed instance is measured at: every {@link FRAME_STRIDE}th, and the last. */
function frameLines(): number[] {
  const lines: number[] = [];
  for (let n = 0; n < QUADS; n += Math.max(1, FRAME_STRIDE)) {
    lines.push(n);
  }
  lines.push(QUADS);
  return lines;
}

/**
 * The change of the drawn ground from frame `before`'s instances to `now`'s at `tS`, both drawn
 * from camera `cam` (the frame's), over the instances of `now` whose bake or words changed: at
 * each of their measured vertices on screen, the drawn surface's move along the datum's normal and
 * its shading normal's turn against the instance over that ground before, or the datum's where
 * nothing was and camera `camBefore` saw it; `null` where nothing changed. `masks` is the run's;
 * each instance whose change shows goes in `pops`.
 */
function frameChangeOf(
  figure: BodyFigure,
  masks: SurveyMasks,
  cam: FrameCamera,
  camBefore: FrameCamera,
  before: ReadonlyMap<string, DrawnInstance>,
  now: ReadonlyMap<string, DrawnInstance>,
  tS: number,
  pops: SeamPop[],
): SeamFrameChange | null {
  const surfaces = new DrawnSurfaces(figure, cam, masks);
  const ancestors = ancestorsOf(before);
  const lines = frameLines();
  const pxPerRad = cam.viewport.widthPx / 2 / Math.tan(cam.fovXRad / 2);
  let measured = 0;
  let movePx = 0;
  let normalDeg = 0;
  let popCount = 0;
  let appears = 0;
  let appearMovePx = 0;
  for (const [areaString, inst] of now) {
    const was = before.get(areaString);
    if (was !== undefined && was.held === inst.held && was.words === inst.words) {
      continue;
    }
    measured += 1;
    let instPx = 0;
    let instDeg = 0;
    let covered = false;
    let bare = false;
    let worst: Omit<SeamPop, "tS" | "areaLevel" | "level" | "ownFade" | "rebakeFade"> | null = null;
    let worstBadness = 0;
    for (const y of lines) {
      for (const x of lines) {
        const dir = vertexDir(inst.area, x, y);
        const n = spheroidNormal(figure, dir);
        const datum = vec3(n[0], n[1], n[2]);
        const drawn = surfaces.at(inst, x, y, datum);
        const p = surfacePoint(figure, dir, drawn.h);
        const rel = sub(vec3(p[0], p[1], p[2]), cam.positionM);
        if (!onScreen(cam, rel, datum)) {
          continue;
        }
        const cover = coverOf(before, ancestors, inst.area, x, y);
        let then: DrawnPlace;
        if (cover !== null) {
          covered = true;
          then = surfaces.at(cover.inst, cover.u, cover.v, datum);
        } else {
          // Nothing drew this ground the frame before: the reference surface did, where seen.
          const q = surfacePoint(figure, dir, 0);
          const back = sub(vec3(q[0], q[1], q[2]), camBefore.positionM);
          if (!onScreen(camBefore, back, datum)) {
            continue;
          }
          bare = true;
          then = { h: 0, normal: datum, surveyed: false };
        }
        const sinIncidence = norm(cross(datum, normalise(rel)));
        const px = (Math.abs(drawn.h - then.h) * sinIncidence * pxPerRad) / norm(rel);
        let deg = 0;
        if (drawn.normal !== null && then.normal !== null) {
          const a = drawn.normal;
          const b = then.normal;
          const cos = Math.min(1, Math.max(-1, a.x * b.x + a.y * b.y + a.z * b.z));
          deg = (Math.acos(cos) * 180) / Math.PI;
        }
        instPx = Math.max(instPx, px);
        instDeg = Math.max(instDeg, deg);
        const badness = Math.max(px / POP_PX, deg / POP_NORMAL_DEG);
        if (badness > worstBadness) {
          worstBadness = badness;
          const k = surfaces.vertex(inst, x, y)[1];
          worst =
            cover === null
              ? { cause: "appear", levelBefore: -1, movePx: px, normalDeg: deg, k, kBefore: 0 }
              : {
                  cause: drawn.surveyed === then.surveyed ? causeOf(inst, cover.inst) : "survey",
                  levelBefore: cover.inst.held.key.level,
                  movePx: px,
                  normalDeg: deg,
                  k,
                  kBefore: surfaces.kAt(cover.inst, cover.u, cover.v),
                };
        }
      }
    }
    movePx = Math.max(movePx, instPx);
    normalDeg = Math.max(normalDeg, instDeg);
    if (worst !== null && (instPx > POP_PX || instDeg > POP_NORMAL_DEG)) {
      popCount += 1;
      pops.push({
        tS,
        areaLevel: inst.area.level,
        level: inst.held.key.level,
        ownFade: inst.ownFade / FADE_STEPS,
        rebakeFade: inst.rebakeFade,
        ...worst,
        movePx: instPx,
        normalDeg: instDeg,
      });
    }
    if (bare && !covered) {
      appears += inst.rises ? 0 : 1;
      appearMovePx = Math.max(appearMovePx, instPx);
    }
  }
  return measured === 0 ? null : { tS, movePx, normalDeg, pops: popCount, appears, appearMovePx };
}

/** What `inst`'s change from `was`, the instance over its ground the frame before, came from. */
function causeOf(inst: DrawnInstance, was: DrawnInstance): SeamPopCause {
  const level = inst.held.key.level;
  const before = was.held.key.level;
  if (level !== before) {
    return level > before ? "refine" : "coarsen";
  }
  return was.held === inst.held ? "fade" : "rebake";
}

/** The primary view's frame of `passView` drawn at `tS`: its view, a sun overhead, no lights. */
function frameInputOf(passView: TerrainPassView, tS: number): TerrainViewFrameInput {
  return {
    view: passView.view,
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    lighting: {
      lights: [],
      occluders: [],
      secondaries: [],
      elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      exposureScale: 1e-5,
    },
    nowMs: 1000 * tS,
  };
}

function isAncestor(a: PatchKey, b: PatchKey): boolean {
  if (a.face !== b.face || a.level >= b.level) {
    return false;
  }
  const s = 2 ** (b.level - a.level);
  return Math.floor(b.i / s) === a.i && Math.floor(b.j / s) === a.j;
}

function ancestorAt(key: PatchKey, level: number): PatchKey {
  const s = 2 ** (key.level - level);
  return { face: key.face, level, i: Math.floor(key.i / s), j: Math.floor(key.j / s) };
}

/** One selection in the trace's span. */
interface TraceEntry {
  readonly tS: number;
  readonly marginRad: number | null;
  readonly selected: number;
  readonly seen: number;
  /** The nearest selected patch's box seen, not at the finest level, metres (the move rule's d). */
  readonly nearestSelectedM: number;
  readonly nearestSurveyM: number | null;
  readonly limitExcess: number;
  readonly levels: Readonly<Record<number, number>>;
}

/** A run's span and its knobs. */
interface ProbeOptions {
  readonly setting: QualitySetting;
  readonly fromS: number;
  readonly toS: number;
}

/** What a run gives: the record, and the trace of its selections in the trace's span. */
interface Played {
  readonly run: SeamRun;
  readonly trace: ReadonlyArray<TraceEntry>;
}

/** Plays TERRAIN TEST's script from `fromS` to `toS` at {@link HZ} and records the drawn ground. */
async function play(options: ProbeOptions): Promise<Played> {
  const { setting, fromS, toS } = options;
  initSync({ module: wasmBytes() });
  vi.useFakeTimers({
    toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"],
  });
  const engine = await countingRenderEngine();
  const fixture = terrainSceneFixture();
  const link = fixture.open();
  const scene = terrainScene();
  const answers: Answers = { bakes: new Map(), count: 0 };
  const session = new TerrainSession({
    engine,
    universe: TERRAIN_TEST_UNIVERSE,
    requests: link.requests,
    linkUp: true,
    createWorker: () => new LatencyWorker(answers, COST_MS),
    workers: WORKERS,
    setting,
    ...(LOOKAHEAD ? { lookaheadClockMs: () => Date.now() } : {}),
    morphInMs: MORPH_MS,
  });
  link.onSurface((event) => {
    if (event.kind === "revisions") {
      session.noteSurfaceRevisions(event.revisions);
    }
    if (event.kind === "resync") {
      session.resync();
    }
  });
  const body = terrainTestGolden().body;
  const widthPx = setting === "high" ? 1920 : 1280;
  const heightPx = setting === "high" ? 1080 : 720;
  const fovXRad = Math.PI / 3;
  let handed = false;
  const reportAt = (tS: number): TerrainViewReport => {
    const sc = scene.sceneAt(tS);
    const pose = scene.cameraAt(tS);
    const regimes = new Map<typeof body, LitRegime>();
    const relief = session.survey(body)?.header?.realised_relief_m ?? null;
    const moon = sc.bodies.find((b) => b.id === body);
    if (moon !== undefined) {
      const centreM = relativeToCamera(
        { kind: "body", body, m: vec3(0, 0, 0) },
        pose,
        cameraSceneOf(sc).origins,
      );
      const radPerPx = pixelAngleRad(
        { orientation: pose.orientation, fovXRad },
        { widthPx, heightPx },
      );
      handed = handsOver(
        limbDistanceM(norm(centreM), moon.radiusM),
        relief,
        handoverTauPx("photorealistic", setting),
        radPerPx,
        handed,
      );
      if (handed) {
        regimes.set(body, "terrain");
      }
    }
    return {
      scene: sc,
      pose,
      fovXRad,
      presentedPx: { widthPx, heightPx },
      style: "photorealistic",
      drawnStyle: "photorealistic",
      setting,
      streamPriority: "primary",
      regimes,
    };
  };

  const frames: SeamFrame[] = [];
  const events: SeamEvent[] = [];
  const collapses: SeamCollapse[] = [];
  const trace: TraceEntry[] = [];
  const latenciesS: number[] = [];
  const resident = new Map<string, Held>();
  // The bake each slot holds, an older bake's kept in its retired slot until the slot is reused.
  const slots = new Map<number, Held>();
  const frameChanges: SeamFrameChange[] = [];
  const pops: SeamPop[] = [];
  const masks = new SurveyMasks();
  let drawnBefore = new Map<string, DrawnInstance>();
  // The camera the frame before drew from.
  let drawnCamera: FrameCamera | null = null;
  const firstDemandS = new Map<string, number>();
  const rebaked: Array<{ readonly old: Held; readonly now: Held }> = [];
  let lastDrawn = new Map<string, DrawnArea>();
  let selectedResident = new Set<string>();
  // The primary view at the last selection: held in an object, since the observer sets it.
  const latest: { cam: Camera | null } = { cam: null };
  let observed = false;
  let nowS = 0;
  let handOverS: number | null = null;
  let firstDrawS: number | null = null;
  let refusedBakes = 0;
  let selectedEvicted = 0;

  const watch = (): void => {
    const pass = session.pass(body);
    if (pass === null || observed) {
      return;
    }
    observed = true;
    handOverS = nowS;
    pass.observe({
      onSelect: (input: SelectionInput, selection: Selection) => {
        const view = input.views[0];
        if (view === undefined) {
          return;
        }
        // The pass's bands: at the view's τ, raised where the budget binds.
        const morphView = {
          ...view,
          tauPx: effectiveTauPx(
            view.tauPx * (1 + SELECTION_MARGIN),
            selection.limitExcess,
            view.weight,
          ),
        };
        const bands = new Float64Array(2 * 32);
        for (let level = 0; level < 31; level += 1) {
          const [start, end] = morphRangeM(pass.planet, level, morphView);
          bands[2 * level] = start;
          bands[2 * level + 1] = end;
        }
        latest.cam = { view, bands };
        for (const request of selection.demand) {
          const keyString = patchKeyString(request.key);
          if (!firstDemandS.has(keyString)) {
            firstDemandS.set(keyString, nowS);
          }
        }
        if (nowS >= TRACE_FROM_S && nowS <= TRACE_TO_S) {
          let seen = 0;
          let nearestSelectedM = Number.POSITIVE_INFINITY;
          const levels: Record<number, number> = {};
          for (const patch of selection.patches.values()) {
            seen += patch.seen ? 1 : 0;
            levels[patch.key.level] = (levels[patch.key.level] ?? 0) + 1;
            if (patch.key.level < pass.planet.finestLevel && (patch.views & 1) !== 0) {
              nearestSelectedM = Math.min(
                nearestSelectedM,
                distanceToBoxM(relativeBounds(patch.bounds, view.camera.positionM)),
              );
            }
          }
          trace.push({
            tS: nowS,
            marginRad: view.frustumMarginRad ?? null,
            selected: selection.patches.size,
            seen,
            nearestSelectedM,
            nearestSurveyM: selection.nearestSurveyByViewM[0] ?? null,
            limitExcess: selection.limitExcess,
            levels,
          });
        }
      },
      onResident: (key: PatchKey) => {
        const keyString = patchKeyString(key);
        const bake = answers.bakes.get(keyString);
        if (bake === undefined) {
          return;
        }
        const held: Held = { key, bake };
        const old = resident.get(keyString);
        if (old !== undefined) {
          rebaked.push({ old, now: held });
        }
        resident.set(keyString, held);
        const slot = pass.resident(keyString)?.slot;
        if (slot !== undefined) {
          slots.set(slot, held);
        }
        const askedS = firstDemandS.get(keyString);
        if (askedS !== undefined) {
          latenciesS.push(nowS - askedS);
          firstDemandS.delete(keyString);
        }
      },
      onRefused: () => {
        refusedBakes += 1;
      },
    });
  };

  const stepS = 1 / HZ;
  try {
    fixture.notePlayed(0);
    for (let n = Math.round(fromS * HZ); n <= Math.round(toS * HZ); n += 1) {
      const tS = n * stepS;
      nowS = tS;
      fixture.notePlayed(tS);
      const viewReport = reportAt(tS);
      session.noteView(PRIMARY_VIEW_ID, viewReport);
      rebaked.length = 0;
      session.step();
      watch();
      // The step's draw in the primary view, as the app draws it, and its change from the frame
      // before's; the engine's records cleared first, as a run's would fill the memory.
      engine.writes.length = 0;
      engine.textureWritten.length = 0;
      engine.dispatched.length = 0;
      engine.targetFrames.length = 0;
      engine.depthLayerRenders.length = 0;
      const seenBody = viewReport.scene.bodies.find((b) => b.id === body);
      const passView =
        seenBody === undefined ? null : terrainPassViewOf(PRIMARY_VIEW_ID, viewReport, seenBody);
      const drawnNow =
        passView !== null &&
        session.drawTerrain(PRIMARY_VIEW_ID, body, frameInputOf(passView, tS)) !== null
          ? instancesOf(engine, slots)
          : new Map<string, DrawnInstance>();
      const drawingPass = session.pass(body);
      const camBefore = drawnCamera;
      const camNow = passView === null ? null : frameCameraOf(passView);
      drawnCamera = camNow;
      if (drawingPass !== null && camNow !== null && camBefore !== null && drawnBefore.size > 0) {
        const change = frameChangeOf(
          drawingPass.planet.figure,
          masks,
          camNow,
          camBefore,
          drawnBefore,
          drawnNow,
          tS,
          pops,
        );
        if (change !== null) {
          frameChanges.push(change);
        }
      }
      drawnBefore = drawnNow;
      // Each frame waits on the clock its step's bakes run on, before the next frame steps.
      // oxlint-disable-next-line no-await-in-loop
      await vi.advanceTimersByTimeAsync(1000 * stepS);
      const pass = session.pass(body);
      const last = pass?.lastStep ?? null;
      const c = latest.cam;
      if (pass === null || last === null || c === null) {
        continue;
      }
      const figure = pass.planet.figure;
      const ds = last.drawSet;
      if (firstDrawS === null && ds.count > 0) {
        firstDrawS = tS;
      }
      let maxGap = 0;
      const nowSelectedResident = new Set<string>();
      for (const [keyString, sp] of last.selection.patches) {
        const isResident = pass.resident(keyString) !== undefined;
        if (isResident) {
          nowSelectedResident.add(keyString);
        } else if (selectedResident.has(keyString)) {
          selectedEvicted += 1;
        }
        if (!sp.seen) {
          continue;
        }
        for (let level = sp.key.level; level >= 0; level -= 1) {
          const at = level === sp.key.level ? keyString : patchKeyString(ancestorAt(sp.key, level));
          if (pass.resident(at) !== undefined) {
            maxGap = Math.max(maxGap, sp.key.level - level);
            break;
          }
        }
      }
      selectedResident = nowSelectedResident;
      const frame: SeamFrame = {
        tS,
        drawn: ds.count,
        standingIn: ds.standingIn,
        missing: ds.missing,
        maxGap,
        limitExcess: last.selection.limitExcess,
        selected: last.selection.patches.size,
      };
      const before = frames.at(-1);
      frames.push(frame);
      if (before !== undefined && isDrawnHalving(before, frame)) {
        // The missing patches farthest from a resident ancestor first: one deep miss is what
        // stands an ancestor in over everything beneath it.
        const missing: Array<SeamCollapse["missing"][number]> = [];
        for (const [keyString, sp] of last.selection.patches) {
          if (!sp.seen || pass.resident(keyString) !== undefined) {
            continue;
          }
          const chain: SeamChainLink[] = [];
          let gap = sp.key.level;
          for (let k = parentKey(sp.key); k !== null; k = parentKey(k)) {
            const ks = patchKeyString(k);
            const isResident = pass.resident(ks) !== undefined;
            if (isResident && gap === sp.key.level) {
              gap = sp.key.level - k.level;
            }
            if (chain.length < 8) {
              chain.push({ level: k.level, resident: isResident, drawnBefore: lastDrawn.has(ks) });
            }
          }
          missing.push({ key: keyString, drawnBefore: lastDrawn.has(keyString), chain, gap });
        }
        missing.sort((a, b) => b.gap - a.gap || (a.key < b.key ? -1 : 1));
        collapses.push({
          tS,
          before: before.drawn,
          after: frame.drawn,
          drawnKeys: ds.patches.map((p) => p.keyString),
          missing: missing.slice(0, 6),
        });
      }
      const nowDrawn = new Map<string, DrawnArea>();
      for (const dp of ds.patches) {
        const held = resident.get(dp.keyString);
        if (held !== undefined) {
          const area = dp.within ?? dp.patch.key;
          nowDrawn.set(patchKeyString(area), { area, held, standIn: dp.standIn });
        }
      }
      // The change from `from`'s surface to `to`'s over `area`, by which is finer; none between
      // a bake and itself.
      const change = (from: Held, to: Held, area: PatchKey, standIn: boolean): boolean => {
        const jump = to.key.level - from.key.level;
        if (jump === 0) {
          return false;
        }
        const [coarse, fine] = jump > 0 ? [from, to] : [to, from];
        events.push({
          tS,
          kind: jump > 0 ? "refine" : "coarsen",
          fineLevel: fine.key.level,
          levelJump: Math.abs(jump),
          ...deltaOf(figure, c, coarse, fine, area),
          standIn,
        });
        return true;
      };
      // Rebakes of a key drawn before and after over one area: its data changed in place.
      const rebakes = new Map(rebaked.map((r) => [patchKeyString(r.now.key), r]));
      for (const [areaString, drawn] of nowDrawn) {
        const r = rebakes.get(patchKeyString(drawn.held.key));
        if (r !== undefined && lastDrawn.get(areaString)?.held === r.old) {
          events.push({
            tS,
            kind: "rebake",
            fineLevel: drawn.held.key.level,
            levelJump: 0,
            ...deltaOf(figure, c, r.old, r.now, drawn.area),
            standIn: drawn.standIn,
          });
        }
      }
      for (const [areaString, drawn] of nowDrawn) {
        const { area, held, standIn } = drawn;
        const was = lastDrawn.get(areaString);
        if (was !== undefined) {
          // The same area, drawn by another bake now: a stand-in replaced, or one standing in.
          if (patchKeyString(was.held.key) !== patchKeyString(held.key)) {
            change(was.held, held, area, standIn);
          }
          continue;
        }
        let found = false;
        for (let k = parentKey(area); k !== null && !found; k = parentKey(k)) {
          const coarse = lastDrawn.get(patchKeyString(k));
          if (coarse !== undefined) {
            change(coarse.held, held, area, standIn);
            found = true;
          }
        }
        if (found) {
          continue;
        }
        for (const [fineString, fine] of lastDrawn) {
          if (!nowDrawn.has(fineString) && isAncestor(area, fine.area)) {
            change(fine.held, held, fine.area, standIn);
            found = true;
          }
        }
        if (!found && lastDrawn.size > 0) {
          events.push({
            tS,
            kind: "appear",
            fineLevel: held.key.level,
            levelJump: 0,
            maxPx: -1,
            levelPx: -1,
            maxNormalDeg: -1,
            minFineK: 0,
            standIn,
          });
        }
      }
      lastDrawn = nowDrawn;
    }
  } finally {
    session.dispose();
    link.close();
    vi.useRealTimers();
  }
  return {
    run: {
      costMs: COST_MS,
      workers: WORKERS,
      setting,
      fromS,
      toS,
      handOverS,
      firstDrawS,
      bakesAnswered: answers.count,
      refusedBakes,
      selectedEvicted,
      latenciesS,
      frames,
      events,
      collapses,
      frameChanges,
      pops,
    },
    trace,
  };
}

/** The pixels and degrees {@link popBinsOf} bins pops by, each bin's lower end. */
const POP_BINS_PX = [1, 1.5, 2, 4, 8, 16] as const;
const POP_BINS_DEG = [2, 4, 8, 16] as const;

/** The bin of `bins` (their lower ends, rising) that `value` falls in, or −1 below them all. */
function binOf(value: number, bins: ReadonlyArray<number>): number {
  return bins.findLastIndex((low) => value > low);
}

/** `pops` by cause, counted by move and by turn in {@link POP_BINS_PX} and {@link POP_BINS_DEG}. */
function popBinsOf(pops: ReadonlyArray<SeamPop>): Record<string, unknown> {
  const out: Record<string, { px: number[]; deg: number[] }> = {};
  for (const pop of pops) {
    const bins = (out[pop.cause] ??= {
      px: POP_BINS_PX.map(() => 0),
      deg: POP_BINS_DEG.map(() => 0),
    });
    const px = binOf(pop.movePx, POP_BINS_PX);
    const deg = binOf(pop.normalDeg, POP_BINS_DEG);
    if (px >= 0) {
      bins.px[px] = (bins.px[px] ?? 0) + 1;
    }
    if (deg >= 0) {
      bins.deg[deg] = (bins.deg[deg] ?? 0) + 1;
    }
  }
  return { px: POP_BINS_PX, deg: POP_BINS_DEG, byCause: out };
}

/** What a test leaves in its task's `meta.seamless`, for vitest's JSON reporter. */
function report(played: Played, summary: SeamSummary): Record<string, unknown> {
  const { run } = played;
  return {
    setting: run.setting,
    costMs: run.costMs,
    workers: run.workers,
    lookahead: LOOKAHEAD,
    draftShare: DRAFT_SHARE,
    fromS: run.fromS,
    toS: run.toS,
    handOverS: run.handOverS,
    firstDrawS: run.firstDrawS,
    bakesAnswered: run.bakesAnswered,
    summary,
    collapses: run.collapses,
    popBins: popBinsOf(run.pops),
    // The largest pops of a frame's change, by move and by turn.
    popsByMove: run.pops.toSorted((a, b) => b.movePx - a.movePx).slice(0, 60),
    popsByTurn: run.pops.toSorted((a, b) => b.normalDeg - a.normalDeg).slice(0, 60),
    trace: played.trace,
    ...(RECORD ? { record: run } : {}),
  };
}

/** The script's first draws on high: past the hand-over at 40.55 s. */
const SHORT_TO_S = 42;

describe("the seamless probe over TERRAIN TEST", () => {
  it(
    "plays to the hand-over and draws the ground, whole",
    { timeout: 120_000 },
    async ({ task }) => {
      const played = await play({ setting: "high", fromS: 0, toS: SHORT_TO_S });
      const summary = summariseSeamRun(played.run);
      Object.assign(task.meta, { seamless: report(played, summary) });
      expect(played.run.handOverS).not.toBeNull();
      expect(played.run.firstDrawS).not.toBeNull();
      expect(summary).toMatchObject({
        holeFrames: 0,
        collapses: 0,
        flickers: 0,
        selectedEvicted: 0,
      });
    },
  );
});

describe.runIf(SLOW)("the seamless probe's acceptance (HYPERION_SLOW_TESTS=1)", () => {
  it.for(settingsPlayed())(
    "draws no collapse, jump or eviction of the selection on %s",
    { timeout: 2 * 60 * 60 * 1000 },
    async (setting, { task }) => {
      const played = await play({
        setting,
        fromS: numberKnob("SEAM_FROM_S", 0),
        toS: numberKnob("SEAM_TO_S", TERRAIN_TEST_SCRIPT.durationS),
      });
      const summary = summariseSeamRun(played.run);
      Object.assign(task.meta, { seamless: report(played, summary) });
      expect({
        holes: summary.holeFrames,
        collapses: summary.collapses,
        flickers: summary.flickers,
        jumps: summary.jumps,
        selectedEvicted: summary.selectedEvicted,
      }).toEqual({ holes: 0, collapses: 0, flickers: 0, jumps: 0, selectedEvicted: 0 });
      // A coarsen's move less its bakes' disagreement: at the delta a rebake lands on one level
      // before the other, and the coarsen shows it (largestCoarsenPx, recorded: the rebake's
      // fade is seamless-2's).
      expect(summary.largestCoarsenLevelPx).toBeLessThanOrEqual(10);
    },
  );
});
