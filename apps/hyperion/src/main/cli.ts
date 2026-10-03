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
 * `--descent-spike` opens the descent spike in place of the consoles (plan R05, T13.c), with its
 * own options: `--setting high|low`, `--seed <u64>`, `--smoke`, `--out <dir>`, `--workers <n>`,
 * `--vertex-path baked-offsets|face-differences`, `--normals double|mesh`, `--ridged on|off`,
 * `--dawn-safety on|off` and `--capture <dir>`. Each is refused without the flag.
 *
 * The variables name the *server*, not this process, so they are not the server's own
 * `HYPERION_ADDR` and `HYPERION_PORT`: an address to listen on and an address to connect to are
 * not the same thing.
 */

import { Command, InvalidArgumentError, Option } from "commander";

import { DEFAULT_SPIKE_SEED, isU64Decimal, type SpikeLaunch } from "../preload/spikeLaunch";

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
  /** The descent spike's options when `--descent-spike` is given, else `undefined`. */
  readonly spike?: SpikeLaunch;
}

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

/** Reads a directory argument: any non-empty path. */
function parseDirectory(value: string): string {
  if (value.length === 0) {
    throw new InvalidArgumentError("expected a directory");
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
      new Option("--setting <SETTING>", "the spike's quality setting").choices(["high", "low"]),
    )
    .addOption(new Option("--seed <U64>", "the spike's seed").argParser(parseSeed))
    .addOption(new Option("--smoke", "a 10 s spike run that exits with a status"))
    .addOption(
      new Option("--out <DIR>", "where the spike's results file goes").argParser(parseDirectory),
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
  if (options["descentSpike"] !== true) {
    const stray = SPIKE_OPTIONS.find((name) => options[name] !== undefined);
    if (stray !== undefined) {
      // Throws a CommanderError, as the parser's own refusals do (`exitOverride`).
      command.error(
        `error: --${kebab(stray)} is an option of --descent-spike, which was not given`,
      );
    }
    return { address, port };
  }
  const spike = spikeLaunchOf(options);
  if (spike.setting === "low" && spike.vertexPath === "baked-offsets") {
    // The low setting's cache does not fit `BakedOffsets` (R05 Design note 4).
    command.error("error: --vertex-path baked-offsets does not fit the low setting's cache");
  }
  return { address, port, spike };
}

/** A commander attribute name as its option: `vertexPath` is `vertex-path`. */
function kebab(name: string): string {
  return name.replaceAll(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

/** The spike's options, each parsed and checked by commander, with their defaults. */
function spikeLaunchOf(options: Readonly<Record<string, unknown>>): SpikeLaunch {
  const text = (name: string): string | null => {
    const value = options[name];
    return typeof value === "string" ? value : null;
  };
  const workers = options["workers"];
  const setting = text("setting");
  const vertexPath = text("vertexPath");
  const normals = text("normals");
  return {
    setting: setting === "low" ? "low" : "high",
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
export function serverUrlOf({ address, port }: ClientArgs): string {
  const url = new URL(`ws://${address}`);
  url.port = String(port);
  url.pathname = WS_PATH;
  return url.href;
}
