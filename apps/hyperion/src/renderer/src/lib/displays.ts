/** Identifies one of the station's displays. Later plans add theirs here and in `App`. */
export type DisplayId = "link" | "galaxy";

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
] as const satisfies ReadonlyArray<DisplayDefinition>;
