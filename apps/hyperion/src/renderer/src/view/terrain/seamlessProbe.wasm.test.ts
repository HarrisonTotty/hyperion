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
import { countingRenderEngine } from "../../test/countingRenderEngine";
import type { LitRegime } from "../bodies/regime";
import { project } from "../camera/projection";
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
import { vertexDir } from "./cube";
import { handoverTauPx, handsOver, limbDistanceM, pixelAngleRad } from "./handover";
import { parentKey, type PatchKey, patchKeyString } from "./patchKey";
import { type BodyFigure, spheroidNormal, surfacePoint } from "./planet";
import {
  isCollapse,
  type SeamChainLink,
  type SeamCollapse,
  type SeamEvent,
  type SeamFrame,
  type SeamRun,
  type SeamSummary,
  summariseSeamRun,
} from "./seamlessSummary";
import type { Selection, SelectionInput, ViewSelectionInput } from "./select";
import { SELECTION_MARGIN } from "./selectionTolerance";
import { TerrainSession, type TerrainViewReport } from "./session";
import { decodeOctahedral } from "./shadingNormal";
import { effectiveTauPx, morphRangeM } from "./terrainPass";
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
 * coarse–fine holds (one edge row).
 *
 * A whole run takes 10 to 25 minutes (pod, 16 vCPU), so `just ci` plays only the first 42 s, to the
 * hand-over and the first draws on high; the whole script on both settings, with the acceptance's
 * bounds, runs only under `HYPERION_SLOW_TESTS=1`. Its knobs are read from the environment
 * (`import.meta.env`), and it writes nothing: each test's figures, and with `SEAM_RECORD=1` its
 * whole record, go to the task's `meta.seamless`, which vitest's JSON reporter saves
 * (`--reporter=json --outputFile.json=<file>`).
 *
 * - `SEAM_SETTING`: `high` or `low`, one setting alone; both by default.
 * - `SEAM_COST_MS` (41: R10.T6.a's one-thread bake) and `SEAM_WORKERS` (3: `defaultHeightWorkers`
 *   on the target's 16 threads).
 * - `SEAM_FROM_S`, `SEAM_TO_S` (the script's span) and `SEAM_HZ` (60).
 * - `SEAM_STRIDE` (2): every how many vertices a change is measured at.
 * - `SEAM_TRACE_FROM_S` and `SEAM_TRACE_TO_S`: each selection between them in `meta.seamless.trace`
 *   (its margin, size, seen patches, nearest ground and levels).
 * - `SEAM_RECORD=1`: the frames, events and latencies too.
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
const TRACE_FROM_S = numberKnob("SEAM_TRACE_FROM_S", Number.POSITIVE_INFINITY);
const TRACE_TO_S = numberKnob("SEAM_TRACE_TO_S", Number.NEGATIVE_INFINITY);
const RECORD = knob("SEAM_RECORD") === "1";

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

/** A height worker on the fake clock: first in first out, a bake taking `costMs`, the rest ~0. */
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
    const cost = message.kind === "bake" ? this.#costMs : 0;
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

/** The primary view as the last selection took it, and its morph bands, level by level. */
interface Camera {
  readonly view: ViewSelectionInput;
  readonly bands: Float64Array;
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

/** `coarse`'s drawn surface at (u, v) of its own quads, on the bake's (0, 0)–(1, 1) diagonal. */
function sampleCoarse(figure: BodyFigure, cam: Camera, coarse: Held, u: number, v: number): number {
  const x = Math.min(QUADS - 1, Math.max(0, Math.floor(u)));
  const y = Math.min(QUADS - 1, Math.max(0, Math.floor(v)));
  const fx = u - x;
  const fy = v - y;
  const h00 = drawnAt(figure, cam, coarse, x, y).h;
  const h11 = drawnAt(figure, cam, coarse, x + 1, y + 1).h;
  if (fx >= fy) {
    const h10 = drawnAt(figure, cam, coarse, x + 1, y).h;
    return h00 + fx * (h10 - h00) + fy * (h11 - h10);
  }
  const h01 = drawnAt(figure, cam, coarse, x, y + 1).h;
  return h00 + fy * (h01 - h00) + fx * (h11 - h01);
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
  readonly maxNormalDeg: number;
  readonly minFineK: number;
}

/** The change from `coarse`'s drawn surface to `fine`'s, measured at `fine`'s vertices. */
function deltaOf(figure: BodyFigure, cam: Camera, coarse: Held, fine: Held): Delta {
  const span = 2 ** (fine.key.level - coarse.key.level);
  const { widthPx, heightPx } = cam.view.viewport;
  const pxPerRad = widthPx / 2 / Math.tan(cam.view.fovXRad / 2);
  let maxPx = 0;
  let maxNormalDeg = 0;
  let minFineK = 1;
  for (let y = 0; y <= QUADS; y += STRIDE) {
    for (let x = 0; x <= QUADS; x += STRIDE) {
      const u = (fine.key.i * QUADS + x) / span - coarse.key.i * QUADS;
      const v = (fine.key.j * QUADS + y) / span - coarse.key.j * QUADS;
      const f = drawnAt(figure, cam, fine, x, y);
      const dh = Math.abs(f.h - sampleCoarse(figure, cam, coarse, u, v));
      const dir = vertexDir(fine.key, x, y);
      const p = surfacePoint(figure, dir, f.h);
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
      minFineK = Math.min(minFineK, f.k);
      const before = normalAt(coarse, u, v);
      const own = normalAt(fine, x, y);
      if (before !== null && own !== null) {
        const after = normalise(
          vec3(
            (1 - f.k) * own.x + f.k * before.x,
            (1 - f.k) * own.y + f.k * before.y,
            (1 - f.k) * own.z + f.k * before.z,
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
  return { maxPx, maxNormalDeg, minFineK };
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
  const firstDemandS = new Map<string, number>();
  const rebaked: Array<{ readonly old: Held; readonly now: Held }> = [];
  let lastDrawn = new Map<string, Held>();
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
      session.noteView(PRIMARY_VIEW_ID, reportAt(tS));
      rebaked.length = 0;
      session.step();
      watch();
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
      if (before !== undefined && isCollapse(before, frame)) {
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
      const nowDrawn = new Map<string, Held>();
      const standIns = new Set<string>();
      for (const dp of ds.patches) {
        const held = resident.get(dp.keyString);
        if (held !== undefined) {
          nowDrawn.set(dp.keyString, held);
          if (dp.standIn) {
            standIns.add(dp.keyString);
          }
        }
      }
      // Rebakes of a key drawn before and after: its data changed in place.
      for (const { old, now } of rebaked) {
        const keyString = patchKeyString(now.key);
        if (lastDrawn.has(keyString) && nowDrawn.has(keyString)) {
          events.push({
            tS,
            kind: "rebake",
            fineLevel: now.key.level,
            levelJump: 0,
            ...deltaOf(figure, c, old, now),
            standIn: false,
          });
        }
      }
      for (const [keyString, held] of nowDrawn) {
        if (lastDrawn.has(keyString)) {
          continue;
        }
        const standIn = standIns.has(keyString);
        let found = false;
        for (let k = parentKey(held.key); k !== null; k = parentKey(k)) {
          const coarse = lastDrawn.get(patchKeyString(k));
          if (coarse !== undefined) {
            events.push({
              tS,
              kind: "refine",
              fineLevel: held.key.level,
              levelJump: held.key.level - k.level,
              ...deltaOf(figure, c, coarse, held),
              standIn,
            });
            found = true;
            break;
          }
        }
        if (found) {
          continue;
        }
        for (const [fineKey, fine] of lastDrawn) {
          if (!nowDrawn.has(fineKey) && isAncestor(held.key, fine.key)) {
            events.push({
              tS,
              kind: "coarsen",
              fineLevel: fine.key.level,
              levelJump: fine.key.level - held.key.level,
              ...deltaOf(figure, c, held, fine),
              standIn,
            });
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
    },
    trace,
  };
}

/** What a test leaves in its task's `meta.seamless`, for vitest's JSON reporter. */
function report(played: Played, summary: SeamSummary): Record<string, unknown> {
  const { run } = played;
  return {
    setting: run.setting,
    costMs: run.costMs,
    workers: run.workers,
    fromS: run.fromS,
    toS: run.toS,
    handOverS: run.handOverS,
    firstDrawS: run.firstDrawS,
    bakesAnswered: run.bakesAnswered,
    summary,
    collapses: run.collapses,
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
      expect(summary).toMatchObject({ holeFrames: 0, collapses: 0, selectedEvicted: 0 });
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
        jumps: summary.jumps,
        selectedEvicted: summary.selectedEvicted,
      }).toEqual({ holes: 0, collapses: 0, jumps: 0, selectedEvicted: 0 });
      expect(summary.largestCoarsenPx).toBeLessThanOrEqual(10);
    },
  );
});
