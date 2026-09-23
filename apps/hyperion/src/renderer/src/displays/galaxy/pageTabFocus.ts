import { createContext, useContext } from "react";

/**
 * Moves the focus to the tab of the galaxy page that a component stands on; `null` outside the
 * pages.
 *
 * @remarks
 * A page's tab heads it and is on show whenever the page is. A control that goes as it is pressed,
 * such as a `RETRY` whose request is then pending, hands the focus to it rather than leave it on
 * the document's body, where a keyboard operator loses their place (WCAG 2.4.3; the orchestrator's
 * ruling 18). Provided by `GalaxyPages`, one function to each page.
 */
export const PageTabFocus = createContext<(() => void) | null>(null);

/** Moves the focus to the tab of the page the calling component stands on; `null` outside them. */
export function usePageTabFocus(): (() => void) | null {
  return useContext(PageTabFocus);
}
