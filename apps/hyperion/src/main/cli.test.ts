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

/** A served world's universe and body, as place-ship's placement names them (R10.T15.b). */
const UNIVERSE = "3504954ff4feabf4";
const BODY = "61ffd967fe000023.0100";

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
      "--weathered",
      "--atmosphere",
      "--inspection",
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
      world: "test-planet",
      style: null,
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
      world: "test-planet",
      style: null,
    });
  });

  it("measures the terrain world's sizes with --sizes on", () => {
    expect(
      parse(["--descent-spike", "--world", "terrain-test", "--smoke", "--sizes", "on"]).spike,
    ).toMatchObject({ world: "terrain-test", sizes: "on" });
  });

  it.each([[[]], [["--sizes", "off"]]] as const)(
    "holds no sizes in the launch with %j, so that it is as before the option",
    (sizes) => {
      expect(
        parse(["--descent-spike", "--world", "terrain-test", "--smoke", ...sizes]).spike,
      ).not.toHaveProperty("sizes");
    },
  );

  it("skips the horizon map with --horizon off, the A/B's (R10.T15.b)", () => {
    expect(
      parse(["--descent-spike", "--world", "terrain-test", "--horizon", "off"]).spike,
    ).toMatchObject({ world: "terrain-test", style: "photorealistic", horizon: "off" });
  });

  it.each([[[]], [["--horizon", "on"]]] as const)(
    "holds no horizon in the launch with %j, so that it is as before the option",
    (horizon) => {
      expect(
        parse(["--descent-spike", "--world", "terrain-test", ...horizon]).spike,
      ).not.toHaveProperty("horizon");
    },
  );

  it("takes --sizes off over the test planet, which measures nothing", () => {
    const { spike } = parse(["--descent-spike", "--sizes", "off"]);
    expect(spike).toMatchObject({ world: "test-planet" });
    expect(spike).not.toHaveProperty("sizes");
  });

  it.each([
    [[], "photorealistic"],
    [["--style", "photorealistic"], "photorealistic"],
    [["--style", "wireframe"], "wireframe"],
  ] as const)("flies the terrain world with %j, in the style %s", (style, drawn) => {
    expect(
      parse(["--descent-spike", "--world", "terrain-test", "--ridged", "off", ...style]).spike,
    ).toMatchObject({ world: "terrain-test", style: drawn, ridged: "off" });
  });

  it("flies a served world named by its universe and body (R10.T15.b)", () => {
    const { spike } = parse([
      "--descent-spike",
      "--world",
      "served",
      "--universe",
      UNIVERSE,
      "--body",
      BODY,
      "--style",
      "wireframe",
      "--sizes",
      "on",
    ]);
    expect(spike).toMatchObject({
      world: "served",
      served: { universe: UNIVERSE, body: BODY },
      style: "wireframe",
      sizes: "on",
    });
  });

  it("names no served world on any other world's launch", () => {
    expect(parse(["--descent-spike", "--world", "terrain-test"]).spike).not.toHaveProperty(
      "served",
    );
  });

  it("takes the test planet by its name, recording no style for its SPIKE LIT main view", () => {
    expect(parse(["--descent-spike", "--world", "test-planet"]).spike).toMatchObject({
      world: "test-planet",
      style: null,
    });
  });

  it.each([
    [["--style", "wireframe"], "--style is an option of a generated world"],
    [["--world", "test-planet", "--style", "photorealistic"], "SPIKE LIT"],
    [["--world", "terrain-test", "--ridged", "on"], "the ridges are the test planet's"],
    [
      ["--world", "terrain-test", "--vertex-path", "face-differences"],
      "--vertex-path does not fit",
    ],
    [["--world", "terrain-test", "--normals", "mesh"], "--normals does not fit"],
    [["--sizes", "on"], "--sizes on is an option of a generated world"],
    [["--world", "served", "--universe", UNIVERSE], "--world served needs --universe and --body"],
    [["--world", "served", "--body", BODY], "--world served needs --universe and --body"],
    [["--universe", UNIVERSE, "--body", BODY], "--universe is an option of --world served"],
    [["--world", "terrain-test", "--body", BODY], "--body is an option of --world served"],
    [
      ["--world", "served", "--universe", UNIVERSE, "--body", BODY, "--ridged", "on"],
      "--ridged on does not fit --world served",
    ],
    [
      ["--world", "served", "--universe", UNIVERSE, "--body", BODY, "--normals", "mesh"],
      "--normals does not fit --world served",
    ],
    [["--world", "test-planet", "--sizes", "on"], "the terrain session's caches"],
    [["--horizon", "off"], "the test planet's bakes make no horizon map"],
    [
      ["--world", "terrain-test", "--style", "wireframe", "--horizon", "off"],
      "the wireframe's bakes skip the horizon map already",
    ],
  ])("refuses %j on its world", (args, reason) => {
    const error = refusal(["--descent-spike", ...args]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain(reason);
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
    ["--world", "earth"],
    ["--style", "lit"],
    ["--sizes", "yes"],
    ["--horizon", "skipped"],
    ["--universe", "Dev Fixture"],
    ["--universe", "00000000000004D2"],
    ["--body", "61ffd967fe000023"],
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
    ["--world", "terrain-test"],
    ["--style", "wireframe"],
    ["--sizes", "on"],
    ["--horizon", "off"],
    ["--universe", "3504954ff4feabf4"],
    ["--body", "61ffd967fe000023.0100"],
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
    expect(args.viewsCheck).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "several-views",
      fixture: "terrain-test",
    });
    expect(args.spike).toBeUndefined();
  });

  it("takes the setting, the smoke and the directory it shares with the spike", () => {
    expect(
      parse(["--views-check", "--setting", "low", "--smoke", "--out", "/data/out"]).viewsCheck,
    ).toEqual({
      setting: "low",
      smoke: true,
      out: "/data/out",
      run: "several-views",
      fixture: "terrain-test",
    });
  });

  it("takes --terrain, the terrain run (R10.T16.d), at its setting", () => {
    expect(parse(["--views-check", "--terrain", "--setting", "low", "--smoke"]).viewsCheck).toEqual(
      { setting: "low", smoke: true, out: null, run: "terrain", fixture: "terrain-test" },
    );
  });

  it("takes --handover, the terrain run's hand-over check (R10.T10.g), beside --terrain", () => {
    expect(
      parse(["--views-check", "--terrain", "--handover", "--setting", "low"]).viewsCheck,
    ).toEqual({
      setting: "low",
      smoke: false,
      out: null,
      run: "handover",
      fixture: "terrain-test",
    });
  });

  it("checks the hand-over on a served world named by its universe and body (R10.T15.b)", () => {
    expect(
      parse(["--views-check", "--terrain", "--handover", "--universe", UNIVERSE, "--body", BODY])
        .viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "handover",
      fixture: "terrain-test",
      served: { universe: UNIVERSE, body: BODY },
    });
  });

  it.each([
    [["--terrain", "--universe", UNIVERSE, "--body", BODY], "options of --terrain --handover"],
    [["--universe", UNIVERSE, "--body", BODY], "options of --terrain --handover"],
    [["--terrain", "--handover", "--universe", UNIVERSE], "needs --universe and --body"],
    [["--terrain", "--handover", "--body", BODY], "needs --universe and --body"],
    [
      ["--terrain", "--handover", "--weathered", "--universe", UNIVERSE, "--body", BODY],
      "--weathered does not fit a served world",
    ],
  ])("refuses a served world on %j", (args, reason) => {
    const error = refusal(["--views-check", ...args]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain(reason);
  });

  it.each([
    [["--views-check", "--terrain", "--weathered"], "terrain"],
    [["--views-check", "--terrain", "--handover", "--weathered"], "handover"],
  ] as const)(
    "takes --weathered, TEST MOON weathered's field (R10.T1.c), beside --terrain: %j",
    (args, run) => {
      expect(parse(args).viewsCheck).toEqual({
        setting: "high",
        smoke: false,
        out: null,
        run,
        fixture: "terrain-test-weathered",
      });
    },
  );

  it("takes --whole <FILE>, TEST MOON's whole body (R10.T14.c), beside --terrain", () => {
    expect(
      parse(["--views-check", "--terrain", "--whole", "/tmp/whole.golden"]).viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "terrain",
      fixture: "terrain-test-whole",
      wholeField: "/tmp/whole.golden",
    });
  });

  it.each([
    [
      ["--views-check", "--whole", "f"],
      "error: --whole is an option of --terrain, which was not given",
    ],
    [
      ["--views-check", "--terrain", "--weathered", "--whole", "f"],
      "error: --whole and --weathered are separate fields; give one",
    ],
    [
      ["--views-check", "--terrain", "--handover", "--whole", "f"],
      "error: --whole is an option of the terrain run, not its hand-over check: the whole body has no survey edge to hand over across",
    ],
  ] as const)("refuses --whole outside the plain terrain run: %j", (args, message) => {
    const error = refusal(args);
    expect([error.exitCode, error.message]).toEqual([1, message]);
  });

  it.each([
    [["--views-check", "--weathered"]],
    [["--views-check", "--weathered", "--smoke"]],
    [["--weathered"]],
    [["--descent-spike", "--weathered"]],
  ])("refuses --weathered without --terrain: %j", (args) => {
    const error = refusal(args);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --weathered is an option of --terrain, which was not given",
    ]);
  });

  it.each([
    [["--views-check", "--handover"]],
    [["--handover"]],
    [["--descent-spike", "--handover"]],
  ])("refuses --handover without --terrain: %j", (args) => {
    const error = refusal(args);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("--handover is an option of --terrain");
  });

  it.each([[["--terrain"]], [["--descent-spike", "--terrain"]], [["--terrain", "--smoke"]]])(
    "refuses --terrain without --views-check: %j",
    (args) => {
      const error = refusal(args);
      expect(error.exitCode).toBe(1);
      expect(error.message).toContain("--terrain is an option of --views-check");
    },
  );

  it("takes --atmosphere, the atmosphere run (R08.T10.b), at its setting", () => {
    expect(
      parse(["--views-check", "--atmosphere", "--setting", "low", "--smoke"]).viewsCheck,
    ).toEqual({
      setting: "low",
      smoke: true,
      out: null,
      run: "atmosphere",
      fixture: "terrain-test",
    });
  });

  it("gives an --atmosphere launch its own setting", () => {
    expect(parse(["--views-check", "--atmosphere", "--setting", "low"]).setting).toBe("low");
  });

  it.each([
    [["--atmosphere"]],
    [["--atmosphere", "--smoke"]],
    [["--descent-spike", "--atmosphere"]],
  ])("refuses --atmosphere without --views-check: %j", (args) => {
    const error = refusal(args);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --atmosphere is an option of --views-check, which was not given",
    ]);
  });

  it.each([
    [["--views-check", "--atmosphere", "--terrain"]],
    [["--views-check", "--atmosphere", "--terrain", "--handover"]],
    [["--views-check", "--atmosphere", "--terrain", "--weathered"]],
    [["--atmosphere", "--terrain"]],
  ])("refuses --atmosphere with --terrain, a separate run: %j", (args) => {
    const error = refusal(args);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --atmosphere and --terrain are separate runs; give one",
    ]);
  });

  it.each([
    [["--views-check", "--atmosphere", "--handover"], "--handover"],
    [["--views-check", "--atmosphere", "--weathered"], "--weathered"],
  ] as const)("refuses %j as an option of a --terrain that was not given", (args, option) => {
    const error = refusal(args);
    expect([error.exitCode, error.message]).toEqual([
      1,
      `error: ${option} is an option of --terrain, which was not given`,
    ]);
  });

  it("takes --inspection, the inspection run (the owner's request), with its body", () => {
    expect(
      parse(["--views-check", "--inspection", "--universe", UNIVERSE, "--body", BODY]).viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "inspection",
      fixture: "terrain-test",
      served: { universe: UNIVERSE, body: BODY },
    });
  });

  it("takes --look, the hand-over check's one look (R10.T15.b.2)", () => {
    expect(
      parse([
        "--views-check",
        "--terrain",
        "--handover",
        "--universe",
        UNIVERSE,
        "--body",
        BODY,
        "--look",
        "handover-terminator",
      ]).viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "handover",
      fixture: "terrain-test",
      served: { universe: UNIVERSE, body: BODY },
      look: "handover-terminator",
    });
  });

  it("takes --look handover-halo on a weathered launch, the halo's look no run takes unnamed (R10.T1.c)", () => {
    expect(
      parse(["--views-check", "--terrain", "--handover", "--weathered", "--look", "handover-halo"])
        .viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "handover",
      fixture: "terrain-test-weathered",
      look: "handover-halo",
    });
  });

  it("refuses --look handover-halo on a served world, whose crater it does not name", () => {
    const error = refusal([
      "--views-check",
      "--terrain",
      "--handover",
      "--universe",
      UNIVERSE,
      "--body",
      BODY,
      "--look",
      "handover-halo",
    ]);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --look handover-halo centres a crater of TERRAIN TEST's moon, not a served world's",
    ]);
  });

  it("refuses --look without --handover", () => {
    const error = refusal(["--views-check", "--terrain", "--look", "handover-30"]);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --look is an option of --handover, which was not given",
    ]);
  });

  it("refuses a look the hand-over check does not make", () => {
    const error = refusal(["--views-check", "--terrain", "--handover", "--look", "handover-45"]);
    expect(error.exitCode).toBe(1);
    expect(error.message).toContain("handover-45");
  });

  it("has the terrain run record its GPU time by pass with --hyperion-gpu-timing (R14's Mac measurements)", () => {
    expect([
      parse(["--views-check", "--terrain", "--hyperion-gpu-timing"]).viewsCheck,
      parse(["--views-check", "--terrain"]).viewsCheck?.passTimes,
      parse(["--views-check", "--hyperion-gpu-timing"]).viewsCheck?.passTimes,
      parse(["--views-check", "--terrain", "--handover", "--hyperion-gpu-timing"]).viewsCheck
        ?.passTimes,
    ]).toEqual([
      {
        setting: "high",
        smoke: false,
        out: null,
        run: "terrain",
        fixture: "terrain-test",
        passTimes: true,
      },
      undefined,
      undefined,
      undefined,
    ]);
  });

  it("takes --timing, the inspection run's frame profile (the owner's report)", () => {
    expect(
      parse(["--views-check", "--inspection", "--universe", UNIVERSE, "--body", BODY, "--timing"])
        .viewsCheck,
    ).toEqual({
      setting: "high",
      smoke: false,
      out: null,
      run: "inspection",
      fixture: "terrain-test",
      served: { universe: UNIVERSE, body: BODY },
      timing: true,
    });
  });

  it.each(["sky-off", "transmittance-off", "per-pixel", "surface-grey"] as const)(
    "takes --composite-probe %s, the frame profile's probe of the composite (the owner's priority)",
    (probe) => {
      expect(
        parse([
          "--views-check",
          "--inspection",
          "--universe",
          UNIVERSE,
          "--body",
          BODY,
          "--timing",
          "--composite-probe",
          probe,
        ]).viewsCheck,
      ).toEqual({
        setting: "high",
        smoke: false,
        out: null,
        run: "inspection",
        fixture: "terrain-test",
        served: { universe: UNIVERSE, body: BODY },
        timing: true,
        compositeProbe: probe,
      });
    },
  );

  it.each([
    [["--views-check", "--inspection", "--universe", UNIVERSE, "--body", BODY]],
    [["--composite-probe", "sky-off"]],
  ] as const)("refuses --composite-probe without the frame profile after %j", (args) => {
    const error = refusal([...args, "--composite-probe", "sky-off"]);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --composite-probe is an option of --timing, which was not given",
    ]);
  });

  it("refuses a --composite-probe that names no probe", () => {
    const error = refusal([
      "--views-check",
      "--inspection",
      "--universe",
      UNIVERSE,
      "--body",
      BODY,
      "--timing",
      "--composite-probe",
      "march-off",
    ]);
    expect(error.exitCode).toBe(1);
  });

  it.each([
    [
      ["--views-check", "--timing"],
      "error: --timing is an option of --inspection, which was not given",
    ],
    [
      ["--views-check", "--atmosphere", "--timing"],
      "error: --timing is an option of --inspection, which was not given",
    ],
  ] as const)("refuses the frame profile without the inspection run %j", (args, message) => {
    const error = refusal([...args]);
    expect([error.exitCode, error.message]).toEqual([1, message]);
  });

  it.each([
    [["--views-check", "--inspection"], "error: the inspection run needs --universe and --body"],
    [
      ["--views-check", "--inspection", "--universe", UNIVERSE],
      "error: the inspection run needs --universe and --body",
    ],
    [
      ["--inspection", "--universe", UNIVERSE, "--body", BODY],
      "error: --inspection is an option of --views-check, which was not given",
    ],
    [
      ["--views-check", "--inspection", "--atmosphere"],
      "error: --inspection is a run of its own; give it alone",
    ],
    [
      ["--views-check", "--inspection", "--terrain", "--universe", UNIVERSE, "--body", BODY],
      "error: --inspection is a run of its own; give it alone",
    ],
  ] as const)("refuses the inspection run %j", (args, message) => {
    const error = refusal([...args]);
    expect([error.exitCode, error.message]).toEqual([1, message]);
  });

  it("draws the check's page at a device-pixel ratio of 1 unless --device-pixel-ratio is given", () => {
    expect([
      parse(["--views-check"]).viewsCheckPixelRatio,
      parse(["--views-check", "--device-pixel-ratio", "2"]).viewsCheckPixelRatio,
      parse([
        "--views-check",
        "--inspection",
        "--universe",
        UNIVERSE,
        "--body",
        BODY,
        "--device-pixel-ratio=3",
      ]).viewsCheckPixelRatio,
      parse([]).viewsCheckPixelRatio,
    ]).toEqual([1, 2, 3, undefined]);
  });

  it.each([["0"], ["1.5"], ["4"], ["two"]])("refuses a device-pixel ratio of %s", (ratio) => {
    expect(refusal(["--views-check", "--device-pixel-ratio", ratio]).exitCode).toBe(1);
  });

  it.each([[["--device-pixel-ratio", "2"]], [["--descent-spike", "--device-pixel-ratio", "2"]]])(
    "refuses --device-pixel-ratio without --views-check: %j",
    (args) => {
      expect(refusal(args).message).toBe(
        "error: --device-pixel-ratio is an option of --views-check, which was not given",
      );
    },
  );

  it("refuses --atmosphere with the descent spike as separate runs", () => {
    const error = refusal(["--views-check", "--atmosphere", "--descent-spike"]);
    expect(error.message).toBe(
      "error: --views-check and --descent-spike are separate runs; give one",
    );
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
    ["--world", "terrain-test"],
    ["--style", "wireframe"],
    ["--sizes", "on"],
    ["--horizon", "off"],
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

describe("the geometry override (plan R14, Design note 17)", () => {
  beforeEach(() => {
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockImplementation(() => true);
    }
  });

  it("takes --geometry on a spike's and a check's launch, its fields over the setting's", () => {
    expect([
      parse(["--descent-spike", "--geometry", "clusterPath=false"]).geometry,
      parse([
        "--views-check",
        "--terrain",
        "--geometry=casterStrideCap=1,overlayDepth=redraw,casterDepthCull=false",
      ]).geometry,
      parse(["--descent-spike", "--geometry", "farCascadeRate=1"]).geometry,
      parse(["--descent-spike"]).geometry,
      parse(["--views-check"]).geometry,
    ]).toEqual([
      { clusterPath: false },
      { casterStrideCap: 1, overlayDepth: "redraw", casterDepthCull: false },
      { farCascadeRate: 1 },
      undefined,
      undefined,
    ]);
  });

  it("refuses --geometry on an ordinary launch, as an option of the spike and the check", () => {
    const error = refusal(["--geometry", "clusterPath=false"]);
    expect([error.exitCode, error.message]).toEqual([
      1,
      "error: --geometry is an option of --descent-spike or --views-check, neither of which was given",
    ]);
  });

  it.each([
    ["clusterpath=false", "is not <field>=<value>"],
    ["clusterPath", "is not <field>=<value>"],
    ["clusterPath=no", 'clusterPath is true or false, not "no"'],
    ["casterStrideCap=3", 'casterStrideCap is 1, 2, 4 or 8, not "3"'],
    ["overlayDepth=scene", "overlayDepth is redraw, scene-depth or cluster"],
    ["clusterPath=true,clusterPath=false", "clusterPath is given twice"],
    ["", "is not <field>=<value>"],
  ])("refuses --geometry %j as a usage error", (value, reason) => {
    const error = refusal(["--descent-spike", "--geometry", value]);
    expect([error.code, error.message.includes(reason)]).toEqual([
      "commander.invalidArgument",
      true,
    ]);
  });
});
