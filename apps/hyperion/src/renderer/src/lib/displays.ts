/** Identifies one of the station's displays. Later plans add theirs here and in `App`. */
export type DisplayId = "link" | "galaxy" | "system" | "view" | "planet";

/** A display as the navigation bar offers it. */
export interface DisplayDefinition {
  readonly id: DisplayId;
  /** Unique title, shown in the header strip and on the display's tab. */
  readonly title: string;
  /**
   * The `KeyboardEvent.key` that selects the display from anywhere, shown on its tab.
   *
   * @remarks
   * Function keys, because letters and digits would fire while the operator types a seed
   * (plan 05, design note D2).
   */
  readonly key: string;
}

/**
 * The station's displays in their fixed navigation order.
 *
 * @remarks
 * A new display is an entry here and a case in `App`'s exhaustive `switch`.
 */
export const DISPLAYS = [
  { id: "link", title: "Link", key: "F1" },
  { id: "galaxy", title: "Galaxy", key: "F2" },
  { id: "system", title: "System", key: "F3" },
  { id: "view", title: "View", key: "F4" },
] as const satisfies ReadonlyArray<DisplayDefinition>;

/**
 * The `PLANET VIEWER`, a development tool's display (plan R15): after the station's displays,
 * offered only where the launch allows it ({@link stationDisplays}).
 */
export const PLANET_VIEWER_DISPLAY = {
  id: "planet",
  title: "Planet Viewer",
  key: "F5",
} as const satisfies DisplayDefinition;

/**
 * The displays the navigation bar offers: the station's, and the `PLANET VIEWER` after them where
 * `planetViewer` is set (`window.hyperion.planetViewer`: a client that is not packaged, or one
 * launched with `--planet-viewer`).
 */
export function stationDisplays(planetViewer: boolean): ReadonlyArray<DisplayDefinition> {
  return planetViewer ? [...DISPLAYS, PLANET_VIEWER_DISPLAY] : DISPLAYS;
}
