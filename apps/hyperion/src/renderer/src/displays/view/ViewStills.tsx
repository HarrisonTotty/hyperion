import { useEffect, useId, useSyncExternalStore } from "react";

import { STILL_CONTROL_IDLE, StillControl } from "../../view/stills/StillControl";
import { StillList } from "../../view/stills/StillList";
import { useStillControl, type ViewStillsModel } from "./useViewStills";

/** Props of {@link ViewStills}. */
export interface ViewStillsProps {
  /** The panel's ID, by which its disclosure button controls it in the compact layout. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout. */
  readonly hidden?: boolean | undefined;
  /** The view a still is taken of, its system designator on the title row (`PRIMARY`). */
  readonly designator: string;
  /** `VIEW`'s stills (`useViewStills`). */
  readonly model: ViewStillsModel;
}

/**
 * `VIEW`'s stills (plan R11, R11.T10.c, Design note 17): the one mount of its still control and its
 * list of saved stills, in a panel of its own, which stands only where `VIEW` can make a still.
 *
 * @remarks
 * It costs nothing while no still runs: the store lists the folder once at the mount, through the
 * main process, and nothing is drawn on the GPU until `TAKE STILL` asks for a still, whose job
 * (`stillJob.ts`) saves it through the store, which lists the folder again.
 */
export function ViewStills({ id, hidden, designator, model }: ViewStillsProps) {
  const titleId = useId();
  const { store, session } = model;
  const listing = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const control = useStillControl(model) ?? STILL_CONTROL_IDLE;
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
      <StillControl
        state={control}
        onTake={() => {
          session.request().catch((error: unknown) => {
            console.error("a still could not be made:", error);
          });
        }}
        onSize={(scale) => {
          session.setScale(scale);
        }}
      />
      <StillList state={listing} />
    </section>
  );
}
