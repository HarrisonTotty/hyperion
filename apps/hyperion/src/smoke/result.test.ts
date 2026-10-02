import { describe, expect, it } from "vitest";

import {
  isOfflineUrl,
  isUncapturedGpuError,
  judgeSmokeRun,
  readSmokeResult,
  SMOKE_EXIT,
  type SmokeResult,
  UNCAPTURED_GPU_ERROR,
} from "./result";

const PASSING: SmokeResult = {
  variant: "default",
  adapter: "google/swiftshader, fallback true",
  capabilities: "subgroups yes",
  checks: [{ name: "T9.a", pass: true, detail: "ok" }],
  setupError: null,
};

describe("the smoke run's verdict", () => {
  it("passes when every check passes and nothing left the page", () => {
    const { lines, exitCode } = judgeSmokeRun(PASSING, [], []);
    expect(exitCode).toBe(SMOKE_EXIT.pass);
    expect(lines).toContain("PASS T9.a: ok");
    expect(lines).toContain("cancelled requests []");
  });

  it("fails on a failed check", () => {
    const failed = { ...PASSING, checks: [{ name: "T9.d", pass: false, detail: "upside down" }] };
    expect(judgeSmokeRun(failed, [], [])).toMatchObject({ exitCode: SMOKE_EXIT.failed });
  });

  it("fails on a cancelled request, naming its URL and resource type", () => {
    const { lines, exitCode } = judgeSmokeRun(
      PASSING,
      [{ url: "https://example.invalid/a.json", resourceType: "xhr" }],
      [],
    );
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("cancelled requests [xhr https://example.invalid/a.json]");
  });

  it("fails when the page ran no checks", () => {
    const { lines, exitCode } = judgeSmokeRun({ ...PASSING, checks: [] }, [], []);
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("FAIL the page ran no checks");
  });

  it("fails on an uncaptured GPU error, printing it", () => {
    const message = `${UNCAPTURED_GPU_ERROR}: Invalid QuerySet`;
    const { lines, exitCode } = judgeSmokeRun(PASSING, [], [message]);
    expect(exitCode).toBe(SMOKE_EXIT.failed);
    expect(lines).toContain("uncaptured GPU errors 1");
    expect(lines).toContain(`GPU ERROR ${message}`);
  });

  it("knows the engine's log of an uncaptured GPU error from other page errors", () => {
    expect(isUncapturedGpuError(`${UNCAPTURED_GPU_ERROR}: Invalid QuerySet`)).toBe(true);
    expect(isUncapturedGpuError("material broken failed to compile")).toBe(false);
  });

  it("is a setup error, exit 2, with no adapter", () => {
    const none = { ...PASSING, adapter: null, checks: [], setupError: "no adapter: no-adapter" };
    expect(judgeSmokeRun(none, [], [])).toMatchObject({ exitCode: SMOKE_EXIT.setup });
  });
});

describe("the page's report", () => {
  it("is read when well formed and refused otherwise", () => {
    expect(readSmokeResult(PASSING)).toEqual(PASSING);
    expect(readSmokeResult({ ...PASSING, checks: [{ name: "x" }] })).toBeNull();
    expect(readSmokeResult("report")).toBeNull();
  });
});

describe("the offline rule", () => {
  it("lets only file: and data: URLs load", () => {
    expect(isOfflineUrl("file:///out/renderer/smoke.html")).toBe(true);
    expect(isOfflineUrl("data:text/plain,x")).toBe(true);
    expect(isOfflineUrl("https://example.invalid/")).toBe(false);
    expect(isOfflineUrl("devtools://devtools")).toBe(false);
  });
});
