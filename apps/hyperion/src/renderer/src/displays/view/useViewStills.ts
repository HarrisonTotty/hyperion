import { useState, useSyncExternalStore } from "react";

import type { StillsApi } from "../../../../preload/api";
import {
  type StillControlState,
  stillStatusLine,
  stillsStandingLine,
} from "../../view/stills/StillControl";
import { encodeStillPng, StillSession, type StillSource } from "../../view/stills/stillJob";
import { StillStore } from "../../view/stills/stillStore";

/** `VIEW`'s stills: made once, so that a memoised stage takes them as a stable prop. */
export interface ViewStillsModel {
  /** The client's saved stills, which a saved still lists again. */
  readonly store: StillStore;
  /** The session's stills: at most one job, and the loss rule. */
  readonly session: StillSession;
}

/**
 * `VIEW`'s stills (plan R11, R11.T10.b): its store and its session over `source`, each made once at
 * the mount, kept through every scene; `null` where `VIEW` has no still source (the photorealistic
 * renderer's hook, `PhotorealStills`), when it offers no still.
 *
 * @param source - Read once, at the mount: a new source takes a new mount.
 * @param stills - The preload's still functions (`window.hyperion.stills`), read once with it.
 */
export function useViewStills(
  source: StillSource | undefined,
  stills: StillsApi,
): ViewStillsModel | null {
  const [model] = useState((): ViewStillsModel | null => {
    if (source === undefined) {
      return null;
    }
    const store = new StillStore(stills);
    const session = new StillSession({
      source,
      encode: encodeStillPng,
      save: (png, label) => store.save(png, label),
      nowMs: () => performance.now(),
      reportError: (error) => {
        console.error("a still could not be made:", error);
      },
    });
    return { store, session };
  });
  return model;
}

/** A subscription to nothing, for a display that offers no still. */
function noSubscription(): () => void {
  return () => undefined;
}

/** No control's state, for a display that offers no still. */
function noControl(): null {
  return null;
}

/** The still control's state as `model`'s session gives it, or `null` without stills. */
export function useStillControl(model: ViewStillsModel | null): StillControlState | null {
  return useSyncExternalStore(
    model?.session.subscribe ?? noSubscription,
    model?.session.getSnapshot ?? noControl,
  );
}

/** No line, for a display that offers no still. */
function noLine(): null {
  return null;
}

/**
 * The stills' lines as they stand away from their panel, under the fold row or MAXIMIZE VIEW's
 * line: the session's standing, with its cause where disabled, and the still's status, its step or
 * how the last ended. Each is a string, so that a subscriber renders again only when its words
 * change, never at the job's 4 Hz progress.
 */
export function useStillsAwayLines(model: ViewStillsModel | null): {
  readonly standing: string | null;
  readonly status: string | null;
} {
  const subscribe = model?.session.subscribe ?? noSubscription;
  const standing = useSyncExternalStore(
    subscribe,
    model === null
      ? noLine
      : () => stillsStandingLine(model.session.getSnapshot().standing, "away"),
  );
  const status = useSyncExternalStore(
    subscribe,
    model === null ? noLine : () => stillStatusLine(model.session.getSnapshot()),
  );
  return { standing, status };
}
