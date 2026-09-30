/**
 * The colour tokens of `docs/frontend/ux-guidelines.md`, as `styles.css` defines them on `:root`.
 *
 * @remarks
 * jsdom applies no stylesheet, and Vitest loads CSS as empty text, so tests set the tokens that
 * canvases read from the computed style themselves. A token changed in the guide and the
 * stylesheet is changed here too.
 */
const COLOUR_TOKENS: Readonly<Record<string, string>> = {
  "--surface-0": "#05080d",
  "--surface-1": "#0b121c",
  "--surface-2": "#131e2d",
  "--line": "#1c2a3a",
  "--line-strong": "#557190",
  "--text": "#c8d6e5",
  "--text-muted": "#8a9db3",
  "--accent": "#5cc8e6",
  "--status-nominal": "#4ade80",
  "--status-advisory": "#60a5fa",
  "--status-caution": "#fbbf24",
  "--status-warning": "#f87171",
  "--target": "#e879f9",
};

/**
 * Sets the colour tokens on the root element, where every element inherits them, as the
 * stylesheet's `:root` rule does in the browser.
 */
export function applyColourTokens(): void {
  for (const [name, value] of Object.entries(COLOUR_TOKENS)) {
    document.documentElement.style.setProperty(name, value);
  }
}
