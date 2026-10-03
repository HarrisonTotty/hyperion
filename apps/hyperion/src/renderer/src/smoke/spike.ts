/**
 * R05.T13.b's captures (plan R05): the descent spike's run, end to end on the harness's engine,
 * with its own workers, measured terrain, atmosphere and two instruments, read back from its three
 * canvases for the lane to look at, under `--smoke-captures`.
 */

import { readTokens } from "../spatial/paint";
import type { RenderEngine } from "../view/engine/types";
import {
  prepareDescent,
  DEFAULT_SPIKE_WORKERS,
  SpikeRun,
  type SpikeFrame,
} from "../view/spike/spikeRun";
import { SurfaceQuery } from "../view/spike/surfaceQuery";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, pause } from "./harness";

/** The spike's seed in the captures: T13.a's tests' own. */
const SEED = 5n;

/** The views' sizes, device pixels. */
const SIZES = {
  main: { widthPx: 480, heightPx: 270 },
  orbit: { widthPx: 240, heightPx: 180 },
  craft: { widthPx: 240, heightPx: 180 },
} as const;

/** The longest a shot waits for its patches, and how long with no new patch counts as settled. */
const SETTLE_MS = 120_000;
const QUIET_MS = 3_000;

/** One shot: a setting and a script time, seconds, or the script's end. */
interface SpikeShot {
  readonly name: string;
  readonly setting: QualitySetting;
  readonly tS: number | "end";
}

/** High on the descent arc, low in the low fast pass and at the hover's end. */
const SHOTS: ReadonlyArray<SpikeShot> = [
  { name: "spike-arc", setting: "high", tS: 400 },
  { name: "spike-low-pass", setting: "low", tS: 1_095 },
  { name: "spike-touchdown", setting: "low", tS: "end" },
];

/** How many pixels of an RGBA image differ from its top-left pixel, and its mean channel value. */
function survey(bytes: Uint8Array | Float32Array): {
  readonly drawn: number;
  readonly mean: number;
} {
  let drawn = 0;
  let sum = 0;
  for (let i = 0; i < bytes.length; i += 4) {
    if (bytes[i] !== bytes[0] || bytes[i + 1] !== bytes[1] || bytes[i + 2] !== bytes[2]) {
      drawn += 1;
    }
    sum += (bytes[i] ?? 0) + (bytes[i + 1] ?? 0) + (bytes[i + 2] ?? 0);
  }
  return { drawn, mean: sum / (0.75 * bytes.length) };
}

function imageOf(
  name: string,
  size: { widthPx: number; heightPx: number },
  bytes: Uint8Array | Float32Array,
): CapturedImage {
  return {
    name,
    width: size.widthPx,
    height: size.heightPx,
    rgba: base64Of(bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes)),
  };
}

/** R05.T13.b's captures: the spike's three canvases at each shot. */
export async function captureSpike(engine: RenderEngine, checks: Checks): Promise<CapturedImage[]> {
  const query = new SurfaceQuery(DEFAULT_SPIKE_WORKERS.query(), "off");
  const started = performance.now();
  const prepared = await prepareDescent(query, SEED).finally(() => {
    query.dispose();
  });
  checks.check(
    "R05.T13.b the descent was measured against the terrain",
    Number.isFinite(prepared.siteHeightM) &&
      Number.isFinite(prepared.trackMaxHeightM) &&
      prepared.contact.firstOnsetS !== null,
    `site ${prepared.siteHeightM.toFixed(1)} m, track bound ` +
      `${prepared.trackMaxHeightM.toFixed(1)} m, contact from ` +
      `${prepared.contact.firstOnsetS?.toFixed(3) ?? "never"} s, held from ` +
      `${prepared.contact.holdFromS?.toFixed(3) ?? "never"} s of ${prepared.profile.durationS} s, ` +
      `in ${Math.round(performance.now() - started)} ms`,
  );
  const tokens = readTokens(document.documentElement);
  const images: CapturedImage[] = [];
  for (const shot of SHOTS) {
    const canvases = { main: addCanvas(), orbit: addCanvas(), craft: addCanvas() };
    let run: SpikeRun | null = null;
    try {
      run = new SpikeRun(
        engine,
        prepared,
        { setting: shot.setting, ridges: "off", createPool: DEFAULT_SPIKE_WORKERS.pool(3) },
        canvases,
      );
      // The shots run in order: each streams, draws and reads back before the next.
      // oxlint-disable-next-line no-await-in-loop
      await run.ready();
      const tS = shot.tS === "end" ? prepared.profile.durationS : shot.tS;
      const input = (nowMs: number) => ({
        nowMs,
        sizes: SIZES,
        tokens,
        remPx: 16,
        selection: null,
      });
      run.frame(input(0));
      const begun = performance.now();
      let lastChange = begun;
      let resident = -1;
      let frame: SpikeFrame | null = null;
      let reads: Promise<Uint8Array | Float32Array>[] = [];
      for (;;) {
        frame = run.frame(input(tS * 1000));
        // A canvas texture expires once the task yields: read the frame back now.
        reads = [run.readBack("main"), run.readBack("orbit"), run.readBack("craft")];
        const now = performance.now();
        const t = frame.terrain;
        if (t.drawn - t.standingIn !== resident) {
          resident = t.drawn - t.standingIn;
          lastChange = now;
        }
        if (
          (t.standingIn === 0 && t.missing === 0) ||
          now - lastChange > QUIET_MS ||
          now - begun > SETTLE_MS
        ) {
          break;
        }
        // oxlint-disable-next-line no-await-in-loop
        await Promise.all(reads);
        // oxlint-disable-next-line no-await-in-loop
        await pause(50);
      }
      // oxlint-disable-next-line no-await-in-loop
      const [main, orbit, craft] = await Promise.all(reads);
      if (main === undefined || orbit === undefined || craft === undefined) {
        throw new Error("the spike's three views were not read back");
      }
      const t = frame.terrain;
      checks.check(
        `R05.T13.b the ${shot.name} shot streamed its patches`,
        t.standingIn === 0 && t.missing === 0,
        `${t.selected} selected, ${t.drawn} drawn, ${t.standingIn} standing in, ${t.missing} ` +
          `missing, limited ${String(t.limited)}, segment ${frame.pose.segment}, clearance ` +
          `${frame.pose.clearanceM.toFixed(1)} m, contact ${String(frame.contact)}, after ` +
          `${Math.round(performance.now() - begun)} ms (${shot.setting})`,
      );
      const lit = survey(main);
      checks.check(
        `R05.T13.b the ${shot.name} view is lit`,
        lit.mean > 4,
        `mean channel ${lit.mean.toFixed(1)} of 255`,
      );
      for (const [name, bytes] of [
        ["orbit", orbit],
        ["craft", craft],
      ] as const) {
        const drawn = survey(bytes);
        checks.check(
          `R05.T13.b the ${shot.name} ${name} instrument draws`,
          drawn.drawn > 0,
          `${drawn.drawn} pixels drawn`,
        );
      }
      images.push(
        imageOf(shot.name, SIZES.main, main),
        imageOf(`${shot.name}-orbit`, SIZES.orbit, orbit),
        imageOf(`${shot.name}-craft`, SIZES.craft, craft),
      );
    } finally {
      run?.dispose();
      for (const canvas of Object.values(canvases)) {
        canvas.remove();
      }
    }
  }
  return images;
}
