/**
 * The `aria-describedby` of a control from the IDs of what describes it now, `null` for a part not
 * shown: the IDs joined by spaces, or `undefined` for none, so that the attribute is left out.
 */
export function describedBy(ids: ReadonlyArray<string | null>): string | undefined {
  const shown = ids.filter((id) => id !== null);
  return shown.length === 0 ? undefined : shown.join(" ");
}
