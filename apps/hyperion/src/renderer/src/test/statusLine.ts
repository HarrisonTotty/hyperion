import type { MatcherFunction } from "@testing-library/react";

/**
 * A text matcher for a status line by its whole words, as `StatusText` sets them: the element that
 * holds the line's parts (its `output`, `p` or `span`) and whose text is `text`, or matches it.
 *
 * @remarks
 * `getByText`'s own matcher reads only an element's own text nodes. It finds a line split into a
 * state and a sentence nowhere, and finds a line set as one state in its part's span rather than in
 * the element that carries the line's classes and ID. Use it for absence too (`queryByText`), which
 * a whole line's string would otherwise pass for nothing.
 */
export function statusLine(text: string | RegExp): MatcherFunction {
  return (_content, element) =>
    element !== null &&
    element.querySelector(":scope > .status-text__state") !== null &&
    (typeof text === "string" ? element.textContent === text : text.test(element.textContent));
}

/**
 * The words of the elements that describe `element` (its `aria-describedby`, in order), each with
 * its white space collapsed and joined by a space: its accessible description as Chromium gives it,
 * for a description made of text.
 *
 * @remarks
 * For a control described by a status line that `StatusText` sets. jest-dom reads a description
 * through dom-accessibility-api, which trims each element's text before joining it to the text
 * beside it, so a sentence's part, which begins with the space after the colon, runs into its state
 * there (`REJECTED:busy`). Chromium keeps the space (`REJECTED: busy`, read from its accessibility
 * tree in P05.T13's hidden run), and Chromium is what the console's assistive technology reads.
 *
 * It reads text alone: a describing element holding a part that the accessibility tree names or
 * hides otherwise (`aria-hidden`, `aria-label`, `alt`) is refused rather than read wrongly.
 *
 * @throws Error where a describing element holds such a part.
 */
export function describedText(element: Element): string {
  const ids = (element.getAttribute("aria-describedby") ?? "")
    .split(/\s+/u)
    .filter((id) => id.length > 0);
  return ids
    .map((id) => {
      const describing = element.ownerDocument.getElementById(id);
      if ((describing?.querySelector("[aria-hidden], [aria-label], [alt]") ?? null) !== null) {
        throw new Error(`#${id} holds a part named or hidden apart from its text`);
      }
      return describing?.textContent ?? "";
    })
    .join(" ")
    .replace(/\s+/gu, " ")
    .trim();
}
