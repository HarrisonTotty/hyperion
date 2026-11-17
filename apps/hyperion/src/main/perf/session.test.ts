import { describe, expect, it } from "vitest";

import { arrivalOf, isLoopback, placeShipArgs, runBuildOf } from "./session";

describe("a perf run's session", () => {
  it("reads the build the recipe states, or refuses a run without it", () => {
    const commit = "0123456789abcdef0123456789abcdef01234567";
    expect(
      runBuildOf({
        HYPERION_PERF_COMMIT: commit,
        HYPERION_PERF_DIRTY: "1",
        HYPERION_PERF_GENERATOR_VERSION: "31",
      }),
    ).toEqual({ commit, dirty: true, generatorVersion: 31 });
    expect(runBuildOf({ HYPERION_PERF_GENERATOR_VERSION: "31" })).toMatch(/just perf-run/);
    expect(runBuildOf({ HYPERION_PERF_COMMIT: commit })).toMatch(/generator version/);
  });

  it("arrives through place-ship at the scene's pins, at the epoch", () => {
    const args = placeShipArgs("ws://127.0.0.1:43211/ws", {
      kind: "body",
      body: "4200acb20000000d.0004",
      survey: { source: "orbital", resolutionM: 50 },
    });
    expect(args).toEqual([
      "--address",
      "127.0.0.1",
      "--port",
      "43211",
      "--json",
      "--universe",
      "HYPERION PERF",
      "--seed",
      expect.stringMatching(/^[0-9a-f]{16}$/),
      "--time",
      "0",
      "--look-at",
      "4200acb20000000d.0004",
      "--survey",
      "orbital",
      "50",
    ]);
    expect(
      placeShipArgs("ws://127.0.0.1:43211/ws", { kind: "barycentre", system: "4200acb200000000" }),
    ).toEqual(expect.arrayContaining(["--look-at", "barycentre", "--system", "4200acb200000000"]));
  });

  it("reads the arrival from place-ship's JSON line", () => {
    const line = JSON.stringify({
      universe: { id: "3504954ff4feabf4", name: "HYPERION PERF", seed: "00000000000004d2" },
      created: true,
      system: "4200acb20000000d",
      body: "4200acb20000000d.0004",
    });
    expect(arrivalOf(`listening\n${line}\n`)).toEqual({
      universe: "3504954ff4feabf4",
      system: "4200acb20000000d",
      body: "4200acb20000000d.0004",
    });
    expect(arrivalOf("no JSON")).toBeNull();
  });

  it("takes only this machine's addresses as the local server's", () => {
    expect(["127.0.0.1", "localhost", "::1"].every(isLoopback)).toBe(true);
    expect(isLoopback("192.0.2.7")).toBe(false);
  });
});
