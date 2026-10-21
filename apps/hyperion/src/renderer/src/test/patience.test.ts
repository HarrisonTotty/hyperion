import { describe, expect, it } from "vitest";

import { PATIENCE_VARIABLE, patienceMsOf } from "./patience";

/** The longest whole number of seconds a timer holds: 2³¹ − 1 ms, cut to the second. */
const LONGEST_TIMER_SECS = Math.floor((2 ** 31 - 1) / 1_000);

/** A `RangeError` whose message names the variable. */
const REFUSAL: unknown = expect.objectContaining({
  name: "RangeError",
  message: expect.stringContaining(PATIENCE_VARIABLE),
});

describe("the harness's patience for a variable's value", () => {
  it("is the server's 180 s when the variable is unset", () => {
    expect(patienceMsOf(undefined)).toBe(180_000);
  });

  it.each(["300", " 300 ", "+300", "0300"])(
    "reads %j as 300 s, as Rust's trim and parse do",
    (raw) => {
      expect(patienceMsOf(raw)).toBe(300_000);
    },
  );

  it.each(["0", "-5", "1.5", "abc", ""])("refuses %j, naming the variable", (raw) => {
    expect(() => patienceMsOf(raw)).toThrow(REFUSAL);
  });

  it("refuses a value that is not a string, naming the variable", () => {
    expect(() => patienceMsOf(300)).toThrow(REFUSAL);
  });

  it("takes the longest whole number of seconds a timer holds", () => {
    expect(patienceMsOf(String(LONGEST_TIMER_SECS))).toBe(LONGEST_TIMER_SECS * 1_000);
  });

  it("refuses one second more, which a timer would fire at once", () => {
    expect(() => patienceMsOf(String(LONGEST_TIMER_SECS + 1))).toThrow(REFUSAL);
  });
});
