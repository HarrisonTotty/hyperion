import { describe, expect, it } from "vitest";

import { errorText } from "./errorText";

describe("an error as a log line", () => {
  it("names a DOMException by its name and message, not as [object DOMException]", () => {
    expect(errorText(new DOMException("the device was lost", "AbortError"))).toBe(
      "AbortError: the device was lost",
    );
  });

  it("names an Error, and an object that carries a name and a message, alike", () => {
    expect([
      errorText(new TypeError("not a buffer")),
      errorText({ name: "OperationError", message: "mapping range too large" }),
    ]).toEqual(["TypeError: not a buffer", "OperationError: mapping range too large"]);
  });

  it("gives anything else as text", () => {
    expect([errorText("lost"), errorText(null), errorText({ name: 1 })]).toEqual([
      "lost",
      "null",
      "[object Object]",
    ]);
  });
});
