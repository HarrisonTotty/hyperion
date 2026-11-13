import { useEffect, useId, useState, useSyncExternalStore } from "react";

import type { StillsApi } from "../../../../preload/api";
import { StillControl, type StillControlState } from "../../view/stills/StillControl";
import { StillList } from "../../view/stills/StillList";
import { StillStore } from "../../view/stills/stillStore";

/** Props of {@link ViewStills}. */
export interface ViewStillsProps {
  /** The panel's ID, by which its disclosure button controls it in the compact layout. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout. */
  readonly hidden?: boolean | undefined;
  /** The view a still is taken of, its system designator on the title row (`PRIMARY`). */
  readonly designator: string;
  /**
   * The preload's still functions, `window.hyperion.stills` unless given: read once, at the mount,
   * so that a new store takes a new `key`.
   */
  readonly stills?: StillsApi | undefined;
}

/**
 * Why `TAKE STILL` is held back until the photorealistic renderer draws a still's tiles: the draw
 * into R01's `createRenderTarget`, a hook in `photoreal/renderer.ts` (plan R11, R11.T10.b's
 * remainder), is not yet built.
 */
export const STILL_RENDERING_NOT_BUILT = "NOT AVAILABLE: still rendering not yet built";

/** The control while no still can run: offered nowhere, nothing in progress. */
const NOT_BUILT_STATE: StillControlState = {
  standing: "available",
  activity: { kind: "idle", last: null },
  heldBack: STILL_RENDERING_NOT_BUILT,
};

/**
 * `VIEW`'s stills (plan R11, R11.T10.c, Design note 17): the one mount of its still control and its
 * list of saved stills, in a panel of its own.
 *
 * @remarks
 * It costs nothing while no still runs: the store lists the folder once at the mount, through the
 * main process, and draws nothing on the GPU. A still is made only on request.
 */
export function ViewStills({ id, hidden, designator, stills }: ViewStillsProps) {
  const titleId = useId();
  const [store] = useState(() => new StillStore(stills ?? window.hyperion.stills));
  const listing = useSyncExternalStore(store.subscribe, store.getSnapshot);
  useEffect(() => {
    // The store keeps only the latest listing's answer, so a second run's (StrictMode's) is kept.
    store.refresh().catch((error: unknown) => {
      console.error("the stills could not be listed:", error);
    });
  }, [store]);
  return (
    <section className="panel view-stills" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        Stills <span className="panel__designator">{designator}</span>
      </h2>
      <StillControl state={NOT_BUILT_STATE} onTake={() => undefined} />
      <StillList state={listing} />
    </section>
  );
}
