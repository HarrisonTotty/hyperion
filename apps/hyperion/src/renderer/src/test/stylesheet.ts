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

/**
 * The declarations of the rule for exactly `selector` inside the stylesheet's at-rule `atRule`
 * (`@container (max-width: 42rem)`), as written between its braces: an at-rule's rules are
 * indented, which {@link stylesheetRule} does not read. The first such at-rule that holds a rule
 * for the selector is read, so that two at-rules of one condition (a list's and a readout's) do not
 * hide each other.
 *
 * @throws Error where the stylesheet has no such at-rule holding a rule for exactly that selector.
 */
export function stylesheetRuleIn(atRule: string, selector: string): string {
  const opening = `\n${atRule} {`;
  let from = STYLES.indexOf(opening);
  while (from >= 0) {
    // The rules inside an at-rule are indented, so its first unindented brace closes it.
    const end = STYLES.indexOf("\n}", from + opening.length);
    const block = STYLES.slice(from + opening.length, end < 0 ? undefined : end).replace(
      /\n {2}/gu,
      "\n",
    );
    const rule = new RegExp(`\\n${literal(selector)} \\{([^}]*)\\}`, "u").exec(block)?.[1];
    if (rule !== undefined) {
      return rule;
    }
    from = STYLES.indexOf(opening, from + opening.length);
  }
  throw new Error(`styles.css has no rule for exactly ${selector} in ${atRule}`);
}
