import { CommanderError } from "commander";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  type ClientArgs,
  DEFAULT_ADDRESS,
  DEFAULT_PORT,
  ENV_SERVER_ADDR,
  ENV_SERVER_PORT,
  parseClientArgs,
  serverUrlOf,
  userArgs,
} from "./cli";
import { DEFAULT_SPIKE_SEED } from "../preload/spikeLaunch";

const VERSION = "1.2.3";

/** Everything commander wrote to the terminal, which the tests capture instead of printing. */
const written: string[] = [];

function parse(args: readonly string[]): ClientArgs {
  return parseClientArgs(args, VERSION);
}

function url(args: readonly string[]): string {
  return serverUrlOf(parse(args));
}

/** The error `args` are refused with. */
function refusal(args: readonly string[]): CommanderError {
  try {
    parse(args);
  } catch (error) {
    if (error instanceof CommanderError) {
      return error;
    }
    throw error;
  }
  throw new Error(`${args.join(" ")} was accepted`);
}

describe("the client's command line", () => {
  beforeEach(() => {
    written.length = 0;
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockImplementation((chunk) => {
        written.push(String(chunk));
        return true;
      });
    }
    // A variable set where the tests run must not change their outcome.
    vi.stubEnv(ENV_SERVER_ADDR, undefined);
    vi.stubEnv(ENV_SERVER_PORT, undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("links to a server on this machine when given nothing", () => {
    expect(parse([])).toEqual({ address: DEFAULT_ADDRESS, port: DEFAULT_PORT, setting: "high" });
    expect(url([])).toBe("ws://127.0.0.1:7878/ws");
  });

  it.each([
    ["--address", "--port"],
    ["-a", "-p"],
  ])("reads the address and the port from %s and %s", (address, port) => {
    expect(url([address, "10.0.0.5", port, "9100"])).toBe("ws://10.0.0.5:9100/ws");
  });

  it("reads an option joined to its value by an equals sign", () => {
    expect(url(["--address=10.0.0.5", "--port=9100"])).toBe("ws://10.0.0.5:9100/ws");
  });

  it.each(["::1", "[::1]"])("accepts the IPv6 address %s", (address) => {
    expect(url([`--address=${address}`])).toBe("ws://[::1]:7878/ws");
  });

  it("accepts a host name, in the case a URL is written in", () => {
    expect(url(["--address=Bridge.Lan"])).toBe("ws://bridge.lan:7878/ws");
  });

  it("falls back to the variables", () => {
    vi.stubEnv(ENV_SERVER_ADDR, "10.0.0.5");
    vi.stubEnv(ENV_SERVER_PORT, "9100");

    expect(url([])).toBe("ws://10.0.0.5:9100/ws");
  });

  it("prefers an option given on the command line to its variable", () => {
    vi.stubEnv(ENV_SERVER_ADDR, "10.0.0.5");
    vi.stubEnv(ENV_SERVER_PORT, "9100");

    expect(url(["--address=192.168.1.2", "--port=7878"])).toBe("ws://192.168.1.2:7878/ws");
  });

  it.each(["ws://10.0.0.5", "10.0.0.5:7878", "10.0.0.5/ws", "127.1", "operator@10.0.0.5", ""])(
    "refuses the address %s, which is no bare address",
    (address) => {
      expect(refusal([`--address=${address}`]).code).toBe("commander.invalidArgument");
    },
  );

  it.each(["0", "65536", "-1", "1.5", "http"])("refuses the port %s", (port) => {
    expect(refusal([`--port=${port}`]).code).toBe("commander.invalidArgument");
  });

  it("names the variable a value it cannot use came from", () => {
    vi.stubEnv(ENV_SERVER_PORT, "http");

    expect(refusal([]).message).toContain(ENV_SERVER_PORT);
  });

  it("offers the help, which names every option, its default and its variable", () => {
    const help = refusal(["--help"]);

    expect([help.code, help.exitCode]).toEqual(["commander.helpDisplayed", 0]);
    const text = written.join("");
    for (const mention of [
      "--address",
      "--port",
      DEFAULT_ADDRESS,
      String(DEFAULT_PORT),
      ENV_SERVER_ADDR,
      ENV_SERVER_PORT,
    ]) {
      expect(text).toContain(mention);
    }
  });

  it("offers the version", () => {
    const version = refusal(["--version"]);

    expect([version.code, version.exitCode]).toEqual(["commander.version", 0]);
    expect(written.join("")).toContain(VERSION);
  });

  it("ignores the switches Chromium, Electron and electron-vite add", () => {
    expect(url(["--no-sandbox", "--inspect", "5858", "--port=9100", "."])).toBe(
      "ws://127.0.0.1:9100/ws",
    );
  });

  it("reports a usage error as an exit code of 1", () => {
    expect(refusal(["--port=0"]).exitCode).toBe(1);
  });
});

describe("the descent spike's options", () => {
  beforeEach(() => {
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockImplementation(() => true);
    }
  });

  it("gives no spike on an ordinary launch", () => {
    expect(parse([]).spike).toBeUndefined();
  });

  it("takes the flag alone with its defaults", () => {
    expect(parse(["--descent-spike"]).spike).toEqual({
      setting: "high",
      seed: DEFAULT_SPIKE_SEED,
      smoke: false,
      out: null,
      workers: null,
      vertexPath: null,
      normals: null,
      ridged: "off",
      dawnSafety: "on",
      capture: null,
      traceProfile: "off",
    });
  });

  it("takes every option", () => {
    expect(
      parse([
        "--descent-spike",
        "--setting",
        "low",
        "--seed",
        "18446744073709551615",
        "--smoke",
        "--out",
        "/data/out",
        "--workers",
        "3",
        "--vertex-path",
        "face-differences",
        "--normals",
        "mesh",
        "--ridged",
        "on",
        "--dawn-safety",
        "off",
        "--capture",
        "/data/capture",
        "--trace-profile",
        "on",
      ]).spike,
    ).toEqual({
      setting: "low",
      seed: "18446744073709551615",
      smoke: true,
      out: "/data/out",
      workers: 3,
      vertexPath: "face-differences",
      normals: "mesh",
      ridged: "on",
      dawnSafety: "off",
      capture: "/data/capture",
      traceProfile: "on",
    });
  });

  it.each([
    ["--setting", "medium"],
    ["--seed", "18446744073709551616"],
    ["--seed", "-1"],
    ["--seed", "07"],
    ["--workers", "0"],
    ["--workers", "65"],
    ["--vertex-path", "offsets"],
    ["--normals", "triple"],
    ["--ridged", "yes"],
    ["--dawn-safety", "maybe"],
    ["--trace-profile", "yes"],
  ])("refuses %s %s", (name, value) => {
    expect(refusal(["--descent-spike", name, value]).exitCode).toBe(1);
  });

  it("refuses baked offsets on the low setting, whose cache they do not fit", () => {
    const error = refusal([
      "--descent-spike",
      "--setting",
      "low",
      "--vertex-path",
      "baked-offsets",
    ]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("low setting");
  });

  it.each([
    ["--seed", "7"],
    ["--smoke"],
    ["--out", "/data/out"],
    ["--workers", "2"],
    ["--vertex-path", "baked-offsets"],
    ["--normals", "mesh"],
    ["--ridged", "on"],
    ["--dawn-safety", "off"],
    ["--capture", "/data/capture"],
    ["--trace-profile", "on"],
  ])("refuses %s without --descent-spike", (...option) => {
    const error = refusal(option);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("--descent-spike");
  });
});

describe("the several-views check's options", () => {
  beforeEach(() => {
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockImplementation(() => true);
    }
  });

  it("gives no check on an ordinary launch", () => {
    expect(parse([]).viewsCheck).toBeUndefined();
  });

  it("takes the flag alone with its defaults, and no spike", () => {
    const args = parse(["--views-check"]);
    expect(args.viewsCheck).toEqual({ setting: "high", smoke: false, out: null });
    expect(args.spike).toBeUndefined();
  });

  it("takes the setting, the smoke and the directory it shares with the spike", () => {
    expect(
      parse(["--views-check", "--setting", "low", "--smoke", "--out", "/data/out"]).viewsCheck,
    ).toEqual({ setting: "low", smoke: true, out: "/data/out" });
  });

  it("refuses to run with the descent spike", () => {
    const error = refusal(["--views-check", "--descent-spike"]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("separate runs");
  });

  it.each([
    ["--seed", "7"],
    ["--workers", "2"],
    ["--vertex-path", "face-differences"],
    ["--normals", "mesh"],
    ["--ridged", "on"],
    ["--dawn-safety", "off"],
    ["--capture", "/data/capture"],
  ])("refuses the spike's %s", (...option) => {
    const error = refusal(["--views-check", ...option]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("not --views-check");
  });

  it("names both runs for a shared option given without either", () => {
    expect(refusal(["--smoke"]).message).toContain("--descent-spike or --views-check");
  });
});

describe("the quality setting VIEW draws at (R07.T17)", () => {
  beforeEach(() => {
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockImplementation(() => true);
    }
  });

  it.each(["high", "low"] as const)("takes --setting %s on an ordinary launch", (setting) => {
    expect(parse(["--setting", setting])).toEqual({
      address: DEFAULT_ADDRESS,
      port: DEFAULT_PORT,
      setting,
    });
  });

  it.each([["--descent-spike"], ["--views-check"]])(
    "gives a %s launch the run's own setting",
    (flag) => {
      expect(parse([flag, "--setting", "low"]).setting).toBe("low");
    },
  );

  it("refuses a setting that is neither high nor low", () => {
    expect(refusal(["--setting", "medium"]).exitCode).toBe(1);
  });
});

describe("userArgs", () => {
  it("drops the entry Electron is launched with in development", () => {
    expect(userArgs(["/usr/bin/electron", ".", "--port", "9100"], false)).toEqual([
      "--port",
      "9100",
    ]);
  });

  it("drops only the executable of a packaged app", () => {
    expect(userArgs(["/opt/hyperion/hyperion", "--port", "9100"], true)).toEqual([
      "--port",
      "9100",
    ]);
  });
});
