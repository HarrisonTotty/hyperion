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
 * - mixed between them by CDLOD's morph factor from its distance to the camera, as the shader's,
 *   for the rim; for the ground, which a tick's line of sight reads at every sample, from the read
 *   place's, a quad's share of the band apart; held and floored at its fades (`coarseFineHolds.ts`'
 *   `fadedMorph`), the grounded contacts' hold left out;
 * - and a point between vertices the barycentric mix of its triangle's, the quad split on its
 *   (0, 0)–(1, 1) diagonal as the mesh splits it.
 *
 * A wholly surveyed bake's heights are its coarse grid's (`residentHeights.ts`), so ground away
 * from the edge is read to within that grid's error; the rim, on edge bakes, at every vertex.
 */

import { type Vec3, vec3 } from "../../geometry/vec3";
import { fadedMorph } from "./coarseFineHolds";
import { vertexDir } from "./cube";
import { DATUM_SLOT, MIX_STEPS, PARENT_MIX_SLOT } from "./gpu/uniforms";
import { type PatchKey, patchIndex } from "./patchKey";
import { type BodyFigure, surfacePoint } from "./planet";
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
  /**
   * Their revision (`ResidentHeights.revision`), which moves as any bake comes or goes: the
   * vertices' heights read under one stand while it does.
   */
  readonly revision: number;
}

/** What a view's survey edge reads of the ground the view drew (R10.T16.c after R10.T16.i). */
export interface DrawnGroundView {
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction over the bakes whose
   * survey mask holds the edge, the finest drawn instance's over it there, m; `null` where no
   * instance drawing such a bake lies over it. Each read leaves its stamp
   * ({@link DrawnGroundView.lastRimStamp}).
   */
  rimHeightAtM(direction: Vec3): number | null;
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction, m, the datum (0)
   * where the instance over it draws unsurveyed ground; `null` where none lies over it. Over an
   * instance at rest (its own patch, no fade, no mix), its bake's own height there, CDLOD's morph
   * left out, as the resident bakes' ground was read.
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
  /**
   * The stamp of the last {@link DrawnGroundView.rimHeightAtM}: the generation of the instance it
   * read, which moves as that instance's record (its band aside) or its slot's bake changes, or as
   * it leaves the drawn set; where it read none, the drawn set's own, which moves as an instance
   * drawing an edge bake comes or goes, or the bakes' rim revision moves.
   */
  lastRimStamp(): number;
  /** Whether a rim read under `stamp` would read the same now, by the same reasoning as above. */
  rimStampStands(stamp: number): boolean;
  /** What the last rim read read, for {@link DrawnGroundView.rimAgain}; `null` where it read none. */
  lastRimToken(): unknown;
  /**
   * The rim at the place a read's token names, read again by its instance's holds and fades alone
   * where only they changed; `undefined` where the place is to be read afresh.
   */
  rimAgain(token: unknown): number | undefined;
}

/** An instance drawing an edge bake, by its area, as the generations keep it. */
interface Generation {
  generation: number;
  /** Its record's words, its band's two aside, as last seen. */
  readonly words: Float64Array;
  bake: ResidentBake;
  /** The frame it was last drawn at. */
  seen: number;
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
  /** What {@link DrawnGround.#heightAt} reads, as {@link RimRead.read} holds it. */
  readonly #read = new Float64Array(READ_WORDS);
  /** What the last rim read read, where its heights mixed with no other surface's. */
  #lastRimRead: RimRead | null = null;
  /** A frame's records of instances drawing edge bakes, and the last frame's, for the revision. */
  #edge = new Float64Array(WORDS * 64);
  #previous = new Float64Array(0);
  #previousWords = -1;
  #revision = 0;
  /** The instances drawing edge bakes, by their area's level and `patchIndex`. */
  readonly #generations = new Map<number, Map<number, Generation>>();
  /** Their generations this frame. */
  readonly #live = new Set<number>();
  #nextGeneration = 1;
  /** Moves as an instance drawing an edge bake comes or goes, or the bakes' rim revision moves. */
  #setRevision = 0;
  #rimRevision = Number.NaN;
  #frame = 0;
  /** Each record's generation this frame, 0 for one drawing no edge bake. */
  readonly #recordGenerations: number[] = [];
  #lastRimStamp = 0;
  /** The instances by the area they draw: by level, finest first, then by `patchIndex`. */
  readonly #byLevel = new Map<number, Map<number, number>>();
  #levels: number[] = [];
  #cameraM: Vec3 = vec3(0, 0, 0);
  #rows: readonly [Vec3, Vec3, Vec3] = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)];
  /**
   * Each vertex's own and morph heights as read since the frame's end, by record and vertex
   * ({@link vertexIndex}), as their place in {@link DrawnGround.#vertexHeights}, two words each
   * (lane perf-render-thread-ground): a vertex's heights depend on its record and the resident
   * bakes alone, and the rim's and the ground's reads take the same vertices again and again, a
   * fading instance's each through its parent's. Forgotten at each frame's begin and end, and as
   * the bakes' revision moves.
   */
  readonly #vertices = new Map<number, number>();
  #vertexHeights = new Float64Array(2 * 256);
  #verticesBakesRevision = Number.NaN;

  constructor(bakes: DrawnGroundBakes, figure: BodyFigure) {
    this.#bakes = bakes;
    this.#figure = figure;
  }

  get revision(): number {
    return this.#revision;
  }

  /** Starts a frame with no instances. */
  begin(): void {
    this.#vertices.clear();
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
    this.#vertices.clear();
    this.#cameraM = cameraM;
    this.#rows = rows;
    this.#levels = [...this.#byLevel.entries()]
      .filter(([, level]) => level.size > 0)
      .map(([level]) => level)
      .toSorted((a, b) => b - a);
    if (this.#edge.length < this.#records.length + 1) {
      this.#edge = new Float64Array(this.#records.length * 2);
    }
    this.#frame += 1;
    let membership = false;
    let words = 0;
    for (let record = 0; record < this.#count; record += 1) {
      const key = this.#keys[record];
      const bake = key === undefined ? null : this.#bakes.bakeAt(key);
      this.#slotBakes[record] = bake;
      this.#recordGenerations[record] = 0;
      const area = this.#areas[record];
      if (bake?.standing === "edge" && area !== undefined) {
        // Its morph band (words 7 and 8) left out: it slews a step at a time, and sets the morph
        // factor alone, as the camera does.
        const at = record * WORDS;
        this.#edge.set(this.#records.subarray(at, at + 7), words);
        this.#edge.set(this.#records.subarray(at + 9, at + WORDS), words + 7);
        membership =
          this.#generationOf(record, area, bake, this.#edge.subarray(words, words + WORDS - 2)) ||
          membership;
        words += WORDS - 2;
      }
    }
    for (const level of this.#generations.values()) {
      for (const [index, entry] of level) {
        if (entry.seen !== this.#frame) {
          this.#live.delete(entry.generation);
          level.delete(index);
          membership = true;
        }
      }
    }
    if (membership || this.#rimRevision !== this.#bakes.rimRevision) {
      this.#setRevision += 1;
      this.#rimRevision = this.#bakes.rimRevision;
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
    if (record < 0) {
      this.#lastRimStamp = -1 - this.#setRevision;
      this.#lastRimRead = null;
      return null;
    }
    this.#lastRimStamp = this.#recordGenerations[record] ?? 0;
    return this.#heightAt(record, direction, true);
  }

  lastRimStamp(): number {
    return this.#lastRimStamp;
  }

  rimStampStands(stamp: number): boolean {
    return stamp < 0 ? stamp === -1 - this.#setRevision : this.#live.has(stamp);
  }

  /**
   * Gives `record`, drawing `area` from an edge bake, its generation this frame: its area's last,
   * where its words (`words`, its band's aside) and its slot's bake are as they were; a new one
   * otherwise. Whether its area was not drawn the frame before.
   */
  #generationOf(record: number, area: PatchKey, bake: ResidentBake, words: Float64Array): boolean {
    let level = this.#generations.get(area.level);
    if (level === undefined) {
      level = new Map();
      this.#generations.set(area.level, level);
    }
    const index = patchIndex(area.face, area.i, area.j);
    let entry = level.get(index);
    const fresh = entry === undefined;
    if (entry === undefined) {
      entry = { generation: this.#nextGeneration, words: words.slice(), bake, seen: this.#frame };
      this.#nextGeneration += 1;
      level.set(index, entry);
    } else {
      let same = entry.bake === bake;
      for (let k = 0; same && k < words.length; k += 1) {
        same = entry.words[k] === words[k];
      }
      if (!same) {
        this.#live.delete(entry.generation);
        entry.generation = this.#nextGeneration;
        this.#nextGeneration += 1;
        entry.words.set(words);
        entry.bake = bake;
      }
      entry.seen = this.#frame;
    }
    this.#live.add(entry.generation);
    this.#recordGenerations[record] = entry.generation;
    return fresh;
  }

  groundHeightAtM(direction: Vec3): number | null {
    const record = this.#find(direction, false);
    const bake = this.#slotBakes[record];
    if (bake === undefined || bake === null) {
      return null;
    }
    const x = this.#slotX(record, this.#foundX);
    const y = this.#slotY(record, this.#foundY);
    if (!bake.surveyedAt(x, y)) {
      return 0;
    }
    // An instance at rest, drawing its own patch with no fade and no mix: its bake's own height,
    // as the resident bakes' own ground was read before R10.T16.i, off the drawn ground by at most
    // CDLOD's morph (its selection error), which a tick's line of sight is tested to a tolerance
    // over; a line of sight reads the ground at every sample, so that this is the frame's cost.
    const r = this.#records;
    const at = record * WORDS;
    const atRest =
      r[at + 1] === r[at + 4] &&
      (r[at + 11] ?? 0) === 0 &&
      (r[at + 12] ?? 0) === 0 &&
      (r[at + 13] ?? 0) === 0;
    if (atRest) {
      return bake.ownAtM(x, y);
    }
    // Off its boundary quads, with no mix, each vertex's factor is one, its fades' floor and the
    // place's CDLOD factor, and a sub-patch's triangle lies in one of its slot's, so that the drawn
    // ground there is its slot's on its triangle, mixed by that factor.
    const qx = Math.floor(this.#foundX);
    const qy = Math.floor(this.#foundY);
    const fades = r[at + 13] ?? 0;
    if (
      ((fades >>> 8) & MIX_STEPS) === 0 &&
      qx >= 1 &&
      qx <= QUADS - 2 &&
      qy >= 1 &&
      qy <= QUADS - 2
    ) {
      const ownM = onTriangles(bake, 0, x, y);
      const half = QUADS / 2;
      const k = fadedMorph(
        r[at + 10] ?? 0,
        r[at + 11] ?? 0,
        r[at + 12] ?? 0,
        fades,
        half,
        half,
        this.#cdlodAt(record, this.#placeDistanceOf(direction, ownM)),
      );
      return k === 0 ? ownM : ownM + (onTriangles(bake, 1, x, y) - ownM) * k;
    }
    return this.#heightAt(record, direction, false);
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
   * factor is each vertex's, from its distance to the camera, where `perVertex` (the rim's reads)
   * and a vertex may stand across the band's start or end from the place; otherwise the place's,
   * which moves it by a quad's share of the morph band, a few hundredths at most, of a vertex's
   * morph, where the band's clamp lies within the triangle (the ground's, for the ticks). A rim
   * read leaves what it read in {@link DrawnGround.lastRimRead}, where its heights mix with no
   * other surface's.
   */
  #heightAt(record: number, direction: Vec3, perVertex: boolean): number {
    const bake = this.#slotBakes[record];
    const area = this.#areas[record];
    if (bake === undefined || bake === null || area === undefined) {
      return 0;
    }
    const r = this.#records;
    const at = record * WORDS;
    const qx = Math.min(QUADS - 1, Math.floor(this.#foundX));
    const qy = Math.min(QUADS - 1, Math.floor(this.#foundY));
    const fx = this.#foundX - qx;
    const fy = this.#foundY - qy;
    const below = fx >= fy;
    const x1 = below ? qx + 1 : qx;
    const y1 = below ? qy : qy + 1;
    // The read: its weights, its vertices, their own and morph heights and their factors.
    const d = this.#read;
    d[0] = 1 - Math.max(fx, fy);
    d[1] = Math.abs(fx - fy);
    d[2] = Math.min(fx, fy);
    d[3] = qx;
    d[4] = qy;
    d[5] = x1;
    d[6] = y1;
    d[7] = qx + 1;
    d[8] = qy + 1;
    const fades = r[at + 13] ?? 0;
    // A vertex's heights mix towards another surface's only where its fade word's mix is not 0.
    const mixes = ((fades >>> 8) & MIX_STEPS) !== 0;
    for (let c = 0; c < 3; c += 1) {
      this.#vertexHeightsInto(record, bake, d[3 + 2 * c] ?? 0, d[4 + 2 * c] ?? 0, mixes, 9 + 2 * c);
    }
    const ownM = d[0] * (d[9] ?? 0) + d[1] * (d[11] ?? 0) + d[2] * (d[13] ?? 0);
    const a = this.#figure.equatorialRadiusM;
    const distanceM = this.#placeDistanceOf(direction, ownM);
    const place = this.#cdlodAt(record, distanceM);
    // Each vertex's own where it may stand across the band's start or end from the place, within a
    // quad's diagonal of it (twice a quad of the face's arc at the area's level): elsewhere the
    // factor is linear in the distance, which moves across the triangle by its curvature alone.
    const quadM = (2 * Math.SQRT2 * a * (Math.PI / 2)) / (2 ** area.level * QUADS);
    const near =
      Math.abs(distanceM - (r[at + 7] ?? 0)) < quadM ||
      Math.abs(distanceM - (r[at + 8] ?? 0)) < quadM;
    for (let c = 0; c < 3; c += 1) {
      d[15 + c] =
        perVertex && near
          ? this.#cdlodAt(
              record,
              this.#distanceOf(
                surfacePoint(
                  this.#figure,
                  vertexDir(area, d[3 + 2 * c] ?? 0, d[4 + 2 * c] ?? 0),
                  d[9 + 2 * c] ?? 0,
                ),
              ),
            )
          : place;
    }
    const holds = r[at + 10] ?? 0;
    const edgeFades = r[at + 11] ?? 0;
    const cornerFades = r[at + 12] ?? 0;
    if (!mixes) {
      // Its morph targets where its factor may need them, as read with its own heights: a rim
      // read's for its reading again; else its own heights in their place.
      const plain = holds === 0 && edgeFades === 0 && cornerFades === 0 && (fades & 0xff) === 0;
      const morphs = perVertex || !plain || d[15] !== 0 || d[16] !== 0 || d[17] !== 0;
      if (!morphs) {
        for (let c = 0; c < 3; c += 1) {
          d[10 + 2 * c] = d[9 + 2 * c] ?? 0;
        }
      }
    }
    if (perVertex) {
      this.#lastRimRead = mixes ? null : new RimRead(area, bake, d.slice());
    }
    return mixedHeight(holds, edgeFades, cornerFades, fades, d);
  }

  /**
   * The rim at the place `token` read ({@link DrawnGround.lastRimRead}) by its instance's holds and
   * fades now, its own and morph heights and factors as it read them: where its area is drawn from
   * the same bake, and its heights still mix with no other surface's; otherwise `undefined`, where
   * the place is to be read afresh. Leaves its stamp, and the read, as a read would.
   */
  rimAgain(token: unknown): number | undefined {
    if (!(token instanceof RimRead)) {
      return undefined;
    }
    const { area, bake, read } = token;
    const record = this.#byLevel.get(area.level)?.get(patchIndex(area.face, area.i, area.j));
    if (record === undefined || this.#slotBakes[record] !== bake) {
      return undefined;
    }
    const r = this.#records;
    const at = record * WORDS;
    const fades = r[at + 13] ?? 0;
    if (((fades >>> 8) & MIX_STEPS) !== 0) {
      return undefined;
    }
    this.#lastRimStamp = this.#recordGenerations[record] ?? 0;
    this.#lastRimRead = token;
    return mixedHeight(r[at + 10] ?? 0, r[at + 11] ?? 0, r[at + 12] ?? 0, fades, read);
  }

  /** What the last rim read read, for its reading again ({@link DrawnGround.rimAgain}); `null`. */
  lastRimToken(): unknown {
    return this.#lastRimRead;
  }

  /**
   * The distance to the camera of the ground `heightM` above the datum at the body-fixed unit
   * `direction`: the spheroid's point M·d plus the height along its normal (`surfacePoint`'s).
   */
  #placeDistanceOf(direction: Vec3, heightM: number): number {
    const a = this.#figure.equatorialRadiusM;
    const c = this.#figure.polarRadiusM;
    const { x, y, z } = direction;
    const nx = x / a;
    const ny = y / a;
    const nz = z / c;
    const along = heightM / Math.sqrt(nx * nx + ny * ny + nz * nz);
    return this.#distanceOf([a * x + along * nx, a * y + along * ny, c * z + along * nz]);
  }

  /** The distance to the camera of the body-fixed point `p`, m from the body's centre. */
  #distanceOf(p: readonly [number, number, number]): number {
    const [r0, r1, r2] = this.#rows;
    const c = this.#cameraM;
    return Math.hypot(
      r0.x * p[0] + r0.y * p[1] + r0.z * p[2] - c.x,
      r1.x * p[0] + r1.y * p[1] + r1.z * p[2] - c.y,
      r2.x * p[0] + r2.y * p[1] + r2.z * p[2] - c.z,
    );
  }

  /** CDLOD's factor of `record`'s morph band at `distanceM` from the camera. */
  #cdlodAt(record: number, distanceM: number): number {
    const r = this.#records;
    const at = record * WORDS;
    const startM = r[at + 7] ?? 0;
    const spanM = (r[at + 8] ?? 0) - startM;
    return spanM > 0 ? Math.min(1, Math.max(0, (distanceM - startM) / spanM)) : 0;
  }

  /**
   * Leaves vertex (`vx`, `vy`) of `record`'s grid's own and morph heights in `#read` at `k` and
   * `k` + 1: mixed towards the surface its fade word names where it `mixes`
   * ({@link DrawnGround.#mixedHeights}), its slot's own otherwise, as read once since the frame's
   * end under the bakes' revision ({@link DrawnGround.#vertices}).
   */
  #vertexHeightsInto(
    record: number,
    bake: ResidentBake,
    vx: number,
    vy: number,
    mixes: boolean,
    k: number,
  ): void {
    const revision = this.#bakes.revision;
    if (revision !== this.#verticesBakesRevision) {
      this.#vertices.clear();
      this.#verticesBakesRevision = revision;
    }
    const index = vertexIndex(record, vx, vy);
    const known = this.#vertices.get(index);
    const read = this.#read;
    if (known !== undefined) {
      read[k] = this.#vertexHeights[known] ?? 0;
      read[k + 1] = this.#vertexHeights[known + 1] ?? 0;
      return;
    }
    if (mixes) {
      this.#mixedHeights(record, bake, vx, vy, k);
    } else {
      read[k] = this.#slotHeight(record, bake, 0, vx, vy);
      read[k + 1] = this.#slotHeight(record, bake, 1, vx, vy);
    }
    const slot = 2 * this.#vertices.size;
    if (slot + 2 > this.#vertexHeights.length) {
      const grown = new Float64Array(2 * this.#vertexHeights.length);
      grown.set(this.#vertexHeights);
      this.#vertexHeights = grown;
    }
    this.#vertexHeights[slot] = read[k] ?? 0;
    this.#vertexHeights[slot + 1] = read[k + 1] ?? 0;
    this.#vertices.set(index, slot);
  }

  /**
   * Leaves vertex (`vx`, `vy`) of `record`'s grid's own and morph heights in `#read` at `k` and
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
    this.#read[k] = ownM;
    this.#read[k + 1] = morphM;
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

/** Vertex (`vx`, `vy`), 0 to {@link QUADS}, of record `record`'s grid as one number. */
function vertexIndex(record: number, vx: number, vy: number): number {
  return (record * (QUADS + 1) + vy) * (QUADS + 1) + vx;
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

/** The numbers of a read ({@link RimRead.read}). */
const READ_WORDS = 18;

/**
 * What a rim read read at a place (plan R10, R10.T16.c after R10.T16.i), to read it again by its
 * instance's holds and fades alone: its area, its slot's bake, and `read`: its triangle's weights
 * (0 to 2), its vertices (3 to 8), their own and morph heights (9 to 14) and their factors (15 to
 * 17).
 */
class RimRead {
  readonly area: PatchKey;
  readonly bake: ResidentBake;
  readonly read: Float64Array;

  constructor(area: PatchKey, bake: ResidentBake, read: Float64Array) {
    this.area = area;
    this.bake = bake;
    this.read = read;
  }
}

/**
 * The height of a read `d` ({@link RimRead.read}) by an instance's holds and fades: each vertex's
 * own and morph heights mixed by `fadedMorph` on its factor, then by the weights.
 */
function mixedHeight(
  holds: number,
  edgeFades: number,
  cornerFades: number,
  fades: number,
  d: Float64Array,
): number {
  const plain = holds === 0 && edgeFades === 0 && cornerFades === 0 && (fades & 0xff) === 0;
  let heightM = 0;
  for (let c = 0; c < 3; c += 1) {
    const own = d[9 + 2 * c] ?? 0;
    const cdlod = d[15 + c] ?? 0;
    const k = plain
      ? cdlod
      : fadedMorph(
          holds,
          edgeFades,
          cornerFades,
          fades,
          d[3 + 2 * c] ?? 0,
          d[4 + 2 * c] ?? 0,
          cdlod,
        );
    heightM += (d[c] ?? 0) * (own + ((d[10 + 2 * c] ?? 0) - own) * k);
  }
  return heightM;
}
