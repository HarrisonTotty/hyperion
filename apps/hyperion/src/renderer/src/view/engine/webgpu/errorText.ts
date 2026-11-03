/**
 * An error as the engine's log lines give it: its name and message.
 *
 * @remarks
 * An error handed to `console.error` beside its text reaches the main process's log as
 * `[object DOMException]` where it is a `DOMException`, as WebGPU's are, which the owner's Mac runs
 * logged 47 to 64 times a terrain run (R14's Mac measurements); its name and message say which.
 */

/**
 * An error's name and message, or `null` for a value with neither: an `Error`'s, or those of an
 * object that carries both, as a `DOMException` does whether or not its engine makes it an `Error`.
 */
export function errorParts(
  error: unknown,
): { readonly name: string; readonly message: string } | null {
  if (error instanceof Error) {
    return { name: error.name, message: error.message };
  }
  if (typeof error !== "object" || error === null) {
    return null;
  }
  const name: unknown = Reflect.get(error, "name");
  const message: unknown = Reflect.get(error, "message");
  return typeof name === "string" && typeof message === "string" ? { name, message } : null;
}

/** An error as one line, `<name>: <message>`, or the value as text where it has neither. */
export function errorText(error: unknown): string {
  const parts = errorParts(error);
  return parts === null ? String(error) : `${parts.name}: ${parts.message}`;
}
