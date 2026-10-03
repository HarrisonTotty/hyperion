import { describe, expect, it } from "vitest";

import {
  isOfflineUrl,
  isUncapturedGpuError,
  judgeSmokeRun,
  readSmokeImages,
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

describe("readSmokeImages", () => {
  /** A 2 × 1 image's eight bytes, base64. */
  const TWO_PIXELS = Buffer.from([1, 2, 3, 255, 4, 5, 6, 255]).toString("base64");
  const image = { name: "hillaire-orbit", width: 2, height: 1, rgba: TWO_PIXELS };

  it("reads well-formed images", () => {
    expect(readSmokeImages({ images: [image] })).toEqual({ images: [image], rejected: 0 });
  });

  it("reads none from a report without images", () => {
    expect(readSmokeImages({ variant: "default" })).toEqual({ images: [], rejected: 0 });
  });

  it("rejects a name that could leave the captures' directory", () => {
    expect(readSmokeImages({ images: [{ ...image, name: "../escape" }] }).rejected).toBe(1);
  });

  it("rejects bytes that do not fill width × height × 4", () => {
    expect(readSmokeImages({ images: [{ ...image, width: 3 }] }).rejected).toBe(1);
    expect(
      readSmokeImages({ images: [{ ...image, width: 0, height: 0, rgba: "" }] }).rejected,
    ).toBe(1);
  });

  it("keeps the good images beside a bad one and counts the bad", () => {
    const { images, rejected } = readSmokeImages({ images: [image, { ...image, rgba: "!!" }] });
    expect(images).toEqual([image]);
    expect(rejected).toBe(1);
  });
});
