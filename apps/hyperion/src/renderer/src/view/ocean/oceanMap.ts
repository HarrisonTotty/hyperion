/**
 * A body's sea on the GPU (plan R11, R11.T8.b; Design notes 11 and 19): the height workers' ocean
 * map, decided into open water by `oceanMaskOf` in the worker beside it, with its depth, its level
 * and its month-blended wind packed a texel a coarse cell, and the store that keeps one a body.
 *
 * @remarks
 * - **The texels** are a 2D array of six 2ᴸ × 2ᴸ layers, a face a layer, in the coverage mask's
 *   layout (`(face · n + j) · n + i`, read through `cubeTexel.wgsl`'s `class_map_texel`, nearest,
 *   never filtered, since the mask is decided per coarse cell and never by a shader), of
 *   {@link OCEAN_MAP_FORMAT}: r the water's depth, m, above 0 exactly over open water and 0 on every
 *   other cell (dry, under ice, unsurveyed); g the water surface less the sea level, m, a closed
 *   basin's own level and 0 over the sea; b and a the month-blended 10 m wind's east and north
 *   components, m/s, 0 off open water. Half floats hold a depth to 8 m at 10 km and the wind to 0.05%.
 *   6 · 4ᴸ · 8 bytes: 48 KiB at level 5, 3 MiB at level 8, filed under `ocean` (R12 Design note 6).
 * - **The wind** turns with the seasons (Design note 19): each cell's twelve monthly winds blended
 *   between the two months nearest the scene's time by R09's `month_blend` (its twin,
 *   {@link monthBlend}), the speeds linearly, since Cox–Munk's slope variance is linear in the mean
 *   speed, along the direction of the blended vectors (lean, 2026-11-13), a month's own where they
 *   cancel. The store blends again when the months change or the weight has moved by
 *   {@link OCEAN_WIND_WEIGHT_STEP}.
 * - **The store** ({@link OceanMapStore}) asks the pool for a body's map when the body is wanted and
 *   again after each of its blocks that surveys a cell, starts the field afresh, or brings a cell
 *   while the map lacks a climate record, as R10's class-map store does; packs it in the frames'
 *   slices ({@link OceanMapStore.step}); remakes its texture after a device loss; and releases it
 *   with the body; a sea of no open water holds no texture. The terrain session holds the store
 *   (`TerrainSession.oceans`), and the photorealistic renderer's `ocean` pass reads it.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { fieldCellCount } from "../terrain/coverage";
import type { Xyz } from "../terrain/cube";
import type { FieldChange, HeightWorkerPool } from "../terrain/workers/pool";
import {
  OCEAN_MAP_MONTHS,
  OCEAN_MAP_WIND_BYTES,
  type OceanMapAnswer,
} from "../terrain/workers/messages";
import { type MonthBlend, monthBlend } from "./monthBlend";
import { type OceanMask, oceanMaskIndexAt } from "./oceanMask";

/** The ocean map's texel format: depth, level, wind east and north, four half floats. */
export const OCEAN_MAP_FORMAT = "rgba16float" satisfies GPUTextureFormat;

/** The values a texel holds. */
export const OCEAN_TEXEL_VALUES = 4;

/** The staged scope's label of an ocean map's upload (`RenderEngine.stagedWrites`). */
export const OCEAN_MAP_LABEL = "ocean map upload";

/**
 * The change in the month blend's weight after which the store blends the winds again: 1 ÷ 32, a
 * month in 32 steps, each under 4% of the difference between two months' winds (lean, 2026-11-13).
 */
export const OCEAN_WIND_WEIGHT_STEP = 1 / 32;

/** The cells a step of {@link packOceanTexelsSteps} packs before it yields. */
const OCEAN_STEP = 4096;

/** The blended vector's length, over the blended speed, below which the months' winds cancel. */
const CANCELLED = 1e-9;

/** 2π ÷ 256, the radians of one of a wind's azimuth codes. */
const AZIMUTH_STEP_RAD = (2 * Math.PI) / 256;

/** A 10 m wind, m/s: towards local east and north. */
export interface OceanWind {
  readonly eastMps: number;
  readonly northMps: number;
  /** The mean speed, at least the vector's length. */
  readonly speedMps: number;
}

/** Month `month`'s wind of the cell at `place` in `answer`, east and north, m/s. */
function monthWind(
  answer: Pick<OceanMapAnswer, "winds" | "windSpeedsMps">,
  place: number,
  month: number,
): OceanWind {
  const at = place * OCEAN_MAP_WIND_BYTES + 2 * month;
  const azimuth = (answer.winds[at] ?? 0) * AZIMUTH_STEP_RAD;
  const speed = answer.windSpeedsMps[answer.winds[at + 1] ?? 0] ?? 0;
  return {
    eastMps: speed * Math.sin(azimuth),
    northMps: speed * Math.cos(azimuth),
    speedMps: speed,
  };
}

/**
 * The 10 m wind of the cell at `place` in `answer` between the two months `blend` names: the mean
 * speeds blended linearly along the direction of the blended vectors, `from`'s direction where they
 * cancel (see the module's remarks).
 */
export function blendedWindAt(
  answer: Pick<OceanMapAnswer, "winds" | "windSpeedsMps">,
  place: number,
  blend: MonthBlend,
): OceanWind {
  const w = blend.weight;
  const from = monthWind(answer, place, blend.from % OCEAN_MAP_MONTHS);
  const to = monthWind(answer, place, blend.to % OCEAN_MAP_MONTHS);
  const speed = (1 - w) * from.speedMps + w * to.speedMps;
  const east = (1 - w) * from.eastMps + w * to.eastMps;
  const north = (1 - w) * from.northMps + w * to.northMps;
  const length = Math.hypot(east, north);
  // Opposed winds cancel to the rounding of their sines, not to 0: a length below 10⁻⁹ of the
  // speed has no direction of its own.
  if (length > CANCELLED * speed) {
    return {
      eastMps: (speed * east) / length,
      northMps: (speed * north) / length,
      speedMps: speed,
    };
  }
  const fromLength = from.speedMps;
  return fromLength > 0
    ? {
        eastMps: (speed * from.eastMps) / fromLength,
        northMps: (speed * from.northMps) / fromLength,
        speedMps: speed,
      }
    : { eastMps: 0, northMps: 0, speedMps: speed };
}

/** A body's sea packed as the ocean's shaders read it (see the module's remarks). */
export interface OceanTexels {
  /** The field's level, 5 to 8. */
  readonly level: number;
  /** n = 2ᴸ, the texels along a face's side. */
  readonly faceTexels: number;
  /** The blend the winds were packed at. */
  readonly blend: MonthBlend;
  /** Six layers of n × n texels, {@link OCEAN_TEXEL_VALUES} half floats a texel. */
  readonly data: Float16Array;
}

/**
 * Packs `answer`'s open water, decided as `mask` decides it, with its winds at `blend`:
 * {@link packOceanTexelsSteps} run to its end.
 */
export function packOceanTexels(
  answer: OceanMapAnswer,
  mask: OceanMask,
  blend: MonthBlend,
): OceanTexels {
  const steps = packOceanTexelsSteps(answer, mask, blend);
  for (;;) {
    const step = steps.next();
    if (step.done === true) {
      return step.value;
    }
  }
}

/**
 * {@link packOceanTexels} as steps for the frames' slices: it yields after every
 * {@link OCEAN_STEP} cells; run to its end, the same texels.
 *
 * @throws RangeError where `answer` holds other than {@link OCEAN_MAP_WIND_BYTES} wind bytes a cell
 *   or a speed table of other than 256.
 */
export function* packOceanTexelsSteps(
  answer: OceanMapAnswer,
  mask: OceanMask,
  blend: MonthBlend,
): Generator<void, OceanTexels, undefined> {
  if (answer.winds.length !== OCEAN_MAP_WIND_BYTES * answer.cells.length) {
    throw new RangeError(
      `an ocean map of ${String(answer.cells.length)} cells has ${String(answer.winds.length)} wind bytes`,
    );
  }
  if (answer.windSpeedsMps.length !== 256) {
    throw new RangeError(`a wind speed table of ${String(answer.windSpeedsMps.length)} codes`);
  }
  const data = new Float16Array(fieldCellCount(mask.level) * OCEAN_TEXEL_VALUES);
  let done = 0;
  for (let at = 0; at < mask.open.length; at += 1) {
    if (mask.open[at] !== 1) {
      continue;
    }
    const place = mask.places[at] ?? -1;
    const wind = blendedWindAt(answer, place, blend);
    data.set(
      [answer.depthM[place] ?? 0, answer.levelM[place] ?? 0, wind.eastMps, wind.northMps],
      at * OCEAN_TEXEL_VALUES,
    );
    done += 1;
    if (done % OCEAN_STEP === 0) {
      yield;
    }
  }
  return { level: mask.level, faceTexels: mask.faceTexels, blend, data };
}

/** What the ocean reads of an open-water texel. */
export interface OceanTexel {
  /** The water's depth, m, above 0. */
  readonly depthM: number;
  /** The water surface less the sea level, m. */
  readonly levelM: number;
  readonly windEastMps: number;
  readonly windNorthMps: number;
}

/**
 * The texel holding a body-fixed direction (it need not be unit), as the shader reads it, nearest:
 * `null` where it is not open water. The CPU twin of `oceanFar.wgsl`'s `ocean_texel`.
 */
export function oceanTexelAt(texels: OceanTexels, direction: Xyz): OceanTexel | null {
  const at = oceanMaskIndexAt(texels, direction) * OCEAN_TEXEL_VALUES;
  const depthM = texels.data[at] ?? 0;
  if (!(depthM > 0)) {
    return null;
  }
  return {
    depthM,
    levelM: texels.data[at + 1] ?? 0,
    windEastMps: texels.data[at + 2] ?? 0,
    windNorthMps: texels.data[at + 3] ?? 0,
  };
}

/**
 * The specification of a body's ocean map: a 2D array of six 2ᴸ × 2ᴸ layers of
 * {@link OCEAN_MAP_FORMAT}, under `ocean` (R11's Design note 18, R12 Design note 6).
 */
export function oceanMapTextureSpec(name: string, level: number): TextureSpec {
  const side = 2 ** level;
  return {
    name,
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: OCEAN_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "ocean",
  };
}

/** What the store asks of the height workers (`HeightWorkerPool`'s). */
export type OceanMapPool = Pick<HeightWorkerPool, "oceanMap" | "onFieldChange">;

/** What the store asks of the engine. */
export type OceanMapEngine = Pick<
  RenderEngine,
  "createTexture" | "writeTexture" | "releaseTexture" | "onRestored" | "stagedWrites"
>;

/** A body's sea as the store hands it out. */
export interface HeldOceanMap {
  readonly answer: OceanMapAnswer;
  readonly mask: OceanMask;
  readonly texels: OceanTexels;
  readonly texture: TextureHandle;
}

/** A body the store is asked for. */
interface HeldBody {
  readonly body: BodyIdHex;
  /** The scene's mean anomaly of the body's seasonal orbit, rad; 0, periapsis, until told. */
  meanAnomalyRad: number;
  /** Whether a query is out, and whether a change came since it was posted. */
  asking: boolean;
  again: boolean;
  answer: OceanMapAnswer | null;
  mask: OceanMask | null;
  texels: OceanTexels | null;
  texture: TextureHandle | null;
  /** The texels being packed in the frames' slices, and the blend they are packed at. */
  packing: { readonly steps: Generator<void, OceanTexels, undefined> } | null;
}

/**
 * The bodies' ocean maps on one engine, fed from a height-worker pool (plan R11, R11.T8.b): one
 * texture a body under `ocean`, remade after a device loss and released with its body.
 *
 * @remarks
 * {@link OceanMapStore.want} asks the pool for a body's map, and the store asks again after each of
 * the body's blocks that surveys a cell, starts the field afresh, or brings a cell while the map is
 * incomplete; an answer to an older query than the body's latest is dropped, and a refused one is
 * reported once a body with `console.error`. {@link OceanMapStore.season} sets the time the winds
 * are blended at. An answer, or a new blend, is packed in the frames' slices
 * ({@link OceanMapStore.step}) and uploaded at the packing's end, the body's map until then the one
 * it held.
 */
export class OceanMapStore {
  readonly #engine: OceanMapEngine;
  readonly #pool: OceanMapPool;
  readonly #name: string;
  readonly #bodies = new Map<BodyIdHex, HeldBody>();
  readonly #reported = new Set<BodyIdHex>();
  readonly #unsubscribe: ReadonlyArray<() => void>;
  #disposed = false;

  /** Feeds from `pool` onto `engine`, its textures named `<name> <body>`. */
  constructor(engine: OceanMapEngine, pool: OceanMapPool, name = "ocean map") {
    this.#engine = engine;
    this.#pool = pool;
    this.#name = name;
    this.#unsubscribe = [
      pool.onFieldChange((change) => {
        this.#changed(change);
      }),
      engine.onRestored(() => {
        this.#restored();
      }),
    ];
  }

  /** The bodies asked for, in the order first asked. */
  get bodies(): ReadonlyArray<BodyIdHex> {
    return [...this.#bodies.keys()];
  }

  /** Asks for `body`'s map, now and after each block that changes it; a body held is left as it is. */
  want(body: BodyIdHex): void {
    this.#assertLive();
    if (this.#bodies.has(body)) {
      return;
    }
    this.#bodies.set(body, {
      body,
      meanAnomalyRad: 0,
      asking: false,
      again: false,
      answer: null,
      mask: null,
      texels: null,
      texture: null,
      packing: null,
    });
    this.#ask(body);
  }

  /** Drops `body`'s map and releases its texture; a body not asked for is no error. */
  release(body: BodyIdHex): void {
    const held = this.#bodies.get(body);
    if (held === undefined) {
      return;
    }
    this.#releaseTexture(held);
    this.#bodies.delete(body);
    this.#reported.delete(body);
  }

  /**
   * Sets the time `body`'s winds are blended at, the seasonal orbit's mean anomaly at the scene's
   * time (rad, finite): packed again where the blend's months differ from the texels' or its weight
   * has moved by {@link OCEAN_WIND_WEIGHT_STEP}. A body not asked for is no error.
   *
   * @throws RangeError for a mean anomaly that is not finite.
   */
  season(body: BodyIdHex, meanAnomalyRad: number): void {
    if (!Number.isFinite(meanAnomalyRad)) {
      throw new RangeError(`a mean anomaly must be finite, not ${String(meanAnomalyRad)}`);
    }
    const held = this.#bodies.get(body);
    if (held === undefined) {
      return;
    }
    held.meanAnomalyRad = meanAnomalyRad;
    const answer = held.answer;
    const mask = held.mask;
    if (answer === null || mask === null) {
      return;
    }
    const blend = this.#blendOf(body, answer, meanAnomalyRad);
    const packed = held.texels?.blend ?? null;
    if (
      blend !== null &&
      (packed === null ||
        packed.from !== blend.from ||
        packed.to !== blend.to ||
        Math.abs(packed.weight - blend.weight) >= OCEAN_WIND_WEIGHT_STEP)
    ) {
      held.packing = { steps: packOceanTexelsSteps(answer, mask, blend) };
    }
  }

  /**
   * `body`'s sea as uploaded, or `null` where it has none, no open water, or its texture is not made
   * (the device lost).
   */
  oceanMapOf(body: BodyIdHex): HeldOceanMap | null {
    const held = this.#bodies.get(body);
    if (
      held === undefined ||
      held.answer === null ||
      held.mask === null ||
      held.texels === null ||
      held.texture === null
    ) {
      return null;
    }
    return { answer: held.answer, mask: held.mask, texels: held.texels, texture: held.texture };
  }

  /** Whether a body's texels are being packed in the frames' slices. */
  get packing(): boolean {
    return [...this.#bodies.values()].some((held) => held.packing !== null);
  }

  /**
   * The frame's slice of the packing: each body's steps in the order first asked until a step ends
   * past `budgetMs`, each body's texture written at its packing's end.
   *
   * @param budgetMs - The slice's length, ms.
   * @param now - A monotonic clock, ms.
   * @returns Whether packing remains.
   */
  step(budgetMs = 2, now: () => number = () => globalThis.performance.now()): boolean {
    const startMs = now();
    for (const [body, held] of this.#bodies) {
      while (held.packing !== null) {
        if (!(now() - startMs < budgetMs)) {
          return true;
        }
        try {
          const step = held.packing.steps.next();
          if (step.done === true) {
            held.packing = null;
            held.texels = step.value;
            this.#upload(held);
          }
        } catch (error: unknown) {
          held.packing = null;
          this.#refused(body, error);
        }
      }
    }
    return false;
  }

  /** Releases every texture and stops listening; the store cannot be used afterwards. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    for (const body of this.#bodies.keys()) {
      this.release(body);
    }
    for (const unsubscribe of this.#unsubscribe) {
      unsubscribe();
    }
    this.#disposed = true;
  }

  #assertLive(): void {
    if (this.#disposed) {
      throw new Error("the ocean-map store was disposed");
    }
  }

  /** The blend of `answer`'s year at `meanAnomalyRad`, or `null` (reported) for a year it cannot blend. */
  #blendOf(body: BodyIdHex, answer: OceanMapAnswer, meanAnomalyRad: number): MonthBlend | null {
    const blend = monthBlend(answer.months, answer.seasonEccentricity, meanAnomalyRad);
    if (blend === null) {
      this.#refused(
        body,
        new RangeError(
          `a year of ${String(answer.months)} months at eccentricity ${String(answer.seasonEccentricity)} has no blend`,
        ),
      );
    }
    return blend;
  }

  /** Posts a query for `body`, or marks it to be asked again once the one out is answered. */
  #ask(body: BodyIdHex): void {
    const held = this.#bodies.get(body);
    if (held === undefined) {
      return;
    }
    if (held.asking) {
      held.again = true;
      return;
    }
    held.asking = true;
    held.again = false;
    void this.#query(body, held).catch((error: unknown) => {
      console.error(`the ocean map of ${body} could not be asked again:`, error);
    });
  }

  /** Asks the pool for `body`'s map and, if `held` is still the body's, takes the answer. */
  async #query(body: BodyIdHex, held: HeldBody): Promise<void> {
    try {
      const replied = await this.#pool.oceanMap(body);
      if (this.#bodies.get(body) === held && !this.#disposed && replied !== null) {
        this.#take(body, held, replied.map, replied.mask);
      }
    } catch (error: unknown) {
      // A refusal of a body released, asked for afresh or disposed since is not its to report.
      if (this.#bodies.get(body) === held && !this.#disposed) {
        this.#refused(body, error);
      }
    }
    held.asking = false;
    if (held.again && this.#bodies.get(body) === held) {
      this.#ask(body);
    }
  }

  /**
   * Takes an answer for `body` with its mask, decided in the worker (`oceanMaskOf`;
   * perf-long-tasks-5), and queues its texels' packing.
   */
  #take(body: BodyIdHex, held: HeldBody, answer: OceanMapAnswer, mask: OceanMask): void {
    const blend = this.#blendOf(body, answer, held.meanAnomalyRad);
    if (blend === null) {
      return;
    }
    if (held.mask !== null && held.mask.level !== mask.level) {
      // A texture of another level cannot take the new texels.
      this.#releaseTexture(held);
      held.texels = null;
    }
    held.answer = answer;
    held.mask = mask;
    held.packing = { steps: packOceanTexelsSteps(answer, mask, blend) };
  }

  /** Reports a query refused or an answer that could not be taken, once a body. */
  #refused(body: BodyIdHex, error: unknown): void {
    if (!this.#reported.has(body)) {
      this.#reported.add(body);
      console.error(`the ocean map of ${body} could not be made:`, error);
    }
  }

  /**
   * Writes `held`'s texels into its texture, making it first where there is none; nothing while the
   * device is lost, the restore making it from the texels held. A sea of no open water holds no
   * texture, a dry world's up to 3 MiB: its texture is released and none is made.
   */
  #upload(held: HeldBody): void {
    const texels = held.texels;
    if (texels === null) {
      return;
    }
    if (held.mask?.openCells === 0) {
      this.#releaseTexture(held);
      return;
    }
    if (held.texture === null) {
      try {
        held.texture = this.#engine.createTexture(
          oceanMapTextureSpec(`${this.#name} ${held.body}`, texels.level),
        );
      } catch (error: unknown) {
        if (error instanceof EngineUnavailable) {
          return;
        }
        throw error;
      }
    }
    const texture = held.texture;
    const side = texels.faceTexels;
    // Staged, so that a map of up to 3 MiB never waits on the GPU process, as the coverage mask's.
    this.#engine.stagedWrites(OCEAN_MAP_LABEL, () => {
      this.#engine.writeTexture(
        texture,
        { x: 0, y: 0, z: 0 },
        { width: side, height: side, depthOrArrayLayers: 6 },
        texels.data,
      );
    });
  }

  /** Releases `held`'s texture, if it has one. */
  #releaseTexture(held: HeldBody): void {
    if (held.texture !== null) {
      this.#engine.releaseTexture(held.texture);
      held.texture = null;
    }
  }

  /** Asks again for a body whose block surveyed cells, started its field, or brought what it lacked. */
  #changed(change: FieldChange): void {
    const held = this.#bodies.get(change.body);
    if (held === undefined) {
      return;
    }
    const incomplete = held.answer !== null && !held.answer.complete;
    if (
      change.sequence === 0 ||
      change.newlySurveyed.length > 0 ||
      (incomplete && change.newlyHeld.length > 0) ||
      held.answer === null
    ) {
      this.#ask(change.body);
    }
  }

  /** Makes every map's texture again on the restored device. */
  #restored(): void {
    for (const held of this.#bodies.values()) {
      if (held.texels === null) {
        continue;
      }
      // The lost device's handle: the resilient engine drops its release.
      this.#releaseTexture(held);
      this.#upload(held);
    }
  }
}
