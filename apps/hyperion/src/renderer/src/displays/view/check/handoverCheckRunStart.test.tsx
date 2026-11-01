import { afterEach, describe, expect, it, vi } from "vitest";

import {
  HANDOVER_CHECK_TIMING,
  type HandoverCheckDeps,
  runHandoverCheck,
} from "./handoverCheckRun";
import { HANDOVER_APPROACH } from "./handoverGeometry";
import { HandoverScenes, terrainTestHandoverWorld } from "./handoverScenes";
import { STOPPED } from "./viewsCheckRun";

/**
 * The hand-over check's start against `VIEW`'s own controls in a document (R10.T15.b.2's runs): a
 * served launch opens on `SERVER`, whose primary never turns photorealistic, so that the run
 * chooses a kept scene before its first press. The run is stopped once it has chosen, the document
 * standing in for `VIEW`.
 */

/** `VIEW`'s scene bar and its primary's canvas, drawn wireframe, the scene `pressed` chosen. */
function viewWith(pressed: string): void {
  const buttons = ["SERVER", "PRECISION TEST", "PHASE TEST"]
    .map((name) => `<button aria-pressed="${String(name === pressed)}">${name}</button>`)
    .join("");
  document.body.innerHTML =
    `<fieldset aria-label="Scene">${buttons}</fieldset>` +
    `<canvas role="application" aria-label="VIEW, WIREFRAME, PRIMARY"></canvas>`;
}

/** What the run did before it was stopped: the scenes it chose, in order. */
function chosenScenes(abort: AbortController): string[] {
  const chosen: string[] = [];
  for (const button of document.querySelectorAll("fieldset button")) {
    button.addEventListener("click", () => {
      const name = button.textContent;
      chosen.push(name);
      for (const each of document.querySelectorAll("fieldset button")) {
        each.setAttribute("aria-pressed", String(each === button));
      }
      abort.abort();
    });
  }
  return chosen;
}

/** The run's deps over the document, `TERRAIN TEST`'s world standing in for a served one. */
function deps(signal: AbortSignal): HandoverCheckDeps {
  return {
    api: { capture: vi.fn<(label: string) => Promise<void>>(() => Promise.resolve()) },
    views: { watch: () => () => undefined, canvasOf: () => ({ widthPx: 0, heightPx: 0 }) },
    terrain: {
      session: null,
      compiled: false,
      sessions: () => [],
      coverageCells: () => new Set<number>(),
      normalMap: () => null,
      readSceneTarget: () => Promise.reject(new Error("not read")),
      faults: () => [],
    },
    scenes: new HandoverScenes(terrainTestHandoverWorld(), HANDOVER_APPROACH.full, 9e6),
    setting: "low",
    document,
    timing: { ...HANDOVER_CHECK_TIMING, waitMs: 1_000, pollMs: 1 },
    nowMs: () => performance.now(),
    sleep: (ms) =>
      new Promise((resolve) => {
        setTimeout(resolve, ms);
      }),
    signal,
    devicePixelRatio: 1,
  };
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("the hand-over check's start (R10.T15.b.2)", () => {
  it("leaves SERVER for PHASE TEST before its first photorealistic press", async () => {
    viewWith("SERVER");
    const abort = new AbortController();
    const chosen = chosenScenes(abort);
    await expect(runHandoverCheck(deps(abort.signal))).rejects.toThrow(STOPPED);
    expect(chosen).toEqual(["PHASE TEST"]);
  });

  it("stays on the kept scene a fixture's launch opens on", async () => {
    viewWith("PRECISION TEST");
    const abort = new AbortController();
    const chosen = chosenScenes(abort);
    const run = runHandoverCheck(deps(abort.signal));
    // It goes on to the photorealistic press, whose CONTROLS the document does not draw.
    await expect(run).rejects.toThrow("CONTROLS PRIMARY was not found");
    expect(chosen).toEqual([]);
  });
});
