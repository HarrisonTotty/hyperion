/**
 * The local server of a perf run (plan R12, R12.T4.b; Design note 13), under the launch contract of
 * `decision-cross-platform-server.md` item 3, as plan 04's P04.T17.c built it.
 *
 * @remarks
 * The server is started with `--stop-on-stdin-close`, piped standard streams and an explicit
 * `--data-dir`, a fresh one for each repetition's restart: the default `./hyperion-data` is relative
 * to the working directory, the read-only `/` for a packaged macOS app and the install directory on
 * Windows. It listens on `--port 0`, and the launcher learns the port the system gave it from the
 * server's `listening` line (`tracing`'s `listening addr=<host>:<port>`, with `NO_COLOR=1` so that
 * no colour codes split it), the lean of R12.T4.b: no fixed port can be taken by another process.
 * It is stopped by ending its standard input, waiting up to {@link STOP_WAIT_MS}, then `kill()`:
 * on Unix a SIGTERM, a second stop request that forces exit 1 if the shutdown the input's end began
 * is still running, and otherwise a graceful stop; on Windows a termination. If the client dies the
 * pipe closes on every platform, so the server never outlives it.
 *
 * A `co-resident` server, a station's stand-in, is pinned to one core: on Linux through util-linux's
 * `taskset --cpu-list 0`; elsewhere it runs unpinned, which the runner states in the record's notes.
 */

import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

/** How long a stop waits for the server to exit after its input ends, ms (the contract's 10 s). */
export const STOP_WAIT_MS = 10_000;

/** How long a start waits for the `listening` line, ms: a fresh data directory loads nothing. */
export const LISTEN_WAIT_MS = 60_000;

/** The core a co-resident server is pinned to (Design note 8). */
export const CO_RESIDENT_CORE = 0;

/** Where a server's placement puts it (`@hyperion/perf`'s `PerfSceneServer`). */
export type LocalPlacement = "local" | "co-resident";

/** A server's address, as its `listening` line states it. */
export interface ListeningAddress {
  readonly host: string;
  readonly port: number;
}

/**
 * The address a server's log line states it listens on, or `null` for any other line:
 * `… INFO hyperion_server: listening addr=127.0.0.1:43211`, an IPv6 host in brackets.
 */
export function listeningAddressOf(line: string): ListeningAddress | null {
  const match = /\blistening\b.*\baddr=(\[[0-9a-fA-F:.]+\]|[^\s:]+):(\d{1,5})\b/.exec(line);
  const host = match?.[1];
  const port = Number(match?.[2]);
  if (host === undefined || !Number.isInteger(port) || port < 1 || port > 65_535) {
    return null;
  }
  return { host: host.startsWith("[") ? host.slice(1, -1) : host, port };
}

/** The bridge's WebSocket URL of a server at `address`. */
export function serverUrlOfAddress(address: ListeningAddress): string {
  const host = address.host.includes(":") ? `[${address.host}]` : address.host;
  return `ws://${host}:${String(address.port)}/ws`;
}

/** What a server is started with. */
export interface ServerLaunch {
  /** The `hyperion-server` binary. */
  readonly binary: string;
  readonly numWorkers: number;
  readonly placement: LocalPlacement;
  readonly platform: NodeJS.Platform;
}

/** A command and its arguments. */
export interface Command {
  readonly file: string;
  readonly args: ReadonlyArray<string>;
}

/**
 * The command that starts a server in `dataDir` under the contract: port 0, its workers, the data
 * directory, the stop on its input's end; pinned to one core through `taskset` where co-resident on
 * Linux.
 */
export function serverCommand(launch: ServerLaunch, dataDir: string): Command {
  const args = [
    "--port",
    "0",
    "--num-workers",
    String(launch.numWorkers),
    "--data-dir",
    dataDir,
    "--stop-on-stdin-close",
  ];
  return launch.placement === "co-resident" && launch.platform === "linux"
    ? {
        file: "taskset",
        args: ["--cpu-list", String(CO_RESIDENT_CORE), launch.binary, ...args],
      }
    : { file: launch.binary, args };
}

/** Whether a launch's server runs pinned to one core, as its record's notes state. */
export function pinnedToOneCore(launch: Pick<ServerLaunch, "placement" | "platform">): boolean {
  return launch.placement === "co-resident" && launch.platform === "linux";
}

/** How a server process ended. */
export interface ServerExit {
  readonly code: number | null;
  readonly signal: string | null;
}

/** A started server process, as {@link LocalServer} drives it. */
export interface ServerProcess {
  /** Ends its standard input: the contract's stop request. */
  endInput(): void;
  /** Kills it: SIGTERM on Unix, a termination on Windows. */
  kill(): void;
  /** Calls `listener` with each line of its standard output and error. */
  onLine(listener: (line: string) => void): void;
  /** Calls `listener` once it exits, or at once if it has. */
  onExit(listener: (exit: ServerExit) => void): void;
}

/** Starts a server process: Node's `spawn`, or a test's fake. */
export type SpawnServer = (
  command: Command,
  env: Readonly<Record<string, string>>,
) => ServerProcess;

/** Node's {@link SpawnServer}: piped standard streams, never a shell. */
export const NODE_SPAWN_SERVER: SpawnServer = (command, env) => {
  const child = spawn(command.file, [...command.args], {
    env: { ...process.env, ...env },
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });
  let exit: ServerExit | null = null;
  const exitListeners: Array<(exit: ServerExit) => void> = [];
  const lineListeners: Array<(line: string) => void> = [];
  child.once("exit", (code, signal) => {
    exit = { code, signal };
    for (const listener of exitListeners.splice(0)) {
      listener(exit);
    }
  });
  child.once("error", (error) => {
    exit ??= { code: null, signal: null };
    for (const listener of lineListeners) {
      listener(`perf run: the server could not be started: ${error.message}`);
    }
    for (const listener of exitListeners.splice(0)) {
      listener(exit);
    }
  });
  // A write after the server exits fails with EPIPE; the exit tells the runner instead.
  child.stdin.on("error", () => undefined);
  for (const stream of [child.stdout, child.stderr]) {
    createInterface({ input: stream }).on("line", (line) => {
      for (const listener of lineListeners) {
        listener(line);
      }
    });
  }
  return {
    endInput: () => {
      child.stdin.end();
    },
    kill: () => {
      child.kill();
    },
    onLine: (listener) => {
      lineListeners.push(listener);
    },
    onExit: (listener) => {
      if (exit === null) {
        exitListeners.push(listener);
      } else {
        listener(exit);
      }
    },
  };
};

/**
 * Resolves after `ms`, unless `signal` aborts first, when its timer is cleared and it never
 * resolves: Node's timers, or a test's.
 */
export type Wait = (ms: number, signal: AbortSignal) => Promise<void>;

/** Node's {@link Wait}. */
export const NODE_WAIT: Wait = (ms, signal) =>
  new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
      },
      { once: true },
    );
  });

/** Races `promise` against `wait(ms)`, `null` on the timeout; the losing timer is cleared. */
async function within<T>(promise: Promise<T>, wait: Wait, ms: number): Promise<T | null> {
  const abort = new AbortController();
  try {
    return await Promise.race([promise, wait(ms, abort.signal).then(() => null)]);
  } finally {
    abort.abort();
  }
}

/** What a {@link LocalServer} is given. */
export interface LocalServerOptions {
  readonly launch: ServerLaunch;
  readonly spawn: SpawnServer;
  readonly wait: Wait;
  /** Each line the server writes, for the run's log. */
  readonly log: (line: string) => void;
}

/** The variables a server is started with: no colour codes in its log, and its `info` lines. */
export const SERVER_ENV: Readonly<Record<string, string>> = { NO_COLOR: "1", RUST_LOG: "info" };

/** A server that failed to start or stop as the contract says. */
export class ServerError extends Error {
  override readonly name = "ServerError";
}

/**
 * The run's local server: started in a data directory, stopped and started again in a fresh one
 * before each repetition (Design note 13).
 */
export class LocalServer {
  readonly #options: LocalServerOptions;
  #process: ServerProcess | null = null;
  #exited: Promise<ServerExit> | null = null;
  #starts = 0;

  constructor(options: LocalServerOptions) {
    this.#options = options;
  }

  /** How many times the server was started. */
  get starts(): number {
    return this.#starts;
  }

  /** Whether a started server is still running. */
  get running(): boolean {
    return this.#process !== null;
  }

  /**
   * Starts the server in `dataDir` and resolves its URL once it listens.
   *
   * @throws {@link ServerError} if one is running, or if it exits or stays silent for
   *   {@link LISTEN_WAIT_MS} before its `listening` line; a silent one is stopped first.
   */
  async start(dataDir: string): Promise<string> {
    if (this.#process !== null) {
      throw new ServerError("a server is running: stop it before starting another");
    }
    const { launch, spawn: start, wait, log } = this.#options;
    const child = start(serverCommand(launch, dataDir), SERVER_ENV);
    this.#process = child;
    this.#starts += 1;
    const exited = new Promise<ServerExit>((resolve) => {
      child.onExit(resolve);
    });
    this.#exited = exited;
    const listening = new Promise<ListeningAddress>((resolve) => {
      child.onLine((line) => {
        log(`server: ${line}`);
        const address = listeningAddressOf(line);
        if (address !== null) {
          resolve(address);
        }
      });
    });
    const outcome =
      (await within(
        Promise.race([
          listening.then((address) => ({ kind: "listening" as const, address })),
          exited.then((exit) => ({ kind: "exited" as const, exit })),
        ]),
        wait,
        LISTEN_WAIT_MS,
      )) ?? ({ kind: "silent" } as const);
    if (outcome.kind === "listening") {
      return serverUrlOfAddress(outcome.address);
    }
    if (outcome.kind === "exited") {
      this.#process = null;
      this.#exited = null;
      throw new ServerError(
        `the server exited before listening (code ${String(outcome.exit.code)}, signal ${String(outcome.exit.signal)})`,
      );
    }
    await this.stop();
    throw new ServerError(
      `the server wrote no listening line in ${String(LISTEN_WAIT_MS / 1000)} s`,
    );
  }

  /**
   * Stops the server under the contract: its input ended, up to {@link STOP_WAIT_MS} for it to
   * exit, then `kill()` and as long again; resolves how it ended, or `null` if none was running.
   *
   * @throws {@link ServerError} if it outlives the kill's wait too.
   */
  async stop(): Promise<ServerExit | null> {
    const child = this.#process;
    const exited = this.#exited;
    if (child === null || exited === null) {
      return null;
    }
    const { wait } = this.#options;
    child.endInput();
    let exit = await within(exited, wait, STOP_WAIT_MS);
    if (exit === null) {
      child.kill();
      exit = await within(exited, wait, STOP_WAIT_MS);
    }
    if (exit === null) {
      throw new ServerError(
        `the server outlived its input's end and a kill by ${String((2 * STOP_WAIT_MS) / 1000)} s`,
      );
    }
    this.#process = null;
    this.#exited = null;
    return exit;
  }
}
