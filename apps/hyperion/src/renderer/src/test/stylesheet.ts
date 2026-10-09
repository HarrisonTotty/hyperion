/**
 * The stylesheet's rules as text, for the tests that read what `styles.css` declares: jsdom
 * applies no stylesheet, and Vitest keeps the text of a CSS file imported with `?raw` alone.
 */
import STYLES from "../styles.css?raw";

/** A selector as a regular expression that matches it literally. */
function literal(selector: string): string {
  return selector.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

/**
 * The declarations of the stylesheet's rule for exactly `selector` (`.view-marks__label`,
 * `:root`), as written between its braces.
 *
 * @throws Error where the stylesheet has no rule for exactly that selector, so that a test of a
 * rule renamed, grouped or nested fails rather than reads nothing.
 */
export function stylesheetRule(selector: string): string {
  const rule = new RegExp(`(?:^|\\n)${literal(selector)} \\{([^}]*)\\}`, "u").exec(STYLES)?.[1];
  if (rule === undefined) {
    throw new Error(`styles.css has no rule for exactly ${selector}`);
  }
  return rule;
}
