import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { InThreadSkyWorker } from "../../test/skyFixtures";
import {
  nominalStore,
  openUniverse,
  renderViewDisplay,
  sceneArrives,
  settle,
  submittedBy,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import type { BodyFramePlan } from "../../view/bodies/draw";
import { PHOTOREAL_PASS_LABELS } from "../../view/photoreal/passes";
import { PhotorealRenderer } from "../../view/photoreal/renderer";
import { histogramWorkgroups } from "../../view/post/histogram";
import type { QualitySetting } from "../../view/quality/qualitySetting";

afterEach(() => {
  vi.useRealTimers();
});

/** The primary's stage, CSS px and device px: taller than the low setting's 720 rows. */
const STAGE_PX = { widthPx: 1600, heightPx: 900 } as const;

interface Setup extends ViewDisplayHarness {
  readonly fake: ReturnType<typeof timedEngineSource>;
}

/** `VIEW` given `setting`, its engine made and its first frames drawn. */
async function setup(setting: QualitySetting): Promise<Setup> {
  const fake = timedEngineSource();
  const harness = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    setting,
    stagePx: STAGE_PX,
  });
  await settle();
  harness.advance(100);
  return { ...harness, fake };
}

/** Switches the `CONTROLS` view to the photorealistic style by its key and lets it draw. */
async function photorealControls({ user, advance }: Setup): Promise<void> {
  await user.keyboard("4");
  // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
  advance(100);
  await settle();
  advance(300);
}

/** `VIEW` given `setting` drawing `PHASE TEST`, in the wireframe until a test switches it. */
async function phaseTest(setting: QualitySetting): Promise<Setup> {
  const view = await setup(setting);
  await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
  view.advance(100);
  return view;
}

/** The sizes the primary's scene target was made and resized at, first first. */
function sceneTargetSizes(view: Setup): ReadonlyArray<{ widthPx: number; heightPx: number }> {
  return view.fake.targets.findLast((target) => target.spec.name === "view:hdr")?.sizes ?? [];
}

/** The levels of a view's bloom chain, from the down targets it made. */
function bloomLevels(view: Setup, name: string): ReadonlyArray<number> {
  return view.fake.targets
    .map((target) => new RegExp(`^${name}:bloom down (\\d+)$`, "u").exec(target.spec.name)?.[1])
    .filter((level) => level !== undefined)
    .map(Number);
}

const PHOTOREAL_CASES = [
  {
    setting: "high",
    internal: { widthPx: 1600, heightPx: 900 },
    stride: 1,
    levels: [1, 2, 3, 4, 5, 6],
    annuli: 4,
    planetshine: 2,
  },
  {
    setting: "low",
    internal: { widthPx: 1280, heightPx: 720 },
    stride: 2,
    levels: [1, 2, 3, 4, 5],
    annuli: 3,
    planetshine: 1,
  },
] as const;

describe("VIEW's photorealistic frame at the setting it is given (R07.T17)", () => {
  it.each(PHOTOREAL_CASES)(
    "renders the $setting setting's scene target at $internal.widthPx × $internal.heightPx on a 1600 × 900 stage",
    async ({ setting, internal }) => {
      const view = await phaseTest(setting);
      await photorealControls(view);
      expect(sceneTargetSizes(view).at(-1)).toEqual(internal);
    },
  );

  it.each(PHOTOREAL_CASES)(
    "takes the $setting setting's histogram at a stride of $stride",
    async ({ setting, internal, stride }) => {
      const view = await phaseTest(setting);
      await photorealControls(view);
      const histograms = view.fake.submissions.filter(
        (each) => each.label === PHOTOREAL_PASS_LABELS.histogram && submittedBy(each) === "view",
      );
      expect(histograms.at(-1)?.workgroups).toEqual(histogramWorkgroups(internal, stride));
    },
  );

  it.each(PHOTOREAL_CASES)(
    "blooms over the $setting setting's levels $levels",
    async ({ setting, levels }) => {
      const view = await phaseTest(setting);
      await photorealControls(view);
      expect(bloomLevels(view, "view")).toEqual(levels);
    },
  );

  it.each(PHOTOREAL_CASES)(
    "lights PHASE TEST's discs with the $setting setting's $annuli annuli and at most $planetshine planetshine sources",
    async ({ setting, annuli, planetshine }) => {
      // The plan the renderer drew from: its records are uploaded, which the fake engine drops.
      const render = vi.spyOn(PhotorealRenderer.prototype, "render");
      const view = await phaseTest(setting);
      await photorealControls(view);
      const result = render.mock.results.at(-1);
      const plan: BodyFramePlan | null = result?.type === "return" ? result.value : null;
      const discs = plan?.discs ?? [];
      const annuliCounts = new Set(
        discs
          .flatMap((disc) => disc.lights.flatMap((light) => light.annuli))
          .map((set) => set.flux.length),
      );
      const mostSecondaries = Math.max(...discs.map((disc) => disc.secondaries.length));
      expect([discs.length > 0, [...annuliCounts], mostSecondaries]).toEqual([
        true,
        [annuli],
        planetshine,
      ]);
    },
  );

  it.each(PHOTOREAL_CASES)(
    "draws a photorealistic instrument beside a wireframe primary at the $setting setting's levels $levels",
    async ({ setting, levels }) => {
      const view = await phaseTest(setting);
      await view.user.click(
        within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
          name: "OPEN",
        }),
      );
      view.advance(300);
      await view.user.click(
        within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", {
          name: "INSTRUMENT 1",
        }),
      );
      await photorealControls(view);
      expect(bloomLevels(view, "instrument-1")).toEqual(levels);
    },
  );
});

/** The `PRIMARY` view's label block's `QUALITY` reading. */
function qualityReading(): string | null {
  const block = screen.getByText("VIEW", { selector: "p" }).parentElement;
  const term = block === null ? null : within(block).queryByText("QUALITY", { selector: "dt" });
  return term?.nextElementSibling?.textContent ?? null;
}

describe("VIEW's statement of the setting it is given (R07.T17)", () => {
  it.each([
    { setting: "high", words: "HIGH" },
    { setting: "low", words: "LOW" },
  ] as const)("reads QUALITY $words on the primary's label block", async ({ setting, words }) => {
    const view = await setup(setting);
    view.advance(300);
    expect(qualityReading()).toBe(words);
  });
});

/** The segments the primary's last wireframe frame drew. */
function wireframeSegments(view: Setup): number {
  const frame = view.fake.submissions.findLast(
    (each) => each.by === "view" && each.materials.includes("wireframe:lines"),
  );
  return (frame?.materials ?? []).reduce(
    (sum, material, i) => sum + (material === "wireframe:lines" ? (frame?.instances[i] ?? 0) : 0),
    0,
  );
}

describe("VIEW's wireframe at the setting it is given (R07.T17)", () => {
  it("draws R02's low wireframe on low, PHASE TEST's graticules at 30° only and so fewer lines", async () => {
    const high = await phaseTest("high");
    high.advance(300);
    const atHigh = wireframeSegments(high);
    cleanup();
    const low = await phaseTest("low");
    low.advance(300);
    const atLow = wireframeSegments(low);
    expect([atHigh > 0, atLow > 0, atLow < atHigh]).toEqual([true, true, true]);
  });
});

/** `VIEW` given `setting` over the server's scene. */
async function serverSceneAt(setting: QualitySetting): Promise<Setup> {
  vi.stubGlobal("Worker", InThreadSkyWorker);
  const view = await setup(setting);
  await openUniverse(view);
  await sceneArrives(view);
  await settle();
  view.advance(300);
  await settle();
  return view;
}

describe("VIEW's sky at the setting it is given (R07.T17)", () => {
  it.each([
    { setting: "high", nMax: 300_000 },
    { setting: "low", nMax: 100_000 },
  ] as const)("asks the $setting setting's N_max, $nMax stars", async ({ setting, nMax }) => {
    const view = await serverSceneAt(setting);
    expect(view.socket.requestsOfKind("sky").at(-1)?.body.n_max).toBe(nMax);
  });
});
