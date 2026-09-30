/** Input types that take no text, on which a letter key is free to be a command. */
const NON_TEXT_INPUTS: ReadonlySet<string> = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

/**
 * Whether a key press lands in a field that takes text, where a letter is typed, not a command.
 *
 * @remarks
 * A display's single-key bindings (plan 05, design note D3) are ignored there: a text input, a
 * `textarea`, a `select` or editable content.
 */
export function isTextEntry(target: EventTarget | null): boolean {
  return (
    (target instanceof HTMLInputElement && !NON_TEXT_INPUTS.has(target.type)) ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}
