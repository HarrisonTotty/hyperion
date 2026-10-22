/**
 * What a height worker answers, as a pure function of the loaded surface module and a request
 * (plan R05, T10.b, Design note 11; plan R10, T6.b).
 *
 * @remarks
 * The worker (`height.worker.ts`) holds no logic of its own: it loads the module and hands each
 * message here, so a Node-environment test drives the same answers from the module's bytes, as
 * R04's `wasm/handleRequest.ts` does for the probe worker. This file never imports
 * `generated/surface/` (only workers and tests may, `surfaceImports.test.ts`); the module's
 * exports arrive through {@link HeightModule}, whose enum values the test pins to the generated
 * glue's.
 *
 * The bake's layout is `hyperion_surface`'s `src/wasm.rs` module documentation: heights and morph
 * targets interleaved per vertex, offsets q₀ then q₁ per vertex on `baked-offsets`, and octahedral
 * normal pairs, which are packed here into half floats (`rg16float` on the GPU, Design note 5).
 * Each getter copies its array out of the module's linear memory into a new typed array whose
 * buffer the worker transfers; the module's own memory cannot be transferred.
 *
 * A body's field lives in the module (R10.T6.b): {@link WorkerFields} hands each payload block to
 * the module's `SurfaceFields`, which decodes it (`wire::decode_block`) into the body's
 * `PartialField` (`PartialField::insert`), and bakes, reads and classifies over it with the body's
 * detail seed. A stand-in field, the descent spike's, is held here undecoded, since the test planet
 * reads no field. A view's terrain readout is one query of the module (R10.T12,
 * {@link WorkerFields.groundReadout}): the line of sight, the reading and the material at its foot,
 * and the ground's two properties, over one synthesiser.
 */

import { type BodyIdHex, type DetailSeedHex, hexToU64, parseBodyId } from "@hyperion/protocol";

import { patchKeyString, type Face, type PatchKey, unreachable } from "../patchKey";
import { vec3 } from "../../../geometry/vec3";
import type {
  BakedPatch,
  BakeSettings,
  ClassMapAnswer,
  FacePoint,
  FieldBlockAnswer,
  FieldForm,
  GroundReadoutQuery,
  HeightSourceSetting,
  HeightWorkerReply,
  HeightWorkerRequest,
  SurfaceGroundReadout,
  SurfaceLineOfSight,
  SurfaceMaterial,
  SurfaceReading,
  SurfaceSlantRange,
  TerrainLayers,
  TerrainMaturity,
  TestPlanetRidges,
  WeightsLayerSetting,
} from "./messages";
import { bakeTransferables, blockTransferables, classMapTransferables } from "./messages";

/** The version of the test planet this client's code was written against, the surface crate's
 * `TEST_PLANET_VERSION` (pinned to it by `heightWasm.test.ts`). A module that bakes another
 * version is stale: its heights are not the ones the client's tests and goldens describe. */
export const EXPECTED_TEST_PLANET_VERSION = 2;

/** The module's `VertexPath` enum values (`src/wasm.rs`'s `JsVertexPath`). */
export const WASM_VERTEX_PATH = { "baked-offsets": 0, "face-differences": 1 } as const;

/** The module's `NormalScale` enum values (`src/wasm.rs`'s `JsNormalScale`). */
export const WASM_NORMAL_SCALE = { mesh: 0, double: 1 } as const;

/** The module's `Ridges` enum values (`src/wasm.rs`'s `JsRidges`). */
export const WASM_RIDGES = { off: 0, on: 1 } as const satisfies Record<TestPlanetRidges, number>;

/** The module's `WeightsLayer` enum values (`src/wasm.rs`'s `JsWeightsLayer`). */
export const WASM_WEIGHTS_LAYER = { baked: 0, skipped: 1 } as const satisfies Record<
  WeightsLayerSetting,
  number
>;

/**
 * The module's `HorizonLayer` enum values (`src/wasm.rs`'s `JsHorizonLayer`), by the body's weights
 * setting: the lit style bakes the horizon map with its weights, and the wireframe skips both
 * (R10.T8.b).
 */
export const WASM_HORIZON_LAYER = { baked: 0, skipped: 1 } as const satisfies Record<
  WeightsLayerSetting,
  number
>;

/** The module's `LineOfSightKind` enum values (`src/wasm.rs`'s `JsLineOfSightKind`). */
export const WASM_LINE_OF_SIGHT = { ground: 0, "no-ground": 1, "not-surveyed": 2 } as const;

/** The module's `BakedPatch`: getters that copy each array out, and its own memory to free. */
export interface WasmBakedPatch {
  heights(): Float32Array;
  offsets(): Float32Array | undefined;
  normals(): Float32Array;
  origin(): Float64Array;
  heightRangeM(): Float32Array;
  readonly boundingRadiusM: number;
  readonly originHeightM: number;
  readonly skirtDepthM: number;
  /** Releases the bake's memory inside the module. */
  free(): void;
}

/** The module's `BakedTerrain`: a body's bake, {@link WasmBakedPatch}'s arrays and R10's layers. */
export interface WasmBakedTerrain extends WasmBakedPatch {
  palette(): Uint8Array;
  weights(): Uint8Array;
  surveyMask(): Uint8Array;
  unsurveyedCells(): Uint32Array;
  horizonNear(): Uint32Array;
  horizonFar(): Uint32Array;
  horizonRefusedCells(): Uint32Array;
  /**
   * Each vertex's two maturity codes, at the patch's band level then its parent's (R10.T1.c's
   * client half; `TerrainMaturity.codes`); empty without a maturity layer.
   */
  maturity(): Uint16Array;
  readonly boundM: number;
  readonly sigmaM: number;
  readonly complete: boolean;
  readonly horizonComplete: boolean;
}

/** The module's `FieldBlock`: what a block taken changed, and the body's level table after it. */
export interface WasmFieldBlock {
  newlySurveyed(): Uint32Array;
  newlyHeld(): Uint32Array;
  levelTable(): Float64Array;
  readonly fieldLevel: number;
  free(): void;
}

/** The module's `Reading`, `surveyed_reading`'s answer. */
export interface WasmReading {
  readonly elevationM: number;
  readonly elevationSigmaM: number;
  readonly slopeRmsRad: number;
  readonly slopeSigmaRad: number;
  readonly slopeBaselineM: number;
  readonly resolutionM: number;
  readonly resolved: boolean;
  free(): void;
}

/** The module's `LineOfSightAnswer`, `slant_range`'s. */
export interface WasmLineOfSight {
  readonly kind: number;
  readonly rangeM: number | undefined;
  readonly sigmaM: number | undefined;
  readonly grazing: boolean;
  readonly resolved: boolean;
  hitM(): Float64Array | undefined;
  readonly atFace: number | undefined;
  readonly atU: number | undefined;
  readonly atV: number | undefined;
  free(): void;
}

/** The module's `GroundAnswer`, `material_at`'s with its properties. */
export interface WasmGround {
  readonly classCode: number;
  readonly entry: number | undefined;
  normalAlbedoBvr(): Float64Array | undefined;
  readonly phaseLaw: number | undefined;
  readonly limbParameter: number | undefined;
  readonly grainDensityKgM3: number | undefined;
  readonly transitionK: number | undefined;
  readonly mechanics: number | undefined;
  free(): void;
}

/** The module's `ClassMapAnswer`, `WorkerFields::class_map`'s (R10.T10.d): its getters copy out. */
export interface WasmClassMap {
  readonly level: number;
  readonly fieldLevel: number;
  classes(): Uint8Array;
  photometry(): Float32Array;
  texels(): Uint32Array;
  fractions(): Float32Array;
  weights(): Uint8Array;
  /**
   * Each texel's maturity code at band level 0, one a texel in `texels()`' order (R10.T1.c's
   * client half; `ClassMapMaturityAnswer.codes`); empty where the header carries no maturity.
   */
  maturity(): Uint16Array;
  /** Each class's band-0 albedo nodes, K × 3 a class, B, V, R (`ClassMapMaturityAnswer.nodes`). */
  nodes(): Float32Array;
  readonly complete: boolean;
  free(): void;
}

/**
 * The module's `NormalMapAnswer`, `WorkerFields::normal_map`'s (R10.T10.g): the field's level, each
 * surveyed cell answered and its octahedral pair, two `f32` a cell; its getters copy out.
 */
export interface WasmNormalMap {
  readonly level: number;
  cells(): Uint32Array;
  pairs(): Float32Array;
  readonly complete: boolean;
  free(): void;
}

/**
 * The module's `GroundReadoutAnswer`, `WorkerFields::ground_readout`'s (R10.T12): its `kind` a
 * {@link WASM_LINE_OF_SIGHT} value, and on the ground the sight's, the reading's, the material's and
 * the properties' getters, `undefined` (or `false`) off it and where not computed.
 */
export interface WasmGroundReadout {
  readonly kind: number;
  readonly rangeM: number | undefined;
  readonly rangeSigmaM: number | undefined;
  readonly grazing: boolean;
  readonly rangeResolved: boolean;
  hitM(): Float64Array | undefined;
  readonly atFace: number | undefined;
  readonly atU: number | undefined;
  readonly atV: number | undefined;
  readonly elevationM: number | undefined;
  readonly elevationSigmaM: number | undefined;
  readonly slopeRmsRad: number | undefined;
  readonly slopeSigmaRad: number | undefined;
  readonly slopeBaselineM: number | undefined;
  readonly resolutionM: number | undefined;
  readonly readingResolved: boolean;
  readonly classCode: number | undefined;
  readonly entry: number | undefined;
  readonly bearingCapacityPa: number | undefined;
  readonly capacitySlopeRad: number | undefined;
  readonly padFriction: number | undefined;
  readonly frictionOnTemperature: boolean;
  readonly temperatureK: number | undefined;
  free(): void;
}

/**
 * The module's `SurfaceFields`: a client's field a body (`src/wasm.rs`'s module documentation). A
 * body is its system's raw ID and its index; a seed is the body's detail seed.
 */
export interface WasmSurfaceFields {
  /** @throws Error naming why the block was refused; the field is then as it was. */
  fieldInit(system: bigint, body: number, block: Uint8Array): WasmFieldBlock;
  /** @throws Error naming why the block was refused; the field is then as it was. */
  fieldChunk(system: bigint, body: number, block: Uint8Array): WasmFieldBlock;
  fieldRelease(system: bigint, body: number): boolean;
  readonly heldBytes: number;
  bake(
    system: bigint,
    body: number,
    seed: bigint,
    face: number,
    level: number,
    i: number,
    j: number,
    vertexPath: number,
    normals: number,
    weights: number,
    horizon: number,
    skirtM: number,
  ): WasmBakedTerrain | undefined;
  reading(
    system: bigint,
    body: number,
    seed: bigint,
    face: number,
    u: number,
    v: number,
    slopeBaselineM: number,
  ): WasmReading | undefined;
  slantRange(
    system: bigint,
    body: number,
    seed: bigint,
    ox: number,
    oy: number,
    oz: number,
    dx: number,
    dy: number,
    dz: number,
    maxM: number,
  ): WasmLineOfSight;
  material(
    system: bigint,
    body: number,
    seed: bigint,
    face: number,
    u: number,
    v: number,
  ): WasmGround | undefined;
  /**
   * A view's terrain readout along a line of sight (R10.T12); `NotSurveyed` where no field is held.
   *
   * @throws Error for a line of sight `slantRange` refuses, a slope baseline other than 2, 5 or
   *   100 m, a pad width that is not finite and positive, or a season's anomaly given and not finite.
   */
  groundReadout(
    system: bigint,
    body: number,
    seed: bigint,
    ox: number,
    oy: number,
    oz: number,
    dx: number,
    dy: number,
    dz: number,
    maxM: number,
    slopeBaselineM: number,
    padWidthM: number,
    seasonMeanAnomalyRad: number | undefined,
  ): WasmGroundReadout;
  /**
   * The photometry of each slot of a bake's palette (R10.T10.b), 8 × 5; `undefined` where no
   * field of the body is held.
   *
   * @throws Error for a palette the module cannot read (another length, or a slot naming no class
   *   or no entry of the body's palette).
   */
  photometry(system: bigint, body: number, palette: Uint8Array): Float32Array | undefined;
  /**
   * The albedo nodes of each slot of a bake's palette at band level `level` and at its parent's
   * (R10.T1.c's client half; `TerrainMaturity.nodes`), 8 × 2 × K × 3; each node repeating the
   * slot's photometry albedo where the body's header carries no maturity; `undefined` where no
   * field of the body is held.
   *
   * @throws Error as {@link photometry} does, and for a level above 24.
   */
  maturityNodes(
    system: bigint,
    body: number,
    palette: Uint8Array,
    level: number,
  ): Float32Array | undefined;
  /**
   * The body's coarse class map for R07's disc (R10.T10.d); `undefined` where no field of the body
   * is held.
   */
  classMap(system: bigint, body: number, seed: bigint): WasmClassMap | undefined;
  /**
   * The body's normal map for R07's disc (R10.T10.g); `undefined` where no field of the body is
   * held.
   */
  normalMap(system: bigint, body: number, seed: bigint): WasmNormalMap | undefined;
  free(): void;
}

/** The part of the surface module a height worker calls. */
export interface HeightModule {
  readonly bakePatch: (
    face: number,
    level: number,
    i: number,
    j: number,
    vertexPath: number,
    normals: number,
    ridges: number,
    skirtM: number,
  ) => WasmBakedPatch;
  readonly testPlanetVersion: () => number;
  /** Makes the worker's fields, `new SurfaceFields()`, once. */
  readonly createFields: () => WasmSurfaceFields;
}

/** Why a loaded module cannot serve: it bakes another test planet than this client's. */
export function staleModuleMessage(module: Pick<HeightModule, "testPlanetVersion">): string | null {
  const version = module.testPlanetVersion();
  return version === EXPECTED_TEST_PLANET_VERSION
    ? null
    : `the surface module bakes test planet version ${String(version)}, ` +
        `but this client expects ${String(EXPECTED_TEST_PLANET_VERSION)}`;
}

/** The octahedral pairs `pairs`, rounded to half floats (to nearest, ties to even). */
export function packNormals(pairs: Float32Array): Float16Array {
  return Float16Array.from(pairs);
}

/** R05's arrays of a module bake, copied out; the caller frees the bake. */
function patchOf(
  baked: WasmBakedPatch,
  key: PatchKey,
  generation: number,
  layers: TerrainLayers | null,
): BakedPatch {
  const [ox, oy, oz] = baked.origin();
  const [low, high] = baked.heightRangeM();
  if (ox === undefined || oy === undefined || oz === undefined) {
    throw new Error("the module's bake has no three-component origin");
  }
  if (low === undefined || high === undefined) {
    throw new Error("the module's bake has no two-ended height range");
  }
  return {
    key,
    generation,
    originM: vec3(ox, oy, oz),
    heights: baked.heights(),
    offsets: baked.offsets() ?? null,
    normals: packNormals(baked.normals()),
    heightRangeM: [low, high],
    boundingRadiusM: baked.boundingRadiusM,
    originHeightM: baked.originHeightM,
    skirtDepthM: baked.skirtDepthM,
    layers,
  };
}

/**
 * Bakes the test planet's `key` with `ridges` and the settings' path and normals (no extra skirt
 * margin) and copies the result out of the module.
 *
 * @throws Whatever the module throws for a key out of range, after freeing nothing it allocated.
 */
export function bakeKey(
  module: HeightModule,
  key: PatchKey,
  generation: number,
  settings: BakeSettings,
  ridges: TestPlanetRidges,
): BakedPatch {
  const baked = module.bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    WASM_VERTEX_PATH[settings.vertexPath],
    WASM_NORMAL_SCALE[settings.normals],
    WASM_RIDGES[ridges],
    0,
  );
  try {
    return patchOf(baked, key, generation, null);
  } finally {
    baked.free();
  }
}

/**
 * The blocks of stand-in fields, held undecoded in JavaScript (R10.T7's), for a body whose
 * height source reads no field: the descent spike's synthetic field on the test planet.
 *
 * @remarks
 * Nothing here is decoded or checked beyond its order: a block of a body with no `field-init`
 * before it is refused, as a payload's is.
 */
export class StandInFields {
  readonly #bodies = new Map<BodyIdHex, ArrayBuffer[]>();
  #bytes = 0;

  /** What is held of every body's field, bytes. */
  get heldBytes(): number {
    return this.#bytes;
  }

  /** The bodies held, in the order their fields were started. */
  get bodies(): ReadonlyArray<BodyIdHex> {
    return [...this.#bodies.keys()];
  }

  /** Whether `body`'s field is held. */
  has(body: BodyIdHex): boolean {
    return this.#bodies.has(body);
  }

  /** Starts `body`'s field from `block`, dropping any field of `body` held. */
  init(body: BodyIdHex, block: ArrayBuffer): void {
    this.release(body);
    this.#bodies.set(body, [block]);
    this.#bytes += block.byteLength;
  }

  /** Adds `block` to `body`'s field; `false`, holding nothing more, if `body` has none started. */
  chunk(body: BodyIdHex, block: ArrayBuffer): boolean {
    const blocks = this.#bodies.get(body);
    if (blocks === undefined) {
      return false;
    }
    blocks.push(block);
    this.#bytes += block.byteLength;
    return true;
  }

  /** Drops `body`'s field, if one is held. */
  release(body: BodyIdHex): void {
    for (const block of this.#bodies.get(body) ?? []) {
      this.#bytes -= block.byteLength;
    }
    this.#bodies.delete(body);
  }
}

/** A block posted to a worker's fields: taken, with what a payload's block changed, or refused. */
export type FieldTake =
  | { readonly kind: "taken"; readonly block: FieldBlockAnswer | null }
  | { readonly kind: "refused"; readonly message: string };

/** A body's ID as the module takes it: its system's raw ID and its index. */
function bodyArgs(body: BodyIdHex): readonly [bigint, number] {
  const { system, bodyIndex } = parseBodyId(body);
  return [hexToU64(system), bodyIndex];
}

/** Whether `error` is a trap of the module (a Rust panic), which may leave the instance broken. */
function isTrap(error: unknown): error is WebAssembly.RuntimeError {
  return error instanceof WebAssembly.RuntimeError;
}

/** `error`'s message. */
function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The coarse fields a height worker holds (R10.T6.b): each body's R09 payload in the module's
 * `SurfaceFields`, a `PartialField` a body, and each stand-in in a {@link StandInFields}.
 *
 * @remarks
 * A field's form is set by its `field-init`: its later blocks are decoded, or held, as it was.
 * Whatever the module throws for a refused block is answered as a refusal; a trap is rethrown, so
 * that the worker raises it and the pool replaces the worker.
 */
export class WorkerFields {
  readonly #module: WasmSurfaceFields;
  readonly #standIns = new StandInFields();
  /** Each body held and its form, in the order the fields were started. */
  readonly #forms = new Map<BodyIdHex, FieldForm>();
  /** Each payload field's level, 5 to 8, as its blocks' answers state it (R10.T10.c). */
  readonly #levels = new Map<BodyIdHex, number>();

  constructor(module: HeightModule) {
    this.#module = module.createFields();
  }

  /** What is held of every body's field, bytes: the module's fields' heap and the stand-ins. */
  get heldBytes(): number {
    return this.#module.heldBytes + this.#standIns.heldBytes;
  }

  /** The bodies held, in the order their fields were started. */
  get bodies(): ReadonlyArray<BodyIdHex> {
    return [...this.#forms.keys()];
  }

  /**
   * Starts `body`'s field from `block` in `form`, replacing any field of `body` held; a refused
   * payload block keeps the field held as it was.
   */
  init(body: BodyIdHex, block: ArrayBuffer, form: FieldForm): FieldTake {
    if (form === "stand-in") {
      this.release(body);
      this.#standIns.init(body, block);
      this.#forms.set(body, form);
      return { kind: "taken", block: null };
    }
    const taken = this.#take(() =>
      this.#module.fieldInit(...bodyArgs(body), new Uint8Array(block)),
    );
    if (taken.kind === "taken") {
      this.#standIns.release(body);
      // Deleted first, so that the body moves to the end of the order of starts.
      this.#forms.delete(body);
      this.#forms.set(body, "payload");
      this.#noteLevel(body, taken);
    }
    return taken;
  }

  /** Adds `block` to `body`'s field; refused if no field of `body` was started. */
  chunk(body: BodyIdHex, block: ArrayBuffer): FieldTake {
    const form = this.#forms.get(body);
    if (form === undefined) {
      return { kind: "refused", message: `a block of ${body} arrived before its field-init` };
    }
    switch (form) {
      case "stand-in":
        this.#standIns.chunk(body, block);
        return { kind: "taken", block: null };
      case "payload": {
        const taken = this.#take(() =>
          this.#module.fieldChunk(...bodyArgs(body), new Uint8Array(block)),
        );
        this.#noteLevel(body, taken);
        return taken;
      }
    }
    return unreachable(form);
  }

  /** Keeps the level a taken block of `body`'s payload states, which its bakes carry. */
  #noteLevel(body: BodyIdHex, taken: FieldTake): void {
    if (taken.kind === "taken" && taken.block !== null) {
      this.#levels.set(body, taken.block.fieldLevel);
    }
  }

  /** Drops `body`'s field, if one is held. */
  release(body: BodyIdHex): void {
    switch (this.#forms.get(body)) {
      case undefined:
        return;
      case "stand-in":
        this.#standIns.release(body);
        break;
      case "payload":
        this.#module.fieldRelease(...bodyArgs(body));
        break;
    }
    this.#forms.delete(body);
    this.#levels.delete(body);
  }

  /**
   * Bakes `key` of `source`'s body over its field (no extra skirt margin), with R10's layers, the
   * horizon map with the weights ({@link WASM_HORIZON_LAYER}), and, where the bake made a maturity
   * layer, its codes with the palette's nodes at the key's level (`TerrainLayers.maturity`,
   * R10.T1.c's client half); `null` where the field cannot make it yet: none is held, or a
   * surveyed vertex reads a cell still to come.
   *
   * @throws Whatever the module throws for a key out of range.
   */
  bake(
    key: PatchKey,
    generation: number,
    settings: BakeSettings,
    source: Extract<HeightSourceSetting, { kind: "body" }>,
  ): BakedPatch | null {
    const baked = this.#module.bake(
      ...bodyArgs(source.body),
      hexToU64(source.detailSeed),
      key.face,
      key.level,
      key.i,
      key.j,
      WASM_VERTEX_PATH[settings.vertexPath],
      WASM_NORMAL_SCALE[settings.normals],
      WASM_WEIGHTS_LAYER[source.weights],
      WASM_HORIZON_LAYER[source.weights],
      0,
    );
    if (baked === undefined) {
      return null;
    }
    try {
      const fieldLevel = this.#levels.get(source.body);
      if (fieldLevel === undefined) {
        throw new Error(`${source.body} was baked over a field whose level no block stated`);
      }
      const palette = baked.palette();
      // The palette's photometry from the field the bake was made over, in the same message.
      const photometry = this.#module.photometry(...bodyArgs(source.body), palette);
      if (photometry === undefined) {
        throw new Error(`the field of ${source.body} went before its bake's photometry`);
      }
      // The maturity layer's codes, and only with them its palette's nodes at the patch's level
      // and its parent's (R10.T1.c's client half), from the same field.
      const codes = baked.maturity();
      let maturity: TerrainMaturity | null = null;
      if (codes.length > 0) {
        const nodes = this.#module.maturityNodes(...bodyArgs(source.body), palette, key.level);
        if (nodes === undefined) {
          throw new Error(`the field of ${source.body} went before its bake's maturity nodes`);
        }
        maturity = { codes, nodes };
      }
      return patchOf(baked, key, generation, {
        palette,
        weights: baked.weights(),
        surveyMask: baked.surveyMask(),
        fieldLevel,
        boundM: baked.boundM,
        sigmaM: baked.sigmaM,
        complete: baked.complete,
        unsurveyedCells: baked.unsurveyedCells(),
        photometry,
        horizonNear: baked.horizonNear(),
        horizonFar: baked.horizonFar(),
        horizonComplete: baked.horizonComplete,
        horizonRefusedCells: baked.horizonRefusedCells(),
        ...(maturity === null ? {} : { maturity }),
      });
    } finally {
      baked.free();
    }
  }

  /**
   * The reading at `at` on `body`, its slope over `slopeBaselineM` metres (R10.T5's
   * `surveyed_reading`); `null` where the ground is not surveyed or no field is held: the
   * readout's `NOT SURVEYED`.
   *
   * @throws Error if `at` is not a point of a face or the baseline is not finite and positive.
   */
  reading(
    body: BodyIdHex,
    detailSeed: DetailSeedHex,
    at: FacePoint,
    slopeBaselineM: number,
  ): SurfaceReading | null {
    const reading = this.#module.reading(
      ...bodyArgs(body),
      hexToU64(detailSeed),
      at.face,
      at.u,
      at.v,
      slopeBaselineM,
    );
    if (reading === undefined) {
      return null;
    }
    try {
      return {
        elevationM: reading.elevationM,
        elevationSigmaM: reading.elevationSigmaM,
        slopeRmsRad: reading.slopeRmsRad,
        slopeSigmaRad: reading.slopeSigmaRad,
        slopeBaselineM: reading.slopeBaselineM,
        resolutionM: reading.resolutionM,
        resolved: reading.resolved,
      };
    } finally {
      reading.free();
    }
  }

  /**
   * Where the line of sight from `originM` along `direction`, body-fixed, meets `body`'s ground
   * within `maxM` metres (R10.T5's `slant_range`): `not-surveyed` where no field is held.
   *
   * @throws Error if the origin is not finite or is the body's centre, the direction is zero or not
   * finite, or the reach is not finite and non-negative.
   */
  slantRange(
    body: BodyIdHex,
    detailSeed: DetailSeedHex,
    originM: readonly [number, number, number],
    direction: readonly [number, number, number],
    maxM: number,
  ): SurfaceLineOfSight {
    const sight = this.#module.slantRange(
      ...bodyArgs(body),
      hexToU64(detailSeed),
      ...originM,
      ...direction,
      maxM,
    );
    try {
      return lineOfSightOf(sight);
    } finally {
      sight.free();
    }
  }

  /**
   * The material at `at` on `body` with its properties (R10.T1.b's `material_at`, T1.a's
   * `properties`); `null` where the ground is not surveyed or no field is held.
   *
   * @throws Error if `at` is not a point of a face.
   */
  material(body: BodyIdHex, detailSeed: DetailSeedHex, at: FacePoint): SurfaceMaterial | null {
    const ground = this.#module.material(
      ...bodyArgs(body),
      hexToU64(detailSeed),
      at.face,
      at.u,
      at.v,
    );
    if (ground === undefined) {
      return null;
    }
    try {
      return materialOf(ground);
    } finally {
      ground.free();
    }
  }

  /**
   * A view's terrain readout of `body` where `query`'s line of sight meets its ground (R10.T12,
   * `WorkerFields::ground_readout`), copied out: `not-surveyed` where no payload field of it is held
   * (none, or a stand-in's, which holds no ground), as the module answers one it holds no field of.
   *
   * @throws Error naming the module's refusal of an argument out of its range (the line of sight,
   *   a baseline other than 2, 5 or 100 m, the pad width, or a season that is not finite).
   */
  groundReadout(
    body: BodyIdHex,
    detailSeed: DetailSeedHex,
    query: GroundReadoutQuery,
  ): SurfaceGroundReadout {
    const { originM, direction } = query;
    const readout = this.#module.groundReadout(
      ...bodyArgs(body),
      hexToU64(detailSeed),
      originM.x,
      originM.y,
      originM.z,
      direction.x,
      direction.y,
      direction.z,
      query.maxM,
      query.slopeBaselineM,
      query.padWidthM,
      query.seasonMeanAnomalyRad,
    );
    try {
      return groundReadoutOf(readout);
    } finally {
      readout.free();
    }
  }

  /**
   * `body`'s coarse class map for R07's disc over its field, with its detail seed (R10.T10.d,
   * `WorkerFields::class_map`), and its normal map (R10.T10.g, `WorkerFields::normal_map`), its
   * pairs rounded to half floats as a bake's are ({@link packNormals}), with the texels' maturity
   * and the classes' nodes where the header carries a maturity (R10.T1.c's client half), copied
   * out; `null` where no payload field of it is held (none, or a stand-in's, which holds no ground).
   *
   * @throws Error where the module answers a class map but no normal map, which holds where the
   *   other does (a broken module).
   */
  classMap(body: BodyIdHex, detailSeed: DetailSeedHex): ClassMapAnswer | null {
    if (this.#forms.get(body) !== "payload") {
      return null;
    }
    const args = [...bodyArgs(body), hexToU64(detailSeed)] as const;
    const map = this.#module.classMap(...args);
    if (map === undefined) {
      return null;
    }
    let normals: WasmNormalMap | undefined;
    try {
      normals = this.#module.normalMap(...args);
      if (normals === undefined) {
        throw new Error(`the module answered ${body}'s class map but no normal map`);
      }
      // The texels' maturity, and only with it the classes' nodes (R10.T1.c's client half).
      const codes = map.maturity();
      return {
        level: map.level,
        fieldLevel: map.fieldLevel,
        classes: map.classes(),
        photometry: map.photometry(),
        texels: map.texels(),
        fractions: map.fractions(),
        weights: map.weights(),
        complete: map.complete,
        normalLevel: normals.level,
        normalCells: normals.cells(),
        normalPairs: packNormals(normals.pairs()),
        normalsComplete: normals.complete,
        ...(codes.length === 0 ? {} : { maturity: { codes, nodes: map.nodes() } }),
      };
    } finally {
      map.free();
      normals?.free();
    }
  }

  /** Frees the module's fields; the object cannot be used afterwards. */
  free(): void {
    this.#module.free();
  }

  /** Takes a block into the module, answering a refusal as one and rethrowing a trap. */
  #take(take: () => WasmFieldBlock): FieldTake {
    let block: WasmFieldBlock;
    try {
      block = take();
    } catch (error: unknown) {
      if (isTrap(error)) {
        throw error;
      }
      return { kind: "refused", message: messageOf(error) };
    }
    try {
      return {
        kind: "taken",
        block: {
          fieldLevel: block.fieldLevel,
          newlySurveyed: block.newlySurveyed(),
          newlyHeld: block.newlyHeld(),
          levels: block.levelTable(),
        },
      };
    } finally {
      block.free();
    }
  }
}

/** A face index the module answered, checked. */
function faceOf(index: number | undefined): Face {
  if (index === 0 || index === 1 || index === 2 || index === 3 || index === 4 || index === 5) {
    return index;
  }
  throw new Error(`the module answered no face, ${String(index)}`);
}

/** A slant range as the module's answers give it, by the line of sight's getters' names. */
interface WasmSlantRange {
  readonly rangeM: number | undefined;
  readonly sigmaM: number | undefined;
  readonly resolved: boolean;
  readonly hit: Float64Array | undefined;
  readonly atFace: number | undefined;
  readonly atU: number | undefined;
  readonly atV: number | undefined;
}

/** A slant range the module answered on the ground, copied out. */
function slantRangeOf(sight: WasmSlantRange): SurfaceSlantRange {
  const [x, y, z] = sight.hit ?? [];
  const { rangeM, atU, atV } = sight;
  if (
    x === undefined ||
    y === undefined ||
    z === undefined ||
    rangeM === undefined ||
    atU === undefined ||
    atV === undefined
  ) {
    throw new Error("the module's line of sight meets the ground with no range");
  }
  return {
    rangeM,
    sigmaM: sight.sigmaM ?? null,
    resolved: sight.resolved,
    hitM: [x, y, z],
    at: { face: faceOf(sight.atFace), u: atU, v: atV },
  };
}

/** The module's terrain readout, copied out. */
function groundReadoutOf(readout: WasmGroundReadout): SurfaceGroundReadout {
  switch (readout.kind) {
    case WASM_LINE_OF_SIGHT["no-ground"]:
      return { kind: "no-ground" };
    case WASM_LINE_OF_SIGHT["not-surveyed"]:
      return { kind: "not-surveyed" };
    case WASM_LINE_OF_SIGHT.ground: {
      const {
        elevationM,
        elevationSigmaM,
        slopeRmsRad,
        slopeSigmaRad,
        slopeBaselineM,
        resolutionM,
        classCode,
        capacitySlopeRad,
      } = readout;
      if (
        elevationM === undefined ||
        elevationSigmaM === undefined ||
        slopeRmsRad === undefined ||
        slopeSigmaRad === undefined ||
        slopeBaselineM === undefined ||
        resolutionM === undefined ||
        classCode === undefined ||
        capacitySlopeRad === undefined
      ) {
        throw new Error("the module's readout meets the ground with no reading");
      }
      return {
        kind: "ground",
        sight: slantRangeOf({
          rangeM: readout.rangeM,
          sigmaM: readout.rangeSigmaM,
          resolved: readout.rangeResolved,
          hit: readout.hitM(),
          atFace: readout.atFace,
          atU: readout.atU,
          atV: readout.atV,
        }),
        reading: {
          elevationM,
          elevationSigmaM,
          slopeRmsRad,
          slopeSigmaRad,
          slopeBaselineM,
          resolutionM,
          resolved: readout.readingResolved,
        },
        material: { classCode, entry: readout.entry ?? null },
        properties: {
          bearingCapacityPa: readout.bearingCapacityPa ?? null,
          capacitySlopeRad,
          padFriction: readout.padFriction ?? null,
          frictionOnTemperature: readout.frictionOnTemperature,
          temperatureK: readout.temperatureK ?? null,
        },
      };
    }
    default:
      throw new Error(`the module answered an unknown readout, ${String(readout.kind)}`);
  }
}

/** The module's line of sight, copied out. */
function lineOfSightOf(sight: WasmLineOfSight): SurfaceLineOfSight {
  switch (sight.kind) {
    case WASM_LINE_OF_SIGHT["no-ground"]:
      return { kind: "no-ground" };
    case WASM_LINE_OF_SIGHT["not-surveyed"]:
      return { kind: "not-surveyed" };
    case WASM_LINE_OF_SIGHT.ground:
      return {
        kind: "ground",
        ...slantRangeOf({
          rangeM: sight.rangeM,
          sigmaM: sight.sigmaM,
          resolved: sight.resolved,
          hit: sight.hitM(),
          atFace: sight.atFace,
          atU: sight.atU,
          atV: sight.atV,
        }),
      };
    default:
      throw new Error(`the module answered an unknown line of sight, ${String(sight.kind)}`);
  }
}

/** The module's material, copied out. */
function materialOf(ground: WasmGround): SurfaceMaterial {
  const entry = ground.entry ?? null;
  const albedo = ground.normalAlbedoBvr();
  const { phaseLaw, limbParameter, grainDensityKgM3, transitionK, mechanics } = ground;
  if (
    albedo === undefined ||
    phaseLaw === undefined ||
    limbParameter === undefined ||
    grainDensityKgM3 === undefined ||
    transitionK === undefined ||
    mechanics === undefined
  ) {
    return { classCode: ground.classCode, entry, properties: null };
  }
  const [b, v, r] = albedo;
  if (b === undefined || v === undefined || r === undefined) {
    throw new Error("the module's albedo has no three bands");
  }
  return {
    classCode: ground.classCode,
    entry,
    properties: {
      normalAlbedoBvr: [b, v, r],
      phaseLaw,
      limbParameter,
      grainDensityKgM3,
      transitionK,
      mechanics,
    },
  };
}

/** A worker's answer and the buffers it transfers with it. */
export interface HeightAnswer {
  readonly reply: HeightWorkerReply;
  readonly transfer: Transferable[];
}

/**
 * Answers `request` from the loaded `module` and the worker's `fields`.
 *
 * @remarks
 * A bake the module refuses (a `JsError`, for a key out of range) is answered `bake-failed`, so
 * that the pool gives up that key alone. A body's bake the field cannot make yet is answered
 * `bake-unsurveyed`, which the pool asks again after the body's next block. A trap
 * (`WebAssembly.RuntimeError`, a Rust panic) is a bug that may leave the instance broken, so it is
 * rethrown: the worker raises it as its error and the pool replaces the worker. A body's field
 * blocks go to `fields` (R10.T6.b: decoded into the module's field, or held undecoded for a
 * stand-in), each answered with what the worker then holds and what a payload's block changed. A
 * class-map query is answered over the body's field (R10.T10.d), with its normal map (R10.T10.g),
 * and a ground-readout query
 * likewise (R10.T12), or refused with the module's message.
 */
export function answerRequest(
  module: HeightModule,
  request: HeightWorkerRequest,
  fields: WorkerFields,
): HeightAnswer {
  switch (request.kind) {
    case "bake":
      return answerBake(module, request, fields);
    case "field-init":
      return fieldAnswer(
        request.id,
        request.body,
        fields,
        fields.init(request.body, request.block, request.form),
      );
    case "field-chunk":
      return fieldAnswer(
        request.id,
        request.body,
        fields,
        fields.chunk(request.body, request.block),
      );
    case "field-release":
      fields.release(request.body);
      return fieldAnswer(request.id, request.body, fields, { kind: "taken", block: null });
    case "class-map":
      return answerClassMap(request, fields);
    case "ground-readout":
      return answerGroundReadout(request, fields);
  }
  return unreachable(request);
}

/**
 * Answers a ground-readout query: the readout, or the module's refusal of an argument; a trap is
 * rethrown, as a bake's is.
 */
function answerGroundReadout(
  request: Extract<HeightWorkerRequest, { kind: "ground-readout" }>,
  fields: WorkerFields,
): HeightAnswer {
  const { id, body, detailSeed } = request;
  try {
    const readout = fields.groundReadout(body, detailSeed, request);
    return { reply: { kind: "ground-readout", id, body, readout }, transfer: [] };
  } catch (error: unknown) {
    if (isTrap(error)) {
      throw error;
    }
    return {
      reply: { kind: "ground-readout-failed", id, body, message: messageOf(error) },
      transfer: [],
    };
  }
}

/**
 * Answers a class-map query: the map, `null` where no payload field of the body is held, or the
 * module's refusal; a trap is rethrown, as a bake's is.
 */
function answerClassMap(
  request: Extract<HeightWorkerRequest, { kind: "class-map" }>,
  fields: WorkerFields,
): HeightAnswer {
  const { id, body } = request;
  try {
    const map = fields.classMap(body, request.detailSeed);
    return { reply: { kind: "class-map", id, body, map }, transfer: classMapTransferables(map) };
  } catch (error: unknown) {
    if (isTrap(error)) {
      throw error;
    }
    return {
      reply: { kind: "class-map-failed", id, body, message: messageOf(error) },
      transfer: [],
    };
  }
}

/** Answers a bake request. */
function answerBake(
  module: HeightModule,
  request: Extract<HeightWorkerRequest, { kind: "bake" }>,
  fields: WorkerFields,
): HeightAnswer {
  const { key, generation, settings } = request;
  const source = settings.source;
  try {
    const bake =
      source.kind === "test-planet"
        ? bakeKey(module, key, generation, settings, source.ridges)
        : fields.bake(key, generation, settings, source);
    if (bake === null) {
      return {
        reply: {
          kind: "bake-unsurveyed",
          id: request.id,
          message: `patch ${patchKeyString(key)}: the field held of ${source.kind === "body" ? source.body : "the body"} cannot make it yet`,
        },
        transfer: [],
      };
    }
    return { reply: { kind: "baked", id: request.id, bake }, transfer: bakeTransferables(bake) };
  } catch (error: unknown) {
    if (isTrap(error)) {
      throw error;
    }
    return {
      reply: {
        kind: "bake-failed",
        id: request.id,
        message: `patch ${patchKeyString(key)}: ${messageOf(error)}`,
      },
      transfer: [],
    };
  }
}

/** The answer to field message `id` of `body`: what the worker now holds, or its refusal. */
function fieldAnswer(
  id: number,
  body: BodyIdHex,
  fields: WorkerFields,
  take: FieldTake,
): HeightAnswer {
  if (take.kind === "refused") {
    return {
      reply: {
        kind: "field-refused",
        id,
        body,
        message: take.message,
        heldBytes: fields.heldBytes,
      },
      transfer: [],
    };
  }
  return {
    reply: { kind: "field-held", id, body, heldBytes: fields.heldBytes, block: take.block },
    transfer: blockTransferables(take.block),
  };
}
