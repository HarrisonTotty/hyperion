import { type ReactNode, useId } from "react";

import { type PlaceIds, type PlacePage, PlaceTabs } from "./PlaceTabs";

/** Props of {@link PlacePanel}. */
export interface PlacePanelProps {
  /** The page standing: `GROUND` only where it is chosen and offered. */
  readonly page: PlacePage;
  /** Why `GROUND` is held back, or `null` where it is offered. */
  readonly groundReason: string | null;
  readonly onPage: (page: PlacePage) => void;
  /** The instrument's name, the place's designator (`INSTRUMENT 1`). */
  readonly designator: string;
  /** The place's ID, by which the compact layout's `CAMERA` disclosure button controls it. */
  readonly id: string;
  /** Whether the compact layout folds the place. */
  readonly hidden: boolean;
  /** The page's body: the camera controls, bare, or the `GROUND` page's plate. */
  readonly children: ReactNode;
}

/**
 * The camera panel's place while `CONTROLS` names an instrument (plan R10, R10.T12.d; guide draft
 * D2): one panel, named for the page it shows, its tabs at its head ({@link PlaceTabs}) and the page
 * under them.
 *
 * @remarks
 * The panel and its tabs stay mounted as the page changes, the page alone swapped, so that the
 * focus on a tab stays where it is.
 */
export function PlacePanel({
  page,
  groundReason,
  onPage,
  designator,
  id,
  hidden,
  children,
}: PlacePanelProps) {
  const baseId = useId();
  const ids: PlaceIds = {
    tab: (each) => `${baseId}-tab-${each}`,
    panel: `${baseId}-page`,
    reason: `${baseId}-reason`,
  };
  return (
    <section
      className="panel view-camera view-place"
      aria-label={`${page === "ground" ? "Ground" : "Camera"} ${designator}`}
      id={id}
      hidden={hidden}
    >
      <PlaceTabs
        page={page}
        groundReason={groundReason}
        onPage={onPage}
        designator={designator}
        ids={ids}
      />
      <div
        role="tabpanel"
        id={ids.panel}
        aria-labelledby={ids.tab(page)}
        className={page === "ground" ? "view-place__page view-ground" : "view-place__page"}
      >
        {children}
      </div>
    </section>
  );
}
