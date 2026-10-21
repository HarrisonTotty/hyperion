/**
 * The messages between the render thread's height-worker pool and its workers (plan R05, T10,
 * Design note 11; plan R10, T7, Design note 16).
 *
 * @remarks
 * The workers are our own same-origin module scripts, so a message's data is a structured clone of
 * one of these types, trusted rather than checked. A bake's typed arrays are transferred, not
 * copied. A body's coarse field travels as R09's payload blocks, each at most 1 MiB: its first
 * block in a `field-init`, every later one, a later payload's block 0 included, in a
 * `field-chunk`, each block cloned to every worker, and `field-release` drops the body. Messages
 * to one worker arrive in the order sent, so a bake posted after a block is made with it.
 *
 * A bake names its height source (R10.T6.b): the test planet with its ridges, which reads no field,
 * or a body, whose patches the worker bakes over the field it holds of it with the body's detail
 * seed. A worker answers each block of a payload with what it changed of the survey and the body's
 * level table after it, and a body's bake with R10's layers. A `class-map` query asks one worker for
 * a body's coarse class map over the field it holds (R10.T10.d), the disc's surface, and a
 * `ground-readout` query a view's whole terrain readout where its line of sight meets the ground
 * (R10.T12), whose answer's types live here with the readings', the worker's wrappers of the
 * module answering in them (`heightBake.ts`).
 */

import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";

import type { TerrainNormals, TerrainVertexPath } from "../../quality/terrainKinds";
import type { Face, PatchKey } from "../patchKey";
import type { BodyFixedVec3 } from "../planet";

/** A baked patch as the worker hands it to the render thread (Design note 5). */
export interface BakedPatch {
  readonly key: PatchKey;
  /** The generation of the request it answers. */
  readonly generation: number;
  /** The patch origin, body-fixed metres. */
  readonly originM: BodyFixedVec3;
  /** Own-level height and morph target per vertex, interleaved, metres above the datum. */
  readonly heights: Float32Array;
  /** On `baked-offsets` only: q₀ and q₁ per vertex from the origin, metres; else `null`. */
  readonly offsets: Float32Array | null;
  /** Octahedral normals in the body-fixed frame, packed by the worker into half floats. */
  readonly normals: Float16Array;
  /** The baked heights' range above the datum, metres. */
  readonly heightRangeM: readonly [number, number];
  /** The bounding radius about the origin, metres. */
  readonly boundingRadiusM: number;
  /**
   * The origin's own height above the datum, metres: h₀ of the `face-differences` path
   * (`PatchTerms`' h₀ in `hyperion_surface::patch::vertex`).
   */
  readonly originHeightM: number;
  /** How far the skirts hang below the edge vertices along the datum's normal, metres. */
  readonly skirtDepthM: number;
  /** R10's layers, for a body's patch; `null` for the test planet's, which has none. */
  readonly layers: TerrainLayers | null;
}

/** The palette byte of no entry, and of an empty slot's class and entry (`src/wasm.rs`). */
export const PALETTE_NONE = 255;

/**
 * R10's layers of a body's baked patch (R10.T6.a's `TerrainLayers`, as the worker hands them over;
 * `src/wasm.rs`'s `BakedTerrain`).
 */
export interface TerrainLayers {
  /**
   * The patch's palette, 8 slots of 2 bytes: each its material class's code then its palette
   * entry, {@link PALETTE_NONE} for no entry, and both {@link PALETTE_NONE} for an empty slot.
   */
  readonly palette: Uint8Array;
  /**
   * Each vertex's 8 weight bytes over the palette's slots, vertex (x, y) at `8 (65 y + x)`, summing
   * to 255 where it has weights; empty when the weights were skipped (the wireframe's).
   */
  readonly weights: Uint8Array;
  /** Each vertex's survey mask, 65 × 65: 255 surveyed, 0 not (Design note 12). */
  readonly surveyMask: Uint8Array;
  /**
   * The level, 5 to 8, of the coarse field the bake was made over, whose cells {@link surveyMask}
   * and {@link unsurveyedCells} are of: the terrain's discard finds each fragment's coarse cell by
   * it (R10.T10.c, `gpu/resources.ts`' `surveyCellCode`).
   */
  readonly fieldLevel: number;
  /** The patch's hard bound, metres. */
  readonly boundM: number;
  /** The patch's σ, the largest omitted RMS over its surveyed cells, metres. */
  readonly sigmaM: number;
  /**
   * Whether every vertex was answered by the field: `false` where some vertex outside the survey
   * took the reference surface or has no weights, which a later block may change.
   */
  readonly complete: boolean;
  /**
   * The index of each cell of the field's level holding a vertex outside the survey, rising, each
   * once: a block that surveys one of them changes the mask (R10's rebake rule, `rebake.ts`).
   */
  readonly unsurveyedCells: Uint32Array;
  /**
   * The horizon map's near field (R10.T8.a's layout, `src/wasm.rs`'s `horizonNear`): four words a
   * vertex, vertex (x, y) at `4 (65 y + x)`, azimuth 2w in word w's low half and 2w + 1 in its
   * high half as `pack2x16snorm` packs e ÷ 90°; empty where the map was skipped.
   */
  readonly horizonNear: Uint32Array;
  /** The horizon map's far field, four words a point, point (a, b) at `4 (9 b + a)`; or empty. */
  readonly horizonFar: Uint32Array;
  /**
   * Whether the map's far field read only ground the field holds; `false` where it read the
   * reference surface for want of a cell (`HorizonMap::is_complete`).
   */
  readonly horizonComplete: boolean;
  /**
   * The index of each cell the map's far field was refused for, rising, each once: a block that
   * brings one, surveyed or margin, may change the map (R10's rebake rule, `rebake.ts`).
   */
  readonly horizonRefusedCells: Uint32Array;
  /**
   * The photometry the lit terrain draws each palette slot with (R10.T10.b; `src/wasm.rs`'s
   * `photometry`), 8 × {@link PHOTOMETRY_STRIDE}, slot k at `5 k`: its normal albedo in the
   * Johnson B, V and R bands under the field's albedo scale, its law's code (`PhaseLaw`, 1 to 3)
   * and the law's L(0); all 0, law code 0, for an empty slot or one whose palette names no
   * substance.
   */
  readonly photometry: Float32Array;
}

/** The values a palette slot's photometry takes in {@link TerrainLayers.photometry}. */
export const PHOTOMETRY_STRIDE = 5;

/**
 * Whether the provisional test planet's octaves 8 to 12 are ridged (R05 Design note 12): the spike
 * runs with and without them, since sharp crests are the worst case for pops. R09's surface
 * generator replaces the test planet, and this with it.
 */
export type TestPlanetRidges = "off" | "on";

/**
 * Whether a body's bake makes R10's class weights: the lit style's do, the wireframe's do not. The
 * lit style's bakes make the horizon map with them (R10.T8.b), and the wireframe's skip both.
 */
export type WeightsLayerSetting = "baked" | "skipped";

/**
 * What a patch's heights come from (R10.T6.b): R05's provisional test planet, with or without its
 * ridges, which the descent spike keeps; or a body, over the field the workers hold of it, with
 * its detail seed (`BodyHooksDto.detail_seed`, R09.T1.b) and whether its weights are baked.
 */
export type HeightSourceSetting =
  | { readonly kind: "test-planet"; readonly ridges: TestPlanetRidges }
  | {
      readonly kind: "body";
      readonly body: BodyIdHex;
      readonly detailSeed: DetailSeedHex;
      readonly weights: WeightsLayerSetting;
    };

/** How a patch is baked: the setting's vertex path and normal resolution, and its height source. */
export interface BakeSettings {
  readonly vertexPath: TerrainVertexPath;
  readonly normals: TerrainNormals;
  readonly source: HeightSourceSetting;
}

/**
 * What a `field-init` starts: R09's payload, whose blocks the worker decodes into the body's field,
 * or a stand-in, blocks of no payload that the worker holds undecoded, as the descent spike's
 * synthetic field stands in for a field's memory on the test planet, which reads none (R10.T7).
 */
export type FieldForm = "payload" | "stand-in";

/** What a block of a payload changed in a worker's field, and the body's level table after it. */
export interface FieldBlockAnswer {
  /** The field's level, 5 to 8, at which the indices of {@link newlySurveyed} are. */
  readonly fieldLevel: number;
  /**
   * The cells the block surveyed that the field did not hold as surveyed, as runs: each the first
   * cell index and the index past the last, flat, rising and disjoint.
   */
  readonly newlySurveyed: Uint32Array;
  /**
   * The cells the block brought, surveyed or margin, that the field did not hold at all, as runs
   * laid out as {@link newlySurveyed}'s (R10.T8.b's rebake rule).
   */
  readonly newlyHeld: Uint32Array;
  /**
   * The body's level table, 25 × 4 as the test planet's `levelTable` (bound, lowest and highest
   * height, largest vertex spacing, metres), as the field now holds it (`worker::level_table`).
   */
  readonly levels: Float64Array;
}

/**
 * A body's coarse class map for R07's disc, as a worker answers it (plan R10, T10.d; `src/wasm.rs`'s
 * `ClassMapAnswer`, the crate's `WorkerFields::class_map`): each texel the survey touches, a cell
 * of {@link ClassMapAnswer.level}, with its surveyed share and its weights over the body's classes.
 */
export interface ClassMapAnswer {
  /** The map's quadtree level, 6: 2^level texels along a face's side. */
  readonly level: number;
  /** The level, 5 to 8, of the field the map was made from. */
  readonly fieldLevel: number;
  /**
   * Its classes, at most 16, two bytes each: the form's class code, then the palette entry,
   * {@link PALETTE_NONE} where the palette names no substance.
   */
  readonly classes: Uint8Array;
  /**
   * Each class's photometry, {@link PHOTOMETRY_STRIDE} values a class as
   * {@link TerrainLayers.photometry}'s: law code 0 where the palette names no substance.
   */
  readonly photometry: Float32Array;
  /** The cell index at {@link ClassMapAnswer.level} (`fieldCellIndex`'s) of each texel, rising. */
  readonly texels: Uint32Array;
  /** Each texel's surveyed share, (0, 1]: its field cells surveyed over its cells. */
  readonly fractions: Float32Array;
  /**
   * Each texel's weight bytes over the classes, texel t's at `classes · t`, summing to 255 but where
   * a class past the 16 was left out.
   */
  readonly weights: Uint8Array;
  /** Whether every texel the survey touches was answered; `false` where a read lacked a cell. */
  readonly complete: boolean;
}

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

/** The distance along a line of sight to the ground it meets, and where (R10.T5's `SlantRange`). */
export interface SurfaceSlantRange {
  readonly rangeM: number;
  /** The range's standard uncertainty, metres; `null` where the line of sight grazes. */
  readonly sigmaM: number | null;
  readonly resolved: boolean;
  /** The hit, body-fixed metres. */
  readonly hitM: readonly [number, number, number];
  /** The hit's foot on the datum. */
  readonly at: FacePoint;
}

/** Where a line of sight meets the ground, as far as the survey can say (R10.T5's `LineOfSight`). */
export type SurfaceLineOfSight =
  | ({ readonly kind: "ground" } & SurfaceSlantRange)
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

/** The ground's material at a point, by its form and its substance (R10.T1.b's `GroundMaterial`). */
export interface SurfaceGroundMaterial {
  /** The form's class code (`MaterialClass`). */
  readonly classCode: number;
  /** The palette entry, `null` where the palette names no substance. */
  readonly entry: number | null;
}

/** The ground's material at a surveyed point, with its properties where its substance is named. */
export interface SurfaceMaterial extends SurfaceGroundMaterial {
  /** `null` where the palette names no substance. */
  readonly properties: SurfaceMaterialProperties | null;
}

/**
 * A baseline a terrain readout's `SLOPE` may be read over, metres: the guide's `BASELINE` choices,
 * the crate's `SLOPE_BASELINES` (R10.T12), and nothing else, which the module refuses.
 */
export type SlopeBaselineM = 2 | 5 | 100;

/**
 * What a view's terrain readout asks where its line of sight meets the ground (R10.T12; the crate's
 * `GroundReadoutQuery`): the line of sight, body-fixed, and the readout's settings.
 */
export interface GroundReadoutQuery {
  /** The line of sight's origin, body-fixed metres. */
  readonly originM: BodyFixedVec3;
  /** Its direction, body-fixed, of any length but zero. */
  readonly direction: BodyFixedVec3;
  /** How far along it the ground is looked for, metres; past the body's far side for `NO GROUND`. */
  readonly maxM: number;
  /**
   * The baseline `SLOPE` is read over, metres (the guide's `BASELINE`), as the `STYLE` panel's
   * `BASELINE` row sets it (R10.T12.a), 2 as the display mounts; the capacity takes 2 m whatever
   * this is.
   */
  readonly slopeBaselineM: SlopeBaselineM;
  /** The width of the pad the bearing capacity is taken under, metres: the reference pad's 1.0. */
  readonly padWidthM: number;
  /**
   * The seasonal orbit's mean anomaly at the instant the view lights the body at, radians from
   * periapsis, finite (`meanAnomalyAt`); `undefined` where no orbit is known (a kept scene's body).
   */
  readonly seasonMeanAnomalyRad: number | undefined;
}

/**
 * The ground's two properties a terrain readout states after `MATERIAL` (R10.T12; the crate's
 * `GroundProperties`), each `null` where it is not computed: the guide's em dash, never 0.
 */
export interface SurfaceGroundProperties {
  /**
   * The ultimate bearing capacity under the query's pad at the body's surface gravity, Pa (the
   * guide's `ULTIMATE BEARING CAPACITY`).
   */
  readonly bearingCapacityPa: number | null;
  /** The RMS slope over the fixed 2 m baseline the capacity was taken on, rad. */
  readonly capacitySlopeRad: number;
  /** The pad's interface friction coefficient, dimensionless (the guide's `PAD FRICTION`). */
  readonly padFriction: number | null;
  /** Whether the friction depends on the temperature, so that the line states its `· AT`. */
  readonly frictionOnTemperature: boolean;
  /**
   * The month's mean surface temperature at the point, kelvin (the guide's `AT`): `null` in a year of
   * twelve months with no season known.
   */
  readonly temperatureK: number | null;
}

/** A view's terrain readout where its line of sight meets the ground (R10.T12's `GroundReadout`). */
export type SurfaceGroundReadout =
  | {
      readonly kind: "ground";
      readonly sight: SurfaceSlantRange;
      /** The reading at the sight's foot, its slope over the query's baseline. */
      readonly reading: SurfaceReading;
      readonly material: SurfaceGroundMaterial;
      readonly properties: SurfaceGroundProperties;
    }
  /** The line of sight meets no ground within its reach: `NO GROUND`. */
  | { readonly kind: "no-ground" }
  /** It meets ground the ship has not surveyed, or no field is held: `NOT SURVEYED`. */
  | { readonly kind: "not-surveyed" };

/**
 * Answer a view's terrain readout of `body` along the query's line of sight over the field the
 * worker holds of it, with its detail seed (R10.T12).
 */
export interface GroundReadoutRequest extends GroundReadoutQuery {
  readonly kind: "ground-readout";
  readonly id: number;
  readonly body: BodyIdHex;
  readonly detailSeed: DetailSeedHex;
}

/** A message from the pool to a height worker. */
export type HeightWorkerRequest =
  /** Bake a patch; the answer carries the same `id`. */
  | {
      readonly kind: "bake";
      readonly id: number;
      readonly key: PatchKey;
      readonly generation: number;
      readonly settings: BakeSettings;
    }
  /**
   * Start `body`'s field from its first block, R09's block 0, which carries the whole header,
   * replacing any field of `body` the worker holds; or, for a stand-in, hold the blocks undecoded.
   */
  | {
      readonly kind: "field-init";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly block: ArrayBuffer;
      readonly form: FieldForm;
    }
  /** Add one more block to `body`'s field, which a `field-init` must have started. */
  | {
      readonly kind: "field-chunk";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly block: ArrayBuffer;
    }
  /** Drop `body`'s field; one not held is no error. */
  | { readonly kind: "field-release"; readonly id: number; readonly body: BodyIdHex }
  /**
   * Answer `body`'s class map for R07's disc over the field the worker holds of it, with its detail
   * seed (R10.T10.d).
   */
  | {
      readonly kind: "class-map";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly detailSeed: DetailSeedHex;
    }
  | GroundReadoutRequest;

/** A message from a height worker to the pool. */
export type HeightWorkerReply =
  /** The bake of request `id`. */
  | { readonly kind: "baked"; readonly id: number; readonly bake: BakedPatch }
  /** Request `id` could not be baked. */
  | { readonly kind: "bake-failed"; readonly id: number; readonly message: string }
  /**
   * Request `id`, a body's patch, cannot be baked yet: the worker holds no field of the body, or a
   * surveyed vertex reads a cell still to come, a survey's margin in a later block (R10.T6.b). The
   * pool asks again after the body's next block.
   */
  | { readonly kind: "bake-unsurveyed"; readonly id: number; readonly message: string }
  /**
   * Field message `id`, of `body`, is done: `heldBytes` is what the worker now holds of every
   * body's field, its coarse-field copy (R12 Design note 6), and `block` what a payload's block
   * changed, `null` for a release or a stand-in.
   */
  | {
      readonly kind: "field-held";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly heldBytes: number;
      readonly block: FieldBlockAnswer | null;
    }
  /**
   * Field message `id` was refused: a block of a body with no `field-init` before it, or one the
   * worker cannot read. The worker's field of `body` is as it was before the message.
   */
  | {
      readonly kind: "field-refused";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly message: string;
      readonly heldBytes: number;
    }
  /**
   * Class-map query `id`'s answer: `body`'s map, or `null` where the worker holds no payload field
   * of it.
   */
  | {
      readonly kind: "class-map";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly map: ClassMapAnswer | null;
    }
  /** Class-map query `id` was refused by the module, which names why. */
  | {
      readonly kind: "class-map-failed";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly message: string;
    }
  /** Ground-readout query `id`'s answer, `body`'s readout. */
  | {
      readonly kind: "ground-readout";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly readout: SurfaceGroundReadout;
    }
  /** Ground-readout query `id` was refused by the module, which names why. */
  | {
      readonly kind: "ground-readout-failed";
      readonly id: number;
      readonly body: BodyIdHex;
      readonly message: string;
    };

/** The buffers a worker transfers with a bake, so that none is copied twice. */
export function bakeTransferables(bake: BakedPatch): Transferable[] {
  const buffers: Transferable[] = [bake.heights.buffer, bake.normals.buffer];
  if (bake.offsets !== null) {
    buffers.push(bake.offsets.buffer);
  }
  if (bake.layers !== null) {
    const {
      palette,
      weights,
      surveyMask,
      unsurveyedCells,
      photometry,
      horizonNear,
      horizonFar,
      horizonRefusedCells,
    } = bake.layers;
    buffers.push(
      palette.buffer,
      weights.buffer,
      surveyMask.buffer,
      unsurveyedCells.buffer,
      photometry.buffer,
      horizonNear.buffer,
      horizonFar.buffer,
      horizonRefusedCells.buffer,
    );
  }
  return buffers;
}

/** The buffers a worker transfers with a block's answer. */
export function blockTransferables(block: FieldBlockAnswer | null): Transferable[] {
  return block === null
    ? []
    : [block.newlySurveyed.buffer, block.newlyHeld.buffer, block.levels.buffer];
}

/** The buffers a worker transfers with a class map. */
export function classMapTransferables(map: ClassMapAnswer | null): Transferable[] {
  return map === null
    ? []
    : [
        map.classes.buffer,
        map.photometry.buffer,
        map.texels.buffer,
        map.fractions.buffer,
        map.weights.buffer,
      ];
}
