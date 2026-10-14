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
 * reads no field.
 */

import { type BodyIdHex, type DetailSeedHex, hexToU64, parseBodyId } from "@hyperion/protocol";

import { patchKeyString, type Face, type PatchKey, unreachable } from "../patchKey";
import { vec3 } from "../../../geometry/vec3";
import type {
  BakedPatch,
  BakeSettings,
  FieldBlockAnswer,
  FieldForm,
  HeightSourceSetting,
  HeightWorkerReply,
  HeightWorkerRequest,
  TerrainLayers,
  TestPlanetRidges,
  WeightsLayerSetting,
} from "./messages";
import { bakeTransferables, blockTransferables } from "./messages";

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
  readonly boundM: number;
  readonly sigmaM: number;
  readonly complete: boolean;
}

/** The module's `FieldBlock`: what a block taken changed, and the body's level table after it. */
export interface WasmFieldBlock {
  newlySurveyed(): Uint32Array;
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

/** A point of a face, in the cube's face coordinates (`hyperion_surface::cube::FaceUv`). */
export interface FacePoint {
  readonly face: Face;
  /** In [−1, 1]. */
  readonly u: number;
  /** In [−1, 1]. */
  readonly v: number;
}

/** What a readout may quote of the ground at a point (R10.T5's `SurveyedReading`). */
export interface SurfaceReading {
  readonly elevationM: number;
  /** The elevation's standard uncertainty, metres; 0 where resolved. */
  readonly elevationSigmaM: number;
  readonly slopeRmsRad: number;
  readonly slopeSigmaRad: number;
  readonly slopeBaselineM: number;
  /** The cell code's resolution the reading was taken at, metres. */
  readonly resolutionM: number;
  /** Whether the survey reaches the band limit, so that the uncertainties are zero. */
  readonly resolved: boolean;
}

/** Where a line of sight meets the ground, as far as the survey can say (R10.T5's `LineOfSight`). */
export type SurfaceLineOfSight =
  | {
      readonly kind: "ground";
      readonly rangeM: number;
      /** The range's standard uncertainty, metres; `null` where the line of sight grazes. */
      readonly sigmaM: number | null;
      readonly resolved: boolean;
      /** The hit, body-fixed metres. */
      readonly hitM: readonly [number, number, number];
      /** The hit's foot on the datum. */
      readonly at: FacePoint;
    }
  | { readonly kind: "no-ground" }
  | { readonly kind: "not-surveyed" };

/** The properties of a material whose substance the palette names (R10.T1.a's). */
export interface SurfaceMaterialProperties {
  /** The normal albedo in the Johnson B, V and R bands. */
  readonly normalAlbedoBvr: readonly [number, number, number];
  /** The photometric law's code (`PhaseLaw`). */
  readonly phaseLaw: number;
  /** The law's L(0) at the material's albedo in V. */
  readonly limbParameter: number;
  readonly grainDensityKgM3: number;
  /** The temperature above which the substance is melt or liquid, kelvin. */
  readonly transitionK: number;
  /** The substance's mechanics family's code (`MechanicsFamily`). */
  readonly mechanics: number;
}

/** The ground's material at a surveyed point (R10.T1.b's `GroundMaterial`, with its properties). */
export interface SurfaceMaterial {
  /** The form's class code (`MaterialClass`). */
  readonly classCode: number;
  /** The palette entry, `null` where the palette names no substance. */
  readonly entry: number | null;
  /** `null` where the palette names no substance. */
  readonly properties: SurfaceMaterialProperties | null;
}

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
      case "payload":
        return this.#take(() => this.#module.fieldChunk(...bodyArgs(body), new Uint8Array(block)));
    }
    return unreachable(form);
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
  }

  /**
   * Bakes `key` of `source`'s body over its field (no extra skirt margin), with R10's layers; `null`
   * where the field cannot make it yet: none is held, or a surveyed vertex reads a cell still to
   * come.
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
      0,
    );
    if (baked === undefined) {
      return null;
    }
    try {
      return patchOf(baked, key, generation, {
        palette: baked.palette(),
        weights: baked.weights(),
        surveyMask: baked.surveyMask(),
        boundM: baked.boundM,
        sigmaM: baked.sigmaM,
        complete: baked.complete,
        unsurveyedCells: baked.unsurveyedCells(),
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

/** The module's line of sight, copied out. */
function lineOfSightOf(sight: WasmLineOfSight): SurfaceLineOfSight {
  switch (sight.kind) {
    case WASM_LINE_OF_SIGHT["no-ground"]:
      return { kind: "no-ground" };
    case WASM_LINE_OF_SIGHT["not-surveyed"]:
      return { kind: "not-surveyed" };
    case WASM_LINE_OF_SIGHT.ground: {
      const hit = sight.hitM();
      const [x, y, z] = hit ?? [];
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
        kind: "ground",
        rangeM,
        sigmaM: sight.sigmaM ?? null,
        resolved: sight.resolved,
        hitM: [x, y, z],
        at: { face: faceOf(sight.atFace), u: atU, v: atV },
      };
    }
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
 * stand-in), each answered with what the worker then holds and what a payload's block changed.
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
  }
  return unreachable(request);
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
