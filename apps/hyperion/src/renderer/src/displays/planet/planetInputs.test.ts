import { describe, expect, it } from "vitest";

import {
  DEFAULT_INPUTS,
  formatInput,
  formatInputsString,
  fromInputsDto,
  inputSpec,
  parseEntry,
  parseInputsString,
  plainDecimal,
  type PlanetInputs,
  refusedInput,
  toInputsDto,
} from "./planetInputs";

/** The sim's `EARTH` of `planetary::viewer::tests`, whose canonical text that test pins. */
const SIM_EARTH: PlanetInputs = {
  seed: "000000000e150001",
  values: {
    starMassMsun: 1,
    feHDex: 0,
    ageGyr: 4.57,
    smaAu: 1,
    eccentricity: 0.0167,
    formedAtAu: null,
    massMearth: 1,
    dayHours: null,
    tiltDeg: null,
    waterRank: null,
    carbonRank: null,
    nitrogenRank: null,
  },
};

describe("the inputs' string", () => {
  it("is the sim's canonical text, which the server hashes into the planet's ID", () => {
    expect(formatInputsString(SIM_EARTH)).toBe(
      "PV1;seed=000000000e150001;star_mass_msun=1;fe_h_dex=0;age_gyr=4.57;mass_mearth=1;" +
        "semi_major_axis_au=1;eccentricity=0.0167",
    );
    const given: PlanetInputs = {
      ...SIM_EARTH,
      values: {
        ...SIM_EARTH.values,
        feHDex: -0,
        massMearth: 0.0001,
        dayHours: 24.5,
        waterRank: 0.25,
      },
    };
    expect(formatInputsString(given)).toBe(
      "PV1;seed=000000000e150001;star_mass_msun=1;fe_h_dex=0;age_gyr=4.57;mass_mearth=0.0001;" +
        "semi_major_axis_au=1;eccentricity=0.0167;day_hours=24.5;water_rank=0.25",
    );
  });

  it("reads back to the same inputs", () => {
    const inputs: PlanetInputs = {
      ...DEFAULT_INPUTS,
      values: { ...DEFAULT_INPUTS.values, formedAtAu: 5.2, tiltDeg: 23.44, nitrogenRank: 0.731 },
    };
    expect(parseInputsString(formatInputsString(inputs))).toEqual({ ok: true, inputs });
  });

  it("refuses a string of another form, a missing input and a value out of range", () => {
    expect(parseInputsString("PV2;seed=1").ok).toBe(false);
    expect(parseInputsString("PV1;seed=1;star_mass_msun=1")).toEqual({
      ok: false,
      reason: "STRING INVALID: enter fe_h_dex=",
    });
    const far = formatInputsString(DEFAULT_INPUTS).replace("eccentricity=0.0167", "eccentricity=2");
    expect(parseInputsString(far)).toEqual({
      ok: false,
      reason: "STRING INVALID: enter eccentricity= 0 to 0.9",
    });
    expect(parseInputsString(`${formatInputsString(DEFAULT_INPUTS)};warp=9`)).toEqual({
      ok: false,
      reason: "STRING INVALID: no input is named warp",
    });
  });
});

describe("an entry", () => {
  it("is read in the panel's unit into the wire's: a rotation in days into hours", () => {
    expect(parseEntry(inputSpec("dayHours"), "1.5")).toEqual({ ok: true, wire: 36 });
    expect(formatInput(inputSpec("dayHours"), 36)).toBe("1.5");
  });

  it("is refused outside its range with what is valid", () => {
    expect(parseEntry(inputSpec("starMassMsun"), "200")).toEqual({
      ok: false,
      reason: "INIT MASS INVALID: enter 0.08 to 150",
    });
    expect(parseEntry(inputSpec("eccentricity"), "abc").ok).toBe(false);
    expect(parseEntry(inputSpec("feHDex"), "-0.5")).toEqual({ ok: true, wire: -0.5 });
  });

  it("takes and shows digits grouped from five, and a rotation in days up to 1,000 d", () => {
    expect(parseEntry(inputSpec("smaAu"), "12,480")).toEqual({ ok: true, wire: 12_480 });
    expect(formatInput(inputSpec("smaAu"), 100_000)).toBe("100,000");
    expect(parseEntry(inputSpec("dayHours"), "1,001")).toEqual({
      ok: false,
      reason: "ROTATION INVALID: enter 0.0042 to 1000",
    });
  });

  it("is shown to its decimals, a positive [Fe/H] with its sign", () => {
    expect(formatInput(inputSpec("feHDex"), 0.25)).toBe("+0.25");
    expect(formatInput(inputSpec("feHDex"), -0)).toBe("0");
    expect(formatInput(inputSpec("massMearth"), 317.8)).toBe("317.8");
    expect(formatInput(inputSpec("eccentricity"), 0.016_71)).toBe("0.0167");
  });
});

describe("the wire form", () => {
  it("carries each input in its wire field, drawn ones null", () => {
    const dto = toInputsDto(DEFAULT_INPUTS);
    expect(dto.day_hours).toBeNull();
    expect(dto.semi_major_axis_au).toBe(1);
    expect(fromInputsDto(dto)).toEqual(DEFAULT_INPUTS);
  });

  it("names the input a refusal's field names", () => {
    expect(refusedInput("inputs.semi_major_axis_au")).toBe("smaAu");
    expect(refusedInput("inputs.seed")).toBe("seed");
    expect(refusedInput("draw_seed")).toBeNull();
  });
});

describe("plainDecimal", () => {
  it("writes no exponent, as Rust's Display writes none", () => {
    expect(plainDecimal(1e-7)).toBe("0.0000001");
    expect(plainDecimal(1.5e21)).toBe("1500000000000000000000");
    expect(plainDecimal(-0)).toBe("0");
  });
});
