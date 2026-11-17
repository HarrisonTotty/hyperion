import { describe, expect, it } from "vitest";

import {
  commandFixture,
  DESKTOP_SYS,
  fixtureCommands,
  fixtureFiles,
  type FixtureChanges,
  X1_YOGA_SYS,
} from "./fixtures/sysfs";
import {
  checkQuiet,
  LOAD_UNCHECKED,
  parsePmsetBatt,
  parsePmsetLowPower,
  parseTlpMode,
  type PowerSupply,
  type PreconditionSources,
  QUIET_LIMITS,
  supplyOf,
} from "./preconditions";

const COMMANDS = {
  "tlp-stat -s": commandFixture("tlp-stat-s.txt"),
  "pmset -g batt": commandFixture("pmset-batt-ac.txt"),
  "pmset -g therm": commandFixture("pmset-therm-nominal.txt"),
  "pmset -g": commandFixture("pmset-g.txt"),
};

interface Machine {
  readonly sysRoot?: string;
  readonly platform?: NodeJS.Platform;
  readonly load?: number;
  readonly changes?: FixtureChanges;
  readonly commands?: Readonly<Record<string, string>>;
  /** Each wait's milliseconds, recorded. */
  readonly waits?: number[];
  /** Called after each wait, to cool the package. */
  readonly afterWait?: () => void;
}

function machine({
  sysRoot = X1_YOGA_SYS,
  platform = "linux",
  load = 0.4,
  changes = {},
  commands = COMMANDS,
  waits = [],
  afterWait = (): void => undefined,
}: Machine = {}): PreconditionSources {
  return {
    platform,
    files: fixtureFiles(sysRoot, changes),
    sysRoot,
    run: fixtureCommands(commands),
    loadavg: () => [load, load, load],
    sleep: (ms) => {
      waits.push(ms);
      afterWait();
      return Promise.resolve();
    },
  };
}

const PKG_TEMP = `${X1_YOGA_SYS}/class/thermal/thermal_zone5/temp`;
const AC_ONLINE = `${X1_YOGA_SYS}/class/power_supply/AC/online`;

describe("checkQuiet on the UHD 620 laptop", () => {
  it("passes on AC, quiet and cool, and records the power profile as found", async () => {
    const report = await checkQuiet(machine());
    expect(report.quiet).toBe(true);
    expect(report.refusals).toEqual([]);
    expect(report.state).toEqual({
      supply: "mains",
      profile: {
        tlpMode: "performance/AC",
        governor: "powersave",
        epp: "balance_performance",
        noTurbo: false,
        pl1Watts: 15,
        pl2Watts: 51,
        tauSeconds: 28,
        raplZone: "intel-rapl-mmio:0",
        lowPowerMode: null,
      },
      loadAverage1m: 0.4,
      startPackageDegC: 52,
      temperatureSource: "x86_pkg_temp",
    });
  });

  it("refuses on battery, and names it", async () => {
    const report = await checkQuiet(machine({ changes: { replaced: { [AC_ONLINE]: "0\n" } } }));
    expect(report.quiet).toBe(false);
    expect(report.state.supply).toBe("battery");
    expect(report.refusals).toEqual(["on battery (AC offline); Design note 7 asks for AC"]);
  });

  it("refuses at a load average of 1.0, and names it", async () => {
    const report = await checkQuiet(machine({ load: 1 }));
    expect(report.quiet).toBe(false);
    expect(report.refusals).toHaveLength(1);
    expect(report.refusals).toEqual([
      "load average 1.00 at the start (Design note 7 asks below 1.0)",
    ]);
  });

  it("refuses above 60 °C after waiting five minutes for the package to cool, and names it", async () => {
    const waits: number[] = [];
    const report = await checkQuiet(
      machine({ changes: { replaced: { [PKG_TEMP]: "61000\n" } }, waits }),
    );
    expect(report.quiet).toBe(false);
    expect(waits.reduce((a, b) => a + b, 0)).toBe(QUIET_LIMITS.coolWaitMs);
    expect(report.cooledMs).toBe(QUIET_LIMITS.coolWaitMs);
    expect(report.refusals).toEqual([
      "package at 61.0 °C after 300 s (Design note 7 asks below 60 °C)",
    ]);
  });

  it("starts once the package has cooled below 60 °C", async () => {
    const replaced: Record<string, string> = { [PKG_TEMP]: "64000\n" };
    let waited = 0;
    const report = await checkQuiet(
      machine({
        changes: { replaced },
        afterWait: () => {
          waited += 1;
          if (waited === 3) {
            replaced[PKG_TEMP] = "59000\n";
          }
        },
      }),
    );
    expect(report.quiet).toBe(true);
    expect(report.cooledMs).toBe(3 * QUIET_LIMITS.coolPollMs);
    expect(report.state.startPackageDegC).toBe(59);
  });

  it("names every precondition that fails", async () => {
    const report = await checkQuiet(
      machine({
        load: 2.5,
        changes: { replaced: { [AC_ONLINE]: "0\n", [PKG_TEMP]: "70000\n" } },
      }),
    );
    expect(report.refusals).toHaveLength(3);
  });

  it("does not read as on battery when a USB-C source is offline beside Mains", async () => {
    // Both UCSI sources read 0 in the tree; the Mains supply is online.
    const report = await checkQuiet(machine());
    expect(report.state.supply).toBe("mains");
  });

  it("reads TLP's mode as null where tlp-stat does not run", async () => {
    const { "tlp-stat -s": _, ...withoutTlp } = COMMANDS;
    const report = await checkQuiet(machine({ commands: withoutTlp }));
    expect(report.state.profile.tlpMode).toBeNull();
    expect(report.quiet).toBe(true);
  });
});

describe("checkQuiet on the development desktop", () => {
  it("passes as a desktop: the mouse's battery is ignored, and Tctl stands in", async () => {
    const report = await checkQuiet(machine({ sysRoot: DESKTOP_SYS, commands: {} }));
    expect(report.quiet).toBe(true);
    expect(report.state).toEqual({
      supply: "desktop",
      profile: {
        tlpMode: null,
        governor: "schedutil",
        epp: null,
        noTurbo: null,
        pl1Watts: null,
        pl2Watts: null,
        tauSeconds: null,
        raplZone: "intel-rapl:0",
        lowPowerMode: null,
      },
      loadAverage1m: 0.4,
      startPackageDegC: 45.25,
      temperatureSource: "k10temp Tctl",
    });
  });
});

describe("checkQuiet off Linux", () => {
  it("reads macOS's AC, Low Power Mode and thermal state from pmset", async () => {
    const report = await checkQuiet(machine({ platform: "darwin" }));
    expect(report.quiet).toBe(true);
    expect(report.state.supply).toBe("mains");
    expect(report.state.profile.lowPowerMode).toBe(false);
    expect(report.state.temperatureSource).toBe("pmset -g therm");
    expect(report.state.startPackageDegC).toBeNull();
  });

  it("refuses macOS on battery or under thermal pressure", async () => {
    const report = await checkQuiet(
      machine({
        platform: "darwin",
        commands: {
          ...COMMANDS,
          "pmset -g batt": commandFixture("pmset-batt-battery.txt"),
          "pmset -g therm": commandFixture("pmset-therm-pressure.txt"),
        },
      }),
    );
    expect(report.state.supply).toBe("battery");
    expect(report.refusals).toEqual([
      "on battery (pmset: Battery Power); Design note 7 asks for AC",
      expect.stringMatching(
        /^thermal pressure after 300 s \(pmset -g therm: .*CPU_Speed_Limit = 71\)$/u,
      ),
    ]);
  });

  it("starts macOS once its thermal pressure has cleared", async () => {
    let reads = 0;
    const sources = machine({ platform: "darwin" });
    const report = await checkQuiet({
      ...sources,
      run: (file, args) => {
        if (args.join(" ") !== "-g therm") {
          return sources.run(file, args);
        }
        reads += 1;
        return Promise.resolve(
          commandFixture(reads < 3 ? "pmset-therm-pressure.txt" : "pmset-therm-nominal.txt"),
        );
      },
    });
    expect(report.quiet).toBe(true);
    expect(report.cooledMs).toBe(2 * QUIET_LIMITS.coolPollMs);
  });

  it("never reads Windows as quiet on os.loadavg()'s zeros", async () => {
    const report = await checkQuiet(machine({ platform: "win32", load: 0 }));
    expect(report.quiet).toBe(false);
    expect(report.refusals).toContain(LOAD_UNCHECKED);
    expect(report.state.loadAverage1m).toBeNull();
    expect(report.state.profile.governor).toBeNull();
  });
});

/** A power supply of `type`, its other files as `extra` gives them. */
function supply(type: string, extra: Partial<PowerSupply> = {}): PowerSupply {
  return { name: type, type, scope: null, online: null, status: null, ...extra };
}

describe("the supply", () => {
  it("ignores every USB source, online or not", () => {
    expect(supplyOf([supply("Mains", { online: 1 }), supply("USB", { online: 0 })]).supply).toBe(
      "mains",
    );
    expect(supplyOf([supply("Mains", { online: 0 }), supply("USB", { online: 1 })]).supply).toBe(
      "battery",
    );
  });

  it("reads a system battery's status where there is no Mains supply", () => {
    expect(supplyOf([supply("Battery", { status: "Discharging" })]).supply).toBe("battery");
    expect(supplyOf([supply("Battery", { status: "Charging" })]).supply).toBe("mains");
  });

  it("is desktop with no Mains supply and no system battery", () => {
    expect(supplyOf([supply("Battery", { scope: "Device", status: "Discharging" })])).toEqual({
      supply: "desktop",
      why: "no Mains supply and no system battery",
    });
  });
});

describe("the commands' parsers", () => {
  it("read TLP's mode in each version's form", () => {
    expect(parseTlpMode(commandFixture("tlp-stat-s.txt"))).toBe("performance/AC");
    expect(parseTlpMode(commandFixture("tlp-stat-s-1.6.txt"))).toBe("AC");
    expect(parseTlpMode("Power profile  = balanced/BAT (manual)\n")).toBe("balanced/BAT");
    expect(parseTlpMode("Error: missing root privilege.\n")).toBeNull();
  });

  it("read pmset's power source and Low Power Mode", () => {
    expect(parsePmsetBatt(commandFixture("pmset-batt-ac.txt"))?.supply).toBe("mains");
    expect(parsePmsetBatt("")).toBeNull();
    expect(parsePmsetLowPower(" lowpowermode         1\n")).toBe(true);
    expect(parsePmsetLowPower(" powermode            1\n")).toBe(true);
    expect(parsePmsetLowPower("")).toBeNull();
  });
});
