/**
 * The smoke page's still of a body handed over to its terrain (plan R11, R11.T10.e; ruling 4a of
 * decision-leans-54), in part 28: the Moon-like fixture's first survey taken by two height workers,
 * one lit pass over it, a view settled 30 km over the survey's edge with the body handed over, and
 * a still of that view's frame through the photorealistic renderer's hook (`PhotorealStills`), its
 * own terrain view noted with the pass at bulk priority and streamed before its first batch.
 *
 * @remarks
 * The pass stands in for the view's terrain session, through the seams a still takes of one
 * (`StillTerrainSession`): each animation frame steps it over the view and the still's own view,
 * as `VIEW`'s loop steps its session, and draws each view's terrain from its own selection. It
 * checks that the still is taken with the body handed over, that its own view streamed within the
 * timeout and drew the terrain in every sample, and that its assembled HDR image is finite and lit.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, scale } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { type LitBodyInput, planLitBodies } from "../view/bodies/draw";
import type { ViewId } from "../view/camera/state";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import type { RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import {
  type DrawnPhotorealFrame,
  type PhotorealFrame,
  PhotorealStills,
} from "../view/photoreal/renderer";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { encodeStillPng, runStillJob, type StillRenderer } from "../view/stills/stillJob";
import { STILL_TERRAIN_STREAMING } from "../view/stills/stillLabel";
import { STILL_TERRAIN_VIEW_ID, type StillTerrainSession } from "../view/stills/stillTerrain";
import { CoverageMask } from "../view/terrain/coverage";
import { LiveFrameTerrain, terrainViewOf } from "../view/terrain/frameTerrain";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "../view/terrain/handover";
import { planetGeometry } from "../view/terrain/planet";
import { BULK_VIEW_WEIGHT } from "../view/terrain/priority";
import { SurveyedCells } from "../view/terrain/surveyLookup";
import { TerrainPass, type TerrainPassView } from "../view/terrain/terrainPass";
import { HeightWorkerPool } from "../view/terrain/workers/pool";
import { moonLikeFixture } from "../test/moonLikeFixture";
import { viewAt } from "../test/terrainSessionFixtures";
import { TEST_STILL_REQUEST } from "../test/stillFixtures";
import { smokeGeometry } from "./geometry";
import { type Checks, halfTexels, pause } from "./harness";
import { MOON_LIKE } from "./litTerrain";
import { EXPOSURE, type Pose, poseOver, VIEWPORT } from "./surveyEdge";

/** The pass's cache budget, 32 MiB, as the frame checks take it. */
const CACHE_BYTES = 2 ** 25;

/** How long the workers may take over the field, and the view over its streaming, ms. */
const STREAM_TIMEOUT_MS = 90_000;

/** The view the still is taken of, as the pass knows it. */
const VIEW: ViewId = viewId("still terrain view");

/** The still's samples: two, so that the jitter's accumulation is drawn, and the part short. */
const SAMPLES = 2;

/** The body as the frame lights it: handed over by a relief of four times τ at its limb. */
function bodyOf(pose: Pose, id: BodyIdHex): LitBodyInput {
  const centreM = scale(pose.cameraM, -1);
  const limbM = limbDistanceM(
    Math.hypot(centreM.x, centreM.y, centreM.z),
    MOON_LIKE.equatorialRadiusM,
  );
  return {
    id,
    centreM,
    figure: { ...MOON_LIKE, pole: { x: 0, y: 0, z: 1 } },
    photometry: PROVISIONAL_PHOTOMETRY,
    rotation: IDENTITY_ROTATION,
    handoverReliefM:
      4 * handoverTauPx("photorealistic", "low") * limbM * pixelAngleRad(pose.camera, VIEWPORT),
    lighting: undefined,
  };
}

/** Waits for `done`, polling every 20 ms, up to `timeoutMs`; whether it came. */
async function until(done: () => boolean, timeoutMs: number): Promise<boolean> {
  const started = performance.now();
  while (!done()) {
    if (performance.now() - started > timeoutMs) {
      return false;
    }
    // The workers and the engine answer on the event loop.
    // oxlint-disable-next-line no-await-in-loop
    await pause(20);
  }
  return true;
}

/** Part 28's still of a body handed over to its terrain (R11.T10.e): see the module's remarks. */
export async function checkTerrainStill(checks: Checks, engine: RenderEngine): Promise<void> {
  const field = moonLikeFixture("first");
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () =>
      new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      }),
  });
  const survey = new SurveyedCells();
  const mask = new CoverageMask(engine, "still terrain smoke: coverage mask");
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
    mask.survey(change);
  });
  let pass: TerrainPass | null = null;
  let stepping = true;
  let detach: (() => void) | null = null;
  try {
    field.blocks.forEach((block, n) => {
      if (n === 0) {
        pool.fieldInit(field.body, block);
      } else {
        pool.fieldChunk(field.body, block);
      }
    });
    const taken = await until(
      () => pool.fieldBlocks(field.body)?.reported === field.blocks.length,
      STREAM_TIMEOUT_MS,
    );
    const made = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      geometry: smokeGeometry("low"),
      planet: planetGeometry({ ...MOON_LIKE, pole: null }, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) => pool.lane(bake),
      survey,
    });
    pass = made;
    made.setReducedMotion(true);
    await made.ready();
    const pose = poseOver();
    const base = bodyOf(pose, field.body);
    const tauPx = handoverTauPx("photorealistic", "low");
    const viewOf = (id: ViewId, size = VIEWPORT, bulk = false): TerrainPassView => {
      const view = terrainViewOf(base, pose.camera, size);
      if (view === null) {
        throw new Error("the body turns");
      }
      return bulk
        ? { id, view, tauPx, weight: BULK_VIEW_WEIGHT, bulk }
        : { id, view, tauPx, weight: 1 };
    };
    // The still's own views, as the session would take them from their reports.
    const stills = new Map<ViewId, TerrainPassView>();
    const drawnFor = new Map<ViewId, number>();
    const holds: Array<ReadonlyArray<BodyIdHex>> = [];
    const step = (): void => {
      made.step({ views: [viewOf(VIEW), ...stills.values()], grounded: [] });
    };
    const session: StillTerrainSession = {
      noteView: (id, report) => {
        stills.set(
          id,
          viewOf(id, report.selectionPx ?? report.presentedPx, report.streamPriority === "bulk"),
        );
      },
      forgetView: (id) => {
        stills.delete(id);
      },
      holdSurvey: (bodies) => {
        holds.push([...bodies]);
        return () => undefined;
      },
      streamedFor: (id) => made.streamedFor(id),
      drawTerrain: (id, _body, input) => {
        const frame = made.drawFor(id, input);
        if (frame.draws.length > 0) {
          drawnFor.set(id, (drawnFor.get(id) ?? 0) + 1);
        }
        return frame;
      },
      drawLateTerrain: (id, _body, byIds) => made.lateDrawFor(id, byIds),
      coverageMask: () => mask.texture,
      coverageWhole: () => mask.whole,
      pass: () => made,
    };
    // The view settled first, its mask ready, as a view's frame gives a still.
    const viewStreamed = await until(() => {
      step();
      return made.maskReadyFor(VIEW) && made.streamedFor(VIEW);
    }, STREAM_TIMEOUT_MS);
    // From here the pass steps each animation frame, as the view's loop steps its session.
    const loop = (): void => {
      if (stepping) {
        step();
        requestAnimationFrame(loop);
      }
    };
    requestAnimationFrame(loop);
    const nowMs = performance.now();
    const terrain = new LiveFrameTerrain(session, {
      view: VIEW,
      camera: pose.camera,
      presentedPx: VIEWPORT,
      exposureScale: EXPOSURE,
      occluderSlopePx: 0,
      nowMs,
    });
    const coverage = terrain.surveyCoverage(base.id);
    const body: LitBodyInput =
      coverage === null
        ? base
        : {
            ...base,
            surveyCoverage: {
              mask: coverage,
              rotation: IDENTITY_ROTATION,
              whole: terrain.surveyedWhole(base.id),
            },
          };
    const lights = [{ disc: sunLikeHostDisc(), centreM: add(body.centreM, scale(pose.sun, AU_M)) }];
    const previousRegimes = new Map([[body.id, "terrain" as const]]);
    const frame: PhotorealFrame = {
      camera: pose.camera,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      role: "eye",
      setting: "low",
      exposureScale: EXPOSURE,
      sky: [],
      starSprites: [],
      hostDraws: new Map(),
      glareSources: [],
      lights,
      bodies: [body],
      depthWriters: [],
      previousRegimes,
      terrain,
      overlay: null,
      meter: null,
    };
    const plan = planLitBodies(
      [body],
      lights,
      {
        camera: pose.camera,
        viewport: VIEWPORT,
        presentedPx: VIEWPORT,
        exposureScale: EXPOSURE,
        annuli: DISC_ANNULI_HIGH,
        planetshine: PLANETSHINE_SOURCES_HIGH,
        setting: "low",
      },
      previousRegimes,
    );
    // Made again at a still's size, as VIEW's drawer makes it, for the tiled comparison.
    const drawn: DrawnPhotorealFrame = {
      frame,
      plan,
      occluderSlopePx: 0,
      remake: (ask) => ({
        ...frame,
        viewport: ask.viewport,
        setting: ask.setting,
        terrain: ask.terrain,
      }),
    };
    const { size: _size, ...request } = TEST_STILL_REQUEST;
    const hook = new PhotorealStills({ samples: SAMPLES });
    detach = hook.attach({
      engine,
      timer: () => "absent",
      frameIntervalNs: () => 16_683_350,
      setting: () => "low",
      drawn: () => drawn,
      request: () => request,
      terrain: () => ({ session, report: viewAt(5e6), occluderSlopePx: 0 }),
    });
    const handedOver = plan.regimes.get(body.id) === "terrain";
    const snapshot = hook.snapshot();
    checks.check(
      "R11.T10.e the hook gives a still of a view whose body is handed over to its terrain",
      taken && viewStreamed && handedOver && snapshot.kind === "scene",
      `blocks taken ${String(taken)}, view streamed ${String(viewStreamed)}, regime ${String(plan.regimes.get(body.id))}, ${snapshot.kind === "scene" ? "a scene" : snapshot.cause}`,
    );
    if (snapshot.kind !== "scene") {
      return;
    }
    const inner = hook.renderer(snapshot.scene);
    // The HDR image, read with the first tile before the job disposes the renderer, and whether the
    // still's view had streamed at its first batch.
    const seen: { hdr: Promise<ArrayBuffer> | null; streamed: boolean | null } = {
      hdr: null,
      streamed: null,
    };
    const renderer: StillRenderer = {
      tiling: inner.tiling,
      units: inner.units,
      prepare: () => inner.prepare(),
      streamed: () => inner.streamed(),
      submit: (first, count) => {
        seen.streamed ??= inner.streamed();
        inner.submit(first, count);
      },
      readTile: (tile) => {
        seen.hdr ??= inner.readHdr();
        return inner.readTile(tile);
      },
      dispose: () => {
        inner.dispose();
      },
    };
    const noted = stills.get(STILL_TERRAIN_VIEW_ID);
    const startMs = performance.now();
    const outcome = await runStillJob({
      device: hook.device,
      renderer,
      request: snapshot.scene.request,
      setting: hook.setting,
      frameIntervalNs: hook.frameIntervalNs(),
      encode: encodeStillPng,
      save: (_png, label) =>
        Promise.resolve({ kind: "saved", still: { id: "terrain still", label } }),
      onStep: () => undefined,
    });
    const elapsedMs = performance.now() - startMs;
    const labelled =
      outcome.kind === "saved" &&
      outcome.still.label.annunciations.includes(STILL_TERRAIN_STREAMING);
    const samplesDrawn = drawnFor.get(STILL_TERRAIN_VIEW_ID) ?? 0;
    checks.check(
      "R11.T10.e a still of a body handed over streams its own terrain view at bulk priority and draws its terrain in every sample",
      outcome.kind === "saved" &&
        noted?.bulk === true &&
        holds.length === 1 &&
        holds[0]?.[0] === body.id &&
        seen.streamed === true &&
        !labelled &&
        samplesDrawn === SAMPLES &&
        !stills.has(STILL_TERRAIN_VIEW_ID),
      `${outcome.kind}; noted bulk ${String(noted?.bulk)}, survey held ${String(holds.length)}, streamed ${String(seen.streamed)}, labelled streaming ${String(labelled)}, terrain drawn in ${String(samplesDrawn)} of ${String(SAMPLES)} samples, forgotten ${String(!stills.has(STILL_TERRAIN_VIEW_ID))}, ${elapsedMs.toFixed(0)} ms`,
    );
    if (seen.hdr === null) {
      return;
    }
    const texels = halfTexels(await seen.hdr);
    let infinite = 0;
    let lit = 0;
    for (let i = 0; i < texels.length; i += 4) {
      const rgb = [texels[i] ?? 0, texels[i + 1] ?? 0, texels[i + 2] ?? 0];
      if (!rgb.every(Number.isFinite)) {
        infinite += 1;
      } else if (Math.max(...rgb) > 0) {
        lit += 1;
      }
    }
    const pixels = VIEWPORT.widthPx * VIEWPORT.heightPx;
    checks.check(
      "R11.T10.e the terrain still's assembled HDR image is finite and lit across the ground",
      infinite === 0 && lit > pixels / 2,
      `${String(infinite)} pixels not finite, ${String(lit)} of ${String(pixels)} lit`,
    );

    // The still at twice the view's size, drawn whole and 2 × 2-tiled, its terrain's clusters culled
    // by each tile's frustum (R11.T10.e.2): first one to stream the finer patches, then the two.
    const takeAt = async (tileLimitPx: number): Promise<TerrainStillTaken> => {
      const source = new PhotorealStills({ samples: 1, tileLimitPx });
      const off = source.attach({
        engine,
        timer: () => "absent",
        frameIntervalNs: () => 16_683_350,
        setting: () => "low",
        drawn: () => drawn,
        request: () => request,
        terrain: () => ({ session, report: viewAt(5e6), occluderSlopePx: 0 }),
      });
      try {
        return await takeStill(source, TILED_SCALE);
      } finally {
        off();
      }
    };
    await takeAt(8192);
    const whole = await takeAt(8192);
    const tiled = await takeAt(TILED_LIMIT_PX);
    const comparable =
      whole.streamed &&
      tiled.streamed &&
      whole.tiles === 1 &&
      tiled.tiles === 4 &&
      whole.hdr.length > 0 &&
      whole.hdr.length === tiled.hdr.length;
    const { largest, over } = comparable
      ? relativeDifferences(whole.hdr, tiled.hdr)
      : { largest: Number.NaN, over: Number.NaN };
    checks.check(
      "R11.T10.e a 2 × 2-tiled still of a body handed over equals its untiled still, its terrain culled by each tile's frustum",
      comparable && over === 0,
      `streamed ${String(whole.streamed)} and ${String(tiled.streamed)} in ${String(whole.tiles)} and ${String(tiled.tiles)} tiles; ${String(over)} channels over ${TILED_TOLERANCE.toExponential(2)}, the largest relative difference ${largest.toExponential(3)}`,
    );
  } finally {
    stepping = false;
    detach?.();
    pass?.dispose();
    offChange();
    pool.terminate();
    mask.release();
  }
}

/** The terrain still's multiple of the view's size in the tiled comparison: 128 × 96 px. */
const TILED_SCALE = 2;

/** The tile limit that cuts the 128 × 96 px still into 2 × 2 tiles, px. */
const TILED_LIMIT_PX = 64;

/**
 * The most a texel of the tiled terrain still may differ from the untiled one's, relative to the
 * larger: two units in the last place of `rgba16float`, as the tiled still's check holds
 * (`stillSize.ts`).
 */
const TILED_TOLERANCE = 2 ** -9;

/** What a still of the terrain view gave: whether it streamed, its tiles and its HDR texels. */
interface TerrainStillTaken {
  readonly streamed: boolean;
  readonly tiles: number;
  readonly hdr: Float32Array;
}

/** A still of `source`'s view at `multiple` times its size, run through the job to its end. */
async function takeStill(source: PhotorealStills, multiple: number): Promise<TerrainStillTaken> {
  const snapshot = source.snapshot({ scale: multiple });
  if (snapshot.kind === "unavailable") {
    return { streamed: false, tiles: 0, hdr: new Float32Array(0) };
  }
  const inner = source.renderer(snapshot.scene);
  const seen: { hdr: Promise<ArrayBuffer> | null } = { hdr: null };
  const renderer: StillRenderer = {
    tiling: inner.tiling,
    units: inner.units,
    prepare: () => inner.prepare(),
    streamed: () => inner.streamed(),
    submit: (first, count) => {
      inner.submit(first, count);
    },
    readTile: (tile) => {
      seen.hdr ??= inner.readHdr();
      return inner.readTile(tile);
    },
    dispose: () => {
      inner.dispose();
    },
  };
  const outcome = await runStillJob({
    device: source.device,
    renderer,
    request: snapshot.scene.request,
    setting: source.setting,
    frameIntervalNs: source.frameIntervalNs(),
    encode: encodeStillPng,
    save: (_png, label) =>
      Promise.resolve({ kind: "saved", still: { id: "terrain still", label } }),
    onStep: () => undefined,
  });
  return {
    streamed:
      outcome.kind === "saved" &&
      !outcome.still.label.annunciations.includes(STILL_TERRAIN_STREAMING),
    tiles: inner.tiling.tiles.length,
    hdr: seen.hdr === null ? new Float32Array(0) : halfTexels(await seen.hdr),
  };
}

/** The colour channels' differences: the largest relative one, and how many pass the tolerance. */
function relativeDifferences(
  a: Float32Array,
  b: Float32Array,
): { readonly largest: number; readonly over: number } {
  let largest = 0;
  let over = 0;
  for (let i = 0; i < a.length; i += 1) {
    if (i % 4 === 3) {
      continue;
    }
    const x = a[i] ?? Number.NaN;
    const y = b[i] ?? Number.NaN;
    const larger = Math.max(Math.abs(x), Math.abs(y));
    const relative = larger === 0 ? 0 : Math.abs(x - y) / larger;
    if (!(relative <= TILED_TOLERANCE)) {
      over += 1;
    }
    largest = Math.max(largest, Number.isNaN(relative) ? Number.POSITIVE_INFINITY : relative);
  }
  return { largest, over };
}
