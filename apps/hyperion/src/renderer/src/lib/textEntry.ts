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

/**
 * Whether a field's text is no entry at all: empty, or spaces only.
 *
 * @remarks
 * The guide's data-entry rule (decision-r07-t13d): a field that enters its value with `Enter` or
 * when it is left enters nothing for such text and never refuses it. As on `Escape`, it drops what
 * was typed and any refusal and shows its value again.
 */
export function isEmptyEntry(text: string): boolean {
  return text.trim() === "";
}
