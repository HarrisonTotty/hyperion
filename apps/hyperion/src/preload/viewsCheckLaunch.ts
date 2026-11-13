/**
 * How the main process hands the several-views check's options (plan R07, T20) to the preload.
 *
 * @remarks
 * As `spikeLaunch.ts` does for the descent spike's: the options ride in the renderer's own `argv`
 * as one extra switch, `--hyperion-views-check=<options>`, present only when the client was
 * launched with `--views-check`, and the preload gives `HyperionApi` its `viewsCheck` member only
 * then. Both sides of the hand-off use this module; it imports types and `spikeLaunch.ts`' ID
 * reader only, which imports types alone, so it is safe in either bundle.
 */

import type {
  AtmosphereLeversOverride,
  CompositeProbeName,
  HandoverLookName,
  LitProbeName,
  MarchProbeName,
  ViewsCheckLaunch,
  ViewsCheckRunKind,
  ViewsCheckTerrainFixture,
} from "./api";
import { readServedWorldNames } from "./spikeLaunch";

export type {
  AtmosphereLeversOverride,
  CompositeProbeName,
  LitProbeName,
  MarchProbeName,
  ViewsCheckLaunch,
  ViewsCheckRunKind,
  ViewsCheckTerrainFixture,
} from "./api";

/** The most draws `--randomize` may ask a planet viewer run for (`MAX_RANDOMIZE_DRAWS`). */
export const MAX_RANDOMIZE_DRAWS = 32;

/** The switch carrying the options. */
const VIEWS_CHECK_SWITCH = "--hyperion-views-check=";

/** The extra renderer argument that carries `launch`, for `webPreferences.additionalArguments`. */
export function viewsCheckSwitch(launch: ViewsCheckLaunch): string {
  return `${VIEWS_CHECK_SWITCH}${encodeURIComponent(JSON.stringify(launch))}`;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The runs a launch may make. */
const RUN_KINDS: ReadonlyArray<ViewsCheckRunKind> = [
  "several-views",
  "terrain",
  "handover",
  "atmosphere",
  "inspection",
  "planet-viewer",
];

/** The fields a terrain run may be served. */
const TERRAIN_FIXTURES: ReadonlyArray<ViewsCheckTerrainFixture> = [
  "terrain-test",
  "terrain-test-weathered",
  "terrain-test-whole",
];

/** The composite's probes an inspection profile may draw under. */
const COMPOSITE_PROBES: ReadonlyArray<CompositeProbeName> = [
  "sky-off",
  "transmittance-off",
  "per-pixel",
  "surface-grey",
];

/** The march's probes an inspection profile may draw under (plan R08, R08.T8.d and R08.T8.e). */
export const MARCH_PROBE_NAMES = [
  "centre",
  "depth-zero",
  "one-source",
  "no-lobes",
] as const satisfies ReadonlyArray<MarchProbeName>;

/**
 * `value` as the atmosphere's levers over a setting's (plan R08, R08.T8.d, R08.T8.e's table and
 * fix-r08-decked-air-route's surface lobes and decked ground), or `null` if it is not an object of
 * at least one of them, each one of its names.
 */
export function readAtmosphereLevers(value: unknown): AtmosphereLeversOverride | null {
  if (!isRecord(value)) {
    return null;
  }
  const {
    marchCover,
    limbRays,
    marchSourceTransmittance: sources,
    surfaceLobes: lobes,
    deckedAirSurface: ground,
    ...rest
  } = value;
  if (
    Object.keys(rest).length > 0 ||
    (marchCover === undefined &&
      limbRays === undefined &&
      sources === undefined &&
      lobes === undefined &&
      ground === undefined) ||
    !(marchCover === undefined || marchCover === "area" || marchCover === "tiles") ||
    !(limbRays === undefined || limbRays === "per-texel" || limbRays === "packed") ||
    !(sources === undefined || sources === "table" || sources === "per-step") ||
    !(lobes === undefined || lobes === "transmitted" || lobes === "extinguished") ||
    !(ground === undefined || ground === "departure" || ground === "uniform")
  ) {
    return null;
  }
  return {
    ...(marchCover === undefined ? {} : { marchCover }),
    ...(limbRays === undefined ? {} : { limbRays }),
    ...(sources === undefined ? {} : { marchSourceTransmittance: sources }),
    ...(lobes === undefined ? {} : { surfaceLobes: lobes }),
    ...(ground === undefined ? {} : { deckedAirSurface: ground }),
  };
}

/** The lit passes' probes an inspection profile may draw under (R08.T9.b's measurement). */
export const LIT_PROBE_NAMES = [
  "transmittance-one",
  "planetshine-centre",
  "no-annuli",
  "per-fragment",
  "terrain-unshaded",
  "terrain-no-cascades",
  "terrain-no-horizon",
  "terrain-no-maturity",
] as const satisfies ReadonlyArray<LitProbeName>;

/** The hand-over check's looks, in the order a run takes them, which `--look` may name one of. */
export const HANDOVER_LOOK_NAMES = [
  "handover-30",
  "handover-90",
  "handover-terminator",
] as const satisfies ReadonlyArray<HandoverLookName>;

/**
 * Every look `--look` may name: the run's ({@link HANDOVER_LOOK_NAMES}), then the halo's
 * (R10.T1.c's close-range halo frame), which no run takes unnamed.
 */
export const NAMED_HANDOVER_LOOK_NAMES = [
  ...HANDOVER_LOOK_NAMES,
  "handover-halo",
] as const satisfies ReadonlyArray<HandoverLookName>;

/**
 * `value` as the check's options, or `null` if it is not one: {@link readLaunchBase}'s, with the one
 * look (`look`) a hand-over check alone may name (R10.T15.b.2's runs), the GPU time by pass
 * (`passTimes`, `true`) a terrain run alone may record (R14's Mac measurements), the count of
 * WebGPU calls (`countGpuCalls`, `true`) an inspection profile alone may make (plan R14), and the
 * lit passes' probe (`litProbe`, R08.T9.b's measurement) an inspection's frame profile alone may
 * draw under, and the `RANDOMIZE` draws (`randomize`, 1 to {@link MAX_RANDOMIZE_DRAWS}) a planet
 * viewer run alone may capture.
 */
export function readViewsCheckLaunch(value: unknown): ViewsCheckLaunch | null {
  const base = readLaunchBase(value);
  if (base === null || !isRecord(value)) {
    return base;
  }
  const { passTimes, countGpuCalls } = value;
  if (passTimes !== undefined && (passTimes !== true || base.run !== "terrain")) {
    return null;
  }
  if (countGpuCalls !== undefined && (countGpuCalls !== true || base.timing !== true)) {
    return null;
  }
  const litProbe = LIT_PROBE_NAMES.find((each) => each === value["litProbe"]);
  if (value["litProbe"] !== undefined && (litProbe === undefined || base.timing !== true)) {
    return null;
  }
  const counted: ViewsCheckLaunch = countGpuCalls === true ? { ...base, countGpuCalls } : base;
  const probed: ViewsCheckLaunch = litProbe === undefined ? counted : { ...counted, litProbe };
  const timed: ViewsCheckLaunch = passTimes === true ? { ...probed, passTimes } : probed;
  const { randomize } = value;
  if (randomize !== undefined) {
    return base.run === "planet-viewer" &&
      value["look"] === undefined &&
      typeof randomize === "number" &&
      Number.isInteger(randomize) &&
      randomize >= 1 &&
      randomize <= MAX_RANDOMIZE_DRAWS
      ? { ...timed, randomize }
      : null;
  }
  if (value["look"] === undefined) {
    return timed;
  }
  const look = NAMED_HANDOVER_LOOK_NAMES.find((each) => each === value["look"]);
  return look !== undefined && base.run === "handover" ? { ...timed, look } : null;
}

/**
 * `value` as the check's options but its look, or `null` if it is not one: every field present and of its type,
 * and the weathered fixture on a terrain run or its hand-over check alone (T20's run and the
 * atmosphere run, plan R08, R08.T10.b, serve no field, so their launches name `TERRAIN TEST`'s, the
 * default); a served world (plan R10, R10.T15.b), its universe's and body's IDs, only on a
 * hand-over check of `TERRAIN TEST`'s own fixture, and always on the inspection run (the owner's
 * request, 2026-11-01), which serves no field either; the whole body's field (plan R10,
 * R10.T14.c), its file's path, on a terrain run alone and exactly when its fixture is the whole
 * body's; the frame profile (`timing`, `true`) on the inspection run alone (the owner's report,
 * 2026-11-01); and the composite's probe (`compositeProbe`, the owner's priority, 2026-11-01) and
 * the march's (`marchProbe`, plan R08, R08.T8.d) on that profile alone.
 */
function readLaunchBase(value: unknown): ViewsCheckLaunch | null {
  if (!isRecord(value)) {
    return null;
  }
  const { setting, smoke, out, served, wholeField, timing, compositeProbe, marchProbe } = value;
  const leversGiven = value["atmosphereLevers"];
  const levers = leversGiven === undefined ? null : readAtmosphereLevers(leversGiven);
  const run = RUN_KINDS.find((each) => each === value["run"]) ?? null;
  const fixture = TERRAIN_FIXTURES.find((each) => each === value["fixture"]) ?? null;
  const probe = COMPOSITE_PROBES.find((each) => each === compositeProbe) ?? null;
  const march = MARCH_PROBE_NAMES.find((each) => each === marchProbe) ?? null;
  if (
    (compositeProbe !== undefined && probe === null) ||
    (marchProbe !== undefined && march === null) ||
    (leversGiven !== undefined && levers === null) ||
    (setting !== "high" && setting !== "low") ||
    typeof smoke !== "boolean" ||
    !(out === null || (typeof out === "string" && out.length > 0)) ||
    run === null ||
    fixture === null ||
    ((run === "several-views" ||
      run === "atmosphere" ||
      run === "inspection" ||
      run === "planet-viewer") &&
      fixture !== "terrain-test")
  ) {
    return null;
  }
  if (fixture === "terrain-test-whole" || wholeField !== undefined) {
    return fixture === "terrain-test-whole" &&
      run === "terrain" &&
      served === undefined &&
      timing === undefined &&
      probe === null &&
      march === null &&
      levers === null &&
      typeof wholeField === "string" &&
      wholeField.length > 0
      ? { setting, smoke, out, run, fixture, wholeField }
      : null;
  }
  if (served === undefined) {
    return run === "inspection" ||
      timing !== undefined ||
      probe !== null ||
      march !== null ||
      levers !== null
      ? null
      : { setting, smoke, out, run, fixture };
  }
  const names = readServedWorldNames(served);
  if (
    names === null ||
    (run !== "handover" && run !== "inspection") ||
    fixture !== "terrain-test"
  ) {
    return null;
  }
  if (timing === undefined) {
    return probe === null && march === null && levers === null
      ? { setting, smoke, out, run, fixture, served: names }
      : null;
  }
  if (timing !== true || run !== "inspection") {
    return null;
  }
  return {
    setting,
    smoke,
    out,
    run,
    fixture,
    served: names,
    timing,
    ...(probe === null ? {} : { compositeProbe: probe }),
    ...(march === null ? {} : { marchProbe: march }),
    ...(levers === null ? {} : { atmosphereLevers: levers }),
  };
}

/**
 * The check's options carried by `argv`, the renderer's arguments, or `null` on any other launch.
 *
 * @throws Error if the switch is there but does not hold valid options, which means the window
 * was created with something other than {@link viewsCheckSwitch}'s output.
 */
export function viewsCheckLaunchFromArgv(argv: readonly string[]): ViewsCheckLaunch | null {
  const carrier = argv.find((argument) => argument.startsWith(VIEWS_CHECK_SWITCH));
  if (carrier === undefined) {
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(decodeURIComponent(carrier.slice(VIEWS_CHECK_SWITCH.length)));
  } catch (error: unknown) {
    throw new Error(`the renderer's ${VIEWS_CHECK_SWITCH} is not JSON`, { cause: error });
  }
  const launch = readViewsCheckLaunch(parsed);
  if (launch === null) {
    throw new Error(`the renderer's ${VIEWS_CHECK_SWITCH} does not hold the check's options`);
  }
  return launch;
}
