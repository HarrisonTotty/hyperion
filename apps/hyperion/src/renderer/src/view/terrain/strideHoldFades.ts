/**
 * The onsets of a view's coarse–fine holds at k = 0 for the camera's strides (plan R14, R14.T3.c;
 * the lean of 2026-11-03 on fix-r14-t3c-shared-bands' acceptance): as a finer patch arrives beside a
 * coarse edge, the edge's level-2 vertices, which may stand on stride 8's chords, fall to their own
 * places over {@link STRIDE_HOLD_FADE_MS} rather than in one frame.
 *
 * @remarks
 * While an edge's hold switches on, its level-2 shared bands are the edge's last before the hold
 * (`StrideEdges` gives none beside a finer patch), and its byte of the instance's shares word
 * (`Instance.strides.z`) falls from 255 to 0, the share of its stride morph the vertex stage leaves
 * a level-2 vertex there (`clusters/strides.ts`' `strideFactor`). The cull holds the edge's blocks at
 * stride 4 from the first frame, so their stride's flip moves nothing; meanwhile the two patches
 * part by at most the edge's stride-8 displacement, falling, over the skirts.
 */

import { edgeBandIndex, NO_STRIDE_BAND, STRIDE_HOLD_FADE_MS } from "../clusters/strides";

/** One patch's edges as the last frame left them. */
interface Track {
  /** Its holds at k = 0 last frame, a bit an edge in `EDGES`' order. */
  held: number;
  /** Each edge's level-2 bands at its 8 blocks, m, the last frame it was not held. */
  readonly level2: Float64Array;
  /** When each edge's hold switched on, ms; NaN for none. */
  readonly onsetMs: Float64Array;
  /** The frame it was last faded in. */
  seen: number;
}

/** The onsets of a view's holds at k = 0 (see the module's remarks), kept by patch. */
export class StrideHoldFades {
  readonly #tracks = new Map<string, Track>();
  #frame = 0;

  /**
   * Patch `keyString`'s shares word this frame at `nowMs`, a byte an edge: `heldEdges` its holds at
   * k = 0 (bits 0 to 3, `EDGES`' order), `bandsM` its edges' shared bands (`StrideEdges.edgesOf`),
   * whose level-2 bands on an edge whose hold switched on within {@link STRIDE_HOLD_FADE_MS} it
   * rewrites with the edge's last before the hold. A patch first seen takes no fade.
   */
  fade(keyString: string, heldEdges: number, bandsM: Float64Array, nowMs: number): number {
    let track = this.#tracks.get(keyString);
    const fresh = track === undefined;
    if (track === undefined) {
      track = {
        held: heldEdges,
        level2: new Float64Array(32).fill(NO_STRIDE_BAND),
        onsetMs: new Float64Array(4).fill(Number.NaN),
        seen: this.#frame,
      };
      this.#tracks.set(keyString, track);
    }
    let word = 0;
    for (let e = 0; e < 4; e += 1) {
      const on = (heldEdges & (1 << e)) !== 0;
      if (!on) {
        track.onsetMs[e] = Number.NaN;
        for (let t = 0; t < 8; t += 1) {
          track.level2[8 * e + t] = bandsM[edgeBandIndex(e, t, 2)] ?? NO_STRIDE_BAND;
        }
        continue;
      }
      if (!fresh && (track.held & (1 << e)) === 0) {
        track.onsetMs[e] = nowMs;
      }
      const sinceMs = nowMs - (track.onsetMs[e] ?? Number.NaN);
      const share = sinceMs >= 0 ? Math.round(255 * (1 - sinceMs / STRIDE_HOLD_FADE_MS)) : 0;
      if (share > 0) {
        word |= share << (8 * e);
        for (let t = 0; t < 8; t += 1) {
          bandsM[edgeBandIndex(e, t, 2)] = track.level2[8 * e + t] ?? NO_STRIDE_BAND;
        }
      }
    }
    track.held = heldEdges;
    track.seen = this.#frame;
    return word >>> 0;
  }

  /** Forgets every patch not faded since the last call, and begins the next frame. */
  sweep(): void {
    for (const [key, track] of this.#tracks) {
      if (track.seen !== this.#frame) {
        this.#tracks.delete(key);
      }
    }
    this.#frame += 1;
  }
}
