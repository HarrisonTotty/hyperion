import { describe, expect, it } from "vitest";

import { takeFlag, takeOption } from "./paths.ts";

describe("the command line's options", () => {
  it("takes an option and its value out of the arguments", () => {
    const args = ["id", "--store", "s.jsonl", "--check"];
    expect(takeOption(args, "--store", "default")).toBe("s.jsonl");
    expect(args).toEqual(["id", "--check"]);
  });

  it("gives the fallback for an option not given", () => {
    expect(takeOption(["id"], "--store", "default")).toBe("default");
  });

  it("refuses an option without its value", () => {
    expect(() => takeOption(["id", "--store"], "--store", "default")).toThrow(
      "--store needs a value",
    );
  });

  it("takes a flag out of the arguments", () => {
    const args = ["--check", "x"];
    expect(takeFlag(args, "--check")).toBe(true);
    expect(args).toEqual(["x"]);
    expect(takeFlag(args, "--check")).toBe(false);
  });
});
