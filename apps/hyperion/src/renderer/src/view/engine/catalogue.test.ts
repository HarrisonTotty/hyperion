import { describe, expect, it } from "vitest";

import { WGSL_CATALOGUE } from "./catalogue";

/**
 * An effect's display name (decided 2026-10-02): upper case, at most three words, no colon and no
 * camelCase. The guide's `GRAPHICS SHADER REFUSED` row makes the catalogue the register of these
 * names.
 */
const DISPLAY_NAME = /^[A-Z][A-Z0-9]*( [A-Z0-9][A-Z0-9-]*){0,2}$/;

/** Every material's and post-process's display name; compute kernels dispatch no refusal. */
const DISPLAY_NAMES: ReadonlyArray<string> = WGSL_CATALOGUE.flatMap((entry) =>
  entry.kind === "compute" ? [] : [entry.spec.displayName],
);

describe("the catalogue's display names", () => {
  it("names R02's wireframe materials as decided", () => {
    expect(DISPLAY_NAMES).toEqual(
      expect.arrayContaining(["WIREFRAME LINES", "BODY OCCLUDER", "HULL OCCLUDER", "STAR SPRITES"]),
    );
  });

  it("gives every effect a name in the console's form", () => {
    expect(DISPLAY_NAMES.filter((name) => !DISPLAY_NAME.test(name))).toEqual([]);
  });

  it("gives no two effects the same name", () => {
    expect(DISPLAY_NAMES.filter((name, index) => DISPLAY_NAMES.indexOf(name) !== index)).toEqual(
      [],
    );
  });

  it("rejects the forms the rule excludes", () => {
    expect(
      ["wireframe:occluderSphere", "BODY OCCLUDER: SPHERE", "THE BODY SPHERE OCCLUDER", ""].map(
        (name) => DISPLAY_NAME.test(name),
      ),
    ).toEqual([false, false, false, false]);
  });
});

describe("the catalogue's compute kernels", () => {
  it("registers R05.T12.b's atmosphere tables", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining(["atmosphere transmittance", "atmosphere multiple scattering"]),
    );
  });
});
