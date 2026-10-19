/**
 * A body's survey coverage as the client holds it: each coarse cell's resolution code, from R09's
 * `surface_field` answers, and the GPU's coverage mask made from it (plan R10, T7; Design notes 12
 * and 16).
 *
 * @remarks
 * The cells are R09's coarse cells, numbered by `hyperion_surface::field::cell_index`: face-major,
 * then the Morton order of (i, j) within the face, 6 · 4ᴸ of them at the field's level L (5 to 8).
 * Each answer's `CoverDto` names the cells its payload carries, each run with its
 * `ResolutionCode`: 0 for a margin cell, held for the synthesis but not surveyed, and c ≥ 1 for a
 * survey no coarser than 1 cm × 2^((c − 1) ÷ 8). Coverage keeps a cell's finest code, as R09's
 * `Cover` does, so a later delta never makes a cell coarser; since R09.T19.b's deltas carry only
 * cells new to the client or surveyed again more finely, that is the same as their replacing the
 * client's copies, as `SurfaceFieldDto.coverage` says. A margin cell counts as unsurveyed
 * everywhere here, in the readouts and the mask alike.
 */

import type { CoverDto } from "@hyperion/protocol";

import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { stToUv } from "./cube";
import { type Edge, EDGES, edgeNeighbour, type Face, type PatchKey } from "./patchKey";
import type { FieldBlockAnswer } from "./workers/messages";

/** The shallowest coarse-field level R09 serves (`SurfaceHeaderDto.level`). */
export const MIN_FIELD_LEVEL = 5;

/** The deepest coarse-field level R09 serves, an Earth's 40 km cells (`SurfaceHeaderDto.level`). */
export const MAX_FIELD_LEVEL = 8;

/** The cells of a coarse-field level: 6 · 4ᴸ (`hyperion_surface::field::cell_count`). */
export function fieldCellCount(level: number): number {
  return 6 * 4 ** level;
}

/** Whether `level` is one R09 serves a field at. */
export function isFieldLevel(level: number): boolean {
  return Number.isInteger(level) && level >= MIN_FIELD_LEVEL && level <= MAX_FIELD_LEVEL;
}

/** The low 16 bits of `x` spread to the even bits, as R09's `spread`. */
function spread(x: number): number {
  let s = x & 0x0000_ffff;
  s = (s | (s << 8)) & 0x00ff_00ff;
  s = (s | (s << 4)) & 0x0f0f_0f0f;
  s = (s | (s << 2)) & 0x3333_3333;
  return (s | (s << 1)) & 0x5555_5555;
}

/** The even bits of `x` gathered into the low 16, the inverse of {@link spread}, as R09's `compact`. */
function compact(x: number): number {
  let c = x & 0x5555_5555;
  c = (c | (c >>> 1)) & 0x3333_3333;
  c = (c | (c >>> 2)) & 0x0f0f_0f0f;
  c = (c | (c >>> 4)) & 0x00ff_00ff;
  return (c | (c >>> 8)) & 0x0000_ffff;
}

/**
 * A coarse cell's index among its level's cells, the client's twin of R09's `cell_index`: the
 * face × 4ᴸ plus the cell's Morton code, whose bit 2k is bit k of i and bit 2k + 1 bit k of j.
 *
 * @throws Error for a cell outside the levels R09 serves.
 */
export function fieldCellIndex(cell: PatchKey): number {
  if (!isFieldLevel(cell.level)) {
    throw new Error(`a coarse cell is at level 5 to 8, not ${cell.level}`);
  }
  return cell.face * 4 ** cell.level + (spread(cell.i) | (spread(cell.j) << 1));
}

/** The coarse cell of `level` whose {@link fieldCellIndex} is `index`, R09's `cell_at_index`. */
export function fieldCellAt(level: number, index: number): PatchKey {
  const perFace = 4 ** level;
  const face = Math.floor(index / perFace);
  const morton = index - face * perFace;
  return { face: faceOfIndex(face), level, i: compact(morton), j: compact(morton >>> 1) };
}

function faceOfIndex(n: number): Face {
  switch (n) {
    case 0:
    case 1:
    case 2:
    case 3:
    case 4:
    case 5:
      return n;
    default:
      throw new Error(`a cell index names face ${n}, not 0 to 5`);
  }
}

/** A resolution code's steps an octave (R09's `RESOLUTION_PER_OCTAVE`). */
const CODES_PER_OCTAVE = 8;

/** Centimetres a metre: code 1 is 1 cm (R09's `CM_PER_M`). */
const CM_PER_M = 100;

/**
 * The coarsest resolution a survey of code `code` admits, m: 1 cm × 2^((c − 1) ÷ 8), the client's
 * twin of R09's `ResolutionCode::resolution`, or `null` for code 0, not surveyed (a margin cell).
 *
 * @remarks
 * Pinned to R09's `tests/golden/wire/codes.golden`, whose `resolution_m` rows it reproduces for
 * every code (`coverage.test.ts`).
 *
 * @throws Error for a code that is not a byte.
 */
export function resolutionOfCodeM(code: number): number | null {
  if (!Number.isInteger(code) || code < 0 || code > 255) {
    throw new Error(`a resolution code is a byte, not ${code}`);
  }
  // Divided, not multiplied by 0.01, as the Rust is, so that every code rounds as its golden row.
  return code === 0 ? null : 2 ** ((code - 1) / CODES_PER_OCTAVE) / CM_PER_M;
}

/** The finer of two codes, a survey beating 0, as R09's `ResolutionCode::finer`. */
function finerCode(a: number, b: number): number {
  if (a === 0) {
    return b;
  }
  if (b === 0) {
    return a;
  }
  return Math.min(a, b);
}

/** F(u, v) = atan(uv ÷ √(1 + u² + v²)), the solid angle of the rectangle [0, u] × [0, v] of a face. */
function corner(u: number, v: number): number {
  return Math.atan((u * v) / Math.sqrt(1 + u * u + v * v));
}

/**
 * The solid angle of each cell of one face at `level`, sr, indexed j × 2ᴸ + i: the gnomonic
 * rectangle [u₀, u₁] × [v₀, v₁] of the face's plane seen from the centre, F(u₁, v₁) − F(u₀, v₁)
 * − F(u₁, v₀) + F(u₀, v₀) with F(u, v) = atan(uv ÷ √(1 + u² + v²)), exact for a cube-sphere cell,
 * whose edges are great circles; every face is the same by symmetry, and the six sum to 4π.
 */
function cellSolidAnglesSr(level: number): Float64Array {
  const n = 2 ** level;
  const edges = Float64Array.from({ length: n + 1 }, (_, k) => stToUv(k / n));
  const angles = new Float64Array(n * n);
  for (let j = 0; j < n; j += 1) {
    const v0 = edges[j] ?? 0;
    const v1 = edges[j + 1] ?? 0;
    for (let i = 0; i < n; i += 1) {
      const u0 = edges[i] ?? 0;
      const u1 = edges[i + 1] ?? 0;
      angles[j * n + i] = corner(u1, v1) - corner(u0, v1) - corner(u1, v0) + corner(u0, v0);
    }
  }
  return angles;
}

/**
 * What merging a cover gives: the new coverage, or why the cover could not be merged, a server
 * bug, which the store reports.
 */
export type CoverMerge =
  | { readonly ok: true; readonly coverage: CoverageView }
  | { readonly ok: false; readonly message: string };

/**
 * A body's coverage at one revision: each coarse cell's finest resolution code, immutable.
 *
 * @remarks
 * A new answer makes a new view ({@link CoverageView.merged}), and the link's state a copy marked
 * stale ({@link CoverageView.withStale}), so a consumer holding a view sees it never change.
 */
export class CoverageView {
  /** The field's level, 5 to 8. */
  readonly level: number;
  /**
   * The revision of the body's coverage it reflects, R09's `SurfaceFieldDto.revision`, or `null`
   * for {@link CoverageView.empty}'s, before any answer.
   */
  readonly revision: number | null;
  /**
   * Whether the coverage may be behind the server's: the link went down, and no answer has been
   * held since it came back. What was received is kept meanwhile; a body whose request failed
   * stays stale until it is wanted afresh and answered.
   */
  readonly stale: boolean;
  /** Each cell's finest code, by cell index; 0 for a cell not surveyed or not held. */
  readonly #codes: Uint8Array;
  #surveyedFraction: number | undefined;
  #best: number | undefined;
  #mask: Uint8Array | undefined;

  private constructor(level: number, codes: Uint8Array, revision: number | null, stale: boolean) {
    this.level = level;
    this.#codes = codes;
    this.revision = revision;
    this.stale = stale;
  }

  /**
   * The coverage of a field of `level` with no cell surveyed, at no revision.
   *
   * @throws Error for a level R09 does not serve.
   */
  static empty(level: number): CoverageView {
    if (!isFieldLevel(level)) {
      throw new Error(`a coarse field is at level 5 to 8, not ${level}`);
    }
    return new CoverageView(level, new Uint8Array(fieldCellCount(level)), null, false);
  }

  /**
   * This coverage with `cover`'s cells merged in, each keeping its finer code, at `revision`, and
   * not stale.
   *
   * @remarks
   * A range must be a whole run of this level's cells, `start` below `end` and `end` at most
   * 6 · 4ᴸ, and its code a byte; R09 sends them sorted and disjoint, which the merge does not
   * need. The revision is the larger of this one's and `revision`, since a body's revision only
   * rises.
   *
   * @param revision - The answer's revision, 0 or more.
   */
  merged(cover: CoverDto, revision: number): CoverMerge {
    const count = this.#codes.length;
    const codes = this.#codes.slice();
    for (const [at, range] of cover.ranges.entries()) {
      const { start, end, resolution } = range;
      if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end <= start) {
        return {
          ok: false,
          message: `coverage range ${at}, ${start}..${end}, is not a run of cells`,
        };
      }
      if (end > count) {
        return {
          ok: false,
          message: `coverage range ${at} ends at cell ${end}, beyond level ${this.level}'s ${count}`,
        };
      }
      if (!Number.isInteger(resolution) || resolution < 0 || resolution > 255) {
        return {
          ok: false,
          message: `coverage range ${at}'s resolution code ${resolution} is not a byte`,
        };
      }
      if (resolution === 0) {
        continue;
      }
      for (let cell = start; cell < end; cell += 1) {
        codes[cell] = finerCode(codes[cell] ?? 0, resolution);
      }
    }
    const merged = this.revision === null ? revision : Math.max(this.revision, revision);
    return { ok: true, coverage: new CoverageView(this.level, codes, merged, false) };
  }

  /** This coverage, marked stale or not; the same object when that changes nothing. */
  withStale(stale: boolean): CoverageView {
    return stale === this.stale
      ? this
      : new CoverageView(this.level, this.#codes, this.revision, stale);
  }

  /**
   * The resolution `cell` was surveyed at, m, or `null` where it was not surveyed (a margin cell,
   * or one not held).
   *
   * @param cell - A cell at the field's level, or a patch deeper, read at its coarse ancestor.
   * @throws Error for a cell shallower than the field's level.
   */
  resolutionAtM(cell: PatchKey): number | null {
    if (cell.level < this.level) {
      throw new Error(
        `a cell of level ${cell.level} is coarser than the field's level ${this.level}`,
      );
    }
    const shift = cell.level - this.level;
    const coarse: PatchKey = {
      face: cell.face,
      level: this.level,
      i: Math.floor(cell.i / 2 ** shift),
      j: Math.floor(cell.j / 2 ** shift),
    };
    return resolutionOfCodeM(this.#codes[fieldCellIndex(coarse)] ?? 0);
  }

  /**
   * The share of the body's surface surveyed, 0 to 1: the surveyed cells' solid angle from the
   * body's centre over 4π, so that the cube's small corner cells weigh as little as they cover. On
   * a spheroid the share of its area differs from this by about its flattening.
   *
   * @remarks
   * Exactly 1 only when every cell is surveyed and exactly 0 when none is, so that a readout can
   * tell the whole body (R10.T13's draft 4, `100 %` only then) from a share that rounds to it.
   */
  surveyedFraction(): number {
    if (this.#surveyedFraction === undefined) {
      const n = 2 ** this.level;
      const perFace = n * n;
      const angles = cellSolidAnglesSr(this.level);
      let surveyedSr = 0;
      let surveyed = 0;
      for (let index = 0; index < this.#codes.length; index += 1) {
        if ((this.#codes[index] ?? 0) !== 0) {
          const morton = index % perFace;
          surveyedSr += angles[compact(morton >>> 1) * n + compact(morton)] ?? 0;
          surveyed += 1;
        }
      }
      this.#surveyedFraction =
        surveyed === this.#codes.length
          ? 1
          : Math.min(1 - Number.EPSILON, surveyedSr / (4 * Math.PI));
    }
    return this.#surveyedFraction;
  }

  /** The finest resolution of any surveyed cell, m, or `null` where none is surveyed. */
  bestResolutionM(): number | null {
    if (this.#best === undefined) {
      let best = 0;
      for (const code of this.#codes) {
        best = finerCode(best, code);
      }
      this.#best = best;
    }
    return resolutionOfCodeM(this.#best);
  }

  /**
   * The GPU coverage mask's texels: one byte a cell a face (Design note 12), {@link
   * COVERAGE_MASK_SURVEYED} where the cell is surveyed and 0 elsewhere, margin cells included, laid
   * out face after face, then row j, then column i, as {@link coverageMaskTextureSpec}'s layers.
   */
  maskTexels(): Uint8Array {
    if (this.#mask === undefined) {
      const n = 2 ** this.level;
      const perFace = n * n;
      const mask = new Uint8Array(this.#codes.length);
      for (let index = 0; index < this.#codes.length; index += 1) {
        if ((this.#codes[index] ?? 0) !== 0) {
          const face = Math.floor(index / perFace);
          const morton = index - face * perFace;
          mask[face * perFace + compact(morton >>> 1) * n + compact(morton)] =
            COVERAGE_MASK_SURVEYED;
        }
      }
      this.#mask = mask;
    }
    return this.#mask;
  }
}

/**
 * One piece of a survey's edge (R10.T10.c, Design note 12): a side of a surveyed coarse cell across
 * which the neighbouring cell, on the same face or across a face's edge, is not surveyed, a margin
 * cell or one not held.
 */
export interface SurveyEdge {
  /** The surveyed cell, at the coverage's level. */
  readonly cell: PatchKey;
  /**
   * Its side the edge runs along, towards the unsurveyed cell: R05's patch edges, `UMin` towards
   * i − 1, `UMax` towards i + 1, `VMin` towards j − 1, `VMax` towards j + 1.
   */
  readonly side: Edge;
}

/**
 * The survey's edge on `face` (R10.T10.c): each side of each surveyed cell of the face whose
 * neighbour across it is not surveyed, margin cells counted unsurveyed, in the order of the cells,
 * row j then column i, and of their sides, `UMin`, `UMax`, `VMin`, `VMax`.
 *
 * @remarks
 * Each piece is listed once, by the face of its surveyed cell, so that the six faces' lists are
 * the whole edge, a piece on a face's edge included, its unsurveyed side on the neighbouring face.
 * Provides' `surveyEdges(face, level)`: the level is the coverage's own.
 */
export function surveyEdges(coverage: CoverageView, face: Face): ReadonlyArray<SurveyEdge> {
  const { level } = coverage;
  const n = 2 ** level;
  const texels = coverage.maskTexels();
  const surveyed = (cell: PatchKey): boolean =>
    (texels[(cell.face * n + cell.j) * n + cell.i] ?? 0) !== 0;
  const edges: SurveyEdge[] = [];
  for (let j = 0; j < n; j += 1) {
    for (let i = 0; i < n; i += 1) {
      const cell: PatchKey = { face, level, i, j };
      if (!surveyed(cell)) {
        continue;
      }
      for (const side of EDGES) {
        if (!surveyed(edgeNeighbour(cell, side))) {
          edges.push({ cell, side });
        }
      }
    }
  }
  return edges;
}

/**
 * The GPU coverage mask's format: one unsigned normalised byte a texel, so that a shader may
 * filter it, where a cell's code would need `r8uint`, which is not filterable.
 */
export const COVERAGE_MASK_FORMAT = "r8unorm" satisfies GPUTextureFormat;

/** The mask's byte for a surveyed cell, 1.0 in the shader. */
export const COVERAGE_MASK_SURVEYED = 255;

/**
 * The specification of a field's coverage mask (Design note 12): a 2D array of six 2ᴸ × 2ᴸ layers,
 * one a face, under `coarse-field-gpu` (R12 Design note 6), 6 · 4ᴸ bytes, at most 384 KiB at
 * level 8, the brainstorm's "coarse field, GPU copy".
 */
export function coverageMaskTextureSpec(name: string, level: number): TextureSpec {
  const side = 2 ** level;
  return {
    name,
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: COVERAGE_MASK_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "coarse-field-gpu",
  };
}

/**
 * One body's coverage mask on the GPU, made through the engine under `coarse-field-gpu` and
 * rewritten from each new coverage and from each block the height workers take.
 *
 * @remarks
 * The mask holds every cell surveyed by the coverages and the blocks given since it was made: a
 * cell once surveyed stays so, as `CoverageView.merged` keeps it. It follows the workers' blocks
 * ({@link CoverageMask.survey}, from the pool's `onFieldChange`) as well as the store's answers, so
 * that the reference surface it discards (R07's mesh figure, `smoothMesh.wgsl`) and the terrain's
 * own masks, each a bake's over the workers' field, agree while a payload streams and after the
 * link cuts one off, which the store's coverage would name only at the next answer (main's ruling
 * on R10.T6.b's question, 2026-10-14; R10.T10.c).
 *
 * The texture is made at the first update and again, empty, when the field's level changes. After
 * a device loss, {@link CoverageMask.restore} makes it again from the cells held, so that a cell
 * only a block surveyed is not lost until the store's next answer; {@link CoverageMask.release}
 * forgets them, for a body the view no longer draws.
 */
export class CoverageMask {
  readonly #engine: Pick<RenderEngine, "createTexture" | "writeTexture" | "releaseTexture">;
  readonly #name: string;
  #texture: TextureHandle | null = null;
  #level: number | null = null;
  /** The texels written, face then row j then column i, as {@link CoverageView.maskTexels}. */
  #texels: Uint8Array | null = null;

  constructor(
    engine: Pick<RenderEngine, "createTexture" | "writeTexture" | "releaseTexture">,
    name: string,
  ) {
    this.#engine = engine;
    this.#name = name;
  }

  /** The mask's texture, or `null` before the first update or after a release. */
  get texture(): TextureHandle | null {
    return this.#texture;
  }

  /** Adds `coverage`'s surveyed cells to the mask, making the texture first if it has none of that level. */
  update(coverage: CoverageView): void {
    const texels = this.#texelsOf(coverage.level);
    const surveyed = coverage.maskTexels();
    for (let at = 0; at < texels.length; at += 1) {
      if ((surveyed[at] ?? 0) !== 0) {
        texels[at] = COVERAGE_MASK_SURVEYED;
      }
    }
    this.#write(coverage.level, texels);
  }

  /**
   * Adds the cells a block surveyed anew (`FieldBlockAnswer.newlySurveyed`'s runs of cell
   * indices at `fieldLevel`) to the mask, making the texture first if it has none of that level.
   *
   * @throws RangeError for a level R09 does not serve, or a run that is not one of its cells.
   */
  survey(change: Pick<FieldBlockAnswer, "fieldLevel" | "newlySurveyed">): void {
    const { fieldLevel: level, newlySurveyed: runs } = change;
    if (!isFieldLevel(level)) {
      throw new RangeError(`a coarse field is at level 5 to 8, not ${String(level)}`);
    }
    const count = fieldCellCount(level);
    for (let r = 0; r + 1 < runs.length; r += 2) {
      const start = runs[r] ?? 0;
      const end = runs[r + 1] ?? 0;
      if (!(start < end && end <= count)) {
        throw new RangeError(
          `cells ${String(start)}..${String(end)} are not a run of level ${String(level)}'s`,
        );
      }
    }
    // A block that surveys nothing changes nothing a texture of its level holds.
    if (runs.length < 2 && this.#texture !== null && this.#level === level) {
      return;
    }
    const texels = this.#texelsOf(level);
    const n = 2 ** level;
    for (let r = 0; r + 1 < runs.length; r += 2) {
      for (let index = runs[r] ?? 0; index < (runs[r + 1] ?? 0); index += 1) {
        const cell = fieldCellAt(level, index);
        texels[(cell.face * n + cell.j) * n + cell.i] = COVERAGE_MASK_SURVEYED;
      }
    }
    this.#write(level, texels);
  }

  /**
   * Makes the texture again from the cells held, after a device loss; nothing where it holds none.
   *
   * @remarks
   * The lost engine's handle is released first, which `ResilientEngine` drops for a handle a lost
   * engine made.
   */
  restore(): void {
    const level = this.#level;
    const texels = this.#texels;
    if (level === null || texels === null) {
      return;
    }
    if (this.#texture !== null) {
      this.#engine.releaseTexture(this.#texture);
      this.#texture = null;
    }
    this.#write(level, texels);
  }

  /** Releases the texture, if it has one, and forgets the cells it held. */
  release(): void {
    if (this.#texture !== null) {
      this.#engine.releaseTexture(this.#texture);
    }
    this.#texture = null;
    this.#level = null;
    this.#texels = null;
  }

  /** The texels held at `level`, all unsurveyed where the mask held another level or none. */
  #texelsOf(level: number): Uint8Array {
    if (this.#level !== level) {
      this.release();
    }
    this.#texels ??= new Uint8Array(fieldCellCount(level));
    return this.#texels;
  }

  #write(level: number, texels: Uint8Array): void {
    const side = 2 ** level;
    if (this.#texture === null) {
      this.#texture = this.#engine.createTexture(coverageMaskTextureSpec(this.#name, level));
      this.#level = level;
    }
    this.#engine.writeTexture(
      this.#texture,
      { x: 0, y: 0, z: 0 },
      { width: side, height: side, depthOrArrayLayers: 6 },
      texels,
    );
  }
}
