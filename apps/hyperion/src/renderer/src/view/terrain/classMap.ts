/**
 * A body's coarse class map for R07's disc (plan R10, T10.d; Design note 8): the classes' weights
 * over the survey, from the height workers' field, as R07's `class-map` surface draws them.
 *
 * @remarks
 * - **The map.** A worker answers it (`HeightWorkerPool.classMap`, the surface crate's
 *   `WorkerFields::class_map`): a texel a cell of {@link CLASS_MAP_LEVEL}, 64 × 64 a face, its
 *   weights `weights_at` at band level 0 at its centre, byte ÷ 255 (a texel of band level n is a
 *   cell of quadtree level n + 6, R10.T2; inside Design note 8's "level 4 or shallower", and of its
 *   "a few hundred kilobytes": 418 KB at 16 classes with R07's gutter). A texel the survey touches
 *   in part (a field finer than level 6) takes its surveyed share, its weights `weights_at` at its
 *   centre where the centre's field cell is surveyed and otherwise their mean at its surveyed
 *   cells' centres, times that share, so that `elsewhere` takes the rest of it; an unsurveyed texel
 *   is `null`, no weight ({@link classMapTexelsOf}, through R07's `packClassMap`).
 * - **The laws** are one per class, the body's (form, entry) pairs, at most 16: the class's
 *   photometry as the worker resolves it (R10.T10.a's c A_N, its law's L(0)) on its law's row law
 *   (`classRowLaw`), a full `PhotometricLaw` of its own; a class whose palette names no substance
 *   shades with `elsewhere`, as the lit terrain's law 0 takes such a slot ({@link classMapLawsOf}).
 *   `elsewhere` is R07's uniform `lawFor(p, q, template)`, the body's photometry's law. R07's disc
 *   scales every law's A for its figure (`oblateAlbedoScale`), the classes' included, as it did.
 * - **The maturity** (R10.T1.c's client half; decision-r10-t1c-client-scope §2.3) comes in the same
 *   answer where the body's header carries one: each texel's band-0 code and each class's band-0
 *   albedo nodes. The texels carry both ({@link classMapTexelsOf}), so that the disc draws each
 *   named class's A from its nodes at the texels' codes and the point folds them into its elements'
 *   shares; the class's law takes its node K − 1, its albedo at the body's background maturity
 *   ({@link classMapLawsOf}).
 * - **The normal map** (R10.T10.g; decision-r10-t16-leans §1, remedy 1) comes in the same answer:
 *   each surveyed cell of the field's own level with the shading normal the coarsest terrain gives
 *   its centre, packed as R07's disc reads it (`bodies/discNormals.ts`' `packNormalMap`,
 *   {@link normalMapTexelsOf}), so that the disc and the terrain share their cell-scale slopes at
 *   the hand-over. The surface carries it beside the class map, texture and texels.
 * - **The store** ({@link ClassMapStore}) owns each body's two textures, both under
 *   `coarse-field-gpu`: it asks the pool for the maps when a body is wanted and again after each
 *   block that surveys cells or brings what an incomplete map lacked (the pool's `onFieldChange`,
 *   the workers' coverage), a map being complete only where its classes and its normals both are;
 *   it remakes both textures after a device loss and releases both with the body.
 * - **The point's elements** (R10.T14.f): as it takes a map the store queues the point integral's
 *   elements for the frames' slices (`bodies/classMapPoint.ts`' `queueClassMapPoint`), the map it
 *   replaces standing in for the point until they are made, and cancels a map's unfinished build as
 *   it drops the map.
 */
import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";

import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import type { PhotometricLaw } from "../appearance/law";
import { cancelClassMapPoint, queueClassMapPoint } from "../bodies/classMapPoint";
import { type NormalMapTexels, normalMapTexture, packNormalMap } from "../bodies/discNormals";
import {
  type ClassMapTexel,
  type ClassMapTexels,
  classMapSurface,
  MAX_DISC_CLASSES,
  packClassMap,
} from "../bodies/discSurface";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { RenderEngine, TextureHandle } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import { fieldCellAt } from "./coverage";
import { MATURITY_NODES } from "./maturityNodes";
import { classPhotometryOf, classRowLaw, PHASE_LAW } from "./terrainMaterial";
import type { ClassMapAnswer, ClassMapMaturityAnswer } from "./workers/messages";
import { PHOTOMETRY_STRIDE } from "./workers/messages";
import type { FieldChange, HeightWorkerPool } from "./workers/pool";

/**
 * The class map's quadtree level: 2⁶ = 64 texels along a face's side, each the cell (f, 6, i, j),
 * its weights at band level 0 (the surface crate's `CLASS_MAP_LEVEL`).
 */
export const CLASS_MAP_LEVEL = 6;

/** The texels along a face's side of the class map, N = 2^{@link CLASS_MAP_LEVEL}. */
export const CLASS_MAP_FACE_TEXELS = 2 ** CLASS_MAP_LEVEL;

/** The largest weight byte. */
const BYTE_MAX = 255;

/** The law the texture's construction is given for each class until a frame gives `elsewhere`. */
const STAND_IN_LAW: PhotometricLaw = classRowLaw(PHASE_LAW.lambert, 0);

/** The classes an answer holds: two bytes each. */
function classCount(answer: ClassMapAnswer): number {
  return answer.classes.length / 2;
}

/**
 * The laws of an answered map's classes, one per class in its order: each class's photometry on its
 * law's row law, a distinct object, or `null` for a class whose palette names no substance (law
 * code 0), which shades with `elsewhere`. Where the map carries its maturity (R10.T1.c's client
 * half), a class's A is its node K − 1, T₀(m_body), the albedo at the body's background maturity
 * from which its texels' nodes draw it; its row and L(0) are its photometry's either way.
 *
 * @throws Error for photometry that is not five values a class, nodes that are not K × 3 a class,
 *   or a law code no law has.
 */
export function classMapLawsOf(answer: ClassMapAnswer): ReadonlyArray<PhotometricLaw | null> {
  const classes = classCount(answer);
  if (answer.photometry.length !== classes * PHOTOMETRY_STRIDE) {
    throw new Error(
      `a class map of ${String(classes)} classes has ${String(answer.photometry.length)} photometry values`,
    );
  }
  const nodes = answer.maturity === undefined ? null : displayNodesOf(answer.maturity, classes);
  return Array.from({ length: classes }, (_, k): PhotometricLaw | null => {
    const photometry = classPhotometryOf(answer.photometry, k);
    if (photometry === null) {
      return null;
    }
    // With a maturity, A is the class's albedo at the body's background maturity, its node K − 1
    // (R10.T1.c's client half), which its nodes' factors are taken against at the point.
    const at = 3 * (MATURITY_NODES * k + MATURITY_NODES - 1);
    const a: Rgb =
      nodes === null
        ? photometry.a
        : [nodes[at] ?? Number.NaN, nodes[at + 1] ?? Number.NaN, nodes[at + 2] ?? Number.NaN];
    return { ...classRowLaw(photometry.code, photometry.l0), a };
  });
}

/**
 * An answered map's nodes in the display channels, as its texels carry them: each class's K nodes,
 * node j of class c at 3 (K c + j), the module's B, V and R turned to r, g and b as
 * `classPhotometryOf` turns a law's A.
 *
 * @throws Error for nodes that are not K × 3 a class.
 */
function displayNodesOf(maturity: ClassMapMaturityAnswer, classes: number): Float32Array {
  const values = 3 * MATURITY_NODES * classes;
  if (maturity.nodes.length !== values) {
    throw new Error(
      `a class map of ${String(classes)} classes has ${String(values)} node values, not ${String(maturity.nodes.length)}`,
    );
  }
  const out = new Float32Array(values);
  for (let at = 0; at < values; at += 3) {
    out[at] = maturity.nodes[at + 2] ?? 0;
    out[at + 1] = maturity.nodes[at + 1] ?? 0;
    out[at + 2] = maturity.nodes[at] ?? 0;
  }
  return out;
}

/**
 * An answered map packed as R07's disc reads it (`packClassMap`, with its gutter): each texel the
 * survey touches its weights byte ÷ 255 times its surveyed share, every other texel `null`; `null`
 * for a map of no class, which draws as the uniform law. Where the answer carries its maturity
 * (R10.T1.c's client half), each texel's code and the classes' nodes in the display channels, a
 * class drawn from its nodes where its palette names a substance (a law of its own).
 *
 * @throws Error for an answer at another level than {@link CLASS_MAP_LEVEL}, of more than
 *   `MAX_DISC_CLASSES` classes, or whose texels, shares, weights and maturity codes do not agree in
 *   number, or whose nodes are not K × 3 a class.
 */
export function classMapTexelsOf(answer: ClassMapAnswer): ClassMapTexels | null {
  const classes = classCount(answer);
  if (answer.level !== CLASS_MAP_LEVEL) {
    throw new Error(
      `a class map is at level ${String(CLASS_MAP_LEVEL)}, not ${String(answer.level)}`,
    );
  }
  if (!Number.isInteger(classes) || classes > MAX_DISC_CLASSES) {
    throw new Error(`a class map holds at most ${String(MAX_DISC_CLASSES)} classes`);
  }
  const count = answer.texels.length;
  if (answer.fractions.length !== count || answer.weights.length !== count * classes) {
    throw new Error(
      `a class map's ${String(count)} texels have ${String(answer.fractions.length)} shares and ${String(answer.weights.length)} weights over ${String(classes)} classes`,
    );
  }
  if (classes === 0) {
    return null;
  }
  const n = CLASS_MAP_FACE_TEXELS;
  const byTexel = new Map<number, number>();
  for (let t = 0; t < count; t += 1) {
    const cell = fieldCellAt(CLASS_MAP_LEVEL, answer.texels[t] ?? 0);
    byTexel.set((cell.face * n + cell.j) * n + cell.i, t);
  }
  const indexOf = ({ face, i, j }: ClassMapTexel): number | undefined =>
    byTexel.get((face * n + j) * n + i);
  const weightsOf = (texel: ClassMapTexel): number[] | null => {
    const t = indexOf(texel);
    if (t === undefined) {
      return null;
    }
    const share = answer.fractions[t] ?? 0;
    return Array.from(
      { length: classes },
      (_, k) => ((answer.weights[t * classes + k] ?? 0) / BYTE_MAX) * share,
    );
  };
  const { maturity } = answer;
  if (maturity === undefined) {
    return packClassMap(n, classes, weightsOf);
  }
  if (maturity.codes.length !== count) {
    throw new Error(
      `a class map's ${String(count)} texels have ${String(maturity.codes.length)} maturity codes`,
    );
  }
  return packClassMap(n, classes, weightsOf, {
    codeOf: (texel) => maturity.codes[indexOf(texel) ?? -1] ?? Number.NaN,
    nodes: displayNodesOf(maturity, classes),
    drawn: Array.from(
      { length: classes },
      (_, k) => classPhotometryOf(answer.photometry, k) !== null,
    ),
  });
}

/**
 * An answered normal map packed as R07's disc reads it (`packNormalMap`, with its gutter, at the
 * field's level): each surveyed cell answered its pair, every other texel `NO_NORMAL`; `null` for a
 * map of no cell, under which the datum's normal shades everywhere.
 *
 * @throws Error for a level outside 5 to 8, pairs that are not two a cell, a cell past the level's,
 *   or a pair coordinate outside [−1, 1].
 */
export function normalMapTexelsOf(answer: ClassMapAnswer): NormalMapTexels | null {
  if (answer.normalCells.length === 0 && answer.normalPairs.length === 0) {
    return null;
  }
  return packNormalMap(answer.normalLevel, answer.normalCells, answer.normalPairs);
}

/**
 * What a frame's lit bodies take their class maps from (R10.T10.d): `litBodiesOf`'s optional
 * source, absent where no view feeds one, every disc then uniform.
 */
export interface ClassMapSource {
  /**
   * The class-map surface of `body`, its unsurveyed ground and its classes of no named substance
   * shading with `elsewhere`; `null` where it has none.
   */
  classMapOf(body: BodyIdHex, elsewhere: PhotometricLaw): ClassMapDiscSurface | null;
}

/** What the store asks of the height workers (`HeightWorkerPool`'s). */
export type ClassMapPool = Pick<HeightWorkerPool, "classMap" | "onFieldChange">;

/** A body's map as the store holds it. */
interface HeldMap {
  readonly texels: ClassMapTexels;
  readonly laws: ReadonlyArray<PhotometricLaw | null>;
  /** The normal map's texels, `null` for none (no surveyed cell answered). */
  readonly normals: NormalMapTexels | null;
  /** Whether every texel the survey touched, and every surveyed cell's normal, was answered. */
  readonly complete: boolean;
  /** Its texture, `null` while the device is lost. */
  texture: TextureHandle | null;
  /** The normal map's texture, `null` while the device is lost or where it has none. */
  normalTexture: TextureHandle | null;
  /** The surface last handed out, with its `elsewhere`; `null` after each upload. */
  surface: ClassMapDiscSurface | null;
  /**
   * The surface last handed out, kept across uploads: the point's stand-in once the map is
   * replaced, whose point integral reads its texels and laws alone (R10.T14.f).
   */
  handedOut: ClassMapDiscSurface | null;
}

/** A body the store is asked for. */
interface HeldBody {
  readonly detailSeed: DetailSeedHex;
  /** Whether a query is out, and whether a change came since it was posted. */
  asking: boolean;
  again: boolean;
  map: HeldMap | null;
}

/**
 * The bodies' class maps and normal maps on one engine, fed from a height-worker pool (R10.T10.d,
 * R10.T10.g): two textures a body under `coarse-field-gpu`, the class map's and the normal map's,
 * remade after a device loss and released with their body.
 *
 * @remarks
 * {@link ClassMapStore.want} asks the pool for a body's map, and the store asks again after each of
 * the body's blocks the workers take that surveys a cell, brings a cell while the map lacked one
 * (its classes or its normals), or starts the field afresh; an answer to an older query than the
 * body's latest is dropped. A map of no class, or none (no field held yet), leaves the body
 * uniform; a map of no normal leaves its surface without one. A query refused by the module is
 * reported once a body, with `console.error`, and asked again after the next block.
 */
export class ClassMapStore implements ClassMapSource {
  readonly #engine: Pick<
    RenderEngine,
    "createTexture" | "writeTexture" | "releaseTexture" | "onRestored"
  >;
  readonly #pool: ClassMapPool;
  readonly #name: string;
  readonly #bodies = new Map<BodyIdHex, HeldBody>();
  readonly #reported = new Set<BodyIdHex>();
  readonly #unsubscribe: ReadonlyArray<() => void>;
  #disposed = false;

  /**
   * Feeds from `pool` onto `engine`, its class maps' textures named `<name> <body>` and its normal
   * maps' `<name> normals <body>`.
   */
  constructor(
    engine: Pick<RenderEngine, "createTexture" | "writeTexture" | "releaseTexture" | "onRestored">,
    pool: ClassMapPool,
    name = "class map",
  ) {
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

  /**
   * Asks for `body`'s map with its detail seed (`BodyHooksDto.detail_seed`), now and after each
   * block that changes it; a body asked for again with the same seed is left as it is, and with
   * another starts afresh.
   */
  want(body: BodyIdHex, detailSeed: DetailSeedHex): void {
    this.#assertLive();
    const held = this.#bodies.get(body);
    if (held?.detailSeed === detailSeed) {
      return;
    }
    this.release(body);
    this.#bodies.set(body, { detailSeed, asking: false, again: false, map: null });
    this.#ask(body);
  }

  /** Drops `body`'s map and releases its textures; a body not asked for is no error. */
  release(body: BodyIdHex): void {
    const held = this.#bodies.get(body);
    if (held === undefined) {
      return;
    }
    this.#drop(held);
    this.#bodies.delete(body);
    this.#reported.delete(body);
  }

  classMapOf(body: BodyIdHex, elsewhere: PhotometricLaw): ClassMapDiscSurface | null {
    const map = this.#bodies.get(body)?.map ?? null;
    if (map === null || map.texture === null) {
      return null;
    }
    if (
      map.surface?.elsewhere !== elsewhere ||
      map.surface.weights !== map.texture ||
      (map.surface.normals?.texture ?? null) !== map.normalTexture
    ) {
      map.surface = {
        kind: "class-map",
        weights: map.texture,
        texels: map.texels,
        laws: map.laws.map((law) => law ?? elsewhere),
        elsewhere,
        normals:
          map.normals === null || map.normalTexture === null
            ? null
            : { texture: map.normalTexture, texels: map.normals },
      };
      map.handedOut = map.surface;
    }
    return map.surface;
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
      throw new Error("the class-map store was disposed");
    }
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
      console.error(`the class map of ${body} could not be asked again:`, error);
    });
  }

  /**
   * Asks the pool for `body`'s map, takes the answer if `held` is still the body's, and asks again
   * if a change came meanwhile.
   */
  async #query(body: BodyIdHex, held: HeldBody): Promise<void> {
    try {
      const answer = await this.#pool.classMap(body, held.detailSeed);
      if (this.#bodies.get(body) === held && !this.#disposed) {
        this.#take(body, held, answer);
      }
    } catch (error: unknown) {
      if (!this.#reported.has(body)) {
        this.#reported.add(body);
        console.error(`the class map of ${body} could not be made:`, error);
      }
    }
    held.asking = false;
    if (held.again && this.#bodies.get(body) === held) {
      this.#ask(body);
    }
  }

  /**
   * Takes an answer for `body` in place of the map it held, and queues the new map's point
   * elements for the frames' slices, the old map's point standing in meanwhile (R10.T14.f).
   */
  #take(body: BodyIdHex, held: HeldBody, answer: ClassMapAnswer | null): void {
    if (answer === null) {
      return;
    }
    const texels = classMapTexelsOf(answer);
    const normals = normalMapTexelsOf(answer);
    if (texels !== null) {
      // Before the old map is dropped, which would cancel its build and forget its stand-in.
      const previous = held.map;
      queueClassMapPoint(
        texels,
        normals,
        previous === null ? null : { texels: previous.texels, surface: previous.handedOut },
      );
    }
    this.#drop(held);
    if (texels === null) {
      return;
    }
    const map: HeldMap = {
      texels,
      laws: classMapLawsOf(answer),
      normals,
      complete: answer.complete && answer.normalsComplete,
      texture: null,
      normalTexture: null,
      surface: null,
      handedOut: null,
    };
    held.map = map;
    this.#upload(body, map);
  }

  /**
   * Makes `map`'s textures, or leaves them `null` while the device is lost, for the restore: both or
   * neither, the class map's released again where the normal map's cannot be made.
   */
  #upload(body: BodyIdHex, map: HeldMap): void {
    try {
      // R07's construction; the laws it is given stand in, `classMapOf` giving each frame's.
      const placeholder = map.laws.map((law) => law ?? STAND_IN_LAW);
      const weights = classMapSurface(
        this.#engine,
        `${this.#name} ${body}`,
        map.texels,
        placeholder,
        STAND_IN_LAW,
      ).weights;
      try {
        map.normalTexture =
          map.normals === null
            ? null
            : normalMapTexture(this.#engine, `${this.#name} normals ${body}`, map.normals).texture;
      } catch (error: unknown) {
        this.#engine.releaseTexture(weights);
        throw error;
      }
      map.texture = weights;
    } catch (error: unknown) {
      if (!(error instanceof EngineUnavailable)) {
        throw error;
      }
    }
    map.surface = null;
  }

  /** Releases `held`'s textures, cancels its point's unfinished build and forgets its map. */
  #drop(held: HeldBody): void {
    if (held.map !== null) {
      this.#release(held.map);
      cancelClassMapPoint(held.map.texels);
    }
    held.map = null;
  }

  /** Releases `map`'s textures, those it holds, and forgets them. */
  #release(map: HeldMap): void {
    if (map.texture !== null) {
      this.#engine.releaseTexture(map.texture);
      map.texture = null;
    }
    if (map.normalTexture !== null) {
      this.#engine.releaseTexture(map.normalTexture);
      map.normalTexture = null;
    }
  }

  /** Asks again for a body whose block surveyed cells, brought what its map lacked, or started. */
  #changed(change: FieldChange): void {
    const held = this.#bodies.get(change.body);
    if (held === undefined) {
      return;
    }
    const incomplete = held.map !== null && !held.map.complete;
    if (
      change.sequence === 0 ||
      change.newlySurveyed.length > 0 ||
      (incomplete && change.newlyHeld.length > 0) ||
      held.map === null
    ) {
      this.#ask(change.body);
    }
  }

  /** Makes every map's textures again on the restored device. */
  #restored(): void {
    for (const [body, held] of this.#bodies) {
      const map = held.map;
      if (map === null) {
        continue;
      }
      // The lost device's handles: the resilient engine drops their release.
      this.#release(map);
      this.#upload(body, map);
    }
  }
}
