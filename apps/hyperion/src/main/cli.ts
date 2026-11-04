/**
 * The client's command line: which hyperion-server the bridge links to.
 *
 * @remarks
 * Every option can instead be set by an environment variable; one given on the command line wins.
 *
 * | Option      | Variable               | Default     |
 * | ----------- | ---------------------- | ----------- |
 * | `--address` | `HYPERION_SERVER_ADDR` | `127.0.0.1` |
 * | `--port`    | `HYPERION_SERVER_PORT` | `7878`      |
 *
 * `--setting high|low` is the quality setting `VIEW` draws at (plan R07, T17; R05 Design note 26),
 * `high` unless given: the operator chooses it at launch, it holds for the run, and launching again
 * changes it. The descent spike and the several-views check take it as their own setting.
 *
 * `--descent-spike` opens the descent spike in place of the consoles (plan R05, T13.c), with its
 * `--setting` and its own options: `--seed <u64>`, `--smoke`, `--out <dir>`, `--workers <n>`,
 * `--vertex-path baked-offsets|face-differences`, `--normals double|mesh`, `--ridged on|off`,
 * `--dawn-safety on|off`, `--capture <dir>` and `--trace-profile on|off` (T14.e: V8's CPU profiler
 * in the trace, off by default; a profiled run is a diagnostic, never judged). Each of its own is
 * refused without the flag.
 *
 * `--world test-planet|terrain-test|served` is the world it flies (plan R10, R10.T14.c): R05's test
 * planet by default, or `TERRAIN TEST`'s moon, whose field the fixture serves through `VIEW`'s
 * terrain session, or a generated body the server at `--address` and `--port` serves (`served`,
 * R10.T15.b), named by `--universe <ID>` and `--body <ID>`, which `--world served` needs and no
 * other world takes. `--style photorealistic|wireframe` is the style a generated world's main view
 * draws in through `VIEW`'s frame drawer, photorealistic by default; a test-planet run's launch
 * records no style (`SpikeLaunch.style` `null`). Three pairings are refused: `--style`
 * with the test planet, whose main view is the spike's own lit view (`SPIKE LIT`); `--ridged on`
 * with a generated world, since the ridges are the test planet's; and `--vertex-path` or
 * `--normals` with a generated world, whose session's passes take the setting's own terrain, until
 * a later task gives them the variants.
 *
 * `--sizes on|off` measures the terrain caches' sizes (plan R10, R10.T14.b;
 * decision-r10-t14b-measure), off by default: over the terrain world, a second view, each pass
 * counted, the height workers' bakes timed and `VIEW`'s readout asked at 4 Hz, the results file
 * gaining a `sizes` block. `--sizes on` is refused with the test planet, as `--style` is: the
 * measurement is the terrain session's. Off, the launch holds no `sizes` at all, so that the run's
 * options and its results file are as they were before the option.
 *
 * `--horizon on|off` bakes the horizon map with the lit style's patches, on by default (plan R10,
 * R10.T15.b): `--horizon off` skips it, the A/B whose run beside an ordinary one gives the map's
 * share of the height workers' bake time, since the module bakes the map in the patch's one call.
 * It is refused with the test planet, whose bakes make no horizon map, and with `--style wireframe`,
 * whose bakes skip it already. On, the launch holds no `horizon`, as `--sizes off` holds no `sizes`.
 *
 * `--geometry <field>=<value>[,…]` overrides how the terrain's geometry is drawn over the setting's
 * (plan R14, Design note 17; `preload/geometryLaunch.ts`), on a `--descent-spike` or
 * `--views-check` launch alone, so that the owner's before and after runs come from one build:
 * `clusterPath=false` draws the way back, as before plan R14.
 *
 * `--compile-budget <n|unlimited>` sets how many pipeline compiles the renderer's engine runs at
 * once (plan R14's Mac measurements; `preload/compileBudgetLaunch.ts`), 1 to 64, on a
 * `--descent-spike` or `--views-check` launch alone: `unlimited` asks each at once, as before the
 * budget, the A/B's way back. Without it the engine takes its default.
 *
 * `--views-check` runs the several-views check in the consoles' `VIEW` (plan R07, T20), with the
 * spike's `--setting`, `--smoke` and `--out`; the spike's other options are refused with it, and
 * so is `--descent-spike`. With `--terrain` (plan R10, R10.T16.d; decision-r10-t16-contracts §6)
 * it runs the terrain run on `TERRAIN TEST` instead, at its `--setting`; `--terrain` is refused
 * without `--views-check`. With `--handover` too (plan R10, R10.T10.g; decision-r10-t16-leans §1)
 * it runs the terrain run's check-only hand-over variant in its place: three looks at the survey
 * of `TERRAIN TEST`'s moon across its hand-over, at 30° and 90° of phase and near the terminator,
 * each frame either side of each switch read back and judged per cell; `--handover` is refused
 * without `--terrain`. With `--weathered` (plan R10, R10.T1.c's client half;
 * decision-r10-t1c-client-scope §3) the terrain run, or its hand-over check, is served `TEST MOON`
 * weathered's field (`terrain_test_weathered.golden`: the moon on a 1 Gyr surface, its young
 * craters ringed by haloes of fresh regolith) in `TERRAIN TEST`'s place (`ViewsCheckLaunch.fixture`),
 * and its results are named `…-terrain-weathered` or `…-handover-weathered`; `--weathered` is
 * refused without `--terrain`, as `--handover` is. With `--whole <FILE>` (plan R10, R10.T14.c's
 * whole-body run, the owner's request) the terrain run is served `TEST MOON`'s whole body, every
 * cell surveyed, from the file the native tool writes (`just views-check --terrain --whole` runs
 * it first), which the main process reads (`ViewsCheckLaunch.wholeField`); its results are named
 * `…-terrain-whole`. It is refused without `--terrain`, beside `--weathered`, and with
 * `--handover`, since the whole body has no survey edge to hand over across. With `--universe <ID>` and `--body <ID>` (plan
 * R10, R10.T15.b) the hand-over check's looks are made on that body as the server serves it, in
 * `TERRAIN TEST`'s place: both or neither, on `--terrain --handover` alone, never with
 * `--weathered`. `--look <NAME>` (R10.T15.b.2) runs the hand-over check's one look of that name
 * (`handover-30`, `handover-90` or `handover-terminator`) in place of all three, a launch a look,
 * or `handover-halo`, which no run takes unnamed: a look from straight above a young crater of
 * `TEST MOON` weathered's survey, closing on until it is resolved (R10.T1.c's close-range halo
 * frame); it is refused without `--handover`. With `--atmosphere` instead (plan R08,
 * R08.T10.b) it runs the atmosphere run on `ATMOSPHERE TEST`, at its `--setting`: the primary
 * photorealistic, its atmosphere media awaited, the moonlit night captured from the seat, then a
 * scripted free-camera flight to a few radii of each air fixture, each captured. `--atmosphere` is
 * refused without `--views-check`, and with `--terrain`, a separate run (`--handover` and
 * `--weathered` beside it are refused first, as options of a `--terrain` that was not given). With
 * `--inspection` (the owner's request, 2026-11-01) it runs the inspection run: the universe
 * `--universe` names opened on `GALAXY`, the system of the body `--body` names opened on `SYSTEM`,
 * the body opened in `VIEW` through the orbit map's `VIEW BODY` and its photorealistic inspection
 * captured; it needs both, and is refused without `--views-check` and beside `--terrain` or
 * `--atmosphere`. With `--timing` too (the owner's report, 2026-11-01) the inspection run profiles
 * its frame from orbit, closed in and low over the ground, per pass and per thread, into
 * `target/views-check/timing/`; `--timing` is refused without `--inspection`. With
 * `--composite-probe sky-off|transmittance-off|per-pixel|surface-grey` too (the owner's priority,
 * 2026-11-01) that profile draws the atmosphere's composite under the probe, its sky branch off, its
 * transmittance across from above off, that transmittance per pixel as the way back forms it, or
 * every drawn surface dimmed by the channels' mean as the grey way back does (plan R08, R08.T9.c),
 * for an A/B of the composite's cost; it is refused without `--timing`. With `--count-gpu-calls`
 * too (plan R14, the owner's Mac) that profile counts every WebGPU call each window makes, by kind
 * and by pass, at a little of the main thread's time; it is refused without `--timing`.
 * With `--march-probe centre` too (plan R08, R08.T8.d) that profile marches every texel's centre
 * ray alone, with no far-limb footprint, for an A/B of the footprint's node rays; with
 * `--march-probe depth-zero|one-source|no-lobes` (R08.T8.e's measurement) each step's sources lit
 * through no air with no lookup, the first source alone, or no forward lobes' higher orders, for an
 * A/B of the step's sources; it is refused without `--timing`, and may stand beside
 * `--composite-probe`.
 * `--march-cover area|tiles` and `--limb-rays per-texel|packed` (plan R08, R08.T8.d) draw that
 * profile's atmospheres with that lever over its setting's, for the A/B of each lever on and off
 * on one build; each is refused without `--timing`.
 * With `--lit-probe transmittance-one|planetshine-centre|no-annuli|per-fragment` too (plan R08,
 * R08.T9.b's measurement) that profile makes the lit materials and kernels under the probe, every
 * source's transmittance through the air 1, a planetshine source's at its centre node alone, the
 * eclipse term with no annuli, or each source's transmittance per fragment whatever the setting's
 * sources' table (its way back), for an A/B of the lit fragment's cost; it is refused without
 * `--timing`.
 *
 * `--hyperion-gpu-timing`, R01's measurement switch, which lifts the timestamps' quantization on
 * every backend but safe mode's (read from Chromium's command line, `GPU_TIMING_SWITCH`), also has a
 * `--terrain` run record each phase's GPU time by pass (R14's Mac measurements), on any platform;
 * every other run takes it for its timestamps alone.
 *
 * `--device-pixel-ratio <N>` draws a hidden check's page at a device-pixel ratio of N, 1 to 3, as a
 * HiDPI display does (the owner's Retina report, 2026-11-01): its offscreen window N times its
 * setting's size at a zoom factor of N, which Chromium counts in `devicePixelRatio`, so that its
 * layout is the setting's size in CSS px and its canvases N times it. It is refused without
 * `--views-check`, and the main process refuses it on a shown run, whose display gives its ratio.
 *
 * The variables name the *server*, not this process, so they are not the server's own
 * `HYPERION_ADDR` and `HYPERION_PORT`: an address to listen on and an address to connect to are
 * not the same thing.
 */

import { Command, InvalidArgumentError, Option } from "commander";

import type {
  AtmosphereLeversOverride,
  CompositeProbeName,
  GeometryOverride,
  MarchProbeName,
  HandoverLookName,
  LitProbeName,
  QualitySettingName,
  ServedWorldNames,
} from "../preload/api";
import { type CompileBudgetLaunch, parseCompileBudget } from "../preload/compileBudgetLaunch";
import { geometryText, parseGeometryOverride } from "../preload/geometryLaunch";
import { GPU_TIMING_SWITCH } from "../preload/graphicsLaunch";
import {
  DEFAULT_SPIKE_SEED,
  isBodyIdText,
  isU64Decimal,
  isUniverseId,
  type SpikeLaunch,
} from "../preload/spikeLaunch";
import {
  LIT_PROBE_NAMES,
  MARCH_PROBE_NAMES,
  NAMED_HANDOVER_LOOK_NAMES,
  type ViewsCheckLaunch,
} from "../preload/viewsCheckLaunch";

/** The variable giving the address of the server to link to, for `--address`. */
export const ENV_SERVER_ADDR = "HYPERION_SERVER_ADDR";
/** The variable giving the port of the server to link to, for `--port`. */
export const ENV_SERVER_PORT = "HYPERION_SERVER_PORT";

/** The address linked to when `--address` is not given: a server on this machine. */
export const DEFAULT_ADDRESS = "127.0.0.1";
/** The port linked to when `--port` is not given, the server's own default. */
export const DEFAULT_PORT = 7878;

/** The path the server serves the bridge WebSocket at. */
const WS_PATH = "/ws";

/** Where the client looks for the server. */
export interface ClientArgs {
  /** IP address or host name, as a URL host: an IPv6 literal keeps its brackets. */
  readonly address: string;
  /** Port the server listens on, 1 to 65535. */
  readonly port: number;
  /**
   * The quality setting `VIEW` draws at (`--setting`, R07.T17): on a spike or a check launch, its
   * own setting.
   */
  readonly setting: QualitySettingName;
  /** The descent spike's options when `--descent-spike` is given, else `undefined`. */
  readonly spike?: SpikeLaunch;
  /** The several-views check's options when `--views-check` is given, else `undefined`. */
  readonly viewsCheck?: ViewsCheckLaunch;
  /**
   * The spike's or the check's geometry override (`--geometry`, plan R14, Design note 17), applied
   * over its setting's, when given, else `undefined`.
   */
  readonly geometry?: GeometryOverride;
  /** The spike's or the check's compile budget (`--compile-budget`), when given, else `undefined`. */
  readonly compileBudget?: CompileBudgetLaunch;
  /**
   * The check's page's device-pixel ratio (`--device-pixel-ratio`), the zoom factor its hidden
   * window is drawn at: 1 by default; `undefined` without `--views-check`.
   */
  readonly viewsCheckPixelRatio?: ViewsCheckPixelRatio;
}

/** A views check's device-pixel ratio, by its window's zoom (`--device-pixel-ratio`). */
export type ViewsCheckPixelRatio = 1 | 2 | 3;

/** The spike's options other than the flag itself, by their commander attribute names. */
const SPIKE_OPTIONS = [
  "setting",
  "seed",
  "smoke",
  "out",
  "workers",
  "vertexPath",
  "normals",
  "ridged",
  "dawnSafety",
  "capture",
  "traceProfile",
  "world",
  "style",
  "sizes",
  "horizon",
  "universe",
  "body",
  "geometry",
  "compileBudget",
] as const;

/** The spike's options that `--views-check` takes too. */
const VIEWS_CHECK_OPTIONS = [
  "setting",
  "smoke",
  "out",
  "universe",
  "body",
  "geometry",
  "compileBudget",
] as const;

/** Reads a `--seed` value: a u64 in decimal. */
export function parseSeed(value: string): string {
  if (!isU64Decimal(value)) {
    throw new InvalidArgumentError("expected an unsigned 64-bit integer in decimal");
  }
  return value;
}

/** Reads a `--workers` value: 1 to 64 height workers. */
export function parseWorkers(value: string): number {
  const workers = /^\d{1,2}$/.test(value) ? Number(value) : Number.NaN;
  if (!Number.isInteger(workers) || workers < 1 || workers > 64) {
    throw new InvalidArgumentError("expected a worker count from 1 to 64");
  }
  return workers;
}

/** Reads a `--device-pixel-ratio` value: 1, 2 or 3. */
export function parseDevicePixelRatio(value: string): ViewsCheckPixelRatio {
  if (value === "1" || value === "2" || value === "3") {
    return value === "1" ? 1 : value === "2" ? 2 : 3;
  }
  throw new InvalidArgumentError("expected a device-pixel ratio of 1, 2 or 3");
}

/** Reads a `--universe` value: a universe's ID, 16 lowercase hexadecimal digits. */
export function parseUniverse(value: string): string {
  if (!isUniverseId(value)) {
    throw new InvalidArgumentError("expected a universe's ID, 16 lowercase hexadecimal digits");
  }
  return value;
}

/** Reads a `--body` value: a body's ID, such as `61ffd967fe000023.0100`. */
export function parseBody(value: string): string {
  if (!isBodyIdText(value)) {
    throw new InvalidArgumentError("expected a body's ID, such as 61ffd967fe000023.0100");
  }
  return value;
}

/** Reads a `--compile-budget` value: `unlimited`, or 1 to 64 compiles at once. */
export function parseCompileBudgetOption(value: string): CompileBudgetLaunch {
  try {
    return parseCompileBudget(value);
  } catch (error: unknown) {
    throw new InvalidArgumentError(error instanceof Error ? error.message : String(error));
  }
}

/**
 * Reads a `--geometry` value, `<field>=<value>[,…]` over the setting's geometry (plan R14), in its
 * canonical form.
 */
export function parseGeometry(value: string): string {
  try {
    return geometryText(parseGeometryOverride(value));
  } catch (error: unknown) {
    throw new InvalidArgumentError(error instanceof Error ? error.message : String(error));
  }
}

/** Reads a directory argument: any non-empty path. */
function parseDirectory(value: string): string {
  if (value.length === 0) {
    throw new InvalidArgumentError("expected a directory");
  }
  return value;
}

/** Checks a file argument: any path, which the main process reads when the run asks for it. */
function parseFile(value: string): string {
  if (value.length === 0) {
    throw new InvalidArgumentError("expected a file");
  }
  return value;
}

/**
 * Canonicalises an `--address` value as a URL host.
 *
 * @remarks
 * An IPv6 literal may be written with or without brackets. The value has to be the whole host the
 * URL parser reads out of it, so anything that parser would drop or rewrite — a scheme, a port, a
 * path, credentials, an unusual spelling of an address — is a usage error rather than a link to
 * somewhere unintended.
 *
 * @throws InvalidArgumentError if the value is not a bare IP address or host name.
 */
export function parseAddress(value: string): string {
  const bracketed = value.startsWith("[") || !value.includes(":") ? value : `[${value}]`;
  const literal = bracketed.toLowerCase();
  const asUrl = `ws://${literal}`;
  const url = URL.canParse(asUrl) ? new URL(asUrl) : undefined;
  if (url === undefined || url.hostname !== literal) {
    throw new InvalidArgumentError(
      "expected an IP address or a host name, without a scheme, a port or a path",
    );
  }
  return url.hostname;
}

/**
 * Reads a `--port` value.
 *
 * @throws InvalidArgumentError if it is not a port a server can be reached on. Port 0, which the
 * server accepts to let the OS choose one, is nothing to connect to.
 */
export function parsePort(value: string): number {
  const port = /^\d{1,5}$/.test(value) ? Number(value) : Number.NaN;
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new InvalidArgumentError("expected a port from 1 to 65535");
  }
  return port;
}

/**
 * The client's command line, ready to parse, reporting `version` for `--version`.
 *
 * @remarks
 * Errors, `--help` and `--version` throw a `CommanderError` once their text is written instead of
 * exiting, so that the caller ends the process as Electron wants. Unknown options and stray
 * arguments are allowed because Chromium, Electron and electron-vite all append switches of their
 * own — `--no-sandbox`, `--inspect`, the entry path — to the command line we are handed.
 */
export function buildCommand(version: string): Command {
  return new Command()
    .name("hyperion")
    .description("HYPERION bridge client. Links to a hyperion-server over a WebSocket.")
    .version(version)
    .allowUnknownOption()
    .allowExcessArguments()
    .showHelpAfterError()
    .addOption(
      new Option("-a, --address <HOST>", "IP address or host name of the server")
        .default(DEFAULT_ADDRESS)
        .env(ENV_SERVER_ADDR)
        .argParser(parseAddress),
    )
    .addOption(
      new Option("-p, --port <PORT>", "port the server listens on")
        .default(DEFAULT_PORT)
        .env(ENV_SERVER_PORT)
        .argParser(parsePort),
    )
    .addOption(
      new Option("--descent-spike", "run the descent spike (plan R05) in place of the consoles"),
    )
    .addOption(
      new Option("--views-check", "run the several-views check (plan R07) in the consoles' VIEW"),
    )
    .addOption(
      new Option(
        "--setting <SETTING>",
        "the quality setting VIEW draws at, or the spike's or the check's (default high)",
      ).choices(["high", "low"]),
    )
    .addOption(
      new Option("--terrain", "the check's terrain run on TERRAIN TEST (plan R10), not T20's"),
    )
    .addOption(
      new Option(
        "--handover",
        "the terrain run's hand-over check (plan R10): its photometry across the hand-over, three looks",
      ),
    )
    .addOption(
      new Option(
        "--weathered",
        "the terrain run, or its hand-over check, on TEST MOON weathered (plan R10): haloed young craters",
      ),
    )
    .addOption(
      new Option(
        "--whole <FILE>",
        "the terrain run on TEST MOON's whole body (plan R10): every cell surveyed, from the native tool's file",
      ).argParser(parseFile),
    )
    .addOption(
      new Option(
        "--atmosphere",
        "the check's atmosphere run on ATMOSPHERE TEST (plan R08): the moonlit night, then a hand flight past each air fixture",
      ),
    )
    .addOption(
      new Option(
        "--inspection",
        "the check's inspection run (the owner's request): --body opened from SYSTEM through VIEW BODY, captured",
      ),
    )
    .addOption(
      new Option(
        "--look <NAME>",
        "the hand-over check's one look, in place of all three (R10.T15.b.2): a launch a look",
      ).choices(NAMED_HANDOVER_LOOK_NAMES),
    )
    .addOption(
      new Option(
        "--timing",
        "the inspection run's frame profile (the owner's report): per pass and per thread, from orbit, close in and low",
      ),
    )
    .addOption(
      new Option(
        `--${GPU_TIMING_SWITCH}`,
        "measurement: unquantized timestamps, and the terrain run's GPU time by pass",
      ),
    )
    .addOption(
      new Option(
        "--device-pixel-ratio <N>",
        "the check's hidden window at a device-pixel ratio of N, 1 to 3, by its zoom (default 1)",
      ).argParser(parseDevicePixelRatio),
    )
    .addOption(
      new Option(
        "--composite-probe <PROBE>",
        "the frame profile's composite probe (the owner's priority): its sky branch, or its transmittance from above, off, that per pixel, or its surfaces grey",
      ).choices(["sky-off", "transmittance-off", "per-pixel", "surface-grey"]),
    )
    .addOption(
      new Option(
        "--count-gpu-calls",
        "the frame profile's count of every WebGPU call, by kind and by pass (plan R14)",
      ),
    )
    .addOption(
      new Option(
        "--march-cover <COVER>",
        "the frame profile's march cover over its setting's (R08.T8.d): the area, or the tile cull",
      ).choices(["area", "tiles"]),
    )
    .addOption(
      new Option(
        "--limb-rays <RAYS>",
        "the frame profile's far-limb node rays over its setting's (R08.T8.d): per texel, or packed",
      ).choices(["per-texel", "packed"]),
    )
    .addOption(
      new Option(
        "--march-probe <PROBE>",
        "the frame profile's march probe (R08.T8.d, R08.T8.e): every texel's centre ray alone, each step's sources through no air, the first source alone, or no forward lobes' higher orders",
      ).choices(MARCH_PROBE_NAMES),
    )
    .addOption(
      new Option(
        "--lit-probe <PROBE>",
        "the frame profile's lit-pass probe (R08.T9.b): every source's transmittance 1, planetshine at its centre node, no eclipse annuli, or the sources' transmittance per fragment",
      ).choices(LIT_PROBE_NAMES),
    )
    .addOption(new Option("--seed <U64>", "the spike's seed").argParser(parseSeed))
    .addOption(new Option("--smoke", "a short hidden spike or check run that exits with a status"))
    .addOption(
      new Option("--out <DIR>", "where the spike's or the check's results file goes").argParser(
        parseDirectory,
      ),
    )
    .addOption(
      new Option("--workers <N>", "the spike's height-worker count").argParser(parseWorkers),
    )
    .addOption(
      new Option("--vertex-path <PATH>", "the terrain's vertex path").choices([
        "baked-offsets",
        "face-differences",
      ]),
    )
    .addOption(new Option("--normals <RES>", "the terrain's normals").choices(["double", "mesh"]))
    .addOption(new Option("--ridged <ON>", "the test planet's ridges").choices(["on", "off"]))
    .addOption(new Option("--dawn-safety <ON>", "Dawn's safety checks").choices(["on", "off"]))
    .addOption(
      new Option("--capture <DIR>", "capture the GPU calls of a span").argParser(parseDirectory),
    )
    .addOption(
      new Option("--trace-profile <ON>", "V8's CPU profiler in the spike's trace").choices([
        "on",
        "off",
      ]),
    )
    .addOption(
      new Option("--world <WORLD>", "the world the spike flies (default test-planet)").choices([
        "test-planet",
        "terrain-test",
        "served",
      ]),
    )
    .addOption(
      new Option(
        "--style <STYLE>",
        "the style a generated world's main view draws in (default photorealistic)",
      ).choices(["photorealistic", "wireframe"]),
    )
    .addOption(
      new Option(
        "--universe <ID>",
        "the served world's universe (--world served, or the hand-over check's)",
      ).argParser(parseUniverse),
    )
    .addOption(
      new Option(
        "--body <ID>",
        "the served world's body (--world served, or the hand-over check's)",
      ).argParser(parseBody),
    )
    .addOption(
      new Option("--sizes <ON>", "measure the terrain world's cache sizes (default off)").choices([
        "on",
        "off",
      ]),
    )
    .addOption(
      new Option(
        "--geometry <OVERRIDES>",
        "the spike's or the check's geometry over its setting's (plan R14): <field>=<value>[,…]",
      ).argParser(parseGeometry),
    )
    .addOption(
      new Option(
        "--compile-budget <N>",
        "the spike's or the check's pipeline compiles at once (plan R14): 1 to 64, or unlimited",
      ).argParser(parseCompileBudgetOption),
    )
    .addOption(
      new Option(
        "--horizon <ON>",
        "bake the horizon map with a generated world's lit patches (default on)",
      ).choices(["on", "off"]),
    )
    .exitOverride();
}

/**
 * Parses `args`, the arguments the user gave, falling back to the environment and then to the
 * defaults.
 *
 * @param version - Reported by `--version`.
 * @throws CommanderError when an argument cannot be used, or when `--help` or `--version` was
 * asked for and has been written; its `exitCode` is the code to end the process with.
 */
export function parseClientArgs(args: readonly string[], version: string): ClientArgs {
  const command = buildCommand(version).parse([...args], { from: "user" });
  const options = command.opts();
  // Both options have a default and a parser, so commander cannot yield another type here.
  const address: unknown = options["address"];
  const port: unknown = options["port"];
  if (typeof address !== "string" || typeof port !== "number") {
    throw new Error("the command line yielded no address and port");
  }
  if (options["handover"] === true && options["terrain"] !== true) {
    command.error("error: --handover is an option of --terrain, which was not given");
  }
  if (options["weathered"] === true && options["terrain"] !== true) {
    command.error("error: --weathered is an option of --terrain, which was not given");
  }
  if (options["whole"] !== undefined && options["terrain"] !== true) {
    command.error("error: --whole is an option of --terrain, which was not given");
  }
  if (options["whole"] !== undefined && options["weathered"] === true) {
    command.error("error: --whole and --weathered are separate fields; give one");
  }
  if (options["whole"] !== undefined && options["handover"] === true) {
    command.error(
      "error: --whole is an option of the terrain run, not its hand-over check: the whole body has no survey edge to hand over across",
    );
  }
  if (options["atmosphere"] === true && options["terrain"] === true) {
    command.error("error: --atmosphere and --terrain are separate runs; give one");
  }
  if (options["terrain"] === true && options["viewsCheck"] !== true) {
    command.error("error: --terrain is an option of --views-check, which was not given");
  }
  if (options["atmosphere"] === true && options["viewsCheck"] !== true) {
    command.error("error: --atmosphere is an option of --views-check, which was not given");
  }
  if (
    options["inspection"] === true &&
    (options["terrain"] === true || options["atmosphere"] === true)
  ) {
    command.error("error: --inspection is a run of its own; give it alone");
  }
  if (options["inspection"] === true && options["viewsCheck"] !== true) {
    command.error("error: --inspection is an option of --views-check, which was not given");
  }
  if (options["timing"] === true && options["inspection"] !== true) {
    command.error("error: --timing is an option of --inspection, which was not given");
  }
  const ratio: unknown = options["devicePixelRatio"];
  if (ratio !== undefined && options["viewsCheck"] !== true) {
    command.error("error: --device-pixel-ratio is an option of --views-check, which was not given");
  }
  if (options["compositeProbe"] !== undefined && options["timing"] !== true) {
    command.error("error: --composite-probe is an option of --timing, which was not given");
  }
  if (options["countGpuCalls"] === true && options["timing"] !== true) {
    command.error("error: --count-gpu-calls is an option of --timing, which was not given");
  }
  if (options["marchProbe"] !== undefined && options["timing"] !== true) {
    command.error("error: --march-probe is an option of --timing, which was not given");
  }
  for (const [option, flag] of [
    ["marchCover", "--march-cover"],
    ["limbRays", "--limb-rays"],
  ] as const) {
    if (options[option] !== undefined && options["timing"] !== true) {
      command.error(`error: ${flag} is an option of --timing, which was not given`);
    }
  }
  if (options["litProbe"] !== undefined && options["timing"] !== true) {
    command.error("error: --lit-probe is an option of --timing, which was not given");
  }
  if (options["look"] !== undefined && options["handover"] !== true) {
    command.error("error: --look is an option of --handover, which was not given");
  }
  if (options["look"] === "handover-halo" && options["universe"] !== undefined) {
    command.error(
      "error: --look handover-halo centres a crater of TERRAIN TEST's moon, not a served world's",
    );
  }
  if (options["viewsCheck"] === true) {
    // Throws a CommanderError, as the parser's own refusals do (`exitOverride`).
    if (options["descentSpike"] === true) {
      command.error("error: --views-check and --descent-spike are separate runs; give one");
    }
    const stray = SPIKE_OPTIONS.find(
      (name) => options[name] !== undefined && !VIEWS_CHECK_OPTIONS.some((each) => each === name),
    );
    if (stray !== undefined) {
      command.error(`error: --${kebab(stray)} is an option of --descent-spike, not --views-check`);
    }
    const viewsCheck = viewsCheckLaunchOf(options);
    const servedRefusal = viewsCheckServedRefusal(options, viewsCheck);
    if (servedRefusal !== null) {
      command.error(`error: ${servedRefusal}`);
    }
    return {
      address,
      port,
      setting: viewsCheck.setting,
      viewsCheck,
      // Parsed by `parseDevicePixelRatio`, so 1, 2 or 3 where given.
      viewsCheckPixelRatio: ratio === 2 || ratio === 3 ? ratio : 1,
      ...geometryOf(options),
      ...compileBudgetOf(options),
    };
  }
  if (options["descentSpike"] !== true) {
    // The setting is the consoles' too (R07.T17); the spike's other options are its own.
    const stray = SPIKE_OPTIONS.find((name) => name !== "setting" && options[name] !== undefined);
    if (stray !== undefined) {
      const owners = VIEWS_CHECK_OPTIONS.some((each) => each === stray)
        ? "--descent-spike or --views-check, neither of which was"
        : "--descent-spike, which was not";
      command.error(`error: --${kebab(stray)} is an option of ${owners} given`);
    }
    return { address, port, setting: settingOf(options) };
  }
  const spike = spikeLaunchOf(options);
  if (spike.setting === "low" && spike.vertexPath === "baked-offsets") {
    // The low setting's cache does not fit `BakedOffsets` (R05 Design note 4).
    command.error("error: --vertex-path baked-offsets does not fit the low setting's cache");
  }
  const refusal = worldRefusal(options, spike);
  if (refusal !== null) {
    command.error(`error: ${refusal}`);
  }
  return {
    address,
    port,
    setting: spike.setting,
    spike,
    ...geometryOf(options),
    ...compileBudgetOf(options),
  };
}

/** The `--compile-budget` given, else nothing (commander has parsed it). */
function compileBudgetOf(
  options: Readonly<Record<string, unknown>>,
): { readonly compileBudget: CompileBudgetLaunch } | Record<string, never> {
  const budget = options["compileBudget"];
  return budget === "unlimited" || typeof budget === "number" ? { compileBudget: budget } : {};
}

/** The `--geometry` override given, else nothing (commander has parsed it). */
function geometryOf(
  options: Readonly<Record<string, unknown>>,
): { readonly geometry: GeometryOverride } | Record<string, never> {
  const geometry = options["geometry"];
  return typeof geometry === "string" ? { geometry: parseGeometryOverride(geometry) } : {};
}

/** The `--setting` given, `high` without one (commander has checked its choices). */
function settingOf(options: Readonly<Record<string, unknown>>): QualitySettingName {
  return options["setting"] === "low" ? "low" : "high";
}

/**
 * Why the spike's options cannot fly together on their world (R10.T14.c, R10.T14.b, R10.T15.b), or
 * `null` where they can: `--style`, `--sizes on` or `--horizon off` with the test planet, `--ridged
 * on` with a generated world, or the terrain variants with it, or `--horizon off` with its
 * wireframe; `--universe` and `--body` with any world but the served one, which needs both.
 */
function worldRefusal(
  options: Readonly<Record<string, unknown>>,
  spike: SpikeLaunch,
): string | null {
  const named = options["universe"] !== undefined || options["body"] !== undefined;
  if (spike.world !== "served" && named) {
    const given = options["universe"] !== undefined ? "--universe" : "--body";
    return `${given} is an option of --world served: it names the served world`;
  }
  let refusal: string | null;
  switch (spike.world) {
    case "test-planet":
      refusal =
        options["style"] !== undefined
          ? "--style is an option of a generated world, --world terrain-test or served: the test planet's main view is the spike's own lit view, SPIKE LIT"
          : spike.sizes === "on"
            ? "--sizes on is an option of a generated world, --world terrain-test or served: the measured sizes are the terrain session's caches"
            : spike.horizon === "off"
              ? "--horizon off is an option of a generated world, --world terrain-test or served: the test planet's bakes make no horizon map"
              : null;
      break;
    case "terrain-test":
    case "served": {
      const variant = spike.vertexPath !== null ? "--vertex-path" : "--normals";
      refusal =
        spike.world === "served" && spike.served === undefined
          ? "--world served needs --universe and --body: the universe and the body the server serves"
          : spike.ridged === "on"
            ? `--ridged on does not fit --world ${spike.world}: the ridges are the test planet's`
            : spike.horizon === "off" && spike.style === "wireframe"
              ? "--horizon off does not fit --style wireframe: the wireframe's bakes skip the horizon map already"
              : spike.vertexPath === null && spike.normals === null
                ? null
                : `${variant} does not fit --world ${spike.world} yet: its terrain session's passes take the setting's own terrain`;
      break;
    }
  }
  return refusal;
}

/**
 * Why a views-check launch's served world cannot be checked (plan R10, R10.T15.b), or `null`: its
 * `--universe` and `--body` both or neither, and both only on `--terrain --handover`, not
 * `--weathered`.
 */
function viewsCheckServedRefusal(
  options: Readonly<Record<string, unknown>>,
  launch: ViewsCheckLaunch,
): string | null {
  const universe = options["universe"] !== undefined;
  const body = options["body"] !== undefined;
  if (launch.run === "inspection") {
    return universe && body ? null : "the inspection run needs --universe and --body";
  }
  if (!universe && !body) {
    return null;
  }
  if (launch.run !== "handover") {
    return "--universe and --body are options of --terrain --handover or --inspection: the served world";
  }
  if (launch.fixture !== "terrain-test") {
    return "--weathered does not fit a served world: the body is the server's, not a fixture";
  }
  return universe && body ? null : "the hand-over check's served world needs --universe and --body";
}

/** A commander attribute name as its option: `vertexPath` is `vertex-path`. */
function kebab(name: string): string {
  return name.replaceAll(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

/** The several-views check's options, each parsed and checked by commander, with their defaults. */
function viewsCheckLaunchOf(options: Readonly<Record<string, unknown>>): ViewsCheckLaunch {
  const out = options["out"];
  const whole = options["whole"];
  // `--handover` and `--weathered` are refused without `--terrain` before this is reached, and
  // `--atmosphere` with it, so an atmosphere launch names `TERRAIN TEST`'s field, the default.
  const run: ViewsCheckLaunch["run"] =
    options["inspection"] === true
      ? "inspection"
      : options["atmosphere"] === true
        ? "atmosphere"
        : options["terrain"] !== true
          ? "several-views"
          : options["handover"] === true
            ? "handover"
            : "terrain";
  return {
    setting: settingOf(options),
    smoke: options["smoke"] === true,
    out: typeof out === "string" ? out : null,
    run,
    fixture:
      options["weathered"] === true
        ? "terrain-test-weathered"
        : typeof whole === "string"
          ? "terrain-test-whole"
          : "terrain-test",
    ...(typeof whole === "string" ? { wholeField: whole } : {}),
    // Both or neither, which `viewsCheckServedRefusal` holds the launch to.
    ...servedOf(options),
    // Refused without `--inspection` before this is reached.
    ...(options["timing"] === true ? { timing: true } : {}),
    // Refused without `--timing` before this is reached; commander has checked its choices.
    ...compositeProbeOf(options),
    // Refused without `--timing` before this is reached.
    ...(options["countGpuCalls"] === true ? { countGpuCalls: true } : {}),
    // Refused without `--timing` before this is reached; commander has checked its choices.
    ...marchProbeOf(options),
    ...atmosphereLeversOf(options),
    ...litProbeOf(options),
    // Refused without `--handover` before this is reached; commander holds it to the three names.
    ...lookOf(options),
    // The terrain run's alone; any other run takes the switch for its timestamps only.
    ...(run === "terrain" && options["hyperionGpuTiming"] === true ? { passTimes: true } : {}),
  };
}

/** The composite's probe `--composite-probe` names where it is given, else nothing. */
function compositeProbeOf(
  options: Readonly<Record<string, unknown>>,
): { readonly compositeProbe: CompositeProbeName } | Record<string, never> {
  const probe = options["compositeProbe"];
  return probe === "sky-off" ||
    probe === "transmittance-off" ||
    probe === "per-pixel" ||
    probe === "surface-grey"
    ? { compositeProbe: probe }
    : {};
}

/** The march's probe `--march-probe` names where it is given, else nothing (plan R08, R08.T8.d). */
function marchProbeOf(
  options: Readonly<Record<string, unknown>>,
): { readonly marchProbe: MarchProbeName } | Record<string, never> {
  const probe = MARCH_PROBE_NAMES.find((each) => each === options["marchProbe"]);
  return probe === undefined ? {} : { marchProbe: probe };
}

/**
 * The atmosphere's levers `--march-cover` and `--limb-rays` name where either is given, else
 * nothing (plan R08, R08.T8.d); commander has checked their choices.
 */
function atmosphereLeversOf(
  options: Readonly<Record<string, unknown>>,
): { readonly atmosphereLevers: AtmosphereLeversOverride } | Record<string, never> {
  const cover = options["marchCover"];
  const rays = options["limbRays"];
  const levers: AtmosphereLeversOverride = {
    ...(cover === "area" || cover === "tiles" ? { marchCover: cover } : {}),
    ...(rays === "per-texel" || rays === "packed" ? { limbRays: rays } : {}),
  };
  return Object.keys(levers).length === 0 ? {} : { atmosphereLevers: levers };
}

/** The lit passes' probe `--lit-probe` names where it is given, else nothing. */
function litProbeOf(
  options: Readonly<Record<string, unknown>>,
): { readonly litProbe: LitProbeName } | Record<string, never> {
  const probe = LIT_PROBE_NAMES.find((each) => each === options["litProbe"]);
  return probe === undefined ? {} : { litProbe: probe };
}

/** The one look `--look` names, where it names one of the hand-over check's, else nothing. */
function lookOf(
  options: Readonly<Record<string, unknown>>,
): { readonly look: HandoverLookName } | Record<string, never> {
  const look = NAMED_HANDOVER_LOOK_NAMES.find((each) => each === options["look"]);
  return look === undefined ? {} : { look };
}

/** The served world `--universe` and `--body` name where both are given, else nothing. */
function servedOf(
  options: Readonly<Record<string, unknown>>,
): { readonly served: ServedWorldNames } | Record<string, never> {
  const universe = options["universe"];
  const body = options["body"];
  return typeof universe === "string" && typeof body === "string"
    ? { served: { universe, body } }
    : {};
}

/** The spike's options, each parsed and checked by commander, with their defaults. */
function spikeLaunchOf(options: Readonly<Record<string, unknown>>): SpikeLaunch {
  const text = (name: string): string | null => {
    const value = options[name];
    return typeof value === "string" ? value : null;
  };
  const workers = options["workers"];
  const vertexPath = text("vertexPath");
  const normals = text("normals");
  const named = text("world");
  const world = named === "terrain-test" || named === "served" ? named : "test-planet";
  return {
    setting: settingOf(options),
    seed: text("seed") ?? DEFAULT_SPIKE_SEED,
    smoke: options["smoke"] === true,
    out: text("out"),
    workers: typeof workers === "number" ? workers : null,
    vertexPath:
      vertexPath === "baked-offsets" || vertexPath === "face-differences" ? vertexPath : null,
    normals: normals === "double" || normals === "mesh" ? normals : null,
    ridged: text("ridged") === "on" ? "on" : "off",
    dawnSafety: text("dawnSafety") === "off" ? "off" : "on",
    capture: text("capture"),
    traceProfile: text("traceProfile") === "on" ? "on" : "off",
    world,
    // The test planet takes no `--style` (`worldRefusal`): its main view is `SPIKE LIT`.
    style:
      world === "test-planet"
        ? null
        : text("style") === "wireframe"
          ? "wireframe"
          : "photorealistic",
    // Absent when off, so that an unmeasured launch's options, and its results file's, are as before.
    ...(text("sizes") === "on" ? { sizes: "on" as const } : {}),
    // Absent when on, the default, so that an ordinary launch's options are as before the option.
    ...(text("horizon") === "off" ? { horizon: "off" as const } : {}),
    // The served world's, which `worldRefusal` holds to `--world served`.
    ...servedOf(options),
  };
}

/**
 * The arguments the user gave, out of a process `argv`.
 *
 * @param packaged - Whether this is a packaged app, which is launched as `hyperion <args>`. In
 * development Electron is launched as `electron <entry> <args>` instead.
 */
export function userArgs(argv: readonly string[], packaged: boolean): readonly string[] {
  return argv.slice(packaged ? 1 : 2);
}

/** The WebSocket URL of the server described by `args`. */
export function serverUrlOf({ address, port }: Pick<ClientArgs, "address" | "port">): string {
  const url = new URL(`ws://${address}`);
  url.port = String(port);
  url.pathname = WS_PATH;
  return url.href;
}
