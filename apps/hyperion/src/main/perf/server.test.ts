import { describe, expect, it } from "vitest";

import {
  type Command,
  LISTEN_WAIT_MS,
  listeningAddressOf,
  LocalServer,
  type ServerExit,
  ServerError,
  type ServerLaunch,
  type ServerProcess,
  serverCommand,
  serverUrlOfAddress,
  STOP_WAIT_MS,
  type Wait,
} from "./server";

/** `tracing`'s line for the server's start, as `main.rs` writes it with `NO_COLOR=1`. */
const LISTENING =
  "2026-11-17T23:40:01.123456Z  INFO hyperion_server: listening addr=127.0.0.1:43211";

const LAUNCH: ServerLaunch = {
  binary: "/repo/target/release/hyperion-server",
  numWorkers: 2,
  placement: "local",
  platform: "linux",
};

/** A server process a test drives: its lines, its exit and what was done to it. */
class FakeProcess implements ServerProcess {
  readonly #lines: Array<(line: string) => void> = [];
  readonly #exits: Array<(exit: ServerExit) => void> = [];
  #exit: ServerExit | null = null;
  inputEnded = false;
  killed = false;
  /** Whether it exits when its input ends, as the contract's server does. */
  constructor(readonly exitsOnInputEnd: boolean) {}

  endInput(): void {
    this.inputEnded = true;
    if (this.exitsOnInputEnd) {
      this.exit({ code: 0, signal: null });
    }
  }

  kill(): void {
    this.killed = true;
  }

  onLine(listener: (line: string) => void): void {
    this.#lines.push(listener);
  }

  onExit(listener: (exit: ServerExit) => void): void {
    if (this.#exit === null) {
      this.#exits.push(listener);
    } else {
      listener(this.#exit);
    }
  }

  write(line: string): void {
    for (const listener of this.#lines) {
      listener(line);
    }
  }

  exit(exit: ServerExit): void {
    this.#exit = exit;
    for (const listener of this.#exits.splice(0)) {
      listener(exit);
    }
  }
}

/** A clock of waits the test fires by hand, each by its length. */
class FakeWaits {
  readonly #pending: Array<{ readonly ms: number; readonly fire: () => void }> = [];

  readonly wait: Wait = (ms, signal) =>
    new Promise((resolve) => {
      const entry = { ms, fire: () => resolve() };
      this.#pending.push(entry);
      signal.addEventListener("abort", () => {
        const at = this.#pending.indexOf(entry);
        if (at >= 0) {
          this.#pending.splice(at, 1);
        }
      });
    });

  /** The lengths of the waits pending. */
  get pending(): ReadonlyArray<number> {
    return this.#pending.map(({ ms }) => ms);
  }

  /** Fires the first pending wait of `ms`. */
  fire(ms: number): void {
    const at = this.#pending.findIndex((entry) => entry.ms === ms);
    const entry = this.#pending[at];
    if (entry === undefined) {
      throw new Error(`no wait of ${String(ms)} ms is pending`);
    }
    this.#pending.splice(at, 1);
    entry.fire();
  }
}

/** Lets every settled promise's callbacks run: a macrotask runs after every microtask. */
function settle(): Promise<void> {
  return new Promise((resolve) => {
    setImmediate(resolve);
  });
}

/** A server over fake processes, each start's process and command kept. */
function harness(exitsOnInputEnd = true) {
  const processes: FakeProcess[] = [];
  const commands: Command[] = [];
  const envs: Array<Readonly<Record<string, string>>> = [];
  const waits = new FakeWaits();
  const server = new LocalServer({
    launch: LAUNCH,
    spawn: (command, env) => {
      const process = new FakeProcess(exitsOnInputEnd);
      processes.push(process);
      commands.push(command);
      envs.push(env);
      return process;
    },
    wait: waits.wait,
    log: () => undefined,
  });
  return { server, processes, commands, envs, waits };
}

describe("the server's listening line", () => {
  it("gives the port the system chose, from tracing's line", () => {
    expect(listeningAddressOf(LISTENING)).toEqual({ host: "127.0.0.1", port: 43211 });
    expect(listeningAddressOf("… INFO hyperion_server: listening addr=[::1]:7878")).toEqual({
      host: "::1",
      port: 7878,
    });
  });

  it("is no other line", () => {
    expect(
      listeningAddressOf("2026-11-17T23:40:01Z  INFO hyperion_server: shutting down"),
    ).toBeNull();
    expect(listeningAddressOf("listening addr=127.0.0.1:0")).toBeNull();
  });

  it("names the bridge's WebSocket", () => {
    expect(serverUrlOfAddress({ host: "127.0.0.1", port: 43211 })).toBe("ws://127.0.0.1:43211/ws");
    expect(serverUrlOfAddress({ host: "::1", port: 7878 })).toBe("ws://[::1]:7878/ws");
  });
});

describe("the server's command", () => {
  it("starts on port 0 in its data directory, stopping when its input ends", () => {
    expect(serverCommand(LAUNCH, "/data/server-data-1")).toEqual({
      file: LAUNCH.binary,
      args: [
        "--port",
        "0",
        "--num-workers",
        "2",
        "--data-dir",
        "/data/server-data-1",
        "--stop-on-stdin-close",
      ],
    });
  });

  it("pins a co-resident server to one core through taskset on Linux, and nowhere else", () => {
    const station = { ...LAUNCH, numWorkers: 1, placement: "co-resident" as const };
    expect(serverCommand(station, "/d")).toEqual({
      file: "taskset",
      args: [
        "--cpu-list",
        "0",
        LAUNCH.binary,
        "--port",
        "0",
        "--num-workers",
        "1",
        "--data-dir",
        "/d",
        "--stop-on-stdin-close",
      ],
    });
    expect(serverCommand({ ...station, platform: "darwin" }, "/d").file).toBe(LAUNCH.binary);
  });
});

describe("the local server", () => {
  it("resolves the URL its listening line gives, its log uncoloured", async () => {
    const { server, processes, envs } = harness();
    const started = server.start("/data/0");
    await settle();
    processes[0]?.write("2026-11-17T23:40:00Z  INFO hyperion_server: loading");
    processes[0]?.write(LISTENING);
    await expect(started).resolves.toBe("ws://127.0.0.1:43211/ws");
    expect(envs[0]).toMatchObject({ NO_COLOR: "1" });
    expect(server.running).toBe(true);
  });

  it("stops by ending its input, and is started again in a fresh directory", async () => {
    const { server, processes, commands } = harness();
    const first = server.start("/data/0");
    await settle();
    processes[0]?.write(LISTENING);
    await first;
    await expect(server.stop()).resolves.toEqual({ code: 0, signal: null });
    expect(processes[0]?.inputEnded).toBe(true);
    expect(processes[0]?.killed).toBe(false);
    const second = server.start("/data/1");
    await settle();
    processes[1]?.write(LISTENING);
    await second;
    expect(commands.map(({ args }) => args[args.indexOf("--data-dir") + 1])).toEqual([
      "/data/0",
      "/data/1",
    ]);
    expect(server.starts).toBe(2);
  });

  it("kills a server that outlives its input's end by 10 s", async () => {
    const { server, processes, waits } = harness(false);
    const started = server.start("/data/0");
    await settle();
    processes[0]?.write(LISTENING);
    await started;
    const stopped = server.stop();
    await settle();
    expect(processes[0]?.killed).toBe(false);
    waits.fire(STOP_WAIT_MS);
    await settle();
    expect(processes[0]?.killed).toBe(true);
    processes[0]?.exit({ code: null, signal: "SIGTERM" });
    await expect(stopped).resolves.toEqual({ code: null, signal: "SIGTERM" });
  });

  it("refuses a server that exits before listening, or stays silent", async () => {
    const exits = harness();
    const exited = exits.server.start("/data/0");
    await settle();
    exits.processes[0]?.exit({ code: 1, signal: null });
    await expect(exited).rejects.toBeInstanceOf(ServerError);
    expect(exits.server.running).toBe(false);

    const silent = harness();
    const quiet = silent.server.start("/data/0");
    await settle();
    expect(silent.waits.pending).toContain(LISTEN_WAIT_MS);
    silent.waits.fire(LISTEN_WAIT_MS);
    await expect(quiet).rejects.toThrow(/no listening line/);
    expect(silent.processes[0]?.inputEnded).toBe(true);
  });

  it("refuses a second start while one runs", async () => {
    const { server, processes } = harness();
    const started = server.start("/data/0");
    await settle();
    processes[0]?.write(LISTENING);
    await started;
    await expect(server.start("/data/1")).rejects.toBeInstanceOf(ServerError);
  });
});
