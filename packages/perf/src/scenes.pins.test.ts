import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { PERF_PINS } from "./scenes.ts";

/** The server's test of the pins (R12.T5.b), whose constants `PERF_PINS` repeats. */
const PINS_TEST = new URL("../../../crates/hyperion-server/tests/perf_scenes.rs", import.meta.url);

/** The text of the Rust constant `name`, written `const <name>: &str = "<text>";`, if it is there. */
function rustText(source: string, name: string): string | undefined {
  return new RegExp(`const ${name}: &str = "([^"]*)";`).exec(source)?.[1];
}

describe("the perf scenes' pins", () => {
  it("are the server test's own (crates/hyperion-server/tests/perf_scenes.rs)", () => {
    const source = readFileSync(PINS_TEST, "utf8");
    expect({
      seed: rustText(source, "SEED_HEX"),
      system: rustText(source, "NEAR_SUN_SYSTEM_HEX"),
      world: rustText(source, "EARTH_SIZED_WORLD"),
      giant: rustText(source, "RINGED_GIANT"),
      nuclearSystem: rustText(source, "NUCLEAR_DISC_SYSTEM_HEX"),
    }).toEqual(PERF_PINS);
  });
});
