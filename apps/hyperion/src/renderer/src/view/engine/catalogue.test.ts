import { describe, expect, it } from "vitest";

import { type CatalogueEntry, WGSL_CATALOGUE } from "./catalogue";

/**
 * An effect's display name (decided 2026-10-02): upper case, at most three words, no colon and no
 * camelCase. The guide's `GRAPHICS SHADER REFUSED` row makes the catalogue the register of these
 * names.
 */
const DISPLAY_NAME = /^[A-Z][A-Z0-9]*( [A-Z0-9][A-Z0-9-]*){0,2}$/;

/** The display name of an entry that carries one: kernels and splats dispatch no refusal. */
function displayNameOf(entry: CatalogueEntry): string | null {
  let name: string | null;
  switch (entry.kind) {
    case "material":
    case "post-process":
      name = entry.spec.displayName;
      break;
    case "compute":
    case "point-splat":
      name = null;
      break;
  }
  return name;
}

/** Every material's and post-process's display name. */
const DISPLAY_NAMES: ReadonlyArray<string> = WGSL_CATALOGUE.flatMap((entry) => {
  const name = displayNameOf(entry);
  return name === null ? [] : [name];
});

describe("the catalogue's kinds", () => {
  it("holds a point splat in the splat's format and blend, with no display name", () => {
    const splats = WGSL_CATALOGUE.filter((entry) => entry.kind === "point-splat");
    expect(splats.length).toBeGreaterThan(0);
    for (const entry of splats) {
      expect(displayNameOf(entry)).toBeNull();
      expect(entry.spec).toMatchObject({ format: "rgba32float", blend: "additive" });
    }
  });
});

describe("the catalogue's display names", () => {
  it("names R02's wireframe materials as decided", () => {
    expect(DISPLAY_NAMES).toEqual(
      expect.arrayContaining(["WIREFRAME LINES", "BODY OCCLUDER", "HULL OCCLUDER", "STAR SPRITES"]),
    );
  });

  it("names both terrain vertex paths' materials on the console", () => {
    expect(DISPLAY_NAMES).toEqual(expect.arrayContaining(["TERRAIN", "TERRAIN OFFSETS"]));
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

  it("registers R07.T8.d's cell pass as BODY DISC CELLS", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toContain("BODY DISC CELLS");
  });
});
