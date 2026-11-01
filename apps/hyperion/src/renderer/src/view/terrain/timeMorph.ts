/**
 * The drawn ground's changes morphed in over time (plan R10, R10.T16.i; the seamless lane's brief
 * 2, the owner's priority, 2026-11-01): each refine and coarsen of the draw set fades over
 * {@link MORPH_IN_MS}, so that a level change lands on the old surface whatever CDLOD's k gives.
 *
 * @remarks
 * CDLOD's morph hides a level change only where it lands at its band's end. A bake's round trip
 * outruns the band, so 20 to 65% of refines landed at k < 0.5 and turned the shading normal by up to
 * 47° in a frame (TERRAIN TEST at 3 × 41 ms, research-terrain-seamless). Here every change of the
 * draw set starts a transition over an area, its footprint, whose fade the instances in it take as
 * their own and those beside it on their edges and corners (`coarseFineHolds.ts`'
 * `TransitionFades`); the shader floors k at the fade (`fadedMorph`):
 *
 * - **refine**: a patch drawn where a coarser one was starts at a fade of 1, on its parent's mesh,
 *   and falls to its own k;
 * - **coarsen**: a patch drawn where finer ones were waits while they rise to a fade of 1 on their
 *   parents' mesh, still drawn and pinned, and then takes their place;
 * - **rebake**: a resident patch's newer bake, stored in a spare slot, mixes its heights, normals
 *   and maturity from its older one, kept in its slot, by a fade falling from 1, the datum's where
 *   the older one's survey mask held its ground unsurveyed; where no slot is spare the newer bake
 *   takes the older's at once, as before, counted;
 * - **rise**: a patch drawn where nothing was, over ground a view saw at the step before (the
 *   reference surface's, newly surveyed), rises out of the datum by the same mix;
 * - a change that reverses one in progress starts from the fade it had reached.
 *
 * The fades are a function of the draw set and the time alone: every instance touching a footprint
 * takes the one fade, so that shared vertices agree. The clock is the views' draws'
 * (`TerrainViewFrameInput.nowMs`): a transition starts at the first draw after the step that made
 * it.
 */

import type { CachedPatch, DrawnPatch, DrawSet } from "./cache";
import type { FadeFootprint } from "./coarseFineHolds";
import { DATUM_SLOT, MAX_SUB_PATCH_LEVELS } from "./gpu/uniforms";
import { childKeys, type PatchKey, patchKeyString } from "./patchKey";

/**
 * The time a change of the drawn ground morphs in over, ms: a hand value (the brief's 0.4 s), the
 * seamless probe's to set.
 */
export const MORPH_IN_MS = 400;

/** The fade a transition from `from` to `to` that began at `startMs` has reached at `nowMs`: smoothstep. */
export function fadeAt(
  from: number,
  to: number,
  startMs: number | null,
  nowMs: number,
  spanMs: number,
): number {
  if (startMs === null) {
    return from;
  }
  const t = spanMs > 0 ? Math.min(1, Math.max(0, (nowMs - startMs) / spanMs)) : 1;
  return from + (to - from) * t * t * (3 - 2 * t);
}

/** A drawn instance as the presented draw set holds it: a {@link DrawnPatch} and its views. */
export interface PresentedPatch extends DrawnPatch {
  /** The views that see it, bit k for view k (`DrawSet.views`). */
  readonly views: number;
}

/** A transition of the drawn ground over one instance's area. */
interface Transition {
  /** The instance's area: a refine's finer one, a coarsen's coarser one. */
  readonly area: PatchKey;
  /** The areas its fade is drawn over: a refine's own, a coarsen's finer patches', a rise's none. */
  readonly footprints: ReadonlyArray<PatchKey>;
  /** A rise's: the area's heights mix from the datum's by the fade, falling from 1. */
  readonly rise: boolean;
  readonly from: number;
  readonly to: number;
  startMs: number | null;
  /**
   * A coarsen's: what is drawn in the area's place until the fade reaches 1, the finer instances
   * that were, and parts of the coarser patch over the rest; `null` for a refine.
   */
  readonly held: ReadonlyArray<HeldPatch> | null;
}

/** An instance a coarsen draws in its area's place. */
interface HeldPatch {
  readonly keyString: string;
  readonly within: PatchKey | null;
  readonly standIn: boolean;
}

/** What {@link TimeMorph.present} reads of the patch cache: its resident patches by key string. */
export interface ResidentLookup {
  get(keyString: string): CachedPatch | undefined;
}

/** The draw set a {@link TimeMorph} presents, rewritten in place each call. */
interface PresentedSet {
  count: number;
  readonly patches: PresentedPatch[];
  readonly slots: Uint32Array;
  standingIn: number;
  missing: number;
  readonly views: Uint32Array;
  standingInViews: number;
  missingViews: number;
}

/** The area an instance draws: a sub-patch's missing patch's, else its patch's. */
function areaOf(drawn: Pick<DrawnPatch, "patch" | "within">): PatchKey {
  return drawn.within ?? drawn.patch.key;
}

/**
 * The cells of `area` that none of `covered` (areas in it, which do not overlap) covers: the
 * quadtree's cut of the rest, coarsest first, each no finer than `deepest`; `null` where the rest
 * would need a cell finer than `deepest`.
 */
function restOf(
  area: PatchKey,
  covered: ReadonlyArray<PatchKey>,
  deepest: number,
): PatchKey[] | null {
  const inside = covered.filter((c) => within(c, area));
  if (inside.length === 0) {
    return [area];
  }
  if (inside.some((c) => c.level === area.level)) {
    return [];
  }
  if (area.level >= deepest) {
    return null;
  }
  const out: PatchKey[] = [];
  for (const child of childKeys(area)) {
    const rest = restOf(child, inside, deepest);
    if (rest === null) {
      return null;
    }
    out.push(...rest);
  }
  return out;
}

/** Whether `inner` lies in `outer`, or is it. */
function within(inner: PatchKey, outer: PatchKey): boolean {
  if (inner.face !== outer.face || inner.level < outer.level) {
    return false;
  }
  const cells = 2 ** (inner.level - outer.level);
  return Math.floor(inner.i / cells) === outer.i && Math.floor(inner.j / cells) === outer.j;
}

/**
 * One terrain pass's transitions of its drawn ground (module remarks): it presents each resolved
 * draw set with its coarsens' finer patches held in place, and gives each frame's footprints.
 */
export class TimeMorph {
  readonly #spanMs: number;
  readonly #set: PresentedSet;
  /** The transitions in progress, by their area's `patchKeyString`. */
  #transitions = new Map<string, Transition>();
  /** The instances last presented, by their area's `patchKeyString`. */
  #presented = new Map<string, PresentedPatch>();
  /** The latest draw's time, ms, at which a reversed transition's fade is read. */
  #nowMs = 0;
  /** A coarsen has reached its fade of 1, so that the next step presents its patch. */
  #swapDue = false;
  #coarsensSwapped = 0;
  #rebakesSwapped = 0;
  /** The presented instances' areas' `patchKeyString`s, in their order. */
  readonly #areas: string[] = [];
  /** Each presented instance's heights' fade word at the last draw time ({@link TimeMorph.heightWords}). */
  readonly #heightWords: Uint32Array;
  /** The rebakes fading, by their patch's `patchKeyString`: the older bake's slot and the start. */
  readonly #rebakes = new Map<string, { readonly slot: number; startMs: number | null }>();
  /** The rebakes' words at the last draw time (`fadeWord`'s bits 8 to 31), by key string. */
  readonly #rebakeWords = new Map<string, number>();
  /** The older bakes' slots whose fades have ended, for the pass to free. */
  readonly #released: number[] = [];

  /**
   * @param capacity - The most instances a draw set presents: the cache's slot count, which the
   *   instance records hold.
   * @param spanMs - The time a change morphs in over, ms; {@link MORPH_IN_MS} by default.
   */
  constructor(capacity: number, spanMs = MORPH_IN_MS) {
    this.#spanMs = spanMs;
    this.#heightWords = new Uint32Array(capacity);
    this.#set = {
      count: 0,
      patches: [],
      slots: new Uint32Array(capacity),
      standingIn: 0,
      missing: 0,
      views: new Uint32Array(capacity),
      standingInViews: 0,
      missingViews: 0,
    };
  }

  /** Whether a coarsen's fade has reached 1, so that the pass presents its draw set again. */
  get swapDue(): boolean {
    return this.#swapDue;
  }

  /**
   * How many coarsens took their place at once, with no fade: their finer patches did not cover
   * their area, were no longer resident, or would have overrun the instance records.
   */
  get coarsensSwapped(): number {
    return this.#coarsensSwapped;
  }

  /** Whether any transition is in progress. */
  get active(): boolean {
    return this.#transitions.size > 0 || this.#rebakes.size > 0;
  }

  /** How many rebakes took their older bake's slot at once, with no fade: no slot was spare. */
  get rebakesSwapped(): number {
    return this.#rebakesSwapped;
  }

  /**
   * Starts the fade of the patch keyed `keyString` from its older bake, kept in `oldSlot`, to its
   * newer one; with `oldSlot` `null` (no slot was spare), counts the swap. A fade of the patch in
   * progress gives up its own older bake's slot ({@link TimeMorph.takeReleased}), and the new one
   * starts from the newer bake it had.
   */
  rebake(keyString: string, oldSlot: number | null): void {
    const going = this.#rebakes.get(keyString);
    if (going !== undefined) {
      this.#released.push(going.slot);
      this.#rebakes.delete(keyString);
      this.#rebakeWords.delete(keyString);
    }
    if (oldSlot === null) {
      this.#rebakesSwapped += 1;
      return;
    }
    this.#rebakes.set(keyString, { slot: oldSlot, startMs: null });
    // At its start: the older bake's whole, until the first draw stamps it.
    this.#rebakeWords.set(keyString, ((255 << 8) | (oldSlot << 16)) >>> 0);
  }

  /**
   * Each presented instance's heights' fade at the last {@link TimeMorph.footprintsAt}, in the
   * presented order: `fadeWord`'s bits 8 to 31, the fade towards its patch's older bake and that
   * bake's slot, or towards the datum (`DATUM_SLOT`) where it rises; 0 with neither.
   */
  get heightWords(): Uint32Array {
    return this.#heightWords;
  }

  /** The older bakes' slots whose fades have ended since the last call, to free. */
  takeReleased(): number[] {
    return this.#released.splice(0);
  }

  /**
   * The draw set to draw for `resolved` (the resolver's, as the cache holds it now): `resolved`'s
   * instances, but where a coarsen holds the finer ones that were presented over an instance's
   * area until its fade reaches 1; and the transitions each change starts. The same object every
   * call, its arrays rewritten in place.
   *
   * @param seenBefore - Whether a view saw an area at the step before, which a patch drawn where
   *   nothing was rises over: newly surveyed ground, not ground newly in view.
   */
  present(
    resolved: DrawSet,
    cache: ResidentLookup,
    seenBefore: (area: PatchKey) => boolean,
  ): DrawSet {
    const set = this.#set;
    const capacity = set.slots.length;
    const before = this.#presented;
    const transitions = new Map<string, Transition>();
    const presented = new Map<string, PresentedPatch>();
    const out: PresentedPatch[] = [];
    // The instances this set will hold: the resolved ones, and the held ones beyond them.
    let projected = resolved.count;
    const hold = (transition: Transition, views: number): boolean => {
      const held = this.#held(transition, cache, views);
      if (held === null || projected + held.length - 1 > capacity) {
        return false;
      }
      projected += held.length - 1;
      for (const patch of held) {
        out.push(patch);
        presented.set(patchKeyString(areaOf(patch)), patch);
      }
      return true;
    };
    for (let n = 0; n < resolved.count; n += 1) {
      const drawn = resolved.patches[n];
      if (drawn === undefined) {
        continue;
      }
      const views = resolved.views[n] ?? 0;
      const area = areaOf(drawn);
      const areaString = drawn.within === null ? drawn.keyString : patchKeyString(area);
      const was = before.get(areaString);
      const going = this.#transitions.get(areaString);
      const steady = was?.keyString === drawn.keyString;
      let swapped = false;
      if (going !== undefined && going.held !== null && !steady) {
        // A coarsen in progress for this instance: its finer patches stay until its fade is 1.
        if (fadeAt(going.from, going.to, going.startMs, this.#nowMs, this.#spanMs) < 1) {
          if (hold(going, views)) {
            transitions.set(areaString, going);
            continue;
          }
          this.#coarsensSwapped += 1;
        }
        swapped = true;
      }
      const patch: PresentedPatch =
        steady && was.patch === drawn.patch && was.standIn === drawn.standIn && was.views === views
          ? was
          : {
              keyString: drawn.keyString,
              patch: drawn.patch,
              standIn: drawn.standIn,
              within: drawn.within,
              views,
            };
      const next = swapped
        ? null
        : steady
          ? (going ?? this.#releasedOver(area))
          : this.#changeOver(area, drawn, before, cache, seenBefore);
      if (next !== null) {
        transitions.set(areaString, next);
        if (next.held !== null) {
          if (hold(next, views)) {
            continue;
          }
          transitions.delete(areaString);
          this.#coarsensSwapped += 1;
        }
      }
      out.push(patch);
      presented.set(areaString, patch);
    }
    this.#transitions = transitions;
    this.#presented = presented;
    this.#swapDue = false;
    set.patches.length = 0;
    this.#areas.length = 0;
    out.forEach((patch, n) => {
      set.patches[n] = patch;
      set.slots[n] = patch.patch.slot;
      set.views[n] = patch.views;
      this.#areas.push(patch.within === null ? patch.keyString : patchKeyString(patch.within));
    });
    set.count = out.length;
    set.standingIn = resolved.standingIn;
    set.missing = resolved.missing;
    set.standingInViews = resolved.standingInViews;
    set.missingViews = resolved.missingViews;
    return set;
  }

  /**
   * The fall from a coarsen given up over `area`, which was held in an ancestor's coarsen that the
   * selection has refined back into: from the fade that coarsen reached, so that nothing jumps;
   * `null` where no ancestor's coarsen held it.
   */
  #releasedOver(area: PatchKey): Transition | null {
    for (const going of this.#transitions.values()) {
      if (going.held !== null && going.area.level < area.level && within(area, going.area)) {
        const reached = fadeAt(going.from, going.to, going.startMs, this.#nowMs, this.#spanMs);
        return reached > 0
          ? {
              area,
              footprints: [area],
              rise: false,
              from: reached,
              to: 0,
              startMs: null,
              held: null,
            }
          : null;
      }
    }
    return null;
  }

  /**
   * The transition `drawn`'s instance over `area` starts, from what was presented there: a refine
   * where a coarser patch was, a coarsen where finer ones covered the whole area, from the fade
   * reached by any transition there; `null` for an instance drawn by the patch that drew it before
   * (a sub-patch of it) with no transition there, where nothing was drawn, or for finer patches that
   * do not cover it or are gone (a coarsen then swaps at once, counted).
   */
  #changeOver(
    area: PatchKey,
    drawn: DrawnPatch,
    before: ReadonlyMap<string, PresentedPatch>,
    cache: ResidentLookup,
    seenBefore: (area: PatchKey) => boolean,
  ): Transition | null {
    // The area, or the nearest coarser one, drawn before.
    for (let level = area.level; level >= 0; level -= 1) {
      const cells = 2 ** (area.level - level);
      const coarser: PatchKey = {
        face: area.face,
        level,
        i: Math.floor(area.i / cells),
        j: Math.floor(area.j / cells),
      };
      const coarserString = patchKeyString(coarser);
      const was = before.get(coarserString);
      if (was === undefined) {
        continue;
      }
      const going = this.#transitions.get(coarserString);
      if (was.patch.key.level < drawn.patch.key.level) {
        const reached =
          going === undefined
            ? 1
            : fadeAt(going.from, going.to, going.startMs, this.#nowMs, this.#spanMs);
        return {
          area,
          footprints: [area],
          rise: false,
          from: reached,
          to: 0,
          startMs: null,
          held: null,
        };
      }
      // Drawn by the same patch, or a coarser one: a transition in progress there goes on over
      // this part of it.
      return going === undefined || going.held !== null
        ? null
        : {
            area,
            footprints: going.rise ? [] : [area],
            rise: going.rise,
            from: going.from,
            to: going.to,
            startMs: going.startMs,
            held: null,
          };
    }
    // Finer instances drawn before over the area: a coarsen, held with parts of the coarser patch
    // over the rest of the area, where they did not cover it all (ground newly in view).
    const finer: PresentedPatch[] = [];
    let reached = 0;
    for (const [areaString, was] of before) {
      if (within(areaOf(was), area)) {
        finer.push(was);
        const going = this.#transitions.get(areaString);
        if (going !== undefined) {
          reached = Math.max(
            reached,
            fadeAt(going.from, going.to, going.startMs, this.#nowMs, this.#spanMs),
          );
        }
      }
    }
    if (finer.length === 0) {
      // Nothing drawn here before: ground a view saw rises out of the reference surface there.
      return before.size > 0 && seenBefore(area)
        ? { area, footprints: [], rise: true, from: 1, to: 0, startMs: null, held: null }
        : null;
    }
    const rest = restOf(area, finer.map(areaOf), drawn.patch.key.level + MAX_SUB_PATCH_LEVELS);
    if (rest === null || finer.some(({ keyString }) => cache.get(keyString) === undefined)) {
      this.#coarsensSwapped += 1;
      return null;
    }
    const held: HeldPatch[] = [
      ...finer.map(({ keyString, within: part, standIn }) => ({
        keyString,
        within: part,
        standIn,
      })),
      ...rest.map((part) => ({ keyString: drawn.keyString, within: part, standIn: drawn.standIn })),
    ];
    return {
      area,
      footprints: finer.map(areaOf),
      rise: false,
      from: reached,
      to: 1,
      startMs: null,
      held,
    };
  }

  /** `transition`'s held patches as the cache holds them now, with `views`; `null` if one is gone. */
  #held(transition: Transition, cache: ResidentLookup, views: number): PresentedPatch[] | null {
    const out: PresentedPatch[] = [];
    for (const { keyString, within: part, standIn } of transition.held ?? []) {
      const patch = cache.get(keyString);
      if (patch === undefined) {
        return null;
      }
      out.push({ keyString, patch, within: part, standIn, views });
    }
    return out;
  }

  /**
   * The footprints of the transitions in progress at `nowMs`, each at its fade there: a frame's
   * draws read them (`TransitionFades`). A transition the step made starts at the first such call;
   * a refine whose fade has reached 0 ends; a coarsen whose fade has reached 1 holds there until
   * the next step presents its patch ({@link TimeMorph.swapDue}).
   */
  footprintsAt(nowMs: number): FadeFootprint[] {
    this.#nowMs = Math.max(this.#nowMs, nowMs);
    for (const [keyString, rebake] of this.#rebakes) {
      rebake.startMs ??= nowMs;
      const fade = fadeAt(1, 0, rebake.startMs, nowMs, this.#spanMs);
      if (nowMs - rebake.startMs >= this.#spanMs) {
        this.#rebakes.delete(keyString);
        this.#rebakeWords.delete(keyString);
        this.#released.push(rebake.slot);
      } else {
        const byte = Math.round(fade * 255);
        this.#rebakeWords.set(
          keyString,
          byte === 0 ? 0 : ((byte << 8) | (rebake.slot << 16)) >>> 0,
        );
      }
    }
    const out: FadeFootprint[] = [];
    const rises = new Map<string, number>();
    for (const [areaString, transition] of this.#transitions) {
      transition.startMs ??= nowMs;
      const fade = fadeAt(transition.from, transition.to, transition.startMs, nowMs, this.#spanMs);
      const ended = nowMs - transition.startMs >= this.#spanMs;
      if (transition.held === null && ended && fade <= 0) {
        this.#transitions.delete(areaString);
        continue;
      }
      this.#swapDue ||= transition.held !== null && ended;
      if (transition.rise) {
        rises.set(areaString, Math.round(fade * 255));
      }
      for (const area of transition.footprints) {
        out.push({ area, fade });
      }
    }
    const words = this.#heightWords;
    const set = this.#set;
    for (let n = 0; n < set.count; n += 1) {
      const rebake = this.#rebakeWords.get(set.patches[n]?.keyString ?? "") ?? 0;
      const rise = rises.get(this.#areas[n] ?? "") ?? 0;
      words[n] = rebake !== 0 ? rebake : rise === 0 ? 0 : ((rise << 8) | (DATUM_SLOT << 16)) >>> 0;
    }
    return out;
  }

  /** Forgets every transition and what was presented: a device's restore, or the pass's fault. */
  clear(): void {
    this.#transitions.clear();
    this.#presented.clear();
    this.#swapDue = false;
    for (const { slot } of this.#rebakes.values()) {
      this.#released.push(slot);
    }
    this.#rebakes.clear();
    this.#rebakeWords.clear();
  }
}
