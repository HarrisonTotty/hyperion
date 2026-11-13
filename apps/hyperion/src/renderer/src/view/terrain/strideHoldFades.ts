/**
 * The onsets and releases of a view's coarse–fine holds at k = 0 for the camera's strides (plan R14,
 * R14.T3.c; the lean of 2026-11-03 on fix-r14-t3c-shared-bands' acceptance, and its release's of
 * 2026-11-05): as a finer patch arrives beside a coarse edge, the edge's level-2 vertices, which
 * may stand on stride 8's chords, fall to their own places over {@link STRIDE_HOLD_FADE_MS} rather
 * than in one frame; and as it leaves, they rise back to their band's k over as long.
 *
 * @remarks
 * Each edge keeps a share of its stride morph that its level-2 vertices draw, 1 free and 0 held,
 * which moves towards the edge's hold by 1 over {@link STRIDE_HOLD_FADE_MS}, from wherever the
 * last switch left it, so that a hold switching back mid-fade moves nothing at once. The edge's
 * byte of the instance's shares word (`Instance.holdFades`) carries it: while the edge is held, the
 * share itself, 255 to 0, its level-2 shared bands the edge's last before the hold (`StrideEdges`
 * gives none beside a finer patch); while it is not, 1 less the share, 255 to 0, its bands its own
 * (`clusters/strides.ts`' `strideFactor`). The cull holds the edge's blocks at stride 4 while the
 * edge is held or its byte is not 0 (`strideFadingEdges`), so their stride's flip moves nothing;
 * meanwhile the two patches part by at most the edge's stride-8 displacement, falling, over the
 * skirts. On the seamless probe on high, the releases switching at once moved the edge's vertices
 * up to 1.1 px in a frame, past high's 1 px.
 */

import { edgeBandIndex, NO_STRIDE_BAND, STRIDE_HOLD_FADE_MS } from "../clusters/strides";
import { fadeByte } from "./coarseFineHolds";

/** One patch's edges as the last frame left them. */
interface Track {
  /** Its holds at k = 0 last frame, a bit an edge in `EDGES`' order. */
  held: number;
  /** Each edge's level-2 bands at its 8 blocks, m, the last frame it was not held. */
  readonly level2M: Float64Array;
  /** Each edge's share at its last switch, and when it switched, ms; NaN for none since first seen. */
  readonly fromShare: Float64Array;
  readonly switchMs: Float64Array;
  /** The frame it was last faded in. */
  seen: number;
}

/** The share of edge `e`'s stride morph at `nowMs` under `track`, its hold `on` or not. */
function shareOf(track: Track, e: number, on: boolean, nowMs: number): number {
  const sinceMs = nowMs - (track.switchMs[e] ?? Number.NaN);
  if (!(sinceMs >= 0)) {
    return on ? 0 : 1;
  }
  const from = track.fromShare[e] ?? (on ? 0 : 1);
  const moved = sinceMs / STRIDE_HOLD_FADE_MS;
  return on ? Math.max(0, from - moved) : Math.min(1, from + moved);
}

/** The onsets and releases of a view's holds at k = 0 (see the module's remarks), kept by patch. */
export class StrideHoldFades {
  readonly #tracks = new Map<string, Track>();
  #frame = 0;

  /**
   * Patch `keyString`'s shares word this frame at `nowMs`, a byte an edge: `heldEdges` its holds at
   * k = 0 (bits 0 to 3, `EDGES`' order), `bandsM` its edges' shared bands (`StrideEdges.edgesOf`),
   * whose level-2 bands on a held edge still fading in it rewrites with the edge's last before the
   * hold. A patch first seen takes no fade.
   */
  fade(keyString: string, heldEdges: number, bandsM: Float64Array, nowMs: number): number {
    let track = this.#tracks.get(keyString);
    const fresh = track === undefined;
    if (track === undefined) {
      track = {
        held: heldEdges,
        level2M: new Float64Array(32).fill(NO_STRIDE_BAND),
        fromShare: new Float64Array(4).fill(Number.NaN),
        switchMs: new Float64Array(4).fill(Number.NaN),
        seen: this.#frame,
      };
      this.#tracks.set(keyString, track);
    }
    let word = 0;
    for (let e = 0; e < 4; e += 1) {
      const on = (heldEdges & (1 << e)) !== 0;
      const was = (track.held & (1 << e)) !== 0;
      if (!fresh && on !== was) {
        // From where the fade the hold leaves had come to.
        track.fromShare[e] = shareOf(track, e, was, nowMs);
        track.switchMs[e] = nowMs;
      }
      const share = shareOf(track, e, on, nowMs);
      const byte = fadeByte(on ? share : 1 - share);
      word |= byte << (8 * e);
      for (let t = 0; t < 8; t += 1) {
        const at = edgeBandIndex(e, t, 2);
        if (!on) {
          track.level2M[8 * e + t] = bandsM[at] ?? NO_STRIDE_BAND;
        } else if (byte > 0) {
          bandsM[at] = track.level2M[8 * e + t] ?? NO_STRIDE_BAND;
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
