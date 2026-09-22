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
 * The variables name the *server*, not this process, so they are not the server's own
 * `HYPERION_ADDR` and `HYPERION_PORT`: an address to listen on and an address to connect to are
 * not the same thing.
 */

import { Command, InvalidArgumentError, Option } from "commander";

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
function parseAddress(value: string): string {
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
function parsePort(value: string): number {
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
  const options = buildCommand(version)
    .parse([...args], { from: "user" })
    .opts();
  // Both options have a default and a parser, so commander cannot yield another type here.
  const address: unknown = options["address"];
  const port: unknown = options["port"];
  if (typeof address !== "string" || typeof port !== "number") {
    throw new Error("the command line yielded no address and port");
  }
  return { address, port };
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
