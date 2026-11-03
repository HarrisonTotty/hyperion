/**
 * The disc's class-map surface (plan R07, T8.b; Design note 24): the layout of R10's coarse
 * class-weights map as `shaders/bodyDisc.wgsl` reads it, its construction and its CPU twin.
 *
 * @remarks
 * A class map holds, for each cell of R05's cube sphere at one level, the weight of each of the
 * body's classes over the cell, surveyed cells only (R10 Design note 8; R10.T10.d fills it). The
 * texture is a 2D array of {@link CLASS_MAP_FORMAT} texels, (N + 2) × (N + 2) per face (R10.T10.d,
 * decision-r07-t8b): the face's N × N texels and a one-texel gutter about them. Face texel (i, j)
 * of face f, stored at (i + 1, j + 1), is the cell s ∈ [i ÷ N, (i + 1) ÷ N),
 * t ∈ [j ÷ N, (j + 1) ÷ N) of R05's face coordinates (u, v) (`xyzToFaceUv`) under S2's quadratic
 * warp (`uvToSt`), so with N = 2^L it is the quadtree's cell (f, L, i, j). The gutter's sides hold
 * the edge texels of the neighbouring faces across each edge, and its four corners no weight. Class
 * k's weight is channel k mod 4 of layer f + 6 ⌊k ÷ 4⌋. An unsurveyed texel holds no weight, and a
 * surveyed one weights that sum to its surveyed share (1 where it is surveyed whole).
 *
 * At a point the disc shades with each class's law by its share and with `elsewhere` by the rest
 * ({@link classMapSharesAt}), through a survey-masked bilinear reconstruction: a point in an
 * unsurveyed texel takes `elsewhere` alone, so the disc shows no pattern past the survey's cells;
 * a point in a surveyed texel takes the share vector (`elsewhere`'s 1 − Σ w and each class's w,
 * {@link surfaceShares}) interpolated over the four nearest texel centres, counting surveyed texels
 * alone and renormalised, so the cells show no blocks; the gutter carries it across a face's edge.
 *
 * A map may carry each surveyed texel's maturity and each class's albedo nodes (plan R10, R10.T1.c's
 * client half; decision-r10-t1c-client-scope §2.3): the texel's unorm16 code (`bake::encode_maturity`)
 * in six more layers after the weights', one a face, its high byte in channel r and its low byte in
 * g, the gutter as the weights'. A class drawn from its nodes then takes as its A, at a point of a
 * surveyed texel, its nodes' albedo at each of the four texels' codes (`maturityNodes.ts`) by the
 * very bilinear weights the shares take, those of the surveyed texels alone, renormalised
 * ({@link classMapAlbedoAt}); the shares stay as they are.
 *
 * A map may carry a fine page (lane fix-coast-class-map; R10.T15.b.2's finding; {@link ClassMapFine}):
 * the texels whose field cells' classes it would draw apart, a coast's, refined to the field's level
 * as tiles in the layers past the others, behind an index layer a face. The reconstruction then reads
 * a refined texel's cells, and a texel beside one its cells nearest the point, so that a coast is
 * drawn along the field's cells as the terrain draws it, not at one class mix a level-6 texel.
 */
import type { ClassMapDiscSurface, DiscNormalMap, DiscSurface } from "../appearance/bodyAppearance";
import type { PhotometricLaw } from "../appearance/law";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { TextureSpec } from "../engine/memory";
import { MAX_TEXTURE_ARRAY_LAYERS } from "../engine/platform";
import type { RenderEngine, TextureHandle } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import { faceUvToXyz, stToUv, uvToSt, type Xyz, xyzToFaceUv } from "../terrain/cube";
import {
  MATURITY_CODE_MAX,
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  maturitySegment,
} from "../terrain/maturityNodes";
import type { Face } from "../terrain/patchKey";

/** The classes one disc's class map may hold; the record and the shader carry this many laws. */
export const MAX_DISC_CLASSES = 16;

/** The class map's texel format: four classes' weights, 0 to 1 in steps of 1 ÷ 255. */
export const CLASS_MAP_FORMAT = "rgba8unorm" as const;

/** Classes per layer: one in each channel. */
export const CLASSES_PER_LAYER = 4;

/** The cube sphere's faces. */
const FACES = 6;

/** The cube sphere's faces, in order. */
const FACE_INDICES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The largest weight a texel's byte holds. */
const UNORM_MAX = 255;

/**
 * How far past a face's edge, in face coordinates, the gutter looks for the neighbouring face's
 * texel: a nudge off the edge, where a point lies on both faces, onto the neighbour's side.
 */
const ACROSS_EDGE = 1e-9;

/** The layers of a class map of `classes` classes: six faces for every four classes. */
export function classMapLayers(classes: number): number {
  return FACES * Math.ceil(classes / CLASSES_PER_LAYER);
}

/**
 * The layers a class map's texture takes: its weights' ({@link classMapLayers}), six more, one a
 * face, where it carries its texels' maturity (R10.T1.c's client half), and where it carries a fine
 * page, its index's six and its pages' ({@link classMapIndexLayer}, {@link classMapFineLayer}).
 */
export function classMapTexelLayers(
  map: Pick<ClassMapTexels, "classes" | "maturity" | "fine">,
): number {
  const maturity = map.maturity !== undefined;
  const base = classMapLayers(map.classes) + (maturity ? FACES : 0);
  return map.fine === undefined
    ? base
    : base + FACES + map.fine.pages * finePageLayers(map.classes, maturity);
}

/**
 * The layer of `face`'s maturity codes in a map of `classes` classes that carries them: past the
 * weights' layers, face + 6 ⌈classes ÷ 4⌉, as `class_map_maturity` reads it in the shader.
 */
export function classMapMaturityLayer(face: Face, classes: number): number {
  return classMapLayers(classes) + face;
}

/** The channel of a maturity layer that holds a code's high byte, r; its low byte is in the next, g. */
export const MATURITY_HIGH_CHANNEL = 0;

/** The texels along a layer's side for N texels along a face's: N and a gutter texel each side. */
export function classMapSide(faceTexels: number): number {
  return faceTexels + 2;
}

/**
 * The layer of `face`'s fine-page index in a map of `classes` classes that carries a fine page:
 * past the weights' layers and the maturity's, face + 6 ⌈classes ÷ 4⌉ (+ 6 under a maturity), as
 * `fine_tile` reads it in the shader.
 */
export function classMapIndexLayer(face: Face, classes: number, maturity: boolean): number {
  return classMapLayers(classes) + (maturity ? FACES : 0) + face;
}

/** The layers a fine page's page takes: one for every four classes, and one for its codes. */
function finePageLayers(classes: number, maturity: boolean): number {
  return Math.ceil(classes / CLASSES_PER_LAYER) + (maturity ? 1 : 0);
}

/**
 * The layer of a fine page's page `page`, its classes 4g to 4g + 3 at `group` g, or its codes at
 * `group` ⌈classes ÷ 4⌉: past the index's six layers, page after page.
 */
export function classMapFineLayer(
  page: number,
  group: number,
  classes: number,
  maturity: boolean,
): number {
  return (
    classMapIndexLayer(0, classes, maturity) +
    FACES +
    page * finePageLayers(classes, maturity) +
    group
  );
}

/** The tiles along a page's side: ⌊(N + 2) ÷ (r + 2)⌋, each tile r + 2 texels a side. */
export function classMapTilesPerRow(faceTexels: number, fineSide: number): number {
  return Math.floor(classMapSide(faceTexels) / (fineSide + 2));
}

/**
 * The most tiles a map's fine page may hold: as many pages as fit in {@link MAX_TEXTURE_ARRAY_LAYERS}
 * past the map's other layers, each {@link classMapTilesPerRow}² tiles.
 */
export function classMapFineCapacity(
  faceTexels: number,
  classes: number,
  maturity: boolean,
  fineSide: number,
): number {
  const base = classMapIndexLayer(0, classes, maturity) + FACES;
  const pages = Math.max(
    0,
    Math.floor((MAX_TEXTURE_ARRAY_LAYERS - base) / finePageLayers(classes, maturity)),
  );
  return pages * classMapTilesPerRow(faceTexels, fineSide) ** 2;
}

/**
 * A class map's maturity, beside its codes in its texels' data (plan R10, R10.T1.c's client half):
 * each class's albedo nodes at band level 0, and which classes are drawn from them.
 */
export interface ClassMapMaturity {
  /**
   * Class c's node j at 3 (K c + j), K = `MATURITY_NODES`: its normal albedo in the display
   * channels r, g and b (the module's R, V and B), unscaled for the figure; all 0 for a class not
   * drawn from its nodes.
   */
  readonly nodes: Float32Array;
  /**
   * Whether each class is drawn from its nodes, in its order: a class of a named substance, with a
   * law of its own, whose A is then its node K − 1, its albedo at the body's background maturity;
   * a class that shades with `elsewhere` is not.
   */
  readonly drawn: ReadonlyArray<boolean>;
}

/** A class map's texels as they are uploaded. */
export interface ClassMapTexels {
  /** N, the texels along each face's side, the gutter apart. */
  readonly faceTexels: number;
  /** The body's classes, one law each. */
  readonly classes: number;
  /**
   * Layer after layer ({@link classMapTexelLayers}), each N + 2 rows y of N + 2 texels x
   * ({@link classMapSide}), four bytes a texel: class 4g + c's weight × 255 in channel c of layer
   * f + 6g; then, where the map carries its maturity, face f's codes in layer
   * {@link classMapMaturityLayer}, each surveyed texel's code ⌊code ÷ 256⌋ in channel r and its
   * code mod 256 in g, b and a 0, 0 at every unsurveyed texel. Face texel (i, j) is at (i + 1, j + 1);
   * the gutter's sides hold the neighbouring faces' edge texels, and its corners nothing.
   */
  readonly data: Uint8Array;
  /** Its maturity's nodes (R10.T1.c's client half); absent for a map without one. */
  readonly maturity?: ClassMapMaturity;
  /** Its fine page, its texels refined to the field's level; absent for a map without one. */
  readonly fine?: ClassMapFine;
}

/**
 * A class map's fine page (lane fix-coast-class-map; R10.T15.b.2's finding): the texels whose field
 * cells' classes it would draw apart, each refined to the field's level as a tile in the layers past
 * the map's own.
 *
 * @remarks
 * Each refined texel's r × r cells, r = 2^(L − 6) of a field of level L, are a tile of (r + 2) ×
 * (r + 2) texels: its cells at (1, 1) to (r, r), in the weights' layout (class 4g + c in channel c of
 * its page's layer g), and a one-texel gutter of the cells about it on the face's grid of N r texels
 * a side, as the reconstruction reads them: a refined neighbour's own cell, an unrefined
 * neighbour's texel as it is stored, across a face's edge the neighbouring face's, and at a cube's
 * corner nothing. Under a maturity each cell's code is in the page's last layer, as the texels'
 * codes. Tile t is in page ⌊t ÷ T²⌋, T = {@link classMapTilesPerRow}, at ((q mod T)(r + 2),
 * ⌊q ÷ T⌋ (r + 2)), q = t mod T². The index's layer of face f ({@link classMapIndexLayer}) holds at
 * each refined texel t + 1, its high byte in r and its low in g, and r in b; 0 elsewhere, the gutter
 * included, so that across a face's edge a refined neighbour counts as its texel.
 *
 * A page at its cells' corners ({@link ClassMapFine.corners}, from the crate's `REFINED_CORNERS_FROM`)
 * holds in each tile, in place of its cells and gutter, the (r + 1)² corners of its cells at (0, 0)
 * to (r, r), the vertices of the terrain's patch of level L − 6 there, all 0 at a corner not
 * surveyed, and cell (a, b)'s survey in channel a of the page's first layer at (r + 1, b), 255 for a
 * surveyed cell and 0 for another; its index holds 255 in a, which a page at the centres leaves 0.
 * The reconstruction then draws a refined cell bilinearly between its four corners, as that patch
 * draws its quad, a corner not surveyed as `elsewhere` alone.
 */
export interface ClassMapFine {
  /**
   * Whether its tiles hold their cells' corners rather than the cells themselves (the crate's
   * `REFINED_CORNERS_FROM`); absent for a page at the cells' centres.
   */
  readonly corners?: true;
  /** r, the field cells along a refined texel's side: 2 or 4. */
  readonly side: number;
  /** The tiles it holds, one a refined texel. */
  readonly tiles: number;
  /** Its pages: ⌈tiles ÷ T²⌉. */
  readonly pages: number;
}

/** One refined texel as {@link packClassMap} takes it: its cells' weights and, under a maturity, codes. */
export interface ClassMapRefinedInput {
  readonly texel: ClassMapTexel;
  /**
   * Its r² cells in rising (j, i), each its weight per class, each in [0, 1] and summing to at most
   * 1, or `null` for a cell not surveyed: a page at its cells' centres'.
   */
  readonly cells?: ReadonlyArray<ReadonlyArray<number> | null>;
  /** Whether each of its r² cells, in rising (j, i), is surveyed: a page at its cells' corners'. */
  readonly surveyed?: ReadonlyArray<boolean>;
  /**
   * The (r + 1)² corners of its cells in rising (j, i), each its weight per class as a cell's, or
   * `null` for a corner not surveyed: a page at its cells' corners'.
   */
  readonly corners?: ReadonlyArray<ReadonlyArray<number> | null>;
  /**
   * Each cell's, or under a page at its corners each corner's, maturity code alike, an integer 0 to
   * 65,535, for a map that carries a maturity.
   */
  readonly codes?: ReadonlyArray<number>;
}

/** What {@link packClassMap} takes to pack a map's fine page. */
export interface ClassMapFineInput {
  /** r, the field cells along a texel's side: 2 or 4. */
  readonly side: number;
  /**
   * Where its refined texels are read: `"corners"` at their cells' corners (the crate's
   * `REFINED_CORNERS_FROM`), each giving its `surveyed` and `corners`; absent at their centres, each
   * giving its `cells`.
   */
  readonly lattice?: "corners";
  /**
   * The refined texels, each of a surveyed texel and at most once. Past
   * {@link classMapFineCapacity} those whose cells stand furthest from their texel's mean are
   * kept, by the sum over their cells and classes of |cell's weight − texel's|, ties to the earlier.
   */
  readonly refined: ReadonlyArray<ClassMapRefinedInput>;
}

/** What {@link packClassMap} takes to pack a map's maturity: its codes, and its nodes as it holds them. */
export interface ClassMapMaturityInput extends ClassMapMaturity {
  /**
   * A surveyed texel's maturity code, an integer 0 to 65,535 (`bake::encode_maturity`: 0 the
   * body's background maturity, 65,535 fresh ground); asked of each texel `weightsOf` surveys.
   */
  readonly codeOf: (texel: ClassMapTexel) => number;
}

/** One texel of a class map: a face and a cell on it. */
export interface ClassMapTexel {
  readonly face: Face;
  /** The column, from 0 at u = −1. */
  readonly i: number;
  /** The row, from 0 at v = −1. */
  readonly j: number;
}

/** A cell coordinate s ∈ [0, 1] as a texel index of N. */
function texelIndex(s: number, faceTexels: number): number {
  return Math.min(Math.max(Math.floor(s * faceTexels), 0), faceTexels - 1);
}

/**
 * The texel holding a body-fixed direction: R05's face and (u, v) there, warped to (s, t) and
 * scaled by N, as `class_map_texel` does in the shader.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 */
export function classMapTexelOf(direction: Xyz, faceTexels: number): ClassMapTexel {
  const { face, u, v } = xyzToFaceUv(direction);
  return {
    face,
    i: texelIndex(uvToSt(u), faceTexels),
    j: texelIndex(uvToSt(v), faceTexels),
  };
}

/** The byte offset of class `k`'s weight at stored texel (x, y) of `face`'s layers, gutter included. */
function storedOffset(map: ClassMapTexels, face: Face, x: number, y: number, k: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = face + FACES * Math.floor(k / CLASSES_PER_LAYER);
  return ((layer * side + y) * side + x) * CLASSES_PER_LAYER + (k % CLASSES_PER_LAYER);
}

/** The byte offset of class `k`'s weight at a face texel. */
function weightOffset(map: ClassMapTexels, texel: ClassMapTexel, k: number): number {
  return storedOffset(map, texel.face, texel.i + 1, texel.j + 1, k);
}

/** The byte offset of the high byte of the maturity code at stored texel (x, y) of `face`. */
function codeOffset(map: ClassMapTexels, face: Face, x: number, y: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = classMapMaturityLayer(face, map.classes);
  return ((layer * side + y) * side + x) * CLASSES_PER_LAYER + MATURITY_HIGH_CHANNEL;
}

/** The maturity code at stored texel (x, y) of `face`, from its two bytes, as the shader reads it. */
function storedCode(map: ClassMapTexels, face: Face, x: number, y: number): number {
  const at = codeOffset(map, face, x, y);
  return (map.data[at] ?? 0) * (UNORM_MAX + 1) + (map.data[at + 1] ?? 0);
}

/**
 * Checks a map's maturity against its classes: the nodes K × 3 a class, a flag a class.
 *
 * @throws Error where either does not fit `classes`.
 */
function checkMaturity(maturity: ClassMapMaturity, classes: number): void {
  if (maturity.nodes.length !== 3 * MATURITY_NODES * classes || maturity.drawn.length !== classes) {
    throw new Error(
      `a class map of ${classes} classes has ${3 * MATURITY_NODES * classes} node values and as many flags as classes, not ${maturity.nodes.length} and ${maturity.drawn.length}`,
    );
  }
}

/**
 * One texel's weights as bytes that keep their sum: each w × 255 rounded down, and the bytes that
 * leaves short of round(255 Σ w) given to the largest remainders (ties to the lower class).
 */
function weightBytes(weights: ReadonlyArray<number>): number[] {
  const scaled = weights.map((w) => Math.min(Math.max(w, 0), 1) * UNORM_MAX);
  const total = Math.min(UNORM_MAX, Math.round(scaled.reduce((sum, w) => sum + w, 0)));
  const bytes = scaled.map((w) => Math.floor(w));
  let short = total - bytes.reduce((sum, b) => sum + b, 0);
  const order = scaled
    .map((w, k) => ({ k, remainder: w - Math.floor(w) }))
    .toSorted((a, b) => (b.remainder !== a.remainder ? b.remainder - a.remainder : a.k - b.k));
  for (const { k } of order) {
    if (short <= 0) {
      break;
    }
    bytes[k] = (bytes[k] ?? 0) + 1;
    short -= 1;
  }
  return bytes;
}

/**
 * The face texel across the edge from gutter texel (x, y) of `face`, x or y being 0 or N + 1 and
 * the other inside the face's N: the neighbouring face's texel at the point just past the edge
 * beside that edge texel's centre, which is the neighbour's own edge texel.
 */
function acrossEdge(face: Face, x: number, y: number, faceTexels: number): ClassMapTexel {
  const n = faceTexels;
  const past = 1 + ACROSS_EDGE;
  const along = (index: number): number => stToUv((index - 0.5) / n);
  let u: number;
  let v: number;
  if (x === 0 || x === n + 1) {
    u = x === 0 ? -past : past;
    v = along(y);
  } else {
    u = along(x);
    v = y === 0 ? -past : past;
  }
  return classMapTexelOf(faceUvToXyz(face, u, v), n);
}

/**
 * Visits each side texel of the one-texel gutter about every face of a cube-sphere map of N texels a
 * face side, as the class map (R10.T10.d) and the normal map (R10.T10.g, `discNormals.ts`) lay it
 * out: stored texel (x, y), x or y 0 or N + 1 and the other 1 to N, with the face texel across the
 * edge whose value it holds, the neighbouring face's own edge texel there. The four corners are not
 * visited.
 */
export function forEachGutterTexel(
  faceTexels: number,
  visit: (face: Face, x: number, y: number, from: ClassMapTexel) => void,
): void {
  for (const face of FACE_INDICES) {
    for (let at = 1; at <= faceTexels; at += 1) {
      for (const [x, y] of [
        [0, at],
        [faceTexels + 1, at],
        [at, 0],
        [at, faceTexels + 1],
      ] as const) {
        visit(face, x, y, acrossEdge(face, x, y, faceTexels));
      }
    }
  }
}

/**
 * Packs a class map from each texel's weights, in the layout the shader reads, its gutter from
 * each face's neighbours.
 *
 * @remarks
 * A texel's weights are each class's share of it, summing to its surveyed share: 1 for a texel
 * surveyed whole (R10.T2's `ClassWeights`, whose bytes sum to 255, pass as byte ÷ 255), less for one
 * the survey covers in part, whose rest `elsewhere` takes. They are quantised to bytes keeping that
 * sum, so a texel surveyed whole leaves no share to `elsewhere`.
 *
 * With `maturity` (R10.T1.c's client half), each surveyed texel's code goes in its face's maturity
 * layer, its gutter as the weights', and the map carries the nodes; the weights' bytes are as
 * without it.
 *
 * With `fine` (lane fix-coast-class-map), its refined texels' cells go in the map's fine page, as
 * many as {@link classMapFineCapacity} allows ({@link ClassMapFine}); the texels' own are as
 * without it.
 *
 * @param weightsOf - A texel's weight per class, each in [0, 1], or `null` for an unsurveyed texel.
 * @param maturity - The texels' maturity codes and the classes' nodes, for a map that carries them.
 * @param fine - The texels refined to the field's level, for a map that carries a fine page.
 * @throws Error if N is not a positive integer, or `classes` is not 1 to {@link MAX_DISC_CLASSES},
 *   or a texel's weights are not `classes` many; with `maturity`, if its nodes are not K × 3 a
 *   class or its flags one a class, or a code is not an integer 0 to 65,535; with `fine`, if r is
 *   not 2 or 4, or a refined texel is not a surveyed face texel, is given twice, or its cells or
 *   codes are not r² or a cell's weights not `classes` many.
 */
export function packClassMap(
  faceTexels: number,
  classes: number,
  weightsOf: (texel: ClassMapTexel) => ReadonlyArray<number> | null,
  maturity?: ClassMapMaturityInput,
  fine?: ClassMapFineInput,
): ClassMapTexels {
  if (!(Number.isInteger(faceTexels) && faceTexels > 0)) {
    throw new Error(`a class map has a positive whole number of texels a side, got ${faceTexels}`);
  }
  if (!(Number.isInteger(classes) && classes > 0 && classes <= MAX_DISC_CLASSES)) {
    throw new Error(`a class map holds 1 to ${MAX_DISC_CLASSES} classes, got ${classes}`);
  }
  if (maturity !== undefined) {
    checkMaturity(maturity, classes);
  }
  const side = classMapSide(faceTexels);
  const held: ClassMapMaturity | undefined =
    maturity === undefined ? undefined : { nodes: maturity.nodes, drawn: maturity.drawn };
  const kept =
    fine === undefined ? [] : keptRefined(faceTexels, classes, weightsOf, maturity, fine);
  const page: ClassMapFine | undefined =
    fine === undefined || kept.length === 0
      ? undefined
      : {
          side: fine.side,
          tiles: kept.length,
          pages: Math.ceil(kept.length / classMapTilesPerRow(faceTexels, fine.side) ** 2),
          ...(fine.lattice === "corners" ? { corners: true as const } : {}),
        };
  const layers = classMapTexelLayers({
    classes,
    ...(held === undefined ? {} : { maturity: held }),
    ...(page === undefined ? {} : { fine: page }),
  });
  const map: ClassMapTexels = {
    faceTexels,
    classes,
    data: new Uint8Array(layers * side * side * CLASSES_PER_LAYER),
    ...(held === undefined ? {} : { maturity: held }),
    ...(page === undefined ? {} : { fine: page }),
  };
  for (const face of FACE_INDICES) {
    for (let j = 0; j < faceTexels; j += 1) {
      for (let i = 0; i < faceTexels; i += 1) {
        const texel = { face, i, j };
        const weights = weightsOf(texel);
        if (weights === null) {
          continue;
        }
        if (weights.length !== classes) {
          throw new Error(
            `texel (${face}, ${i}, ${j}) has ${weights.length} weights for ${classes} classes`,
          );
        }
        weightBytes(weights).forEach((byte, k) => {
          map.data[weightOffset(map, texel, k)] = byte;
        });
        if (maturity !== undefined) {
          putCode(map, texel, maturity.codeOf(texel));
        }
      }
    }
  }
  // The gutter: each side's texels from the neighbouring face's edge, the corners left empty.
  forEachGutterTexel(faceTexels, (face, x, y, from) => {
    for (let k = 0; k < classes; k += 1) {
      map.data[storedOffset(map, face, x, y, k)] = map.data[weightOffset(map, from, k)] ?? 0;
    }
    if (maturity !== undefined) {
      const to = codeOffset(map, face, x, y);
      const of = codeOffset(map, from.face, from.i + 1, from.j + 1);
      map.data[to] = map.data[of] ?? 0;
      map.data[to + 1] = map.data[of + 1] ?? 0;
    }
  });
  if (page !== undefined) {
    if (page.corners === true) {
      packFineCorners(map, page, kept);
    } else {
      packFinePage(map, page, kept);
    }
  }
  return map;
}

/**
 * The refined texels a fine page keeps, checked: all of them where they fit, and otherwise those
 * whose cells stand furthest from their texel's mean ({@link ClassMapFineInput.refined}), in their
 * given order.
 *
 * @throws Error as {@link packClassMap} does for its `fine`.
 */
function keptRefined(
  faceTexels: number,
  classes: number,
  weightsOf: (texel: ClassMapTexel) => ReadonlyArray<number> | null,
  maturity: ClassMapMaturityInput | undefined,
  fine: ClassMapFineInput,
): ReadonlyArray<ClassMapRefinedInput> {
  const r = fine.side;
  if (r !== 2 && r !== 4) {
    throw new Error(`a fine page refines a texel to 2 or 4 cells a side, not ${r}`);
  }
  const seen = new Set<number>();
  const scored = fine.refined.map((refined, order) => {
    const { face, i, j } = refined.texel;
    const own = weightsOf(refined.texel);
    if (!(i >= 0 && i < faceTexels && j >= 0 && j < faceTexels) || own === null) {
      throw new Error(`refined texel (${face}, ${i}, ${j}) is not a surveyed texel of the map`);
    }
    const key = (face * faceTexels + j) * faceTexels + i;
    if (seen.has(key)) {
      throw new Error(`refined texel (${face}, ${i}, ${j}) is given twice`);
    }
    seen.add(key);
    const readings =
      fine.lattice === "corners" ? cornersOf(refined, r, maturity) : cellsOf(refined, r, maturity);
    let score = 0;
    for (const cell of readings) {
      if (cell === null) {
        continue;
      }
      if (cell.length !== classes) {
        throw new Error(
          `a cell of refined texel (${face}, ${i}, ${j}) has ${cell.length} weights for ${classes} classes`,
        );
      }
      cell.forEach((w, k) => {
        score += Math.abs(w - (own[k] ?? 0));
      });
    }
    return { refined, order, score };
  });
  const capacity = classMapFineCapacity(faceTexels, classes, maturity !== undefined, r);
  if (scored.length <= capacity) {
    return fine.refined;
  }
  return scored
    .toSorted((a, b) => (b.score !== a.score ? b.score - a.score : a.order - b.order))
    .slice(0, capacity)
    .toSorted((a, b) => a.order - b.order)
    .map(({ refined }) => refined);
}

/**
 * A refined texel's r² cells, as a fine page at its cells' centres packs them.
 *
 * @throws Error as {@link packClassMap} does for its `fine`.
 */
function cellsOf(
  refined: ClassMapRefinedInput,
  r: number,
  maturity: ClassMapMaturityInput | undefined,
): ReadonlyArray<ReadonlyArray<number> | null> {
  const { face, i, j } = refined.texel;
  const cells = refined.cells ?? [];
  if (cells.length !== r * r || (maturity !== undefined && refined.codes?.length !== r * r)) {
    throw new Error(
      `refined texel (${face}, ${i}, ${j}) has ${cells.length} cells and ${refined.codes?.length ?? 0} codes, not ${r * r}`,
    );
  }
  return cells;
}

/**
 * A refined texel's (r + 1)² corners, as a fine page at its cells' corners packs them, its r² cells'
 * survey checked beside them.
 *
 * @throws Error as {@link packClassMap} does for its `fine`.
 */
function cornersOf(
  refined: ClassMapRefinedInput,
  r: number,
  maturity: ClassMapMaturityInput | undefined,
): ReadonlyArray<ReadonlyArray<number> | null> {
  const { face, i, j } = refined.texel;
  const corners = refined.corners ?? [];
  const surveyed = refined.surveyed ?? [];
  const count = (r + 1) ** 2;
  if (
    surveyed.length !== r * r ||
    corners.length !== count ||
    (maturity !== undefined && refined.codes?.length !== count)
  ) {
    throw new Error(
      `refined texel (${face}, ${i}, ${j}) has ${surveyed.length} cells, ${corners.length} corners and ${refined.codes?.length ?? 0} codes, not ${r * r} and ${count}`,
    );
  }
  return corners;
}

/** The byte offset of the index's texel at stored (x, y) of `face`'s index layer. */
function indexOffset(map: ClassMapTexels, face: Face, x: number, y: number): number {
  const side = classMapSide(map.faceTexels);
  const layer = classMapIndexLayer(face, map.classes, map.maturity !== undefined);
  return ((layer * side + y) * side + x) * CLASSES_PER_LAYER;
}

/** The tile of the texel at stored (x, y) of `face`, −1 for none, as `fine_tile` reads it. */
function storedTile(map: ClassMapTexels, face: Face, x: number, y: number): number {
  if (map.fine === undefined) {
    return -1;
  }
  const at = indexOffset(map, face, x, y);
  return (map.data[at] ?? 0) * (UNORM_MAX + 1) + (map.data[at + 1] ?? 0) - 1;
}

/**
 * The byte offset of channel 0 of a tile's texel at (x, y) of its r + 2 a side, in its page's layer
 * `group`: its classes 4 · group to 4 · group + 3, or at group ⌈classes ÷ 4⌉ its codes.
 */
function tileOffset(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  x: number,
  y: number,
  group: number,
): number {
  const side = classMapSide(map.faceTexels);
  const perRow = classMapTilesPerRow(map.faceTexels, fine.side);
  const q = tile % (perRow * perRow);
  const x0 = (q % perRow) * (fine.side + 2);
  const y0 = Math.floor(q / perRow) * (fine.side + 2);
  const layer = classMapFineLayer(
    Math.floor(tile / (perRow * perRow)),
    group,
    map.classes,
    map.maturity !== undefined,
  );
  return ((layer * side + y0 + y) * side + x0 + x) * CLASSES_PER_LAYER;
}

/** The weight bytes' groups of a map of `classes` classes, the codes' group after them. */
function weightGroups(classes: number): number {
  return Math.ceil(classes / CLASSES_PER_LAYER);
}

/**
 * Writes the fine page of `map`: each kept texel's tile index, its cells, and its tile's gutter
 * from the cells about it ({@link ClassMapFine}).
 */
function packFinePage(
  map: ClassMapTexels,
  page: ClassMapFine,
  kept: ReadonlyArray<ClassMapRefinedInput>,
): void {
  const r = page.side;
  const groups = weightGroups(map.classes);
  const coded = map.maturity !== undefined;
  kept.forEach(({ texel, cells, codes }, tile) => {
    const at = indexOffset(map, texel.face, texel.i + 1, texel.j + 1);
    map.data[at] = Math.floor((tile + 1) / (UNORM_MAX + 1));
    map.data[at + 1] = (tile + 1) % (UNORM_MAX + 1);
    map.data[at + 2] = r;
    (cells ?? []).forEach((cell, n) => {
      if (cell === null) {
        return;
      }
      const x = (n % r) + 1;
      const y = Math.floor(n / r) + 1;
      weightBytes(cell).forEach((byte, k) => {
        const group = Math.floor(k / CLASSES_PER_LAYER);
        map.data[tileOffset(map, page, tile, x, y, group) + (k % CLASSES_PER_LAYER)] = byte;
      });
      if (coded) {
        const code = codes?.[n] ?? Number.NaN;
        if (!(Number.isInteger(code) && code >= 0 && code <= MATURITY_CODE_MAX)) {
          throw new Error(
            `a cell of refined texel (${texel.face}, ${texel.i}, ${texel.j}) has maturity code ${code}, not an integer 0 to ${MATURITY_CODE_MAX}`,
          );
        }
        const to = tileOffset(map, page, tile, x, y, groups);
        map.data[to] = Math.floor(code / (UNORM_MAX + 1));
        map.data[to + 1] = code % (UNORM_MAX + 1);
      }
    });
  });
  // Each tile's gutter, after every tile's cells: the cells about it on the face's grid of N r.
  const fineN = map.faceTexels * r;
  const layers = groups + (coded ? 1 : 0);
  kept.forEach(({ texel }, tile) => {
    for (let y = 0; y <= r + 1; y += 1) {
      for (let x = 0; x <= r + 1; x += 1) {
        if (x > 0 && x <= r && y > 0 && y <= r) {
          continue;
        }
        const from = fineCellAt(texel.face, r * texel.i + x - 1, r * texel.j + y - 1, fineN);
        if (from === null) {
          continue;
        }
        const ci = Math.floor(from.i / r);
        const cj = Math.floor(from.j / r);
        const source = storedTile(map, from.face, ci + 1, cj + 1);
        for (let group = 0; group < layers; group += 1) {
          const to = tileOffset(map, page, tile, x, y, group);
          const of =
            source >= 0
              ? tileOffset(map, page, source, from.i - r * ci + 1, from.j - r * cj + 1, group)
              : group < groups
                ? storedOffset(map, from.face, ci + 1, cj + 1, group * CLASSES_PER_LAYER)
                : codeOffset(map, from.face, ci + 1, cj + 1);
          map.data.copyWithin(to, of, of + CLASSES_PER_LAYER);
        }
      }
    }
  });
}

/**
 * Writes the fine page of `map` at its refined texels' cells' corners ({@link ClassMapFine}): each
 * kept texel's tile index, its index's a 255, its corners from (0, 0) and its cells' survey at
 * (r + 1, j); no gutter, a corner reading no neighbour.
 */
function packFineCorners(
  map: ClassMapTexels,
  page: ClassMapFine,
  kept: ReadonlyArray<ClassMapRefinedInput>,
): void {
  const r = page.side;
  const groups = weightGroups(map.classes);
  const coded = map.maturity !== undefined;
  kept.forEach(({ texel, surveyed, corners, codes }, tile) => {
    const at = indexOffset(map, texel.face, texel.i + 1, texel.j + 1);
    map.data[at] = Math.floor((tile + 1) / (UNORM_MAX + 1));
    map.data[at + 1] = (tile + 1) % (UNORM_MAX + 1);
    map.data[at + 2] = r;
    map.data[at + 3] = UNORM_MAX;
    (surveyed ?? []).forEach((cell, n) => {
      map.data[tileOffset(map, page, tile, r + 1, Math.floor(n / r), 0) + (n % r)] = cell
        ? UNORM_MAX
        : 0;
    });
    (corners ?? []).forEach((corner, n) => {
      if (corner === null) {
        return;
      }
      const x = n % (r + 1);
      const y = Math.floor(n / (r + 1));
      weightBytes(corner).forEach((byte, k) => {
        const group = Math.floor(k / CLASSES_PER_LAYER);
        map.data[tileOffset(map, page, tile, x, y, group) + (k % CLASSES_PER_LAYER)] = byte;
      });
      if (coded) {
        const code = codes?.[n] ?? Number.NaN;
        if (!(Number.isInteger(code) && code >= 0 && code <= MATURITY_CODE_MAX)) {
          throw new Error(
            `a corner of refined texel (${texel.face}, ${texel.i}, ${texel.j}) has maturity code ${code}, not an integer 0 to ${MATURITY_CODE_MAX}`,
          );
        }
        const to = tileOffset(map, page, tile, x, y, groups);
        map.data[to] = Math.floor(code / (UNORM_MAX + 1));
        map.data[to + 1] = code % (UNORM_MAX + 1);
      }
    });
  });
}

/**
 * The cell of a face's grid of `n` a side at (i, j), or across the face's edge where one of the two
 * lies past it, the neighbouring face's cell there; `null` past a cube's corner, where both do.
 */
function fineCellAt(face: Face, i: number, j: number, n: number): ClassMapTexel | null {
  const inI = i >= 0 && i < n;
  const inJ = j >= 0 && j < n;
  if (inI && inJ) {
    return { face, i, j };
  }
  return inI || inJ ? acrossEdge(face, i + 1, j + 1, n) : null;
}

/**
 * Writes a face texel's maturity code as its two bytes, high then low.
 *
 * @throws Error for a code that is not an integer 0 to 65,535.
 */
function putCode(map: ClassMapTexels, texel: ClassMapTexel, code: number): void {
  if (!(Number.isInteger(code) && code >= 0 && code <= MATURITY_CODE_MAX)) {
    throw new Error(
      `texel (${texel.face}, ${texel.i}, ${texel.j}) has maturity code ${code}, not an integer 0 to ${MATURITY_CODE_MAX}`,
    );
  }
  const at = codeOffset(map, texel.face, texel.i + 1, texel.j + 1);
  map.data[at] = Math.floor(code / (UNORM_MAX + 1));
  map.data[at + 1] = code % (UNORM_MAX + 1);
}

/** The weight of each class at stored texel (x, y) of `face`'s layers, as the shader reads it. */
function storedWeights(map: ClassMapTexels, face: Face, x: number, y: number): number[] {
  return Array.from(
    { length: map.classes },
    (_, k) => (map.data[storedOffset(map, face, x, y, k)] ?? 0) / UNORM_MAX,
  );
}

/**
 * The weight of each class in the texel holding a body-fixed direction, byte ÷ 255: the texel's own,
 * unfiltered ({@link classMapSharesAt} reconstructs the shares between texels).
 */
export function classWeightsAt(map: ClassMapTexels, direction: Xyz): number[] {
  const texel = classMapTexelOf(direction, map.faceTexels);
  return storedWeights(map, texel.face, texel.i + 1, texel.j + 1);
}

/**
 * The share of each of {@link discSurfaceLaws}' laws at a point, from its classes' weights: the
 * weights, scaled down to sum to 1 where they sum to more, after `elsewhere`'s share 1 − Σ w (0
 * where they sum to 1 or more), as `texel_shares` does.
 */
export function surfaceShares(weights: ReadonlyArray<number>): number[] {
  const total = weights.reduce((sum, w) => sum + w, 0);
  const classes = total > 1 ? weights.map((w) => w / total) : [...weights];
  return [total >= 1 ? 0 : 1 - total, ...classes];
}

/**
 * The share of each of {@link discSurfaceLaws}' laws at a body-fixed direction under a class map,
 * by the survey-masked bilinear reconstruction `surface_at` computes (R10.T10.d,
 * decision-r07-t8b), at the field's level in a refined texel (lane fix-coast-class-map).
 *
 * @remarks
 * With (s, t) the direction's cell coordinates on its face and n = N: where the texel holding it,
 * (⌊n s⌋, ⌊n t⌋), holds no weight, `elsewhere` alone; otherwise each of the four texels whose
 * centres are nearest, from (⌊n s − ½⌋, ⌊n t − ½⌋), the gutter's among them at a face's edge, by its
 * bilinear weight in (n s − ½, n t − ½), those holding weight alone, each with its own shares
 * ({@link surfaceShares}), summed and divided by their bilinear weights' sum. The texel holding the
 * point is one of the four, with a bilinear weight of at least ¼, so the sum is never 0. At a
 * surveyed texel's centre the shares are its own.
 *
 * Under a fine page ({@link ClassMapFine}) the reconstruction reads its cells, r a texel's side:
 * - in a refined texel, the same rule on the grid of N r cells a face, within its tile: `elsewhere`
 *   alone where the cell holding the point is not surveyed, and otherwise the four nearest cells,
 *   the tile's gutter's among them, those surveyed alone;
 * - in another texel, a refined one of its four nearest stands by its cells at the point held
 *   within their centres' span, (n r s, n r t) clamped to ½ to r − ½ of the tile, each by its
 *   bilinear weight among them times the texel's, those surveyed alone; none of them surveyed
 *   there, it is left out. So the two rules meet at a refined texel's edge, where each
 *   takes half of the cell beside it and half of the texel's, and a coast is drawn along the
 *   field's cells. Across a face's edge a refined texel counts as its own texel.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 */
export function classMapSharesAt(map: ClassMapTexels, direction: Xyz): number[] {
  const elsewhere = (): number[] => [1, ...Array.from({ length: map.classes }, () => 0)];
  const terms = reconstructionTerms(map, direction);
  if (terms === null) {
    return elsewhere();
  }
  const shares = Array.from({ length: map.classes + 1 }, () => 0);
  let norm = 0;
  for (const { at, b } of terms) {
    surfaceShares(weightsAt(map, at)).forEach((share, m) => {
      shares[m] = (shares[m] ?? 0) + b * share;
    });
    norm += b;
  }
  return norm > 0 ? shares.map((share) => share / norm) : elsewhere();
}

/**
 * Where the survey-masked reconstruction reads a map at a direction: its face, the texel holding it
 * and the first of the four whose centres are nearest, with the bilinear fractions there
 * ({@link classMapSharesAt}'s remarks).
 */
interface ReconstructionCorners {
  readonly face: Face;
  /** The texel holding the direction, (⌊n s⌋, ⌊n t⌋), each held to the face's. */
  readonly holding: readonly [number, number];
  /** The first corner's face texel, (⌊n s − ½⌋, ⌊n t − ½⌋), −1 the gutter's. */
  readonly i0: number;
  readonly j0: number;
  /** The fractions (n s − ½ − i₀, n t − ½ − j₀). */
  readonly fx: number;
  readonly fy: number;
  /** (n s, n t). */
  readonly sN: number;
  readonly tN: number;
}

/** The texels the reconstruction reads at `direction`, as `surface_at` finds them. */
function reconstructionCorners(map: ClassMapTexels, direction: Xyz): ReconstructionCorners {
  const n = map.faceTexels;
  const { face, u, v } = xyzToFaceUv(direction);
  const sN = uvToSt(u) * n;
  const tN = uvToSt(v) * n;
  const holding = [
    Math.min(Math.max(Math.floor(sN), 0), n - 1),
    Math.min(Math.max(Math.floor(tN), 0), n - 1),
  ] as const;
  const x = sN - 0.5;
  const y = tN - 0.5;
  const i0 = Math.min(Math.max(Math.floor(x), -1), n - 1);
  const j0 = Math.min(Math.max(Math.floor(y), -1), n - 1);
  return { face, holding, i0, j0, fx: x - i0, fy: y - j0, sN, tN };
}

/** Corner (i₀ + di, j₀ + dj)'s bilinear weight at fractions (fx, fy). */
function cornerWeight(di: number, dj: number, fx: number, fy: number): number {
  return (di === 0 ? 1 - fx : fx) * (dj === 0 ? 1 - fy : fy);
}

/** Where a term of the reconstruction is stored: a texel of a face's layers, or a tile's. */
type StoredAt =
  | { readonly kind: "texel"; readonly face: Face; readonly x: number; readonly y: number }
  | { readonly kind: "tile"; readonly tile: number; readonly x: number; readonly y: number };

/** One term of the reconstruction: a stored texel that holds weight, and its weight there. */
interface ReconstructionTerm {
  readonly at: StoredAt;
  readonly b: number;
}

/**
 * The terms of the survey-masked reconstruction at `direction` ({@link classMapSharesAt}'s
 * remarks), in the shader's order; `null` where the texel or cell holding the point holds no
 * weight, and `elsewhere` takes it all.
 */
function reconstructionTerms(
  map: ClassMapTexels,
  direction: Xyz,
): ReadonlyArray<ReconstructionTerm> | null {
  const { face, holding, i0, j0, fx, fy, sN, tN } = reconstructionCorners(map, direction);
  const { fine } = map;
  const own = storedTile(map, face, holding[0] + 1, holding[1] + 1);
  if (fine !== undefined && own >= 0) {
    return fine.corners === true
      ? cornerCellTerms(map, fine, own, holding, sN, tN)
      : cellTerms(map, fine, own, holding, sN, tN);
  }
  const terms: ReconstructionTerm[] = [];
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const i = i0 + di;
    const j = j0 + dj;
    const surveyed = storedSurveyed(map, face, i + 1, j + 1);
    if (i === holding[0] && j === holding[1] && !surveyed) {
      return null;
    }
    if (!surveyed) {
      continue;
    }
    const b = cornerWeight(di, dj, fx, fy);
    const tile = storedTile(map, face, i + 1, j + 1);
    if (fine !== undefined && tile >= 0) {
      const clamped =
        fine.corners === true
          ? clampedCornerTerms(map, fine, tile, [i, j], sN, tN)
          : clampedCellTerms(map, fine, tile, [i, j], sN, tN);
      for (const term of clamped) {
        terms.push({ at: term.at, b: b * term.b });
      }
      continue;
    }
    terms.push({ at: { kind: "texel", face, x: i + 1, y: j + 1 }, b });
  }
  return terms;
}

/**
 * The terms in refined texel `texel`, tile `tile`, at (n s, n t) = (`sN`, `tN`): the four nearest
 * cells on the grid of N r, those surveyed, by their bilinear weights; `null` where the cell holding
 * the point is not surveyed.
 */
function cellTerms(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  texel: readonly [number, number],
  sN: number,
  tN: number,
): ReadonlyArray<ReconstructionTerm> | null {
  const r = fine.side;
  const x = sN * r - r * texel[0];
  const y = tN * r - r * texel[1];
  const held = (v: number): number => Math.min(Math.max(Math.floor(v), 0), r - 1);
  if (!surveyedAt(map, { kind: "tile", tile, x: held(x) + 1, y: held(y) + 1 })) {
    return null;
  }
  const first = (v: number): number => Math.min(Math.max(Math.floor(v - 0.5), -1), r - 1);
  const x0 = first(x);
  const y0 = first(y);
  const terms: ReconstructionTerm[] = [];
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const at: StoredAt = { kind: "tile", tile, x: x0 + di + 1, y: y0 + dj + 1 };
    if (surveyedAt(map, at)) {
      terms.push({ at, b: cornerWeight(di, dj, x - 0.5 - x0, y - 0.5 - y0) });
    }
  }
  return terms;
}

/**
 * The terms by which refined texel `texel`, tile `tile`, stands as a corner of another texel's
 * reconstruction at (`sN`, `tN`): its cells at the point held within their centres' span, by their
 * bilinear weights among its own cells, those surveyed alone; none where none of them is surveyed.
 */
function clampedCellTerms(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  texel: readonly [number, number],
  sN: number,
  tN: number,
): ReadonlyArray<ReconstructionTerm> {
  const r = fine.side;
  const within = (v: number): number => Math.min(Math.max(v, 0.5), r - 0.5) - 0.5;
  const x = within(sN * r - r * texel[0]);
  const y = within(tN * r - r * texel[1]);
  const x0 = Math.min(Math.floor(x), r - 2);
  const y0 = Math.min(Math.floor(y), r - 2);
  const terms: ReconstructionTerm[] = [];
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const at: StoredAt = { kind: "tile", tile, x: x0 + di + 1, y: y0 + dj + 1 };
    if (surveyedAt(map, at)) {
      terms.push({ at, b: cornerWeight(di, dj, x - x0, y - y0) });
    }
  }
  return terms;
}

/**
 * The terms in refined texel `texel`, tile `tile` of a fine page at its cells' corners, at (n s,
 * n t) = (`sN`, `tN`): the four corners of the cell holding the point by their bilinear weights, a
 * corner not surveyed as law 0 alone, as the terrain's patch of level L − 6 draws its quad there
 * from the weights it bakes, 0 at a vertex outside the survey; `null` where the cell holding the
 * point is not surveyed.
 */
function cornerCellTerms(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  texel: readonly [number, number],
  sN: number,
  tN: number,
): ReadonlyArray<ReconstructionTerm> | null {
  const r = fine.side;
  const x = sN * r - r * texel[0];
  const y = tN * r - r * texel[1];
  const held = (v: number): number => Math.min(Math.max(Math.floor(v), 0), r - 1);
  const cx = held(x);
  const cy = held(y);
  if (!cellSurveyed(map, fine, tile, cx, cy)) {
    return null;
  }
  return cornerTerms(map, tile, cx, cy, x - cx, y - cy, "every");
}

/**
 * The terms by which refined texel `texel`, tile `tile` of a fine page at its cells' corners,
 * stands as a corner of another texel's reconstruction at (`sN`, `tN`): its corners about the
 * point held within its span, by their bilinear weights, those surveyed alone; none where none of
 * them is surveyed.
 */
function clampedCornerTerms(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  texel: readonly [number, number],
  sN: number,
  tN: number,
): ReadonlyArray<ReconstructionTerm> {
  const r = fine.side;
  const within = (v: number): number => Math.min(Math.max(v, 0), r);
  const x = within(sN * r - r * texel[0]);
  const y = within(tN * r - r * texel[1]);
  const cx = Math.min(Math.floor(x), r - 1);
  const cy = Math.min(Math.floor(y), r - 1);
  return cornerTerms(map, tile, cx, cy, x - cx, y - cy, "surveyed");
}

/**
 * The terms of tile `tile`'s cell (`cx`, `cy`) at (`fx`, `fy`) within it: its four corners by their
 * bilinear weights, `every` one, a corner not surveyed as law 0 alone, or those `surveyed` alone.
 */
function cornerTerms(
  map: ClassMapTexels,
  tile: number,
  cx: number,
  cy: number,
  fx: number,
  fy: number,
  corners: "every" | "surveyed",
): ReconstructionTerm[] {
  const terms: ReconstructionTerm[] = [];
  for (let corner = 0; corner < 4; corner += 1) {
    const di = corner % 2;
    const dj = Math.floor(corner / 2);
    const at: StoredAt = { kind: "tile", tile, x: cx + di, y: cy + dj };
    if (corners === "every" || surveyedAt(map, at)) {
      terms.push({ at, b: cornerWeight(di, dj, fx, fy) });
    }
  }
  return terms;
}

/** Whether tile `tile`'s cell (`cx`, `cy`) is surveyed, as its flag stores it ({@link ClassMapFine}). */
function cellSurveyed(
  map: ClassMapTexels,
  fine: ClassMapFine,
  tile: number,
  cx: number,
  cy: number,
): boolean {
  return (map.data[tileOffset(map, fine, tile, fine.side + 1, cy, 0) + cx] ?? 0) > 0;
}

/** The byte offset of class `k`'s weight where `at` is stored. */
function storedAtOffset(map: ClassMapTexels, at: StoredAt, k: number): number {
  let offset: number;
  switch (at.kind) {
    case "texel":
      offset = storedOffset(map, at.face, at.x, at.y, k);
      break;
    case "tile":
      offset =
        tileOffset(map, fineOf(map), at.tile, at.x, at.y, Math.floor(k / CLASSES_PER_LAYER)) +
        (k % CLASSES_PER_LAYER);
      break;
  }
  return offset;
}

/**
 * A map's fine page, which a tile's term implies.
 *
 * @throws Error for a map without one (a bug: no term of it names a tile).
 */
function fineOf(map: ClassMapTexels): ClassMapFine {
  if (map.fine === undefined) {
    throw new Error("a class map without a fine page has no tiles");
  }
  return map.fine;
}

/** The weight of each class where `at` is stored, as the shader reads it. */
function weightsAt(map: ClassMapTexels, at: StoredAt): number[] {
  return Array.from(
    { length: map.classes },
    (_, k) => (map.data[storedAtOffset(map, at, k)] ?? 0) / UNORM_MAX,
  );
}

/** Whether `at` holds any weight: one byte of its classes above 0. */
function surveyedAt(map: ClassMapTexels, at: StoredAt): boolean {
  for (let k = 0; k < map.classes; k += 1) {
    if ((map.data[storedAtOffset(map, at, k)] ?? 0) > 0) {
      return true;
    }
  }
  return false;
}

/** Whether stored texel (x, y) of `face` holds any weight: one byte of its classes above 0. */
function storedSurveyed(map: ClassMapTexels, face: Face, x: number, y: number): boolean {
  return surveyedAt(map, { kind: "texel", face, x, y });
}

/** The maturity code where `at` is stored, from its two bytes, as the shader reads it. */
function codeAt(map: ClassMapTexels, at: StoredAt): number {
  let code: number;
  switch (at.kind) {
    case "texel":
      code = storedCode(map, at.face, at.x, at.y);
      break;
    case "tile": {
      const of = tileOffset(map, fineOf(map), at.tile, at.x, at.y, weightGroups(map.classes));
      code = (map.data[of] ?? 0) * (UNORM_MAX + 1) + (map.data[of + 1] ?? 0);
      break;
    }
  }
  return code;
}

/**
 * Each class's A at a body-fixed direction under a map that carries its maturity (plan R10,
 * R10.T1.c's client half; decision-r10-t1c-client-scope §2.3), as `surface_at` takes it in the
 * shader: unscaled for the figure, display channels.
 *
 * @remarks
 * Over the very terms and weights b of {@link classMapSharesAt}, its surveyed texels and cells
 * alone, a drawn class c's A is Σ b T_c(x) ÷ Σ b, T_c(x) its nodes' albedo at the term's code
 * (`maturityNodes.ts`' segment and mix), a product form beside the shares, which stay as they are.
 * A class not drawn from its nodes, or of no share at the point (`shares[c + 1]` not above 0), is
 * `null`: its law's A, its nodes unread.
 *
 * @param direction - Along the body-fixed axes, z the pole; it need not be unit.
 * @param shares - The laws' shares at `direction` ({@link classMapSharesAt}).
 * @returns Each class's A, or `null` for a class drawn by its law's; `null` for a map without a
 *   maturity, or at a direction whose texel holds no weight, where `elsewhere` takes it all.
 */
export function classMapAlbedoAt(
  map: ClassMapTexels,
  direction: Xyz,
  shares: ReadonlyArray<number>,
): Array<Rgb | null> | null {
  const { maturity } = map;
  if (maturity === undefined) {
    return null;
  }
  const terms = reconstructionTerms(map, direction);
  if (terms === null) {
    return null;
  }
  const corners = terms.map(({ at, b }) => ({
    b,
    segment: maturitySegment(maturityCodeRatio(codeAt(map, at))),
  }));
  const norm = corners.reduce((sum, { b }) => sum + b, 0);
  if (!(norm > 0)) {
    return null;
  }
  return Array.from({ length: map.classes }, (_, c): Rgb | null => {
    if (maturity.drawn[c] !== true || !((shares[c + 1] ?? 0) > 0)) {
      return null;
    }
    const sum: [number, number, number] = [0, 0, 0];
    for (const { b, segment } of corners) {
      const albedo = maturityNodeAlbedo(maturity.nodes, 3 * MATURITY_NODES * c, segment);
      sum[0] += b * albedo[0];
      sum[1] += b * albedo[1];
      sum[2] += b * albedo[2];
    }
    return [sum[0] / norm, sum[1] / norm, sum[2] / norm];
  });
}

/** The laws a surface shades with, in the disc record's order: one, or `elsewhere` then each class's. */
export function discSurfaceLaws(surface: DiscSurface): ReadonlyArray<PhotometricLaw> {
  let laws: ReadonlyArray<PhotometricLaw>;
  switch (surface.kind) {
    case "uniform":
      laws = [surface.law];
      break;
    case "class-map":
      laws = [surface.elsewhere, ...surface.laws];
      break;
  }
  return laws;
}

/**
 * The specification of a class map's texture: (N + 2) × (N + 2) texels a layer, the gutter
 * included, filed under `coarse-field-gpu` beside the coverage mask (R12 Design note 6's coarse
 * field's GPU copy; R10.T10.d), 418 KB at R10's 64 texels a face and 16 classes, and 523 KB with
 * the six layers of a map's maturity (R10.T1.c's client half); with a fine page, 17,424 B more
 * for each of its index's six layers and its pages' (lane fix-coast-class-map), 121 tiles a page
 * of a level-8 field's.
 */
export function classMapTextureSpec(
  name: string,
  map: Pick<ClassMapTexels, "faceTexels" | "classes" | "maturity" | "fine">,
): TextureSpec {
  const side = classMapSide(map.faceTexels);
  return {
    name,
    size: { width: side, height: side, depthOrArrayLayers: classMapTexelLayers(map) },
    dimension: "2d",
    format: CLASS_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "coarse-field-gpu",
  };
}

/**
 * A disc's class-map surface (R10.T10.d's construction): the map uploaded as its texture, with a law
 * per class and the uniform law for unsurveyed texels; the surface carries the map as `texels` too,
 * the texture's CPU copy, which the point regime integrates (R10.T10.f), and the field's normal map
 * where it is given one (R10.T10.g).
 *
 * @remarks
 * The texture is the caller's: it is made again, and this called again, after a device loss
 * (`onRestored`), and released with `releaseTexture` when the map is dropped. So is the normal
 * map's (`discNormals.ts`' `normalMapTexture` makes it), which the surface carries as it is given.
 *
 * @param laws - One per class of `map`, in its channel order.
 * @param elsewhere - The uniform `lawFor(p, q, template)`, for unsurveyed texels.
 * @param normals - The field's normal map over the same survey, or `null` (the default) for none:
 *   the datum's normal then shades everywhere.
 * @throws Error if `laws` are not one per class of `map`.
 */
export function classMapSurface(
  engine: Pick<RenderEngine, "createTexture" | "writeTexture">,
  name: string,
  map: ClassMapTexels,
  laws: ReadonlyArray<PhotometricLaw>,
  elsewhere: PhotometricLaw,
  normals: DiscNormalMap | null = null,
): ClassMapDiscSurface {
  if (laws.length !== map.classes) {
    throw new Error(`a class map of ${map.classes} classes needs as many laws, got ${laws.length}`);
  }
  const weights: TextureHandle = engine.createTexture(classMapTextureSpec(name, map));
  const side = classMapSide(map.faceTexels);
  engine.writeTexture(
    weights,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: classMapTexelLayers(map) },
    map.data,
  );
  return { kind: "class-map", weights, texels: map, laws, elsewhere, normals };
}
