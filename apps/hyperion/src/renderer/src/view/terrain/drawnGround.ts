/**
 * The ground a view drew over a body at its last frame (plan R10, R10.T16.c after R10.T16.i): the
 * CPU twin of `terrainPatch.wgsl`'s vertex heights, from the instance records the pass wrote, which
 * the survey edge's rim and its ticks' line of sight read in place of the resident bakes' own.
 *
 * @remarks
 * Since R10.T16.i the drawn ground is not the resident bakes' own heights while a change of the
 * draw set morphs in, for a minute or more after a session starts: a refine starts on its parent's
 * mesh and its heights move by its parent's own surface less their morph target (the datum's where
 * the parent held the ground unsurveyed), and newly drawn ground rises out of the datum. The rim
 * read from the bakes' own heights then stood under the drawn ground or over thin air, its line
 * broken and its ticks over the dots. Each vertex here is the shader's:
 *
 * - its own and morph heights from its slot's bake (`ResidentHeights.bakeAt`), at its place in the
 *   slot's grid (a sub-patch's in its ancestor's, `sourceGridOf`), mixed on the slot's triangle
 *   there (`surfaceHeightsAt`), and a parent's likewise;
 * - mixed by its fade word's mix towards the surface it names: the datum for a rise, its parent's
 *   own surface less its morph target for a refine's or a coarsen's where it has a parent slot,
 *   and, for a rebake, its newer bake's own, since the older one's heights are not kept here;
 * - mixed between them by CDLOD's morph factor from the read place's distance to the camera (the
 *   shader's from each vertex's, a quad's share of the band apart), held and floored at its fades
 *   (`coarseFineHolds.ts`' `fadedMorph`), the grounded contacts' hold left out;
 * - and a point between vertices the barycentric mix of its triangle's, the quad split on its
 *   (0, 0)–(1, 1) diagonal as the mesh splits it.
 *
 * A wholly surveyed bake's heights are its coarse grid's (`residentHeights.ts`), so ground away
 * from the edge is read to within that grid's error; the rim, on edge bakes, at every vertex.
 */

import { type Vec3, vec3 } from "../../geometry/vec3";
import { fadedMorph } from "./coarseFineHolds";
import { DATUM_SLOT, MIX_STEPS, PARENT_MIX_SLOT } from "./gpu/uniforms";
import { type PatchKey, patchIndex } from "./patchKey";
import type { BodyFigure } from "./planet";
import { columnAt, type FacePlace, facePlaces, type ResidentBake } from "./residentHeights";

/** Quads along a patch's side. */
const QUADS = 64;

/** The numbers a drawn instance's record holds here. */
const WORDS = 14;

/** What the twin reads of a pass's resident bakes. */
export interface DrawnGroundBakes {
  bakeAt(key: PatchKey): ResidentBake | null;
  /** Their rim's revision (`ResidentHeights.rimRevision`), which {@link DrawnGround}'s takes in. */
  readonly rimRevision: number;
}

/** What a view's survey edge reads of the ground the view drew (R10.T16.c after R10.T16.i). */
export interface DrawnGroundView {
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction over the bakes whose
   * survey mask holds the edge, the highest of the drawn instances over it there, m; `null` where
   * no instance drawing such a bake lies over it.
   */
  rimHeightAtM(direction: Vec3): number | null;
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction, m, the datum (0)
   * where the instance over it draws unsurveyed ground; `null` where none lies over it.
   */
  groundHeightAtM(direction: Vec3): number | null;
  /**
   * Its rim's revision: it moves whenever a frame draws a different set of instances over bakes
   * holding the edge, or one of them with a different fade, hold or morph band, and whenever such
   * a bake comes or goes, so that the rim read under one revision stands while it does. The camera
   * does not move it: its morph factor moves the rim by at most its patch's selection error, under
   * τ pixels, as the resident bakes' own rim stood off the drawn ground before R10.T16.i.
   */
  readonly revision: number;
}

/** One drawn instance as the pass writes it, in the twin's terms. */
export interface DrawnInstance {
  /** The slot's patch, whose bake it draws. */
  readonly key: PatchKey;
  /** The area it draws: the slot's patch, or for a sub-patch the selected patch within it. */
  readonly area: PatchKey;
  /** CDLOD's morph band for the slot's level, m from the camera. */
  readonly morphStartM: number;
  readonly morphEndM: number;
  /** Whether the shader is given its parent's slot (`#parentSlotOf`), which its parent mix reads. */
  readonly hasParent: boolean;
  /** Its coarse–fine morph holds, edges' and corners' fades, and fades word, as its record has them. */
  readonly holds: number;
  readonly edgeFades: number;
  readonly cornerFades: number;
  readonly fades: number;
}

/**
 * A view's drawn ground, rewritten at each of its frames ({@link DrawnGround.begin}, `push`,
 * `end`), its instances' records in typed arrays that grow as they must.
 */
export class DrawnGround implements DrawnGroundView {
  readonly #bakes: DrawnGroundBakes;
  readonly #figure: BodyFigure;
  #records = new Float64Array(WORDS * 64);
  /** Each record's slot's patch and drawn area, as pushed. */
  readonly #keys: PatchKey[] = [];
  readonly #areas: PatchKey[] = [];
  #count = 0;
  /** Each record's slot's bake, as the resident bakes held it at {@link DrawnGround.end}. */
  readonly #slotBakes: (ResidentBake | null)[] = [];
  /** The place in its grid, quads, of the record {@link DrawnGround.#find} found last. */
  #foundX = 0;
  #foundY = 0;
  /** The records the rim's and the ground's reads found last, which the next tries first; −1. */
  #lastRim = -1;
  #lastGround = -1;
  /** The mixed own and morph heights of the three vertices {@link DrawnGround.#heightAt} reads. */
  readonly #corners = new Float64Array(6);
  /** A frame's records of instances drawing edge bakes, and the last frame's, for the revision. */
  #edge = new Float64Array(WORDS * 64);
  #previous = new Float64Array(0);
  #previousWords = -1;
  #revision = 0;
  /** The instances by the area they draw: by level, finest first, then by `patchIndex`. */
  readonly #byLevel = new Map<number, Map<number, number>>();
  #levels: number[] = [];
  #cameraM: Vec3 = vec3(0, 0, 0);
  #rows: readonly [Vec3, Vec3, Vec3] = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)];

  constructor(bakes: DrawnGroundBakes, figure: BodyFigure) {
    this.#bakes = bakes;
    this.#figure = figure;
  }

  get revision(): number {
    return this.#revision;
  }

  /** Starts a frame with no instances. */
  begin(): void {
    this.#count = 0;
    this.#lastRim = -1;
    this.#lastGround = -1;
    for (const level of this.#byLevel.values()) {
      level.clear();
    }
  }

  /** Records a drawn instance. */
  push(instance: DrawnInstance): void {
    if ((this.#count + 1) * WORDS > this.#records.length) {
      const grown = new Float64Array(this.#records.length * 2);
      grown.set(this.#records);
      this.#records = grown;
    }
    const r = this.#records;
    const at = this.#count * WORDS;
    const { key, area } = instance;
    r[at] = key.face;
    r[at + 1] = key.level;
    r[at + 2] = key.i;
    r[at + 3] = key.j;
    r[at + 4] = area.level;
    r[at + 5] = area.i;
    r[at + 6] = area.j;
    r[at + 7] = instance.morphStartM;
    r[at + 8] = instance.morphEndM;
    r[at + 9] = instance.hasParent ? 1 : 0;
    r[at + 10] = instance.holds;
    r[at + 11] = instance.edgeFades;
    r[at + 12] = instance.cornerFades;
    r[at + 13] = instance.fades;
    let level = this.#byLevel.get(area.level);
    if (level === undefined) {
      level = new Map();
      this.#byLevel.set(area.level, level);
    }
    level.set(patchIndex(area.face, area.i, area.j), this.#count);
    this.#keys[this.#count] = key;
    this.#areas[this.#count] = area;
    this.#count += 1;
  }

  /**
   * Ends a frame, drawn from `cameraM` (the camera from the body's centre, m along the body frame's
   * axes) with the body's rotation `rows` (body-fixed axes into the body frame's), and moves the
   * revision where the instances drawing edge bakes differ from the frame before's.
   */
  end(cameraM: Vec3, rows: readonly [Vec3, Vec3, Vec3]): void {
    this.#cameraM = cameraM;
    this.#rows = rows;
    this.#levels = [...this.#byLevel.entries()]
      .filter(([, level]) => level.size > 0)
      .map(([level]) => level)
      .toSorted((a, b) => b - a);
    if (this.#edge.length < this.#records.length + 1) {
      this.#edge = new Float64Array(this.#records.length * 2);
    }
    let words = 0;
    for (let record = 0; record < this.#count; record += 1) {
      const key = this.#keys[record];
      const bake = key === undefined ? null : this.#bakes.bakeAt(key);
      this.#slotBakes[record] = bake;
      if (bake?.standing === "edge") {
        // Its morph band (words 7 and 8) left out: it slews a step at a time, and sets the morph
        // factor alone, as the camera does.
        const at = record * WORDS;
        this.#edge.set(this.#records.subarray(at, at + 7), words);
        this.#edge.set(this.#records.subarray(at + 9, at + WORDS), words + 7);
        words += WORDS - 2;
      }
    }
    this.#edge[words] = this.#bakes.rimRevision;
    words += 1;
    let same = this.#previousWords === words;
    for (let k = 0; same && k < words; k += 1) {
      same = this.#previous[k] === this.#edge[k];
    }
    if (!same) {
      this.#revision += 1;
      if (this.#previous.length < words) {
        this.#previous = new Float64Array(this.#edge.length);
      }
      this.#previous.set(this.#edge.subarray(0, words));
      this.#previousWords = words;
    }
  }

  rimHeightAtM(direction: Vec3): number | null {
    const record = this.#find(direction, true);
    return record < 0 ? null : this.#heightAt(record, direction);
  }

  groundHeightAtM(direction: Vec3): number | null {
    const record = this.#find(direction, false);
    const bake = this.#slotBakes[record];
    if (bake === undefined || bake === null) {
      return null;
    }
    const x = this.#slotX(record, this.#foundX);
    const y = this.#slotY(record, this.#foundY);
    return bake.surveyedAt(x, y) ? this.#heightAt(record, direction) : 0;
  }

  /**
   * The finest instance over `direction` whose slot's bake is held, holding the edge where
   * `edgeOnly`, its place in its grid left in {@link DrawnGround.#foundX} and `#foundY`; −1 where
   * none is. On a side two instances share, either: their shared vertices agree.
   */
  #find(direction: Vec3, edgeOnly: boolean): number {
    const places: ReadonlyArray<FacePlace> = facePlaces(direction);
    // The record found last, where the place lies within it, its sides too: the draw set's areas
    // do not overlap, and where two share a side their drawn heights agree there (shared vertices,
    // and the coarse–fine holds at a finer one's), so it reads the place as the finest would. The
    // rim's points lie on cells' sides, and a line of sight's samples mostly fall in one area.
    const last = edgeOnly ? this.#lastRim : this.#lastGround;
    const area = this.#areas[last];
    const place = places.length === 1 ? places[0] : undefined;
    if (area !== undefined && place !== undefined && place.face === area.face) {
      const n = 2 ** area.level;
      const x = (place.s * n - area.i) * QUADS;
      const y = (place.t * n - area.j) * QUADS;
      if (x >= 0 && x <= QUADS && y >= 0 && y <= QUADS) {
        this.#foundX = x;
        this.#foundY = y;
        return last;
      }
    }
    const found = this.#search(places, edgeOnly);
    if (edgeOnly) {
      this.#lastRim = found;
    } else {
      this.#lastGround = found;
    }
    return found;
  }

  /** {@link DrawnGround.#find}'s search of every level, finest first, over `places`. */
  #search(places: ReadonlyArray<FacePlace>, edgeOnly: boolean): number {
    for (const levelNumber of this.#levels) {
      const level = this.#byLevel.get(levelNumber);
      if (level === undefined) {
        continue;
      }
      const n = 2 ** levelNumber;
      for (const { face, s, t } of places) {
        for (let a = 0; columnAt(s, n, a) >= 0; a += 1) {
          const i = columnAt(s, n, a);
          for (let b = 0; columnAt(t, n, b) >= 0; b += 1) {
            const j = columnAt(t, n, b);
            const record = level.get(patchIndex(face, i, j));
            const bake = record === undefined ? null : (this.#slotBakes[record] ?? null);
            if (record !== undefined && bake !== null && (!edgeOnly || bake.standing === "edge")) {
              this.#foundX = Math.min(QUADS, Math.max(0, (s * n - i) * QUADS));
              this.#foundY = Math.min(QUADS, Math.max(0, (t * n - j) * QUADS));
              return record;
            }
          }
        }
      }
    }
    return -1;
  }

  /**
   * The drawn height at `direction`, the place {@link DrawnGround.#find} left of `record`'s grid:
   * its triangle's vertices' heights, each `patchVertexOf`'s, mixed by the place's weights. CDLOD's
   * factor is taken from the place's distance to the camera rather than each vertex's, which moves
   * it by a quad's share of the morph band, a few hundredths at most, of a vertex's morph.
   */
  #heightAt(record: number, direction: Vec3): number {
    const bake = this.#slotBakes[record];
    if (bake === undefined || bake === null) {
      return 0;
    }
    const r = this.#records;
    const at = record * WORDS;
    const qx = Math.min(QUADS - 1, Math.floor(this.#foundX));
    const qy = Math.min(QUADS - 1, Math.floor(this.#foundY));
    const fx = this.#foundX - qx;
    const fy = this.#foundY - qy;
    const below = fx >= fy;
    const w0 = 1 - Math.max(fx, fy);
    const w1 = Math.abs(fx - fy);
    const w2 = Math.min(fx, fy);
    const x1 = below ? qx + 1 : qx;
    const y1 = below ? qy : qy + 1;
    const corners = this.#corners;
    const holds = r[at + 10] ?? 0;
    const edgeFades = r[at + 11] ?? 0;
    const cornerFades = r[at + 12] ?? 0;
    const fades = r[at + 13] ?? 0;
    // A vertex's heights mix towards another surface's only where its fade word's mix is not 0;
    // otherwise its morph targets are read only where its morph factor needs them.
    const mixes = ((fades >>> 8) & MIX_STEPS) !== 0;
    if (mixes) {
      this.#mixedHeights(record, bake, qx, qy, 0);
      this.#mixedHeights(record, bake, x1, y1, 2);
      this.#mixedHeights(record, bake, qx + 1, qy + 1, 4);
    } else {
      corners[0] = this.#slotHeight(record, bake, 0, qx, qy);
      corners[2] = this.#slotHeight(record, bake, 0, x1, y1);
      corners[4] = this.#slotHeight(record, bake, 0, qx + 1, qy + 1);
    }
    // CDLOD's morph factor from the place's distance, on its own surface, to the camera: the
    // spheroid's point M·d plus the height along its normal (`surfacePoint`'s).
    const ownM = w0 * (corners[0] ?? 0) + w1 * (corners[2] ?? 0) + w2 * (corners[4] ?? 0);
    const a = this.#figure.equatorialRadiusM;
    const cPolar = this.#figure.polarRadiusM;
    const { x: dx, y: dy, z: dz } = direction;
    const nx = dx / a;
    const ny = dy / a;
    const nz = dz / cPolar;
    const along = ownM / Math.sqrt(nx * nx + ny * ny + nz * nz);
    const px = a * dx + along * nx;
    const py = a * dy + along * ny;
    const pz = cPolar * dz + along * nz;
    const [r0, r1, r2] = this.#rows;
    const c = this.#cameraM;
    const distanceM = Math.hypot(
      r0.x * px + r0.y * py + r0.z * pz - c.x,
      r1.x * px + r1.y * py + r1.z * pz - c.y,
      r2.x * px + r2.y * py + r2.z * pz - c.z,
    );
    const startM = r[at + 7] ?? 0;
    const spanM = (r[at + 8] ?? 0) - startM;
    const cdlod = spanM > 0 ? Math.min(1, Math.max(0, (distanceM - startM) / spanM)) : 0;
    const plain = holds === 0 && edgeFades === 0 && cornerFades === 0 && (fades & 0xff) === 0;
    if (plain && cdlod === 0) {
      return ownM;
    }
    if (!mixes) {
      corners[1] = this.#slotHeight(record, bake, 1, qx, qy);
      corners[3] = this.#slotHeight(record, bake, 1, x1, y1);
      corners[5] = this.#slotHeight(record, bake, 1, qx + 1, qy + 1);
    }
    if (plain) {
      // No hold or fade: every vertex's factor is CDLOD's (`fadedMorph`'s with none).
      const morphM = w0 * (corners[1] ?? 0) + w1 * (corners[3] ?? 0) + w2 * (corners[5] ?? 0);
      return ownM + (morphM - ownM) * cdlod;
    }
    const heightOf = (vx: number, vy: number, k: number): number => {
      const own = corners[k] ?? 0;
      const f = fadedMorph(holds, edgeFades, cornerFades, fades, vx, vy, cdlod);
      return own + ((corners[k + 1] ?? 0) - own) * f;
    };
    return w0 * heightOf(qx, qy, 0) + w1 * heightOf(x1, y1, 2) + w2 * heightOf(qx + 1, qy + 1, 4);
  }

  /**
   * Leaves vertex (`vx`, `vy`) of `record`'s grid's own and morph heights in `#corners` at `k` and
   * `k` + 1, mixed by its fade word's mix towards the surface it names (`patchVertexOf`'s).
   */
  #mixedHeights(record: number, bake: ResidentBake, vx: number, vy: number, k: number): void {
    const r = this.#records;
    const at = record * WORDS;
    const x = this.#slotX(record, vx);
    const y = this.#slotY(record, vy);
    let ownM = onTriangles(bake, 0, x, y);
    let morphM = onTriangles(bake, 1, x, y);
    const fades = r[at + 13] ?? 0;
    const mix = ((fades >>> 8) & MIX_STEPS) / MIX_STEPS;
    if (mix > 0) {
      const from = fades >>> 20;
      let olderOwnM = ownM;
      let olderMorphM = morphM;
      if (from === DATUM_SLOT) {
        olderOwnM = 0;
        olderMorphM = 0;
      } else if (from === PARENT_MIX_SLOT && (r[at + 9] ?? 0) === 1) {
        const parentM = this.#parentOwnM(record, x, y);
        olderOwnM = ownM + (parentM - morphM);
        olderMorphM = parentM;
      }
      ownM += mix * (olderOwnM - ownM);
      morphM += mix * (olderMorphM - morphM);
    }
    this.#corners[k] = ownM;
    this.#corners[k + 1] = morphM;
  }

  /** Vertex (`vx`, `vy`) of `record`'s grid's own height (`which` 0) or morph target (1). */
  #slotHeight(record: number, bake: ResidentBake, which: 0 | 1, vx: number, vy: number): number {
    return onTriangles(bake, which, this.#slotX(record, vx), this.#slotY(record, vy));
  }

  /** The column in `record`'s slot's grid of column `vx` of its own: `sourceGridOf`'s. */
  #slotX(record: number, vx: number): number {
    const r = this.#records;
    const at = record * WORDS;
    const shift = 2 ** ((r[at + 4] ?? 0) - (r[at + 1] ?? 0));
    return (((r[at + 5] ?? 0) - (r[at + 2] ?? 0) * shift) * QUADS + vx) / shift;
  }

  /** The row in `record`'s slot's grid of row `vy` of its own: `sourceGridOf`'s. */
  #slotY(record: number, vy: number): number {
    const r = this.#records;
    const at = record * WORDS;
    const shift = 2 ** ((r[at + 4] ?? 0) - (r[at + 1] ?? 0));
    return (((r[at + 6] ?? 0) - (r[at + 3] ?? 0) * shift) * QUADS + vy) / shift;
  }

  /**
   * `record`'s parent's own height at the place (x, y) of its slot's grid, the datum's where the
   * parent's survey holds it unsurveyed: `olderHeightsAt` at `parentPlaceOf`.
   */
  #parentOwnM(record: number, x: number, y: number): number {
    const key = this.#keys[record];
    if (key === undefined || key.level === 0) {
      return 0;
    }
    const { face, level, i, j } = key;
    const parent = this.#bakes.bakeAt({ face, level: level - 1, i: i >> 1, j: j >> 1 });
    if (parent === null) {
      return 0;
    }
    const px = ((i & 1) * QUADS + x) / 2;
    const py = ((j & 1) * QUADS + y) / 2;
    return parent.surveyedAt(px, py) ? onTriangles(parent, 0, px, py) : 0;
  }
}

/**
 * `bake`'s own heights (`which` 0) or morph targets (1) at the place (x, y) of its grid, quads,
 * mixed on the triangle of its mesh holding it, its quad split on the (0, 0)–(1, 1) diagonal, as
 * the shader reads a sub-patch's and a parent's (`surfaceHeightsAt`); its vertex's at a vertex.
 */
function onTriangles(bake: ResidentBake, which: 0 | 1, x: number, y: number): number {
  const read = which === 0 ? bake.ownAtM : bake.morphAtM;
  const qx = Math.min(QUADS - 1, Math.floor(x));
  const qy = Math.min(QUADS - 1, Math.floor(y));
  const fx = x - qx;
  const fy = y - qy;
  if (fx === 0 && fy === 0) {
    return read(qx, qy);
  }
  const below = fx >= fy;
  return (
    (1 - Math.max(fx, fy)) * read(qx, qy) +
    Math.abs(fx - fy) * read(below ? qx + 1 : qx, below ? qy : qy + 1) +
    Math.min(fx, fy) * read(qx + 1, qy + 1)
  );
}
