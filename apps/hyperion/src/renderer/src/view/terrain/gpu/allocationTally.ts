/**
 * The engine's allocation and upload tally (plan R05, R05.T11.a, Design note 18): live and peak
 * GPU bytes per memory category and the bytes uploaded each frame, from R01's `onAllocation`. The
 * spike's metrics and R12's itemisation read it.
 *
 * @remarks
 * The tally starts when it is made and ignores a destruction it never saw created (R01's `frame
 * uniforms` buffer is made before a caller can listen, R01's Risks, m5). `uploaded` events carry no
 * category and include the engine's own uniform uploads, which are counted with the rest. A frame's
 * uploads are those since the last {@link AllocationTally.startFrame}.
 */

import type { AllocationEvent, MemoryCategory } from "../../engine/memory";
import type { RenderEngine } from "../../engine/types";

/** Live and peak GPU bytes per category, and uploads a frame. */
export interface AllocationTally {
  /** Bytes alive now in `category`. */
  liveBytes(category: MemoryCategory): number;
  /** The most bytes alive at once in `category`, since creation or {@link resetPeaks}. */
  peakBytes(category: MemoryCategory): number;
  /** Bytes uploaded by `writeBuffer`, `writeTexture` and the engine's own writes this frame. */
  uploadedBytesThisFrame(): number;
  /** Starts a frame: the upload count returns to zero. */
  startFrame(): void;
  /** Lowers every peak to its live bytes. */
  resetPeaks(): void;
  /** Calls `listener` after every event the tally counts; returns its unsubscribe. */
  subscribe(listener: () => void): () => void;
  /** Stops listening to the engine. */
  dispose(): void;
}

/** A tally over `engine`'s allocation events from now on. */
export function allocationTally(engine: RenderEngine): AllocationTally {
  const live = new Map<MemoryCategory, number>();
  const peak = new Map<MemoryCategory, number>();
  /** Each live allocation's bytes and category, by name, so that a release finds its creation. */
  const alive = new Map<string, { readonly bytes: number; readonly category: MemoryCategory }[]>();
  const listeners = new Set<() => void>();
  let uploaded = 0;

  const count = (event: AllocationEvent): void => {
    switch (event.kind) {
      case "created": {
        const list = alive.get(event.name) ?? [];
        list.push({ bytes: event.bytes, category: event.category });
        alive.set(event.name, list);
        const now = (live.get(event.category) ?? 0) + event.bytes;
        live.set(event.category, now);
        peak.set(event.category, Math.max(peak.get(event.category) ?? 0, now));
        break;
      }
      case "destroyed": {
        const list = alive.get(event.name);
        const index =
          list?.findIndex((a) => a.bytes === event.bytes && a.category === event.category) ?? -1;
        if (list === undefined || index < 0) {
          return;
        }
        list.splice(index, 1);
        if (list.length === 0) {
          alive.delete(event.name);
        }
        live.set(event.category, (live.get(event.category) ?? 0) - event.bytes);
        break;
      }
      case "uploaded":
        uploaded += event.bytes;
        break;
    }
    for (const listener of listeners) {
      listener();
    }
  };
  const unsubscribe = engine.onAllocation(count);

  return {
    liveBytes: (category) => live.get(category) ?? 0,
    peakBytes: (category) => peak.get(category) ?? 0,
    uploadedBytesThisFrame: () => uploaded,
    startFrame: () => {
      uploaded = 0;
    },
    resetPeaks: () => {
      peak.clear();
      for (const [category, bytes] of live) {
        peak.set(category, bytes);
      }
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose: unsubscribe,
  };
}
